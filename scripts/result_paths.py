#!/usr/bin/env python3
"""Keep machine paths out of committed conformance results.

The runner expands `{repo}` in a participant descriptor's launch argv into an
absolute path and writes it into every transcript. In a public repository that
path names the machine and its owner, and it says nothing about the behaviour
under test. This module holds the one substitution and the one check, so the
script that records a run and the check that verifies committed results cannot
drift apart.

Usage, to verify committed results:
    scripts/result_paths.py DIR [DIR ...]

Exits nonzero and names the files if any machine path remains.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

CHECKOUT_PLACEHOLDER = "{cbr_checkout}"

# Anything still matching after redaction is a machine path this module does
# not know how to name, and it is refused rather than committed.
MACHINE_PATH = re.compile(r"/(?:Users|Volumes|home)/")


def redact_checkout_root(results, root=ROOT):
    """Replace the checkout's absolute path in a results directory's
    transcripts. Only transcripts are touched; the manifest stays the runner's
    own bytes. Returns how many transcript files changed."""
    root = str(root)
    changed = 0
    for path in sorted((Path(results) / "transcripts").glob("*.jsonl")):
        text = path.read_text()
        if root in text:
            path.write_text(text.replace(root, CHECKOUT_PLACEHOLDER))
            changed += 1
    return changed


def machine_paths(results):
    """Every file under a results directory that still contains a machine path."""
    results = Path(results)
    return [
        str(path.relative_to(results))
        for path in sorted(results.rglob("*"))
        if path.is_file() and MACHINE_PATH.search(path.read_text(errors="replace"))
    ]


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    failed = False
    for directory in argv[1:]:
        found = machine_paths(directory)
        if found:
            failed = True
            print(f"{directory}: machine paths in {len(found)} files: {found[:5]}", file=sys.stderr)
        else:
            print(f"{directory}: no machine paths")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
