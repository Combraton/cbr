//! What the lexical index must do for source code, and the one property that
//! decided the engine (STACK section 5).
//!
//! FTS5 has no camelCase splitting, so `getUserName` is one token and a search
//! for `user` will not find it. That is not a detail to discover later: it is
//! most of what searching code means. CBR normalises text in Rust before
//! indexing, so an identifier is searchable by its parts **and** whole, and
//! the tests below are the statement of what that normalisation owes callers.
//!
//! The engine was chosen for atomicity rather than features: an FTS5 table is
//! more rows in the same database, so indexing joins the caller's transaction.
//! `an_index_row_commits_with_its_transaction` is that argument, checked.

use cbr_memory::lexical::{self, Document};
use rusqlite::Connection;

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    lexical::migrate(&connection).expect("migrates");
    connection
}

fn index(connection: &Connection, tree: &str, path: &str, text: &str) {
    for chunk in lexical::chunks(text, 20) {
        lexical::index_chunk(
            connection,
            &Document {
                tree,
                path,
                blob: "sha256:blob",
                chunk: &chunk,
            },
        )
        .expect("indexes");
    }
}

fn paths(connection: &Connection, tree: &str, query: &str) -> Vec<String> {
    lexical::search(connection, tree, query, 10)
        .expect("searches")
        .into_iter()
        .map(|hit| hit.path)
        .collect()
}

#[test]
fn an_identifier_is_found_by_its_parts_and_whole() {
    let connection = database();
    index(
        &connection,
        "t1",
        "src/user.rs",
        "fn getUserName() -> String {}",
    );
    index(
        &connection,
        "t1",
        "src/queue.rs",
        "def publish_to_queue(msg):",
    );
    index(
        &connection,
        "t1",
        "src/http.rs",
        "class HTTPServerError(Exception):",
    );
    index(
        &connection,
        "t1",
        "src/hash.rs",
        "let digest = sha256Digest(bytes);",
    );

    // The parts, which plain FTS5 cannot do.
    assert_eq!(paths(&connection, "t1", "user name"), ["src/user.rs"]);
    assert_eq!(paths(&connection, "t1", "name"), ["src/user.rs"]);
    assert_eq!(paths(&connection, "t1", "queue"), ["src/queue.rs"]);
    assert_eq!(paths(&connection, "t1", "server error"), ["src/http.rs"]);
    assert_eq!(paths(&connection, "t1", "http"), ["src/http.rs"]);
    assert_eq!(paths(&connection, "t1", "sha"), ["src/hash.rs"]);
    assert_eq!(paths(&connection, "t1", "256"), ["src/hash.rs"]);

    // And the identifier as written, which is what a caller pasting a symbol
    // will type.
    assert_eq!(paths(&connection, "t1", "getUserName"), ["src/user.rs"]);
    assert_eq!(
        paths(&connection, "t1", "publish_to_queue"),
        ["src/queue.rs"]
    );
    assert_eq!(paths(&connection, "t1", "HTTPServerError"), ["src/http.rs"]);
}

/// Splitting must not turn different identifiers into the same one, and there
/// is no stemming: `flush` and `flushed` are different symbols in code.
#[test]
fn normalisation_never_conflates_different_identifiers() {
    let connection = database();
    index(&connection, "t1", "a.rs", "fn flushBuffer() {}");
    index(&connection, "t1", "b.rs", "fn flushedBuffer() {}");
    assert_eq!(paths(&connection, "t1", "flush"), ["a.rs"]);
    assert_eq!(paths(&connection, "t1", "flushed"), ["b.rs"]);
    assert!(paths(&connection, "t1", "flushBuffer").contains(&"a.rs".to_string()));
    assert!(!paths(&connection, "t1", "flushBuffer").contains(&"b.rs".to_string()));
}

/// The decisive property: an index row is part of the caller's transaction,
/// so a rolled-back write leaves no searchable trace and no separate crash
/// window.
#[test]
fn an_index_row_commits_with_its_transaction() {
    let mut connection = database();

    let rolled_back = connection.transaction().expect("transaction");
    for chunk in lexical::chunks("fn discarded() {}", 20) {
        lexical::index_chunk(
            &rolled_back,
            &Document {
                tree: "t1",
                path: "gone.rs",
                blob: "sha256:blob",
                chunk: &chunk,
            },
        )
        .expect("indexes");
    }
    assert_eq!(paths(&rolled_back, "t1", "discarded"), ["gone.rs"]);
    rolled_back.rollback().expect("rolls back");
    assert!(paths(&connection, "t1", "discarded").is_empty());

    let committed = connection.transaction().expect("transaction");
    for chunk in lexical::chunks("fn kept() {}", 20) {
        lexical::index_chunk(
            &committed,
            &Document {
                tree: "t1",
                path: "kept.rs",
                blob: "sha256:blob",
                chunk: &chunk,
            },
        )
        .expect("indexes");
    }
    committed.commit().expect("commits");
    assert_eq!(paths(&connection, "t1", "kept"), ["kept.rs"]);
}

/// An index is derived from one tree. Searching a tree never answers with
/// another tree's content, and re-indexing a tree replaces it rather than
/// accumulating.
#[test]
fn a_search_answers_for_one_tree_and_re_indexing_replaces_it() {
    let connection = database();
    index(&connection, "t1", "src/a.rs", "fn original() {}");
    index(&connection, "t2", "src/a.rs", "fn replacement() {}");
    assert_eq!(paths(&connection, "t1", "original"), ["src/a.rs"]);
    assert!(paths(&connection, "t2", "original").is_empty());
    assert_eq!(paths(&connection, "t2", "replacement"), ["src/a.rs"]);

    lexical::remove_tree(&connection, "t1").expect("removes");
    assert!(paths(&connection, "t1", "original").is_empty());
    assert_eq!(
        paths(&connection, "t2", "replacement"),
        ["src/a.rs"],
        "removing one tree leaves the other"
    );
}

/// A hit names an exact span of the blob, because a citation must resolve to
/// bytes rather than to a file.
#[test]
fn a_hit_names_an_exact_span_of_its_blob() {
    let connection = database();
    let text = (1..=45)
        .map(|line| format!("line {line} of the file\n"))
        .collect::<String>();
    let mut text = text;
    text.push_str("fn findMe() {}\n");
    index(&connection, "t1", "src/long.rs", &text);

    let hits = lexical::search(&connection, "t1", "find me", 10).expect("searches");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let hit = &hits[0];
    let span = &text[hit.start_byte as usize..hit.end_byte as usize];
    assert!(span.contains("fn findMe() {}"), "{span:?}");
    assert!(
        hit.start_line <= 46 && 46 <= hit.end_line,
        "the span names the lines it covers: {hit:?}"
    );
    assert_eq!(hit.blob, "sha256:blob");
}

/// Chunking is what makes spans exact, so it must tile the text: every byte in
/// exactly one chunk, in order, with the line numbers to match.
#[test]
fn chunks_tile_the_text_exactly() {
    for (case, text) in [
        ("empty", String::new()),
        ("one line, no newline", "only line".to_string()),
        ("trailing newline", "a\nb\nc\n".to_string()),
        (
            "long",
            (1..=101)
                .map(|line| format!("{line}\n"))
                .collect::<String>(),
        ),
        ("blank lines", "a\n\n\nb\n".to_string()),
    ] {
        let chunks = lexical::chunks(&text, 20);
        let mut offset = 0usize;
        let mut line = 1u32;
        for chunk in &chunks {
            assert_eq!(
                chunk.start_byte as usize, offset,
                "{case}: chunks are contiguous"
            );
            assert_eq!(chunk.start_line, line, "{case}: lines follow the bytes");
            assert_eq!(
                &text[chunk.start_byte as usize..chunk.end_byte as usize],
                chunk.text,
                "{case}: a chunk is its own bytes"
            );
            offset = chunk.end_byte as usize;
            line = chunk.end_line + 1;
        }
        assert_eq!(offset, text.len(), "{case}: chunks cover the whole text");
    }
}

/// The normalisation itself, stated as text: this is what is stored, and what
/// a query is turned into.
#[test]
fn the_pre_tokeniser_emits_the_whole_and_the_parts() {
    assert_eq!(
        lexical::normalise("getUserName"),
        "getusername get user name"
    );
    assert_eq!(
        lexical::normalise("HTTPServerError"),
        "httpservererror http server error"
    );
    assert_eq!(lexical::normalise("publish_to_queue"), "publish to queue");
    assert_eq!(
        lexical::normalise("sha256Digest"),
        "sha256digest sha 256 digest"
    );
    assert_eq!(lexical::normalise("foo.bar()"), "foo bar");
    assert_eq!(lexical::normalise("kebab-case-name"), "kebab case name");
    assert_eq!(lexical::normalise("plain words here"), "plain words here");
}
