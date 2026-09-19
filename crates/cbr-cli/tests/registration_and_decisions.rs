//! Registering a real checkout, and ingesting its decisions through `cbr`.
//!
//! This is the whole write path a J1 run depends on, exercised over the
//! public socket with no internal shortcut: a repository is registered at
//! launch, a decision document from that repository is ingested as evidence
//! bound to its own basis, a claim citing that evidence is proposed, and the
//! bound authority accepts it. Afterwards the evidence still fetches
//! byte-identically and the claim reads back as accepted at the tree it was
//! made about.
//!
//! No model runs. The decision document is a file this test writes.

use std::io::{BufReader, Read as _};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

const DECISION: &str = "# 0001. Keep the compatibility adapter\n\n\
     Status: accepted\n\n\
     The queue migration keeps the compatibility adapter for one release.\n\
     Renaming the response field breaks the client fixture, so the adapter\n\
     stays until every consumer has moved.\n";

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

struct Fixture {
    directory: tempfile::TempDir,
    socket: PathBuf,
    checkout: PathBuf,
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

        let checkout = directory.path().join("app");
        std::fs::create_dir_all(checkout.join("docs/decisions")).expect("docs");
        std::fs::create_dir_all(checkout.join("src")).expect("src");
        std::fs::write(checkout.join("docs/decisions/0001-adapter.md"), DECISION).expect("writes");
        std::fs::write(
            checkout.join("src/queue.rs"),
            "pub fn drainQueue(limit: u64) -> u64 {\n    limit\n}\n",
        )
        .expect("writes");
        git(&checkout, &["init", "-q", "-b", "main"]);
        git(&checkout, &["add", "."]);
        git(&checkout, &["commit", "-q", "-m", "the adapter decision"]);

        Self {
            socket: sockets.join("cbr.sock"),
            checkout,
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
                started.elapsed() < Duration::from_secs(10),
                "never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    fn credential(&self, principal: &str) -> PathBuf {
        self.data().join("credentials").join(principal)
    }

    fn cbr(&self, principal: &str, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cbr"))
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(self.credential(principal))
            .output()
            .expect("cbr runs")
    }

    fn write(&self, name: &str, text: &str) -> String {
        let path = self.directory.path().join(name);
        std::fs::write(&path, text).expect("writes");
        path.to_str().expect("utf-8").to_string()
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

fn lines(output: &Output) -> Vec<String> {
    assert!(
        output.status.success(),
        "cbr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

fn field(output: &Output, name: &str) -> String {
    lines(output)
        .into_iter()
        .find_map(|line| line.strip_prefix(&format!("{name} ")).map(str::to_string))
        .unwrap_or_else(|| panic!("no {name} line in {:?}", lines(output)))
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

/// One field of the first element of an array member.
fn first(value: &Value, array: &str, field: &str) -> String {
    at(value, &[array])
        .as_array()
        .and_then(|items| items.first().cloned())
        .and_then(|item| item.get(field).and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

#[test]
fn a_registered_checkout_has_its_decision_ingested_cited_and_accepted() {
    let fixture = Fixture::new();
    let registration = format!("app={}", fixture.checkout.display());
    let mut provider = fixture.start(&[registration]);

    // The basis the decision is about, resolved by the client from the
    // checkout it already has. Nothing sends a path over the socket.
    // `basis` prints the target first and the records behind it second.
    let printed = fixture.cbr(
        "owner",
        &[
            "basis",
            "--repo",
            fixture.checkout.to_str().expect("utf-8"),
            "--repo-id",
            "app",
        ],
    );
    let basis = cbr_encoding::parse(lines(&printed)[0].as_bytes()).expect("canonical JSON");
    let tree = first(&basis, "repositories", "tree");
    assert_eq!(tree.len(), 40, "a root tree id: {basis:?}");

    // The decision document itself, ingested as evidence anchored to the
    // commit it was read at.
    let document = fixture.checkout.join("docs/decisions/0001-adapter.md");
    let ingested = fixture.cbr(
        "owner",
        &[
            "ingest",
            document.to_str().expect("utf-8"),
            "--media-type",
            "text/markdown",
            "--source-kind",
            "human_decision_record",
            "--repo",
            fixture.checkout.to_str().expect("utf-8"),
            "--commit",
            "HEAD",
        ],
    );
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    assert_eq!(
        field(&ingested, "size"),
        DECISION.len().to_string(),
        "the whole document"
    );

    // The owner is the authority for the scope the claim is made in.
    ok(&fixture.cbr(
        "owner",
        &["authority", "bind", "svc", "--authority", "owner"],
    ));

    // A claim that cites the ingested decision, and that is only about the
    // tree it was read at.
    let claim = fixture.write(
        "claim.json",
        &format!(
            r#"{{"plane":"normative",
                 "statement":{{"subject":{{"kind":"app.service","id":"queue"}},
                               "predicate":"keeps_compatibility_adapter",
                               "value":true,"cardinality":"single"}},
                 "scope":{{"id":"svc","qualifiers":{{}}}},
                 "support":[{{"support_id":"s1",
                              "evidence":{{"provider":"cbr",
                                           "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                           "digest":"{digest}"}},
                              "ancestry":{{"completeness":"complete",
                                           "roots":[{{"kind":"evidence",
                                                      "provider":"cbr",
                                                      "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                                      "digest":"{digest}"}}]}}}}],
                 "derivation":{{"kind":"human","inputs":[]}},
                 "conditions":[{{"condition_id":"at-tree","kind":"repository_tree",
                                 "repository":"app","expected":"{tree}"}}]}}"#
        ),
    );
    let proposed = ok(&fixture.cbr("owner", &["propose", "adapter", "--content", &claim]));
    assert_eq!(
        text(&proposed, &["reference", "claim"]),
        "adapter",
        "{proposed:?}"
    );
    assert_eq!(
        at(&proposed, &["reference", "revision"]),
        Value::Int(1),
        "{proposed:?}"
    );

    ok(&fixture.cbr(
        "owner",
        &[
            "decide",
            "d1",
            "--claim",
            "adapter",
            "--revision",
            "1",
            "--decision",
            "accepted_for_use",
            "--use",
            "binding",
            "--rationale",
            "the decision record says so and is cited",
        ],
    ));

    let inspected = ok(&fixture.cbr("owner", &["inspect", "adapter"]));
    assert_eq!(
        text(&inspected, &["reliance", "state"]),
        "accepted_for_use",
        "{inspected:?}"
    );
    assert_eq!(
        text(&inspected, &["reliance", "permitted_use"]),
        "binding",
        "{inspected:?}"
    );
    assert_eq!(
        text(&inspected, &["support", "class"]),
        "single_lineage",
        "one captured root, cited once: {inspected:?}"
    );
    assert_eq!(
        text(&inspected, &["availability", "state"]),
        "complete",
        "the cited evidence is still there: {inspected:?}"
    );
    assert_eq!(
        first(&at(&inspected, &["record"]), "conditions", "expected"),
        tree,
        "the claim is about the tree it was made at: {inspected:?}"
    );

    // The cited bytes are still exactly the document, fetched by digest.
    let out = fixture.directory.path().join("fetched.md");
    let fetched = fixture.cbr(
        "owner",
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(
        fetched.status.success(),
        "cbr fetch failed: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&out).expect("reads"),
        DECISION,
        "a citation resolves to the bytes that were ingested"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
    let mut stderr = String::new();
    if let Some(pipe) = provider.stderr.take() {
        BufReader::new(pipe).read_to_string(&mut stderr).ok();
    }
    assert!(
        !stderr.contains(fixture.checkout.to_str().expect("utf-8")),
        "a registered checkout's path is not logged: {stderr}"
    );
}

#[test]
fn a_registration_that_cannot_be_read_refuses_the_launch() {
    let fixture = Fixture::new();
    let missing = fixture.directory.path().join("no-such-checkout");
    let output = Command::new(provider_binary())
        .arg("--data-dir")
        .arg(fixture.data())
        .arg("--config")
        .arg(fixture.directory.path().join("cbr.json"))
        .arg("--socket")
        .arg(&fixture.socket)
        .arg("--register-repository")
        .arg(format!("app={}", missing.display()))
        .stdin(Stdio::null())
        .output()
        .expect("provider runs");
    assert!(!output.status.success(), "the launch must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a readable git checkout"),
        "and say why: {stderr}"
    );
    assert!(
        !fixture.socket.exists(),
        "a refused launch listens on nothing"
    );
}
