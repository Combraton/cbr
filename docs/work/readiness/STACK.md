# Proposed stack for standalone CBR v0.1

> **Status: proposed, not accepted.** Evidence gathered 2026-09-16 from primary sources — crates.io and static.crates.io tarballs, docs.rs, sqlite.org, provider API documentation, local man pages on Darwin 25.3.0, and the pinned protocol source. Versions are what was observed on that date. Each section states the strongest argument against its own recommendation. Nothing here is a selected dependency until an implementation PR records it as a decision. Discussion: [TALK](TALK.md).

[BASELINE §3](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md) makes Rust/Tokio, SQLite and content-addressed payloads the *starting preferences* and requires evidence at the milestone that depends on each. This document supplies that evidence for the choices M1–M4 need, and defers the rest.

## 0. Summary

| Area | Proposed | Confidence | Settled by |
|---|---|---|---|
| Language and toolchain | Rust, pinned to `1.97.1`, edition 2024 | high | Matches the pinned protocol toolchain exactly |
| Concurrency | **Sync core, threads, no Tokio in the core** | high | Direct evidence from the protocol reference (§2) |
| SQLite driver | `rusqlite 0.40.2`, features `bundled, functions, collation, trace, limits, pointer` | high | §1 |
| Durability | WAL + `synchronous=FULL`, `BEGIN IMMEDIATE`, object-before-row with a parent-directory fsync | high | §3 |
| Object store | Hand-rolled `objects/sha256/ab/cd/…`, ~150 lines | high | §4 |
| Lexical search | SQLite FTS5 with a Rust-side pre-tokenizer | medium-high | §5 |
| Code anchors | `tree-sitter 0.27.0` + grammar crates + `tags.scm`; `gix` for tree and blob access | medium | §6 |
| Invalidation | Hand-rolled persistent evaluator in SQLite; salsa as a mechanical model only | high | §7 |
| Model transport | `reqwest` + `serde_json` behind a CBR-owned `Provider` trait; two dialects | medium-high | §8 |
| Tool loop | CBR-owned | high | §8 |
| Token admission | BPE over the serialized body as an upper bound, plus provider count endpoints where they exist | medium | §9 |

## 1. SQLite driver

`rusqlite 0.40.2` (MIT, 2026-08-08) bundles SQLite **3.53.2** via `libsqlite3-sys 0.38.2`, and that build passes `-DSQLITE_ENABLE_FTS5` unconditionally, so full-text search costs nothing extra. `create_collation` and `create_scalar_function` are safe APIs behind the `collation` and `functions` features. Transaction control is direct: `transaction_with_behavior(Immediate)`, RAII savepoints, rollback-on-drop. Measured transitive dependency count with the proposed features: **14 crates**.

`sqlx 0.9.0` is excluded by construction, not by preference. `libsqlite3-sys` declares `links = "sqlite3"`, `sqlx-sqlite` pins `>=0.30.1, <0.38.0` and `rusqlite 0.40.2` requires `^0.38.1`, so Cargo refuses to build both in one graph. Since the sibling protocol repository already uses `rusqlite 0.40.2`, sqlx cannot be the choice here without diverging the workspace. It is also 99 transitive crates against 14, on an older SQLite, with no safe custom-function API.

`turso 0.7.2` (formerly `limbo`) fails two stated retrieval requirements outright: its compatibility matrix marks SQLite FTS3/4/5 unsupported, and `sqlite3_create_collation` unsupported with `COLLATE` only partial — its own note says *unknown collation names are silently treated as the default instead of erroring*, which is exactly the class of silent-wrong-answer CBR cannot tolerate. It is also pre-1.0 by its own README.

**Strongest counter-argument.** `rusqlite::Connection` is `Send` but **not `Sync`**, and every call blocks. Concurrent readers are something you build yourself, where sqlx and libsql hand it to you. The answer is that WAL permits exactly one writer anyway, so CBR wants a single owned writer connection; read connections are cheap to open per thread. The ergonomics we would be buying are ergonomics for a shape we do not have.

**A custom FTS5 tokenizer is reachable but not free.** No crate wraps it. The `fts5_api` surface with `xCreateTokenizer` is present in the bundled bindings, and rusqlite's own tests demonstrate the documented `SELECT fts5(?)` pointer retrieval (needs the `pointer` feature). Budget 150–300 lines of `unsafe` FFI *if* we go that way — §5 proposes not to, at first.

## 2. Concurrency: sync core, no Tokio

This is the clearest-cut decision in the document, and the evidence is the protocol's own reference provider rather than an opinion.

That provider is fully synchronous: `UnixListener::incoming()` with `thread::spawn` per connection, a process-wide `PROCESSING: Mutex<()>` serializing command handling, a per-connection outbox writer thread, and a separate worker thread for context investigations — the last being exactly what [CONTEXT §4](https://github.com/Combraton/protocol/blob/main/docs/spec/profiles/CONTEXT.md) requires when it says a provider *"SHOULD keep answering reads while its investigation executions run"*. It passes all 135 Core fixtures on this machine in 37.5 s, including `core.events.unread-ending-notice-does-not-hold-the-connection`, which is the backpressure fixture that checks the room deadline, the shared notice budget across three subscriptions and the maximum time to closure.

The piece most likely to be assumed to need async — bounding "produced but not yet written" bytes per connection — is 156 lines of `Mutex` + `Condvar` + `VecDeque` with a dedicated writer thread. The session thread queues and reads `pending`; it never blocks on a consumer that stopped reading.

There is also no async SQLite to gain. `tokio-rusqlite 0.8.0` is `thread::spawn` around an event loop of boxed closures; `sqlx-sqlite 0.9.0` spawns an OS thread per connection and its own documentation says the background thread is the one making FFI calls. The choice is not async versus threads. It is threads, or threads plus Tokio.

One Tokio behaviour actively works against a CBR requirement: `spawn_blocking` tasks **cannot be aborted**, and runtime shutdown waits indefinitely for started blocking tasks. CBR must support cancellation of in-flight investigation work and a bounded shutdown. Putting blocking model calls behind `spawn_blocking` would import precisely the wrong semantics.

**Proposed shape.** A reader thread per connection; a single writer thread owning the one `rusqlite::Connection` and applying commands serially inside `BEGIN IMMEDIATE`; a per-connection outbox writer thread; a bounded worker pool of N threads for model calls using a blocking HTTP client, returning results over a channel for the writer thread to commit.

**Strongest counter-argument.** Tokio is 19 transitive crates, not 200, and if CBR ever needs hundreds of concurrent in-flight model calls a thread-per-call pool caps out where Tokio would not — and retrofitting async through a codebase that assumed blocking I/O is the worst possible time to do it. The answer: a concurrency cap on a paid API is wanted regardless, and if N ever needs to exceed roughly 100, Tokio can be introduced **inside the worker pool alone**, owning its own runtime behind a channel boundary, without the core, the stream binding or the SQLite writer ever becoming async. That keeps the option open at near-zero cost, which is strictly better than starting async.

## 3. Durability

The contract from [STORAGE §2](https://github.com/Combraton/combraton/blob/main/docs/architecture/STORAGE.md) is that a crash may leave unreferenced objects, but must never produce a sealed-artifact row whose bytes are missing.

**Settings:** `journal_mode=WAL`, `synchronous=FULL`, `foreign_keys=ON`, a `busy_timeout`, and `BEGIN IMMEDIATE` on every writing transaction.

- `synchronous=NORMAL` is not an option. sqlite.org states plainly that *"WAL mode does lose durability. A transaction committed in WAL mode with `synchronous=NORMAL` might roll back following a power loss or system crash."* That would let a committed sealed-artifact row vanish while its object survived — a hole in a contiguous `(epoch, sequence)` stream, which is worse than the failure we are guarding against. `EXTRA` is identical to `FULL` in WAL mode, so there is nothing above FULL to buy.
- Durability across *application* crashes holds regardless of the setting; only power loss and OS crash are at stake.
- `BEGIN IMMEDIATE` rather than the default `DEFERRED`, because a deferred transaction upgrades to a write transaction lazily and can return `SQLITE_BUSY` partway through — an avoidable failure mode on a single-writer store.

**Object publication order**, each step load-bearing:

1. Create the temp file **in the destination directory**, never `$TMPDIR` — `rename` must stay within one filesystem.
2. Write, then verify the digest and byte count from what was actually written.
3. `File::sync_all()`.
4. `rename(tmp, final)`.
5. `File::open(parent_dir)?.sync_all()`.
6. Only then `BEGIN IMMEDIATE`, insert the descriptor row, `COMMIT`.

Step 5 is the one people skip. `rename(2)` guarantees an instance of the new name always exists across a crash, but says nothing about the parent directory entry reaching disk; SQLite does the same directory sync for its own super-journals and documents that the alternative is the file appearing in `lost+found`. The directory fsync was tested on this machine: `open(dir, O_RDONLY)` followed by `F_FULLFSYNC`, `fsync` and `F_BARRIERFSYNC` all returned 0 on APFS.

**A useful macOS asymmetry, and why it points the safe way.** On Apple targets Rust's `File::sync_all()` and `sync_data()` both compile to `fcntl(fd, F_FULLFSYNC)`, which Apple documents as actually asking the drive to flush to permanent storage — Apple's own `fsync(2)` page warns that plain `fsync` does not, and that *"this is not a theoretical edge case."* SQLite, meanwhile, leaves `PRAGMA fullfsync` off by default and issues a plain `fsync`. So object bytes get the stronger flush and the referencing row gets the weaker one. That asymmetry is in our favour: the object is more durable than the row pointing at it, which is exactly the invariant we want, so we should **leave `PRAGMA fullfsync` off** rather than "fix" it.

**Strongest counter-argument.** `synchronous=FULL` costs a real fsync on every accepted command and will dominate per-command latency; `NORMAL` would be substantially faster for commit-heavy work. Taken, and the fsync cost should be measured on both targets before any commit-rate is promised to anyone. But correctness wins here.

## 4. Content-addressed object store

Hand-rolled, roughly 150 lines: `objects/sha256/ab/cd/<rest>` with two levels of hex fanout, `tempfile` for staging, `sha2 0.11.0` for digests, the §3 publication order, and published objects mode `0444` so immutability is enforced rather than promised.

`cacache 13.1.0` (Apache-2.0) was the candidate and is rejected on the decisive point: **it contains no `sync_all` or `sync_data` anywhere in its source.** Its content writer flushes a userspace buffer and renames; its index is an append-only bucket file opened with `append(true)` and flushed, with no fsync and no directory fsync. It cannot meet the contract without being forked. It also brings its own key-to-content index, a second bookkeeping system that can disagree with our journal. Take its layout, which is the same two-level fanout we would write anyway; reject its durability.

Gotchas worth recording now: never expose a writable hardlink to a store object — the shared inode means a workspace write corrupts the store; copy, or use APFS `clonefile` / `FICLONE`. Content addressing already deduplicates by construction, so hardlink deduplication buys nothing. Two-level hex fanout gives 256 children per level, far from any ext4 directory limit.

## 5. Lexical search

SQLite FTS5, with text normalised in Rust before it is indexed.

The decisive reason is atomicity, not features. CBR's contract is that a command's state change and its event rows commit in one transaction. An FTS5 table is more rows in the same database file, so indexing joins that transaction for free. `tantivy 0.26.2` (MIT) cannot: separate directory, separate commit protocol, separate writer lock, separate crash window, no WAL — anything since the last `commit()` is lost on crash. Using it would force demoting the index to a derived artifact with an "indexed up to `(epoch, sequence)`" watermark and a catch-up loop. That is permanent complexity, plus 115 transitive crates and an `IndexWriter` that demands a 15 MB minimum memory budget.

**FTS5's real limitations for source code, stated so they are not discovered later.** `unicode61` splits on punctuation, so `foo_bar` becomes two tokens unless `tokenchars '_'` is set, and `foo.bar()` always splits. **There is no camelCase splitting at all** — `getUserName` is one token and a search for `user` will not find it, and no built-in tokenizer fixes that. `porter` stemming is actively harmful on identifiers. Prefix queries are trailing-only; leading wildcards need the `trigram` tokenizer, which matches nothing below three characters and would mean a second FTS5 table.

**Proposed fix, cheap and with no `unsafe`:** pre-tokenize in Rust and store a normalised search column alongside the original — `getUserName` indexed as `getUserName get user name`. Take the custom-FTS5-tokenizer FFI route only if measurement shows the cheap version missing things.

**Strongest counter-argument.** If lexical quality over source code matters more than transactionality, tantivy is genuinely the better search engine, needs no `unsafe` for any of it, and the watermark pattern is well-trodden rather than exotic. Turso chose tantivy over FTS5 for its own full-text search, which is a data point about FTS5's ceiling. The escape hatch stays clean precisely because the journal remains the source of truth, so this is reversible if evaluation shows misses.

## 6. Code anchors

`tree-sitter 0.27.0` (MIT, 18 transitive crates with default features) plus per-language grammar crates, using each grammar's shipped `queries/tags.scm`, optionally through `tree-sitter-tags 0.27.0`.

Two hazards to plan around. **ABI:** tree-sitter 0.27.0 accepts language ABI 13–15; grammar crates version independently, which works today but means every grammar must be pinned explicitly and a core bump treated as a compatibility check. **Staleness:** `tree-sitter-rust 0.24.2`, `python 0.25.0`, `javascript 0.25.0` and `go 0.25.0` are current, but `typescript 0.23.2`, `java 0.23.5` and `cpp 0.23.4` were last published in 2024. Only pin the languages we actually claim to support, and say which those are.

**What tags-level anchoring actually buys, and what it does not.** For a blob at tree `T`, parsing and running `tags.scm` yields `(kind, name, byte_range, line_range)` per definition — roughly 200–400 lines of Rust plus a table. That finds *where `fn foo` is defined*. It does **not** resolve *which* `foo` a call refers to. Two `impl` blocks with the same method name, generics, macros, re-exports and shadowing all collapse to several candidates. The honest mitigation, which should be in from the start: **store every candidate with an explicit ambiguity marker and never silently pick one**, and record `(tree_oid, blob_oid)` so a stale anchor is detectable rather than merely wrong. [INTERNALS §2](../../spec/INTERNALS.md) already requires this — "ambiguous resolution remains ambiguous."

Precise resolution has no good option. `stack-graphs 0.14.1` is the only serious language-agnostic attempt and is effectively dormant — last release December 2024, last commit September 2025. LSP-based indexing gives correct answers but means supervising a per-language server process, contradicting the single-local-process shape. SCIP is a serialisation format, not an indexer.

**A licensing finding that needs an explicit ruling.** `git2 0.21.0` and `libgit2-sys` declare `MIT OR Apache-2.0` on crates.io, **but the vendored libgit2 C library is GPLv2 with a linking exception.** The exception permits linking into a distributed binary, so this is probably fine — but if "no copyleft" is a hard policy it needs deciding rather than assuming. `gix 0.87.1` is `MIT OR Apache-2.0` all the way down with no GPL C library and is actively maintained (2026-08-24). **Proposed: use `gix`** and sidestep the question entirely.

## 7. Invalidation and the dependency evaluator

**Salsa cannot be the durable store, and its own source says why.** `salsa 0.28.2` does ship an opt-in `persistence` feature, but its entire public API is two methods — serialise the whole graph at once, deserialise the whole graph at once. No incremental persistence, no transaction, no partial load, no crash-consistency story, no format stability guarantee. It is lossy: accumulators are `#[serde(skip)]` with a TODO. There is no chapter for it in the salsa book, and the crate still self-describes as experimental. Using it as the durable store would mean rewriting the entire memo graph to disk on every commit, in an unstable format, with no atomicity relative to the SQLite transaction. [INTERNALS §5](../../spec/INTERNALS.md) already says salsa is an optional evaluator and not CBR's durable database; the evidence supports that.

**Salsa is exactly the right thing to copy, mechanically.** Three pieces transfer directly: per-memo `verified_at` and `changed_at` revision counters; `backdate_if_appropriate`, which is the Build-Systems-à-la-Carte early cutoff — if the recomputed value compares equal under the declared comparator, the new memo's `changed_at` is set *back*, so dependents see "unchanged" and stop; and `report_untracked_read`, which marks a query as never validatable without re-execution.

Salsa's own guards are the part most worth stealing, because they are the part that is easy to get wrong: backdating is refused when the query participates in a cycle, when the memo is provisional, and when durability *decreased* — salsa's comment calls that last one "a breaking change that our consumers must be aware of." It also ships a `report_backdate_violation` that fires in debug builds, which tells you they got it wrong at least once.

**The shape in SQLite:** five tables — a revision counter (which can simply be the existing `(epoch, sequence)`), `input`, `node` with `changed_at`/`verified_at`/`durability`/`origin`, an ordered `edge` table with a reverse index on dependency, and `output`. The `maybe_changed_after(key, since)` walk memoises within a revision, forces re-execution for untracked origins, walks recorded edges in order, and backdates on comparator equality subject to the guards above. All of it runs inside the same `BEGIN IMMEDIATE` transaction as the command, so a crash mid-recompute leaves the graph exactly as it was.

**The untracked-read hazard, and the one insight that matters for CBR.** Any computation reading something not recorded as an edge — wall clock, environment variable, a file outside the declared tree, a model response — makes validation *unsound*, because it will report "unchanged" for something that changed. There are exactly three dispositions per node type: turn the read into a declared input, mark it always-re-execute, or record explicit staleness. For CBR the important case is the model call, and it has a clean answer: **make model identity, prompt text and sampling parameters real declared inputs.** A model call then becomes a properly tracked node — cacheable, and subject to early cutoff — instead of an untracked hole in the graph. This is also what makes [MODEL-RUNTIME](../../spec/MODEL-RUNTIME.md)'s "replay consumes the recorded output, running the model again is a new derivation" mechanically enforceable rather than a convention.

Size: salsa's whole `src/` is 20,509 lines and its demand-driven core alone is 8,004, but that includes cycle recovery, interning, tracked structs, accumulators, LRU eviction and full parallelism. A single-threaded, cycle-free, persistent version of just the algorithm above is realistically **800–1,500 lines including the SQL**, with a comparable amount of test code.

**Strongest counter-argument.** Salsa is battle-tested by rust-analyzer; a hand-rolled evaluator will have subtle validation bugs that silently serve stale answers — the single worst failure mode for a system whose pitch is evidence-backed memory. The mitigation must be built first, not last: a property-test harness that generates random DAGs and random input mutations and asserts that demand-driven evaluation always agrees with a from-scratch recompute. That harness is the acceptance criterion for this component.

## 8. Model transport and the tool loop

**Proposed: own the wire.** `reqwest` + `serde_json` behind a narrow CBR-owned `Provider` trait, with **two dialects** — OpenAI chat-completions and Anthropic messages. CBR writes the loop.

The reasoning is a constraint, not a preference. [MODEL-RUNTIME §2](../../spec/MODEL-RUNTIME.md) requires budgeting the *fully serialized* request — instructions, tool schemas, messages, tool outputs, provider overhead — before sending, and partitioning, narrowing or repacking if it does not fit. That requires the component that serializes and the component that owns the working-set policy to be the same component. `rig-core 0.42.0` can hand over the exact request bytes through `HttpClientExt::send`, but at that layer you can only *refuse*, not repack, because repacking is a policy decision several layers up where the wire is no longer visible; there is no dry-run serializer. `async-openai 0.42.0`'s tower middleware seam has the identical shape. Owning the serializer collapses that seam to nothing, and the derivation record falls out for free as the exact bytes in and the exact bytes out — which is the whole point.

Two dialects cover every provider likely to be granted. MiniMax speaks both, and its OpenAI-compatible endpoint is now the preferred one while its native `/v1/text/chatcompletion_v2` is marked deprecated and returns HTTP 200 on errors. Anthropic is the one provider that genuinely needs its native surface, for reasons that cannot be designed around: its own OpenAI-compatibility documentation lists `prompt_tokens_details` and `completion_tokens_details` as **"always empty"**, states that prompt caching is unsupported and `response_format` ignored, and says most unsupported fields are silently ignored rather than erroring. Cached-token accounting and schema-constrained output are simply unobtainable there.

**Strongest counter-argument, and it is a real one.** `rig-agent 0.42.0`'s `AgentRun` is already a sans-IO, steppable, serializable state machine that owns turn counting, tool-call validation, invalid-tool-call recovery and usage aggregation while the driver owns all I/O, tool execution, concurrency, cancellation and lifecycle. Its own module documentation describes the boundary in almost the words our spec uses. It is MIT, it is written and tested, and rig-core carries a quirk catalogue we will otherwise rediscover one incident at a time — null-tolerant `object`/`created` fields for gateways that omit them, truncated-tool-call recovery, and a prompt-cache verification harness that found two real bugs.

**Why the recommendation still stands.** rig is in a violent churn phase — 75 commits since the 0.42.0 release, 30 of them marked breaking, including one that dissolved `rig-run` and moved `AgentRun` into `rig-agent` within the last month. It is pre-1.0, so every minor bump is a breaking bump, and `AgentRun`'s own documentation warns that its serialization format *"carries no cross-version stability guarantee yet: resume with the same rig version that suspended the run."* For a service whose product is durable auditable records, pinning the loop to that is a poor trade. The paths are not exclusive: we can own the transport now and borrow `AgentRun`'s *design* — `next_step` / `model_response` / `tool_results` — without the dependency, because CBR must own the checkpoint format under either choice. Revisit at rig 1.0, or at the fourth provider dialect.

**Two hard exclusions.** `llm_adapter` / `llm_runtime` are **AGPL-3.0-only** despite healthy download counts and will surface in any search — a blocker under the no-copyleft constraint. `anthropic-rs` declares `MIT AND Apache-2.0`, conjunctive rather than a choice, and is dead besides. `langchain-rust` last saw a commit on `main` in April 2025.

**If MiniMax is the granted provider, two behaviours must be designed for, not discovered.** It silently ignores `response_format` / `json_schema` — a live probe returned HTTP 200 with free-form prose — so schema-constrained output is unavailable and CBR must validate and repair rather than trust. And `tool_choice: "required"` is silently ignored while `"none"` is honoured, so a loop assuming a forced tool call will misbehave with no error. Also: M3 embeds `<think>…</think>` in `message.content` unless `reasoning_split` is set, and there is no `data: [DONE]` streaming sentinel, so a parser waiting for one will hang.

## 9. Token admission before sending

This is the one place where the specification's wording and reality need reconciling carefully, so it is worth being precise.

| Provider family | Exact local count | Server-side count endpoint |
|---|---|---|
| OpenAI-family | `tiktoken-rs 0.12.0` BPE, exact for OpenAI's own models | none found |
| Anthropic | **None exists.** Anthropic publishes no tokenizer. | `POST /v1/messages/count_tokens` — free, independent rate limit, accepts the full request including `tools` |
| MiniMax | `tokenizer.json` published on Hugging Face for M3/M2 | `POST /v1/responses/input_tokens` |
| Gemini | — | `models/{model}:countTokens`, counts system instructions and tools |
| Groq, DeepSeek, OpenRouter | wrong tokenizer at best | none documented |

**One trap worth naming.** `tiktoken-rs`'s `num_tokens_from_messages` **never counts the `tools` array** — it walks `role`, `content`, `name`, `function_call` and `tool_calls`, adds framing constants adapted from the OpenAI cookbook, and stops. Tool schemas are often the single largest fixed cost in an agentic request. That function must not be used for admission.

**Proposed method.** Run the model's own BPE over the **serialized JSON body about to be sent** — which does include tool schemas and framing punctuation — and treat the result as a conservative *upper bound* for admission, using the provider count endpoint where one exists and an exact answer is worth a round trip.

The honest consequence: the guarantee CBR can make is *"we will never knowingly exceed the budget"*, not *"we will never exceed it"*. MODEL-RUNTIME §2 already anticipates exactly this — a provider overflow caused by a bad estimate triggers a bounded repack and retry with a recorded failure, not unlimited re-investigation. So the specification is satisfiable as written, provided the estimate is a documented upper bound rather than an unvalidated heuristic, and provided the recorded-failure path is real. The estimator's accuracy against actual reported usage should itself be measured and published, per provider and model.

## 10. What is deliberately not decided here

Embedding models and vector indexes (deferred by [INTERNALS §4](../../spec/INTERNALS.md) until evaluation shows a miss simpler methods cannot address); any generated-program or sandboxed-worker backend (deferred by BASELINE §3 until after core memory proof); the supported model matrix and numerical budget defaults, which need a provider grant and measurement; and the project license, which is the owner's.
