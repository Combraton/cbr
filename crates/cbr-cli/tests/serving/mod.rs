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

/// The one principal this fixture has, in the form a conformance launch
/// takes: `ccred1.<principal>.<secret>`.
pub const CREDENTIAL: &str = "ccred1.owner.model-crash-matrix";

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
                r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{CREDENTIAL}"}}],"context":{{"compile":true}},"test_barriers":{{"directory":"{}","enabled":[{enabled}]}},"model":{{"dialect":"responses","model":"MiniMax-M2.7-highspeed","answers":[{scripted}],"usage":5000,"counting":"{counting}"}}}}"#,
                barriers.display()
            ),
        )
        .expect("config");
        // A conformance launch writes no credential file: its principals
        // are given to it, and the runner holds what they present. Here
        // the runner is this test.
        std::fs::write(directory.path().join("credential"), CREDENTIAL).expect("credential");

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
        let child = Command::new(provider_binary())
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(self.directory.path().join("cbr.json"))
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
