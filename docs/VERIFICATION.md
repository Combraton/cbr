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

[Journey verification](verification/JOURNEYS.md) holds the journey-level acceptance matrix and the per-journey evidence records, aligned with the [shared verification model](https://github.com/Combraton/combraton/blob/main/docs/architecture/VERIFICATION.md). J9 has been run, with no model; no other journey has. A journey record names its model or provider, or records `none` or `simulated`; a simulated run is never reported as live-model evidence, and `not_evaluated` never counts as a pass.

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
python3 scripts/run_fixtures.py --filter knowledge. --out conformance/results/m2
python3 scripts/check_results.py conformance/results/m2 conformance/expectations/knowledge.json
```

Source identity shells out to `git`, so the tests need `git` on the path; both CI runners have it.

`build_runner.py` downloads the archive once and reuses it afterwards; `--archive PATH` uses a copy you already have and `--offline` refuses to download. It verifies the archive against the pinned SHA-256, verifies every extracted file against the archive's own `BUNDLE-SHA256SUMS`, checks the archive's recorded commit against the pin, and records the runner identity that `run_fixtures.py` stamps into every results manifest.

| Command | What it establishes | What it does not |
|---|---|---|
| `check_docs.py` | Documentation structure: entrypoints, the `CLAUDE.md` import, local link targets, balanced fences, no private machine paths | Nothing about the product |
| `verify_pin.py` | The vendored Protocol material matches the published release, against three anchors CBR does not control: the release's `BUNDLE-SHA256SUMS`, the recomputed inventory `listing_sha256` that also appears in the annotated tag message, and the inventory's own per-file digests | That CBR implements any contract correctly |
| `cargo fmt --all -- --check` | Formatting only | — |
| `cargo clippy … -D warnings` | Lints clean; warnings fail | Correctness |
| `cargo build --workspace --locked` | The workspace builds from the committed `Cargo.lock` with no dependency resolution | Runtime behaviour |
| `cargo test --workspace --locked` | **92 tests.** In `cbr-encoding`, every pinned encoding vector — 12 canonical, 18 rejected, 1 command intent — plus the property tests and the strict base64 codec, 20 in all; a rejected vector must be refused **for the reason the vector states**, so a parser that refused everything would fail. In `cbr-identity`, 4 property tests of source identity against real git repositories, each with its negative control (below). In `cbr-provider`, 46 unit tests and 17 that drive the real binary: production refuses `core-test/1` and every test control; state, events, a grant **and its revocation**, a capability change **and its event**, and **a claim and its decision** survive `SIGKILL` at their positions; **two concurrent socket sessions, one part-way through a subscription, survive `SIGKILL`** with every acknowledged write at its position and the subscriber's last cursor resuming to exactly what it had not seen; a consumer that stops reading standard output ends the process with the bound recorded; **the storage crash matrix** (below), each row killed at its boundary; a second provider over the same data directory refuses to start; **J9**; and **the derived-artifact ancestry control**. In `cbr-cli`, 5 that run the `cbr` binary against the real provider over its socket (below). **The only evidence for `core.events.backpressure` and `core.effects` is among these tests** (below). | Any profile conformance on its own |
| `build_runner.py` | The runner was built from the published release archive, with its own lockfile, and its identity is recorded | Anything about CBR |
| `run_fixtures.py --filter core.` | **130 of the 135 `core` fixtures pass, 5 are unsupported by name, none fails** — the most the suite allows a provider that never serves `execution/1`. | The 5, permanently (below). **Nothing about `core.effects` or `core.events.backpressure`**, which no fixture CBR can run exercises. |
| `run_fixtures.py --filter stream.` | Runs the suite and writes the runner's manifest, the transcripts, and a `cbr-run.json` sidecar. The manifest stays byte-for-byte the runner's own output. | Nothing on its own: it reports, it does not gate |
| `run_fixtures.py --filter socket. --participant …unix.json` | **11 of the 13 `socket` fixtures pass, 2 are unsupported by name, none fails.** | The 2, permanently: they declare `execution`. **Nothing about a different operating-system user** (below). |
| `run_fixtures.py --filter evidence.` | **All 16 `evidence` fixtures pass.** | Anything the fixtures do not do: they never kill the provider, never use the socket, and never run `cbr` |
| `run_fixtures.py --filter knowledge.` | **All 10 `knowledge` fixtures pass**, with the `knowledge.store` control. | That CBR's memory is useful, or that any claim is true (KNOWLEDGE §14). No fixture uses `serve_altered_claims`, runs over the socket, or runs `cbr`. |
| `result_paths.py conformance/results/*/` | No committed result carries a machine path — the same check `run_fixtures.py` applies when it records a run | That the results are correct |
| `check_results.py` | **The gate.** The outcome multiset matches a recorded expectation exactly: pass count, total, the named unsupported set, and zero `fail`, `timeout`, `harness_error` and `skipped`. | Every suite with no expectation file. `stream`, `core`, `socket`, `evidence` and `knowledge` each have one. |

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

**What is and is not established.** `stream` passes completely; `core` passes every fixture a provider that never serves `execution/1` can pass. `socket` passes every fixture a provider that never serves `execution/1` can pass. `evidence`, `knowledge` and `context` pass completely. `composition` passes every fixture that needs neither `execution/1` nor `verification/1`, which CBR never serves. The store **is** durable — SQLite in WAL mode with `synchronous=FULL`, verified by reading the PRAGMAs back from the open connection, and by tests that `SIGKILL` the provider and restart it over the same data directory. No retrieval, packet compiler or model call exists: a packet's content comes only from the `context.script` test control.

### Knowledge, source identity and the knowledge verbs

**`knowledge/1` is served in full** (KNOWLEDGE §3–§11):
- claim revisions with the `combraton-knowledge-claim/1` record, whose digest any reader recomputes;
- support classes from declared ancestry;
- authority bindings, transfers and reliance decisions, including the author-is-decider case;
- conflicts and drift;
- the applicability evaluator over `repository_tree`, `dirty_snapshot` and `environment_digest`, with dependencies resolved only as exact local references;
- history with stream positions, rights and events.

**A claim revision is immutable in the database, not only in the code.** Revisions live in an insert-only table whose triggers refuse `UPDATE` and `DELETE`, and each revision commits in the same transaction as the claim subject it numbers. A store test shows both refusals.

33 mutants each fail a knowledge fixture at a named step. Two of them are recorded under CBR's own names rather than claimed as the fixture's narrower mutant.

**Source identity** (`crates/cbr-identity`, PROTOCOL-PIN §5):

| Identity | Definition | Negative control |
|---|---|---|
| `tree` | The git root tree object id, with the commit returned beside it. `cbr ingest --repo` records both as capture anchors. | Two commits with identical content are one tree; a content change is a new tree. |
| `dirty.snapshot_digest` | `sha256` over canonical `{ format, base_tree, entries: [path, status, mode, sha256 or null] }`, sorted by path, with deletions explicit. | A deletion, a mode change and an untracked file each change it. Modification time alone, on a modified or a clean file, staging, and an ignored file do not. |
| `environment` | `sha256` over a declared fact set that **includes build facts**. The record says so in `covers`, because Protocol 0.1 has no `build_digest` condition (G1). | A build fact changes it; the order a file lists facts in does not; a fact declared twice is refused. |

Each row has a mutant that fails its test.

**The derived-artifact ancestry control** (PROTOCOL-PIN §3) runs against the real provider:
- a captured log and two derivations over it are sealed, the derivations under `cbr.derivation.transcript`;
- a claim whose two support entries each declare the log as their complete root is `single_lineage`;
- the producer mistake — each derivation declaring itself as its own root — reads `multiple_lineages`, which is why the control exists;
- a provider mutant that takes each entry's own evidence as its root fails the control.

**J9** is recorded in [JOURNEYS](verification/JOURNEYS.md#j9-an-authority-transfer-invalidates-the-stale-decision-path). It passed with no model, and each of its two named controls has a mutant that fails.

**The `cbr` knowledge verbs** are `propose`, `revise`, `decide`, `evaluate`, `inspect`, `history`, `authority bind`, and the local `basis`. They use the public socket only:
- `revise` reads the current revision and digest through the protocol;
- `decide` reads the binding's epoch and the latest decision the same way;
- `evaluate --repo` computes the target's tree, dirty snapshot and environment digest.

`cbr-provider --issue-credential PRINCIPAL` hands a credential to a second principal. `knowledge_verbs.rs` runs a production provider in which a principal named `model` holds a grant that includes `knowledge.decide` and labels its derivation `human`. It still cannot accept its own claim: `not_authority`, with no record written. The owner's review is recorded with `author_is_decider: false`, and the owner's adoption of its own requirement with `true`. No model runs in this test. `model` is a labelled stand-in principal, not model evidence.

**What M2 does not establish.**
- **That any claim is true, that declared ancestry is honest, or that disjoint lineages are independent** (KNOWLEDGE §14).
- **Untested paths.** `serve_altered_claims` is implemented and has no test. A knowledge subject's visibility in events follows `knowledge.read` without a dedicated test. No fixture runs knowledge over the socket.
- **Source-identity gaps.** Not implemented:
  - the non-git directory tree of PROTOCOL-PIN §5;
  - the `workspace` clean/dirty determination under a lock. A snapshot is read without one, so a working tree changing during the read gives a snapshot of no single instant;
  - submodule contents, which are recorded as a gitlink with no digest.

  Identity needs the `git` binary; `gix` remains the M3 choice for bulk tree access.
- **Cost of reads.** `inspect` and `history` scan every record of a kind, which is proportionate to v0.1 counts and not measured beyond them.
- **CLI coverage.** `cbr` has no verb for conflicts, transfers or grants.

### Context: requests, packets and claims in packets

**`context/1` is served with all seven features** (CONTEXT §1–§14) — `context.advisory`, `context.required_before_start`, `context.required_before_transition`, `context.shared_jobs`, `context.updates`, `context.expand` and `context.claims` — over stdio and the Unix socket.
- **Requests.** Step 2 refuses an item that cannot be checked, a misplaced transition and a commit-only basis declared complete. Step 3 refuses an obligation, or a claim check, whose feature the session did not negotiate.
- **Results.** An item is satisfied only by content that meets its check. A required item missing at the deadline is `unmet`; an advisory one follows its fallback. The investigation budget, the output capacity and the deadline are three separate reasons, never reported as one another.
- **Capacity.** Output capacity is reserved for required items' sections before any advisory content, and mandatory content that cannot fit refuses the request with `budget_insufficient` and the size it needs.
- **Packets.** Each revision is an exact sealed evidence artifact of canonical `combraton-context-packet/1` bytes, or `/2` under `context.claims`. It is sealed in CBR's own store with CBR as producer principal, or at a separate evidence provider over that provider's public socket, as CBR's own principal under that provider's grant. A revision is never reported before its artifact is sealed. Updates are new revisions; an old revision keeps its artifact and is never current again.
- **Claims.** A claim is carried as an exact snapshot after its record digest is recomputed. A label never promotes an unaccepted claim, and only `invalid_for_target` makes a section stale. Reading a packet under `context.claims` re-reads each carried claim, locally or at a knowledge provider, and reports `claim_changes`, claim invalidations and unverified items. The published facts never change.
- **Reads.** `context.packet.inspect` serves an exact excerpt that never carries a digest. `context.expand` serves a cited artifact the reader may read, and refuses an unreadable, a nonexistent and a foreign citation identically.

**Where packet content comes from.** Every packet today is prepared by the `context.script` test control. The script says which sections, coverage, omissions, corrections and unmet reasons a job produces. CBR decides everything the protocol makes the provider responsible for: satisfaction, inclusion, labels, claim snapshots, sealing and the read-time facts. The provenance names the compiler `cbr-context-script`. A production launch refuses the control, so a production request has nothing to prepare it and is published at its deadline with every required item `unmet`; `a_request_nothing_prepares_is_published_unmet_at_its_deadline` shows the same path. The deterministic compiler over a real repository is m3c.

**The composition fixtures run separate CBR instances**, each with its own store, reaching one another only over public sockets under grants:
- a context provider sealing at a separate evidence provider, including while that provider is killed and restarted;
- one grant per audience for direct fetch;
- the protocol's independently written, client-only `minimal-executor`, vendored from the release archive, enforcing its dispatch boundary from CBR's read-time claim facts, with CBR reading claims at a separate knowledge provider.

The other 11 composition fixtures declare `execution/1` or `verification/1` and are permanently out of reach.

**CBR's own tests** (`crates/cbr-provider/tests/context.rs`, and unit tests in `context.rs` and `store.rs`) cover what no fixture reaches:
- a publication whose seal is held at the evidence provider past the peer timeout replays after the clock moves, because the first attempt's capture instant is kept, and the diagnostic it logs does not contain the peer credential;
- a packet killed after its object is published and before its batch commits is published exactly once after restart (the crash-matrix row below);
- a correction after publication is reported at the read beside `superseded_by`, and content derived from a corrected authority revision is stale whether prepared before or after the correction;
- capacity is reserved for required content whatever order sections were prepared in, which the reference provider does not do;
- a provider-origin batch commits whole or not at all.

Of 57 mutants, 48 fail a context or composition fixture at a named step and 9 fail only CBR's own tests; [STATE](work/STATE.md) lists every one.

**Packet bytes are kept by the context record too, and that is a retention limit.** A revision's published facts keep its body, which `context.packet.inspect` excerpts after re-encoding it canonically; it is served only while it still digests to the reference. So a packet sealed at a separate evidence provider has a copy at the context provider, and purging a packet artifact ends `evidence.fetch` of it but not its excerpt. Request retention is not implemented.

**What m3a does not establish.**
- **That CBR prepares useful context.** Scripted content is test environment (CONTEXT §12). Nothing is retrieved or compiled.
- **Investigation executions.** CBR has no executor role. A script step it does not perform, `execute`, holds its job rather than pretending it ran.
- **Responsiveness under a stalled peer.** A context provider calls peers while holding its processing lock, bounded by a three-second timeout per frame, so a stalled peer delays every request to that provider for up to that long. Preparation runs on every request and idle poll of an authenticated session, and its cost grows with the number of running jobs.
- **Untested paths.** Event visibility of `context.job` subjects under a grant follows the requests' `context.read` without a dedicated test. Skipping preparation for an unauthenticated connection has no test. A `packet.<request>.<n>` id that collides with an existing artifact stops that publication with a diagnostic and has no test.
- **Read cost.** A request record holds every published revision's facts, and each tick parses every job.

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
| A context packet's object published, its batch not committed (m3a) | `context.packet.after_object_published` — the packet's object published and verified, its request, job and artifact rows not committed | Nothing was reported before the kill. After restart exactly one revision is published, with one `sealed` and one `packet.published` event, and `evidence.fetch` serves bytes matching its digest. This row's test is in `tests/context.rs` |

**Rows that do not exist in CBR at M1**, and so have nothing to kill: *after commit, before dispatch* — CBR dispatches no effects until M4's model calls; *after harness prompt write*, *after invocation dispatch right* and *after completion commit* — PIO's; *after seal, before adoption* and *after adoption, before the provider sees the decision* — Comreton adoption, which standalone CBR does not have; *during branch creation* — Knowledge, M2; *while CBR projections lag* — no projections exist before M2.

**Recovery is a start-time collection pass.** When a provider opens its store, before any session exists, it rechecks the roots — every sealed artifact whose purge, if any, is unconfirmed — and deletes every object no root names, and every staging file an interrupted publication left. Both crash orders are deliberate: a seal publishes before its row, so a crash leaves an unnamed object and never a row naming missing bytes; a purge commits before deleting, so a crash leaves a tombstone over bytes the pass then removes. **No reader can observe `purged` while those bytes remain**, unless deletion itself fails (below): in a running provider the deletion happens under the processing lock before the response or any event delivery, and after a crash the pass runs before anything is served. Because that pass deletes what no committed row names, **a second provider over the same data directory refuses to start**; the lock is a `flock` the kernel releases when the process dies.

Every row's test fails under at least one mutant, listed in STATE, including reversing each of the two orders, removing the pass, and writing `received` outside the command's transaction. **The during-upload row is failed only by a probe** that moves a barrier across the commit rather than breaking product code: it shows the test can tell which side of the commit the kill landed on, not that it catches a particular defect.

**What the crash matrix does not show.**

- **`SIGKILL` is process death, not power loss.** Writes the process completed survive in the operating system's cache, so these tests cannot tell `synchronous=FULL` from `OFF`, or an object file that was `fsync`ed from one that was not. Durability against power loss rests on the PRAGMA read back from the open connection and on `fsync` of each object and its directory, not on a test.
- The kills are made over stdio. The barriers sit in the command path both bindings share; the socket's own `SIGKILL` test is stage (d)'s.
- An error rather than a crash is recovered only at the next start: an object whose seal commit returned an error stays until then, and so do the bytes of a purge whose deletion returned an I/O error after its row committed. That second case is the one way a reader can see `purged` while the bytes are still on disk. Nothing serves them.
- Only evidence artifacts own objects. A context packet sealed in CBR's own store is an evidence artifact, so it is a root like any other. Anything that later publishes into the object store outside an artifact must become a root before it does, or the pass will delete it.

### The socket binding, and what it cannot show here

**The different-user branch of the peer check is a coverage limit on this machine, not a pass.** STREAM §6 requires a connection from another operating-system user to be closed without a frame. The socket directory is `0700`, so an ordinary second user cannot reach the socket at all; the only way to reach the provider's own peer check is a user that bypasses directory permissions. Protocol's CI does that by connecting as **root through passwordless `sudo`**, and root is the only other user it tests. This machine has no passwordless `sudo` (`sudo -n true` asks for a password), so CBR exercises only the positive control: a unit test that a same-user connection's peer is this user. The refusal is implemented with `getpeereid` on macOS and `SO_PEERCRED` on Linux, and has no test that could fail if it were removed.

**Two socket fixtures passed vacuously before the binding existed.** `socket.malformed-clock-file-refused` and `socket.unsafe-directory-refused` require only that the provider fails to start, and an unimplemented binary failed on the unknown `--socket` flag. They are now backed by mutants: removing either check makes its fixture fail at step 0 with "participant listened".

**Committed socket and composition transcripts contain the runner's synthetic test credentials.** The conformance README says they will: the runner derives a deterministic credential per principal from the run's temporary directory and authenticates each session with it. They are not real credentials and grant nothing outside that run. The redaction applied to machine paths does not apply to them.

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
