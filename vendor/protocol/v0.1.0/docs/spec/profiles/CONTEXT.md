# Context profile `context/1` — release candidate

> **Status: Protocol 0.1 release candidate.** Accepted as milestone M4, with owner decisions M4-Q2, M4-Q3 and M4-Q4; §14 accepted with M5 (M5-Q7). Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC §2, §8, §9, §14](../SPEC.md). Task: [M4](../../work/release-0.1/M4.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/PREPARATION-AND-DELIVERY.md` and `docs/spec/SPEC.md` §7, §8 at `3278393`; pio `docs/spec/STANDALONE-CLIENT.md` §5, §6 at `e65b7c0`. Owner decisions M4-Q2, M4-Q3 and M4-Q4 (2026-09-15) are incorporated.

A **context provider** prepares exact, bounded context for a named consumer and reports what it could and could not supply. CBR is one context provider; any service that compiles cited context can implement this profile. A **caller** asks for context on behalf of work it will run. A **consumer** is the execution or harness the context is for.

The profile carries **exact packets and explicit gaps**, not understanding. A delivered packet never establishes comprehension, a static explanation never establishes runtime behavior, and a coverage frontier is never a simultaneous global snapshot (SPEC §14).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `context/1` depends on `core/1` with `core.events`, and on `evidence/1` as a **protocol dependency**: every packet revision is an exact sealed Evidence artifact (§5), so its identity, integrity, retention and fetch reuse EVIDENCE (owner decision M4-Q2). This does not require a separate running service: a standalone context provider MAY implement `evidence/1` for its own packets.
- Negotiating `context/1` does not require `evidence/1` in the same session. The packet reference names the evidence provider holding the bytes, and the reader negotiates `evidence/1` there, which may be this provider.
- **Timing semantics are negotiated explicitly** (SPEC §14): features `context.advisory`, `context.required_before_start` and `context.required_before_transition`. A request using an obligation whose feature was not negotiated is refused with `unsupported_required_feature` (CTX-9).
- Other optional features: `context.shared_jobs` (§4), `context.updates` (§8), `context.expand` (§6).
- A provider that cannot meet a requested boundary or delivery semantics refuses the request explicitly; it never silently degrades a required obligation (CTX-9).

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Request subject `{ "kind": "context.request", "id" }` | One caller's request for context. The caller chooses the ID before any execution exists (CTX-1). | An execution identity or a job identity |
| Job `{ "kind": "context.job", "id" }` | The provider's preparation work. Several compatible requests may share one job (§4). | A request; cancelling it is not cancelling a request |
| Packet `{ "kind": "context.packet", "id" }` with `revision` | One immutable result revision for a request | A mutable document |
| Packet reference `{ packet, revision, artifact: { provider, artifact, digest } }` | How Execution context bindings cite a packet (EXECUTION §13). `artifact` is the exact Evidence reference of that revision's sealed bytes and the evidence provider holding them. | A reference without the artifact and digest |
| Item `item_id` | One required or advisory item within a request | A claim identity (Knowledge is M5) |

When preparation leads to admission, the execution's context binding names the request and the packet reference, so the causal link from request to execution is preserved (CTX-1).

## 3. Requests

| Operation | Kind | Semantics |
|---|---|---|
| `context.request.submit` | command | Creates the request (subject `{ kind: "context.request", id }`, precondition revision 0) |
| `context.request.cancel` | command | Cancels that request only (§4); precondition on its current revision |
| `context.request.inspect` | query | `{ request }` → state, `reason`, job, consumer, limits, item results, and `packets`: each published revision with its packet reference, `current` and `supersedes` |
| `context.packet.inspect` | query | Result facts and a bounded excerpt (§6) |
| `context.expand` | query | Newly authorized cited content (§6, `context.expand`) |

`context.request.submit` payload:

| Member | Meaning |
|---|---|
| `consumer` | `{ task, principal, executor? }`: who the context is for |
| `basis` | Target basis manifest (CTX-2): `repositories: [ { id, tree, workspace: "clean" \| "dirty" \| "unknown", dirty: { snapshot_digest } \| null } ]`, optional `environment`, `configuration` and `build`, and `completeness: "complete" \| "partial"`. Each repository keeps its own tree. A repository whose workspace is not `clean` but whose `dirty` is `null` is a commit-only basis, and the basis MUST then declare `completeness: "partial"`; otherwise the request is `invalid_envelope` at `/payload/basis/completeness`. |
| `items` | Required and advisory items (CTX-3): `{ item_id, selector: { kind, value }, obligation: "advisory" \| "required_before_start" \| "required_before_transition", transition?, reliance: "binding" \| "evidence" \| "hypothesis" \| "reference", selected_by, check }`. `selected_by` names the principal or authority that chose the item. `transition` is present exactly when the obligation is `required_before_transition`. `check` states how satisfaction is decided: `{ kind: "source_included", repository, path }`, `{ kind: "evidence_included", evidence }` or `{ kind: "authority_content_included" }` (the item's authority content at its current authority revision). "Understand the repository" is not a checkable item; an unknown check kind is `invalid_envelope`. |
| `fallback` | For advisory items only: `proceed_with_gap` or `wait_until_deadline` (CTX-7) |
| `limits` | Three separate limits (CTX-4): `deadline` (provider clock), `investigation` budget `{ units, amount }` for the provider's own work, and `output_capacity` `{ units: "bytes", amount }` for the section content of the packet. Exhausting one is reported as that limit and never as another. |
| `authority_content` | Optional binding content supplied directly by an authority: `{ item_id, evidence: reference, authority_revision }` (CTX-14) |
| `origin` | Optional, for requests made by a job's own investigation: `{ initiator, depth, call_budget }` (CTX-19) |

Outcome: `{ request, state, job?, reason?, needed? }`, where `state` is `preparing` or `refused`: a submit never returns a finished preparation. Request states, as `context.request.inspect` reports them: `preparing`, `ready`, `partial`, `unmet`, `refused`, `cancelled`.

- An unknown obligation, reliance label or check kind is `invalid_envelope`; an obligation whose feature was not negotiated is `unsupported_required_feature` with `features`, decided at CORE §10 step 3. A provider MUST NOT accept it under another obligation.
- A request whose mandatory items cannot fit `output_capacity` ends `refused` with reason `budget_insufficient` and `needed: { units: "bytes", amount }`, the size the mandatory items need (CTX-8). A provider MUST NOT drop mandatory items to fit. Advisory content that does not fit is omitted with reason `output_capacity`.
- **After preparation** the request is `ready` when every item is satisfied, `partial` when every required item is satisfied and an advisory item is `degraded`, and `unmet` when a required item is `unmet`.
- **Item results:** while preparing, `satisfied` for items the prepared content already meets and `pending` for the rest; after preparation `satisfied`, `unmet` with `reason` (`deadline_passed`, `investigation_budget_exhausted`, `output_capacity`, `corrected_during_preparation`, `unavailable`, or a provider reason), or `degraded` with `reason` for advisory items under their fallback.

## 4. Jobs, subscribers and cancellation

- A provider MAY attach a request to an existing compatible job that has not yet published: same basis, same items, and the same access scope. In the reference, the same access scope means the same submitting principal; a request from another principal never joins another principal's job. Each attached request is a separate subscriber (`context.shared_jobs`). Without that feature negotiated for the submitting session, a request always gets its own job.
- `context.request.cancel` (command) cancels **that request only**. Outcome `{ request, state: "cancelled", job, job_continues }`. The job continues while any other request still needs it (CTX-10); a cancelled request receives no further packet revisions. Cancelling a request already `ready`, `partial`, `unmet` or `refused` is `not_found`: nothing is being prepared for it. Ending the job itself is a provider decision reported as `context.job.ended` with its reason, for example an exhausted investigation budget or no remaining subscribers.
- A consumer waiting for context MUST NOT be required to hold an execution reservation (CTX-18). If preparation needs an execution, the provider submits it as an ordinary, separately authorized caller, with `origin` naming the job, its depth and call budget. Such an execution is never automatically re-enriched through the same path (CTX-19).
- **No call cycles.** The executor may read this provider's packets at any moment, including while this provider is submitting to it or waiting for its investigation. A context provider SHOULD keep answering reads while its investigation executions run, and SHOULD NOT wait for an executor while unable to answer that executor. The reference provider follows its investigations from a worker, outside its processing lock.

## 5. Results and packets

When a request reaches `ready`, `partial` or `unmet`, the provider publishes a packet revision (revision 1, or a later one under §8). The packet subject is `{ kind: "context.packet", id }` with the request's ID. Its result facts:

| Member | Meaning |
|---|---|
| `reference` | The packet reference (§2) |
| `request`, `job`, `current`, `supersedes?` | The request, the job, whether this revision is the request's current one, and the revision it supersedes (§8) |
| `selected` | Selected source revisions (`{ repository, path, tree }`) and Evidence references (CTX-5) |
| `provenance` | `{ compiler, job, basis }`: compiler and job identity, and the input basis |
| `sections` | `[ { section_id, item_id?, label, length, citations, historical } ]`, in packet order. `label` is required (CTX-20): `binding`, `observation`, `hypothesis`, `unknown`, `declared_requirement`, `source_inspected`, `runtime_observed`, `inferred` or `stale`. `historical: true` marks content kept for the record that is not current for its item (§8). |
| `inclusions` / `omissions` | Section IDs included, and `[ { item_id?, section_id?, reason } ]` with reason `output_capacity`, `authorization`, `applicability` or `unavailable` |
| `citations` | `[ { citation_id, evidence: { provider, artifact, digest } } ]` |
| `coverage` | Per-producer frontiers and gaps: `[ { producer, frontier, gaps } ]` (CTX-6). There is no global cursor. |
| `items` | `[ { item_id, obligation, result, reason? } ]` |
| `applicability` | `{ basis, conditions }`: the basis the packet is valid for, and typed invalidation conditions (§8, CTX-15) |
| `authority` | `[ { item_id, authority_revision, evidence, coverage } ]` for authority-supplied content: the authority revision the packet was prepared against, after any correction during preparation, and what it rests on (CTX-14). Content from an older revision appears only as historical (§8). |

**Packet bytes.** The sealed artifact holds `{ "format": "combraton-context-packet/1", request, revision, supersedes?, sections: [ { section_id, item_id?, label, historical, content, citations } ], items, coverage, omissions, applicability, authority }` as canonical JSON (ENCODING), with media type `application/vnd.combraton.context-packet+json`. A later format is a new `format` value.

- **Producer.** A packet sealed in the provider's own store names the provider (its `provider_id`) as producer principal. A packet sealed at a separate evidence provider names the principal the context provider authenticated as there (§12).
- **One revision per publication.** A publication raises the request's revision once; `context.packet.published` and any `context.request.changed` share it.
- **Packet-to-artifact mapping** (M4-Q2). Each packet revision maps immutably to exactly one sealed Evidence artifact, named by provider, artifact and digest. The mapping is recorded when the revision is published and never changes. A later revision maps to a different artifact.
- **Exact bytes** (CTX-5). The complete packet bytes are the sealed artifact's bytes, fetched with `evidence.fetch` at the named provider. A provider MUST NOT regenerate them.
- **Missing required items stay unmet** (CTX-7). When the deadline passes, every required item not supplied is `unmet`. It is never reported `satisfied`, and it is never downgraded to advisory. Advisory items follow their declared fallback.
- **No unobserved claims** (CTX-14). Coverage frontiers name only what the provider actually ingested. A packet MUST NOT claim a frontier beyond it.

## 6. Inspect and expand

| Operation | Kind | Semantics |
|---|---|---|
| `context.request.inspect` | query | Request state, job, packet references and item results |
| `context.packet.inspect` | query | `{ packet, revision, offset?, max_bytes? }` → the result facts of §5 and an **excerpt** `{ offset, length, size, data_base64 }`: an exact byte range of the packet, bounded by `max_bytes` (default 4096) and the caller's receive limit (CTX-11). The result facts' `omissions` are its omission markers. An excerpt is never presented as the complete sealed bytes: it carries no digest, even when the range covers the whole packet. Complete bytes come from `evidence.fetch`, checked against the reference's digest. |
| `context.expand` | query | `{ packet, revision, citation, offset?, max_bytes? }` → `{ citation, evidence, excerpt }`: a byte range of the cited artifact that the principal is authorized to read, including content omitted from the packet for authorization. It needs `context.packet.read` on the packet and `evidence.read` on the cited artifact at this provider. A citation the principal may not read, a citation that does not exist and a citation held at another provider the reader must query there are all `permission_denied`, identical to each other (CTX-11, CORE-12). A readable citation whose digest is not the artifact's sealed digest is `artifact_digest_mismatch`. |

## 7. Direct fetch by an executor

Grants are provider-local (CORE §15.2): a grant's audience is the provider that issued it. A grant at the context provider therefore never authorizes a read at a different evidence provider (owner decision M4-Q3). For an executor to read a packet directly (CTX-16), the caller arranges **one grant per audience**, each held by the executor's principal and bound to the exact object:

| Audience | Right | Resource |
|---|---|---|
| The context provider | `context.packet.read` | `{ kind: "context.packet", id }` of that packet |
| The evidence provider named in the packet's artifact reference | `evidence.read` | `{ kind: "evidence.artifact", id }` of that revision's artifact |

- The executor reads the result facts at the context provider under the first grant, then fetches the bytes at the evidence provider under the second. It checks the fetched digest against the binding.
- When the context provider implements `evidence/1` for its own packets, both grants are issued there.
- A grant for another packet or artifact, a grant presented at the other provider, or a grant held by another principal is refused. Possessing a packet reference authorizes nothing.

## 8. Updates, corrections and staleness

- **Updates are new revisions** (CTX-12, `context.updates`). A finding during work is published as a new packet revision with its own basis, digest and `supersedes: { revision }`, mapped to a new artifact. Earlier revisions stay retrievable with their original artifact and digest, report `current: false`, and old bytes are never relabeled current. A provider publishes later revisions only for requests submitted in a session that negotiated `context.updates`. Delivery of an update to a running execution is an Execution observation (EXECUTION §13); a late update is recorded `late` (CTX-17).
- **Corrections after publication.** A correction to an authority item after a revision was published invalidates that revision's content for the item. The published facts never change. `context.packet.inspect` reports, at the read, `invalidated_items: [ { item_id, authority_revision } ]` for authority revisions corrected after the revision was prepared, and `superseded_by: { revision }` naming the request's current revision when a newer revision exists. Supersession and invalidation are distinct: a superseded revision with no invalidated items may still be applicable to its selected basis (EXECUTION §13.1 pinning).
- **Correction during preparation** (CTX-13). When an authority supplies a correction to a binding item while a job runs, a derivation made from the older binding MUST NOT become the current result for that item. The provider rebuilds within budget, marks the older result `historical`, or refuses current reliance for that item. Observably: a section derived from the older authority revision is `historical: true` with label `stale`, and the item is `unmet` with reason `corrected_during_preparation` unless a section at the corrected revision is included.
- **Stale basis** (CTX-15, owner decision M4-Q4). A packet's `applicability` carries typed, checkable **conditions**, for example `{ condition_id, kind: "repository_tree", repository, expected }`, `dirty_snapshot`, `environment_digest` or `authority_revision`. An executor bound to the packet checks, at each relevant boundary, only the conditions it can actually observe, and reports each as `match`, `mismatch` or `unavailable` (EXECUTION §13.1). The executor decides nothing about semantic truth and is not an applicability engine: conditions it cannot observe are `unavailable`, which is neither fresh nor necessarily stale.

## 9. Provider unavailability

- A consumer whose context provider is unavailable proceeds according to its obligations (SCN-8): advisory bindings proceed with a visible gap, and required bindings keep the work from being admitted as ready until they are satisfied, the deadline passes (`unmet`), or the authorized caller changes the requirement.
- A provider outage never turns a required item into `satisfied`.

## 10. Rights

| Right | Covers |
|---|---|
| `context.request` | `context.request.submit` and `context.request.cancel` on covered requests |
| `context.read` | `context.request.inspect`, events of covered requests, and events of their jobs |
| `context.packet.read` | `context.packet.inspect` and `context.expand` on covered packets (`{ kind: "context.packet", id }`) |

Expanded content is additionally subject to the Evidence rights of the cited artifact. A `context.packet.inspect` excerpt is part of the result facts and needs only `context.packet.read`; the complete packet bytes come from `evidence.fetch` under `evidence.read` at the evidence provider.

## 11. Events

Context events use Core event records. *Candidate* types:

| Type | Subject | Payload |
|---|---|---|
| `context.request.changed` | request | `{ state, job?, reason? }` |
| `context.packet.published` | request | `{ reference, items }` |
| `context.job.ended` | job | `{ reason }` |
| `context.request.cancelled` | request | `{ job_continues }` |

A packet published by the provider appears as a provider-origin event (CORE §16) together with the sealed artifact's Evidence events when the provider holds the artifact itself.

## 12. Conformance and test controls

- **Reference participants only.** A reference context provider drives a **scripted preparation** selected by launch configuration: packet content per request, delays against the clock file, coverage frontiers, unmet items, mid-job corrections, budget exhaustion and outages. The script is test environment, normative only for its conformance tests (as EXECUTION §15, owner decision Q4). No fixture depends on CBR's memory engine.
- **Script vocabulary** (launch configuration `context`, test control `context.script`): `scripts` maps the ID of the request that starts a job to a list of one-key steps, and `default_script` applies otherwise. Steps: `wait_until` (an instant), `investigate` (units of investigation budget spent), `section` (`section_id`, `item_id?`, `label`, `content`, `citations?`, `authority_revision?`, `source?`), `coverage` (`producer`, `frontier`, `gaps`), `unmet` (`item_id`, `reason`), `omit` (`item_id?`, `reason`), `correction` (`item_id`, `authority_revision`), `conditions` (typed applicability conditions), `publish`, `end` (a job end reason) and `stall`. Steps advance no later than the provider's next request. A `publish` whose advisory items are unsatisfied under `wait_until_deadline` waits for the deadline.
- **Scripted unmet reasons.** A scripted `unmet` step never makes a satisfied item unmet: whether an item is satisfied is decided by its check. For an item that is not satisfied, the scripted reason takes precedence over every reason the item's check would give, including the claim reasons of §14 (`knowledge_unavailable` and `claim_digest_mismatch` too).
- **Mandatory size.** For `budget_insufficient`, the reference provider counts the content of the scripted sections for required items.
- **Scripted content and checks.** A scripted `section` is content the provider prepared; whether it satisfies its item is still decided by the item's `check`: `source_included` needs a section whose `source` names the check's repository and path, `evidence_included` a citation of the exact artifact and digest, and `authority_content_included` a section at the item's current authority revision.
- **Conformance conventions** (as EXECUTION §15.1: normative only for the conformance provider, never for callers): a job started by request `r` has ID `r`, and packet revision `n` of request `r` is sealed as artifact `packet.<r>.<n>`. The provenance `compiler` names the implementation that compiled the packet and is not a convention.
- **Separate participants** (owner decision M4-Q3). Cross-profile scenarios (SCN-1, SCN-4 to SCN-6, SCN-8, SCN-9, SCN-11) run separate reference participants, one per role: context provider, evidence provider and executor, each with its own isolated store. They talk only over public protocol connections, authenticated as their own principals and authorized by grants. A shared executable is acceptable; shared stores and private calls are not.
- **Packets at a separate evidence provider.** The reference context provider, configured with an evidence provider, seals each packet revision there as an ordinary producer: it authenticates as its own principal, uses a grant issued by that provider, and uses deterministic command IDs so a retried publication replays. Until the artifact is sealed, the revision is not published and the request stays `preparing`.

## 13. What context does not establish

A packet establishes which exact bytes, with which labels, coverage and gaps, the provider produced for a request. It does not establish:
- that the consumer read, understood or followed it;
- that a labeled observation still holds after its basis changed;
- that coverage frontiers from different producers describe one moment;
- project acceptance of any claim; Knowledge reliance is M5.

## 14. Claims in packets

> Proposed M5 extension with owner decision M5-Q7 incorporated ([M5](../../work/release-0.1/M5.md#owner-decisions-2026-09-15)). Nothing in this section applies to sessions that did not negotiate `context.claims`; §1–§13 are unchanged for them.

A packet that carries claims preserves Knowledge identity, reliance and applicability (SPEC §2, KNOWLEDGE §12). Its sections are immutable snapshots. Later changes are reported separately at the read, and they make required items unsatisfied through the same invalidation path as authority corrections (§8).

**Negotiation and format.**
- **Feature** `context.claims`. A provider with it reads claims at a knowledge provider, its own or a peer, as an ordinary reader under a grant held there (§7 applies to that audience too).
- **Packet format** `combraton-context-packet/2`. It adds the section member `claim`; format `/1` packets never carry it. A request submitted without `context.claims` gets `/1` packets and M4 behavior.
- **Check kind** `{ kind: "claim_included", claim: reference }` for items. In a request whose session did not negotiate `context.claims`, it is `unsupported_required_feature` with `features` including `context.claims` (the order of `features` is not significant).
- **Recomputing digests.** The provider recomputes each claim record's digest from what it read (KNOWLEDGE §3). A record whose digest differs from the reference is never carried or relied on: its section is omitted with reason `unavailable`, and its item is `unmet` with reason `claim_digest_mismatch`.

**Section claim snapshot**, as read when the packet was prepared:

`claim: { reference, plane, reliance: { state, decision?, permitted_use?, author_is_decider? }, applicability: { result, evaluation? }, conflicts: [ { conflict, kind, status } ], support: { class } }`

- `reference` is the exact revision reference, never "latest".
- `applicability` is the latest evaluation of that exact revision for a target equal to the packet's basis converted to a Knowledge target: each repository's `id` and `tree`, and its `dirty` snapshot only when the basis gives one; `environment` and `build` when present; and `completeness`. `workspace` and `configuration` have no Knowledge counterpart and are dropped. With no such evaluation, `result` is `unknown` and `evaluation` is absent.
- `conflicts` lists only open conflicts.
- The snapshot reflects the claim as the provider read it during preparation, no later than the packet's publication; the instant is provider-defined.

**What a claim section can satisfy.** Item reliance uses rank `binding` > `evidence` > `hypothesis` > `reference`. A `claim_included` item is satisfied when a section for that item that is not `historical` carries exactly its reference and:

| Item reliance | Needs | Otherwise `unmet` with reason |
|---|---|---|
| `binding` or `evidence` | reliance `accepted_for_use` with `permitted_use` of at least the item's rank, and applicability `applicable` | `not_accepted`; `invalid_for_target`; `applicability_not_established` for `needs_check` or `unknown` |
| `hypothesis` or `reference` | reliance other than `rejected` | `not_accepted` |
| any | the claim could be read and its digest verified | `knowledge_unavailable` or `claim_digest_mismatch` |
| any | a section that is not `historical` | `invalid_for_target`, when the item's only section carrying its reference is historical and the reliance rows give no other reason |

**Labels cannot promote a claim, and applicability keeps its distinctions.**
- A section may be labeled `binding` only when its claim is `accepted_for_use` with `permitted_use: binding`. A proposed or rejected claim is never `binding`; the reference provider publishes such a section as `hypothesis`. An extracted instruction stays a `hypothesis` or `inferred` section until decided (KNW-9).
- Only `invalid_for_target` makes a claim section `historical: true` with label `stale`. A claim whose applicability is `needs_check` or `unknown` is not stale and not historical: its section keeps its label with `historical: false`, and its snapshot shows the result.
- Open conflicts involving a carried claim are listed in the snapshot. When a conflict is material to an item, the competing revisions are carried, or omitted with a stated reason, never dropped silently.

**At the read.** `context.packet.inspect`, in a session that negotiated `context.claims`, re-reads carried claims and reports:
- `claim_changes: [ { section_id, claim, change } ]`, at most one entry per change kind per section, in section order and then in this order of `change`:
  - `reliance_changed`;
  - `applicability_changed`, when the result for the basis differs (a re-evaluation with the same result is not a change);
  - `conflict_opened`;
  - `lineage_revised`, whenever the claim has a revision newer than the carried one, including one revised before the packet was prepared; this is information, not invalidity;
  - `unavailable`.
- **Invalidated items.** A required item satisfied at publication by a claim section that no longer meets the table above appears in `invalidated_items` as `{ item_id, claim, reason }`. `reason` is `permitted_use_lost` or `invalid_for_target`. For every reliance, a claim whose latest evaluation for the basis is now `invalid_for_target` would make its section historical, so the item is invalidated with `invalid_for_target`; `permitted_use_lost` comes first when both apply. These sit beside the authority corrections of §8.
- **Unverified items.** A required item satisfied at publication whose claim now cannot be read or verified, or, for a `binding` or `evidence` item, whose latest evaluation for the basis is now `needs_check` or `unknown`, appears in `unverified_items: [ { item_id, claim, reason } ]`. `reason` is `knowledge_unavailable`, `claim_digest_mismatch` or `applicability_not_established`. Unavailable knowledge is unknown, never valid.
- Packet bytes and published facts never change. A newer claim revision or a newer packet revision is never substituted. Supersession stays distinct from invalidity, as in §8.

- **Sessions without `context.claims`** see packet facts without `claim` members, `claim_changes` or `unverified_items`, and `invalidated_items` lists only authority corrections.
- **Knowledge source.** The reference provider reads claims in its own knowledge store, or at the provider named by launch configuration `context.knowledge_provider`.

Execution enforces these at the binding's own boundary under `execution.claim_revalidation` ([EXECUTION §13.3](EXECUTION.md#133-claim-revalidation-executionclaim_revalidation)).
