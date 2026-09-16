# Journey verification for standalone CBR

> **Status: proposed structure, no journey has been run.** Every result column below is empty on purpose. This file is the place journey evidence lands; it is linked from [VERIFICATION](../VERIFICATION.md) and aligned with the shared [verification model](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md). Scope and milestones: [RELEASE-SCOPE](../work/readiness/RELEASE-SCOPE.md).

A journey is the *journey* layer of the shared evidence ladder. Lower layers — build, component, integration — remain necessary and are recorded with the code that introduces them. They cannot substitute for a journey, and a journey cannot substitute for them.

## 1. Rules this document enforces

- **`not_evaluated` never counts as a pass.** An unavailable model, an unreachable provider or an unbuilt adapter makes a journey *indeterminate*, which blocks only the claims that need it.
- **A well-formed packet is not proof of useful memory.** Schema validity, resolvable citations and a fitting token budget are mechanical checks. They are reported separately from whether the packet helped.
- **A replayed transcript is not a live model run.** Deterministic fake models are used for fault injection and are labelled `simulated` in every row they appear in. A journey whose model was simulated may never be reported as live-model evidence.
- **Mechanical citation validity is not semantic support.** A citation that resolves to an exact byte range says nothing about whether those bytes support the sentence citing them. Semantic support is assessed separately, by an oracle that did not produce the packet.
- **Every important correctness claim carries a negative control** — a deliberately broken variant that must fail, for the stated reason, at the stated step. A control that fails for the wrong reason is a failed control.
- **Cost is part of the result.** Cold initialization, maintenance, retrieval and investigation all count. A journey with no cost recorded is incomplete.

## 2. What each journey record must contain

| Field | Meaning |
|---|---|
| Intent and acceptance | What the consumer wanted, and the criterion that distinguishes success from a plausible-looking result |
| Entry point | The actual public operation or command invoked; no internal shortcut |
| Prerequisites and inputs | Registered repositories, grants, configuration, initial memory state (cold, prepared or warm-repeat) |
| Implementation basis | CBR commit, protocol pin, schema and fixture digests, OS and toolchain |
| Model and provider | Provider, model identity, configuration, capacity — or `none`, or `simulated (fake-<name>)` |
| Code and environment basis | `repository.id` and `tree` per repository, `workspace`, `dirty.snapshot_digest`, `environment`, `build` |
| Path taken | The actual evidence → derivation → revision → packet → consumer chain, by record identity |
| Packet identity | Packet ID, revision, sealed artifact reference and digest; citations; omissions with reasons; coverage frontiers and gaps |
| Durable result | What is still true and visible after a service restart, by re-reading the same identities |
| Reproduction | Exact commands, exit statuses, artifact locations |
| Cost | Model calls, tokens in and out, wall clock, time to first useful work, spend |
| Simulated or untested | Every component that was faked, and every segment of the path that was not exercised |
| Properties and limits | Which declared properties passed, which are `not_evaluated`, and what this journey does **not** establish |

## 3. Journey matrix

| # | Journey | Milestone | Needs a live model | Needs PIO | Primary negative control | Result |
|---|---|---|---|---|---|---|
| J1 | A user ingests a real repository and its decisions through the public client, requests code-flow context, and receives a useful cited packet | M3 (deterministic), revisited at M4 (model-assisted) | no for M3, yes for M4 | no | **Stale-source reuse:** move the repository to a new tree without re-ingesting; a packet claiming applicability to the new tree must not be produced. The control fails if the packet is still `applicable`. | — |
| J2 | Large search results, test logs and JSON are processed under bounded model context; omitted material stays explicit | M4 | yes | no | **Silent truncation:** remove the omission marker path; the journey must fail because omitted content is no longer declared. Also: feed an input larger than the summarizer's own capacity and require a typed insufficient-capacity result, never a silent partial summary. | — |
| J3 | A source or requirement changes during preparation; the next delivery exposes the correction and the earlier packet and its history survive | M5 | yes | no | **Correction swallowed:** an older derivation must not become the current result for a corrected item. Removing the `corrected_during_preparation` path must make the journey fail. The earlier packet revision must still fetch its original bytes and report `current: false`. | — |
| J4 | CBR is restarted during model-assisted work; it resumes from durable state without duplicate commits or fabricated completion | M5 | yes | no | **Duplicate commit:** kill the process between the model response and the commit, restart, and require exactly one committed revision. Disable command deduplication and the control must produce two. Separately: a job whose checkpoint is intact must not report a finding it never derived. | — |
| J5 | Conflicting branches or stale evidence do not leak into another task as current truth | M5 | yes | no | **Branch leakage:** a requirement accepted only on branch B must never appear as binding in a packet scoped to branch A. Remove the scope filter and the control must catch it. | — |
| J6 | A fresh agent session uses a CBR packet on a realistic refactoring task; measure whether it preserves constraints and reduces repeated investigation | M7 | **yes** | no | **Unsupported-assertion acceptance:** plant a claim whose cited evidence does not support it, and require either that it is not carried as `binding` or that the downstream session is not led into the error. Compared against a strong native-context baseline, a disciplined-notes baseline and a plain-search baseline. | — |
| J7 | Real public-protocol investigation and consumption with PIO, while standalone no-PIO operation still works | after M5, gated on PIO | yes | **yes** | **Recursive enrichment and reservation deadlock:** a CBR-initiated investigation must not be re-enriched through the same path, and preparation must not wait on a slot its own consumer holds. Both must be shown to fail when the guard is removed. | — |

### Journeys currently blocked

J6 and J7 cannot run today, for different reasons, and neither will be simulated.

- **J6** needs a granted model provider and a permitted spend, plus pre-agreed thresholds derived from pilot variance. None exists. See [RELEASE-SCOPE §5](../work/readiness/RELEASE-SCOPE.md).
- **J7** needs a PIO standalone service. PIO is being built in parallel and has no release. CBR must not depend on its unreleased work.

J2, J3, J4 and J5 each need a model for their full form. Their fault-injection halves can run against a labelled fake model, and doing so is useful, but the row stays `simulated` until a live run replaces it.

## 4. Oracle independence

For J1 and J6 the question "was this packet actually useful" cannot be answered by the component that produced it, nor by a second model reading the same packet — [VERIFICATION §6](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md) and [MODEL-RUNTIME §5](../spec/MODEL-RUNTIME.md) both say a second model reading the same summary is not independent corroboration.

The assessment therefore uses, in order of preference: an executable test or trace assertion that distinguishes the intended path; a predeclared rubric applied by a human blinded to the arm where feasible; and, only as a supplement for genuinely qualitative judgments, a recorded and calibrated model grader that is never the sole oracle for a correctness claim. Shared lineage between the packet and the grader is recorded whenever it exists.

## 5. Per-journey records

Nothing to record yet. Each journey gets its own section here as it is run, using the fields in §2, and links its run artifacts. A journey that fails, or that runs only partially, keeps its record with the failure described; records are not deleted to keep this page green.
