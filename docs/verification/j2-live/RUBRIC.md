# J2 live rubric (final, frozen before the live run)

Written 2026-09-25 against `cbr` at `041ad5f`, before any live output existed. It is the result of combining three drafts (triage, adversarial, baseline-cost); §11 records which draft won each conflict. **Judges never see this file.**

| Frozen file | sha256 |
|---|---|
| `answer_key.json` (written by `answer_key.py`, sha256 `effb5860…c529`) | `e1e2c91eee2a5b1abdb25d987bb4994adbed17636ac1883dfcf2440ef0c5ebd5` |
| `score_j2.py` | `caa5551e2531c18c7b2c45eea5565f9c9fc7c6919294f66e78ba3e538997b7b3` |
| `blind.py` | `de600bc885a9bdba95c2272a77ee08811920f1907dbc5d6f3c3a3d1bf0a197f8` |

Any change after live output exists is a new version. It is reported beside the frozen result and never replaces it.

## 1. What is scored, and the principle

The reader is a consumer who receives the `log` section instead of the artifact, with the task *"which tests failed, and what did they assert"*. Each of the six runs has two arms: the **baseline** (investigation 0, the deterministic rule) and the **assisted** arm (a model chooses per part).

- **Gates** are about mechanics. They earn no points. If a gate fails, the arm is INVALID.
- **SCRIPT criteria** are functions of byte ranges: key spans (computed from the source bytes by `answer_key.py`, never by CBR's parser) are checked against the packet's ledger.
- **JUDGED criteria** are blinded, anchored 0–4 scores. The final value is the median of 3 judges.
- A well-formed packet is not proof of usefulness, and a valid citation is not semantic support (JOURNEYS §1). Cost is reported beside every verdict.

Definitions:
- `full(r)` means every byte of the range r lies inside `[eN]` extents.
- A criterion contributes `weight × level / 4`.
- `S` is the sum over all criteria, out of 100.
- `S_script` is the sum over the SCRIPT criteria only.

## 2. The answer key (`answer_key.json`, all 20 given facts hold, no mismatch)

**Red** (`cargo-test-f73415d.log`, 65,257 B):
- 18 failing tests: 6 in `j2_harness` and 12 in `journey_two`.
- Recorded for each test: its `test … FAILED` line, the name range inside that line, the `---- … stdout ----` block, the panic line, each message line, and the assertion. The assertion runs from the panic line through the last message line, and leaves out cargo's `note: run with RUST_BACKTRACE` line.
- 5 signatures: `j2_harness.rs:36:5` (5 tests), `:503:5` (1), `journey_two.rs:467:46` (10), `:1070:5` (1) and `:788:9` (1). The last two are the only tests with left/right values.
- 3 cargo error atoms:
  - two rerun lines, at 7574–7638 and 15840–15905
  - the targets block, at 65161–65256
- 21 expected named failures: 18 tests plus 3 cargo lines, in byte order. The 17th is at 9879–9968.
- 40 `test result:` lines, totalling 582 passed, 18 failed and 1 ignored. The two FAILED result lines are at 7475–7572 and 15740–15838.

**Green** (`cargo-test.log`, 66,535 B):
- 40 result lines, all `ok`: 644 passed, 0 failed, 1 ignored, 645 tests in all.
- No FAILED line, no panic and no `error:` line.
- The ignored test's line is at 19949–20081, and its binary's result line is at 20083–20176.
- The tail (the last result line through EOF) is at 66440–66535.

**Core** (`conformance-core.manifest.json`, 61,621 B):
- 135 results: 130 `pass` and 5 `unsupported`, with no failure outcome.
- The 5 unsupported records are `results[52]`, `[59]`, `[60]`, `[67]` and `[69]`. Each is recorded with its `{…}` range and the ranges of its `fixture`, `outcome` and `reason` members.
- Also recorded: `summary` at 61565–61619, `suite.fixtures` at 61377–61392 and `status_note` at 61130–61358.

**Line classes (libtest):** failure, identity (`Running`, `Doc-tests`, `running N`, `test result: ok`), pass, ignored, build, blank, other. The key contains no absolute path. Lines that carry one, such as the five `j2_run.py is missing` messages, are recorded by offset and sha256 only.

## 3. Gates (unweighted; every one must hold)

| Gate | Check, by `score_j2.py` |
|---|---|
| A: run | Every report has `mode == "live"` and no `stopped`. The six run ids are present. Each report's tokens are ≤ its `run_ceiling_tokens`. Each input's digest and size equal the key's. Each run's model matches its id: `m27hs` is MiniMax-M2.7-highspeed and `m3` is MiniMax-M3. |
| B: admitted | The item is `satisfied`, the section is present, and `j2_run` reports `problems == []`. |
| C: re-verified | The scorer's own reader, working from the frame counts, confirms: the ledger tiles `[0,size)`; every `[eN]` body equals the source bytes; every named range equals the source bytes; and the packet's `log` omissions equal the ledger's `[oN]`, in order, with protocol reasons applicability / output_capacity / unavailable. |
| D: bounds | Content ≤ 16,384 B. Excerpts ≤ 24. Omissions ≤ 25. Listed names ≤ 16. Parts ≤ 4. `unresolved:` lines appear only in the assisted arm. |
| E: header honest | The digest equals the key's. The capture `git_commit` equals the report's commit. `failures named` equals the key's count (21, 0 or 0). On red, the 16 listed ranges equal the key's first 16, and the line `and 5 more, the first at 9879-9968` is present. |
| F: arm is what it says | The baseline says `chosen by the deterministic rule` and the assisted arm says `chosen by the model, one question per part`. Both arms have the same number of parts. |
| G: reasons honest | Baseline: no key failure span (status line, block, cargo atom, FAILED result line) intersects a `not_selected` extent. Assisted: every range the model chose (taken from the sealed part records, `offered[].path` mapped through `chose_ids`) that is not carried lies only in `over_projection` extents. |
| H: replay | Assisted: the replay item is satisfied, `sections_identical`, `differences == []`, `ambiguous == []`, `replay.questions == parts`, and `investigation == parts`. |
| I: records and spend | `part_records == parts`. There is exactly one record per `part k of P` for this artifact. Each record's `model` equals the run's model, `item == "log"`, its task is the task text verbatim, and every chosen id is one that was offered. In live runs, each record has `usage.input_tokens > 0`. The ledger's usage rows sum to `report.tokens` and are all under request `assisted`, and the records' usage sums to the same total. Tokens are ≤ `launch_ceiling` and ≤ 488,160. |

**C0 (a consistency check, not a gate).** With the artifact id masked, the baseline section must equal the dry run's sha256:
- red `8fee63a1…b561b`
- green `52afd865…e679`
- core `5961b748…a6e2b8`

A mismatch means the binary drifted. The baseline is then rescored as live, and the drift is reported.

The stores are read from a private copy and never opened in place. `--dry` marks only the live-only checks as n/a: the mode, the six ids, and `input_tokens > 0`.

## 4. Weighted criteria (each kind sums to 100)

Noise level: 4 if 0 B are carried, 3 if ≤ 1,024 B, 2 if ≤ 4,096 B, 1 if ≤ 8,192 B, and 0 otherwise.

### Red cargo log (SCRIPT 70, JUDGED 30)

Each test gets a per-test level:

| Level | Condition |
|---|---|
| 4 | The panic line and every message line are `full`. |
| 3 | The panic line and some, but not all, message lines. |
| 2 | The panic line only. |
| 1 | The name is visible: listed in the header, or its status line, block header or failures-list entry is carried. Also 1 when message lines are carried without their panic line. |
| 0 | Otherwise. |

| ID | Criterion | W | How | 4 / 3 / 2 / 1 / 0 |
|---|---|---:|---|---|
| R1 | Assertions shown | 30 | SCRIPT | The mean per-test level, used directly as the level |
| R2 | Failing set identified | 10 | SCRIPT | Tests whose name is visible: 18 / 17 / 15–16 / 9–14 / ≤ 8 |
| R3 | Value-bearing assertions (the 2 left/right tests) | 5 | SCRIPT | 4: both at level 4. 3: one at 4 and the other ≥ 2. 2: one at 4. 1: values carried without their panic line, or partial. 0: neither |
| R4 | Run-level failure evidence | 10 | SCRIPT | Count of: FAILED result line 1; FAILED result line 2; both rerun lines; the targets block |
| R5 | Omissions explicit | 10 | SCRIPT | Each test is **complete** (level 4), **located** (every omission over its missing spans is `over_projection`), **unlocated** (a missing span lies in `not_selected`), or **invisible** (not listed in the header and nothing carried). Levels: 4 = no unlocated and no invisible; 3 = 0 invisible and ≤ 2 unlocated; 2 = 0 invisible and ≤ 6 unlocated; 1 = ≤ 3 invisible; 0 = otherwise |
| R6 | Noise (bytes of pass, build and other lines carried) | 5 | SCRIPT | Noise bands above |
| J1 | Answerability | 20 | JUDGED | Anchors in §5 |
| J2 | Misleadingness | 10 | JUDGED | Anchors in §5 |

Reported, not scored: assertion-bytes recall, block-bytes recall, result lines carried out of 40, signatures shown out of 5, per-test status, identity lines swallowed into `not_selected`, and the per-part selection diagnostics (offered, relevant, chosen, chosen and relevant; flags chose-none, chose-all, chosen-but-not-carried).

### Green cargo log (SCRIPT 65, JUDGED 35)

| ID | Criterion | W | How | 4 / 3 / 2 / 1 / 0 |
|---|---|---:|---|---|
| G1 | Direct answer | 10 | SCRIPT | 4 if `failures named: 0`, the key has 0 and no failure-class byte is carried; otherwise 0 |
| G2 | Byte corroboration | 20 | SCRIPT | Share of the 645 tests whose binary's result line is carried: ≥ .95 / ≥ .75 / ≥ .50 / ≥ .25 / less |
| G3 | End of log | 5 | SCRIPT | 4 if the tail 66440–66535 is carried, else 0 |
| G4 | Ignored test shown | 10 | SCRIPT | 4: both the ignored line and its result line. 3: the ignored line only. 2: the result line only. 0: neither |
| G5 | Noise | 20 | SCRIPT | Noise bands, over pass, build and other lines; the ignored line is neutral |
| J1 / J2 | Answerability / Misleadingness | 25 / 10 | JUDGED | Anchors in §5 |

### JSON manifest (SCRIPT 65, JUDGED 35)

| ID | Criterion | W | How | 4 / 3 / 2 / 1 / 0 |
|---|---|---:|---|---|
| C1 | The document's own totals | 20 | SCRIPT | 4: `summary` and `suite.fixtures` both carried. 2: `summary` only. 1: `suite.fixtures` only. 0: neither |
| C2 | What the outcomes mean (`status_note`) | 5 | SCRIPT | 4 if carried, else 0 |
| C3 | Unsupported fixtures named | 20 | SCRIPT | 4k/5, where k counts records whose `fixture`, `outcome` and `reason` are all carried |
| C4 | Noise (bytes inside `pass` records) | 15 | SCRIPT | Noise bands |
| C5 | Run identity (`format`, `participant`, `runner`, `suite`) | 5 | SCRIPT | The number carried |
| J1 / J2 | Answerability / Misleadingness | 25 / 10 | JUDGED | Anchors in §5 |

**Critical criteria.** A packet with any of these below 2 cannot be USEFUL:
- red: R1 and J1
- green: G1 and J1
- core: C1 and J1

## 5. Judge protocol

- **Packs.** `blind.py pack` builds one folder per run: `P1`…`P6`, each holding `A.txt`, `B.txt`, the source artifact, and `TASK.md`. The anchors are the J1 and J2 text in `TASK.md` and are frozen in `blind.py`.
- **Order and sides.** Pair order is sha256(`"j2-pair|"+run_id`). A is the baseline iff the first hex digit of sha256(`"j2-blind|"+run_id`) is even. Both are fixed now and reproducible. The mapping is written to `sealed/blind_key.json`, which judges never see.
- **Redaction** touches header lines only; excerpt bytes are never changed:
  - the artifact id becomes `[artifact]`
  - `; excerpts chosen by …` becomes `; excerpts chosen by [withheld]`
  - any MiniMax model name becomes `[model]`
- **Judges.** Three independent judges per pair, 18 sheets in total.
  - Judges are fresh sessions from a non-MiniMax lineage.
  - None of them is the builder, the rubric author or whoever ran the live run.
  - They have no access to the report, the costs, this rubric, the key, the script scores or each other.
  - Each judge sees **at most one pair per input kind**, so no judge meets the repeated baseline twice. That needs at least 6 judge sessions.
- **The sheet.** Each judge scores **both** arms on J1 and J2 (0–4), each with a one-line justification that cites a byte range, then states a preference: A, B or none.
- **Unblinding.** `blind.py unblind` turns the sheets into `judged.json`.
  - Final value = the median of the 3 judges.
  - A spread (max − min) greater than 1 is **flagged** and reported with all three scores. It is not resolved by discussion. The human reviewer may adjudicate, as JOURNEYS §4 prefers, and that is recorded.
  - A pair identical after redaction is an attention check: both arms get each judge's mean. A judge who scored A ≠ B on it is flagged, and the preference is forced to none.
  - Fewer than 2 sheets for a pair makes that run NOT MEASURABLE.
- **Calibration, before any live pair.** Each judge configuration is given the dry-run red pair (the fake model's section against the baseline) in a separate session that is never reused. It must prefer the baseline and score the baseline's J1 at least 2 above the fake's. A configuration that fails is replaced.

## 6. Arm comparison and verdicts

**Per arm:**

| Verdict | Condition |
|---|---|
| INVALID | Any gate fails. |
| USEFUL | S ≥ 75 and every critical criterion ≥ 2. |
| PARTIAL | S ≥ 50 and not USEFUL. |
| NOT USEFUL | S < 50. |

Without judged scores, the scorer prints `PROVISIONAL <word>`, using 100·S_script / script max.

**Per run.** ΔS_script = assisted − baseline, and ΔJ1 is the difference of the medians.

| Verdict | Condition |
|---|---|
| NOT MEASURABLE | A baseline gate or a run-level gate failed, or judging is incomplete. |
| WORSE | Any of: the assisted arm is INVALID while the baseline holds; **on red, any test is at a lower per-test level than in the baseline** (non-inferiority); ΔS_script ≤ −5; ΔJ1 ≤ −1. A WORSE caused by ΔJ1 alone is labelled "judged only" but still counts. |
| BETTER | ΔS_script ≥ +5, no WORSE condition, ΔJ1 ≥ 0, and the pairwise majority does not prefer the baseline. Judges alone never produce BETTER. |
| EQUAL | Anything else. It is annotated "difference from judges only, not a result" when \|ΔS_script\| < 5 but \|ΔS\| ≥ 5. |

A verdict is annotated "contested" when the pairwise majority prefers the arm the verdict disfavoured. The two models are never averaged; each is one sample per input.

**Cost**, per run, beside every verdict. The report carries:
- tokens T, calls, repairs, input and output tokens, and per-part latency
- the admission estimate
- T/W, where W = size/4 is roughly the tokens needed to read the whole artifact
- script points per 10k tokens

The cost verdict:

| Cost verdict | Condition |
|---|---|
| NO RETURN | ΔS_script ≤ 0 |
| JUSTIFIED | ≥ 1.0 point per 10k tokens |
| EXPENSIVE | 0.25–1.0 point per 10k tokens |
| NOT JUSTIFIED | < 0.25 point per 10k tokens |

## 7. What "J2 live passes" means (pre-declared)

> **J2 live passes if and only if:**
> 1. every gate A–I holds on all six runs and all twelve arms;
> 2. no run's verdict is WORSE, including "judged only";
> 3. all four red arms are USEFUL, and every green and core arm is at least PARTIAL; and
> 4. judging is complete (at least 2 sheets per pair).
>
> Every run's verdict, ΔS_script, ΔJ1, pairwise result, selection diagnostics and cost are reported, whatever they are.

If every run is EQUAL, the result must be worded: *"The model-assisted projection matched the deterministic rule on all six inputs at T tokens. On these inputs it could not have improved on it (§8). The run shows that the live mechanism holds under a real model, that the model did no harm, and what a projection costs. It does not show that a model selects better than the rule."*

**What a pass does not establish:**
- **Model superiority.** None of the six inputs overflows the rule, so an arm that improves cannot exist on red, and only small gains are possible on green and core.
- **Generalisation.** There is one sample per model per input.
- **Semantic correctness** beyond the byte classes in the key.
- **Cost-effectiveness against reading the artifact directly.** The estimated T/W is about 4.6 on red, 4.2 on green and 5.6 on core.

Proving superiority would need a seventh input that overflows the rule, for example more than 16 KB or more than 24 excerpts of failure text. That is the owner's scope decision.

## 8. Pre-registered results (dry run, fake model answering `u1`; the baseline is deterministic)

| Arm | S_script | Criterion levels |
|---|---|---|
| red baseline | **70.0 / 70** | R1 4.0, R2 4, R3 4, R4 4, R5 4, R6 4 (0 B noise). All 18 tests at level 4, 5 of 5 signatures, 17 of 40 result lines. |
| red fake assisted (negative control) | 22.5 / 70 → WORSE | R1 0.83 (15 tests at level 1, 3 at 0), R2 2, R3 0, R4 2, R5 1 (15 unlocated, 3 invisible), R6 3 (456 B) |
| green baseline | **40.0 / 65** | G1 4, G2 1 (173 of 645 tests; 25 of 40 lines), G3 0, G4 2, G5 4. The fake matches it (EQUAL); its 2 chosen units were declared `over_projection`. |
| core baseline | **45.0 / 65** | C1 4, C2 4, C3 0, C4 4, C5 4. The fake scores 45.25 (C3 0.8, C4 3 from 453 B of one pass record): EQUAL, NOT JUSTIFIED. |

Even with full judged marks, the fake red arm stays below 50. If the frozen rubric ever admitted it, the rubric would be broken.

**Structural expectations, stated now so that EQUAL is not misread later.** The carry order is chosen failures, then identity, then the rest.
- **Red.** The rule already carries every failure within 48 B of the limit. The best the model can do is EQUAL, so red is a non-inferiority test.
- **Green.** Identity is carried first and fills the 24 excerpts, so G2 and G3 are not expected to move in either arm (a refused identity unit is not retried). The model can gain only G4, by choosing 19949–20083 (+5), or lose G5 by adding noise.
- **Core.** Choosing the 5 unsupported records can raise C3 by up to +20. Choosing pass records costs C4.

## 9. Findings common to both arms (reported, not scored)

- **F1.** `failures named: 21` counts 3 cargo `error:` lines alongside the 18 tests.
- **F2.** A failure the model does not choose is declared `not_selected`, the same word used for irrelevant bytes. `and 5 more` hides both of the tests that carry left/right values.
- **F3.** Cargo prints the next `Running` line straight after `error: test failed, to rerun…` with no blank line between them. The diagnostic unit swallows that line, and in a model arm it is declared `not_selected`: 2 identity lines in the dry fake.
- **F4.** The green projection never carries 15 of the 40 result lines, or the log's end.
- **F5.** Core's `"coverage_limits": []` is not identity, so the baseline omits it.

## 10. Known limits of this rubric

- **The baseline is not blind to its author.** It is deterministic, and I saw it (§8) while fixing the thresholds. The anchors were written from the task, but the red bands can only confirm a perfect baseline.
- **The A/B mapping can be derived** from `blind.py`, so judges must not see that file.
- **Tells that redaction cannot remove.** A `not_selected` or `over_projection` label that differs between arms, and `unresolved:` lines, which only the model arm can have. Unresolved lines are kept and flagged.
- **Assertion coverage is measured in bytes.** A carried assertion whose message is itself uninformative (`a header, then a failures line`) still counts in full. J1 is the only place where informativeness is judged.
- **G1 largely repeats gate E.** It mostly rewards CBR's own header count.
- **Noise bands are absolute.** 1 KiB weighs the same on every input.
- **The judge configuration is unvalidated** beyond one calibration pair. Medians over 3 judges are coarse, and ΔJ1 ≤ −1 can come down to one judge's step.
- **Over-projection is not checked for padding.** Gate G checks only that chosen material is not mislabelled `not_selected`; an `over_projection` extent that holds nothing chosen or identity is not caught.
- **Cost thresholds are this rubric's own,** not the owner's. The owner may replace them before the freeze.

## 11. Conflicts resolved

| Conflict | Winner | Why |
|---|---|---|
| Judged form: a reader probe scored by script (triage J1, adversarial [P]) against anchored judge scores | baseline-cost, plus adversarial's misleadingness | The task fixes 3 judges per pair scoring both arms. The probe's content (names and assertions recoverable) is already SCRIPT (R1, R2). A probe would add a second agent population that needs its own calibration. |
| Do judges see the source or the key? | adversarial and baseline-cost (the source) | Judges can then verify cited ranges. The key would turn them into a second copy of the script. |
| Red per-test measure: triage's C2 (1/0.5/0) against adversarial's 5 levels | adversarial | Finer, and it separates a panic line without values (the 13538 cut) from full assertions. Triage's C6 status table was kept as R5. |
| Noise: precision (triage), a linear 1−n/8192 (baseline-cost) or absolute bands (adversarial) | adversarial | Comparable across arms, and independent of how much is carried. |
| Green G0 weight: 25 (triage) or 15 (baseline-cost) | neither: 10 | Gate E already checks the header count against the key (triage Q1). The weight goes to G2 and G5. |
| G2 by lines (adversarial) or by tests (triage) | triage | A reader cares how much of the suite is corroborated. Lines carried is still reported. |
| Core C3: bands (triage) or continuous 4k/5 (adversarial) | adversarial | Continuous, and "named" is defined by member ranges (triage). |
| Red non-inferiority as a WORSE trigger | adversarial and baseline-cost | Given the structure (§8), it is the only question red can answer. |
| ΔJ1 ≤ −1 alone gives WORSE | baseline-cost | Conservative. It is labelled "judged only". Judges can never produce BETTER on their own (JOURNEYS §4). |
| Determinism: gate D (adversarial) or a check C0 (triage, baseline-cost) | C0 | A drifted rule is a finding. It voids the pre-registered baseline numbers, not the run. |
| Blinding: salted (baseline-cost) or unsalted sha256 of the run id | unsalted (task) | The task requires the assignment to be fixed and reproducible now. |
| `unresolved:` lines: withheld (baseline-cost) or kept (adversarial) | adversarial | They are what a consumer reads. The tell is flagged. |
| Pass floors | triage's USEFUL/PARTIAL levels, with baseline-cost's gates-plus-no-WORSE core | A single statement that names every condition. |
| Cost measure: bytes per token (baseline-cost) or points per 10k tokens (triage) | points per 10k tokens, with T/W from baseline-cost | Tied to the same scores as the verdict. |
