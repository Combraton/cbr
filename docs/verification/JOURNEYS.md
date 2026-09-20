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
| J6 | A fresh agent session uses a CBR packet on a realistic refactoring task; measure whether it preserves constraints and reduces repeated investigation. Relevance is measured by **counting downstream re-investigation of content the packet already contained**, not by self-reported prediction, and is diagnostic only — never a gate criterion (ADR 001, question 9). A labelled **pilot** of this journey runs at M3 with no model, as a steer rather than evidence. | M7 | **yes** | no | **Unsupported-assertion acceptance:** plant a claim whose cited evidence does not support it, and require either that it is not carried as `binding` or that the downstream session is not led into the error. Compared against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline. | — |
| J7 | Real public-protocol investigation and consumption with PIO, while standalone no-PIO operation still works | after M5, gated on PIO | yes | **yes** | **Recursive enrichment and reservation deadlock:** a CBR-initiated investigation must not be re-enriched through the same path, and preparation must not wait on a slot its own consumer holds. Both must be shown to fail when the guard is removed. | — |

| J8 | A required item is still unmet when the deadline passes; it stays unmet, while advisory items follow their declared fallback | M3 | **no** | no | **Required silently downgraded:** remove the guard and a required item must be reported `satisfied` at deadline, or reported under an obligation it was not submitted with. Both are failures. The live behaviour must instead be `unmet` with reason `deadline_passed`, and an advisory item under `proceed_with_gap` must be `degraded` with its reason while one under `wait_until_deadline` waits. Deadline expiry must supply no evidence and no consent. | **pass**, M3c, model `none` ([record](#j8-a-deadline-leaves-required-items-unmet-and-advisory-items-at-their-fallback)) |
| J9 | An authority transfer or epoch change invalidates the stale decision path, without editing any history | M2 | **no** | no | **Epoch ignored, or reliance reset:** a decision carrying a superseded `authority_epoch` must be refused with `stale_authority_epoch`; remove the epoch check and it commits. Separately, a transfer must **not** reset reliance — decisions recorded under an earlier epoch stay in effect until the current authority records a later decision about the same revision. A control that wipes reliance on transfer must be caught. No record is edited in either case: supersession is a later record. | **pass**, M2, model `none` ([record](#j9-an-authority-transfer-invalidates-the-stale-decision-path)) |
| J10 | A purge produces a proof-loss report, and export then restore round-trips the same identities | M6 | **no** | no | **Purge bypasses holds, or restore renames identities:** a purge blocked by an active hold must fail with `hold_active` naming the blocking holds; remove the check and it deletes. And a claim whose supporting evidence was purged must remain `accepted_for_use` with availability `purged` — never silently rejected, and never still reported `available`. After export and restore, every claim, decision, evaluation and packet must resolve under its original identity and digest; a control that reassigns local identities must be caught. | — |

### Journeys currently blocked

J7 cannot run today and will not be simulated. **J6 was unblocked on 2026-09-16** and is now a scheduling question rather than a permission one.

- **J6** needed a granted model provider and a permitted spend. Both now exist — MiniMax on the owner's subscription quota, with a bounded envelope debited before every call ([RELEASE-SCOPE §5](../work/readiness/RELEASE-SCOPE.md)). What J6 still needs is **pre-agreed thresholds derived from pilot variance**, and those must be fixed *before* the confirmatory run. A threshold chosen after seeing the result is not a threshold, so J6 stays unrun until the M3 pilot has produced the variance it is derived from.
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

### J6 pilot, brian2: a brownfield repository, no model

**Labelled a pilot throughout.** It is a steer, not evidence, and may not be cited as downstream-task evidence for any gate ([JOURNEYS §1](#1-rules-this-document-enforces), ADR 001 question 6). It is scored by the reviewer against an oracle the owner wrote before the run; this session has not seen it.

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
| Properties and limits | **This establishes nothing about usefulness by itself.** The packet is scored by someone who has the oracle and did not produce the packet. What the run does establish is mechanical and worth having: a 553-file, 7,065-commit repository was indexed cold and answered in under fifteen seconds with no model; every span cites an artifact and a byte range; the coverage names every gap by count and kind; and the working tree's 130 untracked files are declared rather than silently skipped. **Lexical discovery finds only what shares vocabulary with the question** — nine of ten sections came from term overlap alone, and the single anchor section is the only part that used structure. |

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
