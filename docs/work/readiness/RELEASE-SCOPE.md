# Proposed standalone CBR v0.1 release scope and milestone sequence

> **Status: proposed, not accepted.** The owner selects the scope; [STANDALONE-RELEASES](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) requires that any reduction of agreed scope be an explicit decision, so this file states the reduction candidates openly rather than quietly shrinking later. Discussion and open questions in [TALK](TALK.md). Protocol pin and contract mapping in [PROTOCOL-PIN](PROTOCOL-PIN.md).

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

**The two loops.** Request-time gap investigation, and bounded coalesced maintenance with named triggers, scope, budget and finish conditions. Foreground context requests take priority over optional maintenance.

**Client.** A thin `cbr` CLI speaking the public protocol over the socket: ingest, propose, decide, evaluate, request context, inspect and fetch a packet, expand a citation, inspect history, export. It doubles as the headless evaluation client for [benchmarks](https://github.com/Combraton/benchmarks) arm C. It uses no privileged internal shortcut.

**Derivation families.** A deliberately small, named set rather than an open-ended "the model can do anything" surface. Proposed for v0.1, matching the table in [MEMORY-ENGINE §4](../../spec/MEMORY-ENGINE.md):

| Family | Input | Output | Why this one |
|---|---|---|---|
| `distill_investigation` | A bounded set of evidence artifacts from one exploration | A cited flow artifact plus `interpretive` claims and an unresolved-question list | The central example in MEMORY-ENGINE §3; the thing long refactors most need |
| `project_large_result` | A large log, test report or JSON artifact | Named failures, decisive excerpts, run identity, a full-artifact reference, and explicit omissions | Journey 2 depends on it; also the main context-flooding defence |
| `orient_repository` | A registered repository at a tree | A bounded inventory map with declared gaps | Brownfield entry point; no project-wide startup barrier |
| `record_rejected_approach` | A failed attempt and its evidence | A scoped rejection artifact and claim | Named explicitly in MEMORY-ENGINE §4; cheap and high value |
| `answer_gap` | An unmet item during preparation, plus a read scope and budget | Either new supported claims or an explicit unresolved gap | This *is* request-time investigation |

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
| M2 | **Knowledge.** Claim revisions and digests, support with declared ancestry and support classes, authority bindings, reliance decisions, conflicts and drift, applicability evaluation, history. Source identity (PROTOCOL-PIN §5). | `knowledge` 10 | Record a human decision, propose a model-derived claim, show the model cannot accept its own claim, show a drift record keeping both values. Source-identity property tests with negative controls. |
| M3 | **Retrieval and the deterministic packet compiler.** Lexical, identity, code-anchor, temporal and dependency indexes. Request lifecycle, items and checks, the three obligations, budgets kept separate, packet sealing, omissions, coverage frontiers, updates, corrections, expand. | `context` 11, including `context.claims`; the ≈3 composition fixtures needing only context and evidence roles | **Journey 1 with no model at all.** A real repository in, a real cited packet out. If retrieval alone cannot produce a useful packet here, that is a finding worth having before spending on a model runtime. |
| M4 | **Bounded model runtime.** Provider abstraction, pre-send admission over the fully serialized request, the bounded tool surface, the loop, checkpoints, cancellation, restart, derivation records as sealed evidence, honest no-model degradation. | The [MODEL-RUNTIME §6](../../spec/MODEL-RUNTIME.md) selection fixtures: pre-call context control, aggregate tool limits, cancellation, error and usage fidelity, fallback policy, resource-discovery isolation, exact output capture, restart without duplicate commits | Journey 2 end to end. One derivation family against a **real** model, with the transcript sealed and the cost recorded. Fake-model fault injection is labelled as such and never presented as a live run. |
| M5 | **The two loops.** Request-time gap investigation; bounded coalesced maintenance with triggers, priority, budget and finish conditions; subscriber semantics. | The remaining composition fixtures, with an execution peer (§6) | Journeys 3, 4 and 5, each with its negative control. |
| M6 | **Adversarial correctness, retention and packaging.** | The full counterexample table from [PREPARATION §9](../../spec/PREPARATION-AND-DELIVERY.md) | Retention, export and restore round-trip; install and upgrade on macOS and Linux; the limitations page is true. |
| M7 | **Evaluation.** Pilot, then agreed thresholds, then the confirmatory run. | — | Journey 6, and journey 7 once PIO is ready. **Blocked without §5.** |

M1 through M3 need no model provider and no PIO. That is deliberate: it is the largest block of work that can proceed while the provider and budget question is open.

## 5. The blocker, named precisely

The gate's sixth bullet, journeys 6 and parts of 4, and [benchmarks](https://github.com/Combraton/benchmarks) arms A, C and D all require a model provider and a permitted spend. **Neither is granted.** No provider credential has been used and none will be without an explicit grant naming the provider, the models, the spend ceiling and who pays.

Consequences, stated so they are not discovered late:

- **M1–M3 are unaffected** and are the bulk of the protocol-conformance work.
- **M4 can be built and fault-tested against a labelled deterministic fake model**, but it cannot be *accepted* on that evidence. [MODEL-RUNTIME §7](../../spec/MODEL-RUNTIME.md) and the shared verification model both refuse to let a replayed or simulated run stand in for a live one.
- **M7 cannot start.** A comparative claim without arms A and C is not a comparative claim.
- The honest interim status is "model-assisted derivation implemented, live-model evidence absent", not "memory quality unknown but probably fine".

## 6. Dependency on an execution peer

Of the 14 `composition` fixtures at the pin, roughly three need only the context and evidence roles CBR serves. The rest name an executor role. Until PIO is ready, the honest options are to run those against the Protocol repository's own reference executor — a public protocol peer, so legitimate, but it must be labelled a reference executor and never reported as real-adapter evidence — or to leave them unrun and say so. Either way, journey 7 waits for PIO.

## 7. What would make me want to change this scope

Stated in advance so a later reduction is a decision and not a drift:

- If M3 produces a genuinely useful cited packet with no model, that is evidence the deterministic core carries more of the value than assumed, and the model runtime should be narrowed rather than broadened.
- If the pre-send budget admission in M4 turns out to be unachievable to acceptable accuracy for the granted provider, the "never silently omit mandatory constraints" guarantee weakens to "refuse rather than guess", and that weakening must be written down, not absorbed.
- If the source-identity property tests show the dirty-snapshot digest churns so fast that dirty-basis claims are worthless in practice, the dirty-basis story needs rethinking before M3, not after M6.
