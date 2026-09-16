# Verification available now

These repositories contain architecture and development setup, not product runtimes. The checks below validate documentation structure only.

From this repository's root:

```sh
python3 scripts/check_docs.py
git diff --check
```

Python 3 standard library is sufficient; there is no package install step. The script checks required entrypoints, the local `CLAUDE.md` import, ordinary Markdown file targets, balanced fences and private machine paths in Markdown. It exits nonzero on an error. It reports cross-repository links it could not check.

When all five clones are siblings under one directory, also run from each repository root:

```sh
python3 scripts/check_docs.py --workspace ..
```

This additionally resolves Combraton GitHub main-file links against the sibling checkouts, including benchmarks. It does not prove those checkouts match the remote branches. Record their commits when using the result as integration evidence.

Two GitHub Actions workflows run on pushes and pull requests, both with read-only contents permissions and neither fetching sibling repositories. The **Documentation** job runs `check_docs.py` alone. The **Checks** job runs every command in [Product checks](#product-checks) below, as separate steps, on both `ubuntu-latest` and `macos-latest` — the two platforms Protocol 0.1 supports. No step pipes a command into another process, so a failing command's exit status is never replaced by a successful consumer's. Remote URL reachability, Markdown fragment targets, Mermaid rendering, source-manifest consistency, semantic correctness, live harness instruction loading and product behavior need separate inspection. The script is intentionally small and is not a general Markdown parser.

## Journey verification

[Journey verification](verification/JOURNEYS.md) holds the journey-level acceptance matrix and the per-journey evidence records, aligned with the [shared verification model](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md). No journey has been run. A journey record names its model or provider, or records `none` or `simulated`; a simulated run is never reported as live-model evidence, and `not_evaluated` never counts as a pass.

## Product checks

These exist and run. `rust-toolchain.toml` pins `rustc 1.97.1`, matching the toolchain the pinned Protocol release was built and tested with, so `rustup` selects it automatically. Python 3 standard library is sufficient for the two scripts; there is no package install step.

From the repository root:

```sh
python3 scripts/check_docs.py
python3 scripts/verify_pin.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
git diff --check
```

Conformance additionally needs the runner, which is **not** built from this workspace. The vendored runner's manifest inherits `license.workspace` and `edition.workspace` from the Protocol workspace, and the vendored subset carries no `Cargo.lock`, so building it here would need a licence CBR has not selected and could not use `--locked`. It is built from a fresh, checksum-verified extraction of the release archive instead, with the release's own lockfile:

```sh
python3 scripts/build_runner.py
python3 scripts/run_fixtures.py --filter stream. --out conformance/results/m1b
python3 scripts/check_results.py conformance/results/m1b conformance/expectations/stream.json
python3 scripts/run_fixtures.py --filter core.   --out conformance/results/m1c3
python3 scripts/check_results.py conformance/results/m1c3 conformance/expectations/core-c3.json
```

`build_runner.py` downloads the archive once and reuses it afterwards; `--archive PATH` uses a copy you already have and `--offline` refuses to download. It verifies the archive against the pinned SHA-256, verifies every extracted file against the archive's own `BUNDLE-SHA256SUMS`, checks the archive's recorded commit against the pin, and records the runner identity that `run_fixtures.py` stamps into every results manifest.

| Command | What it establishes | What it does not |
|---|---|---|
| `check_docs.py` | Documentation structure: entrypoints, the `CLAUDE.md` import, local link targets, balanced fences, no private machine paths | Nothing about the product |
| `verify_pin.py` | The vendored Protocol material matches the published release, against three anchors CBR does not control: the release's `BUNDLE-SHA256SUMS`, the recomputed inventory `listing_sha256` that also appears in the annotated tag message, and the inventory's own per-file digests | That CBR implements any contract correctly |
| `cargo fmt --all -- --check` | Formatting only | — |
| `cargo clippy … -D warnings` | Lints clean; warnings fail | Correctness |
| `cargo build --workspace --locked` | The workspace builds from the committed `Cargo.lock` with no dependency resolution | Runtime behaviour |
| `cargo test --workspace --locked` | **47 tests.** In `cbr-encoding`, every pinned encoding vector — 12 canonical, 18 rejected, 1 command intent — plus the property tests, 19 in all; a rejected vector must be refused **for the reason the vector states**, so a parser that refused everything would fail. In `cbr-provider`, 23 unit tests and 5 that drive the real binary: production refuses `core-test/1` and every test control, and state, events, a grant **and its revocation** survive `SIGKILL`. | Any profile conformance on its own |
| `build_runner.py` | The runner was built from the published release archive, with its own lockfile, and its identity is recorded | Anything about CBR |
| `run_fixtures.py --filter core.` | **122 of the 135 `core` fixtures pass, 13 are unsupported by name, none fails.** That is every fixture needing no feature beyond `core.events` and `core.grants`, including the two that need the `clock.file` test control. | The 13, which all need `core.capabilities` and arrive in c4; five of them are permanently unsupported (below). |
| `run_fixtures.py --filter stream.` | Runs the suite and writes the runner's manifest, the transcripts, and a `cbr-run.json` sidecar. The manifest stays byte-for-byte the runner's own output. | Nothing on its own: it reports, it does not gate |
| `result_paths.py conformance/results/*/` | No committed result carries a machine path — the same check `run_fixtures.py` applies when it records a run | That the results are correct |
| `check_results.py` | **The gate.** The outcome multiset matches a recorded expectation exactly: pass count, total, the named unsupported set, and zero `fail`, `timeout`, `harness_error` and `skipped`. | Every suite with no expectation file. `stream` and `core` have one; `socket` and `evidence` do not. |

### Why the gate is an outcome multiset

The runner's own `coverage_limits` list **stays empty even when fixtures are reported `unsupported`**. Measured against this provider, the `core` suite gives `49 pass, 13 fail, 73 unsupported` with `coverage_limits: []`. So "no coverage limits" is not a gate — a run in which every fixture was skipped for want of a claimed feature would satisfy it. Each suite therefore has a committed expectation under `conformance/expectations/`, naming the exact counts and the exact fixtures expected to be unsupported, and `check_results.py` refuses anything else.

### A permanent coverage limit: five `core` fixtures CBR can never pass

Five of the 135 `core` fixtures declare the `execution` profile and require the `executor.script` test control:

| Fixture | Also needs |
|---|---|
| `core.events.older-consumer-is-closed-without-notice` | `execution.output` |
| `core.events.slow-consumer-is-closed-with-notice-and-resumes` | `execution.output`, `core.events.backpressure` |
| `core.events.slow-consumer-recovery-reports-retention-gap` | `execution.output`, `core.events.backpressure` |
| `core.events.unread-ending-notice-does-not-hold-the-connection` | `execution.output`, `core.events.backpressure` |
| `core.feature-dependencies-match-negotiation` | `execution.context`, and the `evidence`, `context`, `knowledge` and `verification` profiles |

**CBR never serves `execution/1`** — the [consumer handoff](https://github.com/Combraton/protocol/blob/main/docs/work/release-0.1/CONSUMERS.md) states it plainly, and [ADR 001](decisions/001-standalone-v0.1-scope-and-stack.md) keeps it a client only. **No other participant role can satisfy them either:** these are single-participant `core/` fixtures over stdio with `role: provider`, not compositions, so there is no second role for CBR to occupy. `core.feature-dependencies-match-negotiation` would additionally need CBR to serve `verification/1`, which the agreed release scope excludes.

So the maximum attainable on the `core` suite is **130 of 135**, with exactly those five `unsupported`. Any statement of the form "all 135 core fixtures pass" is unattainable and must not be written.

**What is and is not established.** `stream` passes completely; `core` passes every fixture whose features are within `core.events` and `core.grants`. The `socket` (13) and `evidence` (16) suites have **not** been run and no claim is made about them. The store **is** durable — SQLite in WAL mode with `synchronous=FULL`, verified by reading the PRAGMAs back from the open connection, and by tests that `SIGKILL` the provider and restart it over the same data directory. No packet, model call, Knowledge or Context code exists.

**Declared features are not the whole dependency.** The c3 expectation was computed from each fixture's declared features and confirmed by a measured run, and it was still one fixture short of the truth: `core.events.visibility-follows-direct-read-authority` declares only `core.events` and `core.grants`, but restarts with a changed capability and expects the provider-origin `core.capabilities.changed` event that the change records. Event recording is not gated on negotiation (CORE §16.3), so the fixture is right and the model was incomplete. CBR therefore records the capability snapshot and its change event now; the `core.capabilities` query and the `capability_unavailable` refusal remain c4, and the feature is not claimed. A later expectation computed the same way should be treated as a lower bound on what a stage needs, not an exact one.

**One test control is implemented and declared:** `clock.file`. The provider reads protocol-visible time — grant expiry and `recorded_at` — only from its clock. Under `clock.file` it follows forward writes, ignores backward and malformed ones so an expired grant stays expired, and refuses to start if the file is malformed at launch, when there is no last good instant to keep.

`core-test/1` is served **only** under a `combraton-conformance-config/1` launch configuration. A provider launched in production serves no `core-test/1`, answers `method_not_found` to its operations, and **refuses** a production configuration naming any test control rather than ignoring it. Each of those is a test, and each has a mutant that the test kills.

Committed results live under `conformance/results/`. The `manifest.json` in each is the runner's own output, unedited. Beside it, `cbr-run.json` records the runner's identity — the release archive SHA-256, the source commit and the release `Cargo.lock` SHA-256 — so a fixture outcome names the exact runner that produced it, along with a correction for the runner's `suite.protocol_commit`, which records the Git checkout enclosing `--repo` and therefore names CBR rather than Protocol. **Transcripts from `m1c3` onward have one declared substitution:** the runner expands `{repo}` in the launch argv into the checkout's absolute path, which names the machine and its owner in a public repository, so `run_fixtures.py` replaces that prefix with `{cbr_checkout}`, records the substitution and the file count in `cbr-run.json`, and refuses to finish if any other machine path remains. The earlier `m1b` and `m1c2` transcripts predate this and were **sanitised in place after the fact**, with no history rewrite: the same substitution, a note in each `cbr-run.json` saying when and where the original bytes remain in git history, and no change to any manifest or outcome. `scripts/result_paths.py` holds the one substitution and the one check; `run_fixtures.py` applies it when recording, and CI runs it over every committed results directory.

When reporting a result, give the command, its exit status, the environment and the tested revision. Preserve the producing command's exit status when shortening output: piping a failing build into a successful `tail` or `grep` reports success, and a shortened log is not evidence that the command passed.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios/results, not this service's normative contract. No runtime benchmark has been implemented or run by the documentation setup.
