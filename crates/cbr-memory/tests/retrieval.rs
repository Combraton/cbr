//! The gate for the rules that govern a retrieval answer, written before the
//! rules exist.
//!
//! Each test here states one thing a caller is entitled to and that an index
//! alone does not give: that a projection says how current it is, that an
//! incomplete projection never answers "nothing found", that an index rebuilt
//! from the canonical records is the same index, that a search answers only
//! inside the caller's view, and that an answer is bounded in rows, in span
//! and in total.

use std::path::Path;
use std::process::Command;

use cbr_memory::index;
use cbr_memory::retrieval::{self, Ask, Bounds, Origin, State};
use rusqlite::Connection;

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args([
            "-c",
            "user.name=cbr-test",
            "-c",
            "user.email=cbr-test@invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    index::migrate(&connection).expect("migrates the index");
    retrieval::migrate(&connection).expect("migrates the manifests");
    connection
}

/// A repository with one commit holding `files`, returning its tree.
fn repository(directory: &Path, files: &[(&str, &str)]) -> String {
    git(directory, &["init", "-q", "-b", "main"]);
    commit(directory, files)
}

fn commit(directory: &Path, files: &[(&str, &str)]) -> String {
    for (path, contents) in files {
        let full = directory.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(full, contents).expect("writes");
    }
    git(directory, &["add", "."]);
    git(directory, &["commit", "-q", "-m", "change"]);
    cbr_identity::git_basis(directory, "HEAD")
        .expect("basis")
        .tree
}

fn view<'a>(id: &'a str, checkout: &'a Path, basis: &'a str) -> Vec<retrieval::Readable<'a>> {
    vec![retrieval::Readable {
        id,
        checkout,
        basis,
    }]
}

#[test]
fn an_index_records_the_frontier_and_the_compiler_it_was_built_with() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let first = repository(path, &[("src/queue.rs", "pub fn drainQueue() {}\n")]);
    let connection = database();

    let manifest = retrieval::build(&connection, "svc", path, &first, 7).expect("builds");
    assert_eq!(manifest.repository, "svc");
    assert_eq!(manifest.frontier, first, "the frontier is the tree built");
    assert_eq!(manifest.compiler, retrieval::COMPILER);
    assert_eq!(manifest.built_at, 7);
    assert_eq!(manifest.coverage.indexed, 1, "{:?}", manifest.coverage);

    let read = retrieval::manifest(&connection, "svc")
        .expect("reads")
        .expect("a manifest");
    assert_eq!(read, manifest, "a manifest reads back exactly");

    // Asked about the tree it was built to, the projection is complete.
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &first),
        &Ask::all("drain"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources.len(), 1);
    assert_eq!(answer.sources[0].state, State::Complete, "{answer:?}");
    assert_eq!(answer.sources[0].frontier.as_deref(), Some(first.as_str()));
    assert!(!answer.found.is_empty(), "{answer:?}");
    assert!(
        answer
            .found
            .iter()
            .all(|found| found.origin == Origin::Index),
        "a complete projection answers from the index: {answer:?}"
    );

    // Asked about a later tree, the same projection is lagging.
    let second = commit(path, &[("src/queue.rs", "pub fn drainQueue(n: u64) {}\n")]);
    assert_ne!(second, first);
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &second),
        &Ask::all("drain"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources[0].state, State::Lagging, "{answer:?}");

    // A repository with no manifest at all is unavailable, not empty.
    let answer = retrieval::search(
        &connection,
        &view("other", path, &second),
        &Ask::all("drain"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources[0].state, State::Unavailable, "{answer:?}");
}

#[test]
fn a_lagging_projection_falls_back_to_the_canonical_records_and_never_answers_nothing_found() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let first = repository(path, &[("src/queue.rs", "pub fn drainQueue() {}\n")]);
    let connection = database();
    retrieval::build(&connection, "svc", path, &first, 1).expect("builds");

    // A decision lands after the index was built. The index has never seen
    // this word; the repository's own objects have.
    let second = commit(
        path,
        &[("docs/decision.md", "The compatibility adapter stays.\n")],
    );
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &second),
        &Ask::all("compatibility"),
        &Bounds::default(),
    )
    .expect("searches");

    assert_eq!(answer.sources[0].state, State::Lagging, "{answer:?}");
    assert!(
        answer.declares_its_gaps(),
        "a lagging projection must fall back or say why not: {answer:?}"
    );
    assert!(
        !answer.found.is_empty(),
        "the canonical records hold the word the index never saw: {answer:?}"
    );
    assert!(
        answer
            .found
            .iter()
            .all(|found| found.origin == Origin::Canonical),
        "{answer:?}"
    );
    assert_eq!(answer.found[0].path, "docs/decision.md", "{answer:?}");
}

#[test]
fn an_unavailable_projection_states_why_rather_than_answering_empty() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let tree = repository(path, &[("docs/decision.md", "The queue stays on two.\n")]);
    let connection = database();

    // Never indexed: unavailable, but the canonical records are readable.
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("queue"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources[0].state, State::Unavailable, "{answer:?}");
    assert!(answer.declares_its_gaps(), "{answer:?}");
    assert!(!answer.found.is_empty(), "{answer:?}");

    // Unavailable and unreadable: the answer must carry the reason, because
    // now there is nothing else it can carry.
    let missing = directory.path().join("gone");
    let answer = retrieval::search(
        &connection,
        &view("svc", &missing, &tree),
        &Ask::all("queue"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources[0].state, State::Unavailable, "{answer:?}");
    assert!(answer.found.is_empty(), "{answer:?}");
    assert!(
        answer.sources[0].reason.is_some(),
        "an empty answer must say why it is empty: {answer:?}"
    );
    assert!(answer.declares_its_gaps(), "{answer:?}");

    // Readable, searched, and genuinely nothing there. That is still not
    // "nothing found": the projection is unavailable, so the answer has to
    // say that it was the fallback that looked, and that it came up empty.
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("wordthatisnowhereinthistree"),
        &Bounds::default(),
    )
    .expect("searches");
    assert!(answer.found.is_empty(), "{answer:?}");
    assert_eq!(answer.sources[0].state, State::Unavailable, "{answer:?}");
    assert!(
        answer.sources[0].reason.is_some(),
        "a fallback that found nothing must say so: {answer:?}"
    );
    assert!(answer.declares_its_gaps(), "{answer:?}");
}

#[test]
fn a_page_walks_two_repositories_that_tie_on_score_without_repeating_either() {
    // Two checkouts of the same content: every hit in one ties every hit in
    // the other, which is where a cursor over a merged order goes wrong.
    let first_directory = tempfile::tempdir().expect("temp dir");
    let second_directory = tempfile::tempdir().expect("temp dir");
    let alpha = first_directory.path();
    let beta = second_directory.path();
    let body: String = (0..80)
        .map(|line| format!("the drainQueue call happens at line {line}\n"))
        .collect();
    let alpha_tree = repository(alpha, &[("docs/long.md", &body)]);
    let beta_tree = repository(beta, &[("docs/long.md", &body)]);

    let connection = database();
    retrieval::build(&connection, "alpha", alpha, &alpha_tree, 1).expect("builds");
    retrieval::build(&connection, "beta", beta, &beta_tree, 2).expect("builds");

    let both = vec![
        retrieval::Readable {
            id: "alpha",
            checkout: alpha,
            basis: &alpha_tree,
        },
        retrieval::Readable {
            id: "beta",
            checkout: beta,
            basis: &beta_tree,
        },
    ];

    let all = retrieval::search(
        &connection,
        &both,
        &Ask::all("drain"),
        &Bounds {
            rows: 1000,
            batch_bytes: usize::MAX,
            ..Bounds::default()
        },
    )
    .expect("searches");
    assert!(
        all.found.iter().any(|found| found.repository == "alpha")
            && all.found.iter().any(|found| found.repository == "beta"),
        "both repositories answer: {all:?}"
    );

    let mut walked = Vec::new();
    let mut cursor = None;
    for _ in 0..200 {
        let page = retrieval::search(
            &connection,
            &both,
            &Ask::all("drain"),
            &Bounds {
                rows: 1,
                cursor: cursor.clone(),
                batch_bytes: usize::MAX,
                ..Bounds::default()
            },
        )
        .expect("searches");
        walked.extend(page.found.clone());
        match page.cursor.clone() {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(
        walked, all.found,
        "one row at a time across two tied repositories, each exactly once"
    );
}

#[test]
fn a_rebuilt_index_is_identical_to_the_one_maintained_incrementally() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let first = repository(
        path,
        &[
            ("src/queue.rs", "pub fn drainQueue() {}\n"),
            ("docs/one.md", "The queue stays on version two.\n"),
        ],
    );
    let second = commit(
        path,
        &[
            ("src/queue.rs", "pub fn drainQueue(n: u64) -> u64 { n }\n"),
            ("docs/two.md", "The adapter is kept for one release.\n"),
        ],
    );

    // Maintained: built at the first tree, then at the second, in one store.
    let incremental = database();
    retrieval::build(&incremental, "svc", path, &first, 1).expect("builds");
    retrieval::build(&incremental, "svc", path, &second, 2).expect("rebuilds");

    // Rebuilt from the canonical records alone, in a store that never saw
    // the first tree.
    let rebuilt = database();
    retrieval::build(&rebuilt, "svc", path, &second, 1).expect("builds");

    assert_eq!(
        retrieval::index_digest(&incremental, &second).expect("digests"),
        retrieval::index_digest(&rebuilt, &second).expect("digests"),
        "an index rebuilt from the canonical records is the maintained index"
    );
}

#[test]
fn a_principal_without_read_sees_nothing_from_that_repository_and_cannot_tell_it_apart_from_no_match()
 {
    let first_directory = tempfile::tempdir().expect("temp dir");
    let second_directory = tempfile::tempdir().expect("temp dir");
    let open = first_directory.path();
    let closed = second_directory.path();
    let open_tree = repository(open, &[("src/open.rs", "pub fn openThing() {}\n")]);
    let closed_tree = repository(closed, &[("src/secret.rs", "pub fn embargoedThing() {}\n")]);

    let connection = database();
    retrieval::build(&connection, "open", open, &open_tree, 1).expect("builds");
    retrieval::build(&connection, "closed", closed, &closed_tree, 2).expect("builds");

    let everything = vec![
        retrieval::Readable {
            id: "open",
            checkout: open,
            basis: &open_tree,
        },
        retrieval::Readable {
            id: "closed",
            checkout: closed,
            basis: &closed_tree,
        },
    ];
    let narrow = view("open", open, &open_tree);

    // The word lives only in the repository the narrow principal may not read.
    let full = retrieval::search(
        &connection,
        &everything,
        &Ask::all("embargoed"),
        &Bounds::default(),
    )
    .expect("searches");
    assert!(!full.found.is_empty(), "{full:?}");

    let refused = retrieval::search(
        &connection,
        &narrow,
        &Ask::all("embargoed"),
        &Bounds::default(),
    )
    .expect("searches");
    let absent = retrieval::search(
        &connection,
        &narrow,
        &Ask::all("wordthatisinneitherrepository"),
        &Bounds::default(),
    )
    .expect("searches");

    assert_eq!(
        refused, absent,
        "not allowed and no match are the same answer, to the byte"
    );
    assert!(
        refused
            .sources
            .iter()
            .all(|source| source.repository != "closed"),
        "a repository outside the view is never named: {refused:?}"
    );
}

#[test]
fn a_page_is_bounded_by_rows_and_its_cursor_walks_every_hit_exactly_once() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let body: String = (0..200)
        .map(|line| format!("the drainQueue call happens at line {line}\n"))
        .collect();
    let tree = repository(path, &[("docs/long.md", &body)]);
    let connection = database();
    retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");

    let all = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("drain"),
        &Bounds {
            rows: 1000,
            batch_bytes: usize::MAX,
            ..Bounds::default()
        },
    )
    .expect("searches");
    assert!(all.found.len() > 4, "{:?}", all.found.len());
    assert!(all.cursor.is_none(), "{all:?}");

    let mut walked = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = retrieval::search(
            &connection,
            &view("svc", path, &tree),
            &Ask::all("drain"),
            &Bounds {
                rows: 2,
                cursor: cursor.clone(),
                batch_bytes: usize::MAX,
                ..Bounds::default()
            },
        )
        .expect("searches");
        pages += 1;
        assert!(pages < 100, "the cursor is not advancing");
        assert!(page.found.len() <= 2, "{page:?}");
        walked.extend(page.found.clone());
        match page.cursor.clone() {
            Some(next) => {
                assert_eq!(page.truncated, Some(retrieval::Truncation::Rows));
                assert_eq!(page.found.len(), 2, "{page:?}");
                cursor = Some(next);
            }
            None => break,
        }
    }
    assert_eq!(walked, all.found, "the cursor walks exactly the same rows");

    // A cursor survives being carried as text.
    let encoded = retrieval::Cursor {
        score: -1.25,
        repository: "svc".into(),
        path: "docs/long.md".into(),
        start_byte: 42,
    };
    assert_eq!(
        retrieval::Cursor::decode(&encoded.encode()),
        Some(encoded.clone())
    );
}

#[test]
fn one_read_is_bounded_by_its_span_and_a_batch_by_its_total() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    // One very long line: a twenty-line chunk is not a bounded read.
    let long = format!("drainQueue {}\n", "padding ".repeat(500));
    let body: String = (0..60).map(|_| long.clone()).collect();
    let tree = repository(path, &[("docs/wide.md", &body)]);
    let connection = database();
    retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");

    let clipped = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("drain"),
        &Bounds {
            rows: 10,
            span_bytes: 512,
            batch_bytes: usize::MAX,
            ..Bounds::default()
        },
    )
    .expect("searches");
    assert!(!clipped.found.is_empty(), "{clipped:?}");
    for found in &clipped.found {
        assert!(
            found.end_byte - found.start_byte <= 512,
            "one read is capped: {found:?}"
        );
        assert!(found.clipped, "and says it was cut: {found:?}");
    }

    let batched = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("drain"),
        &Bounds {
            rows: 10,
            span_bytes: 512,
            batch_bytes: 1024,
            ..Bounds::default()
        },
    )
    .expect("searches");
    let total: i64 = batched
        .found
        .iter()
        .map(|found| found.end_byte - found.start_byte)
        .sum();
    assert!(total <= 1024, "the batch is capped too: {total}");
    assert!(
        batched.found.len() < clipped.found.len(),
        "the byte cap bit before the row cap: {batched:?}"
    );
    assert_eq!(batched.truncated, Some(retrieval::Truncation::Bytes));
    assert!(batched.cursor.is_some(), "and the rest is reachable");
}

#[test]
fn a_file_keeps_its_own_span_however_far_it_ranks_behind_the_rest_of_the_tree() {
    // The failure this pins: an unscoped search takes the top N rows of the
    // whole tree and a caller filters them by path afterwards, so a file
    // whose own best span ranks below those N never appears at all — and the
    // caller cites its opening lines instead of its answer.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let mut files: Vec<(String, String)> = Vec::new();
    for index in 0..40 {
        files.push((
            format!("docs/noise-{index:02}.md"),
            "the queue drains and drains and drains\n".repeat(4),
        ));
    }
    // The answer is one line, deep in a file that says nothing else about it.
    let mut answer = "unrelated prose\n".repeat(30);
    answer.push_str("the queue drains through the compatibility adapter\n");
    files.push(("docs/answer.md".into(), answer));
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let tree = repository(path, &borrowed);

    let connection = database();
    retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");

    let unscoped = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("queue drains"),
        &Bounds {
            rows: 8,
            ..Bounds::default()
        },
    )
    .expect("searches");
    assert!(
        !unscoped
            .found
            .iter()
            .any(|found| found.path == "docs/answer.md"),
        "the whole-tree search is crowded out, which is the point: {:?}",
        unscoped.found.iter().map(|f| &f.path).collect::<Vec<_>>()
    );

    let scoped = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::within("queue drains", "docs/answer.md"),
        &Bounds {
            rows: 8,
            ..Bounds::default()
        },
    )
    .expect("searches");
    assert_eq!(scoped.found.len(), 1, "{scoped:?}");
    assert_eq!(scoped.found[0].path, "docs/answer.md");
    assert!(
        scoped.found[0].start_line > 20,
        "and it is the span that holds the answer, not the file's opening: {:?}",
        scoped.found[0]
    );
}

#[test]
fn a_multi_term_query_can_ask_for_every_term_or_for_the_best_partial_match() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let tree = repository(
        path,
        &[
            ("docs/one.md", "the adapter stays\n"),
            ("docs/two.md", "the queue drains\n"),
        ],
    );
    let connection = database();
    retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");

    let all = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("adapter queue"),
        &Bounds::default(),
    )
    .expect("searches");
    assert!(
        all.found.is_empty(),
        "no chunk holds both terms: {:?}",
        all.found
    );

    let partial = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("adapter queue").partial(),
        &Bounds::default(),
    )
    .expect("searches");
    let paths: Vec<&str> = partial.found.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths.len(), 2, "{partial:?}");
    assert!(paths.contains(&"docs/one.md") && paths.contains(&"docs/two.md"));
}

#[test]
fn an_index_built_by_another_compiler_is_lagging_however_current_its_tree() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let tree = repository(path, &[("src/queue.rs", "pub fn drainQueue() {}\n")]);
    let connection = database();
    let manifest = retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");
    assert_eq!(
        retrieval::search(
            &connection,
            &view("svc", path, &tree),
            &Ask::all("drain"),
            &Bounds::default()
        )
        .expect("searches")
        .sources[0]
            .state,
        State::Complete
    );

    // The same tree, built by something else. Its rows are not this build's
    // rows: a different pre-tokeniser or chunk size means a different index
    // for the same source, and trusting it would be trusting an index
    // nobody here produced.
    retrieval::record_manifest(
        &connection,
        &retrieval::Manifest {
            compiler: "cbr-index/0".into(),
            ..manifest
        },
    )
    .expect("records");
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("drain"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(answer.sources[0].state, State::Lagging, "{answer:?}");
    assert!(answer.declares_its_gaps(), "{answer:?}");
}

#[test]
fn an_index_built_by_the_previous_compiler_is_lagging_under_this_one() {
    // Not a synthetic version: `cbr-index/1` is the identity this build
    // shipped under before chunks were bounded in bytes. An index built
    // under it holds one chunk where this build holds several, so it ranks
    // differently and excerpts differently — it is a different index of the
    // same tree, and must not pass as current.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let tree = repository(path, &[("src/queue.rs", "pub fn drainQueue() {}\n")]);
    let connection = database();
    let manifest = retrieval::build(&connection, "svc", path, &tree, 1).expect("builds");
    assert_eq!(manifest.compiler, "cbr-index/2", "this build's identity");

    retrieval::record_manifest(
        &connection,
        &retrieval::Manifest {
            compiler: "cbr-index/1".into(),
            ..manifest
        },
    )
    .expect("records");
    let answer = retrieval::search(
        &connection,
        &view("svc", path, &tree),
        &Ask::all("drain"),
        &Bounds::default(),
    )
    .expect("searches");
    assert_eq!(
        answer.sources[0].state,
        State::Lagging,
        "an index from the previous build is lagging, not complete: {answer:?}"
    );
    assert!(answer.declares_its_gaps(), "{answer:?}");
}
