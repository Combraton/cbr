# M4 close-out

The closing record of milestone M4, **the bounded model runtime** ([issue #21](https://github.com/Combraton/cbr/issues/21)), kept here so it survives outside GitHub. It follows [M1's](../m1/CLOSEOUT.md) and [M3's](../m3/CLOSEOUT.md) close-outs in shape: what the milestone promised, what it delivered, what was measured, and what it did not establish. M4 is the first milestone that spent the owner's quota and sent repository text to a third party, so it adds one section the others did not need: every live session, with what it cost.

## M4 complete

Merged in order, no squash, as merge commits, each pinned with `--match-head-commit` to a head whose CI was green and which the reviewer had seen:

- **readiness**, docs only: #22 as `9092439`, pinned to `bf4694a`
- **m4a**, the envelope: #23 as `7a7398d`, pinned to `081b6f0`
- **m4b**, the wire and the credential: #24 as `025ddfb`, pinned to `324b1cc`
- **the m4b follow-up**, with calibration run 1: #25 as `dabf033`, pinned to `4aaa31f`
- **m4c**, the bounded runtime: #26 as `4a690ae`, pinned to `f53da84`
- **m4d**, derivation records and replay: #27 as `7fa939c`, pinned to `8bc5698`
- **m4e**, model-assisted discovery and the live run as code: #28 as `91d08b1`, pinned to `8898172`
- **an origin's visibility**, asked at run time: #29 as `a6dc450`, pinned to `17620eb`
- **m4f**, what live run 1 found: #30 as `7edc199`, pinned to `a354e88`
- **m4g**, the credential follows the configuration: #31 as `9ee22d0`, pinned to `95a7277`
- **the close-out**, this pull request

Every merge was confirmed afterwards from `merged` and `merged_at`, never `merge_commit_sha`, and every merge commit's second parent is the reviewed head.

**By the owner's decision of 2026-09-23, issue #21 closes on the discovery family's sealed, costed live transcripts — live runs 1 and 3** — and Journey 2 moves to M5.

## What M4 promised, and what it delivered

The promises are [READINESS §1](READINESS.md#1-scope-and-the-promise)'s, issue #21's and [RELEASE-SCOPE §4](../readiness/RELEASE-SCOPE.md)'s M4 row. **Four of them moved to M5 by the owner's decision of 2026-09-23**: model-assisted artifact creation, the tool surface with its loop and checkpoints, MODEL-RUNTIME §6's runtime-selection tests, and Journey 2. They are rows here rather than omissions.

| Promised | Delivered |
|---|---|
| A provider abstraction over the granted provider, with admission before every call and a typed refusal | **Delivered.** MiniMax only, its host a constant rather than configuration. Three dialects over one transport: the Responses API as primary ([ADR 001 question 13](../../decisions/001-standalone-v0.1-scope-and-stack.md)), then chat-completions and Anthropic. The local byte bound admits alone, and the provider's count is asked for only where it could change a decision. Two counters, per-request, per-job and per-run ceilings, and a durable ledger whose reservation is written before the send. `budget_exhausted` is distinct from provider exhaustion, and neither is retried. The tripwire `bound_unsound` is held in the ledger. |
| The credential read only when a model is configured, never handed to a child and never logged | **Delivered.** The `security` tool at an absolute path, by the owner's reading of [ADR 001 question 12](../../decisions/001-standalone-v0.1-scope-and-stack.md). `launch::decide` is a pure function, tested over every combination, and `tests/credential_discipline.rs` reads the test sources. `scripts/debug_launch.sh` is the only hand launch. **No credential in any store after any live session**: checked in [CALIBRATION](CALIBRATION.md) for the calibrations, and by the reviewer for the m4e runs. |
| Redaction at the recording boundary | **Delivered as a type.** `Redacted` has one constructor and the store write takes only it, so redacting after the write is a compile error. |
| Model-assisted **selection** | **Delivered at m4c.** The model chooses among the spans BM25 ranked inside a file the request named, answering with an id from a closed set CBR offered. |
| Model-assisted **discovery** — added to M4 on 2026-09-21, because selection cannot change what a packet finds | **Delivered at m4e and corrected at m4f.** The model proposes search terms, bounded three ways and tokenised by the same function as the question. The union is built under the packet's own per-path cap, with a reserved share for the ordinary reading, and the model chooses ids from it. Negative control 5 runs against both steps with the planted file really in the candidate set. |
| Model-assisted **artifact creation** — a derivation that produces a claim or a projection | **Moved to M5 by the owner's decision of 2026-09-23.** M4's derivations are the records of selection and discovery, and none of them produces a claim or a projection. READINESS §1 had already given projections and the wider derivation families to M5. |
| A bounded runtime: model work out of the preparation tick, with deadlines, cancellation, a concurrency bound, and failure as an item's unmet reason | **Delivered at m4c.** The index build's stall on an unrelated job fell 65×, 49× and 23× on three repositories ([STALL](STALL.md)). |
| Derivation records sealed as evidence, and a packet rebuilt offline from them | **Delivered at m4d, with the design moved.** A record holds no repository text, only paths, line ranges and digests. The rebuild's transport panics if it is reached. Every covered record must agree, or the question is `model_answer_ambiguous`. The operator's remedy is a purge by an authority ([READINESS §6](READINESS.md#6-recording-and-replay)). |
| RELEASE-SCOPE §4's **bounded tool surface, the loop and checkpoints** | **Moved to M5 by the owner's decision of 2026-09-23.** Every model call in M4 is a direct call with a fixed question — the first row of [MODEL-RUNTIME §6](../../spec/MODEL-RUNTIME.md). There is no tool, no loop and no loop checkpoint. What exists of restart is the ledger's reservation and the six model rows of the crash matrix. |
| RELEASE-SCOPE §4's fixture column: **MODEL-RUNTIME §6's eight runtime-selection tests** | **Moved to M5 with the tool runtime, by the owner's decision of 2026-09-23.** Six of the eight are already exercised by M4's direct-call runtime, one of them **with a known failure**. The other two belong to a runtime with tools. Item by item [below](#the-runtime-selection-tests-of-model-runtime-section-6). |
| The first live run as code, then J1 revisited and both sealed pilot questions rerun against two models | **Delivered.** [`scripts/m4e_run.py`](../../../scripts/m4e_run.py) refuses every rule it can enforce before any provider launches, and its dry run is in the suite. It was run three times, and the reviewer scored runs 1 and 3. |
| Journey 2 end to end | **Moved to M5** by the owner's decision of 2026-09-23, as M5's first journey, with its negative controls unchanged. |
| One derivation family against a real model, with the transcript sealed and the cost recorded | **Met by model-assisted discovery**, in live runs 1 and 3. |

**One obligation moved with them.** [RELEASE-SCOPE §2](../readiness/RELEASE-SCOPE.md) gave M4 a test that CBR's own producer never names a derived artifact as an ancestry root. That obligation applies to a family whose output CBR's own producer cites as claim support, and CBR's producer cites no output of M4's that way, so nothing M4 built reaches it. The owner's decision of 2026-09-23 moved it to M5, carried with the first family whose output CBR's own producer cites as claim support.

## The runtime-selection tests of MODEL-RUNTIME section 6

[MODEL-RUNTIME §6](../../spec/MODEL-RUNTIME.md) lists eight things to test before a runtime is selected. RELEASE-SCOPE §4 put them in M4's fixture column; the owner's decision of 2026-09-23 moved them to M5 with the tool runtime. This is what M4's direct-call runtime already exercises of them, and what it does not.

| Item | Status | The test, or why not |
|---|---|---|
| Pre-call context control | **Exercised** by M4's direct-call runtime | The local bound refuses before anything is sent, and counts the whole body: `a_request_the_local_estimate_refuses_never_reaches_the_count_endpoint`, `the_bound_counts_the_whole_body_and_not_only_its_messages`. Nothing outside the request's view reaches a request body, asserted over the bytes sent: `nothing_outside_the_requests_view_is_in_a_request_body`. |
| Aggregate tool limits | **Moved to M5** with the tool runtime | M4 has no tools. Its aggregate limits are on tokens and questions — per request, per job, per run, and the investigation limit — which are the admission row above. |
| Cancellation | **Exercised** | `cancelling_a_request_frees_the_bound_its_call_was_holding`; `a_cancelled_request_seals_nothing_and_still_owes_what_it_spent`. |
| Error and usage fidelity | **Exercised, with a known failure — not a pass** | A failure keeps the usage the provider reported (`a_failure_that_reports_usage_settles_to_what_it_reported`, `an_error_status_is_a_failure_after_the_send_and_keeps_any_usage_it_reported`). Provider exhaustion is its own outcome (`provider_exhaustion_is_a_different_outcome_from_an_exhausted_envelope`). A repair is charged to the same ledger (`a_repair_is_charged_to_the_same_ledger_as_the_call_it_repairs`). **The known failure:** a repaired step's sealed derivation record carries only its last exchange's usage, and a step that ends unmet after a repair drops its earlier attempts the same way, because `model::Runtime::ask` builds its `Cost` from the attempt that ended the question. The ledger is right and the record is not. Found in run 3's record; **fixed after M4's close, at m4h**, with a test that a repaired step's record carries every attempt's usage and equals that question's ledger rows. |
| Model fallback policy | **Exercised**; the policy is **no fallback** | A failed call is the item's typed unmet reason, never another model and never BM25's first choice: `a_failed_call_reaches_the_caller_as_an_unmet_reason_and_never_hangs`. A failed discovery choice leaves the deterministic reading, and the packet declares the step `unavailable`: `a_choice_that_was_never_offered_widens_nothing`. A rebuild with no record for a step says so rather than deciding: `a_rebuild_with_no_record_for_a_step_says_so_rather_than_deciding_for_itself`. |
| Resource-discovery isolation | **Moved to M5** with the tool runtime | The item is about a runtime that finds resources for itself, and M4's models find nothing. Every byte a request carries is composed by CBR inside the view (the view test above), and a proposed term is tokenised, never resolved as a path (negative control 5). A model whose tools can read is the case this test exists for. |
| Exact output capture | **Exercised**, with the cost figure affected by the known failure above | Every exchange is recorded through the redaction boundary, so it is exact except for what redaction removes: `every_exchange_is_recorded_through_the_redaction_boundary`, on the calibration's path. Serving wraps its transport in the same `wire::record::Recording`. Every answer is sealed with its model, admission, cost and choice: `a_record_carries_the_model_the_admission_the_cost_and_the_choice`. |
| Restart without duplicate commits | **Exercised** for the direct-call runtime | The six model rows of the crash matrix, such as `a_kill_during_the_completion_reconciliation_counts_once`, leave every spend counted at least once and never zero. A record sealed twice is one artifact: `the_artifact_id_is_the_records_own_digest`. Restarting a loop from its checkpoint, and J4, are M5's. |

## Outcomes

**Conformance is unchanged across M4.** The expectation files under `conformance/expectations/` and the CI workflows are byte-identical between M3's close (`c726107`) and `9ee22d0`, and every merged head's `Checks` passed on Linux and macOS. So [M3's outcome table](../m3/CLOSEOUT.md#outcomes) stands as it was. The Rust suite went from **181 tests at M3's close to 569 passing and one ignored**, over 38 result lines. The ignored one is the stall harness, which is inert without a checkout to measure.

## Every live session, and what it cost

Five sessions called a model, each on the owner's word at the time and each run once. Nothing else has ever called one, and CI never has.

| Session | Date | Built from | Tokens charged by CBR's ledger | Outcome | Record |
|---|---|---|---:|---|---|
| Calibration run 1 | 2026-09-20 | `025ddfb` | 12,495 | **Stopped at its first call.** The counting endpoint refused a chat-completions body, so no count was obtained. The ledger settled at the estimate because the provider reported no usage; it almost certainly charged nothing. | [CALIBRATION](CALIBRATION.md#run-1-2026-09-20) |
| Calibration run 2 | 2026-09-21 | `dabf033` | 15,582 | **The measurement.** The local bound held on every file, loose by 3.40 to 4.29 times against the input. The provider priced only the completion, at 44 tokens; the other 15,538 were CBR settling unpriced counts at the figure it counted. | [CALIBRATION](CALIBRATION.md#run-2-2026-09-21) |
| m4e live run 1 | 2026-09-22 | `a6dc450` | 65,144 | Six runs completed. It exposed four defects, three of them invisible to a fake transport, all fixed at m4f. | [JOURNEYS](../../verification/JOURNEYS.md#the-m4e-live-run-2026-09-22-both-pilots-and-j1-with-a-model) |
| m4e live run 2 | 2026-09-22 | `7edc199` | 8,473 | **Stopped at its first replay** on `authentication_failed`. m4f's own correction had moved the rebuild's configuration without moving its credential; fixed at m4g. | [JOURNEYS](../../verification/JOURNEYS.md#run-2-2026-09-22-aborted-at-its-first-run-and-what-the-fragment-still-showed) |
| m4e live run 3 | 2026-09-23 | `9ee22d0` | 67,395 | Six runs completed in 200 seconds, with every replay identical and every rebuild authenticating. Two steps were repaired, and none truncated. | [JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g) |

**In all: 169,089 tokens charged**, of which the three m4e runs spent 141,012. That is under the 5,000,000 cap even taken together, against an estimate of 1,500,000 made before any call. Every m4e launch was given the cap less what the launches before it spent, and the stop was checked before each run against that run's computed worst case.

*Correction, 2026-09-26 (m5-arith).* That worst case was one discovery flow, 373,188, and left out the selection questions every run asks first. From m5-arith the stop prices a run at its flow and one selection question per want, 518,628 + wants × 167,753 ([READINESS §9](READINESS.md#the-first-live-run-has-a-cap-before-it-starts)). The runs above stay bounded by what that stop admitted, 2,876,812 in all, and spent 141,012 of it.

**Only the Responses dialect has answered a live completion.** Chat-completions reached the provider once, in calibration run 1's refused count, and the Anthropic dialect never has. `MiniMax-M2.7`, the third granted model, has never been called live.

## What the scores say, and what they do not

The reviewer scored runs 1 and 3 against oracles this session has never seen. Both sets of scores are in [JOURNEYS](../../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g), in the reviewer's words. In run 3:

- **J1 passed on both models.** The ADR that run 1 never offered was now offered and kept.
- **brian2 held two of three facts on both models.** The deterministic compiler had held one of three on both M3 runs. The fact that is still missing was never in the offered set, so this score measures discovery's ranking and not the model's choice.
- **Knowscroll held three of three on both models**, with no claims in the store.
- **`MiniMax-M3` matched `MiniMax-M2.7-highspeed` on every score**, for 25,250 tokens against 42,145, with no repairs.

These are two runs of three questions, and the pilots stay pilots. Neither measures a session; that is J6, at M7.

## Limits carried into M5

- **brian2's missing fact never entered the candidate set.** A term the model proposes still has to occur in the repository and still has to rank. This is an input to M5's readiness, stated generally rather than as this question, and nothing is tuned against it.
- **Knowscroll ran with no claims in the store in both live runs**, so decision memory with a model in the loop has not been run on a real repository.
- **A repaired step's record carries the usage of its last exchange only.** `model::Runtime::ask` builds its `Cost` from the attempt that ended the question. The ledger holds every charge and is right; the record, read on its own, leaves out the attempt it repaired. This was found while writing run 3's record, and **fixed after M4's close, at m4h**.
- **The harness opened the evidence store read-write** to read the spend, the records and the ambiguity report, and a read-write connection checkpoints a store that has a log when it closes. Whether that changed any store of runs 1 to 3 is not known: a provider closes its per-session connections cleanly, so a store usually has no log by the time it is read, and one with no log is left byte-unchanged. **Fixed after M4's close, at m4h**: it now reads without writing a byte.
- **J1's excerpt limit.** The ADR's excerpt covers the start of the question 11 row, while the sentences recording the move to `gix` sit about 600 bytes past the 2,048-byte cap. The citation resolves to them.
- **Provider-side retention cannot be refused.** It is accepted for public repositories only, and any private repository needs the owner's explicit word first ([READINESS §7](READINESS.md#7-what-may-be-sent)). Whether `service_tier: standard` is honoured, and what `store` means, cannot be observed from a response.
- **Nothing ages a derivation record or a recorded exchange out.** The retention policy is M6's ([READINESS §6](READINESS.md#what-is-retained-and-for-how-long)).
- **Selection's output budget is decided, not open**, as the reviewer ruled at round 47. Discovery's floor is 2,048 because a run measured the need for it.
- **Carried from M3:** a packet citing an ingested artifact is not byte-reproducible across stores, and Protocol 0.1 cannot cite a span of an artifact ([Combraton/protocol#16](https://github.com/Combraton/protocol/issues/16)).
- **Two CI timing flakes, both load-related:** `journey_one`'s 120-second wait and `socket.subscription-recheck-race-regression`'s 10-second one. Each fails on one of the push and pull-request pair for the same commit and passes on a re-run. They get their own pull request.

## Survivors and untestable gaps, as recorded

Every mutant, stage by stage, is in [STATE](../STATE.md). These are the ones that were not killed, and the paths no test reaches. None is counted as a kill.

| Stage | What survives or is unreached | Why |
|---|---|---|
| m4b | `url_to_send` not pinning what it built; the transport building its own URL | The URL is made of constants, so removing a check on it changes nothing observable. `PinnedUrl` makes bypassing it a compile error by one route and a visible rewrite by the other. |
| m4b | Either `Drop` body emptied (`Secret`, `Authorization`) | Nothing in safe Rust can observe a released heap buffer. What is killed is the guard doing nothing. |
| m4c | The key forgets the selector | Not reachable through `cbr`, which gives every item of a request the same selector. |
| m4c | The compile never releases what the job asked; a job that ended keeps its answers | Each only frees memory earlier or later. Neither is observable over the protocol, and the second is behaviourally equivalent. |
| m4c | A cancel releases before it is committed | No fault control makes `commit_context` fail. |
| m4c | An out-of-range choice read as the first candidate | Equivalent: the index is always in range while candidates are built one-for-one from spans. |
| m4e | A span candidate's id taken from its input position rather than its kept one | The two differ only for an unreadable blob, which the indexer's size skip makes unreachable. Recorded as a survivor, not claimed equivalent. |
| #29 | The live visibility call to `api.github.com` | It opens a socket to a third party, so it has no test; its parser is tested on nine canned bodies. |
| m4f | The replay gate's call site with a difference in hand | Both launches of a run are over one store, so a dry rebuild always reproduces the packet. The function it calls is unit-tested, and the wiring is held by reading. |
| m4g | The production side of the credential rule, end to end | Only a live run exercises it, and run 3 did. The decision is tested at the call `one_run` makes. |

## The rules M4 kept relearning, and what guards each now

As with M3's three instances, each rule had already been written down once, and the failure was applying it to the next case.

| The rule | Where it was not applied | What guards it now |
|---|---|---|
| **A bound needs a fixture that reaches it** — M3's golden-digest lesson | The work pool counted an unsettled slot for ever, and only a test that filled the bound found it (m4c). The union's cap of twenty was read by nothing because every fixture's union was eleven (m4e, round 40). | Tests whose purpose is to fill the bound; `bloom.md`, which only one term reaches; and assertions on the request body that actually went out |
| **Test the function the production path calls, at the call it makes** | A second function, written so a test needed no temporary directory, was what the credential test pointed at, and nothing in `one_run` called it (m4g, round 50) | One function decides a launch's credential, and it is tested over a real work directory for all four launches, both on what it returned and on what it left on disk |
| **"This cannot be tested here" is a claim to check** | A survivor recorded as unobservable in a dry run died to a source assertion (m4g) | The m4g mutant table, and this close-out's survivor table, which records the reason beside every entry |
| **A number that lives in two languages needs a test across the boundary** | The harness stopped against 363,966 while the Rust figure had become 373,188 (m4f to m4g) | A Rust test reads the Python constant and compares it with the computed figure |
| **A configuration and the credential that authenticates against it are one decision** | m4f moved the rebuild's configuration and left its credential behind, and run 2 stopped on it | `configuration()` returns the path and the body together, and the credential is read off the body |
| **A probe is a measuring instrument** | It recorded seven survivors it never observed (m4d), and it restored half a mutant (m4e) | A probe records `NO-RESULT` when no `test result` line comes back, backs each file up once before any edit, and restores on a signal |
| **An id in a manifest is a label; a pinned origin proves identity, not visibility** | Any checkout under a permitted name was admitted, and a pinned "public" repository was private on the API | The origin is read from the checkout. In live mode an unauthenticated API call is made per origin, and the reviewer checks again at authorisation time. |
| **Reading a report is not reading the evidence** | Both of this session's inferences from run 1's report were wrong | A report states what it says and labels an inference as one |

## What M4 does not establish

- **That a packet is useful.** Every score here is against a list of facts declared before a run, not a measurement of whether a session did better. That is J6, at M7, and its thresholds still have to be fixed before its confirmatory run.
- **That model-assisted discovery helps in general.** The evidence is two runs of three questions. In run 3 it recovered a fact for brian2 that lexical discovery had missed twice, and left another that was never offered.
- **Anything about a model producing a claim or a projection**, a tool loop, or background maintenance. Those are M5's, with J2 as M5's first journey.
- **Anything live about the chat-completions or Anthropic dialects**, or about `MiniMax-M2.7`.
