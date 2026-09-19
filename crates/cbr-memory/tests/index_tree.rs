//! Retrieval over a real repository: the two indexes built from a real git
//! tree, with the coverage they did not reach stated rather than implied.

use std::path::Path;
use std::process::Command;

use cbr_memory::{anchors, index, lexical};
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
    index::migrate(&connection).expect("migrates");
    connection
}

#[test]
fn indexing_a_tree_makes_it_searchable_and_anchored_and_states_what_it_missed() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    git(path, &["init", "-q", "-b", "main"]);
    std::fs::create_dir(path.join("src")).expect("src");
    std::fs::write(
        path.join("src/user.rs"),
        "pub fn getUserName(id: u64) -> String {\n    String::new()\n}\n",
    )
    .expect("writes");
    std::fs::write(
        path.join("notes.md"),
        "# Decision\n\nThe queue stays on version two.\n",
    )
    .expect("writes");
    // A language CBR does not claim: indexed as text, never anchored.
    std::fs::write(path.join("main.go"), "func GoThing() {}\n").expect("writes");
    // Not text at all.
    std::fs::write(path.join("logo.bin"), [0xff, 0xfe, 0x00, 0x01]).expect("writes");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "first"]);
    let first = cbr_identity::git_basis(path, "HEAD").expect("basis");

    let connection = database();
    let coverage = index::index_tree(&connection, path, &first.tree).expect("indexes");
    assert_eq!(coverage.blobs, 4, "{coverage:?}");
    assert_eq!(
        coverage.indexed, 3,
        "the binary file is not text: {coverage:?}"
    );
    assert_eq!(coverage.binary, 1, "{coverage:?}");
    assert_eq!(coverage.anchored, 1, "only the Rust file: {coverage:?}");
    assert_eq!(
        coverage.unanchored_language, 2,
        "Markdown and Go are indexed but not anchored: {coverage:?}"
    );

    // Searchable by the parts of an identifier, and by prose.
    let hits = lexical::search(&connection, &first.tree, "user name", 10).expect("searches");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].path, "src/user.rs");
    let blob = cbr_identity::read_blob(path, &hits[0].blob).expect("blob");
    let text = String::from_utf8(blob).expect("utf-8");
    let span = &text[hits[0].start_byte as usize..hits[0].end_byte as usize];
    assert!(
        span.contains("getUserName"),
        "the hit's span is the bytes it was indexed from: {span:?}"
    );
    assert_eq!(
        lexical::search(&connection, &first.tree, "queue", 10)
            .expect("searches")
            .first()
            .map(|hit| hit.path.clone()),
        Some("notes.md".to_string())
    );

    // Anchored, at this tree, with the blob it was taken from.
    let resolved = anchors::resolve(&connection, &first.tree, "getUserName").expect("resolves");
    assert_eq!(resolved.candidates.len(), 1, "{resolved:?}");
    assert!(!resolved.ambiguous);
    assert_eq!(resolved.candidates[0].path, "src/user.rs");
    assert!(
        anchors::resolve(&connection, &first.tree, "GoThing")
            .expect("resolves")
            .candidates
            .is_empty(),
        "a language CBR does not claim has no anchors at all"
    );

    // Re-indexing the same tree replaces rather than duplicates.
    index::index_tree(&connection, path, &first.tree).expect("re-indexes");
    assert_eq!(
        lexical::search(&connection, &first.tree, "user name", 10)
            .expect("searches")
            .len(),
        1
    );
    assert_eq!(
        anchors::resolve(&connection, &first.tree, "getUserName")
            .expect("resolves")
            .candidates
            .len(),
        1
    );

    // A second tree is indexed beside the first, and each answers for itself.
    std::fs::write(
        path.join("src/user.rs"),
        "pub fn getUserHandle(id: u64) -> String {\n    String::new()\n}\n",
    )
    .expect("writes");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "second"]);
    let second = cbr_identity::git_basis(path, "HEAD").expect("basis");
    index::index_tree(&connection, path, &second.tree).expect("indexes");

    assert_eq!(
        anchors::resolve(&connection, &first.tree, "getUserName")
            .expect("resolves")
            .candidates
            .len(),
        1,
        "the first tree still answers for itself"
    );
    assert!(
        anchors::resolve(&connection, &second.tree, "getUserName")
            .expect("resolves")
            .candidates
            .is_empty(),
        "the name is gone at the second tree"
    );
    assert_eq!(
        anchors::resolve(&connection, &second.tree, "getUserHandle")
            .expect("resolves")
            .candidates
            .len(),
        1
    );
    assert!(
        lexical::search(&connection, &second.tree, "user name", 10)
            .expect("searches")
            .is_empty(),
        "and the old text is not searchable at the new tree"
    );
}
