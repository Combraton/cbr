# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-26.
- **Owner/task:** **One Claude Code session is now the whole team** — implementation lead, independent verifier and evaluator, working through workflows — by the owner's brief of 2026-09-25, replacing the builder and reviewer pair. Its goal is a v0.1 release candidate, with each slice ending in a journey a person could watch. Verification that used to be the reviewer's is a workflow phase run by agents that did not write the change; its findings, mutants and journey evidence go in each pull request. **M1 to M4 are complete**, with closing records in [m1/CLOSEOUT.md](m1/CLOSEOUT.md), [m3/CLOSEOUT.md](m3/CLOSEOUT.md) and [m4/CLOSEOUT.md](m4/CLOSEOUT.md). M5's merges so far: readiness `c2917f7` (#35), the CI flakes `dfa4f65` (#36), **m5a** `9cd388a` (#38), the socket fix `5ab1a4f` (#39), **J2 live's prep** `041ad5f` (#40, pinned to `9093dcd`, `merged_at: 2026-09-25T15:02:56Z`, second parent `9093dcd`, tree identical to it), and **J2 live's record** `1e9df23` (#41, pinned to `04ea4cf`, `merged_at: 2026-09-25T16:35:13Z`, second parent `04ea4cf`, tree identical to it). **J2 live ran on 2026-09-25 and failed its rubric**; m5a-3, which fixes what it found, is on `m5a-3/floor` and not merged, and m5b waits for it. Tracking: [issue #34](https://github.com/Combraton/cbr/issues/34); parent [issue #1](https://github.com/Combraton/cbr/issues/1).
- **Merged:** PR #2 as `d68e9d6`, pinned to `877139f`; PR #4 as `a939446`, pinned to `630011c`; PR #5 (c1) as `8b75129`; PR #6 (c2) as `da1e650`, pinned to `b00ec49`; **PR #7 (c3) as `8ba2594`, pinned to `5b98a9f`, confirmed from `merged: true` and `merged_at: 2026-09-16T15:31:51Z`**. Earlier: PR #6 pinned to `b00ec49`, confirmed from `merged: true` and `merged_at: 2026-09-16T14:26:34Z`**, the draft marked ready first and the head re-read unchanged before merging. Every owner decision, including the ceiling, is in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md). **PR #8 (owner follow-ups) as `f00faaf`, pinned to `40cb30b`, `merged_at: 2026-09-16T16:57:29Z`; PR #9 (c4) as `9a7b8f5`, pinned to `5db9d7c`, `merged_at: 2026-09-16T16:57:53Z`**, in that order, each confirmed from `merged` and `merged_at`. **PR #10 (d) as `96a33f2`, pinned to `a1048e6`, `merged_at: 2026-09-16T18:17:38Z`; then PR #11 (e) as `6c63d91`, pinned to `c2a262d`, `merged_at: 2026-09-16T18:17:56Z`**. PR #11 was stacked on #10's branch, so it was retargeted to `main` after #10 merged and before its own merge; retargeting leaves the head unchanged, and `c2a262d` was re-read before merging. **PR #13 (M1 close-out, README) as `5c667ed`, pinned to `507fc77`, `merged_at: 2026-09-16T18:58:44Z`; then PR #14 (M2) as `39113b2`, pinned to `62cbb99`, `merged_at: 2026-09-16T18:59:00Z`**, #14 retargeted to `main` first. **PR #16 (m3a) as `cbfaebe`, pinned to `1bd9a7a`, confirmed from `merged: true` and `merged_at: 2026-09-19T14:48:17Z`**, the draft marked ready first and the head re-read unchanged; the merge commit's second parent is `1bd9a7a`, so the reviewed head is on `main` unaltered. **PR #17 (m3b) as `c3cdf51`, pinned to `fb67250`, confirmed from `merged: true` and `merged_at: 2026-09-19T15:55:37Z`**, second parent `fb67250`. **PR #18 (m3c) as `2cf6b97`, pinned to `81c5f0e`, confirmed from `merged: true` and `merged_at: 2026-09-20T04:35:37Z`**, second parent `81c5f0e`. **PR #19 (m3d) as `17cc54c`, pinned to `52305b3`, confirmed from `merged: true` and `merged_at: 2026-09-20T07:28:00Z`**, second parent `52305b3`. **PR #20 (m3e) as `c726107`, pinned to `47fd574`**, which closed M3. M4's ten merges, from #22 to #31, are listed with their pins in [m4/CLOSEOUT.md](m4/CLOSEOUT.md#m4-complete), each confirmed the same way and each merge commit's second parent the reviewed head.
- **Merge rule, 2026-09-16, superseded the same day.** This session ran `gh pr merge` on PR #2 after the owner replied "you can merge PR 2" in-session, having first reported the contradicting claim with evidence and waited. It landed the exact reviewed head `877139f` and is kept. A stricter rule was then recorded, and the owner then **granted merge authority under four conditions**, now in [AGENTS.md](../../AGENTS.md): pin with `--match-head-commit`; the head's CI is green; the reviewer has seen that head; no squash. Confirm from `merged` and `merged_at` afterwards, **never `merge_commit_sha`** — GitHub populates that on an open pull request with the test-merge candidate. Tags and releases remain the owner's alone.
- **Owner decision, 2026-09-20, recorded at the m3c review: m3d has two pilot repositories**, as an amendment to [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md) question 6. Knowscroll-v2 stays the decision-memory pilot; the owner's **brian2 fork** is added as a brownfield pilot, registered read-only, whose journey tests discovery and code flow rather than decision memory, whose oracle the owner writes before the run, and whose dirty working tree makes it the first journey to exercise the dirty path. **brian2 is CeCILL-licensed and this repository is MIT, so none of its bytes, excerpts or packets are committed here** — digests, paths, spans, counts and costs only. It is recorded now and acted on only after m3c is cleared; no other brian2 work belongs in this pull request.
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — m5g, the composition feasibility check

On `m5g/composition-check`, off `main` at `5ddc26f`. Docs, one descriptor, one launch script and one `build_runner.py` flag, and after the verification round one Rust test file for those scripts. **No product Rust changed, and no model has been called.** The question came from [READINESS §7](m5/READINESS.md#7-j3-j4-and-j5-and-the-fixtures-that-need-an-execution-peer): can the pinned runner launch one composition participant from a different implementation than the others, with CBR as the context and evidence provider and the protocol's reference provider as the executor?

- **The runner: no.** It takes one `--participant` (`vendor/protocol/v0.1.0/conformance/runner/src/main.rs` line 57, loaded once at line 485). `start_participant` launches every named participant with `descriptor: self.ctx.descriptor` (`exec.rs` line 1293), and so do `start` (lines 1091, 1108) and `expect_start_failure` (line 1397). The `start_participant` step schema (`conformance/schemas/fixture.schema.json` lines 1245–1291) has no member naming an implementation, with `additionalProperties: false`. The vendored runner and schema are byte-identical to the extraction `build_runner.py` produces (`cmp`).
- **A descriptor: yes.** Its launch argv is any program, run once per participant with that participant's rendered configuration (`participant.rs` `spawn`, lines 124–161). `conformance/participants/cbr-with-reference-executor-unix.json` runs `scripts/reference_executor_launch.py`, which execs the reference provider when the configuration has `executor` (pinned `launch-config.schema.json` line 167, "Scripted executor test adapter") and the runner launched a named composition participant, refuses an `executor` configuration anywhere else, and execs `cbr-provider` for every configuration without one. `scripts/build_runner.py --reference-executor` builds `combraton-reference-provider --locked` in the same verified extraction and records its path, SHA-256 and the label "reference executor" in `runner.json` under `reference_executor`. In all fourteen fixtures, only the `exe` participants have `executor` at the top level; the context providers carry theirs inside `context`.
- **Control: the reference alone, 14 of 14 pass** (runner run directly with `--repo` at the extraction and the reference's own `reference-provider-unix.json`, out to `mktemp -d`). So every fixture can pass on this machine.
- **The routed run: 11 pass, 2 fail, 1 unsupported, runner exit 1.** The 8 executor fixtures that pass are `cancelling-one-consumer-leaves-the-other`, `claims-in-packets-block-only-the-required-boundary`, `context-provider-outage-advisory-proceeds-required-waits`, `correction-before-dispatch-blocks-the-superseded-revision`, `executor-fetches-bound-packet-and-seals-outputs`, `executor-verifies-assembled-packet-bytes`, `missing-required-context-blocks-bound-work` and `pinned-and-current-bindings-treat-supersession-differently`. The 3 CBR-only fixtures also pass. The two failures, each reproduced twice more:
  - `blocked-consumer-releases-the-slot-for-preparation`, step 20: *not within 15000 ms: expected success, received error {"code":"not_found"}* for `execution.inspect inv-1`.
  - `preparation-runs-without-the-consumer-slot`, step 17: *result/state: expected "ready", found "preparing"* for `r-2`.
- **Who served what, from the transcripts.** Every one of the 11 executor sessions negotiated `max_frame_bytes` 1048576 and `max_array_items` 256, the reference's values (all 44 sessions of the all-reference control show them). Every one of the 32 others negotiated 2097152 and 4096, CBR's (`crates/cbr-provider/src/config.rs` line 43).
- **Why the two fail: CBR, not the route.** They are the only fixtures whose context script has an `execute` step, `{"execute": {"execution": "inv-1", "depth": 1, "call_budget": …}}`, with the context provider configured with `context.executor` (a socket, a credential and the grant `g-ctx-execute`). CONTEXT §4 (CTX-18, CTX-19) requires the provider to submit that execution itself, as a separately authorised caller with `origin`. CBR's `advance` ends in `_ => break` (`context_ops.rs` line 3750, "a step this provider does not perform, such as an investigation execution, holds the job"), as M3a recorded. So `inv-1` never reaches the executor and the job never publishes.
- **No expectation file.** `check_results.py` refuses any `fail`, and a known-failure allowance would change what the gate means. That is not m5g's to decide. Once the context provider submits investigations, the expectation is `composition.`, 14 total, 13 pass, 1 unsupported (`thirdparty-publisher-feeds-reference-verification`), exit 0.
- **The verification fixture stays blocked.** Its one participant, `evd`, answers `evidence.*` and `verification.receipt.*` on the same `owner` session, and the fixture starts the `minimal-publisher` client, which is not vendored. It needs CBR to serve `verification/1`, which RELEASE-SCOPE §3 excludes.
- **Gates at this head.** `check_docs.py`: 0 errors. `verify_pin.py`: ok. `git diff --check origin/main...HEAD`: clean. The seven standard suites, each into its own `mktemp -d`, each matching its expectation: stream 24 pass; core 130 pass, 5 unsupported; socket 11 pass, 2 unsupported; evidence 16 pass; knowledge 10 pass; context 11 pass; composition 3 pass, 11 unsupported. The reference-executor composition run is recorded above and is not gated. No whole-workspace `cargo test` was run, because no Rust changed; `cargo build --workspace` built.
- **Docs.** READINESS §7 (the finding, the ten with outcomes, the verification fixture), the §9 m5g row and "What this document does not settle"; RELEASE-SCOPE §6; VERIFICATION (the command, two composition rows, the `check_results.py` row and "what is established").
- **Verification round: the route is held to the composition suite.** The verifier ran `run_fixtures.py --filter execution.requires-core-features` through this descriptor and got a pass with CBR serving nothing: the launcher sent any `executor` configuration to the reference provider, and the descriptor claims `execution/1` on its behalf, so a single-participant execution fixture became applicable and passed under a CBR-named participant, the misreporting RELEASE-SCOPE §6 forbids. Both ends now refuse it. The launcher routes an `executor` configuration to the reference only when its three paths are the ones `start_participant` gives one named participant N (`exec.rs` lines 1276–1290: `<work>/participants/N/data`, `<work>/participants/N/config.json`, `<work>/n-N/p.sock`; `start` and `expect_start_failure` give `<work>/data` or `data-G`, `<work>/config.json` and `<work>/sK/p.sock`), and otherwise exits 2 without starting anything. It also exits 2 when the reference binary's SHA-256 is not the one `runner.json` now records. `run_fixtures.py` refuses, before creating or starting anything, any descriptor whose launch argv runs the launcher unless `--filter` starts with `composition.` and selects no vendored fixture outside that suite (the runner's filter is a substring match, `main.rs` line 111). `crates/cbr-cli/tests/reference_executor_launch.rs` covers the routing rule, the refusals, the exit-2 paths (bad argv, unreadable configuration, missing or digestless `runner.json` entry, missing or replaced binaries) and `run_fixtures.py`'s refusal, each script run from a copy in a temporary tree against stub binaries; four of its tests failed against `231f808`. Rerun: the misuse is refused by `run_fixtures.py`, and run through the runner directly it now fails instead of passing; `composition.` through this descriptor is unchanged at 11 pass, 2 fail, 1 unsupported, fixture for fixture; the seven standard suites match their expectations. Gates: `check_docs.py` 0 errors, `verify_pin.py` ok, `git diff --check origin/main...HEAD` clean, `cargo fmt --check` and `cargo clippy -p cbr-cli --tests -D warnings` clean, and a whole-workspace `cargo test --workspace --locked` at `4f1ef53` (docs aside): 710 tests over 43 result lines, 709 passed, one ignored, none failed.
- **Second verification round: the route is the `executor` member, not a name.** Two mutants of the launcher survived its sixteen tests: routing an `executor` configuration by a `provider_id` that starts with `executor`, and by `provider_id == "executor-one"`, because every test's executor configuration was also named so. `the_route_is_the_executor_member_and_not_a_name` launches an `executor` configuration named `context-two`, which must start the reference provider, and a configuration with no `executor` named `executor-x`, which must start `cbr-provider`; both mutants now fail it. A whole-workspace `cargo test --workspace --locked` with it (docs aside): **711 tests over 43 result lines, 710 passed, one ignored, none failed**. Its exact-head check then found four more survivors: three routing on the member **or** a name that no configuration without the member used — `provider_id` `executor-one` or the vendored `executor-1`, or the participant `exe` — and one routing on the member's truthiness, which differs only for `"executor": {}`, which the schema allows. The test now also sends an empty `executor` to the reference and three configurations with no `executor`, under every name an executor goes by here and in the vendored fixtures, to `cbr-provider`; all four mutants fail it. The next check found three more, each on a shape the vendored fixtures use and the tests did not: routing on an `executor` key at any depth, which would send CBR's `ctx` participant to the reference in the eight fixtures that start it with `context.executor` (the executor it submits to, CTX-18 and CTX-19) and credit CBR with the reference's context results; and refusing only an `executor` configuration that names a provider, where `execution.requires-core-features`'s own names none. The routing test now sends that `ctx` configuration to `cbr-provider`, and the refusal test refuses that fixture's configuration as it is; all three mutants fail. A configuration with both `executor` and `context`, which the schema allows and no vendored fixture uses, goes to the reference too, so a launcher that let `context` override the member fails as well. The launcher's binary passes 17 of 17 (its test count is unchanged, so the whole-workspace figure stands).
- **Recorded, not changed.** `run_fixtures.py`'s check for a filter that selects a non-composition fixture by substring (`stray`) never fires on the vendored fixtures, since no fixture id contains `composition.` without starting with it, so its two mutants survive; it is defence in depth, testable only against a fabricated vendor tree. The launcher's `participants` directory check is redundant with its socket check under the runner's real paths, so dropping it survives the tests without opening a hole. Run directly, the runner's own `--fixtures DIR` could supply a fixture outside the vendored suite that starts a named participant with an `executor` configuration, and that participant would go to the reference: the guard is judged by path shape, not fixture id, and "a named composition participant" is exact only for the vendored suite, which is the only one `run_fixtures.py` runs. And a verification setup note: with `<worktree>/target` symlinked to a `CARGO_TARGET_DIR` outside the repository, `j2_harness`'s `the_harness_refuses_every_run_it_should_before_anything_is_spent` fails, because its output-inside-this-repository case resolves through the link; whole-workspace runs are made with the link removed.

**PROPOSED, for the owner. Not filed.**

1. **Protocol runner: a descriptor per composition participant.** Today one descriptor names the whole run. The manifest's `participant` block therefore credits `cbr-with-reference-executor-unix` with `execution/1`, which is really the reference's. The transcript's `start_participant` note shows the launch script, not the binary it chose. And a named participant's `stderr.log` is not copied out of the run's temporary directory. A runner option such as `--participant-for <name>=<descriptor>`, with each participant's descriptor digest in the manifest, would make a mixed-implementation result attribute itself. This is a protocol-repository change. CBR's launch script covers the need until then.
2. **CBR: context-initiated investigation executions (CTX-18, CTX-19).** The `execute` script step and the provider behaviour behind it: submit to the configured executor as its own principal under the configured grant, with `origin: {initiator, depth, call_budget}`, then resume the job when the execution returns. This is the `execution/1` client role PROTOCOL-PIN gives CBR for PIO-backed investigation. It would turn the two failures into passes and let this route get an expectation file. Where it goes in the M5 order is the owner's call.

## Earlier — m5a-3

On `m5a-3/floor`, first off `main` at `041ad5f`, rebased onto `2eedade`, which holds J2 live's record (#41) and `cbr expand` (#42), and in the second fix round onto `5ddc26f`, packet ids within the identifier grammar (#43). **The SHAs in this section are those before that last rebase**, except the second and third fix rounds'; each is now: `0073c33` `57456d1`, `479bcdf` `96a4a0f`, `2168435` `038f0c4`, `b70c1fc` `584792a`, `1648d8b` `5db26d3`, `ef2b2d0` `ef8eaf3`, `c9c739a` `7c77f4b`, `73bd7e4` `c388e7d`, `e01016b` `b3da21f`, `e32d57b` `447481d`, `2bd43e8` `9d60957`, `9654d4b` `d6fbb51` and `1bc8b76` `7ad9fae`. The last rebase's conflicts were in two docs, STATE's head and VERIFICATION's count, each resolved keeping both changes; the code merged cleanly, and `context_ops.rs` builds the projection's omission id with #43's `crate::ids::omission` beside m5a-3's projection. The builder's four commits were `0073c33` (tests first), `479bcdf` (the floor), `2168435` (tests for the survivors) and `b70c1fc` (code docs and counts); before the rebase they were `2c790c3`, `02663a9`, `28383ce` and `4621f29`. Three independent verifiers then read `4621f29`. The fix round for their findings is `1648d8b` (tests first), `ef2b2d0`, `c9c739a`, `73bd7e4` and `e01016b` (the fixes), `e32d57b`, `2bd43e8` and `9654d4b` (tests for three mutants the fixes let survive: M4, C2b and J3), and `1bc8b76`, the docs. **The second fix round** is `7fb2158` (tests first), `bd8791a` (the fix), `6c894fd` (a test for N4) and `f167349` (its docs). **The third** is `f81c931` (tests only), `53884d3` (its docs) and the commit after them, which adds the boundary test's premise and these corrections. **No model has been called.** The rule is [READINESS §3](m5/READINESS.md#3-journey-2-first) and [ADR 001's question 16](../decisions/001-standalone-v0.1-scope-and-stack.md); the rerun's plan is [READINESS §10](m5/READINESS.md#j2-live-rerun-after-m5a-3-inputs-estimate-and-ceiling).

### What m5a-3 is

J2 live failed because a model's choice replaced the rule's: whatever a model did not choose was declared `not_selected`, including failure blocks the parser had already named. From `cbr-project-large-result/2`:

- **The rule's projection is a floor.** Every failure the parser found, then run identity, is carried greedily under the rule's own header, then frozen. Both arms compute it identically, and it is never repacked.
- **A model is asked only what to add.** It is offered, per planned part, only the units that are neither failures nor run identity, that the floor leaves out, and that fit beside the floor on their own. That fit is measured with the model arm drawn at its widest: its label listing all 24 excerpt ids, every omission under `over_projection`, and every planned cut group's `unresolved:` line.
  - A part left with nothing to offer is not asked.
  - The questions are `Plan::asked`, each a subset of a planned part, so the part bounds hold for each.
  - A model's pick follows the floor in byte order, and is kept only if the whole still fits.
- **A question no answer can change is not asked.** If the floor does not fit beside the widest drawing, or nothing could be added, the projection is the rule's, labelled as the rule's, with no call. The header's `N parts` counts the questions, so both arms print the same number, and the harness reads it.
- **Capacity comes first.** More than `MAX_PARTS` planned parts, more than `INPUT_BYTES`, or bytes that are not text: nothing is offered and nothing is worked out. This is the fix round's first fix: `offer` draws the whole projection once per unit, and before capacity is decided nothing bounds how many units there are.
- **Nothing is offered where nobody may be asked, and the floor is drawn once a request.** This is the second fix round's. With no model, or no investigation, `partition` works nothing out and draws nothing. A plan it did work an offer out for keeps the rule's floor, and `render` draws on it rather than drawing it again. A plan with no floor has it drawn by `render`, which works the questions out from it for the header's count alone. So the baseline, which may ask nobody, prints the number of questions the assisted arm asks, which the harness reads to set the assisted request's investigation and gate F compares.
- **The labels say who chose what.** `draw` cuts a run wherever the chooser changes, so every excerpt is wholly the rule's or wholly the model's. The header lists the model's by number (`…; then chosen by the model, one question per part, which added e2, e3, e4`), or says `which added nothing`. Because of that cut, a unit a model adds never joins a floor excerpt; `partition`'s comment used to say joins count, and now does not.
- **Each question's preamble says, in counts only, what is carried and the room left**: failing tests, cargo errors, run identity, and bytes and excerpts. The instruction and the preamble now both say the failures are carried *"whatever you answer, as far as they fit"*.
- **A model arm that would not fit is refused.** `render` returns `None`, and the item is `insufficient_capacity`. This is defence in depth: `offer` makes it unreachable.
- **From the run's findings.** F1: the header counts failing tests and cargo errors apart, and in cargo's JSON a `compiler-message` at level `error` is a cargo error. F3: a cargo error stops at the next run's identity. F5: an empty list at a document's root is identity.
- **Unchanged.** The compiler string stays as `main` has it, `cbr-context-compiler/4` since packet ids (#43): the compiler's golden packet has no projection. The golden projection digest is re-pinned: with `/1`, no F1 line and the planned part count put back, the new code renders the old digest byte for byte.
- **The harness.** `checked()` names a model-assisted projection that drops any byte of a baseline excerpt, whole or in part. A run whose baseline needs no question still makes the assisted request, with every part a projection may ask; it must spend nothing, and it is not replayed.

**The figures, computed by tests from real bodies:** a part's worst call is 41,017, a whole projection's 492,204, and beside discovery 865,392, under the job's 1,000,000. At m5a they were 40,680, 488,160 and 861,348. The builder's head had 41,006, 492,072 and 865,260; the fix round's instruction wording is the rest of the difference ([READINESS §8](m5/READINESS.md#8-every-new-bound-has-its-arithmetic-computed-by-a-test-and-a-fixture-that-reaches-it)).

### Red, then green

- **The builder's tests first, `0073c33`**, run again in a scratch worktree for this record. Each failed for its own reason, against stubs that kept m5a's behaviour:
  - 13 of 49 projection unit tests failed: the arm carrying the floor for every answer; no failure `not_selected`; the offer; no question on a full floor; the header naming the model's excerpts; the empty answer; the longest header; the preamble; capacity; F1; F3; F5; and a floor with nothing that fits.
  - 5 of 23 `journey_two` tests failed: the added and floor-carried arm, the unchosen failure, the unsupported record, a full floor asking nothing, and the investigation limit on offered parts.
  - 2 of 9 `j2_harness` tests failed: a dropped baseline excerpt, and a run that offers nothing.
- **The verifiers' findings, at `1648d8b`.** Where the code was wrong, the test failed at that commit:
  - capacity: an input of five parts was offered two questions of 64 units;
  - render: a model arm of 16,424 bytes was published;
  - JSON errors: the header said `cargo errors: 0` over two carried compiler errors;
  - the instruction: it lacked *"as far as they fit"*.

  Where the code was right, the red is the mutant, each applied, seen to fail its test, and reverted:
  - X16 fails `a_whole_artifact_that_fits…` at ` in 0 parts;`;
  - MUT5 fails the investigation-limit journey at `1 of 2`, `2 of 2`;
  - MUT3/X8b fails `a_group_offered_in_one_question_is_not_reported_as_cut`;
  - X4 fails `a_floor_with_no_room_for_the_lines_a_cut_group_takes_is_not_put_to_a_model`;
  - X1, X2 and X10 fail `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`, which reads the bodies the fake transport recorded;
  - X18 fails `a_failure_the_floor_left_out_is_never_credited_to_the_model`;
  - X13 fails the harness's `partial` case;
  - M21d fails the zero-part run at `replay`;
  - P1 with P1b fails `run_identity_the_floor_left_out_is_never_offered_though_it_would_fit`. P1b alone does not fail it (below).
- **The capacity fix's own mutants**, after it: C-parts, C-large and C-next-large each fail `capacity_is_the_inputs_not_the_offers` at their own assertion.
- **Three mutants the fixes let survive**, each given a test observed failing under it:
  - **M4, `e32d57b`.** The verifiers saw M4 killed, but only by the room's subtraction overflowing. With that subtraction saturating, M4 survived. Its new test builds a floor 1 to 6 bytes over the bound at the widest drawing, which carrying one blank line would bring under it. With M4 applied, that test fails with `offered [[58]]`.
  - **C2b, `2bd43e8`.** C2b removes the call site's size check before a read. m5a killed it with a 1 MiB non-text blob. Once `next` refused a too-large input as well, the outcome was the same either way. The oversize journey now alters the blob's stored object on disk: with C2b the item is `evidence_unavailable`, and without it `insufficient_capacity`, because the object is never read.
  - **J3, `9654d4b`.** J3 names a compiler error by `rendered` before `message`. No fixture had both until this commit, and with J3 applied the header test names it by its rendered line, `error[E0425]: …`, instead of its message.

### The second fix round

A verifier read `1bc8b76` again, after the first fix round. No blocking finding; two should-fix, the rest notes. Each is below with what was done.

- **Should-fix: the floor's cost was paid twice.** `partition` worked out an offer, which draws the rule's floor, for every text input within capacity, baseline requests included, and `render` drew the floor again. The floor is one drawing of the whole projection per failure or identity unit, so both were quadratic. The verifier measured it on a debug build over `{"env": {"x": [0,0,…]}}`, all identity, no model asked: 0.50 s + 0.50 s at 16,384 bytes (8,183 units), 5.0 s + 5.5 s at 65,536, and **22.2 s + 22.6 s at 131,072** (65,527 units). **Now** `partition` takes `may_ask` from the call site (`serving.is_some()`: a model, and an investigation above zero) and works nothing out without it; `render` draws on the floor a plan carries, or draws it once and counts the questions from it. The same probe after the fix (scratch, reverted): nobody may be asked, `partition` 0.8 ms and no drawing, `render` 18.1 s; a model may be asked, `partition` 19.0 s and `render` 1.6 ms, one drawing. That is `main`'s cost, one floor a request, as `main`'s `render` paid it; the floor's own quadratic cost is follow-up 4.
- **The header's count, thought through.** Gate F wants both arms to print the same number of parts, and the harness reads that number from the baseline, which may ask nobody, to set the assisted request's investigation; gates H and I then compare it with the questions asked. A count the baseline could print without working the offer out — the planned parts — would have broken H and I wherever a planned part offers nothing, as on the red log (3 planned, 0 asked). So the count stays the questions, and the baseline works it out from the floor it draws anyway: `offer` beyond the floor is at most 258 drawings, since an input within capacity has at most 4 × 64 units that are not identity. `both_arms_print_the_same_part_count_whether_or_not_a_model_may_be_asked` holds it over seven fixtures that ask none, some or all of their parts: the baseline's count equals the assisted plan's questions, and where no question is needed the two projections are equal byte for byte.
- **Should-fix: no test observed the capacity decision's order** (N1: the offer worked out, then thrown away when over capacity, survived the whole workspace). **Now** `draw` counts its drawings on the thread under `cfg(test)`, and `nothing_is_drawn_to_plan_an_input_over_capacity` requires none for five parts, for too many bytes and for bytes that are not text, whoever may be asked. That test kills C-binary too, which survived as equivalent.
- **N5: `render`'s refusal was tested on bytes only.** With the plan now carrying its floor, a floor over `MAX_EXCERPTS` is constructible: `a_model_arm_over_the_excerpts_a_projection_holds_is_refused` hands `render` every other one of sixty tiny records, thirty excerpts in a few kilobytes, and requires `None`.
- **N7: an empty `message.message`.** `a_compiler_error_whose_message_is_empty_is_named_by_what_it_renders` names one error by its `rendered` and one with both empty by its `reason`, and the header lists no empty name.
- **N3: exactly `INPUT_BYTES` within capacity.** Constructible: a JSON document of 131,072 bytes whose run identity string makes up the bytes, with thirty passing results; which note length leaves the floor room is found by searching, and the first eighty lengths ask about three in four. `an_input_of_exactly_the_input_bound_is_within_capacity_and_asked` requires the question.
- **N4: `next`'s order.** Equivalent at the call site, which refuses a too-large artifact before reading it; `bytes_over_the_input_bound_are_insufficient_before_they_are_not_text` gives `next` a binary one byte over the bound directly, and now kills it.
- **READINESS §10's estimate** said about 45,000 below its own derivation; it now states 46,615 to 48,637, computed.
- **Notes recorded, not changed here:** `score_j2.py` still expects `/1` (its header pattern and its C0 digests), which the change that runs the rerun must freeze; `cbr expand` at an offset equal to the artifact's size writes an empty file and exits 0, where an offset past it is refused; M21c stays not testable here. VERIFICATION's two stale texts the verifier found, and two more sentences of the same claim, are corrected.
- **Red, at `7fb2158`**, against a scaffold with no behaviour (`partition` ignoring `may_ask`, a `Plan` floor nothing fills, the counter): `a_request_that_may_not_ask_a_model_draws_nothing_to_plan`, *62 projections drawn to plan for the rule*; `a_request_draws_the_rules_floor_once`, *the baseline drew 6026; the floor is 3001*; `a_model_arm_over_the_excerpts_a_projection_holds_is_refused`, *published a model arm of 0 excerpts*, since `render` did not yet draw on the floor handed to it. The others passed there, and their red is their mutant: N1, NH, N7 and N3, and N4 after `6c894fd`.

### The third fix round

A verifier read `f167349`, after the second fix round. No blocking finding; four notes: three closed here with tests only (`f81c931`), and the fourth, G2, recorded as the survivor VG2 below. No line of code changed. Its own verifier then found three notes, closed in the commit after `53884d3`: the excerpt boundary test now asserts its premise, that one record more is over on excerpts and not on bytes (R1 still dies, at the refusal and not the premise), and two sentences of this section.

- **The draw counter had no positive control.** Every reading of it was `drawn == 0` or `drawn <= bound`, so a counter that never counts passed every test, whole workspace included: D1 deletes the increment, and D2 moves it into `floor`'s loop so that only the floor counts. **Now** `drawing` resets the counter, draws one projection and requires it to read exactly one before every reading it gives, and `the_draw_counter_counts_exactly_the_projections_drawn` holds each path that draws to a count worked out from the units of a cargo log of three binaries: one for `draw`; one for each failure and identity unit for `floor`; one for `render`'s rule arm on a plan that carries its floor; one for each unit the model arm adds, and one more; and one for bytes that are not text.
- **R1: the refusal at its bound.** `a_model_arm_of_exactly_the_excerpts_a_projection_holds_is_published_and_one_more_is_not` hands `render` a floor of 24 separate records, which it publishes with 24 excerpts, and one of 25, which it refuses. The test that hands thirty did not tell a bound one too loose.
- **CE4: a one-byte message.** `a_compiler_error_whose_message_is_one_byte_is_named_by_it` requires an error whose `message.message` is `x` to be named `x`, not by what it renders.
- **G2 is a survivor, recorded as VG2.** The call site passing `self.model.is_some()` rather than `serving.is_some()` ignores an investigation of zero. Like CA-true, it is equivalent in what it publishes, and only the cost differs.
- **Red, at `f81c931`, is the mutants**, each applied, seen to fail, and reverted. D1 and D2 each fail the four tests that read the counter, at `drawing`'s check: *the counter, reset, did not count the one projection drawn*, `left: 0`. R1 fails the boundary test with *published a model arm of 25 excerpts*. CE4 fails the one-byte test, which names the error by its rendered line, `error[E0425]: cannot find value `x``. The verifier's other mutants of this round were run again there and are killed as the table says.

### The mutant table

**143 mutants at the head, 137 killed.** The second fix round's eighteen are at the end of the table: the verifier's N1–N13, which the verifier ran at `1bc8b76`, and NS, NR, NH, CA-true and CA-false; N1–N7 and the rows whose anchors the fix moved (X16, M4, RF1, C-parts, C-large, C-binary, S1, P1b) were run again at `6c894fd` against the whole `cbr-provider` binary, and CA-true and CA-false through a workspace build, `journey_two` and `j2_harness`. They are:
- the plan's M1–M21 and their variants, and the verifiers' X-series (from their definitions, with the anchors this round moved re-stated);
- the review's MUT1–MUT6;
- all 48 of m5a's and round 66's, among them G1–G8 and K2–K4, which the verifiers did not re-run;
- P1 with P1b;
- this round's own: C-, RF, J, I1 and S1;
- the third fix round's sixteen, the verifier's of `f167349`, at the very end. Their own names repeat rows above, so each is prefixed V here. All but VI1 were run again at `f81c931` against the whole `cbr-provider` binary, and VG2 also through a workspace build, `journey_two`, `j2_harness` and `expand`; VG2's whole-workspace run is the verifier's, at `f167349`.

**How each was run.** It was applied in a scratch worktree, with `git diff` checked to show only its own files changed. Then the workspace was built, and the whole `cbr-provider` binary, `journey_two` and `j2_harness` were run. Finally the file was restored.

**The survivors were then run against the whole workspace**, each through `wslot`. A kill counts only when it names a genuine test, one listed at the unmutated head. That discounts fixture lines echoed in assertion messages, and the timing tests named in VERIFICATION, which failed under load in some runs: `keychain::tests` with `TimedOut`, and once `idle_connections_keep_at_least_half_of_command_throughput`.

**Where the runs happened.** M1–M9 ran at `e01016b`, M10a–A2 at `e32d57b`, and the rest at `2bd43e8`. After the three new tests, BASE, M4, C2b, J3 and every survivor were run again at `9654d4b`. The only differences between those heads are tests.

**MUT2 is killed now**, by `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`: the preamble's room is measured at the widest drawing, so it is where that drawing's `over_projection` shows.

| Mutant | What it does | Result | Killed by |
|---|---|---|---|
| M1 | the model's picks carried before the floor | killed | `a_model_arm_that_does_not_fit_is_refused_and_never_published`, `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` and 1 more |
| M2 | the model's choice replaces the floor (m5a's behaviour) | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it`, `a_failure_the_model_did_not_choose_is_still_carried` and 7 more |
| M3 | the floor repacked under the model's header | killed | `a_model_arm_that_does_not_fit_is_refused_and_never_published`, `the_model_arm_carries_every_byte_the_rule_carries_whatever_the_model_answers` |
| M5 | every unit the floor leaves out offered, fitting or not | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_projection_the_investigation_limit_cannot_cover_is_not_started` and 4 more |
| M6 | an addition priced as a separate excerpt, not drawn | killed | `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` |
| M7 | an empty offer still asks | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_run_that_offers_nothing_asks_nothing_live_or_dry` and 5 more |
| M8 | the model's label on the no-call path | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_run_that_offers_nothing_asks_nothing_live_or_dry` and 5 more |
| M9 | the model's answer ignored (K10 again) | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it`, `what_the_model_added_is_carried_beside_everything_the_rule_carries` |
| M10a | a chosen id resolved to its part-local index (K8 again) | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it`, `what_the_model_added_is_carried_beside_everything_the_rule_carries` |
| M10b | a chosen id resolved against the planned part, not the offered one | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it` |
| M11 | runs not cut where the chooser changes | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_run_that_offers_nothing_asks_nothing_live_or_dry` and 3 more |
| M12 | the model arm's choice does not hold the failures | killed | `a_failure_the_parser_named_is_never_not_selected_in_either_arm` |
| M13a | the investigation claim counts planned parts | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started` |
| M13b | the header counts planned parts | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_projection_the_investigation_limit_cannot_cover_is_not_started` and 5 more |
| M14 | capacity decided on the questions asked | killed | `an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity`, `an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked` and 2 more |
| M15 | the preamble unbounded | killed | `a_parts_worst_call_is_computed_from_its_real_body_and_fits_one_request`, `a_whole_projection_cannot_exhaust_a_job_even_beside_discovery` and 1 more |
| M16 | `FORMAT` left at `/1` | killed | `the_projection_a_fixed_fixture_renders_has_not_changed_without_its_format` |
| M17 | F3 reverted: a cargo error runs on to its blank line | killed | `a_cargo_error_does_not_swallow_the_run_that_follows_it` |
| M18 | F1 merged: a cargo error counted as a failing test | killed | `the_header_counts_failing_tests_and_cargo_errors_apart`, `the_offer_preamble_is_bounded` |
| M19 | F5 reverted: an empty root list is a listing | killed | `an_empty_list_at_a_documents_root_is_identity` |
| M20 | the harness's floor check removed | killed | `the_harness_names_a_model_arm_that_drops_a_baseline_excerpt` |
| M21a | the harness asks a zero-part run with no investigation | killed | `a_run_that_offers_nothing_asks_nothing_live_or_dry` |
| M21b | the harness makes no assisted request at zero parts | killed | `a_run_that_offers_nothing_asks_nothing_live_or_dry` |
| M21d | the harness replays a zero-part run | killed | `a_run_that_offers_nothing_asks_nothing_live_or_dry` |
| X1 | the preamble's excerpt room claimed whole | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| X2 | the preamble's byte room claimed whole | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| X3 | the widest frame without the longer omission reason | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`, `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` and 1 more |
| X4 | the widest frame without the `unresolved:` lines | killed | `a_floor_with_no_room_for_the_lines_a_cut_group_takes_is_not_put_to_a_model` |
| X5 | the widest label listing nine ids | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`, `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` and 1 more |
| X6 | the widest label's arm removed | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`, `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` and 1 more |
| X7 | no chooser kept in the model arm | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it`, `the_header_names_exactly_the_excerpts_the_model_added` and 2 more |
| X8 | `unresolved:` for every planned cut | killed | `a_group_larger_than_a_part_is_the_one_cut_and_the_model_is_told_it_was_made`, `a_group_offered_in_one_question_is_not_reported_as_cut` |
| X8b | a group offered in one question reported as cut (the review's MUT3) | killed | `a_group_offered_in_one_question_is_not_reported_as_cut` |
| X9 | the preamble's cargo-error count lost | killed | `the_offer_preamble_is_bounded` |
| X10 | a question carries an empty floor | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| X13 | the harness flags only an excerpt whose first byte is dropped | killed | `the_harness_names_a_model_arm_that_drops_a_baseline_excerpt` |
| X16 | `offer` without its everything-fits return | killed | `a_whole_artifact_that_fits_is_carried_whole_and_nothing_is_asked` |
| X18 | `render`'s add loop without its `Other` filter | killed | `a_failure_the_floor_left_out_is_never_credited_to_the_model` |
| X19 | the header's cargo-error count fixed at 0 | killed | `the_header_counts_failing_tests_and_cargo_errors_apart` |
| MUT1 | the offer measured under the real label, not the widest | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves`, `a_floor_with_no_room_for_the_lines_a_cut_group_takes_is_not_put_to_a_model` and 1 more |
| MUT2 | the widest drawing no longer forces `over_projection` | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| MUT3 | as X8b | killed | `a_group_offered_in_one_question_is_not_reported_as_cut` |
| MUT4 | the floor repacked greedily under the model arm's header | killed | `a_model_arm_that_does_not_fit_is_refused_and_never_published`, `the_model_arm_carries_every_byte_the_rule_carries_whatever_the_model_answers` |
| MUT5 | a question numbered among planned parts | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started` |
| MUT6 | the preamble's room claimed whole | killed | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| C1a | ledger declares no omission (no [o] line, none recorded) | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it`, `a_failure_the_model_did_not_choose_is_still_carried` and 24 more |
| C1b | call site records no omission | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it`, `a_failure_the_model_did_not_choose_is_still_carried` and 18 more |
| C2 | no insufficient-capacity outcome at all (parts bound and size check) | killed | `an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity`, `an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked` and 2 more |
| C2a | parts bound removed | killed | `an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity`, `an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked` and 2 more |
| C2c | too_large refuses the bound itself | killed | `more_parts_than_the_bound_is_insufficient_capacity_with_or_without_a_model` |
| V1 | excerpt restated upper-cased | killed | `a_capture_anchor_stays_on_one_line_whatever_it_holds`, `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it` and 27 more |
| V2 | excerpt's stated range off by one | killed | `a_capture_anchor_stays_on_one_line_whatever_it_holds`, `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it` and 29 more |
| B1 | offered questions not claimed together (room for one is enough) | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started` |
| B2 | a failed part skipped | killed | `a_part_that_answers_outside_its_offer_leaves_the_item_unmet_with_that_reason`, `a_rebuild_honours_every_artifact_a_record_is_sealed_under` |
| B3 | an id outside its part ignored on the way out | killed | `a_rebuild_refuses_a_record_that_chose_outside_its_own_part` |
| P1 | identity cut into parts (old site: cut's filter) | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started`, `capacity_is_the_inputs_not_the_offers` and 2 more |
| P2 | a part's bytes counted raw | killed | `a_part_holds_at_most_its_bytes_as_the_body_carries_them` |
| P3 | a part's count of candidates unbounded | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started`, `a_part_holds_at_most_its_count_of_candidates` and 1 more |
| P4 | every part's selector the same | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it`, `a_failure_the_model_did_not_choose_is_still_carried` and 10 more |
| R1 | section's bytes unbounded | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it`, `a_failure_the_model_did_not_choose_is_still_carried` and 35 more |
| R2 | excerpts unbounded | killed | `a_run_that_offers_nothing_asks_nothing_live_or_dry`, `a_failure_the_floor_left_out_is_never_credited_to_the_model` and 2 more |
| R3 | an omission holding a chosen unit called not_selected | killed | `a_failure_the_parser_named_is_never_not_selected_in_either_arm`, `an_omission_that_held_something_chosen_is_over_projection_and_one_that_held_nothing_is_not` and 1 more |
| R4 | run identity carried before the failures (now in floor()) | killed | `a_failure_is_carried_even_where_run_identity_alone_would_fill_the_projection`, `a_failure_the_floor_left_out_is_never_credited_to_the_model` and 2 more |
| R5 | what fits whole carried unit by unit | killed | `a_whole_artifact_is_carried_as_one_excerpt_however_many_units_it_has` |
| R6 | nothing offered still asks the model | killed | `a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing`, `a_run_that_offers_nothing_asks_nothing_live_or_dry` and 5 more |
| F1 | JSON level error not a failure | killed | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read` |
| F2 | libtest failure block not a failure | killed | `a_block_longer_than_a_unit_is_cut_on_lines_and_a_line_longer_than_a_unit_on_characters`, `a_failure_is_carried_even_where_run_identity_alone_would_fill_the_projection` and 12 more |
| K1 | Next::Insufficient carried by the rule | killed | `an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity`, `an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked` |
| K7 | a part's record names DISCOVERY_ITEM | killed | `a_large_test_log_is_projected_and_every_byte_is_carried_or_declared_omitted` |
| K8 | a chosen id resolved to its part-local index | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it`, `what_the_model_added_is_carried_beside_everything_the_rule_carries` |
| K10 | answer ignored, rule's choice carried under the model's label | killed | `an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it`, `what_the_model_added_is_carried_beside_everything_the_rule_carries` |
| A1 | anchor newline raw | killed | `a_capture_anchor_stays_on_one_line_whatever_it_holds`, `a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| A2 | anchor backslash not escaped | killed | `a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| A3 | U+2028/U+2029 not escaped | killed | `a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| A4 | anchor cut inside an escape | killed | `a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| H1 | harness admits an input carrying a machine path | killed | `the_harness_refuses_an_input_that_carries_a_machine_path_before_anything_starts` |
| H2 | harness forgets the temporary roots | killed | `the_harness_refuses_an_input_that_carries_a_machine_path_before_anything_starts` |
| H3 | harness does not check excerpts against the source | killed | `the_harness_names_every_way_a_projection_can_fail_its_checks` |
| H4 | harness does not check the packet's omissions | killed | `the_harness_names_every_way_a_projection_can_fail_its_checks` |
| H5 | harness admits a ceiling above the hard cap | killed | `the_harness_refuses_a_ceiling_above_the_hard_cap_and_admits_the_cap_itself` |
| H6 | harness refuses the hard cap itself | killed | `the_harness_refuses_a_ceiling_above_the_hard_cap_and_admits_the_cap_itself` |
| G1 | every named artifact readable at compile | killed | `evidence_the_submitter_cannot_read_is_unavailable_exactly_as_evidence_that_does_not_exist` |
| G2 | the command's readable evidence ignores the grant | killed | `evidence_the_submitter_cannot_read_is_unavailable_exactly_as_evidence_that_does_not_exist` |
| G3 | a record sealed under no evidence | killed | `a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from`, `a_parts_record_is_sealed_under_the_artifact_it_showed_and_no_other` |
| G4 | a record sealed under the job's whole evidence | killed | `a_parts_record_is_sealed_under_the_artifact_it_showed_and_no_other` |
| G5 | `covers` ignores the evidence half | killed | `a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from`, `a_rebuild_honours_every_artifact_a_record_is_sealed_under` and 1 more |
| G6 | the doors treat every listed artifact as readable | killed | `a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from` |
| G7 | the rebuild ignores the evidence half | killed | `a_rebuild_honours_every_artifact_a_record_is_sealed_under` |
| G8 | a job joined under different readable evidence | killed | `context::tests::a_job_is_joined_only_by_a_request_with_the_same_view` |
| K2 | stored bytes not checked against the digest | killed | `stored_bytes_that_no_longer_match_their_digest_are_unavailable_and_never_projected` |
| K3 | an excerpt's `\r\n` normalised | killed | `a_log_with_crlf_line_endings_is_read_as_cargos_and_carried_as_its_own_bytes` |
| K4 | the purge check removed | killed | `a_purged_artifact_is_unavailable_though_its_bytes_are_still_stored` |
| C-parts | `partition`'s early return without the parts bound | killed | `capacity_is_the_inputs_not_the_offers` |
| C-large | `partition`'s early return without `too_large` | killed | `capacity_is_the_inputs_not_the_offers` |
| C-next-large | `next` without `too_large` | killed | `capacity_is_the_inputs_not_the_offers` |
| RF1 | `render` publishes a model arm that does not fit | killed | `a_model_arm_that_does_not_fit_is_refused_and_never_published` |
| J1 | a compiler error in cargo's JSON not named | killed | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read`, `the_header_counts_failing_tests_and_cargo_errors_apart` |
| J2 | a compiler error in cargo's JSON counted as a test | killed | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read`, `the_header_counts_failing_tests_and_cargo_errors_apart` |
| J4 | no name from `reason` | killed | `the_header_counts_failing_tests_and_cargo_errors_apart` |
| I1 | the instruction's old words | killed | `a_parts_worst_call_is_computed_from_its_real_body_and_fits_one_request`, `a_whole_projection_cannot_exhaust_a_job_even_beside_discovery` and 1 more |
| M4 | `offer` without its check that the floor fits at the widest | killed | `no_question_is_asked_about_a_floor_over_the_bound_though_an_addition_would_shrink_it` |
| C2b | call site's size check removed | killed | `an_input_over_the_projections_capacity_is_insufficient_capacity_and_never_a_partial_summary` |
| J3 | a compiler error named by what it renders before its message | killed | `the_header_counts_failing_tests_and_cargo_errors_apart` |
| P1b | identity offered to the model (new site: offer's filter admits any non-failure outside the floor) | survived targeted; whole workspace: survived | — |
| P1+P1b | identity admitted at both of its sites, `cut` and `offer` | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started`, `capacity_is_the_inputs_not_the_offers` and 3 more |
| C-binary | `partition`'s early return without the not-text check | killed (it survived until the second fix round) | `nothing_is_drawn_to_plan_an_input_over_capacity` |
| RF2 | the call site drops a refusal's `insufficient_capacity` | survived targeted; whole workspace: survived | — |
| S1 | the room's subtraction not saturating | survived targeted; whole workspace: survived | — |
| M21c | the harness's 'needed no question and made a call' check removed | survived targeted; whole workspace: survived | — |
| N1 | the offer worked out for every text input, then thrown away over capacity | killed | `nothing_is_drawn_to_plan_an_input_over_capacity`, `a_request_that_may_not_ask_a_model_draws_nothing_to_plan`, `a_request_draws_the_rules_floor_once` |
| N2 | `partition`'s parts bound one part looser | killed (verifier) | `capacity_is_the_inputs_not_the_offers` |
| N3 | `partition` refuses an input of exactly `INPUT_BYTES` | killed | `an_input_of_exactly_the_input_bound_is_within_capacity_and_asked` |
| N4 | `next` decides not-text before capacity | killed | `bytes_over_the_input_bound_are_insufficient_before_they_are_not_text` |
| N5 | `render`'s refusal on bytes only | killed | `a_model_arm_over_the_excerpts_a_projection_holds_is_refused` |
| N6 | `render`'s refusal on excerpts only | killed | `a_model_arm_that_does_not_fit_is_refused_and_never_published` |
| N7 | an empty string names a compiler error | killed | `a_compiler_error_whose_message_is_empty_is_named_by_what_it_renders` |
| N8 | the compiler-message gate removed | killed (verifier) | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read` |
| N9 | a compiler error's name keeps its quotes | killed (verifier) | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read`, `the_header_counts_failing_tests_and_cargo_errors_apart` |
| N10 | the preamble's bytes and excerpts swapped | killed (verifier) | `the_offer_preamble_is_bounded`, `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| N11 | the preamble's tests and errors swapped | killed (verifier) | `the_offer_preamble_is_bounded`, `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| N12 | the room measured under the rule's frame, not the widest | killed (verifier) | `a_parts_question_says_what_the_floor_carries_and_the_room_it_leaves` |
| N13 | each candidate left carried, additions cumulative | killed (verifier) | `only_units_the_rule_does_not_carry_and_that_could_fit_are_offered` and 8 more |
| NS | `partition` works the offer out whoever may be asked | killed | `a_request_that_may_not_ask_a_model_draws_nothing_to_plan` |
| NR | `render` draws the floor again though the plan carries one | killed | `a_request_draws_the_rules_floor_once`, `a_model_arm_over_the_excerpts_a_projection_holds_is_refused` |
| NH | a plan with no floor prints no questions | killed | `both_arms_print_the_same_part_count_whether_or_not_a_model_may_be_asked` |
| CA-false | the call site never lets a model be asked | killed | `a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it` and 11 more in `journey_two` |
| CA-true | the call site plans as if a model may always be asked | survived targeted; whole workspace: survived (726 passed, 0 failed, 1 ignored over 42 lines) | — |
| VG1 | `partition` works the offer out whoever may be asked (its `may_ask` gate removed) | killed | `a_request_that_may_not_ask_a_model_draws_nothing_to_plan` |
| VG2 | the call site plans on whether a model is configured, ignoring an investigation of zero | survived targeted; whole workspace: survived (the verifier's, 726 passed, 0 failed, 1 ignored over 42 lines) | — |
| VG3 | `next` asks a plan with questions though nobody may be asked | killed | `with_no_model_the_rule_carries_the_failures_and_then_the_runs_identity` |
| VG4 | a plan with no floor counts the planned parts, not the questions | killed | `both_arms_print_the_same_part_count_whether_or_not_a_model_may_be_asked`, and at the verifier's run three in `journey_two` and `j2_harness` |
| VG5 | the baseline draws the floor again, ignoring the one worked out | killed | `a_request_draws_the_rules_floor_once` |
| VG6 | `render` ignores the floor the plan carries and draws it again | killed | `a_model_arm_over_the_excerpts_a_projection_holds_is_refused`, `a_request_draws_the_rules_floor_once` and 2 more |
| VD1 | the draw counter never counts | killed (it survived until the third fix round) | `the_draw_counter_counts_exactly_the_projections_drawn`, `nothing_is_drawn_to_plan_an_input_over_capacity` and 2 more |
| VD2 | the draw counter counts only the floor's drawings | killed (it survived until the third fix round) | `the_draw_counter_counts_exactly_the_projections_drawn`, `nothing_is_drawn_to_plan_an_input_over_capacity` and 2 more |
| VR1 | `render`'s refusal on excerpts one too loose | killed (it survived until the third fix round) | `a_model_arm_of_exactly_the_excerpts_a_projection_holds_is_published_and_one_more_is_not` |
| VR2 | `Rendered::fits` on excerpts one too tight | killed | `a_projection_carries_at_most_its_count_of_excerpts`, `the_longest_model_header_is_computed_and_a_fixture_reaches_it` and 1 more |
| VCE1 | a compiler error named by what it renders before its message | killed | `the_header_counts_failing_tests_and_cargo_errors_apart`, `a_compiler_error_whose_message_is_one_byte_is_named_by_it` |
| VCE2 | no name from `reason` | killed | `a_compiler_error_whose_message_is_empty_is_named_by_what_it_renders`, `the_header_counts_failing_tests_and_cargo_errors_apart` |
| VCE3 | a compiler error named by `reason` before its message | killed | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read`, `the_header_counts_failing_tests_and_cargo_errors_apart` and 2 more |
| VCE4 | a one-byte message names nothing | killed (it survived until the third fix round) | `a_compiler_error_whose_message_is_one_byte_is_named_by_it` |
| VCE5 | what a compiler error renders never consulted | killed | `a_compiler_error_whose_message_is_empty_is_named_by_what_it_renders`, `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read` |
| VI1 | the projection's omission id built by hand, not through `crate::ids` | killed (verifier) | `an_evidence_item_of_127_characters_declares_its_omission_inside_the_grammar` |

**The six survivors, kept, each with its reason:**

- **P1b** (`offer` admits identity) is **equivalent on its own**. `asked` is built from the planned parts, which `cut` has already cleared of run identity, so an identity unit `offer` marks addable is never offered. Its test, `run_identity_the_floor_left_out_is_never_offered_though_it_would_fit`, kills P1 and P1b applied together. The rule is kept at both sites.
- **RF2** (the call site drops a refusal's reason) is **unreachable**. `render` refuses only a model arm that does not fit, and `offer` asks only where the floor fits at the widest. RF1, the refusal itself, is killed.
- **S1** (the room's subtraction not saturating) is **equivalent**. The subtraction is reached only after `at_floor.fits()`, so it cannot go below zero. It is saturating so that a panic can never reach a worker there (follow-up 1).
- **M21c** (the harness's "needed no question and made a call" check removed) is **not testable here**. Reaching it needs a provider that calls where its own baseline offered nothing, which is M7 or R6, both killed at the provider. The zero-part run's tests assert `tokens == 0` and `part_records == 0` directly. M21d, the replay at zero parts, is killed by that run's new `replay` assertion.
- **CA-true** (the call site plans with `may_ask` always true) is **equivalent in what it publishes**: `next` still carries the rule where nobody may be asked, and the offer only costs. Its difference is the drawing, which the unit tests hold on `partition` (NS is killed); the provider binary has no counter to read.
- **VG2** (the call site plans on `self.model.is_some()`, not `serving.is_some()`, so an investigation of zero still works an offer out) is **equivalent in what it publishes**, beside CA-true: `next` is still given `serving.is_some()`, false, so it carries the rule, and only the offer's drawing costs. The provider binary has no counter to read, and `partition`'s own `may_ask` gate is held by VG1's test.

### Follow-ups, not fixed here

1. **A panic in a projection's compile leaves the request never settling.** Under R2, the room's subtraction overflowed in a debug build inside a worker thread. The provider stayed up, the request stayed in `preparing`, and `j2_harness` waited 32 minutes at 0% CPU until the verifier killed it. The subtraction is saturating now, and R2 is killed by its unit tests. But any panic on that path would still hang a dry or live run, and neither `cbr request` nor the harness has a timeout for it.
2. **The harness's stop is a whole projection per run, whatever the input's parts.** So the rerun's `--run-ceiling` for invocation 2 has to be 861,357, while its parts bound the spend to 738,306 ([READINESS §10](m5/READINESS.md#j2-live-rerun-after-m5a-3-inputs-estimate-and-ceiling)).
3. **Deferred to m5a-4, as planned.** F2b (`and 5 more`, which gate E pins). F4 (`MAX_EXCERPTS`, which gate D pins). And a deterministic pass that carries a whitespace gap wherever joining two excerpts shrinks the section.
4. **The rule's floor is quadratic in run identity, and that is `main`'s.** `floor` draws the whole projection once per failure or identity unit, and run identity is never cut into parts, so neither `MAX_PARTS` nor `PART_UNITS` bounds it; only `INPUT_BYTES` does. JSON identity of about two bytes a unit is the worst case: in a debug build the floor took about 0.5 s at 16,384 bytes (8,183 units), 5.7 to 6.1 s at 65,536 (32,759) and **18.1 to 19.0 s at 131,072 (65,527)**, measured by this round's scratch probe after its fix; the first round's 0.65 s for 8,700 lines of `running 1 test` was this cost, not a smaller one. `main` pays it once a request, in `render`; the first fix round paid it twice (22.2 s + 22.6 s at 131,072, the verifier's measure), and this round is back to once. Making the floor linear, by drawing it incrementally rather than whole for each unit, is a change of its own, not made here.

### Gates at the head

At `f81c931`, the third fix round's tests, for the static gates and the count; `6c894fd`, the last code commit, for the conformance suites and the survey, since the third fix round changed tests only. This docs commit changes only Markdown.

- **Static gates, all exit 0:** `check_docs.py`, `verify_pin.py`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo build --workspace --locked` and `git diff --check origin/main...HEAD`.
- **`Cargo.lock` is unchanged, and no machine path is committed** (`git grep`).
- **`cargo test --workspace --locked --no-fail-fast`, through `wslot`: 729 passed, 0 failed and 1 ignored (the stall harness), over 42 `test result` lines**, at `f81c931`, at a load average between 12 and 24. By crate: 20, 7, 40, 493 and 170. That is `main`'s 694 over 42 at `5ddc26f`, plus thirty-six: the builder's eighteen, the first fix round's seven, the second's eight and the third's three, all in `projection::tests` but four in `journey_two` and two in `j2_harness`. At `6c894fd` the same run gave 726 passed; right after the rebase, at `7ad9fae`, 718; and at `bd8791a`, the second fix, 725; each with 0 failed and 1 ignored over 42 lines. No timing test failed in any of the four, nor in CA-true's whole-workspace run.
- **The seven conformance suites**, run into `mktemp -d` with the worktree's `target/` a symlink for the length of the run and removed afterwards. Each matches its expectation: `stream` 24; `core` 130 and 5 unsupported; `socket` 11 and 2; `evidence` 16; `knowledge` 10; `context` 11; `composition` 3 and 11.
- **The dry-run survey**, of all nine inputs and both live manifests at `6c894fd`, into `/var/tmp/cbr-j2/g5a`, `g5b` and `g5c`. It is [READINESS §10](m5/READINESS.md#j2-live-rerun-after-m5a-3-inputs-estimate-and-ceiling)'s table, unchanged: every run exited 0 with no problem named; only the core manifest asks, 3 parts, 3 records and 15,000 fake tokens a run, and its replay rebuilt identical sections; red and green make no call, each assisted section is its baseline's byte for byte, and each was given 4 parts. **Every run's summary, the masked baseline digests among it, equals the first fix round's survey** (`f4a`–`f4c`), so neither the rebase nor this round changed a published byte: red `e7d38955…`, green `65666f5e…`, core `9a6f4588…`.

## Earlier — packet ids within the identifier grammar

On `compiler/ids`, off `main` at `2eedade` (`cbr expand`, #42). Commits: `48f5d39` tests first, `76612e7` the implementation and `cbr-context-compiler/4`, `be3b749` two tests a mutant run showed were missing, `256fc70` and `c84c0ad` the docs; then, after independent verification, `d8ff650` tests first, `7af5e8c` the guard and a comment, and these docs. **No model has been called.**

- **What was found, and how.** Verifying `cbr expand` found that a compiled packet's discovered-span section and citation ids were built from the repository path — `dc-span-crates/cbr-cli/tests/journey_two.rs-67989` — while `core/1`'s `identifier`, which the `context` schemas require of section and citation ids, is `^[A-Za-z0-9][A-Za-z0-9._:~-]{0,127}$`. So published packets carried schema-invalid ids, and the provider's own `context.expand` refused them (`check_packet_payload` → `identifier_of`): in an ordinary compiled packet only a citation of a file at a repository's root could be expanded, and a long path also passed 128 bytes. No fixture caught it because conformance fixtures script packets rather than compile them, and J1 fetched artifacts by id and never followed a citation. In the same area, a compiled **source** section's citation named provider `cbr` literally, so under any other `provider_id` it could not be expanded either (follow-ups 1 and 2 of the `cbr expand` section below).
- **The rule** ([VERIFICATION](../VERIFICATION.md#the-deterministic-packet-compiler-and-what-a-packet-is-compiled-from)). `crates/cbr-provider/src/ids.rs` builds every id a packet names. A path or anchor name is encoded — `[A-Za-z0-9._-]` kept, `/` as `:`, every other byte `~XX` — so a span is `span-<repository>:<path>-<start>` and an anchor `anchor-<repository>:<name>`; an id that is already an identifier is unchanged (`s-log.o1`, `c-log`, `d-claim-…`), and one past 128 bytes keeps its prefix, `~~`, 32 hex digits of a SHA-256 and the longest tail that fits. Ties in the drop order sort on `Discovered.key`, the id as `/3` spelled it. `compiler::steps` takes the provider id. The packet artifact id is `packet.<request>.<revision>` while it fits and a shortened id under `packet.` past that.
- **The guard.** `context::ids_outside_grammar` checks every identifier-typed path of the vendored `context.packet.inspect` result (a unit test walks the schema, through its references, against the list), the sealed body's sections and citations, the packet artifact id, and that no section or citation id repeats; `publish_one` runs it before any capture, peer send or seal. A failure is `TickError::IdOutsideGrammar`: nothing of the job's tick is committed, the provider logs the job, the request and the JSON pointers and never the ids, and the tick moves on to the next job rather than returning as `Protocol` would. On a shared job it holds back that job's other requests for the tick, since `publish_one`'s errors reach its caller through `?`.
- **`cbr-context-compiler/4`, and the golden.** `GOLDEN_PACKET_DIGEST` moved from `sha256:c54aa006ba26ec683a7dbd4590a2a34a35297ddce45d0f848dc34c2da0bf8346` to `sha256:52fc69c7af75d681ed522acf72df7813a9ebfb05c55a55b9047ecbbc2197568e`. **The substitution check:** on the new golden bytes, writing the sixteen span ids back (`d-span-app:src:alpha.rs-0` → `d-span-src/alpha.rs-0`, and `dc-` likewise: `:` back to `/` after the repository) and `/4` back to `/3` reproduces `sha256:c54aa006…8346` exactly, so only ids and the version changed and nothing was reordered. The anchor rule (`d-anchor-app:` → `d-anchor-app-`) had nothing to substitute: the golden fixture has no anchor section, so the anchor order is held by `anchors_keep_the_order_of_their_key_not_of_their_rendered_id` and a compiler unit test instead. The golden fixture runs as `cbr`, so the source-citation fix cannot move it. The script is kept in this session's scratch, outside the repository.
- **Red, at `48f5d39` against `2eedade`'s code.** J1, `j1_a_question_finds_its_own_answer_with_a_cited_packet`: *ids outside the identifier grammar*, first `("/sections/2/section_id", "d-span-crates/cbr-cli/tests/journey_one.rs-31752")`. In `expand.rs`: `every_citation_of_a_hostile_repository_expands_to_its_blob` on `d-span-größe.py-0`, `d-span-.hidden/zanzibar.md-0`, `d-span-notes with space/été résumé.md-0` and `d-anchor-app-größe_wert`; `a_compiled_source_citation_names_this_provider_and_expands_under_another_id` on the citation's provider, `["cbr"]` against `["cbr-elsewhere"]`; `the_same_path_at_the_same_byte_in_two_repositories_is_two_sections` on *a /section_id is repeated*, `d-span-ledger.md-0` twice; `anchors_keep_the_order_of_their_key_not_of_their_rendered_id` on `["d-anchor-app-zorbify_widget", "d-anchor-app.x-zorbify_widget"]`. In `crates/cbr-provider/tests/context.rs`: `a_packet_holding_an_id_outside_the_identifier_grammar_is_never_published` on *a packet with an id outside the grammar was published*; `a_request_id_of_127_characters_is_published_under_an_artifact_id_inside_the_grammar` on *the packet artifact id packet.rxxx….1 (136 bytes) is outside the grammar*. In `derivation_claims.rs`: `a_long_claims_omission_and_its_section_share_one_id_inside_the_grammar` on *the claim's id d-claim-drains-xxx… (136 bytes) is outside the grammar*. The two test parsers that read a path out of `d-span-<path>-<start>` (`journey_one.rs`'s spread test and `serving::discovered_paths`) now read the locator; they passed before and after. **Guards of new code, whose red is their mutants:** the five property and pair tests in `ids/tests.rs` (seeded xorshift, no new dependency), the three guard tests in `context.rs`, and two compiler unit tests. One test-first bug was fixed in `76612e7`: the long-request test did not negotiate `context.expand`; its red was the artifact-id assertion, reached before the expand.
- **Mutants**, each applied to `be3b749`'s tree, built, run against its named killers, and reverted; the runner and its logs are in this session's scratch. All killed but the one expected equivalent:

| Mutant | Killed by |
|---|---|
| M1 `/` kept by the encoding | `ids::tests` (four); `every_citation_of_a_hostile_repository_expands_to_its_blob` **only after `be3b749`** — before it, `bounded` shortened every nested path to a digest inside the grammar and the hostile test passed; it now requires `d-span-app:.hidden:zanzibar.md-0` |
| M2 `:` not escaped | `ids::tests` (three) |
| M3 `~` not escaped | `ids::tests` (three) |
| M4 bytes outside ASCII kept raw | `ids::tests` (two); the hostile repository test |
| M5 no length fallback | `ids::tests` (three); the 127-character request test |
| M6 plain truncation instead of a digest | `ids::tests` (three) |
| M7 a marker unshortened ids can produce (`Z9`) | `every_id_is_an_identifier_and_no_two_inputs_share_one`, the pairs test |
| M8 tail cut inside an escape | **survived the first pass**: a cut inside `~XX` still decodes, because hex digits are kept as themselves. Killed at `be3b749` by the property test and `a_tail_never_starts_inside_an_escape_and_prefers_a_component`, which now require the decoded tail to be a suffix of the decoded input |
| M9 repository dropped from the span id | property and pairs tests; `the_same_path_at_the_same_byte_in_two_repositories_is_two_sections` |
| M10 anchor separator back to `-` | the pairs test and the compiler order test; the anchor order test |
| M11 claim omission built apart from the claim section | `a_long_claims_omission_and_its_section_share_one_id_inside_the_grammar` |
| M12 `"cbr"` restored on source citations | `a_source_sections_citation_names_the_provider_it_is_compiled_by`; the `cbr-elsewhere` source test |
| M13 ties broken on the rendered id | the compiler order test; the anchor order test. The golden digest does not kill it: its fixture has no anchors, and span and claim keys order as their ids do |
| M14 guard removed | `a_packet_holding_an_id_outside_the_identifier_grammar_is_never_published` |
| M15 guard checks sections only | `the_guard_names_every_id_outside_the_grammar_by_where_it_is` |
| M16 guard skips the uniqueness check | `the_guard_refuses_one_section_id_or_one_citation_id_named_twice` |
| M17 guard runs after the seal | the guard provider test, by the object count |
| M18 guard failure routed through `Protocol` | the guard provider test: the valid job beside it never publishes |
| M19 packet artifact id fallback removed | the 127-character request test |
| M20 `COMPILER` not bumped | the golden digest |
| M21 the digest's domain tag changed | **survives, equivalent as expected**: every id digests under the same tag |

- **The verification round, at `c84c0ad`.** Two verifiers who did not write the change ran the gates, J1 and a hostile repository through `scripts/debug_launch.sh`, and their own mutants (X1–X18 below). Verdict: no blocking defect, one should-fix and six notes, each handled here.
  - **Should-fix: no compiled packet used an item id long enough to shorten.** Mutant A (the compiler's four `s-` and three `c-` sites built raw) survived every test; only a manual `cbr context … --want <127 i's>=source:deep.txt` showed the guard refusing the packet on every tick. Mutant B (the projection's omission id built raw) was never reached. **Now:** two `expand.rs` tests through `cbr`, a source item and an evidence item of 127 characters, each published with its section and citation shortened to one digest and its citation expanded to the file's or the ingested bytes; the evidence item's bytes are not text, so its projection declares `s-<item>.o1`, shortened and keeping `.o1`.
  - **A required id that is missing got through the guard.** A scripted section with no `section_id` (copied into the facts as `null`; the empty inclusion refused it, but at the wrong pointer) and a citation with no `citation_id` (published). **Now:** `ids_outside_grammar` reports an id the inspect schema requires of an object that is there when it is absent, `null` or not a string, and a `null` in a list of ids; `OPTIONAL_IDENTIFIERS` names the eleven it does not require, and `null` passes only there. The schema test reads each object's `required` and checks the list against it, and reads each `/body/` path as the facts' path it is copied from. `serving::packet_ids` records a missing required id as the empty string, so `assert_packet_ids` fails on it too.
  - **The digest width and the longest tail were claims.** X4 (32 hex digits cut to 8) and X8 (the tail a byte short) survived the whole workspace. **Now:** `ids/tests.rs` computes the digest and the room itself, from 128 bits rather than the constant: the property test checks every shortened id's digest, and every encoded rest's tail against `decode` — the first `:` in the room if there is one, else no longer tail that does not start inside an escape — and a pinned test spells out `s-~~<digest><92 i's>`, 128 bytes, `dc-`'s 91, and a `:` tail.
  - **The `/body/` guard paths were unpinned** (X3, all five removed, survived). The body is built from the same values as the facts today, so the two cannot differ; `the_guard_reads_the_sealed_body_where_it_differs_from_the_facts` breaks only the body, at pointers written out rather than taken from the list, so a body path dropped from the guard fails there.
  - **`publish_one`'s comment overclaimed.** A refused compiled packet does leave something: the objects of the sources and derivations sealed earlier in the tick, which no committed row names, so the start-time collection pass removes them, and each retrying tick writes them again. The comment and the provider test's object assertion now say so; that test is scripted, which is why its count holds.
  - **J1's `assert_eq!(expanded, citations.len())` restated its own loop.** It now requires both claim sections, accepted and rejected, to cite artifacts that were expanded and equal `cbr fetch`'s bytes; mutant J1, a discovered claim citing nothing, fails it.
  - **Cosmetic:** the escape test's reference is now `decode(span("app", path, 7))`, the real shape, not `span-app/<path>-7`.
  - **Recorded, not changed:** JOURNEYS' "29 distinct ids" counts section, citation, omission and packet ids; a count of every identifier value in those files also finds six `ingest.*` evidence ids, 35 in all, none outside the grammar. The verifier's clean whole-workspace run at `c84c0ad` gave 688 over 42 lines, 687 passed, 1 ignored, 0 failed. X5 (the tail's bytes-outside-the-grammar bound zeroed) and X18 (a constant artifact id at the guard's call site) survive as equivalent: every rest reaching `bounded` is already inside the grammar, and `packet_artifact` always is.
- **Red of this round, at `d8ff650` against `c84c0ad`'s code.** `the_guard_names_a_required_id_that_is_absent_or_null`: `/request/id` removed reported `[]`. `a_scripted_packet_missing_a_required_id_is_never_published`: *r-no-citation-id: a packet missing a required id was published*. The rest test code that was already right, so their red is their mutant, each applied, run, seen to fail and reverted, the provider rebuilt before any `cbr-cli` test:

| Mutant | Killed by |
|---|---|
| A: item `s-`/`c-` ids built raw (7 sites) | both 127-character `expand.rs` tests: *never published* / *never left preparing* |
| B: the projection's omission id built raw | `an_evidence_item_of_127_characters_declares_its_omission_inside_the_grammar` (*never left preparing*) |
| X3: the five `/body/` paths removed | `the_guard_reads_the_sealed_body_where_it_differs_from_the_facts`, `the_guard_names_a_required_id_that_is_absent_or_null` and `the_guard_covers_every_identifier_the_inspect_schema_types`; the verifier's X3r, only the three required body paths removed, dies to the first two |
| X4: `DIGEST_DIGITS` 32 → 8 | the property test (*the digest of d-~~68b55da1…*) and the pinned test |
| X8: the tail's lowest start + 1 | the property test (*not the longest tail*, 127 against 121) and the pinned test |
| J1: a discovered claim cites nothing | `j1_a_question_finds_its_own_answer_with_a_cited_packet`: *claim gix-decision cites []* |
| F1: `null` passes at a required id | the absent-or-null unit test; the scripted provider test |
| F2: an absent required id not reported | the same two |
| F3: an optional path dropped from `OPTIONAL_IDENTIFIERS` | the schema test, the absent-or-null test, the repeated-id test |
| F4: a required path (`/sections/*/section_id`) added to it | the schema test; the absent-or-null test |

- **Open policy question.** A request whose packet the guard refuses stays `preparing`; its job is not saved, so every tick compiles it again and logs it again, past its deadline too. For a compiled request with an investigation budget that recompile can ask a model again, because the compile frees the job's answers. Only a script can reach the guard now, but whether such a job should end instead — and under which reason — is the owner's call.
- **Follow-ups, not fixed here.**
  1. `peer.rs` names its commands `publish.<artifact>.prepare`, `.append.<offset>` and `.seal`, which pass 128 bytes for a long packet artifact id.
  2. The `context.script` door does not validate ids; the guard now catches a bad one at publication.
  3. `packet.<request>.<revision>` cannot hold a long request id: to the protocol as an issue.
  4. Item `x`'s omission `s-x.o1` is the section id of an item named `x.o1`; J2's frozen rubric names omissions `s-<item>.o<n>`, so the fix is its own change (`the_recorded_collision_between_an_omission_and_an_item_named_like_one` pins it).
  5. `cbr`'s own command ids cap the ids it can name: `<request>.submit` refuses a request id past 121 characters and `claim.<claim>.propose` a claim id past 114, both valid in the protocol. The tests of a 127-character request and a 128-character claim speak raw frames for that reason.
  6. The `cbr expand` section's follow-up 3 (`--want x=claim:` writes `cbr`) stays open.
- **CI on Ubuntu found what the owner's machine could not.** At `faf6c2b` both Ubuntu runs failed one test, `anchors_keep_the_order_of_their_key_not_of_their_rendered_id`, at the second repository's `git commit`: the test ran `git` itself, with no author, and where macOS guesses one from the user and the host, Ubuntu's runners refuse. Reproduced here by removing every git identity and setting `user.useConfigOnly`; fixed by making the fixtures' own `serving::git`, which names an author, public and using it. **The whole workspace was then run under those same conditions, with no git identity at all: 693 passed, 0 failed, 1 ignored over 42 `test result` lines**, so no other test leans on the machine's identity.
- **Gates after the verification round**, at `7af5e8c`, whose code and tests are the head's: `check_docs.py`, `verify_pin.py`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo build --workspace --locked` and `git diff --check origin/main...HEAD` exit 0; `Cargo.lock` unchanged; no machine path. **`cargo test --workspace --locked --no-fail-fast` under the two-slot lock: 694 tests over 42 `test result` lines, 693 passed, 1 ignored, 0 failed**, at a load average of 10 to 12 — 20 in `cbr-encoding`, 7 in `cbr-identity`, 40 in `cbr-memory`, 463 in `cbr-provider`, 164 in `cbr-cli`: six more than `c84c0ad`: in `cbr-provider` two guard unit tests, one `ids` unit test and the scripted test in `tests/context.rs`; in `cbr-cli` two in `expand.rs`. No load-timing test failed in it. **The conformance suites were run again at `e20d79e` by the round's verifier**, because the guard now refuses a required id that is absent or `null` and 34 of the fixtures' scripted sections carry claim snapshots the provider builds itself: all seven matched their expectations — `stream` 24, `core` 130 and 5 unsupported, `socket` 11 and 2, `evidence` 16, `knowledge` 10, `context` 11, `composition` 3 and 11 — so no fixture's packet trips the widened guard. **A follow-up from that round:** the guard reports a required id of an object that is there, but not a required object a script leaves out altogether, such as a citation with no `evidence`; only the test-control script door can do that.
- **Gates at the first round.** The same static gates exit 0. **`cargo test --workspace --locked --no-fail-fast` at `be3b749`**, whose code is the head's, under the two-slot lock: **688 tests over 42 `test result` lines, 685 passed, 1 ignored, 2 failed** — `keychain::tests::the_credential_leaves_this_module_as_a_header_value_and_nothing_else` and `keychain::tests::the_secret_never_prints_itself`, both *the fake tool answered: TimedOut*, the keychain fake tool's 5-second timeout under load (follow-up 6 of the `cbr expand` section); run alone straight afterwards all 19 keychain tests passed, at a load average near 7. Again at `256fc70`, the docs commit, whose code and tests are the head's: 688 over 42 lines, **686 passed, 1 ignored, 1 failed** — `keychain::tests::the_secret_never_prints_itself`, the same *TimedOut*, at a load average of 28. Nothing this change touches is on their path. **The seven conformance suites**, against the provider built at `be3b749`, into a fresh temporary directory: `stream` 24, `core` 130 and 5 unsupported, `socket` 11 and 2, `evidence` 16, `knowledge` 10, `context` 11, `composition` 3 and 11, each matching its expectation — no scripted fixture's packet trips the guard.

## Earlier — `cbr expand`

On `cli/expand`, first off `main` at `041ad5f` and rebased onto `1e9df23`, J2's live record. **The SHAs below are the commits as they were verified, before the rebase**; the rebase changed no code or test line (`git range-diff`), and they are now `500568e`→`dd3c8f3`, `2a644cd`→`10d941c`, `e8b5c05`→`a047cd8`, `79cd87e`→`b816942`, `781404e`→`f0049da`, `5ea0626`→`1cafabc`, `4abb623`→`2e135de` and `e2fc870`→`8753b1a`. **No model has been called.** [RELEASE-SCOPE §2](readiness/RELEASE-SCOPE.md#2-in-scope)'s client list names "expand a citation", the provider has served `context.expand` since m3a, and the `cbr` verb did not exist. Found while planning J2 live's path proof: the packet cites the log as `c-log`, and the public client had no way to follow the citation.

- **What.** `cbr expand <request> <citation> [--revision N] [--offset O] [--length L] [--out FILE]`. The revision defaults to the last published one, found as `cbr packet` finds it. The read is `context::assemble`, which asks again from wherever a short answer ended; each answer is `context::answer`, which refuses evidence that changes between reads and an excerpt whose length is not its bytes; and the bytes go out through `context::deliver`, which checks a whole artifact against the cited digest before anything is written or printed. [VERIFICATION](../VERIFICATION.md#the-cbr-command-and-the-credentials-it-needs) has what it establishes and what it does not.
- **Tests.** `crates/cbr-cli/tests/expand.rs`, eight through `cbr` against the real provider: whole, ranges, past the end and a zero `--length` refused in the client's own words before anything is sent, one refusal for an unknown citation and two kinds of unreadable one, `--revision` over a scripted two-revision request, a 1.2 MB citation larger than one frame, a compiled citation expanded under a launch whose `provider_id` is not `cbr`, and a scripted citation naming another provider refused as an unknown one is, while the same artifact cited under this provider or none expands. Thirteen unit tests of the loop, the answer, the digest check and the delivery in `crates/cbr-cli/src/context.rs`. One provider test of the compile door below, in `crates/cbr-provider/tests/context.rs`. The fixture gains `start_configured` and `issue_grant_to`.
- **Red.** `cargo test -p cbr-cli --locked --test expand` at the first tests-first commit (`500568e`): all six failed, four on the usage line and two on `unknown option --offset` (`a_range_is_exactly_the_artifacts_bytes_at_that_range` and `an_offset_past_the_end_is_an_error_and_writes_nothing`). At the second (`79cd87e`), against `e8b5c05`'s code: the provider-id test failed because the citation named `cbr`, not `cbr-elsewhere`, and the compile-door test because the item naming another provider was `satisfied`. The zero-length assertion holds for correct code, so its red is its mutant: with the client's guard removed, the words on standard error are the provider's `invalid_envelope` at `/payload/max_bytes`. The unit tests of `deliver` and `answer` test code that was already right and was reached by no test, so their red is theirs too. Each of these mutants was applied, seen to fail its test, and reverted: no digest check in `deliver`; the check after the write and the print; evidence allowed to change between reads; the excerpt's length unchecked; every provider treated as this one at the compile door; the citation keeping the item's own provider (killed by both the provider test and the `cbr` one); and `cbr context` naming `cbr` again. At the third (`4abb623`), found by independent verification: `context.expand`'s refusal of a citation naming another provider (M19) was reached by no test and no fixture, since a compiled evidence section now cites the provider that compiled it, and a compiled source section names `cbr` (follow-up 2), which is this provider's id in every test that compiles a source section. The code is correct, so the red is the mutant: with that check disabled and the workspace rebuilt, `a_citation_naming_another_provider_is_refused_as_an_unknown_one_is` fails with `c-other was expanded`, and with it restored it passes.
- **The evidence item's provider, decided here.** `cbr context` wrote an evidence item as a reference to provider `cbr` literally, compiling copied it into the section's citation without looking at the provider, and `context.expand` refuses a citation that names another provider, so under any launch whose `provider_id` is not `cbr` — every conformance launch, or production configured with its own — a compiled packet's citation could not be expanded. **Client:** the item now names no provider, which is the provider being asked ([EVIDENCE §2](../../vendor/protocol/v0.1.0/docs/spec/profiles/EVIDENCE.md)). It cannot name the id, because on a fresh store, before anything exists, no answer carries it. Several do once something does: a claim reference, a packet's citation, a grant's `audience` through `core.grant.inspect`, and the `core.capabilities` subject after a capability has changed each name the `provider_id`. Before the first claim, packet, grant or change there is none of them, `core.describe`'s `provider.name` is the implementation's name (CBR says `cbr` whatever its `provider_id`), and `core.negotiate` carries none. **Provider:** EVIDENCE §2 supports the compile door, so it was added. A reference names the provider holding the artifact; two artifacts with equal digests keep separate provenance and authorization; and a reference a packet carries always names its provider. So an item naming another provider is `evidence_unavailable`, even when this store holds an artifact of the same id and digest, and one naming none is cited under this provider's id. A reference naming the configured evidence provider is refused the same way: compiling reads only this store, the peer holds packets rather than items, and reading a local artifact under the peer's name is the same mix-up. No fixture sets `context.compile`, so no fixture reaches the door, and all seven suites match their expectations (below).
- **Follow-ups, not fixed here.**
  1. **Done in [packet ids within the identifier grammar](#earlier--packet-ids-within-the-identifier-grammar).** **A discovered span's section and citation ids break the identifier grammar.** `crates/cbr-provider/src/compiler.rs` builds them as `d-{id}` and `dc-{id}` from an id that carries the repository path, as in `dc-span-crates/cbr-cli/tests/journey_two.rs-67989`, and a protocol identifier is `^[A-Za-z0-9][A-Za-z0-9._:~-]{0,127}$` ([`core/1/common.schema.json`](../../vendor/protocol/v0.1.0/schemas/core/1/common.schema.json)). So a published packet carries schema-invalid ids, and `context.expand` refuses them: only a citation whose path has no `/` can be expanded. It is the next pull request, and its gate is that J1's journey expands every citation.
  2. **Done in the same change.** A compiled **source** section's citation names `cbr` literally (`compiler.rs`, `steps`), while a discovered span names the `provider_id`, so under another id a compiled source citation cannot be expanded either.
  3. `cbr context --want <id>=claim:<claim>` still writes provider `cbr`: a claim reference must name its provider (KNOWLEDGE), and on a fresh store the client has no answer that says it, as above.
  4. A scripted section satisfies an `evidence_included` item by artifact and digest alone (`crates/cbr-provider/src/context.rs`, `check_passes`), ignoring the provider its citation names, so a script citing another provider's artifact of the same id and digest satisfies the item.
  5. **M16 survives**: `expand` passing a wrong offset to `deliver` is caught by no test. Documented, not fixed.
  6. **The keychain fake tool's 5 s timeout failed again under load**: four tests `TimedOut` in one of thirteen whole-workspace runs during verification, and in three of the four runs behind the gates line below, eight, three and two tests, at a load average near 20. One of those runs also had `idle_connections_keep_at_least_half_of_command_throughput` fall short. Both are timing under a loaded machine, and neither is touched by this change.
- **Gates at the rebased head `20fb4be`**, whose code is the verified `8753b1a`'s plus two comment edits: `check_docs.py`, `verify_pin.py`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo build --workspace --locked` and `git diff --check origin/main...HEAD` exit 0; `Cargo.lock` unchanged; no machine path. `cargo test --workspace --locked --no-fail-fast` gave **669 passed, 1 failed, 1 ignored over 42 `test result` lines**, under heavy load from other sessions' runs; the run's log records no load figure, and a load average of 28 to 30 was read beside it. The one failure is `session::tests::a_consumer_returning_within_the_notice_budget_receives_the_ending_notice`, *"still too slow: it missed the room deadline"*, a timing test in `session.rs`, which this branch does not touch. **Run on its own beside the same test at `main` (`1e9df23`): the branch failed 2 of 23 runs, both at a load average of 23 or more, and `main` 0 of 25, five of them at 39 or more.** Twenty runs of each were interleaved in pairs, in both orders; the other three of the branch's and five of `main`'s ran one after the other, `main`'s at the higher load. That difference is not significant (Fisher's exact test, p about 0.2), no line this branch changes is on that test's path, and it is recorded as a load-sensitive timing test beside the keychain one (follow-up 6), **not as a pass**; the logs are kept in this session's scratch, outside the repository. At `e2fc870`, before the rebase, the whole-workspace run gave 670 passed over 42 lines. The seven conformance suites were run at `5ea0626`, whose provider is unchanged since `781404e`, the last commit that changed it, and is the provider at this head: `stream` 24, `core` 130 and 5 unsupported, `socket` 11 and 2, `evidence` 16, `knowledge` 10, `context` 11, `composition` 3 and 11, each matching its expectation.

## Earlier — J2 live, run and recorded: failed

On `m5/j2-live-record`, off `main` at `041ad5f`. Docs and the run's record only; no code.

- **The rubric first.** `docs/verification/j2-live/` holds the rubric, its answer key and its three scripts, committed as `84061eb` at 15:39:31Z and pushed at 15:39:34Z, before invocation 1 started at 15:40:00Z. Three drafting agents with different lenses and a synthesiser wrote it; the answer key is computed from the inputs' bytes. Its cost thresholds are this session's.
- **The run.** Six runs in two invocations, as READINESS §10 set them: **120,067 tokens**, 16 calls, no repair ([what it spent](m5/READINESS.md#j2-live-what-it-spent-2026-09-25)).
- **The result: failed.** Every mechanical gate held on all twelve arms. On the failing test log both models' projections were worse than the deterministic rule's, because a model's choice replaces the rule's and both models left out failure blocks the parser had found; on the other two inputs both models chose nothing. Scored by script and by three blinded judges per pair, with no spread above one point on any score, and all six red preferences for the rule ([record](../verification/JOURNEYS.md#j2-live-2026-09-25-the-mechanism-held-and-the-model-made-the-projection-worse--failed)).
- **The path, proven.** `prove_path.py` read each store without writing it and relaunched a copy: every `model_calls` response names the configured model, every ledger equals the report, all 195 excerpts equal the bytes `cbr fetch` serves, and no model-provider key or Keychain secret is in any store.
- **The owner's decision of 2026-09-25: `orient_repository` and `record_rejected_approach` land in v0.1**, answered in the session in the owner's words, *"yes land orient_repository and record_rejected_approach for v0.1"*, which are in that session's transcript and not in this repository. They are m5i and m5j in [READINESS §9](m5/READINESS.md#9-the-pull-requests-each-with-its-gate). Their place in the order, after m5e and before m5f, is this session's proposal.

**Found while planning the path proof: `cbr expand` did not exist.** RELEASE-SCOPE §2 lists "expand a citation" among the client's operations, and `context.expand` has been served since m3a, but the `cbr` command had no verb for it. It is being added test first on `cli/expand`.

**Next.** m5a-3: the parser's failures become a floor a model's choice adds to and cannot remove, with labels that say exactly who chose what; then J2 live again, under the rubric with only mechanical changes, frozen before the rerun, with its estimate written into READINESS first. Then m5b.

## Earlier — J2's live run, prepared

On `m5/j2-live-prep`, first off `main` at `9cd388a` and rebased onto `5ab1a4f` after the socket fix merged. **No model has been called.** The reviewer accepted the prep, its three inputs that ask a model and the `/var/tmp/cbr-j2` location, and **the owner decided the run on 2026-09-25**: all six runs, the `f73415d` log first, as [READINESS §10](m5/READINESS.md#j2-live-the-inputs-the-estimate-the-stop-and-the-cap-decided-2026-09-25) now records. The reviewer's clearance of m5a asked for four things, and the record of each is [READINESS §10](m5/READINESS.md#j2-live-the-inputs-the-estimate-the-stop-and-the-cap-decided-2026-09-25):

- **CBR's own test output at the merged commit, from a location whose path names nothing.** Nine inputs: the suite's output, cargo's JSON messages, and the seven suites' manifests. Each passes `j2_run.py`'s machine-path refusal.
- **Each digest pinned** in a manifest kept outside this repository.
- **A dry run** of that manifest, with each input's parts and baseline.
- **The estimate, the stop and the cap**, written into READINESS with the models and the inputs by name and digest, and since decided by the owner.

**Three things found doing it:**

- **Moving the checkout was not enough.** Cargo's JSON messages still named the home directory, because every dependency's `manifest_path` and `src_path` point into the registry under `CARGO_HOME`. The inputs were produced again with `CARGO_HOME` and `TMPDIR` inside the neutral location, and the first attempt is kept in builder scratch as the record.
- **`j2_run.py`'s socket path can exceed the Unix limit.** It is `<out>/work/<run id>/s/cbr.sock`, and on macOS it must be under 104 bytes. The first survey dry run stopped with the provider refusing to bind. READINESS says the live `--out` must be short. A harness that chose its own short socket directory would remove the trap, which is a change for a code pull request.
- **The merged commit's suite is green, so no input names a failure.** J2 asks which tests failed. So CBR's own output at `f73415d`, m5a's tests-first commit, was proposed as an addition, and **the owner added it**: there eighteen tests fail, and the dry run named 21 failures, the eighteen and cargo's three error lines. It runs first, as invocation 1.

**Where the inputs were produced.** A clone from GitHub at `9cd388a`, in a sparse disk image kept on the SSD and mounted at `/var/tmp/cbr-j2`. Builder scratch stays on the SSD, and no path the output can name holds a user, a machine or a volume. `j2_run.py`'s temporary roots are `/var/folders`, `/private/tmp` and `/tmp`, whose paths are per user or per run; `/var/tmp/cbr-j2` is neither, and the reviewer may judge otherwise.

## Earlier — m5a, Journey 2

On `m5a/project-large-result`, off `main` at `dfa4f65`. [m5/READINESS.md](m5/READINESS.md) §3 and §9 are the scope: `project_large_result` on M4's direct calls, with J2's two negative controls, the projection's arithmetic, and J2's live harness in dry-run mode only. No model has been called since M4's live run 3, and nothing in this change calls one.

**Two records carried into m5a's first commit**, as the reviewer asked:

- **A known survivor from the CI-flake pull request.** The reviewer's mutant that sends `journey_one`'s provider standard error to `Stdio::null()` passes all eight of its tests. The assertion that the registered checkout path is never logged reads the captured file, and with nothing captured it holds by vacuity. It is recorded here as a survivor, not claimed equivalent; the fix is an assertion that the file holds something the provider is known to write, which belongs to a change that touches `journey_one`.
- **The socket-race issue** (`socket.subscription-recheck-race-regression`, diagnosed below under the CI flakes and paused by the owner) is **[#37](https://github.com/Combraton/cbr/issues/37)**. The owner posted it after this change's first commit, which recorded it as not yet posted. Nothing in m5a works on it, and the kept regression test is not run in builder scratch.

### What m5a is

**An `evidence_included` item's section is now a projection of the artifact it names** — J2's `project_large_result`, on M4's direct calls. The artifact was sealed whole by `cbr ingest` before anybody asked, and the section cites it, so a reader can always fetch the whole or any range of it through `context.expand`. The section holds, in a format `projection.rs` declares and a reader can check without it:

- **what was read, and how**: the artifact and its digest, the capture anchors it was sealed with, the format a parser recognised, its size, lines, units and parts, and who chose the excerpts;
- **the failures the document names**, each as its own bytes at a stated range, up to 16 and counted past that;
- **a ledger that tiles the artifact.** Every byte is inside an excerpt, which *is* the artifact's bytes at the range its frame states, or inside an omission with its reason: `not_selected`, `over_projection` or `not_text`. Each omission is also in the packet's own omission list, under the protocol's reason for it: `applicability`, `output_capacity` or `unavailable`.

**Deterministic reading where the format allows.** `projection::read` cuts cargo's test output by its runs, statuses, failure blocks and diagnostics; a JSON document or JSON lines by their structure, with paths built from the raw key bytes; any other text by lines. A failure is what the format says is one: `test … FAILED`, a `---- name stdout ----` block, a cargo `error`, or a JSON object whose `outcome`, `level` or `success` says so. Nothing is decoded or restated, so a value and its unit arrive as they were written.

**The model only chooses.** It is shown one part at a time, as a closed set of excerpt ids, and answers with some of them. Run identity is never offered, because it is carried whatever is chosen. A choice outside the part is `model_choice_not_offered`. With no model, or an investigation of zero, **the deterministic rule** chooses every failure the parser found. What fits whole is carried whole, and no call is made that cannot change the answer.

**Carried in this order, while the section fits:** chosen failures, then run identity, then the rest of what was chosen.

**A summariser is never given more than its capacity.** Candidates are partitioned into parts, and a group is cut across parts only when it is larger than a part. A cut group is declared on an `unresolved:` line, because each part saw only its own pieces. Beyond the capacity the item is `insufficient_capacity`, with no section, no omission and no call — never a partial summary.

**The parts are one flow**, as discovery's two steps are:

- they are claimed together against the investigation limit, so a limit one short starts none of them (`investigation_budget_exhausted`);
- each part is its own sealed record, keyed and replayed by m4d's rules;
- a part that fails ends the item with that part's reason, and never falls back to the rule.

### The bounds, each reached by a fixture

| Bound | Value | What reaches it |
|---|---:|---|
| `UNIT_BYTES`, one excerpt unit | 2,048 | a 120-line failure block, and one 6,144-byte line of three-byte characters |
| `BLOCK_LINES`, lines of plain text in a unit | 20 | 45 lines cut 20, 20, 5 |
| `PART_BYTES`, candidate text in one part, **escaped as the body carries it** | 32,768 | a log filling its first part; the same log written in control characters, which escape six bytes apiece, needing more than four times the parts |
| `PART_UNITS`, candidates in one part | 64 | 65 one-line records split 64 and 1 |
| `MAX_PARTS`, parts in one projection | 4 | a log filling exactly 4 parts is projected; one block more is `insufficient_capacity`, with a model and without. End to end at the call site since round 66: 122,400 bytes of plain text fill 4 parts and are projected, and 124,200, which pass the size check, need a fifth and are refused |
| `INPUT_BYTES`, the largest artifact read at all | 131,072 | `too_large` at the bound and one past it; end to end, a 1 MiB blob, which has no parts to count, is refused before it is read |
| `PROJECTION_BYTES`, the section's content | 16,384 | forty failures of 25 lines: what is left is less than one more block |
| `MAX_EXCERPTS`, carried excerpts | 24 | sixty failures between passing tests: exactly 24 carried, and the omissions at most 25 |
| `NAMED_FAILURES`, `NAME_BYTES` | 16, 160 | twenty failures listed as sixteen and "4 more"; a 486-byte name shown to 160 bytes at the range of what is shown |
| `CAPTURE_ANCHORS`, `ANCHOR_BYTES`, `LABEL_BYTES` | 8, 128, 96 | ten anchors of 138 bytes; since round 66, an anchor of 128 control characters, shown as 20 escapes and the marker; and a JSON key of 96 bytes |
| `JSON_DEPTH` | 64 | 64 levels read as JSON, 65 as lines |

**The arithmetic, computed from real bodies** in `projection::tests`, as `discovery::tests` computes M4's:

- a part at every bound costs **40,680**, under the 250,000 request ceiling;
- a whole projection, every part counted and repaired once, costs **488,160**;
- beside discovery's flow of 373,188 that is 861,348, under the 1,000,000 job ceiling, so a request whose limit covers both is never refused by its own job's ceiling.

The harness stops against the same figure: `WORST_CASE_PROJECTION_TOKENS` and `MAX_PARTS` in `scripts/j2_run.py` are read back by a Rust test. The largest header — every anchor, name and cut at its bound — takes under half the section.

### The mutant table

**Thirty-five mutants, each observed killed against the whole workspace**, run with `--no-fail-fast` in a scratch worktree at the implementation commit, so that every killer is named. Thirty-two died in the first pass. Three survived it, and a fourth died only to the golden digest; each of those four had nothing that stated its rule, and each died, to the test added for it, when run again at `f5aa513`.

**Round 66 added thirteen, each observed killed against the whole workspace at `97920b4`.** Seven are the reviewer's, which survived at `4eb13d4`. Six are mine, on what m5a-2 changed. Every build succeeded, every run gave its `test result` lines, and no keychain test failed in any of them. A1's run printed 42 lines rather than 40, because an assertion message that prints a projection includes the fixture log's own `test result:` lines; as below, the verdict rests on genuine test names.

**Two things about the instrument.** A mutant's run is judged on genuine test names only:

- the probe's pattern for a failed test also matches the fixture logs' own `test … FAILED` lines when an assertion message prints a projection, so those are discounted;
- two keychain tests failed in two of the runs (below), and are discounted too.

No verdict here rests on either kind.

| Mutant | Result | Killed by |
|---|---|---|
| **C1a — control 1**: the ledger declares no omission | killed | 14 tests: every projection read in `journey_two`, the harness dry run, and the unit tests that read a ledger |
| **C1b — control 1 at the packet**: the call site records no omission | killed | 10 tests, among them `a_large_test_log_is_projected_and_every_byte_is_carried_or_declared_omitted` and the harness dry run |
| **C2 — control 2**: no insufficient-capacity outcome at all | killed | `an_input_over_the_projections_capacity_is_insufficient_capacity_and_never_a_partial_summary`, the harness's oversize run, `more_parts_than_the_bound_is_insufficient_capacity_with_or_without_a_model` |
| C2a: the parts bound removed | killed | `more_parts_than_the_bound_is_insufficient_capacity_with_or_without_a_model` |
| C2b: the size check before reading removed | killed | the end-to-end oversize test, by its 1 MiB blob |
| C2c: `too_large` refuses the bound itself | killed | the same unit test |
| V1: an excerpt restated (upper-cased) | killed | 17 tests, each on the bytes |
| V2: an excerpt's stated range off by one | killed | 17 tests |
| B1: the parts not claimed together | killed | `a_projection_the_investigation_limit_cannot_cover_is_not_started` |
| B2: a failed part skipped | killed | `a_part_that_answers_outside_its_offer_leaves_the_item_unmet_with_that_reason` |
| **B3**: an id outside its part ignored on the way out | **survived the first pass**; killed at `f5aa513` | `a_rebuild_refuses_a_record_that_chose_outside_its_own_part` |
| P1: identity offered to the model | killed | `identity_is_never_offered_to_a_model`, `with_nothing_a_model_could_be_offered_the_rule_is_used_and_says_so` |
| P2: a part's bytes counted raw, not escaped | killed | `a_part_holds_at_most_its_bytes_as_the_body_carries_them` |
| P3: a part's count of candidates unbounded | killed | `a_part_holds_at_most_its_count_of_candidates` |
| P4: every part's selector the same | killed | 7 tests: one record per part, and the rebuild |
| R1: the section's bytes unbounded | killed | 13 tests |
| R2: the excerpts unbounded | killed | `a_projection_carries_at_most_its_count_of_excerpts` |
| R3: an omission holding a chosen unit called `not_selected` | killed | `an_omission_that_held_something_chosen_is_over_projection_and_one_that_held_nothing_is_not`, the golden digest |
| **R4**: run identity carried before the failures | killed in the first pass **by the golden digest alone**; at `f5aa513` also by a test of its rule | `a_failure_is_carried_even_where_run_identity_alone_would_fill_the_projection` |
| R5: what fits whole carried unit by unit | killed | `a_whole_artifact_is_carried_as_one_excerpt_however_many_units_it_has` |
| R6: nothing offerable still asks the model | killed | `with_nothing_a_model_could_be_offered_the_rule_is_used_and_says_so` |
| **F1**: a top-level `level: error` not a failure | **survived the first pass**; killed at `f5aa513` | `json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read`, with rustc's own diagnostics |
| F2: a libtest failure block not a failure | killed | 7 tests |
| G1: every named artifact readable at compile | killed | `evidence_the_submitter_cannot_read_is_unavailable_exactly_as_evidence_that_does_not_exist` |
| G2: the command's readable evidence ignores the grant | killed | the same |
| G3: a record sealed under no evidence | killed | `a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from`, `a_parts_record_is_sealed_under_the_artifact_it_showed_and_no_other` |
| G4: a record sealed under the job's whole evidence | killed | `a_parts_record_is_sealed_under_the_artifact_it_showed_and_no_other` |
| G5: `covers` ignores the evidence half | killed | the reader test above, and `derivation::tests::a_reader_missing_a_repository_a_claim_or_an_artifact_does_not` |
| G6: the doors treat every listed artifact as readable | killed | `a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from` |
| **G7**: the rebuild ignores the evidence half | **survived the first pass**; killed at `f5aa513` | `a_rebuild_honours_every_artifact_a_record_is_sealed_under` |
| G8: a job joined under different readable evidence | killed | `context::tests::a_job_is_joined_only_by_a_request_with_the_same_view` |
| H1: the harness admits an input carrying a machine path | killed | `the_harness_refuses_an_input_that_carries_a_machine_path_before_anything_starts` |
| H2: the harness forgets the temporary roots | killed | the same |
| H3: the harness does not check excerpts against the source | killed | `the_harness_names_every_way_a_projection_can_fail_its_checks` |
| H4: the harness does not check the packet's omissions | killed | the same |
| **K1 — the reviewer's, round 66**: at the call site, `Next::Insufficient` carried by the rule, a partial summary | **survived at `4eb13d4`**; killed at `97920b4` | `an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity`, and the harness's `an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked` |
| **K2**: the stored bytes not checked against the digest | **survived at `4eb13d4`**; killed | `stored_bytes_that_no_longer_match_their_digest_are_unavailable_and_never_projected` |
| **K3**: an excerpt's `\r\n` normalised to `\n` | **survived at `4eb13d4`**; killed | `a_log_with_crlf_line_endings_is_read_as_cargos_and_carried_as_its_own_bytes`, at the excerpt's closing line |
| **K4**: the purge check removed | **survived at `4eb13d4`**; killed | `a_purged_artifact_is_unavailable_though_its_bytes_are_still_stored` |
| **K7**: a part's record names no item | **survived at `4eb13d4`**; killed | `a_large_test_log_is_projected_and_every_byte_is_carried_or_declared_omitted`, at "part 0 names another item" |
| **K8**: a chosen id resolved to its part-local index | **survived at `4eb13d4`**; killed | `what_the_model_chose_is_what_is_carried` |
| **K10**: the answer ignored, the rule's choice carried under the model's label | **survived at `4eb13d4`**; killed | `what_the_model_chose_is_what_is_carried` |
| A1: an anchor's newline shown raw | killed | both anchor tests: `a_capture_anchor_stays_on_one_line_whatever_it_holds` and `projection::tests::a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| A2: an anchor's backslash not escaped | killed | `projection::tests::a_capture_anchor_is_shown_on_one_line_whatever_it_holds` |
| A3: U+2028 and U+2029 not escaped | killed | the same |
| A4: an anchor cut inside an escape | killed | the same |
| H5: the harness admits a ceiling above the hard cap | killed | `the_harness_refuses_a_ceiling_above_the_hard_cap_and_admits_the_cap_itself` |
| H6: the harness refuses the cap itself | killed | the same, at "the cap itself was refused" |

**The lesson again, four times.** Each of the four had a rule stated only in code: a second closed-set check, a clause of the failure vocabulary, a door's third half, and an ordering. m4d and round 50 taught that a rule tested at one door is untested at the next, and here the fix was the same: a test at the rule itself. B3 and G7 each looked like "unreachable" at first. Both were reachable with m4h's technique of rewriting a sealed record in a stopped store, so "this cannot be tested here" was, again, a claim to check.

### Found, not asked for

- **A third timing flake, in the keychain tests.** `keychain::tests::the_trailing_newline_is_stripped_and_nothing_else_is` and `the_secret_never_prints_itself` failed in the C2 and C2b runs with `the fake tool answered: TimedOut`: a fake `security` script, which prints one line, did not finish inside the 5-second timeout while the probe's whole-workspace runs loaded the machine. It did not happen in the baseline or in any gate run. **The reviewer saw it too, in 3 of 8 runs, while another session was also testing on the machine.** So it is load, not this change: a 5-second timeout on a fake tool is a timing bound, like the two CI flakes below. This is not diagnosed, and it is recorded for the CI-flake record rather than chased here.
- **An `evidence_included` item needed no read right before m5a**, which is item 1 above: the item's result told whether an artifact with that digest existed.

### Found while building, and fixed here

**1. An evidence item was a way to read an artifact without the right to.** Before m5a an `evidence_included` item needed no `evidence.read`: its section only named the artifact, and whether the item was satisfied said whether an artifact with that digest existed. A projection copies the bytes into the packet, and preparation runs on the provider's own authority, so this would have been a read around the grant. Three changes close it:

- the evidence a request may read is resolved **at the command, where the grant is**, as the view and the claims are;
- an artifact outside it is `evidence_unavailable`, exactly as one never sealed is, and `evidence_the_submitter_cannot_read_is_unavailable_exactly_as_evidence_that_does_not_exist` compares the two packets;
- a job joins another only under the same readable evidence.

**2. A record is sealed under the evidence its question showed.** A part's record identifies what it showed by range and by digest, and a digest of a short excerpt is something a reader can confirm a guess against. So `readable_under` gained `readable_evidence`, checked at fetch, inspect, the listing and the rebuild. The rebuild checks it against the evidence the rebuilding request may read, which is resolved at its command from the artifacts it names, so it is narrower than fetch's reader-wide check: conservative, and the reviewer's to widen. It holds the artifact the part was cut from. The first version sealed the job's whole set, and a test found that it refused a reader of the log a record that shows nothing else. A selection and a discovery step show no evidence and name none, and a record sealed before m5a has no list, so every existing record covers exactly as it did.

**3. Run identity crowded out the failures.** Cargo prints three identity lines per test binary, each its own excerpt, and eight binaries of them filled every excerpt before a failure's block was reached. Now failures come first, identity lines take the blank lines after them, `MAX_EXCERPTS` is 24, and the header carries the capture anchors, which identify the run however little of the log is carried.

**4. Two defects of my own, found reading the diff** and each tested red first:

- an input with nothing a model could be offered — all run identity, too large to carry whole — would have been labelled "chosen by the model" with no part asked;
- a projection that fits whole was carried unit by unit, and could pass through more excerpts than the bound on the way and omit units from something that fits.

### Decisions for the reviewer, settled at round 66

The reviewer accepted three decisions: the standalone projection artifact is m5c's, and is now in [READINESS's m5c row](m5/READINESS.md#9-the-pull-requests-each-with-its-gate) so it is not lost; `cbr-context-compiler/3` stays; and the rebuild's narrower evidence check, item 2 above, stays.

- **The projection is not sealed as a separate artifact.** It is the content of a section of the sealed packet, and every part's exchange is a sealed record. READINESS §5 says a family's product is sealed as evidence under a reserved source kind, and a standalone `cbr.artifact.projection` with its own readable-set gate is the natural reading of that. Nothing in m5a takes a projection by handle; m5c's tool turn is the first thing that will, so I would build it there. It is the reviewer's to overrule.
- **`cbr-context-compiler/3`.** An evidence item's section changed meaning, so the compiler string moved. The golden packet differs from `main`'s by that string and nothing else: substituting `/2` back reproduces the old digest exactly.
- **What a cross-part reference is here.** In a flow that only selects, the one thing partitioning cuts is a block larger than a part, and that is what the `unresolved:` line declares. A failure named in one part whose block is in another is linked by the parser, not by a model, so nothing is lost there.
- **The harness registers no repository.** The basis names CBR at the pinned commit, and the input is ingested with those anchors. With no registration, discovery has nothing to search, and every call a request makes is a part of its projection. The baseline request, with no investigation, says how many parts to ask for.
- **The J2 formats' failure vocabulary is the suite's own**: libtest's statuses, cargo's `level: error` and `success: false`, and the conformance runner's `outcome`. Anything else is read by structure or by lines.

### Red, then green

- **Red on `main`.** `journey_two.rs` (12 tests) and `j2_harness.rs` (6) were run against `main` at `dfa4f65`, in a scratch worktree with only the two test files added. All 18 fail, for their own reasons: the section is `evidence <artifact>`, the oversize input is `satisfied`, the unreadable artifact is `satisfied`, and the harness does not exist. They are committed on their own, at `f73415d`, before the implementation.
- **Guards whose red is their mutants.** The 33 unit tests in `projection/tests.rs` exercise a module that does not exist on `main`, so their red is their mutants. So are the tests added after the first red run: the oversize blob, the per-record evidence and the harness's checks against dishonest packets, each added because a planned mutant had nothing to kill it; and the four of `f5aa513`, each added because a mutant survived or died only to the golden digest, and each observed killing it.
- **The two defects of item 4** were each observed red before their fix.
- **Round 66.** The tests came first, at `40e4c74`, and were run against `4eb13d4`. Three were red, each for its own reason: the anchor test in `projection::tests` and its end-to-end twin, where the reviewer's anchor wrote a second `failures named:` and `read as` line, and the harness admitting a ceiling above the cap. The fix is `97920b4`. The guards for K1, K2, K3, K4, K7, K8 and K10 were green at `4eb13d4`, which is what a guard for correct code is. Their red is the reviewer's mutants, each observed killed in the table above.

### Round 66: the review, and m5a-2

**Not cleared at `4eb13d4`.** The safety step passed, and the gates and all seven suites reproduced. The reviewer's own mutants against the whole workspace found seven survivors, and two defects besides.

**The seven survivors, each now with a test that states its rule.** The killers are in the table above.

- **K1: the part bound at the call site.** `Next::Insufficient` could fall back to the rule. Every end-to-end oversize fixture was over a mebibyte and stopped at the size check, so the part bound was reached only in `projection::tests`, through `next()`. Now 124,200 bytes of plain text, which pass the size check and need a fifth part, end `insufficient_capacity` with a model and without, end to end and in a harness dry run. One block shorter, the same text fills exactly four parts and is projected, so it is the part count that refuses.
- **K2: the digest check.** An object altered on disk, one byte and the same length, is `evidence_unavailable`.
- **K3: `\r\n` normalised in an excerpt.** The J2 log with `\r\n` line endings is read as cargo's, its failures named and its excerpts checked against its bytes.
- **K4: the purge check.** Not equivalent, as the reviewer said. A purge keeps the row `sealed`, and an object that another sealed artifact names is never collected. The same bytes are ingested twice and one of them purged: the purged one is `evidence_unavailable`, and the other is projected.
- **K7: a part's record naming no item.** Each part record's `item` is now asserted.
- **K8 and K10: what the model chose not being what is carried.** The fake picks the second unit of every part, which is passing tests. The projection carries exactly those units besides run identity, declares every failure `not_selected`, and is not the rule's projection.

**The two defects:**

- **A capture anchor could forge header lines.** The reviewer sealed an artifact whose anchor id held `\nfailures named: 0\nread as …`, and the header showed those lines before CBR's own. Now each anchor's kind and id are escaped as JSON escapes a string, plus U+2028 and U+2029, and cut to `ANCHOR_BYTES` between escapes. The backslash is escaped too, so two anchors are never shown alike. The golden projection digest is unchanged, because its anchors have nothing to escape, so `cbr-project-large-result/1` stays; it has never been on `main`.
- **`j2_run.py` had no hard cap.** It now refuses a `--run-ceiling` above m4e's `RUN_CEILING_TOKENS`, imported from `m4e_run.py`, before anything starts, in a dry run as in a live one. The test reads the figure from m4e's harness, and admits the cap itself.

**Nothing else in the header can hold a newline**, which I checked rather than assumed:

- an artifact id is a protocol identifier;
- a digest is checked at seal;
- a libtest name lies inside one line;
- the JSON reader refuses a raw control character in a string, so a JSON label or failure name cannot hold one either.

A name or a label can hold a C1 control or a Unicode separator, which neither of CBR's readers splits on. Both stay the artifact's bytes, because a named failure is checked against the bytes at its range.

**A follow-up, recorded and not for m5a: CBR does not check `evidence/1`'s anchor schema at seal**: strings, a kind of 1 to 64 characters, an id of 1 to 256. The header's escaping makes an anchor harmless to a projection whatever it holds. Checking the schema is the protocol's rule and belongs at the evidence door, in a change of its own.

### Gates at the head

- `cargo test --workspace --locked`: **644 passed, 0 failed, 1 ignored, over 40 `test result` lines**, at `97920b4`, which the notes commit changes only in Markdown. That is 636 at `4eb13d4` and eight added in round 66: six in `journey_two`, one in `j2_harness` and one in `projection::tests`. Before m5a the count was 38 lines; `journey_two` and `j2_harness` are the two new binaries.
- **The seven conformance suites**, run into a scratch directory and never into `conformance/results/`: each matches its committed expectation.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo build --workspace --locked`: clean.
- `scripts/verify_pin.py`, `scripts/check_docs.py` and `git diff --check`: clean.
- `Cargo.lock`: unchanged.
- No machine path in `crates`, `scripts` or `docs` (`git grep`).

## Earlier — the CI flakes

On `ci/idle-lock-and-journey-wait`, off `main` at `c2917f7`. Both flakes the standing CI rule named were read against the workflow's whole history — 169 runs, and the earlier attempts of the two that were re-run.

**journey_one's 120 s wait: slow runners, not a stall in preparation.**

- Three jobs timed out, all on Azure `centralus` runners.
- Every journey_one run over 100 s was on `centralus` (7) or `westus3` (1); in every other region the maximum was 87 s.
- The same runners ran `evaluator_properties` — a property test with its own store and no provider, index or socket — at 5 to 12 times its median, and journey_one still passed on them at up to 428 s.
- Locally, under heavy CPU oversubscription, every wait ran past 120 s (122–134 s) and every one finished. Polling every 500 ms instead of 50 ms made no difference.

**The change:**

- The provider's standard error now goes to a file in the fixture's directory, where it used to go to a pipe nobody read until the end.
- A timeout's message carries that file, so a stall would explain itself.
- The wait is `PACKET_WAIT`, 600 s, derived in its doc comment from those measurements: about 30 s a wait, times the slowest slowdown measured, with room left.

Two observations stand behind it:

- with the wait forced to 0 s, the failure prints the provider's stderr;
- a provider that logs its registered checkout is still caught by the test that reads it, now from the file.

**The socket fixture's idle-poll race is fixed** ([#37](https://github.com/Combraton/cbr/issues/37)):

- The processing lock is FIFO. A socket session with a subscription or time-driven work may reserve a ticket while processing is busy, but never waits or emits the command-contention signal in its idle poll. A nonblocking per-session wake socket makes its reader consume the reservation as soon as the ticket reaches the front; simultaneous command input takes priority and consumes the same ticket. The 40 ms poll remains a fallback, not the progress bound.
- An authenticated, negotiated session with neither a subscription nor time-driven work returns from its idle poll without reserving. Beside eight such connections, one writer committed 7,542 commands in the same three-second window in which it committed 8,412 beside none (7,696 beside two), above the test's one-half floor.
- Request processing consumes any ticket the session already reserved, signals when it must wait, and then blocks. Idle maintenance and subscription delivery share one guard, and every subscription re-check still holds that guard across re-authorization and its event read, so no command commits between them.
- The regression test proves a no-work idle session neither reserves nor signals across several poll intervals, then requires a command on that session to signal contention. Bounded progress passed 200 serial repetitions and 200 four-way parallel rounds under sustained command contention, with the two-second CORE section 16.5 bound unchanged; separate cases kill a missing wake, try-lock-and-skip, a blocking idle poll and a guard split between re-authorization and event reading.
- With the fix, `socket.subscription-recheck-race-regression` passed 100 of 100 runs with the poll temporarily shortened to 1 ms and 100 of 100 at the stock 40 ms; before the fix, 76 of 100 and 0 of 100 failed respectively.

**#39 merged** as `5ab1a4f`, pinned to `7110db0`, and #37 is closed. The reviewer's mutants at `7110db0` left **three survivors, each bounded and none blocking**:

- **S1**: `arm_idle_wake` without its readiness re-check. The cost is at most one 40 ms fallback poll.
- **S2**: `cancel()` not waking the next idle ticket. The cost is at most one 40 ms stall.
- **S3**: the idle filter ignoring `idle_maintenance_owed`. Marking an item overdue then waits for the next request.

**The reviewer measured subscribed idle connections**, which the regression test's throughput figure does not cover: they perform the same as on the old lock. **A pre-existing cost, recorded for later:** with eight subscribed idle connections, a writer keeps only about 25% of its throughput, on the old lock and the new alike.

### Constraints for later heads, from the reviewer's round 59

- **m5e, what starts a gap loop.** Expressing a required fact as an item is **ruled out for the sealed pilot questions**: only the oracle holders could write that item, which is m3e's caveat again. The pilot requests stay exactly as sealed — task, selector, wants — and the trigger must be general, such as discovery's own shortfall, defined without reference to any question.
- **m5c, the existence-oracle test.** It uses **the same path text in two stores**: one where the path exists outside the grant, one where it does not exist. The tool result and the whole turn record — argument included — are then compared byte for byte.

## Earlier — M5 readiness, docs only

Against `main` at `b69f698`. [m5/READINESS.md](m5/READINESS.md) is M5's first pull request, written before any M5 code and reviewed before any, as M4's was. It covers what the reviewer's round 57 asked it to:

- Journey 2 first, with both of its negative controls unchanged.
- The loop, the tool surface and checkpoints, with MODEL-RUNTIME §6's eight tests and what is new for tools beside what M4's direct calls already exercise.
- Artifact creation, with the ancestry-root obligation in the same pull request as the first family whose output CBR's producer cites as claim support.
- J3, J4 and J5 with their controls.
- The composition fixtures: three reachable today, ten blocked on an execution peer, one on verification. The one route, the reference executor, is a check rather than an assumption.
- Every new bound computed by a test and reached by a fixture.
- Run 3's two limits, stated generally.
- A pull-request split with a gate for each, and every live call estimated and capped before it, on the owner's word.

**Nothing in it is sized that belongs to a pull request**: no bound has a number. What was the owner's to choose, the owner chose on 2026-09-24 (below); each live run's cap stays the owner's, set when its estimate exists.

### Round 58: the owner's four decisions, and six corrections

**The owner's decisions of 2026-09-24**, each recorded in the section where it acts and removed from "does not settle":

1. **`read` may name any path in the view.** This knowingly relaxes M4 READINESS §5's *"the model never introduces a path"* for that one tool, and M4's §5 now says so. M5's §4 states the controls that replace the closed set, and negative control 5 is extended to match.
2. **The first claim-producing family is `distill_investigation`**, carrying the ancestry-root obligation, its test and its mutant.
3. **J2's live acceptance runs on CBR's own test output**, produced locally at a pinned commit and sealed whole.
4. **The loop's live run ingests Knowscroll's 22 decisions first**, with the owner's rulings of 2026-09-20, recorded as the owner as at m3d. That is harness code, reviewed before the run.

**The six corrections:**

1. **Order.** J2 is m5a, then the loop, the tools, `distill_investigation`, `answer_gap` with J3 and J5, and maintenance. m5g may go at any point, and every row names its dependencies.
2. **Checkpoints and turn records** get m4d's readable-set gate at all four doors, and hold ids, ranges and digests rather than repository text. The model-prose fields are named and gated.
3. **Replay, turn by turn.** Its gate is in m5b: a loop rebuilt offline reproduces its finding.
4. **Every tool argument is bounded**, and one that breaks a bound is a typed outcome, never trimmed.
5. **Investigation accounting, per case.**
   - Gap loops come after items and discovery.
   - A gap loop is not started below a floor, and above it ends with partial coverage and a named gap.
   - J2's parts are claimed together, as discovery's two steps are.
6. **Background spend** is any call not caused by a request a principal submitted. It sits behind a configuration member that defaults off. The gate is a test that no maintenance call reaches admission while it is off.

**Two facts and one gap found while writing it**, all in the document:

- libtest's JSON output is refused on the pinned 1.97.1 toolchain.
- CBR's locally produced output carries absolute machine paths, so J2's live input has to be produced where the path names nothing.
- `answer_gap` starts from an unmet item, while the pilots' failures were in discovery, so m5e must say what starts a gap loop on such a question.

The PR table gained **m5h**, the loop's live harness.

## Earlier — m4h, a record's whole cost and a harness that reads without writing

Against `main` at `1444040`, the two corrections M4's close-out named: a commit of code and tests, one of notes, one correcting the first after CI refuted a premise of it, one of notes for that, and — after the reviewer's round 56 — one of tests at the door the rule had not been tested at and one giving the rule a single door (below).

### A record accounts for every attempt

**The defect, found writing live run 3's record.** `model::Runtime::ask` built its `Cost` from the attempt that ended the question, so a repaired step's sealed record carried its last exchange alone while the ledger held both: `brian2-m27hs`'s choice record said 4,827 tokens for a step charged 5,222 and 4,827. The reviewer confirmed it, and that a step ending unmet after a repair dropped its earlier attempts the same way.

**The fix puts the figure where the ledger is settled.** `finish()` returns what the ledger now holds for a reservation — the reported usage, the estimate when the provider said nothing, zero when nothing was sent — and a `Charges` value records it for the attempt at that point. `Cost` is every attempt (`Attempted`: admission, completion, input, count call, count prediction) and the repairs; its total is theirs. **Every way out of `ask()` carries every attempt**: answered, unmet after a repair, a call ending unmet after it was charged, and a refused repair — which is why `Outcome::Refused` now carries a cost, because a repair refused after a first attempt and its own count call had spent both. Both record sites build `Spend { cost, latency_ms }`, so one function decides. Calibration keeps reporting its completion's own figures through `Cost::last()`, so nothing it reports changes meaning.

**The format is `cbr-model-derivation/3`**: `usage.tokens` is the question's whole charge and `usage.attempts` keeps each attempt. **A `/2` record still replays**, and that is tested rather than argued: `a_record_sealed_in_the_previous_format_still_replays` rewrites a record into the previous build's shape at its own digest, points its artifact row at it, and rebuilds a packet from it. Nothing reads `format` or `usage` to answer a question, so the stores of live runs 1 to 3 rebuild as they did.

### The harness reads a store without writing a byte of it

**What it did.** It read the spend, the records and the ambiguity report over a read-write connection, and a read-write connection folds a store's log into its database and deletes the log and the shared-memory file when it closes.

**What that did to live runs 1 to 3 is not known, and the first version of this change said otherwise.** It deduced that the harness had checkpointed every store, on the reasoning that a killed provider never closes its connection. **CI refuted the premise.** A provider opens a store connection **per session** (`Provider::connect`) and closes it cleanly, so between sessions it holds none: by the time the harness reads a store it usually has no log at all, which a diagnostic run here confirmed at every read, and a store with no log is left byte-unchanged even by a read-write open — `the_harness_reads_a_store_with_no_log_and_creates_no_file` is green on `main`. Whether any run's store still had a log when the harness read it is not recorded anywhere, so it is not claimed either way.

**What each way of opening does, measured** on scratch stores, one written by a process killed with its rows in the log and one closed cleanly:

| Opened | Killed, rows in the log | No log |
|---|---|---|
| read-write | reads everything; **checkpoints, deletes the log and the shared-memory file** | reads everything; changes nothing |
| `mode=ro` | reads everything; **rewrites the shared-memory file** | reads everything; **creates a log and a shared-memory file** |
| `mode=ro&readonly_shm=1` | reads everything; **changes nothing** | **fails to open, and creates a log** |
| `immutable=1` | **misses the log's rows**; changes nothing | reads everything; **changes nothing** |

**So `mode=ro` alone would not have met the byte-unchanged requirement**, and a store is found in three states, each read its own way:

- **a log and a shared-memory file** — a provider killed while a session held the store: `mode=ro&readonly_shm=1`, which reads the log and writes nothing;
- **no log** — the usual case: `immutable=1`, with nothing to miss;
- **a log and no shared-memory file** — `stop` landing inside the provider's own close, after SQLite unlinked the shared memory and before it deleted the log. **CI found this state**, in three of four dry runs on its macOS runner, where the first version refused it by name; here the close always finished first. Every open in place creates a shared-memory file or misses the log, so the store is read from a **private, transient copy** of the database and its log, and SQLite recovers from the copied log as it would from the original. The test builds this state at its worst — rows only in the log — which a natural one, whose checkpoint ran before the unlink, never is.

`readonly_shm` is a parameter of SQLite's Unix VFS rather than of its documented URI list; the byte comparison in the tests is what holds it.

**The lesson is the one run 1 already taught, in a new place:** a deduction from how one part behaves (`stop` kills) stood in for a check of how the other part behaves (the provider closes per session). Reading `Provider::connect` would have found it; CI did instead.

### Red, then green

The record, replay and harness tests read the sealed JSON, the ledger and the files rather than the new types, so they were run on `main` at `1444040` in a scratch worktree before being run here:

| Test | On `main` | Here |
|---|---|---|
| `a_repaired_steps_record_accounts_for_every_attempt_as_its_ledger_does` | **red**: the record 5,000, the ledger 10,000 | green |
| `a_step_left_unmet_after_its_repair_still_accounts_for_both_attempts` | **red**: 5,000 against 10,000 | green |
| `the_harness_reads_a_killed_stores_ledger_from_its_log_and_changes_no_byte` | **red**: the database rewritten, the log and shared-memory file gone | green |
| `the_harness_reads_a_store_with_no_log_and_creates_no_file` | green — a guard for the new reader, whose red is mutant M6 | green |
| `a_record_sealed_in_the_previous_format_still_replays` | fails at its precondition, because `main` already seals `/2` — a guard, whose red is mutant M9 | green |
| `the_harness_reads_a_store_with_a_log_and_no_shared_memory_from_a_copy` | **red against this change's first commit**, which refused the state as CI saw it | green |

The five new unit tests use the new types, so they cannot run on `main`; their red is their mutants.

### The mutant table

**Twenty, all killed**, each against the whole workspace suite — the fourteen below and the six of round 56 after them; the runtime three again against `cbr-provider` with `--no-fail-fast`, because the first pass stops at the first failing test binary and so names only its killers. The harness mutants were run again against the corrected reader.

| Mutant | Result | What kills it |
|---|---|---|
| **The runtime keeps the last attempt only** (the reviewer's) | killed | both record tests; `a_repaired_question_costs_every_attempt_and_its_cost_is_its_ledger_rows`, `a_question_left_unmet_after_its_repair_is_charged_for_both_attempts` |
| The record's total is the last attempt only | killed | both record tests; `a_record_carries_the_model_the_admission_the_cost_and_the_choice` |
| **Unmet after a repair drops the earlier attempts** (the reviewer's) | killed | `a_step_left_unmet_after_its_repair_still_accounts_for_both_attempts`; `a_question_left_unmet_after_its_repair_is_charged_for_both_attempts` |
| A call ending unmet after a charge drops the earlier attempts | killed | `a_repair_that_fails_after_the_send_still_carries_the_attempt_before_it` |
| A refused repair drops what the question had spent | killed | `a_repair_the_envelope_refuses_still_carries_what_the_question_had_spent` |
| An unpriced settlement charged at nothing | killed | `an_unpriced_completion_is_charged_at_what_the_ledger_holds_for_it` |
| A rebuild reads only records of this build's format | killed | `a_record_sealed_in_the_previous_format_still_replays` |
| **The harness reads the store read-write** (the reviewer's) | killed | `the_harness_reads_a_killed_stores_ledger_from_its_log_and_changes_no_byte` |
| **`immutable=1` instead of `mode=ro`** (the reviewer's) | killed, on the rows: 0 read against 10,049 | the same, and `the_replay_gate_reports_an_ambiguous_question_rather_than_skipping_it` |
| `mode=ro` without the read-only shared memory | killed, on the bytes | the same |
| A store with no log opened `mode=ro` | killed, on the files created | `the_harness_reads_a_store_with_no_log_and_creates_no_file` |
| A log with no shared memory read in place, `immutable=1` | killed, on the rows: 0 against 10,049 | `the_harness_reads_a_store_with_a_log_and_no_shared_memory_from_a_copy` |
| A log with no shared memory read in place, `mode=ro` | killed, on the file created | the same |
| The copy made without the log | killed, on the rows: 0 against 10,049 | the same |

### Round 56: the door the rule was not tested at

**Two of the reviewer's mutants survived all 580 tests.** At discovery's call site the record could be given the last attempt alone (D), and both `Refused` arms could seal no cost at all (R); the same edit as D at selection's call site died (S, the control). So the rule was tested in the runtime and at selection's door, and not at discovery's — **the door live run 3's repairs went through**. It is round 50's shape again: the rule held where the test pointed, and the test pointed at one of two doors.

**Two tests, each end to end through a real provider:**

- `a_repaired_discovery_choice_is_sealed_with_every_attempt_as_its_ledger_charged_them` is run 3's shape: an item, terms, a choice answered in prose and repaired with ids. The choice's record is the last two rows the ledger charged, and the request's records together account for every row.
- `a_repair_the_envelope_refuses_is_sealed_with_what_its_first_attempt_cost` reaches `Refused` at a call site. A first store measures what the first attempt and its repair reserve; a second is launched with `--model-run-ceiling` equal to the first reservation, which admits the first attempt and, once it has settled, cannot admit the repair. The item is unmet with `run_over_ceiling` — asserted, so the test cannot pass by reaching some other ending — and the record carries the first attempt's charge, with the refused repair as a second, unadmitted attempt.

**And one door.** The `Unmet` and `Refused` arms were identical at both sites, which is how the rule could be kept at one and not the other. `derivation::taken` now turns an outcome into its record's answer and cost for both, passing the cost through whole however the question ended; each site keeps only its reading of a usable reply.

| Mutant | On | Result | What kills it |
|---|---|---|---|
| **D** — discovery's site records the last attempt only (the reviewer's) | the code before the door | killed: `[5000]` against `[5000, 5000]` | `a_repaired_discovery_choice_is_sealed_with_every_attempt_as_its_ledger_charged_them` |
| **R** — both `Refused` arms record no cost (the reviewer's) | the code before the door | killed: `None` against `Some(5000)` | `a_repair_the_envelope_refuses_is_sealed_with_what_its_first_attempt_cost` |
| **S** — D's edit at selection's site (the reviewer's control) | the code before the door | killed: 5,000 against 10,000 | `a_repaired_steps_record_accounts_for_every_attempt_as_its_ledger_does` |
| D2 — discovery records the last attempt only, after the door | the code with the door | killed: `[5000]` against `[5000, 5000]` | the discovery test above |
| R2 — the door's `Refused` arm records no cost | the code with the door | killed: `None` against `Some(5000)` | the refused-repair test above |
| S2 — selection records the last attempt only, after the door | the code with the door | killed: 5,000 against 10,000 | `a_repaired_steps_record_accounts_for_every_attempt_as_its_ledger_does`, `a_step_left_unmet_after_its_repair_still_accounts_for_both_attempts` and the refused-repair test |

## Earlier — the M4 close-out

Against `main` at `9ee22d0`, for [issue #21](https://github.com/Combraton/cbr/issues/21). **Documents only; no code.** It records live run 3, the owner's decision on Journey 2, and the close-out itself, and it corrects the statements about calls made that the runs overtook in JOURNEYS, RELEASE-SCOPE, READINESS, VERIFICATION and the docs index. It also brings the repository README to the post-M4 position.

### Live run 3, and why M4 can close

**Run 3 completed on 2026-09-23**, once, on the owner's word carried in the reviewer's run instruction, from `main` at `9ee22d0`. It took 200 seconds and 67,395 tokens, exited 0 with nothing on standard error, and left no provider running. Every item was satisfied, every flow sealed both discovery records, and every replay reproduced its packet's sections with no difference and no ambiguous question. Every rebuild authenticated with the credential its provider issued, which is m4g working live where run 2 had failed. The reviewer scored it: J1 a pass on both models, brian2 two of three on both, Knowscroll three of three on both, with no trap triggered. The record and the scores, in the reviewer's words, are in [JOURNEYS](../verification/JOURNEYS.md#live-run-3-2026-09-23-both-pilots-and-j1-again-after-m4f-and-m4g).

**The owner's decision, 2026-09-23.** Journey 2 moves to M5 as its first journey, with its negative controls unchanged, and #21 closes on the discovery family's sealed, costed live transcripts, runs 1 and 3. That matches [READINESS §1](m4/READINESS.md#1-scope-and-the-promise), which already gave projections to M5.

**What this change corrects, all of it drift the runs caused.** It updates JOURNEYS's status line and matrix, RELEASE-SCOPE §4's M4 and M5 rows, and the status line wherever it read *"live-model evidence pending M4"*. In READINESS it fixes the stop arithmetic, which still said 363,966 and 2,886,034 after m4f had made the flow 373,188, and the statements about calls made. In VERIFICATION it fixes the three rows that said nothing had been run.

**The owner's second decision, 2026-09-23:** the bounded tool surface, the loop and its checkpoints, with MODEL-RUNTIME §6's runtime-selection tests, move to M5, as do model-assisted artifact creation and the ancestry-root obligation. That obligation is carried with the first family whose output CBR's own producer cites as claim support. RELEASE-SCOPE §2 and §4 and the close-out say so, and the close-out lists the eight §6 tests item by item: six already exercised by M4's direct-call runtime, one of them with a known failure, and two that need a runtime with tools.

**Found while writing run 3's record, reported and not fixed:** a repaired step's derivation record carries the usage of its last exchange only, because `model::Runtime::ask` builds its `Cost` from the attempt that ended the question. The ledger holds both charges and is right. The reviewer confirmed it, and that a step ending unmet after a repair drops its earlier attempts the same way. The two prose attempts' output tokens, 968 and 843, are the reviewer's, read from `model_calls`, which this session did not read. The fix goes in the next code change, with a test that a repaired step's record carries every attempt's usage and equals that question's ledger rows, together with a read-only store connection for the harness.

## Earlier — M4e to M4g, model-assisted discovery, the live runs and what they corrected

Against `main`, for [issue #21](https://github.com/Combraton/cbr/issues/21). m4e was built with no model called; the live run was its own step after it merged, and what runs 1 and 2 found is m4f and m4g, below.

m4c's call site chooses among the spans BM25 ranked **inside one file the request already named**. That cannot change what a packet finds, so it cannot move brian2, whose question failed twice because the answer never entered the candidate set. m4e is the step that can: the model may propose **search terms**, CBR runs them through its own index inside the view, and the union is a larger closed set it chooses ids from. [READINESS §5](m4/READINESS.md#model-assisted-discovery) is now what exists rather than what is planned.

### A term is the one input to a model call CBR did not compose

Everything else a request carries is built by CBR out of its own candidates. A term is text the model wrote that CBR then **acts on**, so it is bounded three ways — by count (4), by length (48 bytes) and by characters — and each is the typed reason `model_answer_over_bound`.

**The character bound is what separates a term from a sentence.** A planted file instructing a model produces prose, and prose has spaces in it; a space is not a permitted character, so an instruction arriving where a term was asked for fails the bound rather than being searched for. The separators of a path and a glob *are* permitted, so that a term shaped like one is normalised rather than refused: `src/queue.rs` is `src queue rs`, `**/*.py` is `py`.

**And normalisation is the whole safety argument, because it is not a separate path.** `discovery::words` is `lexical::query_terms` and nothing else — the same tokeniser the request's own words go through, into the same parameterised `MATCH` expression whose every element is one of those tokens quoted. There is no expression a term can write and no path it can name; a test asserts the two functions agree rather than trusting that they do.

**Nothing is trimmed to fit.** Five terms where four were asked for is the typed reason, never the first four: taking part of an answer would be CBR deciding which part of the model's answer to act on.

### The reservation the first version did not have

**The ordinary reading takes a reserved share of the candidate set and no more**: at most 10 of the 20 offered come from the question's own words, and at most 6 are claims.

Without it the step is pointless, and this is not hypothetical. The first version had no reservation, and on the fixture the ordinary reading returned more candidates than the set could hold, filled all twenty, and left a proposed term contributing nothing — `merger.md`, which shares no term with the question, never entered the candidate set at all. That is exactly backwards for the question this step exists for, where the ordinary reading returns confident near-misses and misses the answer. **A test caught it, not a reading.**

### What a failed step leaves, which is not the same for the two

- **Terms fails:** the deterministic reading stands.
- **The choice fails:** the deterministic reading stands, **not the union**. The two steps are one flow — terms proposes where to look, the choice decides that any of it belongs in a packet — so carrying term-driven spans by rank alone would carry them on a suggestion nothing acted on, and BM25 scores from two different queries are not one ranking.
- **The choice is skipped because it could not change the answer:** the union stands, because carrying everything is what the choice would have done.

In every case the packet says a step did not happen, as an omission naming it with `unavailable` — the protocol's own vocabulary, and literally what it was. *Why* is the typed reason in the step's own sealed record.

### The investigation limit counts, where it used to only gate

At m4c the number said *whether*: a model was asked for every item of any request whose budget was above zero. m4e adds two questions per request, and a limit that counted some kinds of call and not others would be two meanings for one number.

So every distinct question spends one unit; a question already asked this compile is free, because its answer is the pool's; items are asked first, because an item is what the request required and discovery is advisory; and **a flow that cannot finish is not started** — discovery claims its two units together or spends neither. Counted on the **question** rather than on the call, because a question the pool defers is still one this compile asked, and a budget counting only settled answers would admit as many calls as there are ticks.

**This changes m4c's behaviour**, deliberately and visibly: a request with two items and a budget of one now satisfies the first and gives the second `investigation_budget_exhausted` rather than a model call. It is the owner's to reverse if that reading is wrong.

### The arithmetic, computed rather than estimated

Every bound on the two requests is a constant, so the largest either can be is a number, and `discovery::tests` computes it against the ceiling it has to fit under — a document cannot notice when a constant moves.

| | Worst case, tokens | Against |
|---|---:|---|
| One item's selection | 39,612 | per-request 250,000 |
| Discovery's terms step | 31,406 | per-request 250,000 |
| Discovery's choice step | 89,916 | per-request 250,000 |
| The whole flow, each step with its permitted repair and its count | 363,966 | per-job 1,000,000 |
| Five such flows | 1,819,830 | run ceiling 5,000,000 |

**Two questions per request — not per item and not per discovered section.** Discovery does not scale with what it finds, which is the property the whole table rests on.

### The record learns two shapes, and the format becomes `/2`

`Proposed` holds the terms **as the model wrote them**, because a record of what CBR made of an answer is not a record of the answer — and it is the evidence that a planted file's instruction reached no further than a term. `ChoseMany` holds the ids.

A candidate now says which **kind** it is, because claims enter candidate sets here and a claim has no line range; without it a record would say `lines 0-0` of a path that is a claim id, which reads as a span of a file that does not exist. The kind is part of the question's digest, so a record sealed by m4d never answers a question asked by this build — correct, because the two showed the model different things. **No `/1` record exists anywhere but in a test store.**

### Claims in candidate sets make m4d's gate load-bearing

The reviewer's mutant at m4d — sealing `readable_under(view, &[])` — survived all 496 tests because no test had a claim in the store: both sides of every comparison were empty lists. m4d added the end-to-end tests; **m4e is where the property does work**, because a choice record's `offered` now holds claim text a job could read. `derivation_claims.rs` has that case with both arms, and the mutant dies on a real candidate set rather than on a constructed one.

**Anchors are not offered**, and that is a boundary rather than an omission: an anchor is where a name is defined and used, and choosing among them would be choosing which definition a name means, which tags cannot say.

### The live run, written down before it is run

[`scripts/m4e_run.py`](../../scripts/m4e_run.py) is the first live run as code, for the reason m4b already recorded about `--calibrate`: *if the first live call needs new plumbing, the first live call runs code nobody reviewed.* Its dry-run mode drives every stage against the fake and is exercised by `m4e_harness.rs`.

Every rule the owner set that a harness can enforce is a refusal made before a provider is launched, so a run that breaks one costs nothing: a checkout whose **origin** is not one of the three public repositories, a checkout off a commit the manifest pins, an investigation budget the items would exhaust before discovery was reached, a model outside the three M4 may name, an output directory inside this repository, `--live` without `--permit-model-network`, a ceiling above 5,000,000 or a stop above 2,250,000. The stop is checked **before a run, against that run's computed worst case**, because halting mid-call would leave a charge nobody reconciled and halting after the fact is a report rather than a bound.

**The replay gate compares sections, not packet digests** — and that was a defect first. A sealed packet names its own request, so two request ids can never be byte-identical and the digest comparison the gate started with always differed. Byte identity across two stores asking the same request is the suite's own gate in `derivation_replay.rs`; what this one asks of a real question is whether the rebuild reproduced the same content from the records.

### The consequence of the ambiguity rule, planned for

A call that timed out beside a successful rerun of the same question leaves two records that disagree, and **that question is unreplayable from then on**. In a live run that is the ordinary state, not a corner case.

So the gate reports it: `--ambiguity <data directory>` names every question whose records differ, both records, and the answers that differ. A gate folding these into "nothing retained" would hide the one failure an operator can act on. **The gate decides nothing** — the provider's rebuild decides and the section comparison says whether it worked; the gate explains. A test pins the two together on one store so the agreement rule cannot move in one and not the other.

**The remedy is in [READINESS §6](m4/READINESS.md#the-consequence-of-the-ambiguity-rule-and-the-operators-remedy) and is demonstrated rather than described**: `evidence.purge` on the **failed** call's record — never the successful one, which would leave a history nobody made — by a principal in `authority_principals`, in a session that negotiated `evidence.retention_control`. The ledger row stays; the call was charged. `a_purge_makes_an_ambiguous_question_replayable_again` runs the whole of it: two runs propose different terms, the rebuild declines, the owner purges one, and the question replays again.

**That test also earns a constant its keep.** The terms are part of the choice question's digest, and the only case where that is load-bearing is exactly this one: after a purge leaves one terms answer standing, the two choice records must be two questions rather than one ambiguous question. Without the test the constant would have been a rule nothing read.

### The mutant table

**Thirty-two mutants over two rounds, thirty-one killed, one surviving.** Each was run against the **whole workspace suite**, not against the tests it was aimed at. Three survived the first pass and were killed only after a test was added, and a fourth — the reviewer's, on the cap the union is composed under — survived the whole of the first round; that is the third milestone running where the survivors have been the useful part of the exercise.

| Mutant | Result | What kills it |
|---|---|---|
| The count bound on proposed terms removed | killed | `a_terms_answer_that_breaks_a_bound_widens_nothing` |
| The length bound on a term removed | killed | `a_term_longer_than_the_bound_is_a_typed_unmet` |
| The character bound on a term removed | killed | `a_terms_answer_that_breaks_a_bound_widens_nothing` |
| A space admitted as a term character | killed | the same; prose stops failing the bound and arrives as a term |
| **A term used verbatim instead of tokenised** | killed | four, including `a_term_that_looks_like_a_path_becomes_plain_words` |
| An id that was not offered resolved to the first candidate | killed | `a_choice_that_was_never_offered_widens_nothing`, and negative control 5 |
| The bound on how many ids may be chosen removed | killed | `a_choice_that_was_never_offered_widens_nothing` |
| The chosen ids kept in the model's order rather than the offered order | killed | `the_choice_comes_back_in_the_order_it_was_offered` |
| **The reservation removed, so the ordinary reading fills the candidate set** | killed | three, including `a_term_the_model_proposed_widens_what_discovery_offers` |
| A failed choice keeps the union instead of the deterministic reading | killed | `a_choice_that_was_never_offered_widens_nothing`, and negative control 5 |
| A flow started with room for one step rather than two | killed | `a_flow_that_cannot_finish_is_not_started` |
| A discovery step's derivation is not sealed | killed | two in `derivation_claims.rs` |
| The readable set narrowed to the view, dropping the claims half | killed | four, including the m4d ones — **now through a real candidate set** |
| The budget check removed at the selection call site | killed, **observed at its own stage** rather than in this sweep | `the_investigation_limit_counts_questions_and_the_budget_runs_out` |
| The cap on carried claims removed when a model chose them | killed | `among_many_eligible_claims_the_packet_carries_the_ones_the_question_is_about` |
| **The terms dropped from the choice question's digest** | killed | `a_purge_makes_an_ambiguous_question_replayable_again`, and only that |
| A rebuild calls a model for a discovery step instead of reading a record | killed | `a_dry_run_drives_every_stage_and_reports_what_it_found` |
| A failed step leaves no omission, so the packet does not say it failed | killed | four |
| **A claim offered as if it were a span** | killed **after a test was corrected** | `a_candidate_set_that_held_a_claim_is_sealed_under_that_claim` |
| **The model's chosen claims ignored, so the deterministic cut stands** | killed **after a test was added** | `a_claim_the_model_did_not_choose_is_left_out_and_one_it_chose_is_carried` |
| **A binding claim droppable by a model that did not list it** | killed | `a_binding_claim_is_carried_whether_the_model_listed_it_or_not` |
| **A view with nothing readable in it still buys a call** | killed **after a test was added** | `a_request_with_nothing_readable_in_its_view_buys_no_call` |
| Span candidate ids taken from the input position rather than the kept one | **survived** | nothing — below |

### The ten of round 40

The reviewer's mutant, and one for each correction it took. All ten killed, each against the whole workspace suite.

| Mutant | Result | What kills it |
|---|---|---|
| **The cap on the union removed** (the reviewer's own) | killed | `the_union_is_bounded_however_far_a_term_reaches` |
| The reservation removed, so the ordinary reading fills the set | killed | four now, the new one among them |
| **Every launch given the whole ceiling rather than what is left** | killed | `each_launch_is_given_what_is_left_of_the_run_and_not_the_whole_of_it` |
| The stop checked after a run rather than before it | killed | the same test's second half |
| **The origin check dropped, so an id is again a repository** | killed | `a_repository_is_what_its_origin_says_and_not_what_the_manifest_called_it` |
| A pinned commit not checked against the checkout | killed | the same |
| **The investigation check dropped, so discovery can be skipped silently** | killed | `a_run_whose_items_would_eat_the_budget_never_reaches_discovery` |
| The work directory put back outside `--out` | killed | `a_dry_run_drives_every_stage_and_reports_what_it_found` |
| Discovery's records counted as every record | killed | the same |
| **The macOS gate removed from the harness test that permits the network** | killed | `no_test_launches_a_serving_provider_with_a_real_model`, extended |

### The three the first pass missed, and the one that still survives

**A test of mine was satisfied by the wrong thing.** `a_candidate_set_that_held_a_claim_is_sealed_under_that_claim` asserted that a request body contained `claim drains` — and a claim's own content *begins* `claim drains revision 1: …`, so the assertion held however the candidate had been labelled. The mutant offering a claim as if it were a span survived it. The fix is to assert on the line that offers it, `[k1] claim drains`, which only the offer can produce. This is the vacuity failure of m4d in a new shape: **a property whose evidence is also produced by the thing it is meant to distinguish is not evidence.**

**A rule with no test for its negative arm.** Nothing asserted that a claim the model did *not* choose is left out; the tests all watched claims being carried. Both arms are now one test, because either alone is satisfied by doing nothing — a rule that always carries passes the first half, and one that always drops passes the second.

**A guard written and not exercised.** The check that a request with nothing readable in its view buys no call was added from a reading and had no test, which is exactly the recurring failure this file keeps recording. It has one now, through the door a grant actually opens: a reader whose grant names no repository at all.

**The survivor, recorded rather than dismissed.** `span_candidates` gives a candidate the id of its position among the **kept** spans; the mutant gives it the position among the spans it was *handed*. The two differ only when a span is dropped for having a blob that cannot be read — and no test reaches that, because the indexer skips a blob too large to read, so a span in the ranked set always has readable bytes. The branch is reachable only when an index outlives the bytes it indexed.

**It is not recorded as equivalent**, and the difference matters: STATE already records a mutant this session argued was equivalent and was not. What can be said is narrower. The two lists are appended in one statement, so they cannot drift; the mutant changes which of two correct-today schemes is used; and if the branch ever became reachable, the mutant's scheme would resolve an id to a span nobody was shown — the closed set broken from the inside, which is why the code is written the other way.

### Round 46: what the live run found, and what a fake could never have

**The run cost 65,144 tokens and found four defects.** Three of them a fake transport could not have shown, and that is the argument for having run it: a fake answers instantly, in the shape it was scripted with, at whatever length the script says.

**Both of my inferences from the report were wrong**, and the reviewer had the evidence. I guessed the six `sections_identical: false` were an index that had moved between the two compiles; the only differing leaf in every store was `citations[].evidence.provider`, `cbr` live against `conformance-provider` rebuilt, because the harness launched the rebuild under a conformance configuration. Every section, span and excerpt was identical. I guessed `j1-m27hs` had reached `worth_choosing`'s false branch; its terms step was `model_answer_truncated`. **Reading a report is not the same as reading the evidence**, and I was right not to open the packets and wrong to reason as though I had.

**The design defect, which is the one that matters.** The union's question half was the raw top of the ranking, while the packet publishes under `DISCOVERED_PER_PATH`. J1's twenty candidates to M3 held thirteen spans of one file and **no span of the ADR the deterministic packet cites** — so the model could not keep a fact it was never offered, and the nine ids it chose were capped to five at publish. `j1-m3` returned code only, and its score measured the candidate set rather than the model. Both halves of the union are now built under the same cap. An offer the packet cannot honour is not an offer.

**A rule I had written down was doing work I had not noticed.** Capping the union makes it smaller, and on the suite's own fixture it fell below `DISCOVERED_SPANS` — at which point `worth_choosing` correctly declined to ask a question that could not change the answer, and nine tests went red because the choose step stopped happening. That is the rule working. The fixture grew to have a choice worth making, and the *narrow* case became reachable for the first time, which closed a gap this file had recorded as unreachable.

**The budget was sized for an answer and not for thinking.** Both discovery steps sat on `MIN_OUTPUT_TOKENS`, 512, because `generation_for` multiplies a small answer. `MiniMax-M3` used 13 to 32 output tokens a step; `MiniMax-M2.7-highspeed` used 277 to 512 *reasoning* before writing anything and truncated two of its three flows — one of them again at the 1,024 its repair asked for. Reasoning is not proportional to the answer, so it cannot be sized from it. Discovery steps now floor at 2,048. Selection is deliberately unchanged: nothing measured truncated there, and raising a bound against no measurement is the estimating these constants exist to stop.

**And a repair spent on a formatting habit.** `MiniMax-M2.7-highspeed` fenced its JSON; that parsed as nothing, cost the one repair the step is allowed, and the repair then ran out of room. A fence is a model being helpful about formatting rather than answering a different question, so it is unwrapped — conservatively, only a whole answer that opens with one, because digging an object out of prose would be guessing which part was the answer.

**The gate now says what differs.** Reporting only that two things are unequal made its reader open six packets to find out. It names the leaf, and carries the values only when they cannot be somebody else's source: an identifier-shaped string is printed, anything else becomes its length and a digest.

### Round 48: the fix that broke the thing beside it

**Run 2 stopped at its first run's replay stage**, 8,473 tokens in, on `authentication_failed`. The cause was m4f's own correction: making the live rebuild launch under the production configuration changed which credential that launch had to present, and the call site chose one from whether the *run* was live — a line written when the rebuild was always a conformance launch. **A configuration and the credential that authenticates against it are one decision, and they were two.**

They are one now. `credential_file` takes the configuration being launched and reads the credential off it, so the bug class disappears with the signature rather than being guarded against: a caller that has the configuration cannot present the wrong credential for it.

**The dry run's blindness here is structural.** In dry mode both launches are conformance and both carry the dry-run credential, so the production side of this rule has no test and cannot have one short of a live run. What is testable is the decision — for each launch the harness makes, what its configuration admits against what it presents — and that is what the test asserts, without launching anything. Said plainly rather than left to a reader of a passing suite.

**And a second drift, found by the reviewer reading rather than by anything failing.** `scripts/m4e_run.py` still refused runs against `WORST_CASE_FLOW_TOKENS = 363_966` while the figure it guards became 373,188 at m4f — the stop was 9,222 short of the flow it bounds. The Python constant is now read by the Rust test that computes the figure, so the two doors cannot drift again. **A number that lives in two languages needs a test that crosses the boundary**, and this one did not have it because the boundary was invisible from either side.

**What the fragment still established.** The one flow that ran was the one run 1 had lost, and both m4f fixes worked on it: `j1-m27hs` completed both discovery steps at 511 and 676 output tokens with no repair, where run 1 truncated at 512; and the ADR span run 1 never offered was candidate `d7` and was chosen. One flow of six, from a run that did not finish — recorded as partial, because the alternative is to say nothing about the only evidence the run produced.

### The m4g mutants, and a test that was not testing anything

**Eight run, seven killed, one that should not have been written.**

The first pass of this PR recorded a survivor — *the rebuild handed the serving stage's credential* — and argued it was unobservable in a dry run. The reviewer found why: **the test meant to hold the rule was asserting a restatement of it.** `credential_for` was a second function, written so the rule could be checked without a temporary directory, and nothing in `one_run` called it. Their mutant — one `or DRY_RUN_CREDENTIAL` inside `credential_file`, which is run 2's defect one level down — passed all twelve harness tests.

So the second function is gone, there is exactly one that decides a launch's credential, and it is tested at the door: called over a real work directory for all four launches, checked on **what it returned and what it left on disk**. A production launch must take the issued credential and write nothing.

| Mutant | Result | What kills it |
|---|---|---|
| **A production configuration falls back to the dry-run credential** (the reviewer's) | killed | `credential_file_gives_each_launch_what_its_own_configuration_admits` |
| A production launch writes a credential file as well | killed | the same |
| **The rebuild handed the serving stage's credential** | killed — *was recorded as a survivor and was not one* | `nothing_but_that_one_function_decides_a_credential` |
| The credential chosen from the run rather than the configuration | killed | the door test |
| A production configuration treated as admitting a written credential | killed | the door test |
| **The harness's worst case drifted from the computed one** | killed | `the_harness_stops_against_the_same_worst_case_this_module_computes` |
| The drift test's assertion made a tautology | **not a mutant** | nothing, and nothing could — below |

**What the survivor actually was.** Not a gap in what a dry run can reach: a gap in what the test was pointed at. The call-site mutant dies to the source assertion, and the reviewer's dies to the door test, and neither needed a live run. **"This cannot be tested here" is a claim to check before it is a claim to record** — I recorded it, and it was wrong.

**The one that should not have been written.** Turning `assert_eq!(x, PUBLISHED_FLOW)` into `assert_eq!(x, x)` survives by construction: it is a mutation of an assertion into a tautology, and the thing that would have to catch it is the test it just emptied. **A mutant on an assertion measures nothing**; mutate what the assertion is about.

### The m4f mutant table

**Thirteen mutants, all killed** — two at the union and two at the gate as the reviewer asked, and one for each correction. Three survived the first pass and were killed by tests added after it, which is where the exercise earns its keep.

| Mutant | Result | What kills it |
|---|---|---|
| **The union's question half taken raw, not per-path capped** | killed | `every_span_the_packet_would_publish_is_offered_to_the_model` and two more |
| **The union's term half taken raw** | killed **after a test was added** | `no_file_fills_the_candidate_set_whichever_half_reaches_it` |
| **The two halves counted against separate tallies** | killed **after a test was added** | the same; the term is chosen to reach a file the question already reached |
| The per-path cap raised so it binds nothing | killed | three, including the narrow-view case |
| Discovery steps back on the answer-sized generation floor | killed | `both_steps_ask_for_room_to_reason_and_not_just_room_to_answer` |
| A fenced answer refused again | killed | `a_fenced_answer_is_read_rather_than_repaired` |
| **Unfencing digs an object out of prose** | killed | `unfencing_does_not_go_looking_for_json_inside_something_else` |
| The live rebuild launched under a conformance configuration again | killed | `a_live_rebuild_is_launched_under_the_configuration_the_live_run_used` |
| The gate reports that sections differ and not where | killed **after a test was added**, and by reading the source | `the_gate_derives_whether_it_matched_from_the_leaves_it_found` |
| The gate prints a differing value whatever it holds | killed | `the_gate_says_which_leaf_differs_and_carries_no_repository_text` |

**One gap, recorded rather than described as covered.** The gate's *call site* is not observable from any test here: both launches of a run are over one store, so a dry rebuild always reproduces the packet and the reporting branch never runs with a difference in hand. The function it calls is unit-tested on canned trees, `sections_identical` is now derived from the leaves so the two cannot disagree, and the wiring between them is held by reading the source — which is a weaker thing, and is why it is written down here.

**The published arithmetic is now a test.** `the_published_arithmetic_is_what_the_bodies_actually_cost` computes all four figures READINESS §3 quotes from the real request bodies and fails when the table drifts. The generation floor moved them: 32,943 and 91,453 a step, 373,188 a flow, 2,239,128 for six.

### Round 40: five bounded corrections, and what each of them was

The reviewer accepted discovery's code as built and returned five corrections — **one survived mutant in it and four defects in the harness**. Each is worth keeping for the shape of the mistake rather than the fix.

**A bound no fixture reached.** `span_room` caps the union at twenty candidates; the reviewer set it to `usize::MAX` and all 550 tests passed. The union in every fixture was eleven, so nothing was ever near the cap, and the 89,916-token worst case the arithmetic rests on was resting on a constant no test read. `bloom.md` is sixteen sheets of chunks that only one term reaches, and the assertion is on the choose **body**: twenty ids offered, no twenty-first, none of the reserved ten taken by the term and at least one of the rest given to it. **This is the golden-digest lesson of m3d again** — a bound is only a bound where something presses against it.

**A hard cap that was per launch.** `Ledger::run_spend` sums the store it was opened over, and every run of §9 opens a new data directory, so passing `--model-run-ceiling 5,000,000` to each of six launches is the cap six times. What held the total down was the per-job ceiling of 1,000,000 plus a stop checked after each run — about 3.25M worst case, under the cap by arithmetic nobody had done. Each launch is now given the cap **less what the launches before it spent**, and the stop is checked **before** a run against that run's computed worst case. READINESS said "enforced by the per-job ceiling"; it says what is true now, and the six runs it names are reconciled with the five the table counted.

**"Public repositories only", enforced against a label.** The id in a manifest is a string somebody typed. `{"id": "brian2", "path": <any checkout>}` was admitted — the owner's word covers repositories, and the check covered names. The origin is now read from the checkout with `git remote get-url origin`, normalised across its spellings, and matched against a pinned table, with the commit checked too when the manifest pins one; both in **dry-run mode as well**, because a dry run reads the same bytes off the same disk. The suite's own fixture now carries `cbr`'s origin, which is the check made against the fixture rather than around it.

**A measurement that could have measured nothing and said so nowhere.** Items are asked first and a flow that cannot finish is not started, so a run with four wants and an investigation of five asks discovery *nothing* — and the packet it produces is indistinguishable from one the model was asked about and did not widen. The harness refuses a run whose investigation is below `wants + 2`, and every run's report counts the `discovery.*` records that were sealed, so a reviewer can see the question was asked rather than infer it from a packet.

**The evidence was in a temp directory.** A live run's store holds the ledger the spend is read from and the records the replay gate replays, and the comment called it "the evidence" while `tempfile.mkdtemp()` put it on internal disk for a reboot to empty. Every work directory is now under `--out`, which `check` already requires to be outside this repository, and nothing in the harness deletes a store.

**And the permit-flag guard had a hole the shape of this harness.** `credential_discipline.rs` excuses a piece that passes `--permit-model-network` while configuring no model — written for a Rust launch whose whole configuration is visible in the piece. The harness's `--live` *writes* a production configuration naming a `model_runtime` and then launches under it, so two test cases passed the flag and were excused. Both are now their own test, gated off macOS with every other one; and the guard counts `"--live"` as configuring a model, so the next one cannot slip through the same way. The two refusals that do not need `--live` — a ceiling above the cap, a stop above the stop — are asserted in dry-run mode, on every machine.

### Round 41: a pinned origin proves identity, not visibility

The reviewer found, at the round-40 review, that `Legend101Zz/Knowscroll-v2` was **private** on the GitHub API while [READINESS §7](m4/READINESS.md#7-what-may-be-sent) and the harness's pinned table both called it public. The owner has since made it public and the reviewer re-verified.

**The gap was mine, and it is not the sentence.** The origin check of round 40 proves a checkout *is* the repository it claims to be. Visibility is a different question and a live one: it is a fact about an account this project does not control, and it can change in either direction under a table committed weeks earlier. The refusal I wrote read as though it enforced §7's "all three are public", and it did not.

So the harness asks, **in live mode only** and **unauthenticated**: one `GET https://api.github.com/repos/<owner>/<name>` per distinct origin, no token, no `gh`, no proxy from the environment, no redirect followed, a timeout. Only HTTP 200 carrying `"private": false` admits a repository; a 404 — which is what a private repository answers a caller with no credential — a 403, a rate limit, a timeout and an unreadable body are refusals by name, before any provider launches. **Unknown lands where private lands.**

**Why unauthenticated is the design and not a shortcut.** A call carrying the owner's token asks *can they see it*, which a private repository answers yes. The question worth asking is *can anyone*, and only a call with nothing attached asks it. A test reads the harness's own code — with its prose stripped by Python's parser rather than by a text search, because the file says in words that it reads no `.netrc` and a plain search cannot tell that sentence from the thing it forbids — and asserts that no name on this path can reach a credential.

**The call itself has no test**, and that is stated rather than left to be inferred: it opens a socket to a third party, a dry run must send nothing off the machine, and no test in the suite may. What is tested is the parser, on nine canned answers. Four mutants on this path — a private repository admitted, the live-only guard removed, a credential added to the request, a 404 admitted — each observed failing and reverted.

### A probe bug, in the other direction from the last one

m4d recorded a probe that reported **seven survivors it never observed**. This one reported a *kill* it should not have trusted, by a different route: `apply_mutant` backed up each file as it processed each edit, so a mutant with **two edits to one file** stored the already-mutated text as its backup and "restored" half a mutant. The half that stayed was an unused loop variable, which changes no behaviour — so the two mutants that ran after it were not wrong, and both were re-run clean to say so rather than reasoned about.

**What it cost was a verdict, not a result.** The re-check of that mutant came back `NOT-APPLIED` because its target text was no longer there, which is the probe refusing to guess — the same discipline that produced `NO-RESULTS` at m4d, working. The fix is one line: back up each file once, before any edit touches it. **A probe is a measuring instrument and it gets the same treatment as the code**, which is now twice this milestone.

## Earlier — M4d, derivation records and replay

[PR #27](https://github.com/Combraton/cbr/pull/27), **merged** as `7fa939c`, pinned to `8bc5698`, `merged_at: 2026-09-21T16:21:16Z`. For [issue #21](https://github.com/Combraton/cbr/issues/21). **No model was called.**

### The permitted hand launch, because a rule broken twice is not working

[VERIFICATION rule 4](../VERIFICATION.md) has been written down, broken, restated and broken again — the second time by me, while debugging m4c. The reviewer's instruction was to stop restating it and **make the permitted path easier than the forbidden one**, and that is `scripts/debug_launch.sh`: a throwaway data directory, a socket, and the credential file `cbr` wants, printed in the form it wants them.

It **cannot be talked into calling a model**. It writes its own configuration, in the production format, naming no `model_runtime`; `--config` is refused, so it cannot be handed one that does; `--permit-model-network` and `--calibrate` are refused by name before anything starts. `launch::decide` then returns `Serve`, which reads no credential, and no permit exists so nothing in the process can open a socket to a provider.

The tests read the **argument vector back from a stub the script execs**, and parse the configuration file that vector points at. "It passes no model" is therefore checked at the exec rather than by reading the script for reassuring text — which is the failure mode the whole rule exists because of. One more test drives the launch end to end through `cbr` over the socket it printed, because a permitted path nobody can use will be walked around a third time.

### What a derivation record holds, and the design change inside it

READINESS §6 said a record would hold repository excerpts and claim text. **It holds none.** What was shown is identified by path, line range and the digest of the text; the bytes are the source artifact the packet already cites, sealed once and shared. So a store that keeps derivations for ever keeps no second copy of anybody's repository, and the retention question below has a much smaller answer than it would have had.

It holds what the reviewer asked for: the model id, the admission path — CBR's local bound alone, or the provider's count — the token usage, the latency, the candidate set offered, and the id chosen.

**A retained answer is found by the digest of its whole question**: the model, the task, the selector, and every candidate in the order it was offered. The answer is an id out of a closed set, so reusing it for a different set would be choosing from a list the model never saw. A file edited since the call is a different question and finds nothing.

### Cancellation, failure, and what each leaves behind

**Taking the answer is what seals the record.** A call whose answer is never taken — the request was cancelled, or its job ended — seals nothing, because the compile that would have read it never ran. What it **spent** stays in the ledger and must: a call that went out was charged.

The test has teeth rather than relying on timing: it holds the call *after it has sent*, cancels the request, and only then releases the barrier. The call completes, settles into a slot nobody will ever ask about again, the ledger carries the charge, and no derivation exists.

A call that was made and could not be used **is** recorded, with the reason it failed for. Recording only the answers that worked would make the ledger and the derivations disagree about how many calls there were.

### Readable only under the job's own view

The readable set is sealed on the artifact record and checked in `evidence.fetch` and `evidence.inspect`, **after** step 6 has decided the caller may read artifacts at all. Only derivations carry the member, so anything sealed before m4d is read exactly as it was. The answer is `permission_denied` rather than `not_found`: the caller is authorised for the subject and what they lack is the content behind it.

**Both arms are tested.** A reader holding `evidence.read` over every artifact but not the repository is refused; a reader who holds the repository is not. Without the second, "refuse everybody" would pass.

The same check gates replay, so a narrow job cannot read a wider one's answers from the inside — the gate walked around from within, which is the shape of the M3 leak.

And the m3c byte-identity assertion applied to derivations: the packet a reader outside the job's view receives is byte-identical to the one they would have received in a store where no model was ever called. **Nothing today puts a derivation into a packet.** That assertion is a gate for what someone later might do, not a bug it found.

### The rebuild, and the one claim I had to change

`--replay-model` answers every question from a retained record, charges nothing, seals nothing, reads no credential and is refused the permit. Its transport is `model::Panicking`, which panics if reached — contained by the pool as the typed `work_panicked`, so the worst case of a call site that one day forgets is a failure with a name rather than a call an offline rebuild promised not to make.

**The gate compares digests across two stores, where READINESS said within one.** That caveat is about *ingested* artifacts, whose ids embed the instant they were ingested; a model-assisted packet of this shape cites *source* artifacts, whose ids are the blob and are the same in any store with the same tree. So one store rebuilds `probe` from what a different request retained, a second store answers `probe` live, and the two sealed packets are the same bytes. The ingested-artifact caveat is unchanged and still reported.

A question nothing retained an answer for is the item's own typed reason, `model_answer_not_retained`, and **never a quiet fall back to BM25's first**. Without that the digest comparison would pass for the wrong reason on any question nobody retained.

### What is retained, and for how long

Stated in [READINESS §6](m4/READINESS.md#what-is-retained-and-for-how-long) because a request body holds repository text. Three things a call leaves: the ledger row (tokens, no content), the recorded exchange (`model_calls`, redacted — **the only place a request body is kept**), and the derivation record (no repository text, retention class `model-derivation`).

**Nothing ages any of them out.** CBR has no retention policy and the operator's remedy is the data directory. M6 owns the policy; m4d owes it a name to key on and an honest account of what exists. For a public repository this is public text on the owner's own disk; for a private one it would be that repository's text in a second place, kept indefinitely, which is one more reason the bar in §7 stays where it is.

### The listing was a third door, and only the survivor analysis found it

The readable-set gate went on `evidence.fetch` and `evidence.inspect`. **`evidence.query` was left open**, and a listing hands back the same descriptor `inspect` would — so a reader holding `evidence.read` over every artifact could be shown every derivation's job, request and model, having been refused the two doors either side of it. The mutant *remove the gate from `inspect`* surviving is what sent me looking; the hole it pointed at was somewhere else.

Fixed by splitting the check: `covers_derivation` answers the question, `readable_derivation` turns a `false` into `permission_denied`, and the listing hides the row and counts it as filtered, because hiding is not an error.

### The mutant table

**Twenty mutants, twenty killed.** Each was run against the whole workspace suite, not against the tests it was aimed at. Three survived the first pass and were killed only after a test was added; the fourth row of that group is the listing hole above, which the survivors led to.

| Mutant | Result | What kills it |
|---|---|---|
| The question's digest drops the candidates offered | killed | `a_candidate_set_with_one_fewer_is_a_different_question`, and the rebuild answering an edited file |
| …drops the model | killed | `a_different_selector_or_task_or_model_is_a_different_question` |
| A candidate's text is not digested, so an edit is the same question | killed | `a_question_whose_candidates_changed_is_not_answered_from_an_old_record` |
| `covers` always true | killed | `a_reader_who_cannot_read_the_repository_cannot_read_the_derivation_about_it` |
| `covers` checks the view and not the claims | killed | `a_reader_missing_a_repository_or_a_claim_does_not` |
| An unreadable reason is guessed at as `model_call_failed` | killed | `a_reason_this_build_does_not_know_is_not_guessed_at` |
| Nothing is sealed when the answer is taken | killed | the three record tests |
| The record is sealed without its readable set | killed | the readable-set tests |
| The chosen id resolves by position rather than by id | killed | `the_span_cited_is_the_one_the_model_chose` |
| **The artifact id is the question's digest, not the record's** | **survived, then killed** | added `two_answers_to_one_question_are_two_records`: a model is not a function, and the second answer was being silently dropped |
| The replay ignores the question's digest | killed | `a_question_whose_candidates_changed_…` |
| **The replay ignores the readable set** | **survived, then killed** | added `a_rebuild_does_not_answer_from_a_record_made_under_a_wider_view`: the fetch gate walked around from the inside |
| The replay falls back to BM25's first when nothing is retained | killed | `a_rebuild_with_nothing_retained_says_so_rather_than_choosing_for_itself` |
| The gate is removed from `fetch` | killed | `a_reader_who_cannot_read_the_repository_…` |
| **The gate is removed from `inspect`** | **survived, then killed** | added `a_reader_outside_the_view_cannot_inspect_it_either` — and looking for why it survived found the listing |
| **The gate is absent from a listing** | killed | `a_reader_outside_the_view_does_not_see_it_in_a_listing`, written red against code that had no gate there |
| A replay reads a credential | killed | `a_replay_never_reads_a_credential_and_never_opens_the_network` |
| A replay may be given `--permit-model-network` | killed | the same, and `a_rebuild_refuses_every_way_of_being_asked_for_that_would_not_be_offline` |
| `debug_launch.sh` passes `--permit-model-network` through | killed | `the_hand_launch_refuses_to_permit_model_network` |
| `debug_launch.sh`'s own configuration names a `model_runtime` | killed | `the_configuration_this_launch_runs_under_names_no_model` |

The three that survived are the useful part of the table. Two of them were cases I had reasoned about and not tested — *the artifact id is content-addressed, so of course two answers are two records* — and one of them was a hole in a place I had not looked at all.

### The reviewer's two corrections

**A rebuild was not deterministic when retained answers disagreed.** `retained()` took the first match in `der.<digest>` order, and that digest covers `made_at` — so with one question answered `c2` once and `c1` once, the same history produced either of two packets, decided by a hash of a timestamp. The reviewer measured it at five runs apiece. m4e reruns the same questions live, so the state is not hypothetical; it is what a second run of a pilot leaves behind.

The rule now: **every covered record is read, and they must agree.** Agreement is the answer; disagreement is `model_answer_ambiguous`, the item's own reason. **A retained failure beside a retained choice counts as a disagreement**, deliberately — preferring the usable one would be the rebuild deciding which of two histories to reproduce, improving on the past rather than replaying it, which is the same fault as taking the first match with better manners. A record this build cannot read counts as its own answer and takes part in the agreement, because a record nobody can read is no evidence the others are right.

**The claims half of the readable set had no end-to-end test.** The reviewer's mutant — `readable_under(assist.view, &[])` — survived all 496 tests, because no test in the suite had a claim in the store at all: both sides of every comparison were empty lists and the property was true by vacuity. A derivation names no claim today, which is why this was a correction rather than a leak; in m4e claims enter the candidate sets and it becomes load-bearing.

`tests/derivation_claims.rs` now proposes a claim, runs a model-assisted request, and takes both arms through all four doors: `inspect`, a listing, `fetch`, and a rebuild. The only difference between the two reader grants is the claim — both cover **both** registered repositories, because the job that sealed the record belongs to the authority, whose view is every registration, and a reader missing one is refused for *that* reason with the claim half never reached.

### Two smaller things the reviewer named

**A purged record could still answer a rebuild.** `state` stays `sealed` through a purge — only the `purge` member and the availability change — so a check on the state alone does not notice, and the object can outlive the purge until collection runs, or indefinitely when another sealed artifact shares the bytes. `retained()` now skips any record carrying a `purge`. The test ingests the record's own bytes as a second artifact first, so that the object survives the purge and the test is about the purge rather than about a missing file; without the fix it fails.

**READINESS §6 named two gated doors and there are three**, and the retention table did not say that the **task and the selector** are kept in the record. They are the requester's own words rather than the repository's, and they are kept because the question's digest is taken over them; both are now stated.

### A probe that reported seven results it never observed

The first run of the correction mutants said **all seven survived**, including one I had just watched a test fail red against. They had not survived: moving `cited_span` into the shared fixture had broken `model_selection.rs`'s import, so `cargo test --workspace` had not compiled since — and the probe decided "killed" by looking for `FAILED` in output that was **empty**. A build that never ran produced a table of survivors.

Fixed in two places: the import, and the probe, which now records `NO-RESULTS` and refuses to call anything a survivor when no `test result` line came back. **The rule this breaks is the one already written down** — do not log a mutant kill you did not observe — and it breaks it in the other direction, which is just as wrong and much easier to miss, because a survivor looks like diligence. Every per-binary run I had done in between compiled fine, which is exactly why the breakage stayed invisible.

### The correction mutants

Seven, each run against the whole workspace suite after the probe was fixed, all killed.

| Mutant | Result | What kills it |
|---|---|---|
| `retained()` returns the first match | killed | `a_rebuild_whose_records_disagree_says_so_rather_than_picking_one` |
| A retained choice is preferred over a retained failure | killed | `a_retained_failure_beside_a_retained_choice_is_a_disagreement` |
| `retained()` ignores the `purge` member | killed | `a_rebuild_does_not_answer_from_a_record_that_was_purged` |
| The seal drops the job's claims (**the reviewer's**) | killed | `a_job_that_could_read_a_claim_seals_it_into_the_readable_set`, and all four doors |
| `covers` drops the claims half | killed | the same, where before only the unit test held it |
| The reader's claims are computed as empty | killed | `a_reader_with_the_claim_passes_every_door` |
| The rebuild compares no claims | killed | `a_rebuild_answers_a_reader_with_the_claim_and_not_one_without` |

### The m4e scope decision, recorded at m4d and acted on at m4e

Model-assisted selection chooses among spans ranked **inside one file the request already named**. It does not touch discovery, so it cannot change *what a packet finds*. **That is why it cannot move brian2**: that question failed twice because the answer never entered the candidate set, and a better choice inside the wrong file is still the wrong file.

So m4e needs **model-assisted discovery**, designed now in [READINESS §5](m4/READINESS.md#model-assisted-discovery) inside the same safety property: the model may propose a bounded number of **search terms**, which are untrusted query input and never paths — CBR runs them through its own index, inside the view, exactly as it runs the task's own words; the union is a larger closed set it chooses ids from; every step is a recorded derivation. It still never names a file, a span or a label, and negative control 5 is re-run against both steps.

**What it cannot do:** a term the model proposes still has to occur in the repository. This makes CBR look where its own reading of the task did not suggest; it does not make CBR find what is not written down.

## Earlier — M4c, the bounded runtime

[PR #26](https://github.com/Combraton/cbr/pull/26) against `main`, for [issue #21](https://github.com/Combraton/cbr/issues/21). **In progress.** Its first commit is the record of calibration run 2.

### Calibration run 2 — the measurement, and the bound held

**Authorised by the owner on 2026-09-21, run once from `main` at `dabf033`, release profile.** The record is [m4/CALIBRATION.md](m4/CALIBRATION.md).

**No provider count exceeded its local estimate on any of the six files, so READINESS §10's stop condition did not fire and M4 is not stopped.** Against the input bound alone the ratios are **3.40 to 4.29**, which is what READINESS predicted; the table's own ratio column runs 5.35 to 27.82 because it includes the fixed 5,128 tokens of reserved generation and margin, and for a small input that is what it measures rather than the bound. The table should carry the input column.

Four things the run found that no fixture had:

- **The count response carries no usage member**, so the provider says nothing about what a count costs. CBR settles a count at the figure it counted, which is the conservative reading of silence — and **15,538 of the 15,582 tokens this run charged were CBR charging itself for seven counts the provider never priced.** An observation for the owner, not a change made here.
- **The counting endpoint over-predicted the bill by 4.4×** on the one completion: it predicted 122 input tokens and the provider charged 28. Safe, and it means the count buys conservatism the local bound already provides, at the price of a call. One sample.
- **`usage.output_tokens_details` does not exist.** The reference names `reasoning_tokens`; the service sends no breakdown, so **CBR cannot tell how much of a completion was reasoning** — which matters on the M2.x models, where reasoning cannot be turned off and took the whole sixteen-token limit.
- **A response echoes the request back**: 37 members where the reference describes 12, with `service_tier` returned as `null` though `standard` was sent.

**The credential appears nowhere in the store**, and every recorded request is byte-identical to the source file it was read from — checked by digest, on a corpus that is five thousand lines about credentials and redaction and which an over-eager redactor would have shredded.

**And a defect in the calibration itself:** it reports `STOPPED` and exits non-zero for a *truncated* completion, which is an ordinary outcome with a cost. A run that obtained every measurement it exists for is recorded as having stopped. Fixed in this milestone.

### The count becomes conditional, and the bound gets a tripwire

**What run 2 measured, turned into design.** The count is no longer made before every call: the **local bound admits alone and the call settles by usage**, and the provider count is made only when it can change a decision — a refusal on the window, the month, the job or the run ceiling, where a tighter figure could admit. Not on the per-request ceiling, which is a limit on how large one request may be and which the local bound is the conservative measure of. Which evidence admitted a call is recorded, `admitted_local` or `admitted_count`. [ADR 001 question 15](../decisions/001-standalone-v0.1-scope-and-stack.md) and [READINESS §3](m4/READINESS.md).

**A caller whose purpose is the count asks for it**: `Counting::Always`, used by the calibration, because a comparison with no count is no comparison. Making that explicit rather than implicit is what kept the calibration's second measurement alive through the change.

**The tripwire** keeps §10's stop rule alive in production: a `usage.input_tokens` above the local input bound for that request means the bound is wrong, which is recorded as its own ledger kind and admits no further model call. **Held in the ledger rather than a process flag** — stronger than the rule asked for, because the bound is a property of the code and a restart with the same code has the same bound.

Two rules overlap and the order is pinned by a test: a charge above the **local bound** stops everything; a charge above the **count's prediction** is a finding. The first is the graver claim and wins.

**A constant died of it.** m4a's `estimate` added a fixed 4,096-token generation reserve because admission happened before the request's own limit was known. Every call site knows it now, so the reservation carries the real figure — `estimate` had no caller left and `RESERVED_GENERATION_TOKENS` with it. Clippy found that, not me. The three tests m4b wrote to kill the mutants in those constants are rewritten around `input_bound`, and the property the reserve carried is asserted where the reservation is now made. It is also what made run 2's ratio column measure the reservation instead of the bound.

### Reasoning spends the output budget, and m4b had a rule wrong

Run 2's completion spent all sixteen of its output tokens on reasoning and returned no answer. On the M2.x models reasoning cannot be turned off, and the service reports **no breakdown**, so `max_output_tokens` has to cover reasoning *plus* the answer and CBR cannot learn the split by measuring.

**The sizing rule: four times what the answer needs, never below 512, and a truncation repaired once by doubling.** The multiplier is not an estimate of how much a model thinks — it follows from an asymmetry. Billing is by tokens **produced**, so an over-sized limit costs nothing that is not used, while an under-sized one costs the whole call and returns nothing. Generosity is the cheap error here. m4e replaces the multiplier with a measurement.

**m4b had truncation as not repairable**, reasoning that asking again under the same limit gives the same answer. True, and beside the point: the repair asks again with a **larger** limit. Under the old rule run 2's call was simply lost. It is repairable now, it consumes a repair, and when the repair is spent it ends as the item's typed unmet reason.

### The stall, measured away

M3 recorded the index build holding the preparation tick as a known limit and READINESS §8 made resolving it m4c's first job. `work::Pool` bounds work in flight at two, answers every ask with a typed `Progress`, and blocks no caller, so the tick that asks is the tick that returns. **Measured before and after with the same instrument, on the same machine and the same three trees** ([STALL](m4/STALL.md)), an unrelated job's worst wait falls from 8.5s, 11.1s and 3.4s to 0.13s, 0.23s and 0.15s — 65×, 49× and 23×. Time to first packet is unchanged, which is the expected result: the build costs what it costs, and the provider stops holding everything else while it pays.

Two decisions inside it are worth naming because the obvious alternative is wrong in each.

**Deferred, not queued.** Work beyond the bound is refused for now and asked about again next tick. A queue would be a promise to do work nobody may want any more, made at the moment CBR has least idea whether it is still wanted.

**A settled answer is kept until its caller takes it.** Releasing on first report breaks any compile needing two calls — the first answer would be gone by the tick the second arrived — and, worse, a released *failure* is a retry: the next tick asks again, the key is free, and the pool starts the work afresh. That is not the bounded repair READINESS §8 allows; it is an unbounded loop spending a shared quota.

### Serving calls a model

`SERVING_CALLS_A_MODEL` is true, in the commit that gave serving a call site. A request that authorised an investigation now asks a model, while preparing, which of the spans BM25 ranked inside the file an item named to cite. **No live call was made: the transport is the `model.fake` control.**

**The answer cannot widen what is cited**, and that is the whole design. The model is shown a closed set of CBR's own candidates and answers with one of their ids — never a path, a line or a repository — so the worst any answer can do is choose a worse candidate from that list. An id it was never offered, a structure that is not a choice, a provider failure, a refusal at admission and a timeout all end as **the item's typed unmet reason**, never as the span BM25 would have chosen, which would report a model-assisted selection that no model made. There is no repair for an answer that is well formed and wrong: asking again would spend a shared quota on the same question.

**A request that authorised no investigation calls nothing**, and gets exactly the compiler M3 shipped — from the same binary, which is what makes it m4e's baseline rather than a second build nobody ran.

**The fake became a transport for the call site** rather than a scripted call at startup, and the six model crash rows moved with it: they now kill a provider in the middle of preparing a real request. They live in `cbr-cli`'s tests, because reaching that call site means submitting a request and `cbr` is the client that submits one.

**One rule was loosened, and only one.** Compiling is production-only and the fake transport is a conformance control, so the two could never overlap and the call site could not be reached under `SIGKILL` at all. A conformance launch may now ask to compile, with `context.compile`. No fixture sets it, and `a_conformance_launch_prepares_nothing_unless_it_asks_to_compile` holds both halves.

### A CI flake this milestone made likely

`a_configured_model_whose_credential_is_unreadable_refuses_the_launch` failed once on Linux at `db17494` — `Unavailable` where `NotFound` was expected — while the same commit's other run passed. `Unavailable` is what CBR reports when the child could not be **started**, which has nothing to do with the exit code the test is about.

**The cause is a fork race, and it is the suite's own.** Another thread forking while the fake tool's write handle is still open leaves its child holding that handle, and the `exec` which follows fails with `ETXTBSY`. The window is the few instructions between creating the script and closing it, and it is crossed more often the more tests run beside it — this milestone added a hundred, most of which start processes. [VERIFICATION](../VERIFICATION.md) already records one timing assertion rewritten for the same reason at m3c; this is the second.

The fake-tool reads retry past that one refusal and nothing else, so a tool that is genuinely unstartable still refuses and every other outcome comes back from the first attempt. **No assertion was softened.**

### The mutant table

**Twenty-seven mutants, twenty-two killed, five surviving** — each run against *every* target, not the binary's unit tests alone, which is the mistake m4b made and which returned two false survivors then.

| Mutant | Killed by |
|---|---|
| pool: `Done` releases the slot, as it did before | `a_request_with_two_items_asks_two_questions_and_holds_both_answers` |
| pool: the bound counts settled answers too | `a_result_waiting_to_be_taken_does_not_hold_the_bound` |
| pool: **the bound counts map entries, not threads** | `a_finished_unit_nobody_asked_about_again_does_not_hold_the_bound` |
| pool: `release_all` matches nothing | `a_job_releases_every_answer_it_asked_for_by_naming_itself` |
| pool: a panicking unit is reported `Done` | `work_that_fails_reports_its_typed_reason_and_is_not_retried` |
| pool: a passed deadline does not time the work out | `a_deadline_that_passes_ends_the_work_as_timed_out` |
| selection: an id that was not offered selects the first candidate | `an_id_that_was_never_offered_leaves_the_item_unmet` |
| selection: the schema offers no closed set of ids | `the_schema_offers_exactly_the_candidates_and_nothing_else` |
| selection: one candidate is worth asking about | `one_candidate_is_not_a_question_worth_paying_for` |
| selection: a non-string id is read as the first candidate | `an_answer_of_the_wrong_shape_is_refused_rather_than_guessed_at` |
| call site: the investigation budget is ignored | `a_request_that_authorises_no_investigation_calls_nothing_at_all` |
| call site: a failed call falls back to the span BM25 ranked first | `a_call_that_fails_leaves_the_item_unmet_with_the_reason_it_failed_for` |
| call site: the answer is released the moment it is read | `a_request_with_two_items_…` |
| call site: the key forgets the path, so two files share one answer | `a_request_with_two_items_…` |
| call site: a cancelled job keeps its answers | `cancelling_a_request_frees_the_bound_its_call_was_holding` |
| clock: an instant without its `Z` is still an instant | `anything_that_is_not_an_instant_is_refused_rather_than_guessed_at` |
| clock: the month and day are not checked | as above |
| conformance: a launch that did not ask to compile compiles anyway | `a_conformance_launch_prepares_nothing_unless_it_asks_to_compile` |
| fake transport: a count call eats the scripted completion answer | three of the six crash rows |
| fake transport: the scripted usage is dropped | `a_completion_settles_to_what_the_provider_said_it_cost` |
| launch: `SERVING_CALLS_A_MODEL` is false, so the constant lies | `a_serving_launch_with_a_model_and_the_permit_reads_a_credential_and_serves` |
| cli: cancel sends revision 0 rather than the one it saw | `cancelling_a_request_…` |

**The five survivors, and why each is one.** None is counted as a kill.

| Survivor | Why it survives |
|---|---|
| the key forgets the **selector** | **Not reachable through this client.** `cbr` gives every item of a request the same selector, so two items on one file always ask the same question. Over the protocol an item carries its own, and there a key without the selector hands the second item a choice made from a candidate list it was never shown. The test says so rather than implying coverage it has not got. |
| the compile never releases what the job asked | The job-end release covers every job that ends, so this one only frees memory **earlier** — at publication rather than at ending. It matters for a job that compiles and then stalls, which nothing here produces. A map entry is not observable over the protocol. |
| a job that ended keeps its answers | By the time a job ends, every call it made has settled — the compile needed the answers to finish — so there is no running slot to give back and nothing but memory to free. Behaviourally equivalent. |
| a cancel releases before it is committed | The ordering is right and no test forces a commit to fail. Making one fail needs a fault control this build has not got for `commit_context`. |
| an out-of-range choice is read as the first candidate | **Equivalent.** `selection::chosen` returns a position within the candidates, and the candidates are built one-for-one from the ranked spans, so the index is always in range. The guard is defence for a future where the two diverge. |

### Four defects the mutants found, and two they could not

**The first pass was twenty-one mutants; fifteen were killed.** Of its six survivors, four were real defects and two looked equivalent — and one of those two was not, which is the story below.

**Two were defects I had already found by re-reading my own committed code**, and the probe said why nothing caught them: *every test had one item per request*, so no compile ever held two answers at once.

- **An answer released the moment it was read.** A compile needing two calls asks across several ticks; releasing the first when it was first reported means the next tick asks that question again, at a second charge.
- **A key that forgot the path**, so two items citing different files shared one answer.

Both are killed now by a request with two items on two files, asserting **exactly two completions** — fewer means an answer was reused for a question it was not asked, more means one was released and asked again.

**Two were gaps in what was asserted.**

- **The fake's scripted usage was dropped and nothing noticed**, which means nothing tested that a completion **settles to the provider's figure** rather than to the reservation. The reservation is the local bound and over-states by three to four times; settling at it would charge a shared quota for something nobody spent. That is the envelope's whole point and it was untested.
- **Setting `SERVING_CALLS_A_MODEL` back to false broke no test.** Every test about it is an implication — *if the constant, skip* — which is right for rules that stop applying when it flips and leaves **nothing at all constraining the flipped value**. It could have been silently reverted. A flat assertion holds it now: a launch given a model and the permit is `ReadCredentialThenServe`, and a change that removes the call site has to change that test.

**Two looked equivalent.** One was, and one was the worst defect of the milestone:

- *An out-of-range choice read as the first candidate.* `selection::chosen` returns a position within the candidates, and the candidates are built one-for-one from the ranked spans, so the index is always in range. The guard is defence for a future where the two diverge.
- *The compile never releases what the job asked.* That one is equivalent, and for the reason given: the compile step is spliced out of the script when it runs, so nothing asks again.

**The reasoning that said the other release mutants were equivalent too was wrong**, and the next section is what it missed. Being able to argue a mutant is equivalent is not the same as it being one.

### Two more defects, from re-reading the call site

**The pool takes a factory now, not the work.** The work closure was built on every tick and dropped unused whenever the answer was `Running` or `Deferred` — which is every tick but the first. A closure that opens a connection to the store therefore opened one per tick and threw it away, thousands over one call, **paid on the preparation tick this milestone exists to keep short**. `progress` takes `impl FnOnce() -> W` and calls it only when work actually starts, which moves "build nothing until it is needed" out of a comment and into the signature.

**The key is the question, not the file.** It carries the selector as well as the path, because two selectors rank a file differently and `c2` does not mean the same span to both: a shared key would hand the second item a choice made from a candidate list it was never shown. **This suite cannot test that half** and the test says so — `cbr` gives every item of a request the same selector, so two items on one file always ask the same question. Over the protocol an item carries its own.

### The defect the cancellation test found

**One repository permanently cost half the concurrency bound.** A slot counted against the bound while its `settled` answer was `None` — and an answer settles only when somebody *asks*. `ensure_index` asks once, is told `Running`, and by the time the build has finished its caller reads the manifest back and **never asks again**, so that slot stayed "in flight" for the life of the process. With the bound at two, one repository left one slot; two repositories would have meant no model call could ever start.

It was invisible to every test and to every mutant, because nothing until now needed two units of work at once. It surfaced while building the cancellation test below, whose whole point is filling the bound: the second request's call came back `Deferred` with the index build still holding a slot it had finished with seconds earlier.

**The bound counts threads now, not map entries**: a unit whose thread has finished is not in flight, asked about or not. A pool test holds it, and that test fails against the old rule.

One other thing this cost an hour: **`cargo test -p cbr-cli` does not rebuild `cbr-provider`.** An experiment raising `CONCURRENCY` to six appeared to rule the bound out, and had in fact tested the old binary. The workspace has to be built first, which STATE has said since M3 and which is easy to forget when the crate under test is the client.

### Cancellation

`cbr` had **no cancel verb** — the provider serves `context.request.cancel` and the client could not send it, so the milestone's cancellation had no end-to-end test at all. It has one now, and it reads the request's current revision itself rather than asking for it on the command line: the caller is cancelling *this* request, not a particular version of it, and a number they had to look up first is a number they can get wrong.

A job that has ended lets go of everything it asked a model, however it ended — published, out of investigation, or past its deadline — and so does a job whose last request is cancelled. The thread is not killed, because Rust cannot, but **its answer is never read, so nothing it chose reaches a packet**: that is what *a cancelled call leaves no partial derivation record* means here. What it already spent stays in the ledger and must: a call that went out and was charged is a charge, and forgetting it would overspend a quota shared with the owner's other tools.

**Where it is observable, and where it is not.** A settled answer costs a map entry and no more, so releasing one frees memory and nothing a test can see. A call *still in flight* is the case with teeth: it holds one of the two slots the bound allows, and cancelling the request that owns it gives that slot back rather than leaving the next job waiting for work nobody wants. Two barriers hold two calls, a third request is deferred and stays deferred however often it is polled, and cancelling the first lets the third's call happen. The release is also ordered **after** the commit: a cancel that failed to commit is a job still running, and freeing its answers there would have the next tick ask every question again.

The index build is not cancelled, and says why: it writes its manifest as it goes, so cancelling it would leave exactly the partial record the rule forbids, and it has no single owner besides — several jobs may want the same repository at the same tree, and it is idempotent.

### What the flip costs, and the guard that pays for it

While `SERVING_CALLS_A_MODEL` was false, `--permit-model-network` with a valid model and no `--calibrate` was a **refusal**. It is now a launch that reads the owner's Keychain and serves — the exact shape of the lapse recorded below. So the rule moved out of the constant and into `tests/credential_discipline.rs`, which reads the sources and requires every place a test passes that flag to be gated off macOS, to ask for the calibration, or to configure no model. It reads text, so it is a guard against carelessness and not against intent; carelessness is what the lapse was.

The launch tests' macOS gating was reviewed in that commit and stands. The one test that passes the flag with a valid configuration asks for the calibration and is gated; three others pass it with no model configured, which is refused outright.

### A second improvised launch, recorded

**I launched the provider by hand again**, debugging why the call site was not reached: a shell command against the real binary. It read no credential — the configuration named no `model_runtime`, so `launch::decide` returned `Serve` — and it is still the practice [VERIFICATION](../VERIFICATION.md) rule 4 forbids, a month of which is how the first lapse happened. The fix was the one the rule implies: the diagnosis moved into the test, whose failure message now prints what the request last said, and that is what found the real cause.

### Two defects in VERIFICATION's own commands

Found by following them. The conformance block named `conformance/results/m1b` and the rest as output directories, so **running the documented commands overwrites the committed record of the stage that produced them** — `m1b` holds what M1b measured, when CBR declared `core/1` and nothing else, and a rerun replaces it with today's descriptor. The block now runs into a temporary directory and says why. The context command also named `m3a`, where the committed record is `m3a-context`.

### The owner's decision on provider-side retention

Recorded 2026-09-21 and closed in [READINESS §7](m4/READINESS.md): the API offers no request option to prevent retention, and the owner accepts that **for public repositories only** — CBR's own source, Knowscroll-v2 and brian2. **Any private repository still needs the owner's explicit word**, and the absence of a retention control is a reason that bar stays where it is rather than a reason to lower it. A stated limit, not a solved problem.

## Earlier — the m4b follow-up: the review's residuals and the responses dialect

[PR #25](https://github.com/Combraton/cbr/pull/25), **merged** as `dabf033`, pinned to `4aaa31f`, confirmed from `merged: true` and `merged_at: 2026-09-20T19:46:35Z`.

### A credential read that should not have been possible, and the fix

**I read the owner's Keychain entry outside the authorised calibration.** Probing the binary's careless-launch refusals from a shell, I used a configuration that named a valid model together with `--permit-model-network`. That is a *serving* launch, which the build accepted, so it read the key.

**The cause, named precisely: improvising a launch by hand.** Not a gap in a test, not a missed review — a command typed at a prompt to see what a refusal looked like, against the real binary with the real configuration. Everything that has ever protected that key is in the test suite or in an authorised run, and neither was in play. [VERIFICATION](../VERIFICATION.md) now carries that as rule 4 beside the three about what the code permits, because the fix that followed closes this particular door and the habit is what closes the next one.

What it did not do, verified afterwards: no model call (`model_calls` empty), no ledger row, nothing sent — the transport is constructed only by the calibration path — nothing printed, and no credential-shaped bytes anywhere in the store. The bytes were read into memory and zeroed on drop. It was still a read that was not authorised, and it is recorded here rather than tidied away.

**The fix is the one the lapse points at.** While [`launch::SERVING_CALLS_A_MODEL`](../../crates/cbr-provider/src/launch.rs) is false, a serving launch with a model configured is **refused**: this build has no call site to spend a credential at, and [READINESS §2](m4/READINESS.md) forbids reading one into a process that has no use for it — *"a prompt, an audit entry and a secret in a process that had no use for one"*. So **the calibration is now the only launch that reads a credential at all**, and it needs three flags and a ceiling. The constant is set true by the commit that connects selection to the transport, and the tests that constrain that row are implications, so they relax then rather than having to be found and deleted.

### The three residuals, and the error shape

1. **A double-percent-encoded nested URL leaked.** `%253A` decodes to `%3A`, which decodes to `:` — one round of decoding saw a string that still looked like nothing. Decoding now runs **until the text stops changing**, under a bound of five; a value still changing at the bound is replaced rather than written, because its meaning has not been seen. The property test gained the doubly-encoded shape.
2. **`model_runtime` was a denylist** of five names, and `url`, `api_base` and `proxy` walked past it — the shape named one review earlier, in the same file it was named in. It is an allowlist of three now, and **the top-level configuration had the same shape**: unknown members were read past in silence, so a misspelling was a setting the operator believed they had made. Also an allowlist. Worth saying: the protocol's own `launch-config.schema.json` is `additionalProperties: false`, so CBR's denylist was weaker than the specification it implements.
3. **The launch decision is a pure function** now, `launch::decide`, with every row tested without starting a process — see [VERIFICATION](../VERIFICATION.md), rule 3.
4. **The parser learns the error shape the live service returned**, and the rule is general: **a body that carries an error member is a failure whatever the HTTP status says, in every dialect.** `"error": null` is not one, because the Responses API carries it on every success.

### Two things about my own instruments

- **The mutation probe was running only the binary's unit tests.** Two allowlist mutants came back "survived" that were in fact killed by integration tests in `tests/model_launch.rs`. Implausible survivors are what caught it. Every mutant in this change was re-checked against all targets.
- **The conformance command block in VERIFICATION had no `composition` line**, and none for `context`. Improvising with the stdio participant skips all fourteen composition fixtures, which reads like a regression and is not one. Over the socket participant it is 3 passing and 11 permanently unsupported, matching the expectation. Both commands are now in the block.

### The responses dialect, and the design change it forced

**The counting endpoint counts a Responses-shaped request**, confirmed against the provider's published API reference read on 2026-09-21 under the reviewer's authorisation. A count of one serialization says nothing about the cost of another, so the count only means something if the completion goes to `/v1/responses` with the same `input`, `instructions` and `tools`. The **Responses API is now the primary wire** ([ADR 001 question 13](../decisions/001-standalone-v0.1-scope-and-stack.md)); chat-completions and Anthropic stay exactly as built, as secondary, and **neither is counted by an endpoint that does not describe it** — for those the local bound alone admits, which is what it was built to be able to do.

Three things followed, each with its own test:

- **The generation limit binds to `max_output_tokens`** there. m4b noted that asking the dialect cost nothing while every answer agreed; one milestone later they do not, which is the case the guard was written for.
- **Reasoning tokens are output tokens and cannot be disabled on the M2.x models**, so a sixteen-token limit can be spent entirely on reasoning and end `status: incomplete` with no text. That is an ordinary outcome carrying a real cost, reported with its usage rather than as a failure — and reasoning is its own output item, never read as the answer.
- **`service_tier` is sent as `standard` explicitly and `priority` never.** It is the owner's quota.

**Two of the reviewer's pointers did not survive checking**, and are recorded as corrections rather than carried forward: the counting endpoint is **not** documented as unbilled or quota-exempt — the page says nothing about billing — so its cost stays reserved and settled like any other send until a live run says otherwise; and **`store` is a response property, not a request one**, so CBR sends none and whether the provider retains repository text is now an open question for the owner in [READINESS §7](m4/READINESS.md).

**Fixtures now carry three labels** — `documented`, `observed`, `guessed` — with each source page and its retrieval date cited. m4b had one label, which was honest about confidence and silent about provenance, and that silence is what cost a calibration run.

**The calibration gained its second comparison**: the counting endpoint's prediction for the completion's request against the `usage.input_tokens` the provider then charged for the same request. The first says whether CBR's bound is sound; the second says whether making the count is worth anything. A charge above the prediction by more than 2% is a **finding**, never a stop — the stop condition is the local bound being wrong, and the two must not be confused.

### Mutants

| Mutant | Outcome |
|---|---|
| The status alone decides · a null error member read as an error | killed (2) |
| Decoding stops after one round · an unsettled encoding written out anyway | killed (2) |
| The `model_runtime` allowlist back to a denylist · the top-level allowlist removed | killed (2) |
| A serving launch reads a credential with no call site | killed |
| A truncation is not repairable again · the repair re-asks in the same space · the output floor is removed | killed (3) |
| The count made when the local bound already admits · every refusal buys a count · a per-request refusal buys a count · which path admitted is not recorded | killed (4) |
| The tripwire removed · a tripped wire does not stop later calls · the tripwire fires on ordinary calls | killed (3) |
| A truncated completion stops the run again · a refused completion is not a stop · the input usage dropped from the cost | killed (3) |
| The responses limit binds to `max_tokens` · every dialect treated as counted · the count body carries the completion-only members · the service tier is `priority` · an incomplete status read as complete | killed (5) |
| Reasoning read as the answer | **survived**, and the mutant was equivalent: both arms did nothing. Rewritten as a mutant that really assigns the reasoning to the answer, and the test rewritten too — it had used the `incomplete` fixture, which ends `Truncated` whatever the parser does with its reasoning. Now killed. |
| `reads_credential` forgets the serving decision | **equivalent while `SERVING_CALLS_A_MODEL` is false** — that decision is unreachable today, so nothing can observe it. `both_live_decisions_are_reachable` demands it the moment the constant is true. |

## The calibration — run 1, and what one live call found

**Authorised by the owner at the m4b review, run once from `main` at `025ddfb`, and it stopped at its first call.** The record is [m4/CALIBRATION.md](m4/CALIBRATION.md). One run, no retries: it is not re-run without a further authorisation.

**No token count was obtained, so the byte bound is neither confirmed nor falsified.** READINESS §10's stop condition is a provider count above its local estimate; there was no provider count. **M4 is not stopped, and it is not cleared either.**

The counting endpoint rejected the body: `binding: expr_path=input, cause=missing required parameter`, code `invalid_prompt`. The endpoint exists and validates — `POST /v1/responses/input_tokens` is the right address — and the body CBR sent was the wrong shape: a chat-completions body keyed on `messages`, where the endpoint wants `input`.

**The lesson is about the labelling, not the guess.** m4b graded its fixtures by confidence and named the count *response* as the least verified thing in the module. It said nothing about the count *request*, which was equally a guess and is the one that failed. A labelled guess costs a line in a README; an unlabelled one costs the run.

What one live call did establish, on the first try and against a real server: the Keychain read, the permit, the pinned host, TLS, the serialized request, the recording boundary and the ledger's conservative settlement all behaved as built. The credential appears nowhere in the store — 0 matches for `bearer` or `authorization` across the database, its write-ahead log and its shared-memory file — because the header is not recorded at all.

One row in the ledger, settled `unknown` at its estimate of 12,495 tokens. The provider almost certainly charged nothing, since the request never passed validation, so that row over-counts — which is the direction the envelope exists to err in.

## This change — M4b, the wire, beginning with three mutants the suite did not kill

[PR #24](https://github.com/Combraton/cbr/pull/24) against `main`, for [issue #21](https://github.com/Combraton/cbr/issues/21). **No live call has been made, and none can be made by this suite**: the transport cannot be constructed without a permit that only `--permit-model-network` produces, and no test has one.

### The first commit: three constants nothing was reading

The review found one mutant surviving at m4a's head — removing `SAFETY_MARGIN_TOKENS` from the completion's reservation — and asked whether the same held inside `budget::estimate`. **It held for all three.** Each was documented, each was named in a constant, and each could be deleted with the whole workspace staying green:

| Mutant | Before | Now |
|---|---|---|
| No margin in the completion's reservation | survives, workspace green | `the_completions_reservation_covers_the_margin_as_well_as_the_generation` |
| No margin in `budget::estimate` | survives, workspace green | `the_estimate_reserves_generation_and_margin_above_the_input_bound` |
| No per-message overhead in `budget::estimate` | survives, workspace green | `the_estimate_grows_with_the_messages_it_frames` |

**This is the fourth instance of the same shape in two milestones**: a rule written down, named in a constant, documented in prose, and read by no test. The m4a reservation test asserted the completion's figure was "larger than the count's", which stayed true without the margin. The estimate's two terms were asserted by nothing at all.

"Red first" here means **red under the mutant**: the production code was already correct and what was missing was the test, so the demonstration is that each new test fails when its constant is removed and passes when it is restored. All three red runs are in the pull request.

One of the three tests had to be rewritten after clippy called it out: `MESSAGE_OVERHEAD_TOKENS > 0` is a constant assertion, and the equality beside it (`delta == 4 * OVERHEAD`) is satisfied by `0 == 0`. The statement that actually holds is **strictly greater**: four messages cost more than none.

### The READINESS contradiction, resolved

§1 said m4b makes no live call; §3 called the tokenizer comparison "m4b's first measurement", which needs one. **m4b is fixtures only.** The comparison becomes **the calibration**, its own step after m4b is reviewed and merged and only on the owner's explicit word at that time, with its protocol written down now ([READINESS §10](m4/READINESS.md#10-the-calibration-and-what-stops-m4)): token-count calls over CBR's own public sources plus one non-Latin text, one completion capped at 16 generated tokens, a hard run ceiling of 100,000 tokens through the ledger, every call recorded and redacted, and a table of local estimate against provider count per file. **One provider count above its local estimate means the bound is unsound: stop, report, and nothing else in M4 proceeds.**

The two forward references that said "m4b's first measurement" — in `budget.rs`'s module header and in m4a's limits — now say what is actually true: byte-level is an **assumption about the provider's tokenizer**, and the calibration is what checks it.


### The credential, and ADR 001 question 12

The owner resolved the constraint [READINESS §2](m4/READINESS.md) left open. **"Never passed to a child process" means the secret is never *handed to* a child** — by argument vector, environment or standard input — and reading it back from one over a private pipe is not that. So the mechanism is the `security` tool, and the decision is recorded with both reasons beyond the dependency count, the rejected alternative, and what would reopen it ([ADR 001 question 12](../decisions/001-standalone-v0.1-scope-and-stack.md)): Keychain access control is **per program**, and `cbr-provider` is unsigned and rebuilt constantly, so an in-process read means a prompt after every rebuild or an *Always Allow* on an unsigned binary; and it is how the owner's other tools already read this same key. A signed, notarised release binary reopens it.

Every line of the mechanism has a test, and **every test runs against an injected fake tool**, so no test on any machine reads the owner's key. The launch member `model_runtime` is what "a model is configured" means, and it is validated — provider, dialect, model, `https` endpoints — **before** the credential is read, so a configuration mistake is refused identically on every machine and the Keychain is never touched to discover that the launch was never going to work.

### The wire

Both dialects' serializer and parser over one transport. Four things worth naming:

- **The generation limit is bound, not mentioned.** m4a's check asked whether the figure appeared anywhere in the serialized body, which a body capped at 4,096 whose prose says "about 16 spans" satisfies. It now parses the body and reads the member the dialect's provider reads. The dialect is asked even though both surfaces name it `max_tokens` today, and a test says so rather than hiding it.
- **Two dialect differences that degrade silently rather than erroring**, each with its own test: the OpenAI wire carries tool arguments as a **JSON string** that has to be read a second time, and the Anthropic wire reports usage as **two halves** that have to be added.
- **A `<think>` marker is refused, not stripped.** Stripping would put CBR in the business of deciding which half of a response was the answer. The usage is still read off it, because a refused answer is still a charge.
- **A second JSON reader, for untrusted bytes.** `cbr_encoding::parse` implements the protocol's value domain and is right to refuse `{"temperature":0.7}`; a provider's response is not a protocol value, and refusing one over a number CBR never reads would turn a good answer into a bounded repair that spends real tokens. What CBR **seals** still goes back through the protocol's domain, so the strictness stays at the point of recording.

**Prose where a structure was asked for, and text where a tool call was demanded, are the only repairable outcomes**, because they are what this provider documentedly does. The bound is one repair, it is a whole call admitted and charged against the same ledger, and it does not send the model's own answer back — repository text is untrusted and so is what a model makes of it.

### Redaction, and why the types carry it

A provider response can carry a **third party's live credential**: one pilot repository has already seen a speech response return a presigned object-store URL with the key and signature in its query string. Redaction runs between the transport and the store, and `record` takes a `Redacted` whose only constructor is `redact`, so **"redaction moved after the write" is a compile error rather than a test failure**. The scan test opens a real store file and reads back every byte of every file it left behind, write-ahead log included.

### The transport, and the dependencies

`Http` cannot be built without a `net::Permit`, which only `--permit-model-network` produces. A configured model is deliberately not enough. m4a's `the_crate_has_no_network_dependency_in_its_tree` had to be made to fail; it became **two** tests rather than none — the four crates that need no network client still have none, and the one that does names in code the exact 26 packages it added. Every dependency is listed with its version, licence and reason in [STACK §8.2](readiness/STACK.md#82-what-the-transport-added-and-why-each-one-is-there), with one named cost: the trust anchors are compiled in and do not track the operator's own trust store.

### What m4b does not establish

- **No fixture here has been compared with the live service.** Every one is hand-written from public documentation, nothing was fetched from the provider to make them, and the word `unverified` is in each file name, each constant name and the directory's README. A passing test says CBR reads what CBR *believes* the provider sends.
- **The count response's shape is a guess.** The parser accepts a small closed set of member names and refuses a body carrying none of them, which leaves the local estimate standing. It is named as the least verified thing in the module.
- **Two status codes are mapped to exhaustion and that mapping is a guess.** Anything else non-zero degrades to "something failed", never to success.
- **The transport now has a call site**: the calibration, built here on the reviewer's instruction — *if the first live call needs new plumbing, the first live call runs code nobody reviewed*. It has not been run. The allowances that call site makes unnecessary are gone; what remains is `Role::Assistant` and two `Want` variants, each scoped to `not(test)` and each exercised by the serializer's and parser's tests, waiting for m4c's loop to construct them.
- **Still no token count of CBR's has been compared with the provider's.** That is the calibration, [READINESS §10](m4/READINESS.md#10-the-calibration-and-what-stops-m4).

### Defects of my own, and one vacuous test

Three, all caught by tests rather than by reading:

1. **The Keychain read could still hang.** Killing the tool does not close its pipe if it left a grandchild holding it, and the first version joined the reader unconditionally after the kill — blocking for the grandchild's whole lifetime, which is the hang the timeout exists to prevent arriving one step later. Every wait is now bounded by the same deadline and a reader still running at it is detached.
2. **Redaction did the presigned-URL case by halves.** The first version consumed the text as it went, so the "is this inside a URL" test only saw the bytes since the last separator: it redacted the first secret parameter and then could no longer tell it was in a URL, leaving every parameter after it in the clear.
3. **Two of my own mutants survived, and both were the test's fault.** The proxy test asserted "no proxy" without setting one, so it held whether or not the code asked for it. The count-endpoint test used only the OpenAI endpoint, which is a prefix of the counting endpoint, so a transport deriving the count URL from the dialect passed. Both tests are fixed and both mutants now die.

**One vacuous test written, then deleted rather than patched.** It took a copy of the header bytes before zeroing and asserted the copy was non-zero, which cannot fail; it also duplicated a test that already existed. This is the same failure the clippy finding in the first commit was, arriving by hand rather than by lint, which is worth recording because the lint will not always be there.

### The review's two defects, and the shape they share

**This is the fifth and sixth instance of the same shape**, and it is worth naming precisely because the earlier four looked different from each other: *a rule enforced at the level of a string that somebody else controls the spelling of.*

- **The endpoint host was not pinned**, so "MiniMax only" was a label. Configuration accepted provider `minimax` with an endpoint at `collector.example`, at the lookalike `api.minimax.io.collector.example`, and at the userinfo form `api.minimax.io@collector.example` — each of which names the provider correctly and addresses somebody else, and each of which would have sent the owner's credential and repository text there. The rule was checked against a **string an operator's mistake or a planted configuration file writes**.
- **Redaction read raw text**, so a JSON-escaped URL (`https:\/\/…\u0026Signature=…`) hid its own shape from the "is this a URL" test, and a percent-encoded URL nested in a parameter that named no secret hid inside one. The rule was checked against a **string the serializer controls the encoding of**.

The earlier four were the same thing in other clothes: a rule written down and read by no test; a golden digest over a fixture too small to reach what it guarded; a platform-gated claim whose mutant could only die somewhere else. **The common fix is not more cases — it is to move the check to a level the other party does not choose.** The host is now parsed rather than matched, and configuration supplies no endpoint at all; redaction runs over decoded values and re-serializes, so an escape has nothing to hide behind.

Two of the guards are now **types** rather than tests, joining `Redacted`: a `PinnedUrl` whose only constructor checks the host, and the `net::Permit` the transport needs. Fabricating either outside its module is a compile error.

### What could not be killed, and why

Three mutants survive and are recorded as survivors:

- **`url_to_send` not pinning what it built**, and **the transport building its own URL** — because the URL is made of constants, so removing a check on it changes nothing a test can observe. The check exists for a future code path that builds one some other way, which is what the reviewer asked for; `PinnedUrl` makes bypassing it a visible rewrite of the call site rather than a deleted line, but it does not make it a test failure.
- **Either `Drop` body emptied** (`Secret`, `Authorization`, `Scrubber`) — nothing in safe Rust can observe a released heap buffer. What is killed is the guard doing nothing.

One mutant is **equivalent and therefore not counted**: comparing the calibration's stop condition against the *believed* count rather than the *reported* one. A count above the local bound is necessarily above the implausibility floor, so the disbelief rule never fires on the case being looked for. The distinction is real for the **table** — a count below the floor must be recorded as the provider's own figure, or the run hides exactly how loose the bound is — and that has its own test. The first version of this module claimed the distinction mattered for the stop condition; it does not, and the comment was corrected rather than left as a plausible-sounding reason.

### Mutants

| Mutant | Outcome |
|---|---|
| The environment is not cleared for the Keychain tool | killed |
| The zeroing guard does not zero | killed |
| Multi-line tool output accepted as a password | killed |
| A non-zero exit from the tool ignored | killed |
| The trailing newline kept as part of the secret | killed |
| The tool found through `PATH` | killed |
| A fallback to the environment added | killed |
| `reasoning_split` not set | killed |
| `stream: false` dropped | killed |
| **The generation check reverted to m4a's textual search** | killed |
| `framed_messages` forgets the system instruction | killed |
| The Anthropic system instruction sent as a message | killed |
| The think-marker refusal removed | killed |
| The think marker checked only as a prefix | killed |
| The Anthropic usage read as the input half only | killed |
| The OpenAI tool arguments not read a second time | killed |
| A non-zero `base_resp` status ignored | killed |
| Disagreeing count members take the first | killed |
| Redaction does nothing | killed |
| Secret names matched exactly rather than by suffix | killed |
| **Redaction moved after the write** | **compile error** |
| The repair bound raised to eight | killed |
| The repair echoes the model's own answer back | killed |
| An unrepairable outcome repaired anyway | killed |
| An error status turned into an error before the body is read | killed |
| Every timeout treated as nothing having been sent | killed |
| HTTP 429 not read as exhaustion | killed |
| A failure drops the usage the provider reported | killed |
| A permit available without the gate | killed |
| The agent takes a proxy from the environment | survived, **test fixed**, now killed |
| The count sent to the dialect's own endpoint | survived, **test fixed**, now killed |
| Host checked as a prefix · the userinfo check removed | killed (2) |
| The JSON path removed · the lossy path passing through · the exact-match scrub removed · the percent-decode of a URL value removed · the fail-closed check after scrubbing removed | killed (5) |
| The calibration's stop condition removed · the table recording the believed count · the completion not capped at sixteen · the non-Latin text dropped from the corpus | killed (4) |
| `url_to_send` not pinning what it built · the transport building its own URL | **survive**, and cannot be killed: the URL is a constant, so removing a check on it changes nothing observable. `PinnedUrl` makes bypassing it a compile error for the fabrication route and a visible rewrite for the other. |
| `supported()` always true | **survived on macOS at first**, and would have died only in CI — a kill nobody here could observe. The platform rule is now a function of an operating system's *name* rather than of the machine running the tests, so it is **killed everywhere**, observed locally. |
| Either `Drop` body emptied (`Secret`, `Authorization`) | **survives, and cannot be killed** — nothing in safe Rust can observe a released heap buffer. What is killed is the guard doing nothing. |

## Earlier — M4a, the envelope before any transport

[PR #23](https://github.com/Combraton/cbr/pull/23) against `main`, for [issue #21](https://github.com/Combraton/cbr/issues/21), scoped exactly as [READINESS §1](m4/READINESS.md) sets it out. **No network code, no credential read, and no HTTP or TLS dependency anywhere in the workspace** — a test walks `Cargo.lock` from `cbr-provider` and fails if one becomes reachable.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 25 files, 163 links, 27 heading anchors, 0 errors; 433 and 420 match |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **221 tests** (207 at the review's head `499ad80`, 181 at the readiness PR) |
| `git diff --check` | 0 | — |
| all seven suites + `check_results.py` | 0 | `stream` 24/24 · `core` 130/5/0 · `socket` 11/2/0 · `evidence` 16/0/0 · `knowledge` 10/0/0 · `context` 11/0/0 · `composition` 3 pass, 11 unsupported — **unchanged** |

**The gate, before the code — and where that discipline slipped, and what it cost.** The thirteen `budget` tests were written and run against a module whose every function was `unimplemented!()`: **12 failed, 1 passed**. The one that passed is the no-network guard, which was true before the code existed and is a regression guard rather than a gate; it is not counted as one. **The eight `model` tests were not run red**: they were written in the same step as the module beneath them, which is the discipline m3c set and this round did not keep.

**The review then found three defects, and two of them were in `model.rs`** — the module whose tests never ran red. That is not a coincidence worth explaining away. A test written after the code it tests is written to agree with it: the completion reserving the provider's input count alone, and a failure after the send settling to zero, were both things the code did and the tests were shaped around. The thirteen `budget` tests, written first, found no such defect. **The eleven tests for the three findings were written and run red first**, and their red run is in the pull request.

### The three defects the review found

| Defect | What the probe showed | The rule now |
|---|---|---|
| **The completion was admitted against the provider's input count alone.** `admit(.., refined.max(1))` dropped the reserved generation and the margin the local estimate carried, and trusted whatever figure came back. | Window nearly full, count answered 1, completion spent 50,000: the ledger row read `estimate 1, usage 50,000`, admitted. The comment above that line said a provider reporting less than it charges does not widen the envelope; the code did exactly that. | The reservation is the refined input count **plus the generation the request asks for plus the margin**. The request must declare that limit or it is never sent. A count below an eighth of the local bound is a recorded **anomaly** and the local figure stands. Usage above the reservation is a recorded **divergence**. |
| **A failure after the send settled to zero.** | A 100,000-byte body, count fine, completion times out: the ledger recorded 0. The provider may have charged. | Split by what is known: **nothing left the process** spends nothing; **sent with no usage reported** keeps the reservation's estimate, as a kind of its own; **a reported usage** settles to that. The transport's answer says which, and `Answer` carries that distinction now, before m4b builds a real one on it. |
| **The crash matrix never reached the completion.** One scripted answer was consumed by the count, a `Completed` answered to a count fell into the failure arm, and the process paused at the count's first boundary in all three rows. | The scripted run ended `Unmet(model_call_failed)` with one send, and the scripted usage was never recorded. | Both answers are scripted; the boundaries are **named per call**; there are **six rows**, and each asserts **which call's reservation is on disk, by request id**. A mismatched answer is its own recorded failure, not a fall-through. |

Two smaller things with them. **Admission takes `BEGIN IMMEDIATE` across the check and the write**, because m4c adds concurrency and a check-then-write race is an overspend; the test observes it by holding the write lock on another connection. And a **run-level ceiling** from the launch (`--model-run-ceiling`) sits beside per-request and per-job: it is checked **after** the two counters, so it can only lower the owner's envelope, never raise it. m4e's five-million cap is that, enforced rather than intended.

### The local estimate is a byte bound, and why

**The byte-based upper bound, not the published tokenizer.** A byte-level BPE emits at most one token per byte of its input — every token decodes to at least one byte and the tokens tile the input — so the UTF-8 byte length of the serialized request is an upper bound on its token count, in any script and under any vocabulary. That is a property, not a measurement, which is what makes it safe to build on without a provider.

The alternative was MiniMax's published `tokenizer.json`, vendored with its digest and licence. It needs fetching once, and READINESS rules out a download at build or run time; this session also has no authorisation to fetch from the provider. The bound is loose — three to four times the real count for English prose — and loose-and-sound was the right trade against tight-and-unverifiable.

**The property is one-sided and pinned on a corpus:** it may over-estimate, it must never under-estimate. The corpus is the crate's own sources, read at test time so it grows with the repository, plus a fixed non-Latin text in Devanagari, Japanese and Chinese. The reference is the worst case a byte-level BPE can emit, obtained by arithmetic rather than measured. A second test states why the bound is bytes rather than characters: on that non-Latin text a character count is **smaller** than the byte count, so a character-based estimate would under-count exactly where it matters.

### Two-step admission, and the ledger

`POST /v1/responses/input_tokens` carries the fully serialized request, so **the count call is a send** and is treated as one everywhere: admitted against the ledger, recorded, and its own cost debited. The local estimate runs first and alone can refuse, and when it does nothing leaves the process. `Recorder`, the only transport in this build, counts a count call among the sends it saw.

The ledger is in the store, under the same WAL, `synchronous=FULL` and `BEGIN IMMEDIATE` discipline as the records it guards, and migrates with them. A reservation is written **before** the send and the same row **becomes** the settlement, so there is never a moment when both are counted and never one when neither is. Time comes from the existing clock control: the rolling five hours and the calendar month are tested by moving the clock, never by sleeping.

### Crash matrix, the envelope's three rows

Driven by the `model.fake` control, which a production configuration refuses like every other — it makes the provider perform one call through the ledger at startup, pausing at a barrier, so the boundaries belong to a process a test can kill at them. **The call selects nothing and changes no packet**; it exists so that these rows are reachable.

| Row | Boundary | After the kill |
|---|---|---|
| Reserved, nothing sent | `model.after_reservation` | the reservation survives and is still a reservation |
| Sent, not reconciled | `model.after_send` | the reservation still holds its estimate, which over-counts |
| Inside reconciliation | `model.during_reconciliation` | counted exactly once — never twice, never neither |

The property is the same after all three and is deliberately one-sided: **the spend is counted at least once and is never zero.** A ledger that forgets a spend overspends someone else's quota; one that counts it twice only refuses a call it could have allowed.

**Negative control 1, at m4a rather than at m4e:** with no model configured the golden packet digest is unchanged — the existing test says so — and the ledger stays empty, which a second test says. A row written without a model would mean a call nobody asked for, which is what "background spend is zero" forbids.

### Mutants

**Eighteen, all killed, all observed, each run alone — nine for the first round and nine for the review's three findings. Two of the first nine survived their first run and are recorded as survivors.**

| Mutant | Killed by | At |
|---|---|---|
| The local admission check removed | `a_request_the_local_estimate_refuses_never_reaches_the_count_endpoint` | the fake transport asserting it was never called |
| **The provider count reached for a request the local estimate refuses** | the same test | the same assertion — the count endpoint is a send |
| One of the two counters dropped | `each_counter_refuses_on_its_own` | a window-exhausted case the monthly counter alone admits |
| **The reservation written after the send** | `every_crash_boundary_is_reached_in_order`, **after it was given something to assert** | "a reservation is written before its send, never after it" |
| Counters lost on restart | `an_unsettled_reservation_is_still_counted_after_a_restart` | the spend read back from a reopened store |
| **Provider exhaustion reported as `budget_exhausted`** | `provider_exhaustion_is_a_different_outcome_from_an_exhausted_envelope`, **after the reason was given one source** | the two reasons compared |
| The estimate under-counts, characters rather than bytes | `the_estimate_never_falls_below_what_any_byte_level_tokenizer_could_emit` | the non-Latin text |
| A refusal writes a reservation anyway | `a_refusal_writes_no_reservation_and_is_itself_recorded` | "and nothing was reserved" |
| The no-network guard does not walk the tree | `the_crate_has_no_network_dependency_in_its_tree` | `rusqlite` added to the forbidden list, which must then be found |
| Reserve the input count alone | `the_completion_reserves_its_generation_and_margin_not_the_input_count_alone` | the completion is admitted where it should be refused |
| Every failure settles to zero | `a_failure_after_the_send_keeps_the_estimate…` and `…settles_to_what_it_reported` | the spend the provider may have charged |
| An implausible count is believed | `a_count_implausibly_below_the_local_bound_is_an_anomaly…` | no anomaly is recorded |
| The generation-declared check removed | `a_request_that_does_not_declare_its_generation_limit_is_never_sent` | a body without its limit leaves the process |
| A divergence is not recorded | `usage_above_the_reservation_is_recorded_as_a_divergence` | spending more than was reserved goes unremarked |
| Admission without `BEGIN IMMEDIATE` | `the_check_and_the_write_are_one_transaction` | a second writer is not kept out |
| The run ceiling checked before the envelope | `a_run_ceiling_lowers_the_envelope_and_never_raises_it` | a ceiling above the envelope would raise it |
| A mismatched answer falls through to failure | `an_answer_of_the_wrong_kind_is_a_recorded_failure…` | it is reported as a transport error |
| Both calls share one barrier name | `the_boundaries_are_named_per_call…` | a row could pass at the wrong boundary |

**Why the two survived, in one sentence each.** The boundary test **collected the names of the boundaries and asserted nothing at them**, so a build that reserved after it sent still passed: it now asserts what is true at each one, which is that no send has happened when a reservation is written. And the reason a provider-exhausted call reports was **written twice** — as a match in `budget.rs` and as a literal in `model.rs` — so mutating one left the other answering; that is the milestone's own recurring shape in a new place, and it is fixed by making `model.rs` ask `budget.rs` rather than repeat it.

### What m4a does not establish

- **No token count here has been compared with the provider's.** The estimate is sound by construction and unmeasured in practice, and **byte-level is an assumption about the provider's tokenizer rather than a fact**. The check is **the calibration** — its own step after m4b merges, on the owner's word ([READINESS §10](m4/READINESS.md#10-the-calibration-and-what-stops-m4)) — and until it has run the bound's looseness is a guess.
- **Nothing has been sent anywhere.** The only transport is a recorder, and the only thing that reaches it is a test control a production configuration refuses.
- **The ledger has never held a real spend.** Every number in it so far was written by a test.
- **The fake call selects nothing**, so nothing here shows a model improving a packet — or a packet surviving a model. That is m4c onward.

## Earlier — M4 readiness, docs only

[PR #22](https://github.com/Combraton/cbr/pull/22) against `main`, for [issue #21](https://github.com/Combraton/cbr/issues/21). **No code, no transport, no credential read, no model call.** M4 is the first milestone that spends the owner's quota and sends repository text to a third party, so it starts with [`docs/work/m4/READINESS.md`](m4/READINESS.md) and nothing else, reviewed before any of that exists.

The readiness document states the scope and the PR split, restates the owner's standing decisions as constraints **with the place each is enforced**, puts the two-counter envelope in code before any transport, treats the four measured provider behaviours as design inputs rather than discoveries, makes every call an evidence artifact redacted at the recording boundary and replayable offline, bounds what may be sent to the view and readable claims the command already resolves, takes model work out of the preparation tick where M3 measured a 3.9–12.8s stall, and states how M4 is judged — J1 with a model, both sealed pilot questions rerun, with the negative controls and expected mutants written down in advance.

**Both pilot rescores are recorded with it.** brian2's rerun **failed again, identically**: one of three required facts, no trap, baseline holding none. Knowscroll's rerun **passed, three of three**, with the caveat that the change which moved it was proposed by the reviewer, who holds the oracle — so it confirms a general fix was general and is **not independent evidence of usefulness**. brian2's unchanged failure is what shows nothing was tuned to pass. Both questions stay sealed and run again at M4.

### The review round, and the seven additions

The review's largest catch is one this session had not seen: **`POST /v1/responses/input_tokens` is itself a send.** Counting there first serialises the request and puts it on the wire *before* admission has decided whether it may go — spending whatever the count costs and defeating the check it was meant to serve. Admission is therefore two steps: a **local conservative estimate that alone can refuse**, and the provider count only for a request that step has already admitted, itself under the view rule, itself recorded, its own cost debited. That is why m4a now has a complete admission path with **no network at all**, which is what CI exercises.

The other six: the counters are **durable and conservative** — in the store, surviving restart, a reservation written before the send so a crash leaves the spend counted rather than forgotten, over CBR's own rolling five hours, with provider-reported exhaustion a **distinct** typed outcome because the quota is shared and CBR only sees its own spending; **repository text is untrusted input**, so model-assisted selection chooses only among candidate ids CBR offered and never introduces a path, span, citation or label, with negative control 5 planting a file that tells it to; **a derivation record holds repository excerpts**, so it is readable only under the job's own view and is never reachable through a packet by a reader who could not read its contents; **the Keychain is touched only when a model is configured**, which CI never is, with the read mechanism a stated m4b decision and its tradeoff written down now; **m4e has a hard cap of 5,000,000 tokens** enforced by `--model-run-ceiling` given to each launch as the cap less what the launches before it spent — not, as this said until the round-40 review, by the per-job ceiling, which bounds one job and so bounds one run of six — with a stop at half again over the estimate; and fragment validation becomes a mechanism.

### The fourth instance of a rule kept by memory

`check_docs.py` did not validate anchors, so two heading changes at m3e left three links pointing at headings that no longer existed. **This session caught them by hand** — which is precisely the failure this milestone has now recorded three times before: the rule was known, applied to the case in front of it, and not made structural.

| Round | The rule that existed | Where it was not applied |
|---|---|---|
| m3c review 1 | a packet's provenance names its compiler | the publication path passed the constant unconditionally |
| m3c review 2 | resolve the readable set at the command | claims, added three commits later, did not |
| m3d | a change to how an index is built changes `COMPILER` | the chunker changed and the constant did not |
| **M4 readiness** | **a link into a heading must name a heading that exists** | **two headings were renamed and three links were fixed by memory** |

`scripts/check_docs.py` now resolves fragments in Markdown links, same-file and cross-file, by GitHub's slug rule — including the case that broke, where an em dash is dropped as punctuation and the spaces either side each become a hyphen, so the anchor carries a doubled hyphen nobody would type. **The rule is itself checked on every run** against `scripts/testdata/anchor-slugs.md`; there is no flag to skip it. Two mutants, both observed: a renamed heading with the link left behind is caught as an error and exits 1, and collapsing whitespace in the slug rule fails the self-test on three of its seven cases.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 25 files, 162 links, **27 heading anchors**, 0 errors; 433 and 420 files match |
| `cargo fmt` · `clippy --all-targets -- -D warnings` · `build --locked` · `test --workspace --locked` · `git diff --check` | 0 | **181 tests**, unchanged — this pull request touches no crate |

## Earlier — M3, complete

M3 closed on 2026-09-20. [Issue #15](https://github.com/Combraton/cbr/issues/15) is closed and the record is [`docs/work/m3/CLOSEOUT.md`](m3/CLOSEOUT.md). Five pull requests, each pinned and confirmed: #16 `cbfaebe` · #17 `c3cdf51` · #18 `2cf6b97` · #19 `17cc54c` · **#20 `c726107`, pinned to `47fd574`, confirmed from `merged: true` and `merged_at: 2026-09-20T09:19:03Z`**, second parent `47fd574`. **PR #22 (M4 readiness) as `9092439`, pinned to `bf4694a`, confirmed from `merged: true` and `merged_at: 2026-09-20T09:56:46Z`**, second parent `bf4694a`. **PR #23 (m4a) as `7a7398d`, pinned to `081b6f0`, confirmed from `merged: true` and `merged_at: 2026-09-20T11:25:01Z`**, second parent `081b6f0`.

## Earlier — M3e, what the pilots found

[PR #20](https://github.com/Combraton/cbr/pull/20) against `main`, and the M3 close-out with it. Four changes, each general rather than fitted to a pilot's question, each with its test written and run **before** the code and its mutant after. **These are compiler changes made after a pilot run, so both pilots were rerun in full** — same questions verbatim, same selectors — and rescored.

**The gate, before the code.** All four tests were written against the behaviour as it stood and run: **0 passed, 4 failed**, each at the assertion naming the rule it broke. They are in `crates/cbr-cli/tests/pilot_findings.rs`, every one on a repository the test writes rather than on either pilot.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 23 files, 0 errors; 433 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **181 tests** (181 at m3d, 132 on `main` before M3) |
| `git diff --check` | 0 | — |
| all seven suites + `check_results.py` | 0 | `stream` 24/24 · `core` 130/5/0 · `socket` 11/2/0 · `evidence` 16/0/0 · `knowledge` 10/0/0 · `context` 11/0/0 · `composition` 3 pass, 11 unsupported — **unchanged** |

### The four changes

1. **Coverage names modified tracked files.** `dirty_gaps` reported untracked files and a snapshot that no longer matched, and said nothing about tracked files whose bytes on disk differ from the ones the index holds. Knowscroll had 21 of them; brian2 had none, which is why nothing showed. The three states the snapshot distinguishes — untracked, modified, deleted — now get a line each, counted and broken down by kind. Searching the committed tree remains the owner's decision; not saying so was the defect, and it is the false-absence rule's own case.
2. **A symbolic link never supplies an item's content.** `AGENTS.md` in Knowscroll is mode `120000`, and the packet cited its nine bytes — the target's *name* — as the file, reporting the item satisfied. **The rule now, in one piece:** a link is resolved inside the same tree, relative to its own directory, refusing an absolute target, one that climbs out of the tree, and a chain over `LINK_HOPS`; if it lands on a regular file of the same tree the item is satisfied **from the target and cited at the target's path**, with the locator naming the link it came through; otherwise the item is **unmet**, reason `source_is_a_link`, and the coverage says where the link pointed, because CONTEXT bounds an item's reason at 64 characters. A `source_included` check is satisfied by the section's path **or** its `via`, both of which are sealed in the packet, so satisfaction stays decidable from the packet alone.
3. **Claim relevance discriminates.** "A condition names a repository of the basis" is true of every claim in a one-repository store, so it selected nothing: 22 of 22 decisions went into a packet about one of them. Eligibility is unchanged and still decided **from the claim alone** — reading cited evidence to decide eligibility would make every claim over a large shared file eligible for every question. Among eligible claims the compiler ranks by the question's terms against the statement, the scope's qualifiers and the cited evidence **at the span the claim names**, carries `CARRIED_CLAIMS` of them, and omits the rest with reason `applicability`, counted. A claim that does not rank is omitted, not demoted. The span comes from the scope's `lines` qualifier, which is where a claim over a shared artifact has to put it until [protocol#16](https://github.com/Combraton/protocol/issues/16) is answered; without it every claim over one file scores identically and the ranking is no ranking.
4. **A span reaches its neighbours.** In both pilots something the question wanted sat a few lines past a cited span's edge, on the far side of a fixed twenty-line boundary that has nothing to do with the question. A **ranked** span now reaches the chunk either side of it while the excerpt stays within `EXCERPT_BYTES` and stays one contiguous byte range of the artifact; when only one side fits, the side holding more of the query's terms wins, and on a tie the following chunk does. A first chunk ranked nothing and is not extended. `compiler::COMPILER` is `cbr-context-compiler/2`, and the golden packet digest proves the bump.

### A fifth defect, found by the rerun rather than by the suite

Widening made two adjacent hits in one file land on the same bytes, and the overlap check ran **before** widening and only against what the items had taken — discovered spans were never checked against each other at all. brian2's rerun came back with one section published twice. Overlap is now decided after widening and against every span the packet already holds. The hermetic test for it needed a fixture whose chunks are small enough that each fits beside its neighbour inside `EXCERPT_BYTES`; the first two attempts at that fixture did not collide and the mutant survived both, which is recorded below rather than smoothed over.

### A property reported, not fixed

**An ingested artifact's id carries the instant it was ingested**, and a claim's revision digest is taken over a record holding that id, so two stores that ingest identical bytes and propose identical claims agree on neither. A packet citing an ingested artifact is byte-reproducible **within** its store and not across stores. The golden packet guard found this by failing on three consecutive runs with three different digests; it normalises both out, narrowly and with an assertion that both normalisations fired, and the property is recorded in [JOURNEYS](../verification/JOURNEYS.md#what-the-pilots-changed-and-what-changed-back) and in the close-out. Fixing it means changing what an ingest artifact is identified by, which is not one of the four changes this round was for.

### The pilots, rerun and rescored

Both ran again in full against the final binary, from fresh stores, with the questions verbatim and the selectors unchanged. brian2: 10 sections, 0 omissions, **15,568 bytes** of content where the first run had 10,837; 12.8s to index and compile. Knowscroll: **13 sections and 18 omissions, 18,950 bytes** where the first run had 31 sections, 0 omissions and 22,205; 3.9s. All four changes are visible in the Knowscroll packet — 21 modified tracked files named, the item satisfied from `CLAUDE.md` through the link, 4 claims of 22 carried and 18 counted, and a span widened from 20 lines to 40. **Both first runs failed their oracles** — brian2 one of three required facts, Knowscroll two of three, neither triggering its trap, the `git grep` baseline holding none of either set — and the reviewer rescores the second runs.

### One test's budget was scaled, and why it is here rather than hidden

`under_pressure_a_packet_loses_what_it_can_most_afford_to` submits at a capacity chosen to make about half the advisory content fit. Wider spans mean the same budget buys fewer and larger sections, and at 5,000 bytes only one span fitted, where the test needs at least two to tell an ordering from a truncation. The capacity is 8,000; **no assertion changed**. That the same budget now buys less is the trade the widening rule makes.

### Mutants

**This round: 12, all killed, all observed.** Each was run alone, so no other test's failure could be mistaken for the guard working.

| Mutant | Killed by | At |
|---|---|---|
| Drop the modified-file count from the coverage | `a_modified_tracked_file_is_named_in_the_coverage_beside_the_untracked_ones` | no gap names the modified file |
| `follow_link` always returns `Direct` — the old symlink behaviour | `a_link_inside_the_tree_is_followed_to_its_target_and_one_pointing_out_is_unmet` | the section carries the link's own bytes again |
| Eligibility alone selects — the cap removed | `among_many_eligible_claims_the_packet_carries_the_ones_the_question_is_about` | all eight eligible claims are carried |
| No extension — `widen` returns its span unchanged | `a_cited_span_reaches_the_neighbouring_chunk_when_the_excerpt_has_room` | the answer two lines past the edge is outside the excerpt |
| Overlap decided before widening | same test | one section is published twice |
| `CARRIED_CLAIMS` 4 → 6 | `the_packet_a_fixed_fixture_produces_has_not_changed_without_the_compiler_string` | two more claim sections |
| `compiler::COMPILER` `/2` → `/3`, no build change | same | the sealed packet names its producer |
| `DISCOVERED_SPANS` 8 → 6 | same | two discovered spans disappear |
| `EXCERPT_BYTES` 2048 → 1024 | same | every long excerpt is cut |
| `index::CHUNK_LINES` 20 → 25 | `the_index_a_fixed_tree_produces_has_not_changed_without_the_compiler_string` | the chunk rows move |
| `retrieval::COMPILER` `cbr-index/2` → `/3`, no build change | same | a bump nobody earned fails |
| `lexical::CHUNK_BYTES` 2048 → 4096 | same | `wide.md` stops splitting |

**One mutant survived twice before it was counted.** "Overlap decided before widening" passed against two successive versions of its fixture, because neither produced two adjacent hits whose chunks were small enough to widen into each other — the defect it reproduces came from brian2, where they were. The fixture was corrected until the mutant failed, and the survivals are recorded here for the same reason m3d's two were: a guard whose fixture never reaches the case it guards is not a guard.

### Coverage limits

- **Four fixes, none of them relevance.** Nothing here makes lexical retrieval understand a question, and neither pilot's failure was answered by changing what its question finds.
- **The reruns are rescored by the reviewer**, not by this session, which has seen neither oracle. A second failure is recorded as one.
- **Cross-store reproducibility of a packet citing an ingested artifact is not established** and is now known not to hold.
- **No model anywhere.** Model calls are M4.

## Earlier — M3d, the journey-6 pilots

[PR #19](https://github.com/Combraton/cbr/pull/19) against `main`. Two pilot repositories, neither of them this one, and the guard the milestone's own failures argued for.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 22 files, 0 errors; 433 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **177 tests** (175 at the previous head, 171 at m3c, 132 on `main` before M3) |
| `git diff --check` | 0 | — |
| all seven suites + `check_results.py` | 0 | `stream` 24/24 · `core` 130/5/0 · `socket` 11/2/0 · `evidence` 16/0/0 · `knowledge` 10/0/0 · `context` 11/0/0 · `composition` 3 pass, 11 unsupported — **unchanged** |

### What changed

- **Two pilots ran, and both records are in [JOURNEYS](../verification/JOURNEYS.md).** brian2, a 553-file brownfield repository with a question about a third-party function; Knowscroll-v2, 22 owner decisions over one append-only file with a question about how a recorded provider response must be handled. Each was asked the owner's question verbatim, with the selector written down before the run. **Neither packet was scored by this session**, which has seen neither oracle.
- **brian2 was scored by the reviewer and failed.** Of three required facts it holds one — its coverage, including the untracked files. It avoided the trap; the `git grep` baseline holds none of the three and its top hit is what the trap warns against. **The packet beat the baseline and did not meet the oracle.** Which facts were missing is not recorded, because the same question runs again at M4 with a model and that rerun has to be fair. **The compiler was not changed in response.**
- **Knowscroll's 22 decisions were applied as the owner decided them**, through the public client, each with the rationale `owner decision, 2026-09-20, m3d pilot`: 21 accepted for use as `binding`, D-019 as `evidence` because it records what a spike verified. Nothing rejected, nothing superseded, nothing left proposed.
- **A golden index digest**, `retrieval::GOLDEN_FIXTURE_DIGEST`, beside `retrieval::COMPILER`. It covers the chunk rows, the anchor rows, the coverage report and a fixed set of query results over a fixture tree written byte for byte in the test, **together with `COMPILER` itself** — so a build change with no version bump fails, and a version bump with no build change fails too. Ranking order is deliberately outside it: BM25 is unevaluated and a guard that fails on a tie gets switched off rather than fixed.
- **A golden packet digest**, on the same shape, answering the question the review asked. See below.
- **One issue filed on the protocol repository** and nothing else touched there: [Combraton/protocol#16](https://github.com/Combraton/protocol/issues/16), the case where a claim's support cannot cite a span of an artifact. It describes the case and the measurement and **proposes nothing normative**. Linked from [VERIFICATION](../VERIFICATION.md).

### Was the golden shape worth applying to the packet compiler? Yes, and it is done

The review asked for the index version and for a statement about the packet compiler's. The statement is: **it is cheap and it is in**, as `GOLDEN_PACKET_DIGEST` in `crates/cbr-cli/tests/registration_and_decisions.rs`, over the packet the existing fixture already produces.

Three things are worth recording about it, because they differ from the index case.

1. **The constant is not beside `compiler::COMPILER`, and cannot be.** `cbr-provider` is a binary-only crate with no library target, so nothing outside it can import that constant and every test of the compiler drives the built binary. The guard still binds the version, because **the compiler string is inside the bytes it digests**: a sealed packet's coverage names its producer. Bumping `compiler::COMPILER` moves the digest.
2. **The fixture had to grow before the guard meant anything.** As it stood, the fixture repository produced fewer spans than `DISCOVERED_SPANS` allows and no excerpt near `EXCERPT_BYTES`, so mutating either bound left the packet byte-identical — the guard would have passed a real change to both. Six more source files were added to the fixture, which now fills the discovery cap exactly and carries excerpts over 1 KiB, and the test asserts both so the next person cannot quietly shrink it back.
3. **The packet compiler was already better guarded than the index was**, which is why this is a regression detector rather than a hole being closed. A packet states its producer to every consumer; an index's manifest could claim `complete` under a string that had gone stale, which is the defect this milestone actually shipped.

### What the pilots found about the tool, reported and not fixed

Both are recorded in [JOURNEYS](../verification/JOURNEYS.md#j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun) and neither is changed in this pull request: a compiler change after a pilot run must be declared and the run repeated in full, and the review's instruction for this round was to go no further than the golden-digest work.

1. **A dirty working tree's modified tracked files are not named in the coverage.** `untracked_gap` reports untracked files, and reports a snapshot that no longer matches; it says nothing about tracked files that are modified. Knowscroll-v2 has 21 of them, so 21 files were searched at their committed bytes with no gap stated. brian2 could not have shown this — it had 130 untracked files and no modified tracked file. Measured against the false-absence rule of INTERNALS §5, this is a gap that is not stated.
2. **A required item was satisfied from a symlink.** `AGENTS.md` in that tree is mode `120000`, a link to `CLAUDE.md`. The indexer skips symlinks on purpose; `select_source` reads the blob directly and cited its nine bytes — the link target's name — as the item's content, reporting the item `satisfied`. The locator says exactly what it cited, so nothing is misreported; a consumer asking for `AGENTS.md` still got a path.

### Mutants

**This round: 6 mutants, all killed, all observed.** Each was killed by the golden test written for it, run alone so that no other test's failure could be mistaken for the guard working.

| Mutant | Killed by | At |
|---|---|---|
| `index::CHUNK_LINES` 20 → 25 | `the_index_a_fixed_tree_produces_has_not_changed_without_the_compiler_string` | the chunk rows move and the digest with them |
| `lexical::CHUNK_BYTES` 2048 → 4096 | same | `wide.md` stops splitting — the fixture file that exists so this is a row change and not only a recorded parameter |
| `retrieval::COMPILER` `cbr-index/2` → `/3`, with no build change | same | the version is inside the digest, so a bump nobody earned fails |
| `compiler::COMPILER` `cbr-context-compiler/1` → `/2`, with no build change | `the_packet_a_fixed_fixture_produces_has_not_changed_without_the_compiler_string` | the sealed packet names its producer |
| `compiler::DISCOVERED_SPANS` 8 → 6 | same | two discovered spans disappear |
| `compiler::EXCERPT_BYTES` 2048 → 1024 | same | every long excerpt is cut |

**Two of these survived their first run and are recorded as such.** `DISCOVERED_SPANS` and `EXCERPT_BYTES` were **equivalent on the fixture as it stood** — the packet did not change, because the fixture never reached either bound. That is the same failure the milestone keeps producing in a new place: a guard fitted to the case in front of it. The fixture was enlarged, both mutants were re-run, and both now fail the golden test; the assertions that keep the fixture at that size are in the test.

### Coverage limits

- **A pilot is a steer, not evidence.** Neither packet may be cited as downstream-task evidence for any gate. brian2's failure is a scored result against an oracle; Knowscroll's packet has not been scored at the time of writing.
- **Neither pilot measures a session.** No agent used either packet for any work, so nothing here shows whether work went better with it. That is J6 proper, at M7, and it still needs thresholds fixed before its confirmatory run.
- **The golden digests cover output, not correctness.** They fail when the build changes without its version; they say nothing about whether the index or the packet is any good.
- **Ranking is still BM25 and still unevaluated**, and deliberately outside the index golden.
- **No model anywhere.** Model calls are M4.

## Earlier — M3c, the deterministic packet compiler

[PR #18](https://github.com/Combraton/cbr/pull/18) against `main`, in the order the review asked for, each gate stated before its code. Four commits: `10e8f61`, `b8e9856`, `0958a25`, `ef521a0`.
1. **the five rules a retrieval answer owes its caller**, each with its test, before any packet is compiled;
2. **a registered checkout, and the view a grant makes of it**;
3. **the deterministic packet compiler, and J1** on this repository, with its negative control;
4. **J8, the journey records and this one**;
5. **the first review round**: five corrections, and the owner's m3d amendment recorded in ADR 001 question 6;
6. **the second review round**: the authorization leak discovery opened, and five more corrections.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 22 files, 0 errors; 433 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **171 tests** (166 and 156 at the two reviews, 132 on `main`) |
| `git diff --check` | 0 | — |
| all seven suites + `check_results.py` | 0 | `stream` 24/24 · `core` 130/5/0 · `socket` 11/2/0 · `evidence` 16/0/0 · `knowledge` 10/0/0 · `context` 11/0/0 · `composition` 3 pass, 11 unsupported — **unchanged** |

**The gate, before the code.** The eight retrieval tests were written and run against a `retrieval` module whose every function was `unimplemented!()`: **0 passed, 7 failed**, the eighth and ninth added afterwards for two mutants the first seven did not kill. The compiler's own gate is J1's oracle, declared in the test file's header before the run.

### What changed

- **`cbr_memory::retrieval`**, the rules an index alone does not give: a build manifest with the source frontier, the compiler and the coverage; `complete` / `lagging` / `unavailable` against the basis actually asked about; the false-absence rule with a bounded canonical fallback; a view that is the authorization step made into a type; and three separate bounds with a keyset cursor. A lagging projection is not consulted at all — its rows describe a different tree, so a hit in it would cite the wrong bytes.
- **`repositories`**, in the provider: registration as launch configuration (`--register-repository <id>=<path>`), a `cbr.repository` subject that never carries the path, and `view`, which turns a session's grant into the repositories it may read. A registration whose checkout cannot be read **refuses the launch**.
- **`compiler`**, the deterministic packet compiler. Its provenance is `cbr-context-compiler/1`, distinct from `cbr-context-script` by construction. It produces a *script*, spliced in place of its own marker step, so everything downstream is m3a's already-tested path — and the script is ordered by item, by repository and by item again, never by the order things were found in, which is what makes the packet reproducible.
- **Cited files are sealed as evidence** under their blob id, with the tree, the repository and the path as capture anchors, so a citation resolves to an exact span at a named tree and J1 checks it against `git cat-file blob`.
- **`cbr context`, `cbr request`, `cbr packet`**, so a journey has a public entry point. The client negotiates every `context/1` feature as optional and resolves the basis locally; no path crosses the socket.
- **The provider now depends on `cbr-identity` and `cbr-memory`.** Until M2 only the client ever resolved a basis.
- **Compiling is production-only.** A conformance launch is a test harness, and there a request with no script is still a request nothing prepares, which is what every context fixture was measured against.

### Journeys

**J1 and J8 both pass, with no model**, and both are recorded in [JOURNEYS](../verification/JOURNEYS.md) with the full field set.

- **J1 runs against this repository**, registered at launch. Its oracle was declared before the run: the decision record and the code it is about must both be in the packet, the coverage must name the frontier, and nothing read from source may be labelled `binding`. Measured: **8.0s to the first packet, 6.1s of it the index build over 1,015 blobs**, one coverage gap declared. Every citation was fetched and compared with the blob at the named tree.
- **J1's negative control is hermetic** — it has to move a repository, and moving this one would tie the test to its own branch's history. It has a positive half as well as a negative one: a compiler that simply cached the first tree would pass the negative half and fail the step where a request at the new tree must get the new bytes.
- **J8** separates the two fallbacks at one instant: `proceed_with_gap` publishes rather than waiting, `wait_until_deadline` holds the publication while an advisory item is unsatisfied. At the deadline the required item is `unmet` with `deadline_passed` and the advisory one `degraded` with the same reason, and the packet it publishes **cites nothing, includes nothing and satisfies nothing**.

### The review round, and what it corrected

The reviewer ran J1 by hand against a registered checkout and read the packet. Five things were wrong or thin; all five are fixed in this pull request, and the record says how each got past what was already there.

**The packet's provenance said `cbr-context-script`.** The publication path passed that constant unconditionally and never read the job's own `compiler` field, so the "distinct by construction" this branch reported was true of two constants and not of any packet. **How it passed 21 mutants: no test read the field.** The mutant that swapped the two constants was killed only by the coverage line, which is built from a different one, so it looked covered and was not. Both halves are asserted now — a compiled packet in J1, a scripted packet in the m3a test — and a mutant reverting the line is killed by the first.

**Span selection missed the answer.** `select_source` searched the whole repository and filtered by path afterwards, so a named file's own best span lost to every other file's and the packet cited a file's opening twenty lines while the answer was on line 24. The search is scoped to the path **in SQL** now, and a multi-term selector has a written ladder: every term in one chunk, else the best ranked partial match, else the opening chunk, marked `first-chunk` rather than the old `whole-file`, which said twenty lines were a file.

**A question is prose and a selector is chosen words.** Asking for every term of "why does source identity use gix rather than the git binary" returns only chunks that quote the question — in this repository, the test that asks it. Discovery uses the ranked partial reading; `select_source`, given a path, keeps the strict one.

**J1 was a partial and did not say so.** The request named both answer files by exact path, so two of its three oracle facts could not fail; the task text was never read; no decision was ingested as a claim; anchors were unused; and sections carried a locator line and no content, 281 bytes in all. The journey is redone: the request names **no path that answers it**, the ADR is ingested and accepted as a claim, a rejected claim is planted as a second trap, and the compiler has to find the rest. The record now reports 11 sections and 9 citations, and says what it still does not establish.

**Discovery, and what a section carries.** Beyond item-bound sections the compiler adds sections with no `item_id` (CONTEXT section 6): spans the question finds across the view, at most two per file so one file cannot take the budget; anchors for the **identifier-shaped** names only, because ordinary words of a task are function names somewhere in any large repository and pulled in every helper called `git`; and every claim judged against the basis, labelled by what an authority permitted, with rejected and inapplicable ones carried as `stale` and historical. A section now carries a **bounded excerpt** of the span it cites, so the output capacity counts what a consumer reads.

**Four smaller corrections.** A manifest built by a different compiler version is `lagging`, not `complete`. `may_join` compares the view, so a request under a narrower grant never joins a job compiled under a wider one. A blob's size is read from the object header before the blob is read, in all three read paths. And the index build inside the preparation tick is recorded in [VERIFICATION](../VERIFICATION.md) as a known limit with its measured cost, to be resolved in M4's bounded runtime rather than papered over with an unsupervised thread now.

### The second review round: a leak discovery opened

The reviewer probed the packet with a grant that covered repositories and nothing of knowledge, and got back a binding section carrying another principal's claim id, its state and its full statement.

**The cause.** `discover_claims` walked every claim subject in the store and called `knowledge_inspect`, which authorizes nothing — it is the body of an operation whose step 6 ran before it, in the command that normally calls it. Preparation calls it directly, on the provider's own authority, so nothing stood between a claim and any packet.

**How it shipped.** The repository view had exactly this shape from the first commit of this branch — resolve at the command, carry the ids in the job, compare them in `may_join` — and it had a test. Claims were added three commits later and got none of it. **The view test covered repositories, and nothing covered claims.** The same sentence as the provenance defect one round earlier: a rule nothing reads is a rule nothing holds, and here the rule existed and was simply not applied to the second thing that needed it.

**The fix, and its test.** The job carries `readable_claims`, resolved at submit from the grant with `knowledge.read`, `may_join` compares the set, and preparation reads nothing outside it. The test does not check that the section was labelled carefully: it asserts the packet is **byte-identical** to one prepared in a store where the claim was never proposed. The same test exists for a repository outside the view.

**Five more corrections in the same round.** A discovered claim carries its `claim` reference, so CONTEXT section 14's read-time facts see it and a later rejection shows at the read; it cites the evidence its claim rests on. Relevance is a stated rule — a condition naming a repository of the basis, or a shared term with the question — and everything else is omitted with reason `applicability` rather than silently. Drop order follows INTERNALS section 5 step 5 instead of the section id, which had let an anchor outlive a binding decision. A rejected claim's content names the decision that rejected it, because the label vocabulary has no `rejected` and `stale` alone says "was once valid". And the negative control now covers discovery: every blob any section cites is in the tree the packet names.

### The third time the same rule was not applied

`retrieval::COMPILER` is the identity a build writes into every manifest, and its own doc comment says a change to how an index is built is a change to that string. Bounding a chunk in bytes changed how every index is built — one chunk where there were several — and the constant still read `cbr-index/1`. An index from the previous build would have passed as `complete`.

The mechanism that catches this was built one round earlier, for exactly this reason: a manifest whose compiler differs is `lagging`, with a test. **The rule existed and was not applied, again**, which is now the third instance of the same shape in this milestone:

| Round | The rule that existed | Where it was not applied |
|---|---|---|
| m3c review 1 | a packet's provenance names its compiler | the publication path passed the constant unconditionally; no test read the field |
| m3c review 2 | resolve the readable set at the command, carry it in the job | the repository view did this; claims, added three commits later, did not |
| m3d | a change to how an index is built changes `COMPILER` | the chunker changed and the constant did not |

The pattern is not carelessness about the rule — each time the rule was written down, deliberately, and had a test. It is that **the rule was applied to the case in front of it and not made structural**, so the next case had to remember. Two of the three were caught by review rather than by anything in the repository.

### Mutants

**This round: 10 mutants, all killed.** Five survived their first run and are killed by tests written for them; each is named below with what was missing. No mutant here is a WRONG-REASON kill — every one fails at the assertion that states the rule it broke — with one exception, noted in the table.

| Mutant | Killed by | At |
|---|---|---|
| **Discovery reads every claim in the store** — the leak itself | `a_claim_the_grant_does_not_cover_is_absent_from_the_packet_byte_for_byte` | "a claim outside the grant is identical to a claim that never existed" |
| Discovery reads every registered repository | `a_request_naming_a_repository_outside_the_grant_is_answered_only_from_the_view` **and** `a_repository_outside_the_grant_contributes_nothing_to_discovery` | the item's unmet reason, and the byte-identity of the packet. **The first is arguably a WRONG-REASON kill for discovery**: it catches the guard through the item path, not through discovery, which is why the second was written |
| A discovered claim carries no reference | `a_discovered_claim_is_a_claim_section_and_a_later_rejection_shows_at_the_read` | "a discovered claim carries its reference, so a reader can check it" |
| **A discovered claim cites nothing** | same test, **after a citation assertion was added** | "a claim section cites its support" |
| **Every readable claim goes into every packet** | `a_readable_claim_that_does_not_bear_on_the_request_is_omitted_with_its_reason`, **written for it** | the toaster claim appears among the sections |
| **An irrelevant claim is dropped silently** | same test, **written for it** | "an irrelevant claim is omitted, not dropped: []" |
| **Discovered sections are ordered by their id** | `under_pressure_a_packet_loses_what_it_can_most_afford_to`, **after the test was given anchors to lose** | "and so does the code the question is about" |
| **The drop order is reversed** | same test, same repair | "the binding decision outlives everything advisory" |
| A rejected claim does not name its decision | `j1_a_question_finds_its_own_answer_with_a_cited_packet` | "a rejected claim names the decision that rejected it: … no longer current" |
| A job is joined whatever claims it could read | `context::tests::a_job_is_joined_only_by_a_request_with_the_same_view` | "a request that may read more claims never joins a narrower job" |

**Why five survived, in one sentence each.** Every claim in every test was relevant and accepted, so the relevance rule, its omission and the claim's evidence citation were written and never executed. And the drop-order test used a selector that produces no anchors, at a capacity where everything fit, so "anchors go before spans" passed with no anchors present.

**One claim I had made was wrong and is corrected in the test.** `context::inclusion` **packs** rather than truncates: it walks sections in order and keeps each that still fits, so a small low-priority section can occupy room a larger one could not use. What the order decides is priority, not exclusion. The assertion now pins the statement that is true and that distinguishes the two orderings — historical material is the first thing to go while task evidence stays.

**The round before: 12 mutants, all killed.** They exist because the round's first lesson was that a rule nothing reads is a rule nothing holds — `provenance.compiler` was wrong under 21 green mutants because no test read the field.

| Mutant | Killed by |
|---|---|
| **The packet forgets which compiler made it** — the defect itself | J1, on `provenance.compiler` |
| **The search is not scoped to the named path** | J1, where a cited span stops being the named file's bytes at the named line |
| A partial match still requires every term | `a_multi_term_query_can_ask_for_every_term_or_for_the_best_partial_match` |
| A question is read as every term | J1: the answer stops being found, because only chunks quoting the question match |
| **A section carries only its locator** | `an_excerpt_is_what_the_output_capacity_counts` · J1 |
| An excerpt is taken from the start of the file | J1, at the excerpt-equals-the-artifact-at-this-line check |
| Discovery adds nothing | `an_excerpt_is_what_the_output_capacity_counts` · J1 |
| A rejected claim is served as current | J1's second trap |
| **One file may take the whole discovery budget** | `discovery_covers_more_than_one_file_however_loud_a_single_file_is`, **written for it**: it survived its first run, because the decision record still landed inside the top eight by score with the cap removed, so the cap improved the packet while nothing pinned it |
| A manifest from another compiler is still complete | `an_index_built_by_another_compiler_is_lagging_however_current_its_tree` |
| A narrower request joins a wider job | `a_job_is_joined_only_by_a_request_with_the_same_view` |
| A blob is read before its size is checked | `a_blob_is_measured_before_it_is_read` |

From the first round: all observed, all restored. **21 distinct mutants, all killed**: 12 against the retrieval rules, 9 against the compiler and its provider glue. Five of them survived their first batch and are killed by tests written for them, named below.

| Mutant | Killed by |
|---|---|
| **The false-absence rule removed** — the review's named mutant | `a_lagging_projection_falls_back_to_the_canonical_records_and_never_answers_nothing_found` and `an_unavailable_projection_states_why_rather_than_answering_empty`, both at `declares_its_gaps` |
| An empty fallback says nothing · an unreadable fallback says nothing | `an_unavailable_projection_states_why_rather_than_answering_empty`; **the first needed a third case written for it** |
| A lagging projection answers from its stale index | the frontier and state assertions of `an_index_records_the_frontier_and_the_compiler_it_was_built_with` |
| The view is not the search scope | `a_principal_without_read_sees_nothing_from_that_repository_and_cannot_tell_it_apart_from_no_match` |
| The row cap is not applied · the cursor is never owed · the keyset is applied after the page | `a_page_is_bounded_by_rows_and_its_cursor_walks_every_hit_exactly_once` |
| A tie at the cursor score is always taken | `a_page_walks_two_repositories_that_tie_on_score_without_repeating_either`, **written for it** |
| One read is not capped · a batch is not capped | `one_read_is_bounded_by_its_span_and_a_batch_by_its_total` |
| The digest includes insertion order | `a_rebuilt_index_is_identical_to_the_one_maintained_incrementally` |
| **The basis is ignored and the checkout's `HEAD` is read** — the control's own mutant | **J1's negative control**, at the step where a request at the old tree must still get the old bytes |
| **Source read from a repository is served as `binding`** | **J1's trap** |
| A compiled packet claims the scripted compiler | J1, at the coverage producer |
| Coverage is never reported | J1, at the coverage assertion |
| A citation names another blob of the same tree | J1, at “the cited artifact is the blob at the named tree, exactly” |
| The compiled script follows the order things were found in | `the_script_is_ordered_by_item_whatever_order_the_compiler_decided_in`, **after an ascending-order assertion was added**: the first form of that test compared two orderings of the same reversal and passed |
| A projection that cannot be built is silently complete | `a_projection_that_cannot_be_built_is_reported_unavailable_rather_than_empty`, **written for it** |
| The view is not consulted at preparation | `a_request_naming_a_repository_outside_the_grant_is_answered_only_from_the_view`, **written for it** |

**Two mutations are recorded rather than counted, because neither was a fair test.**
- **One was equivalent as first written.** Guarding the view check with `&& !view.is_empty()` behaves identically for every view a session can actually hold, so it could never fail. It was corrected to remove the check outright before it was counted.
- **One was replaced.** Sealing a cited file under a digest that does not match its bytes makes `publish_object` refuse the object, so preparation never publishes and the journey times out instead of failing at the citation check. A timeout is not a clean kill and the integrity check upstream makes that mutation unreachable, so it was replaced by one that seals a *consistent* digest of a different blob of the same tree — which is what the citation comparison is there to catch.

### Coverage limits

- **Determinism is a property of the build, not of the content.** INTERNALS section 4 is explicit that a structurally consistent build is not a claim that what it selected is right. J1's oracle is a predeclared list of facts, not a measurement of whether a session did better with the packet; that is J6.
- **The request names the files.** Retrieval chooses which span of a named file to cite, not which file to look at. Ranking is still BM25 and still unevaluated.
- **The canonical fallback's read budget has no test.** Nothing in the suite reads the 8 MiB it allows before it stops and says so.
- **One repository per journey.** A basis naming several repositories is supported and untested end to end.
- **No model anywhere.** Model-assisted selection is M4, and a compiled packet must not be described as model-assisted.

## Earlier — M3b, retrieval and the dependency evaluator

[PR #17](https://github.com/Combraton/cbr/pull/17), merged as `c3cdf51` pinned to `fb67250`. Rebased onto `main` after PR #16 landed, so every hash below is the rebased one. Eleven commits:
1. the evaluator's gate, written and run against an unimplemented evaluator (`2af15f0`);
2. the evaluator, and the bug its harness found (`15a5a87`);
3. the lexical index and its pre-tokeniser (`89dedbd`);
4. source identity moves to `gix` and gains bulk tree and blob reads (`4928a9b`);
5. indexing a tree, with the coverage it did not reach (`9c04f51`);
6. the query simplification and three tests that can fail (`c09b400`);
7. the tests two surviving mutants asked for, and this record (`f1084a6`);
8. the last mutant, rerun and observed (`6aaaec6`);
9. this record's own numbers, corrected against a measured run (`ac249f3`);
10. this record names the pull request (`4f43083`);
11. the records the reviewer asked for at the m3b review: the killing seeds, what m3b deferred, and ADR 001 question 11 reconciled.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 22 files, 0 errors; 433 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **132 tests** after the rebase (117 before it, 107 on `main`): 20 encoding, 6 identity, 23 memory, 57 provider unit, 21 against the real provider, 5 running `cbr` against it. The evaluator harness is 17s of that |
| `git diff --check` | 0 | — |
| all seven suites + `check_results.py` | 0 | `stream` 24/24 · `core` 130/5/0 · `socket` 11/2/0 · `evidence` 16/0/0 · `knowledge` 10/0/0 · `context` 11/0/0 · `composition` 3 pass, 11 unsupported, 0 fail — **re-measured after the rebase**, all unchanged |

**The gate, derived before implementing.** STACK §7 names the acceptance criterion for a hand-rolled evaluator: a property harness over random DAGs and random input mutations, with demand-driven evaluation always agreeing with a from-scratch recompute. It was written first and measured against the unimplemented evaluator — 5 tests, all failing with "not implemented" — before any of it existed. The lexical tests were run the same way against a stubbed module: 7 failing, then 7 passing.

**What the harness caught.** At seed 1, an input whose durability *decreased* announced its change only at the new, lower level, so a memo that had recorded the higher durability shallow-verified against that level and served a stale value. A write now announces at the higher of the old and new durability. This is the exact failure mode STACK §7 warned about — "subtle validation bugs that silently serve stale answers" — found by the mitigation it prescribed, in the first seed.

### What changed

- **`crates/cbr-memory`**, a new crate of derived memory, all of it working on a caller-supplied connection so index and memo rows commit in the caller's transaction:
  - `evaluator`: `changed_at`/`verified_at` per memo, ordered edges with a reverse index, early cutoff by backdating, durability levels, and the two guards (no backdating when durability decreased; never validate an untracked read without re-executing). Cycles are refused.
  - `lexical`: FTS5 with a Rust pre-tokeniser. An identifier is stored whole and as its parts; a query asks for the parts. Hits name spans of blobs, never text.
  - `anchors`: tree-sitter tags for Rust, Python, JavaScript, TypeScript and TSX only. Every candidate kept, ambiguity marked, each anchor recorded at its tree and blob.
  - `index`: indexing one tree, reporting blobs seen, indexed, anchored, and the three gaps — binary, too large, unanchored language.
- **`cbr-identity` moved to `gix`**: `git_basis` and `dirty_snapshot` in process with no installed `git`, plus `tree_entries` and `read_blob`. This is ADR 001 question 11's first named trigger — retrieval reading a tree's blobs in bulk. **That row now records that it fired**, since both branches are on `main`: the acceptance it named, the four identity property tests with their negative controls, passes unchanged, and the runtime dependency on an installed `git` is gone. Non-git directory identity is still absent, exactly as the row says.
- **Two tests added to make mutants killable**: a call site so a reference is not offered as a definition, a symlink so only regular blobs are indexed, and a staged-then-restored change so the index half of "dirty" is covered.

### Mutants

All observed, all restored. 25 mutants against `cargo test`: 24 fail a test and 1 is equivalent.

| Mutant | Killed by | At |
|---|---|---|
| A durability decrease announced only at its new level | the property harness | seed 1, round 5 |
| Backdating without the durability guard | same | seed 141, round 6 |
| Backdating whatever the value · deep verification checking only the first dependency | same | seed 1, round 2 |
| An untracked memo validated like any other | `an_untracked_read_is_never_validated_without_re_executing` · the harness | seed 1, round 1 |
| Shallow verification at the highest durability | the harness · the early-cutoff test | seed 8, round 2 |
| Never backdating | `a_recomputed_value_that_did_not_change_does_not_re_execute_its_dependents` | "the constant is recomputed, its dependent is not" |
| Identifiers not split · only the parts stored, never the whole · a chunk ending one byte late | the tree-index test · `the_pre_tokeniser_emits_the_whole_and_the_parts` · `chunks_tile_the_text_exactly` | — |
| A search answering across every tree · removing one tree removing every tree · re-indexing accumulating | the tree-index test | "the old text is not searchable at the new tree" · a constraint violation · the re-index count |
| A reference resolving as a definition · ambiguity never marked · resolution picking the first candidate · an anchor answering for any tree · every file claimed as Rust | the anchor tests | the two-definition resolution · "two definitions of one name are ambiguous" · the same · "a different tree has no anchors" · the language map |
| Binary content indexed lossily · symlinks and gitlinks indexed as blobs | the tree-index test | the coverage counts |
| A tree listing its subtrees as blobs · a staged change not dirty · untracked files collapsed into their directory | the identity tests | "a tree is not a blob" · the staged-change case · "the file, not its directory" |
| **Setting an input ignores a durability change** | **nothing — equivalent** | With the same value, ignoring a durability change cannot produce a wrong answer: the input keeps its recorded durability, and the next change to its value announces at the higher of the old and new, so every memo still sees it. Recorded, not counted as a kill |
| Shallow validation not recording that it verified | `a_durable_memo_is_verified_without_walking_and_records_it` | It survived the first batch, which had no test for it; the test was written and the mutant rerun against it. `verified_at: 1` where the revision is 2 — a memo that validates without walking never advances, so it re-walks its edges on every later read |

#### The killing seed of every evaluator mutant, and why 200 is not decoration

The harness walks `1..=200` in order and stops at the first disagreement, so the seed it names **is the lowest seed that kills that mutant**. Recorded so that nobody shrinks the range without knowing what it costs.

| Evaluator mutant | Lowest killing seed | Round |
|---|---|---|
| An untracked memo validated like any other | 1 | 1 |
| Backdating whatever the value | 1 | 2 |
| Deep verification checking only the first dependency | 1 | 2 |
| A durability decrease announced only at its new level | 1 | 5 |
| Shallow verification at the highest durability | 8 | 2 |
| **Backdating without the durability guard** | **141** | 6 |
| Never backdating | none | no seed kills it; `a_recomputed_value_that_did_not_change_does_not_re_execute_its_dependents` does |
| Shallow validation not recording that it verified | none | `a_durable_memo_is_verified_without_walking_and_records_it` does |
| Setting an input ignores a durability change | none | equivalent; see the row above |

**The seed count is load-bearing.** The highest lowest-killing-seed is **141**, so a harness of 100 seeds lets the missing-durability-guard mutant through: it is the one guard the harness alone defends, and it is silent when it fails. **Do not reduce the seed count below 141.** It is 200, which leaves a margin of 59 seeds and costs about 17 seconds. The reviewer reproduced this independently at the m3b review by removing the guard and watching the harness fail at seed 141, round 6.

### What m3b deferred, and why it is named here

m3b built the retrieval mechanisms; it did not build the rules that govern their answers. The earlier record did not say so, which made the omission look like a decision rather than an absence. The reviewer named four items at the m3b review; they are **not implemented**, and are listed as five rows below because the build manifest and the false-absence behaviour it enables are separate pieces of work:

| Deferred | What is missing | Why it matters | Lands |
|---|---|---|---|
| **Build manifests** (INTERNALS §2) | the source frontier an index was built to, the compiler version that built it, and a state of `complete`, `lagging` or `unavailable` per index | without them a reader cannot tell a current index from one that stopped a thousand commits ago, and every answer reads as authoritative | m3c, first |
| **The false-absence test** | a lagging index must return a bounded canonical fallback or say its coverage is incomplete — **never "nothing found"** | INTERNALS §5 forbids reading an empty result as absence. Journey 1's negative control is meaningless until this holds: "no packet claims applicability" and "the index never looked" are indistinguishable without it | m3c, first |
| **Rebuild equality** | a test that an index rebuilt from the canonical records is byte-identical to the incrementally maintained one | this is what makes the index *derived* rather than authority; without it, incremental drift is undetectable and unrecoverable | m3c, first |
| **The access rule on search** | a search must never return content the principal cannot read, with a test | retrieval currently has no principal at all. As soon as the compiler calls it under a request, an unfiltered index is a disclosure channel that no grant check upstream can close | m3c, first |
| **Bounding beyond one `LIMIT`** | a pagination cursor, span and byte caps per read, and an aggregate cap across a batch | one `LIMIT` bounds a row count, not a response. A single oversized blob or a wide batch still answers without bound | m3c, first |

**These are m3c's first commits, each with its test, before any packet is compiled** — the compiler is their first consumer, so building it first would mean writing its tests against rules that do not exist yet.

### Coverage limits

- **Nothing here is reachable over the protocol yet**; the packet compiler at m3c is its first caller.
- **Ranking is BM25 with no evaluation of usefulness.**
- **Scale is untested**: the largest indexed tree is a test repository, and the chunk size and 1 MiB blob cap are unmeasured.
- **Anchors resolve names, not references**; ambiguity stays ambiguous by design.
- **Non-git trees still have no identity**, and a submodule is a gitlink with no contents.

## Earlier — M3a, the Context profile

[PR #16](https://github.com/Combraton/cbr/pull/16), merged as `cbfaebe` pinned to `1bd9a7a`. Five commits (the record below is as it was measured on that head, and `5. results, CI and documentation` is `1bd9a7a` itself):
1. two records the reviewer asked for in the M3 branch: ADR 001 question 11 (the `git` binary, not `gix`) and the M4 obligation that CBR's own producer never names a derived artifact as an ancestry root (`448534f`);
2. the vendored client-only `minimal-executor` and its client module, and the context and composition expectations, derived before implementing (`080fca6`);
3. the profile (`a156851`);
4. the tests the fixtures cannot give, and capacity reserved for required content (`34568ab`);
5. results, CI and documentation.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 22 files, 0 errors; 433 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **107 tests**: 20 encoding, 4 identity, 57 provider unit, 21 against the real provider, 5 running `cbr` against it |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter context.` + `check_results.py` | 0 | **11 pass, 0 unsupported, 0 fail**, stable over three runs |
| `run_fixtures.py --filter composition.` (Unix descriptor) + `check_results.py` | 0 | **3 pass, 11 unsupported, 0 fail**, the 11 exactly as expected, stable over three runs |
| `stream` / `core` / `socket` / `evidence` / `knowledge` + `check_results.py` | 0 | 24/24 · 130/5/0 · 11/2/0 · 16/0/0 · 10/0/0, unchanged |
| `result_paths.py conformance/results/*/` | 0 | no machine paths, m1b to m3a |

**Expected versus measured.**
- **Derived before implementing:** context 11 / 0 / 0 (`context.json`); composition 3 pass and 11 permanently unsupported, the 11 named (`composition.json`): ten declare `execution/1` and one `verification/1`.
- **The unimplemented run**, against M2's binary with an uncommitted scratch descriptor declaring the claims: context 0 / 11 fail / 0; composition 0 / 3 fail / 11 unsupported, the 11 exactly those.
- **Measured:** both on the first complete run. A first-run pass proves nothing by itself, which is why every guard below has a mutant, and why the rules no fixture reaches have their own tests.

### What changed

- **`context.rs`**, pure rules: step-2 validation of every payload as closed objects plus the rules a schema cannot state; the features a submit needs at step 3; item results decided by each item's check; inclusion with capacity reserved for required items; claim snapshots with the record digest recomputed; the reliance table; packet compilation into canonical bytes and facts; read-time claim facts; exact excerpts.
- **`provider/context_ops.rs`**:
  - submit and cancel through `admit_command`, and the three queries;
  - preparation as a per-job tick against the provider clock, applied to an in-memory batch and committed in one owner transaction with provider-origin events;
  - local sealing, with the packet's object published and verified before its batch commits;
  - sealing at a separate evidence provider, where a failed seal commits nothing of the step and keeps the capture instants, so the retry replays.
- **`peer.rs`**, the provider's protocol client for evidence and knowledge peers. No failure it reports carries a credential, a socket path or a peer's error details.
- **The store**: `commit_provider_batch`, which refuses a stale base or an event at a revision the batch did not write; and a further subject change may carry no event.
- **The `context.script` control**, refused in production like every control, with a redacting `Debug` form because its peers carry credentials. Declared in both descriptors with `context/1` and its seven features.
- **A barrier**, `context.packet.after_object_published`, not declared in a descriptor because no fixture awaits it.
- **Preparation never runs for an unauthenticated connection**, since it can call peers.
- **`run_fixtures.py`** launches the runner with `PYTHONDONTWRITEBYTECODE=1`: the composition suite runs the vendored kernel, and its bytecode cache made `verify_pin.py` fail on a local run. CI verifies the pin before any fixture runs, so CI was not affected. The committed results were recorded before this change, which alters only the environment.
- **CI** gates the context and composition suites. Results are committed under `conformance/results/m3a-context` and `m3a-composition`.

### Mutants

All observed, all restored. Each fixture step is the runner's numbering. 57 distinct mutants in 67 runs. Two first runs did not build and were rerun with a corrected edit; the other eight extra runs put the same mutant against both fixtures and tests, against a second suite, or again after the inclusion change. 48 fail a fixture; 9 fail only CBR's own tests.

| Mutant | Killed by | At |
|---|---|---|
| Check ignored | `claims-in-packets-are-negotiated-snapshots` · `items-are-satisfied-only-by-their-check` | 29 · 7 |
| Missing required reported satisfied · required downgraded to degraded | each: `claims-in-packets` 29, `correction-during-preparation` 6, `deadline-leaves-required-unmet` 9, `expand-returns-only-authorized-citations` 11, `items-are-satisfied` 7, `limits-are-separate` 3, `request-items-are-checkable` 10 | — |
| Scripted unmet overrides satisfied · historical claim reason unavailable | `claims-in-packets` | 36 · 36 |
| Label promotes claim · unknown applicability marked stale · lineage revision unreported | same | 31 · 31 · 31 |
| Claim digest unchecked · claim read names no revision | same | 29 · 29 |
| Claim invalidation unreported: permitted use lost · invalid for target · hypothesis invalidation unreported | same | 34 · 39 · 39 |
| Claims format without negotiation · claim members served without negotiation | same | 48 · 43 |
| Unreadable claim carried | same | 31, as a schema violation (a null claim in `claim_changes`), not the omission |
| Unavailable knowledge valid | `composition.thirdparty-kernel-enforces-required-claim-boundary` | 61; the context suite does not kill it |
| Global coverage cursor · unobserved frontier claimed · authority revision dropped | `packet-is-an-exact-sealed-evidence-artifact` | 7 · 7 · 7 |
| Local packet producer is the caller | same | 10 |
| Unlabeled section · excerpt carries a digest | seven and eight fixtures at their first packet read, as schema violations; first `claims-in-packets` 31 | — |
| Commit-only basis complete accepted · uncheckable item accepted · transition as advisory | `request-items-are-checkable` | 7 · 2 · 8 |
| Obligation features ungated | `claims-in-packets` 44 · `request-items-are-checkable` 8 · `requires-core-events-and-gates-optional-features` 3 | — |
| `context.expand` ungated | `requires-core-events` | 5, `permission_denied` instead of `unsupported_required_feature` |
| Shared job across principals · cancel ends the job | `shared-job-survives-one-subscriber-cancelling` | 8 · 12 |
| Corrected reason lost · stale derivation current · authority check ignores corrections | `correction-during-preparation` | 6 · 7 · 9 |
| Capacity reason lost · mandatory refusal skipped · limits collapsed | `limits-are-separate` | 6 · 8 · 3 |
| Capacity reservation removed | `limits-are-separate` 6 · both capacity unit tests | — |
| Evidence check ignores digest | `items-are-satisfied` | 7 |
| Old revision relabeled current, request · packet | `updates-are-new-revisions` | 10 · 12 |
| Packet list replaced | `correction-during-preparation` 9 · `updates-are-new-revisions` 10 | — |
| Deadline pass skipped · wait fallback ignored | `deadline-leaves-required-unmet` | 9 · 6 |
| Job progress not saved | seven fixtures; first `correction-during-preparation` 9 | — |
| Expand ignores the grant | `expand-returns-only-authorized-citations` | 18 |
| Packet grant covers all packets | `composition.direct-fetch-needs-one-grant-per-audience` | 21; the context suite does not kill it |
| Packet reported before seal · peer seal skipped | `composition.packets-are-sealed-at-a-separate-evidence-provider` | 15 · 9 |
| Capture instants not kept · kept but ignored on retry | `a_publication_interrupted_at_the_evidence_provider_replays_after_the_clock_moves` | "the retried publication replays the steps that applied" |
| Packet object never published | `a_packet_is_never_published_before_its_local_seal_commits` | the fetched bytes' digest |

**Where the packet crash row lives.** `a_packet_is_never_published_before_its_local_seal_commits` is in **`crates/cbr-provider/tests/context.rs`**, not in `crates/cbr-provider/tests/crash_matrix.rs`. It kills the provider at the `context.packet.after_object_published` barrier and checks the revision is published exactly once after restart. The matrix table in `crash_matrix.rs` lists the five STORAGE §5 rows only, so this row is findable nowhere else; it is named here for that reason.
| Corrections not reported at read · `superseded_by` unreported · stale-at-addition unmarked | `a_correction_after_publication_is_reported_at_the_read_beside_supersession` | `invalidated_items` · `superseded_by` · `s-late` historical |
| Mandatory items dropped at inclusion | `mandatory_content_is_included_past_capacity_and_advisory_content_is_omitted` | the included sections; no fixture kills it |
| Batch base unchecked · batch event revision unchecked | `a_provider_batch_commits_whole_or_not_at_all` | the stale-base refusal · the unwritten-revision refusal |

**Named honestly.**
- **Fixtures alone left five mutants alive** in the first batch: mandatory items dropped at inclusion, unavailable knowledge valid, corrections not reported at read, `superseded_by` unreported, and packet grant covering all packets. Two die in the composition suite; the other three needed CBR's own tests, which now kill them.
- **The mandatory-items survivor exposed an ordering gap.** With advisory content prepared before required content, the reference provider includes the required section past the output capacity. CBR now reserves capacity for required content first; no fixture orders sections that way, so this rests on a unit test.
- **Mutant names follow the protocol reference's mutants where they match**, but each is CBR's own edit to CBR's code and is recorded as run.

### Coverage limits

- **Packet content is scripted.** Nothing is retrieved or compiled until m3b and m3c.
- **No executor role.** Composition fixtures that need `execution/1` or `verification/1` are permanently out of reach, and the script step `execute` holds its job.
- **Peer calls hold the processing lock**, bounded by a three-second timeout per frame.
- **Packet bytes are kept by the context record too**, so purging a packet artifact ends its fetch but not its excerpt. Request retention is not implemented.
- **Untested:** event visibility of `context.job` under a grant, skipping preparation for an unauthenticated connection, and an artifact id collision on publication.

## Earlier — M2, Knowledge

One pull request, four commits:
1. the expectation, derived before implementing (`b6c0188`);
2. the profile (`7b7c115`);
3. source identity, the ancestry control, J9 and a claim surviving `SIGKILL` (`732ff6f`);
4. the `cbr` knowledge verbs, results, CI and documentation.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 22 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **92 tests**: 20 encoding, 4 identity, 46 provider unit, 17 against the real provider, 5 running `cbr` against it |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter knowledge.` + `check_results.py` | 0 | **10 pass, 0 unsupported, 0 fail**, stable over three runs |
| `stream` / `core` / `socket` (Unix descriptor) / `evidence` + `check_results.py` | 0 | 24/24 · 130/5/0 · 11/2/0 · 16/0/0, unchanged |
| `result_paths.py conformance/results/*/` | 0 | no machine paths, m1b to m2 |

**Expected versus measured.**
- **Derived before implementing** (`knowledge.json`): 10 / 0 / 0, with `knowledge.store`.
- **The unimplemented run**, against M1's binary with an uncommitted scratch descriptor: 0 / 10 fail / 0 with the control declared, and 0 / 9 fail / 1 unsupported without it, the one being `knowledge.applicability-follows-precedence`.
- **Measured**: 10 / 0 / 0 on the first complete run. A first-run pass proves nothing by itself, which is why every guard below has a mutant.

### What changed

- **`knowledge.rs`**, pure rules:
  - step-2 validation of every payload as closed objects;
  - the record and its digest;
  - support classes;
  - structural comparison;
  - condition findings and result precedence.
- **`provider/knowledge_ops.rs`**: all eleven operations.
  - The base check is by number and digest.
  - A decision is checked in section 6's order: epoch, then preconditions, then existence, authority, digest and the latest decision.
  - Dependencies resolve only as exact local references, and the claims they name need read rights.
- **The store**: an insert-only `knowledge_revisions` table with `UPDATE` and `DELETE` triggers. The revision row commits with its claim subject; the store refuses a row out of step with it.
- **`admit_command` takes an `Epoch`**: unchecked, a `core-test` scope, or a knowledge binding compared only when the scope is bound. `Step6::Bind` lets only an authority principal without a grant bind or transfer.
- **The `knowledge.store` control**: the evaluator pin and `serve_altered_claims`, refused in production and declared in both descriptors. Production pins `cbr-conditions` v1; conformance defaults to the documented `reference-conditions` v1.
- **`crates/cbr-identity`**: git root trees, dirty snapshots and the environment fact set, through `git` plumbing with fixed arguments and no shell.
- **`cbr`**:
  - the knowledge verbs and the local `basis`;
  - `ingest --source-kind` and `--repo` (tree and commit anchors);
  - a `--grant` option on every verb;
  - the session split into modules.
- **`cbr-provider --issue-credential PRINCIPAL`**.
- **CI** gates the knowledge suite. Results are committed under `conformance/results/m2`.

### Mutants

All observed, all restored. Each fixture step is the runner's numbering.

| Mutant | Killed by | At |
|---|---|---|
| A revision read returns the lineage's latest | `claim-revisions-are-immutable-with-base-checks` · `dependencies-resolve-only-exact-references` · `history-keeps-every-record-with-links` | 6 · 20 · 6 |
| Revise base unchecked | `claim-revisions-are-immutable-with-base-checks` | 10 |
| Empty roots accepted | same | 17 |
| History drops superseded revisions | `history-keeps-every-record-with-links` | 12 |
| Absent validity recorded as `{}` | same | 13 |
| Any principal is the bound authority | `only-the-bound-authority-decides` 15 · `conflict-resolution-needs-the-bound-authority` 11 · `a_model_labelled_producer_cannot_accept_its_own_claim…` | "the model must not accept its own claim" |
| Self-adoption unrecorded | `only-the-bound-authority-decides` 24 · `history` 12 · `reliance-applicability…` 8 | — |
| Transfer resets reliance | `only-the-bound-authority-decides` 46 · J9 | reliance after the transfer |
| Decision epoch optional | `only-the-bound-authority-decides` | 21, `stale_authority_epoch` instead of `invalid_envelope` |
| Decision supersession unchecked | same | 50 |
| Bind by grant · authority binds under grant | same | 7 · 7 |
| Decision epoch unchecked | `only-the-bound-authority-decides` 47 · J9 | A's stale decision |
| Epoch after preconditions | `only-the-bound-authority-decides` | 48, `precondition_failed` instead of `stale_authority_epoch` |
| Decision claim read unchecked | same | 22 |
| Resolve without authority | `conflict-resolution-needs-the-bound-authority` | 11 |
| Drift as conflict · qualifiers disjoint unchecked | `conflicts-are-structural…` | 4 · 11 |
| Potential reported demonstrated | `conflicts-are-structural…` 4 · `conflict-resolution…` 10 · `history` 12 | — |
| Dependency digest ignored · provider ignored · missing satisfied | `dependencies-resolve-only-exact-references` | 8 · 10 · 8 |
| Reference provider unchecked · dependency read unauthorized · self reference before provider | same | 30 · 36 · 24 |
| Unknown outranks mismatch | `applicability-follows-precedence` | 3 |
| Incomplete coverage applicable | `applicability-checks-snapshots…` 5 · `applicability-follows-precedence` 6 | — |
| Dirty snapshot as tree | `applicability-checks-snapshots…` | 4 |
| Support entries as origins · equal-digest roots disjoint · overlap as multiple | `support-classes-follow-declared-ancestry` | 15 · 19 · 9 |
| Single status field · normative implies binding | `reliance-applicability-health-and-availability-are-independent` | 11 · 13 |
| Derivation is its own root | `two_derivations_over_one_captured_log_are_one_lineage` | "two derivations over one captured root are one lineage" |
| Claim revisions in a TEMP table | `a_claim_and_its_decision_survive_sigkill_at_their_positions` | "survive at the same positions" |
| Update trigger inert | `a_claim_revision_can_be_neither_updated_nor_deleted` | "an update is refused by the database" |
| Tree is the commit · snapshot hashes mtime · deletions dropped · mode ignored · build facts ignored | the identity tests | the tree check · "modification time alone is not identity" · "a deletion is identity" · "a mode change is identity" · the build-fact assertion |
| `cbr decide` names no latest decision | `a_model_labelled_producer…` | `precondition_failed {"latest_decision":"adopt"}` |
| `cbr evaluate` target tree is the commit | `revise_evaluate_and_history_follow_a_real_repository` | the `applicable` assertion |

**Named honestly.**

- **The fixture batch was 33 mutants.** The `m2(2/4)` commit message says 32; that was a miscount of the batch log, corrected here.
- Two fixture mutants are recorded under their own names rather than claimed as the fixture's narrower mutant: "a revision read returns the latest" stands in for `claim-revision-overwritten`, and "absent validity recorded as `{}`" for `validity-filled-from-recorded`.
- "Decision epoch optional" failed with `stale_authority_epoch` rather than a success, because an absent epoch reaches the binding check as 0. It is killed at the fixture's step, for a reason other than committing.

### Coverage limits

- **No fixture runs knowledge over the socket.** The CLI tests do, in production mode.
- **Untested controls and paths.** `serve_altered_claims` is implemented and has no test. Event visibility of knowledge subjects has no dedicated test.
- **Source identity** has no non-git tree, no `workspace` determination under a lock, and no submodule contents. It needs the `git` binary.
- **Read cost.** `inspect` and `history` scan every record of a kind.
- **CLI coverage.** `cbr` has no verb for conflicts, transfers or grants. A grant in the CLI test is issued over a raw socket session.
- **No model call.** The model producer is a labelled principal.

## Earlier — M1 close-out and README

Three things the reviewer asked for immediately after the merges, done in that order:

1. **Issue #3 closed** with the [close-out comment](https://github.com/Combraton/cbr/issues/3#issuecomment-5702383702): the outcome table across the five result sets (the encoding vectors plus `stream`, `core`, `socket` and `evidence`), the permanent coverage limits, and every mutant from stages (a) to (e). The same text is committed as [m1/CLOSEOUT.md](m1/CLOSEOUT.md) (`342eab7`).
2. **README no longer says no product runtime or installation command exists** (`46285ad`). It states what exists and what does not. There is still no release and no install command, only a build from source, and the README's snippet was run before it was written down. The documentation map, the work README and VERIFICATION's opening paragraph were updated to match.
3. **M2 opened as [issue #12](https://github.com/Combraton/cbr/issues/12)**, expecting `knowledge` **10 / 0 / 0 with `knowledge.store`**. That expectation was measured by a run against M1's binary with the claims declared and nothing implemented, using an uncommitted scratch descriptor: 10 fail / 0 unsupported with the control, and 9 fail / 1 unsupported without it (`knowledge.applicability-follows-precedence`).

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` | 0 | 22 files, 124 links, 0 errors |
| `git diff --check` | 0 | — |
| The README snippet (build, private socket directory, provider, `cbr ingest`, `cbr fetch`, `cmp`) | 0 | byte-identical |

**Found while running the snippet:** a Unix socket path must fit the platform's `sun_path` limit, 104 bytes on macOS. The provider refuses a longer path at bind (`path must be shorter than SUN_LEN`), before listening. The README says to keep the path short.

## Earlier — M1 stage (e)

The Evidence profile, the `cbr` CLI, and the storage crash matrix. Three commits: the profile (`c0c974b`), the CLI with credential administration (`56fe7ee`), and the crash matrix with results and documentation.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 21 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **78 tests**: 20 encoding, 41 provider unit, 14 against the real provider, 3 running `cbr` against it |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | 130 / 5 / 0, unchanged |
| `run_fixtures.py --filter socket. --participant …unix.json` + `check_results.py` | 0 | 11 / 2 / 0, unchanged, with the descriptor's claims synced to include `evidence/1` |
| `run_fixtures.py --filter evidence.` + `check_results.py` | 0 | **16 pass, 0 unsupported, 0 fail**, stable over three runs of the final binary |
| `result_paths.py conformance/results/*/` | 0 | no machine paths in m1b, m1c2, m1c3, m1c4, m1d, m1e |
| The crash-matrix, CLI and restart integration tests | 0 | stable over three consecutive runs |

**Expected versus measured.** `evidence.json` was derived from declared profiles, features and controls — all 16 reachable, none permanently unsupported — and confirmed by a run with the claims declared and nothing implemented: 16 fail, 0 unsupported. Measured: **16 / 0 / 0, as predicted.** One fixture reached past its own profile: `evidence.publish-grant-bound-to-work` issues grants carrying an `evidence.work_binding` constraint, so `core.grant.issue` gained that constraint kind.

### What changed

- **The profile** (`c0c974b`): prepare, append, seal, abandon, inspect, query, fetch, hold, release and purge on the Core command path; manifests; the proof-loss record; the `evidence.store` control. The object is published and verified from disk before the row that names it; a purge commits before it deletes.
- **`cbr`** (`56fe7ee`): `cbr ingest` and `cbr fetch`, a separate binary over the public socket that links no provider code; fetch verifies against the sealed digest before writing. Production socket starts issue a credential per CORE §18.1 (only its digest stored, handed off `0600` in `0700`); `--rotate-credential` and `--revoke-credential` apply to the next authentication without a restart. This closes stage (d)'s limit that a production socket had no way to issue a credential.
- **The crash matrix**: five tests, each killing the real provider with `SIGKILL` at a named barrier and asserting the durable fact after restart. Rows and the rows that do not exist at M1 are in [VERIFICATION](../VERIFICATION.md#the-storage-crash-matrix).
- **Found while mapping the collection row, and fixed.** Before this change a purge killed between its commit and its deletion left the object on disk **permanently**, under a record saying `purged`: nothing ever deleted it later. A start-time collection pass now rechecks the roots and deletes every object no sealed, unconfirmed-purge artifact names, plus leftover staging files. The same pass collects the orphan a crashed seal leaves.
- **Also found, and fixed:** that pass would be unsafe if two providers served one data directory, since one's collection could delete an object the other had published and not yet named. Nothing prevented that. A serving provider now holds an exclusive `flock` on `provider.lock`; a second refuses to start, and `SIGKILL` releases it.
- The Unix participant descriptor claims `evidence/1`, its features and `evidence.store` again, like the stdio one. No socket fixture declares them; the socket outcome is unchanged.
- CI runs the evidence suite and gates it against `evidence.json`.

### Mutants

All observed, all restored. Fixture steps are the runner's numbering. A mutant against another package's binary is only valid after `cargo build --workspace`: a first run of `restart-rotates-live-credential` under `cargo test --test ingest_and_fetch` **survived because the provider binary was not rebuilt**. The mutant script now builds the workspace first; that kill is from the rerun, and the earlier survival is not counted either way.

| Mutant | Killed by | At |
|---|---|---|
| Seal without its digest check | `evidence.seal-refuses-digest-mismatch-and-is-idempotent` step 4 | `unavailable`, not a sealed artifact — **a second guard**, `publish_object`'s own digest check, not the one named |
| Append accepts an offset already received | `evidence.chunk-retransmission-and-conflicts` step 8 | `unavailable` — **a second guard**, the chunk table's primary key |
| Purge ignores holds | `expired-hold-stops-protecting` 7 · `purge-requires-release-authority-for-holds` 19 | — |
| A withheld manifest child evaluated anyway | `manifest-completeness-respects-authorization` step 45 | "expected withheld, found present" |
| Stored bytes not verified on read | `integrity-failure-is-never-served` step 11 | — |
| Work binding ignored | `publish-grant-bound-to-work` step 13 | — |
| `cbr fetch` skips its digest check | `fetch_refuses_bytes_that_do_not_match_the_sealed_digest` | "altered bytes must be refused" — only this test; against an honest provider nothing shows it |
| `cbr fetch` stops after the first chunk | `ingested_bytes_fetch_identically_after_sigkill_and_restart` | "fetch failed", reported by `cbr`'s own digest check |
| `cbr ingest` in one append | same test | "ingest failed: Broken pipe" — the provider closed on the oversized frame, so ingest really is multi-chunk |
| A restart rotates a live credential | same test | "a restart keeps a live handed-off credential rather than rotating it" |
| Rotation keeps the old credential valid | `a_rotated_or_revoked_credential_no_longer_authenticates` | "the rotated-out credential is refused" |
| Authentication ignores revocation | same test | "the rotated-out credential is refused" |
| Revocation does nothing | same test | "a revoked credential is refused" |
| `received` written outside the command's transaction | `before_commit_nothing_is_accepted_and_the_same_command_applies` | "no bytes were accepted". Four other crash tests also failed, incidentally, on the extra revision |
| The deduplication record not consulted | `after_commit_before_the_acknowledgment_the_retry_replays_and_appends_nothing` | the retry, `precondition_failed` instead of a replay; also the purge replay in the collection test |
| The seal's row committed before its object | `an_object_published_before_its_seal_row_is_collected_and_never_served` | "the object was published before the kill" |
| The purge deletes before it commits | `a_purge_killed_before_deletion_is_finished_at_restart_and_rechecks_roots` | "the kill landed before the deletion" |
| No start-time collection | the orphan test · the collection test | "the orphan was collected at start" · "the interrupted deletion was finished at start" |
| A confirmed purge still counted as a root | the collection test | "the interrupted deletion was finished at start" |
| Roots ignore every artifact | the collection test | "the kill landed before the deletion" — **earlier than the shared-object assertion it was aimed at**: the pass at the previous start had already deleted every object |
| The data-directory lock never refuses | `a_second_provider_over_the_same_data_directory_refuses_to_start` | "a second provider must not start" |
| *Probe:* the before-commit barrier moved past the commit | the before-commit test · `a_killed_upload_stays_staged_is_never_served_and_resumes` | "no bytes were accepted" · `received` |
| *Probe:* the after-commit barrier moved before the commit | the after-commit test | "the unacknowledged append is durable" |

The two **probes** move a barrier, not product code. They show the tests can tell which side of a commit the kill landed on. **The during-upload row has no product-defect mutant; only the probe fails it.**

### Coverage limits

- **`SIGKILL` is not power loss.** It cannot distinguish `synchronous=FULL` from `OFF` or a synced file from an unsynced one; that durability rests on the PRAGMA read back and on `fsync` of objects and directories, not on a test.
- The crash-matrix kills are over stdio. The socket's own `SIGKILL` test is stage (d)'s.
- Recovery from an **error** rather than a crash waits for the next start: an object whose seal commit failed, and the bytes of a purge whose deletion failed — the one case where `purged` is visible while bytes remain on disk, never served.
- Only evidence artifacts are roots. M3's packets must become roots before they are published into the object store.
- A partial `cbr ingest` is not resumed; the staged artifact stays staged and is not timed out in production.
- The different-user peer check, the 2 `socket` and 5 `core` fixtures that declare `execution`, and the absence of fixture evidence for `core.effects` and `core.events.backpressure`: all unchanged from earlier stages.
- Every committed transcript set, m1b through m1e, contains the runner's per-run temporary directory, which on macOS is a per-user path under `/var/folders`. It carries no user name and is not one of the paths `result_paths.py` refuses.

## Earlier — M1 stage (d)

The Unix-socket binding. Three commits: G6 and G7 recorded (`a339a1a`), the binding with `core.authenticate` and the recheck barrier (`1693243`), and durability, results and documentation.

| Command | Exit | Result |
|---|---|---|
| `check_docs.py` / `verify_pin.py` | 0 / 0 | 21 files, 0 errors; 429 and 420 files match their anchors |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | **63 tests**: 19 encoding, 36 provider unit, 8 against the real binary |
| `git diff --check` | 0 | — |
| `run_fixtures.py --filter stream.` + `check_results.py` | 0 | 24 of 24, unchanged |
| `run_fixtures.py --filter core.` + `check_results.py` | 0 | 130 / 5 / 0, unchanged |
| `run_fixtures.py --filter socket. --participant …unix.json` + `check_results.py` | 0 | **11 pass, 2 unsupported by name, 0 fail**, stable over three runs |
| `result_paths.py conformance/results/*/` | 0 | no machine paths in m1b, m1c2, m1c3, m1c4, m1d |

**Expected versus measured.** `socket.json` was derived from declared profiles, features, controls and barriers: 11 / 2 / 0 with the barrier. An unimplemented run gave 2 pass, 8 fail, 3 unsupported — and **the 2 passes were vacuous**: `malformed-clock-file-refused` and `unsafe-directory-refused` need only a failed start, and the old binary failed on the unknown `--socket` flag. With the binding and the barrier undeclared: 10 / 3 / 0. With the barrier declared, in the commit that implements its pause point: **11 / 2 / 0, as predicted.**

### What changed

- `socket.rs`: pathname socket `0600`; directory refused unless a real directory owned by this user with no group or other permissions, checked before the store opens; peer uid by `getpeereid` / `SO_PEERCRED`, a different user closed without a frame; one thread per connection; lifetime tied to standard input.
- `Provider::connect`: a further session with its own store connection and the process's shared clock, and none of the start-time effects.
- A process-wide processing lock serializes requests and subscription re-checks; contention signals `processing.lock.contended`. The barrier `subscription.recheck.after_authorization` sits between re-authorization and the event read under that lock.
- `core.authenticate` on its real path: unauthenticated sessions may call only `core.describe` and `core.authenticate`; constant-time digest comparison across every stored credential; one indistinguishable failure; only digests kept.
- Idle sessions poll every 40 ms; a frame split across a timeout is kept; idle wakes re-check subscriptions, deliver other sessions' events, and mark overdue obligations.
- The conformance default principal is `conformance-caller`; an explicit principal wins (a first version overwrote it, and five SIGKILL tests failed until fixed).

### Mutants

All observed, all restored.

| Mutant | Killed by | At |
|---|---|---|
| No authentication required | `authentication-required-before-negotiation` 2 · `feature-dependencies-after-authentication` 2 · `authentication-failures-indistinguishable` 4 | `expected error authentication_required` |
| Socket directory with group or other access accepted | `socket.unsafe-directory-refused` | step 0, "participant listened" |
| Malformed clock file at start accepted | `socket.malformed-clock-file-refused` | step 0, "participant listened" — **matches the fixture's declared `clock-file-start-unchecked` kill: step 0, reason contains "listened"** |
| Revoked credential accepted | `authentication-failures-indistinguishable` | step 3, `received success {"principal":"mallory"}` |
| Re-check outside the processing lock | `socket.subscription-recheck-race-regression` | step 13, `params/ended: missing` — **exactly the fixture's declared `recheck-outside-lock` kill: step 13, same reason** |
| Idle sessions inert (no re-check, no delivery) | `events-delivered-across-sessions` 6 · `idle-subscription-ends-at-grant-expiry` 7 · `idle-subscription-ends-on-revocation-elsewhere` 7 · `subscription-recheck-race-regression` 13, all timeouts | — |
| A connection advances the deduplication generation | `two_socket_sessions_one_mid_subscription_survive_sigkill` | the writer's first put refused `dedupe_history_unavailable`, before the kill |

**Named honestly.** The idle mutant is broader than the fixtures' `idle-subscriptions-not-rechecked`: it also stops cross-session delivery, so `idle-subscription-ends-at-grant-expiry` failed at step 7, not the declared step 9. It is recorded under its own name rather than claimed as that kill. The peer-check refusal has **no mutant**, because nothing here can observe it.

### Coverage limits

- **Different-user peer check:** a coverage limit, not a pass. Protocol's CI tests it only as root through passwordless `sudo`; this machine has none (`sudo -n true` asks for a password).
- The 2 `socket` fixtures declaring `execution`, permanently.
- `evidence` (16) not run.
- No production credential administration exists: a production socket launch has no way to issue a credential yet. Stage (e)'s CLI needs one, and brings the handoff file of CORE §18.1.

## Earlier — M1 stage (c4)

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
