#!/usr/bin/env python3
"""Journey 2's live run, as code, reviewed before it is ever run.

J2's live acceptance runs on **CBR's own test output** (m5 READINESS
section 3 and section 10, the owner's decision of 2026-09-24): logs and
JSON produced locally by running CBR's public suite at a pinned commit,
sealed whole, projected, and checked. This is that run, with a dry-run
mode that drives every stage against the `model.fake` control and never
opens a socket to anything but the provider.

    python3 scripts/j2_run.py --manifest M --out D --dry-run
    python3 scripts/j2_run.py --manifest M --out D --live \\
        --permit-model-network --run-ceiling N

**The live run is not authorised by this file existing.** `--live` needs
`--permit-model-network`, a `--run-ceiling` the owner set when the run's
estimate was written into READINESS, and the owner's word at the time,
which no flag can stand in for.

What it does, per input in the manifest:

  1. launches a provider over a data directory under `--out`, bounded by
     what is left of the run ceiling rather than by the whole of it, and
     **registers no repository**: nothing here is about a tree's files, so
     discovery has nothing to search and every call a request makes is a
     part of its projection;
  2. seals the input whole with `cbr ingest`, anchored at the pinned
     commit of this repository;
  3. asks for it with no investigation -- the deterministic baseline,
     which costs nothing and says how many parts the input fills -- and
     then with exactly that many questions;
  4. checks both projections against the input's own bytes: the ledger
     tiles the artifact, every excerpt is the artifact's bytes at its
     stated range, every omission is in the packet's own list;
  5. relaunches the same store with `--replay-model` and rebuilds the
     model-assisted projection from its records -- **the replay gate**.

What it refuses, before anything starts:

  * **an input in which a machine path appears** -- the check
    `result_paths.py` applies to committed results, with the temporary
    roots a local run writes as well. An input is sealed whole, so it is
    never scrubbed: output that names the machine it ran on is produced
    again from somewhere whose path names nothing;
  * an input whose bytes are not the digest the manifest pins, when it
    pins one;
  * this checkout's origin not being `Combraton/cbr`, or the pinned
    commit not being in it; in live mode, an origin `api.github.com`
    does not say is public;
  * an output directory inside this repository;
  * `--live` without `--run-ceiling`, or without
    `--permit-model-network`;
  * a `--run-ceiling` above m4e's hard cap of 5,000,000, in either mode;
  * a model outside the three M4 may name.

And it stops **before** the run whose worst case could cross the ceiling,
rather than after one that did: the worst case of a projection is a
computed number (`projection::tests`), so the arithmetic is done before
the tokens are.

The report holds names, digests, sizes, counts, costs and the checks'
verdicts. **It holds no path from the manifest**: an input is named by
its file name and its digest.
"""

import argparse
import base64
import json
import os
import re
import socket
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import result_paths  # noqa: E402

# The reviewed pieces this run shares with m4e's, rather than a second copy
# of each: the configuration and the credential that goes with it, the
# read-only store reader, and the replay gate's diff.
from m4e_run import (  # noqa: E402
    MODELS,
    PUBLIC_REPOSITORIES,
    REMEDY,
    REPOSITORY,
    RUN_CEILING_TOKENS,
    SETTLE_SECONDS,
    Refused,
    ambiguity,
    binaries,
    cbr,
    configuration,
    credential_file,
    differing_leaves,
    git_says,
    normalised_origin,
    packet_digest,
    records,
    refuse,
    refuse_unless_public,
    spend,
    stop,
)

# `projection::tests` computes both, and reads them back from here: the
# worst a whole projection can cost, every part counted and repaired once,
# and the most parts one projection asks.
WORST_CASE_PROJECTION_TOKENS = 488_160
MAX_PARTS = 4

# The prefix of every part's question, which is how a part's record is told
# from anything else a store holds.
PART_SELECTOR = "project_large_result "

# What a projection may carry, as the J2 journey asks for it.
TASK = "which tests failed, and what did they assert"
ITEM = "log"
CAPACITY = 65536

# **A machine path, as `result_paths.py` defines one for committed
# results, and the temporary roots a local run also writes.** Test output
# prints where a failing test's scratch directory was, and on macOS that is
# under `/var/folders`; a path there names the machine as surely as a home
# directory does.
TEMPORARY_ROOTS = re.compile(r"/(?:private/)?var/folders/|/private/tmp/|(?<![\w.])/tmp/")


def machine_path_at(data):
    """Where the first machine path in `data` starts, or `None`.

    Read as text with anything undecodable replaced, so a binary input is
    searched too. **The offset, never the path**: the refusal must not
    print the thing it exists to keep out of the record.
    """
    text = data.decode("utf-8", "replace")
    found = [
        match.start()
        for match in (
            result_paths.MACHINE_PATH.search(text),
            TEMPORARY_ROOTS.search(text),
        )
        if match
    ]
    return min(found) if found else None


def sha256_of(data):
    import hashlib

    return "sha256:" + hashlib.sha256(data).hexdigest()


# ---- what may be run ----------------------------------------------------


def check(manifest, out, live, permit, ceiling):
    """Every refusal, before a provider is launched or a byte is sent.

    Returns the output directory, the pinned commit and each run's input
    bytes, which are read here once: the bytes checked for machine paths
    are the bytes that are ingested.
    """
    if live and ceiling is None:
        refuse(
            "--live needs --run-ceiling. Each live run's cap is the owner's, "
            "set when its estimate is written into READINESS section 10, and "
            "this script has no default to fall back on."
        )
    if live and not permit:
        refuse(
            "--live needs --permit-model-network as well. A live run reads "
            "the owner's credential and opens a socket to a provider, and "
            "having built the harness is not permission to use it."
        )
    # **m4e's hard cap is this run's too**, in a dry run as in a live one: a
    # dry run reads the same flags, and one that admitted what a live run
    # refuses would say nothing about the live run.
    if ceiling is not None and ceiling > RUN_CEILING_TOKENS:
        refuse(
            f"--run-ceiling {ceiling} is above the hard cap of "
            f"{RUN_CEILING_TOKENS} that m4e's harness set and J2's shares. "
            "Raising it is the owner's decision and not this script's."
        )
    out = Path(out).resolve()
    if out == REPOSITORY or REPOSITORY in out.parents:
        refuse(
            f"--out {out} is inside {REPOSITORY}. A run's stores and packets "
            "are its evidence and are kept where they are written, outside "
            "the tree."
        )

    # **This checkout is the repository the output came from**, by its
    # origin rather than by what anyone called it.
    origin = normalised_origin(
        git_says(
            REPOSITORY,
            ["remote", "get-url", "origin"],
            f"{REPOSITORY} has no origin remote, so nothing says which repository it is",
        )
    )
    if origin not in PUBLIC_REPOSITORIES["cbr"]:
        refuse(
            f"{REPOSITORY} has origin {origin!r}, which is not "
            f"{' or '.join(PUBLIC_REPOSITORIES['cbr'])}. J2 runs on CBR's own "
            "test output, by the owner's decision, and nothing else."
        )
    commit = manifest.get("commit")
    if not isinstance(commit, str) or not commit:
        refuse("the manifest pins no commit that its inputs were produced at")
    commit = git_says(
        REPOSITORY,
        ["rev-parse", "--verify", f"{commit}^{{commit}}"],
        f"the pinned commit {commit!r} is not in {REPOSITORY}",
    )

    runs = manifest.get("runs")
    if not isinstance(runs, list) or not runs:
        refuse("the manifest names no runs")
    seen = set()
    inputs = {}
    for run in runs:
        for member in ("id", "input"):
            if member not in run:
                refuse(f"a run is missing {member!r}: {run.get('id', run)!r}")
        if run["id"] in seen:
            refuse(f"two runs share the id {run['id']!r}")
        seen.add(run["id"])
        model = run.get("model", manifest.get("model"))
        if model not in MODELS:
            refuse(
                f"{run['id']}: {model!r} is not one of the three models M4 may "
                f"name: {', '.join(MODELS)}"
            )
        path = Path(run["input"]).expanduser()
        if not path.is_file():
            refuse(f"{run['id']}: its input is not a file")
        data = path.read_bytes()
        if not data:
            refuse(f"{run['id']}: its input is empty")
        at = machine_path_at(data)
        if at is not None:
            refuse(
                f"{run['id']}: its input carries a machine path at byte {at}. "
                "An input is sealed whole and never scrubbed, so output that "
                "names the machine it was produced on is produced again from a "
                "location whose path names nothing (READINESS section 3)."
            )
        pinned = run.get("digest")
        if pinned and sha256_of(data) != pinned:
            refuse(
                f"{run['id']}: its input is not the bytes the manifest pins "
                f"({sha256_of(data)} against {pinned})"
            )
        inputs[run["id"]] = {"data": data, "name": path.name, "model": model}

    # **Last, and only when something will be sent**, as m4e does it.
    if live:
        refuse_unless_public({"cbr": {"origin": origin}})
    return out, commit, inputs


# ---- driving a provider -------------------------------------------------


def launch(provider, work, config, extra):
    """A provider over `work`, **with no repository registered**."""
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


def ingest(client, endpoint, credential, path, commit):
    """Seal the input whole, anchored at the commit it was produced at."""
    ingested = cbr(
        client,
        endpoint,
        credential,
        ["ingest", str(path), "--repo", str(REPOSITORY), "--commit", commit],
    )
    if ingested.returncode != 0:
        refuse(f"ingest failed: {ingested.stderr.strip()}")
    fields = dict(
        line.split(" ", 1) for line in ingested.stdout.splitlines() if " " in line
    )
    return fields["artifact"], fields["digest"]


def ask(client, endpoint, credential, request, artifact, digest, commit, investigation):
    """One request for the artifact, polled until it settles."""
    submitted = cbr(
        client,
        endpoint,
        credential,
        [
            "context",
            request,
            "--repo",
            str(REPOSITORY),
            "--repo-id",
            "cbr",
            "--commit",
            commit,
            "--obligation",
            "required_before_start",
            "--task",
            TASK,
            "--capacity",
            str(CAPACITY),
            "--investigation",
            str(investigation),
            "--want",
            f"{ITEM}=evidence:{artifact}@{digest}",
        ],
    )
    if submitted.returncode != 0:
        refuse(f"{request}: submit failed: {submitted.stderr.strip()}")
    started = time.monotonic()
    while True:
        polled = cbr(client, endpoint, credential, ["request", request])
        if polled.returncode != 0:
            refuse(f"{request}: inspect failed: {polled.stderr.strip()}")
        inspected = json.loads(polled.stdout)
        if inspected.get("state") != "preparing":
            return inspected
        if time.monotonic() - started > SETTLE_SECONDS:
            refuse(f"{request} never left preparing")
        time.sleep(0.25)


def sealed_packet(client, endpoint, credential, request):
    printed = cbr(client, endpoint, credential, ["packet", request, "--excerpt", "10000000"])
    if printed.returncode != 0:
        refuse(f"{request}: packet failed: {printed.stderr.strip()}")
    facts = json.loads(printed.stdout)
    return printed.stdout, json.loads(base64.b64decode(facts["excerpt"]["data_base64"]))


def item_of(inspected):
    for item in inspected.get("items", []):
        if item.get("item_id") == ITEM:
            return {"result": item.get("result"), "reason": item.get("reason", "")}
    return {"result": None, "reason": ""}


# ---- reading a projection, from the format it declares ------------------


def read_projection(content):
    """The header, the named failures and the ledger of a projection.

    Written from the format the projection declares, not from the code that
    renders it. An excerpt is taken **by the count its frame states** and
    never by searching for its end marker: the bytes are the artifact's,
    and an artifact may contain anything.
    """
    data = content.encode("utf-8")
    at = 0

    def line():
        nonlocal at
        end = data.find(b"\n", at)
        end = len(data) if end < 0 else end
        text = data[at:end].decode("utf-8")
        at = end + 1
        return text

    header = []
    while True:
        if at >= len(data):
            raise ValueError("no `failures named:` line")
        text = line()
        if text.startswith("failures named: "):
            named_count = int(text[len("failures named: "):])
            break
        header.append(text)
    named, unresolved, extents = [], [], []
    while at < len(data):
        text = line()
        if text.startswith("  and "):
            continue
        if text.startswith("  "):
            name, _, span = text[2:].rpartition(" at bytes ")
            start, end = (int(n) for n in span.split("-"))
            named.append((name, start, end))
            continue
        if text.startswith("unresolved: "):
            unresolved.append(text[len("unresolved: "):])
            continue
        tag, _, rest = text.partition("] bytes ")
        span, _, rest = rest.partition(", lines ")
        start, end = (int(n) for n in span.split("-"))
        _, _, what = rest.partition(", ")
        if tag.startswith("[e"):
            excerpt = data[at : at + (end - start)]
            at += end - start
            closing = f"\n[end {tag[1:]}]\n".encode()
            if data[at : at + len(closing)] != closing:
                raise ValueError(f"excerpt {tag[1:]} is not closed where its frame says")
            at += len(closing)
            extents.append({"carried": True, "start": start, "end": end, "bytes": excerpt})
        elif tag.startswith("[o"):
            extents.append(
                {
                    "carried": False,
                    "number": int(tag[2:]),
                    "start": start,
                    "end": end,
                    "reason": what[len("omitted: "):],
                }
            )
        else:
            raise ValueError(f"not a ledger line: {text!r}")
    return {
        "header": header,
        "named": named,
        "named_count": named_count,
        "unresolved": unresolved,
        "extents": extents,
    }


PROTOCOL_REASON = {
    "not_selected": "applicability",
    "over_projection": "output_capacity",
    "not_text": "unavailable",
}


def checked(packet, source):
    """What a packet's projection is, and whether it holds.

    `problems` is empty exactly when the ledger tiles the source, every
    excerpt is the source's bytes at its range, every named failure is the
    source's bytes at its range, and the packet's omissions for the item are
    exactly the ledger's. Anything else is named.
    """
    section = next(
        (s for s in packet.get("sections", []) if s.get("item_id") == ITEM), None
    )
    if section is None:
        return {"section": False}
    problems = []
    try:
        read = read_projection(section["content"])
    except (ValueError, UnicodeDecodeError) as unreadable:
        return {"section": True, "problems": [f"unreadable: {unreadable}"]}
    expected = 0
    carried = omitted_bytes = 0
    for extent in read["extents"]:
        if extent["start"] != expected:
            problems.append(f"bytes {expected}-{extent['start']} are neither carried nor declared")
        if extent["carried"]:
            carried += extent["end"] - extent["start"]
            if extent["bytes"] != source[extent["start"] : extent["end"]]:
                problems.append(f"the excerpt at {extent['start']}-{extent['end']} is not the source's bytes")
        else:
            omitted_bytes += extent["end"] - extent["start"]
        expected = extent["end"]
    if expected != len(source):
        problems.append(f"the ledger stops at byte {expected} of {len(source)}")
    for name, start, end in read["named"]:
        if source[start:end] != name.encode("utf-8"):
            problems.append(f"a named failure at {start}-{end} is not the source's bytes")
    declared = [
        (f"s-{ITEM}.o{extent['number']}", PROTOCOL_REASON.get(extent["reason"], "?"))
        for extent in read["extents"]
        if not extent["carried"]
    ]
    packet_omissions = [
        (omission.get("section_id"), omission.get("reason"))
        for omission in packet.get("omissions", [])
        if omission.get("item_id") == ITEM
    ]
    if declared != packet_omissions:
        problems.append("the packet's omissions are not the projection's")
    how = next((line for line in read["header"] if line.startswith("read as ")), "")
    parts = re.search(r" (\d+) parts;", how)
    return {
        "section": True,
        "how": how,
        "parts": int(parts.group(1)) if parts else None,
        "named_failures": read["named_count"],
        "excerpts": sum(1 for extent in read["extents"] if extent["carried"]),
        "omissions": len(declared),
        "bytes_carried": carried,
        "bytes_omitted": omitted_bytes,
        "unresolved": len(read["unresolved"]),
        "content_bytes": len(section["content"].encode("utf-8")),
        "problems": problems,
    }


def part_records(data):
    """The sealed records of this store's projection parts."""
    return [
        (identifier, record)
        for identifier, record in records(data)
        if record.get("question", {}).get("selector", "").startswith(PART_SELECTOR)
    ]


# ---- one run ------------------------------------------------------------


def one_run(run, given, out, provider, client, live, ceiling, commit):
    work = out / "work" / run["id"]
    work.mkdir(parents=True, exist_ok=True)
    source = given["data"]
    staged = work / given["name"]
    staged.write_bytes(source)
    result = {
        "id": run["id"],
        "model": given["model"],
        "input": {"name": given["name"], "digest": sha256_of(source), "size": len(source)},
        "commit": commit,
        "launch_ceiling": ceiling,
        "data": str(work / "data"),
    }
    shaped = {**run, "model": given["model"]}
    dry_answers = run.get("dry_answers", ["ids:u1"])
    config, config_body = configuration(work, shaped, live, dry_answers)
    extra = ["--model-run-ceiling", str(ceiling)]
    if live:
        extra = ["--permit-model-network", *extra]
    child, endpoint = launch(provider, work, config, extra)
    credential = credential_file(work, config_body)
    try:
        artifact, digest = ingest(client, endpoint, credential, staged, commit)
        result["artifact"] = artifact
        if digest != sha256_of(source):
            refuse(f"{run['id']}: the store sealed {digest}, not the input's digest")

        # **The baseline**: no investigation, no call, and the number of
        # parts the input fills, read from what the projection says.
        baseline = ask(client, endpoint, credential, "baseline", artifact, digest, commit, 0)
        result["baseline"] = {"item": item_of(baseline)}
        parts = None
        if packet_digest(baseline):
            printed, packet = sealed_packet(client, endpoint, credential, "baseline")
            (out / f"{run['id']}.baseline.packet.json").write_text(printed)
            result["baseline"].update(checked(packet, source))
            parts = result["baseline"].get("parts")
        how = result["baseline"].get("how", "")
        asks = bool(parts) and "chosen by the deterministic rule" in how
        if asks:
            assisted = ask(
                client, endpoint, credential, "assisted", artifact, digest, commit, parts
            )
            result["assisted"] = {"item": item_of(assisted), "investigation": parts}
            if packet_digest(assisted):
                printed, packet = sealed_packet(client, endpoint, credential, "assisted")
                (out / f"{run['id']}.assisted.packet.json").write_text(printed)
                result["assisted"].update(checked(packet, source))
                assisted_sections = packet.get("sections")
            else:
                assisted_sections = None
        else:
            result["assisted"] = {
                "skipped": (
                    "nothing to ask: the baseline carried the input whole, found "
                    "it was not text, or found it over the projection's capacity"
                )
            }
    finally:
        stop(child)

    data = work / "data"
    total, charges = spend(data)
    result["tokens"] = total
    result["charges"] = [{"kind": kind, "tokens": tokens} for kind, tokens in charges]
    result["part_records"] = len(part_records(data))

    if asks:
        # **The replay gate**, under the same configuration the live run
        # used, as m4e's is: every part answered from what was retained.
        replay_config, replay_body = configuration(work, shaped, live, dry_answers, replay=True)
        child, endpoint = launch(
            provider, work, replay_config, ["--replay-model", "--model-run-ceiling", str(ceiling)]
        )
        credential = credential_file(work, replay_body)
        try:
            rebuilt = ask(
                client, endpoint, credential, "replay", artifact, digest, commit, parts
            )
            rebuilt_sections = None
            if packet_digest(rebuilt):
                _, packet = sealed_packet(client, endpoint, credential, "replay")
                rebuilt_sections = packet.get("sections")
            result["replay"] = {"item": item_of(rebuilt)}
        finally:
            stop(child)
        differences = differing_leaves(assisted_sections, rebuilt_sections)
        result["replay"]["differences"] = differences
        result["replay"]["sections_identical"] = assisted_sections is not None and not differences
        result["replay"].update(ambiguity(data))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description="Journey 2's run")
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--binaries", default=str(REPOSITORY / "target" / "debug"))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--permit-model-network", action="store_true")
    parser.add_argument(
        "--run-ceiling",
        type=int,
        help=(
            "the most the whole run may spend. Required in live mode, where it "
            "is the owner's cap; a dry run spends only the fake's figures."
        ),
    )
    options = parser.parse_args(argv)
    if options.live == options.dry_run:
        print(
            "j2_run: choose one of --dry-run and --live. A run that is neither "
            "is a run nobody asked for.",
            file=sys.stderr,
        )
        return 2
    try:
        manifest = json.loads(Path(options.manifest).read_text())
        out, commit, inputs = check(
            manifest,
            options.out,
            options.live,
            options.permit_model_network,
            options.run_ceiling,
        )
        provider, client = binaries(options.binaries)
        out.mkdir(parents=True, exist_ok=True)
        ceiling = options.run_ceiling
        if ceiling is None:
            ceiling = WORST_CASE_PROJECTION_TOKENS * len(manifest["runs"])
        print(
            f"j2_run: {'LIVE' if options.live else 'dry run'}, {len(manifest['runs'])} runs, "
            f"worst case {WORST_CASE_PROJECTION_TOKENS:,} a run, ceiling {ceiling:,}"
        )
        report = {
            "mode": "live" if options.live else "dry-run",
            "commit": commit,
            "run_ceiling_tokens": ceiling,
            "worst_case_projection_tokens": WORST_CASE_PROJECTION_TOKENS,
            "remedy_for_an_ambiguous_question": REMEDY,
            "runs": [],
        }
        spent = 0
        for run in manifest["runs"]:
            remaining = ceiling - spent
            if remaining < WORST_CASE_PROJECTION_TOKENS:
                report["stopped"] = (
                    f"{run['id']} was not started: {remaining} left under the "
                    f"ceiling of {ceiling} is less than one projection's "
                    f"{WORST_CASE_PROJECTION_TOKENS} worst case"
                )
                print(f"j2_run: STOPPED. {report['stopped']}", file=sys.stderr)
                break
            result = one_run(
                run, inputs[run["id"]], out, provider, client, options.live, remaining, commit
            )
            report["runs"].append(result)
            spent += result.get("tokens", 0)
            print(
                f"  {result['id']}: {result.get('tokens', 0):,} tokens, "
                f"{result.get('part_records', 0)} part records, "
                f"baseline {result['baseline']['item']['result']}, "
                f"assisted {result['assisted'].get('item', {}).get('result', 'skipped')}"
            )
        report["tokens"] = spent
        (out / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True))
        print(f"j2_run: {spent:,} tokens over {len(report['runs'])} runs; report {out / 'report.json'}")
        return 0
    except Refused as refusal:
        print(f"j2_run: {refusal}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
