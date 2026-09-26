# J2 live rubric v1.1 (mechanical changes for `/2`, frozen before the rerun)

Written 2026-09-26 against m5a-3 (PR #44, `m5a-3/floor` at `e3a6d6d`), before the rerun and before any `/2` live output existed. **Judges never see this file.**

**v1.1 is [v1](../RUBRIC.md) with only the mechanical changes below.** v1's criteria, weights, bands, answer key, judge protocol, verdict rules and [§7's pass statement](../RUBRIC.md#7-what-j2-live-passes-means-pre-declared) are unchanged and apply to the rerun. v1's files are unchanged and stay byte-identical: `RUBRIC.md` `0f7a6910…4214f`, `answer_key.json` `e1e2c91e…0ebd5`, `answer_key.py` `effb5860…c529`, `score_j2.py` `caa5551e…97b7b3`, `blind.py` `de600bc8…97f8`, and `run-2026-09-25/`. v1's first result stays the result of the first run. The rerun is reported under v1.1 and, beside it, under v1 as it was frozen.

| v1.1 file | sha256 |
|---|---|
| `score_j2.py` | `2b05be95d21b0212dcdb2390a658b282d43905568313450ae7c119d01975475b` |

This file cannot list its own digest. Its digest is in the commit that adds it.

## Why a v1.1 is needed

m5a-3 changes what the projection prints, which is the text v1's scorer parses. It does not change the task, the inputs or the answer key. Its main change is that the rule's projection becomes a floor, and a model is asked only what to add to it. In the header, m5a-3:

- moves the format to `cbr-project-large-result/2`;
- adds a line `failing tests: N; cargo errors: M` just before `failures named:` (finding F1);
- labels the model arm as the rule's floor plus what the model added: `…; excerpts chosen by the deterministic rule: failures, then run identity; then chosen by the model, one question per part, which added e2, e3, e4` (or `which added nothing`);
- asks no question that no answer could change. A 0-part run makes no call. Its assisted section equals the baseline's byte for byte, and it is not replayed;
- raises the computed worst case of a whole projection from 488,160 to 492,204 tokens.

Read literally, v1 fails every one of the six `/2` dry runs on these format changes alone ([below](#under-v1-literally)). No projection has to get worse for that to happen. v1.1 reads the new text and checks the same facts.

## The changes

| | What m5a-3 changed | What v1.1 changes | Why it is mechanical |
|---|---|---|---|
| **a. Gate E** | The format is `/2`, and an F1 line comes before `failures named:`. | E requires `cbr-project-large-result/2`. It reads the F1 line directly before `failures named:` and requires its counts to equal the key's. Red has 18 failing tests (`failing_tests`) and 3 cargo errors (`cargo_errors`). Green and core have 0 and 0. Every v1 check of E is kept: digest, commit, `failures named` against the key (21 / 0 / 0), and on red the first 16 listed ranges and `and 5 more, the first at bytes 9879-9968` against the key. | E checks the same facts against the same key. The only addition, the F1 line, is checked against counts the key already holds. |
| **b. C0** | Changing the header changes the masked baseline's digest. | The digests are re-pinned to the `/2` dry run at m5a-3's head. Red: `e7d389553f098504be79f6580399d734a9d84a3963a7070f1ad90a15a6ff04f7`. Green: `65666f5efc2c55dec9825f32ed1045473047765181dd8dd1e5e7471b36382a8f`. Core: `9a6f45883f67f971276d8b93e5d9a87357445c63117cbc64860b45c85e484b8a`. | Same masking rule. The pins are those in [READINESS §10](../../../work/m5/READINESS.md), as published on `m5a-3/floor`. They are reproduced here by the scorer (below). |
| **c. Gate I** | The worst case of a whole projection is 492,204, computed by m5a-3's tests. | The worst case is 492,204. | It is the same bound, recomputed for the new code. |
| **d. Gate F** | Both arms now name the rule. The model arm names it and then says what the model added. | The baseline says `chosen by the deterministic rule` and not `chosen by the model`. An assisted arm that asked ends `; then chosen by the model, one question per part, which added` followed by `nothing` or `eN, …`. Both arms still print the same number of parts, which is the baseline's. | F still checks that each arm says who chose. It reads the new wording. |
| **e. A 0-part run** | When the baseline's header says `0 parts`, the harness still makes the assisted request, which must spend nothing and is not replayed. | Such a run is **EQUAL by construction**. For it, F on the assisted arm, H and I are replaced by seven checks: from the harness's report, `part_records == 0`, the run's tokens are `0`, no charges are listed and there is no `replay` key; from the store, which the report is never trusted over, the ledger holds no row at all and no part record is sealed; and the assisted section equals the baseline section byte for byte. Its comparison verdict is `EQUAL`, and its cost verdict is `NO RETURN` at 0 tokens. The baseline arm's own F still reads its label and `0 parts`. B, C, D, E and G still apply to both arms. | With no question, no model acted and there is nothing to replay. The seven checks prove it, the store's two against what the provider wrote rather than what the harness reported, and two byte-identical sections can only compare EQUAL under §6. |
| **f. v1 mode** | — | `--rubric v1` applies v1's checks unchanged. Per run, it names each v1 check that fails literally and why. `--rubric v1.1` is the default. It refuses any packet that is not `/2` and exits 2. | This is reporting only. On the `/2` dry run and on v1's own live output, v1 mode's scores are v1's scorer's scores exactly (below). |

Nothing else changes. The reader, the criteria functions, the store reads, the verdict thresholds and the pass statement are v1's code, and the scorer's `--rubric v1` path proves that. **Two robustness changes in v1.1's path only**, found by the check below: a section the declared format cannot read is a failed `C` gate, an INVALID arm, where v1's reader stopped the scorer; and gate E's F1 expectation reads each kind's own answer-key field and fails on a missing one, where a default of 0 would have hidden a renamed field.

**How v1.1 was checked before it was frozen.** An agent that did not write it mapped every hunk of the diff from v1's scorer to (a)–(f), ran it over the `/2` dry runs, and attacked it with tampered copies. One attack passed the first version: a 5,000-token usage row written into a zero-part run's store, with the report left alone, because the zero-part gates read only the report. The store checks of (e) close it; every attack — that row, a one-byte change in the header and at the tail, a part record, tokens, a replay key, and the worst case at 492,204 and 492,205 — was then rerun and each is caught, and the dry runs at `80cdfbe` pass every gate with C0 matching.

## How to run it

```
cd docs/verification/j2-live
python3 v1.1/score_j2.py --rubric v1.1 --out <out 1> --out <out 2> --key answer_key.json --inputs <inputs> [--judged judged.json] [--json score.json]
python3 v1.1/score_j2.py --rubric v1   --out <out 1> --out <out 2> --key answer_key.json --inputs <inputs> [--judged judged.json]
```

## Pre-registered: the `/2` dry run under v1.1

These results come from the two dry runs at m5a-3's head, one for each live manifest, against the labelled fake, with `--dry`:

- **All gates hold on all six runs and all twelve arms.**
- **C0 matches on all six runs.**
- **Red and green are EQUAL by construction**, with 0 parts, 0 tokens, no replay and identical sections.

| Run | Baseline S_script | Assisted S_script | Comparison | Cost |
|---|---|---|---|---|
| red, both models | 70.0 / 70 (R1 4, R2 4, R3 4, R4 4, R5 4, R6 4) | 70.0 / 70 | EQUAL by construction | NO RETURN, 0 tokens |
| green, both models | 40.0 / 65 (G1 4, G2 1, G3 0, G4 2, G5 4) | 40.0 / 65 | EQUAL by construction | NO RETURN, 0 tokens |
| core, both models | 45.0 / 65 (C1 4, C2 4, C3 0, C4 4, C5 4) | 45.25 / 65 (the fake adds e2, e3, e4: C3 0.8, C4 3) | EQUAL | NOT JUSTIFIED (the fake's 15,000 tokens) |

The `/2` baselines score what v1's baselines scored in [v1 §8](../RUBRIC.md#8-pre-registered-results-dry-run-fake-model-answering-u1-the-baseline-is-deterministic).

What follows for the rerun is stated now so that it is not misread later:

- **Only the core runs can move.** Red and green cannot be WORSE or BETTER.
- **Red's assisted arms are the baseline's.** Their per-test levels, and so their verdict, are whatever the baseline's are. Judging still decides whether that verdict is USEFUL.
- **§7 can pass although no model chooses anything useful.** It passes when every gate holds, both core runs are no worse than their baselines, the arms meet §7's levels and judging completes. If every run is EQUAL, §7's EQUAL wording applies. **Such a pass shows that the model did no harm. It does not show that a model selects better than the rule.**

## Under v1, literally

The same dry runs scored with `--rubric v1` come out **every arm INVALID and every run NOT MEASURABLE**, for these reasons only:

- **E, all twelve arms.** The header says `cbr-project-large-result/2`, and v1 reads `/1` only. Every other part of E holds under v1's reader, which treats the F1 line as one more header line.
- **F, the four assisted arms of red and green.** The assisted arm asked nothing, so it carries the rule's label `excerpts chosen by the deterministic rule: failures, then run identity`, not `chosen by the model, one question per part`. On core, v1's substring check passes, because `/2`'s model label contains v1's words.
- **H, the same four arms.** There is no replay key, because a run that asks nothing is not replayed. `investigation` is 4, while `parts` is 0: the harness gives the assisted request every part a projection may ask, and nothing is asked.
- **C0, all six baselines.** The masked digests differ from v1's `/1` pins. This is the format change, and it is not a gate.

v1 mode's scores match v1's frozen `score_j2.py` exactly, JSON aside from the added `why`, `rubric` and `zero_part` fields. They match both on the `/2` dry run above and on the 2026-09-25 `/1` live output, where every gate still holds and red is still WORSE on both models.

## What v1.1 refuses

v1.1 is not meant to score `/1` output. Given the 2026-09-25 live output directories, it prints `REFUSED`, names each of the twelve packets as `cbr-project-large-result/1`, and exits 2 without scoring. The same happens when one `/1` packet is mixed in with `/2` packets.

## Checked before freezing

The v1.1 scorer was run against tampered copies of the `/2` dry runs, one change at a time. Each change was caught:

- F1 says 17 failing tests: E fails.
- F1 line removed: E fails.
- An asking assisted arm carries `/1`'s label: F fails, and the run is WORSE.
- The baseline carries the model's label: F fails, and the run is NOT MEASURABLE.
- A 0-part run spends a token, has a replay key or has `part_records` 1: the matching Z check fails, and the run is WORSE.
- A 0-part assisted section differs from the baseline by one byte: the section check fails, and the run is WORSE.
- A baseline drifts from its pin: C0 reports it.
- A run spends 492,205 tokens: I fails, naming the worst case.
- `and 5 more` moved by one byte: E fails.
- A `/1` packet is mixed in: the scorer refuses.

The 492,205 case also broke the ledger sums, so it does not test the new bound in isolation.
