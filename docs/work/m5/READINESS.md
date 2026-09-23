# M5 readiness — the two loops, the tool runtime, and the first derivation families

**Written before any M5 code, and reviewed before any.** M4 showed why the readiness document comes first: the decisions that matter are made in it, when they are cheap to change. M5 is where CBR stops asking a model fixed questions and starts letting a model **act** — read, search, propose — inside a loop CBR runs. That is a larger step than anything M4 took, and every bound it needs is written down here before there is code to bend it.

Tracking: [issue #34](https://github.com/Combraton/cbr/issues/34). Scope: [RELEASE-SCOPE §4](../readiness/RELEASE-SCOPE.md)'s M5 row, including what the owner's decisions of 2026-09-23 moved there from M4. The runtime it builds on is M4's ([CLOSEOUT](../m4/CLOSEOUT.md)). The specifications it realises are [MEMORY-ENGINE](../../spec/MEMORY-ENGINE.md), [MODEL-RUNTIME](../../spec/MODEL-RUNTIME.md) and [PREPARATION-AND-DELIVERY](../../spec/PREPARATION-AND-DELIVERY.md).

## 1. Scope, and the promise

| M5 delivers | M5 does not |
|---|---|
| **Journey 2 first**: large search results, test logs and JSON processed under bounded model context, with what was omitted kept explicit (§3) | Replace M4's envelope. Admission, the ledger, the ceilings, the credential rule and the recording boundary are M4's and stay exactly as they are. |
| **The loop, the tool surface and checkpoints**: a bounded, CBR-owned investigation loop over a small read-only tool set, with durable checkpoints and code-enforced termination (§4) | Give a model shell, network, repository writes or instruction discovery. None is a memory tool ([MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md)). |
| **Model-assisted artifact creation**: derivation families that produce claims and projections, sealed as evidence, with the ancestry-root obligation carried by the first family whose output CBR's own producer cites as claim support (§5) | Let a model decide anything. A model-proposed claim is never `binding`; that is an authority's act. |
| **The two loops**: request-time gap investigation, and bounded, coalesced maintenance with named triggers, priority, budget and finish conditions (§6) | Spend in the background by default. Background spend stays **zero** until the owner enables it ([ADR 001](../../decisions/001-standalone-v0.1-scope-and-stack.md), question 1). |
| **J3, J4 and J5**, each with its negative control (§7) | Promise usefulness. That is J6, at M7. |

**What stays deterministic, and must be shown to.** Everything M3 and M4 made deterministic stays so: the compiler's order, sealing, citations to exact bytes at a named tree, coverage, the drop order, and replay from retained records. A loop may change *what is investigated*; it may not change *how what it found is recorded*, and a request that authorises no investigation still gets exactly the compiler M3 shipped. The golden digests stay the guard.

## 2. Two limits carried in from M4, stated generally

Both come from live run 3 ([JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)), and both are properties of the design, not of a question.

**A fact missing from the candidate set cannot be chosen, so discovery's ranking is the ceiling.** M4's model chooses among ids CBR offered. However good the choice, it cannot keep a fact that was never ranked into the set — which is exactly what the reviewer found in both brian2 flows of run 3. **This is the claim M5's loop exists to test**: a model that can search and read, inside the view, can reach what a ranking did not offer. It is also the claim most easily reported wrongly, so the live plan (§10) runs the same sealed questions against the loop with M4's discovery as the baseline, and the comparison says whether the loop reached a fact the ranking missed — not whether the packet looks better.

**Decision memory with a model in the loop has not been run on a real repository.** Both live Knowscroll runs had zero claims in the store; the harness registers a repository and asks, and ingests nothing. So nothing yet says how a model behaves when binding decisions compete with source spans for its attention. §10 proposes that the M5 live run ingest Knowscroll's decisions first, as the m3d pilot did; that is the owner's to decide.

## 3. Journey 2 first

**What it is** ([JOURNEYS](../../verification/JOURNEYS.md#3-journey-matrix)): large search results, test logs and JSON are processed under bounded model context, and omitted material stays explicit. It is the `project_large_result` family of [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md): input a large log, test report or JSON artifact; output named failures, decisive excerpts, run identity, a full-artifact reference and explicit omissions.

**Its two negative controls, unchanged:**

1. **Silent truncation.** Remove the omission-marker path, and the journey must fail because omitted content is no longer declared.
2. **Oversize input.** Feed an input larger than the summariser's own capacity, and require a typed insufficient-capacity result — never a silent partial summary.

**How it is built, as rules before code:**

- **The whole payload is sealed first**, as evidence with its digest and coverage; the projection is derived from it and cites it. A projection never replaces the artifact, and a reader can always fetch the full bytes ([MEMORY-ENGINE §5](../../spec/MEMORY-ENGINE.md)).
- **Deterministic parsing where the format allows**: a test runner's report, a JSON path, a log's line structure. The model is used for interpretation and selection, not for reading a structure a parser can read.
- **Values and units are never changed** in a projection ([MEMORY-ENGINE §4](../../spec/MEMORY-ENGINE.md)). A projected number is the source's bytes at a cited range, not the model's restatement of it. This is not hypothetical here: brian2's pilot question is about units being silently dropped.
- **A summariser is never given more than its own capacity** ([MODEL-RUNTIME §4](../../spec/MODEL-RUNTIME.md)). Input that does not fit is partitioned, each part a bounded call with its own record, or the result is the typed insufficient-capacity outcome of control 2. Partitioning keeps cross-part references as unresolved items rather than losing them.
- **Every omission is a typed record** with its reason and its extent, carried into the packet the way M3's omissions are.

**Open for the owner:** which real inputs J2's live acceptance uses. The candidates that keep the provider-side retention decision of [M4 READINESS §7](../m4/READINESS.md#7-what-may-be-sent) intact are CBR's own public CI logs and test output, which are public text.

## 4. The loop, the tool surface and checkpoints

**CBR owns the loop.** [STACK §8](../readiness/STACK.md) decided it: the design of `rig-agent`'s `AgentRun` (`next_step`, `model_response`, `tool_results`) is borrowed and the dependency is not, because CBR must own the checkpoint format under either choice. M4's runtime is its single step: every turn is an M4 call, admitted, charged, recorded and sealed exactly as today.

### The tool surface

From [MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md), and no more: **search** permitted evidence, **read** a bounded span, **inspect** artifact revisions, **compare** scoped code snapshots, **list** dependencies, **propose** a memory patch, and **produce** a packet candidate.

- **Every tool call is authorised again in CBR**, against the view and readable claims resolved at the command — the rule M3 learned when discovery ran on the provider's own authority and leaked every claim. A tool is a door, and every door gets the gate.
- **Tool payloads are untrusted data.** A span that says "ignore your instructions" is repository text. Negative control 5 extends to every tool: a planted file read through `read` or found through `search` changes nothing outside the tool's own bounded result.
- **No instruction discovery.** A repository's `AGENTS.md` or `CLAUDE.md` is text to read, never instructions to follow.
- **`tool_choice: "required"` is silently ignored by the provider** ([M4 READINESS §4](../m4/READINESS.md#4-provider-facts-that-are-design-inputs-not-discoveries)). A text answer where a tool call was demanded is an ordinary outcome of the loop, handled like M4's prose answers: bounded repair, charged to the same ledger.

### Termination and checkpoints

- **Termination is code's, never the model's.** Step, call, byte and token limits and a declared finish condition end a job ([MODEL-RUNTIME §4](../../spec/MODEL-RUNTIME.md)). "The model says it is done" is one input to the finish condition, not the condition.
- **A checkpoint is a durable record** holding the fields [MODEL-RUNTIME §1](../../spec/MODEL-RUNTIME.md) names — goal, pinned basis, constraints, artifact ids, inspected evidence ids and ranges, findings with support, competing hypotheses, unresolved dependencies, pending work, consumed budget. The mechanically known fields come from the runtime, not from a model's recollection.
- **A resumed job rebuilds its working context from the checkpoint and its sources**, never from a transcript. Hidden reasoning is not persisted and nothing depends on it.
- **A finding is committed through the validated write path**, with the same revision checks every other write has, so a delayed worker cannot overwrite a later correction (§7, J3).

### MODEL-RUNTIME §6's eight runtime-selection tests

The owner's decision of 2026-09-23 moved these to M5 with the tool runtime. [M4's close-out](../m4/CLOSEOUT.md#the-runtime-selection-tests-of-model-runtime-section-6) records what the direct-call runtime already exercises; this is what is new for tools.

| Item | Already exercised by M4's direct calls | New for the loop and its tools |
|---|---|---|
| Pre-call context control | Yes — admission counts the whole body | Tool schemas and tool results are in the body; admission runs again before every turn, after every tool batch |
| Aggregate tool limits | No — M4 has no tools | **Entirely new**: a per-batch response budget, so ten bounded results cannot overflow the next call; overflow spills to storage and returns handles |
| Cancellation | Yes | Cancelling mid-loop leaves a checkpoint and no partial commit |
| Error and usage fidelity | Yes, and since m4h a record carries every attempt | Usage per turn, summed per job against the ledger; a tool's error is a typed outcome the loop reads, never an exception it swallows |
| Model fallback policy | Yes — no fallback | A failed turn's policy: end with partial coverage and a named gap, or one bounded retry; never a fallback to another model or to BM25's first |
| Resource-discovery isolation | Not applicable — M4's models read nothing | **Entirely new**: every read is inside the view and every search is CBR's own index; no instruction files, no paths outside a grant, and the planted-file control against every tool |
| Exact output capture | Yes, through the recording boundary | Every tool call and tool result is recorded; every turn is a sealed derivation |
| Restart without duplicate commits | Yes, for the ledger and the seal | **J4**: a kill between a model response and its commit leaves exactly one committed revision after restart |

## 5. Model-assisted artifact creation, and the ancestry obligation

**The families** are [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md)'s: `distill_investigation`, `project_large_result`, `orient_repository`, `record_rejected_approach` and `answer_gap`. Each is a named job with a fixed input, a fixed output shape and its own finish condition — not "the model can do anything".

- **What a family produces is sealed as evidence** under a CBR-specific source kind reserved for derivation output ([PROTOCOL-PIN](../readiness/PROTOCOL-PIN.md)), with CBR's own principal as producer and a coverage describing what the derivation inspected.
- **A claim it proposes is proposed**, by CBR's principal, and is never `binding`. The m2 rule that a model cannot accept its own claim holds unchanged.
- **Prose a model writes enters a packet only as `inferred`**, citing what it was derived from ([M4 READINESS §5](../m4/READINESS.md#5-repository-text-is-untrusted-input)).

**The ancestry-root obligation**, moved from M4 by the owner's decision of 2026-09-23: CBR's own producer never names a derived artifact as an ancestry root. When a family's output is cited as claim support, each support entry's `ancestry.roots` are the captured evidence the derivation read. **It lands in the same pull request as the first family whose output CBR's own producer cites as claim support**, with its test and its mutant:

- **The test** drives that family through CBR's own producer, with a labelled fake model for determinism, over one captured artifact twice, and requires `single_lineage` ([PROTOCOL-PIN](../readiness/PROTOCOL-PIN.md), the negative control for the ancestry rule).
- **The mutant** makes the producer list the derived artifact as its own root. The test must kill it by reading `multiple_lineages`.

`project_large_result` produces a projection, not claim support, so J2 alone does not carry the obligation. **Open for the owner:** which family comes second — `record_rejected_approach` is the smallest that proposes a claim, and `distill_investigation` the one long refactors most need.

## 6. The two loops

**Request-time gap investigation** is `answer_gap`: an unmet item during preparation, a read scope and a budget, and either new supported claims or an explicit unresolved gap. It is the loop of §4 run for one item, under the request's own deadline and investigation limit — and it is where limit 1 of §2 is tested, because a loop that can search and read is the first thing that can reach past the ranked set.

**Maintenance** is bounded and coalesced ([MEMORY-ENGINE §6](../../spec/MEMORY-ENGINE.md)). Every job has a basis, a question, a permission scope, a priority, a budget, a progress record and a finish condition.

- **Named triggers**: explicit requests, completed investigations, failed checks, handoffs, corrections, and gaps found while preparing a packet. Related events are batched; nothing launches a model per line of a log.
- **Foreground first**: a context request takes priority over optional maintenance, under the bounded scheduling M4's work pool already provides.
- **Deduplicated** by source revision, job purpose and configuration, so a newer event makes an older result historical rather than current.
- **Background spend is zero until enabled.** A maintenance job that would spend with background spend off is not started, and says so.

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

- **The first M5 composition step checks exactly that, docs only.** Until it passes, all eleven stay **blocked**.
- **Journey 7 waits for PIO** whatever that check finds.

## 8. Every new bound has its arithmetic computed by a test, and a fixture that reaches it

M4's lesson, twice over: a bound nothing presses against is not tested (the pool's unsettled slot at m4c, the union's cap at m4e), and a published figure a test does not compute drifts (m4f to m4g). So, for every bound M5 adds:

- **It is a named constant**, and the worst case it permits is **computed by a test** from the real request bodies, against the per-request, per-job and run ceilings it has to fit under — as `discovery::tests` computes M4's flow.
- **A fixture reaches it.** A fixture presses each bound — an investigation that exhausts its step limit, a tool batch that overflows its aggregate budget, a J2 input larger than the summariser's capacity — and the assertion is on what went out or what was refused, not on a count someone typed.
- **A number that lives in two places has a test across the boundary**, as the harness's worst case does since m4g.

The bounds already known to be needed, before any is sized: steps and calls per job; bytes per tool result; bytes per tool batch; span-read bytes; search page rows; checkpoint size; the working-set allocation of [MODEL-RUNTIME §2](../../spec/MODEL-RUNTIME.md); J2's input capacity and its projection's size; maintenance's coalescing window and per-trigger budget. **None is sized here.** Each is sized in the pull request that introduces it, with its arithmetic test beside it.

## 9. The pull requests, each with its gate

| | Scope | Gate |
|---|---|---|
| **m5 readiness** | This document and the M5 tracking issue. Docs only. | The reviewer has seen it. **No M5 code before that.** |
| **m5a** | **The loop and its checkpoints**, over M4's single step, with no tools but a fixed read. Durable checkpoint records; code-enforced termination; resume from checkpoint. | **J4's control**: a kill between model response and commit leaves exactly one revision, and two with deduplication disabled. Termination by code under a scripted model that never says it is done. The loop's worst case computed against the per-job ceiling. Mutants on each. |
| **m5b** | **The tool surface** and its aggregate limits; authorisation again at every tool; negative control 5 across every tool. | §4's table: pre-call control, aggregate limits and resource-discovery isolation, each with a fixture that reaches its bound. A repository outside the view appears in no tool result and no request body, asserted over the bytes sent. |
| **m5c** | **J2**: `project_large_result`, with deterministic parsing where the format allows and the model for selection. | **J2's two negative controls**, end to end through the public client with a labelled fake. The projection's arithmetic as a test. |
| **m5d** | **The first claim-producing family**, with **the ancestry-root obligation** and its mutant. | `single_lineage` over one captured artifact twice; the mutant reads `multiple_lineages`. A proposed claim is never binding. |
| **m5e** | **Request-time gap investigation** (`answer_gap`), and **J3** and **J5**. | J3's and J5's negative controls. A request that authorises no investigation still produces the golden packet. |
| **m5f** | **Maintenance**: triggers, coalescing, priority, per-trigger budget, finish conditions, deduplication, and subscriber semantics. Background spend zero until enabled. | A trigger storm is one job; a foreground request pre-empts maintenance; with background spend off, no maintenance job spends. |
| **m5g** | **The composition feasibility check** of §7. Docs, plus a descriptor only if the runner allows one. | Either the ten execution fixtures run against the reference executor, labelled as such, or the record says why they cannot. |

**Every pull request after this one**: the test first and observed red, then the fix; mutants observed dying against the whole workspace; the gates of VERIFICATION; and no model called by anything in the suite.

## 10. Live calls: each planned, estimated and capped before it, and each on the owner's word at the time

M4's practice stands. No live call happens inside a pull request's work, and none happens without its own step.

- **An estimate computed before the call**, from the arithmetic tests of §8 rather than chosen by eye. M4's estimate was 1,500,000 against a measured 65,144 and 67,395. The next estimate starts from those measurements.
- **A hard cap**, enforced by the run ceiling the harness already passes as the cap less what earlier launches spent, and **a stop checked before each run** against that run's computed worst case.
- **The owner's word at the time**, carried in the reviewer's run instruction, after the reviewer has pinned the manifest and checked every origin public by the API.

**Proposed, for the owner:**

1. **J2 live**, once m5c merges, over CBR's own public CI output.
2. **The loop against the sealed pilot questions**, once m5e merges, with M4's discovery as the baseline. This is the run that tests limit 1 of §2. For Knowscroll, it is proposed to ingest its decisions first, which tests limit 2.

Each run's estimate and cap are written into this document before the run, and **the cap stays the owner's to set**. M4's was 5,000,000.

## What this document does not settle

- **No bound is sized** and no family beyond J2's is chosen. Both are pull-request decisions, reviewed at their heads.
- **Which family comes second, which inputs J2's live acceptance uses, and whether the loop's live run ingests Knowscroll's decisions** are the owner's.
- **Whether the ten execution fixtures can run** against the reference executor is m5g's check, not an assumption.
- **Nothing here is evidence of anything.** M5 has made no call and has no code.
