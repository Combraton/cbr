//! `scripts/escape_scan.py`, the survey behind the candidate cut.
//!
//! A candidate's text and path are shown to a model within
//! `selection::CANDIDATE_TEXT_BYTES` and `CANDIDATE_PATH_BYTES` as a request
//! body carries them, and a field past its bound is shown cut, with a
//! marker. Whether a repository has such fields is a property of its tree,
//! so the script reads a commit's tree the way the indexer and retrieval
//! do — regular files up to `MAX_BLOB_BYTES`, valid UTF-8, chunks of at
//! most `CHUNK_LINES` lines closed at the first newline at or past
//! `CHUNK_BYTES`, each clipped at retrieval's `span_bytes` — and counts what
//! a body would carry. `selection::tests` holds its constants to the code's.
//!
//! The fixture below puts one of each shape in a tree, so every count the
//! script prints is one the test predicts from how the fixture was built.
//! Two properties matter as much as the counts: **it prints no repository
//! text**, only the commit, counts and repository-relative paths, because
//! it is meant to be run over repositories whose text may not be copied;
//! and **it writes nothing**, because it reads through `git ls-tree` and
//! `git cat-file` and nothing else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["-c", "user.name=cbr", "-c", "user.email=cbr@invalid"])
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

fn scan(repository: &Path, arguments: &[&str]) -> Output {
    Command::new("python3")
        .arg(root().join("scripts").join("escape_scan.py"))
        .arg("--repo")
        .arg(repository)
        .args(arguments)
        .output()
        .expect("python3 runs")
}

/// Retrieval's span, in raw bytes.
const SPAN_BYTES: usize = 4_096;

/// The first line of the escape-dense file, which holds a planted word.
const DENSE_FIRST: &str = "The queue drains here, planted-beta.\n";

/// A directory component that a body carries in 700 bytes and a file
/// system holds in 200: 100 `x` and 100 U+0001.
fn component() -> String {
    format!("{}{}", "x".repeat(100), "\u{1}".repeat(100))
}

fn nested_path() -> String {
    let c = component();
    format!("nested/{c}/{c}/{c}/deep.md")
}

/// A path a body carries in exactly 1,024 bytes, the path's cut: `edge/`,
/// 169 U+0001 at six bytes each, and `/f.md`. At the bound, not past it.
fn edge_path() -> String {
    format!("edge/{}/f.md", "\u{1}".repeat(169))
}

/// The first line of a file whose last line has no newline and is dense
/// in C0 controls: a cut span in the chunk that ends the file.
const TAIL_FIRST: &str = "A tail line, planted-eta.\n";

/// How many U+0001 follow [`TAIL_FIRST`], with no newline after them.
const TAIL_CONTROLS: usize = 1_500;

/// **Every word `planted` is repository text**, so it must never be in
/// what the script prints.
struct Fixture {
    _directory: tempfile::TempDir,
    checkout: PathBuf,
    commit: String,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let checkout = directory.path().join("checkout");
        std::fs::create_dir_all(&checkout).expect("checkout");
        let write = |path: &str, bytes: &[u8]| {
            let path = checkout.join(path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, bytes).expect("writes");
        };
        // One span each, carried in about what they hold.
        write(
            "plain.md",
            b"An ordinary line about the queue, planted-alpha.\n",
        );
        write("run.sh", b"#!/bin/sh\necho planted-epsilon\n");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                checkout.join("run.sh"),
                std::fs::Permissions::from_mode(0o755),
            )
            .expect("0755");
        }
        // A span of C0 controls, each carried in six bytes: past the cut.
        let mut dense = DENSE_FIRST.to_string();
        dense.push_str(&"\u{1}".repeat(4_090));
        dense.push('\n');
        write("dense.md", dense.as_bytes());
        // A span of quotation marks, each carried in two: exactly the cut.
        write(
            "quotes.md",
            format!("{}\n", "\"".repeat(SPAN_BYTES)).as_bytes(),
        );
        // A line longer than a span, clipped at it in raw bytes.
        write("long.md", format!("{}\n", "z".repeat(10_000)).as_bytes());
        // Forty-five lines and a tail with no newline: 20, 20, and the rest.
        let mut lines: String = (1..=45)
            .map(|n| format!("line {n} planted-delta\n"))
            .collect();
        lines.push_str("a tail with no newline");
        write("lines.md", lines.as_bytes());
        // Three lines of 1,500 bytes: a chunk closes at the first newline at
        // or past 2,048 bytes, so two lines, then one.
        write(
            "wide.md",
            format!("{}\n", "y".repeat(1_499)).repeat(3).as_bytes(),
        );
        // Two lines ending exactly at 2,048 bytes close a chunk there, and
        // the third is a chunk of its own.
        write(
            "exact.md",
            format!("{}\n{}\ntail\n", "w".repeat(1_023), "w".repeat(1_023)).as_bytes(),
        );
        // A line clipped inside a two-byte character: the provider decodes
        // the half it keeps as U+FFFD, three bytes, so the span is carried
        // in 1 + 2,047 × 2 + 3 = 4,098.
        write("split.md", format!("x{}\n", "é".repeat(2_100)).as_bytes());
        // The file's last chunk, with no newline of its own, past the cut.
        write(
            "tail.md",
            format!("{TAIL_FIRST}{}", "\u{1}".repeat(TAIL_CONTROLS)).as_bytes(),
        );
        // A path a body carries past its bound, and one exactly at it.
        write(&nested_path(), b"A deep file, planted-gamma.\n");
        write(&edge_path(), b"At the edge, planted-zeta.\n");
        // Neither is indexed: one is not UTF-8, one is over the blob bound.
        write("blob.bin", &[0xff, 0xfe, 0x00, 0x01]);
        write("huge.txt", &vec![b'a'; (1 << 20) + 1]);
        // Exactly at the blob bound, which the indexer reads.
        write("edge.txt", &vec![b'b'; 1 << 20]);
        // Not a regular file, and not indexed.
        std::os::unix::fs::symlink("plain.md", checkout.join("link.md")).expect("symlink");

        git(&checkout, &["init", "-q", "-b", "main"]);
        git(&checkout, &["add", "-A"]);
        git(&checkout, &["commit", "-q", "-m", "the tree"]);
        let commit = git(&checkout, &["rev-parse", "HEAD"]);
        Fixture {
            _directory: directory,
            checkout,
            commit,
        }
    }

    /// Every file under the checkout, `.git` included, with its bytes.
    fn every_file(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(at: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(at).expect("reads") {
                let path = entry.expect("entry").path();
                let kind = std::fs::symlink_metadata(&path).expect("metadata");
                if kind.is_dir() {
                    walk(&path, into);
                } else if kind.is_file() {
                    into.insert(path.clone(), std::fs::read(&path).expect("reads"));
                } else {
                    into.insert(path.clone(), Vec::new());
                }
            }
        }
        let mut files = BTreeMap::new();
        walk(&self.checkout, &mut files);
        files
    }
}

fn report(fixture: &Fixture) -> cbr_encoding::Value {
    let output = scan(&fixture.checkout, &["--json", &fixture.commit]);
    assert!(
        output.status.success(),
        "the scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(&output.stdout).expect("the scan prints one JSON object")
}

fn member<'a>(value: &'a cbr_encoding::Value, path: &[&str]) -> &'a cbr_encoding::Value {
    path.iter().fold(value, |value, name| match value {
        cbr_encoding::Value::Object(members) => members
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("no member {name} in {path:?}")),
        other => panic!("{path:?} reaches {other:?}, not an object"),
    })
}

fn number(value: &cbr_encoding::Value, path: &[&str]) -> i64 {
    match member(value, path) {
        cbr_encoding::Value::Int(n) => *n,
        other => panic!("{path:?} is {other:?}, not a number"),
    }
}

fn text<'a>(value: &'a cbr_encoding::Value, path: &[&str]) -> &'a str {
    match member(value, path) {
        cbr_encoding::Value::String(s) => s,
        other => panic!("{path:?} is {other:?}, not a string"),
    }
}

#[test]
fn the_scan_counts_what_a_body_would_carry_of_every_span_at_a_named_commit() {
    let fixture = Fixture::new();
    let report = report(&fixture);
    assert_eq!(text(&report, &["commit"]), fixture.commit);

    // Fifteen regular files; the symlink is not one. Thirteen are
    // indexed, `edge.txt` among them at exactly the blob bound.
    assert_eq!(number(&report, &["files", "regular"]), 15);
    assert_eq!(number(&report, &["files", "indexed"]), 13);
    assert_eq!(number(&report, &["files", "too_large"]), 1);
    assert_eq!(number(&report, &["files", "not_utf8"]), 1);

    // plain, run.sh, dense, quotes, long, split, tail, edge.txt and the
    // two deep files: one each; lines: three; wide and exact: two each.
    assert_eq!(number(&report, &["spans", "total"]), 17);

    // The dense span: its first line whole, then U+0001 up to the clip,
    // each carried in six bytes, and its newline in two.
    let dense = (DENSE_FIRST.len() - 1) + 2 + (SPAN_BYTES - DENSE_FIRST.len()) * 6;
    assert_eq!(
        number(&report, &["spans", "widest", "carried_bytes"]),
        dense as i64
    );
    assert_eq!(text(&report, &["spans", "widest", "path"]), "dense.md");
    assert_eq!(number(&report, &["spans", "widest", "start_line"]), 1);
    assert_eq!(number(&report, &["spans", "widest", "end_line"]), 2);

    // Past 4,096 carried: dense, tail, the quotes at exactly 8,192, and
    // split at 4,098. The long line and `edge.txt` are clipped at 4,096
    // raw bytes and carried in as many, so neither is past it.
    assert_eq!(number(&report, &["spans", "over_4096"]), 4);
    // Past the cut of 8,192: dense and tail. The quotes are at it, not
    // past.
    assert_eq!(number(&report, &["spans", "over_8192"]), 2);
    assert_eq!(number(&report, &["spans", "with_six_byte_escapes"]), 2);

    // Paths are the indexed files'. The nested one is carried in 2,117
    // bytes: `nested/`, three components of 700 and their two slashes, and
    // `/deep.md`. The edge path is at 1,024, which is not past the cut.
    assert_eq!(number(&report, &["paths", "total"]), 13);
    assert_eq!(
        number(&report, &["paths", "longest", "carried_bytes"]),
        7 + 3 * 700 + 2 + 8
    );
    assert_eq!(text(&report, &["paths", "longest", "path"]), nested_path());
    assert_eq!(number(&report, &["paths", "over_1024"]), 1);
}

#[test]
fn the_scan_names_every_span_and_path_a_model_would_be_shown_cut() {
    let fixture = Fixture::new();
    let report = report(&fixture);
    let cut = match member(&report, &["spans", "cut"]) {
        cbr_encoding::Value::Array(items) => items.clone(),
        other => panic!("spans.cut is {other:?}"),
    };
    // The dense span, and the tail that ends its file on its second line
    // with no newline, in path order.
    let tail = (TAIL_FIRST.len() - 1) + 2 + TAIL_CONTROLS * 6;
    let named: Vec<(String, i64, i64, i64)> = cut
        .iter()
        .map(|span| {
            (
                text(span, &["path"]).to_string(),
                number(span, &["start_line"]),
                number(span, &["end_line"]),
                number(span, &["carried_bytes"]),
            )
        })
        .collect();
    let dense = (DENSE_FIRST.len() - 1) + 2 + (SPAN_BYTES - DENSE_FIRST.len()) * 6;
    assert_eq!(
        named,
        vec![
            ("dense.md".to_string(), 1, 2, dense as i64),
            ("tail.md".to_string(), 1, 2, tail as i64),
        ]
    );
    let paths = match member(&report, &["paths", "cut"]) {
        cbr_encoding::Value::Array(items) => items.clone(),
        other => panic!("paths.cut is {other:?}"),
    };
    assert_eq!(
        paths,
        vec![cbr_encoding::Value::String(nested_path())],
        "the one path past 1,024 carried bytes"
    );

    // The same, in the words a person reads.
    let output = scan(&fixture.checkout, &[&fixture.commit]);
    assert!(output.status.success());
    let words = String::from_utf8(output.stdout).expect("utf8");
    assert!(words.contains(&fixture.commit), "{words}");
    assert!(words.contains("dense.md lines 1-2"), "{words}");
    assert!(
        words.contains("spans past 8,192 carried bytes, shown cut: 2"),
        "{words}"
    );
    assert!(
        words.contains("paths past 1,024 carried bytes, shown cut: 1"),
        "{words}"
    );
}

#[test]
fn the_scan_prints_no_repository_text_only_counts_and_paths() {
    let fixture = Fixture::new();
    for arguments in [
        vec!["--json", fixture.commit.as_str()],
        vec![fixture.commit.as_str()],
    ] {
        let output = scan(&fixture.checkout, &arguments);
        assert!(output.status.success());
        for (stream, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
            let printed = String::from_utf8_lossy(bytes);
            assert!(
                !printed.contains("planted"),
                "{arguments:?} printed repository text on {stream}: {printed}"
            );
            assert!(
                !printed.contains('\u{1}'),
                "{arguments:?} printed a raw control character on {stream}"
            );
            assert!(
                !printed.contains("\"\"\"\""),
                "{arguments:?} printed the quotation marks of quotes.md on {stream}"
            );
            for run in ["zzzz", "yyyy", "wwww", "bbbb", "\u{e9}\u{e9}"] {
                assert!(
                    !printed.contains(run),
                    "{arguments:?} printed a span's text on {stream}: {run}"
                );
            }
        }
    }
}

#[test]
fn the_scan_writes_nothing_to_the_repository_it_reads() {
    let fixture = Fixture::new();
    let before = fixture.every_file();
    for arguments in [
        vec!["--json", fixture.commit.as_str()],
        vec![fixture.commit.as_str()],
    ] {
        assert!(scan(&fixture.checkout, &arguments).status.success());
    }
    let after = fixture.every_file();
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the scan created or removed a file"
    );
    assert!(before == after, "the scan changed a file's bytes");
}

#[test]
fn the_scan_refuses_anything_but_a_commit_and_prints_nothing_to_count() {
    let fixture = Fixture::new();
    let tree = git(&fixture.checkout, &["rev-parse", "HEAD^{tree}"]);
    for revision in ["no-such-revision", tree.as_str()] {
        let output = scan(&fixture.checkout, &["--json", revision]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{revision} was scanned: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            output.stdout.is_empty(),
            "{revision}: printed a result for something that is not a commit"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not a commit"),
            "{revision}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // And a revision is required: the scan does not guess one.
    let output = scan(&fixture.checkout, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
