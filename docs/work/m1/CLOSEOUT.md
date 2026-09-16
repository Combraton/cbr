# M1 close-out

The closing record of milestone M1, posted as the [closing comment on issue #3](https://github.com/Combraton/cbr/issues/3#issuecomment-5702383702) on 2026-09-16 and kept here so it survives outside GitHub. The text below is the comment, unchanged apart from this heading and paragraph.

## M1 complete

Merged in order, no squash, as merge commits: stages (a) and (b) #4 as `a939446` · (c1) #5 as `8b75129` · (c2) #6 as `da1e650` · (c3) #7 as `8ba2594` · owner follow-ups #8 as `f00faaf` · (c4) #9 as `9a7b8f5` · (d) #10 as `96a33f2` · (e) #11 as `6c63d91`. Evidence for every row below is in `docs/work/STATE.md` at `6c63d91`, with commands, exit statuses and results under `conformance/results/`.

### Outcomes: the five result sets M1 gates

The encoding vectors plus the four fixture suites this issue names. Each fixture suite is gated in CI on both `ubuntu-latest` and `macos-latest` by `scripts/check_results.py`, against an exact outcome multiset: pass count, total, the named unsupported set, and zero `fail`, `timeout`, `harness_error` or `skipped`.

| Suite | Total | Pass | Unsupported | Fail | Expectation | Results |
|---|---|---|---|---|---|---|
| Encoding vectors (`encoding/1`) | 31 | 31 | 0 | 0 | every pinned vector: 12 canonical, 18 rejected **for the stated reason**, 1 command intent | `cargo test` |
| `stream` | 24 | 24 | 0 | 0 | `stream.json` | `m1b` |
| `core` | 135 | 130 | **5, permanent** | 0 | `core-c4.json` | `m1c4` |
| `socket` | 13 | 11 | **2, permanent** | 0 | `socket.json` | `m1d` |
| `evidence` | 16 | 16 | 0 | 0 | `evidence.json` | `m1e` |

Every expectation was derived from the fixtures' declared profiles, features, controls and barriers **before** implementation, and confirmed by a run with the claims declared and nothing implemented. c3 showed that such a derivation is a **lower bound**: one fixture depended on event recording its features did not name.

### Observable acceptance

- **`cbr ingest` then `cbr fetch` over the public socket, byte-identical, checked against the sealed digest, after `SIGKILL` and restart:** `ingested_bytes_fetch_identically_after_sigkill_and_restart`. `cbr` is a separate binary that links no provider code. `fetch_refuses_bytes_that_do_not_match_the_sealed_digest` shows it does its own check even when the provider serves altered bytes.
- **Storage crash matrix, fault-injected by real termination:** each STORAGE §5 row that exists in CBR at M1 is killed with `SIGKILL` at a barrier on its boundary, with no response written, then asserted after restart: before commit; **between append and acknowledgment**; during upload; **the orphan object** (object published, seal row not committed); during collection. The rows that do not exist at M1 are listed in VERIFICATION, each with its reason.
- **Distinguishing:** an in-memory store fails the `SIGKILL` tests. A regenerated payload fails the byte comparison and the digest check. A fetch served from a cache fails `integrity-failure-is-never-served`, whose object is damaged on disk.

### Permanent coverage limits

- **5 `core` fixtures** declare `execution` and need `executor.script`: `core.events.older-consumer-is-closed-without-notice`, `core.events.slow-consumer-is-closed-with-notice-and-resumes`, `core.events.slow-consumer-recovery-reports-retention-gap`, `core.events.unread-ending-notice-does-not-hold-the-connection`, `core.feature-dependencies-match-negotiation`.
- **2 `socket` fixtures** declare `execution`: `socket.closing-a-session-does-not-cancel`, `socket.idle-obligation-overdue-without-traffic`.
- **`core.effects` and `core.events.backpressure` have no fixture evidence in CBR's role.** All 70 fixtures declaring either also declare `execution`. Both rest on CBR's own mutation-checked tests, and neither should be called conformance-tested.

### Limits of this environment and of the method

- **Different-user peer check:** a coverage limit, not a pass. Protocol's CI tests it only as root through passwordless `sudo`, which this machine does not have. The refusal has no test that could fail.
- **`SIGKILL` is not power loss.** Durability against power loss rests on `synchronous=FULL` read back from the open connection and on `fsync` of objects and their directories, not on a test.
- The crash-matrix kills run over stdio; the socket's own `SIGKILL` test covers two concurrent sessions. An error, as opposed to a crash, is recovered only at the next start. Only evidence artifacts are object-store roots. A partial `cbr ingest` is not resumed.

### Found by M1's own tests and fixed within M1

- A purge killed between commit and deletion left bytes on disk permanently under `purged`. Fixed by a start-time collection pass.
- That pass was unsafe with two providers over one data directory. Fixed by an exclusive lock.
- A missed CI expectation (c3), a conformance default overwriting an explicit principal (d), and a backpressure test that never withheld anything (c4). All three are recorded in STATE.

### Every mutant

Each one was observed failing and then restored. A mutant that survived, or that was malformed, is not counted as a kill. "Second guard" means the mutant was caught by a different check from the one it targeted. "Probe" means it moved a barrier rather than breaking product code.

| Stage | Mutant | Killed by |
|---|---|---|
| a | Object members sorted by code point, not UTF-16 | pinned `key-order-utf16-rfc8785` vector; `member_order_is_utf16_not_code_point` |
| a | Extensions not in `requires` enter the intent | pinned intent vector |
| a | A slash `requires` entry missing from `extensions` tolerated | `a_requires_entry_with_a_slash_must_be_present_in_extensions` |
| a | Duplicate `requires` tolerated | `duplicate_requires_entries_are_refused` |
| b | Pre-negotiation frame limit never applied | `stream.frame-limit-raised-after-negotiation`, `stream.frame-over-limit-closes`, step 2 |
| b | `max_array_items` decided before `max_depth` | `depth_outranks_every_other_limit` |
| b | Scalars counted toward depth | three limit tests |
| b | The gate itself: `core` results against the `stream` expectation | `check_results.py` |
| c1 | Production serves `core-test` | `a_production_provider_does_not_serve_core_test` |
| c1 | Production ignores a test control | `a_production_configuration_refuses_a_test_control_rather_than_ignoring_it` |
| c1 | Query authorization after the subject lookup | `core.grants.authorization-without-grants-feature` step 2 |
| c2 | Events outside the command transaction | `core.events.commands-append-contiguous-events` step 4, 15 fixtures fail |
| c2 | A sequence gap hidden | `core.events.retention-gap-returns-snapshot` step 9 |
| c2 | Published digest verified from the buffer, not disk | two store tests |
| c3 | No per-delivery re-check (carried from c2) | `subscription-ends-at-grant-expiry` 11, `…-when-grant-revoked` 9, `…-when-grant-stops-authorizing` 10 |
| c3 | Revocation does not cascade | `core.grants.revocation-cascades` 10; `core.events.multi-event-command-contiguous` 11 |
| c3 | A grant honoured past expiry | `expired-grant-refused` 10, `expiry-instant-is-exclusive` 11, `test-clock-never-moves-backward` 8, `subscription-ends-at-grant-expiry` 11 |
| c3 | A grant held by anyone authorizes | `holder-only` 6, `denial-reason-order` 8 |
| c3 | Step 6 before deduplication | `revoked-grant-refused-replay-kept` 14 |
| c3 | Revocation acknowledged, not written | `a_grant_and_its_revocation_both_survive_sigkill` |
| c3 | Changed capability subject missing from a retention snapshot | `a_retention_snapshot_lists_the_capability_subject_once_it_has_changed` |
| c4 | `unknown` admits the command | `core.capabilities.unknown-is-not-supported` 2 |
| c4 | Capability loss ignored | four `core.capabilities` fixtures |
| c4 | Claim depends on writes | `claim-does-not-depend-on-writes` 2 |
| c4 | Capabilities checked before deduplication | `loss-refuses-new-but-replays-bound` 7, `checked-after-authorization-before-preconditions` 2 |
| c4 | Every start announced as a capability change | `a_capability_change_and_its_event_survive_sigkill_at_their_positions` |
| c4 | Room wait without a deadline | three session tests |
| c4 | `consumer_too_slow` without negotiation | `an_older_consumer_is_closed_without_a_notice` |
| c4 | Notice budget restarted per subscription | `a_consumer_that_never_reads_is_closed_within_twice_the_notice_budget` (1.54 s against 600 ms) |
| c4 | Notifications ignore the bound | same (3558 bytes against 2048) |
| c4 | A withheld notification's cursor moves | `withheld_notifications_are_delivered_later_and_never_skipped`. **It survived at first, with 0 withholdings in 120 writes; the test was strengthened to 121** |
| c4 | Abort records the effect failed | two effects tests |
| c4 | Overdue counted satisfied | two effects tests |
| c4 | Overdue announced every tick | two effects tests |
| c4 | Deadline exclusive of its instant | two effects tests |
| c4 | Absent effect `not_found` under a grant | `reading_an_effect_under_a_grant_does_not_reveal_whether_it_exists` |
| d | No authentication required | three `authentication` fixtures, step 2–4 |
| d | Unsafe socket directory accepted | `socket.unsafe-directory-refused` step 0 |
| d | Malformed clock file accepted | `socket.malformed-clock-file-refused` step 0, **exactly the fixture's declared kill** |
| d | Revoked credential accepted | `authentication-failures-indistinguishable` step 3 |
| d | Re-check outside the processing lock | `socket.subscription-recheck-race-regression` step 13, **exactly the fixture's declared kill** |
| d | Idle sessions inert | four socket fixtures (timeouts). **Broader than the fixture's named mutant, so recorded under its own name** |
| d | A connection advances the deduplication generation | `two_socket_sessions_one_mid_subscription_survive_sigkill` |
| e | Seal without digest check | `seal-refuses-digest-mismatch-and-is-idempotent` step 4, **second guard** |
| e | Append accepts a received offset | `chunk-retransmission-and-conflicts` step 8, **second guard** |
| e | Purge ignores holds | `expired-hold-stops-protecting` 7, `purge-requires-release-authority-for-holds` 19 |
| e | Withheld manifest child evaluated | `manifest-completeness-respects-authorization` 45 |
| e | Stored bytes not verified on read | `integrity-failure-is-never-served` 11 |
| e | Work binding ignored | `publish-grant-bound-to-work` 13 |
| e | `cbr fetch` skips its digest check | `fetch_refuses_bytes_that_do_not_match_the_sealed_digest` |
| e | `cbr fetch` stops after one chunk | `ingested_bytes_fetch_identically_after_sigkill_and_restart` |
| e | `cbr ingest` in one append | same |
| e | A restart rotates a live credential | same. **It survived at first because the provider binary was not rebuilt; that run is not counted and the kill is from the rerun** |
| e | Rotation keeps the old credential | `a_rotated_or_revoked_credential_no_longer_authenticates` |
| e | Authentication ignores revocation | same |
| e | Revocation does nothing | same |
| e | `received` outside the command transaction | `before_commit_nothing_is_accepted_and_the_same_command_applies` |
| e | Deduplication record not consulted | `after_commit_before_the_acknowledgment_the_retry_replays_and_appends_nothing` |
| e | Seal row before object | `an_object_published_before_its_seal_row_is_collected_and_never_served` |
| e | Purge deletes before commit | `a_purge_killed_before_deletion_is_finished_at_restart_and_rechecks_roots` |
| e | No start-time collection | orphan and collection tests |
| e | Confirmed purge still a root | collection test |
| e | Roots ignore every artifact | collection test, **earlier than its target assertion** |
| e | Data-directory lock never refuses | `a_second_provider_over_the_same_data_directory_refuses_to_start` |
| e | *Probe:* before-commit barrier past the commit | before-commit and during-upload tests. **The during-upload row has no product-defect mutant** |
| e | *Probe:* after-commit barrier before the commit | after-commit test |

**Declared kill steps.** Two d-stage kills match a fixture's declared kill step and reason exactly. For the others, the steps are the observed ones. Most fixtures name their mutants without declaring kill steps.

