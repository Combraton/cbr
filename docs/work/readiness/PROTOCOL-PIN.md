# Protocol pin, verification evidence and the CBR surface

> **Status: proposed, not accepted.** Part of the implementation-readiness milestone for [issue #1](https://github.com/Combraton/cbr/issues/1). Nothing here selects a release scope or a stack; see [TALK](TALK.md) for the discussion and the open questions.

CBR pins **Protocol `v0.1.0`**, an annotated tag at commit `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`, tree `ad57cc4ef067834c20d868dbcc844f70b8ef223f`. Protocol `main` is not equivalent to the release and is not the contract.

## 1. What was verified, and how

Verified on 2026-09-16 on macOS 25.3.0 (arm64), from the published release assets, following the [extracted-archive procedure](https://github.com/Combraton/protocol/blob/main/docs/work/release-0.1/CONSUMERS.md). No Git metadata was needed and the sibling `protocol` checkout was not modified.

| Check | Command | Result |
|---|---|---|
| Tag is annotated and points at the named commit | `git cat-file -t v0.1.0`; `git rev-parse v0.1.0^{commit}` | `tag`; `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc` |
| Release assets match their published checksums | `shasum -a 256 -c SHA256SUMS` | `combraton-protocol-0.1.0-source.tar.gz: OK`, `release-manifest.json: OK` |
| The manifest names the tagged commit | `release-manifest.json` | `source_commit` = `cbf8e4df…`, `source_tree` = `ad57cc4e…`, `tested_head` = `f7e67975…` with the same tree |
| Every file inside the archive matches | `shasum -a 256 -c BUNDLE-SHA256SUMS` | 539 files OK, 0 failures |
| Normative inventory matches the release record | `python3 scripts/release_inventory.py --verify` (file-system mode, in the extracted archive) | `420 files, listing sha256 80b39377b10685c29bb5823e69ee539ef91bb1eace5b049f2894b603ce08b41d: ok` |
| The archive's own documentation is internally consistent | `python3 scripts/check_docs.py` | 48 files, 273 links, 0 errors |

The inventory's `listing_sha256` matches the value recorded in three independent places: the annotated tag message, `docs/release/0.1/inventory.json` in the tagged tree, and the release's `release-manifest.json`.

**Tag signature.** `git tag -v v0.1.0` reports `error: no signature found`. The tag is unsigned; Protocol 0.1 defers signing with Remote trust. Integrity here rests on the checksum chain above, not on a signature.

### The tagged tree versus `main`

`git diff --name-only cbf8e4d..71ed2c4` returns exactly three files, all under `docs/work/`: `STATE.md`, `release-0.1/HANDOFF.md` and `release-0.1/M6.md`. No schema, fixture, specification, script or Rust source differs. This matters for one reason only: a conformance run executed against the sibling checkout at `main` exercises code, schemas and fixtures that are byte-identical to the tag. It does **not** make `main` the pin.

### Toolchain and fixture-suite evidence

Run against the protocol repository's own reference provider — not against CBR, which does not exist yet — to establish that the pinned toolchain and suite work on this machine.

| Suite | Result | Wall clock |
|---|---|---|
| `--filter core.` | 135 fixtures, 135 pass, 0 not passing | 37.5 s |
| `--filter context.` | 11 fixtures, 11 pass, 0 not passing | — |
| `--filter knowledge.` | 10 fixtures, 10 pass, 0 not passing | — |
| `--filter evidence.` | 16 fixtures, 16 pass, 0 not passing | — |

Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, matching the release manifest's `rust_toolchain: "1.97.1"`. This is evidence about the suite and the machine. It is **not** evidence about CBR.

## 2. The surface CBR implements and calls

From the pinned [consumer handoff](https://github.com/Combraton/protocol/blob/main/docs/work/release-0.1/CONSUMERS.md).

| Direction | Profile | Why |
|---|---|---|
| **Serves** | `core/1` with `core.events`, `core.grants`, `core.capabilities` | The command path every other profile depends on |
| **Serves** | `knowledge/1` | Claim revisions, support and declared ancestry, authority bindings, reliance decisions, conflicts, applicability evaluations, history |
| **Serves** | `context/1` with `context.claims` | Requests, obligations, jobs, packets, updates, corrections, expand |
| **Serves** | `evidence/1` | Required in practice: `context/1` depends on Evidence as a protocol dependency because every packet revision **is** a sealed Evidence artifact. A standalone CBR holds its own packet and support bytes, so it serves Evidence for them. |
| **Serves (test only)** | `core-test/1` | 127 of the 135 Core fixtures reference `core-test` subjects. Without it, the Core suite cannot run. It is conformance-only and must never be exposed outside a test configuration. |
| **Calls** | `execution/1` | Client only, and only for optional PIO-backed investigation. CBR is never an execution provider. |
| **Calls** | `evidence/1` | At providers named by support citations it reads |
| **Calls** | `verification/1` | Optional, only when receipts are consumed |
| **Not implemented** | `coordination`, `remote-trust` | Declared `not_in_release` by Protocol 0.1 |

Fixture counts at the pin, from `conformance/fixtures/`: `core` 135, `stream` 24, `socket` 13, `evidence` 16, `context` 11, `knowledge` 10, `composition` 14, `compat` 1. `execution` (51) and `verification` (5) are outside CBR's provider role.

## 3. How CBR's accepted records map onto the pinned contracts

The protocol froze more of CBR's data model than the specs make obvious. Where a protocol record exists, it should **be** the canonical record rather than a projection of a parallel private one, so that a wire read and an internal read cannot diverge.

| Record in [INTERNALS](../../spec/INTERNALS.md) / [SPEC](../../spec/SPEC.md) | Where it lives under Protocol 0.1 | Private to CBR? |
|---|---|---|
| Evidence descriptor and sealed payload | `evidence/1` artifact | no |
| Assertion revision | `knowledge/1` claim revision, with its `combraton-knowledge-claim/1` record and SHA-256 digest | no |
| Memory artifact — a flow explanation, a rejected approach, a distilled investigation | **A sealed `evidence/1` artifact, cited by claims.** `statement.value` is capped at 4096 bytes of canonical JSON, so prose artifacts cannot be claim values. | no |
| Derivation record — prompt, configuration, inputs, exact output | **Sealed `evidence/1` artifacts**, referenced from the claim's `derivation.record` and `derivation.inputs` | no |
| Reliance decision | `knowledge/1` decision, under an authority binding | no |
| Applicability evaluation | `knowledge/1` evaluation | no |
| Conflict and drift record | `knowledge/1` conflict | no |
| Context packet revision | `context/1` packet, sealed as an Evidence artifact | no |
| Entity revision, aliases, rename/split/merge lineage | — | **yes** |
| Memory patch (a typed multi-operation proposal) | — | **yes**; the wire commits one record per command |
| Build manifest, index frontier and coverage | — | **yes**, rebuildable |
| Memory job, checkpoint, budget accounting | — | **yes** |
| Retrieval indexes | — | **yes**, rebuildable accelerators |
| Working-set and context-budget policy | — | **yes** |

Two consequences worth stating plainly, because they are easy to get wrong later:

- **A model's output is evidence before it is knowledge.** The transcript is a sealed artifact; the interpretation drawn from it is a `proposed` claim revision; neither is accepted until an authority records a decision. `derivation.kind: "human"` grants nothing — KNOWLEDGE §6 says authentication and the authority binding decide, never a label.
- **Repetition cannot manufacture support.** KNOWLEDGE §5 computes a support class from declared ancestry roots. Adding entries that share a root, or that declare nothing, can never reach `multiple_lineages`. CBR must declare ancestry honestly rather than emit one support entry per retrieval hit.

### Sealing derived artifacts, and why ancestry must skip them

A derived artifact — a model transcript, a flow explanation, a rejected-approach note, a distilled investigation — is sealed as evidence, so it needs rules that keep it distinguishable from something CBR actually observed.

- **Source kind.** A derived artifact is sealed under a CBR-specific source kind reserved for derivation output, for example `cbr.derivation.transcript`, `cbr.artifact.flow` or `cbr.artifact.rejected_approach`. It is **never** sealed under a captured-observation kind such as `terminal_output`, `test_report` or `build_log`. Those kinds mean "these bytes were captured from a running thing", and a derivation did not capture anything.
- **Producer.** The producer principal is CBR's own, because EVIDENCE §3 fixes `producer.principal` to the session principal of the `prepare` command and refuses a prepare naming another principal. A derived artifact therefore cannot be made to look like a third-party observation, which is the correct behaviour.
- **Coverage.** A derived artifact's coverage describes what the derivation inspected, with its gaps. It never inherits the coverage of its inputs.

**The ancestry rule.** When a derived artifact is cited as claim support, the support entry's `ancestry.roots` MUST name the **captured evidence the derivation was drawn from**, never the derived artifact itself. KNOWLEDGE §5 states the general principle — "a wrapper is not an origin. The support entry's own `evidence` artifact is never a root unless the producer lists it" — and for derived artifacts naming yourself as your own root is precisely the mistake that manufactures apparent independence.

**Negative control for this rule.** Take one captured artifact `C`. Run two different derivations over it, producing derived artifacts `D1` and `D2`. Support a claim with two entries, one citing `D1` and one citing `D2`, each declaring `ancestry: { completeness: "complete", roots: [C] }`. The provider MUST report support class **`single_lineage`**, because one root is the same in every entry (KNOWLEDGE §5 rule 3).

The control fails — and the guard is broken — if that claim reports `multiple_lineages`, which is what happens the moment a derivation lists itself as its own root: two entries with complete, disjoint roots `{D1}` and `{D2}` satisfy rule 4 and the claim falsely reads as independently corroborated. Two model passes over the same log are not two sources. This control belongs in the M2 suite and gets its own mutant in M6.

## 4. Gaps found while reading the pinned contracts

These are recorded now so they are not discovered mid-implementation and quietly worked around. None is a blocker for starting. Per the consumer handoff, a real gap costs a versioned protocol revision with a demonstrating fixture — never a locally widened schema or a private field.

| # | Gap | Consequence for CBR | Proposed disposition |
|---|---|---|---|
| G1 | **No condition kind for build identity.** `basis` and an evaluation `target` both carry `build`, but the condition vocabulary is only `repository_tree`, `dirty_snapshot` and `environment_digest` (KNOWLEDGE §3, §8). A claim valid only for a particular build cannot state that as a checkable condition. | A behaviour claim that depends on build identity can be evaluated `applicable` on a target whose build differs. This is exactly the "unchanged span, changed runtime configuration" invariant CBR owns in [PLAN §13](https://github.com/Combraton/combraton/blob/main/docs/architecture/PLAN.md). | **Decided (ADR 001, question 4).** Include build facts in CBR's declared environment fact set so they are covered by `environment_digest`, and say so in the packet. File a `build_digest` condition-kind proposal on the Protocol repository **with a reproducing fixture**. No private field, no locally widened schema. **Filed 2026-09-16 as [Combraton/protocol#11](https://github.com/Combraton/protocol/issues/11)**, a proposal for a future minor, with two fixtures: one that **passes against the v0.1.0 reference provider** and so records the gap (a claim for `build-1` is `applicable` on `build-2`; the kind is `invalid_envelope`; the environment-folding workaround works but conflates build with environment), and the proposed behaviour, which fails at propose today. |
| G2 | **No public search over knowledge or evidence text.** `knowledge/1` has `inspect` and `history` but no query. `evidence/1` has `query` over producer, source, work, media type, digest and scope — not over content. | A direct standalone client cannot ask "what do you know about X". The only retrieval surface is submitting a context request and reading the packet. | **Decided (ADR 001, question 5).** Intended for v0.1: context is the retrieval surface. A local `cbr search` diagnostic is permitted provided it is labelled non-protocol and is never a scored path for the evaluation client. |
| G3 | **Condition vocabularies differ between profiles.** `context/1` §8 has a fourth kind, `authority_revision`, that `knowledge/1` §8 does not. | A packet's applicability conditions and a claim's applicability conditions are not the same set, so they cannot share one evaluator path without care. | Implement two condition evaluators over one shared core, and never silently coerce one vocabulary into the other. |
| G4 | **No batch atomicity on the wire.** Each claim, decision, evaluation and conflict is its own command with its own precondition. A five-claim proposal that fails on the third leaves three committed. | CBR can still commit a patch atomically in its own transaction internally, but an external producer cannot. | Accept for v0.1 and document the non-atomicity at the public boundary. Revisit only if a real consumer is harmed. |
| G5 | **Repository registration and local source reading are outside the protocol.** Nothing in 0.1 registers a repository or grants scoped filesystem reads, yet CBR must read source under a grant ([PREPARATION §3](../../spec/PREPARATION-AND-DELIVERY.md)) and its tool surface includes bounded span reads ([MODEL-RUNTIME §3](../../spec/MODEL-RUNTIME.md)). | Source identity and read scope are CBR-private configuration. Two CBR installations could disagree about what `repository.id` and `tree` mean. | Define them in CBR and publish the definition (§5). Not a protocol change: the protocol deliberately treats the basis as opaque strings. |
| G6 | **CORE §19.4 does not name the overdue-obligation event's subject or payload.** It says a provider-origin event marks the obligation `overdue`, and names the aborted event fully — `core.effect.obligation.aborted` with `{ effect, obligation, target }` on the effect subject — but gives the overdue event no subject and no payload. The reference provider emits it on the `execution.execution` subject with `{ effect, obligation }`, which has no counterpart for an effect recorded without an execution. | Two providers can disagree about where a consumer should look for an overdue obligation, and a consumer written against the reference would not find CBR's. | **Decided (M1 stage c4).** CBR emits `core.effect.obligation.overdue` on the effect subject `{ "kind": "core.effect", "id" }` with the aborted event's payload `{ effect, obligation, target }`. Filed with G7 as [Combraton/protocol#12](https://github.com/Combraton/protocol/issues/12), asking which subject providers should agree on. |
| G7 | **The §19 banner is stale against the 0.1 release.** §19 opens "Proposed for milestone M3 … Not normative until M3 is accepted with schemas and fixtures", but `docs/release/0.1/README.md` lists `core.effects` among the released `core/1` features, its schemas ship, 70 fixtures declare it, and EXECUTION.md records M3 as accepted. | A reader stopping at the banner could treat §19 as optional. | **Decided (M1 stage c4).** CBR treats §19 as normative on the strength of the release README and the shipped schemas. Filed with G6 as [Combraton/protocol#12](https://github.com/Combraton/protocol/issues/12). |

## 5. Proposed source-identity definition

Protocol 0.1 treats `repository.id`, `tree`, `dirty.snapshot_digest`, `environment` and `build` as opaque strings compared by exact equality. Exact equality is only useful if the producer computes them deterministically, so CBR must fix its own definitions and publish them.

| Field | Proposed definition | Why, and what it costs |
|---|---|---|
| `repository.id` | A stable identifier assigned when the repository is registered with this CBR installation. Never the filesystem path. | Moving or re-cloning a checkout must not change claim identity. Cost: two installations need an explicit agreement to compare claims; the protocol already scopes claims per provider, so this is consistent. |
| `tree` (git) | The **root tree object id** of the commit, `git rev-parse <commit>^{tree}` — not the commit id. The **commit id is recorded in the evidence descriptor's capture anchors** (ADR 001, question 8). | Two commits with identical content are the same content basis, and applicability conditions are about content. Recording the commit alongside removes the obvious cost: content identity governs applicability while the commit stays recoverable, so "which commit was this" never needs reconstructing. Submodule contents are covered by the tree; submodule *commits* are recorded as gitlink entries only. |
| `tree` (non-git directory) | `sha256` over the canonical JSON of a sorted list of `[path, mode, sha256-of-bytes]` for every non-ignored file. | Gives a deterministic content identity where Git gives none. Cost: O(size) to compute, so it needs a cache keyed by mtime+size, and the cache must never be trusted across a restart without revalidation. |
| `dirty.snapshot_digest` | `sha256` over canonical JSON of `{ base_tree, entries: [[path, status, mode, blob_sha256]] }`, sorted by path, covering tracked-modified, staged, deleted and untracked-not-ignored files. Deletions are present with an explicit status. | A dirty basis must distinguish "file removed" from "file absent from the listing". Cost: the digest changes on every keystroke in an editor, which is correct but makes dirty-basis claims short-lived. |
| `workspace` | `clean` only when the snapshot above is empty **and** the repository was read under a lock or re-read identically afterwards. Otherwise `dirty`, or `unknown` when CBR could not establish either. | [PREPARATION §5](../../spec/PREPARATION-AND-DELIVERY.md) forbids pretending a changing checkout was immutable. Cost: `unknown` will be common, and a basis with `dirty: null` on a non-clean workspace must declare `completeness: "partial"` (CONTEXT §3). |
| `environment` | `sha256` over canonical JSON of an explicitly declared, ordered set of environment facts the installation is configured to capture. Never "whatever `env` printed". | Undeclared capture is not reproducible and would silently invalidate everything. Cost: facts nobody declared are invisible to applicability — which is honest, and must be reported as coverage rather than assumed absent. |
| `build` | Same shape as `environment`. Per ADR 001 question 4, **build facts are included in the declared environment fact set**, so build identity is covered by `environment_digest` and the packet says so explicitly. See G1. | Makes the gap survivable without a private field. Cost: build and environment can no longer be distinguished by a reader of the condition alone, which is exactly what the `build_digest` proposal is for. |

Each of these needs a property test with a **negative control**: a change that must alter the digest (a deleted file, a mode change, a submodule bump) and a change that must not (mtime alone, an ignored file).

**Implemented at M2** in `crates/cbr-identity` for git trees, dirty snapshots and the environment fact set, each with its negative control and a mutant ([VERIFICATION](../../VERIFICATION.md#knowledge-source-identity-and-the-knowledge-verbs)). Not yet implemented: the non-git directory tree, the `workspace` clean/dirty determination under a lock, and a submodule bump beyond recording the gitlink.

## 6. How CBR pins, reproducibly

The procedure below is the one used above and is the one implementation should automate. It touches no sibling checkout.

```sh
gh release download v0.1.0 --repo Combraton/protocol
shasum -a 256 -c SHA256SUMS
tar -xzf combraton-protocol-0.1.0-source.tar.gz
cd combraton-protocol-0.1.0
shasum -a 256 -c BUNDLE-SHA256SUMS
python3 scripts/release_inventory.py --verify
```

Implementation vendors, from that extraction only, the `schemas/` trees for the profiles CBR serves and calls, the `conformance/` fixture set and runner, and keeps `docs/spec/` from the same commit as the normative text. The pinned tag, commit, `listing_sha256` and bundle SHA-256 are recorded in the repository so a fresh session can re-verify without trusting this file.
