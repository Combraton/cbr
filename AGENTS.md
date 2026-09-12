# CBR — working instructions

Build standalone evidence-backed memory and exact context delivery. Own immutable evidence, validated memory revisions, applicability, bounded model-assisted jobs and exact task packets. Do not grant project authority, adopt a design or approve your own inferred claims as human intent. PIO is an optional investigation adapter; direct model-provider use must remain possible. No shared writable product stores.

## Read the right sources

Start with [README](README.md) and [the documentation map](docs/README.md), then [memory spec](docs/spec/SPEC.md), [memory engine](docs/spec/MEMORY-ENGINE.md), [preparation/delivery](docs/spec/PREPARATION-AND-DELIVERY.md), and [model runtime](docs/spec/MODEL-RUNTIME.md). Read the [accepted baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md) and relevant shared/domain sections for boundary changes. Follow [the shared development workflow](https://github.com/Combraton/combraton/blob/main/docs/DEVELOPMENT.md); record its commit/revision for multi-session work. Research and old code are references, not silent overrides of accepted decisions.

## Preserve these boundaries

- Keep evidence, typed revisions, derived views and exact delivered packets distinct. Preserve citations, uncertainty, conflicts, source/basis identity and retained historical packets.
- Model assistance is central but fallible. Bound every call and aggregate job/tool output; keep durable progress outside model context. A million-token model or full coding-harness fork is not required.
- Consolidate in bounded background work; expose recorded binding corrections promptly. Brownfield assimilation is progressive, not a default global startup barrier.
- The caller chooses advisory/start/transition obligations. Return explicit gaps; deadline expiry is not proof. Preparation must not deadlock on a resource held by its waiting consumer.

## Work and coordination

Inspect the assigned issue/task, branch, head, worktree and uncommitted changes before editing. Preserve unrelated work. For a large task, persist a small plan with outcome, scope, acceptance, dependencies and next step in `docs/work/` or the linked issue; do not rely on chat alone. One owner per task; one isolated worktree per concurrent writer. Agree shared contracts before consumers diverge.

Use subagents when a bounded independent investigation or review will help; pass scope, relevant invariants, source revisions and expected evidence explicitly. Prefer read-only helpers. Parallel writers require separate worktrees and non-overlapping scope/resources. Collect and verify results. Use separate top-level sessions for independently owned component implementations; no recursive swarm or permanent model-to-repo assignment is required.

Changing extraction/validation, entity or temporal semantics, retrieval, source invalidation, retention, model/tool runtimes, provider libraries or context budgets requires a bounded evidence-backed comparison and explicit failure cases. Record accepted choices and superseded sections in the owning [decision record](docs/decisions/README.md). Escalate a needed change of direction, authority or reserved judgment; routine scoped investigation and repair proceed automatically.

## Verify and hand off

Run `python3 scripts/check_docs.py` from the repository root for documentation changes; see [verification](docs/VERIFICATION.md). Product runtime/build/test commands do not exist yet: do not invent them or report product checks as passed. Add reproducible commands when implementation introduces them.

Future product validation must cover unsupported claims, stale source/branch scope, correction during preparation, exact packet bytes, missing required context and aggregate budget/restart behavior. Quality claims need downstream outcomes and full preparation/model cost.

Review the actual diff at recorded base/head. Before a session ends, persist commits/files, commands with exit status and evidence, unresolved facts, active resources and the next action in the task handoff. Treat old handoffs as historical observations; reconcile them with the checkout. Keep public records free of credentials and private transcripts.

Use existing native harnesses to ship v0.1. Combraton self-development is deferred until all four usable v0.1 releases. Do not install ECC/global hooks, select a model or relax runtime permissions merely because a reference suggests it.
