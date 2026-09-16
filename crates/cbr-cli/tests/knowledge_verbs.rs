//! The `cbr` knowledge verbs against a production provider over its socket.
//!
//! A standalone human principal (`owner`) binds a scope to itself and decides.
//! A model-labelled producer (`model`) holds its own credential and a grant to
//! propose, read and even `knowledge.decide`, and labels its derivation
//! `human`. It still cannot accept its own claim: only the bound authority
//! decides, and neither a grant nor a label makes a principal that.
//!
//! No model is run. `model` is a principal name, and its claim is a file this
//! test writes: a labelled stand-in for a model producer, not model evidence.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

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
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {arguments:?}");
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

    fn start(&self) -> Child {
        let child = Command::new(provider_binary())
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(self.directory.path().join("cbr.json"))
            .arg("--socket")
            .arg(&self.socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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

/// Issue a grant as `owner` with a raw protocol session over the same socket.
/// There is no `cbr grant` verb; this is test setup through the public
/// protocol, not a shortcut into the store.
fn issue_grant(fixture: &Fixture, id: &str, holder: &str) {
    let credential = std::fs::read_to_string(fixture.credential("owner")).expect("credential");
    let stream = UnixStream::connect(&fixture.socket).expect("connects");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    let mut call = |frame: String| {
        writeln!(writer, "{frame}").expect("writes");
        let mut line = String::new();
        reader.read_line(&mut line).expect("reads");
        assert!(line.contains("\"result\""), "{line}");
    };
    call(format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"core.authenticate","params":{{"operation":"core.authenticate","message_id":"a","payload":{{"credential":"{}"}}}}}}"#,
        credential.trim_end()
    ));
    call(r#"{"jsonrpc":"2.0","id":2,"method":"core.negotiate","params":{"operation":"core.negotiate","message_id":"n","payload":{"caller":{"name":"t","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.grants"],"optional_features":[]}]}}}"#.to_string());
    let envelope = format!(
        r#"{{"operation":"core.grant.issue","message_id":"g","command_id":"grant-{id}","dedupe_generation":1,"subject":{{"kind":"core.grant","id":"{id}"}},"preconditions":[{{"subject":{{"kind":"core.grant","id":"{id}"}},"revision":0}}],"requires":[],"payload":{{"holder":"{holder}","audience":"cbr","rights":["knowledge.propose","knowledge.read","knowledge.decide"],"resources":[{{"kind":"knowledge.claim"}},{{"kind":"knowledge.decision"}},{{"kind":"knowledge.evaluation"}},{{"kind":"knowledge.authority"}}],"delegation":{{"allowed":false,"max_depth":0}}}}}}"#
    );
    let digest =
        cbr_encoding::command_digest(&cbr_encoding::parse(envelope.as_bytes()).unwrap()).unwrap();
    let envelope = envelope.replacen(
        r#""payload""#,
        &format!(r#""command_digest":"{digest}","payload""#),
        1,
    );
    call(format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"core.grant.issue","params":{envelope}}}"#
    ));
}

fn claim_content(value: &str, derivation: &str, tree: Option<&str>) -> String {
    let conditions = tree.map_or(String::new(), |t| {
        format!(
            r#","conditions":[{{"condition_id":"at-tree","kind":"repository_tree","repository":"app","expected":"{t}"}}]"#
        )
    });
    format!(
        r#"{{"plane":"normative","statement":{{"subject":{{"kind":"app.service","id":"billing"}},"predicate":"retry_limit","value":{value},"cardinality":"single"}},"scope":{{"id":"svc","qualifiers":{{}}}},"support":[],"derivation":{{"kind":"{derivation}","inputs":[]}}{conditions}}}"#
    )
}

#[test]
fn a_model_labelled_producer_cannot_accept_its_own_claim_and_the_owner_decides() {
    let fixture = Fixture::new();
    let provider = fixture.start();
    let issued = Command::new(provider_binary())
        .arg("--data-dir")
        .arg(fixture.data())
        .arg("--config")
        .arg(fixture.directory.path().join("cbr.json"))
        .args(["--issue-credential", "model"])
        .output()
        .expect("administration runs");
    assert!(issued.status.success());
    issue_grant(&fixture, "g-model", "model");

    let bound = ok(&fixture.cbr(
        "owner",
        &["authority", "bind", "svc", "--authority", "owner"],
    ));
    assert_eq!(at(&bound, &["epoch"]), Value::Int(1));

    // The model proposes, labelling its derivation `human`.
    let content = fixture.write("model-claim.json", &claim_content("5", "human", None));
    let proposed = ok(&fixture.cbr(
        "model",
        &[
            "propose",
            "retry",
            "--content",
            &content,
            "--grant",
            "g-model",
        ],
    ));
    assert_eq!(text(&proposed, &["reference", "claim"]), "retry");

    // And tries to accept it, holding knowledge.decide.
    let refused = fixture.cbr(
        "model",
        &[
            "decide",
            "self-accept",
            "--claim",
            "retry",
            "--decision",
            "accepted_for_use",
            "--use",
            "binding",
            "--rationale",
            "mine",
            "--grant",
            "g-model",
        ],
    );
    assert!(
        !refused.status.success(),
        "the model must not accept its own claim"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("not_authority"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let inspected = ok(&fixture.cbr("owner", &["inspect", "retry"]));
    assert_eq!(text(&inspected, &["reliance", "state"]), "proposed");
    assert_eq!(text(&inspected, &["record", "producer"]), "model");

    // The bound authority decides, and it is a review, not an adoption.
    let accepted = ok(&fixture.cbr(
        "owner",
        &[
            "decide",
            "review",
            "--claim",
            "retry",
            "--decision",
            "accepted_for_use",
            "--use",
            "hypothesis",
            "--rationale",
            "reviewed",
        ],
    ));
    assert_eq!(at(&accepted, &["author_is_decider"]), Value::Bool(false));
    assert_eq!(at(&accepted, &["epoch"]), Value::Int(1));

    // A human requirement stated and adopted by its author is recorded as
    // exactly that.
    let own = fixture.write("own-claim.json", &claim_content("3", "human", None));
    ok(&fixture.cbr("owner", &["propose", "limit", "--content", &own]));
    let adopted = ok(&fixture.cbr(
        "owner",
        &[
            "decide",
            "adopt",
            "--claim",
            "limit",
            "--decision",
            "accepted_for_use",
            "--use",
            "binding",
            "--rationale",
            "my requirement",
        ],
    ));
    assert_eq!(at(&adopted, &["author_is_decider"]), Value::Bool(true));

    // A later decision about the same revision names the one it replaces,
    // which `decide` reads through the protocol.
    let superseded = ok(&fixture.cbr(
        "owner",
        &[
            "decide",
            "withdraw",
            "--claim",
            "limit",
            "--decision",
            "rejected",
            "--rationale",
            "withdrawn",
        ],
    ));
    assert_eq!(text(&superseded, &["value"]), "rejected");

    let history = ok(&fixture.cbr("owner", &["history", "retry"]));
    let decisions = at(&history, &["decisions"]);
    let decisions = decisions.as_array().expect("decisions");
    assert_eq!(decisions.len(), 1, "the refused decision left no record");
    assert_eq!(text(&decisions[0], &["decider"]), "owner");
    let limit = ok(&fixture.cbr("owner", &["history", "limit"]));
    let limit_decisions = at(&limit, &["decisions"]);
    assert_eq!(
        text(
            &limit_decisions.as_array().unwrap()[1],
            &["supersedes_decision"]
        ),
        "adopt"
    );

    let mut provider = provider;
    provider.kill().expect("SIGKILL");
    provider.wait().expect("reaped");
}

#[test]
fn revise_evaluate_and_history_follow_a_real_repository() {
    let fixture = Fixture::new();
    let provider = fixture.start();
    let repository = fixture.directory.path().join("app");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q", "-b", "main"]);
    std::fs::write(repository.join("retry.conf"), "limit = 3\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-q", "-m", "first"]);
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let repo = repository.to_str().unwrap();

    ok(&fixture.cbr(
        "owner",
        &["authority", "bind", "svc", "--authority", "owner"],
    ));
    let first = fixture.write("v1.json", &claim_content("3", "deterministic", Some(&tree)));
    ok(&fixture.cbr("owner", &["propose", "limit", "--content", &first]));
    let second = fixture.write("v2.json", &claim_content("4", "deterministic", Some(&tree)));
    let revised = ok(&fixture.cbr("owner", &["revise", "limit", "--content", &second]));
    assert_eq!(at(&revised, &["reference", "revision"]), Value::Int(2));

    // The target's identity is computed from the repository by `cbr`.
    let applicable = ok(&fixture.cbr(
        "owner",
        &[
            "evaluate",
            "at-first",
            "--claim",
            "limit",
            "--repo",
            repo,
            "--repo-id",
            "app",
        ],
    ));
    assert_eq!(text(&applicable, &["result"]), "applicable");

    std::fs::write(repository.join("retry.conf"), "limit = 4\n").unwrap();
    git(&repository, &["commit", "-q", "-am", "second"]);
    let moved = ok(&fixture.cbr(
        "owner",
        &[
            "evaluate",
            "at-second",
            "--claim",
            "limit",
            "--repo",
            repo,
            "--repo-id",
            "app",
        ],
    ));
    assert_eq!(
        text(&moved, &["result"]),
        "invalid_for_target",
        "the tree moved, so the condition no longer matches"
    );

    let history = ok(&fixture.cbr("owner", &["history", "limit"]));
    let revisions = at(&history, &["revisions"]);
    let revisions = revisions.as_array().unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(
        at(&revisions[1], &["supersedes", "revision"]),
        Value::Int(1)
    );
    let evaluations = at(&history, &["evaluations"]);
    assert_eq!(evaluations.as_array().unwrap().len(), 2);
    // Revision 1 stays inspectable with its own record.
    let old = ok(&fixture.cbr("owner", &["inspect", "limit", "--revision", "1"]));
    assert_eq!(at(&old, &["record", "statement", "value"]), Value::Int(3));
    assert_eq!(at(&old, &["current_revision"]), Value::Int(2));

    let mut provider = provider;
    provider.kill().expect("SIGKILL");
    provider.wait().expect("reaped");
}
