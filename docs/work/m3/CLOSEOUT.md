# M3 close-out

The closing record of milestone M3, **Context** ([issue #15](https://github.com/Combraton/cbr/issues/15)), kept here so it survives outside GitHub. It follows [M1's close-out](../m1/CLOSEOUT.md) in shape: what the milestone promised, what it delivered, what was measured, and what it did not establish.

## M3 complete

Merged in order, no squash, as merge commits, each pinned with `--match-head-commit` to a head whose CI was green and which the reviewer had seen: **m3a** the Context profile, #16 as `cbfaebe` pinned to `1bd9a7a` · **m3b** retrieval and the dependency evaluator, #17 as `c3cdf51` pinned to `fb67250` · **m3c** the deterministic packet compiler, #18 as `2cf6b97` pinned to `81c5f0e` · **m3d** the journey-6 pilots, #19 as `17cc54c` pinned to `52305b3` · **m3e** what the pilots found, this pull request. Every merge was confirmed afterwards from `merged` and `merged_at`, never `merge_commit_sha`, and each merge commit's second parent was checked to be the reviewed head.

## What M3 promised, and what it delivered

| Promised | Delivered |
|---|---|
| The `context/1` profile, served | `context` 11 of 11, `composition` 3 of 14 with the other 11 permanently out of reach (10 declare `execution`, one `verification`, neither of which CBR serves). Gated in CI on Linux and macOS against an exact outcome multiset. |
| Retrieval that can say what it did **not** reach | A build manifest with the source frontier and the compiler; `complete` / `lagging` / `unavailable` judged against the basis actually asked about; the false-absence rule with a bounded canonical fallback; a view that is the authorization step made into a type; three separate bounds with a keyset cursor applied in SQL. |
| A **deterministic** packet compiler over a real repository | `cbr-context-compiler/2`, its provenance distinct from `cbr-context-script` by construction, producing a script ordered by item and repository rather than by the order anything was found in. Two compilations in one store produce the same packet byte for byte. |
| J1, end to end, through the public client | **Passed**, against an oracle declared in the test file's header before the run, with a negative control and a mutant for the control itself. |
| J8, the deadline path | **Passed.** |
| A labelled **pilot** of J6, as a steer rather than evidence | Two of them, on repositories nobody wrote for CBR: brian2, 553 files and 5.3 MB, and Knowscroll-v2, 22 owner decisions in one append-only file. Both were scored by the reviewer against oracles this session has never seen, and **both failed**. |

## Outcomes

| Suite | Total | Pass | Unsupported | Fail | Expectation |
|---|---|---|---|---|---|
| `stream` | 24 | 24 | 0 | 0 | `stream.json` |
| `core` | 135 | 130 | **5, permanent** | 0 | `core-c4.json` |
| `socket` | 13 | 11 | **2, permanent** | 0 | `socket.json` |
| `evidence` | 16 | 16 | 0 | 0 | `evidence.json` |
| `knowledge` | 10 | 10 | 0 | 0 | `knowledge.json` |
| `context` | 11 | 11 | 0 | 0 | `context.json` |
| `composition` | 14 | 3 | **11, permanent** | 0 | `composition.json` |

Unchanged across every M3 pull request. The Rust suite went from **132 tests on `main` before M3** to **181**.

## The journeys M3 ran

| Journey | Result | Record |
|---|---|---|
| J1, a question finds its own answer with a cited packet | **pass**, model `none` | [record](../../verification/JOURNEYS.md#j1-a-question-finds-its-own-answer-with-a-cited-packet) |
| J1's negative control, a moved tree is never silently answered from | **pass** | [record](../../verification/JOURNEYS.md#j1s-negative-control-a-moved-tree-is-never-silently-answered-from) |
| J8, a deadline leaves required items unmet and advisory items at their fallback | **pass**, model `none` | [record](../../verification/JOURNEYS.md#j8-a-deadline-leaves-required-items-unmet-and-advisory-items-at-their-fallback) |
| J6 pilot, brian2 | **failed** its oracle twice: one of three required facts on both runs, no trap triggered, baseline held none | [record](../../verification/JOURNEYS.md#j6-pilot-brian2-a-brownfield-repository-no-model--failed) |
| J6 pilot, Knowscroll-v2 | **failed** at m3d, two of three; **passed** the m3e rerun, three of three, no trap | [record](../../verification/JOURNEYS.md#j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun) |

Both pilots were rerun in full at m3e after the compiler changed, with the same questions verbatim and the same selectors, and rescored. **The caveat on Knowscroll's pass is part of the result:** the change that moved it was proposed by the reviewer, who holds the oracle, so the pass confirms that a general fix was general and is **not independent evidence of usefulness**. What shows nothing was tuned to pass is brian2, whose failure is unchanged across both runs against the same change. Both questions stay sealed and run again at M4. **A pilot is a steer and never gate evidence**, and neither pilot measures a session: no agent used either packet for any work, which is J6 proper and is M7's.

## Limits carried into M4

- **Lexical discovery finds only what shares vocabulary with the question.** That is not a defect of a journey, it is what a lexical index does with prose, and the two failed pilots are its measured cost. M4 is where retrieval gets something other than term overlap to work with.
- **Ranking is BM25 and unevaluated.** J1 measured it placing three spans above the one that answered its question. Nothing is tuned against it; the golden index digest deliberately excludes ranking order, because a guard that fails on a scoring tie gets switched off rather than fixed.
- **The index build happens inside the preparation tick**, and holds it throughout. Measured on three repositories: **7.2s** on CBR's own 1,066 blobs, **12.8s** on brian2's 553 blobs and 5.3 MB, **3.9s** on Knowscroll's 145. Every other job on that provider waits it out. M4's bounded runtime is where that is resolved.
- **Protocol 0.1 cannot cite a span of an artifact.** A repository whose decisions live in one append-only file gives every claim the same artifact and the same digest, and the range that distinguishes them lives in the claim's scope. Reported as [Combraton/protocol#16](https://github.com/Combraton/protocol/issues/16), which proposes nothing normative. M3e's claim ranking reads the range from that scope qualifier, which is the workaround the gap forces.
- **A packet citing an ingested artifact is not byte-reproducible across stores.** An ingest artifact's id carries the instant it was ingested and a claim's revision digest is taken over a record holding it. Reported, not fixed.
- **No model anywhere in M3.** Model-assisted selection is M4, and nothing here may be described as model-assisted.

## The three "rule written, not applied" instances, and what guards each now

M3's recurring failure was not a missing rule. Each time the rule was written down deliberately, had a test, was applied to the case in front of it, and was **not made structural** — so the next case had to be remembered. Two of the three were caught by review rather than by anything in the repository.

| The rule that existed | Where it was not applied | What guards it now |
|---|---|---|
| A packet's provenance names its compiler | The publication path passed the constant unconditionally, and **21 mutants passed because no test read the field** | J1 asserts `provenance.compiler`, and the golden packet digest covers the compiler string because a sealed packet's coverage names its producer |
| Resolve the readable set at the command and carry it in the job | The repository view did this; claims, added three commits later, did not, and every claim in the store went into every packet | A byte-identity test: the packet is identical to one prepared in a store where the claim was never proposed |
| A change to how an index is built changes `retrieval::COMPILER` | The chunker changed to a byte bound and the constant did not, so an index from the previous build would have passed as `complete` | `retrieval::GOLDEN_FIXTURE_DIGEST` over a fixture tree, covering the chunk rows, the anchor rows, the coverage and a fixed set of queries **together with the version string**, so a build change without a bump fails and a bump without a build change fails |

A fourth, found at m3e and worth the same treatment: **a guard is worth nothing if its fixture never reaches the bound it guards.** Two mutants on the packet compiler's own constants survived because the golden fixture produced fewer spans than the cap allowed and no excerpt near the byte cap. The fixture was enlarged, the mutants were re-run, and the test now asserts the fixture reaches both bounds so it cannot quietly shrink back.

## What M3 does not establish

- **That a packet is useful.** Every oracle M3 used is a list of facts declared before a run, not a measurement of whether a session did better with the packet. That is J6, at M7, and it still needs thresholds fixed before its confirmatory run.
- **That determinism is correctness.** INTERNALS §4 is explicit that a structurally consistent build is not a claim that what it selected is right.
- **Anything about a second repository in one basis end to end**, about the canonical fallback's 8 MiB read budget, or about a model-assisted selection.
