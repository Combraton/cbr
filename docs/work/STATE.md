# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. Active task: [issue #3](https://github.com/Combraton/cbr/issues/3), milestone M1 stage (a). Branch `m1/walking-skeleton`, stacked on `readiness/standalone-0.1` until [PR #2](https://github.com/Combraton/cbr/pull/2) merges, then rebased onto `main`. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Readiness milestone:** complete and reviewed. PR #2 at `877139f` is acceptable to the reviewer, pinned for merge at that commit; merging is the owner's action and has not happened. All nine decisions are recorded in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md).
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M1 stage (a)

Vendored the pinned Protocol material and implemented `encoding/1`: a strict reader for the JSON value domain, canonical form, digests, and command intent.

- **Vendored** 430 files at `vendor/protocol/v0.1.0/`, copied from the verified release archive and **not** from the sibling `protocol` checkout: `docs/spec/`, `schemas/`, `conformance/{fixtures,vectors,schemas,runner}/`, the normative `inventory.json`, `BUNDLE-SHA256SUMS`, `LICENSE` and `RELEASE-SOURCE.json`. The reference provider, the independent Python provider and Protocol's own participant descriptors are deliberately absent; CBR writes its own implementation and its own test controls.
- **`scripts/verify_pin.py`** checks the vendored material against three anchors CBR does not control, so a fresh session can re-verify without trusting this repository.
- **`crates/cbr-encoding`** implements the domain by hand rather than over a general JSON parser, because the rules that matter are the ones a permissive parser discards: duplicate members collapse, `1.0` and `1` compare equal, and large integers round — any of which would let two different commands produce one digest.
- **`scripts/check_docs.py`** now skips `vendor/`, whose Markdown belongs to another repository and is checked by `verify_pin.py` instead.

## Checks actually run, with exit status

On this machine, `rustc 1.97.1`, macOS 25.3.0 arm64, at the committed revision. Every command in [VERIFICATION](../VERIFICATION.md):

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/check_docs.py` | 0 | 21 files, 104 links, 0 errors |
| `python3 scripts/verify_pin.py` | 0 | 429 files match `BUNDLE-SHA256SUMS`, 420 match the normative inventory, listing sha256 `80b39377b1…` |
| `cargo fmt --all -- --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `cargo build --workspace --locked` | 0 | — |
| `cargo test --workspace --locked` | 0 | 19 tests: 4 vector tests covering all 31 pinned cases, 15 property tests |
| `git diff --check` | 0 | — |

**Mutation evidence, so the suite is known to discriminate rather than merely be green.** Two guards were removed and the tests rerun:

| Mutant | Killed by | Also caught by a pinned vector? |
|---|---|---|
| Sort object members by code point instead of UTF-16 code units | `canonical_cases_round_trip_to_exact_bytes_and_digests` **and** `member_order_is_utf16_not_code_point` | **Yes** — the pinned `key-order-utf16-rfc8785` vector catches it alone |
| Let extensions not named in `requires` into the command intent | `command_intent_matches_the_pinned_vector` | **Yes** — the pinned intent vector |
| Tolerate a `requires` entry containing a slash with no matching member in `extensions` | `a_requires_entry_with_a_slash_must_be_present_in_extensions` | **No** — no pinned vector covers it; this is the review defect below |
| Tolerate duplicate `requires` entries | `duplicate_requires_entries_are_refused` | **No** |

Each mutant was killed by exactly one test, run with `--no-fail-fast` so a later suite could not mask a kill. Every guard was restored and the suite returned to 19 passing.

## Defect found in review, and fixed here

`command_intent` accepted a `requires` entry containing a slash with no matching member in `extensions`, returning a valid intent with empty extensions. [CORE §5.1](../../vendor/protocol/v0.1.0/docs/spec/profiles/CORE.md) says a feature name never contains a slash, an extension key always contains one, "that is how a `requires` entry is told apart", and an entry containing a slash MUST also be present in `extensions` — a violation being `invalid_envelope`. The comment claiming the two were indistinguishable was wrong, and `IntentError::RequiredExtensionMissing` was dead code.

Two pinned fixtures confirm the required behaviour, and both would have failed at stage (c): `core.envelope.required-extension-must-be-present` sends exactly this envelope and requires `invalid_envelope` with `applied_count: 0`, and `core.envelope.requires-unique` requires the same for a duplicated entry. Both rules are now enforced in `command_intent`, because an envelope that violates either has no well-defined intent: the digest it would produce belongs to a command that must never be accepted.

`Cargo.toml` also gained `exclude = ["vendor"]`. This was already broken before the change: the vendored runner's manifest inherits `license.workspace` and `edition.workspace`, and because it sits inside this workspace Cargo tried to resolve that inheritance here and failed, since CBR has selected no licence. The runner is built from a fresh checksum-verified extraction instead, in stage (b).

- **Coverage limits, stated rather than implied:** no CBR provider, participant descriptor, store, packet or model call exists. **No conformance fixture suite has been run against CBR** — the Core 135, stream 24, socket 13 and Evidence 16 suites are M1's acceptance and are run in the stages that introduce the code they exercise. The conformance runner is vendored but not yet built or wired.
- **Awaiting the owner**, none of which blocks M1: the project licence; the two ADR 001 blanks (provider/model/ceiling/account, and the journey-6 pilot repository); and authorization to file the `build_digest` proposal on the protocol repository. Raised on issue #1.
- **Task resources:** no CBR process or service is running. A verified extraction of the release archive lives in this session's scratchpad only; re-create it from [PROTOCOL-PIN §6](readiness/PROTOCOL-PIN.md) if needed — the vendored copy plus `verify_pin.py` is the durable record.
- **Next action:** M1 stage (b), the stream binding against the 24 `stream` fixtures. That stage adds a script that builds the conformance runner from a verified extraction of the release archive with the release's own `Cargo.lock`, recording the archive SHA-256, source commit and lockfile SHA-256 into every results manifest; CBR's stdio participant descriptor, claiming only `core/1` and `core-test/1` and only the test controls actually implemented; a committed `results/m1b/` with the runner's `manifest.json` and transcripts; and a broken-frame-limit mutant run recorded here.
