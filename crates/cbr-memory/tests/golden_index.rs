//! A golden index digest over a fixed fixture tree.
//!
//! **Why this test exists.** Three times in this milestone a rule was written
//! down, tested, and then not applied to the next case that needed it; twice
//! it was review that caught it rather than the repository. One of those was
//! [`retrieval::COMPILER`](cbr_memory::retrieval::COMPILER): the chunker
//! changed and the compiler string did not, so an index built by the old
//! chunker would have been reported `complete` at a basis it does not
//! describe. Remembering to bump a version string is not a mechanism.
//!
//! So the mechanism is here. The digest below covers everything an index
//! build decides about a fixed tree: which blobs it reached, how it split
//! them into chunks, what it anchored, what it could not reach, and what a
//! fixed set of queries matches. Change any of that and this test fails and
//! says to bump `COMPILER`. Bump `COMPILER` without changing the build and it
//! fails too, because the digest is over the string as well.
//!
//! **What the digest deliberately leaves out: ranking order.** A query's
//! matched set is in the digest; the BM25 order of that set is not. Ranking
//! is unevaluated (`docs/verification/JOURNEYS.md`, J1), ties between equal
//! scores are not ordered by anything this crate controls, and a guard that
//! fails on a tie would be turned off rather than fixed.

use std::path::Path;
use std::process::Command;

use cbr_memory::{anchors, index, lexical, retrieval};
use rusqlite::Connection;

/// The fixture tree, written byte for byte here rather than checked in as
/// files, so that the thing the digest is over cannot drift by an editor
/// adding a trailing newline. Each file is here for one reason:
const FIXTURE: &[(&str, &[u8])] = &[
    // Rust: anchored, and long enough to split into several chunks so the
    // chunker's line and byte bounds are both in the digest.
    ("src/queue.rs", QUEUE_RS.as_bytes()),
    // TypeScript: a second grammar, so a tree-sitter bump shows up here.
    ("src/app.ts", APP_TS.as_bytes()),
    // Text in a language with no anchors: indexed, never anchored.
    ("vendor/main.go", b"package main\n\nfunc GoThing() {}\n"),
    // Markdown: the ordinary case, and the one a decision record is.
    (
        "notes.md",
        b"# Decision\n\nThe queue stays on version two.\n",
    ),
    // Few lines, many bytes: this one crosses `CHUNK_BYTES` long before it
    // reaches `CHUNK_LINES`, so the byte bound shows up in the rows and not
    // only in the recorded parameter. Without it a change to the byte bound
    // would split nothing in this fixture.
    ("wide.md", WIDE_MD.as_bytes()),
    // A final line with no newline of its own.
    ("tail.txt", b"no trailing newline"),
    // Not valid UTF-8: counted as binary, indexed as nothing.
    ("logo.bin", &[0xff, 0xfe, 0x00, 0x01]),
];

const QUEUE_RS: &str = "\
//! A queue that flushes on a deadline.

pub struct Queue {
    pending: Vec<String>,
}

impl Queue {
    pub fn new() -> Self {
        Queue {
            pending: Vec::new(),
        }
    }

    /// Push one entry. The queue never drops on push.
    pub fn push(&mut self, entry: String) {
        self.pending.push(entry);
    }

    /// Flush every pending entry and return how many went out.
    pub fn flush(&mut self) -> usize {
        let count = self.pending.len();
        self.pending.clear();
        count
    }
}

pub fn drainQueue(queue: &mut Queue) -> usize {
    queue.flush()
}
";

const WIDE_MD: &str = "\
Paragraph 0 the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. \n\
Paragraph 1 the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. \n\
Paragraph 2 the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. \n\
Paragraph 3 the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. \n\
Paragraph 4 the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. the queue drains on a deadline and the adapter rewrites it. \n\
";

const APP_TS: &str = "\
export interface Credential {
  token: string;
}

export function redactCredential(value: string): string {
  return value.replace(/token=[^&]+/g, \"token=REDACTED\");
}
";

/// The queries the digest pins. They cover the pre-tokeniser (an identifier
/// that splits on case), a term that must not be stemmed into another, and a
/// two-term query under both readings.
const QUERIES: &[(&str, lexical::Terms)] = &[
    ("drainQueue", lexical::Terms::All),
    ("flush", lexical::Terms::All),
    ("flushed", lexical::Terms::All),
    ("redactCredential", lexical::Terms::All),
    ("queue version", lexical::Terms::All),
    ("queue version", lexical::Terms::Any),
];

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

/// Write the fixture and return its tree id. A tree id is content-addressed,
/// so no commit is made and no author, date or message can reach it.
fn fixture(path: &Path) -> String {
    git(path, &["init", "-q", "-b", "main"]);
    for (name, bytes) in FIXTURE {
        let file = path.join(name);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&file, bytes).expect("writes");
    }
    // A symlink: its content is a path, not text of this tree, and the
    // indexer skips it. Knowscroll's `AGENTS.md` is one.
    std::os::unix::fs::symlink("notes.md", path.join("link.md")).expect("symlink");
    git(path, &["add", "-A"]);
    git(path, &["write-tree"])
}

/// Everything the build decided about this tree, in one canonical text.
fn record(connection: &Connection, tree: &str, coverage: &index::Coverage) -> String {
    let mut lines = vec![
        "cbr-memory golden index/1".to_string(),
        format!("compiler {}", retrieval::COMPILER),
        format!("chunk_lines {}", index::CHUNK_LINES),
        format!("chunk_bytes {}", lexical::CHUNK_BYTES),
        format!("max_blob_bytes {}", index::MAX_BLOB_BYTES),
        format!("tree {tree}"),
        format!(
            "coverage blobs={} indexed={} anchored={} binary={} too_large={} unanchored={}",
            coverage.blobs,
            coverage.indexed,
            coverage.anchored,
            coverage.binary,
            coverage.too_large,
            coverage.unanchored_language,
        ),
    ];
    for (kind, count) in &coverage.unanchored_by_kind {
        lines.push(format!("coverage_kind {kind} {count}"));
    }

    let mut statement = connection
        .prepare(
            "SELECT path, blob, start_byte, end_byte, start_line, end_line
             FROM lexical_chunk WHERE tree = ?1
             ORDER BY path, start_byte",
        )
        .expect("prepares");
    let rows = statement
        .query_map([tree], |row| {
            Ok(format!(
                "chunk {} {} {} {} {} {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .expect("queries");
    for row in rows {
        lines.push(row.expect("row"));
    }

    let mut statement = connection
        .prepare(
            "SELECT path, name, kind, start_line, end_line, is_definition
             FROM anchor WHERE tree = ?1
             ORDER BY path, name, kind, start_line",
        )
        .expect("prepares");
    let rows = statement
        .query_map([tree], |row| {
            Ok(format!(
                "anchor {} {} {} {} {} {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .expect("queries");
    for row in rows {
        lines.push(row.expect("row"));
    }

    for (query, terms) in QUERIES {
        let hits =
            lexical::search_after(connection, tree, query, None, None, *terms, 32).expect("search");
        let reading = match terms {
            lexical::Terms::All => "all",
            lexical::Terms::Any => "any",
        };
        // Sorted, not ranked: see the module header.
        let mut matched: Vec<String> = hits
            .iter()
            .map(|hit| format!("{}@{}", hit.path, hit.start_byte))
            .collect();
        matched.sort();
        lines.push(format!(
            "query {reading} {query:?} {} [{}]",
            matched.len(),
            matched.join(" ")
        ));
    }

    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[test]
fn the_index_a_fixed_tree_produces_has_not_changed_without_the_compiler_string() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    let tree = fixture(path);

    let connection = Connection::open_in_memory().expect("opens");
    index::migrate(&connection).expect("migrates");
    let coverage = index::index_tree(&connection, path, &tree).expect("indexes");

    let text = record(&connection, &tree, &coverage);
    let digest = cbr_encoding::digest_bytes(text.as_bytes());

    assert_eq!(
        digest,
        retrieval::GOLDEN_FIXTURE_DIGEST,
        "\n\nThe index this fixture produces has changed.\n\n\
         If the change is intended, it is a change to how an index is built, \
         so `retrieval::COMPILER` must be bumped in the same commit and this \
         digest updated with it — an index built by the previous build is not \
         this build's index, and reporting it `complete` cites the wrong \
         bytes. If the change is not intended, this is the bug.\n\n\
         What the build now produces:\n\n{text}"
    );

    // Indexing the same tree twice is idempotent, so the digest is a property
    // of the tree and the build, not of how many times it ran.
    let again = index::index_tree(&connection, path, &tree).expect("reindexes");
    assert_eq!(coverage, again);
    assert_eq!(
        cbr_encoding::digest_bytes(record(&connection, &tree, &again).as_bytes()),
        digest
    );

    // The guard is only worth having if the fixture reaches the shapes it
    // claims to: an anchored file in two languages, an unanchored language,
    // a blob that is not text, and a symlink that is not a blob of this tree.
    assert!(coverage.anchored >= 2, "{coverage:?}");
    assert_eq!(coverage.binary, 1, "{coverage:?}");
    assert!(coverage.unanchored_language >= 2, "{coverage:?}");
    assert_eq!(coverage.blobs, FIXTURE.len(), "the symlink is not a blob");
    assert!(
        anchors::resolve(&connection, &tree, "drainQueue")
            .expect("resolves")
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/queue.rs")
    );
}
