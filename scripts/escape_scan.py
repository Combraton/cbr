#!/usr/bin/env python3
"""What a request body would carry of every span a commit's tree can offer.

A candidate is shown to a model within `selection::CANDIDATE_TEXT_BYTES`
of text and `CANDIDATE_PATH_BYTES` of path, **as a request body carries
them**: the body is JSON, so a quotation mark is carried in two bytes and
a C0 control in six. A field past its bound is shown cut, with a marker
(m4 READINESS section 5, ADR 001 question 17). Whether a repository has
such fields is a property of its tree, so this reads a commit's tree the
way CBR's indexer and retrieval do and counts:

- the regular files, how many are indexed, and how many the indexer skips
  (over `MAX_BLOB_BYTES`, or not UTF-8);
- the spans: the indexer's chunks, at most `CHUNK_LINES` lines each and
  closed at the first newline at or past `CHUNK_BYTES`, each clipped at
  retrieval's `SPAN_BYTES` and decoded as the provider decodes it;
- the widest span as a body carries it, how many are carried past
  `SPAN_BYTES` and past the cut, and how many hold a six-byte escape;
- the indexed paths: the longest as a body carries it, and how many are
  past the path's cut;
- every span and path a model would be shown cut, by path and lines.

`selection::tests` reads the six bounds below beside the constants they
copy, so the two cannot drift.

**It prints the commit, counts and repository-relative paths, and never
repository text**, so it may be run over a repository whose text may not
be copied. A path is printed escaped as a body carries it, and DEL and the
C1 controls, which a body carries raw, as JSON's six-byte escapes too, so
no control character in a path -- C0, DEL or C1 -- is ever written raw.
`--json` escapes everything outside printable ASCII. **It writes
nothing**: it reads through `git rev-parse`, `git ls-tree` and
`git cat-file` alone.

What it does not establish: that a span it counts was ever offered.
Retrieval ranks spans and a question is shown a handful, so this is an
upper bound on what a model could be shown cut, not a measurement of what
one was. And it reads a commit's tree, so a dirty working tree is not
what it surveys.

Usage:
    scripts/escape_scan.py [--repo PATH] [--json] COMMIT

`--repo` defaults to the repository this script is in. Anything but a
commit is refused with exit status 2 and nothing on standard output.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

# The indexer's bounds (`cbr_memory::index`, `cbr_memory::lexical`).
MAX_BLOB_BYTES = 1_048_576
CHUNK_LINES = 20
CHUNK_BYTES = 2_048
# Retrieval's span, in raw bytes (`retrieval::Bounds::default().span_bytes`).
SPAN_BYTES = 4_096
# The cut (`selection::CANDIDATE_TEXT_BYTES`, `CANDIDATE_PATH_BYTES`), as a
# body carries a field.
CANDIDATE_TEXT_BYTES = 8_192
CANDIDATE_PATH_BYTES = 1_024

# The regular-file modes the indexer reads. A symlink's target is a path,
# and a submodule is another repository's identity.
REGULAR = {b"100644", b"100755"}

# How many bytes a body carries each character in, where that is not its
# UTF-8: cbr-encoding's canonical form gives seven characters a two-byte
# escape and every other C0 control `\u00xx`, six bytes. Nothing else is
# escaped.
SHORT = '"\\\b\t\n\f\r'
CARRIED = {chr(c): 6 for c in range(0x20)}
CARRIED.update({c: 2 for c in SHORT})


def carried_bytes(text):
    """How many bytes a request body carries `text` in, quotes excluded."""
    carried = len(text.encode("utf-8"))
    for character, width in CARRIED.items():
        found = text.count(character)
        if found:
            carried += found * (width - len(character.encode("utf-8")))
    return carried


def six_byte_escapes(text):
    """Whether a body carries any character of `text` in six bytes."""
    return any(text.count(character) for character, width in CARRIED.items() if width == 6)


# The control characters a request body carries raw, and a terminal can
# read as commands: DEL, and C1, among them U+009B, a control sequence
# introducer. JSON escapes C0 and nothing else.
UNESCAPED_CONTROLS = {chr(0x7F), *(chr(c) for c in range(0x80, 0xA0))}


def shown(path):
    """A path as a body carries it, without the quotes, and with DEL and
    the C1 controls escaped as well: no control character is ever printed
    raw, and otherwise the path is as it is."""
    escaped = json.dumps(path, ensure_ascii=False)[1:-1]
    return "".join(
        f"\\u{ord(c):04x}" if c in UNESCAPED_CONTROLS else c for c in escaped
    )


def chunks(data):
    """The indexer's chunks of `data`, which is valid UTF-8, as
    `(start, end, start_line, end_line)`: `lexical::chunks_bounded` with
    `CHUNK_LINES` and `CHUNK_BYTES`, tiling the text exactly."""
    start = 0
    start_line = 1
    counted = 0
    at = data.find(b"\n")
    while at != -1:
        counted += 1
        line_end = at + 1
        if counted == CHUNK_LINES or line_end - start >= CHUNK_BYTES:
            yield start, line_end, start_line, start_line + counted - 1
            start = line_end
            start_line += counted
            counted = 0
        at = data.find(b"\n", at + 1)
    if start < len(data):
        # A final line with no newline of its own.
        yield start, len(data), start_line, start_line + data.count(b"\n", start)


def git(repo, *arguments, stdin=None):
    return subprocess.run(
        ["git", "-C", str(repo), *arguments],
        input=stdin,
        capture_output=True,
        check=False,
    )


def commit_of(repo, revision):
    """The full id of `revision` when it names a commit, and otherwise
    None. A tree or a blob is not a commit, and neither is a typo."""
    if revision.startswith("-"):
        return None
    found = git(repo, "rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}")
    if found.returncode != 0:
        return None
    return found.stdout.decode().strip()


def blobs(repo, commit):
    """Every regular file of `commit`'s tree, as `(path, oid, size)`."""
    listing = git(repo, "ls-tree", "-r", "-z", "-l", "--full-tree", commit)
    if listing.returncode != 0:
        raise SystemExit("escape_scan: git ls-tree failed")
    for entry in listing.stdout.split(b"\0"):
        if not entry:
            continue
        meta, path = entry.split(b"\t", 1)
        mode, kind, oid, size = meta.split()
        if kind != b"blob" or mode not in REGULAR:
            continue
        yield path.decode("utf-8", "replace"), oid.decode(), int(size)


def contents(repo, oids):
    """Each blob's bytes, read in one `git cat-file --batch`."""
    if not oids:
        return {}
    batch = git(repo, "cat-file", "--batch", stdin="".join(f"{oid}\n" for oid in oids).encode())
    if batch.returncode != 0:
        raise SystemExit("escape_scan: git cat-file failed")
    out = batch.stdout
    read = {}
    at = 0
    for oid in oids:
        header_end = out.index(b"\n", at)
        name, _, size = out[at:header_end].split(b" ")
        size = int(size)
        read[name.decode()] = out[header_end + 1 : header_end + 1 + size]
        at = header_end + 1 + size + 1
    return read


def scan(repo, commit):
    files = list(blobs(repo, commit))
    small = [(path, oid) for path, oid, size in files if size <= MAX_BLOB_BYTES]
    read = contents(repo, sorted({oid for _, oid in small}))

    report = {
        "commit": commit,
        "files": {
            "regular": len(files),
            "indexed": 0,
            "too_large": len(files) - len(small),
            "not_utf8": 0,
        },
        "spans": {
            "total": 0,
            f"over_{SPAN_BYTES}": 0,
            f"over_{CANDIDATE_TEXT_BYTES}": 0,
            "with_six_byte_escapes": 0,
            "widest": None,
            "cut": [],
        },
        "paths": {
            "total": 0,
            f"over_{CANDIDATE_PATH_BYTES}": 0,
            "longest": None,
            "cut": [],
        },
    }
    spans = report["spans"]
    paths = report["paths"]
    for path, oid in sorted(small):
        data = read[oid]
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            report["files"]["not_utf8"] += 1
            continue
        report["files"]["indexed"] += 1

        carried_path = carried_bytes(path)
        paths["total"] += 1
        if paths["longest"] is None or carried_path > paths["longest"]["carried_bytes"]:
            paths["longest"] = {
                "carried_bytes": carried_path,
                "raw_bytes": len(path.encode("utf-8")),
                "path": path,
            }
        if carried_path > CANDIDATE_PATH_BYTES:
            paths[f"over_{CANDIDATE_PATH_BYTES}"] += 1
            paths["cut"].append(path)

        for start, end, start_line, end_line in chunks(data):
            # Retrieval clips a span in raw bytes, possibly inside a
            # character, and the provider decodes it lossily.
            raw = data[start : min(end, start + SPAN_BYTES)]
            text = raw.decode("utf-8", "replace")
            carried = carried_bytes(text)
            spans["total"] += 1
            if spans["widest"] is None or carried > spans["widest"]["carried_bytes"]:
                spans["widest"] = {
                    "carried_bytes": carried,
                    "raw_bytes": len(raw),
                    "path": path,
                    "start_line": start_line,
                    "end_line": end_line,
                }
            if carried > SPAN_BYTES:
                spans[f"over_{SPAN_BYTES}"] += 1
            if carried > CANDIDATE_TEXT_BYTES:
                spans[f"over_{CANDIDATE_TEXT_BYTES}"] += 1
                spans["cut"].append(
                    {
                        "path": path,
                        "start_line": start_line,
                        "end_line": end_line,
                        "carried_bytes": carried,
                    }
                )
            if six_byte_escapes(text):
                spans["with_six_byte_escapes"] += 1
    return report


def words(report):
    """The report, in the words a person reads."""
    files, spans, paths = report["files"], report["spans"], report["paths"]
    lines = [
        f"escape_scan: commit {report['commit']}",
        f"files: {files['regular']:,} regular, {files['indexed']:,} indexed; "
        f"skipped as the indexer skips them: {files['too_large']:,} over "
        f"{MAX_BLOB_BYTES:,} bytes, {files['not_utf8']:,} not UTF-8",
        f"spans: {spans['total']:,}",
    ]
    widest = spans["widest"]
    if widest is not None:
        lines.append(
            f"widest span: {widest['carried_bytes']:,} carried bytes "
            f"({widest['raw_bytes']:,} raw), {shown(widest['path'])} "
            f"lines {widest['start_line']}-{widest['end_line']}"
        )
    lines += [
        f"spans past {SPAN_BYTES:,} carried bytes: {spans[f'over_{SPAN_BYTES}']:,}",
        f"spans past {CANDIDATE_TEXT_BYTES:,} carried bytes, shown cut: "
        f"{spans[f'over_{CANDIDATE_TEXT_BYTES}']:,}",
        f"spans with a six-byte escape: {spans['with_six_byte_escapes']:,}",
        f"paths: {paths['total']:,}",
    ]
    longest = paths["longest"]
    if longest is not None:
        lines.append(
            f"longest path: {longest['carried_bytes']:,} carried bytes "
            f"({longest['raw_bytes']:,} raw), {shown(longest['path'])}"
        )
    lines.append(
        f"paths past {CANDIDATE_PATH_BYTES:,} carried bytes, shown cut: "
        f"{paths[f'over_{CANDIDATE_PATH_BYTES}']:,}"
    )
    for span in spans["cut"]:
        lines.append(
            f"cut: {shown(span['path'])} lines {span['start_line']}-{span['end_line']}, "
            f"{span['carried_bytes']:,} carried bytes"
        )
    for path in paths["cut"]:
        lines.append(f"cut path: {shown(path)}")
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="What a request body would carry of every span a commit's tree can offer."
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="the repository to read (default: the one this script is in)",
    )
    parser.add_argument("--json", action="store_true", help="print one JSON object")
    parser.add_argument("commit", help="the commit whose tree is surveyed")
    options = parser.parse_args(argv)

    commit = commit_of(options.repo, options.commit)
    if commit is None:
        print(f"escape_scan: {options.commit!r} is not a commit", file=sys.stderr)
        return 2
    report = scan(options.repo, commit)
    if options.json:
        print(json.dumps(report, ensure_ascii=True, sort_keys=True))
    else:
        print(words(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
