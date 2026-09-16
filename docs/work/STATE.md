# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. Active task: [issue #3](https://github.com/Combraton/cbr/issues/3), milestone M1 stage (c1), on branch `m1c1/sqlite-store-and-core`. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Merged:** PR #2 as `d68e9d6`, pinned to `877139f`. Stages (a) and (b) are PR #4 at head `630011c`, rebased onto `main`, retargeted, CI green, awaiting the owner's merge. All ten decisions are in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md).
- **Merge rule, 2026-09-16, superseded the same day.** This session ran `gh pr merge` on PR #2 after the owner replied "you can merge PR 2" in-session, having first reported the contradicting claim with evidence and waited. It landed the exact reviewed head `877139f` and is kept. A stricter rule was then recorded, and the owner then **granted merge authority under four conditions**, now in [AGENTS.md](../../AGENTS.md): pin with `--match-head-commit`; the head's CI is green; the reviewer has seen that head; no squash. Confirm from `merged` and `merged_at` afterwards, **never `merge_commit_sha`** — GitHub populates that on an open pull request with the test-merge candidate. Tags and releases remain the owner's alone.
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M1 stage (c1)

The durable store and the Core command path, gated at **62 pass / 73 unsupported / 0 fail**.

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/check_docs.py` | 0 | 21 files, 0 errors |
| `python3 scripts/verify_pin.py` | 0 | 429 files match `BUNDLE-SHA256SUMS`, 420 match the inventory |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **33 tests** |
| `git diff --check` | 0 | — |
| `build_runner.py --offline` | 0 | runner from the verified archive |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | **62 pass, 73 unsupported by name, 0 fail** |

**Expected versus measured:** the pre-analysis predicted 62 pass and 73 unsupported for c1 from each fixture's declared features. Measured: exactly that. Nothing in the 62 needed a feature after all, and no fixture behaved differently from the prediction.

### What changed

- **CORE §8, read before written.** Section 8 governs operations that act *under* an authority that can be taken over. `core-test.authority.claim` is the takeover itself, carries no `authority_epoch`, and is ordered by its precondition on the authority subject. Verified across the whole pinned corpus rather than from the two failing fixtures: all 198 `core-test.subject.put` commands carry `authority_epoch`, all 16 `core-test.authority.claim` omit it. The reading and its citation are a comment on the command path. **The epoch is now the authority subject's own revision**, so the two cannot diverge.
- **Authorization (step 6) moved into `handle`**, before any operation looks anything up, for every protected operation including queries. The query path previously leaked a subject's absence through `not_found`.
- **`core.authenticate`** per CORE §18 for stdio: the launch configuration assigns the principal, so the operation refuses with `already_authenticated` before and after negotiation. The socket form waits for stage (d).
- **Payload objects are closed**, per §5.1, validated per operation; and a command must carry a precondition on its own subject.
- **`unsupported_profiles` lists only `coordination` and `remote-trust`.** A declaration produces `declared_unsupported`; a profile merely not served produces `unknown_profile`, and `negotiation.execution-requests-against-any-provider` accepts only the latter.
- **The SQLite store** replaces the in-memory one: WAL, `synchronous=FULL`, `foreign_keys=ON`, `BEGIN IMMEDIATE`, with the state change and the command record in one transaction. **PRAGMAs are read back from the open connection**, not inferred from the code that set them.
- **`publish_object`** lands with the full ordering — stage in the destination directory, verify the digest from what was written, `sync_all`, rename, **fsync the parent directory** — and marks published objects read-only. It is deliberately ahead of its caller, has no fixture, and says so in its own doc comment.
- **`core-test/1` is conformance-only.** It is served only under a `combraton-conformance-config/1` launch configuration; a production provider answers `method_not_found` to its operations and refuses a `cbr-config/1` configuration naming any test control.

### Mutants

| Mutant | Killed by | At |
|---|---|---|
| Production still serves `core-test` | `a_production_provider_does_not_serve_core_test` | — |
| Production silently ignores a test control | `a_production_configuration_refuses_a_test_control_rather_than_ignoring_it` | — |
| Authorization after the subject lookup on queries | `core.grants.authorization-without-grants-feature` | step 2, `expected error permission_denied, received success` |

All restored; the suite returned to 62 of 135 with 0 failing. Earlier stages' eight mutants remain recorded above.

### Coverage limits

`socket` (13) and `evidence` (16) have **not** been run. The 73 unsupported `core` fixtures are named in `conformance/expectations/core-c1.json`; 68 become available in c2 to c4, and **five never will** — they declare the `execution` profile, which CBR never serves. `core.effects` is named by no non-execution `core` fixture, so no `core` run gives evidence for it. The object store has no fixture and rests on its own test. No feature, packet or model call exists.

## Earlier — M1 stage (b)

- **Licence: MIT**, decided by the owner on 2026-09-16 and recorded as ADR 001 question 10. `LICENSE` matches Protocol's byte-for-byte; `license = "MIT"` in `[workspace.package]`. `exclude = ["vendor"]` and the archive-built runner stay: the licence removed the Cargo inheritance error, not the reason a fixture result must name the runner that produced it.
- **Stage (b)'s unexercised note is closed by measurement**, not assertion. Within c1's 62-fixture scope, `core-test.authority.claim` is exercised by **6** fixtures and `core-test.subject.get` by **13**. Two of the six already fail on authority-epoch semantics, so those operations were unexercised *and* wrong.
- **Awaiting the owner**, none of which blocks M1: the two ADR 001 blanks (provider/model/ceiling/account, and the journey-6 pilot repository), and authorization to file the `build_digest` proposal on the protocol repository. Raised on issue #1. The licence is decided.
- **Task resources:** no CBR process or service is running. A verified extraction of the release archive lives in this session's scratchpad only; re-create it from [PROTOCOL-PIN §6](readiness/PROTOCOL-PIN.md) if needed — the vendored copy plus `verify_pin.py` is the durable record.
- **Stage (c) is split**, one pull request each, so every gate is honest. Expected outcomes computed from each fixture's declared features:

  | Stage | Claims | Expected |
  |---|---|---|
  | c1 | no features | 62 pass, 73 unsupported, 0 fail |
  | c2 | `core.events` | 80 pass, 55 unsupported, 0 fail |
  | c3 | `core.grants` | 122 pass, 13 unsupported, 0 fail |
  | c4 | `core.capabilities`, `core.effects`, `core.events.backpressure` | 130 pass, 5 unsupported, 0 fail |

  The 13 no-feature fixtures failing today are exactly the 13 `fail` results in the measured baseline. Note `core.effects` is named by no non-execution `core` fixture, so c4's evidence for it comes from elsewhere.
- **Measured c1 baseline at this head:** `50 pass, 12 fail, 73 unsupported`. The `check_limits` fix resolved `core.envelope.limits-at-and-over-boundary`, so the failing set is 12 rather than 13. **None of the 62 needs a feature**: every failure is base `core/1` plus durability. Causes are durability across restart (4), per-principal deduplication scope (1), authority-epoch semantics (2), `core.authenticate` absent (1), payload objects not validated as closed (1), the primary-subject precondition (1), authorization skipped on the query path so a nonexistent subject leaks its absence (1), and negotiating an unserved profile (1). No test control is needed anywhere in the 62; 9 of them restart the provider.
- **Next action:** open the c1 draft pull request against `main` once #4 merges, then stage (c2), `core.events`, expecting 80 pass / 55 unsupported / 0 fail. Superseded plan text follows for reference: the SQLite store from STACK §3 replacing the in-memory store, restart and dedupe generations across restart, PRAGMAs asserted by reading them back from the open connection, and a test proving a production-configured CBR refuses `core-test/1` at negotiation and rejects every test control. Superseded plan text follows for reference: the Core command path against the 135 `core` fixtures, with `core-test/1` reachable only through the conformance launch configuration and a test proving a production-configured CBR refuses `core-test/1` and every control. That stage replaces the in-memory store with the SQLite store from STACK §3 — WAL, `synchronous=FULL`, `BEGIN IMMEDIATE` — asserted by reading the PRAGMAs back from the open connection, and implements the features the Core suite needs (`core.events`, `core.grants`, `core.capabilities`).
