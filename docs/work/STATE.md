# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. Active task: [issue #3](https://github.com/Combraton/cbr/issues/3), milestone M1 stage (b). Branch `m1/walking-skeleton`, stacked on `readiness/standalone-0.1` until [PR #2](https://github.com/Combraton/cbr/pull/2) merges, then rebased onto `main`. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Readiness milestone:** complete and reviewed. PR #2 at `877139f` is acceptable to the reviewer, pinned for merge at that commit; merging is the owner's action and has not happened. All nine decisions are recorded in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md).
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M1 stage (b)

The stream binding, and the minimum command path the `stream` fixtures exercise.

- **`scripts/build_runner.py`** builds the conformance runner from a fresh, checksum-verified extraction of the release archive, with the release's own `Cargo.lock`. It is deliberately not a workspace member: the vendored manifest inherits `license.workspace` and `edition.workspace`, and the vendored subset has no lockfile. It records the archive SHA-256, source commit and lockfile SHA-256.
- **`scripts/run_fixtures.py`** runs fixtures against CBR and stamps that identity into every manifest, so a fixture outcome names the runner that produced it. It also records a correction: the runner's own `suite.protocol_commit` comes from the Git checkout enclosing `--repo`, so it names **CBR's** HEAD, not Protocol's.
- **`conformance/participants/cbr-provider.json`** claims only `core/1` and the conformance-only `core-test/1`, with **no features and no test controls**, because none is implemented. Its launch path is relative to `--repo`, so the committed descriptor holds no machine-specific path.
- **`crates/cbr-provider`** implements the stream binding, the JSON-RPC mapping, negotiation, and the Core command path in the order CORE §10 fixes, plus all four `core-test/1` operations.

## Checks actually run, with exit status

On this machine, `rustc 1.97.1`, macOS 25.3.0 arm64, at the committed revision.

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/check_docs.py` | 0 | 21 files, 0 errors |
| `python3 scripts/verify_pin.py` | 0 | 429 files match `BUNDLE-SHA256SUMS`, 420 match the normative inventory, listing sha256 `80b39377b1…` |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | 26 tests |
| `git diff --check` | 0 | — |
| `python3 scripts/build_runner.py` | 0 | 539 files match `BUNDLE-SHA256SUMS`; runner built with release `Cargo.lock` `91827bbe11…` |
| `python3 scripts/run_fixtures.py --filter stream. --out conformance/results/m1b` | 0 | 24 of 24 `pass` |
| `python3 scripts/check_results.py conformance/results/m1b conformance/expectations/stream.json` | 0 | Outcome multiset matches the recorded expectation exactly |

Results and transcripts are committed under `conformance/results/m1b/`.

## Mutants

| Mutant | Killed by | At | Caught by a pinned vector? |
|---|---|---|---|
| Sort object members by code point instead of UTF-16 code units | `canonical_cases_round_trip_to_exact_bytes_and_digests`, `member_order_is_utf16_not_code_point` | — | Yes |
| Let extensions not named in `requires` into the command intent | `command_intent_matches_the_pinned_vector` | — | Yes |
| Tolerate a `requires` entry containing a slash with no matching member in `extensions` | `a_requires_entry_with_a_slash_must_be_present_in_extensions` | — | No |
| Tolerate duplicate `requires` entries | `duplicate_requires_entries_are_refused` | — | No |
| **Never apply the binding's pre-negotiation frame limit** | `stream.frame-limit-raised-after-negotiation` **and** `stream.frame-over-limit-closes` | both at **step 2**, reason `expected error frame_too_large, received success` | — |
| Decide `max_array_items` before `max_depth` | `depth_outranks_every_other_limit` | — | — |
| Count scalars toward nesting depth | `a_value_exactly_at_a_limit_is_within_it`, `array_items_outrank_string_bytes`, `scalars_do_not_add_depth` | — | — |
| The gate itself: `core` results checked against the `stream` expectation | `check_results.py`, naming every discrepancy | — | — |

Every guard was restored and the suite returned to 24 of 24 passing with 19 unit tests green.

## Corrections from review

- **`check_limits` reported the first violation in traversal order.** CORE §10 step 2 fixes the priority as depth, array items, string bytes, payload bytes, which is not the order one traversal finds them in. Each limit now gets its own pass. Writing the tests exposed a second, worse bug: the depth walk counted **scalars**, so `{"payload":{"a":[1,2]}}` measured depth 4 rather than 3 and a message at the limit was refused. §9 says scalars add nothing. Both are mutation-checked.
- **`run_fixtures.py` edited the runner's own manifest.** It now writes a `cbr-run.json` sidecar beside it, so `manifest.json` stays byte-for-byte the runner's output and two runs can be compared without one having been edited by CBR.
- **The gate was "no coverage limits", which is not a gate.** The runner leaves `coverage_limits` empty even when fixtures are unsupported: measured here, the `core` suite gives `49 pass, 13 fail, 73 unsupported` with `coverage_limits: []`. Gates are now committed expectations under `conformance/expectations/` asserting the exact multiset and the exact named unsupported set, enforced by `scripts/check_results.py` in CI. Verified to discriminate by checking the `core` results against the `stream` expectation.
- **"135 core fixtures pass" is unattainable** and is removed. Five declare the `execution` profile, which CBR never serves, and no other participant role can satisfy them — they are single-participant stdio fixtures with `role: provider`, not compositions. Maximum is 130 of 135, recorded as a permanent coverage limit in [VERIFICATION](../VERIFICATION.md).

## A defect this stage found in its own scope

The first fixture run was 23 of 24. `stream.method-operation-mismatch-refused` failed because the provider's known-operation set listed only the operations the fixtures happened to call. `core-test.subject.get` **is** a real operation of `core-test/1` — its schemas are in the pinned release — so CORE §10 step 1 wrongly reported `method_not_found` for a known operation, hiding the method/operation mismatch that step 2 owns. The set is now the profile's whole set, and `core-test.subject.get` and `core-test.authority.claim` are implemented rather than named and then refused.

- **Coverage limits, stated rather than implied:** `stream` is the **only** suite run against CBR. `core` (135), `socket` (13) and `evidence` (16) have not been run and nothing here claims them. The store is **in memory and not durable** — no `stream` fixture requires durability, so the SQLite store from STACK §3 arrives with the Core suite, which does exercise restart. Until then nothing may claim CBR survives a restart with state intact. `core-test.authority.claim` and `core-test.subject.get` are implemented from their schemas but **unexercised** by this suite. No feature, test control, packet, model call, Knowledge or Context code exists.
- **Awaiting the owner**, none of which blocks M1: the project licence; the two ADR 001 blanks (provider/model/ceiling/account, and the journey-6 pilot repository); and authorization to file the `build_digest` proposal on the protocol repository. Raised on issue #1.
- **Task resources:** no CBR process or service is running. A verified extraction of the release archive lives in this session's scratchpad only; re-create it from [PROTOCOL-PIN §6](readiness/PROTOCOL-PIN.md) if needed — the vendored copy plus `verify_pin.py` is the durable record.
- **Stage (c) is split**, one pull request each, so every gate is honest. Expected outcomes computed from each fixture's declared features:

  | Stage | Claims | Expected |
  |---|---|---|
  | c1 | no features | 62 pass, 73 unsupported, 0 fail |
  | c2 | `core.events` | 80 pass, 55 unsupported, 0 fail |
  | c3 | `core.grants` | 122 pass, 13 unsupported, 0 fail |
  | c4 | `core.capabilities`, `core.effects`, `core.events.backpressure` | 130 pass, 5 unsupported, 0 fail |

  The 13 no-feature fixtures failing today are exactly the 13 `fail` results in the measured baseline. Note `core.effects` is named by no non-execution `core` fixture, so c4's evidence for it comes from elsewhere.
- **Next action:** M1 stage (c1) as its own pull request: the SQLite store from STACK §3 replacing the in-memory store, restart and dedupe generations across restart, PRAGMAs asserted by reading them back from the open connection, and a test proving a production-configured CBR refuses `core-test/1` at negotiation and rejects every test control. Superseded plan text follows for reference: the Core command path against the 135 `core` fixtures, with `core-test/1` reachable only through the conformance launch configuration and a test proving a production-configured CBR refuses `core-test/1` and every control. That stage replaces the in-memory store with the SQLite store from STACK §3 — WAL, `synchronous=FULL`, `BEGIN IMMEDIATE` — asserted by reading the PRAGMAs back from the open connection, and implements the features the Core suite needs (`core.events`, `core.grants`, `core.capabilities`).
