# Knowledge profile `knowledge/1` — release candidate

> **Status: Protocol 0.1 release candidate.** Accepted as milestone M5, with owner decisions M5-Q1 to M5-Q4 and M5-Q8 ([M5](../../work/release-0.1/M5.md#owner-decisions-2026-09-15)). Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Architecture: [SPEC §2, §8, §9](../SPEC.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/SPEC.md` §3–§6, §9–§11 and `docs/spec/INTERNALS.md` §1–§3 at `3278393`; combraton `docs/architecture/MODEL.md` §3–§5 at `9af69ce`.

A **knowledge provider** keeps scoped claims with their lineage, support, conflicts, applicability evaluations and reliance decisions. CBR is one knowledge provider; any service that keeps challengeable claims can implement this profile. A **producer** proposes and revises claims. An **authority** records reliance decisions for a bound scope. A **reader** inspects claims and their history.

The profile keeps **separate records for separate questions**:
- evidence says which bytes were received (EVIDENCE);
- a claim revision says what someone proposed, on what support;
- a reliance decision says what an authority permits the claim to be used for;
- an applicability evaluation says how a revision relates to one target basis.

None of these is derived from another, and none is stored as one status field (KNW-3).

The profile compares **structure, not meaning**. Every rule below uses exact equality of identifiers, qualifier values, trees and canonical JSON values. It is not a semantic reasoning engine. Where structure cannot settle a question, the answer is reported as uncertain, never guessed.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `knowledge/1` depends on `core/1` with `core.events`, and on `evidence/1` as a protocol dependency: support is cited by exact Evidence reference (§5). As for Context, this does not require a separate running service, and negotiating `knowledge/1` does not require `evidence/1` in the same session.
- Required Core features are published here and enforced at negotiation, as for Execution and Evidence: a required `knowledge/1` request without `core.events` is refused with `unsupported_profile` and a `dependency_not_selected` item. `core.grants` is optional; without it only authority principals operate. Deciding still needs the authority binding (§6).
- Every command follows the Core command path (CORE §10). A claim revision, a decision, an evaluation or a conflict record commits in one owner transaction with its events.

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Claim `{ "kind": "knowledge.claim", "id" }` | One stable claim lineage at one provider. The producer chooses the ID. Its Core revision is the number of its latest claim revision. | A statement's content |
| Claim revision reference `{ provider, claim, revision, digest }` | One immutable revision. `digest` is the canonical digest (ENCODING) of the revision record (§3). | A reference to "the latest" revision |
| Decision `{ "kind": "knowledge.decision", "id" }` | One immutable reliance decision about one claim revision (§6) | A property of the claim |
| Evaluation `{ "kind": "knowledge.evaluation", "id" }` | One immutable applicability evaluation of one revision against one target basis (§8) | A current status of the claim |
| Conflict `{ "kind": "knowledge.conflict", "id" }` | One potential or demonstrated conflict or drift between two revisions (§7) | A deletion or a winner |
| Authority binding `{ "kind": "knowledge.authority", "id": scope }` | The single accepting authority for scope `scope`. The subject ID is the scope ID, so a scope has at most one binding (§6). | A grant |

- **Revisions are immutable.** A revision's record never changes once proposed. Supersession, rejection, challenge and corrected validity are later records; they never edit the revision (KNW-1, KNW-8).
- **References grant nothing.** Possessing a claim reference authorizes no read, as for evidence.
- **Provider-qualified references.** Every reference to a claim or artifact names its provider. A digest alone never identifies a claim, an artifact or its provenance. A command naming a claim revision under another provider (a decision, a conflict, an evaluation) gets `not_found`: that revision is not held here, even when a local claim has the same ID and revision.

## 3. Claim revisions (M5-Q1)

| Operation | Kind | Semantics |
|---|---|---|
| `knowledge.claim.propose` | command | Subject `{ kind: "knowledge.claim", id }`, precondition revision 0. Creates revision 1. |
| `knowledge.claim.revise` | command | Subject is the claim, with a precondition on its current revision. Creates the next revision. |

Both carry the same content members; `revise` adds `supersedes`. Outcome: `{ reference }`.

| Member | Meaning |
|---|---|
| `plane` | `normative` (what should hold), `observed` (what an observation reports), or `interpretive` (an explanation or inference). An unknown plane is `invalid_envelope` (KNW-2). |
| `statement` | `{ subject: { kind, id }, predicate, value, cardinality }`. `value` is canonical JSON of at most 4096 bytes. `cardinality` says whether the predicate admits several values at once for one subject: `single`, `multiple` or `unknown`. |
| `scope` | `{ id, qualifiers }`. `id` names the scope. `qualifiers` is an object of string values that narrows it, for example `{ "environment": "staging" }`; an empty object means no narrowing was declared. |
| `validity` | `{ from?, until? }`, instants. The half-open interval `[from, until)` when the claim holds or was observed. An absent bound is **unknown**; it is never filled from the recorded time and never read as unbounded. When both are present, `from` MUST be before `until`: an empty or inverted interval is `invalid_envelope` at `/payload/validity` (step 2). |
| `basis` | Optional target anchors the claim concerns: `{ repositories: [ { id, tree } ], environment?, build?, completeness: "complete" \| "partial" }` |
| `support` | `[ { support_id, evidence, ancestry } ]` (§5) |
| `derivation` | `{ kind: "deterministic" \| "model_assisted" \| "human", record?, inputs }`. Descriptive only: it never grants or denies any permission (§6). `record` and `inputs` are Evidence references. |
| `dependencies` | `[ { provider, claim, revision, digest } ]`: revisions this claim's applicability depends on (§8) |
| `conditions` | Typed applicability conditions: `{ condition_id, kind: "repository_tree", repository, expected }`, `{ condition_id, kind: "dirty_snapshot", repository, expected }` or `{ condition_id, kind: "environment_digest", expected }`. These are the Context condition kinds (CONTEXT §8). |
| `health` | Optional, only on `observed` claims: `healthy`, `degraded`, `failing` or `unknown`, as the observation reports it. On any other plane it is `invalid_envelope` at `/payload/health`. |
| `supersedes` | `revise` only: `{ revision, digest }` of the lineage's current revision |

**Revision record and digest.** The provider records:

`{ "format": "combraton-knowledge-claim/1", provider, claim, revision, producer, plane, statement, scope, validity, basis, support, derivation, dependencies, conditions, health, supersedes }`

`producer` is the session principal. An absent `validity`, `basis` or `supersedes` is recorded as `null`, absent `dependencies` and `conditions` as `[]`, and an absent `health` as `not_applicable`. The reference `digest` is the `sha256` canonical digest (ENCODING) of this record. `knowledge.claim.inspect` returns the record, so any reader can recompute the digest.

- **Unique IDs.** `support_id` values are unique within a revision, and so are `condition_id` values; a duplicate is `invalid_envelope` at `/payload/support` or `/payload/conditions` (step 2). Each entry is validated first, so an invalid entry is reported at its own path before any duplicate.
- **Base check** (KNW-1). A stale Core precondition is `precondition_failed`. A `supersedes.revision` other than the current revision is `invalid_envelope` at `/payload/supersedes/revision`, and a wrong digest at `/payload/supersedes/digest`. A provider MUST NOT overwrite a revision or merge concurrent revisions silently.
- **Producer.** A revision's producer is always the session principal, whatever `derivation.kind` says.
- **New revisions start unaccepted.** No member of `propose` or `revise` sets reliance. A new revision is `proposed` until a decision names it (§6). This includes extracted instructions: a `normative` claim a model derived from a transcript is only `proposed` (KNW-9).

## 4. Independent facets (KNW-3)

`knowledge.claim.inspect` (query) `{ claim, revision? }` returns `{ reference, record, current_revision, reliance, applicability, health, availability, support, conflicts }`. The revision defaults to the current one. Each facet names the records it comes from, so a cached projection can be rebuilt:

| Facet | Shape | Comes from |
|---|---|---|
| `reliance` | `{ state: "proposed" \| "accepted_for_use" \| "rejected" \| "superseded", decision?, permitted_use?, author_is_decider? }` | The latest decision about this revision (§6), or `proposed` when there is none. `superseded` means an authority decided so, not merely that a newer revision exists. |
| `applicability` | `[ { evaluation, target, result, evaluator } ]` | The latest evaluation of this revision for each distinct target (§8). There is no applicability without a target. |
| `health` | `healthy`, `degraded`, `failing`, `unknown` or `not_applicable` | The revision record (§3) |
| `availability` | `{ state, counts: { available, unavailable, purged, unknown } }` | The support evidence as the provider observes it at the read (§5) |
| `support` | `{ class, unknown_ancestry }` | The support entries (§5) |
| `conflicts` | `[ { conflict, kind, status, state } ]` | Conflict records naming exactly this revision (§7), open and resolved, in recorded order |

A revision that is `accepted_for_use` and `invalid_for_target` for some target reports both. A historically accepted claim whose evidence was purged stays `accepted_for_use`, with availability `purged`. A provider MUST NOT collapse these facets into one status.

## 5. Support and declared ancestry (KNW-4, M5-Q2)

Each support entry is `{ support_id, evidence: { provider, artifact, digest }, ancestry: { completeness, roots } }`:
- `roots` are the origins the producer declares for this support. Each root is an exact, provider-qualified reference: `{ kind: "evidence", provider, artifact, digest }` or `{ kind: "claim", provider, claim, revision, digest }`.
- `completeness` is `complete` (these are all its origins), `partial` (some origins may be missing) or `unknown` (nothing is declared). With `unknown`, `roots` MUST be empty; with `complete` or `partial`, `roots` MUST name at least one root. Otherwise the command is `invalid_envelope` at `/payload/support/<i>/ancestry/roots`.
- **A wrapper is not an origin.** The support entry's own `evidence` artifact is never a root unless the producer lists it. With `unknown` ancestry the entry has no known origin, so it can never add a lineage.
- **Root identity** is the whole reference. Two roots are **the same** when every member is equal. Two roots with different references but an equal digest are **not known to differ**: equal bytes may be one origin copied, so such a pair is never treated as disjoint.

**Pair relation.** For two entries:
- `shared` when they have a root that is the same;
- `disjoint` when both are `complete`, no root is the same, and no pair of their roots has an equal digest;
- `undetermined` otherwise.

**Support class**, reported as `support: { class, unknown_ancestry: [ support_id ] }`. The first matching rule applies:

1. `unsupported`: no entries.
2. `undetermined`: some entry has `unknown` ancestry, and no two entries are `disjoint`.
3. `single_lineage`: one root is the same in every entry.
4. `multiple_lineages`: at least two entries are `disjoint`.
5. `overlapping_lineages`: there are at least two entries, every pair of entries is `shared`, and no root is common to all.
6. `undetermined`: any other case.

For example:
- roots `{A,B}`, `{B,C}` and `{A,C}`, all complete, are `overlapping_lineages`: every pair shares a root, and none is common to all;
- `{A}` and `{A}` are `single_lineage`;
- `{A}` and `{B}`, both complete, are `multiple_lineages`;
- `{A}` complete with `{B}` partial is `undetermined`;
- `{A}` with an unknown-ancestry entry is `undetermined`.

- **Repetition is not corroboration.** Adding entries that share a root, or that declare nothing, never produces `multiple_lineages`.
- `multiple_lineages` says only that at least two entries declare disjoint, complete origins. It is not proof of independence, and the provider never calls it independent.
- **Availability.** Counts over support artifacts, as the provider can observe them:
  - `available`, `unavailable` or `purged` for artifacts in a store it reads;
  - `unknown` for artifacts it cannot check.

  With no support entries, `state` is `unknown`. Otherwise it is `complete` when all are available, `partial` when some are, `unknown` when none are and some are unknown, `purged` when all are purged, and `unavailable` otherwise. It names no artifact.

## 6. Authority bindings and reliance decisions (KNW-6, M5-Q3)

| Operation | Kind | Semantics |
|---|---|---|
| `knowledge.authority.bind` | command | Subject `{ kind: "knowledge.authority", id: scope }`, precondition revision 0. Payload `{ authority: principal }`. Records epoch 1. |
| `knowledge.authority.transfer` | command | Subject is the binding, with a precondition on its current revision. Payload `{ authority: principal }`. Raises the epoch by one. |
| `knowledge.authority.get` | query | `{ scope }` → `{ scope, authority, epoch, revision }`, where `revision` is the binding subject's Core revision |
| `knowledge.decision.record` | command | Subject `{ kind: "knowledge.decision", id }`, precondition revision 0 |

- **Binding.** Only the provider's authority principals (CORE §15.1) bind or transfer, and a grant never lets anyone else do so: anyone else gets `permission_denied` with `not_authority`, whatever grant it names. An authority principal that names a grant is restricted to that grant (CORE §15.5), and no right covers binding, so it too gets `not_authority`. Binding a scope that is already bound fails its revision-0 precondition, `precondition_failed`. A scope has exactly one accepting authority. There is no hierarchy between scopes in 0.1: scope IDs match only when equal.
- **Decision payload.**

  | Member | Meaning |
  |---|---|
  | `claim` | The exact revision reference |
  | `decision` | `accepted_for_use`, `rejected` or `superseded` |
  | `permitted_use` | Required for `accepted_for_use`, absent otherwise: `binding`, `evidence`, `hypothesis` or `reference`, the reliance labels a packet may carry (CONTEXT §3) |
  | `supersedes_decision` | The ID of the decision it replaces for this revision, required exactly when one exists (§9) |
  | `validation_basis` | What it rests on: `{ evidence: [ references ], receipts: [ receipt references ], target? }` |
  | `rationale` | Free text |

  The command's `authority_epoch` (CORE §5) carries the binding epoch; without it the command is `invalid_envelope` at `/authority_epoch` (step 2).
- **Who decides.** Core authorization comes first, with rights `knowledge.decide` on the decision and `knowledge.read` on the claim (CORE §15.5). Step 7 then follows CORE §10: the authority epoch, then the Core revision preconditions, then this profile's checks.
  - **Authority epoch** (CORE §8). The binding's epoch is the authority epoch of the claim's scope. The check applies when the named revision exists at this provider (claim ID and revision, whatever the digest) and its scope is bound. `authority_epoch` lower than the binding's epoch is `stale_authority_epoch`, with `current_epoch` only if the principal may read the binding subject; higher is `unknown_authority_epoch`. With no such revision or binding there is no epoch to compare, and the checks below decide.
  - **Core revision preconditions** (CORE §7).
  - **Then, in this order:**
    1. The named revision does not exist at this provider, including a reference naming another provider: `not_found`.
    2. The claim's scope has no binding: `permission_denied` with `not_authority`.
    3. The session principal is not the bound authority: `permission_denied` with `not_authority`. A grant never makes a principal an authority, whatever rights it carries. No deciding is delegated in 0.1.
    4. The reference's digest differs from the revision's: `invalid_envelope` at `/payload/claim/digest`.
    5. `supersedes_decision` absent while a decision about the revision exists, present when none exists, or not the latest one: `precondition_failed` with details `{ latest_decision }` (`null` when none exists).
- **Authentication and binding, not labels, control permission.** A revision's `derivation.kind` has no effect on who may decide. A model producer that is not the bound authority cannot accept its output, even if it labels the derivation `human`.
- **The bound authority may adopt its own claim.** When the session principal is also the revision's producer, the decision is recorded with `author_is_decider: true`, and inspect and history report it. Such a decision is an adoption by its author, never a separate review. A human authority can therefore state a requirement and then adopt it through a separate decision, without an artificial producer identity.
- **Transfer preserves decisions.** Decisions recorded under an earlier epoch stay in effect until the current authority records a later decision about the same revision. A transfer never resets reliance.
- **Nothing else decides.** None of these ever records a decision:
  - proposing, revising or deriving a claim;
  - opening a conflict;
  - a satisfied verification assessment (VERIFICATION §9);
  - appearing in several summaries.

## 7. Conflicts and drift (KNW-2, KNW-5, M5-Q1)

`knowledge.conflict.open` (command) names exactly two revisions: subject `{ kind: "knowledge.conflict", id }`, precondition revision 0, payload `{ revisions: [ reference, reference ], note? }`. The provider compares them structurally, in this order. The first rule that separates them refuses the command with `claims_not_comparable` and `details.reason`:

| # | Check | Refusal reason |
|---|---|---|
| 1 | Statement subjects equal | `subject_differs` |
| 2 | Predicates equal | `predicate_differs` |
| 3 | Scope IDs equal | `scope_differs` |
| 4 | Qualifiers present in both have equal values | `qualifiers_disjoint` |
| 5 | Repositories present in both have equal trees; `environment` and `build`, where both are present, are equal | `basis_disjoint` |
| 6 | Validity intervals, where both are fully known, overlap | `validity_disjoint` |
| 7 | Neither cardinality is `multiple` | `multiple_values_permitted` |
| 8 | Values differ (canonical JSON) | `values_equal` |

So "staging uses 5 workers" and "production uses 3 workers" are not comparable (reason 4), and "supports English" and "supports French" under a `multiple` predicate are not a conflict (reason 7).

- **Kind.** `drift` when one revision is `normative` and the other `observed`: "must use queue v2" against "currently uses queue v1" is a gap between intent and observation. Otherwise the kind is `conflict`.
- **Potential or demonstrated.** A record is `demonstrated` only when every dimension is certain:
  - the qualifier objects are equal;
  - both bases are present and `complete`, with the same repositories, trees, `environment` and `build`;
  - both validity intervals are fully known;
  - both cardinalities are `single`;
  - the planes are equal, or are `normative` and `observed`.

  Otherwise it is `potential`, and the record lists which dimensions were uncertain: `uncertain: [ "qualifiers" | "basis" | "validity" | "cardinality" | "plane" ]`. A potential conflict is a report worth checking, not an established incompatibility.
- **Nothing is removed.** Neither kind deletes, rewrites or demotes a revision. While a record is `open`, its competing values are all listed. `knowledge.claim.inspect` lists the record for both revisions.
- **Opening checks** (step 7, after `knowledge.propose` on the record and `knowledge.read` on both claims, and after the Core revision preconditions): a named revision does not exist at this provider, `not_found`; a reference digest differs, `invalid_envelope` at `/payload/revisions/<i>/digest`; then the comparison above. Naming the same revision twice is `invalid_envelope` at `/payload/revisions` (step 2).
- **Who opens and who resolves.** Any principal with `knowledge.propose` may open a record, including a model producer, and may add a suggested distinguishing observation in `note`.
  - `knowledge.conflict.resolve` (command, precondition on the record's revision) records `{ resolution: "select" | "narrow_scope" | "reject_support" | "supersede" | "request_observation" | "not_a_conflict", selected?, rationale }`, with `authority_epoch`.
  - `authority_epoch` is required, as for decisions (`invalid_envelope` at `/authority_epoch`, step 2).
  - Authorization needs `knowledge.decide` on the record. Step 7 follows CORE §10, as for decisions (§6):
    - the authority epoch of the revisions' scope, when the record exists and that scope is bound;
    - the Core revision preconditions;
    - the record does not exist or is already resolved: `not_found`;
    - checks 2 and 3 of §6 for the revisions' scope;
    - `selected`, required exactly for `select` and naming one of the two revisions: otherwise `invalid_envelope` at `/payload/selected`. Only the bound authority resolves. Recency and producer confidence never resolve a record.
  - A resolution changes no revision and records no reliance decision; those remain separate operations.

## 8. Applicability (KNW-7, M5-Q4)

`knowledge.applicability.evaluate` (command) records an evaluation. Subject `{ kind: "knowledge.evaluation", id }`, precondition revision 0, payload `{ claim: reference, target }`. It needs `knowledge.propose` on the evaluation and `knowledge.read` on the claim; a missing revision is `not_found`, and a differing digest `invalid_envelope` at `/payload/claim/digest`. `target` has the shape of a claim `basis`, and each repository may also carry `dirty: { snapshot_digest }`. The provider's evaluator `{ id, version }` is pinned in the record.

**Findings.** Each condition gets one finding against the target:
- `match` or `mismatch` when the evaluator observed the anchor;
- `missing_anchor` when the target lacks the repository, snapshot or environment the condition names;
- `unsupported` when the evaluator does not implement that condition kind.

Each dependency resolves only as an **exact local reference**. It resolves when it names this provider, and a revision this provider holds with exactly its claim ID, revision and digest. It then gets its finding from the latest evaluation recorded for that exact reference (provider, claim, revision and digest) and a target equal to this one (canonical JSON):
- `match` if that evaluation is `applicable`;
- `mismatch` if it is `invalid_for_target`;
- `unchecked`, with `reason`, in every other case. The first row that applies gives the reason:

| `reason` | When |
|---|---|
| `remote_dependency` | The dependency names another provider. This profile defines no cross-provider dependency evaluation, and the provider never substitutes an evaluation of a local revision with the same claim ID, even when that ID and revision are the evaluated revision's own. |
| `self_reference` | The dependency names this provider and the evaluated revision's own claim ID and revision, whatever its digest |
| `unresolved` | No revision with that claim ID, revision and digest exists here. This covers malformed or guessed references, and references to revisions proposed later. |
| `not_evaluated` | The reference resolves, but no evaluation of it exists for an equal target |
| `not_established` | The latest such evaluation is `needs_check` or `unknown` |

An evaluation of a different digest, provider, revision or target never satisfies a dependency.

**Validation assumptions.**
- **Why content-addressed cycles are infeasible.** A revision's digest is SHA-256 over a record that includes its dependencies' digests. A dependency on itself, directly or through other revisions, would need the digest of a record that includes that same digest. Under the preimage and collision resistance of SHA-256, such a cycle cannot feasibly be built.
- **What that does not guarantee.** It says nothing about whether an incoming reference is valid. Producers may name any provider, claim, revision and digest. Propose and revise do not resolve dependencies, so a guessed, malformed, remote, future or self-referential reference is accepted as written.
- **What the evaluator does instead.** It relies on exact resolution and never on hash properties. Anything it cannot resolve stays `unchecked`, so the revision is at best `needs_check`. No unrelated or cached evaluation can make such a dependency count as a match.
- **No cycle detection.** The profile needs no general cycle detection: a supported case that could form a cycle would have to resolve by exact digest, and building one is infeasible.

**Result.** The first rule that applies wins:

| # | When | Result |
|---|---|---|
| 1 | Any `mismatch` | `invalid_for_target`: an observed relevant mismatch settles it, whatever else is incomplete |
| 2 | Any `missing_anchor` or `unsupported` | `unknown`: this evaluator cannot settle it against this target |
| 3 | Any `unchecked` | `needs_check`: the conditions it could observe match, but a dependency is not established; its `reason` says why |
| 4 | No conditions and no dependencies | `unknown`: nothing checkable was declared |
| 5 | Otherwise (every finding `match`) | `applicable` |

The evaluation records `{ claim, target, evaluator, result, findings: [ { condition_id?, dependency?, finding, reason? } ], supersedes_evaluation }`, where `supersedes_evaluation` is the previous latest evaluation of the same revision for an equal target, or `null`.

- Incomplete checks never yield `applicable`. An unavailable input never compares equal to an earlier one.
- `needs_check` and `unknown` mean "not established", not "stale" or "historical". Readers, including context packets (CONTEXT §14), MUST keep them distinct from `invalid_for_target`.
- Evaluations never change reliance, and decisions never change an evaluation.

## 9. History (KNW-8, M5-Q8)

`knowledge.claim.history` (query) `{ claim }` returns the lineage's records in recorded order:
- `revisions`: `[ { reference, producer, supersedes, recorded_at, position } ]`;
- `decisions`: `[ { decision, claim, value, permitted_use, decider, author_is_decider, epoch, supersedes_decision, recorded_at, position } ]`;
- `evaluations`: `[ { evaluation, claim, target, result, evaluator, supersedes_evaluation, recorded_at, position } ]`;
- `conflicts`: `[ { conflict, kind, status, revisions, state, resolution, recorded_at, position } ]`.

Each list is in recorded order. `position` is the stream position `{ epoch, sequence }` of the event that recorded the record (CORE §16), or `null` when that event is no longer retained, and `recorded_at` is the provider clock at commit. A resolved conflict appears once, with its resolution.

- Every record is immutable. Every link between records is explicit: `supersedes` between revisions, `supersedes_decision` and `supersedes_evaluation`.
- A superseded revision stays inspectable with its record and facets.
- Validity is only what producers supplied; unknown bounds stay unknown.
- **Not in 0.1.** Reconstructing what was known at a past position, or what is now known about a past interval, is deferred. The records above keep what such a view needs, without inventing missing history.

## 10. Rights

| Right | Covers |
|---|---|
| `knowledge.propose` | `claim.propose` and `claim.revise` on covered claims; `conflict.open` on covered conflicts; `applicability.evaluate` on covered evaluations |
| `knowledge.read` | `claim.inspect` and `claim.history` on covered claims, `authority.get`, and events of covered subjects |
| `knowledge.decide` | Needed for `decision.record` and `conflict.resolve`, and never sufficient (§6) |

Rights on named revisions: `decision.record` also needs `knowledge.read` on its claim; `conflict.open` on both claims; `applicability.evaluate` on its claim and on every claim its dependencies name at this provider, because the findings reveal whether those revisions exist here. `conflict.resolve`, `claim.revise` and references inside claim content need no further right, and references naming another provider add none.

## 11. Events

Core event records. *Candidate* types:

| Type | Subject | Payload |
|---|---|---|
| `knowledge.claim.revised` | claim | `{ reference }` |
| `knowledge.decision.recorded` | decision | `{ claim, decision, permitted_use?, author_is_decider, supersedes_decision }` |
| `knowledge.evaluation.recorded` | evaluation | `{ claim, result, evaluator, supersedes_evaluation }` |
| `knowledge.conflict.opened` | conflict | `{ kind, status, revisions }` |
| `knowledge.conflict.resolved` | conflict | `{ resolution }` |
| `knowledge.authority.bound` / `knowledge.authority.transferred` | binding | `{ authority, epoch }` |

## 12. Claims in context packets

A packet that carries claims preserves their identity, reliance and applicability (SPEC §2). The Context side is [CONTEXT §14](CONTEXT.md#14-claims-in-packets) (feature `context.claims`), and boundary enforcement is [EXECUTION §13.3](EXECUTION.md#133-claim-revalidation-executionclaim_revalidation).

## 13. Conformance and test controls

- **Reference participants only.** A reference knowledge provider keeps claims in its own store, and evaluates applicability over typed conditions and declared dependencies only. No fixture depends on CBR's memory engine, model runtime or retrieval.
- **Launch configuration `knowledge`** (test control):
  - `evaluator: { id, version, condition_kinds }`;
  - the adversarial `serve_altered_claims`: claims whose inspected record is altered while the reference and digest stay the same, so readers are tested on recomputing digests, like EVIDENCE's `serve_altered_bytes`.

  Participants declare `claims.test_controls: ["knowledge.store"]`.
- Support availability is read from the provider's own evidence store (EVIDENCE §14); artifacts at other providers count as `unknown`.

## 14. What knowledge does not establish

A claim revision establishes that an authenticated producer proposed this content, with this declared support and ancestry. It does not establish:
- that the claim is true;
- that declared ancestry is complete or honest, or that disjoint lineages are independent;
- that a potential conflict is a real incompatibility;
- that an accepted claim applies to a target it was not evaluated against;
- that a decision recorded here is project acceptance anywhere else. Coordination is unsupported in 0.1, and the local authority binding is the standalone mode.
