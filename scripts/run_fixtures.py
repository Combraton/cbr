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

    # The runner records `suite.protocol_commit` from the Git checkout that
    # encloses `--repo`. Here that is CBR, not Protocol, so the field names the
    # wrong repository. Its own fields are left untouched and the correction is
    # recorded beside them, namespaced, so a reader is never left guessing which
    # commit the fixtures actually came from.
    pin = json.loads((VENDOR / "PIN.json").read_text())
    manifest["cbr_runner_identity"] = identity
    manifest["cbr_runner_exit_status"] = completed.returncode
    manifest["cbr_fixture_source"] = {
        "note": (
            "Fixtures and schemas came from the vendored Protocol release, not from the "
            "repository the runner inspected for suite.protocol_commit. That field names "
            "CBR's own HEAD and should be read as such."
        ),
        "vendored_at": str(VENDOR.relative_to(ROOT)),
        "protocol_tag": pin["tag"],
        "protocol_commit": pin["commit"],
        "inventory_listing_sha256": pin["inventory_listing_sha256"],
        "verified_by": "scripts/verify_pin.py",
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")

    outcomes = {}
    for entry in manifest.get("results", []):
        outcome = entry.get("outcome", "unknown")
        outcomes[outcome] = outcomes.get(outcome, 0) + 1
    limits = manifest.get("coverage_limits", [])
    passed = outcomes.get("pass", 0)
    total = sum(outcomes.values())
    print(f"outcomes: {outcomes}")
    print(f"{passed} of {total} passing")
    print(f"coverage_limits: {limits if limits else 'none'}")
    print(f"runner exit status: {completed.returncode}")
    print("Only `pass` counts as passing. `unsupported` and `skipped` are coverage limits.")
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
