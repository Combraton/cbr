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
| **m4a** | **The envelope, before any transport exists.** Two-step admission whose local step alone can refuse; a durable ledger with a reservation written before the send; per-request and per-job ceilings; `budget_exhausted` and the distinct provider-exhaustion outcome. A fake transport that records what it was asked to send and never opens a socket. | Tests with the fake transport, both counters independently, a restart between reservation and reconciliation. Mutants: the local check removed; the provider count reached for a request the local step refused; the reservation written after the send; counters lost on restart. **No network code and no credential read in this PR.** |
| **m4b** | **The wire and the credential. Fixtures only — no live call.** Both dialects' serializer and parser, the Keychain read once at construction and only when a model is configured, redaction at the recording boundary, the four provider behaviours handled as ordinary outcomes. | Recorded-fixture tests for both dialects, the fixtures written by hand from public documentation and **labelled as unverified against the live service**. A test that a credential-shaped string in a response is redacted before anything reaches the store. A test that the Keychain is read exactly once, and that a configured model with no Keychain is a **refused launch**, never a fallback. A test that no test in the suite resolves a host. |
| **the calibration** | **The first live calls, and the only ones before m4e.** Its own step, **after m4b is reviewed and merged**, and only on the owner's explicit word at that time. [§10](#10-the-calibration-and-what-stops-m4). | One provider count above its local estimate stops M4. |
| **m4c** | **The bounded runtime.** Model work leaves the preparation tick; deadlines, cancellation, a concurrency bound; failure reported as an item's unmet reason. The index build's stall is resolved here. | The measured stall falls; a cancelled call leaves no partial record; a failed call leaves an unmet item with a reason and never a hang. |
| **m4d** | **Derivation records and replay.** Every call sealed as an evidence artifact; a packet rebuilt offline from retained records. | A test rebuilds a model-assisted packet with the transport refused and compares digests. |
| **m4e** | **The first live run, and the journeys.** J1 revisited with a model; both sealed pilot questions rerun, **under a hard cap of 5,000,000 tokens** enforced by the per-job ceiling. | Scored by the reviewer against the same oracles, with the deterministic runs as baselines and tokens, spend and latency recorded beside them. A run exceeding its estimate by more than half stops and is reported. |

## 2. The owner's standing decisions, and where each is enforced

Every row is the owner's, recorded before M4 and unchanged by it. The right-hand column is where M4 will make it true; none of that code exists yet.

| Constraint | Enforced at |
|---|---|
| **MiniMax only.** No other provider without the owner naming it. | The provider registry admits one provider id, from configuration, and a launch naming any other **refuses to start**, as an unreadable repository registration already does. |
| **Primary wire** `https://api.minimax.io/v1`, OpenAI-compatible. **Second dialect** `https://api.minimax.io/anthropic`. | **Three** serializers and parsers over one transport, and the host is a **constant** rather than configuration — an endpoint a launch can set is one a mistake or a planted file can move. Which OpenAI-compatible surface under `/v1` is primary was settled by [ADR 001 question 13](../../decisions/001-standalone-v0.1-scope-and-stack.md) after the first calibration run: the **Responses** API, because it is the only dialect the counting endpoint describes. Chat-completions and Anthropic are secondary and are admitted by the local bound alone. |
| **Models** `MiniMax-M2.7-highspeed` (extraction, large-result projection), `MiniMax-M2.7` (ordinary derivation), `MiniMax-M3` (synthesis, request-time investigation, image input). | A model id outside the three is refused at admission, before serialization. The model id is recorded in every derivation record. |
| **Credential:** macOS Keychain service `minimax_api_key`, **read at process start only**, never logged, never in the repository, **no other credential ever read**. | One read, in the provider's construction, **and only when a model is configured** — see below. Into a value that is never `Debug`-printed and never serialized. A test asserts the key's bytes appear in no artifact, no event, no error and no log line, which is [CORE §18.1](https://github.com/Combraton/combraton/blob/main/docs/spec/protocol/CORE.md) applied to this credential. |
| **Envelope:** 20M tokens per 5-hour window, 200M per month. Background spend zero until enabled. | §3. |

**The credential is read only when a model is configured.** A launch with no model configured **never touches the Keychain** — which is what CI is, what every conformance run is, and what a developer running the suite is. A Keychain read is a prompt, an audit entry and a secret in a process that had no use for one, so "no model configured" must reach the transport layer as a configuration state rather than as a key that is fetched and then unused.

**How the read is done was m4b's decision, and it is made: the `security` tool**, at an absolute path, with arguments fixed at compile time and no shell. The tradeoff stated here before there was code — no dependency but a child process, against in-process but a dependency in the credential path — was resolved by the owner reading their own constraint: **"never passed to a child process" means the secret is never handed to a child**, by argument vector, environment or standard input, and reading it back from one over a private pipe is not that. Two reasons beyond the dependency count: Keychain access control is **per program** and this binary is unsigned and rebuilt constantly, so an in-process read means a prompt after every rebuild or an *Always Allow* on an unsigned binary; and it is how the owner's other tools already read this same key. The full decision, the rejected alternative and what would reopen it are recorded in [ADR 001 question 12](../../decisions/001-standalone-v0.1-scope-and-stack.md). The value is still **never placed in the environment**, **never handed to a child**, and **zeroed on drop**. **Mutant: the Keychain read with no model configured**, killed by a test that launches without one and asserts no read happened.

**What CI and Linux do instead: nothing live.** The Keychain is macOS-only and the key is the owner's, so **CI never calls a model and never reads a credential**. Every CI test runs against the fake transport or recorded fixtures, and a live run happens only on the owner's machine, deliberately, with its cost recorded. A test that would need a live call is marked and skipped rather than silently passing; a suite that cannot tell the difference between "no model configured" and "model agreed with us" is not a suite. **A replayed transcript is not a live run**, and a labelled fake is fault injection, never acceptance ([MODEL-RUNTIME §7](../../spec/MODEL-RUNTIME.md)).

## 3. The budget, in code before the first call exists

The quota is **shared with the owner's other tools** ([STACK §8.1](../readiness/STACK.md)), so an overspend degrades their working environment rather than merely costing money. That is why admission is m4a — before any transport — and why exhaustion is a typed result and never a retry.

### Admission is two steps, because the count call is itself a send

`POST /v1/responses/input_tokens` carries **the fully serialized request to the provider**. It is a send. An admission design that counts there first has already sent the body it was deciding whether to send, which defeats the point twice over: it spends whatever the count costs, and it puts content on the wire before anything decided that the content was permitted to go.

So:

1. **A local, conservative estimate decides first**, from MiniMax's published `tokenizer.json` where it applies and a byte-based upper bound otherwise. It is deliberately an over-estimate: a local step that guesses low would admit requests the provider then refuses, which is the leak in another form. **This step alone can refuse**, with `budget_exhausted`, and nothing leaves the process.
2. **The provider count runs only for a request the local step has already admitted.** It is a model call in every respect that matters: it is **under the same view rule** as any other call (§7), it is **itself recorded** as a derivation record (§6), and **its own cost, if any, is debited** from the same two counters. A count that is free is still recorded; "free" is the provider's word, not a fact CBR should build on.
3. **The provider's count refines the estimate** for the real send, which is admitted against the counters a third time with the accurate figure.

**m4a therefore has a complete admission path with no network at all**, which is exactly what CI exercises: the local estimate, both counters, the typed refusal, and the ceilings, with a fake transport asserting it was never called.

### The counters are durable, and conservative under failure

- **They live in the store**, in CBR's own ledger, and **survive restart**. Counters held in memory are counters that a crash forgets, and a forgotten spend is an overspend against someone else's quota.
- **A reservation is written before the send and reconciled after.** A crash between the two therefore leaves the spend **counted rather than forgotten** — the conservative direction. Reconciliation replaces the reservation with the response's actual usage, and a divergence between estimate and usage is recorded, because a systematically low estimate is how an envelope leaks.
- **The window is CBR's own rolling five hours over its own ledger**, not the provider's window and not a wall-clock bucket. CBR cannot see the provider's counter, so it accounts for what it spent and nothing else.
- **Two counters, checked independently.** A rolling five hours and a calendar month have different reset semantics, and the month can be almost untouched while the window is exhausted. Passing one is not passing.
- **Per-request and per-job ceilings sit under the envelope**, so one runaway job cannot consume a window even when the window has room.
- **Background spend is zero** until the owner enables it: no speculative call, no warming, no prefetch.

### Two different exhaustions, and neither is retried

- **`budget_exhausted`** is CBR's own: the ledger says this call would exceed the window or the month. The call is **never sent**.
- **The provider reports exhaustion while CBR's ledger has room.** This will happen, because the quota is shared with the owner's other tools and CBR only ever sees its own spending. It is a **distinct typed outcome** — the envelope was not the binding constraint, so reporting `budget_exhausted` would be a lie about which limit was hit. It is **not retried**, and it **does not corrupt the ledger**: the reservation for a call the provider refused is reconciled to what was actually spent, which for a refusal is whatever the provider says and otherwise nothing.

Neither one loops. A retry loop against a shared quota is a denial of service against its owner.

### Gate and mutants

Tests with a fake transport that asserts it was never called when admission refuses; both counters exercised separately; the reconciliation path exercised with a usage figure that differs from the estimate; a restart between reservation and reconciliation.

| Mutant | Must be killed by |
|---|---|
| The local admission check removed | the fake transport asserting it was never called |
| **The provider count reached for a request the local estimate refuses** | the same assertion — the count endpoint is a send and the fake transport counts it as one |
| One of the two counters dropped | a window-exhausted case the monthly counter alone would admit |
| **The reservation written after the send** | a crash injected between send and reconciliation, after which the spend is still counted |
| **Counters lost on restart** | a restart with a spend already recorded, after which the window is still exhausted |
| Provider-reported exhaustion reported as `budget_exhausted` | the typed outcome assertion |

## 4. Provider facts that are design inputs, not discoveries

Four behaviours are already recorded in [STACK §8.1](../readiness/STACK.md) and are stated by the owner independently of any call CBR has made. M4 designs for them:

1. **`response_format` and `json_schema` are silently ignored** — HTTP 200 with free-form prose. So **CBR parses and validates every model output**, and a schema is a thing CBR checks rather than a thing the provider guarantees.
2. **`tool_choice: "required"` is silently ignored** while `"none"` is honoured. A text response where a tool call was demanded is therefore an **ordinary outcome to repair**, not a transport error. Repair spends real tokens, so the repair budget is bounded and debited from the same envelope.
3. **`MiniMax-M3` embeds `<think>…</think>` in `message.content`** unless `reasoning_split` is set. Reasoning text reaching a sealed derivation record as if it were output would be a correctness problem, so the split is set and the parser refuses content that still carries the marker.
4. **There is no `data: [DONE]` sentinel**, so **M4 does not stream**; whole responses only.

The rule under all four: **nothing downstream trusts a shape the model was only asked for.** Invalid output is a **recorded failure with a bounded retry**, and the failure is in the derivation record — not swallowed, not retried until it looks right.

## 5. Repository text is untrusted input

A file in a repository can carry text addressed to a model. CBR reads repositories it does not own — brian2 is a third party's, and every future one will be somebody's — so this is not hypothetical, and a model that acts on such text is a model doing what the repository said rather than what the request asked.

The rule is structural rather than a matter of prompting:

- **Model-assisted selection chooses only among candidate ids CBR offered**, which is a **closed set** built by the deterministic path: the spans retrieval found, the anchors, the eligible claims. The model returns ids from that set and nothing else.
- **An id outside the set is invalid output**: recorded as such and repaired within the bounded budget of §4, or dropped. It is never resolved, never looked up, never treated as a hint.
- **The model never introduces a path, a span, a citation or a label.** Every one of those comes from the deterministic path, which is the same property that makes a packet reproducible and its citations resolvable to exact bytes.
- **Prose the model writes enters a packet only as a section labelled `inferred`**, citing the inputs it was derived from, and **never `binding`**. `binding` is an authority's act, and nothing a model produces can be one — the same rule that already forbids a source file being promoted to `binding` in M3's J1.

**Negative control 5:** a repository file is planted that instructs the model to select something outside the candidate set, to mark a claim `binding`, or to reveal content from outside the view. The packet is unchanged outside the closed set: no id that was not offered, no label the model chose, nothing from beyond the view. **Mutant: ids accepted without the closed-set check**, killed by that control.

This is the least surprising part of the design and the easiest to erode later, so it is written down before there is any code to erode.

## 6. Recording and replay

**Every model call is an evidence artifact.** The request, the response, the model id, the token counts and the latency, sealed the way M3 seals a cited file, so a derivation can be inspected long after the call.

**Redaction happens at the recording boundary**, not afterwards. A provider response can carry a third party's live credential — that is a known failure mode on one of the pilot repositories, recorded in [RELEASE-SCOPE §5](../readiness/RELEASE-SCOPE.md), not a precaution. Redaction therefore runs between the transport and the store, so an unredacted body never reaches disk, and the test is that the store contains no match rather than that the log looks clean.

**A derivation record holds repository excerpts and claim text**, so it is evidence with a readable set, not a log. Its artifact is **readable only under the same view and readable claims as the job that made it**, and it is **never reachable through a packet by a reader who could not read its contents** — a citation that hands over material the citing reader was not permitted to see is the M3 leak arriving through a new door. **Gate:** the m3c byte-identity assertion, applied to derivation records — a packet prepared for a reader outside the job's view is byte-identical to one prepared in a store where the derivation never happened.

**A packet built with a model is reproducible from the retained records without calling the model again**, as [INTERNALS §5](../../spec/INTERNALS.md) requires. **Gate:** a test rebuilds a model-assisted packet with the transport constructed to panic if called, and compares digests with the original.

One thing already known and carried in: an ingested artifact's id embeds the instant it was ingested, and a claim's revision digest is taken over a record holding it, so **a packet citing an ingested artifact is byte-reproducible within its store and not across stores** ([JOURNEYS](../../verification/JOURNEYS.md#what-the-pilots-changed-and-what-changed-back)). M4's replay test compares within one store, and the cross-store property stays reported rather than assumed away.

## 7. What may be sent

**Only content inside the requesting session's view and its readable claims** — the same two sets M3 already resolves **at the command, where the grant is**, and carries in the job. Nothing about a model call re-opens that question, and the model runtime never reads the store on its own authority.

This is the leak M3 shipped and fixed once already: discovery called an operation body whose authorization was step 6 of a command that had not run, and every claim in the store went into every packet. **Gate:** a test proves that a repository or a claim outside the submitting grant's view appears nowhere in a request body — asserted over the serialized bytes the fake transport received, not over the selection that preceded it.

**Provider-side retention: a stated limit, not a solved problem.** The Responses API offers **no request option by which CBR can say "do not retain this"**: `store` is a response property, not a request one (published OpenAPI schema, read 2026-09-21; the live response of calibration run 2 carried `store: false`, which CBR did not ask for and cannot ask for).

**The owner's decision, 2026-09-21: this is accepted for public repositories only** — CBR's own source, Knowscroll-v2 and brian2. It is accepted as a limit that is understood rather than as a risk that has been removed: those repositories are public, so what is sent is already public, and the provider may retain it.

**Any private repository still needs the owner's explicit word before a single byte of it is sent**, exactly as §7 already requires, and that word has not been given for any. The absence of a retention control is a reason that bar stays where it is, not a reason to lower it.

**Two more things are unobservable, and stay stated as such.** Both from calibration run 2, observed 2026-09-21:

- **Whether `service_tier: standard` was honoured is unknown.** CBR sends it explicitly and the response returns `service_tier: null`, so the field is accepted and not reflected. Nothing in a response says which tier served it. CBR keeps sending `standard` because sending nothing would leave a default to change under it; it cannot claim the request was honoured.
- **`store` came back `false` without being asked.** There is no request parameter for it, so CBR did not ask and cannot; the value observed on one response was `false`. **One response is not a policy**, and this does not soften the retention limit above — it is a single observation recorded next to it.

Both are things CBR **cannot** verify rather than things it has not got round to verifying, which is a different claim and is the one being made.

**The repositories.** brian2 is CeCILL-licensed and public; Knowscroll-v2 is public. Sending their text to a provider is sending public text, and the licence still governs what CBR may **commit** — digests, paths, spans, counts and costs only, which is unchanged. **Any private repository needs the owner's explicit word before a single byte of it is sent**, and CBR has no such word today.

## 8. Bounded runtime

**Model work leaves the preparation tick.** M3 measured the cost of doing long work inside it: the index build holds the tick throughout, **7.2s** on CBR's own 1,066 blobs, **12.8s** on brian2's 553 blobs and 5.3 MB, and **3.9s** on Knowscroll's 145, with every other job on that provider waiting it out. A model call is longer and less predictable than any of those, so M4 is where this is resolved — for the index build as well as for the model.

- **Deadlines** are the request's, already in the protocol, and a call that would outlast one is not started.
- **Cancellation** drops the request; a cancelled call leaves no partial derivation record.
- **A concurrency bound** caps calls in flight, because a shared quota plus unbounded concurrency is the overspend of §3 arriving by another route.
- **Failure is an item's unmet reason, never a hang.** A provider error, a timeout, a refusal at admission and an invalid output after bounded repair all end as a typed reason a consumer can read.

## 9. How M4 is judged

**J1 revisited with a model**, against the same predeclared oracle, with the deterministic M3 run as its baseline. The question is not whether the model produces something plausible; it is whether the packet holds the facts the oracle names, and whether every citation still resolves to exact bytes at a named tree.

**Both sealed pilot questions rerun.** They stay sealed: this session has never seen either oracle and does not score either packet.

- **brian2 is the fair test.** Its question failed the deterministic compiler twice — one of three required facts on both runs, unchanged — and its failure is the measured cost of lexical discovery finding only what shares vocabulary with the question. Whether a model gets past that is exactly what M4 claims, so this is the run that can falsify the claim.
- **Knowscroll is the weaker signal**, and the caveat travels with it: its rerun passed, but the change that moved it was proposed by the reviewer, who holds the oracle. A pass under those conditions confirms that a general fix was general; it is not independent evidence of usefulness.
- **Recorded beside each:** tokens in and out, spend against both counters, latency, the model id, and the deterministic run's packet for comparison.

### The first live run has a cap before it starts

Stated now, before any call, so that the number is a limit rather than a description of what happened.

| | Estimate |
|---|---:|
| J1 revisited with a model | 150,000 tokens |
| brian2's question | 600,000 tokens |
| Knowscroll's question | 400,000 tokens |
| Repair, retry and the count calls of §3, across all three | 350,000 tokens |
| **Expected total for m4e** | **1,500,000 tokens** |

**The hard cap for the whole of m4e is 5,000,000 tokens** unless the owner raises it — a little over three times the estimate, because an estimate made before the first live call has ever run is not a measurement. It is **enforced by the per-job ceiling of §3, not by intention**: m4e runs under a job whose ceiling is that number, and a call that would cross it is refused with `budget_exhausted` like any other.

**Exceeding the estimate by more than half stops the run and is reported.** At 2,250,000 tokens the run halts, whatever state it is in, and what was spent and on what is reported before anything continues. A run that quietly costs three times its estimate has told you something about the estimate that you only learn if it stops.

These are estimates, and the first thing m4e produces is the measurement that replaces them.

### Negative controls, stated before the runs

1. **No model configured.** The same request produces today's packet, byte-identical to the golden digest. If it does not, M4 has changed the deterministic path while claiming not to.
2. **The model refuses or returns nothing usable.** The packet is still produced, from the deterministic selection, with the failure recorded as a derivation and the item's reason stating it. Degradation is honest, not silent.
3. **A claim outside the grant, with a model in the loop.** The packet is byte-identical to one prepared where the claim was never proposed — the m3c assertion, re-run with the model runtime present.
4. **A replayed transcript is labelled.** A run against recorded fixtures never appears in a record as a live run.
5. **A repository file instructs the model.** A planted file tells it to select something outside the candidate set, to mark a claim `binding`, or to reveal content from beyond the view. The packet is unchanged outside the closed set (§5).

### The mutants expected to be needed

| Mutant | Must be killed by |
|---|---|
| The local admission check removed | the fake transport asserting it was never called |
| The provider count reached for a request the local estimate refuses | the same assertion; the count endpoint is a send |
| One of the two counters dropped | a window-exhausted case that the monthly counter alone would admit |
| The reservation written after the send | a crash injected between send and reconciliation |
| Counters lost on restart | a restart with a spend already recorded |
| Ids accepted without the closed-set check | negative control 5 |
| The credential read on every call rather than at process start | a test that the Keychain is read exactly once |
| The Keychain read with no model configured | a launch without one, asserting no read happened |
| Redaction moved after the write | a store scan finding the credential-shaped string on disk |
| A model output trusted without validation | free-form prose where a schema was requested, carried into a packet |
| `<think>` content carried into a derivation record | the parser's refusal |
| A repository outside the view included in a request body | the serialized-bytes assertion of §7 |
| The replay path calling the provider | a transport that panics when called |
| A cancelled call leaving a partial record | the record's absence |
| An unbounded repair loop | the repair budget's ceiling |

## 10. The calibration, and what stops M4

**m4b makes no live call.** §1 says so, and an earlier draft of §3 contradicted it by calling the tokenizer comparison "m4b's first measurement". The comparison is real and necessary, and it is **its own step**: after m4b is reviewed and merged, and **only on the owner's explicit word at that time**. Having built a transport is not permission to use it.

**What it is for.** The local estimate is sound by construction — a byte-level BPE emits at most one token per byte — but **byte-level is an assumption about the provider's tokenizer, not a fact CBR has checked**. The calibration checks it, and it is the only thing that can.

### The protocol, fixed before the run

- **Token-count calls**, on a **fixed corpus of CBR's own public source files** plus one non-Latin text — the same corpus the estimate's one-sidedness is already pinned against. Those sources are this repository's own and public, so nothing here sends anyone else's text.
- **One completion**, with a generation limit of **16 tokens**, so the completion path is exercised once end to end and cannot cost much even if everything else is wrong.
- **A hard run ceiling of 100,000 tokens**, through the run-level ceiling m4a built (`--model-run-ceiling`), so the cap is enforced by the ledger rather than by intention.
- **Every call recorded and redacted** like any other, per §6.
- **The result is a table**: the local estimate against the provider's count, per file.
- **And one more comparison, for the single completion**: the counting endpoint's prediction for that request against the `usage.input_tokens` the provider then charged for the same request. The first comparison says whether CBR's bound is sound; **this one says whether making the count is worth anything at all**. A charge above the prediction by more than **2%** is reported as a finding.
- **It has an entry point, built and reviewed in m4b**: `cbr-provider --calibrate <path>`, which needs `--permit-model-network` and a `--model-run-ceiling` no higher than 100,000, serves nothing, and exits. It was built there rather than on the day because *if the first live call needs new plumbing, the first live call runs code nobody reviewed.* It is tested end to end against the fake transport, stop condition included, and **has not been run**.

### What ends it

**One provider count above its local estimate means the bound is unsound.** Then: **stop, report, and nothing else in M4 proceeds until it is fixed.** Not "note it and widen the margin" — the whole admission design rests on the estimate never falling below the truth, and one counter-example says it does. The fix is a different estimate, and it gets its own review.

A count *below* the local estimate is the expected case and is not a finding. The bound is loose by three to four times for prose; measuring how loose is what the table is for.

**The second comparison is a finding, never a stop.** A bill above the count endpoint's prediction says the prediction is not reliable, which is a fact about the count endpoint for the owner to weigh; it does not falsify the local bound, which is the thing every admission rests on. The two must not be confused, which is why they have different consequences written down before the run.

### What run 1 established, and what it did not

**Run 1 stopped at its first call** and is recorded in [CALIBRATION.md](CALIBRATION.md). It sent the counting endpoint a chat-completions body; the endpoint documents a Responses-shaped one and refused it. So **no count was obtained, the bound is neither confirmed nor falsified, and the protocol above is unchanged** — except that the primary wire is now the Responses dialect ([ADR 001 question 13](../../decisions/001-standalone-v0.1-scope-and-stack.md)), which is what makes the count comparable to the completion it predicts. A second run needs the owner's word again.

## What this document does not settle

- **The usable context per model is measured in M4, never asserted here** ([STACK §8.1](../readiness/STACK.md)). A budget that widens to fill an advertised window is the failure the admission design exists to prevent.
- **Nothing here is evidence of anything.** No call has been made. Every number above is either the owner's decision or a measurement from M3, and the interim status line stays *"model-assisted derivation implemented; live-model evidence pending M4"* until there is a sealed transcript with a recorded cost.
