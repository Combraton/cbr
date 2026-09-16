# Verification profile `verification/1` — release candidate

> **Status: Protocol 0.1 release candidate.** Accepted as milestone M5, with owner decisions M5-Q5, M5-Q6 and M5-Q9 ([M5](../../work/release-0.1/M5.md#owner-decisions-2026-09-15)); owner decision U2 bounds the depth: receipts plus `evaluate_contract` job references, with no signing and no evaluator orchestration. Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC §2, §5, §8, §10, §12](../SPEC.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: combraton `docs/architecture/VERIFICATION.md` §1–§3, §5–§7 and `docs/architecture/MODEL.md` §5 at `9af69ce`.

A **verifier** evaluates named properties of exact subjects under a contract and reports each property's result. A **receipt holder** keeps receipts and answers assessments; a verifier holds its own receipts. A **reader** checks what a receipt still says about the subjects it cares about.

A receipt is a **scoped observation**. It says which properties an identified evaluator found to pass, fail, go unevaluated or stay indeterminate, for exact subjects, in a stated environment and period. It is not a stop command, not project acceptance, and not a reliance decision (VER-2, SPEC §12).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `verification/1` depends on `core/1` with `core.events` and `core.capabilities`, and on `evidence/1` as a protocol dependency: contracts and receipts are exact sealed Evidence artifacts (§2). `core.grants` is optional, as for Evidence. Required Core features are enforced at negotiation.
- Optional features: `verification.jobs` (`evaluate_contract` and `job.inspect`, §4) and `verification.record` (`receipt.record`, §5). `receipt.inspect` and `receipt.assess` are part of the base profile, so a session with either feature, or neither, can read and assess the receipts it may read.
- A CI publisher that only records receipts needs neither Execution nor Knowledge.

## 2. Identities (M5-Q5)

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Contract reference `{ provider, artifact, digest }` | An Evidence reference to the exact sealed contract artifact (§3). A changed contract is a new artifact. | A contract name, a digest alone, or "the latest" contract |
| Property `property_id` | One property within one contract | A property of another contract, even with the same name |
| Subject role `role` | One subject slot a contract defines, such as `tree` or `output` | A subject identity |
| Job `{ "kind": "verification.job", "id" }` | One requested evaluation (§4) | A receipt; a job's state proves nothing about properties |
| Receipt `{ "kind": "verification.receipt", "id" }` | One immutable receipt record at its holder | A verdict on anything other than its subjects, contract and environment |
| Receipt reference `{ provider, receipt, artifact: { provider, artifact, digest } }` | The receipt subject and the exact sealed artifact holding its bytes | A reference without the artifact and digest |
| Evaluator `{ id, version }` | The implementation and version that evaluated | An authority |

**Receipt subject and artifact.** When a receipt is issued or recorded, its canonical bytes (§5) are sealed as one Evidence artifact. The mapping from the receipt subject to that artifact is recorded in the same transaction and never changes. The receipt reference carries both. A reader MUST check the artifact **and** its digest, as for any Evidence reference, and the receipt's content is exactly the sealed bytes. A provider MUST NOT regenerate or relabel them. The artifact's producer principal is the holder's own principal (EVIDENCE §3); the evaluator's principal is inside the receipt (§5).

## 3. Contracts

A contract is a sealed Evidence artifact in format `combraton-verification-contract/1` (media type `application/vnd.combraton.verification-contract+json`, canonical JSON):

| Member | Meaning |
|---|---|
| `format` | Required: `combraton-verification-contract/1` |
| `outcome` | Free text: the outcome the contract is about |
| `subjects` | Non-empty `[ { role, kind } ]` with unique roles, for example `{ role: "tree", kind: "repository_tree" }` or `{ role: "output", kind: "evidence.artifact" }` |
| `properties` | Non-empty `[ { property_id, layer, required, statement } ]` with unique IDs and boolean `required`; `layer` is `static`, `component`, `integration`, `journey`, `runtime_path` or `qualitative`. A lower layer never stands in for a higher one. |
| `environment` | `{ required_anchors: [ name ] }`: anchors whose values an assessment must match (§7). Empty when none. |
| `evaluators` | `[ id ]`: evaluator IDs whose results this contract accepts, or `null` for any |
| `freshness` | `{ max_age_seconds }` or `null`: how long after `observed_until` a receipt may be assessed as current |

A contract is a closed object, as envelopes are (CORE §5.1). Every member above is required. A member of the wrong type, or a member the table does not list, makes the content `invalid_format`.

- **Reading a contract.** A holder reads a contract at its named provider, as an ordinary reader, and checks the digest. In the reference, a holder reads contracts only from its own store.
- A command whose contract cannot be read is refused with `contract_unavailable` and `details.reason`, decided in this order: `unreachable` (another provider this holder cannot read), `not_found`, `not_sealed`, `unavailable` (purged, pending purge, corrupt or otherwise unavailable), then `artifact_digest_mismatch` for a readable contract whose digest differs, then `invalid_format` for content that is not a contract as the table above requires. The artifact's media type is not checked.
- A caller must be authorized to read the contract: `evidence.read` on the contract's artifact subject at this provider. Otherwise the operation is `permission_denied`, identical for a nonexistent contract.

## 4. Evaluating a contract (`verification.jobs`, VER-3, M5-Q9)

`verification.evaluate_contract` (command): subject `{ kind: "verification.job", id }`, precondition revision 0.

| Member | Meaning |
|---|---|
| `contract` | Contract reference |
| `subjects` | `[ { role, digest, subject? } ]`: one entry per contract role. `digest` is exact: a tree ID or an algorithm-qualified artifact digest. `subject` optionally names the protocol subject it came from. |
| `environment` | `{ anchors: { name: value } }` requested |
| `evaluator` | `{ id, version }` |

- **Outcome: a job reference only.** `{ job, state: "queued" }`. A command never returns property results; the report is a separate receipt (VER-3).
- **Evaluator capability.** Each installed evaluator version is advertised as the capability predicate `verification.evaluator.<id>.v<version>` (CORE §17). Evaluator IDs match `[a-z][a-z0-9_-]{0,63}` and versions `[a-z0-9_-]{1,32}`. Neither contains a dot, so the encoding is unambiguous: the name has exactly five dot-separated segments, the fourth is the ID, and the fifth is `v` followed by the version. Every predicate name is therefore a Core dotted name. For example, evaluator `journey` version `2` is `verification.evaluator.journey.v2`, and version `v2` would be `verification.evaluator.journey.vv2`. If that predicate's status is not `supported`, the command is refused with `capability_unavailable` at step 7. The provider never substitutes another version.
- **ID shared with receipts.** A job's receipt takes the job's ID. An `evaluate_contract` whose job ID already names a receipt, or a `receipt.record` whose receipt ID already names a job, is `precondition_failed`. Its `failed` entry names the existing subject with `expected: 0`, and includes `current` where the principal may read that subject, as for any precondition entry (CORE §7).
- **Order at step 7**, following CORE §10:
  1. **Evaluator capability**, before the preconditions (CORE §17). A version the snapshot does not list has no evidence of support, so its status is `unknown` (CORE §17.1); a listed version reports its own status.
  2. The Core revision preconditions (CORE §7).
  3. The job ID names no receipt.
  4. Contract readability (§3).
  5. **Subject roles.** A missing, duplicated or unknown role is `invalid_envelope` at `/payload/subjects`, with details `missing`, `duplicated` and `unknown`, each a list of names and always all three present.
- **Pinned evaluator.** The job records `evaluator: { id, version }` and never changes it. Its receipt names that evaluator.
- **Durable property results.** A verifier records each property result on the job as it is evaluated, in the owner transaction that observes it. A recorded result is never rewritten.
- `verification.job.inspect` (query) `{ job }` → `{ job, state: "queued" | "running" | "completed", contract, subjects, evaluator, recorded: [ { property_id, result, reason? } ], receipt? }`. `recorded` lists only results the evaluator reported; results assigned at completion (below) appear only in the receipt. A job is `queued` when submitted, `running` from its first evaluation, and `completed` exactly when its receipt is issued; each change appends `verification.job.changed`.
- **Losing the pinned evaluator.** When the predicate of the job's evaluator is no longer `supported`, including across a provider restart, the job completes at its next evaluation:
  - recorded results are kept;
  - every property without a recorded result is `indeterminate` with reason `evaluator_unavailable`.

  It is never `pass` or `fail`. A later installed version never continues the job. A job whose evaluator is lost before its first evaluation never ran: it goes from `queued` directly to `completed`, without `running`, and its receipt's `observed_from` equals `observed_until`.
- **Ending without a result.** When the evaluator finishes without reporting a property, that property is `not_evaluated` with reason `not_reported`.
- **Issued receipt members.**
  - `environment` holds only the anchors the evaluator observed, never the requested ones (`{ anchors: {} }` when it observed none);
  - `inputs` and `provenance` are what the evaluator reports (empty for the reference);
  - `scope` is the contract's `outcome`;
  - `observed_from` is when the job started running and `observed_until` when it completed.

  Issuing appends `verification.receipt.issued`, then the receipt artifact's events, then `verification.job.changed` to `completed`.

## 5. Receipts (VER-1)

Receipt content, the canonical JSON sealed in format `combraton-verification-receipt/1` (media type `application/vnd.combraton.verification-receipt+json`):

| Member | Meaning |
|---|---|
| `format`, `receipt` | The format and the receipt ID |
| `subjects` | `[ { role, digest, subject? } ]`, exactly one entry per contract role |
| `contract` | The contract reference |
| `evaluator` | `{ id, version, principal }`. For an issued receipt, `principal` is the verifier's; for a recorded one, it is the session principal of the recording command. |
| `environment` | `{ anchors: { name: value } }` as observed |
| `inputs` | Evidence references the evaluation used |
| `properties` | `[ { property_id, result, reason?, evidence } ]`, exactly one entry per contract property, in contract order. `result` is `pass`, `fail`, `not_evaluated` or `indeterminate`; `reason` is required exactly when the result is `not_evaluated` or `indeterminate`. |
| `observed_from`, `observed_until` | The observation period, on the evaluator's clock; `observed_from` ≤ `observed_until` |
| `valid_until` | An explicit end of validity, or `null` |
| `scope` | Free text: what the receipt covers |
| `job` | The job, or `null` for a recorded receipt |
| `provenance` | Execution and Evidence references for how the evaluation ran |

- **Complete listing.** The following are `invalid_envelope`, with `path` and details `missing`, `duplicated` and `unknown` (each a list, all three always present):
  - a receipt whose subject roles or property IDs miss, duplicate or add to the contract's, at `/payload/subjects` or `/payload/properties`;
  - a `reason` missing where it is required, or present where it is not (step 2);
  - `observed_from` after `observed_until`, at `/payload/observed_until` (step 2).

  Unavailable checks are listed as `not_evaluated` or `indeterminate`; they are never omitted and never collapsed into `pass` or `fail` (VER-1).
- **Recording** (`verification.record`). `verification.receipt.record` (command): subject `{ kind: "verification.receipt", id }`, precondition revision 0. Its payload is the receipt content without `format`, `receipt`, `evaluator.principal` and `job`. At step 7, after the Core revision preconditions, the holder checks that the receipt ID names no job (§4), then the contract (§3), the subject roles and the property listing. It then seals the receipt and records the mapping. The receipt is the command's primary subject, so `verification.receipt.recorded` precedes the artifact's events (CORE §16.3). The artifact's events are Evidence events for its publication (EVIDENCE §12). This profile does not fix which ones a holder's own publication appends; for example, `evidence.artifact.appended` may or may not appear, so readers MUST NOT depend on the artifact's event count or revision.
- **Issuing.** A completed job's receipt has the job's ID as its receipt ID.
- **Artifact convention** (conformance providers only, as CONTEXT §12): receipt `r` is sealed as artifact `receipt.<r>`.
- Receipts are immutable. Re-evaluating produces a new receipt; a failed receipt stays visible after a later pass.
- `verification.receipt.inspect` (query) `{ receipt }` → `{ reference, content }`. When the sealed bytes cannot be read or fail integrity, the query is `unavailable`; content is never regenerated.

## 6. Results are scoped observations (VER-2)

- A `fail` changes nothing outside the receipt:
  - it does not change any execution axis;
  - it does not cancel or stop work;
  - it does not revoke grants or change reliance.

  A consumer that wants to act on a failure does so through its own authorized operations.
- A receipt about an execution's output is not the execution's `evaluation` axis. An executor MAY record an attributed reference to it there (EXECUTION §3), and never derives it.

## 7. Assessment (VER-4, M5-Q6)

`verification.receipt.assess` (query) reports what one receipt establishes for a stated basis and time. It is read-only: it records nothing, emits no event and accepts nothing.

**Request:** `{ receipt: receipt reference, contract: contract reference, subjects: [ { role, digest } ], environment?: { anchors }, at? }`.
- `subjects` MUST list every contract role exactly once. A missing, duplicated or unknown role is `invalid_envelope` at `/payload/subjects`, decided once the contract is read.
- `at` is an instant. When it is absent, the provider's clock is used.

**Result:**

| Member | Meaning |
|---|---|
| `assessed_at` | The instant used |
| `time_basis` | `provider_clock` when `at` was absent, otherwise `caller_selected` |
| `present_validity` | `established` only when `time_basis` is `provider_clock` and `overall` is `satisfied`; otherwise `not_established`. A result for a caller-selected time is never proof of present validity. |
| `basis` | The subjects and environment the assessment compared, echoed |
| `checks` | `{ receipt, contract, subjects, environment, evaluator, time }`, each `passed`, `failed` or `unverifiable`, with reasons |
| `properties` | `[ { property_id, required, result, status: "satisfied" \| "failed" \| "not_satisfied", reasons } ]`, in contract order |
| `overall` | `satisfied`, `failed` or `not_satisfied` |
| `reasons` | Reasons that apply to the whole receipt |

**Receipt-level checks**, each adding its reasons:

| Check | Fails with | Unverifiable with |
|---|---|---|
| Receipt | The reference's artifact or digest differs from the recorded mapping: `receipt_reference_mismatch` | The sealed bytes cannot be read or fail integrity: `receipt_unavailable` |
| Contract | The receipt's contract reference is not exactly the requested reference (provider, artifact and digest): `contract_mismatch` | The requested contract cannot be read: `contract_unavailable` |
| Subjects | Some role's digest differs from the receipt's: `subject_mismatch` | The receipt cannot be used: its receipt check's reason |
| Environment | A required anchor's value differs between request and receipt: `environment_mismatch` | A required anchor is missing from the request or the receipt: `environment_unverified`. The contract cannot be read: `contract_unavailable`. The receipt cannot be used: its reason. |
| Evaluator | `evaluators` is listed and excludes the receipt's evaluator: `evaluator_not_permitted` | The contract cannot be read: `contract_unavailable`. The receipt cannot be used: its reason. |
| Time | `assessed_at` is before `observed_until`: `before_observation`. `assessed_at` is at or after `valid_until`, or strictly later than `observed_until` plus `max_age_seconds`: `stale`. At exactly `observed_until` plus `max_age_seconds` the receipt is still current. When the contract cannot be read, `max_age_seconds` is unknown, and only `valid_until` and `observed_until` are compared. | The receipt cannot be used: its reason |

**Several reasons.** When more than one cell applies, a check lists every applicable reason, `contract_unavailable` first and then the receipt's reason. Top-level `reasons` lists each check's reasons in check order (receipt, contract, subjects, environment, evaluator, time), without duplicates.

`before_observation` means a result dated after the requested time cannot show what was known then.

**Property status.**
- If any check is `failed` or `unverifiable`, every property is `not_satisfied`, with those reasons, whatever its recorded result.
- Otherwise `pass` is `satisfied`, `fail` is `failed`, and `not_evaluated` or `indeterminate` is `not_satisfied`, with the result as its reason.
- When the contract cannot be read, or the receipt cannot be used (a reference that differs from its mapping, or bytes that cannot be read), `properties` is empty and `overall` is `not_satisfied`: no property result is presented without both. A receipt that cannot be used also makes the contract-mismatch comparison `unverifiable` with the receipt's reason.

**Overall.** `not_satisfied` if any check is `failed` or `unverifiable`, or if `properties` is empty; otherwise `failed` if any required property is `failed`; otherwise `not_satisfied` if any required property is not `satisfied`; otherwise `satisfied`. A contract with only optional properties is therefore `satisfied` only when every check passed.

**Authorization.** A reader not authorized for the receipt gets `permission_denied`, identical for an existing and a nonexistent receipt (CORE-12); an authorized reader naming a receipt that does not exist gets `not_found`. Assessment needs `verification.read` on the receipt and `evidence.read` on the contract's artifact subject.

## 8. Rights

| Right | Covers |
|---|---|
| `verification.evaluate` | `evaluate_contract` on covered jobs |
| `verification.record` | `receipt.record` on covered receipts |
| `verification.read` | `job.inspect` on covered jobs, `receipt.inspect` and `receipt.assess` on covered receipts, and events of covered subjects |

Reading a contract or cited evidence needs Evidence rights at its provider.

## 9. Verification grants no authority

- Issuing, recording or assessing a receipt never records a reliance decision (KNOWLEDGE §6), resolves a conflict, or accepts project direction.
- A verifier principal is not an authority because its receipts pass.
- A `satisfied` assessment is an input that a bound authority MAY cite in a decision's `validation_basis`; the decision is still the authority's own operation.
- A human override of a failing receipt is an authority decision with its own rationale and scope; it never rewrites the receipt.

## 10. Events

*Candidate* types:

| Type | Subject | Payload |
|---|---|---|
| `verification.job.changed` | job | `{ state, evaluator }` |
| `verification.property.recorded` | job | `{ property_id, result, reason? }` |
| `verification.receipt.issued` | receipt | `{ reference, job }`, where `job` is the job ID |
| `verification.receipt.recorded` | receipt | `{ reference }` |

## 11. Conformance and test controls

- **Reference participants only.** A reference verifier drives a **scripted evaluator** from launch configuration `verifier` (test control):
  - `evaluators: [ { id, version, status } ]`, from which the capability predicates are derived;
  - `scripts`, mapping job IDs to steps: `wait_until` (instant), `property` (`property_id`, `result`, and `reason` exactly for `not_evaluated` and `indeterminate`), `observe` (`anchors` the evaluator observed), `valid_until` (instant) and `complete`;
  - `default_script` for other jobs.

  Steps advance no later than the provider's next request, as EXECUTION §15. A job with no script completes at once; a script that runs out without `complete` leaves the job `running`. A job stays `queued` while its script waits at a leading `wait_until`: its first evaluation, which makes it `running`, is the first step other than `wait_until`. So a job whose evaluator is lost during that wait never ran (§4). Restarting with different `evaluators` simulates an upgraded or lost evaluator (SCN-7).
- No fixture depends on a real browser, CI system or model reviewer.

## 12. What verification does not establish

A receipt establishes that an identified evaluator reported these property results for these exact subjects, under this contract, in this environment and period. It does not establish:
- properties the contract did not name, or layers other than those evaluated;
- anything about a different subject, including an integration of individually verified parts;
- independence of evaluators that share sources;
- present validity for a caller-selected time;
- permission to rely on a claim or to accept project direction.
