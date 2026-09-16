# ADR 001: standalone CBR v0.1 scope, stack and evaluation posture

- **Status:** accepted, and now complete. The project licence, the model provider and spend envelope, and the journey-6 pilot repository were all decided on 2026-09-16. One number inside the spend envelope arrived in example form and is flagged in [Unresolved](#unresolved); no work waits on it.
- **Date:** 2026-09-16.
- **Authority:** the owner, answering the nine questions in [TALK §5](../work/readiness/TALK.md), after independent review of [PR #2](https://github.com/Combraton/cbr/pull/2) at head `b8ba7f3` against base `3278393`. The reviewer independently reproduced the protocol pin facts, the condition-vocabulary gap, the reference provider's synchronous design, the fixture counts and the crate licences.
- **Scope:** CBR-local implementation choices. Wire and compatibility decisions belong to Protocol; cross-system authority belongs to Combraton. This record selects nothing outside this repository.

## Problem

The readiness milestone produced proposals for release scope, stack, milestones and journey acceptance, and returned nine decisions requiring owner judgment. Without them, implementation could not start without either guessing or silently narrowing the agreed product.

## Affected contracts

Nothing on the wire. CBR remains pinned to Protocol `v0.1.0` (`cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`), serving `core/1`, `knowledge/1`, `context/1` with `context.claims`, and `evidence/1` for its own packet and support bytes, plus `core-test/1` behind the conformance launch configuration. It calls `execution/1` and `verification/1` as a client only.

## Selected choices

| # | Question | Decision |
|---|---|---|
| 1 | Is background maintenance in v0.1? | **In, and narrow.** Coalesced triggers; background spend defaults to **zero** until explicitly enabled; foreground context requests take priority. It is not the first thing to cut. |
| 2 | Does CBR ship a CLI? | **Yes**, the `cbr` CLI as proposed in [RELEASE-SCOPE §2](../work/readiness/RELEASE-SCOPE.md), speaking the public protocol over the socket and doubling as the headless evaluation client. No privileged internal shortcut. |
| 3 | Provider and spend budget | **MiniMax**, on the owner's subscription quota, addressed through both dialects at one provider: OpenAI-compatible `https://api.minimax.io/v1` as the primary wire and Anthropic-compatible `https://api.minimax.io/anthropic` as the second. Models `MiniMax-M2.7-highspeed` (extraction, large-result projection), `MiniMax-M2.7` (ordinary derivation), `MiniMax-M3` (synthesis, request-time investigation, image input). Admission counts through `POST /v1/responses/input_tokens`. The mechanism is unchanged and now load-bearing: the persisted envelope is debited **before** every call, never reconciled after, and exhaustion is a typed `budget_exhausted` result rather than a retry loop — the quota is shared with the owner's own tools, so an overspend degrades them, not just CBR. Background spend stays zero until explicitly enabled. Full terms and consequences in [STACK §8.1](../work/readiness/STACK.md). |
| 4 | The build-condition gap (G1) | Define CBR's declared **environment fact set to include build facts**, so build identity is covered by `environment_digest`, and **say so in the packet** rather than leaving it implicit. Separately, file a `build_digest` condition-kind proposal on the Protocol repository **with a reproducing fixture**. No private field, no locally widened schema. |
| 5 | No public memory search | **Intended for v0.1.** Context is the retrieval surface. A local `cbr search` diagnostic is permitted provided it is **labelled non-protocol** and is never presented as a protocol operation or used by the evaluation client as a scored path. |
| 6 | The early journey-6 pilot at M3 | **Yes**, against **Knowscroll-v2** ([`Legend101Zz/Knowscroll-v2`](https://github.com/Legend101Zz/Knowscroll-v2), public), with its decision records `D-001`–`D-022` as the decision input and one open backlog task as the refactor. Labelled **pilot**: it is a steer, not evidence, and may not be cited as downstream-task evidence for the gate. Evidence capture redacts credentials **at the recording boundary**, not afterwards — that repository has already once seen a provider response carry a live third-party credential, so this is a known failure mode there rather than a precaution. |
| 7 | CBR's own mutant set | **Yes.** One mutant per named negative control in [JOURNEYS](../verification/JOURNEYS.md), built in **M6**, with `WRONG-REASON` reported when a mutant fails at the wrong step or for the wrong reason — the pattern the Protocol repository already runs at scale. |
| 8 | Source identity | **As proposed** in [PROTOCOL-PIN §5](../work/readiness/PROTOCOL-PIN.md): `tree` is the git root tree object id, not the commit. **Additionally, record the commit id in the evidence descriptor**, which removes the cost noted in that section — content identity governs applicability while the commit remains recoverable from evidence. |
| 10 | Project licence | **MIT**, the same text and holder line as Protocol. `LICENSE` at the repository root, `license = "MIT"` in `[workspace.package]`, inherited by every crate. This removes the Cargo inheritance error that `exclude = ["vendor"]` worked around, but **not** the reason for building the conformance runner from the release archive: that is about naming the exact runner that produced a result, and it stands regardless. |
| 9 | Relevance measurement | **Diagnostic only, in the M7 pilot.** Prefer **counting downstream re-investigation of content the packet already contained** over self-reported prediction, which is weaker and gameable. Never a gate criterion. |

Two design statements accepted alongside them, both now recorded in [PROTOCOL-PIN §3](../work/readiness/PROTOCOL-PIN.md):

- Derived artifacts are sealed under a CBR-specific derivation source kind with CBR as producer, never a captured-observation kind.
- A derived artifact cited as claim support declares ancestry roots naming the **captured evidence it was derived from**, never itself. Negative control: a claim supported only by two derived artifacts sharing one captured root must report `single_lineage`.

## Unresolved

Both open lines were closed on 2026-09-16. **One number inside them arrived in example form** and is recorded here rather than quietly rounded into a decision.

| Item | Status |
|---|---|
| ~~**Project licence**~~ | **Resolved 2026-09-16: MIT**, matching Protocol, holder line `Copyright (c) 2026 Combraton contributors`. Question 10. |
| ~~**Provider and spend budget**~~ | **Resolved 2026-09-16: MiniMax**, on the owner's subscription quota, with the endpoints, models, counting endpoint and key location recorded in [STACK §8.1](../work/readiness/STACK.md). Question 3. |
| ~~**Journey-6 pilot repository**~~ | **Resolved 2026-09-16: Knowscroll-v2**, decisions `D-001`–`D-022` as input, one open backlog task as the refactor, labelled pilot. Question 6. |
| **The numeric ceiling** | **Flagged, not guessed.** The owner's answer gave the CBR share of the quota as `[e.g. 20M]` tokens per 5-hour window and `[e.g. 300M]` per month — the example markers were left in place. CBR adopts **20M per 5-hour window and 300M per month** as the working envelope so nothing is blocked, and labels them *provisional, pending a one-word confirmation*. They are written in one place (the envelope configuration), and every number CBR derives from them is derived, not copied, so a correction is a one-line change and not a search. |

A provisional ceiling is safe here in a way a provisional provider was not: the envelope is enforced by the same debit-before-call mechanism whatever the number is, and an envelope set too low fails **closed**, as a typed `budget_exhausted`, which is the direction an error should point when the quota is shared with the owner's own tools.

Owner actions are recorded on [issue #1](https://github.com/Combraton/cbr/issues/1); they do not need a new decision record.

## Alternatives considered and rejected

Cutting the maintenance loop to a later release was available and refused: the CBR gate names maintenance as a substantive capability, and deferring it would have been a scope reduction requiring its own explicit decision. Adopting `rig-agent`'s loop was the strongest library alternative and is rejected for now on churn and serialization-stability grounds, revisitable at rig 1.0 — the full argument, including the case against this decision, is in [STACK §8](../work/readiness/STACK.md). Tokio was rejected on direct evidence from the protocol reference provider rather than preference ([STACK §2](../work/readiness/STACK.md)). Adding a public search operation was rejected as a protocol change CBR does not need in v0.1.

## Primary evidence

Protocol pin verified from the published release archive: `BUNDLE-SHA256SUMS` 539/539 files, `release_inventory.py --verify` reporting 420 files and listing sha256 `80b39377b1…`, matching the annotated tag message and `release-manifest.json`. Toolchain and suite exercised against the Protocol repository's own reference provider on `rustc 1.97.1`: core 135/135, context 11/11, knowledge 10/10, evidence 16/16 — evidence about the suite and the machine, not about CBR. Stack evidence and its counter-arguments are recorded per area in [STACK](../work/readiness/STACK.md), each from primary sources observed 2026-09-16.

## Consequences

Milestones M1 through M3 proceed immediately and still need no model provider — that independence was designed in and is worth keeping even now that a provider exists, because it is what lets M3 answer *"is the deterministic packet useful on its own"* without a model in the room to flatter the answer.

With the grant in place, **M4 can be accepted on live evidence and M7 can start.** What remains blocked is narrower and has a different cause: journey 7 waits on a PIO release, and the ten executor-bearing `composition` fixtures wait on an execution peer. Neither is a spend question.

Two obligations follow from *how* the grant is framed. The quota is **shared with the owner's own tools**, so the budget envelope is no longer only a cost control — an overspend degrades the owner's working environment, which makes debit-before-call load-bearing rather than tidy. And a 5-hour window plus a monthly ceiling are **two counters with different reset semantics**: the month can be almost untouched while the window is exhausted, so the envelope carries both and admission checks both. A single monthly counter would let CBR pass admission and still starve the owner's next session.

The `cbr search` diagnostic and the mutant set are additions to the proposed scope, not reductions. The commit-id addition to the evidence descriptor is a small widening of CBR's own descriptor use, not a protocol change.

## Verification

Documentation structure only, at this revision: `python3 scripts/check_docs.py` and `python3 scripts/check_docs.py --workspace ..` both exit 0 with 0 errors; `git diff --check` clean. **No runtime check exists yet.** Each decision above acquires real verification in the milestone that implements it, with the acceptance stated in [RELEASE-SCOPE §4](../work/readiness/RELEASE-SCOPE.md); M1 must introduce the repository's first reproducible build and test commands into [VERIFICATION](../VERIFICATION.md) in the same change that introduces the code they check.

## Superseded

Nothing. This is the first CBR-local decision record. It does not alter [ADR 001 in Combraton](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md), which retains the standalone-first sequencing.
