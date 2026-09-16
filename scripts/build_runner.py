#!/usr/bin/env python3
"""Build the Protocol conformance runner from the verified release archive.

The runner is not a member of CBR's Cargo workspace and must not become one:
its manifest inherits `license.workspace` and `edition.workspace` from the
Protocol workspace, and the vendored subset carries no `Cargo.lock`, so an
in-workspace build would need a licence CBR has not selected and could not use
`--locked`. Building from a fresh extraction also means the runner is built
with the release's own lockfile, so a fixture result can name the exact runner
that produced it.

Steps:
  1. Obtain the source archive, reusing a cached copy when its digest matches.
  2. Check it against the release's `SHA256SUMS`, then the pinned digest.
  3. Extract under `target/`, then check every extracted file against the
     archive's own `BUNDLE-SHA256SUMS`.
  4. `cargo build -p combraton-conformance --locked` inside the extraction.
  5. Write `runner.json`: the runner path and the three identities that any
     results manifest must carry.

Exits nonzero on any failure. Verifies the runner's provenance, not CBR.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PIN = ROOT / "vendor" / "protocol" / "v0.1.0" / "PIN.json"
WORK = ROOT / "target" / "protocol-release"
RELEASE_URL = "https://github.com/Combraton/protocol/releases/download/{tag}/{name}"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def fetch(url, destination):
    print(f"downloading {url}")
    with urllib.request.urlopen(url) as response, destination.open("wb") as out:
        shutil.copyfileobj(response, out)


def obtain_archive(pin, supplied, offline):
    """Return the verified archive path, reusing a cached copy when possible."""
    name = pin["bundle_asset"]
    expected = pin["bundle_sha256"]
    cached = WORK / name

    if supplied is not None:
        source = Path(supplied)
        if not source.is_file():
            sys.exit(f"archive not found: {source}")
        if source.resolve() != cached.resolve():
            WORK.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, cached)
    elif cached.is_file() and sha256(cached) == expected:
        print(f"reusing verified archive {cached}")
    elif offline:
        sys.exit(f"no verified archive at {cached} and --offline was given")
    else:
        WORK.mkdir(parents=True, exist_ok=True)
        fetch(RELEASE_URL.format(tag=pin["tag"], name=name), cached)

    found = sha256(cached)
    if found != expected:
        sys.exit(f"archive sha256 {found} does not match the pinned {expected}")
    print(f"archive sha256 {found}: matches the pin")
    return cached


def safe_extract(archive, into):
    """Extract, refusing any member that would escape the destination."""
    with tarfile.open(archive, "r:gz") as tar:
        members = tar.getmembers()
        for member in members:
            target = (into / member.name).resolve()
            if not str(target).startswith(str(into.resolve())):
                sys.exit(f"archive member escapes the destination: {member.name}")
            if member.issym() or member.islnk():
                sys.exit(f"archive member is a link: {member.name}")
        tar.extractall(into, members=members)


def verify_extraction(tree):
    """Check every extracted file against the archive's own checksum list."""
    sums_path = tree / "BUNDLE-SHA256SUMS"
    if not sums_path.is_file():
        sys.exit("extraction has no BUNDLE-SHA256SUMS")
    recorded = {}
    for line in sums_path.read_text().splitlines():
        if line.strip():
            digest, _, name = line.partition("  ")
            recorded[name] = digest

    problems, checked = [], 0
    for path in sorted(tree.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(tree).as_posix()
        if rel == "BUNDLE-SHA256SUMS":
            continue
        if rel not in recorded:
            problems.append(f"not listed in BUNDLE-SHA256SUMS: {rel}")
        elif sha256(path) != recorded[rel]:
            problems.append(f"checksum mismatch: {rel}")
        else:
            checked += 1
    missing = set(recorded) - {
        p.relative_to(tree).as_posix() for p in tree.rglob("*") if p.is_file()
    }
    for name in sorted(missing):
        problems.append(f"listed but not extracted: {name}")
    if problems:
        for problem in problems[:20]:
            print(problem, file=sys.stderr)
        sys.exit(f"{len(problems)} extraction problems")
    print(f"extraction: {checked} files match BUNDLE-SHA256SUMS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", help="Use this already-downloaded source archive")
    parser.add_argument("--offline", action="store_true", help="Never download")
    args = parser.parse_args()

    pin = json.loads(PIN.read_text())
    archive = obtain_archive(pin, args.archive, args.offline)

    tree = WORK / f"combraton-protocol-{pin['tag'].lstrip('v')}"
    if tree.exists():
        shutil.rmtree(tree)
    safe_extract(archive, WORK)
    if not tree.is_dir():
        sys.exit(f"archive did not contain {tree.name}/")
    verify_extraction(tree)

    # The archive records the commit it was exported from. Check it against the
    # pin so a mismatched archive cannot silently produce a runner attributed to
    # the pinned commit.
    source = json.loads((tree / "RELEASE-SOURCE.json").read_text())
    if source["commit"] != pin["commit"]:
        sys.exit(f"archive commit {source['commit']} does not match the pinned {pin['commit']}")

    lock_sha256 = sha256(tree / "Cargo.lock")
    print(f"building combraton-conformance with the release's own Cargo.lock ({lock_sha256[:16]}...)")
    build = subprocess.run(
        ["cargo", "build", "-p", "combraton-conformance", "--locked"],
        cwd=tree,
        check=False,
    )
    if build.returncode != 0:
        sys.exit(f"cargo build failed with exit {build.returncode}")

    runner = tree / "target" / "debug" / "combraton-conformance"
    if not runner.is_file():
        sys.exit(f"runner binary not found at {runner}")

    # Every results manifest copies these three, so a fixture outcome names the
    # exact runner that produced it.
    identity = {
        "format": "cbr-runner-identity/1",
        "runner": str(runner.relative_to(ROOT)),
        "protocol_tag": pin["tag"],
        "source_commit": source["commit"],
        "archive_sha256": pin["bundle_sha256"],
        "cargo_lock_sha256": lock_sha256,
    }
    (WORK / "runner.json").write_text(json.dumps(identity, indent=2) + "\n")

    print(json.dumps(identity, indent=2))
    print(f"runner identity written to {(WORK / 'runner.json').relative_to(ROOT)}")
    print("Verifies the runner's provenance, not CBR's behaviour.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
