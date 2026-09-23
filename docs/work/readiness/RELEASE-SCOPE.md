# Proposed standalone CBR v0.1 release scope and milestone sequence

> **Status: accepted 2026-09-16** by [ADR 001](../../decisions/001-standalone-v0.1-scope-and-stack.md), with the provider/spend grant and the pilot repository still unfilled there. Scope reductions from here are explicit decisions, not drift. The owner selects the scope; [STANDALONE-RELEASES](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) requires that any reduction of agreed scope be an explicit decision, so this file states the reduction candidates openly rather than quietly shrinking later. Discussion and open questions in [TALK](TALK.md). Protocol pin and contract mapping in [PROTOCOL-PIN](PROTOCOL-PIN.md).

## 1. What "complete" means here

The [CBR gate](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) has six bullets. Restated as things that can be shown or refuted:

1. **Installable and independent.** A user installs CBR, points it at a repository and a model provider, and uses it with no PIO and no Combraton process.
2. **The record model works.** Immutable evidence with provenance; validated typed revisions; applicability; conflicting and historical claims kept, not collapsed; exact cited packets; scoped retention; export and restore.
3. **The memory engine is substantive.** Model-assisted derivation, bounded maintenance and request-time investigation are real capabilities. A lexical store with a vector index bolted on does not satisfy this.
4. **The runtime is bounded.** Per-call and aggregate limits on context, tools and model work; durable job progress; cancellation; restart; explicit gaps; recorded supported model capacities; no mandatory million-token model.
5. **Adversarial correctness.** False support, stale code or environment, changed authority, correction races, branch leakage, oversized results, missing required items — each with a negative control that fails when the guard is removed.
6. **Downstream evidence.** Real task comparison against a strong native-context baseline, counting cold initialization, maintenance, retrieval and investigation cost, publishing uncertainty and unsuccessful cases.

Bullets 1–5 are within this session's control. **Bullet 6 is not**, without a granted model provider and a permitted spend (§5).

## 2. In scope

**Protocol surface.** `core/1` (with `core.events`, `core.grants`, `core.capabilities`), `evidence/1` (with `evidence.manifests`, `evidence.retention_control`, `evidence.work_binding`), `knowledge/1`, `context/1` (with `context.advisory`, `context.required_before_start`, `context.required_before_transition`, `context.shared_jobs`, `context.updates`, `context.expand`, `context.claims`), over the `stream/1` binding on both stdio and Unix domain socket, macOS and Linux. `core-test/1` behind a test-only configuration. `execution/1` and `verification/1` as a **client** only.

**Store.** One SQLite database plus a content-addressed object store, per [STORAGE](https://github.com/Combraton/combraton/blob/main/docs/architecture/STORAGE.md). Durable commit ordering, the crash matrix, the event stream with epochs and explicit retention gaps, export and restore.

**Retrieval.** Exact identity, lexical search, code anchors resolved to spans at a named tree, a temporal index, and an explicit dependency index with demand-driven bounded invalidation and early cutoff under a declared comparator.

**Model runtime.** A CBR-owned provider abstraction; pre-send budget admission over the fully serialized request; a small bounded tool surface; a bounded tool loop with durable checkpoints, cancellation and restart-without-duplicate-commit; derivation records sealed as evidence; honest degradation to deterministic retrieval when no qualified model is configured, with intelligent synthesis then explicitly unavailable.

**The two loops.** Request-time gap investigation, and bounded coalesced maintenance with named triggers, scope, budget and finish conditions. Foreground context requests take priority over optional maintenance. **Background spend defaults to zero** until explicitly enabled, and every model call debits a persisted budget envelope *before* the call rather than being reconciled after. The maintenance loop stays in v0.1 and is not the first candidate for reduction (ADR 001, question 1).

**Client.** A thin `cbr` CLI speaking the public protocol over the socket: ingest, propose, decide, evaluate, request context, inspect and fetch a packet, expand a citation, inspect history, export. It doubles as the headless evaluation client for [benchmarks](https://github.com/Combraton/benchmarks) arm C. It uses no privileged internal shortcut.

It additionally carries a **`cbr search` diagnostic**, which is **not a protocol operation** and is labelled as such wherever it appears. Protocol 0.1 has no public search over knowledge or evidence content, and this does not add one: it reads CBR's local indexes for a human debugging retrieval. The evaluation client never uses it as a scored path, because a scored operation must be one a user can reach through the public interface.

**Derivation families.** A deliberately small, named set rather than an open-ended "the model can do anything" surface. Proposed for v0.1, matching the table in [MEMORY-ENGINE §4](../../spec/MEMORY-ENGINE.md):

| Family | Input | Output | Why this one |
|---|---|---|---|
| `distill_investigation` | A bounded set of evidence artifacts from one exploration | A cited flow artifact plus `interpretive` claims and an unresolved-question list | The central example in MEMORY-ENGINE §3; the thing long refactors most need |
| `project_large_result` | A large log, test report or JSON artifact | Named failures, decisive excerpts, run identity, a full-artifact reference, and explicit omissions | Journey 2 depends on it; also the main context-flooding defence |
| `orient_repository` | A registered repository at a tree | A bounded inventory map with declared gaps | Brownfield entry point; no project-wide startup barrier |
| `record_rejected_approach` | A failed attempt and its evidence | A scoped rejection artifact and claim | Named explicitly in MEMORY-ENGINE §4; cheap and high value |
| `answer_gap` | An unmet item during preparation, plus a read scope and budget | Either new supported claims or an explicit unresolved gap | This *is* request-time investigation |

**An M5 obligation for every family above: CBR's own producer never names a derived artifact as an ancestry root.** It was M4's until the owner's decision of 2026-09-23 moved it to M5, where it is carried with **the first family whose output CBR's own producer cites as claim support**; CBR's producer cites no output of M4's that way. When a family's output is cited as claim support, each support entry's `ancestry.roots` are the **captured evidence the derivation read**, never the derived artifact and never another derivation. The M2 control, `two_derivations_over_one_captured_log_are_one_lineage`, shows what the provider reports when a producer gets this wrong: `multiple_lineages`, false corroboration. **M5 must show CBR's producer cannot get it wrong**, in the pull request that adds that family. The acceptance is a test that drives a family through CBR's own producer, with a labelled fake model for determinism, over one captured artifact twice, and requires `single_lineage`. A mutant makes the producer list the derived artifact as its own root, and the test must catch it.

**Documentation.** Install, upgrade, retention, export and restore, supported model capacities, and a limitations page that names every deferral below.

## 3. Out of scope for v0.1, and why

Each of these is deferred by an accepted document, not by convenience.

| Deferred | Authority for deferring |
|---|---|
| Vector and embedding retrieval | [INTERNALS §4](../../spec/INTERNALS.md): add it "only when the evaluation shows missed retrieval that simpler methods cannot address" |
| Graph-first retrieval, trained memory models, automatic entity fusion, generational activation, personalised ranking | [INTERNALS §7](../../spec/INTERNALS.md) defer list |
| Generated-program and programmatic workers | [BASELINE §3](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md): "After core memory proof", with sandbox, resource and provenance fixtures plus measured benefit first |
| `coordination` and `remote-trust` profiles; signed receipts; offline receipt verification | Declared `not_in_release` by Protocol 0.1 |
| Windows | Protocol 0.1 supports macOS and Linux only (U4) |
| Temporal reconstruction views ("what was known at position P") | Protocol M5-Q8 defers them; `knowledge.claim.history` keeps what such a view would need |
| Serving `execution/1` or `verification/1` | CBR is a client of both, never a provider |
| A TUI | PIO owns the terminal interface; CBR ships a CLI |
| A model matrix beyond what is actually tested | [MODEL-RUNTIME §5](../../spec/MODEL-RUNTIME.md): no supported model is marketed as capable of every task merely because it fits the schema |

## 4. Milestone sequence

One rule shapes the order: **every milestone ends with something a person can watch happen, not only a green suite.** The risk this guards against is real — the protocol layer has crisp, objective acceptance and the memory engine does not, so effort drifts to the part that scores itself.

| # | Milestone | Fixture acceptance | Observable acceptance |
|---|---|---|---|
| M1 | **Walking skeleton over the real command path.** Store, durable commit ordering, event stream with epochs and gaps, grants, capabilities, the Core command path, the stream binding on stdio and socket, `core-test/1`, Evidence upload/seal/fetch/query. | `core` 135, `stream` 24, `socket` 13, `evidence` 16 against a CBR participant descriptor | `cbr ingest` a real file and `cbr fetch` it back over the public socket, byte-identical, surviving a service restart. Storage crash matrix fault-injected. |
| M2 | **Knowledge.** Claim revisions and digests, support with declared ancestry and support classes, authority bindings, reliance decisions, conflicts and drift, applicability evaluation, history. Source identity (PROTOCOL-PIN §5). | `knowledge` 10 | Record a human decision, propose a model-derived claim, show the model cannot accept its own claim, show a drift record keeping both values, and **J9** (an authority transfer invalidates the stale decision path without editing history). Source-identity property tests with negative controls, including the **derived-artifact ancestry control**: two derived artifacts sharing one captured root must report `single_lineage`, never `multiple_lineages`. |
| M3 | **Retrieval and the deterministic packet compiler.** Lexical, identity, code-anchor, temporal and dependency indexes. Request lifecycle, items and checks, the three obligations, budgets kept separate, packet sealing, omissions, coverage frontiers, updates, corrections, expand. | `context` 11, including `context.claims`; the ≈3 composition fixtures needing only context and evidence roles | **Journey 1 with no model at all**, plus **J8** (a required item unmet at deadline stays unmet). A real repository in, a real cited packet out. Also the **journey-6 pilot**, labelled a steer and not evidence, against **Knowscroll-v2** with decisions `D-001`–`D-022` as the decision input and one open backlog task as the refactor (§5). If retrieval alone cannot produce a useful packet here, that is a finding worth having before spending on a model runtime. |
| M4 | **Bounded model runtime.** Provider abstraction, pre-send admission over the fully serialized request, cancellation, restart, derivation records as sealed evidence, honest no-model degradation. The bounded tool surface, the loop and checkpoints were here until the owner's decision of 2026-09-23 moved them to M5. | None of its own: [MODEL-RUNTIME §6](../../spec/MODEL-RUNTIME.md)'s runtime-selection tests moved to M5 with the tool runtime, by the owner's decision of 2026-09-23. What M4's direct-call runtime already exercises of them, and what it does not, is [item by item in the M4 close-out](../m4/CLOSEOUT.md#the-runtime-selection-tests-of-model-runtime-section-6). | One derivation family against a **real** model, with the transcript sealed and the cost recorded — **met by model-assisted discovery**, whose live runs 1 and 3 are sealed, costed transcripts ([JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g), [M4 close-out](../m4/CLOSEOUT.md)). Journey 2 was here until the **owner's decision of 2026-09-23** moved it to M5. The provider is granted — **MiniMax** (§5, [STACK §8.1](STACK.md)). Fake-model fault injection remains part of M4 and remains **labelled as such**; a deterministic fake is the right tool for fault injection and the wrong tool for acceptance, and that does not change now that a live run is possible. |
| M5 | **The two loops.** Request-time gap investigation; bounded coalesced maintenance with triggers, priority, budget and finish conditions; subscriber semantics. **Moved from M4 by the owner's decision of 2026-09-23:** the bounded tool surface, the loop and its checkpoints; model-assisted artifact creation, a derivation that produces a claim or a projection; and the ancestry-root obligation of §2. | The remaining composition fixtures, with an execution peer (§6). **And, moved from M4 with the tool runtime**, [MODEL-RUNTIME §6](../../spec/MODEL-RUNTIME.md)'s runtime-selection tests: pre-call context control, aggregate tool limits, cancellation, error and usage fidelity, fallback policy, resource-discovery isolation, exact output capture, restart without duplicate commits | **Journey 2 first** — moved from M4 by the owner's decision of 2026-09-23, with its negative controls unchanged: silent truncation, and an input larger than the summarizer's capacity giving a typed insufficient-capacity result. Then journeys 3, 4 and 5, each with its negative control. |
| M6 | **Adversarial correctness, retention and packaging.** Plus **CBR's own mutant set**: one mutant per named negative control in [JOURNEYS](../../verification/JOURNEYS.md), each a build with one guard removed, reported `WRONG-REASON` when it fails at the wrong step or for the wrong reason (ADR 001, question 7). | The full counterexample table from [PREPARATION §9](../../spec/PREPARATION-AND-DELIVERY.md); every mutant killed by its declared control at its declared step | Retention, export and restore round-trip; install and upgrade on macOS and Linux; the limitations page is true. Journeys J9 and J10. |
| M7 | **Evaluation.** Pilot, then agreed thresholds, then the confirmatory run. | — | Journey 6, and journey 7 once PIO is ready. Unblocked by the provider grant; journey 7 still waits on PIO (§6). |

M1 through M3 need no model provider and no PIO. That was deliberate as insurance while the provider question was open, and it is worth keeping now that it is closed: M3's acceptance asks whether a **deterministic** packet is useful, and that question is only honestly answerable with no model in the room to flatter the answer.

## 5. The provider grant, and what is still blocked

**Resolved 2026-09-16.** The provider and spend question that blocked the gate's sixth bullet, journey 6 and parts of journey 4 is answered (ADR 001, question 3). Terms in full: [STACK §8.1](STACK.md).

- **Provider:** MiniMax, on the owner's subscription quota — 100M tokens per 5-hour window, ~1.7B per month, **shared** across text, image and speech and with the owner's other tools.
- **Wires:** OpenAI-compatible `https://api.minimax.io/v1` as primary, Anthropic-compatible `https://api.minimax.io/anthropic` as the second dialect. Both dialects are exercised against one provider at no extra cost — which tests CBR's serializer and parser, and does **not** test Anthropic-specific behaviour.
- **Models:** `MiniMax-M2.7-highspeed` for extraction and large-result projection, `MiniMax-M2.7` for ordinary derivation, `MiniMax-M3` for synthesis, request-time investigation and image input.
- **Envelope:** 20M tokens per 5-hour window and 200M per month for CBR, confirmed by the owner ([STACK §8.1](STACK.md) is the single source), debited before every call. **Background spend is zero until explicitly enabled.** Exhaustion is a typed `budget_exhausted`, never a retry loop.
- **Credential:** macOS Keychain service `minimax_api_key`, read at process start only, never logged, never in the repository. No other credential is read.
- **Pilot repository** for the M3 journey-6 steer: **Knowscroll-v2** ([`Legend101Zz/Knowscroll-v2`](https://github.com/Legend101Zz/Knowscroll-v2), public), decisions `D-001`–`D-022` as the decision input, one open backlog task as the refactor. **Labelled pilot, never gate evidence.** Evidence capture redacts credentials **at the recording boundary** — that repository has already once seen a provider response carry a live third-party credential, so this is a known failure mode there, not a precaution.

### What is still blocked, and why it is a different kind of blocker

The remaining blockers are **availability**, not permission, and no amount of spending removes them:

- **Journey 7** needs a PIO standalone service. PIO is being built in parallel and has no release. CBR must not depend on its unreleased work.
- **Ten of the fourteen `composition` fixtures** name an executor role (§6). The three that need no executor are named there and are in M3's acceptance.
- **M7's thresholds** must be derived from pilot variance *before* the confirmatory run, not chosen after seeing it. Having a provider makes the pilot possible; it does not supply the thresholds, and a threshold picked after the result is not a threshold.

### What the grant does not change

- A replayed transcript is still not a live run, and a labelled fake model is still not acceptance evidence. [MODEL-RUNTIME §7](../../spec/MODEL-RUNTIME.md) and the shared verification model are unchanged by the existence of a budget.
- The status line is now *"model-assisted derivation implemented; live-model evidence recorded"*. It said *"…live-model evidence pending M4"* until M4 produced sealed transcripts with recorded costs — model-assisted discovery's live runs 1 and 3 ([JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)) — and neither a replayed transcript nor a labelled fake would have changed it.

## 6. Dependency on an execution peer

Of the 14 `composition` fixtures at the pin, exactly three need no executor role:

| Fixture | Participant roles | Notes |
|---|---|---|
| `composition.direct-fetch-needs-one-grant-per-audience` | context, evidence | The one-grant-per-audience rule of CONTEXT §7 |
| `composition.packets-are-sealed-at-a-separate-evidence-provider` | context, evidence | CBR as a producer at a peer evidence provider, with deterministic command IDs so a retried publication replays |
| `composition.thirdparty-kernel-enforces-required-claim-boundary` | context, knowledge, evidence | Declares `context/1`, `knowledge/1`, `evidence/1` — all roles CBR serves. It is driven by the protocol's own client-only `minimal-executor` kernel from `conformance/thirdparty/`, which is **not** an `execution/1` provider and **not** a CBR dependency. |

`composition.thirdparty-publisher-feeds-reference-verification` is sometimes mistaken for a fourth: it needs only an evidence participant, but it declares `verification/1`, which CBR does not serve.

The remaining ten name an executor. Until PIO is ready the honest options are to run them against the Protocol repository's own reference executor — a public protocol peer, so legitimate, but labelled a reference executor and never reported as real-adapter evidence — or to leave them unrun and say so. Either way, journey 7 waits for PIO.

**Test controls are ours to implement, not to borrow.** These fixtures declare `requires_controls`; the third needs `clock.file`, `context.script`, `knowledge.store` and `evidence.store`. A participant that does not declare a required control has that fixture reported `unsupported` and listed under `coverage_limits` — never a pass. So CBR implements the documented control vocabulary itself, gated behind the conformance launch configuration per [CORE §13.1](https://github.com/Combraton/protocol/blob/main/docs/spec/profiles/CORE.md) ("test control is environment-only"). It does not reuse the reference provider's scripted stores, which are that implementation's test scaffolding and explicitly not a library to depend on.

## 7. What would make me want to change this scope

Stated in advance so a later reduction is a decision and not a drift:

- If M3 produces a genuinely useful cited packet with no model, that is evidence the deterministic core carries more of the value than assumed, and the model runtime should be narrowed rather than broadened.
- If the pre-send budget admission in M4 turns out to be unachievable to acceptable accuracy for the granted provider, the "never silently omit mandatory constraints" guarantee weakens to "refuse rather than guess", and that weakening must be written down, not absorbed.
- If the source-identity property tests show the dirty-snapshot digest churns so fast that dirty-basis claims are worthless in practice, the dirty-basis story needs rethinking before M3, not after M6.
