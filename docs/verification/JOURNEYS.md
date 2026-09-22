# Journey verification for standalone CBR

> **Status: three journeys have been run — J9 at M2, and J1 and J8 at M3c, all three with no model.** Every other result column below is empty on purpose. This file is the place journey evidence lands; it is linked from [VERIFICATION](../VERIFICATION.md) and aligned with the shared [verification model](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md). Scope and milestones: [RELEASE-SCOPE](../work/readiness/RELEASE-SCOPE.md).

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
| J1 | A user ingests a real repository and its decisions through the public client, requests code-flow context, and receives a useful cited packet | M3 (deterministic), revisited at M4 (model-assisted) | no for M3, yes for M4 | no | **Stale-source reuse:** move the repository to a new tree without re-ingesting; a packet claiming applicability to the new tree must not be produced. The control fails if the packet is still `applicable`. | **pass**, M3c, model `none` ([record](#j1-a-question-finds-its-own-answer-with-a-cited-packet)) |
| J2 | Large search results, test logs and JSON are processed under bounded model context; omitted material stays explicit | M4 | yes | no | **Silent truncation:** remove the omission marker path; the journey must fail because omitted content is no longer declared. Also: feed an input larger than the summarizer's own capacity and require a typed insufficient-capacity result, never a silent partial summary. | — |
| J3 | A source or requirement changes during preparation; the next delivery exposes the correction and the earlier packet and its history survive | M5 | yes | no | **Correction swallowed:** an older derivation must not become the current result for a corrected item. Removing the `corrected_during_preparation` path must make the journey fail. The earlier packet revision must still fetch its original bytes and report `current: false`. | — |
| J4 | CBR is restarted during model-assisted work; it resumes from durable state without duplicate commits or fabricated completion | M5 | yes | no | **Duplicate commit:** kill the process between the model response and the commit, restart, and require exactly one committed revision. Disable command deduplication and the control must produce two. Separately: a job whose checkpoint is intact must not report a finding it never derived. | — |
| J5 | Conflicting branches or stale evidence do not leak into another task as current truth | M5 | yes | no | **Branch leakage:** a requirement accepted only on branch B must never appear as binding in a packet scoped to branch A. Remove the scope filter and the control must catch it. | — |
| J6 | A fresh agent session uses a CBR packet on a realistic refactoring task; measure whether it preserves constraints and reduces repeated investigation. Relevance is measured by **counting downstream re-investigation of content the packet already contained**, not by self-reported prediction, and is diagnostic only — never a gate criterion (ADR 001, question 9). A labelled **pilot** of this journey runs at M3 with no model, as a steer rather than evidence. | M7 | **yes** | no | **Unsupported-assertion acceptance:** plant a claim whose cited evidence does not support it, and require either that it is not carried as `binding` or that the downstream session is not led into the error. Compared against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline. | journey **unrun**; two labelled pilots, each run twice with no model. brian2 **failed both times**, one of three required facts ([record](#j6-pilot-brian2-a-brownfield-repository-no-model--failed)); Knowscroll-v2 failed at m3d with two of three and **passed the m3e rerun** with three of three, under a caveat that travels with it ([record](#j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun)). Neither is evidence, and [neither measures a session](#what-neither-pilot-establishes). |
| J7 | Real public-protocol investigation and consumption with PIO, while standalone no-PIO operation still works | after M5, gated on PIO | yes | **yes** | **Recursive enrichment and reservation deadlock:** a CBR-initiated investigation must not be re-enriched through the same path, and preparation must not wait on a slot its own consumer holds. Both must be shown to fail when the guard is removed. | — |

| J8 | A required item is still unmet when the deadline passes; it stays unmet, while advisory items follow their declared fallback | M3 | **no** | no | **Required silently downgraded:** remove the guard and a required item must be reported `satisfied` at deadline, or reported under an obligation it was not submitted with. Both are failures. The live behaviour must instead be `unmet` with reason `deadline_passed`, and an advisory item under `proceed_with_gap` must be `degraded` with its reason while one under `wait_until_deadline` waits. Deadline expiry must supply no evidence and no consent. | **pass**, M3c, model `none` ([record](#j8-a-deadline-leaves-required-items-unmet-and-advisory-items-at-their-fallback)) |
| J9 | An authority transfer or epoch change invalidates the stale decision path, without editing any history | M2 | **no** | no | **Epoch ignored, or reliance reset:** a decision carrying a superseded `authority_epoch` must be refused with `stale_authority_epoch`; remove the epoch check and it commits. Separately, a transfer must **not** reset reliance — decisions recorded under an earlier epoch stay in effect until the current authority records a later decision about the same revision. A control that wipes reliance on transfer must be caught. No record is edited in either case: supersession is a later record. | **pass**, M2, model `none` ([record](#j9-an-authority-transfer-invalidates-the-stale-decision-path)) |
| J10 | A purge produces a proof-loss report, and export then restore round-trips the same identities | M6 | **no** | no | **Purge bypasses holds, or restore renames identities:** a purge blocked by an active hold must fail with `hold_active` naming the blocking holds; remove the check and it deletes. And a claim whose supporting evidence was purged must remain `accepted_for_use` with availability `purged` — never silently rejected, and never still reported `available`. After export and restore, every claim, decision, evaluation and packet must resolve under its original identity and digest; a control that reassigns local identities must be caught. | — |

### Journeys currently blocked

J7 cannot run today and will not be simulated. **J6 was unblocked on 2026-09-16** and is now a scheduling question rather than a permission one.

- **J6** needed a granted model provider and a permitted spend. Both now exist — MiniMax on the owner's subscription quota, with a bounded envelope debited before every call ([RELEASE-SCOPE §5](../work/readiness/RELEASE-SCOPE.md)). What J6 still needs is **pre-agreed thresholds derived from pilot variance**, and those must be fixed *before* the confirmatory run. A threshold chosen after seeing the result is not a threshold. **The two m3d pilots have now run**, one scored and failed against its oracle and one awaiting its score; the thresholds are still to be fixed, and until they are, J6 proper stays unrun.
- **J7** needs a PIO standalone service. PIO is being built in parallel and has no release. CBR must not depend on its unreleased work.

J8, J9 and J10 need no model at all and are reachable inside the milestones that introduce them. J2, J3, J4 and J5 each need a model for their full form; with the grant in place that is now reachable at M4 and M5. Their fault-injection halves still run against a **labelled fake model**, which remains the right tool for fault injection and the wrong one for acceptance — the row stays `simulated` until a live run replaces it, and the existence of a budget does not change that.

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

#### The scores, which are the reviewer's and are in the reviewer's words

> brian2-m3 1/3 (fundamentalunits.py 301-340, first time the file is cited); brian2-m27hs 2/3 (fundamentalunits.py 1661-1700 + unitsafefunctions.py 61-120); knowscroll-m3 3/3 with zero claims (DECISIONS.md 621-660 + _spike.mjs 1-40); knowscroll-m27hs 1/3 (choice truncated); j1-m27hs both facts via the deterministic fallback; j1-m3 code only (ADR never offered — item 4). No trap triggered anywhere.

This session has never seen either oracle and scored nothing.

**What the run establishes, narrowly.** brian2's question had failed the deterministic compiler twice, holding one of three facts both times, and `fundamentalunits.py` had never been cited at all; both live runs cite it, and one holds two of three. That is the first evidence that a model proposing search terms reaches something lexical discovery did not — which is the claim m4e was built to test. It is two runs of one question, not a measurement.

**And what it does not.** `j1-m3` returned code only, because the ADR its answer needed **was never offered to the model**: the candidate set was the raw top of the ranking rather than the per-path-capped reading the packet publishes, so thirteen spans of one file filled it. A fact never offered cannot be kept, and a score under that condition measures the candidate set rather than the model. Fixed at m4f; the pairs above were run before the fix and should be read with it in mind.

### What neither pilot establishes

Both records above measure a packet. **Neither measures a session.** No agent session used either packet for any task: nothing here shows that work went better with one, or that a constraint was preserved that would otherwise have been lost, or that investigation was not repeated. That is J6 proper, and it is M7's — with pre-agreed thresholds derived from these pilots' variance, fixed before the confirmatory run, against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline.

What the pilots are for is narrower and worth having on its own: they are the first runs of the write path and the read path over repositories this session did not write, at sizes and in shapes the fixtures do not reach — 5.3 MB and 553 files in one, 22 owner decisions over one append-only file in the other — and they produced the variance M7's thresholds will be derived from, one packet scored against an oracle and one awaiting its score.

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
