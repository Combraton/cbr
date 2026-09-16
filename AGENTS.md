# CBR — working instructions

Build standalone evidence-backed memory and exact context delivery. Own immutable evidence, validated memory revisions, applicability, bounded model-assisted jobs and exact task packets. Do not grant project authority, adopt a design or approve your own inferred claims as human intent. PIO is an optional investigation adapter; direct model-provider use must remain possible. No shared writable product stores.

## Read the right sources

Start with [README](README.md) and [the documentation map](docs/README.md), then [memory spec](docs/spec/SPEC.md), [memory engine](docs/spec/MEMORY-ENGINE.md), [preparation/delivery](docs/spec/PREPARATION-AND-DELIVERY.md), and [model runtime](docs/spec/MODEL-RUNTIME.md). Read the [accepted baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md) and relevant shared/domain sections for boundary changes. Follow [the shared development workflow](https://github.com/Combraton/combraton/blob/main/docs/DEVELOPMENT.md); record its commit/revision for multi-session work. Research and old code are references, not silent overrides of accepted decisions.

## Preserve these boundaries

- Keep evidence, typed revisions, derived views and exact delivered packets distinct. Preserve citations, uncertainty, conflicts, source/basis identity and retained historical packets.
- Model assistance is central but fallible. Bound every call and aggregate job/tool output; keep durable progress outside model context. A million-token model or full coding-harness fork is not required.
- Consolidate in bounded background work; expose recorded binding corrections promptly. Brownfield assimilation is progressive, not a default global startup barrier.
- The caller chooses advisory/start/transition obligations. Return explicit gaps; deadline expiry is not proof. Preparation must not deadlock on a resource held by its waiting consumer.

## Current standalone-first milestone

Support PIO's standalone client as an optional context consumer using public profiles and explicit caller authority. Retain direct-provider/no-PIO operation; reciprocal harness investigations must not recurse through automatic enrichment or deadlock on the waiting consumer. Follow [release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) and [ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md). Comparative evaluation lives in [benchmarks](https://github.com/Combraton/benchmarks); product acceptance remains evidence-based.

## Work and coordination

Inspect the assigned issue/task, branch, head, worktree and uncommitted changes before editing. Preserve unrelated work. For a large task, persist a small plan with outcome, scope, acceptance, dependencies and next step in `docs/work/` or the linked issue; do not rely on chat alone. One owner per task; one isolated worktree per concurrent writer. Agree shared contracts before consumers diverge.

Use subagents when a bounded independent investigation or review will help; pass scope, relevant invariants, source revisions and expected evidence explicitly. Prefer read-only helpers. Parallel writers require separate worktrees and non-overlapping scope/resources. Collect and verify results. Use separate top-level sessions for independently owned component implementations; no recursive swarm or permanent model-to-repo assignment is required.

Changing extraction/validation, entity or temporal semantics, retrieval, source invalidation, retention, model/tool runtimes, provider libraries or context budgets requires a bounded evidence-backed comparison and explicit failure cases. Record accepted choices and superseded sections in the owning [decision record](docs/decisions/README.md). Escalate a needed change of direction, authority or reserved judgment; routine scoped investigation and repair proceed automatically.

## Verify and hand off

Run the commands in [verification](docs/VERIFICATION.md), which lists every check that exists and states what each does and does not establish. Do not invent a command or report a product check as passed. Add reproducible commands in the same change that introduces the code they check.

Future product validation must cover unsupported claims, stale source/branch scope, correction during preparation, exact packet bytes, missing required context and aggregate budget/restart behavior. Quality claims need downstream outcomes and full preparation/model cost.

**Tags and releases are the owner's alone.** Never run `gh release` or `git tag`, and never push directly to `main`. Prepare the change, report the exact head, and stop.

**Merging a pull request is permitted under four conditions**, all of them, by the owner's decision of 2026-09-16:

- pin the merge with `--match-head-commit` to an exact head;
- that head's CI is green;
- the reviewer has seen that head;
- no squash, so the stage commits and their recorded mutants survive as the review trail.

Afterwards confirm the result from `merged` and `merged_at`. **Never read `merge_commit_sha` as evidence of a merge**: GitHub populates it on an *open* pull request with the test-merge candidate, which looks exactly like a merge commit and is not one.

Review the actual diff at recorded base/head. Before a session ends, persist commits/files, commands with exit status and evidence, unresolved facts, active resources and the next action in the task handoff. Treat old handoffs as historical observations; reconcile them with the checkout. Keep public records free of credentials and private transcripts.

Use existing native harnesses to ship v0.1. Combraton self-development is deferred until all four usable v0.1 releases. Do not install ECC/global hooks, select a model or relax runtime permissions merely because a reference suggests it.

## Session state and prompt lifecycle

Read [current session state](docs/work/STATE.md) at startup and reconcile it with the assigned issue, actual branch/head, diff, task handoff and relevant running resources before acting. Maintain this small navigation snapshot at meaningful checkpoints and before pausing, handing off or completing work. Record timestamp/owner, task and PR links, inspected revisions, completed work, remaining work, decisions and unresolved questions, actual checks/evidence and their limits, active resources, and the next action. Link detailed task records instead of copying transcripts or maintaining a second backlog. GitHub issues own live progress; when unavailable, identify the local task record as a temporary fallback and reconcile it later. Separate sessions do not automatically share context.

Give the human a short update at startup and meaningful checkpoints: what changed or was found, what works with evidence, what remains uncertain or needs judgment, and what happens next. Update stale setup/status statements when implementation makes them false. A snapshot is dated evidence, not permission to repeat completed actions.

Treat one-off kickoff and continuation prompts as temporary instructions tied to a task, owner and source revision. When work advances, rewrite the active prompt to the remaining work and link the current state. On completion or supersession, first preserve useful decisions, outcomes, unresolved items and evidence in durable task/decision records; then delete a disposable prompt or replace its executable instructions with a clearly labeled completed/superseded notice and links to the outcome or successor. Remove or update active links to retired prompts. Do not leave an obsolete prompt looking ready to execute.

Preserve reusable templates as templates. Do not delete accepted specifications, decision history, evidence, user source material, or another session's active prompt as cleanup. For local/untracked prompts, preserve necessary context durably before removal; Git cannot recover an untracked file. Limit cleanup to the current task's owned prompts. If a referenced prompt is unavailable, record the unresolved cleanup instead of claiming it was deleted. Fresh sessions must check prompt status and current state rather than blindly replaying a saved prompt.
