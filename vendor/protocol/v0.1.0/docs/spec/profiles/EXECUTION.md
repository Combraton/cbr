# Execution profile `execution/1` — release candidate

> **Status: Protocol 0.1 release candidate.** Accepted as milestone M3 (with corrections C1–C3); §13.1 and §13.2 accepted with M4, §13.3 with M5; the enforced feature dependency of §13.3 is the M6 resolution. Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC §5–§8, §12–§14](../SPEC.md). Task: [M3](../../work/release-0.1/M3.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: pio `docs/spec/SPEC.md`, `docs/spec/INTERNALS.md` and `docs/spec/STANDALONE-CLIENT.md` at `e65b7c0`; cbr `docs/spec/PREPARATION-AND-DELIVERY.md` at `3278393`.

An **executor** runs work in an existing agent harness and reports what actually happened. PIO is one executor; a third-party minimal executor must be able to implement this profile without PIO, Combraton or any private library (REL-7). A **caller** submits work and reads execution facts. The profile covers identity, admission, delivery proof, runtime state, results and completion receipts, cancellation, reconciliation, and timeouts, plus optional features for steering, native actions, controller leases, workspaces, usage, context bindings, discovery and output telemetry.

The profile reports **execution truth**, not project acceptance. A result, a zero exit or a delivered prompt never establishes that work is correct, adopted or understood (SPEC §5).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `execution/1` depends on `core/1` with features `core.events`, `core.capabilities` and `core.effects` (CORE §19, M3 draft). `core.grants` is optional; without it only authority principals operate.
- **Required Core features (owner decision, 2026-09-14).** `execution/1` requires `core.events`, `core.capabilities` and `core.effects`. This document is where that requirement is published. Negotiation enforces it:
  - **Required request.** If a required `execution/1` request lacks one of those Core features, negotiation is refused with `unsupported_profile`. `details.unsatisfied` lists one actionable item per missing feature: `{ "profile": "execution", "feature", "reason": "dependency_not_selected" }`.
  - **Optional request.** An optional `execution/1` request without them is not selected. The same items appear in `unselected`, and execution operations are then `profile_not_negotiated`.
  - **Manifest unchanged.** `core.describe` keeps its accepted shape: `depends_on` names only `core`.
  - **Machine-readable dependencies.** `core.feature_dependencies` (CORE §4.3, M6-Q1) reports these Core features as a `profile` entry for `execution/1`. A provider that supports `execution.claim_revalidation` also reports its dependency on `execution.context_revalidation` (§13.3) as a `feature` entry.
  - **Older participants.** A caller that does not request `execution/1` gets unchanged results. A provider without `execution/1` reports an optional request as `unknown_profile` in `unselected`, and refuses a required one as `unsupported_profile` (CMP-7).
- A provider advertises `execution/1` and the optional features it implements (§12). A caller lists the features it requires; an unsupported required feature is refused at negotiation (CORE §4).
- Every execution operation follows the Core command path (CORE §10): deduplication before authorization, authorization before capabilities, authority epoch and preconditions, then one owner transaction that commits the state change, its events and its effect records.

## 2. Identities

Identities are distinct and never reused for one another (PIO-I §2, SPEC §3).

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| `execution` subject `{ "kind": "execution.execution", "id" }` | One admitted unit of work. The caller chooses the ID and creates it with precondition revision 0. The kind `execution.execution` follows Core's `<profile>.<kind>` grammar and is used consistently in schemas, events, `kinds` filters, grant resources and fixtures; human-facing labels say "Execution" (owner decision, 2026-09-14). | A retry counter |
| `predecessor` | The execution this one retries or continues. A retry is always a new execution (EXE-1). | Permission to reuse the predecessor's identity, receipts or effects |
| `command_id` | Core command identity. Retransmission reads or completes the prior operation; it never runs work again. | A new attempt |
| `invocation_id` | One executor-visible external invocation inside an execution. Opaque harness-internal calls are outside it and declared as a coverage limit. | Whichever result was selected |
| `delivery_id` | One prompt, steering or context delivery episode. It is a Core effect ID (CORE §19). | A fresh ID per network retry |
| `native` | Harness session and turn references, where the harness exposes them. | Proof of delivery |
| `host` | `{ "id", "generation" }`: an owned host slot and its process generation. | A bare process ID |
| `completion_id` | One immutable result submission (§6). | Permission to overwrite a later attempt |
| `correlation` | Caller-owned opaque metadata, returned unchanged (EXE-20). | A Combraton workflow model |

## 3. Status axes

Execution state is six independent axes. Each axis changes only through an execution event carrying its **evidence class** and source (§9). Later observations append; they never rewrite an earlier observation (EXE-3). A provider MUST NOT manufacture earlier states to make progress look linear (SPEC §5).

| Axis | Values | Establishes |
|---|---|---|
| `admission` | `queued` (with `queue_reason`), `admitted`, `refused` (with `reason` and optional `alternative`) | The executor's capacity and policy decision (EXE-2) |
| `delivery` | `pending`, `acknowledged`, `delivered`, `not_delivered`, `failed_before_delivery`, `ambiguous` (§3.1) | The current determination of whether the brief reached the harness, with its evidence |
| `runtime` | `not_started`, `preparing`, `active`, `requires_action` (with `action_id` and `owner`), `quiescent`, `exited`, `unknown` | Current observed state (EXE-4) |
| `result` | `absent`, `partial`, `returned` | Output availability |
| `exit` | `{ "code" }`, `{ "signal" }`, `forced_termination`, `unavailable` | Process outcome, where meaningful |
| `evaluation` | `not_requested`, or an attributed external reference | A caller or verifier's assessment, never derived from `exit` or `result` (EXE-5) |

**Delivery proof classes** (EXE-3):

| Class | Meaning |
|---|---|
| `provider_ack_id` | The harness returned an acknowledgment identity for this delivery |
| `echo` | The harness echoed the submission; what an echo proves is defined by the adapter's capability predicate |
| `transport_only` | The transport accepted the bytes; the harness said nothing |
| `bytes_written` | Bytes were written to a terminal or stream. Never reported as `acknowledged` |

### 3.1 Delivery determinations

Owner decision, 2026-09-14. Each delivery has a **current determination** and a **history**. The execution's `delivery` axis is the current determination of its delivery.

| Determination | Meaning | Terminal |
|---|---|---|
| `pending` | Still awaiting evidence. Dispatch may not have begun, or it began and no establishing evidence has arrived. | no |
| `acknowledged` | A correlated harness acknowledgment for this delivery (`provider_ack_id`), or an `echo` whose meaning the adapter's evidenced capability contract establishes | yes |
| `delivered` | Later evidence, such as reconciliation, established that the brief reached the harness, without an acknowledgment | yes |
| `not_delivered` | Later evidence established that a dispatched brief did not reach the harness | yes |
| `failed_before_delivery` | Dispatch provably never began and the executor declared the delivery finished. It is never reopened; a later attempt is a new execution. | yes |
| `ambiguous` | Dispatch may have begun and the outcome is unknown, for example after a crash with the dispatch marker written, or when the evidence wait ends | no |

**Rules:**
- **Transport acceptance and terminal writes** (`transport_only`, `bytes_written`) alone establish nothing. They are recorded as evidence while the determination stays `pending`.
- **Waits end.** When the delivery wait ends without establishing evidence, `pending` becomes:
  - `ambiguous` if dispatch began;
  - `failed_before_delivery` if it provably did not.

  A delivery never stays `pending` after its wait ends (§8).
- **Resolution replaces the current view.** When later evidence resolves a delivery, the current determination changes to the resolved value with that evidence, for example `ambiguous` → `delivered`. The previous determination, with its evidence, is appended to `history`, which is never rewritten. The original `delivery.observed` event stays in the event stream. The axis does not stay `ambiguous` merely to preserve history.
- **Resolution never submits.** Reconciliation reads evidence; it never causes another submission.
- **One dispatch per delivery.** A delivery is dispatched at most once under its identity. Harness reports that arrive while it is still `pending` are evidence for that dispatch, never a new attempt.
- **Evidence classes.** Proof classes are defined above. Other evidence classes are named by the executor, but each MUST truthfully state the basis of its observation: recorded before dispatch, dispatch may have begun, never dispatched, a wait that ended, a reconciliation finding, and so on. The conformance executor's names are in §15.1.
- **What delivery does not establish.** `acknowledged` and `delivered` are distinct facts, and neither establishes comprehension, compliance or task success.
- **Public record.** `execution.inspect` shows each delivery as `{ delivery_id, delivery, evidence, proof_class?, determined_at, history: [ { delivery, evidence?, recorded_at } ] }`.
- **Effect status.** The delivery effect's status (CORE §19) follows the determination: `acknowledged` and `delivered` give `succeeded`; `not_delivered` and `failed_before_delivery` give `failed`; `ambiguous` gives `unknown`; `pending` gives `pending`.
- **Refused admission** records the execution with no delivery and no effect. Its `delivery` axis is `failed_before_delivery`, because nothing was or will be dispatched. Runtime is `not_started`; the other axes keep their initial values (`result: absent`, `exit: unavailable`, `evaluation: not_requested`). A queued execution's `delivery` is `pending` and its runtime `not_started`.

**Runtime before and after admission** (owner decision C1, 2026-09-14; a change to the draft vocabulary):
- `not_started`: no execution work has begun. Every execution starts here. Queued and refused executions stay here: a refused execution never dispatched, has no delivery and has recorded no execution effect.
- `preparing`: the executor admitted the execution and is preparing it; no harness observation has arrived yet. Admission moves runtime from `not_started` to `preparing`. The `execution.admission.changed` event that records admission carries the new `runtime`, so the change has its event.
- Once admitted, runtime never returns to `not_started`. Runtime `unknown` means the executor cannot observe work that may have begun; it is never used for work that has not started.

`requires_action` is a runtime condition, not a progress level. `cancel_requested` is not `cancelled` (§7).

## 4. Operations (base profile)

A minimal executor implements these. *Candidate* names.

| Operation | Kind | Semantics |
|---|---|---|
| `execution.submit` | command | Creates the execution (precondition revision 0). Payload: `brief` (digest and media type, or an inline bounded brief), `adapter` requirements, `restrictions` with required enforcement levels (§10), `timeouts` (§8), optional `predecessor`, `correlation`, `budget` (§12) and `context_bindings` (§13). Outcome: execution reference and the admission observation. |
| `execution.inspect` | query | Current axes, receipts, the obligations still waiting (`open` or `overdue`) and an events cursor for this execution. Closed obligations (`satisfied`, `aborted`) are read through `core.effects.get`. Bounded: large output and transcripts are never inlined (PIO-I §7). |
| `execution.cancel` | command | Records a cancellation request and its forwarding effect, and returns a **request receipt** (`cancel_requested`). The outcome is observed later as `cancelled`, `refused`, `not_supported` or `unknown` (EXE-8). |
| `execution.reconcile` | query | Given a `command_id` or `delivery_id`, returns the scoped observations the executor holds and any open obligations. It MUST NOT submit, resubmit or restart anything (EXE-10). |

Commands that act on an existing execution (`execution.cancel` and the feature commands in §11) get `not_found` when the execution does not exist.

**Watching** uses Core subscriptions with `kinds: ["execution.execution"]` (CORE §16). Executor observations appear no later than the provider's next request or idle re-check. Execution events are ordinary Core events; Core cursors, gaps, epochs, filtering and idle-lapse rules apply unchanged (EXE-7).

**Detaching is not cancellation** (EXE-22). Closing a session, disconnecting or restarting a client leaves executions running. A reconnecting caller queries or reconciles; it does not replay a remembered submit as new work.

## 5. Admission

- Admission is decided in the submit's owner transaction: command identity, grant, authority epoch, adapter capability requirements, restriction enforcement, workspace reservation (§11), queue policy and budget (§12).
- A **required restriction** whose enforcement level exceeds what the adapter advertises is `refused` with reason `enforcement_unavailable` and, where one exists, an actionable `alternative`. It is never admitted with weaker enforcement (EXE-2).
- **Missing capability information is `unknown`, and `unknown` is not `supported`** (CORE §17, EXE-11).
- If canonical persistence fails, no external invocation is authorized (PIO-I §3). The caller sees `unavailable`; nothing is bound.
- **Order.** Refusals come first: restriction enforcement, adapter predicates, then the budget (§12). An execution that is not refused is `queued` when a `required_before_start` context binding is unsatisfied (`queue_reason: "context_binding_unsatisfied"`, §13) or when the executor is at capacity (`queue_reason: "capacity"`). Otherwise it is `admitted`.
- **Queued work.** A queued execution has no delivery and no effect. An execution queued for capacity is admitted, oldest first, when capacity frees; the admission is a provider-origin `execution.admission.changed` event naming the new `delivery_id`. When its `queue` timeout passes, it is `refused` with reason `queue_timeout`.
- **Outcome shape.** `admitted` carries `delivery_id`; `queued` carries `queue_reason`; `refused` carries `reason` and, where one exists, `alternative`. `execution.inspect` repeats `queue_reason`, `reason` and `alternative`.

## 6. Completion receipts

- A completion is an immutable result record bound to execution, `invocation_id`, `host` generation, claim token, output descriptors and capture coverage (EXE-6).
- **Duplicate:** the same `completion_id` with the same digest returns the existing receipt.
- **Conflict:** the same `completion_id` with different content is rejected and retained as a conflict observation.
- **Old attempt:** a result from a superseded claim or host generation is retained under its original identity, but it cannot finalize the current execution or its successor.
- Receipt durability, publication elsewhere, effect reconciliation and external verification are separate facts. Publication never implies acceptance.
- **Submission is internal to each executor in Protocol 0.1** (owner decision Q1, 2026-09-14). No public `execution.complete` operation exists.
  - Every completion outcome MUST be observable through `execution.inspect` and `execution.completion.recorded` events: recorded, duplicate, conflict and superseded attempt.
  - An executor MUST NOT require a PIO-specific host, library or storage format to produce or expose receipts.
  - Conformance drives duplicates, conflicts and late old attempts through the scripted executor (§15).

## 7. Cancellation and fencing

- `execution.cancel` commits the request first, then forwards it through the harness's supported mechanism, then observes the outcome.
- **Forwarding effect.** The command records a new effect of kind `execution.cancel_forwarding` (the conformance executor names it `<execution>.cancel-<n>`, §15.1), retry class `idempotent_key` with the effect ID as its key, and an open obligation for the outcome. Its ID is in the acknowledgment's `effect_refs`. A forwarding attempt whose outcome is unknown is retried with the same key (CORE §19.3); if no attempt completes, the outcome is `unknown`.
- A lost cancel acknowledgment is recovered by retransmitting the same command, which returns the same receipt through Core deduplication (EXE-8).
- Escalation to process termination requires verified host ownership and generation; a process number alone is insufficient (PIO §8).
- Cancellation, a stopped process and reconciled external effects are separate observations. Outstanding external effects survive cancellation, timeout and abandonment as open obligations (CORE §19).
- Fences stop stale mediated commands and results from cooperating hosts. They cannot retract an effect already sent by an unrestricted process; that limitation is part of the contract.

### 7.1 Restart recovery

Owner decision, 2026-09-14. After a restart, an accepted execution whose delivery is still `pending` MAY be recovered and dispatched under the **same execution and delivery identity**, but only when the executor can establish that dispatch never began. That requires all of the following:

1. **Write-ahead dispatch marker.** Before any harness write, the executor durably records a dispatch marker for the delivery, bound to the host generation that will send. This is mandatory for every executor.
2. **Intact durable history.** The absence of a marker is proof only if the journal is intact. If the executor cannot vouch for journal continuity, for example after a restore or a new stream epoch (CORE §16.1), a missing marker is not proof: the delivery becomes `ambiguous`.
3. **Fencing.** Every recovery decision advances the host generation and appends `execution.host.changed`. A dispatcher whose generation is not the current one MUST NOT send, including one naming a generation the executor never issued; its attempt is recorded (`execution.dispatch.fenced`). A dispatcher from an older generation MUST NOT send afterwards, and its attempt is recorded (`execution.dispatch.fenced`).
4. **Revalidation before dispatch.** The executor revalidates at the recovery point, and dispatches only if all of these still hold:
   - no cancellation has been requested;
   - the submitter is still authorized (an authority, or an active, unexpired grant bound to a current epoch);
   - no delivery or execution deadline has passed;
   - applicable execution constraints still hold.
5. **Recorded decision.** Every recovery records `execution.recovery.decided` and an entry in the execution's `recovery` list:
   - `{ delivery_id, decision, reason, recorded_at }`;
   - `decision` is `dispatch_resumed`, `ambiguous` or `failed_before_delivery`;
   - `reason` is one of `provably_not_dispatched`, `dispatch_may_have_begun`, `journal_not_intact`, `cancelled`, `authorization_lost`, `deadline_passed` or `recovery_policy`.

**Outcomes:**
- **Marker present.** If dispatch might have begun, the delivery becomes `ambiguous` and is reconciled. It is never resent blindly.
- **Terminal.** A delivery already durably declared `failed_before_delivery` is never reopened.
- **Obligations.** A `failed_before_delivery` decision made because a deadline passed (`deadline_passed`) ends a wait, so the delivery's open obligations become `overdue`, with their events (CORE §19.4). A decision the executor bases on its own knowledge that nothing was dispatched (`cancelled`, `authorization_lost`, `recovery_policy`) is the expected observation and satisfies them.
- **Termination policy.** An executor may terminate provably undispatched work as a declared recovery policy (`recovery_policy`). That policy is allowed, not required by the Protocol.


## 8. Timeouts

Five distinct timeouts, each with its own `execution.timeout.passed` event when it passes (EXE-19). Each passes at most once.

| Timeout | Closes | Counted from | When it passes |
|---|---|---|---|
| `queue` | Waiting for admission | Submission | A still-queued execution is `refused` with reason `queue_timeout` |
| `delivery` | Waiting for delivery evidence | Admission | The evidence wait ends (below); open obligations of the delivery become `overdue` |
| `execution_deadline` | The permitted wall-clock execution window | Admission | Nothing else changes: runtime, effects and liability stay as observed |
| `inactivity` | Waiting for any runtime observation | The latest executor observation | Runtime and exit stay as observed |
| `reconciliation` | Waiting for an ambiguous delivery to be resolved | The moment delivery became `ambiguous` | Delivery stays `ambiguous`; open obligations become `overdue` |

When the `delivery` timeout passes, the evidence wait ends: a `pending` delivery becomes `ambiguous` if dispatch began, or `failed_before_delivery` if it provably did not (§3.1). A timeout closes a permitted wait or triggers a control action. It never proves that remote work ended, never refunds unresolved liability (§12), and never marks an effect as not having happened (CORE §19).

Each timeout applies only while its wait is open: `queue` while queued, `delivery` while delivery is `pending`, `execution_deadline` and `inactivity` while runtime is not `exited`, and `reconciliation` while delivery is `ambiguous`. A timeout passes at the first provider-clock instant at or after its start plus its seconds.

Timeouts are evaluated against the provider clock. Conformance controls that clock from the environment (§15).

## 9. Execution events

Execution events use Core event records (CORE §16.2) with subject `{ "kind": "execution.execution", "id" }`. *Candidate* types:

| Type | Payload |
|---|---|
| `execution.admission.changed` | `{ "admission", "runtime", "reason"?, "queue_reason"?, "delivery_id"? }`; `runtime` is the runtime after this change (`preparing` on admission, otherwise `not_started`); `delivery_id` when a queued execution is admitted |
| `execution.delivery.observed` | `{ "delivery_id", "delivery", "proof_class"?, "evidence" }`; `proof_class` only when the evidence is a proof class (§3) |
| `execution.delivery.reconciled` | `{ "delivery_id", "outcome": "delivered" \| "not_delivered" \| "unknown", "delivery", "evidence" }`; `delivery` is the resulting current determination |
| `execution.recovery.decided` | `{ "delivery_id", "decision", "reason", "host" }` (§7.1) |
| `execution.dispatch.fenced` | `{ "delivery_id", "generation", "current_generation" }`: an older dispatcher was refused (§7.1) |
| `execution.runtime.changed` | `{ "runtime", "action_id"?, "owner"? }` |
| `execution.result.changed` | `{ "result" }` |
| `execution.exit.observed` | `{ "exit" }`; also sets runtime `exited`. If an executor appends a separate `execution.runtime.changed` for that change, it follows this event (causal order). |
| `execution.completion.recorded` | `{ "completion_id", "digest", "invocation_id", "host", "status": "recorded" \| "duplicate" \| "conflict" \| "superseded_attempt" }` |
| `execution.cancel.requested` / `execution.cancel.observed` | `{ "receipt" }` / `{ "outcome" }` |
| `execution.timeout.passed` | `{ "timeout" }` |
| `execution.host.changed` | `{ "host" }`, whenever the host generation changes, including when recovery advances it (§7.1) |
| `core.effect.obligation.overdue` | `{ "effect", "obligation" }`, on the execution subject whose effect it is |
| `execution.steer.requested` | `{ "steer_id", "request": "recorded" \| "not_supported" }` (§11.1) |
| `execution.steer.delivery.observed` | `{ "steer_id", "delivery_id", "delivery", "evidence" }` |
| `execution.steer.behavior.observed` | `{ "steer_id", "evidence" }` |
| `execution.action.answered` | `{ "action_id", "response_effect" }` (§11.2) |
| `execution.controller.claimed` | `{ "epoch", "controller" }`, on subject `{ "kind": "execution.controller", "id": <host id> }` (§11.3) |
| `execution.workspace.checkpointed` | `{ "checkpoint_id", "complete" }` (§11.4) |
| `execution.usage.observed` | `{ "invocation_id", "basis", "measure"?, "amount"?, "recorded_at" }` (§12) |
| `execution.context.delivery.observed` | The delivery observation of §13 |
| `execution.context.checked` / `execution.transition.blocked` | A revalidation check, and a transition held for context (§13.1) |
| `execution.scheduling.changed` | `{ capacity: "held" \| "released", reason }`, where reason is a context block reason, `capacity`, an ending reason, or `resumed`: the capacity slot released while blocked before dispatch, reacquired, or given up when the execution ends before dispatch (§13.1) |
| `execution.transition.observed` | `{ "transition" }`: a named runtime transition that context bindings can depend on |
| `execution.output.lost` | A lost range (§14.1) |

**Causal order.** An event that records a decision, a passed timeout or an observation precedes the events it causes. For example, `execution.recovery.decided` precedes the delivery observation and host change it causes, and `execution.timeout.passed` precedes the overdue marking and determination it causes. Order among events with the same cause is otherwise unspecified (§15.1 fixes it for the conformance executor).

`origin` is `command` for events caused by a caller command, and `provider` for observations from the harness or host (CORE §16.2). Provider-origin events never carry a `command_id`.

## 10. Restrictions and enforcement

- Every restriction names a scope and a required **enforcement level**: `enforced` (OS or sandbox), `mediated` (tool boundary) or `cooperative` (instruction only). These are the same three levels as CORE §17 (SPEC §7).
- Adapter capability predicates reuse CORE §17 and add `adapter_version`, `harness_version`, `protocol_version`, `probed_at`, `limits` and `enforcement` (EXE-11).
- A worktree is not a sandbox. An executor MUST NOT claim `enforced` for a restriction that only instructions implement.

## 11. Optional features

| Feature | Operations and rules | Rows |
|---|---|---|
| `execution.steering` | `execution.steer` returns a request receipt. Recorded request, native delivery acknowledgment and later observed behavior are three separate facts. Without live steering, the refusal is explicit: `not_supported` with a supported alternative. Delivery never proves comprehension. | EXE-9 |
| `execution.actions` | `execution.respond_action` answers a native action request. `action_id` belongs to one execution; answering it in another is `not_found` and authorizes nothing. | EXE-14 |
| *(base rights)* | Under a grant, `execution.submit` and `execution.cancel` cover the execution subject; `execution.read` covers `execution.inspect`, `execution.reconcile`, execution events and the execution's effects through `core.effects.get`. | CORE-11, CMP-6 |
| `execution.controller` | `execution.controller.claim` takes the single mutating controller lease for a host. It advances the Core authority epoch of scope `execution.controller:<host id>`, and mutating execution commands carry that epoch. A stale controller is refused with `stale_authority_epoch`. Read-only observers are unaffected. | EXE-13 |
| `execution.workspaces` | Workspace lease descriptor: repository, base, writer identity, lease epoch, permitted paths and effects, cleanup policy. `execution.workspace.checkpoint` records coverage the executor probed itself, including dirty and untracked state. Agent-reported commit IDs are annotations, not receipts. | EXE-15 |
| `execution.usage` | Usage observations with a basis of `observed`, `estimated`, `unknown` or `enforced_bound`. Liability for an unknown outcome is unresolved and is not refunded by a timeout. A hard ceiling the adapter cannot enforce is refused at admission. | EXE-16 |
| `execution.context` | Context bindings at submit and delivery observations for packets and updates (§13). | EXE-17, EXE-18 |
| `execution.discovery` | Optional, executor-neutral (owner decision Q2). `execution.discovery.list` returns installations and endpoints with separate facts: detected, adapter recognized, version supported, authentication known or unknown, reachable, last verified. Detection is never offered as usable capability. Schemas plus positive and adversarial fixtures use scripted installations. Probing real installations is PIO work. | EXE-12 |
| `execution.output` | Output telemetry, spooled separately from semantic events (§14). | OBS-7, TRN-4 |
| `execution.continuation` | Native resume and fork where supported. Without native support, a continuation is labeled `fresh_continuation`, never `resumed`. | EXE-23 |

**Negotiation.** Each feature is an `execution/1` feature name. An operation of a feature that was not negotiated is refused at step 3 with `unsupported_required_feature` naming the feature. A submit payload member that belongs to a feature that was not negotiated (`workspace`, `budget`, `context_bindings`, `continuation`) is `invalid_envelope` at that member's path, as for Core's `grant` member.

**Rights.** Under a grant, each feature command needs the right named after its operation on the subject it acts on: `execution.steer`, `execution.respond_action` and `execution.workspace.checkpoint` on the execution; `execution.controller.claim` on the controller subject. `execution.discovery.list` needs right `execution.discovery.list` on `{ "kind": "execution.discovery", "id": "installations" }`. Submit and read rights do not cover them.

The shapes below are the Protocol 0.1 names.

### 11.1 Steering

- `execution.steer` (command) on an execution, payload `{ "message": { digest, media_type } }`.
- **Live steering.** The outcome is `{ steer_id, request: "recorded", delivery_id }`. The command records effect `delivery_id` of kind `execution.steering_delivery` (retry class `non_repeatable`) with an open evidence obligation.
- **Without live steering.** The outcome is `{ steer_id, request: "not_supported", alternative }`, with no effect. The request is still recorded.
- **Three facts.** `execution.inspect` lists `steering` entries: `{ steer_id, request, recorded_at, delivery_id?, delivery, proof_class?, evidence?, behavior, behavior_evidence?, alternative? }`.
  - `delivery` is `pending` until evidence arrives, `acknowledged` with a correlated acknowledgment, or `ambiguous` when the attempt's outcome is unknown.
  - `behavior` is `not_observed` until the executor observes behavior attributable to the message. An acknowledgment is not observed behavior, and neither proves comprehension.

### 11.2 Native actions

- A harness action request sets runtime `requires_action`. `execution.inspect` then carries `runtime_detail: { action_id, owner }`, and lists `actions`: `{ action_id, owner, state: "pending" | "answered", requested_at, answered_at?, response_effect? }`.
- `execution.respond_action` (command) on an execution, payload `{ action_id, response: { digest, media_type } }`.
  - The action ID belongs to that execution. An ID that is not a pending action of this execution, including one pending in another execution or already answered, is `not_found` and authorizes nothing.
  - The outcome is `{ action_id, state: "answered", response_effect }`. The command records a new effect of kind `execution.action_response` (`non_repeatable`), named in `response_effect`.
- A pending action survives an executor restart under the same ID (SCN-12).

### 11.3 Controller lease

- `execution.controller.claim` (command) on subject `{ "kind": "execution.controller", "id": <host id> }` with precondition revision equal to the current epoch. It advances the epoch by one and returns `{ epoch }`. A host ID the executor does not own is `not_found`.
- Once a host has a controller epoch, the mutating execution commands on its executions (`execution.submit`, `execution.cancel`, `execution.steer`, `execution.respond_action`, `execution.workspace.checkpoint`) carry `authority_epoch` (CORE §8):
  - lower than the current epoch, or absent: `stale_authority_epoch`, with `current_epoch` when the caller may read the controller subject;
  - higher: `unknown_authority_epoch`.
- A reconnecting controller claims again, which advances the epoch. Queries and subscriptions need no epoch.

### 11.4 Workspaces

- Submit member `workspace: { repository, base, permitted_paths?, permitted_effects?, cleanup: "retain" | "remove" }`.
- Admission records the lease `{ lease_id, repository, base, writer, lease_epoch, permitted_paths?, permitted_effects?, cleanup }`; `writer` is the submitting principal.
- `execution.workspace.checkpoint` (command) on an execution with a lease records what the executor probed itself: `{ checkpoint_id, lease_epoch, recorded_at, probed_at?, head?, coverage: { tracked, dirty, untracked }, complete, dirty_paths?, untracked_paths?, annotations }`.
  - Each coverage area is `probed` or `not_probed`. `complete` is true only when every area was probed. `head`, `dirty_paths` and `untracked_paths` appear only for probed areas.
  - A commit the agent reports is an annotation `{ kind: "agent_reported_commit", value, basis: "agent_report", recorded_at }`, never the checkpoint's `head`.
- An execution without a lease is `not_found` for checkpoints.

### 11.5 Discovery

- `execution.discovery.list` (query), payload `{}`, returns `installations`: `{ installation_id, harness, version?, detected, adapter_recognized, version_supported, authentication, reachable, last_verified?, usable }`.
- `adapter_recognized`, `version_supported` and `reachable` are `yes`, `no` or `unknown`; `authentication` is `authenticated`, `unauthenticated` or `unknown`.
- `usable` is true only when the installation is detected, every fact is positive and `last_verified` is present. Unknown is never reported as positive.

### 11.6 Continuation

- Submit member `continuation: { of, mode: "resume" | "fork" }`. The label is decided at admission and appears in the submit outcome and `execution.inspect`: `{ of, requested, label }`.
- `label` is `resumed` or `forked` only when the adapter predicate `adapter.resume` or `adapter.fork` is `supported`; otherwise `fresh_continuation`. A caller that needs native resume requires the predicate (§5).

## 12. Usage and liability

Usage is recorded once per invocation with its basis (§11, `execution.usage`). Admission considers settled consumption, active reservations and unresolved liability. A reservation is released only with durable evidence that the invocation was never dispatched and can no longer be dispatched under its identity; a crash, timeout or lease expiry is not such evidence (PIO-I §5).

- **Budget request.** Submit member `budget: { pool, ceiling: "hard" | "soft", amount? }` names a budget pool the executor defines. `amount` defaults to 1, in the pool's measure.
- **Admission.** An unknown pool is refused with `budget_unavailable`. A `hard` ceiling on a measure the adapter cannot enforce is refused with `enforcement_unavailable` and an alternative. If the pool's reserved and settled amounts plus this amount exceed its limit, the execution is refused with `budget_exhausted`.
- **Reservation.** `not_reserved` for refused work; `reserved` from admission or queueing; `settled` once every invocation's usage is `observed` or `enforced_bound`; `released` only when the delivery is `failed_before_delivery` or queued work is refused. Settled amounts still count against the pool.
- **Observations and liability.** `execution.inspect` carries `usage: { observations: [ { invocation_id, basis, measure?, amount?, recorded_at } ], liability, budget? }`.
  - `liability` is `none` with no observations; `resolved` when each invocation's latest basis is `observed` or `enforced_bound`; otherwise `unresolved`.
  - A passed timeout changes neither the reservation nor the liability.

## 13. Context bindings and delivery observations

The Context profile is M4. M3 defines only the binding and the delivery observation, so that an executor can enforce and report them.

- **Binding at submit:** packet or update reference, exact digest, obligation (`advisory`, `required_before_start`, `required_before_transition` with the transition named), and the authority that selected it.
- **Admission:** an execution whose `required_before_start` binding is unsatisfied is not admitted as ready. Advisory bindings add no startup barrier, and a missing advisory item stays visible as a gap (EXE-17).
- **Delivery observation:** exact digest, target execution and native session, boundary (initial prompt, supported turn steering, explicit context request), and outcome: `queued`, `delivered`, `acknowledged`, `late`, `unavailable` or `unknown`. A packet arriving after its dependent boundary is `late` (EXE-18). No observation proves comprehension.
- **Binding shape.** Submit member `context_bindings: [ { binding_id, packet: { ref, digest }, obligation, transition?, selected_by } ]`; `transition` is required for `required_before_transition`.
- **Binding state.** A binding is `satisfied` when the executor holds the packet with that exact reference and digest. Otherwise it is `gap` for an advisory binding and `unsatisfied` for a required one. `execution.inspect` carries `context: { bindings, deliveries }` with each binding's `state`.
- **Observation record.** `{ binding_id, digest, target: { execution, native_session? }, boundary, outcome, recorded_at }`.
  - `unavailable` when the adapter does not support the boundary.
  - `late` when a `required_before_transition` binding's packet arrives after that transition was observed (`execution.transition.observed`).
  - Otherwise the harness's report: `acknowledged`, `delivered`, `queued`, or `unknown` when the handoff's outcome is unknown.

### 13.1 Context revalidation (`execution.context_revalidation`)

Owner decision M4-Q4 (2026-09-15). A **negotiated, versioned extension** of the M3 binding: without this feature, bindings, states and inspect results are exactly as above. Its compatibility fixtures show an M3-level caller is unaffected.

- **Binding members.**
  - `packet` is a packet reference `{ packet, revision, artifact: { provider, artifact, digest } }` (CONTEXT §2).
  - `conditions` lists typed conditions copied from the packet's applicability: `{ condition_id, kind, ... expected }` with kinds `repository_tree`, `dirty_snapshot`, `environment_digest`, `authority_revision`.
  - `fetch` names the per-audience read grants the executor uses (CONTEXT §7): `{ context?: { provider, grant }, evidence: { provider, grant } }`. Without `fetch`, the executor holds only packets its host was given.
  - `request` names the context request the packet answers (`{ kind: "context.request", id }`), so the causal link from request to execution is kept (CTX-1).
  - `require_current` (boolean, default `false`) is the authority-selected requirement that the bound revision must still be the request's current revision. Without it the binding is **pinned**: a newer revision does not by itself invalidate it. `require_current: true` needs `fetch.context`, because currency is read there; without it the binding is `invalid_envelope` at `/payload/context_bindings/<i>/require_current`.
  - Submitting any of these members, or a packet reference, without the feature is `invalid_envelope` at that member.
- **Packet facts at each check.** When the binding names a context fetch grant, every check reads the bound revision's result facts at the context provider:
  - the implicit condition `packet.facts` is `match` when the facts were read and no required item of the bound revision was invalidated by a later correction (CONTEXT §8 `invalidated_items`), `mismatch` when one was, with `observed` `corrected: <item_id>[, <item_id>…]`, and `unavailable` when the context provider cannot be read (SCN-5, SCN-8). Pinning never bypasses a correction that invalidates a required item;
  - only when the binding has `require_current: true`, the implicit condition `packet.current` is `match` while the bound revision is current, `mismatch` once it is superseded, with `observed` `superseded by revision <n>`, and `unavailable` when the facts cannot be read;
  - a superseded revision without that requirement stays usable while its checks hold: supersession is information (`superseded_by`), not invalidity;
  - a context provider's reference that differs from the binding (SCN-9), or facts reporting a required item `unmet` (SCN-4), mean the packet is not held for this binding;
  - the executor never substitutes a newer revision: the binding keeps its exact packet reference, and only the authorized caller can bind other content, with a new submit.
- **What the executor checks.** Only conditions it can actually observe, against the basis it holds, for example its workspace tree. Each check result is `match`, `mismatch` or `unavailable`, with the observed value where there is one, and evidence. An unobservable condition is `unavailable`: never treated as fresh, and not necessarily stale. The executor decides nothing about semantic truth, does not accept project direction, and is not an applicability engine.
- **Binding revalidation state:** `current` (every condition matches and the exact packet bytes are held), `stale` (some condition mismatches), or `unknown` (nothing mismatches, but a condition is unavailable or the bytes are not held). Each binding carries it as `revalidation`, only under this feature: the state of the binding's latest check. Its M3 `state` stays: `satisfied` only while `current`, otherwise `gap` or `unsatisfied` by obligation.
- **Effect by obligation:**
  - `advisory`: follow the declared fallback and show the gap; a stale or unknown advisory binding never blocks.
  - `required_before_start`: block the start boundary until the binding is `current`. At admission the execution is `queued` with `queue_reason` `context_binding_stale` (a condition mismatches), `context_binding_unsatisfied` (the bytes are not held) or `context_binding_unknown` (otherwise). With several bindings not current, the reason is the first of stale, unsatisfied and unknown that applies. At dispatch an admitted execution is not dispatched, `context.blocked` is `{ boundary: "dispatch", binding_id, reason }` until the binding is current, and it releases its capacity slot (below).
  - `required_before_transition`: block **only** the named transition until the binding is `current`. Work before it continues. The executor records `execution.transition.blocked` with `{ boundary: "transition", binding_id, transition, reason }`, which `context.blocked` also shows.
- **Boundaries.** Revalidate at each relevant boundary: admission, dispatch of the initial brief (including after queueing or after restart recovery), and each named transition. An earlier check never guarantees a later boundary is current.
- **Records.** A check appends `{ binding_id, boundary, transition?, checked_at, observed_basis, held, fetch?, results: [ { condition_id, result, observed?, evidence } ], state }` to `context.checks` in `execution.inspect`, with event `execution.context.checked`. `fetch` gives the reason packet bytes could not be obtained. An executor re-evaluating a blocked boundary records a new check when its result differs from the last check of that binding at that boundary. Checks never rewrite the packet or earlier checks.
- **Compatibility (CMP-8).** A session that did not negotiate the feature sees the M3 shape of every binding, including bindings submitted by another session under the feature: no `revalidation`, `conditions`, `fetch`, `request`, `checks` or `blocked`; a packet reference shown as `{ ref: <packet id>, digest }`; and `context_binding_stale` or `context_binding_unknown` shown as `context_binding_unsatisfied`.
- **Fetch.** When the executor fetches the packet itself, it computes the digest of the bytes it actually assembled and compares it with the binding's immutable digest. A digest the provider advertises in its response is not evidence of the bytes received. Bytes with another digest leave the binding `unsatisfied` and are never delivered or held.
- **Capacity while blocked before dispatch.** An admitted execution blocked at the dispatch boundary by a required binding MUST NOT keep its scheduling capacity slot, which the context preparation may itself need (CTX-18).
  - **Release.** The execution keeps its identity, admission, delivery identity and pending delivery effect. It also keeps its budget reservation and unresolved liability (§12), its workspace lease and host ownership (§11.4, §7.1), and its controller epoch. Only the capacity slot is released. `execution.inspect` carries `scheduling: { capacity: "released", reason }`, and `execution.scheduling.changed` records each change. Work with a released slot does not count toward capacity.
  - **Ending instead of resuming.** A released execution was never dispatched. At its dispatch boundary the executor first applies the §7.1 revalidation. If cancellation was requested, the submitter is no longer authorized, or a delivery or execution deadline has passed, the delivery becomes `failed_before_delivery` (reason `cancelled`, `authorization_lost` or `deadline_passed`) and the execution is never dispatched. A passed delivery timeout also closes the wait under §8; either way the released execution ends before dispatch once, and `scheduling.reason` is `deadline_passed`. Obligations follow §7.1: a deadline leaves them `overdue`, and the other reasons satisfy them. The ending's evidence classes and event order are in §15.
  - **Resuming.** Otherwise the executor revalidates the context bindings at dispatch and then reacquires capacity. If capacity is full, it waits with `scheduling.reason: "capacity"`. Once capacity is acquired it records `scheduling: { capacity: "held" }`, with event reason `resumed`, and dispatches exactly once, under the same delivery identity, subject to the dispatch marker and host generation fencing of §7.1. Resumption never creates another delivery, and a delivery already dispatched or determined is never dispatched again.
  - `scheduling` appears once the slot first changes, and sessions without this feature never see it. When a slot frees, the order between queued work being admitted and released work resuming is unspecified (a coverage limit).

### 13.2 Evidence outputs (`execution.evidence_outputs`)

EXE-21, with EVIDENCE §11. A completion record may carry `outputs: [ { role, evidence: { provider, artifact, digest } } ]`. They name artifacts the executor sealed under its work binding; `execution.inspect` lists them. A reference authorizes nothing.

- Outputs are listed only for executions submitted in a session that negotiated the feature, and only after the evidence provider confirmed the seal. An output that could not be sealed is not listed.
- The reference executor seals each recorded completion's content as artifact `output.<execution>.<completion_id>`, with `work` naming the execution, under a grant bound to that work (EVIDENCE §10).
- **Origin.** A submit may carry `origin: { initiator, depth, call_budget }` for work a context job initiated (CONTEXT §4, CTX-19); `execution.inspect` preserves it.

### 13.3 Claim revalidation (`execution.claim_revalidation`)

Owner decision M5-Q7. A **negotiated extension** of §13.1 for packets that carry claims (CONTEXT §14). It requires `execution.context_revalidation`, and negotiation enforces that as a feature-triggered dependency (CORE §4.2): requested without it, `execution.claim_revalidation` is not selected (`dependency_not_selected`). Without the feature, bindings behave exactly as §13.1, and claim changes are not enforced by the executor: a caller that needs enforcement negotiates this feature as required.

- **Reading.** For a binding with `fetch.context` submitted under the feature, the executor reads packet facts in a session that negotiated `context.claims` at the context provider.
- **`packet.facts` gains two outcomes**, beside authority corrections:
  - `mismatch`, when `invalidated_items` names a required item of the bound revision with a `claim`, meaning its claim lost its permitted use or became invalid for the basis. `observed` is `claim invalidated: <item_id>[, <item_id>…]`. An authority correction keeps its `corrected: …` form, listed first when both apply.
  - `unavailable`, when `unverified_items` names a required item of the bound revision: the claim cannot be read or verified, or its applicability is no longer established.
- **Effect by obligation**, unchanged from §13.1:
  - an `advisory` binding records the check and proceeds with its gap automatically;
  - a `required_before_start` binding blocks admission or dispatch;
  - a `required_before_transition` binding blocks only its named transition.

  A stale result gives `context_binding_stale`, and an unavailable one gives `context_binding_unknown`.
- **Never substitution.** The executor keeps the exact packet reference. A newer claim revision (`lineage_revised`) or a newer packet revision is information, not invalidity, unless the binding has `require_current` (§13.1).
- **Compatibility.** Sessions that did not negotiate the feature see §13.1 check results, where claim-based outcomes do not occur.

## 14. Output telemetry and backpressure

- **Separate channel.** Output chunks travel in a telemetry channel separate from semantic events, read with `execution.output.read` using byte-offset cursors.
- **Explicit telemetry loss.** A lost range records `from` and `to` offsets, the byte count where known, the reason, and its effect on evidence coverage (OBS-7). Telemetry may be coalesced or discarded only with declared lost ranges.
- **Semantic events are never dropped from the stream.** Backpressure ends a *delivery connection*; it never skips an event (TRN-4). The rules are in CORE §16.5 "Backpressure" (proposed M3 draft):
  - memory and pending notification bytes are bounded per connection;
  - an ending notice `consumer_too_slow` is attempted only for consumers that negotiated `core.events.backpressure`, and only within a bounded write time;
  - the connection is then closed;
  - recovery is by reconnecting from the consumer's last durably processed cursor, with retention gaps reported explicitly.
- **Separate reporting.** The telemetry policy and the semantic-event policy are negotiated and reported separately.

### 14.1 Output read (`execution.output`)

- **Spool.** The executor keeps up to a declared number of output bytes per execution. Offsets count every byte the executor received, from 0. When the spool is full, the oldest bytes are discarded.
- `execution.output.read` (query), payload `{ execution, offset?, max_bytes? }`, returns `{ execution, offset, data_base64, next_offset, end_offset, lost_ranges, coverage, policy }`.
  - `offset` is where the returned data starts: the requested offset, or the oldest retained byte if that offset was discarded.
  - `data_base64` holds at most `max_bytes` bytes. `next_offset` follows them; `end_offset` is the total received.
  - `policy` is `{ spool_bytes, overflow: "discard_oldest" }`: the telemetry policy, reported separately from the semantic-event policy (CORE §16.5).
  - `coverage` is `incomplete` whenever any lost range exists.
- **Lost ranges.** Each is `{ from, to, bytes?, reason, coverage: "incomplete" }`.
  - `spool_limit`: bytes the executor received and discarded, with `from`, `to` and `bytes` always present. Adjacent discards are coalesced into one range; coalescing is declared by the range itself.
  - `harness_dropped` or `capture_unavailable`: bytes that never reached the spool, at offset `from` = `to`, with `bytes` only when the harness reported the count.
- **Loss is a semantic fact.** Every loss also appends `execution.output.lost` with that loss's range. Semantic events are never dropped or coalesced.
- Under a grant, reading output needs `execution.read` on the execution.

## 15. Conformance and test controls

Execution fixtures need harness behavior, faults and time that ordinary operations cannot produce on demand. All such control stays outside the production protocol, extending CORE §13.1 ([decision 007](../../decisions/007-execution-test-controls.md), accepted with refinements).

- **Scripted executor.**
  - The reference executor drives a scripted fake harness selected by launch configuration: acknowledge, echo only, write bytes only, crash after write, never exit, return late, refuse cancellation, request an action, exceed an output spool.
  - Step 6 vocabulary: `output` (`text`, or `repeat` and `bytes`), `output_lost` (`reason`, optional `bytes`), `runtime_burst` (that many runtime observations at once), and executor setting `output_spool_bytes`.
  - Step 5 vocabulary (launch-configuration schema `executor`): `wait_for` a steer, cancellation or answered action; `steer_deliver`, `steer_behavior`; `request_action`; `workspace` probe results and `agent_reports_commit`; `usage`; `context_delivery` and `transition`; `transport_errors`, which makes the next effect attempts end with unknown outcomes; `probe_status`, a read-class status probe. Executor settings: `capacity`, `host_id`, `budget_pools`, `context_packets`, `installations`, and adapter `steering`, `enforced_bounds` and `context_boundaries`.
  - The script vocabulary is normative **only for the conformance tests that use it** (owner decision Q4). A production executor may satisfy it through a test adapter; it does not need a production scripting engine.
  - Real harness adapters are PIO's work and PIO's evidence.
- **Controllable clock.**
  - Launch configuration can name a clock file that the runner replaces atomically. The provider takes its protocol-visible time from it: expiry, timeouts, deadlines, `recorded_at`.
  - Virtual time never moves backward. A missing or malformed file at start refuses the launch; during a run, the provider keeps its last good instant.
  - Real-time watchdogs in the runner and in any barrier stay independent of it, so a frozen clock cannot hang a suite.
- **Process faults.** The runner kills the provider at points the script or a barrier reaches, then restarts it over the same data directory.
- **Store faults.** Launch configuration can make the next owner transactions of an operation fail and roll back (`commit_unavailable`, answered `unavailable` with nothing bound), or commit and then answer `internal_error` (`response_internal_error`, outcome unknown to the caller). Participants declare `store.faults`.
- **Barriers, implementation-specific.**
  - A participant may declare named pause points. The runner waits for a point to be reached, synchronizes on explicit provider signals, and releases it. That gives a deterministic interleaving for concurrency regressions, with bounded failure handling instead of sleeps.
  - Fixtures that use barriers apply only to participants declaring those names. Others report `unsupported`, recorded as a coverage limit and never as a pass.

No execution operation, method name, event type or error is reserved for testing. A provider launched without a conformance launch configuration exposes none of these controls.

### 15.1 Conformance executor conventions

Normative only for an executor running under the conformance launch configuration (owner decision Q4, confirmed at M3 acceptance as C3). They fix identifiers, class names and orderings the contract leaves to executors, so fixtures can observe them. A production executor may choose differently within the public contract, and callers must not depend on these values.

Nothing here relaxes a requirement stated elsewhere. Required identity relationships, evidence meaning, authorization, durability, causal ordering (§9) and recovery rules (§7.1) bind every implementation. Where an entry below restates such a rule for the script, the rule's normative home is the section named.

- **Identifiers.**
  - Prompt delivery effect: `<execution>.delivery-1`, with obligation `<effect>.evidence`.
  - Steering: request `<execution>.steer-<n>`, delivery effect `<steer id>.delivery`, obligation `<effect>.evidence`.
  - Cancel forwarding: `<execution>.cancel-<n>`, obligation `<effect>.outcome`. Action response: `<execution>.response-<n>`, obligation `<effect>.evidence`. Status probe: `<execution>.probe-<n>`, obligation `<effect>.result`.
  - Checkpoint `<execution>.checkpoint-<n>`; workspace lease `<execution>.workspace`.
- **Effect kinds:** `execution.prompt_submission`, `execution.steering_delivery`, `execution.cancel_forwarding`, `execution.action_response`, `execution.status_probe`.
- **Evidence classes.** Sources are free text.

  | Observation | Class |
  |---|---|
  | Effect recorded in its owner transaction (`pending`) | `recorded_before_dispatch` |
  | Write-ahead dispatch marker written (`pending`, appended to the effect) | `dispatch_intent` |
  | Harness report for a dispatch | the proof class |
  | Attempt whose outcome is unknown | `transport_error` |
  | Recovery decision, on the delivery record | `recovery` |
  | Recovery decision, on the effect | `dispatch_uncertain` (ambiguous) or `never_dispatched` (failed before delivery) |
  | Delivery wait ended | `delivery_timeout` if dispatched, otherwise `delivery_timeout_before_dispatch` |
  | Released work ended before dispatch (§13.1) | `delivery_timeout_before_dispatch` on the delivery record and the effect when the delivery timeout passed; otherwise `scheduling` on the delivery record and `never_dispatched` on the effect |
  | Reconciliation finding | `reconciliation` |
  | Cancellation answered by the harness | `harness_response` |
  | Status probe answered | `harness_status` |
  | Behavior attributable to a steering message | `harness_observation` |
- **Script steps.**
  - Dispatch happens only at `deliver`, at `crash: after_write`, and at a `stale_dispatch` that is not fenced. A `stale_dispatch` naming any generation other than the current one is fenced (§7.1).
  - The first `deliver` is the one dispatch attempt. A later `deliver` while delivery is still `pending` is further evidence for the same dispatch, with no new attempt (§3.1). Once delivery is no longer `pending`, `deliver` and `crash` steps have no effect: a delivery is never dispatched twice, and later evidence for an ambiguous delivery is scripted with `reconcile_finds`.
  - `stall` stops the script without dispatching. `exit` also sets runtime `exited`.
  - When every requested action is answered, `wait_for: action` sets runtime `active` and sends each response, which the harness acknowledges (`provider_ack_id`; the response effect `succeeded`, its obligation satisfied).
  - `transport_errors` makes the next attempts end unknown. Retryable classes (`read`, `idempotent_key`) get at most three attempts; `non_repeatable` gets one.
  - Capacity is released when runtime is `exited` or delivery is `failed_before_delivery` or `not_delivered`. An executor may also release it when it observes a `cancelled` outcome.
  - A determination made because a wait ended leaves the delivery's obligations `overdue` (CORE §19.4); a determination made from evidence satisfies them.
- **Inspect members.** `steering` and `actions` appear only once they have an entry.
- **Event order.** The causal order in §9 binds everyone. Among events with the same cause, the conformance executor emits, at a delivery timeout, `execution.timeout.passed`, then `core.effect.obligation.overdue` for each open obligation, then `execution.delivery.observed`. At recovery it emits `execution.recovery.decided`, the delivery observation if any, then `execution.host.changed`. When released work ends before dispatch (§13.1), whatever the reason, it emits the cause first: `execution.timeout.passed` for the delivery timeout and the execution deadline, each if it passed (in that order; other timeouts, such as inactivity, do not end the wait and follow the ending), then `core.effect.obligation.overdue` for each open obligation when a deadline ended the wait, then `execution.delivery.observed`, and last `execution.scheduling.changed`, which records the ending. Cancellation and lost authorization have no event of their own at that point, so their ending starts with the delivery observation. This order is among one execution's events; events of other subjects may fall between them.

## 16. What execution does not establish

An execution fact is the executor's observation with its stated evidence. It does not establish:
- correctness, project acceptance or comprehension;
- that a cancelled execution's external effects stopped;
- that a timeout ended remote work;
- that usage without an enforced bound is complete;
- that an enforcement level holds beyond what the adapter actually enforces.
