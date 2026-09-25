#!/usr/bin/env python3
"""J2 live rubric: the answer key, computed from the three inputs' bytes only.

    python3 answer_key.py --inputs DIR [--out answer_key.json]

Imports nothing from the CBR checkout and never reads CBR's parser: every
fact here comes from this file's own regexes and its own position-keeping
JSON scanner. Every range is a half-open byte range [start, end) of the
input; a *line* range excludes its newline. Any line whose text carries an
absolute path is recorded by offset (and sha256) only, never by text.

The facts the rubric was given are checked at the end; a mismatch is
reported in `mismatches` and on stderr, never forced.
"""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

INPUTS = {
    "red": ("cargo-test-f73415d.log", "05a1090e4ec78780d625c7e28958bed65ab0a4f83566debd9c64e5c583d2a326"),
    "green": ("cargo-test.log", "b89ad11e5a5ce5975f89b70ca91d14d400fa8c490f971fa5f603f80470898807"),
    "core": ("conformance-core.manifest.json", "06a0f54234b49df6c459cfec6217e8f1b50c6dd5e03cc4ede619ffbd2ed69a35"),
}
EXPECTED = {
    "red": {"failing_tests": 18, "by_binary": {"j2_harness": 6, "journey_two": 12},
            "cargo_error_atoms": 3, "named_failures": 21, "result_lines": 40},
    "green": {"passed": 644, "failed": 0, "ignored": 1, "result_lines": 40},
    "core": {"pass": 130, "unsupported": 5, "results": 135},
}

# An absolute path of the kind that names a machine or a temporary root.
PATHLIKE = re.compile(r"(?<![\w.])/(?:Users|home|private|var|tmp|Volumes|opt|root)/")

RUNNING = re.compile(r"^\s+Running (?:unittests (\S+)|tests/(\S+)\.rs) \(target/\S+/deps/([A-Za-z0-9_\-]+?)-[0-9a-f]+\)$")
DOCTESTS = re.compile(r"^\s+Doc-tests (\S+)$")
RUNNING_N = re.compile(r"^running \d+ tests?$")
STATUS = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(.*)$")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out")
BLOCK = re.compile(r"^---- (\S+) std(?:out|err) ----$")
PANIC = re.compile(r"^thread '([^']+)' \(\d+\) panicked at (\S+):$")
NOTE = re.compile(r"^note: run with `RUST_BACKTRACE=1`")
ERROR = re.compile(r"^error: ")
BUILD = re.compile(r"^\s*(Compiling|Finished|Blocking|Downloaded|Updating|warning)\b")


def lines_of(data):
    out, at = [], 0
    while at < len(data):
        end = data.find(b"\n", at)
        nl = end if end >= 0 else len(data)
        out.append({"no": len(out) + 1, "s": at, "e": nl, "full_e": min(nl + 1, len(data)),
                    "text": data[at:nl].decode("utf-8")})
        at = nl + 1
    return out


def safe_text(text):
    """The text, or None when it carries a path (then only offsets are kept)."""
    return None if PATHLIKE.search(text) else text


def sha(b):
    return hashlib.sha256(b).hexdigest()


def span(line):
    return [line["s"], line["e"]]


def merge_classes(pieces):
    """[(s, e, cls)] sorted, adjacent same-class merged."""
    out = []
    for s, e, c in pieces:
        if out and out[-1][2] == c and out[-1][1] == s:
            out[-1][1] = e
        else:
            out.append([s, e, c])
    return out


def libtest(data):
    L = lines_of(data)
    cls = ["other"] * len(L)
    binary = None
    binaries, statuses, results, errors, blocks, failure_lists, ignored = [], [], [], [], {}, [], []
    i = 0
    while i < len(L):
        ln, t = L[i], L[i]["text"]
        m = RUNNING.match(t)
        d = DOCTESTS.match(t)
        if m:
            binary = m.group(2) or m.group(3).replace("-", "_")
            binaries.append({"binary": binary, "running_line": span(ln)})
            cls[i] = "identity"
        elif d:
            binary = "doctests:" + d.group(1)
            binaries.append({"binary": binary, "running_line": span(ln)})
            cls[i] = "identity"
        elif RUNNING_N.match(t):
            cls[i] = "identity"
        elif RESULT.match(t):
            r = RESULT.match(t)
            rec = {"binary": binary, "line": span(ln), "line_no": ln["no"], "status": r.group(1),
                   "passed": int(r.group(2)), "failed": int(r.group(3)), "ignored": int(r.group(4))}
            results.append(rec)
            if binaries:
                binaries[-1]["result_line"] = span(ln)
                binaries[-1]["tests"] = rec["passed"] + rec["failed"] + rec["ignored"]
            cls[i] = "identity" if r.group(1) == "ok" else "failure"
        elif STATUS.match(t):
            s = STATUS.match(t)
            name, st = s.group(1), s.group(2)
            if st == "FAILED" and s.group(3) == "":
                statuses.append({"binary": binary, "name": name, "status_line": span(ln),
                                 "name_range": [ln["s"] + 5, ln["s"] + 5 + len(name.encode())]})
                cls[i] = "failure"
            elif st == "ok" and s.group(3) == "":
                cls[i] = "pass"
            elif st == "ignored":
                ignored.append({"binary": binary, "name": name, "line": span(ln),
                                "text": safe_text(t)})
                cls[i] = "ignored"
        elif t == "failures:":
            # The name list is the "failures:" whose next line is indented.
            j = i + 1
            if j < len(L) and L[j]["text"].startswith("    "):
                entries = []
                cls[i] = "failure"
                while j < len(L) and L[j]["text"].startswith("    "):
                    entries.append({"name": L[j]["text"].strip(), "line": span(L[j])})
                    cls[j] = "failure"
                    j += 1
                failure_lists.append({"binary": binary, "heading": span(ln), "entries": entries})
                i = j
                continue
            cls[i] = "failure"
        elif BLOCK.match(t):
            name = BLOCK.match(t).group(1)
            j = i + 1
            while j < len(L) and not BLOCK.match(L[j]["text"]) and L[j]["text"] != "failures:":
                j += 1
            k = j - 1
            while k > i and L[k]["text"].strip() == "":
                k -= 1
            blk = {"binary": binary, "header": span(ln), "block": [ln["s"], L[k]["e"]]}
            for x in range(i, k + 1):
                cls[x] = "failure"
            p = next((x for x in range(i + 1, k + 1) if PANIC.match(L[x]["text"])), None)
            if p is not None:
                pm = PANIC.match(L[p]["text"])
                msg = []
                x = p + 1
                while x <= k and L[x]["text"].strip() != "" and not BLOCK.match(L[x]["text"]):
                    if not NOTE.match(L[x]["text"]):
                        msg.append(L[x])
                    x += 1
                blk.update({
                    "panic_line": span(L[p]), "panic_thread": pm.group(1), "location": pm.group(2),
                    "message_lines": [span(m) for m in msg],
                    "message_text": [safe_text(m["text"]) for m in msg],
                    "message_sha256": [sha(data[m["s"]:m["e"]]) for m in msg],
                    "assertion": [L[p]["s"], (msg[-1]["e"] if msg else L[p]["e"])],
                })
            blocks[name] = blk
            # blank lines between blocks stay "failure" only inside the block
            i = j
            continue
        elif ERROR.match(t):
            atom = {"line": span(ln), "text": safe_text(t), "continuation": []}
            cls[i] = "failure"
            j = i + 1
            while j < len(L) and L[j]["text"].startswith("    `"):
                atom["continuation"].append(span(L[j]))
                cls[j] = "failure"
                j += 1
            atom["atom"] = [ln["s"], (L[j - 1]["e"])]
            errors.append(atom)
            i = j
            continue
        elif t.strip() == "":
            cls[i] = "blank"
        elif BUILD.match(t):
            cls[i] = "build"
        i += 1
    for x in range(len(L)):
        if cls[x] == "other" and L[x]["text"].strip() == "":
            cls[x] = "blank"
    classes = merge_classes([(L[x]["s"], L[x]["full_e"], cls[x]) for x in range(len(L))])
    return L, binaries, statuses, results, errors, blocks, failure_lists, ignored, classes


def totals(results):
    return {k: sum(r[k] for r in results) for k in ("passed", "failed", "ignored")}


def red_key(data):
    L, binaries, statuses, results, errors, blocks, flists, ignored, classes = libtest(data)
    tests = []
    for st in statuses:
        b = blocks.get(st["name"], {})
        entry = next((e for fl in flists if fl["binary"] == st["binary"] for e in fl["entries"]
                      if e["name"] == st["name"]), None)
        heading = next((fl["heading"] for fl in flists if fl["binary"] == st["binary"]), None)
        tests.append({
            "binary": st["binary"], "name": st["name"], "status_line": st["status_line"],
            "name_range": st["name_range"], "block_header": b.get("header"), "block": b.get("block"),
            "panic_line": b.get("panic_line"), "location": b.get("location"),
            "message_lines": b.get("message_lines", []), "message_text": b.get("message_text", []),
            "message_sha256": b.get("message_sha256", []), "assertion": b.get("assertion"),
            "failures_list_entry": entry["line"] if entry else None, "failures_list_heading": heading,
            "value_bearing": any((t or "").lstrip().startswith("left:") for t in b.get("message_text", []))
            and any((t or "").lstrip().startswith("right:") for t in b.get("message_text", [])),
        })
    # signatures: (location, first message line digest)
    sig = {}
    for t in tests:
        k = (t["location"], t["message_sha256"][0] if t["message_sha256"] else "")
        sig.setdefault(k, []).append(t["name"])
    signatures = [{"location": k[0], "first_message_sha256": k[1],
                   "first_message_text": next((safe_text(data[x["message_lines"][0][0]:x["message_lines"][0][1]].decode())
                                               for x in tests if x["name"] == v[0] and x["message_lines"]), None),
                   "tests": v} for k, v in sig.items()]
    # What a parser that names every FAILED status and every cargo error line lists, in byte order.
    named = sorted([("test", t["name"], t["name_range"]) for t in tests] +
                   [("cargo_error", None, e["line"]) for e in errors], key=lambda x: x[2][0])
    failing_result = [r for r in results if r["status"] == "FAILED"]
    rerun = [e for e in errors if e["text"] and e["text"].startswith("error: test failed, to rerun pass")]
    targets = [e for e in errors if e["text"] and re.match(r"^error: \d+ targets? failed:$", e["text"])]
    return {
        "input": INPUTS["red"][0], "size": len(data), "lines": len(L),
        "failing_tests": tests,
        "failing_tests_count": len(tests),
        "by_binary": {b: sum(1 for t in tests if t["binary"] == b) for b in sorted({t["binary"] for t in tests})},
        "signatures": signatures,
        "cargo_errors": [{"line": e["line"], "atom": e["atom"], "continuation": e["continuation"],
                          "text": e["text"]} for e in errors],
        "rerun_lines": [e["line"] for e in rerun],
        "targets_block": targets[0]["atom"] if targets else None,
        "failing_result_lines": [r["line"] for r in failing_result],
        "failures_lists": [{"binary": f["binary"], "heading": f["heading"],
                            "entries": [e["line"] for e in f["entries"]]} for f in flists],
        "result_lines": [{k: r[k] for k in ("binary", "line", "line_no", "status", "passed", "failed", "ignored")}
                         for r in results],
        "totals": totals(results),
        "ignored_tests": ignored,
        "expected_named": [{"kind": k, "name": n, "range": r} for k, n, r in named],
        "expected_named_count": len(named),
        "binaries": binaries,
        "line_classes": classes,
    }


def green_key(data):
    L, binaries, statuses, results, errors, blocks, flists, ignored, classes = libtest(data)
    panics = [span(l) for l in L if PANIC.match(l["text"])]
    ign = []
    for g in ignored:
        res = next((r for r in results if r["binary"] == g["binary"] and r["line"][0] > g["line"][0]), None)
        ign.append({**g, "result_line": res["line"] if res else None})
    last = results[-1]["line"] if results else None
    return {
        "input": INPUTS["green"][0], "size": len(data), "lines": len(L),
        "failed_status_lines": [s["status_line"] for s in statuses],
        "panic_lines": panics, "error_lines": [e["line"] for e in errors],
        "result_lines": [{k: r[k] for k in ("binary", "line", "line_no", "status", "passed", "failed", "ignored")}
                         for r in results],
        "totals": totals(results),
        "tests_total": sum(r["passed"] + r["failed"] + r["ignored"] for r in results),
        "ignored_tests": ign,
        "tail": [last[0], len(data)] if last else None,
        "binaries": binaries,
        "expected_named_count": len(statuses) + len(errors),
        "line_classes": classes,
    }


# ---- a position-keeping JSON scanner --------------------------------------

class Scan:
    def __init__(self, data):
        self.d, self.i = data, 0

    def ws(self):
        while self.i < len(self.d) and self.d[self.i] in b" \t\r\n":
            self.i += 1

    def value(self):
        """Returns (start, end, kind, payload)."""
        self.ws()
        s, c = self.i, self.d[self.i:self.i + 1]
        if c == b"{":
            self.i += 1
            members = []
            self.ws()
            if self.d[self.i:self.i + 1] == b"}":
                self.i += 1
                return (s, self.i, "object", members)
            while True:
                self.ws()
                ks, ke, _, key = self.value()
                self.ws()
                assert self.d[self.i:self.i + 1] == b":"
                self.i += 1
                v = self.value()
                members.append({"key": key, "key_start": ks, "start": ks, "end": v[1], "value": v})
                self.ws()
                c = self.d[self.i:self.i + 1]
                self.i += 1
                if c == b"}":
                    return (s, self.i, "object", members)
                assert c == b","
        if c == b"[":
            self.i += 1
            items = []
            self.ws()
            if self.d[self.i:self.i + 1] == b"]":
                self.i += 1
                return (s, self.i, "array", items)
            while True:
                items.append(self.value())
                self.ws()
                c = self.d[self.i:self.i + 1]
                self.i += 1
                if c == b"]":
                    return (s, self.i, "array", items)
                assert c == b","
        if c == b'"':
            self.i += 1
            while True:
                ch = self.d[self.i:self.i + 1]
                if ch == b"\\":
                    self.i += 2
                    continue
                self.i += 1
                if ch == b'"':
                    break
            return (s, self.i, "string", json.loads(self.d[s:self.i]))
        m = re.compile(rb"-?\d+(\.\d+)?([eE][+-]?\d+)?|true|false|null").match(self.d, self.i)
        self.i = m.end()
        return (s, self.i, "scalar", json.loads(self.d[s:self.i]))


def member(obj, key):
    return next((m for m in obj[3] if m["key"] == key), None)


def core_key(data):
    root = Scan(data).value()
    results = member(root, "results")["value"]
    recs, counts = [], {}
    for idx, r in enumerate(results[3]):
        outcome = member(r, "outcome")["value"][3]
        counts[outcome] = counts.get(outcome, 0) + 1
        rec = {"index": idx, "range": [r[0], r[1]], "outcome": outcome}
        if outcome != "pass":
            rec.update({
                "fixture": member(r, "fixture")["value"][3],
                "reason": member(r, "reason")["value"][3],
                "polarity": member(r, "polarity")["value"][3],
                "members": {k: [member(r, k)["start"], member(r, k)["end"]] for k in ("fixture", "outcome", "reason")},
            })
        recs.append(rec)
    root_members = {m["key"]: [m["start"], m["end"]] for m in root[3]}
    suite = member(root, "suite")["value"]
    summary = member(root, "summary")["value"]
    return {
        "input": INPUTS["core"][0], "size": len(data),
        "outcome_counts": counts,
        "results_count": len(recs),
        "summary_counts": {m["key"]: m["value"][3] for m in summary[3]},
        "suite_fixtures": member(suite, "fixtures")["value"][3],
        "summary_member": root_members["summary"],
        "suite_member": root_members["suite"],
        "suite_fixtures_member": [member(suite, "fixtures")["start"], member(suite, "fixtures")["end"]],
        "status_note_member": root_members["status_note"],
        "root_members": root_members,
        "identity_members": {k: root_members[k] for k in ("format", "participant", "runner", "suite")},
        "non_pass": [r for r in recs if r["outcome"] != "pass"],
        "pass_records": [r["range"] for r in recs if r["outcome"] == "pass"],
        "failure_outcomes": [r["index"] for r in recs if r["outcome"] not in ("pass", "unsupported", "skipped")],
        "expected_named_count": 0,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--inputs", required=True)
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent / "answer_key.json"))
    a = ap.parse_args()
    key = {"format": "j2-answer-key/1", "ranges": "half-open [start, end) byte offsets; line ranges exclude the newline",
           "inputs": {}}
    mismatches = []
    for kind, (name, digest) in INPUTS.items():
        data = (Path(a.inputs) / name).read_bytes()
        if sha(data) != digest:
            mismatches.append(f"{kind}: {name} sha256 {sha(data)} is not the pinned {digest}")
        k = {"red": red_key, "green": green_key, "core": core_key}[kind](data)
        k["sha256"] = "sha256:" + sha(data)
        key["inputs"][kind] = k
    r, g, c = key["inputs"]["red"], key["inputs"]["green"], key["inputs"]["core"]
    er, eg, ec = EXPECTED["red"], EXPECTED["green"], EXPECTED["core"]
    checks = [
        ("red failing tests", r["failing_tests_count"], er["failing_tests"]),
        ("red by binary", r["by_binary"], er["by_binary"]),
        ("red cargo error atoms", len(r["cargo_errors"]), er["cargo_error_atoms"]),
        ("red named failures", r["expected_named_count"], er["named_failures"]),
        ("red result lines", len(r["result_lines"]), er["result_lines"]),
        ("red FAILED result lines sum", sum(x["failed"] for x in r["result_lines"]), er["failing_tests"]),
        ("red every failing test has a block, panic and message",
         all(t["block"] and t["panic_line"] and t["message_lines"] for t in r["failing_tests"]), True),
        ("green passed", g["totals"]["passed"], eg["passed"]),
        ("green failed", g["totals"]["failed"], eg["failed"]),
        ("green ignored", g["totals"]["ignored"], eg["ignored"]),
        ("green result lines", len(g["result_lines"]), eg["result_lines"]),
        ("green all ok", all(x["status"] == "ok" for x in g["result_lines"]), True),
        ("green no FAILED/panic/error", len(g["failed_status_lines"]) + len(g["panic_lines"]) + len(g["error_lines"]), 0),
        ("green ignored lines", len(g["ignored_tests"]), eg["ignored"]),
        ("core pass", c["outcome_counts"].get("pass"), ec["pass"]),
        ("core unsupported", c["outcome_counts"].get("unsupported"), ec["unsupported"]),
        ("core results", c["results_count"], ec["results"]),
        ("core summary agrees", c["summary_counts"], {"pass": ec["pass"], "unsupported": ec["unsupported"]}),
        ("core suite.fixtures", c["suite_fixtures"], ec["results"]),
        ("core no failure outcome", c["failure_outcomes"], []),
    ]
    for label, got, want in checks:
        if got != want:
            mismatches.append(f"{label}: got {got!r}, the given fact is {want!r}")
    key["checks"] = [{"check": l, "got": g_, "given": w, "ok": g_ == w} for l, g_, w in checks]
    key["mismatches"] = mismatches
    text = json.dumps(key, indent=1, sort_keys=True)
    if PATHLIKE.search(text):
        at = PATHLIKE.search(text).start()
        sys.exit(f"answer_key: refusing to write, an absolute path appears at char {at}")
    Path(a.out).write_text(text + "\n")
    for m in mismatches:
        print("MISMATCH:", m, file=sys.stderr)
    print(f"answer_key: wrote {a.out}; {len(checks) - len(mismatches)}/{len(checks)} given facts hold")
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
