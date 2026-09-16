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

| Command | What it establishes | What it does not |
|---|---|---|
| `check_docs.py` | Documentation structure: entrypoints, the `CLAUDE.md` import, local link targets, balanced fences, no private machine paths | Nothing about the product |
| `verify_pin.py` | The vendored Protocol material matches the published release, against three anchors CBR does not control: the release's `BUNDLE-SHA256SUMS`, the recomputed inventory `listing_sha256` that also appears in the annotated tag message, and the inventory's own per-file digests | That CBR implements any contract correctly |
| `cargo fmt --all -- --check` | Formatting only | — |
| `cargo clippy … -D warnings` | Lints clean; warnings fail | Correctness |
| `cargo build --workspace --locked` | The workspace builds from the committed `Cargo.lock` with no dependency resolution | Runtime behaviour |
| `cargo test --workspace --locked` | Every pinned encoding vector — 12 canonical, 18 rejected, 1 command intent — plus the property tests, 19 in all. A rejected vector must be refused **for the reason the vector states**, so a parser that refused everything would fail. | Any profile conformance. No provider exists yet, so no fixture suite has been run against CBR. |

**Not yet present, and not claimed:** no CBR provider, no participant descriptor, no conformance run, no store, no packet, no model call. The Core, stream, socket and Evidence fixture suites are M1's acceptance and are run in the pull requests that introduce the code they exercise, not before.

When reporting a result, give the command, its exit status, the environment and the tested revision. Preserve the producing command's exit status when shortening output: piping a failing build into a successful `tail` or `grep` reports success, and a shortened log is not evidence that the command passed.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios/results, not this service's normative contract. No runtime benchmark has been implemented or run by the documentation setup.
