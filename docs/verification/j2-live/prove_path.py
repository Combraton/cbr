#!/usr/bin/env python3
"""Prove the path a J2 live run took, from its stores, through the public client.

For every run in <out>/report.json:

  1. the store, opened read-only (m4e_run.evidence): every model_calls row
     (call kind, dialect, configured model, the model the provider says
     answered, status, usage), every ledger row, every sealed part record,
     the event types and command count;
  2. key-shaped strings over every file of the store, counted and never
     printed;
  3. a COPY of the store relaunched under the run's own production
     configuration with --replay-model (no transport, no credential read),
     then through the `cbr` client: `cbr packet` for baseline and assisted
     (the sealed packet digest must equal the one the harness wrote), and
     `cbr fetch` of the cited artifact (bytes must equal the input and its
     digest), and every carried excerpt of both packets compared with the
     fetched bytes at its stated range.

Nothing here reads the Keychain or opens a network socket: a replay launch
under a production configuration without --permit-model-network serves
from records. The store the harness wrote is never opened for writing.
"""

import argparse
import base64
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

# docs/verification/j2-live/ -> the repository root.
REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "scripts"))

import j2_run  # noqa: E402
from m4e_run import configuration, credential_file, evidence, records, spend, stop, cbr  # noqa: E402

KEY_SHAPES = {
    "jwt": re.compile(rb"eyJ[A-Za-z0-9_-]{20,}"),
    "sk": re.compile(rb"sk-[A-Za-z0-9_-]{20,}"),
    "bearer": re.compile(rb"Bearer [A-Za-z0-9._-]{20,}"),
    "keychain_service": re.compile(rb"minimax_api_key"),
}


def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def store_facts(data, model):
    facts = {}
    with evidence(data) as connection:
        calls = connection.execute(
            "SELECT call, dialect, model, length(sent), received FROM model_calls ORDER BY id"
        ).fetchall()
        rows = []
        for call, dialect, configured, sent_len, received in calls:
            answered = status = None
            usage = {}
            try:
                body = json.loads(bytes(received).decode("utf-8", "replace"))
                answered = body.get("model")
                status = body.get("status")
                usage = body.get("usage") or {}
            except (ValueError, AttributeError):
                pass
            rows.append(
                {
                    "call": call,
                    "dialect": dialect,
                    "configured_model": configured,
                    "answered_by": answered,
                    "status": status,
                    "input_tokens": usage.get("input_tokens"),
                    "output_tokens": usage.get("output_tokens"),
                    "sent_bytes": sent_len,
                    "received_bytes": len(bytes(received)),
                }
            )
        facts["model_calls"] = rows
        facts["model_calls_all_configured_model"] = all(r["configured_model"] == model for r in rows)
        facts["ledger"] = [
            {"kind": k, "tokens": t, "estimate": e}
            for k, t, e in connection.execute(
                "SELECT kind, tokens, estimate FROM model_ledger ORDER BY id"
            ).fetchall()
        ]
        events = connection.execute(
            "SELECT type, count(*) FROM events GROUP BY type ORDER BY min(sequence)"
        ).fetchall()
        facts["event_types"] = {t: n for t, n in events}
        facts["commands"] = connection.execute("SELECT count(*) FROM commands").fetchone()[0]
    total, charges = spend(data)
    facts["spent"] = total
    parts = [
        (identifier, record)
        for identifier, record in records(data)
        if record.get("question", {}).get("selector", "").startswith(j2_run.PART_SELECTOR)
    ]
    facts["part_records"] = [
        {
            "id": identifier,
            "selector": record.get("question", {}).get("selector"),
            "model": (record.get("model") or {}),
            "usage": record.get("usage"),
            "outcome": record.get("outcome", {}).get("kind")
            if isinstance(record.get("outcome"), dict)
            else record.get("outcome"),
        }
        for identifier, record in parts
    ]
    return facts


def key_scan(directory):
    counts = {name: 0 for name in KEY_SHAPES}
    files = 0
    for path in Path(directory).rglob("*"):
        if path.is_file():
            files += 1
            blob = path.read_bytes()
            for name, pattern in KEY_SHAPES.items():
                counts[name] += len(pattern.findall(blob))
    return {"files": files, "counts": counts}


def excerpts_of(packet):
    section = next((s for s in packet.get("sections", []) if s.get("item_id") == j2_run.ITEM), None)
    if section is None:
        return None, []
    read = j2_run.read_projection(section["content"])
    return section, [e for e in read["extents"] if e["carried"]]


def relaunch_and_read(run, out, provider, client, source, live=True):
    work = out / "work" / run["id"]
    copy = out / "proof" / run["id"]
    if copy.exists():
        shutil.rmtree(copy)
    copy.mkdir(parents=True)
    shutil.copytree(work / "data", copy / "data")
    shaped = {"id": run["id"], "model": run["model"]}
    config, body = configuration(copy, shaped, live, ["ids:u1"], replay=True)
    child, endpoint = j2_run.launch(
        provider, copy, config, ["--replay-model", "--model-run-ceiling", "1"]
    )
    credential = credential_file(copy, body)
    result = {}
    try:
        for arm in ("baseline", "assisted"):
            written = out / f"{run['id']}.{arm}.packet.json"
            if not written.exists():
                continue
            printed = cbr(client, endpoint, credential, ["packet", arm, "--excerpt", "10000000"])
            if printed.returncode != 0:
                result[arm] = {"error": printed.stderr.strip()}
                continue
            again = json.loads(printed.stdout)
            before = json.loads(written.read_text())
            packet_bytes = base64.b64decode(again["excerpt"]["data_base64"])
            packet = json.loads(packet_bytes)
            section, carried = excerpts_of(packet)
            cite = section["citations"][0]["evidence"]
            fetched_path = copy / f"{arm}.fetched"
            fetched = cbr(
                client,
                endpoint,
                credential,
                ["fetch", cite["artifact"]["id"], "--digest", cite["digest"], "--out", str(fetched_path)],
            )
            fetched_bytes = fetched_path.read_bytes() if fetched.returncode == 0 else None
            mismatched = [
                (e["start"], e["end"])
                for e in carried
                if fetched_bytes is None or fetched_bytes[e["start"] : e["end"]] != e["bytes"]
            ]
            result[arm] = {
                "packet_same_after_restart": again.get("excerpt") == before.get("excerpt"),
                "cited_artifact": cite["artifact"]["id"],
                "cited_digest": cite["digest"],
                "fetch_exit": fetched.returncode,
                "fetched_equals_input": fetched_bytes == source,
                "fetched_digest": sha(fetched_bytes) if fetched_bytes is not None else None,
                "excerpts_checked": len(carried),
                "excerpts_not_equal_to_fetched_bytes": mismatched,
            }
    finally:
        stop(child)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    parser.add_argument("--binaries", default=str(REPO / "target" / "debug"))
    parser.add_argument("--inputs", required=True, help="the directory holding the manifest's inputs")
    parser.add_argument("--dry", action="store_true", help="the out dir is a dry run (conformance configuration)")
    options = parser.parse_args()
    out = Path(options.out).resolve()
    report = json.loads((out / "report.json").read_text())
    provider = Path(options.binaries) / "cbr-provider"
    client = Path(options.binaries) / "cbr"
    proof = {
        "binaries": {p.name: sha(p.read_bytes()) for p in (provider, client)},
        "report_digest": sha((out / "report.json").read_bytes()),
        "runs": [],
    }
    for run in report["runs"]:
        source = (Path(options.inputs) / run["input"]["name"]).read_bytes()
        data = out / "work" / run["id"] / "data"
        entry = {
            "id": run["id"],
            "model": run["model"],
            "input_digest_matches": sha(source) == run["input"]["digest"],
            "store": store_facts(data, run["model"]),
            "key_scan": key_scan(out / "work" / run["id"]),
            "through_client": relaunch_and_read(
                {"id": run["id"], "model": run["model"]}, out, provider, client, source, not options.dry
            ),
        }
        entry["ledger_equals_report"] = entry["store"]["spent"] == run.get("tokens")
        proof["runs"].append(entry)
    (out / "proof.json").write_text(json.dumps(proof, indent=2, sort_keys=True))
    print(json.dumps(proof, indent=1)[:6000])


if __name__ == "__main__":
    main()
