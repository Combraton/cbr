# M4 readiness — the bounded model runtime

**Written before anything in M4 was built and before any model had been called.** This document exists because M4 is the first milestone that spends the owner's quota and sends repository text to a third party, and both of those are decisions rather than implementation details. It was docs-only and it was reviewed before any transport code, any credential read, or any call.

**As of M4's close it is a record as well as a plan.** m4a to m4g are built and merged, and the milestone's close-out is [CLOSEOUT](CLOSEOUT.md). **Models have been called in five live sessions, each on the owner's word at the time**: two calibration runs ([§10](#10-the-calibration-and-what-stops-m4)) and three runs of the m4e harness ([§9](#9-how-m4-is-judged)), the second of which stopped at its first replay. Nothing else has ever called a model, and CI never has. Sections that describe what exists say so at their head, and the places the design moved are marked where they moved.

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

**What model-assisted selection is, and what it is not. Recorded 2026-09-21, because m4e depends on it.** m4c's call site chooses among the spans BM25 ranked **inside one file the request already named**. It does not touch discovery, so it cannot change *what a packet finds* — only which part of an already-found file it cites.

That is why it cannot move brian2. **That question failed twice** ([JOURNEYS](../../verification/JOURNEYS.md)) because the answer never entered the candidate set at all, and a better choice inside the wrong file is still the wrong file. A milestone that only improves selection can improve Knowscroll's kind of question and can do nothing whatever for brian2's, and reporting a model-assisted run without saying so would be reporting a result the design could not have produced.

**So m4e needs model-assisted discovery**, designed in [§5](#5-repository-text-is-untrusted-input) below and built behind the fake transport before any live run.

**M5 owns** the memory engine's projections and the wider derivation families; M4 owns the runtime they will run in.

### The pull requests

Split as M3 was, each with its gate stated before its code, each reviewed at its head before merging.

| | Scope | Gate |
|---|---|---|
| **m4a** | **The envelope, before any transport exists.** Two-step admission whose local step alone can refuse; a durable ledger with a reservation written before the send; per-request and per-job ceilings; `budget_exhausted` and the distinct provider-exhaustion outcome. A fake transport that records what it was asked to send and never opens a socket. | Tests with the fake transport, both counters independently, a restart between reservation and reconciliation. Mutants: the local check removed; the provider count reached for a request the local step refused; the reservation written after the send; counters lost on restart. **No network code and no credential read in this PR.** |
| **m4b** | **The wire and the credential. Fixtures only — no live call.** Both dialects' serializer and parser, the Keychain read once at construction and only when a model is configured, redaction at the recording boundary, the four provider behaviours handled as ordinary outcomes. | Recorded-fixture tests for both dialects, the fixtures written by hand from public documentation and **labelled as unverified against the live service**. A test that a credential-shaped string in a response is redacted before anything reaches the store. A test that the Keychain is read exactly once, and that a configured model with no Keychain is a **refused launch**, never a fallback. A test that no test in the suite resolves a host. |
| **the calibration** | **The first live calls, and the only ones before m4e.** Its own step, **after m4b is reviewed and merged**, and only on the owner's explicit word at that time. [§10](#10-the-calibration-and-what-stops-m4). | One provider count above its local estimate stops M4. |
| **m4c** | **The bounded runtime.** Model work leaves the preparation tick; deadlines, cancellation, a concurrency bound; failure reported as an item's unmet reason. The index build's stall is resolved here, and serving gets the call site that makes a credential worth reading. | The measured stall falls ([STALL](STALL.md): 65×, 49× and 23×); a cancelled call leaves no partial record; a failed call leaves an unmet item with a reason and never a hang. **No live call.** |
| **m4d** | **Derivation records and replay.** Every call sealed as an evidence artifact, readable only under the job's own view; a packet rebuilt offline from retained records; the retention statement below. And the permitted hand launch, `scripts/debug_launch.sh`. | A test rebuilds a model-assisted packet with a transport that panics if called and compares digests. The m3c byte-identity assertion applied to derivations. Cancellation and failure leave no partial record. |
| **m4e** | **Model-assisted discovery, and the live run as code.** The two steps of [§5](#5-repository-text-is-untrusted-input), behind the fake transport; the investigation limit counting rather than gating; the harness and its replay gate. **No live call.** | Negative control 5 against both steps; the arithmetic of [§3](#what-model-assisted-discovery-costs-against-the-three-ceilings) as a test; a dry run that drives the harness end to end. |
| **the live run** | **The first live run, and the journeys.** J1 revisited with a model; both sealed pilot questions rerun against both models, **under a hard cap of 5,000,000 tokens** enforced by the run ceiling, given to each launch as the cap less what the launches before it spent ([§9](#9-how-m4-is-judged)). Its own step, on the owner's word at the time, after m4e is reviewed and merged. | Scored by the reviewer against the same oracles, with the deterministic runs as baselines and tokens, spend and latency recorded beside them. A run exceeding its estimate by more than half stops and is reported. |

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

### Admission is one step, and the count is a second only when it can change the answer

**Revised 2026-09-21, from what calibration run 2 measured.** The design below was written before any call had been made. What the run found:

- **The count sends the repository text a second time.** Everything §7 says about what may be sent applies to it twice over.
- **The provider does not price it.** Its response carries no usage member, so CBR settles a count at the figure it counted — the conservative reading of silence. **15,538 of the 15,582 tokens that run were CBR charging itself for seven counts the provider never priced.**
- **It over-predicted the bill by 4.4×** on the one completion where both figures exist: 122 predicted, 28 charged. That is conservatism the local bound already provides.

So **the local bound admits, alone, and the call settles by usage** — which was always the truth of it. The provider count is made only where it **can change a decision**: the local bound would refuse on the window, the month, the job or the run ceiling, and a tighter figure could admit. **Not** on the per-request ceiling, which is a policy limit on how large one request may be; the local bound is deliberately the conservative measure of that, and letting the provider's figure talk CBR into sending a bigger request inverts the direction the bound protects.

**Which path admitted a call is recorded** — `admitted_local` or `admitted_count` in the ledger, and carried into the derivation record — because a selection admitted on the local bound and one admitted on the provider's count were decided by different evidence.

**One data point is not a rule.** The 4.4× and the unpriced count are one run of six files and one completion, on requests far smaller than a real packet. m4e's requests are realistic sizes and will say more; until then this is the reading of one measurement, and it is the reading that spends less. A caller whose purpose *is* the count — the calibration — asks for it explicitly.

**And the count is still settled at the figure it counted**, not at zero, until an observation says the provider charges nothing. The documentation makes no statement about billing for that endpoint, and "unbilled because a page says so" is what the calibration exists to avoid.

### The tripwire: the calibration's stop rule, kept alive

Six files cannot prove a bound; a tripwire can hold it. **At every settlement, a `usage.input_tokens` above the local input bound for that request means the bound is wrong** — and every admission CBR has ever made rests on it. It is recorded as its own ledger kind, `bound_unsound`; no further model call is admitted; and the call ends with the typed reason `local_bound_unsound`.

**Held in the ledger rather than in a process flag**, which is stronger than "for the process": the bound is a property of the code, so a restart with the same code has the same bound, and forgetting at restart would forget the one observation that invalidates everything the store has admitted.

Two rules that overlap, in order: a charge above the **local bound** stops everything, because the bound is wrong; a charge above the **count endpoint's prediction** by more than the stated margin is a finding, because the prediction is poor. The first is the graver claim and it wins.

### The original two-step design, and what remains of it

The count call **is** a send, and everything below about that remains true — it is admitted, recorded, charged and under the same view rule. What changed is when it is made.

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

### What model-assisted discovery costs, against the three ceilings

**Added at m4e, and computed rather than estimated.** Every bound on the two discovery requests is a constant — at most 6 candidates shown to the terms step, at most 20 to the choice, each candidate at most one span of retrieval's own `span_bytes` — so the largest request either step can make is a number, and `discovery::tests` computes it against the ceiling it has to fit under. A document cannot notice when a constant moves; that test can.

**That paragraph is true since m5-arith (2026-09-26), and was not quite true before it.** A span is capped in raw bytes, and a request body is JSON, which carries a quotation mark in two bytes and a C0 control in six. So a candidate could be carried in up to six times its span, and twenty spans of control characters made a choice the per-request ceiling refused outright. Since m5-arith a candidate's text is shown within 8,192 carried bytes and its path within 1,024, cut with a marker past either ([§5](#what-a-candidate-shows-a-model-and-where-it-is-cut)), so the largest request each step can make is a number again.

| | One send, tokens | One question, tokens | Against |
|---|---:|---:|---|
| One item's selection | 83,620 | 167,753 | per-request 250,000 for a send |
| Discovery's terms step | 63,718 | 129,484 | per-request 250,000 for a send |
| Discovery's choice step | 193,548 | 389,144 | per-request 250,000 for a send |
| **The whole two-step flow**, each step's first send and its widest repair | | **518,628** | per-job 1,000,000 |
| **Six such flows**, which is the six runs §9 names | | **3,111,768** | m4e's run ceiling 5,000,000 |

**How each figure is computed.** A send is what admission reserves for it on the local bound, `model::send_worst`: the body's bytes as serialized, 8 per message the provider frames (the instruction among them), the generation the body binds, and the margin of 1,024. A question is its first send and its widest repair, `model::question_worst`: a truncation repair doubles the generation, and any other repair adds CBR's repair sentence as one more message; each step here takes the wider of the two. The bodies are the widest each step can build: the longest name in `wire::MODELS` (`MiniMax-M2.7-highspeed`, 12 bytes longer than `MiniMax-M3`), a task of 4,096 bytes where the protocol admits 256 code points of at most six carried bytes, every candidate's text at the 8,192-byte cut and its path at 1,024, seven-digit line numbers, and every term at its bound; one item's selection offers `selection::CANDIDATES` of them, 8, the rows its call site asks retrieval for, and a test holds the call site to that constant. **Each figure is the most over every dialect a launch can configure** (`model::over_every_dialect`, over `wire::Dialect::ALL`, which is also the only list `Dialect::parse` reads a configured name from, so a dialect a launch can name is one the figures price). The OpenAI dialect frames the most on every body here: it alone carries the schema a structure asks for. The flow is its two questions in one dialect, since one launch serves both. `discovery::tests` and `selection::tests` compute each figure through those functions and pin it, and `scripts/m4e_run.py` reads the flow and the selection question from the same tests.

**Changed at m5-arith, 2026-09-26** ([ADR 001 question 17](../../decisions/001-standalone-v0.1-scope-and-stack.md), accepted by the owner 2026-09-26). Until then the table read 39,612, 32,943, 91,453, **373,188** and 2,239,128. Those priced every step as three copies of one send — a count, the send and a repair — on a candidate's raw bytes, `MiniMax-M3`'s name and one message fewer than the provider frames; selection's 39,612 was in no test. Three copies of a send overstate a question: a count is made only where the send itself did not fit (below). The raw bytes understated one: a body carries escaping text in more. The live runs were far from both, as §9 records. m5-arith's own first figures were the Responses dialect's alone, though a launch may configure `openai` or `anthropic`; its verification round took each over every dialect, which raised every figure, the flow's by 1,136.

**The figures above moved at m4f**, because both discovery steps now ask for room to reason: the live run measured `MiniMax-M2.7-highspeed` spending 277 to 512 output tokens thinking before it wrote anything, and truncating two of its three flows at the 512-token floor both steps had been sitting on. `budget::DISCOVERY_MIN_OUTPUT_TOKENS` is 2,048, and a test computes every figure above from the real bodies and fails when the table drifts from them. **Selection's bound stays at `budget::MIN_OUTPUT_TOKENS`, and that is decided rather than open.** Nothing measured truncated there; raising a bound against no measurement is the estimating these constants exist to stop. The reviewer ruled on it at round 47, and it is recorded here as settled so that a later reader does not reopen it for want of a sentence.

**Six, because §9 names six runs**: J1 revisited and both sealed pilot questions, each against two models. An earlier draft of this table said five, which was a number chosen to show the ceiling was not close rather than the number the plan runs.

**The run ceiling is per store, so the harness subtracts.** `Ledger::run_spend` sums the ledger it was opened over, and every run of §9 opens a new data directory; a ceiling handed to each launch unreduced is therefore the cap *once per run* rather than once. [`scripts/m4e_run.py`](../../../scripts/m4e_run.py) passes `--model-run-ceiling` to every launch as the cap **less what the launches before it spent**, so the 5,000,000 bounds the run and not the launch. Without that, six runs bounded only by the per-job ceiling of 1,000,000 could reach 6,000,000, and the stop below is checked before each run rather than after it for the same reason.

**The flow is two questions per request — not per item, and not per discovered section.** That is the property the arithmetic rests on: discovery does not scale with what it finds, so a packet carrying eight discovered spans costs what a packet carrying one costs.

**What one question can hold on the serving path**, as m5-arith states it. A question is at most two attempts, its send and one repair ([`model::REPAIRS`]), and each attempt is at most two sends, a count and a completion, so four sends in all. It holds less than four sends' reservations, because of when a count is made. The local bound admits first. A count is made only after the local bound is refused on the job, the window, the month or the run — a counter a tighter figure could satisfy — and never after a per-request refusal (`model::SERVING_COUNTING`, which both serving launches name and a source test holds them to). The count and the completion admitted on it are then both admitted on the counter that refused the send, so together they hold less than the send would have. So a question holds at most **its first send and its widest repair**, which is what the table prices. `model::tests::no_serving_question_holds_more_than_question_worst` measures that on the real call path, over a grid of ceilings around each body's reservations, rather than arguing it.

**What that assumes**, in the words of `model::question_worst`'s own documentation:

> **What it assumes.** Bills are within their reservations: the byte bound holds, a completion admitted on a count is billed no more than the count said, `max_output_tokens` is honoured, and a failed call reports no more than it reserved. And nothing frees room on the refusing counter **between the local refusal and the completion's admission** — while the count is admitted, sent and settled, and after it. If room is freed anywhere in that window, the completion is admitted on the counter as it then is, and an attempt can hold more than the send `W` it counted for. With an honest count, at most the count body's own bound `Ic`, the most is `W + Ic − (local − Ic)`: the count settled at `Ic` and a completion reserved at it, which is `W` less what the count body omits (`local − Ic`, the input bound of the three members a count body does not carry). A count below the implausibility floor is not believed, and its attempt holds `W` and less than the floor beside it, which is less. So a question holds at most `question_worst` and less than one count's reservation more for each count it made, and a test pins the maximum. Every ceiling still holds at admission.

**The freed-room exception is real and pinned.** `Runtime::call` admits the count and then the completion in two separate admissions, and nothing ties the second to the room that refused the send. So when another job's reservation settles anywhere from the local refusal to the completion's admission — while the count is admitted, on the wire, answered or being settled, or after it — the completion is admitted on the room that settlement freed. `model::tests::an_attempt_admitted_on_a_count_after_room_was_freed_holds_at_most_its_counts_reservation_more` builds that with the count in flight and asserts that the question holds its count, a completion reserved at it and its repair, and that every admission was within its ceiling. `model::tests::room_freed_anywhere_from_the_refusal_to_the_completions_admission_adds_less_than_the_count` frees the room at each place in that window the call path names, with every honest count, and pins the most an attempt holds at exactly `W + Ic − (local − Ic)`. Closing it is a change to admission and settlement, which §1 amends by the owner's decision of 2026-09-26 ([m5 READINESS §1](../m5/READINESS.md#1-scope-and-the-promise)); it is m5-settle, decided to close before m5h's live run.

**Under `Counting::Always` no bound is derived from the body.** The count comes first and its figure is floored but not capped, so a count above the local bound reserves above it. Only the calibration counts that way, and the source test above holds serving to `SERVING_COUNTING`.

**Settlement can pass a ceiling; admission cannot.** Every ceiling holds at admission. A call then settles at what the provider billed, which can exceed what was reserved, and the excess lands on counters that were checked before it existed. Four routes, each asserted with its overage and a `divergence` row by `model::tests::settlement_passes_a_ceiling_only_by_what_an_admitted_call_was_billed_over_its_reservation`, and each leaving the next admission on that counter refused:

1. a completion admitted on a count is billed above the count;
2. the input is billed above the byte bound, which also records `bound_unsound` and stops every later call though the charge stands, or the output above `max_output_tokens`;
3. a count settles at the provider's figure capped at the completion body's input bound, which is above what the count reserved by the three members the count body leaves out — 66 tokens on the published bodies;
4. a failed call reports usage above its reservation.

None is recorded in a live run. m4e's run 3 records every call admitted on the local bound alone, no count call and every charge within its local estimate ([JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g)); J2's first live run records every call admitted on the local bound alone ([m5 READINESS §10](../m5/READINESS.md#j2-live-what-it-spent-2026-09-25)); and no live record names a count outside the calibration, which counts by design. So the figures in this section bound what admission can reserve, not what a bill can charge.

**The investigation limit counts both of them**, and every other question too. At m4c the number gated: a model was asked for every item of any request whose budget was above zero, so it said *whether* and not *how much*. m4e adds two questions per request, and a limit that counted some kinds of call and not others would be two meanings for one number. So:

- **every distinct question a compile puts to a model spends one unit**, and a question already asked this compile is free, because its answer is the work pool's;
- **items are asked first**, because an item is what the request required and discovery is advisory;
- a request whose budget is gone gets the typed reason `investigation_budget_exhausted` for that item, never the span BM25 would have chosen — which would report a model-assisted selection no model made;
- **a flow that cannot finish is not started.** Discovery claims its two units together or spends neither: the terms step's answer is worth nothing without the budget to choose among what it widens to;
- and therefore **a run must authorise at least one question per want plus two**, which the harness refuses before it starts. The two rules above compose into a silence: items first, and a flow that cannot finish is not started, means a request whose items use the budget asks discovery nothing — and the packet it produces is indistinguishable from one the model was asked about and did not widen. A measurement that can fail that way without saying so is not a measurement, so the harness refuses the run and every run's report counts the `discovery.*` records that were sealed.

**Nothing is sealed for a refusal here.** A derivation record is one model exchange, and no exchange happened, so `investigation_budget_exhausted` is not among the reasons a record can carry.

[`model::REPAIRS`]: ../../../crates/cbr-provider/src/model.rs

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

Five behaviours are already recorded in [STACK §8.1](../readiness/STACK.md) and are stated by the owner independently of any call CBR has made. M4 designs for them:

1. **`response_format` and `json_schema` are silently ignored** — HTTP 200 with free-form prose. So **CBR parses and validates every model output**, and a schema is a thing CBR checks rather than a thing the provider guarantees.

   **Observed live, in run 3 (2026-09-23).** On both pilot questions `MiniMax-M2.7-highspeed` answered discovery's choice step in prose, and each cost the step its one repair — 4,827 and 8,922 tokens — after which it answered in the shape asked. `MiniMax-M3` answered all six of its steps in shape. A **fenced** answer has been unwrapped without a repair since m4f, and `j1-m27hs`'s terms step was one; **prose is not unwrapped and should not be**, because finding an answer inside prose would be CBR guessing which part of it was the answer. The repair is the designed outcome working, and it is priced in the worst case of §3.
2. **`tool_choice: "required"` is silently ignored** while `"none"` is honoured. (Confirmed: the Responses API documents `none` and `auto` only, so a call cannot be demanded there at all.) A text response where a tool call was demanded is therefore an **ordinary outcome to repair**, not a transport error. Repair spends real tokens, so the repair budget is bounded and debited from the same envelope.
3. **Reasoning spends the output budget, and on the M2.x models it cannot be turned off** (observed, calibration run 2). `max_output_tokens` must therefore cover **reasoning plus the answer**, and the service reports **no breakdown** — there is no `output_tokens_details` — so CBR cannot learn the split by measuring.

   **The sizing rule, and why.** The generation budget is four times what the answer itself needs, never below 512 tokens, and a truncation is repaired once by **doubling** rather than by asking again in the same space. The multiplier is not an estimate of how much a model thinks; it follows from an asymmetry. **Billing is by tokens produced**, so a limit that is too large costs nothing that is not used, while a limit that is too small costs the whole call and returns nothing — which is exactly what run 2 did with sixteen tokens. Generosity is the cheap error here and parsimony the expensive one. m4e measures what is actually used and replaces the multiplier with a figure.

   **`status: incomplete` with no text is a first-class outcome**: recorded with its usage, counted against the bounded repair, and ending as the item's typed unmet reason when the repair is spent. m4b had it as *not* repairable, reasoning that asking again under the same limit gives the same answer — true, and beside the point, because the repair asks again with a larger one.

   **For m4e:** run model-assisted selection on `MiniMax-M3`, whose documentation says reasoning is off by default, **beside** `MiniMax-M2.7-highspeed`, and compare tokens, latency and outcome. Two models on the same question is a comparison the owner will want and neither model alone provides.

4. **`MiniMax-M3` embeds `<think>…</think>` in `message.content`** unless `reasoning_split` is set. Reasoning text reaching a sealed derivation record as if it were output would be a correctness problem, so the split is set and the parser refuses content that still carries the marker.
5. **There is no `data: [DONE]` sentinel**, so **M4 does not stream**; whole responses only.

The rule under all four: **nothing downstream trusts a shape the model was only asked for.** Invalid output is a **recorded failure with a bounded retry**, and the failure is in the derivation record — not swallowed, not retried until it looks right.

## 5. Repository text is untrusted input

**The selection half was built at m4c and the discovery half at m4e. This section is now what exists.**

A file in a repository can carry text addressed to a model. CBR reads repositories it does not own — brian2 is a third party's, and every future one will be somebody's — so this is not hypothetical, and a model that acts on such text is a model doing what the repository said rather than what the request asked.

The rule is structural rather than a matter of prompting:

- **Model-assisted selection chooses only among candidate ids CBR offered**, which is a **closed set** built by the deterministic path: the spans retrieval found, the anchors, the eligible claims. The model returns ids from that set and nothing else.
- **An id outside the set is invalid output**: recorded as such and repaired within the bounded budget of §4, or dropped. It is never resolved, never looked up, never treated as a hint.
- **The model never introduces a path, a span, a citation or a label.** Every one of those comes from the deterministic path, which is the same property that makes a packet reproducible and its citations resolvable to exact bytes. **Relaxed for one tool at M5, knowingly:** by the owner's decision of 2026-09-24, M5's `read` tool may name any path in the view, and [M5 READINESS §4](../m5/READINESS.md#what-read-may-name-and-the-controls-that-replace-the-closed-set) states the controls that replace the closed set. Everything else here holds, including that a citation and a label are never the model's.
- **Prose the model writes enters a packet only as a section labelled `inferred`**, citing the inputs it was derived from, and **never `binding`**. `binding` is an authority's act, and nothing a model produces can be one — the same rule that already forbids a source file being promoted to `binding` in M3's J1.

**Negative control 5:** a repository file is planted that instructs the model to select something outside the candidate set, to mark a claim `binding`, or to reveal content from outside the view. The packet is unchanged outside the closed set: no id that was not offered, no label the model chose, nothing from beyond the view. **Mutant: ids accepted without the closed-set check**, killed by that control.

This is the least surprising part of the design and the easiest to erode later, so it is written down before there is any code to erode.

### Model-assisted discovery

**Designed 2026-09-21, built at m4e. This section is now what exists**, with the two places the design moved marked as such.

[§1](#1-scope-and-the-promise) records why it is needed: selection chooses inside a file the request already named, so it cannot change what a packet finds. Discovery is where that changes, and discovery is also where the closed-set rule above stops being obviously enough — a model that may influence *what is looked for* is a model with more reach than one that picks among spans.

**Two steps, and the property is the same one as above: the model never names a file, a span or a label.** The design said three; the third was never a step, it was the recording, and it applies to both.

1. **It may propose search terms, and nothing else.** The model is shown the task and the deterministic discovery's top candidates, and may answer with **a small bounded number of additional search terms**. A term is **untrusted query input, never a path**: CBR runs it through its own lexical index, inside the requesting session's view, exactly as it runs the words of the task itself. There is no branch where a model's answer is resolved as a path, and so nothing for a term shaped like one to reach.

2. **The union is a larger closed set, and the choice is still an id.** Deterministic discovery's candidates and the term-driven ones form one candidate set; the model chooses **a bounded number of ids** from it, and those become discovered sections carrying **the labels, ranks and citations the deterministic path already gave them**. The model supplies no label, no `binding`, no citation and no span. **Negative control 5 is re-run against both steps**: a planted file that instructs the model is run past the term step and the choice step, and the packet is unchanged outside the closed set.

**Every step is a recorded derivation**, so the packet replays offline exactly as a selection does: the terms proposed and the ids chosen are in the sealed record, and a rebuild that finds no record for a step says so rather than deciding for itself.

#### Three bounds, because a term is the one thing CBR did not compose

Every other input to a model call is built by CBR out of its own candidates. A term is text the model wrote that CBR then **acts on**, so it is bounded three ways before it is used, each of them the typed reason `model_answer_over_bound`:

- **by count** — at most **4**, in the schema and again in the parser, because a schema is a request and not a guarantee;
- **by length** — at most **48 bytes** each;
- **by characters** — alphanumeric, and the separators a path or a glob is made of (`_ - . / * :`). **Not a space.**

**The character bound is what distinguishes a term from a sentence**, and that is the point of it: a planted file instructing a model produces prose, and prose has spaces in it. So an instruction arriving where a term was asked for fails the bound rather than being searched for. The separators are *permitted* so that a term shaped like a path is **normalised** rather than refused — `src/queue.rs` is the words `src queue rs`, `**/*.py` is `py` — because words is all a term is ever used as.

**Nothing is trimmed to fit.** Five terms where four were asked for is the typed reason, not the first four: silently taking part of an answer would be CBR deciding which part of the model's answer to act on, which is the closed-set mistake in the other direction.

#### What the search does with a term, and why that is the whole safety argument

`discovery::words` is `lexical::query_terms` **and nothing else** — the same tokeniser the request's own words go through, producing the same alphanumeric tokens, which go into the same parameterised `MATCH` expression where every element is one of those tokens quoted. There is no expression a term can write, no path it can name and no second query language with its own bugs. What comes back is a span CBR found, at a path CBR resolved, in a repository the grant admitted.

#### The candidate set is what the packet could publish

**Both halves of the union are built under the packet's own per-path cap**, `compiler::DISCOVERED_PER_PATH`, and the live run is why. Until m4f the question's half was the raw top of the ranking while the cap was applied at publish, so a file that out-ranked the rest filled the set: J1's twenty candidates held thirteen spans of one file and **no span of the file the deterministic packet cites**. The model was asked to choose a packet out of a set that could not contain the packet CBR would otherwise have published, and the nine ids it chose were capped to five on the way out.

**An offer the packet cannot honour is not an offer, and a fact never offered cannot be kept.** `j1-m3` returned code only for exactly this reason, and its score measures the candidate set rather than the model.

A consequence worth stating, because it changed behaviour elsewhere: a capped union is smaller, so on a small view it can fall to or below `DISCOVERED_SPANS` — at which point `worth_choosing` correctly declines to ask a question that could not change the answer, and the union stands. That is the rule working, not the cap failing.

#### The reservation, which the first version did not have and needed

**The ordinary reading takes a reserved share of the candidate set and no more**: at most **10** of the **20** candidates offered come from the question's own words, and at most **6** are claims.

Without a reservation the step is pointless, and this is not a hypothetical: the first version had none, and on a repository where the ordinary reading returns more candidates than the set can hold it filled every slot, so a proposed term could contribute nothing. That is exactly backwards for the question this step exists for — brian2's, where the ordinary reading returned plenty of confident near-misses and missed the answer. A test caught it.

The reserved share is therefore *up to* `FROM_QUESTION` rather than exactly it — the question contributes as many spans as its own capped reading has. What makes it meaningful is the floor: **the model is offered at least as many of the question's spans as the packet would publish**, which one test states as a count and another as which files they come from.

#### What a failed step leaves, which is not the same for the two

- **The terms step fails:** the deterministic reading stands, unchanged.
- **The choice fails:** the deterministic reading stands, **not the union**. The two steps are one flow — the terms step proposes where to look and the choice is what decides that any of it belongs in a packet — so carrying term-driven spans by rank alone would carry them on the strength of a suggestion nothing acted on, and BM25 scores from two different queries are not one ranking.
- **The choice is skipped because it could not change the answer:** the union stands, because carrying everything is exactly what the choice would have done.

In every case **the packet says a step did not happen**, as an omission naming the step with the reason `unavailable` — which is the protocol's own vocabulary for `context.packet.inspect` and is literally what it was. *Why* is the typed reason in the step's own derivation record, which is sealed whether the answer was usable or not.

#### Claims enter the candidate set here

The eligible claims are offered beside the spans, which is what makes m4d's readable-claims gate **load-bearing rather than merely correct**: a choice record's `offered` now holds claim text a job could read, so a reader who cannot read the claim must not be served the record. Both arms are in `derivation_claims.rs`. The reviewer's mutant — sealing `readable_under(view, &[])` — now leaks through a real candidate set rather than through two empty lists.

**Anchors are not offered.** They are not ranked candidates: an anchor is where a name in the request is defined and used, and choosing among them would be choosing which definition a name means, which tags cannot say ([§8](#8-bounded-runtime) and the m3b evaluator). They stay deterministic.

#### What the suite does not reach

Two gaps, recorded rather than described as covered.

- ~~**The choice that would decide nothing is never skipped end to end.**~~ **Closed at m4f.** It became reachable when the candidate set started being built under the per-path cap: a capped union of a narrow view falls below what the packet publishes, so everything offered is going in anyway. `a_choice_that_could_not_change_the_answer_is_not_asked` runs the whole of it — the terms step widens, the choice is not asked, the union stands, and the packet declares no step unavailable, because none failed.
- **A span candidate id is a position among the spans that were kept**, and a mutant taking it from the input position survives. The two differ only when a blob in the offered list cannot be read, which the indexer's own size skip makes unreachable from a request. It is written here as a survivor rather than claimed equivalent.

#### What it cannot do, stated before anybody hopes otherwise

A term the model proposes **still has to occur in the repository**. This makes CBR look in places its own reading of the task did not suggest; it does not make CBR find something that is not written down, and it cannot answer a question whose answer is absent from the text. If brian2's question fails again after this, that is the answer it deserves rather than a bug in the step.

### What a candidate shows a model, and where it is cut

**Added at m5-arith, 2026-09-26, accepted by the owner the same day** ([ADR 001 question 17](../../decisions/001-standalone-v0.1-scope-and-stack.md), accepted). It changes a context budget, so AGENTS.md asks for the comparison and the failure cases below.

**The rule.** Every step that offers a candidate — one item's selection, and discovery's terms and choice — frames it through one function, `selection::shown`. It shows the candidate's text within `CANDIDATE_TEXT_BYTES`, **8,192**, and its path within `CANDIDATE_PATH_BYTES`, **1,024**, each measured as a request body carries it. A field past its bound is shown as its longest prefix that fits beside the marker `[cut]`, then the marker. The cut falls between characters (`wire::request::carried_within`), so it never splits an escape. A field within its bound is shown whole, byte for byte as before.

**Why those two numbers.** A span is capped at retrieval's `span_bytes`, 4,096 raw bytes, and 8,192 is twice that, pinned to it by a test. So a span is cut only when a body carries it in more than twice its raw bytes. Text whose every character is carried in at most two bytes — quotation marks, backslashes, newlines, tabs, two-byte UTF-8 — never is, with one exception: a clip inside a character leaves a replacement character, carried in three bytes, which can tip a span of nothing but two-byte characters over by a byte. Text dense in the other C0 controls, each carried in six bytes, is what the cut is for. The protocol bounds a selection path at 1,024 code points, so a path the protocol admits, whose every character a body carries in one byte, is never cut. **Discovery's paths come from the repository tree**, which nothing but the file system bounds, and a path of control characters there is cut like text.

**What it changes, and what it does not.** Only what the model is shown. The model answers with an id, and the id names the whole candidate: `derivation::offered` digests the full text rather than what was shown of it, a chosen span is cited and published as the repository's bytes, and replay reads the same records. So packets, citations, derivation records and replay are unchanged.

**The comparison, made with no model call.** It surveys the trees the live runs used, plus what those runs sent.

| Survey | Covered | Result |
|---|---|---|
| `scripts/escape_scan.py`, at the commits m4e's manifests pinned for J1 and after | cbr at `a6dc450` (12,110 spans), `9ee22d0` (12,169), `2f8c666` (13,830) and `651c86e` (14,149) | Widest span 5,150 carried bytes; **none past 8,192**; no span holds a six-byte escape; longest path 135 carried bytes, none past 1,024. Past 4,096: 38, 38, 53 and 66 spans. |
| The same tree scan, by the planning agents with an equivalent scanner, counts only | brian2 at `4960df7` (7,467 spans) and Knowscroll at `3e8991e` (2,191), the pilot trees | None past 8,192 or 1,024. 4 of Knowscroll's spans past 4,096. |
| Every 4,096-byte window, by the planning agents | the same repositories | The widest in each carried in 5,346, 4,972 and 4,646 bytes |
| The bodies m4e's runs 1 to 3 sent, by the planning agents, counts only, no sealed question printed | 31 discovery bodies, 409 candidates | Largest candidate 3,526 carried bytes; longest path 83 |

**No span or path of any surveyed tree is cut at 8,192 and 1,024**: 35,657 spans across cbr at two commits, brian2 and Knowscroll, as the planning agents counted them. The scanner of this change reproduces their cbr figures exactly. **The unified frame is byte-identical to the one it replaced** for every text that fits: a differential compared the two over 272,525 windows of 4,096 bytes of the working tree, and every window of every tracked file was identical; the 843 that differed were all in three untracked Python bytecode files (in the stage's scratch, not committed). Every fake-model journey and the m4e and J2 dry runs pass unchanged.

**The failure cases, in both directions:**

1. **A span dense in C0 controls is shown cut, with the marker.** The model sees less of it, and its id still resolves to the whole span. `crates/cbr-cli/tests/model_discovery.rs`'s `an_escape_dense_repository_is_asked_its_choice_within_the_published_flow` holds it end to end: at `651c86e` that fixture's choice was refused at 402,821 tokens, over the per-request 250,000, and the packet declared the choice unavailable. Now it is asked, at 142,541 of the 193,548 the table allows; 16 of its 20 candidates are shown cut, 2 of them at the path, the ordinary frames are byte-identical, and every cited span carries its full bytes.
2. **A path past 1,024 carried bytes is shown cut.** Ids stay distinct, because an id is CBR's `d1` or `c1`, never the path.
3. **In the other direction**, an escape-dense repository whose choice the ceiling used to refuse is now asked, so more calls are made, within the per-request and per-job ceilings.
4. **`CANDIDATE_TEXT_BYTES` follows `span_bytes`.** A test pins the one to twice the other, so a wider span cannot leave the cut behind.
5. **A text that ends in `[cut]` looks cut when it is not.** The model answers with an id, so nothing follows from it.

**Rejected alternatives.** A cut at 4,096 text bytes and 256 path bytes would give smaller figures — about 264,564 for the flow in the Responses dialect, derived by the planning agents and computed by no test — but would cut 38 to 66 of cbr's spans at the commits scanned and 4 of Knowscroll's, all of them ordinary text. No cut at all leaves the arithmetic uncomputable, since a body carries a span in up to six times its bytes, and leaves escape-dense repositories unanswerable at the per-request ceiling, as failure case 1 shows.

**What `escape_scan.py` does not establish** is in [VERIFICATION](../../VERIFICATION.md#the-model-runtime): it counts spans a tree can offer, not spans any question was shown.

## 6. Recording and replay

**Built in m4d. This section is now what exists**, with the two places the design moved marked as such.

**Redaction happens at the recording boundary**, not afterwards. A provider response can carry a third party's live credential — that is a known failure mode on one of the pilot repositories, recorded in [RELEASE-SCOPE §5](../readiness/RELEASE-SCOPE.md), not a precaution. Redaction therefore runs between the transport and the store, so an unredacted body never reaches disk, and the test is that the store contains no match rather than that the log looks clean.

**Every exchange a request takes an answer from is sealed as an evidence artifact**, holding the model id, the admission path — CBR's local bound alone, or the provider's count — the token usage, the latency, the candidate set that was offered, and the id that was chosen. Sealed in the tick's own batch with the atomicity every other record gets: the object is published and verified from disk before the row naming it is committed, so a crash leaves an object no row names and never a half-written derivation.

**The usage is the whole question's, attempt by attempt** — changed at m4h, after live run 3. A repair is a whole call with its own charge, and a record that carried only the attempt that ended the question said 4,827 tokens for a step the ledger charged 5,222 and 4,827. Each attempt is now kept as the ledger settled it, so a record's `usage.tokens` equals its question's ledger rows by construction, and `usage.attempts` keeps them one by one. The format is `cbr-model-derivation/3`. **A `/2` record still replays**: a rebuild reads neither the format nor the usage, and a test rebuilds a packet from a `/2` record sealed at its own digest. What a `/2` record cannot say is what its repairs cost; its ledger always could.

**A derivation record holds no repository text. This is a change from the design above**, which said it would hold excerpts and claim text. What was shown is identified by path, line range and the digest of the text; the bytes are the source artifact the packet already cites, sealed once and shared. So a store that keeps derivations for ever keeps no second copy of anybody's repository, and the retention question below has a much smaller answer than it would have had.

**Taking the answer is what seals it.** A call whose answer is never taken — the request was cancelled, or its job ended — seals nothing, because the compile that would have read it never ran. What it **spent** stays in the ledger and must: a call that went out was charged, and forgetting it would overspend a quota shared with the owner's other tools. The test holds a call after it has sent, cancels the request, and only then releases it.

**Readable only under the job's own view and readable claims.** The readable set is sealed on the artifact record and checked at **three doors** — `evidence.fetch`, `evidence.inspect` and `evidence.query`'s listing — *after* step 6 has decided the caller may read artifacts at all; only derivations carry the member, so everything sealed before m4d is read exactly as it was. A listing hands back the same descriptor `inspect` would, so it hides the row and counts it as filtered; hiding is not an error. The answer is `permission_denied` rather than `not_found`, because the caller is authorised for the subject and what they lack is the content behind it. The same check gates **replay**, so a narrow job cannot read a wider one's answers from the inside. **Gate, as asked for:** the m3c byte-identity assertion applied to derivation records — the packet a reader outside the job's view receives is byte-identical to the one they would have received in a store where no model was ever called. Nothing today puts a derivation into a packet; the assertion is what will notice if something later does.

**A packet built with a model is reproducible from the retained records without calling the model again**, as [INTERNALS §5](../../spec/INTERNALS.md) requires. `--replay-model` answers every question from a retained record, charges nothing, seals nothing and holds a transport that **panics if it is ever reached** — contained by the work pool as a typed failure, so the worst case is a failure with a name and never a call an offline rebuild promised not to make. A question nothing retained an answer for is the item's own typed reason, `model_answer_not_retained`, and **never a quiet fall back to BM25's first**: that is the difference between a rebuild and a rerun.

**A retained answer is found by the digest of its whole question** — the model, the task, the selector, and every candidate in the order it was offered, each identified by its path, its lines and the digest of the text shown. The answer is an id out of a closed set, so reusing it for a different set would be choosing from a list the model never saw. A file edited since the call is a different question and finds nothing.

**Every covered record for the question is read, and they must agree.** A model is not a function: two calls can ask one question and be told different things, and **m4e reruns the same questions live**, so a store holding both is the ordinary state rather than a corner case. The records are stored under a digest that covers the instant each was made, so answering from *the first one found* would make the rebuilt packet depend on a hash of a timestamp — the same history producing either of two packets, which the reviewer measured at five runs apiece. So:

- **They agree:** that is the answer, whatever it is.
- **They disagree:** the item is unmet with `model_answer_ambiguous`. The rebuild declines rather than choosing.
- **A retained failure beside a retained choice is a disagreement**, deliberately. Preferring the usable one would be the rebuild deciding which of two histories to reproduce — improving on the past rather than replaying it — which is the same fault as taking the first match, with better manners. A reader who wants the successful call's packet can ask for it by its own request; a rebuild's job is to be a function of what was kept.
- **A record this build cannot read counts as its own answer**, `model_record_unreadable`, and so takes part in the agreement: a record nobody can read is no evidence that the others are right.

#### The consequence of the ambiguity rule, and the operator's remedy

**Added at m4e, because m4e is where it stops being hypothetical.** A model call that timed out, followed by a successful rerun of the same question, leaves two retained records that disagree — and from then on **that question is unreplayable for ever**. The rule is right and it has a cost, and the cost is not a corner case: it is what a second run of a pilot leaves behind, which is exactly what m4e does.

So **the m4e replay gate reports how many questions were ambiguous and why, and never silently skips them.** `scripts/m4e_run.py --ambiguity <data directory>` groups every sealed, unpurged record by the question it answers and names each question whose records differ, with both records and the answers that differ. A gate that folded these into "nothing retained" would hide the one failure an operator can actually act on. The gate **decides nothing** — the provider's own rebuild decides, and the packet comparison says whether it worked; the gate explains. A test pins the two together on one store, so the agreement rule cannot move in one and not the other.

**The remedy is a purge, and it is the owner's.** An ambiguous question becomes replayable again only once one of its records is gone:

- **What:** `evidence.purge` on the artifact holding **the failed call's record** — never the successful one. Purging the answer that worked would leave a history nobody made, which is the same fault as a rebuild preferring the usable record, arrived at by hand.
- **By whom:** a principal in `authority_principals`. Destroying evidence is an authority's act and no consumer's.
- **Under what right:** in a session that negotiated `evidence.retention_control`, which is not a feature a session gets by default — the point being that purging is never something that happens incidentally.
- **What it does not do:** it does not remove the ledger row. The call was charged and stays charged. Nor does it delete the object immediately: another sealed artifact may share the bytes, and collection is separate. What changes is that a rebuild stops reading it.

**Neither the harness nor CBR ever purges on its own.** The gate reports; the owner decides.

**A purged record is not read at all.** A purge is a deliberate act of destroying evidence, and `state` stays `sealed` through one — only the `purge` member and the availability change — so a check on the state alone does not notice. Its object can still be on disk until collection runs, and longer when another sealed artifact shares the bytes, so answering from one would serve what somebody ordered destroyed.

**The replay comparison is across two stores, not within one.** The design above said within one, because of the caveat below; the caveat is about **ingested** artifacts, and a model-assisted packet of this shape cites **source** artifacts, whose ids are the blob and are therefore the same in any store with the same tree. So the gate compares the digest of a packet rebuilt in one store against the digest of the same request answered live in another, which is the stronger statement. A packet citing an ingested artifact still is not byte-reproducible across stores: an ingested artifact's id embeds the instant it was ingested, and a claim's revision digest is taken over a record holding it ([JOURNEYS](../../verification/JOURNEYS.md#what-the-pilots-changed-and-what-changed-back)). That stays reported rather than assumed away.

### What is retained, and for how long

**Stated because a request body holds repository text.** The policy is M6's; what m4d owes is an accurate account of what exists now and a name for a policy to key on.

| What | Holds | Lives in | How long |
|---|---|---|---|
| The ledger row | Token counts. **No content at all.** | `model_ledger` | For ever. It is the spend record, and a forgotten spend overspends a shared quota. |
| The recorded exchange | The request as it went out and the response as it came back, **both through the redaction boundary**. **This is the only place a request body is kept, and a request body holds repository text.** | `model_calls` | For ever. Nothing purges it. |
| The derivation record | Model, admission, usage, latency, the candidate set by path, lines and digest, the id chosen — and **the task and the selector, as text**. Those are the requester's own words rather than the repository's, and they are kept because the question's digest is taken over them and a record that could not be re-derived from its own contents would not be auditable. **No repository text.** | An evidence artifact, retention class `model-derivation` | For ever by default, and subject to the evidence profile's holds and purge like any other artifact. A purged record is also no longer readable by a rebuild. |

**"For ever" is a statement rather than a solution.** CBR has no retention policy: nothing ages a row out, and the operator's remedy today is the data directory, which deleting deletes them with. M6 owns the policy; m4d owns giving it something to key on, which is the retention class and the fact that only one of the three rows holds repository text at all.

**What this changes about §7.** Nothing about the provider's side — that limit is unchanged. What is new is CBR's **own** side: a store that has made model calls holds redacted request bodies for every repository in the jobs' views. For a public repository that is public text on the owner's own disk. For a private repository it would be that repository's text in a second place, kept indefinitely, which is one more reason the bar in §7 stays exactly where it is.

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

**The repositories.** brian2 is CeCILL-licensed and public; Knowscroll-v2 is public — **and that was not true when this sentence was first written.** At the round-40 review the reviewer found it private on the GitHub API while this document and the harness both called it public; the owner has since made it public and the reviewer re-verified. The correction is not the sentence, which was restored to true by somebody else's action: it is that **a document cannot hold a fact about an account it does not control**, so the harness now asks the API in live mode and the reviewer asks again at authorisation time. Sending their text to a provider is sending public text, and the licence still governs what CBR may **commit** — digests, paths, spans, counts and costs only, which is unchanged. **Any private repository needs the owner's explicit word before a single byte of it is sent**, and CBR has no such word today.

## 8. Bounded runtime

**Model work leaves the preparation tick.** M3 measured the cost of doing long work inside it: the index build holds the tick throughout, **7.2s** on CBR's own 1,066 blobs, **12.8s** on brian2's 553 blobs and 5.3 MB, and **3.9s** on Knowscroll's 145, with every other job on that provider waiting it out. A model call is longer and less predictable than any of those, so M4 is where this is resolved — for the index build as well as for the model. m4c moves the build; [STALL](STALL.md) is the before-and-after measurement.

- **Deadlines** are the request's, already in the protocol, and a call that would outlast one is not started.
- **Cancellation** drops the request; a cancelled call leaves no partial derivation record.
- **A concurrency bound** caps calls in flight, because a shared quota plus unbounded concurrency is the overspend of §3 arriving by another route.
- **Failure is an item's unmet reason, never a hang.** A provider error, a timeout, a refusal at admission and an invalid output after bounded repair all end as a typed reason a consumer can read.

### What m4c built, and the two decisions inside it

**The work pool.** `work::Pool` bounds work in flight at two and answers every ask with a typed `Progress`; nothing blocks the caller, so the tick that asks is the tick that returns. Work beyond the bound is **deferred, not queued**, because by the next tick the request may have been cancelled or its deadline passed. A settled answer is **kept until its caller takes it**: a compile needing two calls asks across several ticks, and a released failure would be a retry nobody asked for.

**The deadline is converted once, not kept twice.** The pool measures monotonic time and the protocol measures instants. `clock::unix_of` converts the request's deadline at the moment the call is asked about; keeping a second deadline beside the protocol's would be two clocks disagreeing about one request.

**Cancellation is the job's, not the item's.** A compile that produced its script has read every answer it asked for, and it releases them together. The index build is not cancelled at all and says why: it writes its manifest as it goes, so cancelling it would leave exactly the partial record the rule forbids, and it has no single owner besides.

**What the model is asked.** Which of the spans BM25 already ranked, inside the one file the item named, to cite. It answers with one of a **closed set of ids CBR offered** — never a path, a line or a repository — so the worst any answer can do is choose a worse candidate from CBR's own list. An id that was not offered ends as the item's unmet reason, with no repair: the answer was well formed and wrong, and asking again would spend a shared quota on the same question.

**A request that authorised no investigation calls nothing**, and gets exactly the compiler M3 shipped. That is what makes the same binary the baseline m4e's journeys are scored against, rather than a second build nobody ran.

**One rule was loosened to test this, and only one.** Compiling is a production capability and the fake transport is a conformance control, so the two could never overlap and the call site could not be reached under `SIGKILL` at all. A conformance launch may now ask to compile, with `context.compile`. No fixture sets it, and a test holds what a launch that does not set it still does.

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

**The hard cap for the whole of m4e is 5,000,000 tokens** unless the owner raises it — a little over three times the estimate, because an estimate made before the first live call has ever run is not a measurement.

**It is enforced by `--model-run-ceiling`, given to each launch as the cap less what the launches before it spent.** An earlier draft of this section said the per-job ceiling enforced it. That was wrong, and the way it was wrong is worth keeping: `Ledger::run_spend` sums the store it was opened over, each of the six runs opens a new one, so the per-job 1,000,000 of §3 bounds *a run* and six of them bound 6,000,000. The cap is a property of the whole, so the number passed to each launch has to be a property of the whole too.

**Exceeding the estimate by more than half stops the run and is reported.** At 2,250,000 tokens the run halts and what was spent and on what is reported before anything continues. The stop is checked **before a run, against what that run could cost** — `spent` plus the run's own price, its flow and one selection question per want, 518,628 + wants × 167,753 (§3) — rather than after a run that had already crossed it. The worst the whole can then reach is that stop less the least a run is priced at, one want's 686,381, plus the per-job ceiling of the run that was started under it: **2,563,619 tokens**, under the cap with room the ledger does not depend on. It does not count what settlement can add over a reservation (§3), and nor does the figure below.

**Changed at m5-arith, 2026-09-26.** Until then the stop priced a run at one flow, `spent + 373,188`, and no selection question, though every run asks at least one; the whole was then **2,876,812 tokens**, the stop less 373,188 plus the per-job ceiling. That is still the bound on the three recorded runs, which that stop admitted, and they spent 141,012 against it. A run's price is the harness's stop and not a bound on one job: three wants price a run at 1,021,887, above the per-job ceiling, which is what bounds the job.

These are estimates, and the first thing m4e produces is the measurement that replaces them.

### The harness is code, reviewed before it is run

**Added at m4e.** The first live run is not a sequence of commands typed on the day: it is [`scripts/m4e_run.py`](../../../scripts/m4e_run.py), reviewed at a head, with a **dry-run mode that drives every stage against the fake transport** and is exercised by the suite (`m4e_harness.rs`). The reason is the one m4b already recorded about `--calibrate`: *if the first live call needs new plumbing, the first live call runs code nobody reviewed.*

Per run it launches a provider **over a data directory under `--out`**, registers the repository, submits one context request, polls until it settles, writes the packet where the reviewer can score it, reads the ledger, and then relaunches the same store with `--replay-model` for the replay gate above. The store is where the ledger and the records are, which is to say it is the evidence the report is a summary of; it is not a temporary directory a reboot empties, and nothing in the harness deletes one.

**Nor does reading it write to it** — changed at m4h. Until then the harness read the spend and the records over a read-write connection, and a read-write connection folds a store's log into its database and deletes the log when it closes. **Whether that changed any store of live runs 1 to 3 is not known**: a provider opens a store connection per session and closes it cleanly, so its store usually has no log by the time the harness reads it, and a store with no log is left byte-unchanged even by a read-write open. Measured on scratch stores: `mode=ro` reads a log but rewrites the shared-memory file, and on a store with no log creates both; `immutable=1` changes nothing and misses a log. So a store is read three ways, one for each state it is found in: with a log and a shared-memory file — a provider killed while a session held it — `mode=ro` with a read-only shared memory, which reads the log and writes nothing; with no log, the usual case, `immutable=1`, with nothing to miss; and with a log and no shared-memory file — `stop` landing inside the provider's own close, after SQLite unlinked the shared memory and before it deleted the log, which CI found — from a private copy of the database and its log, because every open in place either creates a shared-memory file or misses the log. Three tests hold the three states to a byte comparison of every file.

#### The visibility check, and what it deliberately is not

**It runs in live mode alone.** A dry run sends nothing off the machine, and **no test in the suite opens a socket to GitHub** — the call has no test, and this says so rather than leaving a reader to infer it from a suite that passes. What is tested is the parser, on canned bodies: a 200 saying public, a 200 saying private, a 404, a 403, a 301, a body that is not JSON, a body with no `private` member, and `"false"` as a string rather than the boolean.

**It is unauthenticated, and that is the point.** `GET https://api.github.com/repos/<owner>/<name>` with no token, no `gh`, no proxy taken from the environment, no `.netrc`, a timeout, and no redirect followed. A call carrying the owner's credential would answer a different question — *can they see it* rather than *can anyone* — and a private repository answers the first one yes. Two tests hold that: one reads the harness's own code, with its prose stripped by a parser rather than by a text search, and asserts that no name on the path can reach a credential.

**Only one answer admits a repository**: HTTP 200 carrying `"private"` exactly `false`. A 404 — which is also what a private repository returns to a caller with no credential — a 403, a rate limit, a timeout, an unreadable body and a missing member are each a refusal by name, before any provider launches. **Unknown belongs on the same side as private**, because the failure this exists to prevent is a third party's private text reaching a provider.

**It is checked again at authorisation time, by the reviewer.** A constant committed weeks earlier cannot know what an account did yesterday, and neither can a check made at the start of a run that then takes an hour.

**Every run's report says how many of its records were discovery's.** Two is the flow; one is a flow that stopped at the terms step; none is a run that never reached discovery. Without that number a baseline packet and a packet the model did not widen read alike, and the run that measured nothing would be the one nobody noticed.

**Every rule in this document that the harness can enforce is a refusal it makes before a provider is launched**, so a run that breaks one costs nothing:

| Refused | Because |
|---|---|
| A repository id outside `cbr`, `brian2`, `knowscroll` | §7: the owner's word covers those three, and all three are public. A private repository needs the owner's explicit word before a single byte of it is sent. |
| **A checkout whose `origin` is not that repository's** | An id is a label the manifest typed. Checked against the label alone, `{"id": "brian2", "path": <any checkout>}` was admitted — so the origin is read from the checkout, in both modes, before anything is launched. The owner's word covers repositories, not names. |
| **An origin the API does not say is public** — live mode only | Pinning an origin proves a checkout **is** the repository it claims to be. It does not prove that repository is public, which is a live fact about an account somebody else controls: at the round-40 review one of the three pinned origins was private on the API while this document called it public. Asked once per distinct origin, before any provider is launched. |
| **A checkout off the commit the manifest pins**, when it pins one | A pilot question was sealed against a tree. Answering it over a different one measures something else. |
| **An investigation budget the items would exhaust** | §3: items are asked first and a flow that cannot finish is not started, so such a run asks discovery nothing and says so nowhere. |
| A model outside the three of §2 | The registry admits three; a harness that could name a fourth would be a way round it. |
| An output directory inside this repository | A pilot's packet holds a third party's repository text. Only digests, paths, spans, counts and costs are committed. |
| `--live` without `--permit-model-network` | Having built the harness is not permission to use it. |
| A run ceiling above 5,000,000, or a stop above 2,250,000 | The cap and the stop above. Both can be lowered and neither raised; raising either is the owner's decision. |

**J1 revisited and both sealed pilot questions, each against `MiniMax-M2.7-highspeed` and `MiniMax-M3`** — the same question twice, so the difference is the model and nothing else. That is the six runs §3's arithmetic is against. The manifest naming them is the operator's own file, outside this repository, because the pilot questions are sealed and this session has never seen either.

#### What the manifest must say

Written out so that the owner's file can be written without reading the script, and so that the reviewer can check it against the refusals above rather than against an intention.

| Member | |
|---|---|
| `id` | a name for the run, unique in the file |
| `repository.id` | one of `cbr`, `brian2`, `knowscroll` |
| `repository.path` | a checkout whose **`origin` is that repository's**, which is what the id is checked against |
| `repository.commit` | optional; when it is there the checkout must be at it, because a pilot question was sealed against a tree |
| `model` | one of the three of §2 |
| `task` and `selector` | the question, as `cbr context` takes them |
| `wants` | the items, each `<id>=<want>` |
| `capacity` | the packet's byte capacity |
| `investigation` | at least `len(wants) + 2`, so discovery is reached |
| `dry_answers` | dry runs only: what the fake answers, in the order a compile asks |

**The run happened.** On 2026-09-22, once, authorised by the owner: six runs, 65,144 tokens, every item satisfied, no ambiguous question, the credential absent from every store, and every launch ceiling the cap less what the launches before it spent. The record — digests, repository commits, token counts, per-step outcomes and the reviewer's scores in the reviewer's words — is in [JOURNEYS](../../verification/JOURNEYS.md#the-m4e-live-run-2026-09-22-both-pilots-and-j1-with-a-model). What it found is in §5 and §3 above, and it is the point of having run it: **three of the four defects it exposed were invisible to a fake transport**, because a fake answers instantly, in the shape it was scripted with, at whatever length the script says.

**It ran twice more.** Run 2, on 2026-09-22 after m4f, stopped at its first run's replay stage on `authentication_failed`, 8,473 tokens in: m4f's own correction had moved the rebuild to the production configuration without moving the credential it presented. m4g made the two one decision ([JOURNEYS](../../verification/JOURNEYS.md#run-2-2026-09-22-aborted-at-its-first-run-and-what-the-fragment-still-showed)). **Run 3, on 2026-09-23 from `main` at `9ee22d0`, completed**: the same six runs, **67,395 tokens** in 200 seconds, every item satisfied, every flow sealing both of its discovery records, every replay reproducing its packet's sections with no difference and no ambiguous question, and every rebuild authenticating with the credential its provider issued. No step truncated and no count was made; two steps were repaired once each, both `MiniMax-M2.7-highspeed` answering the choice in prose (§4). The reviewer scored J1 a pass on both models, brian2 two of three on both, and Knowscroll three of three on both; the record, with the scores in the reviewer's words, is in [JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g).

**The stop is checked between runs, not inside one.** Halting mid-call would leave a charge nobody reconciled; halting between them leaves the ledger settled and lets the report say what was spent and on what.

**The report holds digests, paths, spans, counts and costs, and no repository text** — the licence rule for the pilots, asserted by a test rather than remembered, because the report is the thing most likely to be pasted somewhere.

### Negative controls, stated before the runs

1. **No model configured.** The same request produces today's packet, byte-identical to the golden digest. If it does not, M4 has changed the deterministic path while claiming not to.
2. **The model refuses or returns nothing usable.** The packet is still produced, from the deterministic selection, with the failure recorded as a derivation and the item's reason stating it. Degradation is honest, not silent.
3. **A claim outside the grant, with a model in the loop.** The packet is byte-identical to one prepared where the claim was never proposed — the m3c assertion, re-run with the model runtime present.
4. **A replayed transcript is labelled.** A run against recorded fixtures never appears in a record as a live run.
5. **A repository file instructs the model.** A planted file tells it to select something outside the candidate set, to mark a claim `binding`, or to reveal content from beyond the view. The packet is unchanged outside the closed set (§5). **Run against both discovery steps at m4e**, with the file really in the repository and really in the candidate set — a control that scripted the model without ever sending it the file would be testing the script. The scripted model then does what the file says: answers with a path, which is a term and so is tokenised into words that resolve to nothing; and chooses an id that was never offered, which selects nothing. Each is a typed unmet, a recorded derivation, and nothing widened. **One assertion inside it is vacuous and is kept anyway**: "nothing outside the view appears" is trivially true of a term, because a term is only ever a query run through the index over the view and there is no branch by which it could reach anything else. The guard is the absence of that branch, not the outcome of the search — which is exactly what a mutant would add, so the assertion is worth making and is not worth mistaking for evidence.

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
- **What here is evidence, and what is not.** This document was written before any call was made. The live sessions since are recorded where they happened — [CALIBRATION](CALIBRATION.md) for the two calibrations, [JOURNEYS](../../verification/JOURNEYS.md) for the three m4e runs — and model-assisted discovery's runs 1 and 3 are sealed, costed live transcripts. So the status line is now *"model-assisted derivation implemented; live-model evidence recorded"*. Every estimate above that no run replaced is still an estimate.
