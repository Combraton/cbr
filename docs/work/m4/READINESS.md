# M4 readiness — the bounded model runtime

**Nothing in M4 has been built and no model has been called.** This document exists because M4 is the first milestone that spends the owner's quota and sends repository text to a third party, and both of those are decisions rather than implementation details. It is docs-only and it is reviewed before any transport code, any credential read, or any call.

Scope and sequence: [ADR 001](../../decisions/001-standalone-v0.1-scope-and-stack.md), [RELEASE-SCOPE §4](../readiness/RELEASE-SCOPE.md). The provider terms are the owner's and are already recorded in [RELEASE-SCOPE §5](../readiness/RELEASE-SCOPE.md) and [STACK §8.1](../readiness/STACK.md); this document restates them **as constraints with the place each one is enforced**, and adds nothing to them.

## 1. Scope, and the promise

M4 delivers **model-assisted selection and artifact creation inside a bounded runtime**. It does not make CBR a model wrapper: the parts of M3 that are deterministic stay deterministic, and a packet built with a model is still sealed, cited and reproducible.

| M4 delivers | M4 does not |
|---|---|
| A provider abstraction with both granted dialects, an admission check before every call, and a typed refusal when the envelope would be exceeded | Choose a provider. MiniMax is the owner's decision and the only one. |
| Model-assisted **selection**: which spans and claims a packet should carry, where M3 ranks by term overlap alone | Replace retrieval. The lexical index, the anchors and the evaluator are unchanged; the model chooses among what they find. |
| Model-assisted **artifact creation**: a derivation that produces a claim or a projection, recorded as evidence with its inputs | Decide anything. A model never marks a claim `binding`; that is an authority's act and stays one. |
| Bounded runtime: model work out of the preparation tick, with deadlines, cancellation and a concurrency bound | Stream. [STACK §8.1](../readiness/STACK.md) records why, and the decision is revisited only on a measured cancellation-latency problem. |
| Derivation records that replay offline | Promise usefulness. That is J6, at M7. |

**What stays deterministic, and must be shown to:** the compiler's ordering, the sealing of a packet, the citation of every span to a byte range at a named tree, the coverage report, and the drop order. A model may change *what is selected*; it may not change *how what is selected is recorded*. The golden packet digest is the guard that says so, and M4 keeps it: a packet compiled with no model available must still be byte-identical to today's.

**M5 owns** the memory engine's projections and the wider derivation families; M4 owns the runtime they will run in.

### The pull requests

Split as M3 was, each with its gate stated before its code, each reviewed at its head before merging.

| | Scope | Gate |
|---|---|---|
| **m4a** | **The envelope, before any transport exists.** Two counters, admission over the fully serialized request, per-request and per-job ceilings, typed `budget_exhausted`. A fake transport that records what it was asked to send and never opens a socket. | Tests with the fake transport, including both counters independently. Mutant: the admission check removed. **No network code in this PR.** |
| **m4b** | **The wire and the credential.** Both dialects' serializer and parser, the Keychain read at process start, redaction at the recording boundary, the four provider behaviours handled as ordinary outcomes. Still no live call. | Recorded-fixture tests for both dialects. A test that a response carrying a credential-shaped string is redacted before it reaches an artifact. |
| **m4c** | **The bounded runtime.** Model work leaves the preparation tick; deadlines, cancellation, a concurrency bound; failure reported as an item's unmet reason. The index build's stall is resolved here. | The measured stall falls; a cancelled call leaves no partial record; a failed call leaves an unmet item with a reason and never a hang. |
| **m4d** | **Derivation records and replay.** Every call sealed as an evidence artifact; a packet rebuilt offline from retained records. | A test rebuilds a model-assisted packet with the transport refused and compares digests. |
| **m4e** | **The first live run, and the journeys.** J1 revisited with a model; both sealed pilot questions rerun. | Scored by the reviewer against the same oracles, with the deterministic runs as baselines and tokens, spend and latency recorded beside them. |

## 2. The owner's standing decisions, and where each is enforced

Every row is the owner's, recorded before M4 and unchanged by it. The right-hand column is where M4 will make it true; none of that code exists yet.

| Constraint | Enforced at |
|---|---|
| **MiniMax only.** No other provider without the owner naming it. | The provider registry admits one provider id, from configuration, and a launch naming any other **refuses to start**, as an unreadable repository registration already does. |
| **Primary wire** `https://api.minimax.io/v1`, OpenAI-compatible. **Second dialect** `https://api.minimax.io/anthropic`. | Two serializers and two parsers over one transport. The endpoint is configuration, not a literal in the call path. |
| **Models** `MiniMax-M2.7-highspeed` (extraction, large-result projection), `MiniMax-M2.7` (ordinary derivation), `MiniMax-M3` (synthesis, request-time investigation, image input). | A model id outside the three is refused at admission, before serialization. The model id is recorded in every derivation record. |
| **Credential:** macOS Keychain service `minimax_api_key`, **read at process start only**, never logged, never in the repository, **no other credential ever read**. | One read, in the provider's construction, into a value that is never `Debug`-printed and never serialized. A test asserts the key's bytes appear in no artifact, no event, no error and no log line, which is [CORE §18.1](https://github.com/Combraton/combraton/blob/main/docs/spec/protocol/CORE.md) applied to this credential. |
| **Envelope:** 20M tokens per 5-hour window, 200M per month. Background spend zero until enabled. | §3. |

**What CI and Linux do instead: nothing live.** The Keychain is macOS-only and the key is the owner's, so **CI never calls a model and never reads a credential**. Every CI test runs against the fake transport or recorded fixtures, and a live run happens only on the owner's machine, deliberately, with its cost recorded. A test that would need a live call is marked and skipped rather than silently passing; a suite that cannot tell the difference between "no model configured" and "model agreed with us" is not a suite. **A replayed transcript is not a live run**, and a labelled fake is fault injection, never acceptance ([MODEL-RUNTIME §7](../../spec/MODEL-RUNTIME.md)).

## 3. The budget, in code before the first call exists

The quota is **shared with the owner's other tools** ([STACK §8.1](../readiness/STACK.md)), so an overspend degrades their working environment rather than merely costing money. That is why the admission check is m4a — before any transport — and why exhaustion is a typed result and never a retry.

- **Two counters, checked independently.** A 5-hour window and a calendar month have different reset semantics, and the month can be almost untouched while the window is exhausted. Admission checks both; passing one is not passing.
- **Counted before sending**, over the **fully serialized request** — instructions, tool schemas, messages, excerpts, tool outputs and the provider's own overhead — using MiniMax's `POST /v1/responses/input_tokens`, with reserved generation and a safety margin, per [MODEL-RUNTIME §2](../../spec/MODEL-RUNTIME.md).
- **Reconciled after**, from the response's usage, so the counters track what was actually spent and not what was estimated. A divergence between estimate and usage is recorded, because a systematically low estimate is how an envelope leaks.
- **A call that would exceed either counter is refused with a typed `budget_exhausted` and is never sent.** Not truncated, not retried, not queued behind a sleep. A retry loop against a shared quota is a denial of service against its owner.
- **Per-request and per-job ceilings sit under the envelope**, so one runaway job cannot consume a window even when the window has room.
- **Background spend is zero** until the owner enables it, which means no speculative call, no warming, and no prefetch.

**Gate:** tests with a fake transport that asserts it was never called when admission refuses; both counters exercised separately; the reconciliation path exercised with a usage figure that differs from the estimate. **Mutant: the admission check removed**, which must fail at the assertion that the transport was not called.

## 4. Provider facts that are design inputs, not discoveries

Four behaviours are already recorded in [STACK §8.1](../readiness/STACK.md) and are stated by the owner independently of any call CBR has made. M4 designs for them:

1. **`response_format` and `json_schema` are silently ignored** — HTTP 200 with free-form prose. So **CBR parses and validates every model output**, and a schema is a thing CBR checks rather than a thing the provider guarantees.
2. **`tool_choice: "required"` is silently ignored** while `"none"` is honoured. A text response where a tool call was demanded is therefore an **ordinary outcome to repair**, not a transport error. Repair spends real tokens, so the repair budget is bounded and debited from the same envelope.
3. **`MiniMax-M3` embeds `<think>…</think>` in `message.content`** unless `reasoning_split` is set. Reasoning text reaching a sealed derivation record as if it were output would be a correctness problem, so the split is set and the parser refuses content that still carries the marker.
4. **There is no `data: [DONE]` sentinel**, so **M4 does not stream**; whole responses only.

The rule under all four: **nothing downstream trusts a shape the model was only asked for.** Invalid output is a **recorded failure with a bounded retry**, and the failure is in the derivation record — not swallowed, not retried until it looks right.

## 5. Recording and replay

**Every model call is an evidence artifact.** The request, the response, the model id, the token counts and the latency, sealed the way M3 seals a cited file, so a derivation can be inspected long after the call.

**Redaction happens at the recording boundary**, not afterwards. A provider response can carry a third party's live credential — that is a known failure mode on one of the pilot repositories, recorded in [RELEASE-SCOPE §5](../readiness/RELEASE-SCOPE.md), not a precaution. Redaction therefore runs between the transport and the store, so an unredacted body never reaches disk, and the test is that the store contains no match rather than that the log looks clean.

**A packet built with a model is reproducible from the retained records without calling the model again**, as [INTERNALS §5](../../spec/INTERNALS.md) requires. **Gate:** a test rebuilds a model-assisted packet with the transport constructed to panic if called, and compares digests with the original.

One thing already known and carried in: an ingested artifact's id embeds the instant it was ingested, and a claim's revision digest is taken over a record holding it, so **a packet citing an ingested artifact is byte-reproducible within its store and not across stores** ([JOURNEYS](../../verification/JOURNEYS.md#what-the-pilots-changed-and-what-changed-back)). M4's replay test compares within one store, and the cross-store property stays reported rather than assumed away.

## 6. What may be sent

**Only content inside the requesting session's view and its readable claims** — the same two sets M3 already resolves **at the command, where the grant is**, and carries in the job. Nothing about a model call re-opens that question, and the model runtime never reads the store on its own authority.

This is the leak M3 shipped and fixed once already: discovery called an operation body whose authorization was step 6 of a command that had not run, and every claim in the store went into every packet. **Gate:** a test proves that a repository or a claim outside the submitting grant's view appears nowhere in a request body — asserted over the serialized bytes the fake transport received, not over the selection that preceded it.

**The repositories.** brian2 is CeCILL-licensed and public; Knowscroll-v2 is public. Sending their text to a provider is sending public text, and the licence still governs what CBR may **commit** — digests, paths, spans, counts and costs only, which is unchanged. **Any private repository needs the owner's explicit word before a single byte of it is sent**, and CBR has no such word today.

## 7. Bounded runtime

**Model work leaves the preparation tick.** M3 measured the cost of doing long work inside it: the index build holds the tick throughout, **7.2s** on CBR's own 1,066 blobs, **12.8s** on brian2's 553 blobs and 5.3 MB, and **3.9s** on Knowscroll's 145, with every other job on that provider waiting it out. A model call is longer and less predictable than any of those, so M4 is where this is resolved — for the index build as well as for the model.

- **Deadlines** are the request's, already in the protocol, and a call that would outlast one is not started.
- **Cancellation** drops the request; a cancelled call leaves no partial derivation record.
- **A concurrency bound** caps calls in flight, because a shared quota plus unbounded concurrency is the overspend of §3 arriving by another route.
- **Failure is an item's unmet reason, never a hang.** A provider error, a timeout, a refusal at admission and an invalid output after bounded repair all end as a typed reason a consumer can read.

## 8. How M4 is judged

**J1 revisited with a model**, against the same predeclared oracle, with the deterministic M3 run as its baseline. The question is not whether the model produces something plausible; it is whether the packet holds the facts the oracle names, and whether every citation still resolves to exact bytes at a named tree.

**Both sealed pilot questions rerun.** They stay sealed: this session has never seen either oracle and does not score either packet.

- **brian2 is the fair test.** Its question failed the deterministic compiler twice — one of three required facts on both runs, unchanged — and its failure is the measured cost of lexical discovery finding only what shares vocabulary with the question. Whether a model gets past that is exactly what M4 claims, so this is the run that can falsify the claim.
- **Knowscroll is the weaker signal**, and the caveat travels with it: its rerun passed, but the change that moved it was proposed by the reviewer, who holds the oracle. A pass under those conditions confirms that a general fix was general; it is not independent evidence of usefulness.
- **Recorded beside each:** tokens in and out, spend against both counters, latency, the model id, and the deterministic run's packet for comparison.

### Negative controls, stated before the runs

1. **No model configured.** The same request produces today's packet, byte-identical to the golden digest. If it does not, M4 has changed the deterministic path while claiming not to.
2. **The model refuses or returns nothing usable.** The packet is still produced, from the deterministic selection, with the failure recorded as a derivation and the item's reason stating it. Degradation is honest, not silent.
3. **A claim outside the grant, with a model in the loop.** The packet is byte-identical to one prepared where the claim was never proposed — the m3c assertion, re-run with the model runtime present.
4. **A replayed transcript is labelled.** A run against recorded fixtures never appears in a record as a live run.

### The mutants expected to be needed

| Mutant | Must be killed by |
|---|---|
| The admission check removed | the fake transport asserting it was never called |
| One of the two counters dropped | a window-exhausted case that the monthly counter alone would admit |
| The credential read on every call rather than at process start | a test that the Keychain is read exactly once |
| Redaction moved after the write | a store scan finding the credential-shaped string on disk |
| A model output trusted without validation | free-form prose where a schema was requested, carried into a packet |
| `<think>` content carried into a derivation record | the parser's refusal |
| A repository outside the view included in a request body | the serialized-bytes assertion of §6 |
| The replay path calling the provider | a transport that panics when called |
| A cancelled call leaving a partial record | the record's absence |
| An unbounded repair loop | the repair budget's ceiling |

## What this document does not settle

- **The usable context per model is measured in M4, never asserted here** ([STACK §8.1](../readiness/STACK.md)). A budget that widens to fill an advertised window is the failure the admission design exists to prevent.
- **Nothing here is evidence of anything.** No call has been made. Every number above is either the owner's decision or a measurement from M3, and the interim status line stays *"model-assisted derivation implemented; live-model evidence pending M4"* until there is a sealed transcript with a recorded cost.
