#!/usr/bin/env python3
"""Run pinned conformance fixtures against CBR's provider.

Builds nothing implicitly: the provider and the runner must already exist, so
a result can never be produced by a stale binary without that being visible.

Every result manifest is stamped with the runner's identity — the release
archive's SHA-256, the source commit and the release `Cargo.lock` SHA-256 — so
a fixture outcome names the exact runner that produced it. Outcomes are
reported as the runner reports them: `unsupported` and `skipped` are listed
under coverage limits and are never counted as passes.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import result_paths

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor" / "protocol" / "v0.1.0"
RUNNER_IDENTITY = ROOT / "target" / "protocol-release" / "runner.json"
PROVIDER = ROOT / "target" / "debug" / "cbr-provider"
DESCRIPTOR = ROOT / "conformance" / "participants" / "cbr-provider.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--filter", required=True, help="Fixture id prefix, for example 'stream.'")
    parser.add_argument("--out", required=True, help="Results directory, relative to the repository root")
    args = parser.parse_args()

    if not RUNNER_IDENTITY.is_file():
        sys.exit("no runner: run scripts/build_runner.py first")
    identity = json.loads(RUNNER_IDENTITY.read_text())
    runner = ROOT / identity["runner"]
    if not runner.is_file():
        sys.exit(f"runner binary missing at {runner}; re-run scripts/build_runner.py")
    if not PROVIDER.is_file():
        sys.exit(f"provider binary missing at {PROVIDER}; run cargo build --workspace")

    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    command = [
        str(runner),
        "run",
        "--repo",
        str(VENDOR),
        "--participant",
        str(DESCRIPTOR),
        "--filter",
        args.filter,
        "--out",
        str(out),
    ]
    print(" ".join(command))
    completed = subprocess.run(command, check=False)

    manifest_path = out / "manifest.json"
    if not manifest_path.is_file():
        sys.exit(f"runner wrote no manifest to {manifest_path} (exit {completed.returncode})")

    manifest = json.loads(manifest_path.read_text())
    redacted = result_paths.redact_checkout_root(out)
    leftovers = result_paths.machine_paths(out)
    if leftovers:
        sys.exit(f"machine paths remain after redaction in {len(leftovers)} files: {leftovers[:5]}")

    # Written beside the manifest, never into it. The manifest is the runner's
    # own output and stays byte-for-byte what the runner produced, so a reader
    # comparing two runs is never comparing one that CBR edited.
    #
    # The correction below matters: the runner derives `suite.protocol_commit`
    # from the Git checkout enclosing `--repo`, which here is CBR, so that field
    # names CBR's HEAD rather than Protocol's.
    pin = json.loads((VENDOR / "PIN.json").read_text())
    sidecar = {
        "format": "cbr-run-record/1",
        "manifest": "manifest.json",
        "runner_identity": identity,
        "runner_exit_status": completed.returncode,
        "filter": args.filter,
        "fixture_source": {
            "note": (
                "Fixtures and schemas came from the vendored Protocol release. The manifest's "
                "suite.protocol_commit names the Git checkout enclosing --repo, which is CBR, "
                "not Protocol."
            ),
            "vendored_at": str(VENDOR.relative_to(ROOT)),
            "protocol_tag": pin["tag"],
            "protocol_commit": pin["commit"],
            "inventory_listing_sha256": pin["inventory_listing_sha256"],
            "verified_by": "scripts/verify_pin.py",
        },
        "redactions": [
            {
                "files": redacted,
                "replaced": "the absolute path of the CBR checkout the run was made from",
                "with": result_paths.CHECKOUT_PLACEHOLDER,
                "where": "transcripts/*.jsonl only; manifest.json is untouched",
                "reason": (
                    "The runner expands {repo} in the descriptor's launch argv into an absolute "
                    "path, which names the machine and its owner and says nothing about CBR's "
                    "behaviour. The substitution is a fixed prefix, so it is reversible and "
                    "changes no outcome."
                ),
            }
        ],
        "outcomes": {},
        "unsupported": sorted(
            entry["fixture"] for entry in manifest.get("results", []) if entry.get("outcome") == "unsupported"
        ),
    }
    for entry in manifest.get("results", []):
        outcome = entry.get("outcome", "unknown")
        sidecar["outcomes"][outcome] = sidecar["outcomes"].get(outcome, 0) + 1
    (out / "cbr-run.json").write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n")

    outcomes = sidecar["outcomes"]
    passed = outcomes.get("pass", 0)
    total = sum(outcomes.values())
    print(f"outcomes: {outcomes}")
    print(f"{passed} of {total} passing")
    print(f"runner exit status: {completed.returncode}")
    print(
        "The runner leaves coverage_limits empty even when fixtures are unsupported, so an "
        "outcome count is the only honest gate. Use scripts/check_results.py."
    )
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
