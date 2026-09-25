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
        // Enough discoverable material that the compiler's own bounds bite.
        // With only the two files above, a fixture packet carries fewer
        // spans than `DISCOVERED_SPANS` allows and no excerpt reaches
        // `EXCERPT_BYTES`, so a change to either bound leaves the packet
        // byte-identical and the golden digest below proves nothing about
        // them. Six files, because `DISCOVERED_PER_PATH` caps a single file
        // at two spans however long it is.
        for (rank, module) in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"]
            .into_iter()
            .enumerate()
        {
            // Each module mentions the task's words a different number of
            // times, so retrieval has something to rank by and the order of
            // the discovered spans is not a tie broken by whatever the query
            // plan happened to do.
            let mut source = format!(
                "//! {}\n\n",
                "The compatibility adapter rewrites the queue. ".repeat(rank + 1)
            );
            for stage in 0..6 {
                source.push_str(&format!(
                    "/// Stage {stage} of the compatibility adapter: it rewrites the queue's \
                     response field on the way out, so that a client fixture written against \
                     the old adapter keeps working while the queue migration lands.\n\
                     /// The adapter is deliberately dull. It drains the queue, renames the \
                     response field, and hands the result back to the caller unchanged in \
                     every other respect, which is what the decision record says it must do.\n\
                     pub fn {module}AdapterStage{stage}(queue: u64) -> u64 {{\n    \
                     queue + {stage}\n}}\n\n"
                ));
            }
            std::fs::write(checkout.join(format!("src/{module}.rs")), source).expect("writes");
        }
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

/// Issue a grant as `owner` over a raw protocol session. There is no
/// `cbr grant` verb; this is test setup through the public protocol, not a
/// shortcut into the store.
fn issue_grant(fixture: &Fixture, id: &str, holder: &str, repository: &str) {
    use std::io::{BufRead, Write};
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
        r#"{{"operation":"core.grant.issue","message_id":"g","command_id":"grant-{id}","dedupe_generation":1,"subject":{{"kind":"core.grant","id":"{id}"}},"preconditions":[{{"subject":{{"kind":"core.grant","id":"{id}"}},"revision":0}}],"requires":[],"payload":{{"holder":"{holder}","audience":"cbr","rights":["context.request","context.read","context.packet.read"],"resources":[{{"kind":"context.request"}},{{"kind":"context.job"}},{{"kind":"context.packet"}},{{"kind":"cbr.repository","id":"{repository}"}}],"delegation":{{"allowed":false,"max_depth":0}}}}}}"#
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

/// The access rule, at the compiler rather than at the index: a principal
/// whose grant covers one repository sees that one and nothing of the other,
/// even when its request names both.
#[test]
fn a_request_naming_a_repository_outside_the_grant_is_answered_only_from_the_view() {
    let fixture = Fixture::new();
    let closed = fixture.directory.path().join("closed");
    std::fs::create_dir_all(closed.join("src")).expect("src");
    std::fs::write(
        closed.join("src/secret.rs"),
        "pub fn embargoedThing() -> u8 {\n    7\n}\n",
    )
    .expect("writes");
    git(&closed, &["init", "-q", "-b", "main"]);
    git(&closed, &["add", "."]);
    git(&closed, &["commit", "-q", "-m", "embargoed"]);

    let mut provider = fixture.start(&[
        format!("app={}", fixture.checkout.display()),
        format!("closed={}", closed.display()),
    ]);

    // A second principal, with a grant over `app` only.
    let issued = Command::new(provider_binary())
        .arg("--data-dir")
        .arg(fixture.data())
        .arg("--config")
        .arg(fixture.directory.path().join("cbr.json"))
        .arg("--issue-credential")
        .arg("reader")
        .output()
        .expect("issues");
    assert!(issued.status.success(), "{:?}", issued);
    issue_grant(&fixture, "g-reader", "reader", "app");

    // The request names the repository the grant does not cover.
    let submitted = Command::new(env!("CARGO_BIN_EXE_cbr"))
        .args([
            "context",
            "scoped",
            "--repo",
            closed.to_str().expect("utf-8"),
            "--repo-id",
            "closed",
            "--want",
            "code=source:src/secret.rs",
            "--selector",
            "embargoed thing",
            "--capacity",
            "65536",
            "--grant",
            "g-reader",
        ])
        .arg("--socket")
        .arg(&fixture.socket)
        .arg("--credential-file")
        .arg(fixture.credential("reader"))
        .output()
        .expect("cbr runs");
    assert!(
        submitted.status.success(),
        "the submit itself is allowed: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );

    let inspected = Command::new(env!("CARGO_BIN_EXE_cbr"))
        .args(["request", "scoped", "--grant", "g-reader"])
        .arg("--socket")
        .arg(&fixture.socket)
        .arg("--credential-file")
        .arg(fixture.credential("reader"))
        .output()
        .expect("cbr runs");
    let inspected = ok(&inspected);
    let items = at(&inspected, &["items"])
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default();
    assert_eq!(items.len(), 1, "{inspected:?}");
    assert_eq!(text(&items[0], &["result"]), "unmet", "{inspected:?}");
    assert_eq!(
        text(&items[0], &["reason"]),
        "source_unavailable",
        "a repository outside the view answers exactly as one that is not \
         registered: {inspected:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

/// A projection that cannot be built says so. The basis here names a
/// repository at a tree that repository does not have — a real thing to ask
/// for when two checkouts are confused, and the answer has to be "this
/// projection is unavailable", never a packet that looks like it searched.
#[test]
fn a_projection_that_cannot_be_built_is_reported_unavailable_rather_than_empty() {
    let fixture = Fixture::new();
    let other = fixture.directory.path().join("other");
    std::fs::create_dir_all(&other).expect("dir");
    std::fs::write(other.join("README.md"), "another repository\n").expect("writes");
    git(&other, &["init", "-q", "-b", "main"]);
    git(&other, &["add", "."]);
    git(&other, &["commit", "-q", "-m", "other"]);

    let mut provider = fixture.start(&[
        format!("app={}", fixture.checkout.display()),
        format!("other={}", other.display()),
    ]);

    // The basis is resolved from `app`'s checkout but labelled `other`, so
    // the tree it names is not in the repository the provider will read.
    let submitted = fixture.cbr(
        "owner",
        &[
            "context",
            "confused",
            "--repo",
            fixture.checkout.to_str().expect("utf-8"),
            "--repo-id",
            "other",
            "--want",
            "doc=source:docs/decisions/0001-adapter.md",
            "--selector",
            "compatibility adapter",
            "--capacity",
            "65536",
        ],
    );
    assert!(
        submitted.status.success(),
        "{}",
        String::from_utf8_lossy(&submitted.stderr)
    );

    // As above: preparation takes ticks, and each `request` is one.
    let started = std::time::Instant::now();
    let inspected = loop {
        let inspected = ok(&fixture.cbr("owner", &["request", "confused"]));
        if !at(&inspected, &["packets"])
            .as_array()
            .map(<[Value]>::is_empty)
            .unwrap_or(true)
        {
            break inspected;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(120),
            "no packet after two minutes: {inspected:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let items = at(&inspected, &["items"])
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default();
    assert_eq!(text(&items[0], &["result"]), "unmet", "{inspected:?}");

    let packet = ok(&fixture.cbr("owner", &["packet", "confused"]));
    let coverage = at(&packet, &["coverage"])
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default();
    assert_eq!(coverage.len(), 1, "{packet:?}");
    let gaps: Vec<String> = at(&coverage[0], &["gaps"])
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default()
        .iter()
        .map(|gap| gap.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        gaps.iter().any(|gap| gap == "projection unavailable"),
        "the coverage says the projection is unavailable: {gaps:?}"
    );
    assert!(
        gaps.iter().any(|gap| gap.contains("could not be built")),
        "and why: {gaps:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

/// One run of the same request in its own store, returning the packet's
/// exact bytes.
///
/// Two runs differ only in what the test sets up, and the packet body
/// carries no instant and no store-local identity — the request id, the
/// tree and the content-addressed artifact ids are all the same — so the
/// bytes are comparable byte for byte. That is the strongest statement of
/// "this principal learned nothing about it": not that the claim was
/// labelled carefully, but that the answer is the answer they would have
/// got had the claim never existed.
fn packet_bytes_for(
    setup: impl FnOnce(&Fixture, &str, &str),
    register_second: bool,
    as_owner: bool,
) -> Vec<u8> {
    let fixture = Fixture::new();
    let second = fixture.directory.path().join("other");
    std::fs::create_dir_all(second.join("src")).expect("dir");
    std::fs::write(
        second.join("src/embargoed.rs"),
        "pub fn embargoedAdapterThing() -> u8 {\n    7\n}\n",
    )
    .expect("writes");
    git(&second, &["init", "-q", "-b", "main"]);
    git(&second, &["add", "."]);
    git(&second, &["commit", "-q", "-m", "embargoed"]);

    let mut registrations = vec![format!("app={}", fixture.checkout.display())];
    if register_second {
        registrations.push(format!("closed={}", second.display()));
    }
    let mut provider = fixture.start(&registrations);

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

    let ingested = fixture.cbr(
        "owner",
        &[
            "ingest",
            fixture
                .checkout
                .join("docs/decisions/0001-adapter.md")
                .to_str()
                .expect("utf-8"),
            "--media-type",
            "text/markdown",
            "--source-kind",
            "human_decision_record",
            "--repo",
            fixture.checkout.to_str().expect("utf-8"),
        ],
    );
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    setup(&fixture, &artifact, &digest);
    let _ = tree;

    // A reader whose grant covers `app` and the context subjects, and
    // nothing of knowledge and nothing of `closed`.
    let issued = Command::new(provider_binary())
        .arg("--data-dir")
        .arg(fixture.data())
        .arg("--config")
        .arg(fixture.directory.path().join("cbr.json"))
        .arg("--issue-credential")
        .arg("reader")
        .output()
        .expect("issues");
    assert!(issued.status.success());
    issue_grant(&fixture, "g-reader", "reader", "app");

    // The basis names both repositories whether or not the second is
    // registered and whether or not the grant covers it. Without that the
    // compiler never reaches the view check at all, because it walks the
    // basis: an earlier version of this test registered a repository the
    // basis never named and proved nothing.
    let principal = if as_owner { "owner" } else { "reader" };
    let mut arguments: Vec<String> = [
        "context",
        "probe",
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
        "--repo-id",
        "app",
        "--also",
    ]
    .iter()
    .map(|argument| (*argument).to_string())
    .collect();
    arguments.push(format!("closed={}", second.display()));
    arguments.extend(
        [
            "--want",
            "code=source:src/queue.rs",
            "--selector",
            "adapter",
            "--task",
            "what does the compatibility adapter do to the queue",
            "--capacity",
            "65536",
        ]
        .iter()
        .map(|argument| (*argument).to_string()),
    );
    if !as_owner {
        arguments.extend(["--grant".to_string(), "g-reader".to_string()]);
    }
    let submitted = Command::new(env!("CARGO_BIN_EXE_cbr"))
        .args(&arguments)
        .arg("--socket")
        .arg(&fixture.socket)
        .arg("--credential-file")
        .arg(fixture.credential(principal))
        .output()
        .expect("cbr runs");
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );

    let read = |arguments: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cbr"));
        command.args(arguments);
        if !as_owner {
            command.args(["--grant", "g-reader"]);
        }
        command
            .arg("--socket")
            .arg(&fixture.socket)
            .arg("--credential-file")
            .arg(fixture.credential(principal))
            .output()
            .expect("cbr runs")
    };
    // **Preparation takes ticks now.** The index build left the
    // preparation tick in m4c, so a first request waits for it rather than
    // having it done inline — which is the point: every other job on the
    // provider carries on meanwhile. Each `request` call is a poll, and
    // each poll is a tick.
    let started = std::time::Instant::now();
    loop {
        let inspected = ok(&read(&["request", "probe"]));
        if !at(&inspected, &["packets"])
            .as_array()
            .map(<[Value]>::is_empty)
            .unwrap_or(true)
        {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(120),
            "no packet after two minutes: {inspected:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let printed = ok(&read(&["packet", "probe", "--excerpt", "1000000"]));
    let data = text(&printed, &["excerpt", "data_base64"]);
    let bytes = cbr_encoding::decode_base64(&data).expect("base64");

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
    bytes
}

/// Propose a claim about `app` and have the owner accept it as binding.
/// Six claims, all eligible, differing only in how much of the request's
/// question they share. The golden packet is built over these so that
/// `compiler::CARRIED_CLAIMS` is inside the digest: with fewer eligible
/// claims than the cap, changing the cap would change nothing and the
/// guard would be proving something it does not test.
fn accept_six_claims(fixture: &Fixture, artifact: &str, digest: &str) {
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
    ok(&fixture.cbr(
        "owner",
        &["authority", "bind", "app", "--authority", "owner"],
    ));
    for (claim, value) in [
        (
            "c-adapter",
            "the compatibility adapter stays for one release",
        ),
        ("c-queue", "the queue keeps its ordering guarantee"),
        ("c-naming", "a module is named after what it does"),
        ("c-licence", "the repository is MIT"),
        ("c-review", "a change is reviewed before it lands"),
        ("c-release", "a release is tagged by the owner"),
    ] {
        let content = fixture.write(
            &format!("{claim}.json"),
            &format!(
                r#"{{"plane":"normative",
                     "statement":{{"subject":{{"kind":"app.decision","id":"{claim}"}},
                                   "predicate":"decides","value":"{value}",
                                   "cardinality":"single"}},
                     "scope":{{"id":"app","qualifiers":{{}}}},
                     "support":[{{"support_id":"s1",
                                  "evidence":{{"provider":"cbr",
                                               "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                               "digest":"{digest}"}},
                                  "ancestry":{{"completeness":"complete",
                                               "roots":[{{"kind":"evidence","provider":"cbr",
                                                          "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                                          "digest":"{digest}"}}]}}}}],
                     "derivation":{{"kind":"human","inputs":[]}},
                     "conditions":[{{"condition_id":"at-tree","kind":"repository_tree",
                                     "repository":"app","expected":"{tree}"}}]}}"#
            ),
        );
        ok(&fixture.cbr("owner", &["propose", claim, "--content", &content]));
        ok(&fixture.cbr(
            "owner",
            &[
                "decide",
                &format!("d-{claim}"),
                "--claim",
                claim,
                "--revision",
                "1",
                "--decision",
                "accepted_for_use",
                "--use",
                "binding",
                "--rationale",
                "the owner decided it",
            ],
        ));
    }
}

fn accept_a_claim(fixture: &Fixture, artifact: &str, digest: &str) {
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
    ok(&fixture.cbr(
        "owner",
        &["authority", "bind", "svc", "--authority", "owner"],
    ));
    let claim = fixture.write(
        "secret.json",
        &format!(
            r#"{{"plane":"normative",
                 "statement":{{"subject":{{"kind":"app.service","id":"queue"}},
                               "predicate":"keeps_compatibility_adapter","value":true,
                               "cardinality":"single"}},
                 "scope":{{"id":"svc","qualifiers":{{}}}},
                 "support":[{{"support_id":"s1",
                              "evidence":{{"provider":"cbr",
                                           "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                           "digest":"{digest}"}},
                              "ancestry":{{"completeness":"complete",
                                           "roots":[{{"kind":"evidence","provider":"cbr",
                                                      "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                                      "digest":"{digest}"}}]}}}}],
                 "derivation":{{"kind":"human","inputs":[]}},
                 "conditions":[{{"condition_id":"at-tree","kind":"repository_tree",
                                 "repository":"app","expected":"{tree}"}}]}}"#
        ),
    );
    ok(&fixture.cbr("owner", &["propose", "adapter", "--content", &claim]));
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
            "the decision record says so",
        ],
    ));
}

#[test]
fn a_claim_the_grant_does_not_cover_is_absent_from_the_packet_byte_for_byte() {
    // The leak this closes: discovery walked every claim in the store and
    // called `knowledge_inspect`, which authorizes nothing because its
    // authorization is step 6 of the command that normally calls it. A
    // reader with no knowledge right got a binding section carrying another
    // principal's claim id, state and full statement.
    //
    // The assertion is not that the section was labelled carefully. It is
    // that the packet is the packet they would have received had the claim
    // never been proposed.
    let with_claim = packet_bytes_for(accept_a_claim, false, false);
    let without_claim = packet_bytes_for(|_, _, _| {}, false, false);
    assert_eq!(
        String::from_utf8_lossy(&with_claim),
        String::from_utf8_lossy(&without_claim),
        "a claim outside the grant is identical to a claim that never existed"
    );
}

#[test]
fn a_repository_outside_the_grant_contributes_nothing_to_discovery() {
    // The same statement for repositories: no discovered span, no anchor
    // section, no coverage entry — the packet a reader gets is the packet
    // they would have got had the repository never been registered.
    let with_second = packet_bytes_for(|_, _, _| {}, true, false);
    let without_second = packet_bytes_for(|_, _, _| {}, false, false);
    assert_eq!(
        String::from_utf8_lossy(&with_second),
        String::from_utf8_lossy(&without_second),
        "a repository outside the grant is identical to one never registered"
    );
    assert!(
        !String::from_utf8_lossy(&with_second).contains("embargoed"),
        "and nothing of it appears at all"
    );

    // And the mechanism is not merely absent: the authority principal, whose
    // view is every registration, gets a coverage entry for the same
    // repository from the same basis. Without this arm the assertion above
    // would be satisfied by a compiler that never looked at either.
    let as_owner = packet_bytes_for(|_, _, _| {}, true, true);
    let seen = String::from_utf8_lossy(&as_owner);
    assert!(
        seen.contains("over closed"),
        "the owner's packet covers the second repository: {seen}"
    );
    assert!(
        !String::from_utf8_lossy(&with_second).contains("over closed"),
        "and the reader's does not"
    );
}

/// The digest of the packet the fixture above produces, under the packet
/// compiler this repository builds.
///
/// This is the same shape as `cbr_memory::retrieval::GOLDEN_FIXTURE_DIGEST`,
/// and it exists for the same reason: `cbr-provider`'s `compiler::COMPILER`
/// is a version string a human has to remember to bump, and in this milestone
/// a version string was already forgotten once.
///
/// **Why the constant is here and not beside `compiler::COMPILER`.**
/// `cbr-provider` is a binary-only crate with no library target, so nothing
/// outside it can import that constant, and every test of the compiler drives
/// the built binary. The digest still guards the version, because the
/// compiler string is *inside* the bytes it covers: a sealed packet's
/// coverage names its producer. Bump `compiler::COMPILER` and this digest
/// changes; change what the compiler emits and it changes too. Either way the
/// two are updated in the same commit or this test fails.
///
/// **Re-pinned at `cbr-context-compiler/4`**, which keeps packet ids inside
/// the identifier grammar. Its bytes differ from `/3`'s only in the sixteen
/// span section and citation ids and the compiler string: writing
/// `d-span-app:src:alpha.rs-0` back as `d-span-src/alpha.rs-0` (and `dc-`
/// likewise) and `/4` back as `/3` reproduces `/3`'s digest,
/// `sha256:c54aa006…8346`, exactly — so nothing was selected, ordered or
/// dropped differently. The fixture has no anchor section.
const GOLDEN_PACKET_DIGEST: &str =
    "sha256:52fc69c7af75d681ed522acf72df7813a9ebfb05c55a55b9047ecbbc2197568e";

#[test]
fn the_packet_a_fixed_fixture_produces_has_not_changed_without_the_compiler_string() {
    let bytes = packet_bytes_for(accept_six_claims, false, true);
    // **An ingested artifact's id carries the instant it was ingested**
    // (`ingest.<digest-prefix>.<nanoseconds>`), so two stores that ingest
    // identical bytes mint different ids — and a claim citing one inherits
    // that through its own revision digest, which is taken over a record
    // holding the id. Two runs of this fixture are two stores, so neither
    // the id nor any claim digest agrees between them.
    //
    // **That is not something the compiler decides**, and it was found by
    // this guard rather than assumed: the first version of it pinned a
    // digest that changed on every run. It is normalised out here, narrowly
    // and visibly, and reported as a finding rather than fixed in this
    // pull request. What the guard still covers is every decision the
    // compiler makes — which claims are carried and in what order, which
    // spans, how wide, the coverage, and the compiler string itself.
    let bytes = stable(&bytes);
    let digest = cbr_encoding::digest_bytes(&bytes);
    assert_eq!(
        digest,
        GOLDEN_PACKET_DIGEST,
        "\n\nThe packet this fixture produces has changed.\n\n\
         If the change is intended, it is a change to what the context \
         compiler emits, so `cbr-provider`'s `compiler::COMPILER` must be \
         bumped in the same commit and this digest updated with it. If it is \
         not intended, this is the bug.\n\nWhat the compiler now produces:\n\n{}",
        String::from_utf8_lossy(&bytes)
    );
    // The guard is worth nothing if the bytes do not carry the version it
    // claims to guard, and worth little if the fixture never reaches the
    // bounds the compiler applies: a fixture too small to fill
    // `DISCOVERED_SPANS` or to exceed `EXCERPT_BYTES` leaves both of those
    // free to change without moving the digest. Both were checked by
    // mutating them and watching this test.
    let sealed = cbr_encoding::parse(&bytes).expect("canonical JSON");
    let sections = sealed
        .get("sections")
        .and_then(Value::as_array)
        .expect("sections");
    let discovered = sections
        .iter()
        .filter(|section| text(section, &["section_id"]).starts_with("d-span-"))
        .count();
    assert_eq!(discovered, 8, "the fixture fills DISCOVERED_SPANS");
    let claims = sections
        .iter()
        .filter(|section| text(section, &["section_id"]).starts_with("d-claim-"))
        .count();
    assert_eq!(
        claims, 4,
        "and carries exactly CARRIED_CLAIMS of six eligible"
    );
    assert!(
        sections
            .iter()
            .any(|section| text(section, &["content"]).len() > 1024),
        "and at least one excerpt is longer than half of EXCERPT_BYTES"
    );
    assert!(
        String::from_utf8_lossy(&bytes).contains("cbr-context-compiler/"),
        "the sealed packet names its producer"
    );
    // The normalisation has to have fired, or this guard is over bytes that
    // happened to agree — and a second, independent run has to agree with
    // the first, or the digest is pinning one run's luck.
    assert!(
        String::from_utf8_lossy(&bytes).contains(".<instant>")
            && String::from_utf8_lossy(&bytes).contains("<claim revision>"),
        "both normalisations fired, so neither is silently a no-op"
    );
    let again = stable(&packet_bytes_for(accept_six_claims, false, true));
    assert_eq!(
        cbr_encoding::digest_bytes(&again),
        digest,
        "two independent runs of the same fixture compile the same packet"
    );
}

/// The packet with the two things an ingest instant reaches normalised
/// out: the artifact id that carries it, and the claim revision digests
/// taken over records that hold that id.
///
/// Deliberately narrow. A `digest` is rewritten only inside an object that
/// also names a `claim`, and an id only where it starts `ingest.` and ends
/// in digits, so anything else that differed between two runs still moves
/// the result.
fn stable(bytes: &[u8]) -> Vec<u8> {
    fn walk(value: &Value) -> Value {
        match value {
            Value::Object(members) => {
                let names_a_claim = members
                    .iter()
                    .any(|(name, value)| name == "claim" && value.as_str().is_some());
                Value::Object(
                    members
                        .iter()
                        .map(|(name, member)| {
                            if names_a_claim && name == "digest" {
                                return (name.clone(), Value::String("<claim revision>".into()));
                            }
                            (name.clone(), walk(member))
                        })
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(walk).collect()),
            Value::String(text) => Value::String(normalise_ids(text)),
            other => other.clone(),
        }
    }
    let parsed = cbr_encoding::parse(bytes).expect("the sealed packet is canonical JSON");
    cbr_encoding::to_canonical(&walk(&parsed))
}

/// Replace the nanosecond suffix of every `ingest.<prefix>.<nanos>` that
/// occurs in `text`, wherever in the string it appears — an id occurs both
/// as a value of its own and inside a section's prose.
fn normalise_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("ingest.") {
        let (before, tail) = rest.split_at(start);
        out.push_str(before);
        let id: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
            .collect();
        match id.rsplit_once('.') {
            Some((head, instant))
                if !instant.is_empty() && instant.chars().all(|c| c.is_ascii_digit()) =>
            {
                out.push_str(head);
                out.push_str(".<instant>");
            }
            _ => out.push_str(&id),
        }
        rest = &tail[id.len()..];
    }
    out.push_str(rest);
    out
}
