#!/usr/bin/env python3
"""J2 live rubric: the gates and every SCRIPT criterion, for both arms of every run.

    python3 score_j2.py --out RUN_DIR --key answer_key.json --inputs INPUT_DIR \
        [--judged judged.json] [--dry] [--json scores.json]

RUN_DIR (repeatable: one per j2_run.py invocation) is what `scripts/j2_run.py --out` wrote: report.json, <id>.baseline.packet.json,
<id>.assisted.packet.json and work/<id>/data (the store, read from a private copy, never
opened in place). Nothing is imported from the CBR checkout: the projection is read from the
format it declares, by this file's own reader, and checked against the input's bytes.

`--dry` marks live-only gates (mode live, a real model's input tokens) as n/a, so the script
can be exercised on the fake-model dry run. `--judged` (produced by `blind.py unblind`) adds
the JUDGED criteria; without it every verdict is printed PROVISIONAL (script criteria only).
"""

import argparse
import base64
import hashlib
import json
import re
import shutil
import sqlite3
import statistics
import sys
import tempfile
from pathlib import Path

TASK = "which tests failed, and what did they assert"
ITEM = "log"
PROJECTION_BYTES, MAX_EXCERPTS, NAMED_LISTED, MAX_PARTS = 16384, 24, 16, 4
WORST_CASE_PROJECTION_TOKENS = 488_160
MODELS = {"m27hs": "MiniMax-M2.7-highspeed", "m3": "MiniMax-M3"}
RUN_IDS = ["red-m27hs", "red-m3", "log-m27hs", "log-m3", "core-m27hs", "core-m3"]
KIND_OF_INPUT = {"cargo-test-f73415d.log": "red", "cargo-test.log": "green",
                 "conformance-core.manifest.json": "core"}
PROTOCOL_REASON = {"not_selected": "applicability", "over_projection": "output_capacity",
                   "not_text": "unavailable"}
# The deterministic baseline's section, artifact id masked, as the dry run of 2026-09-25
# produced it (C0, a consistency check: a mismatch rescores, it does not void).
DRY_BASELINE_MASKED_SHA256 = {
    "red": "8fee63a1456969997aa5ac1378fd7dab05f657ea9f617cbd66610e2ffe3b561b",
    "green": "52afd8657e1e41e1e9d3369aa94b9d1b405904ef6c1869f410c0907abeeeb679",
    "core": "5961b748208d92fc0fb761d424813a4ac44e6d7a16e54114b0a8341547a6e2b8",
}

# Weights (RUBRIC.md section 3). S = scripted, J = judged. Each kind sums to 100.
WEIGHTS = {
    "red": {"R1": 30, "R2": 10, "R3": 5, "R4": 10, "R5": 10, "R6": 5, "J1": 20, "J2": 10},
    "green": {"G1": 10, "G2": 20, "G3": 5, "G4": 10, "G5": 20, "J1": 25, "J2": 10},
    "core": {"C1": 20, "C2": 5, "C3": 20, "C4": 15, "C5": 5, "J1": 25, "J2": 10},
}
CRITICAL = {"red": ["R1", "J1"], "green": ["G1", "J1"], "core": ["C1", "J1"]}
JUDGED = ("J1", "J2")


# ---- reading a projection -------------------------------------------------

def read_projection(content):
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
        t = line()
        if t.startswith("failures named: "):
            named_count = int(t[len("failures named: "):])
            break
        header.append(t)
    named, more, unresolved, extents = [], None, [], []
    while at < len(data):
        t = line()
        m = re.match(r"^  and (\d+) more, the first at bytes (\d+)-(\d+)$", t)
        if m:
            more = (int(m.group(1)), int(m.group(2)), int(m.group(3)))
            continue
        if t.startswith("  "):
            name, _, sp = t[2:].rpartition(" at bytes ")
            s, e = (int(n) for n in sp.split("-"))
            named.append((name, s, e))
            continue
        if t.startswith("unresolved: "):
            unresolved.append(t[len("unresolved: "):])
            continue
        m = re.match(r"^\[(e|o)(\d+)\] bytes (\d+)-(\d+), lines (\d+)-(\d+), (.*)$", t)
        if not m:
            raise ValueError(f"not a ledger line: {t[:80]!r}")
        kind, num, s, e = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))
        if kind == "e":
            body = data[at:at + (e - s)]
            at += e - s
            closing = f"\n[end e{num}]\n".encode()
            if data[at:at + len(closing)] != closing:
                raise ValueError(f"excerpt e{num} not closed where its frame says")
            at += len(closing)
            extents.append({"carried": True, "n": num, "s": s, "e": e, "bytes": body, "what": m.group(7)})
        else:
            reason = m.group(7)
            if not reason.startswith("omitted: "):
                raise ValueError(f"omission o{num} has no reason")
            extents.append({"carried": False, "n": num, "s": s, "e": e, "reason": reason[len("omitted: "):]})
    return {"header": header, "named_count": named_count, "named": named, "more": more,
            "unresolved": unresolved, "extents": extents, "content_bytes": len(data)}


class View:
    """What a reader of one projection holds: carried ranges and omitted ranges."""

    def __init__(self, proj):
        self.p = proj
        self.carried = [(x["s"], x["e"]) for x in proj["extents"] if x["carried"]]
        self.omitted = [x for x in proj["extents"] if not x["carried"]]

    def cov(self, s, e):
        return sum(max(0, min(e, b) - max(s, a)) for a, b in self.carried)

    def full(self, r):
        return r is not None and r[1] > r[0] and self.cov(r[0], r[1]) == r[1] - r[0]

    def any(self, r):
        return r is not None and self.cov(r[0], r[1]) > 0

    def omitted_over(self, r):
        return [o for o in self.omitted if o["s"] < r[1] and r[0] < o["e"]]

    def class_bytes(self, classes, wanted):
        return sum(self.cov(s, e) for s, e, c in classes if c in wanted)


def band(x, cuts):
    """cuts = thresholds for 4,3,2,1 (x >= cut); else 0."""
    for level, cut in zip((4, 3, 2, 1), cuts):
        if x >= cut:
            return level
    return 0


def noise_level(n):
    return 4 if n == 0 else 3 if n <= 1024 else 2 if n <= 4096 else 1 if n <= 8192 else 0


# ---- script criteria ------------------------------------------------------

def red_levels(v, key):
    listed = {(s, e) for _, s, e in v.p["named"]}
    out = []
    for t in key["failing_tests"]:
        P = v.full(t["panic_line"])
        M = [v.full(m) for m in t["message_lines"]]
        header_listed = tuple(t["name_range"]) in listed
        name_visible = header_listed or v.full(t["status_line"]) or v.full(t["block_header"]) \
            or v.full(t["failures_list_entry"])
        if P and all(M):
            lv = 4
        elif P and any(M):
            lv = 3
        elif P:
            lv = 2
        elif name_visible or any(M):
            lv = 1
        else:
            lv = 0
        missing = ([] if P else [t["panic_line"]]) + [m for m, ok in zip(t["message_lines"], M) if not ok]
        reasons = sorted({o["reason"] for r in missing for o in v.omitted_over(r)})
        invisible = not header_listed and not any(v.any(r) for r in
                                                  (t["status_line"], t["block"], t["failures_list_entry"]))
        if lv == 4:
            status = "complete"
        elif invisible:
            status = "invisible"
        elif "not_selected" in reasons:
            status = "unlocated"
        else:
            status = "located"
        out.append({"name": t["name"], "binary": t["binary"], "level": lv, "name_visible": bool(name_visible),
                    "header_listed": header_listed, "status": status, "missing_reasons": reasons,
                    "value_bearing": t["value_bearing"],
                    "left_right_carried": any(M) and not P})
    return out


def score_red(v, key):
    lv = red_levels(v, key)
    n = len(lv)
    mean = sum(x["level"] for x in lv) / n
    named = sum(1 for x in lv if x["name_visible"])
    vb = [x for x in lv if x["value_bearing"]]
    at4 = sum(1 for x in vb if x["level"] == 4)
    other_ge2 = any(x["level"] >= 2 for x in vb if x["level"] != 4)
    if at4 == 2:
        r3 = 4
    elif at4 == 1 and other_ge2:
        r3 = 3
    elif at4 == 1:
        r3 = 2
    elif any(x["level"] >= 1 and (x["left_right_carried"] or x["level"] >= 2) for x in vb):
        r3 = 1
    else:
        r3 = 0
    fr = key["failing_result_lines"]
    items = {"result_line_1": v.full(fr[0]), "result_line_2": v.full(fr[1]),
             "rerun_lines": all(v.full(r) for r in key["rerun_lines"]),
             "targets_block": v.full(key["targets_block"])}
    st = {s: sum(1 for x in lv if x["status"] == s) for s in ("complete", "located", "unlocated", "invisible")}
    if st["unlocated"] == 0 and st["invisible"] == 0:
        r5 = 4
    elif st["invisible"] == 0 and st["unlocated"] <= 2:
        r5 = 3
    elif st["invisible"] == 0 and st["unlocated"] <= 6:
        r5 = 2
    elif st["invisible"] <= 3:
        r5 = 1
    else:
        r5 = 0
    noise = v.class_bytes(key["line_classes"], {"pass", "build", "other"})
    crit = {"R1": mean, "R2": band(named, (18, 17, 15, 9)), "R3": r3, "R4": sum(items.values()),
            "R5": r5, "R6": noise_level(noise)}
    detail = {"per_test": [{k: x[k] for k in ("name", "binary", "level", "status")} for x in lv],
              "assertion_levels": [x["level"] for x in lv], "names_identified": named,
              "run_level_items": items, "status_counts": st, "noise_bytes": noise,
              "assertion_bytes_recall": round(sum(v.cov(*t["assertion"]) for t in key["failing_tests"]) /
                                              sum(t["assertion"][1] - t["assertion"][0] for t in key["failing_tests"]), 4),
              "block_bytes_recall": round(sum(v.cov(*t["block"]) for t in key["failing_tests"]) /
                                          sum(t["block"][1] - t["block"][0] for t in key["failing_tests"]), 4),
              "result_lines_carried": sum(1 for r in key["result_lines"] if v.full(r["line"])),
              "signatures_shown": sum(1 for sg in key["signatures"] if any(
                  x["level"] == 4 for x in lv if x["name"] in sg["tests"]))}
    return crit, detail


def score_green(v, key, proj):
    failure_bytes = v.class_bytes(key["line_classes"], {"failure"})
    direct = proj["named_count"] == 0 and failure_bytes == 0 and key["expected_named_count"] == 0
    covered_tests = sum(b.get("tests", 0) for b in key["binaries"] if v.full(b.get("result_line")))
    share = covered_tests / key["tests_total"]
    ig = key["ignored_tests"][0]
    il, rl = v.full(ig["line"]), v.full(ig["result_line"])
    g4 = 4 if il and rl else 3 if il else 2 if rl else 0
    noise = v.class_bytes(key["line_classes"], {"pass", "build", "other"})
    crit = {"G1": 4 if direct else 0, "G2": band(share, (0.95, 0.75, 0.50, 0.25)),
            "G3": 4 if v.full(key["tail"]) else 0, "G4": g4, "G5": noise_level(noise)}
    detail = {"tests_corroborated": covered_tests, "tests_total": key["tests_total"],
              "share": round(share, 4),
              "result_lines_carried": sum(1 for r in key["result_lines"] if v.full(r["line"])),
              "ignored_line": il, "ignored_result_line": rl, "noise_bytes": noise}
    return crit, detail


def score_core(v, key):
    s, f = v.full(key["summary_member"]), v.full(key["suite_fixtures_member"])
    c1 = 4 if s and f else 2 if s else 1 if f else 0
    named = [r for r in key["non_pass"] if all(v.full(m) for m in r["members"].values())]
    noise = sum(v.cov(a, b) for a, b in key["pass_records"])
    ident = sum(1 for r in key["identity_members"].values() if v.full(r))
    crit = {"C1": c1, "C2": 4 if v.full(key["status_note_member"]) else 0,
            "C3": 4 * len(named) / len(key["non_pass"]), "C4": noise_level(noise), "C5": ident}
    detail = {"unsupported_named": [r["fixture"] for r in named], "pass_record_bytes": noise,
              "pass_records_touched": sum(1 for a, b in key["pass_records"] if v.cov(a, b)),
              "identity_members_carried": ident}
    return crit, detail


# ---- the store, read from a private copy ---------------------------------

def open_store(data_dir):
    src = Path(data_dir) / "cbr.sqlite"
    if not src.exists():
        return None, None
    tmp = Path(tempfile.mkdtemp(prefix="j2score-"))
    for suffix in ("", "-wal", "-shm"):
        p = Path(str(src) + suffix)
        if p.exists():
            shutil.copy2(p, tmp / p.name)
    return sqlite3.connect(tmp / "cbr.sqlite"), tmp


def store_facts(data_dir):
    con, tmp = open_store(data_dir)
    if con is None:
        return None
    try:
        cols = [r[1] for r in con.execute("pragma table_info(model_ledger)")]
        ledger = [dict(zip(cols, r)) for r in con.execute("select * from model_ledger order by id")]
        recs = []
        for ident, value in con.execute("SELECT id, value FROM subjects WHERE kind = 'evidence.artifact' "
                                        "AND id LIKE 'der.%' ORDER BY id"):
            art = json.loads(value)
            if art.get("state") != "sealed" or art.get("purge") is not None:
                continue
            h = art["descriptor"]["digest"].split(":", 1)[-1]
            p = Path(data_dir) / "objects" / "sha256" / h[0:2] / h[2:4] / h[4:]
            if p.exists():
                rec = json.loads(p.read_text())
                if rec.get("question", {}).get("selector", "").startswith("project_large_result "):
                    recs.append(rec)
        return {"ledger": ledger, "records": recs}
    finally:
        con.close()
        shutil.rmtree(tmp, ignore_errors=True)


def ranges_of_offer(rec):
    out = {}
    for o in rec.get("question", {}).get("offered", []):
        a, b = o["path"].rsplit("@", 1)[1].split("-")
        out[o["id"]] = (int(a), int(b))
    return out


# ---- gates ---------------------------------------------------------------

def arm_gates(arm, packet_text, rep, kind, key, source, commit, parts_expected):
    g, notes = {}, []
    facts = json.loads(packet_text)
    packet = json.loads(base64.b64decode(facts["excerpt"]["data_base64"]))
    section = next((s for s in packet.get("sections", []) if s.get("item_id") == ITEM), None)
    g["B_satisfied_and_checked"] = (rep.get("item", {}).get("result") == "satisfied" and section is not None
                                    and rep.get("problems") == [])
    if section is None:
        return g, None, None, notes
    proj = read_projection(section["content"])
    ex = proj["extents"]
    # C: independent re-verification against the source bytes
    ok, at = True, 0
    for x in ex:
        if x["s"] != at:
            ok = False
            notes.append(f"gap/overlap at {at}")
        if x["carried"] and x["bytes"] != source[x["s"]:x["e"]]:
            ok = False
            notes.append(f"excerpt e{x['n']} is not the source's bytes")
        at = x["e"]
    ok &= at == len(source)
    for name, s, e in proj["named"]:
        if source[s:e] != name.encode():
            ok = False
            notes.append(f"named {s}-{e} is not the source's bytes")
    declared = [(f"s-{ITEM}.o{x['n']}", PROTOCOL_REASON.get(x["reason"], "?")) for x in ex if not x["carried"]]
    theirs = [(o.get("section_id"), o.get("reason")) for o in packet.get("omissions", []) if o.get("item_id") == ITEM]
    ok &= declared == theirs
    g["C_reverified"] = ok
    n_ex = sum(1 for x in ex if x["carried"])
    g["D_bounds"] = (proj["content_bytes"] <= PROJECTION_BYTES and n_ex <= MAX_EXCERPTS
                     and len(declared) <= MAX_EXCERPTS + 1 and len(proj["named"]) <= NAMED_LISTED
                     and (rep.get("parts") or 0) <= MAX_PARTS and (arm == "assisted" or not proj["unresolved"]))
    # E: header honesty
    h = proj["header"]
    m1 = re.match(r"^projection cbr-project-large-result/1 of evidence (\S+) at (sha256:[0-9a-f]{64})$", h[0] if h else "")
    m2 = re.search(r"git_commit ([0-9a-f]{40})", h[1] if len(h) > 1 else "")
    exp = key.get("expected_named", [])
    listed_ok = [(s, e) for _, s, e in proj["named"]] == [tuple(x["range"]) for x in exp[:NAMED_LISTED]]
    more_ok = (proj["more"] is None) if len(exp) <= NAMED_LISTED else (
        proj["more"] is not None and proj["more"][0] == len(exp) - NAMED_LISTED
        and (proj["more"][1], proj["more"][2]) == tuple(exp[NAMED_LISTED]["range"]))
    g["E_header_honest"] = bool(m1 and m1.group(2) == key["sha256"] and m2 and m2.group(1) == commit
                                and proj["named_count"] == key["expected_named_count"]
                                and (kind != "red" or (listed_ok and more_ok))
                                and (kind == "red" or not proj["named"]))
    how = next((l for l in h if l.startswith("read as ")), "")
    pm = re.search(r" (\d+) parts;", how)
    parts = int(pm.group(1)) if pm else None
    want = "chosen by the deterministic rule" if arm == "baseline" else "chosen by the model, one question per part"
    g["F_how_line"] = want in how and parts is not None and (parts_expected is None or parts == parts_expected)
    v = View(proj)
    # G (baseline half): no failure span of the key sits in a not_selected extent
    if arm == "baseline":
        spans = []
        if kind == "red":
            for t in key["failing_tests"]:
                spans += [t["status_line"], t["block"]]
            spans += [e["atom"] for e in key["cargo_errors"]] + key["failing_result_lines"]
        elif kind == "core":
            spans += [r["range"] for r in key["non_pass"] if r["outcome"] not in ("unsupported", "skipped")]
        bad = [s for s in spans if any(o["reason"] == "not_selected" for o in v.omitted_over(s))]
        g["G_reasons_honest"] = not bad
        if bad:
            notes.append(f"{len(bad)} failure spans declared not_selected")
    return g, proj, v, notes


def selection(v, recs, key, kind):
    """Model choices vs what was carried; relevant = a unit touching a key fact."""
    if kind == "red":
        rel = [(s, e) for s, e, c in key["line_classes"] if c == "failure"]
    elif kind == "core":
        rel = [tuple(r["range"]) for r in key["non_pass"]]
    else:
        rel = [tuple(t["line"]) for t in key["ignored_tests"]]
    touch = lambda r: any(a < r[1] and r[0] < b for a, b in rel)  # noqa: E731
    parts, chosen_all, not_carried, bad_reason = [], [], [], []
    for rec in sorted(recs, key=lambda r: r["question"]["selector"]):
        offer = ranges_of_offer(rec)
        ids = rec.get("answer", {}).get("chose_ids", []) or []
        ch = [offer[i] for i in ids if i in offer]
        chosen_all += ch
        parts.append({"selector_part": rec["question"]["selector"].rsplit(" part ", 1)[-1],
                      "offered": len(offer), "relevant_offered": sum(1 for r in offer.values() if touch(r)),
                      "chosen": len(ids), "chosen_relevant": sum(1 for r in ch if touch(r)),
                      "unknown_ids": [i for i in ids if i not in offer]})
    for r in chosen_all:
        if not v.full(r):
            not_carried.append(r)
            if any(o["reason"] != "over_projection" for o in v.omitted_over(r)):
                bad_reason.append(r)
    flags = []
    if parts and all(p["chosen"] == 0 for p in parts):
        flags.append("chose-none")
    if parts and all(p["offered"] and p["chosen"] >= 0.9 * p["offered"] for p in parts):
        flags.append("chose-all")
    if not_carried:
        flags.append(f"chosen-but-not-carried:{len(not_carried)}")
    return {"parts": parts, "flags": flags, "chosen_ranges_misdeclared": bad_reason}


def run_score(run, out, key_all, inputs_dir, dry, judged):
    rid = run["id"]
    kind = KIND_OF_INPUT.get(run["input"]["name"])
    key = key_all["inputs"][kind]
    source = (Path(inputs_dir) / run["input"]["name"]).read_bytes()
    res = {"id": rid, "kind": kind, "model": run.get("model"), "gates": {}, "notes": [], "arms": {}}
    rg = res["gates"]
    rg["A_right_input"] = (run["input"]["digest"] == key["sha256"]
                           and "sha256:" + hashlib.sha256(source).hexdigest() == key["sha256"]
                           and run["input"]["size"] == key["size"])
    rg["A_right_model"] = run.get("model") == MODELS.get(rid.split("-", 1)[1])
    parts = run.get("baseline", {}).get("parts")
    store = store_facts(Path(out) / "work" / rid / "data")
    for arm in ("baseline", "assisted"):
        pf = Path(out) / f"{rid}.{arm}.packet.json"
        rep = run.get(arm, {})
        a = {"gates": {}, "criteria": {}, "detail": {}}
        res["arms"][arm] = a
        if not pf.exists():
            a["gates"]["B_satisfied_and_checked"] = False
            continue
        g, proj, v, notes = arm_gates(arm, pf.read_text(), rep, kind, key, source, run["commit"], parts)
        a["gates"].update(g)
        res["notes"] += [f"{arm}: {n}" for n in notes]
        if proj is None:
            continue
        if kind == "red":
            crit, det = score_red(v, key)
        elif kind == "green":
            crit, det = score_green(v, key, proj)
        else:
            crit, det = score_core(v, key)
        a["criteria"], a["detail"] = crit, det
        a["detail"].update({"content_bytes": proj["content_bytes"], "excerpts": len(v.carried),
                            "omissions": len(v.omitted), "carried_bytes": sum(e - s for s, e in v.carried),
                            "unresolved": len(proj["unresolved"]),
                            "identity_lines_in_not_selected": sum(
                                1 for s, e, c in key.get("line_classes", []) if c == "identity" and any(
                                    o["reason"] == "not_selected" and o["s"] <= s and e <= o["e"] for o in v.omitted))})
        masked = re.sub(r"(of evidence )\S+", r"\1[artifact]", _content(pf), count=1)
        a["detail"]["masked_sha256"] = hashlib.sha256(masked.encode()).hexdigest()
        if arm == "baseline":
            a["detail"]["C0_matches_dry_run"] = a["detail"]["masked_sha256"] == DRY_BASELINE_MASKED_SHA256[kind]
        if arm == "assisted":
            rep_r = run.get("replay", {})
            a["gates"]["H_replay"] = (rep_r.get("item", {}).get("result") == "satisfied"
                                      and rep_r.get("sections_identical") is True and rep_r.get("differences") == []
                                      and rep_r.get("ambiguous") == [] and rep_r.get("questions") == parts
                                      and rep.get("investigation") == parts)
            recs = [r for r in (store or {}).get("records", [])
                    if r.get("question", {}).get("selector", "").split(" ")[2:3] == [run.get("artifact")]]
            sel_ok = store is not None and len(recs) == parts and run.get("part_records") == parts
            sel_ok &= sorted(r["question"]["selector"].rsplit(" part ", 1)[-1] for r in recs) == \
                sorted(f"{k} of {parts}" for k in range(1, (parts or 0) + 1))
            for r in recs:
                offer = ranges_of_offer(r)
                sel_ok &= (r.get("model") == run.get("model") and r["question"].get("model") == run.get("model")
                           and r.get("item") == ITEM and r["question"].get("task") == TASK
                           and all(i in offer for i in (r.get("answer", {}).get("chose_ids") or [])))
                if not dry:
                    sel_ok &= (r.get("usage", {}).get("input_tokens") or 0) > 0
            a["gates"]["I_records"] = bool(sel_ok)
            if store is not None:
                sel = selection(v, recs, key, kind)
                a["detail"]["selection"] = sel
                a["gates"]["G_reasons_honest"] = not sel["chosen_ranges_misdeclared"]
            else:
                a["gates"]["G_reasons_honest"] = False
                res["notes"].append("assisted: no store, choices unverifiable")
    # cost
    led = (store or {}).get("ledger", [])
    usage = [r for r in led if r.get("kind") == "usage"]
    recs_all = (store or {}).get("records", [])
    tokens = run.get("tokens", 0)
    rg["I_spend"] = (store is not None and sum(r["tokens"] for r in usage) == tokens
                     and all(r.get("request") == "assisted" for r in usage)
                     and sum(r.get("usage", {}).get("tokens", 0) for r in recs_all) == tokens
                     and tokens <= run.get("launch_ceiling", 0) and tokens <= WORST_CASE_PROJECTION_TOKENS)
    inp = sum(r.get("usage", {}).get("input_tokens") or 0 for r in recs_all)
    res["cost"] = {"tokens": tokens, "calls": len(run.get("charges", [])), "input_tokens": inp,
                   "output_tokens": tokens - inp if inp else None,
                   "repairs": sum(r.get("usage", {}).get("repairs", 0) for r in recs_all),
                   "latency_ms": [r.get("latency_ms") for r in recs_all],
                   "estimate": sum(r.get("estimate", 0) for r in usage),
                   "part_records": run.get("part_records"), "parts": parts,
                   "whole_artifact_tokens_ref": round(key["size"] / 4),
                   "tokens_over_whole_artifact": round(tokens / (key["size"] / 4), 2),
                   "launch_ceiling": run.get("launch_ceiling")}
    # totals and verdicts
    W = WEIGHTS[kind]
    for arm, a in res["arms"].items():
        crit = dict(a["criteria"])
        jd = (judged or {}).get(rid, {}).get(arm, {})
        for j in JUDGED:
            vals = jd.get(j) or []
            if len(vals) >= 2:
                crit[j] = statistics.median(vals)
                a["detail"][f"{j}_spread_flag"] = max(vals) - min(vals) > 1
        a["criteria"] = crit
        a["S_script"] = round(sum(W[c] * crit[c] / 4 for c in W if c not in JUDGED and c in crit), 2)
        a["S_script_max"] = sum(w for c, w in W.items() if c not in JUDGED)
        a["complete"] = all(j in crit for j in JUDGED)
        a["S"] = round(sum(W[c] * crit[c] / 4 for c in W if c in crit), 2) if a["complete"] else None
        a["gates_pass"] = bool(a["gates"]) and all(a["gates"].values()) and all(
            v for k, v in rg.items() if not (dry and k == "A_mode_live"))
        if not a["gates_pass"]:
            a["verdict"] = "INVALID"
        else:
            pct = a["S"] if a["complete"] else 100 * a["S_script"] / a["S_script_max"]
            crits_ok = all(crit.get(c, 4) >= 2 for c in CRITICAL[kind])
            word = "USEFUL" if pct >= 75 and crits_ok else "PARTIAL" if pct >= 50 else "NOT USEFUL"
            a["verdict"] = word if a["complete"] else f"PROVISIONAL {word}"
    res["comparison"] = compare(res, kind, judged)
    ds = res["comparison"].get("dS_script")
    res["cost"]["script_points_per_10k_tokens"] = (round(ds / (tokens / 10000), 3) if tokens and ds is not None else None)
    res["cost"]["verdict"] = cost_verdict(ds, tokens)
    return res


def _content(pf):
    facts = json.loads(Path(pf).read_text())
    packet = json.loads(base64.b64decode(facts["excerpt"]["data_base64"]))
    return next(s for s in packet["sections"] if s.get("item_id") == ITEM)["content"]


def compare(res, kind, judged):
    b, a = res["arms"]["baseline"], res["arms"]["assisted"]
    if not b.get("gates_pass"):
        return {"verdict": "NOT MEASURABLE", "why": "baseline or run-level gate failed"}
    if not a.get("gates_pass"):
        return {"verdict": "WORSE", "why": "assisted arm INVALID while the baseline holds"}
    ds = round(a["S_script"] - b["S_script"], 2)
    why = []
    if kind == "red":
        lost = [x["name"] for x, y in zip(a["detail"]["per_test"], b["detail"]["per_test"]) if x["level"] < y["level"]]
        if lost:
            why.append(f"red non-inferiority: {len(lost)} tests at a lower level than the baseline")
    if ds <= -5:
        why.append(f"dS_script {ds}")
    complete = a["complete"] and b["complete"]
    dJ = round(a["criteria"]["J1"] - b["criteria"]["J1"], 2) if complete else None
    if dJ is not None and dJ <= -1:
        why.append(f"dJ1 {dJ}")
    prefs = (judged or {}).get(res["id"], {}).get("prefer", [])
    majority = next((p for p in ("baseline", "assisted", "none") if prefs.count(p) >= 2), None)
    dS = round(a["S"] - b["S"], 2) if complete else None
    if why:
        word = "WORSE"
    elif ds >= 5 and (dJ is None or dJ >= 0) and majority != "baseline":
        word = "BETTER"
    else:
        word = "EQUAL"
    notes = []
    if word == "EQUAL" and dS is not None and abs(ds) < 5 and abs(dS) >= 5:
        notes.append("difference from judges only, not a result")
    if word == "WORSE" and all(w.startswith("dJ1") for w in why):
        notes.append("judged only")
    if (word == "BETTER" and majority == "baseline") or (word == "WORSE" and majority == "assisted"):
        notes.append("contested")
    return {"verdict": word if complete else f"PROVISIONAL {word}", "dS_script": ds, "dS": dS, "dJ1": dJ,
            "pairwise": prefs, "pairwise_majority": majority, "why": why, "annotations": notes}


def cost_verdict(ds, tokens):
    if not tokens:
        return "no spend"
    if ds is None or ds <= 0:
        return "NO RETURN"
    rate = ds / (tokens / 10000)
    return "JUSTIFIED" if rate >= 1.0 else "EXPENSIVE" if rate >= 0.25 else "NOT JUSTIFIED"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, action="append", help="a j2_run.py output dir; repeat for each invocation")
    ap.add_argument("--key", required=True)
    ap.add_argument("--inputs", required=True)
    ap.add_argument("--judged")
    ap.add_argument("--dry", action="store_true")
    ap.add_argument("--json")
    a = ap.parse_args()
    key = json.loads(Path(a.key).read_text())
    judged = json.loads(Path(a.judged).read_text()) if a.judged else None
    reports = [(o, json.loads((Path(o) / "report.json").read_text())) for o in a.out]
    runs = [run_score(r, o, key, a.inputs, a.dry, judged) for o, rep in reports for r in rep["runs"]]
    ids = [r["id"] for _, rep in reports for r in rep["runs"]]
    report_gates = {"A_mode_live": all(rep.get("mode") == "live" for _, rep in reports) or a.dry,
                    "A_not_stopped": all("stopped" not in rep for _, rep in reports),
                    "A_six_runs": sorted(ids) == sorted(RUN_IDS) or a.dry,
                    "A_total_within_ceiling": all(rep.get("tokens", 0) <= rep.get("run_ceiling_tokens", 0)
                                                  for _, rep in reports)}
    all_gates = all(report_gates.values()) and all(ar["gates_pass"] for r in runs for ar in r["arms"].values())
    complete = all(ar["complete"] for r in runs for ar in r["arms"].values())
    fails = []
    for r in runs:
        if r["comparison"]["verdict"].endswith("WORSE"):
            fails.append(f"{r['id']} WORSE")
        for arm, ar in r["arms"].items():
            need = "USEFUL" if r["kind"] == "red" else "PARTIAL"
            v = ar.get("verdict", "INVALID").replace("PROVISIONAL ", "")
            if v == "INVALID" or (need == "USEFUL" and v != "USEFUL") or v == "NOT USEFUL":
                fails.append(f"{r['id']} {arm} {ar.get('verdict')}")
    overall = ("PASS" if all_gates and not fails else "FAIL")
    overall = overall if complete and not a.dry else f"PROVISIONAL {overall}"
    out = {"report_gates": report_gates, "runs": runs, "j2_live": overall, "failing": fails}
    if a.json:
        Path(a.json).write_text(json.dumps(out, indent=1, default=str))
    print(f"report gates: {report_gates}")
    for r in runs:
        c = r["comparison"]
        print(f"\n{r['id']} ({r['kind']}, {r['model']}) cost {r['cost']['tokens']} tokens, "
              f"{r['cost']['calls']} calls, {r['cost']['repairs']} repairs, "
              f"T/W {r['cost']['tokens_over_whole_artifact']} -> {r['cost']['verdict']}")
        for arm, ar in r["arms"].items():
            bad = [k for k, v in ar["gates"].items() if not v]
            crit = " ".join(f"{k}={round(v, 2)}" for k, v in ar["criteria"].items())
            print(f"  {arm:9s} S_script {ar.get('S_script')}/{ar.get('S_script_max')} S {ar.get('S')} "
                  f"{ar.get('verdict')} | {crit} | failed gates {bad or '-'}")
        print(f"  comparison: {c['verdict']} dS_script={c.get('dS_script')} {c.get('why') or ''} "
              f"{c.get('annotations') or ''}")
        for n in r["notes"]:
            print(f"  note: {n}")
    print(f"\nJ2 live: {overall} {fails or ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
