# Journey verification for standalone CBR

> **Status, 2026-09-26: J9 has been run at M2, and J1 and J8 at M3c, all three with no model. J1 was revisited at M4 with a model, in live runs 1 and 3, and passed on both models in run 3. J6 has two labelled pilots, run twice with no model at M3 and rerun with a model at M4; J6 proper is unrun. J2's two negative controls hold at m5a against a labelled fake model, which is `simulated`, and J2's live run on 2026-09-25 failed: the mechanism held on every arm, and the model made the projection of a failing test log worse than the deterministic rule it replaced. m5a-3 made the parser's failures a floor a model adds to and cannot remove, `simulated` against the fake, and **J2 live, rerun on it on 2026-09-26, passed**: no run worse than the rule, every run equal to it, and on the one input where a model could have added something, it added nothing.** Every other result column below is empty on purpose. This file is the place journey evidence lands; it is linked from [VERIFICATION](../VERIFICATION.md) and aligned with the shared [verification model](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md). Scope and milestones: [RELEASE-SCOPE](../work/readiness/RELEASE-SCOPE.md).

A journey is the *journey* layer of the shared evidence ladder. Lower layers — build, component, integration — remain necessary and are recorded with the code that introduces them. They cannot substitute for a journey, and a journey cannot substitute for them.

## 1. Rules this document enforces

- **`not_evaluated` never counts as a pass.** An unavailable model, an unreachable provider or an unbuilt adapter makes a journey *indeterminate*, which blocks only the claims that need it.
- **A well-formed packet is not proof of useful memory.** Schema validity, resolvable citations and a fitting token budget are mechanical checks. They are reported separately from whether the packet helped.
- **A replayed transcript is not a live model run.** Deterministic fake models are used for fault injection and are labelled `simulated` in every row they appear in. A journey whose model was simulated may never be reported as live-model evidence.
- **Mechanical citation validity is not semantic support.** A citation that resolves to an exact byte range says nothing about whether those bytes support the sentence citing them. Semantic support is assessed separately, by an oracle that did not produce the packet.
- **Every important correctness claim carries a negative control** — a deliberately broken variant that must fail, for the stated reason, at the stated step. A control that fails for the wrong reason is a failed control. From M6 each named control in this document has a corresponding **mutant**: a build of CBR with that one guard removed, which the control must kill at its declared step, reported `WRONG-REASON` otherwise (ADR 001, question 7).
- **Cost is part of the result.** Cold initialization, maintenance, retrieval and investigation all count. A journey with no cost recorded is incomplete.

## 2. What each journey record must contain

| Field | Meaning |
|---|---|
| Intent and acceptance | What the consumer wanted, and the criterion that distinguishes success from a plausible-looking result |
| Entry point | The actual public operation or command invoked; no internal shortcut |
| Prerequisites and inputs | Registered repositories, grants, configuration, initial memory state (cold, prepared or warm-repeat) |
| Implementation basis | CBR commit, protocol pin, schema and fixture digests, OS and toolchain |
| Model and provider | Provider, model identity, configuration, capacity — or `none`, or `simulated (fake-<name>)` |
| Code and environment basis | `repository.id` and `tree` per repository, `workspace`, `dirty.snapshot_digest`, `environment`, `build` |
| Path taken | The actual evidence → derivation → revision → packet → consumer chain, by record identity |
| Packet identity | Packet ID, revision, sealed artifact reference and digest; citations; omissions with reasons; coverage frontiers and gaps |
| Durable result | What is still true and visible after a service restart, by re-reading the same identities |
| Reproduction | Exact commands, exit statuses, artifact locations |
| Cost | Model calls, tokens in and out, wall clock, time to first useful work, spend |
| Simulated or untested | Every component that was faked, and every segment of the path that was not exercised |
| Properties and limits | Which declared properties passed, which are `not_evaluated`, and what this journey does **not** establish |

## 3. Journey matrix

| # | Journey | Milestone | Needs a live model | Needs PIO | Primary negative control | Result |
|---|---|---|---|---|---|---|
| J1 | A user ingests a real repository and its decisions through the public client, requests code-flow context, and receives a useful cited packet | M3 (deterministic), revisited at M4 (model-assisted) | no for M3, yes for M4 | no | **Stale-source reuse:** move the repository to a new tree without re-ingesting; a packet claiming applicability to the new tree must not be produced. The control fails if the packet is still `applicable`. | **pass**, M3c, model `none` ([record](#j1-a-question-finds-its-own-answer-with-a-cited-packet)); revisited at M4 with a model, **pass on both models** in live run 3 ([record](#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)) |
| J2 | Large search results, test logs and JSON are processed under bounded model context; omitted material stays explicit | **M5**, its first journey — moved from M4 by the owner's decision of 2026-09-23, controls unchanged | yes | no | **Silent truncation:** remove the omission marker path; the journey must fail because omitted content is no longer declared. Also: feed an input larger than the summarizer's own capacity and require a typed insufficient-capacity result, never a silent partial summary. | **live: passed on the rerun**, 2026-09-26 at `80cdfbe`, 46,452 tokens: every gate on all twelve arms, no run worse than the rule and every run equal to it, and the model added nothing ([record](#j2-live-rerun-2026-09-26-passed--and-the-model-added-nothing)). **live: failed**, 2026-09-25 at `041ad5f`, MiniMax-M2.7-highspeed and MiniMax-M3, 120,067 tokens: every gate of the mechanism held on all twelve arms, and on the failing test log both models' projections were **worse** than the deterministic rule's, by a rubric frozen before the run ([record](#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed)). **simulated**, m5a and m5a-3 (format `/2`), model `fake`: both controls hold end to end through the public client, and a model's answer only adds to the rule's projection ([record](#j2-a-large-result-projected-with-what-was-left-out-declared--simulated)). |
| J3 | A source or requirement changes during preparation; the next delivery exposes the correction and the earlier packet and its history survive | M5 | yes | no | **Correction swallowed:** an older derivation must not become the current result for a corrected item. Removing the `corrected_during_preparation` path must make the journey fail. The earlier packet revision must still fetch its original bytes and report `current: false`. | — |
| J4 | CBR is restarted during model-assisted work; it resumes from durable state without duplicate commits or fabricated completion | M5 | yes | no | **Duplicate commit:** kill the process between the model response and the commit, restart, and require exactly one committed revision. With the commit's durable identity removed, two findings; with deduplication removed, a typed conflict and no section. That control follows the owner's decision of 2026-09-26 ([m5 READINESS](../work/m5/READINESS.md) item 9). Separately: a job whose checkpoint is intact must not report a finding it never derived. | — |
| J5 | Conflicting branches or stale evidence do not leak into another task as current truth | M5 | yes | no | **Branch leakage:** a requirement accepted only on branch B must never appear as binding in a packet scoped to branch A. Remove the scope filter and the control must catch it. | — |
| J6 | A fresh agent session uses a CBR packet on a realistic refactoring task; measure whether it preserves constraints and reduces repeated investigation. Relevance is measured by **counting downstream re-investigation of content the packet already contained**, not by self-reported prediction, and is diagnostic only — never a gate criterion (ADR 001, question 9). A labelled **pilot** of this journey runs at M3 with no model, as a steer rather than evidence. | M7 | **yes** | no | **Unsupported-assertion acceptance:** plant a claim whose cited evidence does not support it, and require either that it is not carried as `binding` or that the downstream session is not led into the error. Compared against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline. | journey **unrun**; two labelled pilots, each run twice with no model. brian2 **failed both times**, one of three required facts ([record](#j6-pilot-brian2-a-brownfield-repository-no-model--failed)); Knowscroll-v2 failed at m3d with two of three and **passed the m3e rerun** with three of three, under a caveat that travels with it ([record](#j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun)). Neither is evidence, and [neither measures a session](#what-neither-pilot-establishes). Both questions were rerun with a model at M4; in live run 3 brian2 held **two of three** on both models and Knowscroll **three of three** on both, with no claims in the store ([record](#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)). Still pilots. |
| J7 | Real public-protocol investigation and consumption with PIO, while standalone no-PIO operation still works | after M5, gated on PIO | yes | **yes** | **Recursive enrichment and reservation deadlock:** a CBR-initiated investigation must not be re-enriched through the same path, and preparation must not wait on a slot its own consumer holds. Both must be shown to fail when the guard is removed. | — |

| J8 | A required item is still unmet when the deadline passes; it stays unmet, while advisory items follow their declared fallback | M3 | **no** | no | **Required silently downgraded:** remove the guard and a required item must be reported `satisfied` at deadline, or reported under an obligation it was not submitted with. Both are failures. The live behaviour must instead be `unmet` with reason `deadline_passed`, and an advisory item under `proceed_with_gap` must be `degraded` with its reason while one under `wait_until_deadline` waits. Deadline expiry must supply no evidence and no consent. | **pass**, M3c, model `none` ([record](#j8-a-deadline-leaves-required-items-unmet-and-advisory-items-at-their-fallback)) |
| J9 | An authority transfer or epoch change invalidates the stale decision path, without editing any history | M2 | **no** | no | **Epoch ignored, or reliance reset:** a decision carrying a superseded `authority_epoch` must be refused with `stale_authority_epoch`; remove the epoch check and it commits. Separately, a transfer must **not** reset reliance — decisions recorded under an earlier epoch stay in effect until the current authority records a later decision about the same revision. A control that wipes reliance on transfer must be caught. No record is edited in either case: supersession is a later record. | **pass**, M2, model `none` ([record](#j9-an-authority-transfer-invalidates-the-stale-decision-path)) |
| J10 | A purge produces a proof-loss report, and export then restore round-trips the same identities | M6 | **no** | no | **Purge bypasses holds, or restore renames identities:** a purge blocked by an active hold must fail with `hold_active` naming the blocking holds; remove the check and it deletes. And a claim whose supporting evidence was purged must remain `accepted_for_use` with availability `purged` — never silently rejected, and never still reported `available`. After export and restore, every claim, decision, evaluation and packet must resolve under its original identity and digest; a control that reassigns local identities must be caught. | — |

### Journeys currently blocked

J7 cannot run today and will not be simulated. **J6 was unblocked on 2026-09-16** and is now a scheduling question rather than a permission one.

- **J6** needed a granted model provider and a permitted spend. Both now exist — MiniMax on the owner's subscription quota, with a bounded envelope debited before every call ([RELEASE-SCOPE §5](../work/readiness/RELEASE-SCOPE.md)). What J6 still needs is **pre-agreed thresholds derived from pilot variance**, and those must be fixed *before* the confirmatory run. A threshold chosen after seeing the result is not a threshold. **Both pilots have now run and both are scored**, twice with no model at M3 and again with a model at M4; the thresholds are still to be fixed, and until they are, J6 proper stays unrun.
- **J7** needs a PIO standalone service. PIO is being built in parallel and has no release. CBR must not depend on its unreleased work.

J8, J9 and J10 need no model at all and are reachable inside the milestones that introduce them. J2, J3, J4 and J5 each need a model for their full form; with the grant in place that is now reachable at M5, **J2 first** — it was M4's until the owner's decision of 2026-09-23 moved it, which matches [READINESS §1](../work/m4/READINESS.md#1-scope-and-the-promise), where projections were already M5's. Their fault-injection halves still run against a **labelled fake model**, which remains the right tool for fault injection and the wrong one for acceptance — the row stays `simulated` until a live run replaces it, and the existence of a budget does not change that.

## 4. Oracle independence

For J1 and J6 the question "was this packet actually useful" cannot be answered by the component that produced it, nor by a second model reading the same packet — [VERIFICATION §6](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md) and [MODEL-RUNTIME §5](../spec/MODEL-RUNTIME.md) both say a second model reading the same summary is not independent corroboration.

The assessment therefore uses, in order of preference: an executable test or trace assertion that distinguishes the intended path; a predeclared rubric applied by a human blinded to the arm where feasible; and, only as a supplement for genuinely qualitative judgments, a recorded and calibrated model grader that is never the sole oracle for a correctness claim. Shared lineage between the packet and the grader is recorded whenever it exists.

## 5. Per-journey records

Each journey gets its own section here as it is run, using the fields in §2, and links its run artifacts. A journey that fails, or that runs only partially, keeps its record with the failure described; records are not deleted to keep this page green.

### J6 pilots: the questions and the selectors, recorded before the runs

The two questions are the owner's, verbatim, and are used as the task text unchanged. **The oracles are with the reviewer; this session has not seen either.** Selector terms are drawn only from words of the question, and the choice is written down here **before** the run so it cannot be adjusted after seeing a packet. A compiler change after a pilot run is reported as one and the run is repeated in full.

| Pilot | Question, verbatim | Selector | Why those words |
|---|---|---|---|
| **brian2** | "np.concatenate on arrays with units returns a plain array with no units. Where in brian2 does the unit get lost, and what would have to change?" | `np.concatenate units` | `np.concatenate` is the **only identifier-shaped** phrase in the question, so it is the only thing anchors can resolve to a definition and its uses; the compiler splits it and also looks up `concatenate`. `units` is the one domain word that separates the unit machinery from the rest of a large library. Both are words of the question. |
| **Knowscroll-v2** | "When a spike records a provider's response, how must URLs and credentials in it be handled, and where is that enforced?" | `credentials URLs response` | The question contains no identifier-shaped word at all, so **no anchor lookup is possible** and this pilot tests lexical discovery only — which is worth stating rather than discovering afterwards. `credentials`, `URLs` and `response` are the three content words that are not common English; `spike`, `provider`, `handled` and `enforced` are either ubiquitous in that repository or too general to narrow anything. |

**A required item that is not the answer** is named in each request, as J1 does: `README.md` for brian2 and `AGENTS.md` for Knowscroll, neither of which has anything to do with its question. It exists to show that capacity is reserved for a required item before anything discovered, and its section is expected to be irrelevant to the question.

### J6 pilot, Knowscroll-v2: decision memory, no model — **failed, then passed on the rerun**

**Labelled a pilot throughout**, on the same terms as brian2. It is a steer, not evidence, and may not be cited as downstream-task evidence for any gate ([JOURNEYS §1](#1-rules-this-document-enforces), ADR 001 question 6). It was scored by the reviewer against an oracle the owner wrote; this session has not seen it.

**First run, at m3d: FAILED.** Of the three facts a correct packet had to hold, the packet held **two**, and no trap was triggered. The `git grep` baseline holds none of the three.

**Second run, at m3e: PASSED**, three of three, no trap. **The caveat is part of the result:** the compiler change that moved it was proposed by the reviewer, who holds the oracle, so the pass confirms a general fix was general and is **not independent evidence of usefulness**. brian2's rerun, which failed identically to its first run against the same change, is what shows nothing was tuned to pass.

Both first runs beat the plain lexical baseline and neither met its oracle. Both questions stay sealed and run again at M4.

Recorded in the same shape as brian2's: digests, paths, spans, counts and costs, and no content of that repository.

| Field | Record |
|---|---|
| Intent and acceptance | Decision memory: ingest a repository's own decision records, let the owner decide their standing, then ask a question whose answer depends on which of them are current. Acceptance is not this session's to judge; this record reports what the packet contained and how each part was found. |
| Entry point | `cbr ingest`, `cbr authority bind`, `cbr propose`, `cbr decide`, `cbr context`, `cbr request`, `cbr packet` and `cbr fetch` over the provider's Unix socket. The repository is registered read-only at launch by `--register-repository`; its path never crosses the wire. |
| Prerequisites and inputs | `Legend101Zz/Knowscroll-v2` at `HEAD` `3e8991e`, 18 commits, tree `10692a77…`: **145 entries, 6,743,819 bytes** — 141 regular files, 3 executable, 1 symlink. A fresh data directory, cold. **CBR read it and changed nothing in it.** |
| Question, verbatim | "When a spike records a provider's response, how must URLs and credentials in it be handled, and where is that enforced?" |
| Selector | `credentials URLs response`, chosen and [recorded above](#j6-pilots-the-questions-and-the-selectors-recorded-before-the-runs) before the run. |
| Model and provider | `none` |
| Code and environment basis | One repository at tree `10692a77…`, `workspace: dirty`, snapshot digest `sha256:fea8bb96…`. The working tree holds **21 modified tracked files and 9 untracked files**; by the owner's decision the pilot searches the tree as committed. |
| Decisions applied | **22, through the public client as owner**, each with rationale `owner decision, 2026-09-20, m3d pilot`: D-001–D-018 and D-020–D-022 **accepted for use as `binding` (21)**; D-019 **accepted for use as `evidence` (1)**, because it records what a spike verified, which is an observation rather than a rule. None rejected, none superseded, none left proposed. Verified afterwards over all 22: `binding=21 evidence=1`, nothing else. |
| Path taken | register → ingest the decisions file → bind the scope → propose 22 claims → decide all 22 → submit → index built at the basis → discovery: ranked retrieval over the view from the task, anchors for the identifier-shaped selector terms, and every claim judged against the request basis → publish → read → fetch a citation. |
| Packet identity | One revision. **31 sections, 0 omissions**, 22,205 bytes of section content against 65,536 of capacity; sealed packet 43,410 bytes, `packet.knowscroll-pilot.1`, digest `sha256:69a877b0…`. One item section, 22 claim sections, 8 discovered spans, and **no anchor section**. |
| Sections, how each was found | Below. |
| Coverage, as the packet states it | frontier `10692a77…`; **2 blobs were over the size cap** (two `.jpg` renders of 1,393,819 and 1,501,849 bytes, against a cap of 1 MiB); **117 blobs are in a language with no anchors** — of those 69 `.md`, 23 `.json`, 8 `.html`, 7 no extension, 3 `.sh`, 3 `.svg`, 2 `.txt`, 2 `.yaml`; and **9 files are untracked and in no tree, so they are not searched** — of those 8 `.md`, 1 `.html`. Behind that: 144 blobs considered (the symlink is not a blob of this tree), 142 indexed, 2,191 chunks over 138 files, 25 files in an anchored language (15 `.mjs`, 9 `.ts`, 1 `.cjs`) of which 23 produced anchors, 965 anchors in total. |
| Baseline | `git grep -w -i` at the same tree for the selector's terms, ranked by how many distinct terms a file holds. Below. The reviewer scores it against the same oracle. This is the simple lexical baseline INTERNALS §7 names; the strong native-context baseline is M7's. |
| Durable result | The packet and its cited artifacts survive in the store and re-read by identity. `cbr fetch src.2af0e8e4… --digest sha256:46b09eda…` returned 35,986 bytes **byte-identical to `git cat-file blob 2af0e8e4…`** at the named tree. |
| Reproduction | The exact commands are in the pull request. They register the checkout by path, so they reproduce only on a machine that has it. |
| Cost | Model calls 0, tokens 0, spend 0. **Submit 0.1s; index build and compile 4.7s; time to first packet 4.9s.** Store after the run **10.6 MB**, holding the ingested artifact, 22 claims, 22 decisions, the index and the packet. |
| Simulated or untested | Nothing is simulated. **Untested here:** the dirty-bytes path — the working tree has 21 modified tracked files and the pilot searched the committed tree by the owner's decision, so the modified bytes were never read; a second repository in one basis; and any model-assisted selection, which is M4. |
| **Second run, at m3e** | Same question verbatim, same selector, same tree, a fresh store, against a compiler carrying [M3e's four changes](#what-the-pilots-changed-and-what-changed-back). **13 sections, 18 omissions, 18,950 bytes of section content** against 65,536 of capacity (31 sections, 0 omissions and 22,205 bytes in the first run); sealed packet **28,195 bytes**; submit 0.1s, index build and compile **3.9s**, time to first packet **4.0s**, store **10.6 MB**; producer `cbr-context-compiler/2`. All four changes are visible in it: the coverage now names **21 tracked files modified in the working tree, all `.md`**; the required item is satisfied from `CLAUDE.md` lines 1–20, 990 bytes, with the locator saying `reached through the symbolic link AGENTS.md`, where the first run cited the link's own nine bytes; **4 of the 22 eligible claims are carried** (`d-018`, `d-021`, `d-022` as `binding`, `d-019` as `observation`) and **18 are omitted with reason `applicability`**, where the first run carried all 22 undifferentiated; and `scripts/spikes/_spike.mjs` is cited at lines 1–40, 2,218 bytes, where it was lines 1–20 and 1,271. **Rescored: PASSED, three of three required facts, no trap triggered**, where the first run held two of three. **One caveat travels with it and is not optional:** the change that moved it — a ranked span reaching its neighbours — was proposed by the reviewer, who holds the oracle. A pass under those conditions confirms that a general fix was general. **It is not independent evidence that a packet is useful.** What shows nothing was tuned to pass is brian2's failure, which is unchanged across both runs against the same compiler change. This question is sealed and runs again at M4. |
| Properties and limits | **This establishes nothing about usefulness by itself**, and nothing at all about whether a session worked better with the packet — see the statement below. What it does establish is mechanical: a repository's 22 decisions were ingested, decided by the owner through the public client, and carried into a packet with their permitted use and deciding decision attached, next to eight spans found by retrieval alone; every span cites an artifact and a byte range; and 21 `binding` sections are distinguished from 1 `observation` by the owner's decision rather than by anything the compiler inferred. **Lexical discovery finds only what shares vocabulary with the question** — all eight spans came from term overlap, and this question produced **no anchor section at all**: of the three names looked up, `credentials`, `URLs` and `response`, none is a definition or a use anchored at this tree. The anchor table's nearest entry is a different name, `URL`, a class with two rows. That was predicted in the selector table before the run and it held. |

**Sections, in packet order.** Rank is the drop order of [INTERNALS §5 step 5](https://github.com/Combraton/combraton/blob/main/docs/architecture/INTERNALS.md). Nothing here says whether a section is relevant: that is the oracle's to say.

| # | Section | Rank | How found | Bytes |
|---:|---|---|---|---:|
| 1 | `AGENTS.md`, lines 1–1, bytes 0–9 | required item, before anything discovered | named by the request; no selector term matched in the file, so its opening chunk | 177 |
| 2–22 | claims `d-001`–`d-018`, `d-020`–`d-022` | `BindingClaim` | the claim names a repository of this basis | 277–279 each |
| 23 | claim `d-019` | `CurrentClaim`, labelled `observation` | the same | 280 |
| 24 | `docs/founding/video-harness/03-HOW-IT-WORKS.md` lines 161–180, bytes 8206–9084 | `Span` | ranked partial match on the task, over the whole view | 1,075 |
| 25 | `docs/founding/PROMPT_SESSION_00.md` lines 436–450, bytes 63196–65244 | `Span` | the same | 2,235 |
| 26 | `scripts/spikes/_spike.mjs` lines 1–20, bytes 0–1101 | `Span` | the same | 1,271 |
| 27 | `docs/founding/video-harness/research/03-agentic-video-generation-continuity-eval.md` lines 466–479, bytes 92875–94923 | `Span` | the same | 2,284 |
| 28 | `docs/research/extracts/minimax-h3.md` lines 116–124, bytes 10599–12647 | `Span` | the same | 2,237 |
| 29 | `docs/founding/video-harness/CUTROOM-EXPLAINER.html` lines 501–518, bytes 45315–47363 | `Span` | the same | 2,251 |
| 30 | `docs/founding/video-harness/research/01-agent-harnesses.md` lines 372–382, bytes 87251–89299 | `Span` | the same | 2,259 |
| 31 | `docs/founding/video-harness/research/03-agentic-video-generation-continuity-eval.md` lines 480–494, bytes 95139–97187 | `Span` | the same | 2,284 |

**The `git grep` baseline at the same tree**, for the selector's three terms, word-matched and case-insensitive, ranked by how many distinct terms a file holds and then by line count. 46 files match at least one term.

| Distinct terms | Lines | Path |
|---:|---:|---|
| 3 | 9 | `docs/founding/video-harness/research/01-agent-harnesses.md` |
| 3 | 8 | `docs/founding/PROMPT_SESSION_00.md` |
| 3 | 8 | `docs/research/minimax-h3.md` |
| 3 | 6 | `docs/research/extracts/minimax-h3.md` |
| 3 | 4 | `docs/founding/video-harness/03-HOW-IT-WORKS.md` |
| 2 | 16 | `docs/research/steering-systems.md` |
| 2 | 8 | `docs/founding/video-harness/research/03-agentic-video-generation-continuity-eval.md` |
| 2 | 4 | `docs/founding/ARCHITECTURE.md` |
| 2 | 3 | `docs/design/kiosk-set.standalone.html` |
| 2 | 3 | `docs/founding/video-harness/CUTROOM-EXPLAINER.html` |

One structural difference between the two, stated without claiming it helped: **the decisions file is nearly invisible to the baseline and is 22 sections of the packet.** `steering/DECISIONS.md` holds one of the three terms — `response`, on five lines — and neither `credentials` nor `URLs`, so it ranks near the bottom of a term-overlap list. The packet carries all 22 of its decisions, with their permitted use, because they arrived through the decision path rather than through retrieval. Whether the decision that answers the question is among them is the oracle's to say.

**Two things this run found about the tool, neither of them fixed here.** Both are reported rather than acted on: a compiler change after a pilot run has to be declared and the run repeated in full, and the review's instruction for this pull request was to go no further than the golden-digest work.

1. **A dirty working tree's modified files are not named in the coverage.** `untracked_gap` reports untracked files and the case where the declared snapshot no longer matches; it says nothing about **tracked files that are modified**. This tree has 21 of them, so 21 files were searched at their committed bytes and the packet's coverage does not say so. brian2 could not have shown this: it had 130 untracked files and no modified tracked file. Against the false-absence rule of INTERNALS §5 this is a gap that is not stated.
2. **A required item was satisfied from a symlink.** `AGENTS.md` in this tree is mode `120000`, a symlink to `CLAUDE.md`. The indexer skips symlinks deliberately — a symlink's content is a path, not text of the tree — but `select_source` reads the blob directly and cited its nine bytes, `CLAUDE.md`, as the item's content. The locator is honest about what it cited and the item is reported `satisfied`. A consumer asking for `AGENTS.md` got the link target's name.

### J6 pilot, brian2: a brownfield repository, no model — **failed**

**Labelled a pilot throughout.** It is a steer, not evidence, and may not be cited as downstream-task evidence for any gate ([JOURNEYS §1](#1-rules-this-document-enforces), ADR 001 question 6). It was scored by the reviewer against an oracle the owner wrote before the run; this session has not seen it.

**Scored: FAILED.** Of the three facts a correct packet had to hold, the packet holds one — its coverage, stated well, including the untracked files. It did not fall into the trap: the use sites are labelled inferred and nothing in the packet claims the defect is there. The `git grep` baseline holds none of the three, and its top hit is what the trap warns against. **The packet beat the baseline and did not meet the oracle.** Which facts were missing is deliberately not recorded here: the same question runs again at M4 with a model, and that rerun has to be fair.

This is the **measured instance of the limit J1 already states**: lexical discovery finds what shares vocabulary with the question. J1 recorded it as a property; this is what it costs on a real question in a real repository. **The compiler was not changed in response** — tuning it against a question after seeing the result would make the M4 rerun meaningless.

| Field | Record |
|---|---|
| Intent and acceptance | Ask a brownfield repository a question about a third-party function and see what a packet finds. Acceptance is not this session's to judge: the owner wrote the facts a correct packet must contain and at least one trap, the reviewer scores the packet against them, and this record reports only what the packet contained and how each part was found. |
| Entry point | `cbr context`, `cbr request`, `cbr packet` over the provider's Unix socket. The repository is registered read-only at launch by `--register-repository`; its path never crosses the wire. |
| Prerequisites and inputs | The owner's brian2 fork, `Legend101Zz/brian2`, at `HEAD` tree `375bd477…`: 7,065 commits, 553 tracked files, **5,345,464 bytes** in the tree. A fresh data directory, cold. **CBR read it and changed nothing in it.** |
| Question, verbatim | "np.concatenate on arrays with units returns a plain array with no units. Where in brian2 does the unit get lost, and what would have to change?" |
| Selector | `np.concatenate units`, chosen and [recorded above](#j6-pilots-the-questions-and-the-selectors-recorded-before-the-runs) before the run. |
| Model and provider | `none` |
| Code and environment basis | One repository at tree `375bd477…`, `workspace: dirty`, snapshot digest `sha256:1bfec9ed…`. The working tree holds 130 untracked files and **no modified tracked file**, so by the owner's decision the pilot searches the tree as committed and declares what that leaves out. |
| Path taken | register → submit → index built at the basis → discovery: ranked retrieval over the view from the task, anchors for the one identifier-shaped phrase, claims (none: brian2 has no decision records) → publish → read. |
| Packet identity | One revision. **10 sections, 0 omissions**, 10,837 bytes of section content against 65,536 of capacity. Nine source spans and one anchor section; no claim sections, because this repository has none. |
| Coverage, as the packet states it | frontier `375bd477…`; **22 blobs were not text**; **220 blobs are in a language with no anchors** — of those 67 `.rst`, 25 `.cpp`, 25 no extension, 23 `.pyx`, 19 `.py_`, 11 `.txt`, 7 `.md`, 6 `.h`; and **130 files are untracked and in no tree, so they are not searched** — of those 44 `.lock`, 22 `.json`, 17 `.py`, 14 `.o`, 11 `.md`, 7 `.cpp`. |
| Baseline | `git grep -w` at the same tree for the selector's terms, ranked by how many distinct terms a file holds, top ten. The reviewer scores it against the same oracle. This is the simple lexical baseline INTERNALS §7 names; the strong native-context baseline is M7's. |
| Durable result | The packet and its cited artifacts survive in the store and re-read by identity. **No byte, excerpt or packet of brian2 is committed to this repository**: it is CeCILL-licensed and this repository is MIT, so the record carries digests, paths, spans, counts and costs only. |
| Reproduction | The exact commands are in the pull request. They register the checkout by path, so they reproduce only on a machine that has it. |
| Cost | Model calls 0, tokens 0, spend 0. **553 blobs, 5,345,464 bytes indexed. Index build and compile 12.6s. Time to first packet 14.8s.** Store after the run **19.6 MB**. The index build holds the preparation tick throughout, so every other job on that provider waits those 12.6 seconds — the known limit recorded in [VERIFICATION](../VERIFICATION.md), to be resolved in M4's bounded runtime. |
| Simulated or untested | Nothing is simulated. **Untested here:** the dirty-bytes path, which this checkout cannot exercise because it has no modified tracked file; decision memory, which brian2 has none of; and any second repository in one basis. |
| A compiler change after the run, declared | The first run produced **9 sections and no anchor section at all**: `concatenate` has five call sites in this tree and no definition, because it is numpy's, and the compiler emitted an anchor section only when a definition existed. A question about someone else's function therefore got nothing from the half of discovery meant to answer it. That is a general defect, not a property of this question — it holds for every third-party symbol in every repository — and it was fixed and **the run repeated in full**, as the review's protocol requires. The second run is the one recorded here; the only difference is section 10. |
| **Second run, at m3e** | Same question verbatim, same selector, same repository at the same tree, against a compiler carrying [M3e's four changes](#what-the-pilots-changed-and-what-changed-back). **10 sections, 0 omissions, 15,568 bytes of section content** (10,837 in the first run); index build and compile **12.8s**, time to first packet **13.0s**, store **19.2 MB**; producer `cbr-context-compiler/2`. Coverage identical. Five of the nine source spans are wider because a ranked span now reaches its neighbours — `docs_sphinx/developer/units.rst` lines 161–197 where it was 181–197, `brian2/tests/test_units.py` 1141–1180 where it was 1141–1160, `units.rst` 61–100 where it was 81–100, `changes.rst` 1–40 where it was 21–40, `timedarray.py` 241–280 where it was 241–260 — and the span that was the second half of one of those is now a different region of the same file, `test_units.py` 601–660. The anchor section is unchanged. **Rescored: FAILED, unchanged from the first run** — one of three required facts, no trap triggered, and the `git grep` baseline still holding none of the three. Which facts are missing is still not recorded: this question is sealed and runs again at M4. |
| Properties and limits | **This establishes nothing about usefulness by itself.** The packet is scored by someone who has the oracle and did not produce the packet. What the run does establish is mechanical and worth having: a 553-file, 7,065-commit repository was indexed cold and answered in under fifteen seconds with no model; every span cites an artifact and a byte range; the coverage names every gap by count and kind; and the working tree's 130 untracked files are declared rather than silently skipped. **Lexical discovery finds only what shares vocabulary with the question** — nine of ten sections came from term overlap alone, and the single anchor section is the only part that used structure. |

### What the pilots changed, and what changed back

Both pilots were run at m3d, scored, and **both failed their oracles** — brian2 holding one of three required facts, Knowscroll two of three, neither triggering its trap, and the plain `git grep` baseline holding none of either set. Neither failure was answered by tuning the compiler against the question that produced it. After the four changes below both were rerun and rescored: **brian2 failed again, identically**, and **Knowscroll passed, three of three**. The asymmetry is the useful part — the same change moved one and not the other, which is what a general fix looks like and what a tuned one would not.

What the pilots *did* produce is four defects that are not about either question, each fixed at m3e with its own test and its own mutant, each on a hermetic repository the test writes:

| What the pilots found | What the rule is now |
|---|---|
| A tree with modified tracked files was searched at its committed bytes and **no gap said so**. Knowscroll had 21; brian2 had none, which is why nothing showed. | The coverage names untracked, **modified** and deleted files separately, each counted and broken down by kind. Searching the committed tree stays the owner's decision; not saying so was the defect. |
| A **symbolic link** supplied an item's content: `AGENTS.md` is mode `120000`, and the packet cited its nine bytes — the target's *name* — as the file. | A link is resolved inside the same tree and the item is satisfied **from the target, cited at the target's path**, with the locator naming the link. An absolute target, one that climbs out of the tree, a chain over eight hops, or anything that is not a regular file of the tree leaves the item **unmet**, and the coverage says where the link pointed, because CONTEXT bounds an item's reason at 64 characters. |
| **Eligibility was doing the work of selection.** In a one-repository store "a condition names a repository of the basis" is true of everything, so 22 of 22 decisions went into a packet about one of them. | Eligibility is unchanged and decided from the claim alone. Among eligible claims the compiler **ranks** — the question's terms against the statement, the scope's qualifiers, and the cited evidence **at the span the claim names** — carries `CARRIED_CLAIMS` of them, and omits the rest with reason `applicability`, counted. |
| In both pilots something the question wanted lay a few lines past a cited span's edge, on the far side of a fixed twenty-line boundary. | A **ranked** span reaches the chunk either side of it while the excerpt stays inside `EXCERPT_BYTES` and stays one contiguous byte range; on a tie the following chunk wins. A first chunk, which ranked nothing, is not extended. |

**A fifth defect the reruns found, in one of the fixes.** Widening made two adjacent hits in one file land on the same bytes, and the overlap check ran *before* widening and only against what the items had taken — so brian2's second run came back with one section published twice. Overlap is now decided after widening and against every span the packet already holds, discovered ones included. It was found by running the pilot, not by the suite, which is the argument for running them.

**And one that is reported rather than fixed.** An ingested artifact's id carries the instant it was ingested, and a claim's revision digest is taken over a record holding that id, so **two stores that ingest identical bytes and propose identical claims agree on neither**. A packet citing an ingested artifact is byte-reproducible within its store and not across stores. The golden packet guard found this by failing on every run; it normalises both out, narrowly and visibly, and the property is recorded here.

### The m4e live run, 2026-09-22: both pilots and J1, with a model

**The first live run of the model runtime**, authorised once by the owner, driven by [`scripts/m4e_run.py`](../../scripts/m4e_run.py) at `a6dc450`. Six runs: J1 revisited and both sealed pilot questions, each against `MiniMax-M2.7-highspeed` and `MiniMax-M3`, so the difference between a pair is the model and nothing else. **65,144 tokens in total** — 2.98% of the 2,239,128 six-flow worst case — every item satisfied, no question ambiguous, and the credential absent from every store.

| Run | Repository at | Tokens | Records | Terms step | Choice step |
|---|---|---:|---:|---|---|
| `j1-m27hs` | `a6dc450` | 3,278 | 1 | `model_answer_truncated` | not reached |
| `j1-m3` | `a6dc450` | 5,607 | 2 | answered | answered |
| `brian2-m27hs` | `4960df7` | 14,208 | 2 | answered, one repair | answered, one repair |
| `brian2-m3` | `4960df7` | 6,233 | 2 | answered | answered |
| `knowscroll-m27hs` | `3e8991e` | 23,501 | 2 | answered | `model_answer_truncated` |
| `knowscroll-m3` | `3e8991e` | 12,317 | 2 | answered | answered |

Packet digests, in the same order: `79762ecf…`, `116922fa…`, `32f52cd2…`, `abd71084…`, `74358b75…`, `a1370d9c…`. The packets themselves are outside this repository, where the reviewer scored them; **no repository text is recorded here**, which is the licence rule for the pilots.

**The model split is the finding.** `MiniMax-M3` used 13 to 32 output tokens per step and answered cleanly in all six of its steps. `MiniMax-M2.7-highspeed` spent 277 to 512 tokens reasoning before writing anything, and truncated two of its three flows — J1's terms step at the 512-token floor, and Knowscroll's choice again at the 1,024 its repair asked for. One of its flows also spent a repair on a fenced ```` ```json ```` answer. Both are fixed at m4f, and both were invisible to a fake transport.

**Knowscroll ran with zero claims in the store.** The harness registers a repository and asks a question; it ingests no decisions, so nothing in that run exercised decision memory at all. Its result is **lexical discovery only**, and is not evidence about the decision-memory path that the m3d/m3e pilot was about.

#### Run 2, 2026-09-22: aborted at its first run, and what the fragment still showed

**It stopped, and the reason was a defect in the harness introduced by the corrections to the harness.** Authorised once, over the same six questions re-pinned to `7edc199`, it refused at the first run's *replay* stage:

```
m4e_run: j1-m27hs: submit failed: cbr: core.authenticate: authentication_failed {}
```

The live half had completed — packet written, two discovery records sealed, **8,473 tokens** charged — and the rebuild could not authenticate. m4f made the live rebuild launch under the *production* configuration so that citations would name the same provider on both sides; the credential it presented was still chosen from whether the run was live, by a call site written when the rebuild was always a conformance launch. A production configuration names no `credentials` member, so the rebuild offered one the provider had never heard of.

**A configuration and the credential that authenticates against it are one decision**, and they were two. They are one now: `credential_file` reads the credential off the configuration being launched, so a caller holding the configuration cannot present the wrong credential for it.

**No dry run could have caught it, and that is structural rather than an oversight.** In dry mode both launches are conformance and both carry the dry-run credential, so the production side of this rule has no test and cannot have one short of a live run. It is asserted where it can be — over the harness's own decision about each launch, without launching anything — and the gap is named here rather than left to be inferred from a suite that passes.

**What the fragment established, and it is only a fragment.** Both m4f fixes worked on the one flow that ran, which is worth recording because the flow in question is the one run 1 lost:

- **The reasoning budget holds.** `j1-m27hs` completed *both* discovery steps, at 511 and 676 output tokens, with no repair. In run 1 the same flow truncated at the 512-token floor and its choice step was never reached.
- **The ADR was offered, and chosen.** The span run 1 never put in front of the model was candidate `d7` and the model took it — which is the per-path cap doing exactly what it was added for.

Neither is a score and neither is a measurement: it is one flow of six, from a run that did not finish. It is recorded as what a partial run established, labelled partial, because the alternative is to say nothing about the only evidence the run produced.

#### The scores, which are the reviewer's and are in the reviewer's words

> brian2-m3 1/3 (fundamentalunits.py 301-340, first time the file is cited); brian2-m27hs 2/3 (fundamentalunits.py 1661-1700 + unitsafefunctions.py 61-120); knowscroll-m3 3/3 with zero claims (DECISIONS.md 621-660 + _spike.mjs 1-40); knowscroll-m27hs 1/3 (choice truncated); j1-m27hs both facts via the deterministic fallback; j1-m3 code only (ADR never offered — item 4). No trap triggered anywhere.

This session has never seen either oracle and scored nothing.

**What the run establishes, narrowly.** brian2's question had failed the deterministic compiler twice, holding one of three facts both times, and `fundamentalunits.py` had never been cited at all; both live runs cite it, and one holds two of three. That is the first evidence that a model proposing search terms reaches something lexical discovery did not — which is the claim m4e was built to test. It is two runs of one question, not a measurement.

**And what it does not.** `j1-m3` returned code only, because the ADR its answer needed **was never offered to the model**: the candidate set was the raw top of the ranking rather than the per-path-capped reading the packet publishes, so thirteen spans of one file filled it. A fact never offered cannot be kept, and a score under that condition measures the candidate set rather than the model. Fixed at m4f; the pairs above were run before the fix and should be read with it in mind.

### Live run 3, 2026-09-23: both pilots and J1 again, after m4f and m4g

**The third live run, and the first after the corrections runs 1 and 2 forced.** Authorised by the owner, instructed by the reviewer once the manifest was re-pinned to `9ee22d0` and all four origins were re-checked public, and driven by [`scripts/m4e_run.py`](../../scripts/m4e_run.py) from `main` at `9ee22d0` with `--live` and `--permit-model-network` and no other flag. The same six questions as run 1, at the same pilot trees, each against both models. **Exit 0, nothing on standard error, 200 seconds, 67,395 tokens** — 3.01% of the 2,239,128 six-flow worst case. Every item was satisfied, every flow sealed both of its discovery records, and every replay reproduced its packet's sections with no difference and no ambiguous question. Each launch was given the cap less what the launches before it spent, from 5,000,000 down to 4,945,027.

| Run | Repository at | Tokens | Records | Terms step | Choice step |
|---|---|---:|---:|---|---|
| `j1-m27hs` | `9ee22d0` | 8,350 | 2 | answered, 448 output tokens; fenced, and unwrapped without a repair | answered, 637 output |
| `j1-m3` | `9ee22d0` | 6,810 | 2 | answered, 25 output | answered, 21 output |
| `brian2-m27hs` | `4960df7` | 12,084 | 2 | answered, 410 output | **answered in prose** (5,222 tokens, 968 output), then **one repair** (4,827 tokens, 538 output) |
| `brian2-m3` | `4960df7` | 6,018 | 2 | answered, 21 output | answered, 21 output |
| `knowscroll-m27hs` | `3e8991e` | 21,711 | 2 | answered, 518 output | **answered in prose** (9,218 tokens, 843 output), then **one repair** (8,922 tokens, 512 output) |
| `knowscroll-m3` | `3e8991e` | 12,422 | 2 | answered, 24 output | answered, 25 output |

Packet digests, in the same order: `e1cb9aac…`, `6cf6a2ff…`, `36e2617a…`, `57204953…`, `9da7dae3…`, `0129ba0a…`. The packets and the stores are outside this repository, where the reviewer scored them; **no repository text is recorded here**.

**Where the figures come from.** Tokens, records and charges are the harness's report. Output tokens and repairs are each step's sealed derivation record, `usage.tokens` less `usage.input_tokens`. A repaired step's record carries the usage of its last exchange only (below), so the prose attempt of each repaired step is taken from elsewhere: its total tokens from the ledger, and its output tokens, **968 and 843, from the reviewer's reading of the recorded exchanges in `model_calls`**, which this session did not read. Every call was admitted by the local bound alone, and no count call was made.

**Checked independently by the reviewer, from the stores:** no key-shaped string in any of the 82 files; no credential file written for any launch, so every rebuild authenticated with the credential its provider issued — m4g working live, where run 2 had failed; every ledger equal to the report and every charge within its local estimate; and all fourteen calls completed, none truncated.

#### Run 3's scores, which are the reviewer's and in the reviewer's words

> Run 3: j1-m27hs pass; j1-m3 pass (the ADR is now offered and kept); brian2-m27hs 2/3; brian2-m3 2/3 (run 1: 1/3); knowscroll-m27hs 3/3 (run 1: 1/3); knowscroll-m3 3/3. No trap triggered anywhere. In both brian2 flows the missing fact was never in the offered set, so that score measures discovery's ranking and not the model's choice. Knowscroll again ran with zero claims in the store. M3 matched M2.7-highspeed on every score for 25,250 tokens against 42,145, with no repairs.

This session has never seen either oracle and scored nothing.

**What changed since run 1, stated from the record rather than inferred.** The ADR run 1 never offered was offered and kept, which is what m4f's per-path cap was added for. No step truncated: `MiniMax-M2.7-highspeed` used 410 to 968 output tokens a call against the 2,048 floor, where run 1 truncated two of its flows at 512. The fenced answer cost no repair. The rebuilds authenticated. `knowscroll-m27hs`, whose choice step truncated in run 1, completed it.

**The model split, again.** `MiniMax-M3` used 21 to 25 output tokens a call and its records state 0.6 to 2.1 seconds a step; `MiniMax-M2.7-highspeed` used 410 to 968 a call and 9.5 to 27.0 seconds a step, and spent a repair on each pilot.

**A prose answer where a structure was asked for is the ordinary outcome [READINESS §4](../work/m4/READINESS.md#4-provider-facts-that-are-design-inputs-not-discoveries) designed for, and it happened.** On both pilot questions `MiniMax-M2.7-highspeed` answered the choice step in prose; CBR read the answer as unusable, repaired once with its own sentence rather than the model's text, and the repair answered in shape. The two repairs cost 4,827 and 8,922 tokens.

**brian2 is not something to fix in M4 or to tune against.** In both flows the fact the packet lacked was never in the candidate set, so two of three measures discovery's ranking, not the model's choice. It is an input to M5's readiness, stated generally rather than as this question.

**Knowscroll again ran with no claims in the store**, so, as in run 1, its result says nothing about the decision-memory path the m3d and m3e pilot was about.

**J1's excerpt limit, found by the reviewer and recorded rather than fixed.** The ADR section's excerpt covers the start of the question 11 row, but the sentences recording the move to `gix` sit about 600 bytes past the 2,048-byte excerpt cap; the citation resolves to them. It is the limit [J1's M3c record](#j1-a-question-finds-its-own-answer-with-a-cited-packet) already states, measured on a real packet.

**A repaired step's record understates what its question cost.** Found while writing this record: `model::Runtime::ask` builds its `Cost` from the attempt that ended the question, so `brian2-m27hs`'s choice record says 4,827 tokens and one repair while the ledger holds both charges, 5,222 and 4,827. The ledger is the spend and it is right; the record, read on its own, leaves out the attempt it repaired. The reviewer confirmed it, and that a step ending unmet after a repair drops its earlier attempts the same way. Fixed at m4h, with a test that a repaired step's sealed record carries every attempt's usage and equals that question's ledger rows; records sealed since are format `/3`, and the `/2` records of these runs still replay.

**Runs 1 and 3 are the discovery family's sealed, costed live transcripts**, and by the owner's decision of 2026-09-23 they are what [issue #21](https://github.com/Combraton/cbr/issues/21) closes on.

### What neither pilot establishes

Both records above measure a packet. **Neither measures a session.** No agent session used either packet for any task: nothing here shows that work went better with one, or that a constraint was preserved that would otherwise have been lost, or that investigation was not repeated. That is J6 proper, and it is M7's — with pre-agreed thresholds derived from these pilots' variance, fixed before the confirmatory run, against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline.

What the pilots are for is narrower and worth having on its own: they are the first runs of the write path and the read path over repositories this session did not write, at sizes and in shapes the fixtures do not reach — 5.3 MB and 553 files in one, 22 owner decisions over one append-only file in the other — and they produced the variance M7's thresholds will be derived from, every packet scored against its oracle by the reviewer.

### J2 live rerun, 2026-09-26: passed — and the model added nothing

**Passed, by rubric v1.1, frozen before the rerun.** The same six runs as the first live run, on the same inputs and the same two models, after m5a-3 made the parser's failures a floor that a model's choice adds to and cannot remove. Every gate held on all twelve arms, and no run was worse than its baseline. **Every run came out equal to its baseline.** For that case [the rubric pre-declared](j2-live/RUBRIC.md#7-what-j2-live-passes-means-pre-declared) this wording, here with T = 46,452: *"The model-assisted projection matched the deterministic rule on all six inputs at T tokens. On these inputs it could not have improved on it (§8). The run shows that the live mechanism holds under a real model, that the model did no harm, and what a projection costs. It does not show that a model selects better than the rule."* **Its second sentence does not hold**, and §8 of the same rubric says so: on the core manifest, choosing the five `unsupported` records could have raised C3 by up to +20. There a model could have improved on the rule, and both models chose nothing. The other three sentences hold as written.

**The rubric was fixed first.** [Rubric v1.1](j2-live/v1.1/RUBRIC.md) changes v1 mechanically, for the `/2` projection, plus two robustness changes in its own path, neither of which fired here — a section the format cannot read is an INVALID arm where v1's reader stopped, and gate E's new check of the header's counts stops the scorer with an error on a missing answer-key field, where the silent default of 0 in v1.1's first draft would have hidden a renamed one. v1's criteria, weights, bands, verdict rules and pass statement apply unchanged. An agent that did not write it checked it before it was frozen and found one hole — a run that asked nothing was judged from the harness's report alone, so a usage row written into its store passed; v1.1 now reads the store as well — and a second agent rechecked it with seventeen attacks of its own. It was committed in `13f5911` and `587ddf1`, the last at 22:53:01Z, the rerun's ceilings in `8d08bef` at 22:53:12Z, and all three were pushed at 22:57:14Z; invocation 1 started at 22:57:23Z. v1.1 says its digest is in the commit that adds it: the frozen text is `587ddf1`'s, since `13f5911` held a draft naming the scorer before the store checks. Its account of that check says the worst case at 492,204 and at 492,205 was each caught; 492,204 is the bound itself and correctly passes, and only 492,205 is caught. (Times in this record are UTC, and they fall on 2026-09-25 there; the record's date, 2026-09-26, is the session's own, IST, where the rerun started at 04:27.) **Under v1, read literally, every arm fails**, and only on what v1.1 changed: the `/2` format, and, for the runs that asked nothing, a label that names no model, and a replay that did not happen with an investigation number, 4, that v1 expected to equal the parts asked, 0 ([`score-v1-script.json`](j2-live/run-2026-09-26/score-v1-script.json)).

| Field | Record |
|---|---|
| Intent and acceptance | [RUBRIC §7](j2-live/RUBRIC.md#7-what-j2-live-passes-means-pre-declared), unchanged: every gate on every arm; no run worse than its baseline; all four red arms useful and every other arm at least partial; judging complete. |
| Entry point | `scripts/j2_run.py --live --permit-model-network`, as before: `cbr ingest`, `cbr context … --want log=evidence:<artifact>@<digest>`, `cbr request` and `cbr packet` over the provider's socket, then a replay where a part was asked. |
| Prerequisites and inputs | The same three inputs, byte-identical to their pinned digests. A fresh store per run, no repository registered, cold. |
| Implementation basis | `main` at `80cdfbe`, m5a-3's merge (#44). A debug build from a clean tree at 2026-09-25T22:48:07Z: `cbr` `sha256:4f9ee141…1bae` and `cbr-provider` `sha256:0fb354e6…1e66`, the same digests after both invocations. Projection format `cbr-project-large-result/2`, `cbr-context-compiler/4`. Protocol `v0.1.0`; Rust 1.97.1; macOS. |
| Model and provider | MiniMax, Responses dialect, production configuration; `MiniMax-M2.7-highspeed` and `MiniMax-M3`. Every `model_calls` row's response names the configured model, with status `completed`. |
| Path taken | sealed input → baseline request (investigation 0, the rule) → assisted request → where the baseline's header named parts to ask, one bounded call a part and a replay; where it named none, no call at all. |
| Packet identity | Twelve packets in [the rerun's record](j2-live/run-2026-09-26/), two a run, each packet's sealed digest in its own file at `reference.artifact.digest`. |
| Durable result | [`prove_path.py`](j2-live/prove_path.py) on a copy of each store: every packet reads back unchanged after a restart, `cbr fetch` returns each input's exact bytes, and **all 188 carried excerpts** equal those bytes at their stated ranges. |
| Reproduction | As for the first run, with `--rubric v1.1` given to [`v1.1/score_j2.py`](j2-live/v1.1/score_j2.py). |
| Cost | **46,452 tokens**, all of it the core manifest's: 6.3% of the 738,306 the survey's parts allowed, and 163 below the 46,615 to 48,637 READINESS estimated. **6 calls**, none repaired. The red and green logs spent **nothing**. Invocation 1 took 2 s and invocation 2 took 18 s. The rubric's cost verdict is **NO RETURN** on all six runs. |
| Simulated or untested | Nothing is simulated. **Untested:** a model that adds what an input offers — on the core manifest both were offered the five `unsupported` records and chose none; an input whose failures overflow the projection, which the red log does not reach — its floor fits with 34 bytes to spare; another prompt; any model but these two. The judges are one model family (below). |
| Properties and limits | Below. |

**The runs.**

| Run | Tokens | Parts asked | What the model chose | Output tokens a part | Latency a part |
|---|---:|---|---|---|---|
| `red-m27hs` | 0 | 0 | not asked | — | — |
| `red-m3` | 0 | 0 | not asked | — | — |
| `log-m27hs` | 0 | 0 | not asked | — | — |
| `log-m3` | 0 | 0 | not asked | — | — |
| `core-m27hs` | 23,559 | 3 | nothing, of 59, 58 and 18 offered | 378, 569, 166 | 4.0–7.7 s |
| `core-m3` | 22,893 | 3 | nothing, of 59, 58 and 18 offered | 6, 6, 6 | 0.9–2.4 s |

**Why red and green asked nothing, and what that proves.** The red log's floor — all eighteen failures with their assertions, cargo's three errors and the run identity that fits — fills 16,350 of the section's 16,384 bytes, less room than the model's longest label would need. The green log's run identity takes every excerpt a projection holds. So neither was offered a unit, neither made a call, and each assisted section is its baseline's byte for byte. **Rubric v1.1 checks that against the store, not only the report**: no ledger row, no sealed part record, and no charge in the report; and `prove_path.py` found no `model_calls` row in any of the four stores. Where the first run's model lost failures the parser had found, it now cannot reach them.

**What the models did with the core manifest.** Each was offered every unit the floor left room for, the five `unsupported` results among them — the gain the rubric pre-registered (up to +20 on C3). Both answered every part with an empty list, as in the first run, `MiniMax-M2.7-highspeed` after 166 to 569 output tokens, all but a few of them reasoning. That is a literal answer to *which tests failed* — an unsupported fixture is not a failed one — and the instruction allows it: *an empty list is a complete answer*. But it is the answer the rubric scores against: C3, the unsupported fixtures named, weighs 20, and every core judge scored J1 at 3 on both arms and gave the missing unsupported fixtures as the reason. The projection is the rule's, and the header says the model *added nothing*.

**The scores.**

| Run | Baseline S | Assisted S | ΔS_script | Verdict |
|---|---:|---:|---:|---|
| `red-m27hs` | 97.5, useful | 97.5, useful | 0 | equal by construction |
| `red-m3` | 97.5, useful | 97.5, useful | 0 | equal by construction |
| `log-m27hs` | 68.75, partial | 68.75, partial | 0 | equal by construction |
| `log-m3` | 66.25, partial | 66.25, partial | 0 | equal by construction |
| `core-m27hs` | 71.25, partial | 71.25, partial | 0 | equal |
| `core-m3` | 71.25, partial | 71.25, partial | 0 | equal |

**The judges.** Three per pair, blinded, a fresh calibration session first, on the first run's calibration pair and [its key](j2-live/run-2026-09-25/judging/calibration_blind_key.json) — it preferred the rule over the fake model's section by 4 to 2, as in the first run — and six judge sessions, each seeing at most one pair of each kind: eighteen sheets ([`judging/`](j2-live/run-2026-09-26/judging/)). **Every pair was identical after redaction**, so every sheet was an attention check: every judge scored A and B alike, no preference was given, and no score spread above one point. The judges' configuration is the first run's: fresh sessions on Claude Opus 5.5, independent by session and not by model lineage.

**What the rerun establishes.** The mechanism holds under a real model, again, and on these inputs **the model did no harm**: every assisted section carries every byte of its baseline's, at the same ranges, where the first run's lost failures the parser had found. That this holds *whatever* a model answers is m5a-3's property, shown against the fake and `simulated` ([the J2 row](#3-journey-matrix)); the rerun is consistent with it and could not test it, since four runs asked nothing and every answer in the other two was empty. Nor is it a bound on the score: an addition can still cost a criterion — on the core manifest a pass record costs C4 — so a model that added only pass records could score below the rule. The rerun also measured what asking a model cost on these inputs where it added nothing: 22,893 to 23,559 tokens a run on the core manifest, for no change, on an input where the rubric scored the omission it left in place.

**What it does not establish.** That a model selects better than the rule, on any input: on the only input where it could have, it chose nothing. So J2's `model-assisted` half is, on this evidence, a cost with no return, and whether a model's choice is worth asking for in `project_large_result` is an open question, not a result.

**What was edited after the run, and what was checked afterwards.**

- Each `report.json` is committed with its runs' `data` member removed, because it names where the store was written, and with a trailing newline, as in the first run. So the `report_digest` in each `proof.json`, `3875eda6…` and `4a5ebfd6…`, is the digest of the report as the harness wrote it, not of the committed file. The packets are byte-identical to what the harness wrote, and the proofs to what `prove_path.py` wrote.
- `prove_path.py` ran from its committed location on `main` at `80cdfbe`, unchanged since #41, and relaunched a provider on a copy of each store in replay mode with no network permit. Its two proofs ended at 22:57:56Z and 22:57:58Z, within fifteen seconds of invocation 2's end, which is how the first run's record placed its own in-window proofs under [VERIFICATION's rule 4](../VERIFICATION.md#four-rules-that-keep-the-owners-key-out-of-every-run): as part of the authorised run. As then, READINESS §10 did not plan them; this session took the owner's brief, which asks for the path to be proven through the public client, as authorising them, though it names no such launch. Nothing reproduced them outside the run's window.
- The inputs' one origin is this repository: its own suite's output at a pinned commit. `j2_run.py`'s live mode asks the GitHub API before it launches anything and refuses unless the answer is `private: false`, so both invocations checked it before their first call; it gave the same answer when read again while this was recorded.
- **One path is in the packets.** As in the first run, the red log carries `/private/var/tmp/cbr-j2`, where the inputs were produced, and it names no user, machine or volume. Here all four red packets carry it on five lines each, the assisted as well as the baseline, since each assisted section is its baseline's; the eight packets of invocation 2 carry none. The excerpts are base64, so `git grep` does not see it.

### J2 live, 2026-09-25: the mechanism held, and the model made the projection worse — failed

**Failed, by criteria fixed before the run.** Six runs on CBR's own test output, each input on MiniMax-M2.7-highspeed and MiniMax-M3, as the owner decided them ([READINESS §10](../work/m5/READINESS.md#j2-live-the-inputs-the-estimate-the-stop-and-the-cap-decided-2026-09-25)). Every mechanical gate held on all twelve arms, the six model-assisted and the six deterministic baselines. What failed is what the model was for. **On the failing test log, both models' projections were worse than the deterministic rule's**, because a model's choice replaces the rule's and both models left out failure blocks the parser had already found. **On the green log and the conformance manifest, both models chose nothing**, although each was offered the units by which, the rubric had pre-registered, a model could beat the rule there.

**The rubric was fixed before the first call.** [RUBRIC](j2-live/RUBRIC.md), its answer key and its three scripts (`answer_key.py`, `score_j2.py`, `blind.py`) were drafted by three independent agents with different lenses, merged by a fourth, committed as `84061eb` at 15:39:31Z and pushed at 15:39:34Z; invocation 1 started at 15:40:00Z, and its first model call is recorded at 15:40:01Z. The answer key is computed from the inputs' bytes, never by CBR's parser. Three limits of that, stated rather than left to be found: **its cost thresholds are this session's, not the owner's**; the baselines were visible to its author, because they are deterministic ([RUBRIC §10](j2-live/RUBRIC.md#10-known-limits-of-this-rubric)); and 26 seconds lay between the push and the run, so nobody else reviewed the rubric before it was used.

| Field | Record |
|---|---|
| Intent and acceptance | [RUBRIC §7](j2-live/RUBRIC.md#7-what-j2-live-passes-means-pre-declared), fixed before the run: every gate on every arm; no run worse than its baseline; all four red arms useful and every other arm at least partial; judging complete. |
| Entry point | `scripts/j2_run.py --live --permit-model-network`, which drives `cbr ingest`, `cbr context … --want log=evidence:<artifact>@<digest>`, `cbr request` and `cbr packet` over the provider's socket, then relaunches the store with `--replay-model` and rebuilds. No internal shortcut. |
| Prerequisites and inputs | The three inputs of READINESS §10 that ask a model, byte-identical to their pinned digests: `cargo-test-f73415d.log` (65,257 B, 18 tests failing), `cargo-test.log` (66,535 B, green) and `conformance-core.manifest.json` (61,621 B, 130 pass and 5 unsupported). A fresh store per run, no repository registered, cold. |
| Implementation basis | `main` at `041ad5f`, #40's merge, whose code is `5ab1a4f`'s. A debug build from a clean tree: `cbr` `sha256:e2ce3c8a…b6fc` and `cbr-provider` `sha256:810b5725…3d35`, hashed when they were built, before invocation 1 (by a dry-run proof kept in the session's scratch, outside this repository), again between the invocations, and again after both by `prove_path.py`. Protocol `v0.1.0`; Rust 1.97.1; macOS. |
| Model and provider | MiniMax, Responses dialect, production configuration; `MiniMax-M2.7-highspeed` and `MiniMax-M3`. Every `model_calls` row's response names the configured model, with status `completed`. |
| Path taken | sealed input → baseline request (investigation 0, the rule, no call) → assisted request (investigation = parts; one bounded call per part, each its own sealed record) → both packets checked against the input's bytes → the store relaunched with `--replay-model` and the assisted section rebuilt from its records. |
| Packet identity | Twelve packets in [the run's record](j2-live/run-2026-09-25/), two a run; each packet's sealed digest is in its own file, at `reference.artifact.digest`. The red log's are `7071cac0…` and `7ff3ab28…` (`red-m27hs`, baseline and assisted) and `e0ac34a9…` and `48633894…` (`red-m3`). Every section cites the ingested artifact as `c-log`. |
| Durable result | Checked by [`prove_path.py`](j2-live/prove_path.py) on a copy of each store: the packet reads back unchanged after a restart, `cbr fetch` of the cited artifact returns the input's exact bytes, and **all 195 carried excerpts** equal those bytes at their stated ranges. |
| Reproduction | Below. |
| Cost | **120,067 tokens**, 5.3% of the 2,250,000 held and 4.1% of the 2,928,960 worst case. 16 calls, 0 repairs, every call admitted by the local bound alone. Invocation 1 took 26 s and invocation 2 took 49 s of wall clock. **The rubric's cost verdict is NO RETURN on all six runs**: −8.0 and −16.2 script points per 10,000 tokens on the red log, and 0 on the others. |
| Simulated or untested | Nothing is simulated. **Untested:** an input whose failures overflow the projection; another prompt; any model but these two. The judges are one model family, calibrated on one pair (below). |
| Properties and limits | Below. |

**The runs.** Tokens are the ledger's, which equal the report's and the sealed records' own usage. Output tokens and latency are each part's sealed record.

| Run | Tokens | Parts: chosen of offered | Output tokens a part | Latency a part |
|---|---:|---|---|---|
| `red-m27hs` | 18,714 | 30 of 64, **0 of 23**, 1 of 10 | 649, 524, 386 | 9.5–10.3 s |
| `red-m3` | 17,696 | 23 of 64, **0 of 23**, 1 of 10 | 97, 6, 9 | 1.6–2.0 s |
| `log-m27hs` | 19,280 | 0 of 43, 0 of 22 | 759, 1,057 | 14.0–23.0 s |
| `log-m3` | 17,762 | 0 of 43, 0 of 22 | 6, 6 | 2.2 s |
| `core-m27hs` | 23,931 | 0 of 60, 0 of 58, 0 of 18 | 728, 683, 283 | 5.0–12.7 s |
| `core-m3` | 22,684 | 0 of 60, 0 of 58, 0 of 18 | 6, 6, 6 | 0.9–1.2 s |

**The gates, all held on all twelve arms** ([RUBRIC §3](j2-live/RUBRIC.md#3-gates-unweighted-every-one-must-hold)): every ledger tiles its input; every excerpt and every named failure is the input's bytes at its stated range, re-checked by the scorer's own reader; every packet's omissions are the ledger's; every bound holds; each header's digest, commit and failure count match the answer key; each arm says who chose; every replay rebuilt identical sections with no ambiguous question; one sealed record per part, naming the run's model. **No model-provider key and nothing from the Keychain** appears in any of the 70 files the six runs' work directories hold, counted by shape; each store holds its own CBR owner credential, as every store does.

**The scores.** S is out of 100: script criteria computed from byte ranges against the answer key, and judged criteria from three blinded judges per pair (below).

| Run | Baseline S | Assisted S | ΔS_script | ΔJ1 | Verdict |
|---|---:|---:|---:|---:|---|
| `red-m27hs` | 97.5, useful | 70.0, partial | −15.0 | −2 | **worse**: 6 tests shown at a lower level than the baseline |
| `red-m3` | 97.5, useful | 56.25, partial | −28.75 | −2 | **worse**: 15 tests lower |
| `log-m27hs` | 66.25, partial | 66.25, partial | 0 | 0 | equal: the model chose nothing |
| `log-m3` | 66.25, partial | 66.25, partial | 0 | 0 | equal: the model chose nothing |
| `core-m27hs` | 71.25, partial | 71.25, partial | 0 | 0 | equal: the model chose nothing |
| `core-m3` | 73.75, partial | 73.75, partial | 0 | 0 | equal: the model chose nothing |

**What the models chose on the red log**, from their sealed records. The rule carries all eighteen failures, each with its panic line, message and location, beside the run's identity lines, in 11,872 bytes.

- `MiniMax-M2.7-highspeed` chose all eighteen `… FAILED` lines, all twelve of `journey_two`'s failure blocks, and cargo's closing `error: 2 targets failed`. It chose **none of `j2_harness`'s six failure blocks** and nothing at all in the second part.
- `MiniMax-M3` chose the eighteen `… FAILED` lines, both `failures:` lists, three of `journey_two`'s twelve blocks, and the same closing line. It too chose none of `j2_harness`'s blocks and nothing in the second part.
- **Neither model chose a passing test.** Every unit either chose is one the answer key counts as relevant. The room the unchosen blocks left was taken by run identity of passing test binaries, which CBR's own fill order carries after the chosen failures.
- **Whatever a model does not choose is declared `not_selected`**, including a block the parser had already named as a failure. So `j2_harness`'s six tests reach a reader by name only in both model arms, and under `MiniMax-M3` nine of `journey_two`'s twelve assertions do too. Neither model chose cargo's two `error: test failed, to rerun…` lines, so each model arm also loses two lines of run identity, and its run-level failure evidence falls from 4 to 3.

One judge put it in these words, about the `MiniMax-M3` pair: the unchosen assertion bodies are *"declared 'not_selected', which labels the very content the task asks for as not chosen for it."*

**What the models passed up on the other two inputs.** The rubric pre-registered where a model could beat the rule ([RUBRIC §8](j2-live/RUBRIC.md#8-pre-registered-results-dry-run-fake-model-answering-u1-the-baseline-is-deterministic)): by choosing the manifest's five unsupported records (up to +20) and the green log's ignored test (+5). **Both were offered, in every run, and neither model chose them.** Every answer on those two inputs was an empty list, so the assisted sections are identical to the baselines but for the header line that says who chose. That is a literal answer to the question each part asked, *which tests failed*: an unsupported fixture and an ignored test are not failures, and `MiniMax-M2.7-highspeed`'s own reasoning, recorded with its answer, says of an unsupported record that it is *"NOT a failure"*. It spent 283 to 1,057 output tokens reasoning before each empty answer. So the gain the rubric pre-registered was one a model could reach only by reading the task more widely than it was put.

**The judges.** Three per pair, blinded to the arm and the model, each seeing at most one pair of each input kind; six sessions, eighteen sheets ([`judging/`](j2-live/run-2026-09-25/judging/)). **Their configuration:** fresh Claude Code workflow agent sessions on Claude Opus 5.5, the same model as the session that wrote the rubric, ran the live calls and orchestrated the judging, though none of the judge sessions did any of those; so their independence is by session and not by model lineage, and none is MiniMax. Each read only its own three pairs' folders, by instruction, which nothing enforced. A calibration session first judged the dry run's red pair, the fake model's section against the rule's, and passed: it preferred the rule and scored its answerability 4 against 2. **No spread above one point on any score**, no attention flag on the four pairs that were identical after redaction, and all six preferences on the red pairs were for the rule. (The calibration's blind key names its pairs `red-m3` and `red-m27hs`, after the dry run it was packed from; they are the fake model's sections, not the live ones.)

**What the run establishes.** The live mechanism holds under a real model: sealed whole, bounded, every omission declared, every excerpt the input's bytes, the model's cost in the ledger and in each sealed record, the answer replayable without the network, and no provider key in any store. It also measured what a projection costs: 17,696 to 23,931 tokens for an input of 61 to 67 KB, about 1.1 to 1.6 times an estimate of the tokens in the whole input (its bytes over four), before a reader reads anything.

**What it does not establish.** That a model selects better than the rule: where it could have lost it lost, and where it could have won it chose nothing. Anything about another model, another prompt, or an input whose failures overflow the projection. One sample per model per input.

**Found by the run, in both arms** ([RUBRIC §9](j2-live/RUBRIC.md#9-findings-common-to-both-arms-reported-not-scored)):

- `failures named: 21` counts cargo's three `error:` lines with the eighteen tests.
- `and 5 more` hides both of the tests whose assertions carry left and right values.
- Cargo prints the next `Running` line directly after `error: test failed, to rerun…`, and the diagnostic unit swallows it, so in a model arm a line of run identity is declared `not_selected`.
- The green projection carries 25 of its 40 result lines and not the log's end, because run identity fills the 24 excerpts; its judges scored answerability 3 of 4 for that reason.
- The conformance projection names none of the five unsupported fixtures.

**How to reproduce it.** The inputs, the manifests and the stores are outside this repository; the stores are what `score_j2.py` and `prove_path.py` read, at `<out>/work/<run id>/data`.

```sh
cargo build --workspace --locked   # at this record's head, whose code is 041ad5f's; target/debug is what the harness and the proof default to
python3 scripts/j2_run.py --manifest <manifest> --out <short dir> --live --permit-model-network --run-ceiling <N>
cd docs/verification/j2-live
python3 blind.py pack --out <out 1> --out <out 2> --inputs <inputs> --dest <judging dir>
python3 blind.py unblind --dest <judging dir> --sheets <sheets dir> --judged judged.json
python3 score_j2.py --out <out 1> --out <out 2> --key answer_key.json --inputs <inputs> --judged judged.json --json score.json
python3 prove_path.py --out <out> --inputs <inputs>
```

`unblind` needs the blind key at `<judging dir>/sealed/blind_key.json`. **`prove_path.py` launches a provider on a copy of each store**, so on the owner's machine it is, outside an authorised run, the departure from VERIFICATION's rule 4 recorded below, until it becomes a reviewed mode of the harness. An independent re-run of `score_j2.py` over the stores with the committed `judged.json`, and of `unblind` over the committed sheets, reproduced `score-final.json` and `judged.json` byte for byte.

**What was edited after the run, and what was not.**

- Each `report.json` is committed with its runs' `data` member removed, because it names where the store was written, and with a trailing newline. So the `report_digest` in each `proof.json`, `0eaafdaa…` and `31257014…`, is the digest of the report as the harness wrote it, not of the committed file. The packets and the proofs are byte-identical to the harness's.
- `prove_path.py` is committed with two lines changed from the copy that ran: the repository root is found from the script's own location, and `--inputs` has no default, so the script names no path on this machine. Its logic is unchanged, and it reproduced invocation 1's proof exactly from its committed location, on a further copy of the stores, eight minutes after invocation 2 ended: two more replay-only launches, one per store, outside the run's window.
- `prove_path.py` relaunches a provider, and [VERIFICATION's rule 4](../VERIFICATION.md#four-rules-that-keep-the-owners-key-out-of-every-run) permits launches only through the suite, an authorised run, or `scripts/debug_launch.sh`. Its launch is exactly the harness's own replay launch — the production configuration with `--replay-model` and no network permit, which serves from records and reads no credential — on a copy of each store, as part of this authorised run. READINESS §10 did not plan the proofs; the owner's brief for this session asks for the path to be proven through the public client, which needs a provider on each store, and this session took that as authorising them; the brief names no such launch. Invocation 1's proof ran while invocation 2 was live, and invocation 2's proof ended at 15:41:53Z, twenty seconds after the invocation ended: **that is the end of the run's window here. The two launches that reproduced the proof afterwards were outside it, so none of rule 4's three paths covered them**: the same replay-only launch, reading no credential and opening no network, on copies, and recorded here as a departure from the rule rather than folded into the run. That is the reason `prove_path.py` should become a reviewed mode of the harness.
- The path `/private/var/tmp/cbr-j2`, where the inputs were produced, is in two of the run's three inputs: on seven lines of the red log and five of the green one. Each red baseline packet carries five of them, inside `j2_harness`'s failure blocks, and no other packet carries any. It names no user, machine or volume, which is why the inputs were produced there.

**What happens next.** A model must not be able to undo what a parser found. m5a-3 makes the parser's failures a floor that a model's choice adds to and cannot remove, with labels that say exactly who chose what, and it is built test first. J2 live then runs again, under this rubric with only mechanical changes, frozen before the rerun and reported beside this one, never replacing it.

**Its packets' ids, read again at `cbr-context-compiler/4`.** The six runs were compiled under `/3`, whose discovered spans named repository paths in their ids, outside the protocol's identifier grammar — a defect found afterwards, while verifying `cbr expand`, and fixed at `/4` ([STATE](../work/STATE.md)). **These records are not affected**: every id in the twelve committed packet files is an identifier — 29 distinct ones, `s-log`, `c-log`, `s-log.o<n>` and the packet artifacts — because their coverage is empty and nothing was discovered, and `/4` spells every one of them the same. A rerun under `/4` compares like with like, and the rubric's `s-log.o<n>` does not move.

### J2: a large result, projected, with what was left out declared — simulated

**This is not J2's acceptance.** The model is the `model.fake` control, labelled so in every configuration it runs under, and J2's live run on CBR's own test output ([m5 READINESS §10](../work/m5/READINESS.md#10-live-calls-each-planned-estimated-and-capped-before-it)) is recorded above: at `/1` [it failed](#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed), and at `/2` [its rerun passed](#j2-live-rerun-2026-09-26-passed--and-the-model-added-nothing), with the model adding nothing. What a fake can establish is the mechanism: what is sealed, what is sent, what is refused, and what the packet declares.

| Field | Record |
|---|---|
| Intent and acceptance | A large test log, build log or JSON document is carried into a packet as a projection that names its failures, carries decisive excerpts as its own bytes, identifies its run, cites the whole artifact, and declares every byte it did not carry. |
| Entry point | `cbr ingest` seals the input whole; `cbr context … --want log=evidence:<artifact>@<digest>` asks for it; `cbr packet` and `cbr fetch` read the result. No internal shortcut. |
| Prerequisites and inputs | Synthetic inputs written by the tests: cargo's test output of 8 binaries and 3 failures, the same log with `\r\n` line endings, a log of forty failures between passing tests, a conformance manifest of 160 results with 2 failing, a core-shaped manifest with one failing and one `unsupported` result, a document whose run identity is most of a projection, 20,000 bytes that are not text, a 1.2 MB log, and plain text of 122,400 and 124,200 bytes, the first filling exactly four parts and the second needing a fifth. Three sealed artifacts are made unusual on purpose: one purged while another artifact keeps its bytes, one whose stored object is altered on disk, and one sealed over raw frames under a capture anchor holding newlines. The basis names a repository the provider never registered, so nothing else in the request can ask a model. |
| Implementation basis | m5a at `cbr-context-compiler/3`, then m5a-3 on `cbr-context-compiler/4`, the packet ids change's (#43), which m5a-3 leaves as it is; projection format **`cbr-project-large-result/2`**; protocol `v0.1.0`. |
| Model and provider | `simulated (fake)`: `model.fake`, answering `ids:u1` to every part, `ids:u999` for the refused part, `ids:u2` for the tests of what a model adds, and `ids:` for a model that adds nothing. |
| Path taken | sealed artifact → the deterministic baseline (no investigation), which says how many questions the input needs → the same request with exactly that many questions, one sealed record per part, each asking what to add to the rule's projection → a packet whose section tiles the artifact → a rebuild from the records. A baseline that needs no question is asked again with every part a projection may ask, and spends nothing. |
| Properties | **Silent truncation**: every projection read in `journey_two.rs` is checked, by a reader written from the declared format, to tile the sealed artifact — excerpts equal to its bytes at their ranges, omissions declared in the packet's own list — so removing the path that declares an omission fails the journey; mutants C1a and C1b are that removal. **Oversize input**, at both of the capacity's bounds: a 1.2 MB log and a 1 MiB blob, which the size check refuses before reading, and **a 124,200-byte log that passes the size check and needs a fifth part**, refused at the call site. Each ends `insufficient_capacity`, with no section, no omission, no part record and no charge, with investigation and without, and the 124,200-byte case is also a dry run of the harness; one block shorter, the same log fills exactly four parts and is projected. Mutants C2, C2a, C2b, C2c and K1. **What the model adds is carried beside everything the rule carries** (m5a-3): the fake adds a passing test in every part; the projection carries every excerpt of the rule's at its range, and the excerpts its header names as the model's are exactly the units the records say were chosen, so it is not the rule's projection (K8, K10, M9, M10). A model that adds nothing leaves the rule's ledger as it was, every excerpt, omission and reason, and the header says it added nothing. An `unsupported` result, which the rule does not carry, is carried when a model adds it and is attributed to it. **A question no answer can change is not asked**: forty failures between passing tests fill every excerpt a projection holds, and a request that authorises every part gets the rule's projection with no record and no charge. **Each question says what is true**: its preamble names the failing tests and cargo errors carried and the room the floor leaves, as the rule's own section implies, and it is numbered among the questions asked, not the parts the document fills. **Only sealed, readable, unpurged bytes are projected**: an altered object and a purged artifact whose bytes another artifact keeps are `evidence_unavailable` (K2, K4), and a `\r\n` log is carried as its own bytes (K3). **The header is CBR's own**: a capture anchor holding newlines is shown escaped on one line, and the header holds exactly one `failures named:` and one `read as` line. |
| Cost | Nothing live. The fake charges 5,000 per call, one call per part. |
| Simulated or untested | The model. Whether what a real model adds makes a projection better than the deterministic rule's is what the live run measures; at `/2` its rerun found both models adding nothing, so it is still unmeasured. |
| What this does not establish | Usefulness, and anything about a real model. That the parsers read every shape cargo can print: they read the shapes CBR's suite writes. |

### J1: a question finds its own answer with a cited packet

| Field | Record |
|---|---|
| Intent and acceptance | A user registers a real repository, ingests a decision from it, accepts that decision, then **asks a question without naming what answers it** — and gets back a packet that finds the answer, cites it to exact bytes, and distinguishes what an authority decided from what CBR merely read. **Acceptance is scored against a list written before the run**, in the header of `crates/cbr-cli/tests/journey_one.rs`. The first version of this journey named both answer files by exact path, so the first two oracle facts could not fail; that version is superseded and its weakness is recorded below. |
| Entry point | `cbr ingest`, `cbr authority bind`, `cbr propose`, `cbr decide`, `cbr context`, `cbr request`, `cbr packet` and `cbr fetch`, over the provider's Unix socket under a handed-off credential. No internal call. |
| Prerequisites and inputs | **This repository**, registered at launch, read at the tree `HEAD` names. A fresh data directory. The request carries the task, one selector (`gix`), and **one required item that is deliberately not the answer** (`README.md`). |
| Implementation basis | Branch `m3c/packet-compiler`; protocol pin `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; Rust 1.97.1; macOS, and Linux and macOS in CI. |
| Model and provider | `none`. Nothing in this path calls a model, and the compiler consults none by construction. |
| Code and environment basis | One repository, `cbr`, at the root tree of `HEAD`, with `workspace` and `dirty` as the client resolved them. No environment or build facts. |
| Path taken | ingest the ADR → bind the scope → propose and accept a claim citing it (`binding`) → propose and **reject** a second claim → submit → first tick compiles: index at the basis, item sections, then discovery — ranked retrieval over the view from the task, anchors for identifier-shaped names, and every claim judged against the request basis → publish → read the packet → fetch every citation. |
| Packet identity | One revision. **11 sections, 1 coverage entry, 0 omissions** at 65,536 bytes of capacity, 12,446 bytes of section content. Ordered as INTERNALS section 5 step 5 requires: the required item first, then the binding decision, then spans by retrieval rank, then the rejected alternative last. The accepted claim appears labelled `binding`; the rejected one appears labelled `stale` and `historical`; nine source spans appear labelled `source_inspected`, among them `crates/cbr-identity/src/lib.rs` lines 21–40 (which say in so many words that git is read through `gix`, and why) and `docs/decisions/001-standalone-v0.1-scope-and-stack.md` lines 21–40 (which contain the question 11 row). |
| Durable result | The packet, its sealed bytes and the cited artifacts stay in the store; `cbr fetch` returns each cited artifact byte-identically afterwards, and the bytes equal `git cat-file blob <blob>` at the named tree. |
| Revisited at `cbr-context-compiler/4` | **Every id is an identifier and every citation is expanded.** Until `/4` a discovered span's ids carried its path, so this packet's own provider refused to expand most of its citations; the journey fetched artifacts by id and never followed a citation, which is how that went unseen. The test now checks every section id, citation id, inclusion and omission against the protocol's grammar, and runs `cbr expand j1 <citation>` on **every** citation: a repository blob whole against `git cat-file blob`, and at the range its locator names against both the blob and the excerpt the section shows; an ingested artifact against `cbr fetch`. The number expanded must equal the number of citations, no kind of citation is exempt, and at least one expanded locator names a path below the repository's root, so a flat repository could not pass. At `2eedade` it failed on `d-span-crates/cbr-cli/tests/journey_one.rs-31752`; at `/4` it passes. |
| Reproduction | `cargo build --workspace --locked && cargo test --workspace --locked --test journey_one`; exit 0. The cost line prints with `-- --nocapture`. **No packet is committed as an artifact:** a packet from this repository names the tree it was prepared at, so a committed copy would be stale at the next commit and would invite comparison against a tree it was never about. The record is the test, which rebuilds it. |
| Cost | Model calls 0, tokens 0, spend 0. **Time to first packet 7.3s**, of which **7.2s was the index build and the compile**. The tree held **1,066 blobs**. Store after the run **19.0 MB**. |
| Simulated or untested | Nothing is simulated. **Untested in this journey:** a second repository in one basis; an index that lags the basis (retrieval's fallback has its own tests); a dirty working tree's bytes, which m3d's brownfield pilot is the first to exercise; a model-assisted selection, which is M4. |
| Properties and limits | **Passed**, scored against the predeclared list: the accepted decision as `binding`, the decision record cited to a span holding question 11, the code cited to a span holding `gix`, the coverage naming the frontier, and both traps — nothing read from source is `binding`, and the rejected claim is present and never current. Every citation resolved to the blob at the named tree, and every excerpt was the artifact's own bytes at the line its locator names. **Lexical discovery finds only what shares vocabulary with the task.** The selector `gix` is a word taken from the question, and the spans that came back are the spans that use that word; a task whose answer is written in different words from the question would not be found this way at all. That is a property of lexical retrieval, not a bug in this journey, and it is M4's problem, not m3c's. **Ranking placed three noise spans above the answer** — two from the test file that quotes the question, one from a module sharing the words *repository*, *checkout* and *basis* — so the answer was found, cited, and fourth. That is measured at m3d against each pilot's oracle and against a plain `git grep` baseline, and addressed at M4; it is not tuned away here. **In this run, four of ten discovered sections are noise** — three of them the test file that quotes the question, which is what asking a repository about itself costs. **What it still does not establish:** that the packet was *useful*. The oracle is a list of facts declared in advance, not a measurement of whether a session did better with the packet, which is J6. Ranking is BM25 and unevaluated; two of the eleven sections are the test file that quotes the question, which is an honest consequence of asking a repository about itself. And an excerpt is bounded at 2,048 bytes: for the decision record, whose chunk of twenty table rows is far larger, the excerpt shows the opening of the span while the question 11 row is further inside it — the citation is how a reader gets the rest. |

### J1's negative control: a moved tree is never silently answered from

| Field | Record |
|---|---|
| Intent and acceptance | A repository moves to a new tree and nothing re-ingests it. A packet published at the old tree must still name the old tree and cite the old tree's blob, and a later request naming the old tree must still be answered from the old bytes. The control fails if any citation resolves to the new tree's blob. |
| Entry point | The same `cbr` verbs. |
| Prerequisites and inputs | **A purpose-built repository, not this one.** The control has to move a repository, and moving this one would tie the test to its own branch's history. |
| Path taken | commit A → register → request at A → packet cites A's blob → commit B, nothing re-ingested → the published packet still names A and cites A → a new request at B cites B's blob → a new request at A still cites A's blob. |
| Properties and limits | The positive half matters as much as the negative one: a compiler that simply cached the first tree would pass the negative half, and fails at the step where a request at the new tree must get the new bytes. Its mutant — read the checkout's `HEAD` instead of the basis — is killed here. |

### J8: a deadline leaves required items unmet and advisory items at their fallback

| Field | Record |
|---|---|
| Intent and acceptance | When a deadline passes with items still unsatisfied, a required item is `unmet` with reason `deadline_passed` and an advisory item is `degraded` with the same reason — and the two fallbacks are distinguishable at the same instant: `proceed_with_gap` publishes rather than waiting, `wait_until_deadline` holds the publication while an advisory item is unsatisfied. **Expiry supplies no evidence and no consent:** the packet it publishes cites nothing, includes nothing and satisfies nothing. |
| Entry point | `context.request.submit`, `context.request.inspect` and `context.packet.inspect` as protocol frames over the stdio binding to the real `cbr-provider` binary. No internal call. |
| Prerequisites and inputs | A fresh data directory and a fixed clock, moved past the deadline by restarting the provider. Two requests over the same basis: one `proceed_with_gap` with an advisory item, one `wait_until_deadline` with a required item and an advisory one. Every item names a path nothing will satisfy, so the only thing that can resolve them is the deadline. |
| Implementation basis | `crates/cbr-provider/tests/context.rs`, branch `m3c/packet-compiler`; protocol pin `v0.1.0`; Rust 1.97.1. |
| Model and provider | `none` |
| Code and environment basis | A declared basis naming one repository at a fixed tree. Nothing is read from a checkout, because nothing satisfies anything. |
| Path taken | submit `proceed_with_gap` → published at once, advisory `degraded` with its reason → submit `wait_until_deadline` at the same instant → still `preparing`, no packet → provider restarted with the clock past the deadline → published, required `unmet` `deadline_passed`, advisory `degraded` `deadline_passed`. |
| Packet identity | One revision, with no sections and no citations. |
| Durable result | The request is `unmet` after the restart and stays so; its one packet revision reads back with the same items. |
| Reproduction | `cargo test --workspace --locked --test context j8`; exit 0. |
| Cost | Model calls 0, tokens 0. About five seconds of wall clock, almost all of it two provider starts. |
| Simulated or untested | The clock is a test control (`clock.file`), which is how the deadline is made to pass without waiting for it; the semantics under test are the provider's, not the clock's. **Untested:** a deadline passing while a peer call is in flight, and one passing between the seal and the publication. |
| Properties and limits | Passed, alongside the `context.deadline-leaves-required-unmet-and-advisory-follows-fallback` fixture, which covers the same rule from the protocol's side. This journey adds what the fixture does not: the `wait_until_deadline` arm observed waiting at a named instant, and the assertion that the packet an expiry publishes carries no citation, no section and no satisfied item. |

### J9: an authority transfer invalidates the stale decision path

| Field | Record |
|---|---|
| Intent and acceptance | After a scope's authority is transferred, a decision under the superseded epoch is refused and the previous authority can no longer decide, while every earlier decision stays in effect and no record is edited. Distinguishing: removing the epoch check lets a stale decision commit, and wiping reliance on transfer shows `proposed`; both are caught. |
| Entry point | `knowledge.authority.bind`, `knowledge.authority.transfer`, `knowledge.claim.propose`, `knowledge.decision.record`, `knowledge.claim.inspect` and `knowledge.claim.history`, sent as protocol frames over the stdio binding to the real `cbr-provider` binary. No internal call. |
| Prerequisites and inputs | A fresh data directory. `owner` is the provider's authority principal; `authority-a` and `authority-b` act under grants carrying `knowledge.propose`, `knowledge.read` and `knowledge.decide`. Cold memory state. |
| Implementation basis | The test landed in `732ff6f` (`crates/cbr-provider/tests/knowledge.rs`); protocol pin `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; Rust 1.97.1; run on macOS and in CI on Linux and macOS. |
| Model and provider | `none` |
| Code and environment basis | Not applicable: the claim declares no basis and no conditions. |
| Path taken | bind `svc` → A (epoch 1); A proposes `c` revision 1; A records `d1` `accepted_for_use` under epoch 1; `owner` transfers `svc` → B (epoch 2); A under epoch 1 → `stale_authority_epoch`; A under epoch 2 → `permission_denied` `not_authority`; B inspects `c` → `accepted_for_use` by `d1`; B under epoch 1 → `stale_authority_epoch`; history compared; B records `d2` `rejected` superseding `d1` under epoch 2. |
| Packet identity | None; no packet exists at M2. |
| Durable result | History after the transfer and the three refusals is byte-identical to history before them. `d2` is a new record linked to `d1` by `supersedes_decision`, and `d1`'s history entry, position included, is unchanged. A separate test, `a_claim_and_its_decision_survive_sigkill_at_their_positions`, shows a claim and its decision survive `SIGKILL` at their positions. |
| Reproduction | `cargo build --workspace --locked && cargo test --workspace --locked --test knowledge j9`; exit 0. |
| Cost | No model calls, no tokens; about one second of wall clock. |
| Simulated or untested | Nothing is simulated. Untested: a transfer while another session is mid-decision; decisions reached through `cbr decide` rather than frames. The CLI path is exercised separately by `crates/cbr-cli/tests/knowledge_verbs.rs`. |
| Properties and limits | Passed, each with a mutant observed failing: the stale epoch refused (`decision-epoch-unchecked` fails at A's stale decision), and reliance kept across a transfer (`transfer-resets-reliance` fails at B's inspect). It does not establish that a decision is project acceptance anywhere else (KNOWLEDGE §14). |
