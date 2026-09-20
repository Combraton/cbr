# Current session state — cbr

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-19.
- **Owner/task:** Claude Code session as implementation lead for standalone CBR. **M1 and M2 are complete**; M1's closing record is [m1/CLOSEOUT.md](m1/CLOSEOUT.md). Active task: **M3, [issue #15](https://github.com/Combraton/cbr/issues/15)**, in four pull requests against `main`, each reviewed at its head before merging: m3a the Context profile (**merged**) and m3b retrieval and the dependency evaluator (**merged**); **m3c the packet compiler on a real repository (PR #18, open)**; then m3d the journey-6 pilot. Parent: [issue #1](https://github.com/Combraton/cbr/issues/1). An independent reviewer session reviews this read-only.
- **Merged:** PR #2 as `d68e9d6`, pinned to `877139f`; PR #4 as `a939446`, pinned to `630011c`; PR #5 (c1) as `8b75129`; PR #6 (c2) as `da1e650`, pinned to `b00ec49`; **PR #7 (c3) as `8ba2594`, pinned to `5b98a9f`, confirmed from `merged: true` and `merged_at: 2026-09-16T15:31:51Z`**. Earlier: PR #6 pinned to `b00ec49`, confirmed from `merged: true` and `merged_at: 2026-09-16T14:26:34Z`**, the draft marked ready first and the head re-read unchanged before merging. Every owner decision, including the ceiling, is in [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md). **PR #8 (owner follow-ups) as `f00faaf`, pinned to `40cb30b`, `merged_at: 2026-09-16T16:57:29Z`; PR #9 (c4) as `9a7b8f5`, pinned to `5db9d7c`, `merged_at: 2026-09-16T16:57:53Z`**, in that order, each confirmed from `merged` and `merged_at`. **PR #10 (d) as `96a33f2`, pinned to `a1048e6`, `merged_at: 2026-09-16T18:17:38Z`; then PR #11 (e) as `6c63d91`, pinned to `c2a262d`, `merged_at: 2026-09-16T18:17:56Z`**. PR #11 was stacked on #10's branch, so it was retargeted to `main` after #10 merged and before its own merge; retargeting leaves the head unchanged, and `c2a262d` was re-read before merging. **PR #13 (M1 close-out, README) as `5c667ed`, pinned to `507fc77`, `merged_at: 2026-09-16T18:58:44Z`; then PR #14 (M2) as `39113b2`, pinned to `62cbb99`, `merged_at: 2026-09-16T18:59:00Z`**, #14 retargeted to `main` first. **PR #16 (m3a) as `cbfaebe`, pinned to `1bd9a7a`, confirmed from `merged: true` and `merged_at: 2026-09-19T14:48:17Z`**, the draft marked ready first and the head re-read unchanged; the merge commit's second parent is `1bd9a7a`, so the reviewed head is on `main` unaltered. **PR #17 (m3b) as `c3cdf51`, pinned to `fb67250`, confirmed from `merged: true` and `merged_at: 2026-09-19T15:55:37Z`**, second parent `fb67250`.
- **Merge rule, 2026-09-16, superseded the same day.** This session ran `gh pr merge` on PR #2 after the owner replied "you can merge PR 2" in-session, having first reported the contradicting claim with evidence and waited. It landed the exact reviewed head `877139f` and is kept. A stricter rule was then recorded, and the owner then **granted merge authority under four conditions**, now in [AGENTS.md](../../AGENTS.md): pin with `--match-head-commit`; the head's CI is green; the reviewer has seen that head; no squash. Confirm from `merged` and `merged_at` afterwards, **never `merge_commit_sha`** — GitHub populates that on an open pull request with the test-merge candidate. Tags and releases remain the owner's alone.
- **Owner decision, 2026-09-20, recorded at the m3c review: m3d has two pilot repositories**, as an amendment to [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md) question 6. Knowscroll-v2 stays the decision-memory pilot; the owner's **brian2 fork** is added as a brownfield pilot, registered read-only, whose journey tests discovery and code flow rather than decision memory, whose oracle the owner writes before the run, and whose dirty working tree makes it the first journey to exercise the dirty path. **brian2 is CeCILL-licensed and this repository is MIT, so none of its bytes, excerpts or packets are committed here** — digests, paths, spans, counts and costs only. It is recorded now and acted on only after m3c is cleared; no other brian2 work belongs in this pull request.
- **Inspected revisions:** protocol `v0.1.0` = `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; combraton `9af69ce`; pio `e65b7c0`; benchmarks `c8d5878`.

## This change — M3c, the deterministic packet compiler

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
