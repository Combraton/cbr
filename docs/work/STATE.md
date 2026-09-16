# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. Active task: [issue #3](https://github.com/Combraton/cbr/issues/3), milestone M1 stage (c3), on branch `m1c3/core-grants`. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Merged:** PR #2 as `d68e9d6`, pinned to `877139f`; PR #4 as `a939446`, pinned to `630011c`; PR #5 (c1) as `8b75129`; **PR #6 (c2) pinned to `b00ec49`, confirmed from `merged: true` and `merged_at: 2026-09-16T14:26:34Z`**, the draft marked ready first and the head re-read unchanged before merging. All ten decisions and both formerly open owner lines are in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md).
- **Merge rule, 2026-09-16, superseded the same day.** This session ran `gh pr merge` on PR #2 after the owner replied "you can merge PR 2" in-session, having first reported the contradicting claim with evidence and waited. It landed the exact reviewed head `877139f` and is kept. A stricter rule was then recorded, and the owner then **granted merge authority under four conditions**, now in [AGENTS.md](../../AGENTS.md): pin with `--match-head-commit`; the head's CI is green; the reviewer has seen that head; no squash. Confirm from `merged` and `merged_at` afterwards, **never `merge_commit_sha`** — GitHub populates that on an open pull request with the test-merge candidate. Tags and releases remain the owner's alone.
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M1 stage (c3)

`core.grants`: grant records, issue, inspect and revoke, delegation, expiry against a controlled clock, cascading revocation, and step-6 authorization with the existence-hiding rule. Three commits: the owner's decisions recorded, then issue/inspect/revoke, then delegation and expiry.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 21 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **47 tests**: 19 encoding, 23 provider unit, 5 against the real binary |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | **122 pass, 13 unsupported by name, 0 fail** |

**CI on the first push failed, correctly.** At `da19a8e` both platforms measured 122 pass / 13 unsupported, but `.github/workflows/checks.yml` still gated against `core-c2.json`, so the step refused a result that did not match its recorded expectation — better or not. The workflow was missed when VERIFICATION was updated; it now names `core-c3.json`. Every local check above had passed, which is why local checks alone are not the gate.

**Expected versus measured.** `core-c3.json` was computed from declared features **before** implementing, then confirmed by a measured run with `core.grants` declared and nothing implemented: 80 pass, 40 fail, 15 unsupported. The two unsupported beyond the computed 13 were exactly the two fixtures needing `clock.file` — `core.grants.test-clock-never-moves-backward` and `core.events.subscription-ends-at-grant-expiry` — so the reviewer's count of two is measured, not only read from the fixture files. `clock.file` was declared in the descriptor only once implemented. After the first grants commit: 107 pass, 13 fail, 15 unsupported, every failure delegation or expiry. At the head: **122 / 13 / 0**, as computed.

**Where the model was short.** At 121 one fixture still failed: `core.events.visibility-follows-direct-read-authority` declares only `core.events` and `core.grants`, but restarts with `capabilities: {"core-test.writes": "unknown"}` and expects the provider-origin `core.capabilities.changed` event at sequence 7. Event recording is not gated on negotiation (CORE §16.3), so the fixture is right and a features-only computation cannot see the dependency. CBR now records the capability snapshot and its change event, and a retention snapshot lists `core.capabilities` once it has changed, as CORE §16.4 requires — no fixture combines the two, so a store test covers it. The `core.capabilities` query and the `capability_unavailable` refusal stay in c4, and the feature is not claimed. **The c4 expectation should be read as a lower bound on what c4 needs, for the same reason.**

### What changed

- **A grant is a subject** of kind `core.grant` whose record is the subject value, so revisions, the command transaction and durability are the store's, not a second table that could disagree with the first.
- **Step 6 moved.** It ran in `handle()` before digest and deduplication, which was harmless while authorization was a session property. With grants it is wrong: CORE §15.5 has a replay skip step 6. Commands now authorize **between** deduplication and preconditions; queries still authorize first, with nothing to deduplicate.
- **The decision returns a reason, not a boolean.** The holder check is part of *finding* a grant, so another principal's revoked grant is `grant_not_found`, and rights are all checked before any resource.
- **Disclosure follows read authority.** A current revision in `precondition_failed` and a current epoch in `stale_authority_epoch` are shown only to a principal that may read the subject. Event and snapshot filtering use the same function, so "could this principal read that subject" has one answer.
- **Issuing checks validity before authority**: audience, expiry, binding scope, then the issuing rules. A delegated grant's parent is found as the issuer's own, then must be usable, then may not be exceeded — rights, resources, expiry, depth and authority binding.
- **Revocation cascades in one transaction**, primary first, walking *through* already-revoked grants rather than stopping at them, so the cascade does not depend on an invariant holding forever. `Commit` gained `also` for the further subjects.
- **A subscription remembers its grant by id** and re-resolves it before every delivery; a captured copy could not notice a revocation.
- **The provider clock** (`clock.rs`) is the only source of protocol-visible time. `clock.file` follows forward writes, ignores backward and malformed ones, and refuses to start on a malformed file. `recorded_at` now comes from it, and the store's duplicate date arithmetic is gone.
- **Transcripts are redacted at the recording boundary.** The runner expands `{repo}` into the checkout's absolute path; `run_fixtures.py` now substitutes `{cbr_checkout}`, records it in `cbr-run.json`, and refuses to finish if any machine path remains.

### Mutants

All observed, all restored; the suite returned to 122 / 13 / 0 after each. Steps are the runner's own numbering.

| Mutant | Killed by | At |
|---|---|---|
| **Carried from c2:** no per-delivery re-check — the subscription continues under its grant, never re-authorized | `core.events.subscription-ends-at-grant-expiry` | step 11, **timeout**: `no frame within 2000 ms` |
| | `core.events.subscription-ends-when-grant-revoked` | step 9, **timeout**: `no frame within 5000 ms` |
| | `core.events.subscription-ends-when-grant-stops-authorizing` | step 10, `params/ended: missing` |
| Revocation does not cascade | `core.grants.revocation-cascades` | step 10, `result/outcome/revoked: expected length 2, found 1` |
| | `core.events.multi-event-command-contiguous` | step 11, `result/items: expected 4 items, found 3` |
| A grant honoured past its expiry | `core.grants.expired-grant-refused` | step 10, `expected error permission_denied, received success` |
| | `core.grants.expiry-instant-is-exclusive` | step 11, same |
| | `core.grants.test-clock-never-moves-backward` | step 8, same |
| | `core.events.subscription-ends-at-grant-expiry` | step 11, **timeout** |
| A grant held by anyone authorizes | `core.grants.holder-only` | step 6, `expected error permission_denied, received success` |
| | `core.grants.denial-reason-order` | step 8, `expected "grant_not_found", found "revoked"` |
| Step 6 before deduplication for a command naming a grant | `core.grants.revoked-grant-refused-replay-kept` | step 14, `expected success, received error … "revoked"` |
| A revocation acknowledged but not written | `a_grant_and_its_revocation_both_survive_sigkill` | "a grant revoked before the kill is still refused after it" |
| A changed capability subject never listed in a retention snapshot | `a_retention_snapshot_lists_the_capability_subject_once_it_has_changed` | — |

**On "declared steps", stated exactly.** Of the fixtures above, **none declares a kill step for these mutants.** Several name them — `subscription-survives-authorization-loss`, `no-revocation-cascade`, `ignore-expiry`, `accept-any-holder`, `deny-replay-after-revocation` — with `kill_expectations` absent, so the steps in the table are the observed ones. Where a fixture declares a step for a *related* mutant, the observation agrees with it: `subscription-ends-at-grant-expiry` declares `clock-file-ignored` at step 11 with reason `no frame within`, which is where and how both the re-check and the expiry mutants failed. `test-clock-never-moves-backward` declares steps 10 and 12 for its clock mutants; the expiry mutant fails earlier, at step 8, the first expired check, which is what removing expiry altogether should do. **The carried kill is now logged, because it was observed.**

### Coverage limits

`socket` (13) and `evidence` (16) have **not** been run. The 13 unsupported are named in `core-c3.json`; all need `core.capabilities`, eight arrive in c4 and five never will. **One constraint kind is implemented: none.** Issuing a grant with any constraint is refused at `/payload/constraints/0/kind` rather than silently widened; no fixture in scope exercises it. The only authority scope tracked is `core-test`. Grant descendants are found by scanning grant subjects, proportionate to v0.1's counts and not measured beyond them. The `m1b` and `m1c2` transcripts still carry the checkout's absolute path; they are unchanged because rewriting committed evidence, or history, is the owner's call.

## Earlier — M1 stage (c2)

`core.events`: the durable event log, positions, cursors, epochs, retention gaps, subscriptions and the per-connection outbox.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 21 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **37 tests** |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | **80 pass, 55 unsupported by name, 0 fail** |

**Expected versus measured:** the gate was 80 pass / 55 unsupported / 0 fail. Measured exactly that. All 18 events-only fixtures pass, and none needed a test control, as predicted.

### What changed

- **Events commit in the command's transaction** with the state change and the command record (CORE §16.3). `operation_ref` comes from a durable counter minted inside that transaction, so references are unique across restarts.
- **Positions are `(epoch, sequence)`**, contiguous within an epoch. Cursors carry the stream identity, so a cursor from another store is `invalid_cursor`; a cursor past the current epoch's head was never issued; a cursor into an earlier epoch stays valid past that epoch's end, which is what epochs exist to report.
- **Epoch changes and retention gaps are decided per epoch inside the read walk.** Emitting the gap as a global prefix gave the right answer from the start and the wrong one from a cursor in a closed epoch; `gap-spanning-epochs` is what distinguishes them.
- **Retention records a watermark when it discards**, so a read starting at or before it is told events are missing rather than handed the next surviving event.
- **Subscriptions are served from a per-connection outbox** — `Mutex` plus `Condvar` plus a queue with a dedicated writer thread, per STACK §2. The session thread never blocks on a consumer that stopped reading, and queueing gives CORE §16.5's ordering for free: a notification caused by a command on this connection is pushed after that command's response. Output produced but not written is bounded at 8 MiB; over it, items are withheld and produced later, **never skipped**.
- **`item_too_large` is a recorded ending**, not a dropped frame: the subscription ends with an empty notification naming where delivery stopped, and the read of that position returns `internal_error` rather than a view that silently omits it.
- **Authorization is re-checked before every delivery**, so a subscription cannot outlive the authority that created it.

### Mutants

| Mutant | Killed by | At |
|---|---|---|
| Events appended outside the command's transaction | `core.events.commands-append-contiguous-events` (15 fixtures fail) | step 4, `result/items: expected 2 items, found 0` |
| A sequence gap hidden | `core.events.retention-gap-returns-snapshot` (2 fixtures fail) | step 9, `result/items/0/gap: missing` |
| Verify a published digest from the input buffer instead of from disk | `the_published_digest_is_verified_from_the_bytes_on_disk` and `publishing_an_object_verifies_its_digest_and_makes_it_read_only` | — |

All restored; the suite returned to 80 of 135 with 0 failing.

**Carried to c3, by agreement:** the fixture-level kill for *a subscription surviving authorization loss*. All three fixtures that exercise it declare `core.grants`, which c2 does not implement: `core.events.subscription-ends-at-grant-expiry`, `core.events.subscription-ends-when-grant-revoked` and `core.events.subscription-ends-when-grant-stops-authorizing`. The guard is implemented and covered by the unit test `a_subscription_ends_when_its_principal_stops_being_an_authority`, which revokes through the store and shows the next delivery ends the subscription. **No mutant kill is logged for it, because none was observed.**

### Coverage limits

`socket` (13) and `evidence` (16) have **not** been run. The 55 unsupported are named in `conformance/expectations/core-c2.json`; 50 arrive in c3 and c4, five never will. **`core.events.backpressure` is not negotiated**, so no backpressure guarantee is declared and the output bound is CBR's own discipline; `consumer_too_slow` is not implemented. `core.effects` is named by no non-execution `core` fixture. The object store still has no fixture and rests on its own tests.

## Earlier — M1 stage (c1)

The durable store and the Core command path, gated at **62 pass / 73 unsupported / 0 fail**.

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/check_docs.py` | 0 | 21 files, 0 errors |
| `python3 scripts/verify_pin.py` | 0 | 429 files match `BUNDLE-SHA256SUMS`, 420 match the inventory |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | 33 tests |
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
- **Awaiting the owner:** confirmation of the provisional 20M/300M token ceiling (the owner's answer left the `[e.g. …]` markers in place, so CBR adopted the numbers and labelled them provisional), and authorization to file the `build_digest` proposal on the protocol repository. Raised on issue #1. The licence, the provider and the pilot repository are all decided.
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
