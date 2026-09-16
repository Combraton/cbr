# Evidence profile `evidence/1` — release candidate

> **Status: Protocol 0.1 release candidate.** Accepted as milestone M4, with owner decisions M4-Q1, M4-Q2, M4-Q5, M4-Q6 and M4-Q7. Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC §2, §8–§10, §12](../SPEC.md). Task: [M4](../../work/release-0.1/M4.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/SPEC.md` §3, §4, §10 and `docs/spec/PREPARATION-AND-DELIVERY.md` at `3278393`; pio `docs/spec/STANDALONE-CLIENT.md` at `e65b7c0`. Owner decisions M4-Q1, M4-Q2, M4-Q5, M4-Q6 and M4-Q7 (2026-09-15) are incorporated.

An **evidence provider** accepts immutable bytes with their provenance, keeps them under declared retention, and lets authorized principals find and fetch them. CBR is one evidence provider; a CI system or runtime observer publishing through this profile is another, and a non-Combraton evidence publisher must be able to implement it without any Combraton library (SPEC §13). A **producer** publishes evidence. A **reader** finds and fetches it.

The profile preserves **what was captured, by whom and under which coverage**. It does not establish that the content is true, complete beyond its declared coverage, or accepted by anyone (§13).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `evidence/1` depends on `core/1` with feature `core.events` (CORE §16). `core.grants` is optional; without it only authority principals operate.
- Required Core features are published here and enforced at negotiation, as for Execution (owner decision D2): a required `evidence/1` request without `core.events` is refused with `unsupported_profile` and a `dependency_not_selected` item; an optional request is left unselected with the same item.
- Optional features: `evidence.manifests` (§7) and `evidence.retention_control` (holds, release and purge, §9). Base providers still report availability and retention class.
- Every command follows the Core command path (CORE §10). Chunks, seals, holds and purges commit in owner transactions with their events.

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Artifact subject `{ "kind": "evidence.artifact", "id" }` | One published descriptor at one provider. The producer chooses the ID and creates it with precondition revision 0. | The content's identity |
| `digest` | Algorithm-qualified digest of the sealed bytes (ENCODING §3) | Proof of provenance or authorization |
| Evidence reference `{ provider?, artifact, digest }` | How other profiles cite evidence. `provider` is the evidence provider's `provider_id`. A reference without it means the provider being asked; references carried to other participants (packet references, execution outputs) always name it. A consumer MUST check the artifact **and** the exact digest. | A reference by locator, or an authorization |
| Hold subject `{ "kind": "evidence.hold", "id" }` | One retention obligation on one artifact (§9) | A copy of the artifact |
| `locator` | Where the provider stores or serves the bytes. Informative only (§3). | Identity, or something to fetch |

- **Exact identity.** A sealed artifact's bytes are immutable. Two artifacts with equal digests hold equal bytes, but they keep separate provenance, retention and authorization: equal bytes never establish equal provenance or permission.
- **References grant nothing.** Possessing a reference, a digest or a locator authorizes no read (§8).

## 3. Descriptor

Fixed at `prepare`, immutable once sealed:

| Field | Meaning |
|---|---|
| `digest`, `size`, `media_type` | The declared content, verified at seal (§4). The digest uses an algorithm this provider supports for digests (ENCODING §3: `sha256`, and `sha512` when its `core.describe` manifest lists `core.digest-sha512`). Another algorithm is `unsupported_digest_algorithm` with `algorithm` and `supported`, and a digest of the wrong length is `invalid_envelope` at `/payload/digest`. Both are decided at step 2, with the rest of the descriptor's validation. |
| `producer` | `{ principal, producer_id? }`. `principal` is always the session principal of the `prepare` command (EVD-4); a prepare naming another principal is `invalid_envelope` at `/payload/producer/principal`. |
| `source` | What was captured: `{ kind, id }`, for example a terminal stream, a test report, or a build log of a named execution |
| `scope` | The visibility scope the artifact belongs to; opaque to this profile |
| `capture` | `{ captured_at, uncertainty?: { not_before, not_after }, anchors }`; `not_before` after `not_after` is `invalid_envelope` at `/payload/capture/uncertainty`. Anchors identify code trees, dirty snapshots, build and environment identities (EVD-4). Capture time is metadata, never ordering. |
| `coverage` | Required (EVD-7): `{ completeness: "complete" \| "partial" \| "unknown", covered, gaps }`. A source of kind `terminal_output` MUST NOT declare `complete` coverage of a tool trace. |
| `work` | Optional work binding reference, for example an execution subject (§10) |
| `retention_class` | Provider-defined class name, reported, never inferred |
| `locator` | Optional and **informational**. A provider MUST NOT fetch from it implicitly, and a locator carrying credentials (user information, tokens or signed parameters) is `invalid_envelope` at `/payload/locator`. |

## 4. Upload and seal (owner decision M4-Q1)

Bytes travel **in-band**, in bounded base64 chunks over the negotiated binding. Out-of-band transfer is deferred to Remote trust.

| Operation | Kind | Semantics |
|---|---|---|
| `evidence.upload.prepare` | command | Creates the artifact in state `staged` (precondition revision 0). Outcome `{ artifact, state: "staged", received: 0, chunk_limit }`. |
| `evidence.upload.append` | command | Precondition on the artifact's current revision. Payload `{ offset, data_base64 }`. Appends the decoded bytes; each append raises the revision by one. Outcome `{ received }`. |
| `evidence.seal` | command | Precondition on the current revision. Verifies size and digest, then makes the artifact `sealed`. Outcome `{ state: "sealed", digest, size, already_sealed }`. |
| `evidence.upload.abandon` | command | Precondition on the current revision. Ends a staged upload: state `abandoned` with reason `abandoned_by_producer`, staged bytes discarded, descriptor kept as a tombstone. |

**Upload operations in detail** (owner approval, 2026-09-15):

| | `evidence.upload.append` | `evidence.upload.abandon` |
|---|---|---|
| Authorization (step 6) | `evidence.publish` covering the artifact | `evidence.publish` covering the artifact |
| Preconditions | the artifact's current revision | the artifact's current revision |
| Errors (step 7) | `not_found` without a staged upload; `upload_offset_mismatch` (`after_reconcile`: inspect `received`, then append from it with a new command); `upload_size_exceeded` (`no`: the declared size cannot grow) | `not_found` unless the artifact is staged. A sealed, held, purge-pending, purged or already abandoned artifact is never abandoned, so abandon is never a way to discard sealed or held content; that needs `evidence.purge` (§9). |
| State and revision | `received` grows; revision + 1 | state `abandoned`, staged bytes discarded; revision + 1 |
| Event | `evidence.artifact.appended { received }` | `evidence.artifact.abandoned { reason: "abandoned_by_producer" }` |
| Idempotency | The same command replays its outcome (CORE §6). A rejection binds nothing. | The same command replays its outcome. A new abandon command on the abandoned artifact is `not_found`. |

Append and abandon need a staged upload, and seal needs a staged or sealed artifact. On an artifact without one (sealed for append and abandon, abandoned for all three) they are `not_found`: the upload they act on does not exist. `evidence.inspect` still reports the artifact and its state.

**Chunk sizing.**
- `chunk_limit` is the largest number of **decoded** bytes one append may carry. A provider MUST choose it so that an append of that size, base64-encoded, with the envelope overhead the provider declares (`chunk_overhead_bytes`, in the prepare outcome), fits its negotiated payload and frame limits, and so that the base64 string fits its string limit. Base64 grows data by 4/3.
- An append with more than `chunk_limit` decoded bytes is `limit_exceeded` with `{ limit: "chunk_limit", maximum }`, even if its frame fits. Like the Core limits it names no subject, so it is decided at CORE §10 step 2, before authorization and preconditions. Data that is not valid padded base64 is `invalid_envelope` at `/payload/data_base64`.
- `chunk_limit` is the largest multiple of 3 whose base64 form fits every one of those limits.
- A frame over the frame limit is handled by the binding (`frame_too_large`) before any of this.
- `evidence.fetch` returns fewer bytes than requested when the encoded response would exceed the caller's receive limit, and at least one byte whenever one fits (as CORE §16.4 does for items).

**Interrupted uploads.**
- `evidence.inspect` on a staged artifact reports `received`: the bytes durably committed, and its current revision.
- A producer that lost an append response inspects first. If `received` includes the chunk, the append was committed; otherwise it retransmits the **same command** (same `command_id`, offset, bytes and precondition), which Core deduplication makes safe.

**Duplicate and conflicting chunks.**
- The same append command retransmitted replays its outcome (CORE §6).
- A **different** command sending bytes for an offset already received fails its revision precondition (`precondition_failed`), whether or not the bytes are identical. The provider never overwrites received bytes and never silently accepts a duplicate under a new command.
- An append whose precondition matches but whose `offset` differs from `received` is `upload_offset_mismatch` with `received`.
- An append that would exceed the declared `size` is `upload_size_exceeded`.

**Seal.**
- **Partial uploads cannot be sealed** (EVD-2): `upload_incomplete` with `received` and `size`. Nothing changes.
- **Digest mismatch**: `content_digest_mismatch` with the digest the provider computed from the received bytes. Nothing changes; the artifact stays `staged`. Because received bytes are never overwritten, the producer can only abandon the upload and prepare a new artifact.
- **Idempotent.** Retransmitting the seal command replays its outcome. A new seal command on an already sealed artifact, with a precondition on its current revision, returns `already_sealed: true` at the unchanged revision and appends no event.
- A provider MUST NOT report `sealed` for bytes it did not verify.
- **Staging timeout.** A provider MAY abandon a staged upload after a declared staging timeout; `evidence.inspect` then reports state `abandoned` with reason `staging_expired`, never `not_found`.

## 5. Inspect, query and fetch

| Operation | Kind | Semantics |
|---|---|---|
| `evidence.inspect` | query | `{ artifact }` → descriptor, state, revision, `received` while staged, availability (§9), holds the reader may see in hold ID order, and completeness for manifests (§7) |
| `evidence.query` | query | Filters on producer, source, work, media type, digest and scope; readable artifacts in artifact ID order, staged, abandoned and purged ones included; bounded page with `next_cursor` only when more remain; a cursor this provider did not issue is `invalid_cursor`; `filtered` as below |
| `evidence.fetch` | query | `{ artifact, digest, offset?, max_bytes? }` → `{ artifact, digest, offset, data_base64, next_offset, size }` for a sealed, available artifact |

- **Query filtering** (owner approval, 2026-09-15). `filtered` is `true` whenever the reader's authorization does not cover every artifact, whether or not an unreadable artifact matched; adding or removing an unreadable matching artifact changes nothing in the reader's result. It never reveals that an unreadable artifact matches the filters, which a digest filter would otherwise turn into an existence test.
- **Not sealed.** Fetch of a staged or abandoned artifact is `not_found` to an authorized reader: no sealed content exists. `evidence.inspect` reports its state.
- **Exact bytes.** Fetched bytes are the sealed bytes; a provider MUST NOT regenerate, re-encode or normalize them.
- **Reference mismatch.** A fetch whose `digest` differs from the artifact's sealed digest is refused with `artifact_digest_mismatch` and no bytes. Sealed content is immutable, so this never means "fetch the latest content": the reference names other bytes, or a different artifact, than the one requested.
- **Integrity on read.** Stored bytes that no longer match the digest are never served; availability becomes `unavailable` with reason `integrity_failed`.
- **Not available.** For a sealed artifact whose availability is not `available`, fetch returns `{ artifact, digest, offset, size, availability }` with empty `data_base64` and `next_offset` equal to `offset`. It never serves partial, pending or unverified bytes as the sealed content.
- `evidence.inspect` and `evidence.query` report availability as observed at the read, including an integrity failure. A query records nothing, so a failure detected during a read is reported, not recorded as an event.

## 6. Errors

All are decided at CORE §10 step 7, **after** authorization (step 6) and after the revision preconditions. A principal not authorized for the artifact gets the step 6 `permission_denied`, identical for existing and nonexistent artifacts; it never receives these errors, their details, or any digest (CORE-12).

| Code | Retry | Raised by | State change | `details` |
|---|---|---|---|---|
| `upload_offset_mismatch` | `after_reconcile` | append | none | `received` |
| `upload_size_exceeded` | `no` | append | none | `received`, `size` |
| `upload_incomplete` | `after_reconcile` | seal | none | `received`, `size` |
| `content_digest_mismatch` | `no` | seal | none; the artifact stays `staged` | `computed` |
| `artifact_digest_mismatch` | `no` | fetch, and any operation taking an evidence reference | none | — |
| `hold_active` | `after_reconcile` | purge | none | `holds`: IDs of the blocking active holds the caller may see, in ID order; `filtered: true` when others exist |

**After `upload_incomplete`.** A rejection binds nothing (CORE §10), but the original seal command cannot simply be retransmitted. The caller must first append the missing bytes, and each append raises the artifact's revision, so the seal's revision precondition is then stale. The caller seals with a precondition on the new current revision. Reusing the original `command_id` is allowed, because the rejection bound nothing, but its intent has changed; callers SHOULD use a new `command_id`. The same seal command remains valid only if nothing has been appended since, and then it can only fail the same way. Fixture `evidence.partial-upload-then-append-then-seal` covers this.

## 7. Manifests (`evidence.manifests`, owner decision M4-Q7)

- **Format.** A manifest artifact has media type `application/vnd.combraton.evidence-manifest+json` and content `{ "format": "combraton-evidence-manifest/1", "children": [ { role, evidence: { provider?, artifact, digest }, required } ] }`. Children name the exact artifact **and** digest. A later format is a new `format` value, never a silent change.
- **Validation at seal.** Content that is not a valid manifest of a supported format is refused with `invalid_envelope` at `/payload` of the seal.
- **Completeness is per reader.** `evidence.inspect` on a manifest reports, for that reader:
  - each child's state: `present` (sealed, available, digest matches, readable by this reader), `missing` (known absent to a reader who may know: no such artifact, purge pending or purged, abandoned or unsealed, or the artifact's digest differs), `unverified` (held at another provider, or not currently available: unavailable, partial, or failing integrity), or `withheld` (the reader may not read the child, whether or not it exists);
  - `completeness`: `complete` when every required child is `present`; `incomplete` when a required child is `missing`; otherwise `undetermined`.
- A provider MUST NOT report a manifest `complete` when a required child is not present, and MUST NOT turn `withheld` or `unverified` into `missing`, which would leak existence or claim knowledge it lacks.
- A manifest authorizes nothing about its children.

## 8. Authorization and rights

Under a grant (CORE §15). Grants are provider-local: a grant's audience is the provider that issued it, so a grant at a Context provider never authorizes reads at an Evidence provider.

| Right | Covers |
|---|---|
| `evidence.publish` | prepare, append, seal and abandon on covered artifacts |
| `evidence.read` | inspect, query results and fetch of covered artifacts, and their events |
| `evidence.hold` | placing a hold on a covered artifact |
| `evidence.release` | releasing a hold on a covered hold subject (§9) |
| `evidence.purge` | purge of a covered artifact |

Resources name kind `evidence.artifact` or `evidence.hold`, with optional `id` or `id_prefix`.

## 9. Availability and retention (owner decision M4-Q6)

- **Availability**, observed separately from state: `available`, `partial`, `unavailable` (with `reason`), `purge_pending`, `purged`. Staged and sealed artifacts are `available` while their stored bytes are intact; an abandoned artifact is `unavailable` with its abandonment reason; `partial` means some sealed bytes are known lost and nothing is served as the sealed content.
- **Holds** (`evidence.retention_control`).
  - `evidence.hold` (command) creates a hold subject on an artifact (precondition revision 0 on the hold), with `{ artifact, holder_ref, reason, expires_at? }`. The creating principal is the hold's **owner**. `expires_at` must be later than the provider clock, otherwise `invalid_envelope` at `/payload/expires_at`. A hold on an artifact that is not sealed, or whose purge was already requested, is `not_found`: nothing would be retained.
  - **Visibility.** A hold is visible to its owner, to authority principals, and to any reader of the held artifact; hold events follow the same rule.
  - **Expiry.** When the provider clock reaches `expires_at`, the hold becomes `expired` with the provider-origin event `evidence.hold.expired`. It no longer blocks a purge, and releasing it is `not_found`.
  - `evidence.release` (command) releases a hold, with a precondition on the hold's revision. It may be issued by the owner, by an authority principal, or under a grant with `evidence.release` covering that hold (grants are issued and delegated under CORE §15). Releasing a hold that is not active is `not_found`.
  - A released or expired hold stays inspectable to those who may see it; hold states are `active`, `released` and `expired`.
- **Purge.** `evidence.purge` (command) on an artifact.
  - **Preconditions:** the artifact's current revision, and each hold named in `release_holds` at its current revision, so concurrent hold changes are detected.
  - **Authorization, step 6:** `evidence.purge` on the artifact, and release authority for **every** named hold. Naming a hold is intent, not authorization: purge permission never bypasses another holder's retention.
  - **Active holds, step 7:** if any active hold remains that is not named, the purge is refused with `hold_active`.
  - **Commit, one owner transaction:** release every named hold (each gets a revision and an event), make availability `purge_pending`, and record the durable proof-loss record.
  - **Physical deletion** is confirmed separately. When the store confirms it, availability becomes `purged` with its own event. Until then the provider reports `purge_pending`: deletion requested, not yet confirmed. A store that confirms deletion within the purge's own transaction reports `purged` in the outcome, with both events.
  - **Event order.** The purge appends the artifact's own events first (`purge_requested`, then `purged` when confirmed at once), then one `evidence.hold.released` per named hold, as CORE §16.3 orders a command's events.
  - The descriptor remains as a tombstone. A purged artifact is never `not_found` to a principal who may read it, and never silently disappears from `query`.
  - **Repeated purge.** A new purge command on an artifact already `purge_pending` or `purged`, with a precondition on its current revision, returns its current availability with empty `released_holds` at the unchanged revision and appends no event. Holds it names are still authorized at step 6.
- **Proof-loss record** (EVD-6): `{ artifact, digest, requested_at, confirmed_at?, released_holds, affected, coverage }`.
  - `affected` lists the known dependencies the **reader** may inspect, as `{ kind, subject }` in kind then ID order. Kinds: `evidence.hold`, `evidence.manifest_child` (a manifest listing the artifact), and, where a provider tracks them, `context.packet_citation` and `execution.output` for packets citing it and outcome references (0.1 providers are not required to track these two).
  - `coverage` declares the dependency kinds the provider tracks (`tracked`), and `filtered: true` when dependencies were withheld from this reader. It never exposes a dependency the reader cannot inspect. A provider may track more kinds than another; `tracked` says which.

## 10. Work bindings

- A grant issued for a unit of work names the destination through its resources (for example an `id_prefix` for that work's artifacts) and the producer through its holder. The artifact's `work` names the bound work.
- A prepare outside the grant's resources is `permission_denied` with `out_of_scope`.
- **Bound work** (feature `evidence.work_binding`). A grant is bound to a unit of work only through an explicit, typed constraint: `constraints: [ { kind: "evidence.work_binding", work: { kind, id } } ]` (CORE §15.3). Destination and work are independent, and both must match:
  - the artifact must be covered by the grant's resources (otherwise `out_of_scope`);
  - the descriptor's `work` must equal the constraint's `work`, and a missing `work` does not match (otherwise `permission_denied` with `binding_violation`, after rights and scope, at step 6);
  - neither widens the other, and resources of other kinds in the same grant, for example read access to an execution, never imply a work binding;
  - issuing such a constraint needs `evidence.work_binding` negotiated in the issuing session; a delegated grant keeps it.
- An executor that publishes its output as evidence is an ordinary producer under such a grant.

## 11. Execution outcome references (EXE-21)

Cross-profile with EXECUTION §6:
- A completion record carries `outputs: [ { role, evidence: { provider, artifact, digest } } ]` when the executor negotiated `execution.evidence_outputs`, and `execution.inspect` lists them.
- Each reference names an artifact the executor sealed under its work binding; its digest MUST equal the sealed digest. An executor MUST NOT list an artifact it has not sealed.
- A caller resolves a reference at the named evidence provider under its own grant there; the reference authorizes nothing.

## 12. Events

Subjects `evidence.artifact` or `evidence.hold`. *Candidate* types:

| Type | Payload |
|---|---|
| `evidence.artifact.staged` | `{ descriptor }` |
| `evidence.artifact.appended` | `{ received }` |
| `evidence.artifact.sealed` | `{ digest, size }` |
| `evidence.artifact.abandoned` | `{ reason }` |
| `evidence.hold.placed` / `evidence.hold.released` / `evidence.hold.expired` | `{ artifact, holder_ref }` |
| `evidence.availability.changed` | `{ availability, reason? }`, when the provider records an availability change outside a command, for example from a store integrity scan |
| `evidence.artifact.purge_requested` | `{ requested_at }` |
| `evidence.artifact.purged` | `{ confirmed_at }`; the proof-loss record is read through `evidence.inspect`, filtered for the reader |

## 13. What evidence does not establish

A sealed artifact establishes that these exact bytes were received from this authenticated principal, with this declared provenance and coverage. It does not establish:
- that the content is true, or that a claim derived from it holds;
- completeness beyond the declared coverage; terminal output is not a complete tool trace;
- independence: shared material published twice is not corroboration;
- availability beyond its retention class and holds.

## 14. Conformance and test controls

- **Reference participants only.** Fixtures use a reference evidence provider with a scripted store; no fixture depends on CBR's storage.
- **Store controls** in launch configuration (decision 007), `evidence_store`: `corrupt` (stored bytes that fail integrity), `unavailable` (artifacts the store cannot serve), `staging_timeout_seconds`, `deletion_delay_seconds` (physical deletion is confirmed that long after the purge request, on the controlled clock), and the adversarial `serve_altered_bytes`: fetch returns those artifacts' bytes altered, while keeping the sealed digest and `available` metadata, so readers are tested on verifying what they assemble. Participants declare `claims.test_controls: ["evidence.store"]`. Holds and purges run through the protocol operations; no operation is reserved for testing.
