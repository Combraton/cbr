//! Behaviour that no conformance fixture can reach, exercised against the real
//! binary over the real stdio binding.
//!
//! Two things are checked here that the `core` suite cannot check, because the
//! suite always launches the provider with a conformance configuration and
//! always stops it politely:
//!
//! - a **production** configuration serves no `core-test/1` and refuses a test
//!   control outright rather than ignoring it;
//! - state survives a **`SIGKILL`**, not merely a clean shutdown, so the two
//!   restart fixtures pass because of the durable path and not because a
//!   graceful exit flushed something.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn binary() -> PathBuf {
    // The test binary sits in target/<profile>/deps; the provider is two up.
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cbr-provider")
}

struct Provider {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl Provider {
    fn start(data_dir: &Path, config: Option<&Path>) -> Self {
        let mut command = Command::new(binary());
        command.arg("--data-dir").arg(data_dir);
        if let Some(config) = config {
            command.arg("--config").arg(config);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("provider starts");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        Self { child, reader }
    }

    /// Send one frame and read the response frame.
    fn call(&mut self, frame: &str) -> String {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{frame}").expect("writes");
        stdin.flush().expect("flushes");
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("reads a response");
        line
    }

    fn negotiate(&mut self, profiles: &str) -> String {
        self.call(&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"core.negotiate","params":{{"operation":"core.negotiate","message_id":"m-n","payload":{{"caller":{{"name":"t","version":"1"}},"receive_limits":{{"max_frame_bytes":1048576}},"profiles":[{profiles}]}}}}}}"#
        ))
    }

    fn kill(mut self) {
        self.child.kill().expect("SIGKILL");
        self.child.wait().expect("reaped");
    }

    fn stop(mut self) {
        drop(self.child.stdin.take());
        self.child.wait().expect("exits");
    }
}

const CORE_ONLY: &str =
    r#"{"name":"core","majors":[1],"required":true,"required_features":[],"optional_features":[]}"#;
const CORE_AND_TEST: &str = r#"{"name":"core","majors":[1],"required":true,"required_features":["core.events"],"optional_features":[]},{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}"#;

fn conformance_config(directory: &Path) -> PathBuf {
    let path = directory.join("conformance.json");
    std::fs::write(
        &path,
        r#"{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"]}"#,
    )
    .expect("writes config");
    path
}

#[test]
fn a_production_provider_does_not_serve_core_test() {
    let directory = tempfile::tempdir().expect("temp dir");

    // No configuration at all is a production launch.
    let mut provider = Provider::start(directory.path(), None);
    let describe = provider.call(
        r#"{"jsonrpc":"2.0","id":1,"method":"core.describe","params":{"operation":"core.describe","message_id":"m-d","payload":{}}}"#,
    );
    assert!(
        !describe.contains("core-test"),
        "a production manifest must not advertise core-test: {describe}"
    );

    // Requesting it as required refuses the whole negotiation.
    let refused = provider.negotiate(CORE_AND_TEST);
    assert!(
        refused.contains("unsupported_profile"),
        "core-test must be refused at negotiation in production: {refused}"
    );
    provider.stop();

    // And its operations do not exist, rather than merely being unauthorised.
    let mut provider = Provider::start(directory.path(), None);
    let negotiated = provider.negotiate(CORE_ONLY);
    assert!(
        negotiated.contains("\"result\""),
        "core alone negotiates: {negotiated}"
    );
    let denied = provider.call(
        r#"{"jsonrpc":"2.0","id":2,"method":"core-test.subject.applied_count","params":{"operation":"core-test.subject.applied_count","message_id":"m-q","payload":{"subject":{"kind":"core-test.subject","id":"s-1"}}}}"#,
    );
    assert!(
        denied.contains("method_not_found"),
        "a conformance-only operation does not exist in production: {denied}"
    );
    provider.stop();

    // The same provider under a conformance configuration does serve it, so
    // the test above is measuring the mode and not a missing implementation.
    let config = conformance_config(directory.path());
    let mut provider = Provider::start(directory.path(), Some(&config));
    let negotiated = provider.negotiate(CORE_AND_TEST);
    assert!(
        negotiated.contains("core-test"),
        "conformance mode serves core-test: {negotiated}"
    );
    provider.stop();
}

#[test]
fn a_production_configuration_refuses_a_test_control_rather_than_ignoring_it() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("production.json");

    // Every member the conformance configuration uses to drive fixtures.
    for control in [
        r#""clock":{"fixed":"2026-01-01T00:00:00Z"}"#,
        r#""capabilities":{"store.writable":"unsupported"}"#,
        r#""credentials":[{"credential":"ccred1.x.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}]"#,
        r#""test_barriers":{"directory":"/tmp","enabled":[]}"#,
        r#""events":{"new_epoch_on_start":true}"#,
        r#""faults":{}"#,
        r#""dedupe":{"advance_on_start":3}"#,
    ] {
        std::fs::write(&path, format!(r#"{{"format":"cbr-config/1",{control}}}"#))
            .expect("writes config");
        let output = Command::new(binary())
            .arg("--data-dir")
            .arg(directory.path())
            .arg("--config")
            .arg(&path)
            .stdin(Stdio::null())
            .output()
            .expect("runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "a production configuration carrying {control} must fail to start, not ignore it"
        );
        assert!(
            stderr.contains("refused rather than ignored"),
            "the refusal must name the control: {stderr}"
        );
    }

    // A production configuration with no test control starts normally, so the
    // refusals above are about the controls and not about the format.
    std::fs::write(&path, r#"{"format":"cbr-config/1","principal":"owner"}"#).expect("writes");
    let mut provider = Provider::start(directory.path(), Some(&path));
    let negotiated = provider.negotiate(CORE_ONLY);
    assert!(
        negotiated.contains("\"result\""),
        "an ordinary production config starts: {negotiated}"
    );
    provider.stop();
}

#[test]
fn committed_state_survives_sigkill() {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = conformance_config(directory.path());

    let mut provider = Provider::start(directory.path(), Some(&config));
    provider.negotiate(CORE_AND_TEST);
    // A put whose digest the provider recomputes, so this is the real command
    // path rather than a store call dressed up as one.
    let envelope = r#"{"operation":"core-test.subject.put","message_id":"m-1","command_id":"cmd-1","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-1"},"revision":0}],"authority_epoch":0,"requires":[],"payload":{"value":"survives"}}"#;
    let value = cbr_encoding::parse(envelope.as_bytes()).expect("envelope parses");
    let digest = cbr_encoding::command_digest(&value).expect("intent");
    let put = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"core-test.subject.put","params":{}}}"#,
        envelope.replace(
            r#""payload""#,
            &format!(r#""command_digest":"{digest}","payload""#)
        )
    );
    let accepted = provider.call(&put);
    assert!(
        accepted.contains("\"replay\":false"),
        "the command applied: {accepted}"
    );

    // No clean shutdown, no drain, no flush: the process dies where it stands.
    provider.kill();

    let mut provider = Provider::start(directory.path(), Some(&config));
    provider.negotiate(CORE_AND_TEST);
    let count = provider.call(
        r#"{"jsonrpc":"2.0","id":3,"method":"core-test.subject.applied_count","params":{"operation":"core-test.subject.applied_count","message_id":"m-3","payload":{"subject":{"kind":"core-test.subject","id":"s-1"}}}}"#,
    );
    assert!(
        count.contains("\"applied_count\":1"),
        "state must survive SIGKILL, not just a clean exit: {count}"
    );

    // And the command identity is still bound, so a retransmission replays
    // rather than applying a second time.
    let replayed = provider.call(&put);
    assert!(
        replayed.contains("\"replay\":true"),
        "the command record must survive too: {replayed}"
    );

    // The event committed before the kill is readable afterwards at the same
    // position. A store that rebuilt its log from current state could satisfy
    // applied_count above and still fail this.
    let read = provider.call(
        r#"{"jsonrpc":"2.0","id":4,"method":"core.events.read","params":{"operation":"core.events.read","message_id":"m-4","payload":{"limit":100,"from":"start"}}}"#,
    );
    assert!(
        read.contains("\"epoch\":1") && read.contains("\"sequence\":1"),
        "the event survives SIGKILL at its original position: {read}"
    );
    assert!(
        read.contains("\"command_id\":\"cmd-1\""),
        "and still names its command: {read}"
    );
    provider.stop();
}

#[test]
fn a_retention_gap_after_restart_is_reported_rather_than_closed() {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = conformance_config(directory.path());

    let mut provider = Provider::start(directory.path(), Some(&config));
    provider.negotiate(CORE_AND_TEST);
    for (n, revision) in [(1, 0), (2, 1), (3, 2)] {
        let envelope = format!(
            r#"{{"operation":"core-test.subject.put","message_id":"m-{n}","command_id":"cmd-{n}","dedupe_generation":1,"subject":{{"kind":"core-test.subject","id":"s-1"}},"preconditions":[{{"subject":{{"kind":"core-test.subject","id":"s-1"}},"revision":{revision}}}],"authority_epoch":0,"requires":[],"payload":{{"value":"v{n}"}}}}"#
        );
        let value = cbr_encoding::parse(envelope.as_bytes()).expect("envelope parses");
        let digest = cbr_encoding::command_digest(&value).expect("intent");
        let framed = format!(
            r#"{{"jsonrpc":"2.0","id":{n},"method":"core-test.subject.put","params":{}}}"#,
            envelope.replace(
                r#""payload""#,
                &format!(r#""command_digest":"{digest}","payload""#)
            )
        );
        assert!(provider.call(&framed).contains("\"replay\":false"));
    }
    provider.kill();

    // Restart keeping only the last event. The earlier ones are gone, and a
    // reader starting from the beginning must be told so.
    let retaining = directory.path().join("retaining.json");
    std::fs::write(
        &retaining,
        r#"{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"events":{"retain_last":1}}"#,
    )
    .expect("writes config");
    let mut provider = Provider::start(directory.path(), Some(&retaining));
    provider.negotiate(CORE_AND_TEST);
    let read = provider.call(
        r#"{"jsonrpc":"2.0","id":9,"method":"core.events.read","params":{"operation":"core.events.read","message_id":"m-9","payload":{"limit":100,"from":"start"}}}"#,
    );
    assert!(
        read.contains(r#""kind":"retention""#),
        "a gap is reported, not closed over: {read}"
    );
    assert!(
        read.contains(r#""snapshot""#),
        "and it carries a snapshot: {read}"
    );
    assert!(
        !read.contains(r#""error""#),
        "a gap is an item, never a refusal: {read}"
    );
    provider.stop();
}
