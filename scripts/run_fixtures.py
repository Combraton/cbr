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
import os
import subprocess
import sys
from pathlib import Path

import result_paths

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor" / "protocol" / "v0.1.0"
RUNNER_IDENTITY = ROOT / "target" / "protocol-release" / "runner.json"
PROVIDER = ROOT / "target" / "debug" / "cbr-provider"
DESCRIPTOR = ROOT / "conformance" / "participants" / "cbr-provider.json"
REFERENCE_LAUNCHER = "reference_executor_launch.py"
COMPOSITION = "composition."


def launches_reference_executor(descriptor):
    """Whether a descriptor launches through scripts/reference_executor_launch.py.

    Judged by its launch argv, not its file name, so a renamed copy of
    cbr-with-reference-executor-unix.json is the same descriptor.
    """
    try:
        argv = json.loads(descriptor.read_text())["launch"]["argv"]
    except (OSError, ValueError, KeyError, TypeError):
        return False
    return any(isinstance(part, str) and part.endswith(REFERENCE_LAUNCHER) for part in argv)


def outside_composition(selection):
    """Vendored fixture ids the runner's --filter selects outside the composition suite.

    The runner selects a fixture when its id *contains* the filter, so the
    prefix alone is not the rule: every selected id must be a composition one.
    """
    ids = []
    for path in sorted((VENDOR / "conformance" / "fixtures").rglob("*.json")):
        try:
            fixture_id = json.loads(path.read_text()).get("id")
        except (OSError, ValueError, AttributeError):
            continue
        if isinstance(fixture_id, str) and selection in fixture_id and not fixture_id.startswith(COMPOSITION):
            ids.append(fixture_id)
    return ids


def refuse_reference_executor_outside_composition(selection, descriptor):
    """The reference-executor descriptor runs the composition suite and nothing else.

    It claims execution/1 on the reference provider's behalf. Any other
    fixture it made applicable would be reported under a CBR-named
    participant, which RELEASE-SCOPE section 6 forbids.
    """
    if not launches_reference_executor(descriptor):
        return
    if not selection.startswith(COMPOSITION):
        sys.exit(
            f"refused: {descriptor.name} launches the reference executor and runs composition fixtures "
            f"only; --filter must start with {COMPOSITION!r}, got {selection!r}"
        )
    stray = outside_composition(selection)
    if stray:
        sys.exit(
            f"refused: {descriptor.name} launches the reference executor and runs composition fixtures "
            f"only; --filter {selection!r} also selects {stray[:5]}"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--filter", required=True, help="Fixture id prefix, for example 'stream.'")
    parser.add_argument("--out", required=True, help="Results directory, relative to the repository root")
    parser.add_argument(
        "--participant",
        default=str(DESCRIPTOR.relative_to(ROOT)),
        help="Participant descriptor, relative to the repository root (default: the stdio descriptor)",
    )
    args = parser.parse_args()

    # Before anything is created or started.
    refuse_reference_executor_outside_composition(args.filter, ROOT / args.participant)

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
        str(ROOT / args.participant),
        "--filter",
        args.filter,
        "--out",
        str(out),
    ]
    print(" ".join(command))
    # The composition suite launches the vendored client-only kernel, a Python
    # program. Without this, Python writes __pycache__ into vendor/, and the
    # pinned tree no longer matches its checksums.
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    completed = subprocess.run(command, check=False, env=environment)

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
        "participant": args.participant,
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
