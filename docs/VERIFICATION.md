# Verification available now

This repository contains architecture and development documentation and, since M1, a protocol provider and the `cbr` command. The two checks in this section validate documentation structure only; [Product checks](#product-checks) below cover the product.

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
python3 scripts/run_fixtures.py --filter core.   --out conformance/results/m1c4
python3 scripts/check_results.py conformance/results/m1c4 conformance/expectations/core-c4.json
python3 scripts/run_fixtures.py --filter socket. --participant conformance/participants/cbr-provider-unix.json --out conformance/results/m1d
python3 scripts/check_results.py conformance/results/m1d conformance/expectations/socket.json
python3 scripts/run_fixtures.py --filter evidence. --out conformance/results/m1e
python3 scripts/check_results.py conformance/results/m1e conformance/expectations/evidence.json
```

`build_runner.py` downloads the archive once and reuses it afterwards; `--archive PATH` uses a copy you already have and `--offline` refuses to download. It verifies the archive against the pinned SHA-256, verifies every extracted file against the archive's own `BUNDLE-SHA256SUMS`, checks the archive's recorded commit against the pin, and records the runner identity that `run_fixtures.py` stamps into every results manifest.

| Command | What it establishes | What it does not |
|---|---|---|
| `check_docs.py` | Documentation structure: entrypoints, the `CLAUDE.md` import, local link targets, balanced fences, no private machine paths | Nothing about the product |
| `verify_pin.py` | The vendored Protocol material matches the published release, against three anchors CBR does not control: the release's `BUNDLE-SHA256SUMS`, the recomputed inventory `listing_sha256` that also appears in the annotated tag message, and the inventory's own per-file digests | That CBR implements any contract correctly |
| `cargo fmt --all -- --check` | Formatting only | — |
| `cargo clippy … -D warnings` | Lints clean; warnings fail | Correctness |
| `cargo build --workspace --locked` | The workspace builds from the committed `Cargo.lock` with no dependency resolution | Runtime behaviour |
| `cargo test --workspace --locked` | **78 tests.** In `cbr-encoding`, every pinned encoding vector — 12 canonical, 18 rejected, 1 command intent — plus the property tests and the strict base64 codec, 20 in all; a rejected vector must be refused **for the reason the vector states**, so a parser that refused everything would fail. In `cbr-provider`, 41 unit tests and 14 that drive the real binary: production refuses `core-test/1` and every test control; state, events, a grant **and its revocation**, and a capability change **and its event** survive `SIGKILL`; **two concurrent socket sessions, one part-way through a subscription, survive `SIGKILL`** with every acknowledged write at its position and the subscriber's last cursor resuming to exactly what it had not seen; a consumer that stops reading standard output ends the process with the bound recorded; **the storage crash matrix** (below), each row killed at its boundary; and a second provider over the same data directory refuses to start. In `cbr-cli`, 3 that run the `cbr` binary against the real provider over its socket (below). **The only evidence for `core.events.backpressure` and `core.effects` is among these tests** (below). | Any profile conformance on its own |
| `build_runner.py` | The runner was built from the published release archive, with its own lockfile, and its identity is recorded | Anything about CBR |
| `run_fixtures.py --filter core.` | **130 of the 135 `core` fixtures pass, 5 are unsupported by name, none fails** — the most the suite allows a provider that never serves `execution/1`. | The 5, permanently (below). **Nothing about `core.effects` or `core.events.backpressure`**, which no fixture CBR can run exercises. |
| `run_fixtures.py --filter stream.` | Runs the suite and writes the runner's manifest, the transcripts, and a `cbr-run.json` sidecar. The manifest stays byte-for-byte the runner's own output. | Nothing on its own: it reports, it does not gate |
| `run_fixtures.py --filter socket. --participant …unix.json` | **11 of the 13 `socket` fixtures pass, 2 are unsupported by name, none fails.** | The 2, permanently: they declare `execution`. **Nothing about a different operating-system user** (below). |
| `run_fixtures.py --filter evidence.` | **All 16 `evidence` fixtures pass.** | Anything the fixtures do not do: they never kill the provider, never use the socket, and never run `cbr` |
| `result_paths.py conformance/results/*/` | No committed result carries a machine path — the same check `run_fixtures.py` applies when it records a run | That the results are correct |
| `check_results.py` | **The gate.** The outcome multiset matches a recorded expectation exactly: pass count, total, the named unsupported set, and zero `fail`, `timeout`, `harness_error` and `skipped`. | Every suite with no expectation file. `stream`, `core`, `socket` and `evidence` each have one. |

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

**What is and is not established.** `stream` passes completely; `core` passes every fixture a provider that never serves `execution/1` can pass. `socket` passes every fixture a provider that never serves `execution/1` can pass. `evidence` passes completely. The store **is** durable — SQLite in WAL mode with `synchronous=FULL`, verified by reading the PRAGMAs back from the open connection, and by tests that `SIGKILL` the provider and restart it over the same data directory. No packet, model call, Knowledge or Context code exists.

### The `cbr` command, and the credentials it needs

**`cbr ingest` and `cbr fetch` are a client, not a shortcut.** `cbr` is a separate binary that depends only on `cbr-encoding`: it opens the provider's Unix socket, authenticates, negotiates `core/1` with `core.events` and `evidence/1`, and uses `evidence.upload.prepare`, `append`, `seal` and `evidence.fetch`. It links no provider code and opens no store. `fetch` checks the bytes it assembled against the digest it asked for, which is the sealed digest, **before** writing anything, and writes nothing on a mismatch.

Issue #3's acceptance is `ingested_bytes_fetch_identically_after_sigkill_and_restart`: 3 MiB and 123 bytes, several chunks each way, ingested over the socket; the provider is killed with `SIGKILL` and started again over the same data directory; `cbr fetch` returns byte-identical content matching the sealed digest. The same test checks that a wrong digest is `artifact_digest_mismatch` and writes nothing, the modes of the credential file (`0600`), its directory (`0700`) and the socket (`0600`), and that the credential appears in no output of `cbr` or the provider. `fetch_refuses_bytes_that_do_not_match_the_sealed_digest` restarts the provider with the `serve_altered_bytes` control, so the provider serves altered bytes behind honest metadata: `cbr` refuses them. That test exists because the provider's own integrity check would otherwise hide whether `cbr` checks at all.

**Credentials (CORE §18.1).** A production socket start issues `ccred1.<principal>.<secret>` from `/dev/urandom` if no live handed-off credential exists, stores only its SHA-256 with its principal and revocation state, and writes it to `credentials/<principal>` in the data directory. `cbr-provider --rotate-credential` and `--revoke-credential PRINCIPAL` are administration commands that run beside a serving provider; `core.authenticate` reads the store each time, so both apply to the next authentication. `a_rotated_or_revoked_credential_no_longer_authenticates` shows the rotated-out and revoked credentials refused, and a restart reissuing after revocation.

**What `cbr` does not do.** An ingest interrupted part-way is not resumed by a later `cbr ingest`, which creates a new artifact; the partial one stays staged, is never served, and is not timed out, because staging timeouts exist only as a test control. The credential file protects against other operating-system users, not against programs running as the owner (CORE §18.3).

### The storage crash matrix

[STORAGE §5](https://github.com/Combraton/combraton/blob/main/docs/architecture/STORAGE.md) names the crash boundaries. **Each row that exists in CBR at M1 is fault-injected by killing the real provider at that line.** A test barrier inside the provider (decision 007) creates `<name>.reached` and waits; the test sees the marker, sends `SIGKILL`, confirms no response frame was written, restarts over the same data directory and asserts the durable fact through the protocol. The tests are in `crates/cbr-provider/tests/crash_matrix.rs`.

| Row | Killed at | Durable fact asserted after restart |
|---|---|---|
| Before command commit | `evidence.append.before_commit` | Nothing accepted: `received` and revision unchanged, no event; the same command ID then applies once, not as a replay |
| After accepted, before the caller received the acknowledgment | `evidence.append.after_commit` — the `SIGKILL` between append and acknowledgment | The append is durable; the same command ID replays with the original revision; `received` is not doubled; exactly one `appended` event; the sealed content is byte-identical |
| During artifact upload | idle between appends, then `evidence.append.before_commit` on the next | Still staged with the durable `received`; `evidence.fetch` is `not_found`; seal is `upload_incomplete`; no object, no `sealed` event; the upload resumes from `received` and fetches byte-identical |
| The orphan object (STORAGE §2) | `evidence.seal.after_object_published` — object published and verified from disk, row not committed | Before restart the object is on disk, complete. After: collected; the artifact is still staged at its old revision, `fetch` is `not_found`, no `sealed` event — **no receipt for the seal**. The same seal then applies and publishes again |
| During collection | `evidence.purge.after_commit` — row committed, object not deleted | The artifact is `purged` and fetch serves nothing; the deletion was finished at start; the purge replays. When another sealed artifact shares the bytes, **the object is kept** and still served for that one |

**Rows that do not exist in CBR at M1**, and so have nothing to kill: *after commit, before dispatch* — CBR dispatches no effects until M4's model calls; *after harness prompt write*, *after invocation dispatch right* and *after completion commit* — PIO's; *after seal, before adoption* and *after adoption, before the provider sees the decision* — Comreton adoption, which standalone CBR does not have; *during branch creation* — Knowledge, M2; *while CBR projections lag* — no projections exist before M2.

**Recovery is a start-time collection pass.** When a provider opens its store, before any session exists, it rechecks the roots — every sealed artifact whose purge, if any, is unconfirmed — and deletes every object no root names, and every staging file an interrupted publication left. Both crash orders are deliberate: a seal publishes before its row, so a crash leaves an unnamed object and never a row naming missing bytes; a purge commits before deleting, so a crash leaves a tombstone over bytes the pass then removes. **No reader can observe `purged` while those bytes remain**, unless deletion itself fails (below): in a running provider the deletion happens under the processing lock before the response or any event delivery, and after a crash the pass runs before anything is served. Because that pass deletes what no committed row names, **a second provider over the same data directory refuses to start**; the lock is a `flock` the kernel releases when the process dies.

Every row's test fails under at least one mutant, listed in STATE, including reversing each of the two orders, removing the pass, and writing `received` outside the command's transaction. **The during-upload row is failed only by a probe** that moves a barrier across the commit rather than breaking product code: it shows the test can tell which side of the commit the kill landed on, not that it catches a particular defect.

**What the crash matrix does not show.**

- **`SIGKILL` is process death, not power loss.** Writes the process completed survive in the operating system's cache, so these tests cannot tell `synchronous=FULL` from `OFF`, or an object file that was `fsync`ed from one that was not. Durability against power loss rests on the PRAGMA read back from the open connection and on `fsync` of each object and its directory, not on a test.
- The kills are made over stdio. The barriers sit in the command path both bindings share; the socket's own `SIGKILL` test is stage (d)'s.
- An error rather than a crash is recovered only at the next start: an object whose seal commit returned an error stays until then, and so do the bytes of a purge whose deletion returned an I/O error after its row committed. That second case is the one way a reader can see `purged` while the bytes are still on disk. Nothing serves them.
- Only evidence artifacts own objects in M1. Anything that later publishes into the object store, such as M3's packets, must become a root before it does, or the pass will delete it.

### The socket binding, and what it cannot show here

**The different-user branch of the peer check is a coverage limit on this machine, not a pass.** STREAM §6 requires a connection from another operating-system user to be closed without a frame. The socket directory is `0700`, so an ordinary second user cannot reach the socket at all; the only way to reach the provider's own peer check is a user that bypasses directory permissions. Protocol's CI does that by connecting as **root through passwordless `sudo`**, and root is the only other user it tests. This machine has no passwordless `sudo` (`sudo -n true` asks for a password), so CBR exercises only the positive control: a unit test that a same-user connection's peer is this user. The refusal is implemented with `getpeereid` on macOS and `SO_PEERCRED` on Linux, and has no test that could fail if it were removed.

**Two socket fixtures passed vacuously before the binding existed.** `socket.malformed-clock-file-refused` and `socket.unsafe-directory-refused` require only that the provider fails to start, and an unimplemented binary failed on the unknown `--socket` flag. They are now backed by mutants: removing either check makes its fixture fail at step 0 with "participant listened".

**Committed socket transcripts contain the runner's synthetic test credentials.** The conformance README says they will: the runner derives a deterministic credential per principal from the run's temporary directory and authenticates each session with it. They are not real credentials and grant nothing outside that run. The redaction applied to machine paths does not apply to them.

### Two features with no fixture evidence

**`core.effects` and `core.events.backpressure` have no fixture evidence in CBR's role.** At the pin, 70 fixtures declare one or both features, across `core`, `socket`, `execution`, `composition`, `compat` and `verification`, and **every one of the 70 also declares the `execution` profile**, which CBR never serves, so none can run against CBR. One further fixture, `core.negotiation.execution-requests-against-any-provider`, mentions `core.effects` inside a negotiation request without declaring it; it runs and passes, and exercises nothing about effects. The `execution` fixtures that exercise effect semantics through execution operations — for example `execution.unknown-effect-outcome-is-not-not-found`, `execution.aborting-an-obligation-leaves-the-effect-unknown` and `execution.effects-resolve-only-with-target-authority` — informed CBR's tests, which is not the same as passing them. Both features are implemented and declared, and **both rest entirely on CBR's own tests**, each mutation-checked:

- **`core.events.backpressure`** — `session::tests` drive the real session loop and provider against a writer the test holds or delays. They show a consumer that never reads is declared too slow at the room deadline and closed within twice the notice budget, with output before closure measured within the bound and three subscriptions sharing one notice budget; a session that did not negotiate the feature is closed without a notice; a consumer that returns within the budget receives `consumer_too_slow`; and withheld notifications are delivered later, in order, with none skipped. One integration test shows the same over real pipes: an unread standard output ends the process, and the ending is recorded on standard error with the bound. **The session thread does wait on a stalled consumer — for the room deadline and no longer, and never inside a write**; that wait is the backpressure. A command's commit never waits on output.
- **`core.effects`** — unit tests on the record and provider tests through `handle`: an effect recorded in its command's transaction and read back whole; reading one under a grant refuses an existing and an absent effect identically; an obligation past its deadline is marked overdue once, by a provider-origin event, and is not satisfied by the wait ending; and aborting it leaves an `unknown` effect `unknown`. **Nothing in M1 produces an effect**; CBR's first real effects are model calls in M4.

A fixture passing is evidence another implementation agrees; these tests are evidence CBR does what its authors read the specification to require. They are not the same kind of evidence, and neither feature should be described as conformance-tested.

**Declared features are not the whole dependency.** The c3 expectation was computed from each fixture's declared features and confirmed by a measured run, and it was still one fixture short of the truth: `core.events.visibility-follows-direct-read-authority` declares only `core.events` and `core.grants`, but restarts with a changed capability and expects the provider-origin `core.capabilities.changed` event that the change records. Event recording is not gated on negotiation (CORE §16.3), so the fixture is right and the model was incomplete. CBR therefore records the capability snapshot and its change event now; the `core.capabilities` query and the `capability_unavailable` refusal remain c4, and the feature is not claimed. A later expectation computed the same way should be treated as a lower bound on what a stage needs, not an exact one. The c4 expectation was checked for the same effect and none was found: all 130 passed with only `core.capabilities` implemented.

**One test control is implemented and declared:** `clock.file`. The provider reads protocol-visible time — grant expiry and `recorded_at` — only from its clock. Under `clock.file` it follows forward writes, ignores backward and malformed ones so an expired grant stays expired, and refuses to start if the file is malformed at launch, when there is no last good instant to keep.

`core-test/1` is served **only** under a `combraton-conformance-config/1` launch configuration. A provider launched in production serves no `core-test/1`, answers `method_not_found` to its operations, and **refuses** a production configuration naming any test control rather than ignoring it. Each of those is a test, and each has a mutant that the test kills.

Committed results live under `conformance/results/`. The `manifest.json` in each is the runner's own output, unedited. Beside it, `cbr-run.json` records the runner's identity — the release archive SHA-256, the source commit and the release `Cargo.lock` SHA-256 — so a fixture outcome names the exact runner that produced it, along with a correction for the runner's `suite.protocol_commit`, which records the Git checkout enclosing `--repo` and therefore names CBR rather than Protocol. **Transcripts from `m1c3` onward have one declared substitution:** the runner expands `{repo}` in the launch argv into the checkout's absolute path, which names the machine and its owner in a public repository, so `run_fixtures.py` replaces that prefix with `{cbr_checkout}`, records the substitution and the file count in `cbr-run.json`, and refuses to finish if any other machine path remains. The earlier `m1b` and `m1c2` transcripts predate this and were **sanitised in place after the fact**, with no history rewrite: the same substitution, a note in each `cbr-run.json` saying when and where the original bytes remain in git history, and no change to any manifest or outcome. `scripts/result_paths.py` holds the one substitution and the one check; `run_fixtures.py` applies it when recording, and CI runs it over every committed results directory.

When reporting a result, give the command, its exit status, the environment and the tested revision. Preserve the producing command's exit status when shortening output: piping a failing build into a successful `tail` or `grep` reports success, and a shortened log is not evidence that the command passed.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios/results, not this service's normative contract. No runtime benchmark has been implemented or run by the documentation setup.
