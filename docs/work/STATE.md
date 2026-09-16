# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. Active task: [issue #3](https://github.com/Combraton/cbr/issues/3), milestone M1 stage (c4), on branch `m1c4/core-capabilities`, stacked on the owner follow-ups branch `owner/ceiling-builddigest-transcripts` (PR #8). Merge commits keep SHAs, so this head does not change when #8 merges. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Merged:** PR #2 as `d68e9d6`, pinned to `877139f`; PR #4 as `a939446`, pinned to `630011c`; PR #5 (c1) as `8b75129`; PR #6 (c2) as `da1e650`, pinned to `b00ec49`; **PR #7 (c3) as `8ba2594`, pinned to `5b98a9f`, confirmed from `merged: true` and `merged_at: 2026-09-16T15:31:51Z`**. Earlier: PR #6 pinned to `b00ec49`, confirmed from `merged: true` and `merged_at: 2026-09-16T14:26:34Z`**, the draft marked ready first and the head re-read unchanged before merging. Every owner decision, including the ceiling, is in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md).
- **Merge rule, 2026-09-16, superseded the same day.** This session ran `gh pr merge` on PR #2 after the owner replied "you can merge PR 2" in-session, having first reported the contradicting claim with evidence and waited. It landed the exact reviewed head `877139f` and is kept. A stricter rule was then recorded, and the owner then **granted merge authority under four conditions**, now in [AGENTS.md](../../AGENTS.md): pin with `--match-head-commit`; the head's CI is green; the reviewer has seen that head; no squash. Confirm from `merged` and `merged_at` afterwards, **never `merge_commit_sha`** — GitHub populates that on an open pull request with the test-merge candidate. Tags and releases remain the owner's alone.
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M1 stage (c4)

`core.capabilities` fixture-backed; `core.events.backpressure` and `core.effects` on CBR's own tests. Three commits, one per feature.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 21 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **60 tests**: 19 encoding, 34 provider unit, 7 against the real binary |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | **130 pass, 5 unsupported by name, 0 fail** |
| `result_paths.py conformance/results/*/` | 0 | no machine paths in m1b, m1c2, m1c3, m1c4 |

**Expected versus measured.** `core-c4.json` was computed from declared features first, then confirmed by a run with `core.capabilities` declared and nothing implemented: 122 pass, 8 fail, 5 unsupported — the 8 exactly the non-execution fixtures naming `core.capabilities`. Measured at `eadd556`: 130 / 5 / 0, stable over three runs. **The c3 lower-bound effect was checked for and not found.** Declaring `core.events.backpressure` and then `core.effects` each changed no fixture outcome.

One timeout was seen, on `core.acknowledgment.effect-refs-empty-for-effect-free-operations` step 1, in a run made against a stale binary while a build was failing. It did not reproduce in three runs of the built head.

### What changed

- **`core.capabilities`** (`eadd556`): the query, unprotected; `capability_unavailable` at step 7 before the epoch and preconditions and after deduplication, so a bound command replays; only `put` depends on `core-test.writes`; an unnamed predicate is `unknown`. A production launch reconciles an empty snapshot so the query answers.
- **`core.events.backpressure`** (`f48dbb7`): the session loop moved into `session.rs` so tests drive it. Output within `max_pending_notification_bytes`; a notification that would exceed it is withheld with its cursor unmoved; a response waits for room; a stall gives the consumer `backpressure_notice_ms` to drain everything; too slow means one `consumer_too_slow` notice per subscription under one shared budget if negotiated, none if not, then closure. On stdio closure is process exit, recorded on stderr with the bound measured. Negotiating the feature adds both bounds to `limits`.
- **`core.effects`** (`316f052`): effects as `core.effect` subjects recorded in the authorizing command's transaction; `core.effects.get`; `core.effects.abort_obligation`, which never changes status; obligations marked overdue once by a provider-origin event when their deadline passes; the producer API for M4.

**Correction to STACK §2's wording.** "The session thread never blocks on a consumer that stopped reading" is too strong. It waits for the room deadline and no longer, and never inside a write; on one connection that wait is the backpressure. A command's commit never waits on output.

**Accepted by the reviewer, 2026-09-16.**

- **A deliberate divergence from the reference provider, not from the specification.** Under a grant, `core.effects.get` reports `out_of_scope` whenever the effect's target is not readable, including when there is no effect. CORE-12 requires only that an existing and an absent effect are refused identically, and does not fix the reason; CBR satisfies that. The reference provider reports the reason computed from its single target family's read right, which CBR cannot do because its effect targets will not share one family and an absent effect has no right to evaluate. A consumer written against the reference may see a different `details.reason` from CBR for the same situation; it will never see CBR distinguish existence.
- **Two gaps recorded** in [PROTOCOL-PIN §4](readiness/PROTOCOL-PIN.md) and filed together as [Combraton/protocol#12](https://github.com/Combraton/protocol/issues/12): **G6**, the overdue-obligation event's unnamed subject and payload (CBR uses the effect subject and `{ effect, obligation, target }`); **G7**, the stale "not normative" banner on §19.

### Mutants

All observed, all restored. Fixture steps are the runner's numbering.

| Mutant | Killed by | At |
|---|---|---|
| `unknown` admits the command | `core.capabilities.unknown-is-not-supported` | step 2 |
| Capability loss ignored | `checked-after-authorization-before-preconditions` 6 · `loss-refuses-new-but-replays-bound` 8 · `restored-capability-admits-commands` 2 · `unknown-is-not-supported` 2 | — |
| Claim depends on writes | `core.capabilities.claim-does-not-depend-on-writes` | step 2 |
| Capabilities checked before deduplication | `loss-refuses-new-but-replays-bound` step 7 (the replay refused) · `checked-after-authorization-before-preconditions` step 2 (before authorization) | — |
| Every start announced as a capability change | `a_capability_change_and_its_event_survive_sigkill_at_their_positions` | "the snapshot survives SIGKILL" (revision 3, not 2) |
| Room wait without a deadline | `never_reads` and `older_consumer` watchdogs · `returning_within_budget` | test |
| `consumer_too_slow` to a session that did not negotiate it | `an_older_consumer_is_closed_without_a_notice` | test |
| Notice budget restarted per subscription | `a_consumer_that_never_reads_is_closed_within_twice_the_notice_budget` | closed after 1.54 s against 2 × 300 ms |
| Notifications ignore the bound | same test | 3558 bytes before any notice against 2048 |
| A withheld notification's cursor moves | `withheld_notifications_are_delivered_later_and_never_skipped` | "sub-1: … none skipped" |
| Abort records the effect as failed | `effects::aborting_a_wait_never_changes_the_effects_status` · `an_overdue_obligation_is_announced_…` | EFF-4 |
| Overdue counted satisfied | `a_passed_deadline_makes_an_obligation_overdue_not_satisfied` · the provider test | test |
| Overdue announced every tick | both | "one provider-origin overdue event, not one per request" |
| Deadline exclusive of its instant | both | test |
| An absent effect answers `not_found` under a grant | `reading_an_effect_under_a_grant_does_not_reveal_whether_it_exists` | "refused identically (CORE-12)" |

**A mutant that first survived.** Moving a withheld notification's cursor — skipping it — failed no test. The skip test used one subscription, and a response plus one notification always fit the bound together, so nothing was ever withheld: a probe counted **0 withholdings in 120 writes**. It now uses three subscriptions of 300-byte values, withholding on every write (**121** counted), and the mutant is killed. A malformed first attempt at the abort mutant changed nothing; it is not counted.

**Measured backpressure ending** (test log): 1930 bytes pending against a bound of 2048; at most 1930 before any notice, 2446 including three; declared 310 ms after a 300 ms stall began, closed 305 ms later.

### Coverage limits

`socket` (13) and `evidence` (16) have **not** been run. The 5 unsupported `core` fixtures are permanent. **`core.effects` and `core.events.backpressure` have no fixture evidence in CBR's role**: all 70 fixtures that declare either also declare `execution`. Nothing in M1 produces an effect. Effect retention is unbounded, so `effect_history_unavailable` never arises yet. Obligations are marked overdue at the next request, which on stdio is the earliest a passing is observable; the socket binding needs a timer (stage d).

### Next: the two stages that close M1, with expected outcomes stated before implementing

| Stage | Scope | Expected | Why |
|---|---|---|---|
| **(d)** | The Unix socket binding, `core.authenticate`, cross-session delivery, idle re-checks on a timer | **socket 11 pass / 2 unsupported / 0 fail** | 13 fixtures. `socket.closing-a-session-does-not-cancel` and `socket.idle-obligation-overdue-without-traffic` declare `execution` and are permanent. `socket.subscription-recheck-race-regression` needs the barrier `subscription.recheck.after_authorization`; **11 assumes CBR implements and declares it** (10 / 3 / 0 if not). `clock.file` is already declared. A second participant descriptor with `binding: unix`. |
| **(e)** | `evidence/1`: upload, append, seal, fetch, query, holds, purge, manifests; the `evidence.store` control; the `cbr ingest` / `cbr fetch` round trip surviving restart; the storage crash matrix from issue #3 | **evidence 16 pass / 0 unsupported / 0 fail** | 16 fixtures. Reachable only with features `evidence.retention_control`, `evidence.manifests` and `evidence.work_binding` — the last is a grant constraint kind, which CBR currently refuses — and controls `clock.file` and `evidence.store`. |

Both will be confirmed by an unimplemented run before implementing, and both are **lower bounds** in the sense c3 found: a fixture can depend on recording its features do not name.

## Earlier — owner follow-ups

Three owner decisions from 2026-09-16, one commit each.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` | 0 | 21 files, 0 errors |
| `result_paths.py conformance/results/*/` | 0 | m1b, m1c2, m1c3: no machine paths |
| `check_results.py` on `m1b` / `m1c2` / `m1c3` | 0 / 0 / 0 | 24 · 80 + 55 · 122 + 13 — unchanged by the sanitisation |
| `git diff --check` | 0 | — |

- **Ceiling confirmed: 20M tokens per 5-hour window, 200M per month** (`285e579`), in ADR 001, STACK §8.1 and RELEASE-SCOPE §5. The confirmed monthly figure is lower than the `[e.g. 300M]` example. **Correction:** ADR 001 had said the numbers were "written in one place (the envelope configuration)"; no envelope configuration exists yet. M4 introduces it, seeded from STACK §8.1, which is the single source until then.
- **`build_digest` filed as [Combraton/protocol#11](https://github.com/Combraton/protocol/issues/11)** (`63322a8`), an issue for a future minor, recorded in PROTOCOL-PIN §4 G1. Nothing else in the protocol repository was touched. The reproducer fixture **passes against the v0.1.0 reference provider**, which is the evidence: a claim for `build-1` is `applicable` on `build-2`, the kind is `invalid_envelope` at `/payload/conditions/0`, and the environment-folding workaround works while conflating build with environment. The proposed-behaviour fixture fails at propose today. Both ran from a scratch copy of the verified release extraction, with the reference provider built there from the release `Cargo.lock`.
- **The 104 older transcripts sanitised in place** (`d4700eb`), no history rewrite: 24 in `m1b`, 80 in `m1c2`. Each `cbr-run.json` now says they were sanitised after the fact and that the original bytes remain at `b00ec49`. `scripts/result_paths.py` holds the one substitution and the one check; `run_fixtures.py` uses it when recording, it verified all three directories afterwards, and CI now runs it over every committed results directory.

## Earlier — M1 stage (c3)

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
