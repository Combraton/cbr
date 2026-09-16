# Core profile `core/1` — release candidate

> **Status: Protocol 0.1 release candidate.** The command path was accepted with M1, §15–§18 with M2 (§18 by decision 006), §19 with M3, and §4.3 `core.feature_dependencies` is the M6 addition (owner decision M6-Q1). Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC](../SPEC.md). Plan: [release plan](../../work/release-0.1/PLAN.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md).

This document defines the Core command path: sessions, negotiation, command and query envelopes, the order of checks, idempotency, preconditions, authority epochs, acknowledgments and errors. Grants, events and subscriptions, capability snapshots and credentials are specified in §15–§18 (milestone M2). Effect reconciliation is Core too (§19, milestone M3).

Wire encoding, digests and transport are defined separately:

- [Canonical encoding and digests](../bindings/ENCODING.md)
- [Local stream binding](../bindings/STREAM.md)

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Terms

| Term | Meaning |
|---|---|
| Participant | Any program speaking this protocol. |
| Provider | The participant that owns the subjects an operation addresses, such as an executor or context service. |
| Caller | The participant sending an operation to a provider. One program can be a caller of one provider and a provider to others. |
| Principal | The identity the provider attributes to a connection, established by the transport binding. A message cannot assert its own principal. |
| Subject | An object owned by the provider, named by `kind` and `id`. |
| Revision | A provider-owned integer per subject. `0` means the subject does not exist. Every committed change increases it. It says nothing about any other subject or provider. |
| Command | An operation that may change provider state. Commands are idempotent under their command identity. |
| Query | An operation that reads state. A query is not recorded as an operation and has no command identity. |
| Operation | A named command or query, written `<profile>.<name>`, such as `core.negotiate`. |

## 2. Identities are distinct

| Identity | Scope | Must not be used as |
|---|---|---|
| Transport request ID (JSON-RPC `id`) | One connection; matches a response to its request | An idempotency key, a message ID or a durable reference |
| `message_id` | One transmission of one envelope; unique per sender | A command identity; a retransmission gets a new `message_id` |
| `command_id` | One intended change, chosen by the caller; unique within the caller principal's deduplication scope | A subject ID, an execution ID or a retry counter |
| Subject `kind` + `id` | One provider-owned object | Proof that an operation happened |
| `operation_ref` | The provider's durable record of an accepted command | A command ID supplied by the caller |

A retransmitted command keeps its `command_id` and content and gets a new `message_id` and transport request ID. A deliberately new attempt is a new command with a new `command_id` (CORE-1).

## 3. Sessions

A session is one authenticated connection under the transport binding.

1. **Before negotiation**, a provider MUST answer only `core.describe`, `core.feature_dependencies` (§4.3) and `core.negotiate` (and `core.authenticate`, §18). A socket session must also authenticate before `core.negotiate`. Any other known operation gets `negotiation_required`; an unknown one gets `method_not_found` (§10).
2. **`core.negotiate` succeeds at most once per session.** A second call gets `already_negotiated`. To change the negotiated set, open a new session.
3. **After negotiation**, an operation of a profile that was not selected gets `profile_not_negotiated`. This applies even when the provider supports that profile.
4. **Session state is not durable.** Closing a session neither cancels nor confirms anything in flight. Callers reconcile by command identity after reconnecting ([STREAM](../bindings/STREAM.md)).

## 4. Describe and negotiate

### 4.1 `core.describe` (query)

This returns the provider's manifest:

- `provider`: name and version.
- `profiles`: every supported profile, with its supported major versions, features and `depends_on` (the profiles it requires).
- `unsupported_profiles`: profiles the provider declares unsupported, each with `reason` `not_in_release` or `not_implemented`. For a 0.1 release this list includes at least `coordination` and `remote-trust`.
- `limits` (§9), the provider's receive limits.
- `dedupe_window` (§6.3). Its initial values are provider-chosen, so callers and fixtures read them rather than assume them.
- `unknown_extensions`: `preserve` or `drop` (§5.1).

A provider MUST NOT list a profile as supported unless it also supports every profile that profile depends on (REL-2).

### 4.2 `core.negotiate` (query)

**Request.** The caller identifies itself (`caller: { name, version }`) and gives `receive_limits: { max_frame_bytes }`, the largest frame it accepts from the provider (at least 1 MiB). It lists the profiles it wants. For each profile it gives:

- `majors`: acceptable major versions.
- `required`: whether the session is useless without this profile.
- `required_features`: features the caller relies on.
- `optional_features`: features the caller can use if present.

**Provider rules:**

- For each profile, select the highest major version both sides accept.
- Select the intersection of features.
- **Refuse the whole negotiation** if a `required` profile cannot be selected or a required feature is missing. Use `unsupported_version` when no common major exists, `unsupported_profile` when the profile is unknown or declared unsupported, and `unsupported_required_feature` for a missing feature. The error `details.unsatisfied` lists every unsatisfied item as `{ profile, feature?, reason }`, with reasons `unknown_profile`, `declared_unsupported`, `no_common_major`, `unknown_feature` or `dependency_not_selected`. When items fail for different reasons, the code is chosen in this order: `unsupported_profile`, then `unsupported_version`, then `unsupported_required_feature`.
- Optional profiles and features that cannot be selected are reported in `unselected` with a reason. They are not errors.
- Include `core` implicitly; a caller cannot deselect it. A `core` entry is always treated as required, whatever its `required` flag.
- An optional profile whose required feature is missing is not selected. Each missing feature is reported in `unselected` as `unknown_feature`; this is not a refusal.
- **Dependencies (§4.3)** are applied after selection:
  - **Profile-triggered.** A profile whose dependency is not selected is not selected. For each missing dependency the item is `{ profile, reason: "dependency_not_selected" }`, or `{ profile, feature: <missing dependency feature>, reason: "dependency_not_selected" }` for a missing feature. A required profile's items refuse the negotiation with `unsupported_profile`.
  - **Feature-triggered** (M6-Q1). A requested feature whose dependency is not selected is itself not selected, and the item is `{ profile, feature: <the requested feature>, reason: "dependency_not_selected" }`. When the feature was required in a required profile, the negotiation is refused with `unsupported_required_feature`. When it was required in an optional profile, the profile is not selected. When it was optional, the item is reported in `unselected`.
  - **Order.** Profile-triggered dependencies are applied first. Feature-triggered dependencies are then judged against the profiles and features still selected, and are applied again until nothing changes. A profile that was not selected contributes no feature items: when a required profile is refused for a missing Core feature, its feature-triggered items are not also listed.
  - **One item per cause.** An optional profile left unselected by a required feature's dependency is reported with that one item, and its other features are not listed.
  - A dependency is applied only when its dependent profile or feature was requested. A caller that requests neither sees no dependency items. The order of items within `unsatisfied` and `unselected` is not significant.
- A feature named under a profile it does not belong to is `unknown_feature` for the profile it was listed under.
- The same profile listed twice is `invalid_envelope`.
- `unsatisfied` lists only the items that caused the refusal.
- The result's `limits` are the provider's receive limits. A response that would exceed the caller's `receive_limits` is replaced by `internal_error`; the caller cannot tell whether a command it answers was bound.
- A refused negotiation leaves the session unnegotiated; the caller may negotiate again. `already_negotiated` applies only after a successful negotiation and is decided after steps 1–3 of §10.

**Result:** the selected profiles with major version and features, `unselected` items, effective limits, and the current deduplication window.

Unknown feature names in `optional_features` are ignored and reported as unselected. Unknown names in `required_features` are refused (CORE-3).

### 4.3 `core.feature_dependencies` (query, M6-Q1)

This reports, machine-readably, the in-session dependencies that negotiation enforces (matrix CMP-5). `core.describe` keeps its accepted shape.

- **Payload** `{}`. **Result:** `{ dependencies: [ entry ] }`, where each entry is:

  | Member | Meaning |
  |---|---|
  | `profile`, `major` | A dependent profile major the provider supports, as `core.describe` lists it |
  | `trigger` | `{ kind: "profile" }`: the dependency applies whenever that profile major is selected. `{ kind: "feature", feature }`: it applies only when that feature of the profile is selected. |
  | `requires` | A non-empty list of `{ profile, major, features }`. That profile major must be selected in the same session, together with every listed feature. An empty `features` list means the profile major alone is required. |

- **What is listed.** Exactly the dependencies negotiation enforces within one session (§4.2), and nothing else.
  - Every supported profile major other than `core` has one `profile` entry. It names `core/1` and the Core features that profile requires, possibly none.
  - A `feature` entry follows for each feature with an enforced dependency.
  - A protocol dependency on a separate provider is not a session dependency and is not listed. For example, Context cites Evidence artifacts, but the reader negotiates `evidence/1` wherever the reference points.
- **Order.** Profiles in `core.describe` order. Within a profile, the `profile` entry first, then `feature` entries in `core.describe` feature order.
- **When it may be called.** Before or after negotiation, beside `core.describe` (§3, §10 step 1). It belongs to base `core/1` and is not a feature. Authentication rules are unchanged: on a shared transport, it needs `core.authenticate` first (§18.2).
- **Read-only.** It creates no command record, appends no event, and neither negotiates nor changes session state.
- **Bounded.** The result depends only on the provider's manifest and fits the default 1 MiB frame.
- **Advisory.** Negotiation stays authoritative. A consumer still requests every dependency it relies on, and the provider enforces them whether or not the query was called.
- **Older providers** answer `method_not_found` (§10 step 1). The consumer then uses the dependencies stated in the profile documents it pinned and still requests them at negotiation. A fallback never drops or assumes a required feature.

## 5. Envelopes

A **command** is sent as the `params` of a transport request. All command envelope fields are listed here; the schema is `schemas/core/1/command.schema.json`.

| Field | Required | Meaning |
|---|---|---|
| `operation` | yes | Operation name. MUST equal the transport method name. |
| `message_id` | yes | Per-transmission identity. |
| `command_id` | yes | Command identity (§6). |
| `dedupe_generation` | yes | Deduplication generation under which the caller issued this command (§6.3). |
| `subject` | yes | `{ "kind": string, "id": string }` primary subject. |
| `preconditions` | yes | Array, possibly empty, of `{ "subject": …, "revision": integer ≥ 0 }` (§7). |
| `authority_epoch` | no | Epoch the caller acts under, when the operation's profile requires one (§8). |
| `requires` | yes | Array, possibly empty, of feature names or extension keys this command's meaning depends on (§5.1). |
| `correlation` | no | Opaque caller metadata object. Not interpreted; not part of the command digest. |
| `caused_by` | no | Array of references explaining why the command exists. Not part of the command digest. |
| `extensions` | no | Object keyed by namespaced extension keys (§5.1). |
| `grant` | no | ID of the grant the caller acts under (§15). Allowed only when feature `core.grants` is negotiated. Not part of the command digest. |
| `command_digest` | yes | Algorithm-qualified digest of the command intent ([ENCODING](../bindings/ENCODING.md)). |
| `payload` | yes | Operation-specific object. |

A **query** has `operation`, `message_id`, optional `requires`, optional `extensions`, optional `grant` (§15) and `payload`. It has no command identity, preconditions or digest.

### 5.1 Closed objects, `requires` and extensions

Envelope and payload objects are **closed**. A field that the negotiated profile version and features do not define makes the message invalid (`invalid_envelope`). An unknown field cannot be classified as safe to ignore, so it is treated as required semantics and refused (CORE-4).

Optional additions that a receiver may safely ignore go in `extensions`. Each key is a domain the extension author controls, a slash, and a name, such as `example.org/trace`.

- A feature name is `<profile>.<feature>`, such as `core.digest-sha512`, and never contains a slash.
- An extension key always contains exactly one slash.

That is how a `requires` entry is told apart.

- An extension is **optional** unless its key appears in `requires`. A receiver MAY ignore an optional extension it does not understand. When the operation defines stored content, the receiver MUST either store the extension unchanged with that content or declare in its manifest that it drops unknown extensions.
- An extension key or feature name listed in `requires` MUST be understood and negotiated. Otherwise the message is refused with `unsupported_required_feature`, before any state change.
- `requires` entries MUST be unique. An entry containing a slash MUST also be present in `extensions`. A violation of either rule is `invalid_envelope`. An empty `requires` means nothing beyond the negotiated profile versions is required.
- A caller MUST NOT send fields that belong to a feature the session did not select. Such a field in another operation's envelope, such as `grant` without `core.grants`, is an unknown field: `invalid_envelope` at step 2.
- A caller MUST NOT call an operation that belongs to an unselected feature. Its own params are validated against its schema at step 2 as usual, and the operation is then refused at step 3 with `unsupported_required_feature` naming the feature.

This mirrors the "critical" marking used by JWS `crit` and X.509 critical extensions. The decision record [002](../../decisions/002-schema-language-and-extensibility.md) gives the evidence and alternatives.

## 6. Command identity and idempotency

### 6.1 Command intent and digest

The **command intent** is the JSON object with exactly these members of the envelope:

- `operation`
- `subject`
- `preconditions`
- `requires`
- `payload`
- `extensions` — always present in the intent, as an object containing only the extensions whose keys are listed in `requires`; an empty object if there are none

`command_digest` is the digest of the intent under [ENCODING](../bindings/ENCODING.md). The provider recomputes it and refuses a mismatch with `digest_mismatch` (CORE-6).

`message_id`, `dedupe_generation`, `authority_epoch`, `correlation`, `caused_by` and optional extensions are excluded. A retransmission can change them without becoming a different command. For example, a caller that reconnects under a new authority epoch can retransmit the same command.

### 6.2 Deduplication scope and binding

The deduplication key is **(principal's deduplication scope, `command_id`)**. The same `command_id` from a different scope is a different command. A provider MUST NOT reveal that another scope used it (CORE-12, M2).

A command identity is **bound** when the provider durably accepts the command (§10). A command rejected before acceptance — invalid, stale precondition, stale epoch, unsupported feature — does not bind its identity. A later transmission is evaluated afresh and may get a different result if state has changed.

Once bound:

- A command with the same key and the **same** `command_digest` returns the stored acknowledgment and outcome unchanged, with `replay: true` (CORE-7).
- A command with the same key and a **different** `command_digest` is refused with `idempotency_conflict`. The error names the command ID only; it does not disclose the stored intent (CORE-7).

### 6.3 Deduplication generations

A provider cannot keep every command record forever. It also must never treat a forgotten command as new, because that could repeat a side effect (CORE-8). Generations make forgetting explicit without relying on clocks:

1. **Provider window.** The provider publishes a window `{ "oldest_retained": g_old, "current": g_cur }` in `core.describe` and `core.negotiate`. It MUST retain the records of every bound command issued under a generation ≥ `g_old`.
2. **Advancing.** The provider may advance `current` at any time. It may raise `oldest_retained` only by discarding records of generations below the new value.
3. **Issuing.** A caller issues a new command under the `current` generation it last observed. It persists the command, including the generation, before sending, so a retransmission after a crash carries the original generation.

**Lookup.** When a command arrives, with `g` its `dedupe_generation`:

| Condition | Result |
|---|---|
| `g > current` | `invalid_envelope`: a generation the provider never issued. |
| A record exists for the key | Rules of §6.2. |
| No record and `g < oldest_retained` | `dedupe_history_unavailable`. The provider cannot tell whether the command was accepted earlier. The caller must reconcile by inspecting the subject. It must not simply resend under a new command ID if a duplicate effect would be harmful. |
| No record and `oldest_retained ≤ g ≤ current` | New command; continue processing. |

A command journaled by a caller long ago but never sent also gets `dedupe_history_unavailable`. That is conservative on purpose: the provider cannot distinguish "never received" from "received and forgotten".

## 7. Preconditions and revisions

Each precondition entry names a subject and the revision the caller expects:

- `revision: 0` means the subject MUST NOT exist.
- `revision: n > 0` means the subject MUST exist at exactly revision `n`.

All preconditions of a command are checked together. Two entries naming the same subject are `invalid_envelope`. A subject kind the provider does not own is a subject that does not exist. If any fails, the command is refused with `precondition_failed` and **no** part of it takes effect. The error lists every failed entry. Where the principal may read the subject, the entry includes its current revision (CORE-9); otherwise `current` is omitted. An authority principal acting without a grant may read every subject. Under a grant, the principal may read the subjects that grant would show it in events (§16.6). On `core.grant.*` operations, whose `grant` field is not evaluated (§15.5), a principal that is not an authority may read only grants it holds or issued. Under a grant, a subject of a kind no profile defines is never readable.

Preconditions are checked **after** the idempotency lookup. A retransmitted create whose original succeeded therefore returns the original acknowledgment, even though the subject now exists and a fresh evaluation would fail.

A revision is meaningful only for its owning provider and subject. A caller MUST NOT use one provider's revision or event position as a precondition on another provider's subject.

## 8. Authority epochs

Some operations act under an authority that can be taken over, such as a controller lease or a grant epoch; their profiles define which. For those operations the provider tracks a current epoch per authority scope and checks `authority_epoch`:

| Condition | Result |
|---|---|
| Epoch absent where the operation requires one | `invalid_envelope`, unless the profile states otherwise. A profile whose epoch exists only once claimed may treat an absent epoch as epoch 0, so it is `stale_authority_epoch` once any epoch exists (EXECUTION §11.3). |
| Epoch lower than the current epoch | `stale_authority_epoch`. The error includes `current_epoch` only if the principal may read the scope's authority subject (§7 rules), such as `core-test.authority` for scope `core-test`. |
| Epoch higher than any the provider issued | `unknown_authority_epoch` |

The epoch check comes after the idempotency lookup. A retransmission of an already-accepted command still returns its stored result after an epoch change (CORE-10).

A grant can be bound to an authority scope's epoch; it stops authorizing when that epoch is superseded (§15.4). Leases in later profiles define their own epoch issuance.

## 9. Limits

The provider declares these limits in its manifest and negotiation result:

| Limit | Meaning |
|---|---|
| `max_frame_bytes` | Transport frame limit, enforced by the binding before JSON decoding. |
| `max_payload_bytes` | Canonical encoded size of `payload`. |
| `max_string_bytes` | Longest string value, in UTF-8 bytes. |
| `max_array_items` | Longest array. |
| `max_depth` | Deepest nesting of objects and arrays. |

Limits other than the frame limit are measured over the whole `params` object of a request, including `extensions` and `correlation`:

- **Depth.** `params` is depth 1; each nested object or array adds one; scalars add nothing.
- **Strings.** Every string, member names included, counts toward `max_string_bytes`.
- **Arrays.** Every array counts toward `max_array_items`.
- **Payload.** `max_payload_bytes` is the canonical encoded size of `payload`.
- **Binding fields.** JSON-RPC `id` and `method` are bounded by the binding, not by these limits.

A value exactly at a limit is within it. A message within the frame limit that exceeds another limit is refused with `limit_exceeded`, naming the limit. Profiles may define tighter per-field limits (CORE-5).

## 10. Command processing order

A provider MUST apply these steps in order. The first failing step determines the error. Steps 1–4 happen before any durable change.

| Step | Check | Error on failure |
|---|---|---|
| 1 | In this order, using the JSON-RPC `method`: operation known; session negotiated (unless the operation is `core.describe`, `core.feature_dependencies`, `core.negotiate` or `core.authenticate`); operation's profile selected | `method_not_found`, `negotiation_required`, `profile_not_negotiated` |
| 2 | In this order: limits (§9: depth, array items, string bytes, payload bytes); method equals `operation`; closed objects and types; envelope semantics (`requires` entries unique and extension keys present, precondition rules, known-algorithm digest length) | `limit_exceeded`, `invalid_envelope` |
| 3 | Every `requires` entry negotiated and understood, for queries as well as commands; if the operation belongs to a feature (such as `core.grants` or `core.events`), that feature is selected | `unsupported_required_feature` |
| 4 | Digest algorithm supported (`sha512` only when `core.digest-sha512` was negotiated); `command_digest` matches the recomputed digest. `details.expected` is the provider's recomputed digest under the caller's algorithm. | `unsupported_digest_algorithm`, `digest_mismatch` |
| 5 | Deduplication lookup (§6). `dedupe_generation > current` is checked first, even when a record exists. Records are filed under the command's own `dedupe_generation`. A retransmission under a different digest algorithm has a different `command_digest` and is `idempotency_conflict`. | `invalid_envelope`, `idempotency_conflict`, `dedupe_history_unavailable`; or return the stored result |
| 6 | Authorization (§15): the principal is an authority for the operation, or names a valid grant covering it | `invalid_envelope`, `permission_denied` |
| 7 | In this order: capabilities the operation depends on are `supported` (§17); authority epoch (§8); preconditions (§7) | `capability_unavailable`, `stale_authority_epoch`, `unknown_authority_epoch`, `precondition_failed` |
| 8 | In one owner transaction: commit the command record binding its identity, the state change, resulting events and effect records | `unavailable` if the provider cannot commit; nothing is bound |
| 9 | Return the acknowledgment and outcome | — |

**Why step 5 precedes step 6:** the stored result is returned only within the same principal's deduplication scope, and a replay performs no new mediated operation. A principal whose grant was revoked may therefore still replay its own already-bound command and receive the stored result. Every new command or query needs current authorization (§15.5).

A query applies steps 1–3, then authorization (step 6), then the operation's read rules. It creates no command record.

## 11. Acknowledgment and outcome

A successful command returns:

```json
{
  "acknowledgment": {
    "command_id": "cmd-7",
    "command_digest": "sha256:…",
    "operation_ref": "op-41",
    "subject": {"kind": "core-test.subject", "id": "s-1"},
    "revision": 1,
    "effect_refs": []
  },
  "outcome": {},
  "replay": false
}
```

`effect_refs` lists the IDs of effects the command durably recorded (§19, M3 draft), such as an execution's prompt submission. It does not imply that those effects occurred or succeeded.

**Compatibility (owner decision, 2026-09-14):**
- **Widening.** Earlier drafts allowed only an empty array here, so widening it is a real schema change.
- **Gating.** Effect references appear only for operations of a negotiated profile or feature that records effects, such as `execution/1` with `core.effects`. Every M1 and M2 operation, and every effect-free command, still returns `[]`.
- **Replay.** A replay returns the same original references.
- **Reading effects.** References resolve only through `core.effects.get`, under read authority on the effect's target. Another principal's effect and a nonexistent effect are refused alike. An acknowledgment means the provider durably recorded the command and its immediate state change. It does not mean any external effect happened, succeeded, was verified or was accepted by anyone. Each profile defines what its outcome establishes.

A replay returns an `acknowledgment` and `outcome` byte-for-byte equal under canonical encoding to the original, with `replay: true`.

## 12. Errors

Errors use the transport's error object. The symbolic `data.code` is normative; numeric transport codes are fixed by the binding. Every error `data` contains:

- `code`: symbolic error.
- `retry`: one of `no`, `same_command`, `after_reconcile`, `after_renegotiate`.
- `details`: object, possibly empty, with the members listed below.

| Code | Retry | Meaning | `details` |
|---|---|---|---|
| `invalid_envelope` | `no` | Schema violation, unknown field, or impossible value | `path`, `reason` |
| `limit_exceeded` | `no` | A declared limit other than the frame limit was exceeded | `limit`, `maximum` |
| `negotiation_required` | `after_renegotiate` | Operation before successful negotiation | — |
| `already_negotiated` | `no` | Second negotiation in one session | — |
| `method_not_found` | `no` | Operation unknown to the provider | `operation` |
| `profile_not_negotiated` | `after_renegotiate` | Operation's profile not selected in this session | `profile` |
| `unsupported_version` | `no` | Negotiation: no common major version for a required profile | `unsatisfied` |
| `unsupported_profile` | `no` | Negotiation: required profile unknown, declared unsupported, or missing a dependency | `unsatisfied` |
| `unsupported_required_feature` | `no` | Negotiation: a required feature is unavailable; or a message's `requires` names something not negotiated or understood | `unsatisfied` in negotiation; `features` for a message |
| `unsupported_digest_algorithm` | `no` | Digest algorithm not supported | `algorithm`, `supported` |
| `digest_mismatch` | `no` | `command_digest` differs from the recomputed digest | `expected` |
| `idempotency_conflict` | `no` | Command identity bound to a different intent | `command_id` |
| `dedupe_history_unavailable` | `after_reconcile` | Command identity may have been used but its record was discarded | `oldest_retained` |
| `effect_history_unavailable` | `after_reconcile` | The effect may have been recorded but its record is no longer retained (§19.2) | — |
| `upload_offset_mismatch` | `after_reconcile` | Evidence: an append's offset is not the bytes received so far (EVIDENCE §4) | `received` |
| `upload_size_exceeded` | `no` | Evidence: an append would exceed the declared size | `received`, `size` |
| `upload_incomplete` | `after_reconcile` | Evidence: seal before all declared bytes were received; a new seal needs the revision after the missing appends | `received`, `size` |
| `content_digest_mismatch` | `no` | Evidence: received bytes do not match the declared digest | `computed` |
| `artifact_digest_mismatch` | `no` | Evidence: a reference's digest differs from the artifact's sealed digest; sealed content is immutable | — |
| `hold_active` | `after_reconcile` | Evidence: purge while an active hold is not released by this command | `holds`, `filtered` |
| `claims_not_comparable` | `no` | Knowledge: two revisions cannot conflict under the structural comparison rules (KNOWLEDGE §7); proposed M5 | `reason` |
| `contract_unavailable` | `after_reconcile` | Verification: the named contract cannot be read or is not a valid contract (VERIFICATION §3); proposed M5 | `reason` |
| `capability_unavailable` | `after_reconcile` | A capability the operation depends on is `unsupported` or `unknown` right now (§17) | `capability`, `status` |
| `stale_authority_epoch` | `after_reconcile` | Caller's epoch was superseded | `current_epoch` if permitted |
| `unknown_authority_epoch` | `no` | Epoch never issued | — |
| `precondition_failed` | `after_reconcile` | One or more revision preconditions unsatisfied | `failed`: entries with `subject`, `expected`, and `current` if permitted |
| `invalid_cursor` | `no` | Cursor malformed, from another stream, or beyond the stream's end (§16) | `reason` |
| `authentication_required` | `no` | A socket session called an operation other than `core.describe` or `core.authenticate` before authenticating (§18) | — |
| `authentication_failed` | `no` | The credential is unknown, malformed or revoked; these cases are indistinguishable (§18) | — |
| `already_authenticated` | `no` | The session already has a principal (§18) | — |
| `not_found` | `no` | Query target absent or not visible to this principal | — |
| `permission_denied` | `no` | The principal is not authorized for this operation on this subject (§15.5) | `reason` |
| `unavailable` | `same_command` | Provider temporarily cannot process; nothing was bound | — |
| `internal_error` | `after_reconcile` | Provider failed in an undefined way; outcome unknown | — |

Profile error codes are raised only after authorization (§15.5), so an unauthorized principal never receives them or their details. A profile limit that names no subject, such as Evidence's `chunk_limit`, is decided with the Core limits at step 2 and reveals nothing.

`retry: same_command` means retransmitting the identical command, with the same `command_id`, is safe. It never means "send a new command". A caller that loses a response retransmits the same command; `unavailable` and `internal_error` do not tell the caller whether the command was bound.

## 13. Conformance-only test profile `core-test/1`

Core behavior needs some subject to act on, so the conformance suite defines `core-test/1`. It is **not a product profile**. Providers MAY implement it only to run Core fixtures, and MUST NOT expose it outside test configurations.

Fixtures reach every domain state — revisions, epochs, bound commands — only through these ordinary operations. The separate test-control channel changes only the environment (§13.1).

| Operation | Kind | Semantics |
|---|---|---|
| `core-test.authority.claim` | command | Subject `{ "kind": "core-test.authority", "id": "core-test" }`. Advances the authority epoch of scope `core-test` by one; outcome `{ "epoch": n }`. Precondition on the authority subject's revision, which equals the epoch. No `authority_epoch` field. Models a controller takeover. |
| `core-test.subject.put` | command | Payload `{ "value": string, "labels"?: { string: string } }`. `labels` exists only so fixtures can exercise canonical member ordering with arbitrary keys; it is not stored. With precondition revision `0`, creates the subject at revision 1. With precondition revision `n`, replaces its value at revision `n + 1`. Requires `authority_epoch` for scope `core-test`. Outcome `{ "value": … }`. |
| `core-test.subject.get` | query | Payload `{ "subject": … }`. Returns `{ "subject": …, "revision": n, "value": … }` or `not_found`. |
| `core-test.subject.applied_count` | query | Payload `{ "subject": … }`. Returns `{ "subject": …, "applied_count": n }`, the number of commands that changed the subject; 0 for a subject that never existed. Fixtures use it to detect a re-executed duplicate without trusting the acknowledgment. |

The primary subject's precondition entry is mandatory for `core-test.subject.put`; without it the command is `invalid_envelope`. A provider exposes `core-test` only when launched with a conformance launch configuration.

Rights (§15): `core-test.claim` on the authority subject for `claim`; `core-test.write` on the primary subject for `put`, plus `core-test.read` on every other subject named in its preconditions; `core-test.read` on the payload subject for `get` and `applied_count`. The authority scope `core-test` is the one advanced by `claim`. The authority scope starts at epoch 0, where the authority subject does not exist.

### 13.1 Test control is environment-only

Conformance runs need states that ordinary operations cannot reach in bounded time, such as a provider restart or discarded deduplication history. The suite reaches them only through the **environment** of the process under test, never through a protocol operation:

- **Launch.** The runner launches the provider with a data directory and a **launch configuration** file ([conformance README](../../../conformance/README.md#launch-configuration)). The file can set the session principal and authority principals, limits, deduplication retention, event retention and epoch changes, capability status, a fixed provider clock, a controlled clock file, and implementation-specific barriers ([decision 007](../../decisions/007-execution-test-controls.md)). Files the runner writes in its per-case work directory, such as the clock file and barrier releases, are environment too; process kills are lifecycle, not protocol.
- **Restart.** A restart is ending the process and launching it again over the same data directory.
- **No domain writes.** The launch configuration MUST NOT create, modify or delete subjects, commands, epochs or grants. Those are reached only through real operations.

No protocol operation or method name is reserved for testing. Product endpoints expose none.

## 14. What Core does not establish

- An acknowledgment is not execution, delivery, verification or acceptance.
- A missing response is not failure, and a closed session is not cancellation.
- A revision orders changes to one subject at one provider; it is not a global clock.
- Negotiated support for a feature says the provider implements it. It does not say the underlying system, such as a harness, can enforce it. Those limits are profile-specific capability facts.

## 15. Principals and grants

Owner decision U3 (release plan §6): grants are **provider-held records referenced by ID**. Bearer tokens that carry their own authority are deferred with Remote trust. This section is negotiated as the Core feature `core.grants`.

### 15.1 Principals and authorities

The transport binding establishes the session **principal** ([STREAM §5](../bindings/STREAM.md#5-stdio-form)); a message cannot assert its own principal.

A provider has a configured, provider-local identity `provider_id` and a configured set of **authority principals**. These principals hold implicit full authority over the provider's subjects. In standalone use this is the owning user or the caller that user designates ([ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md)). How a product configures authorities is outside the protocol. Conformance launch configuration sets `authority_principals` and defaults it to the session principal.

Every other principal acts only under a grant.

### 15.2 Grant records

A grant is a provider-owned subject of kind `core.grant`. Its record contains:

| Field | Meaning |
|---|---|
| `id` | The subject ID |
| `issuer` | Principal that issued it |
| `holder` | The only principal that may act under it |
| `audience` | The `provider_id` it is valid at. It must equal the issuing provider's own ID. |
| `rights` | Non-empty set of right names (`<profile>.<right>`), defined by each profile |
| `resources` | Non-empty list of `{ "kind": subject kind }`, optionally narrowed by exactly one of `id` or `id_prefix` |
| `expires_at` | Optional instant `YYYY-MM-DDTHH:MM:SSZ` (UTC), evaluated against the provider's clock only |
| `authority_binding` | Optional `{ "scope", "epoch" }`: the grant authorizes only while that authority scope's current epoch equals `epoch` |
| `delegation` | `{ "allowed": boolean, "max_depth": integer ≥ 0 }` |
| `parent` | Optional ID of the grant it was delegated from |
| `state` | `active` or `revoked` |

### 15.3 Operations

| Operation | Kind | Semantics |
|---|---|---|
| `core.grant.issue` | command | Subject `{ "kind": "core.grant", "id": … }` with precondition revision `0`. Payload is the record without `id`, `issuer` and `state`. Outcome `{ "grant": record }` at revision 1. |
| `core.grant.revoke` | command | Subject is the grant, with a precondition on its current revision. Payload `{}`. Revokes the grant and every grant delegated from it, directly or transitively. Outcome `{ "revoked": [ids] }` lists every grant revoked by this command. |
| `core.grant.get` | query | Payload `{ "grant": id }`. Visible to its holder, its issuer and authority principals. Anyone else gets `not_found`. |

**Issuing:**

- A grant without `parent` may be issued only by an authority principal.
- A grant with `parent` may be issued only by the parent's holder, while the parent is active, unexpired and still bound to a current epoch, and only when the parent's delegation is `allowed` with `max_depth ≥ 1`. The child MUST NOT exceed its parent:
  - rights ⊆ parent rights;
  - every child resource is covered by a parent resource;
  - if the parent expires, the child expires no later;
  - child `max_depth` ≤ parent `max_depth − 1`;
  - the child has the same `authority_binding` as the parent, if the parent has one.

  A violation is `permission_denied` with reason `delegation_exceeded`.
- The step 6 validity checks on `issue` run in this order, before the issuing rules above: `audience`, `expires_at`, then `authority_binding` scope. Their `invalid_envelope` `details.path` values are `/payload/audience`, `/payload/expires_at` and `/payload/authority_binding/scope`.
- `issue` and `revoke` carry exactly one precondition, on the grant subject itself: revision `0` for `issue` and at least `1` for `revoke`. Anything else is `invalid_envelope` at step 2.
- An `authority_binding` whose `scope` is not an authority scope the provider tracks is `invalid_envelope`, checked with `audience` and `expires_at` at step 6. A binding to a later epoch of a known scope is accepted; the grant authorizes once that epoch is current.
- Unknown right names and resource kinds are accepted. They never match an operation or subject.
- An `audience` other than the provider's own ID, or an `expires_at` not after the provider's current time, is `invalid_envelope`. These checks run at step 6, after deduplication, so an already-bound issue command still replays after its expiry has passed.
- A grant is expired from the instant `expires_at` onward: it authorizes only while the provider clock is strictly before `expires_at`.

**Revoking:** a grant may be revoked by its issuer or by an authority principal. Anyone else gets `permission_denied` with reason `not_authority`, whether or not the grant exists. Revoking a grant that is already revoked is `permission_denied` with reason `revoked`, decided after that check. The outcome's `revoked` lists the named grant first, then its descendants that were still active, in a provider-chosen order; each gets a new revision and one `core.grant.revoked` event. Already revoked descendants are neither listed nor changed. Revocation stops future operations under the grant. It does not undo operations already accepted, and it does not recall effects that later profiles may already have sent.

- **Constraints.** A grant may carry `constraints`: a list of typed restrictions, each `{ kind, ... }`, whose kinds are defined by profile features (for example `evidence.work_binding`, EVIDENCE §10).
  - A constraint only narrows what the grant's rights and resources allow; it never widens them.
  - Issuing a grant with a constraint kind this provider does not implement is `invalid_envelope` at `/payload/constraints/<i>/kind`. A kind whose feature the issuing session did not negotiate is `unsupported_required_feature` with `features`.
  - A delegated grant carries every constraint of its parent, unchanged, otherwise `delegation_exceeded`. It may add constraints, which only narrow it further.
  - Resource kinds never imply a constraint.

### 15.4 Grants and authority epochs

A grant with `authority_binding` stops authorizing once that scope's epoch changes. Nothing is rewritten; the record stays `active`, but step 6 refuses it with reason `authority_epoch_stale`. This lets a takeover invalidate everything the previous controller delegated without enumerating it.

### 15.5 Authorization (step 6)

For each command or query that a profile protects:

1. **A `grant` field is present.** This applies even to an authority principal, which is then restricted to that grant. It must name a grant whose holder is the session principal, that is active, unexpired, bound to a current epoch if bound at all, and whose rights and resources cover every right the operation needs.
2. **No `grant` field.** The principal must be an authority principal.
3. **Outcome.** Otherwise the operation is refused with `permission_denied` and `details.reason`, one of:
   - `grant_required` — no grant named, and the principal is not an authority;
   - `not_authority` — issuing a root grant as a non-authority;
   - `grant_not_found` — no such grant held by this principal, so a grant held by someone else is indistinguishable from a nonexistent one;
   - `revoked`;
   - `expired`;
   - `authority_epoch_stale`;
   - `right_missing` — some needed right is not granted;
   - `out_of_scope` — a subject is not covered;
   - `delegation_exceeded`.

   When several reasons apply, report the first in this order: `grant_not_found` (checked before any other property of the grant, so another principal's revoked grant is still `grant_not_found`); `revoked`; `expired`; `authority_epoch_stale`; `right_missing` for any needed right; `out_of_scope` for any needed subject.

**Which operations are protected.** In this document: the `core-test` operations (§13), `core.events.read` and `core.events.subscribe` (§16.6). The `core.grant.*` operations follow their own rules (§15.3). `core.describe`, `core.feature_dependencies`, `core.negotiate`, `core.authenticate`, `core.capabilities` and `core.events.unsubscribe` are not protected. A `grant` field on an unprotected or `core.grant.*` operation is validated but not evaluated. `core.events.unsubscribe` removes only the session's own subscriptions; an unknown subscription is `not_found`.

**Without `core.grants`.** Authorization still applies when the session did not negotiate `core.grants`. Such a session cannot name a grant, so a principal that is not an authority is refused with `grant_required`.

**Existence is never revealed by authorization.** Step 6 runs before any check that depends on whether a subject exists. An unauthorized principal gets the same `permission_denied` for an existing and a nonexistent subject (CORE-12). `precondition_failed` details include current revisions only for subjects the principal may read, and every precondition subject needs read authority anyway.

**Deduplication is per principal** (§6.2). Another principal's command identity is invisible. Reusing it is simply a new command.

A replay of an already-bound command skips step 6 (§10), so revocation does not hide a principal's own earlier outcome.

### 15.6 What grants do not establish

A grant is authorization at one provider. It is not identity proof, is not transferable to another provider, and is not a promise that an underlying system will enforce the same limits. Execution profiles report enforcement levels separately. An expired or revoked grant does not mean nothing happened under it.

## 16. Events and subscriptions

Every provider records what it committed as an ordered, durable **event stream**. Callers read it with opaque cursors or subscribe to it on a connection. This section is negotiated as the Core feature `core.events`. Matrix rows: CORE-2, OBS-1 to OBS-6, OBS-8.

### 16.1 Stream identity and positions

A provider store has one semantic stream with a stable `stream` ID.

- **Epochs.** Positions are `(epoch, sequence)`. Epochs start at 1. Within an epoch, sequences start at 1 and are **contiguous**: the stored stream has no holes.
- **New epoch.** A provider starts one when it can no longer vouch for continuity, for example after restoring from a backup or rebuilding its store. The previous epoch keeps the events the provider still vouches for, through `vouched_through`. Consumers are told about the change explicitly (§16.4).
- **Order.** Positions order events within one stream only. `recorded_at` is metadata; it neither orders events nor proves causality (CORE-2).

### 16.2 Event records

| Field | Meaning |
|---|---|
| `stream`, `epoch`, `sequence` | Position |
| `type` | Profile-defined event type, such as `core-test.subject.changed` |
| `subject`, `revision` | The subject that changed and its revision after the change |
| `origin` | `command` for events caused by an accepted command, `provider` for events the provider records itself (such as a capability change, §17) |
| `operation_ref`, `command_id` | The accepted command that caused the event; present exactly when `origin` is `command` |
| `caused_by` | The command envelope's `caused_by`, copied unchanged; an empty array if absent or for provider-origin events |
| `recorded_at` | Provider clock instant, metadata only |
| `payload` | Normalized, profile-defined facts sufficient for a consumer's reducer. Never a prose summary. |

**Core event types:**

| Type | Payload |
|---|---|
| `core.grant.issued` | `{ "grant": record }` |
| `core.grant.revoked` | `{ "state": "revoked" }`, one event per revoked grant |

**core-test event types:**

| Type | Payload |
|---|---|
| `core-test.subject.changed` | `{ "value" }` |
| `core-test.authority.claimed` | `{ "epoch" }` |

### 16.3 Recording rules

- An accepted command appends its events in the same owner transaction as its state change (§10 step 8). If that transaction fails, neither exists.
- Events are recorded whether or not any session negotiated `core.events`. The stream belongs to the store, not to a session.
- A replay, a rejected command and a query append nothing. A duplicate transmission never produces a duplicate event.
- A command that changes several subjects appends events in a stable order: the primary subject first.

### 16.4 `core.events.read` (query)

**Payload:** exactly one of `cursor` (from an earlier result) or `from` (`"start"` or `"now"`), required `limit` (1–1000 items), and optional `kinds` (subject kinds to include).

**Result:** `{ "stream": { "id", "epoch" }, "items": [...], "next_cursor", "filtered" }`.

`items` is an ordered list. Each item is exactly one of:

| Item | Meaning |
|---|---|
| `{ "event": record }` | The next event after the cursor that the principal may see |
| `{ "epoch_change": { "from_epoch", "to_epoch", "vouched_through" } }` | The stream moved to a new epoch after `vouched_through` in `from_epoch`. Nothing after that position in the old epoch will ever be delivered. Reading continues in `to_epoch` from sequence 1. |
| `{ "gap": { "kind": "retention", "from", "to", "snapshot" } }` | Some events from `from` onward were discarded under retention, so positions `from` through `to` are not delivered as individual events. `snapshot` is `{ "as_of": position, "subjects": [ { "subject", "revision", "state" } ] }` for the visible subjects at `as_of`, which equals `to`. The provider chooses `to` anywhere from the last discarded position up to the stream head. A provider that keeps only current state uses the head; one that keeps historical snapshots may stop at the discard boundary and deliver later retained events individually. Reading continues after `to`. A gap may span epochs: `from` and `to` can lie in different epochs, and epoch changes inside the gap are not reported separately, because the snapshot supersedes them. |

Rules:

- **Cursors.** `next_cursor` is the position after everything the result covered: after the last item or, when the read reached the end, after any trailing events that were hidden. Resuming from it never repeats and never skips an item. A cursor that is malformed, belongs to another stream, or points beyond the stream's current end is `invalid_cursor`. Cursors are opaque; callers MUST NOT construct or interpret them.
- **No silent gaps.** A provider MUST NOT present an incomplete history as complete. If events after the cursor were discarded, the first item is a `gap` with a snapshot, never the next surviving event.
- **`from: "now"`** returns no items and a cursor at the current end.
- **Filtering.** `filtered` is `true` exactly when at least one event or snapshot subject in the range covered by this result was hidden by authorization or `kinds`. The skipped sequence numbers are then not gaps.
- **Snapshot state.**
  - `core-test.subject`: `{ "value" }`;
  - `core-test.authority`: `{ "epoch" }`;
  - `core.grant`: `{ "grant": record }`;
  - `core.capabilities`: `{ "predicates" }`.

  Other profiles define their own.

  `subjects` lists every visible subject changed by an event at or before `as_of`. A provider may also list visible subjects that no event changed, such as the initial capability snapshot.
- **Cursors in earlier epochs.** A cursor into an earlier epoch is valid even when it points past that epoch's `vouched_through`, or past any sequence that epoch ever had. That happens when the provider lost events it had already delivered, which is what epochs exist to report. The first item is then the `epoch_change`, and a `vouched_through` below the cursor's position tells the consumer it holds events the provider no longer vouches for. When such a cursor is at or past the vouched end and later positions were discarded, the `epoch_change` comes first and the gap starts at sequence 1 of the next epoch. A well-formed cursor from another stream, or one past the head of the current epoch, is `invalid_cursor`. `details.reason` is informative; its values are not specified.
- **Size.** A read returns fewer than `limit` items when the whole response would exceed the caller's `receive_limits.max_frame_bytes` (§4.2), but at least one item whenever the first item fits. If not even one item fits, the read is `internal_error`.

### 16.5 Subscriptions

`core.events.subscribe` (query) takes the payload of `core.events.read` without `limit`. It returns `{ "subscription", "stream" }`.

- **Delivery.** The provider then sends JSON-RPC notifications `core.events.notify` with params `{ "subscription", "items", "next_cursor" }` on the same connection. First comes the backlog from the requested position, then new items as commands commit. Every notification frame fits the caller's receive limit; the provider splits items across notifications as needed.
- **Ordering.** A notification carrying events caused by a command on the same connection is sent after that command's response.
- **Lifetime.** A subscription ends with its session or with `core.events.unsubscribe` (payload `{ "subscription" }`). If the grant it was created under stops authorizing (revoked, expired or epoch-stale), the provider sends one final `core.events.notify` with `"items": []` and `"ended": { "reason": "authorization_lost" }`, then delivers nothing more on it. If a single item cannot fit the caller's receive limit even alone, the subscription ends the same way with reason `item_too_large`; the item is never skipped. The final notification's `next_cursor` is where delivery stopped: after the last position the subscription covered, never past an item it would still have delivered.
- **Authorization.** Subscribing needs the same authorization as reading (§16.6), checked when subscribing, after every request on the connection, and before each delivery. A lapse ends the subscription even when nothing is pending for it.
- **Lapses caused elsewhere.** On a shared transport, a command on another connection can end a subscription's authorization, for example by revoking its grant or claiming a new authority epoch. The provider then sends the final notification without waiting for a request on the subscriber's connection. Two rules make this observable:
  - **Ordering, normative.** No item committed after the command that ended authorization is delivered on that subscription. Authorization and the items to deliver are evaluated against the same committed state.
  - **Latency, conformance bound.** Fixtures expect the final notification within 2 seconds of the ending command's response on the other connection.
- **Backpressure (feature `core.events.backpressure`, M3).** Semantic events are never skipped. A consumer that stops reading loses its connection, not events:
  - **Bounded pending output.** A provider bounds, per connection, the memory and bytes of output produced but not yet written. When a subscription's next notification would exceed that bound, the provider stops producing items for the connection's subscriptions; the withheld items stay undelivered, never skipped.
  - **Stall and room deadline.** A stall starts when the next notification or response would exceed the bound. The consumer must drain all pending output within `backpressure_notice_ms` of the stall's start. Partial progress does not extend that deadline. A consumer that misses it is **too slow**. A consumer that drains within each deadline is keeping up and is not closed.
  - **Ending notices, one budget.** If the session negotiated `core.events.backpressure`, the provider attempts one final `core.events.notify` for every subscription on the connection, with `"items": []` and `"ended": { "reason": "consumer_too_slow" }`. All of the connection's notices share one budget of `backpressure_notice_ms`, starting when the consumer was declared too slow. The budget does not restart per subscription, and nothing else is waited for before closure.
  - **Closure.** When the notice budget ends, or immediately when no notice is owed, the provider closes the connection and writes nothing more on it, whether or not the notices were written. Every subscription on it ends.
  - **Maximum time to closure.** A too-slow connection is closed within `2 × backpressure_notice_ms` of its stall's start, plus the provider's own processing latency; without the feature, within `backpressure_notice_ms`. No further stall starts once closure has begun, so repeated waits cannot extend it.
  - **Older consumers.** A session that did not negotiate `core.events.backpressure` never receives `consumer_too_slow`. Its connection is closed without that notice. Adding an enum value is not automatically backward-compatible: a consumer that validates `ended.reason` against the values it knows could reject or misread an unfamiliar one. The reason is therefore sent only after negotiation.
  - **Recovery.** The consumer reconnects and reads or subscribes from its last durably processed cursor. Events after that position are delivered in order. If some were discarded under retention, the first item is an explicit `gap` with a snapshot (§16.4), never a silent skip.
  - **Declared bounds.** Negotiating the feature adds `max_pending_notification_bytes` and `backpressure_notice_ms` to the result's `limits`. Without the feature, those members are absent. A provider that implements the feature bounds and closes every session this way, including sessions that did not negotiate it; those sessions only lack the declaration and the notice. A provider without the feature makes no backpressure guarantee.
  - **Conformance.** Fixtures pause the runner's reading and synchronize on implementation-specific signals (decision 007): `backpressure.stall.started` at a stall's start, `backpressure.limit.reached` when the consumer is declared too slow, and `backpressure.connection.closed` after closure. `core.events.unread-ending-notice-does-not-hold-the-connection` checks the room deadline, the shared notice budget across three subscriptions, and the maximum time to closure from the signals' times, with 750 ms of slack. A participant that does not declare them reports these fixtures as `unsupported` coverage limits.
- **Expiry while idle.** Expiry needs no command. A grant stops authorizing at `expires_at` on the provider clock (§15.3), and no item committed at or after that instant is delivered under it.
  - On a shared transport the provider re-checks idle subscriptions at the same latency as other lapses, so the final notification follows without a request.
  - On stdio, only the session itself commits commands, so nothing can become deliverable while it is idle. There the provider may report the end after the next request.
- **Consumers.** Semantic events are never dropped silently. Consumers deduplicate by position and resume from the last cursor they durably processed. A reconnect may therefore replay items.

### 16.6 Authorization

Reading or subscribing needs an authority principal, or a grant with right `core.events.read`. Authority principals see every event. Under a grant, an event or snapshot subject is included only if the session principal could read that subject directly:

| Subject kind | Visible under a grant when |
|---|---|
| A profile subject such as `core-test.subject` or `core-test.authority` | The same grant also has the profile's read right (`core-test.read`) with resources covering the subject |
| `core.grant` | The session principal is that grant's holder or issuer. This holds for authority principals too: an authority reading under a grant is restricted to it (§15.5), although `core.grant.get` shows it every grant. |
| `core.capabilities` | The grant's resources cover kind `core.capabilities`; no profile read right is needed |
| A kind no profile defines | Never |

`core.events.read` alone therefore reveals nothing the principal could not already read.

**Limitation:** sequence numbers reveal how many events filtering hid, though not their content or subjects. Profiles that need to hide event counts need a separate stream design.

### 16.7 What events do not establish

An event records a fact the provider committed. It is not delivery to any consumer, not verification of the fact's correctness, and not project acceptance. A stream position is not a global clock and says nothing about another provider's stream.

## 17. Capability snapshots

Negotiation (§4.2) says which protocol features a provider implements. **Capabilities** say what the provider can actually do right now, and on what evidence. For example: whether its store accepts writes, or, in later profiles, whether a harness adapter can enforce a restriction. This section is negotiated as the Core feature `core.capabilities`. Matrix rows: CORE-16, CORE-17.

### 17.1 `core.capabilities` (query)

Payload `{}`. Result:

| Field | Meaning |
|---|---|
| `revision` | Integer that increases whenever any predicate's name set, status, enforcement or `evidence.source` changes. A change to `observed_at` alone does not raise it. It is stable across restarts when nothing changed. |
| `predicates` | Array of `{ "name", "status", "enforcement"?, "evidence" }` |

Predicate fields:

- `status` is `supported`, `unsupported` or `unknown`.
- `enforcement`, where relevant, is `enforced`, `mediated` or `cooperative` ([PIO §7](https://github.com/Combraton/pio/blob/main/docs/spec/SPEC.md)).
- `evidence` is `{ "source", "observed_at"? }` and says how the provider knows.

A predicate without evidence of support is `unknown`, never `supported`.

### 17.2 Loss and pinned meaning

- **Refusal.** A command whose operation depends on a capability whose current status is not `supported` is refused with `capability_unavailable`, naming the capability and its status. This happens at step 7, before its preconditions, so nothing changes.
- **Pinned meaning.** A command already bound before the capability was lost still replays its stored outcome (step 5 precedes step 7). Loss does not rewrite accepted history (CORE-17).
- **Reconciliation.** Existing work that depends on a lost capability needs reconciliation under the owning profile's rules. Core records the loss; it does not declare such work failed.

### 17.3 Observing changes

Each revision change appends a provider-origin event of type `core.capabilities.changed`:

- subject `{ "kind": "core.capabilities", "id": provider_id }`;
- `revision` equal to the snapshot revision;
- payload `{ "predicates": [...] }`.

A new store's initial snapshot, revision 1, is not a change and appends no event; read it with `core.capabilities`. Callers watch capabilities by reading or subscribing with `kinds: ["core.capabilities"]` (§16). Under a grant, this needs resources covering that subject kind.

### 17.4 core-test capability

`core-test.writes` reports whether `core-test.subject.put` can be accepted. `put` depends on it. Conformance launch configuration can set it to `unsupported` or `unknown`, simulating a store that became read-only or a probe that has not run.

### 17.5 What capabilities do not establish

A `supported` predicate is the provider's current observation with its stated evidence. It is not a guarantee that a later operation succeeds, and not proof about systems the provider does not observe.

## 18. Principal credentials on shared connections

Accepted by [decision 006](../../decisions/006-unix-socket-principal-credential.md) (owner decision U12). Transports that many processes can reach — in 0.1, the [Unix-socket binding](../bindings/STREAM.md#6-unix-domain-socket-form) — need an application-level credential to establish a session principal. The stdio binding needs none, because its principal is assigned by the process that launched the provider.

### 18.1 Credentials

- **Format.** A credential is `ccred1.<principal>.<secret>`: the principal ID, then 32 bytes from a cryptographically secure random source, encoded as unpadded base64url (43 characters).
- **Storage.** Providers store only the SHA-256 digest of the whole credential string, together with its principal and revocation state.
- **Handoff.** A provider that hands a credential to a local caller writes it, followed by one line feed, to a file with mode `0600` inside a directory with mode `0700` owned by the user.
- **Administration.** Issuing, rotating and revoking credentials is provider administration. It is outside the protocol operations in 0.1, but every provider supporting a shared transport MUST support revocation and rotation.
- **Secrecy.** A credential MUST NOT appear in logs, error messages, error details, events, results or process command lines. It MUST NOT be passed to child processes through the environment.

### 18.2 `core.authenticate`

- **Payload:** `{ "credential": string }`. **Result:** `{ "principal": id }`.
- **Order.** A shared-transport session starts unauthenticated. Before `core.authenticate` succeeds, the only operations allowed are `core.describe` and `core.authenticate`. Anything else is refused with `authentication_required`, before §10 step 1's negotiation check.
- **Success.** A valid, unrevoked credential binds its principal to the session for the connection's lifetime. The principal then governs authorities (§15.1), grants, deduplication scope (§6.2) and event visibility.
- **Failure.** The provider compares digests in constant time. Unknown, malformed and revoked credentials all produce `authentication_failed` with empty `details`, and the same timing class where practical. After three failures on one connection the provider MAY close it.
- **Repeats.** A session that already has a principal gets `already_authenticated` on another `core.authenticate`. This includes every stdio session.

### 18.3 What credentials do not establish

A successful authentication proves that the caller possessed the credential. It does not prove which program the caller is. On a machine where coding agents run as the same user without file-access enforcement, an agent that can read the credential file can act as that principal. Execution providers must report the enforcement level protecting credential files (`enforced`, `mediated` or `cooperative`) in their capabilities. Revoking a credential stops future authentications. It does not end sessions already authenticated with it; the provider's administration may close those explicitly.

## 19. Effects and obligations

> Proposed for milestone M3 with the [Execution profile](EXECUTION.md). Not normative until M3 is accepted with schemas and fixtures. This section is negotiated as the Core feature `core.effects`. Matrix rows: EFF-1 to EFF-4.

An **effect** is an action a provider takes, or attempts, outside its own store: submitting a prompt, forwarding a cancellation, delivering a steering message or a context packet. Its outcome can be uncertain even when the provider's own records are durable (SPEC §6).

### 19.1 Effect descriptors

An effect is recorded in the same owner transaction as the command that authorizes it (§10 step 8), before any external I/O. Its descriptor is immutable:

| Field | Meaning |
|---|---|
| `id` | Stable effect ID. A network retry of the same effect keeps it. |
| `kind` | Profile-defined, such as `execution.prompt_submission` |
| `target` | What the effect acts on, such as an execution and its native session |
| `payload_digest` | Digest of the payload sent or to be sent (ENCODING) |
| `authorization` | The grant, or the authority principal, and the authority epoch it was authorized under |
| `retry_class` | One of the classes in §19.3 |
| `operation_ref` | The accepted command that recorded it |

Its **status** is observed separately and appended as evidence: `pending`, `succeeded`, `failed`, `unknown`. Every observation names its evidence class and source (EFF-1).

### 19.2 Querying after response loss

`core.effects.get` (query) takes `{ "effect": id }` and returns `{ "effect": descriptor, "revision", "status", "observations", "attempts", "obligations" }`. `revision` is the effect record's revision, used as the precondition revision of subject `{ "kind": "core.effect", "id" }` (§19.4). `status` is the latest observation's status. Each observation has `status`, `evidence: { class, source }` and `recorded_at`. Obligations have `id`, `expects`, `deadline` and `state`. Reading an effect needs read authority on its target. A principal without it gets the same `permission_denied` for an existing effect and for an effect ID that does not exist (CORE-12).

- After a lost response, a caller queries by the same effect ID before any new attempt (EFF-2).
- A provider that cannot establish the outcome reports `unknown` with an open obligation. It MUST NOT answer `not_found`, `failed` or "did not happen" for an effect it recorded.
- An effect ID the provider never recorded is `not_found`. An effect record the provider no longer retains is `effect_history_unavailable`, never `not_found`.

### 19.3 Retry classes

| Class | A retry is permitted |
|---|---|
| `pure` | Always; no external state changes |
| `read` | Under the caller's cost and privacy policy |
| `idempotent_key` | Only within the target's promised key semantics, and with the same key |
| `compensatable` | Only after the compensating action's outcome is known |
| `non_repeatable` | Never automatically; reconciliation or a new explicit decision is required |

A provider MUST NOT retry an effect outside its class. Blind retry of a `non_repeatable` effect after an `unknown` outcome is a conformance failure (EFF-3).

- **Attempts.** Each try at the external action is recorded in `attempts`: `{ attempt, idempotency_key?, outcome: "completed" | "failed" | "unknown", recorded_at }`. An attempt's outcome describes the call, not the effect: a completed call can still leave the effect `pending` or `unknown`.
- **Keys.** An `idempotent_key` effect carries `idempotency_key` in its descriptor, and every attempt uses that same key.

### 19.4 Obligations

An obligation records an expected observation: an effect outcome, a lifecycle transition, or required terminal output coverage. Each has a deadline on the provider clock.

- **States:** `open`, `satisfied`, `overdue`, `aborted`.
- **Deadline:** when the deadline passes with no observation, a provider-origin event marks the obligation `overdue`, even if no other event arrives.
- **An ended wait is not an observation.** When a deadline passes, or a profile ends a wait because it timed out, the obligation is `overdue`, not `satisfied`, even if the wait's end leads to a determination (EXECUTION §8). Only the expected observation satisfies it.
- **Aborting:** closes the wait. It MUST NOT change the effect's status: an aborted wait for an `unknown` effect leaves the effect `unknown` (EFF-4).
  - `core.effects.abort_obligation` (command) on subject `{ "kind": "core.effect", "id" }`, with precondition revision equal to the effect's revision and payload `{ "obligation" }`. It returns `{ effect, obligation: { id, state: "aborted" }, status }`, and appends `core.effect.obligation.aborted` with payload `{ effect, obligation, target }` on the effect subject.
  - An obligation that is not `open` or `overdue` is `not_found`. The command records no effect, so `effect_refs` is `[]`.
  - Under a grant it needs right `core.effects.abort_obligation` on the effect's target. An effect subject is visible in events exactly when its target is.
- **Survival:** obligations survive cancellation, timeouts and restarts until satisfied or explicitly aborted.

### 19.5 What effects do not establish

A `succeeded` status is the provider's observation under its stated evidence class. It is not proof of the target's internal state, and a closed obligation is not proof that an effect did not happen.
