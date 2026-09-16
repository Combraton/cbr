#!/usr/bin/env python3
"""Verify the vendored Protocol pin against anchors CBR does not control.

Checks, in order:
  1. every vendored file against the release's own BUNDLE-SHA256SUMS;
  2. the inventory's aggregate listing_sha256, recomputed from the vendored
     inventory.json, against PIN.json (a value that also appears in the
     annotated tag message and the release manifest);
  3. every vendored file inside the inventory's normative scope against the
     inventory's own recorded digest.

This validates the pin, not product behaviour. Exits nonzero on any mismatch.
"""
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor" / "protocol" / "v0.1.0"
SELF_AUTHORED = {"PIN.json", "README.md"}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def vendored_files():
    for path in sorted(VENDOR.rglob("*")):
        if path.is_file():
            rel = path.relative_to(VENDOR).as_posix()
            if rel not in SELF_AUTHORED:
                yield rel, path


def main():
    problems = []
    pin = json.loads((VENDOR / "PIN.json").read_text())

    # 1. Against the release's published per-file checksums.
    sums = {}
    for line in (VENDOR / "BUNDLE-SHA256SUMS").read_text().splitlines():
        if line.strip():
            digest, _, name = line.partition("  ")
            sums[name] = digest
    checked_bundle = 0
    for rel, path in vendored_files():
        if rel == "BUNDLE-SHA256SUMS":
            continue
        if rel not in sums:
            problems.append(f"not listed in BUNDLE-SHA256SUMS: {rel}")
            continue
        if sha256(path) != sums[rel]:
            problems.append(f"bundle checksum mismatch: {rel}")
        checked_bundle += 1

    # 2. The aggregate listing digest, recomputed the way the release computes it.
    inventory = json.loads((VENDOR / "docs/release/0.1/inventory.json").read_text())
    canonical = json.dumps(
        inventory["files"], separators=(",", ":"), sort_keys=True, ensure_ascii=False
    ).encode()
    listing = hashlib.sha256(canonical).hexdigest()
    if listing != pin["inventory_listing_sha256"]:
        problems.append(
            f"inventory listing_sha256 {listing} does not match the pinned "
            f"{pin['inventory_listing_sha256']}"
        )
    if inventory["totals"]["listing_sha256"] != listing:
        problems.append("inventory.json totals disagree with its own files list")
    if inventory["totals"]["files"] != pin["inventory_files"]:
        problems.append("inventory file count does not match the pin")

    # 3. Against the inventory's own per-file digests, for files in its scope.
    recorded = {entry["path"]: entry for entry in inventory["files"]}
    checked_inventory = 0
    for rel, path in vendored_files():
        entry = recorded.get(rel)
        if entry is None:
            continue
        if sha256(path) != entry["sha256"] or path.stat().st_size != entry["bytes"]:
            problems.append(f"inventory mismatch: {rel}")
        checked_inventory += 1

    for problem in problems:
        print(problem, file=sys.stderr)
    print(
        f"protocol pin {pin['tag']} at {pin['commit']}: "
        f"{checked_bundle} files match BUNDLE-SHA256SUMS, "
        f"{checked_inventory} match the normative inventory, "
        f"listing sha256 {listing}: {'ok' if not problems else 'FAILED'}"
    )
    print("Verifies the pinned contract material only. Not a product check.")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
