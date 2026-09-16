# CBR documentation

Read [repository scope](../README.md), then [memory spec](spec/SPEC.md), [memory engine](spec/MEMORY-ENGINE.md), [preparation/delivery](spec/PREPARATION-AND-DELIVERY.md), and [model runtime](spec/MODEL-RUNTIME.md). These current specs are authoritative for this repository. Most of what they describe is not implemented yet: what exists today is the protocol provider for `core/1` and `evidence/1` and the `cbr` ingest and fetch commands (see [the repository README](../README.md#what-exists-now)).

- [Development workflow](https://github.com/Combraton/combraton/blob/main/docs/DEVELOPMENT.md) — ownership, parallel work, reviews and fresh-session recovery.
- [Verification](VERIFICATION.md) — commands that actually exist, what they establish, and their limits, including the permanent conformance coverage limits and the storage crash matrix.
- [M1 close-out](work/m1/CLOSEOUT.md) — milestone M1's outcome table across all five result sets, coverage limits and every mutant.
- [Current state](work/STATE.md) — the dated session snapshot: the active stage, its measured results and mutants.
- [Decision records](decisions/README.md) — accepted internal choices and supersessions.
- [Task work](work/README.md) — durable plans and handoffs.
- [ADR 001](decisions/001-standalone-v0.1-scope-and-stack.md) — the accepted scope, stack and evaluation posture for standalone v0.1. Every owner decision is recorded there.
- [Implementation readiness](work/readiness/TALK.md) — the discussion ADR 001 settled, with its [protocol pin](work/readiness/PROTOCOL-PIN.md), [release scope and milestones](work/readiness/RELEASE-SCOPE.md) and [stack evidence](work/readiness/STACK.md).
- [Journey verification](verification/JOURNEYS.md) — the journey acceptance matrix and evidence records. J9 has been run, with no model; no other journey has.
- [Shared baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md) — product ownership and invariants.
- [Publication provenance](https://github.com/Combraton/combraton/blob/main/docs/architecture/PUBLICATION.md) — source import and historical material boundary.

For cross-repository work, also read the affected public contracts: [PIO](https://github.com/Combraton/pio/blob/main/docs/spec/SPEC.md), [CBR](https://github.com/Combraton/cbr/blob/main/docs/spec/SPEC.md), and [Protocol](https://github.com/Combraton/protocol/blob/main/docs/spec/SPEC.md). Navigation links track main; task packets pin actual source revisions.

- [Standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) and [accepted sequencing decision](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md).
- [PIO standalone client contract](https://github.com/Combraton/pio/blob/main/docs/spec/STANDALONE-CLIENT.md).
- [Cross-product benchmarks](https://github.com/Combraton/benchmarks).
