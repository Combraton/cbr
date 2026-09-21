#!/usr/bin/env python3
"""The m4e run, as code, reviewed before it is ever run.

The first live run is the one thing in M4 that spends the owner's quota
on realistic requests and sends a third party's repository to a
provider. So it is not a sequence of commands typed on the day: it is
this file, reviewed at a head, with a dry-run mode that proves it end to
end against the fake transport and never opens a socket.

    python3 scripts/m4e_run.py --manifest M --out D --dry-run
    python3 scripts/m4e_run.py --manifest M --out D --live \\
        --permit-model-network
    python3 scripts/m4e_run.py --ambiguity DATA_DIR

**The live run is not authorised by this file existing.** `--live` needs
`--permit-model-network` typed as well, and the owner's word at the
time, which no flag can stand in for.

What it does, per run in the manifest:

  1. launches a provider over a throwaway data directory,
  2. registers the repository and submits one context request,
  3. polls until it settles, and writes the packet where the reviewer
     can score it,
  4. relaunches the same store with `--replay-model` and rebuilds the
     same question offline, comparing packet digests -- **the replay
     gate**,
  5. reports what was retained, what replayed, and every question the
     records disagree about, with the answers that disagree.

What it refuses, before anything starts:

  * a repository that is not one of the three the owner's word in
    READINESS section 7 covers, which are all public;
  * an output directory inside this repository, because a pilot's
    packet must never be committed;
  * `--live` without `--permit-model-network`;
  * a run ceiling above the 5,000,000 hard cap of READINESS section 9.

And it stops, mid-run, at 2,250,000 tokens -- the estimate plus half --
reporting what was spent and on what rather than quietly costing three
times what was predicted.

The report holds digests, paths, spans, counts and costs. **It holds no
repository text**, which is the licence rule for the pilots and is
checked by a test rather than remembered.
"""

import argparse
import base64
import json
import os
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parent

# READINESS section 9. The estimate is what m4e expects to spend; the
# stop is the estimate plus half, at which the run halts whatever state
# it is in; the ceiling is the hard cap, enforced by the ledger through
# `--model-run-ceiling` rather than by this script's intention.
ESTIMATE_TOKENS = 1_500_000
STOP_TOKENS = 2_250_000
RUN_CEILING_TOKENS = 5_000_000

# READINESS section 7: the owner's word covers these three and no
# others, and all three are public. A repository id outside this set is
# refused by name rather than by a flag somebody could pass.
PUBLIC_REPOSITORIES = ("cbr", "brian2", "knowscroll")

# The three models M4 may name (READINESS section 2). m4e runs the
# highspeed extraction model beside the synthesis one, which is what
# "MiniMax-M2.7-highspeed beside MiniMax-M3" means: the same question,
# twice, so the difference is the model and nothing else.
MODELS = ("MiniMax-M2.7-highspeed", "MiniMax-M2.7", "MiniMax-M3")

PRINCIPAL = "owner"
DRY_RUN_CREDENTIAL = "ccred1.owner.m4e-dry-run"

SETTLE_SECONDS = 900


class Refused(Exception):
    """A condition that stops the run before anything is spent."""


def refuse(why):
    raise Refused(why)


# ---- what may be run ----------------------------------------------------


def check(manifest, out, live, permit, ceiling):
    """Every refusal, before a provider is launched or a byte is sent."""
    if live and not permit:
        refuse(
            "--live needs --permit-model-network as well. A live run reads "
            "the owner's credential and opens a socket to a provider, and "
            "having built the harness is not permission to use it."
        )
    if ceiling > RUN_CEILING_TOKENS:
        refuse(
            f"--run-ceiling {ceiling} is above m4e's hard cap of "
            f"{RUN_CEILING_TOKENS}. Raising it is the owner's decision and "
            "not this script's."
        )
    out = Path(out).resolve()
    if out == REPOSITORY or REPOSITORY in out.parents:
        refuse(
            f"--out {out} is inside {REPOSITORY}. A pilot's packet holds a "
            "third party's repository text and must never be committed; "
            "the reviewer scores it where it is written, outside the tree."
        )
    runs = manifest.get("runs")
    if not isinstance(runs, list) or not runs:
        refuse("the manifest names no runs")
    seen = set()
    for run in runs:
        for member in ("id", "repository", "model", "task", "selector", "wants"):
            if member not in run:
                refuse(f"a run is missing {member!r}: {run.get('id', run)!r}")
        if run["id"] in seen:
            refuse(f"two runs share the id {run['id']!r}")
        seen.add(run["id"])
        if run["model"] not in MODELS:
            refuse(
                f"{run['id']}: {run['model']!r} is not one of the three "
                f"models M4 may name: {', '.join(MODELS)}"
            )
        identifier = run["repository"].get("id")
        if identifier not in PUBLIC_REPOSITORIES:
            refuse(
                f"{run['id']}: {identifier!r} is not one of the repositories "
                "the owner's word covers, which are the three public ones: "
                f"{', '.join(PUBLIC_REPOSITORIES)}. A private repository "
                "needs the owner's explicit word before a single byte of it "
                "is sent."
            )
        path = Path(run["repository"].get("path", "")).expanduser()
        if not (path / ".git").exists():
            refuse(f"{run['id']}: {path} is not a git checkout")
    return out


# ---- driving a provider -------------------------------------------------


def binaries(directory):
    provider = Path(directory) / "cbr-provider"
    client = Path(directory) / "cbr"
    for binary in (provider, client):
        if not binary.exists():
            refuse(f"{binary} is not built; cargo build --workspace first")
    return provider, client


def configuration(work, run, live, dry_answers):
    """The launch configuration, which decides what transport exists.

    A live run is a **production** configuration naming a
    `model_runtime`, which is the member a credential read comes from. A
    dry run is a **conformance** configuration carrying the fake, which
    a production launch refuses outright -- so the two can never be
    confused for one another by a flag.
    """
    path = work / "cbr.json"
    if live:
        body = {
            "format": "cbr-config/1",
            "principal": PRINCIPAL,
            "authority_principals": [PRINCIPAL],
            "model_runtime": {
                "provider": "minimax",
                "dialect": "responses",
                "model": run["model"],
            },
        }
    else:
        body = {
            "format": "combraton-conformance-config/1",
            "principal": PRINCIPAL,
            "authority_principals": [PRINCIPAL],
            "credentials": [{"credential": DRY_RUN_CREDENTIAL}],
            "context": {"compile": True},
            "model": {
                "dialect": "responses",
                "model": run["model"],
                "answers": dry_answers,
                "usage": 5000,
                "counting": "when_it_could_admit",
            },
        }
    path.write_text(json.dumps(body))
    return path


def launch(provider, work, run, config, extra):
    sockets = work / "s"
    sockets.mkdir(exist_ok=True)
    os.chmod(sockets, 0o700)
    endpoint = sockets / "cbr.sock"
    if endpoint.exists():
        endpoint.unlink()
    child = subprocess.Popen(
        [
            str(provider),
            "--data-dir",
            str(work / "data"),
            "--config",
            str(config),
            "--socket",
            str(endpoint),
            "--register-repository",
            f"{run['repository']['id']}={Path(run['repository']['path']).expanduser()}",
            *extra,
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    started = time.monotonic()
    while True:
        try:
            with socket.socket(socket.AF_UNIX) as probe:
                probe.connect(str(endpoint))
            break
        except OSError:
            if child.poll() is not None:
                stderr = child.stderr.read().decode(errors="replace")
                refuse(f"the provider refused to start: {stderr.strip()}")
            if time.monotonic() - started > 60:
                child.kill()
                refuse("the provider never listened")
            time.sleep(0.05)
    return child, endpoint


def stop(child):
    child.kill()
    child.wait()


def credential_file(work, live):
    if live:
        # A production launch issues its own credential at start, which
        # is the same path `scripts/debug_launch.sh` prints.
        return work / "data" / "credentials" / PRINCIPAL
    path = work / "credential"
    path.write_text(DRY_RUN_CREDENTIAL)
    return path


def cbr(client, endpoint, credential, arguments):
    return subprocess.run(
        [str(client), *arguments, "--socket", str(endpoint), "--credential-file", str(credential)],
        capture_output=True,
        text=True,
        check=False,
    )


def submit_and_settle(client, endpoint, credential, run, request):
    arguments = [
        "context",
        request,
        "--repo",
        str(Path(run["repository"]["path"]).expanduser()),
        "--repo-id",
        run["repository"]["id"],
        "--selector",
        run["selector"],
        "--task",
        run["task"],
        "--capacity",
        str(run.get("capacity", 262144)),
        "--investigation",
        str(run.get("investigation", 5)),
    ]
    for want in run["wants"]:
        arguments += ["--want", want]
    submitted = cbr(client, endpoint, credential, arguments)
    if submitted.returncode != 0:
        refuse(f"{run['id']}: submit failed: {submitted.stderr.strip()}")
    started = time.monotonic()
    while True:
        polled = cbr(client, endpoint, credential, ["request", request])
        if polled.returncode != 0:
            refuse(f"{run['id']}: inspect failed: {polled.stderr.strip()}")
        inspected = json.loads(polled.stdout)
        if inspected.get("state") != "preparing":
            return inspected
        if time.monotonic() - started > SETTLE_SECONDS:
            refuse(f"{run['id']}: {request} never left preparing")
        time.sleep(0.25)


def packet_digest(inspected):
    published = inspected.get("packets") or []
    if not published:
        return None
    return published[-1].get("reference", {}).get("artifact", {}).get("digest")


def sealed_sections(printed):
    """The sections of a sealed packet, out of a `context.packet.inspect`.

    **Not the packet's digest**, and the reason is worth stating where
    the comparison is made: a sealed packet names its own request and its
    own job, so two requests can never have equal bytes and a digest
    comparison between them would always differ. Byte identity across
    *two stores asking the same request* is the suite's own gate
    (`derivation_replay.rs`); what this gate asks of a real question is
    whether the rebuild reproduced the same content from the records.
    """
    excerpt = printed.get("excerpt", {}).get("data_base64")
    if not excerpt:
        return None
    packet = json.loads(base64.b64decode(excerpt))
    return json.dumps(packet.get("sections"), sort_keys=True)


def inspect_packet(client, endpoint, credential, run, request):
    fetched = cbr(
        client,
        endpoint,
        credential,
        ["packet", request, "--excerpt", "10000000"],
    )
    if fetched.returncode != 0:
        refuse(f"{run['id']}: packet failed: {fetched.stderr.strip()}")
    return fetched.stdout


def write_packet(client, endpoint, credential, out, run, request, digest):
    """Write the sealed packet where the reviewer can score it.

    Outside the repository, which `check` has already insisted on: the
    bytes are a pilot repository's text and the licence rule for the
    pilots is that only digests, paths, spans, counts and costs are
    committed.
    """
    if digest is None:
        return None, None
    destination = out / f"{run['id']}.packet.json"
    printed = inspect_packet(client, endpoint, credential, run, request)
    destination.write_text(printed)
    return str(destination), sealed_sections(json.loads(printed))


# ---- what the store says it spent, and what it retained -----------------


def spend(data):
    """Every charge in the ledger, and their total.

    `admitted_*` rows are notes about which path admitted a call, not
    charges, and are not counted -- counting them would double every
    call's cost.
    """
    connection = sqlite3.connect(data / "cbr.sqlite")
    try:
        rows = connection.execute(
            "SELECT kind, tokens FROM model_ledger ORDER BY id"
        ).fetchall()
    finally:
        connection.close()
    charges = [(kind, tokens) for kind, tokens in rows if not kind.startswith("admitted_")]
    return sum(tokens for _, tokens in charges), charges


def object_path(data, digest):
    hexed = digest.split(":", 1)[-1]
    return data / "objects" / "sha256" / hexed[0:2] / hexed[2:4] / hexed[4:]


def records(data):
    """Every sealed, unpurged derivation record, read from the store."""
    connection = sqlite3.connect(data / "cbr.sqlite")
    try:
        rows = connection.execute(
            "SELECT id, value FROM subjects "
            "WHERE kind = 'evidence.artifact' AND id LIKE 'der.%' ORDER BY id"
        ).fetchall()
    finally:
        connection.close()
    found = []
    for identifier, value in rows:
        artifact = json.loads(value)
        if artifact.get("state") != "sealed" or artifact.get("purge") is not None:
            continue
        path = object_path(data, artifact["descriptor"]["digest"])
        if not path.exists():
            continue
        found.append((identifier, json.loads(path.read_text())))
    return found


def ambiguity(data):
    """**The replay gate's own report: what could not be replayed, and why.**

    A rebuild answers a question only when every record covering it
    agrees, and declines otherwise. In a live run that state is ordinary
    rather than exotic: a call that timed out and a successful rerun of
    the same question leave two records that disagree, and the question
    is then unreplayable for ever.

    So this does not decide anything -- the provider's rebuild already
    did that, and the digests above say whether it worked. This
    *explains* it: it groups the records by the question they answer and
    names every question whose records differ, with the answers that
    differ. A gate that quietly reported "not retained" for these would
    be hiding the one failure the operator can actually act on.
    """
    questions = {}
    for identifier, record in records(data):
        digest = record.get("question", {}).get("digest", "")
        answer = json.dumps(record.get("answer"), sort_keys=True)
        questions.setdefault(digest, []).append(
            {
                "record": identifier,
                "selector": record.get("question", {}).get("selector", ""),
                "answer": answer,
                "made_at": record.get("made_at", ""),
            }
        )
    report = []
    for digest, holders in sorted(questions.items()):
        answers = sorted({holder["answer"] for holder in holders})
        if len(answers) == 1:
            continue
        report.append(
            {
                "question": digest,
                "selector": holders[0]["selector"],
                "records": sorted(holder["record"] for holder in holders),
                "answers": answers,
                "why": (
                    "two records covering this question answer it differently, "
                    "so a rebuild declines rather than choosing which history "
                    "to reproduce"
                ),
            }
        )
    return {"questions": len(questions), "ambiguous": report}


# ---- the remedy, stated where the operator will look --------------------

REMEDY = (
    "An ambiguous question is replayable again only once one of its records "
    "is gone. Purging one is an authority's act: `evidence.purge` on the "
    "record's artifact, by a principal in `authority_principals`, in a "
    "session that negotiated `evidence.retention_control`. It destroys "
    "evidence, so it is the owner's decision and never this harness's -- "
    "and the record to purge is the failed call, never the successful one, "
    "because purging the answer that worked would leave a history nobody "
    "made. See READINESS section 6."
)


# ---- one run ------------------------------------------------------------


def one_run(run, out, provider, client, live, ceiling):
    work = Path(tempfile.mkdtemp(prefix="cbr-m4e."))
    result = {"id": run["id"], "model": run["model"], "repository": run["repository"]["id"]}
    try:
        config = configuration(work, run, live, run.get("dry_answers", []))
        extra = []
        if live:
            extra = ["--permit-model-network", "--model-run-ceiling", str(ceiling)]
        child, endpoint = launch(provider, work, run, config, extra)
        credential = credential_file(work, live)
        try:
            inspected = submit_and_settle(client, endpoint, credential, run, "live")
            digest = packet_digest(inspected)
            result["items"] = [
                {
                    "item": item.get("item_id"),
                    "result": item.get("result"),
                    "reason": item.get("reason", ""),
                }
                for item in inspected.get("items", [])
            ]
            result["packet"] = digest
            result["packet_file"], live_sections = write_packet(
                client, endpoint, credential, out, run, "live", digest
            )
        finally:
            stop(child)

        data = work / "data"
        total, charges = spend(data)
        result["tokens"] = total
        result["charges"] = [{"kind": kind, "tokens": tokens} for kind, tokens in charges]
        result["records"] = len(records(data))

        # **The replay gate.** The same store, no transport, no
        # credential, no permit: every question answered from what was
        # retained or not at all.
        replay_config = work / "cbr-replay.json"
        replay_config.write_text(
            json.dumps(
                {
                    "format": "combraton-conformance-config/1",
                    "principal": PRINCIPAL,
                    "authority_principals": [PRINCIPAL],
                    "credentials": [{"credential": DRY_RUN_CREDENTIAL}],
                    "context": {"compile": True},
                    "model_runtime": {
                        "provider": "minimax",
                        "dialect": "responses",
                        "model": run["model"],
                    },
                }
            )
        )
        child, endpoint = launch(provider, work, run, replay_config, ["--replay-model"])
        credential = credential_file(work, False)
        try:
            rebuilt = submit_and_settle(client, endpoint, credential, run, "replay")
            rebuilt_sections = (
                sealed_sections(
                    json.loads(inspect_packet(client, endpoint, credential, run, "replay"))
                )
                if packet_digest(rebuilt)
                else None
            )
            result["replay"] = {
                "packet": packet_digest(rebuilt),
                "items": [
                    {
                        "item": item.get("item_id"),
                        "result": item.get("result"),
                        "reason": item.get("reason", ""),
                    }
                    for item in rebuilt.get("items", [])
                ],
            }
        finally:
            stop(child)
        # **What the gate actually compares.** The sections, because the
        # packet digest covers the request's own id and two requests can
        # never be byte-identical; byte identity across two stores is the
        # suite's gate, not this one's.
        result["replay"]["sections_identical"] = (
            live_sections is not None and live_sections == rebuilt_sections
        )
        result["replay"].update(ambiguity(work / "data"))
        return result
    finally:
        if not live:
            shutil.rmtree(work, ignore_errors=True)
        else:
            # A live run's store is the evidence. It is left where it is
            # and named, rather than deleted by the thing that made it.
            result["data"] = str(work / "data")


def main(argv=None):
    parser = argparse.ArgumentParser(description="the m4e run")
    parser.add_argument("--manifest")
    parser.add_argument("--out")
    parser.add_argument(
        "--ambiguity",
        help=(
            "report what in an existing data directory cannot be replayed, "
            "and stop. The operator's question after a run, answerable "
            "without running anything again."
        ),
    )
    parser.add_argument("--binaries", default=str(REPOSITORY / "target" / "debug"))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--permit-model-network", action="store_true")
    parser.add_argument("--run-ceiling", type=int, default=RUN_CEILING_TOKENS)
    options = parser.parse_args(argv)

    if options.ambiguity:
        found = ambiguity(Path(options.ambiguity))
        print(json.dumps({**found, "remedy": REMEDY}, indent=2, sort_keys=True))
        return 0

    if not options.manifest or not options.out:
        print("m4e_run: --manifest and --out are needed for a run", file=sys.stderr)
        return 2

    if options.live == options.dry_run:
        print(
            "m4e_run: choose one of --dry-run and --live. A run that is "
            "neither is a run nobody asked for.",
            file=sys.stderr,
        )
        return 2

    try:
        manifest = json.loads(Path(options.manifest).read_text())
        out = check(
            manifest,
            options.out,
            options.live,
            options.permit_model_network,
            options.run_ceiling,
        )
        provider, client = binaries(options.binaries)
        out.mkdir(parents=True, exist_ok=True)

        print(f"m4e_run: {'LIVE' if options.live else 'dry run'}, "
              f"{len(manifest['runs'])} runs, estimate {ESTIMATE_TOKENS:,} tokens, "
              f"stop at {STOP_TOKENS:,}, ceiling {options.run_ceiling:,}")

        report = {
            "mode": "live" if options.live else "dry-run",
            "estimate_tokens": ESTIMATE_TOKENS,
            "stop_tokens": STOP_TOKENS,
            "run_ceiling_tokens": options.run_ceiling,
            "remedy_for_an_ambiguous_question": REMEDY,
            "runs": [],
        }
        spent = 0
        for run in manifest["runs"]:
            result = one_run(
                run,
                out,
                provider,
                client,
                options.live,
                options.run_ceiling,
            )
            report["runs"].append(result)
            spent += result.get("tokens", 0)
            print(
                f"  {result['id']}: {result.get('tokens', 0):,} tokens, "
                f"{result.get('records', 0)} records, "
                f"replay {'identical' if result.get('replay', {}).get('sections_identical') else 'DIFFERS'}, "
                f"{len(result.get('replay', {}).get('ambiguous', []))} ambiguous"
            )
            # **The stop, checked between runs.** Halting mid-call would
            # leave a charge nobody reconciled; halting between them
            # leaves the ledger settled and says what was spent.
            if spent > STOP_TOKENS:
                report["stopped"] = (
                    f"spent {spent} tokens, which is more than half again over "
                    f"the {ESTIMATE_TOKENS} estimate"
                )
                print(f"m4e_run: STOPPED. {report['stopped']}", file=sys.stderr)
                break
        report["tokens"] = spent
        report["ambiguous_questions"] = sum(
            len(run.get("replay", {}).get("ambiguous", [])) for run in report["runs"]
        )

        (out / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True))
        print(f"m4e_run: {spent:,} tokens over {len(report['runs'])} runs; "
              f"{report['ambiguous_questions']} ambiguous questions; "
              f"report {out / 'report.json'}")
        if report["ambiguous_questions"]:
            print(f"m4e_run: {REMEDY}", file=sys.stderr)
        return 0
    except Refused as refusal:
        print(f"m4e_run: {refusal}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
