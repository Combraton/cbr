//! **J1, on a real registered repository, with no model.**
//!
//! The repository is this one: CBR's own checkout, registered at launch and
//! read at the tree `HEAD` names. Its decision records and its source are the
//! real thing, which is the point — a journey run against a fixture written
//! to be found proves that the fixture was found.
//!
//! # The oracle, declared before the run
//!
//! The question asked is **"why does CBR's source identity use `gix` rather
//! than the installed `git` binary, and where is that decided?"**. A correct
//! packet for that question must contain:
//!
//! 1. **the decision record** `docs/decisions/001-standalone-v0.1-scope-and-stack.md`,
//!    which is where question 11 is answered;
//! 2. **the code the decision is about**, `crates/cbr-identity/src/lib.rs`;
//! 3. **its own coverage**: the frontier it searched, named as a tree, and
//!    the gaps it did not reach.
//!
//! And one trap it must not fall into:
//!
//! 4. **nothing selected from source may be labelled `binding`.** A file in a
//!    repository is evidence of what the repository says; it is not an
//!    accepted decision, and a packet that promotes it to binding would be
//!    asserting that CBR has decided something it has only read. Only a claim
//!    the authority accepted, or authority content the request supplied, may
//!    carry that label.
//!
//! Every citation must additionally resolve to an exact span at the named
//! tree: the test fetches each cited artifact and compares it with the blob
//! the packet says it came from.
//!
//! A packet that is correct and cited but omits 1 or 2 is a **failed J1**,
//! not a partial one. That is what scoring against a predeclared list means.

use std::io::Read as _;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

/// The decision record the question is about.
const DECISION_PATH: &str = "docs/decisions/001-standalone-v0.1-scope-and-stack.md";
/// The code that decision is about.
const CODE_PATH: &str = "crates/cbr-identity/src/lib.rs";

fn provider_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let binary = path.join("cbr-provider");
    assert!(
        binary.exists(),
        "{} is not built; run the tests with `cargo test --workspace`",
        binary.display()
    );
    binary
}

/// This repository's root: the real checkout J1 runs against.
fn checkout() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    assert!(
        path.join(".git").exists(),
        "{} is not a checkout",
        path.display()
    );
    path
}

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
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

struct Fixture {
    directory: tempfile::TempDir,
    socket: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let sockets = directory.path().join("s");
        std::fs::create_dir(&sockets).expect("socket dir");
        std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
        std::fs::write(
            directory.path().join("cbr.json"),
            r#"{"format":"cbr-config/1","principal":"owner"}"#,
        )
        .expect("config");
        Self {
            socket: sockets.join("cbr.sock"),
            directory,
        }
    }

    fn data(&self) -> PathBuf {
        self.directory.path().join("data")
    }

    fn start(&self, registrations: &[String]) -> Child {
        let mut command = Command::new(provider_binary());
        command
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(self.directory.path().join("cbr.json"))
            .arg("--socket")
            .arg(&self.socket);
        for registration in registrations {
            command.arg("--register-repository").arg(registration);
        }
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("provider starts");
        let started = Instant::now();
        while UnixStream::connect(&self.socket).is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    fn cbr(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cbr"))
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(self.data().join("credentials").join("owner"))
            .output()
            .expect("cbr runs")
    }
}

fn ok(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "cbr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("canonical JSON on stdout")
}

fn at(value: &Value, path: &[&str]) -> Value {
    let mut current = value.clone();
    for name in path {
        current = current.get(name).cloned().unwrap_or(Value::Null);
    }
    current
}

fn text(value: &Value, path: &[&str]) -> String {
    at(value, path).as_str().unwrap_or_default().to_string()
}

/// Every byte under a directory, for the cost a run leaves on disk.
fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => directory_bytes(&entry.path()),
            _ => entry.metadata().map(|data| data.len()).unwrap_or(0),
        })
        .sum()
}

fn array(value: &Value, path: &[&str]) -> Vec<Value> {
    at(value, path)
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default()
}

/// Drive the provider's clock-driven preparation by asking it something, and
/// return the packet once one is published.
fn wait_for_packet(fixture: &Fixture, request: &str) -> (Value, Duration) {
    let started = Instant::now();
    loop {
        let inspected = ok(&fixture.cbr(&["request", request]));
        if !array(&inspected, &["packets"]).is_empty() {
            let elapsed = started.elapsed();
            return (ok(&fixture.cbr(&["packet", request])), elapsed);
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "no packet after two minutes: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn j1_a_registered_repository_answers_a_question_with_a_cited_packet() {
    let repository = checkout();
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let submitted = Instant::now();
    let outcome = ok(&fixture.cbr(&[
        "context",
        "j1",
        "--repo",
        repository.to_str().expect("utf-8"),
        "--repo-id",
        "cbr",
        "--want",
        &format!("decision=source:{DECISION_PATH}"),
        "--want",
        &format!("code=source:{CODE_PATH}"),
        "--selector",
        "gix source identity tree blob",
        "--task",
        "why does source identity use gix rather than the git binary",
        "--capacity",
        "65536",
    ]));
    assert_eq!(
        text(&outcome, &["outcome", "state"]),
        "preparing",
        "{outcome:?}"
    );

    let (packet, waited) = wait_for_packet(&fixture, "j1");
    let to_first_packet = submitted.elapsed();

    // ---- the oracle, scored -------------------------------------------
    let sections = array(&packet, &["sections"]);
    let items = array(&packet, &["items"]);
    let citations = array(&packet, &["citations"]);
    let coverage = array(&packet, &["coverage"]);

    let satisfied: Vec<String> = items
        .iter()
        .filter(|item| text(item, &["result"]) == "satisfied")
        .map(|item| text(item, &["item_id"]))
        .collect();

    // 1 and 2: both required facts, by item, not by hope.
    assert!(
        satisfied.contains(&"decision".to_string()),
        "required fact 1 (the decision record) is missing: {items:?}"
    );
    assert!(
        satisfied.contains(&"code".to_string()),
        "required fact 2 (the code it is about) is missing: {items:?}"
    );

    // 3: the packet says what it searched and what it did not reach.
    assert_eq!(coverage.len(), 1, "{coverage:?}");
    assert_eq!(
        text(&coverage[0], &["frontier"]),
        tree,
        "the coverage names the tree that was searched: {coverage:?}"
    );
    assert!(
        text(&coverage[0], &["producer"]).contains("cbr-context-compiler/1"),
        "the compiler names itself: {coverage:?}"
    );

    // 4: the trap. Source is evidence; only a decided claim is binding.
    for section in &sections {
        if section.get("claim").is_none() {
            assert_ne!(
                text(section, &["label"]),
                "binding",
                "source read from a repository is not a binding decision: {section:?}"
            );
        }
    }

    // Every citation resolves to an exact span at the named tree.
    assert!(!citations.is_empty(), "{packet:?}");
    for citation in &citations {
        let artifact = text(citation, &["evidence", "artifact", "id"]);
        let digest = text(citation, &["evidence", "digest"]);
        let blob = artifact
            .strip_prefix("src.")
            .expect("a source citation names its blob");
        let out = fixture.directory.path().join(format!("{blob}.bytes"));
        let fetched = fixture.cbr(&[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ]);
        assert!(
            fetched.status.success(),
            "a citation must fetch: {}",
            String::from_utf8_lossy(&fetched.stderr)
        );
        let from_packet = std::fs::read(&out).expect("reads");
        let from_tree = Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["cat-file", "blob", blob])
            .output()
            .expect("git runs");
        assert!(from_tree.status.success(), "the blob is in the tree");
        assert_eq!(
            from_packet, from_tree.stdout,
            "the cited artifact is the blob at the named tree, exactly"
        );
    }

    // What was omitted, and why, is in the packet rather than implied.
    let omissions = array(&packet, &["omissions"]);
    for omission in &omissions {
        assert!(
            !text(omission, &["reason"]).is_empty(),
            "an omission carries its reason: {omission:?}"
        );
    }

    // ---- cost, recorded even with no model ----------------------------
    // `waited` is the index build and the compile; `to_first_packet` adds the
    // submit and the client's own round trips.
    let blobs = git(&repository, &["ls-tree", "-r", "--name-only", &tree])
        .lines()
        .count();
    let store = directory_bytes(&fixture.data());
    eprintln!(
        "J1 cost: model none, no tokens. Time to first packet {to_first_packet:?}, of which \
         index build and compile {waited:?}. Tree {tree}, {blobs} blobs in it. Store after the \
         run {store} bytes. Sections {}, citations {}, coverage gaps {}.",
        sections.len(),
        citations.len(),
        coverage
            .first()
            .map(|one| array(one, &["gaps"]).len())
            .unwrap_or(0)
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

#[test]
fn j1_negative_control_a_moved_tree_is_never_silently_answered_from() {
    // **The control.** A repository moves to a new tree and nothing
    // re-ingests it. A packet already published must still name the tree it
    // was prepared at, and a request that names the old tree must still be
    // answered from the old tree's bytes — not from whatever is checked out
    // now. The control fails if a citation resolves to the new tree's blob.
    //
    // Purpose-built rather than run against this checkout: the control has
    // to move a repository, and moving this one would make the test depend
    // on its own branch's history.
    let fixture = Fixture::new();
    let repository = fixture.directory.path().join("moving");
    std::fs::create_dir_all(repository.join("src")).expect("src");
    std::fs::write(
        repository.join("src/queue.rs"),
        "pub fn drainQueue() -> &'static str {\n    \"the old behaviour\"\n}\n",
    )
    .expect("writes");
    for arguments in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    ] {
        git(&repository, &arguments);
    }
    let old_commit = git(&repository, &["rev-parse", "HEAD"]);
    let old_tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let old_blob = git(&repository, &["rev-parse", "HEAD:src/queue.rs"]);

    let mut provider = fixture.start(&[format!("moving={}", repository.display())]);
    let submit = |request: &str, commit: &str| {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "moving",
            "--commit",
            commit,
            "--want",
            "code=source:src/queue.rs",
            "--selector",
            "drain queue",
            "--capacity",
            "65536",
        ]))
    };
    submit("before", &old_commit);
    let (before, _) = wait_for_packet(&fixture, "before");
    let cited_before = citation_blobs(&before);
    assert_eq!(cited_before, vec![old_blob.clone()], "{before:?}");

    // The repository moves. Nothing re-ingests it.
    std::fs::write(
        repository.join("src/queue.rs"),
        "pub fn drainQueue() -> &'static str {\n    \"the new behaviour\"\n}\n",
    )
    .expect("writes");
    git(&repository, &["add", "."]);
    git(
        &repository,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "two",
        ],
    );
    let new_tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let new_blob = git(&repository, &["rev-parse", "HEAD:src/queue.rs"]);
    assert_ne!(old_tree, new_tree);
    assert_ne!(old_blob, new_blob);

    // 1. The published packet still names the tree it was prepared at.
    let (again, _) = wait_for_packet(&fixture, "before");
    let basis = array(&again, &["applicability", "basis", "repositories"]);
    assert_eq!(text(&basis[0], &["tree"]), old_tree, "{again:?}");
    assert_ne!(
        text(&basis[0], &["tree"]),
        new_tree,
        "a packet never claims applicability to a tree nobody prepared it at"
    );
    assert_eq!(citation_blobs(&again), vec![old_blob.clone()], "{again:?}");

    // 2. A new request at the old tree is still answered from the old tree,
    //    even though the index has since been rebuilt for the new one.
    submit("after-at-new", "HEAD");
    let (at_new, _) = wait_for_packet(&fixture, "after-at-new");
    assert_eq!(
        citation_blobs(&at_new),
        vec![new_blob.clone()],
        "a request at the new tree gets the new bytes: {at_new:?}"
    );
    submit("after-at-old", &old_commit);
    let (at_old, _) = wait_for_packet(&fixture, "after-at-old");
    assert_eq!(
        citation_blobs(&at_old),
        vec![old_blob],
        "and one at the old tree still gets the old bytes: {at_old:?}"
    );
    assert_ne!(citation_blobs(&at_old), citation_blobs(&at_new));

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

/// The blobs a packet's citations name, in order.
fn citation_blobs(packet: &Value) -> Vec<String> {
    array(packet, &["citations"])
        .iter()
        .map(|citation| {
            text(citation, &["evidence", "artifact", "id"])
                .strip_prefix("src.")
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

#[test]
fn a_sealed_packet_rebuilds_to_the_same_digest() {
    // Reproducibility from retained records (TALK section 3): the same
    // request over the same tree compiles to the same bytes. Two requests
    // differ only in their id, which a packet's bytes do not carry.
    let repository = checkout();
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let mut digests = Vec::new();
    for request in ["r-one", "r-two"] {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "cbr",
            "--want",
            &format!("code=source:{CODE_PATH}"),
            "--selector",
            "dirty snapshot tree entries",
            "--capacity",
            "65536",
        ]));
        let (packet, _) = wait_for_packet(&fixture, request);
        let sections: Vec<(String, String)> = array(&packet, &["sections"])
            .iter()
            .map(|section| (text(section, &["section_id"]), text(section, &["label"])))
            .collect();
        let citations: Vec<String> = array(&packet, &["citations"])
            .iter()
            .map(|citation| text(citation, &["evidence", "digest"]))
            .collect();
        digests.push((sections, citations));
    }
    assert_eq!(
        digests[0], digests[1],
        "the same request over the same tree selects the same content"
    );

    let mut stderr = String::new();
    provider.kill().expect("kills");
    if let Some(pipe) = provider.stderr.take() {
        std::io::BufReader::new(pipe)
            .read_to_string(&mut stderr)
            .ok();
    }
    provider.wait().expect("reaps");
    assert!(
        !stderr.contains(repository.to_str().expect("utf-8")),
        "the checkout path is never logged: {stderr}"
    );
}
