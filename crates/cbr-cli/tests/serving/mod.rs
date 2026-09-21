//! The fixture every test of the serving model call site shares: a
//! provider over a checkout it writes, with the `model.fake` transport and
//! the `cbr` client.
//!
//! **No model is called.** The transport is the `model.fake` control,
//! which carries its own model identity so a launch that needs a model
//! call still needs no credential, and which a production configuration
//! refuses outright.

#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

/// The one principal this fixture has, in the form a conformance launch
/// takes: `ccred1.<principal>.<secret>`.
pub const CREDENTIAL: &str = "ccred1.owner.model-crash-matrix";

/// A second principal, who is **not** an authority and reads exactly
/// what a grant gives it. m4d's readable-set gate has nothing to say
/// until somebody other than the job's own principal asks.
pub const READER: &str = "ccred1.reader.model-derivation-reader";

pub fn provider_binary() -> PathBuf {
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

/// Commit whatever is in the checkout now, so a test can move the tree
/// a request is about.
pub fn commit(repository: &Path, message: &str) {
    git(repository, &["add", "-A"]);
    git(repository, &["commit", "-q", "-m", message]);
}

fn git(repository: &Path, arguments: &[&str]) {
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
}

/// A file with several chunks that all answer the same selector, so the
/// ranked candidates are more than one and **there is something to
/// choose between**. A call that cannot change the answer is not made.
/// Text of the repository the request names but no item asks for.
pub const UNASKED: &str = "A sentence about compaction that no item asked for.\n";

/// Text of a repository this provider can read and no request is about.
pub const OUTSIDE: &str = "The elsewhere repository speaks of a different queue entirely.\n";

pub fn many_candidates() -> String {
    let mut text = String::new();
    for section in 1..=8 {
        text.push_str(&format!("## Section {section}: draining the queue\n\n"));
        for line in 0..18 {
            text.push_str(&format!(
                "The queue drains on shutdown, case {section}.{line}, and the drain is ordered.\n"
            ));
        }
        text.push('\n');
    }
    text
}

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub socket: PathBuf,
    pub checkout: PathBuf,
    pub outside: PathBuf,
    pub barriers: PathBuf,
}

impl Fixture {
    /// A fixture whose preparation pauses at `barrier`, for the crash
    /// matrix. The count is always made, because three of its six rows
    /// are the count's boundaries and a serving call counts only when the
    /// local bound refuses on a counter a tighter figure could satisfy.
    pub fn paused_at(barrier: &str) -> Self {
        Self::build(&[barrier], &["choose:c2"], "always")
    }

    /// A fixture that pauses at **two** barriers, which is how two model
    /// calls are held in flight at once: a barrier pauses only the first
    /// thread to reach it, so one name holds one call. With
    /// `counting: always` the first call stops at the count's boundary
    /// and the second, finding that one used, goes on to the
    /// completion's.
    pub fn paused_at_two() -> Self {
        Self::build(
            // **After the send, not after the reservation.** A call held
            // before it sends has recorded nothing, and a test watching
            // the recordings cannot tell it apart from a call that never
            // started.
            &["model.count.after_send", "model.completion.after_send"],
            &["choose:c1"],
            "always",
        )
    }

    /// A fixture whose fake model answers `answers`, one per completion,
    /// with the counting a serving call site really uses.
    pub fn answering(answers: &[&str]) -> Self {
        Self::build(&[], answers, "when_it_could_admit")
    }

    fn build(barriers_enabled: &[&str], answers: &[&str], counting: &str) -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let sockets = directory.path().join("s");
        std::fs::create_dir(&sockets).expect("socket dir");
        std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
        let barriers = directory.path().join("barriers");
        std::fs::create_dir(&barriers).expect("barrier dir");

        let checkout = directory.path().join("app");
        std::fs::create_dir_all(&checkout).expect("checkout");
        std::fs::write(checkout.join("queue.md"), many_candidates()).expect("writes");
        // **A second file an item can name**, so a request can need two
        // calls. One item per request never exercises a compile holding
        // two answers at once, which is the case the pool's
        // keep-until-taken rule exists for.
        std::fs::write(checkout.join("cache.md"), many_candidates()).expect("writes");
        // A third, for the request that has to wait for the bound.
        std::fs::write(checkout.join("index.md"), many_candidates()).expect("writes");
        std::fs::write(checkout.join("unasked.md"), UNASKED).expect("writes");
        git(&checkout, &["init", "-q", "-b", "main"]);
        git(&checkout, &["add", "-A"]);
        git(&checkout, &["commit", "-q", "-m", "the tree"]);

        // **A second repository, registered and never in a basis.** It is
        // readable by this provider and outside the view of every request
        // below, which is what READINESS §7's gate is about.
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(&outside).expect("checkout");
        std::fs::write(outside.join("elsewhere.md"), OUTSIDE).expect("writes");
        git(&outside, &["init", "-q", "-b", "main"]);
        git(&outside, &["add", "-A"]);
        git(&outside, &["commit", "-q", "-m", "the other tree"]);

        // **`context.compile` is why this is a conformance launch that
        // compiles.** The model call site lives inside compiling, and
        // reaching it needs the fake transport, which only a conformance
        // launch may have — a production one serves no test control at
        // all. No fixture of the conformance suite sets it.
        let enabled = barriers_enabled
            .iter()
            .map(|barrier| format!("\"{barrier}\""))
            .collect::<Vec<_>>()
            .join(",");
        let scripted = answers
            .iter()
            .map(|answer| format!("\"{answer}\""))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            directory.path().join("cbr.json"),
            format!(
                r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{CREDENTIAL}"}},{{"credential":"{READER}"}}],"context":{{"compile":true}},"test_barriers":{{"directory":"{}","enabled":[{enabled}]}},"model":{{"dialect":"responses","model":"MiniMax-M2.7-highspeed","answers":[{scripted}],"usage":5000,"counting":"{counting}"}}}}"#,
                barriers.display()
            ),
        )
        .expect("config");
        // A conformance launch writes no credential file: its principals
        // are given to it, and the runner holds what they present. Here
        // the runner is this test.
        std::fs::write(directory.path().join("credential"), CREDENTIAL).expect("credential");
        std::fs::write(directory.path().join("credential.reader"), READER).expect("credential");

        Fixture {
            socket: sockets.join("cbr.sock"),
            checkout,
            outside,
            barriers,
            directory,
        }
    }

    pub fn data(&self) -> PathBuf {
        self.directory.path().join("data")
    }

    pub fn start(&self) -> Running {
        self.launch(&self.directory.path().join("cbr.json"), &[])
    }

    fn launch(&self, config: &Path, extra: &[&str]) -> Running {
        // A stopped provider leaves its socket file behind, so a second
        // launch over the same directory would find a path that exists
        // and refuses connections. The provider unlinks it itself; this
        // is so the wait below cannot see the old one.
        let _ = std::fs::remove_file(&self.socket);
        let child = Command::new(provider_binary())
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(config)
            .args(extra)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--register-repository")
            .arg(format!("app={}", self.checkout.display()))
            .arg("--register-repository")
            .arg(format!("outside={}", self.outside.display()))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
        Running(child)
    }

    /// Start again over the same data directory as an **offline
    /// rebuild**: no fake transport, no credential, no permit, and a
    /// transport that panics if it is ever reached.
    ///
    /// The configuration is a second file this fixture writes, because
    /// a replay names its model in `model_runtime` — a retained answer
    /// is found by a question that says which model was asked — where a
    /// live fixture names it in the `model.fake` control. A launch has
    /// one transport, and a launch given both is refused.
    pub fn start_replaying(&self) -> Running {
        let config = self.directory.path().join("cbr-replay.json");
        std::fs::write(
            &config,
            format!(
                r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{CREDENTIAL}"}},{{"credential":"{READER}"}}],"context":{{"compile":true}},"model_runtime":{{"provider":"minimax","dialect":"responses","model":"MiniMax-M2.7-highspeed"}}}}"#
            ),
        )
        .expect("config");
        self.launch(&config, &["--replay-model"])
    }

    pub fn cbr(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cbr"))
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(self.directory.path().join("credential"))
            .output()
            .expect("cbr runs")
    }

    /// The same, as the second principal, under the grant it holds.
    pub fn cbr_as_reader(&self, grant: &str, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cbr"))
            .args(arguments)
            .args(["--grant", grant])
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(self.directory.path().join("credential.reader"))
            .output()
            .expect("cbr runs")
    }

    /// Issue a grant to the second principal, over raw frames.
    ///
    /// `cbr` has no grant verb — grants are the authority's act and the
    /// CLI is a consumer's client — so this speaks the protocol the way
    /// `registration_and_decisions` does. The audience is the
    /// conformance launch's provider id, not `cbr`: a grant for another
    /// provider is refused before its rights are read at all.
    pub fn issue_grant(&self, id: &str, rights: &[&str], resources: &str) {
        use std::io::{BufRead, BufReader, Write};
        let stream = UnixStream::connect(&self.socket).expect("connects");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut writer = stream;
        let mut call = |frame: String| {
            writeln!(writer, "{frame}").expect("writes");
            let mut line = String::new();
            reader.read_line(&mut line).expect("reads");
            assert!(line.contains("\"result\""), "{line}");
        };
        call(format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"core.authenticate","params":{{"operation":"core.authenticate","message_id":"a","payload":{{"credential":"{CREDENTIAL}"}}}}}}"#
        ));
        call(
            r#"{"jsonrpc":"2.0","id":2,"method":"core.negotiate","params":{"operation":"core.negotiate","message_id":"n","payload":{"caller":{"name":"t","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.grants"],"optional_features":[]}]}}}"#
                .to_string(),
        );
        let rights = rights
            .iter()
            .map(|right| format!("\"{right}\""))
            .collect::<Vec<_>>()
            .join(",");
        let envelope = format!(
            r#"{{"operation":"core.grant.issue","message_id":"g","command_id":"grant-{id}","dedupe_generation":1,"subject":{{"kind":"core.grant","id":"{id}"}},"preconditions":[{{"subject":{{"kind":"core.grant","id":"{id}"}},"revision":0}}],"requires":[],"payload":{{"holder":"reader","audience":"conformance-provider","rights":[{rights}],"resources":[{resources}],"delegation":{{"allowed":false,"max_depth":0}}}}}}"#
        );
        let digest = cbr_encoding::command_digest(
            &cbr_encoding::parse(envelope.as_bytes()).expect("canonical"),
        )
        .expect("digest");
        let envelope = envelope.replacen(
            r#""payload""#,
            &format!(r#""command_digest":"{digest}","payload""#),
            1,
        );
        call(format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"core.grant.issue","params":{envelope}}}"#
        ));
    }
}

/// A running provider, stopped when the test lets go of it.
///
/// A killed child nobody waits for is a zombie, and a suite that leaves
/// one behind per test leaves a great many. Dropping is also how a crash
/// row kills at its boundary, so the two are the same act.
pub struct Running(Child);

impl Running {
    /// Kill it now, rather than at the end of the scope.
    pub fn stop(self) {}
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ---- driving a request through, and reading what it left behind --------
//
// Shared because m4d's derivation records are about the same runs m4c's
// selection tests drive: one copy of "submit, poll until it settles" and
// one of each store query, rather than two that drift.

/// Submit one request and poll until it is no longer preparing.
pub fn prepared(fixture: &Fixture, request: &str, investigation: &str) -> Value {
    let submitted = fixture.cbr(&[
        "context",
        request,
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
        "--repo-id",
        "app",
        "--selector",
        "queue drains shutdown",
        "--task",
        "what drains the queue",
        "--capacity",
        "65536",
        "--investigation",
        investigation,
        "--want",
        "q=source:queue.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = fixture.cbr(&["request", request]);
        assert!(
            polled.status.success(),
            "inspect: {}",
            String::from_utf8_lossy(&polled.stderr)
        );
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            return inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{request} never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Every request body the provider recorded, as text.
pub fn bodies_sent(data: &Path) -> Vec<String> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare("SELECT sent FROM model_calls ORDER BY id")
        .expect("the recordings table exists");
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .expect("queries")
        .map(|row| String::from_utf8_lossy(&row.expect("row")).to_string())
        .collect()
}

/// The ledger rows that are charges, with the request each is for.
pub fn ledger(data: &Path) -> Vec<(String, String, i64)> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare("SELECT request, kind, tokens FROM model_ledger ORDER BY id")
        .expect("the ledger table exists");
    let rows: Vec<(String, String, i64)> = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .expect("queries")
        .map(|row| row.expect("row"))
        .collect();
    rows.into_iter()
        .filter(|(_, kind, _)| !kind.starts_with("admitted_"))
        .collect()
}

/// Every recorded exchange, as (job, request, call).
pub fn calls(data: &Path) -> Vec<(String, String, String)> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare("SELECT job, request, call FROM model_calls ORDER BY id")
        .expect("the recordings table exists");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("queries")
        .map(|row| row.expect("row"))
        .collect()
}

/// Submit one request over `file`, without waiting for it.
pub fn submit(fixture: &Fixture, request: &str, file: &str) {
    let want = format!("q=source:{file}");
    let submitted = fixture.cbr(&[
        "context",
        request,
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
        "--repo-id",
        "app",
        "--selector",
        "queue drains shutdown",
        "--task",
        "what drains the queue",
        "--capacity",
        "65536",
        "--investigation",
        "1",
        "--want",
        &want,
    ]);
    assert!(
        submitted.status.success(),
        "submit {request}: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
}

/// Poll every request named, so each one's job gets ticks.
pub fn poll(fixture: &Fixture, requests: &[&str]) {
    for request in requests {
        let _ = fixture.cbr(&["request", request]);
    }
}

/// Every derivation artifact the store holds, as (id, artifact record).
///
/// Read out of the store because the protocol has no "list every
/// artifact" query and a test should not invent one. The *content* is
/// then fetched through `cbr fetch`, which is the real path a reader
/// takes.
pub fn derivations(data: &Path) -> Vec<(String, Value)> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare(
            "SELECT id, value FROM subjects
             WHERE kind = 'evidence.artifact' AND id LIKE 'der.%'
             ORDER BY id",
        )
        .expect("the subjects table exists");
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("queries")
        .map(|row| {
            let (id, value) = row.expect("row");
            (
                id,
                cbr_encoding::parse(value.as_bytes()).expect("a canonical record"),
            )
        })
        .collect()
}

/// The item's result, and its reason when it has one.
pub fn result(inspected: &Value, item_id: &str) -> (String, String) {
    let item = inspected
        .get("items")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|item| item.get("item_id").and_then(Value::as_str) == Some(item_id))
        .cloned()
        .unwrap_or_else(|| panic!("no item {item_id} in {inspected:?}"));
    (
        item.get("result")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        item.get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}
