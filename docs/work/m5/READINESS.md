# M5 readiness — the two loops, the tool runtime, and the first derivation families

**Written before any M5 code, and reviewed before any.** M4 showed why the readiness document comes first: the decisions that matter are made in it, when they are cheap to change. M5 is where CBR stops asking a model fixed questions and starts letting a model **act** — read, search, propose — inside a loop CBR runs. That is a larger step than anything M4 took, and every bound it needs is written down here before there is code to bend it.

Tracking: [issue #34](https://github.com/Combraton/cbr/issues/34). Scope: [RELEASE-SCOPE §4](../readiness/RELEASE-SCOPE.md)'s M5 row, including what the owner's decisions of 2026-09-23 moved there from M4. The runtime it builds on is M4's ([CLOSEOUT](../m4/CLOSEOUT.md)). The specifications it realises are [MEMORY-ENGINE](../../spec/MEMORY-ENGINE.md), [MODEL-RUNTIME](../../spec/MODEL-RUNTIME.md) and [PREPARATION-AND-DELIVERY](../../spec/PREPARATION-AND-DELIVERY.md).

**The owner's decisions of 2026-09-24**, each recorded where it acts:

1. **`read` may name any path in the view** — §4, [What `read` may name](#what-read-may-name-and-the-controls-that-replace-the-closed-set).
2. **The first claim-producing family is `distill_investigation`**, and the ancestry-root obligation lands with it — §5.
3. **J2's live acceptance runs on CBR's own test output** — §3 and §10.
4. **Before the loop's live run on the Knowscroll question, the harness ingests Knowscroll's 22 decisions** as the owner ruled them — §2, §9 (m5h) and §10.

**The owner's decisions of 2026-09-25:**

5. **`orient_repository` and `record_rejected_approach` land in v0.1**, so all five of [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md)'s families are M5's — §5 and §9 (m5i and m5j). The owner answered the question this document had left open, in the session, in these words, which are in that session's transcript and not in this repository: *"yes land orient_repository and record_rejected_approach for v0.1"*. Where the two go in the order is this session's proposal (§9), not part of the decision.
6. **Live calls under a standing grant**: MiniMax as much as the work needs, with no approval per run, within CBR's envelope — §10. It is in the owner's brief to this session, outside this repository.

## 1. Scope, and the promise

| M5 delivers | M5 does not |
|---|---|
| **Journey 2 first**: large search results, test logs and JSON processed under bounded model context, with what was omitted kept explicit (§3) | Replace M4's envelope. Admission, the ledger, the ceilings, the credential rule and the recording boundary are M4's and stay exactly as they are. |
| **The loop, the tool surface and checkpoints**: a bounded, CBR-owned investigation loop over a small read-only tool set, with durable checkpoints and code-enforced termination (§4) | Give a model shell, network, repository writes or instruction discovery. None is a memory tool ([MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md)). |
| **Model-assisted artifact creation**: derivation families that produce claims and projections, sealed as evidence, with the ancestry-root obligation carried by `distill_investigation`, the first family whose output CBR's own producer cites as claim support (§5) | Let a model decide anything. A model-proposed claim is never `binding`; that is an authority's act. |
| **The two loops**: request-time gap investigation, and bounded, coalesced maintenance with named triggers, priority, budget and finish conditions (§6) | Spend in the background by default. Background spend stays **zero** until the owner enables it ([ADR 001](../../decisions/001-standalone-v0.1-scope-and-stack.md), question 1). |
| **J3, J4 and J5**, each with its negative control (§7) | Promise usefulness. That is J6, at M7. |

**What stays deterministic, and must be shown to.** Everything M3 and M4 made deterministic stays so: the compiler's order, sealing, citations to exact bytes at a named tree, coverage, the drop order, and replay from retained records. A loop may change *what is investigated*; it may not change *how what it found is recorded*, and a request that authorises no investigation still gets exactly the compiler M3 shipped. The golden digests stay the guard.

## 2. Two limits carried in from M4, stated generally

Both come from live run 3 ([JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)), and both are properties of the design, not of a question.

**A fact missing from the candidate set cannot be chosen, so discovery's ranking is the ceiling.** M4's model chooses among ids CBR offered. However good the choice, it cannot keep a fact that was never ranked into the set — which is exactly what the reviewer found in both brian2 flows of run 3. **This is the claim M5's loop exists to test**: a model that can search and read, inside the view, can reach what a ranking did not offer. It is also the claim most easily reported wrongly, so the live plan (§10) runs the same sealed questions against the loop with M4's discovery as the baseline, and the comparison says whether the loop reached a fact the ranking missed — not whether the packet looks better.

**Decision memory with a model in the loop has not been run on a real repository.** Both live Knowscroll runs had zero claims in the store; the harness registers a repository and asks, and ingests nothing. So nothing yet says how a model behaves when binding decisions compete with source spans for its attention. **By the owner's decision of 2026-09-24, the loop's live run on the Knowscroll question ingests Knowscroll's 22 decisions first**, with the owner's rulings of 2026-09-20 — 21 accepted for use as `binding`, D-019 as `evidence` — recorded as the owner, as at m3d ([JOURNEYS](../../verification/JOURNEYS.md#j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun)). That is a change to the harness, which is code, and it is reviewed before the run (m5h, §9).

## 3. Journey 2 first

**What it is** ([JOURNEYS](../../verification/JOURNEYS.md#3-journey-matrix)): large search results, test logs and JSON are processed under bounded model context, and omitted material stays explicit. It is the `project_large_result` family of [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md): input a large log, test report or JSON artifact; output named failures, decisive excerpts, run identity, a full-artifact reference and explicit omissions. **It is m5a, the first code of M5**, built on M4's direct calls: one bounded question per part, with no loop and no tools.

**Its two negative controls, unchanged:**

1. **Silent truncation.** Remove the omission-marker path, and the journey must fail because omitted content is no longer declared.
2. **Oversize input.** Feed an input larger than the summariser's own capacity, and require a typed insufficient-capacity result — never a silent partial summary.

**How it is built, as rules before code:**

- **The whole payload is sealed first**, as evidence with its digest and coverage; the projection is derived from it and cites it. A projection never replaces the artifact, and a reader can always fetch the full bytes ([MEMORY-ENGINE §5](../../spec/MEMORY-ENGINE.md)).
- **Deterministic parsing where the format allows**: a test runner's report, a JSON path, a log's line structure. The model is used for interpretation and selection, not for reading a structure a parser can read.
- **Values and units are never changed** in a projection ([MEMORY-ENGINE §4](../../spec/MEMORY-ENGINE.md)). A projected number is the source's bytes at a cited range, not the model's restatement of it. This is not hypothetical here: brian2's pilot question is about units being silently dropped.
- **A summariser is never given more than its own capacity** ([MODEL-RUNTIME §4](../../spec/MODEL-RUNTIME.md)). Input that does not fit is partitioned, each part a bounded call with its own record, or the result is the typed insufficient-capacity outcome of control 2. Partitioning keeps cross-part references as unresolved items rather than losing them.
- **Every omission is a typed record** with its reason and its extent, carried into the packet the way M3's omissions are.
- **The model adds to what the parser found, and never removes it** (m5a-3, after J2's live run of 2026-09-25). In m5a a model's choice replaced the rule's, and whatever it did not choose was declared `not_selected`, failure blocks the parser had already named among them. On the failing log, both models' projections scored 55 and 41.25 script points against the rule's 70, because both left out failures the rule carries ([JOURNEYS](../../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed)). From `cbr-project-large-result/2` the rule's projection — every failure the parser found, then run identity — is a **floor**, computed first and frozen. A model is asked only what to add to it, and is offered only units that are neither failures nor run identity, that the floor leaves out, and that would fit beside it drawn at the model arm's widest. It is asked nothing when nothing could be added. So for every answer a model can give, the model arm carries every byte the rule's arm carries, and no failure the parser found is ever `not_selected`. The header names the excerpts the model added, and every excerpt is wholly the rule's or wholly the model's. The decision is [ADR 001's question 16](../../decisions/001-standalone-v0.1-scope-and-stack.md).

**Why it comes first, beyond the owner's order.** [MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md) has large tool results *"stored with digest/coverage, then projected"*. The tool surface (m5c) therefore needs J2's sealing and projection to exist, and building J2 first means tools are built on it rather than beside it.

**Its live inputs, by the owner's decision of 2026-09-24: CBR's own test output.** Logs and JSON produced locally by running CBR's public suite at a pinned commit, sealed whole. Nothing third-party. Two facts the harness has to meet, found while writing this:

- **libtest's own JSON output is unavailable on the pinned toolchain.** `cargo test -- -Z unstable-options --format json` is refused on rustc 1.97.1, the channel `rust-toolchain.toml` pins, with *"the option `Z` is only accepted on the nightly compiler"*. The JSON therefore comes from what stable produces: cargo's `--message-format json`, and the conformance runner's `manifest.json` when VERIFICATION's block is run into a temporary directory.
- **Output produced locally names the machine it ran on.** Compiler messages carry absolute paths in `package_id`, `manifest_path`, `src_path` and `filenames`, and the runner's transcripts carry the checkout path, which is why `run_fixtures.py` substitutes it before committing. Scrubbing after the seal would mean the input is no longer sealed whole, so the suite is run from a location whose path names nothing private. Before the send, the harness refuses any input in which a check of `result_paths.py`'s kind finds a machine path.

## 4. The loop, the tool surface and checkpoints

**CBR owns the loop.** [STACK §8](../readiness/STACK.md) decided it: the design of `rig-agent`'s `AgentRun` (`next_step`, `model_response`, `tool_results`) is borrowed and the dependency is not, because CBR must own the checkpoint format under either choice. M4's runtime is its single step: every turn is an M4 call, admitted, charged, recorded and sealed exactly as today.

### The tool surface

From [MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md), and no more: **search** permitted evidence, **read** a bounded span, **inspect** artifact revisions, **compare** scoped code snapshots, **list** dependencies, **propose** a memory patch, and **produce** a packet candidate.

- **Every tool call is authorised again in CBR**, against the view and readable claims resolved at the command — the rule M3 learned when discovery ran on the provider's own authority and leaked every claim. A tool is a door, and every door gets the gate.
- **Tool payloads are untrusted data.** A span that says "ignore your instructions" is repository text. Negative control 5 extends to every tool, as stated below.
- **No instruction discovery.** A repository's `AGENTS.md` or `CLAUDE.md` is text to read, never instructions to follow.
- **`tool_choice: "required"` is silently ignored by the provider** ([M4 READINESS §4](../m4/READINESS.md#4-provider-facts-that-are-design-inputs-not-discoveries)). A text answer where a tool call was demanded is an ordinary outcome of the loop, handled like M4's prose answers: bounded repair, charged to the same ledger.

### What `read` may name, and the controls that replace the closed set

**By the owner's decision of 2026-09-24, `read` may name any path in the view.** This knowingly relaxes [M4 READINESS §5](../m4/READINESS.md#5-repository-text-is-untrusted-input)'s *"the model never introduces a path, a span, a citation or a label"*, for this tool alone. Everything else keeps it:

- M4's selection and discovery still choose ids from closed sets.
- `inspect`, `compare` and `list` take ids from sets CBR returned.
- No tool lets a model introduce a citation or a label.

What replaces the closed set for `read`:

- **Paths are resolved against the job's pinned tree through CBR's own tree index, never the filesystem.** A link is followed by m3e's rule ([VERIFICATION](../../VERIFICATION.md)): inside the same tree and relative to its own directory; an absolute target, one that climbs out, or a chain over `LINK_HOPS` is refused; a regular file of the tree is read from the target, at the target's path, with the link named. The target is authorised again as a path in its own right.
- **`..`, absolute paths, URLs, and anything else outside the path grammar are typed refusals.** The grammar is CBR's own, since the protocol bounds a path only by length. It is written, with its tests, in m5c.
- **A path's length and characters are bounded, and so is the read range**, each a named constant sized in m5c.
- **Every read is authorised again**, against the view and the readable claims resolved at the command.
- **An out-of-view path answers exactly as a nonexistent one does**, so no tool is an existence oracle. The gate compares the two results byte for byte, and the turn records they leave.
- **Citations and labels stay CBR's.** CBR resolves the bytes at the named tree and cites them itself. The model names a path and a range; a packet never carries a citation the model wrote.

**Negative control 5, extended.** A planted file that steers reads *within* the view is now permitted, because a model that may name any path in the view can be led to any of them. So the control asserts, over every tool and over the bytes sent to the provider:

- nothing outside the view is read or revealed;
- no existence is leaked;
- there is no label or citation the model chose;
- nothing is binding.

### Tool arguments are untrusted input

A tool argument is text a model wrote and CBR then acts on, which is what m4e said of a term. Every argument is bounded, and **every argument that breaks a bound is a typed outcome, never trimmed**: taking part of an argument would be CBR deciding which part of the model's answer to act on (m4e's rule).

| Tool | Argument | Bound |
|---|---|---|
| `search` | query | M4's term bounds on count, length and characters (today 4 terms of at most 48 bytes, no spaces), through M4's own tokeniser into CBR's own index |
| `read` | path and range | The bounds of [What `read` may name](#what-read-may-name-and-the-controls-that-replace-the-closed-set) |
| `inspect`, `compare`, `list` | ids | Only ids from sets CBR returned to this job; any other id is invalid output, never resolved |
| `propose` | a memory patch | Validated by the write path every other write goes through; it cites only ids CBR returned, and its statement is bounded in bytes; a proposal, never `binding` |
| `produce` | a packet candidate | Section ids from sets CBR returned; the labels, citations and order are the compiler's |

### Termination and checkpoints

- **Termination is code's, never the model's.** Step, call, byte and token limits and a declared finish condition end a job ([MODEL-RUNTIME §4](../../spec/MODEL-RUNTIME.md)). "The model says it is done" is one input to the finish condition, not the condition.
- **A checkpoint is a durable record** holding the fields [MODEL-RUNTIME §1](../../spec/MODEL-RUNTIME.md) names — goal, pinned basis, constraints, artifact ids, inspected evidence ids and ranges, findings with support, competing hypotheses, unresolved dependencies, pending work, consumed budget. The mechanically known fields come from the runtime, not from a model's recollection.
- **A resumed job rebuilds its working context from the checkpoint and its sources**, never from a transcript. Hidden reasoning is not persisted and nothing depends on it.
- **A finding is committed through the validated write path**, with the same revision checks every other write has, so a delayed worker cannot overwrite a later correction (§7, J3).

**Checkpoints and per-turn loop records get m4d's readable-set gate at every door** ([M4 READINESS §6](../m4/READINESS.md#6-recording-and-replay)): `evidence.fetch`, `evidence.inspect`, `evidence.query`'s listing, and replay. The set is sealed on the record and checked after the caller has been allowed to read artifacts at all. A listing hides the row and counts it as filtered, and a direct read is `permission_denied`.

**They hold no repository text where ids, ranges and digests will do**, as m4d's derivation records hold none. What was inspected is a path, a range and the digest of its bytes, and a tool result is referenced by its digest. The bytes are the source artifact, or the sealed payload of §3, and are never copied in.

**Where they must hold model prose, they do, and it is gated.** Findings, competing hypotheses and the unresolved-question list are model-authored ([MODEL-RUNTIME §1](../../spec/MODEL-RUNTIME.md): *"explanatory fields remain model-authored derivations with provenance"*). Each may quote repository text, so each:

- is readable only under the readable set;
- is bounded in bytes by a named constant;
- enters a packet only as `inferred`, citing its support.

A turn record also keeps each tool argument as the model wrote it, as m4e's `Proposed` keeps terms. That is model text too, and it is the evidence that a planted instruction reached no further than an argument; it sits behind the same gate.

### Replay, turn by turn

The loop replays by m4d's rule, applied to every turn:

- **Each turn's question digest covers everything that turn was shown**: the job contract, the checkpoint, the tool schemas, and earlier tool results by their digests. A turn shown different evidence is a different question.
- **A rebuild with the panicking transport answers every turn from its record**, charges nothing and reads no credential. A turn nothing retained is `model_answer_not_retained`, never a quiet fall back to the deterministic path.
- **Tool results are recomputed at the pinned basis during a rebuild and must match their recorded digests**, or the rebuild ends with a typed outcome. A result that no longer matches is not the evidence the recorded answer saw.
- **The ambiguity rule applies per turn.** Two records that disagree for one turn make that turn unreplayable, and the rebuild stops there with a type, because every later turn's question includes that turn's answer. A loop rerun live can leave many ambiguous questions, and the harness must report each one — the turn, both records, and the answers that differ — as m4e's `--ambiguity` does.

**The gate is in the loop's own pull request (m5b): a loop rebuilt offline reproduces its finding.** m5c extends the recompute-and-match rule to every tool.

### MODEL-RUNTIME §6's eight runtime-selection tests

The owner's decision of 2026-09-23 moved these to M5 with the tool runtime. [M4's close-out](../m4/CLOSEOUT.md#the-runtime-selection-tests-of-model-runtime-section-6) records what the direct-call runtime already exercises; this is what is new for tools.

| Item | Already exercised by M4's direct calls | New for the loop and its tools |
|---|---|---|
| Pre-call context control | Yes — admission counts the whole body | Tool schemas and tool results are in the body; admission runs again before every turn, after every tool batch |
| Aggregate tool limits | No — M4 has no tools | **Entirely new**: a per-batch response budget, so ten bounded results cannot overflow the next call. Overflow is sealed with its digest and coverage and projected by J2's family, and the turn gets the projection and a handle. |
| Cancellation | Yes | Cancelling mid-loop leaves a checkpoint and no partial commit |
| Error and usage fidelity | Yes, and since m4h a record carries every attempt | Usage per turn, summed per job against the ledger; a tool's error is a typed outcome the loop reads, never an exception it swallows |
| Model fallback policy | Yes — no fallback | A failed turn's policy: end with partial coverage and a named gap, or one bounded retry; never a fallback to another model or to BM25's first |
| Resource-discovery isolation | Not applicable — M4's models read nothing | **Entirely new**: a read names a path in the view, resolved through CBR's tree index at the pinned tree and never the filesystem. Every search is CBR's own index. No instruction files, no existence oracle, and the extended negative control 5 against every tool. |
| Exact output capture | Yes, through the recording boundary | Every tool call and tool result is recorded; every turn is a sealed derivation |
| Restart without duplicate commits | Yes, for the ledger and the seal | **J4**: a kill between a model response and its commit leaves exactly one committed revision after restart |

## 5. Model-assisted artifact creation, and the ancestry obligation

**The families** are [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md)'s: `distill_investigation`, `project_large_result`, `orient_repository`, `record_rejected_approach` and `answer_gap`. Each is a named job with a fixed input, a fixed output shape and its own finish condition — not "the model can do anything". **All five land in v0.1**, by the owner's decision of 2026-09-25: `record_rejected_approach` is m5i and `orient_repository` is m5j.

- **What a family produces is sealed as evidence** under a CBR-specific source kind reserved for derivation output ([PROTOCOL-PIN](../readiness/PROTOCOL-PIN.md)), with CBR's own principal as producer and a coverage describing what the derivation inspected.
- **A claim it proposes is proposed**, by CBR's principal, and is never `binding`. The m2 rule that a model cannot accept its own claim holds unchanged.
- **Prose a model writes enters a packet only as `inferred`**, citing what it was derived from ([M4 READINESS §5](../m4/READINESS.md#5-repository-text-is-untrusted-input)).

**The first claim-producing family is `distill_investigation`, by the owner's decision of 2026-09-24.** Its input is a bounded set of evidence artifacts from one exploration. Its output is a cited flow artifact, `interpretive` claims, and an unresolved-question list. It reads what it distils through the tools, so **it needs the loop and the tools first** (m5b and m5c), and it is m5d.

**The ancestry-root obligation**, moved from M4 by the owner's decision of 2026-09-23, **lands in m5d with `distill_investigation`, with its test and its mutant.** CBR's own producer never names a derived artifact as an ancestry root. When the family's output is cited as claim support, each support entry's `ancestry.roots` are the captured evidence the derivation read.

- **The test** drives `distill_investigation` through CBR's own producer, with a labelled fake model for determinism, over one captured artifact twice, and requires `single_lineage` ([PROTOCOL-PIN](../readiness/PROTOCOL-PIN.md), the negative control for the ancestry rule).
- **The mutant** makes the producer list the derived artifact as its own root. The test must kill it by reading `multiple_lineages`.

`project_large_result` produces a projection, not claim support, so J2 does not carry the obligation. `answer_gap` proposes claims too and comes after `distill_investigation` (m5e), so the obligation already holds when it arrives.

## 6. The two loops

**Request-time gap investigation** is `answer_gap`: an unmet item during preparation, a read scope and a budget, and either new supported claims or an explicit unresolved gap. It is the loop of §4 run for one item, under the request's own deadline and investigation limit — and it is where limit 1 of §2 is tested, because a loop that can search and read is the first thing that can reach past the ranked set.

**One thing m5e has to settle, found while writing this.** `answer_gap` starts from an *unmet item*. The pilot questions' failures were in discovery, not in an item: brian2's request names no answer path, and nothing in it is unmet when discovery's ranking misses the fact. So for the live comparison of §10 to exercise the loop at all, m5e must say what starts a gap loop on a question with no unmet item. There are two choices:

- the question's required fact is expressed as an item the loop can find unmet;
- discovery's own shortfall counts as a gap.

Either is reviewed at m5e's head, before the harness of m5h is written against it.

### What a loop spends of a request's investigation limit

m4e made the limit one number with one meaning: every distinct question spends one unit, a question already asked this compile is free, and items are asked before discovery. **A loop turn is a question and spends one unit of the same limit.** What happens when the limit cannot cover a whole flow is picked per case:

| Case | Counts against | When the limit cannot cover it |
|---|---|---|
| An item's own selection question (M4) | The request's limit, one unit, asked first | Not asked: the item gets `investigation_budget_exhausted`. Unchanged. |
| Discovery's two steps (M4) | The request's limit, two units, claimed together, after items | **Not started.** Unchanged: its steps are one flow, and half of it carries nothing usable. |
| A J2 projection inside a request (m5a) | The request's limit, one unit per part, claimed together | **Not started**, typed. Its parts are one flow, like discovery's, and a projection with a part never read would name failures from part of a log as if from all of it. |
| A gap loop for an unmet item (m5e) | The request's limit, one unit per turn, **after items and discovery** | **Not started below its floor**, a named constant sized in m5e that covers one search, one read and an answer. **Above it, started, and ended at the limit with partial coverage and a named gap**: what it inspected, and `investigation_budget_exhausted`. Never a finding it did not derive. |
| `distill_investigation` and other family jobs outside a request (m5d, m5f) | No request's limit: the job's own step limit and the per-job ceiling, and for maintenance its per-trigger budget | **Ended with partial coverage**: its output already carries an unresolved-question list, and that list names what was not reached. Not started when background spend is off (below). |

**Gap loops come after discovery** so that M4's flow runs unchanged in any request whose limit covers it. That is what makes §10's comparison against M4's discovery a comparison, and it gives the open-ended part what is left. It is the owner's to reverse, as m4e's ordering was.

### Maintenance

**Maintenance** is bounded and coalesced ([MEMORY-ENGINE §6](../../spec/MEMORY-ENGINE.md)). Every job has a basis, a question, a permission scope, a priority, a budget, a progress record and a finish condition.

- **Named triggers**: explicit requests, completed investigations, failed checks, handoffs, corrections, and gaps found while preparing a packet. Related events are batched; nothing launches a model per line of a log.
- **Foreground first**: a context request takes priority over optional maintenance, under the bounded scheduling M4's work pool already provides.
- **Deduplicated** by source revision, job purpose and configuration, so a newer event makes an older result historical rather than current.

**Background spend** is any model call not caused by a request a principal submitted. Every trigger above but an explicit request is background by that definition. m5d's `distill_investigation` runs only when a principal submits it, so nothing before m5f spends in the background.

- **Enabling it is a configuration member that defaults off**, and turning it on is the owner's word.
- **With it off, no maintenance call reaches admission.** A maintenance job that would call a model is not started, and says so.
- **The gate is a test**: with it off, a trigger that would start a model-assisted job produces no admission, no ledger row, and no reach of the transport.

## 7. J3, J4 and J5, and the fixtures that need an execution peer

Each journey keeps the negative control [JOURNEYS](../../verification/JOURNEYS.md#3-journey-matrix) gave it:

| Journey | What it shows | Negative control |
|---|---|---|
| **J3** | A source or requirement changes during preparation; the next delivery exposes the correction, and the earlier packet and its history survive | **Correction swallowed**: an older derivation must not become the current result for a corrected item. Removing the `corrected_during_preparation` path must fail the journey; the earlier revision still fetches its original bytes and reports `current: false`. |
| **J4** | CBR restarts during model-assisted work and resumes from durable state, with no duplicate commit and no fabricated completion | **Duplicate commit**: kill between the model response and the commit, restart, and require exactly one committed revision; with command deduplication disabled the control must produce two. A job whose checkpoint is intact must not report a finding it never derived. |
| **J5** | Conflicting branches or stale evidence do not leak into another task as current truth | **Branch leakage**: a requirement accepted only on branch B never appears as binding in a packet scoped to branch A; remove the scope filter and the control catches it. |

**The composition fixtures** ([RELEASE-SCOPE §6](../readiness/RELEASE-SCOPE.md#6-dependency-on-an-execution-peer)). Of the fourteen at the pin, three run today: `direct-fetch-needs-one-grant-per-audience`, `packets-are-sealed-at-a-separate-evidence-provider` and `thirdparty-kernel-enforces-required-claim-boundary`.

The other eleven declare a profile CBR never serves:

- **Ten declare `execution`**: `blocked-consumer-releases-the-slot-for-preparation`, `cancelling-one-consumer-leaves-the-other`, `claims-in-packets-block-only-the-required-boundary`, `context-provider-outage-advisory-proceeds-required-waits`, `correction-before-dispatch-blocks-the-superseded-revision`, `executor-fetches-bound-packet-and-seals-outputs`, `executor-verifies-assembled-packet-bytes`, `missing-required-context-blocks-bound-work`, `pinned-and-current-bindings-treat-supersession-differently` and `preparation-runs-without-the-consumer-slot`.
- **One declares `verification`**: `thirdparty-publisher-feeds-reference-verification`.

**What is reachable, and what is not established.** Each of these fixtures starts every participant — the evidence provider, the context provider and the executor — from **the one participant descriptor under test**. So with CBR's descriptor the executor participant is CBR, which serves no `execution/1`, and the fixture is unsupported. The pinned release's source archive does carry the protocol's own reference provider (`conformance/reference`, which has an execution module), and RELEASE-SCOPE §6 allows it as a peer so long as a result is labelled a **reference executor** and never reported as real-adapter evidence. **Whether the runner can launch one participant from a different implementation than the others is not established.**

- **m5g checks exactly that, docs only, and may go at any point.** Until it passes, all eleven stay **blocked**.
- **Journey 7 waits for PIO** whatever that check finds.

## 8. Every new bound has its arithmetic computed by a test, and a fixture that reaches it

M4's lesson, twice over: a bound nothing presses against is not tested (the pool's unsettled slot at m4c, the union's cap at m4e), and a published figure a test does not compute drifts (m4f to m4g). So, for every bound M5 adds:

- **It is a named constant**, and the worst case it permits is **computed by a test** from the real request bodies, against the per-request, per-job and run ceilings it has to fit under — as `discovery::tests` computes M4's flow.
- **A fixture reaches it.** A fixture presses each bound — an investigation that exhausts its step limit, a tool batch that overflows its aggregate budget, a J2 input larger than the summariser's capacity, an argument one byte over its bound — and the assertion is on what went out or what was refused, not on a count someone typed.
- **A number that lives in two places has a test across the boundary**, as the harness's worst case does since m4g.

These bounds are already known to be needed, before any is sized:

- steps and calls per job;
- bytes per tool result, and bytes per tool batch;
- span-read bytes, and the read range;
- a path's length and its characters;
- search page rows;
- checkpoint size, and the bytes of each model-prose field in it;
- the gap loop's floor;
- the working-set allocation of [MODEL-RUNTIME §2](../../spec/MODEL-RUNTIME.md);
- J2's input capacity, and its projection's size;
- maintenance's coalescing window and per-trigger budget.

**None is sized here.** Each is sized in the pull request that introduces it, with its arithmetic test beside it.

**J2's, as sized.** `projection::tests` computes each from real request bodies, and pins it:

- **A part's worst call is 41,017 tokens.** That is a part at every bound (`PART_BYTES` of candidates as the body carries them, `PART_UNITS` ids, the widest labels and numbers, and the preamble at its widest), with a task larger than the protocol admits.
- **A whole projection's worst case is 492,204**: that part, sent three times (counted once and repaired once, as discovery's arithmetic has it) for each of `MAX_PARTS` parts. `j2_run.py`'s `WORST_CASE_PROJECTION_TOKENS` is the same figure, read back across the boundary by a test.
- **Beside discovery's published flow of 373,188, it is 865,392**, under the per-job ceiling of 1,000,000, asserted by a test.

m5a's figures were 40,680, 488,160 and 861,348. m5a-3 added the preamble, which says in counts what the floor carries and the room it leaves (at most `PREAMBLE_BYTES`, 142), and lengthened the instruction to ask what to add. Those two changes are the difference.

## 9. The pull requests, each with its gate

In the order the owner set: J2, then the loop, then the tools, then the families that need both. **J2 live failed on 2026-09-25**, so m5a-3 and J2's live rerun come before m5b. **The two families added on 2026-09-25 go after m5e and before m5f**, so that maintenance, whose triggers include completed investigations, can start them. The whole order is m5a, m5a-3 and J2's rerun, m5b, m5c, m5d, m5e, m5i, m5j, m5f and m5h, with m5g at any point. Where m5a-3 and the two families sit is this session's proposal and the owner's to change.

| | Scope | Depends on | Gate |
|---|---|---|---|
| **m5 readiness** | This document and the M5 tracking issue. Docs only. | — | The reviewer has seen it. **No M5 code before that.** |
| **m5a** | **J2**: `project_large_result` on M4's direct calls. The whole payload sealed first, deterministic parsing where the format allows, the model for selection, partitioning or a typed insufficient-capacity outcome. And J2's live harness (§3), dry run only. | Readiness; M4's runtime as it is | **J2's two negative controls**, end to end through the public client with a labelled fake. A projected value is the source's bytes at its cited range. The projection's arithmetic as a test. The harness refuses an input carrying a machine path. |
| **m5a-3** | **J2's fix, after its live run failed** ([JOURNEYS](../../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed)): the parser's failures become a floor a model's choice adds to and cannot remove; a model is offered only what could still be carried, and asked nothing when nothing could; the header names the excerpts the model added. Projection format `/2`. | m5a | **By construction, for every answer a model can give, the model arm carries every byte the rule's arm carries**, as a test over answers and fixtures, with a mutant for each way to break it. No failure the parser found is ever `not_selected`. No call when nothing offered could be carried. m5a's mutants re-run. Then J2 live again, under the frozen rubric with only mechanical changes, frozen before the rerun. |
| **m5b** | **The loop and its checkpoints**, over M4's single step, with a fixed read and no tools. Durable checkpoint and turn records; code-enforced termination; resume from checkpoint; the readable-set gate at every door; replay turn by turn. | M4's runtime. It follows m5a by the owner's order, not because it needs anything m5a builds. | **J4's control**: a kill between model response and commit leaves exactly one revision, and two with deduplication disabled. Termination by code under a scripted model that never says it is done. The loop's worst case computed against the per-job ceiling. **A loop rebuilt offline reproduces its finding.** The readable-set gate on checkpoints and turn records at fetch, inspect, the listing and replay, both arms of each, with a mutant per door. |
| **m5c** | **The tools** and their aggregate limits: the seven of §4, their argument bounds, `read`'s reach and the controls that replace the closed set, authorisation again at every tool, negative control 5 extended. **And a projection sealed as its own artifact**, the `cbr.artifact.projection` m5a's STATE proposed, under the source kind §5 reserves for derivation output: m5a carries a projection as the content of a packet section, with each part's exchange a sealed record, and a tool turn is the first thing that takes a projection by handle. The reviewer accepted that split at m5a's review. | m5a, because a large tool result is sealed and projected by J2's family; m5b, for the loop that calls them | §4's table: pre-call control, aggregate limits and resource-discovery isolation, each with a fixture that reaches its bound. **An out-of-view path and a nonexistent one give byte-identical results and records.** Every argument bound a typed outcome, never trimmed, with a fixture at each. A repository outside the view appears in no tool result and no request body, asserted over the bytes sent. Replay's recompute-and-match rule over every tool. A sealed projection is served only to a reader who may read the artifact it projects, at every door, both arms of each. |
| **m5d** | **`distill_investigation`**, with **the ancestry-root obligation** and its mutant. Started only by a principal's submission. | m5b and m5c: it reads what it distils through the tools | `single_lineage` over one captured artifact twice; the mutant reads `multiple_lineages`. A proposed claim is never binding; its claims are `interpretive` and cited; its unresolved-question list names what it did not reach. |
| **m5e** | **Request-time gap investigation** (`answer_gap`), and **J3** and **J5**, with the investigation accounting of §6 and what starts a gap loop on a question with no unmet item. | m5b and m5c; m5d, because a gap's claims are claim support and the ancestry obligation must already hold | J3's and J5's negative controls. A request that authorises no investigation still produces the golden packet. A fixture below the gap loop's floor that starts no loop, and one at the limit that ends with partial coverage and a named gap. M4's flow is unchanged in a request whose limit covers it. |
| **m5f** | **Maintenance**: triggers, coalescing, priority, per-trigger budget, finish conditions, deduplication, and subscriber semantics. Background spend defined as in §6, behind a configuration member that defaults off. | m5d and m5e, because completed investigations and gaps found while preparing are among its triggers | **With background spend off, no maintenance call reaches admission.** A trigger storm is one job; a foreground request pre-empts maintenance. |
| **m5i** | **`record_rejected_approach`**, by the owner's decision of 2026-09-25: a failed attempt and its evidence in, a scoped rejection artifact and an `interpretive` claim out ([RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md)). Started only by a principal's submission until m5f. | m5d, because its claim is claim support and the ancestry obligation must already hold; m5c, for the tools it reads its evidence through | Its claim is never binding and cites the captured evidence of the attempt as its ancestry roots, `single_lineage` over one attempt recorded twice. The rejection is scoped: a packet at another scope does not carry it as current. Its arithmetic as a test, and its journey through the public client. |
| **m5j** | **`orient_repository`**, by the owner's decision of 2026-09-25: a registered repository at a tree in, a bounded inventory map with declared gaps out. Progressive, never a startup barrier (AGENTS.md). | m5c, for `search`, `read` and `list`; m5e, so that a gap loop can use a map when one exists | The map is bounded by named constants, each reached by a fixture; every part of the tree it did not reach is a declared gap by count and kind, as M3's coverage is; a request never waits for it; a map at an old tree is not offered as current at a new one. Its journey through the public client. |
| **m5g** | **The composition feasibility check** of §7. Docs, plus a descriptor only if the runner allows one. | Nothing; it may go at any point | Either the ten execution fixtures run against the reference executor, labelled as such, or the record says why they cannot. |
| **m5h** | **The loop's live harness**: the loop arm and M4's discovery arm over the same sealed questions; per-turn ambiguity reporting; and, before the Knowscroll question, **Knowscroll's 22 decisions ingested with the owner's rulings of 2026-09-20, recorded as the owner as at m3d**. | m5e | A dry run against the fake. The 22 decisions are applied through the public client as the owner, with m3d's rationale, and verified afterwards as `binding=21 evidence=1`, nothing else. Every ambiguous turn is named in the report. m4e's refusals all still hold. |

**Every pull request after this one**: the test first and observed red, then the fix; mutants observed dying against the whole workspace; the gates of VERIFICATION; and no model called by anything in the suite.

## 10. Live calls: each planned, estimated and capped before it

M4's practice stands, with one change since 2026-09-25: **the owner's standing grant**, below, replaces the owner's word at the time and the owner's cap per run. No live call happens inside a pull request's work, and none happens without its own step.

- **An estimate computed before the call**, from the arithmetic tests of §8 rather than chosen by eye. M4's estimate was 1,500,000 against a measured 65,144 and 67,395. The next estimate starts from those measurements.
- **A hard cap**, enforced by the run ceiling the harness already passes as the cap less what earlier launches spent, and **a stop checked before each run** against that run's computed worst case.
- **The owner's word at the time**, carried in the reviewer's run instruction, after the reviewer has pinned the manifest and checked every origin public by the API. This held through J2's first live run. Since the standing grant, the session that runs the call writes its estimate and ceiling here first, and checks its inputs' origins public itself.

**The runs, as the owner decided them on 2026-09-24:**

1. **J2 live**, once m5a merges, over **CBR's own test output**: logs and JSON produced locally by running CBR's public suite at a pinned commit, sealed whole, with no machine path in them (§3). Nothing third-party.
2. **The loop against the sealed pilot questions**, once m5h merges, with M4's discovery as the baseline. This is the run that tests limit 1 of §2. **For Knowscroll, the harness ingests its 22 decisions first**, which tests limit 2.

**Caps were the owner's**, per run, set when each run's estimate existed, and written into this document before the run; M4's cap was 5,000,000. **Since 2026-09-25 the owner's grant is standing**: MiniMax may be called as much as the work needs, with no approval per run, on `MiniMax-M2.7-highspeed` and `MiniMax-M3` only and on content from public repositories only. Each run's estimate and ceiling are written here before it, and what it spent after. Raising a harness's hard-coded cap is allowed, with the reason recorded. Every run stays inside CBR's envelope, **20M tokens per 5-hour window and 200M per month** ([STACK §8.1](../readiness/STACK.md) is the single source, per [ADR 001](../../decisions/001-standalone-v0.1-scope-and-stack.md)), which is drawn from the owner's subscription quota, shared with the owner's other tools ([RELEASE-SCOPE](../readiness/RELEASE-SCOPE.md)); a run that would approach the envelope stops and goes to the owner. The grant is in the owner's brief to this session, which is outside this repository, as decision 6 at the top of this document says.

### J2 live: the inputs, the estimate, the stop and the cap, decided 2026-09-25

**Decided by the owner on 2026-09-25, and run the same day**, after #40 merged as `041ad5f`: [what it spent](#j2-live-what-it-spent-2026-09-25) is below, and [the record](../../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed) is in JOURNEYS.

**How the inputs were produced.** CBR was cloned from GitHub at `9cd388a`, m5a's merge, into a disk image mounted at `/var/tmp/cbr-j2`, so that no path the output could name holds a user, a machine or a volume. `CARGO_HOME` and `TMPDIR` were there too, and that mattered. With the checkout alone moved, cargo's JSON still named the home directory: every dependency's `manifest_path` and `src_path` point into the registry under `CARGO_HOME`, which §3 anticipated as a kind of path but not as that one. `cargo test --workspace --locked` passed, with 644 tests over 40 result lines, and VERIFICATION's seven conformance suites each matched their expectations. Every input passes `j2_run.py`'s machine-path refusal, and nothing in any of them names the owner or the machine. The manifest pinning each digest is kept outside this repository. The `f73415d` log was produced the same way, from the same clone checked out at that commit, with `--no-fail-fast` so that every test binary ran.

**Every input, and what the harness's dry run did with it**, against the labelled fake:

| Input | Commit | Bytes | Digest | Read as | Parts | Baseline, no investigation | Asks a model live |
|---|---|---:|---|---|---:|---|---|
| `cargo-test-f73415d.log`: `cargo test --workspace --locked --no-fail-fast` | `f73415d` | 65,257 | `sha256:05a1090e4ec78780d625c7e28958bed65ab0a4f83566debd9c64e5c583d2a326` | cargo's test output | 3 | satisfied: 582 passed and **18 failed**, 21 failures named, the eighteen and cargo's three error lines | yes |
| `cargo-test.log`: `cargo test --workspace --locked`, standard output and error | `9cd388a` | 66,535 | `sha256:b89ad11e5a5ce5975f89b70ca91d14d400fa8c490f971fa5f603f80470898807` | cargo's test output | 2 | satisfied: 24 excerpts of run identity, 25 omissions, **no failure named** | yes |
| `conformance-core.manifest.json`: the runner's `manifest.json` for the `core` suite | `9cd388a` | 61,621 | `sha256:06a0f54234b49df6c459cfec6217e8f1b50c6dd5e03cc4ede619ffbd2ed69a35` | JSON | 3 | satisfied: 130 pass and 5 unsupported, **no failure named** | yes |
| `cargo-test-build.jsonl`: `cargo test --workspace --locked --no-run --message-format json` | `9cd388a` | 196,225 | `sha256:ada6f236bbc49a2c2d2d7de32d5284e34ffca6615205f9a2baff8ac8f4945d6c` | — | — | **`insufficient_capacity`**: over 131,072 bytes, refused before it is read | no |
| the other six suites' manifests: `stream`, `composition`, `evidence`, `socket`, `context`, `knowledge` | `9cd388a` | 5,890 to 11,459 | in the manifest | JSON | 1 | satisfied: everything fits, carried whole | no |

An input the baseline carries whole, or refuses, makes no call in either mode, so its dry run is what a live run would produce. The live runs are therefore the three inputs that ask. In every dry run the ledger tiled the input, the checks named no problem, and the replay gate rebuilt identical sections with no ambiguous question. The reviewer scanned the three that ask and found no home, volume or machine path and no credential shape, and accepted `/var/tmp/cbr-j2` as a location that names nothing.

**Why `f73415d` as well.** The merged commit's suite is green, so neither of its inputs names a failure, and J2 asks which tests failed. At `f73415d`, m5a's tests-first commit and now in `main`'s history, J2's eighteen tests fail against the code before them.

**The runs, as the owner decided them: six, in two invocations, in this order.** Each input runs on **MiniMax-M2.7-highspeed** and **MiniMax-M3**, the two models M4 measured live. `MiniMax-M2.7` has never been called live, and nothing here needs it. Each invocation has its own manifest, pinned to the commit its input was produced at and kept outside this repository.

| Invocation | Manifest pinned to | Runs | `--run-ceiling` | `--out` |
|---|---|---|---:|---|
| **1, first** | `f73415d` | `cargo-test-f73415d.log` on both models | 976,320 | `/var/tmp/cbr-j2/o1` |
| **2** | `9cd388a` | `cargo-test.log` and `conformance-core.manifest.json`, each on both models | the lesser of 1,952,640 and 2,250,000 less what invocation 1 spent | `/var/tmp/cbr-j2/o2` |

- **Total J2 spend is held to 2,250,000** across both invocations. Invocation 2's ceiling is what holds it, computed before invocation 2 starts from what invocation 1's report says it spent, which is the sum of its stores' ledgers. **The hard cap is 5,000,000.**
- **The worst case**: 6 × 488,160 = 2,928,960, where 488,160 is `WORST_CASE_PROJECTION_TOKENS`, a whole projection with every part at every bound and repaired once, computed in `projection::tests` from real request bodies. By the dry run's parts it is less: 8 parts a model and 16 calls, each at most 40,680 tokens and repaired once, which is 16 × 122,040 = 1,952,640. Both are worst cases, and nothing has measured a projection's call. If invocation 1 spent its whole worst case of 976,320, invocation 2's ceiling would be 1,273,680.
- **In `j2_run.py` one number is both the stop and the cap.** As the stop, it is checked before each run: a run starts only if the ceiling, less what the runs before it spent, covers that run's worst case of 488,160. As the cap, it is passed to each launch as the ceiling less what earlier launches spent, and the provider's ledger enforces it.
- **The `--out` is short, and on the SSD.** Both directories are under `/var/tmp/cbr-j2`, the disk image the inputs were produced in, attached again so the runs' stores, which are the evidence, stay on the SSD. The harness puts each provider's socket at `<out>/work/<run id>/s/cbr.sock`, and on macOS a Unix socket path must be under 104 bytes; the first survey dry run stopped there with a longer one. These paths are about 50.

### J2 live: what it spent, 2026-09-25

Run from `main` at `041ad5f`, with its binaries built from a clean tree, the scoring rubric committed first (`84061eb`, 15:39:31Z, pushed 15:39:34Z), and each invocation's ceiling as decided above.

| Invocation | Started | Took | `--run-ceiling` | Spent | Runs |
|---|---|---:|---:|---:|---|
| 1 | 15:40:00Z | 26 s | 976,320 | **36,410** | `red-m27hs` 18,714, `red-m3` 17,696 |
| 2 | 15:40:44Z | 49 s | 1,952,640, the lesser of that and 2,250,000 − 36,410 | **83,657** | `log-m27hs` 19,280, `log-m3` 17,762, `core-m27hs` 23,931, `core-m3` 22,684 |

- **Total 120,067 tokens**: 5.3% of the 2,250,000 held, 4.1% of the 2,928,960 worst case, and 6.1% of the 1,952,640 the dry run's parts allowed. Sixteen calls, none repaired, every one admitted by the local bound alone. Each run's ledger equals the harness's report and the sum of its sealed records' usage.
- **The estimate was the worst case, and in tokens the worst case was far away.** Every part but each input's last was closed by its bound as designed — the red log's first at `PART_UNITS`, 64 units, and the others at 79.5 to 93% of `PART_BYTES` in raw bytes, and 96.7 to 99.6% as the request body carries them — but the largest request carried 9,808 input tokens and the costliest part 10,348 in all, against the 40,680 a part at every bound can cost: a part's bytes are escaped text, and the bound on tokens is the conservative byte bound.
- **The run failed its rubric**, and J2 runs again after m5a-3 ([JOURNEYS](../../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed)). That rerun's inputs, estimate and ceiling are written here before it runs.

### J2 live rerun after m5a-3: inputs, estimate and ceiling

**Written before the rerun, decided on 2026-09-26 under the owner's standing grant of 2026-09-25, and run the same day** ([what it spent](#j2-live-rerun-what-it-spent-2026-09-26)) — MiniMax as much as the work needs, with each run's estimate and ceiling written here before it and its spend after, inside CBR's envelope ([STACK §8.1](../readiness/STACK.md)). **Invocation 1's `--run-ceiling` is 492,204 and invocation 2's is 861,357**, the least the harness's stop admits (below); the rerun as a whole is held to **738,306** by the parts the survey found, and is expected to spend about 47,000. Both are far inside the 20,000,000 a five-hour window allows. **Rubric v1.1**, v1's mechanical changes for the `/2` projection, is committed and pushed in the change that runs the rerun, before its first call, and the rerun is scored under v1 as well.

**The inputs and the invocations are as before.** The same three inputs, byte-identical to the digests in the table above, each on `MiniMax-M2.7-highspeed` and `MiniMax-M3`:

- **invocation 1**, pinned to `f73415d`: the red log on both models;
- **invocation 2**, pinned to `9cd388a`: the green log and the core manifest, each on both models.

**The dry-run survey at m5a-3's head**, against the labelled fake: all nine inputs, and both live manifests. Every run exited 0 with no problem named, every assisted arm carried every baseline excerpt, and the one replay rebuilt identical sections. The masked sha256 is `score_j2.py`'s C0 rule: the section's content with the artifact id after `of evidence` replaced by `[artifact]`.

| Input | Parts asked | A call? | Baseline | Masked baseline sha256 |
|---|---:|---|---|---|
| `cargo-test-f73415d.log` (red) | **0** | **no**: the assisted request, given 4 parts, makes no call, and its section is the baseline's byte for byte | 16,350 bytes, 21 excerpts; `failing tests: 18; cargo errors: 3` | `e7d389553f098504be79f6580399d734a9d84a3963a7070f1ad90a15a6ff04f7` |
| `cargo-test.log` (green) | **0** | **no**, the same way | 8,504 bytes; run identity fills all 24 excerpts | `65666f5efc2c55dec9825f32ed1045473047765181dd8dd1e5e7471b36382a8f` |
| `conformance-core.manifest.json` | **3** | **yes**: 3 calls a model | 1,917 bytes, 2 excerpts; the fake's answer adds `e2, e3, e4` | `9a6f45883f67f971276d8b93e5d9a87357445c63117cbc64860b45c85e484b8a` |
| `cargo-test-build.jsonl` | — | no | `insufficient_capacity`, refused before it is read | none: no section |
| `stream`, `composition`, `evidence`, `socket`, `context`, `knowledge` manifests | 0 | no | everything fits, carried whole | `d48ca00c…cdd`, `e9023416…5ffb0`, `000fd9d9…b65f653`, `44d0b714…db190`, `ae5d24dc…54b8`, `b904920b…99d8` |

- **Red and green make no call, so each is equal to its baseline by construction.** The red log's floor leaves less room than the model's widest label needs: it carries all eighteen failures and cargo's three errors in 16,350 of 16,384 bytes. The green log's run identity takes every excerpt a projection holds. Neither arm spends a token, and neither can be worse or better than its baseline.
- **Only the core manifest asks: 3 parts a model, so 6 calls.** Its units that could be added include the five `unsupported` results, which the rule does not carry because they are not failures.
- **The worst case is 6 × 41,017 × 3 = 738,306 tokens**: six parts at every bound, each counted and repaired once. The ceiling is that worst case: nothing else can be spent, because no other run asks a part.
- **The expected spend is 46,615 to 48,637, about 47,000.** The core runs spent 23,931 and 22,684 at `/1`, 46,615 in all, with three parts each. At `/2` the core manifest is offered every unit of its three planned parts, 59, 58 and 18 (computed from the input at m5a-3's head), so its questions carry the candidates `/1`'s did. Each is at most 337 bytes longer: a part's worst case grew from 40,680 to 41,017 with the preamble and the instruction's new words, and the byte bound counts those as at most 337 tokens, 2,022 over six questions. Red and green, which spent 73,452 between them at `/1`, now spend nothing.
- **The harness's stop needs more than that as its `--run-ceiling`.** Before each run it requires a whole projection's worst case, 492,204, to be left, whatever the input's parts. So invocation 1 needs at least 492,204, and spends nothing. Invocation 2 needs at least 861,357: `core-m27hs`'s 369,153 worst case, then 492,204 left for `core-m3`. The provider's ledger still holds each launch to what the ceiling leaves, and the parts the survey found hold the whole rerun to 738,306. A stop computed from the input's own parts would remove the gap, and that is a change to the harness, not to this plan.
- **The hard cap stays 5,000,000.** The `--out` directories are short and under `/var/tmp/cbr-j2`, as before.

### J2 live rerun: what it spent, 2026-09-26

Run from `main` at `80cdfbe`, with rubric v1.1 (`13f5911`, `587ddf1`) and these ceilings (`8d08bef`) committed and pushed first, at 22:57:14Z. Times are UTC, on 2026-09-25; the date in the heading is the session's, IST.

| Invocation | Started | Took | `--run-ceiling` | Spent | Runs |
|---|---|---:|---:|---:|---|
| 1 | 22:57:23Z | 2 s | 492,204 | **0** | `red-m27hs` 0, `red-m3` 0 |
| 2 | 22:57:25Z | 18 s | 861,357 | **46,452** | `log-m27hs` 0, `log-m3` 0, `core-m27hs` 23,559, `core-m3` 22,893 |

- **46,452 tokens**, 6 calls, no repair: 6.3% of the 738,306 the parts allowed, and 163 below the 46,615 to 48,637 estimated. **J2's live runs have spent 166,519 in all.**
- **The rerun passed** its rubric, and every run was equal to the rule ([JOURNEYS](../../verification/JOURNEYS.md#j2-live-rerun-2026-09-26-passed--and-the-model-added-nothing)).

## What this document does not settle

- **No bound is sized.** Each is a pull-request decision, reviewed at its head.
- **What starts a gap loop on a question with no unmet item** is m5e's to settle (§6), before m5h is written against it.
- ~~Whether `orient_repository` and `record_rejected_approach` land in M5.~~ **Settled by the owner on 2026-09-25: both land**, as m5i and m5j.
- **Whether the ten execution fixtures can run** against the reference executor is m5g's check, not an assumption.
- **Each live run's cap** is set when its estimate exists: by the owner until 2026-09-25, and since then by the session that runs it, under the owner's standing grant, before the run.
- **Nothing in this document is evidence of anything.** The evidence is where it is recorded: m5a's code and its mutants in [STATE](../STATE.md), and J2's live runs in JOURNEYS: [the first](../../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed) and [the rerun](../../verification/JOURNEYS.md#j2-live-rerun-2026-09-26-passed--and-the-model-added-nothing).
