#!/usr/bin/env python3
"""Assert a fixture run produced exactly the expected outcomes.

The runner's own `coverage_limits` list stays empty even when fixtures are
reported `unsupported`, so "no coverage limits" is not a gate: a run where
every fixture was skipped for want of a claimed feature would satisfy it. The
only honest gate is the exact outcome multiset, together with the exact set of
fixtures expected to be unsupported, named.

Usage:
    scripts/check_results.py <results-dir> <expectation-file>
"""

import json
import sys
from pathlib import Path


def main(argv):
    if len(argv) != 3:
        sys.exit(__doc__)
    results = Path(argv[1])
    expected = json.loads(Path(argv[2]).read_text())
    manifest = json.loads((results / "manifest.json").read_text())
    sidecar = json.loads((results / "cbr-run.json").read_text())

    outcomes = {}
    unsupported = []
    for entry in manifest["results"]:
        outcome = entry["outcome"]
        outcomes[outcome] = outcomes.get(outcome, 0) + 1
        if outcome == "unsupported":
            unsupported.append(entry["fixture"])
    unsupported.sort()

    problems = []
    if outcomes.get("pass", 0) != expected["pass"]:
        problems.append(f"expected {expected['pass']} passing, got {outcomes.get('pass', 0)}")
    for forbidden in ("fail", "timeout", "harness_error", "skipped"):
        count = outcomes.get(forbidden, 0)
        if count:
            named = sorted(
                entry["fixture"] for entry in manifest["results"] if entry["outcome"] == forbidden
            )
            problems.append(f"{count} {forbidden}: {named}")
    if unsupported != sorted(expected["unsupported"]):
        missing = sorted(set(expected["unsupported"]) - set(unsupported))
        extra = sorted(set(unsupported) - set(expected["unsupported"]))
        if extra:
            problems.append(f"unexpectedly unsupported: {extra}")
        if missing:
            problems.append(f"expected unsupported but were not: {missing}")
    total = sum(outcomes.values())
    if total != expected["total"]:
        problems.append(f"expected {expected['total']} fixtures, got {total}")
    if sidecar["runner_exit_status"] != expected.get("runner_exit_status", 0):
        problems.append(f"runner exit status {sidecar['runner_exit_status']}")

    for problem in problems:
        print(problem, file=sys.stderr)
    print(
        f"{results}: {outcomes}, {len(unsupported)} unsupported by name: "
        f"{'ok' if not problems else 'FAILED'}"
    )
    print(f"expectation: {expected['note']}")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
