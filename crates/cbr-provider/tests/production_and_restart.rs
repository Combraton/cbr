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

/// Frame a command envelope with the digest the provider will recompute, so a
/// test exercises the real command path rather than a store call in disguise.
fn command(id: i64, envelope: &str) -> String {
    let value = cbr_encoding::parse(envelope.as_bytes()).expect("envelope parses");
    let digest = cbr_encoding::command_digest(&value).expect("intent");
    let operation = value
        .get("operation")
        .and_then(|v| v.as_str())
        .expect("operation");
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"{operation}","params":{}}}"#,
        envelope.replace(
            r#""payload""#,
            &format!(r#""command_digest":"{digest}","payload""#)
        )
    )
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

/// A conformance configuration for one session's principal. The authority set
/// stays `owner`, so a session launched as `agent-1` acts only under a grant.
fn session_config(directory: &Path, name: &str, principal: &str) -> PathBuf {
    let path = directory.join(format!("{name}.json"));
    std::fs::write(
        &path,
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"{principal}","authority_principals":["owner"]}}"#
        ),
    )
    .expect("writes config");
    path
}

const CORE_AND_GRANTS: &str = r#"{"name":"core","majors":[1],"required":true,"required_features":["core.grants"],"optional_features":[]},{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}"#;

/// A grant is durable state, and so is its revocation.
///
/// The `core` suite proves both within a session and across a clean stop, but
/// every fixture stops the provider politely. This kills it. The failure this
/// guards against is specific and plausible: a provider that keeps grants in
/// memory and rebuilds them from an event log would satisfy every fixture and
/// then, after a crash, either forget a grant — inconvenient — or forget a
/// **revocation**, which silently restores authority the owner withdrew.
#[test]
fn a_grant_and_its_revocation_both_survive_sigkill() {
    let directory = tempfile::tempdir().expect("temp dir");
    let owner = session_config(directory.path(), "owner", "owner");
    let agent = session_config(directory.path(), "agent", "agent-1");

    let mut provider = Provider::start(directory.path(), Some(&owner));
    provider.negotiate(CORE_AND_GRANTS);
    for (n, id) in [(1, "g-live"), (2, "g-dead")] {
        let issued = provider.call(&command(
            n,
            &format!(
                r#"{{"operation":"core.grant.issue","message_id":"m-{id}","command_id":"issue-{id}","dedupe_generation":1,"subject":{{"kind":"core.grant","id":"{id}"}},"preconditions":[{{"subject":{{"kind":"core.grant","id":"{id}"}},"revision":0}}],"requires":[],"payload":{{"holder":"agent-1","audience":"conformance-provider","rights":["core-test.read","core-test.write"],"resources":[{{"kind":"core-test.subject","id_prefix":"s-"}}],"delegation":{{"allowed":false,"max_depth":0}}}}}}"#
            ),
        ));
        assert!(issued.contains("\"replay\":false"), "issued {id}: {issued}");
    }
    let revoked = provider.call(&command(
        3,
        r#"{"operation":"core.grant.revoke","message_id":"m-rev","command_id":"revoke-g-dead","dedupe_generation":1,"subject":{"kind":"core.grant","id":"g-dead"},"preconditions":[{"subject":{"kind":"core.grant","id":"g-dead"},"revision":1}],"requires":[],"payload":{}}"#,
    ));
    assert!(
        revoked.contains(r#""revoked":["g-dead"]"#),
        "revoked: {revoked}"
    );

    // The process dies where it stands, with no drain and no flush.
    provider.kill();

    let mut provider = Provider::start(directory.path(), Some(&agent));
    provider.negotiate(CORE_AND_GRANTS);
    let under_live = provider.call(&command(
        4,
        r#"{"operation":"core-test.subject.put","message_id":"m-a","command_id":"cmd-a","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-1"},"revision":0}],"authority_epoch":0,"requires":[],"grant":"g-live","payload":{"value":"a"}}"#,
    ));
    assert!(
        under_live.contains("\"replay\":false"),
        "a grant issued before the kill still authorizes after it: {under_live}"
    );
    let under_dead = provider.call(&command(
        5,
        r#"{"operation":"core-test.subject.put","message_id":"m-b","command_id":"cmd-b","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-2"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-2"},"revision":0}],"authority_epoch":0,"requires":[],"grant":"g-dead","payload":{"value":"b"}}"#,
    ));
    assert!(
        under_dead.contains(r#""reason":"revoked""#),
        "a grant revoked before the kill is still refused after it: {under_dead}"
    );
    provider.stop();
}

/// `(epoch, sequence, revision)` of every `core.capabilities.changed` event in
/// a `core.events.read` response, read by parsing rather than by substring,
/// because canonical form orders members and adjacency means nothing.
fn change_events(response: &str) -> Vec<(i64, i64, i64)> {
    let value = cbr_encoding::parse(response.trim_end().as_bytes()).expect("response parses");
    let int = |v: Option<&cbr_encoding::Value>| match v {
        Some(cbr_encoding::Value::Int(n)) => *n,
        other => panic!("not an integer: {other:?}"),
    };
    value
        .get("result")
        .and_then(|r| r.get("items"))
        .and_then(|i| i.as_array())
        .expect("items")
        .iter()
        .filter_map(|item| item.get("event"))
        .filter(|event| {
            event.get("type").and_then(|t| t.as_str()) == Some("core.capabilities.changed")
        })
        .map(|event| {
            (
                int(event.get("epoch")),
                int(event.get("sequence")),
                int(event.get("revision")),
            )
        })
        .collect()
}

/// A capability snapshot and the event recording its change are durable, at
/// the positions they were first given.
///
/// The change is recorded at launch, before any frame, so this is the one
/// piece of event history a provider writes with no command to hang it on.
/// Two failures are plausible and both are checked: the snapshot lost across a
/// crash, so its revision drops back and the change is announced again at a
/// new position; or the change event rebuilt from the snapshot on every start,
/// which would duplicate it.
#[test]
fn a_capability_change_and_its_event_survive_sigkill_at_their_positions() {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = |name: &str, status: Option<&str>| {
        let path = directory.path().join(format!("{name}.json"));
        let capabilities = status
            .map(|s| format!(r#","capabilities":{{"core-test.writes":"{s}"}}"#))
            .unwrap_or_default();
        std::fs::write(
            &path,
            format!(
                r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"]{capabilities}}}"#
            ),
        )
        .expect("writes config");
        path
    };
    const CORE_WITH_CAPABILITIES: &str = r#"{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.capabilities"],"optional_features":[]},{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}"#;
    const CAPABILITIES: &str = r#"{"jsonrpc":"2.0","id":2,"method":"core.capabilities","params":{"operation":"core.capabilities","message_id":"m-c","payload":{}}}"#;
    const READ: &str = r#"{"jsonrpc":"2.0","id":3,"method":"core.events.read","params":{"operation":"core.events.read","message_id":"m-r","payload":{"limit":100,"from":"start"}}}"#;

    // A new store: the initial snapshot is revision 1 and appends nothing.
    let supported = config("supported", None);
    let mut provider = Provider::start(directory.path(), Some(&supported));
    provider.negotiate(CORE_WITH_CAPABILITIES);
    let first = provider.call(CAPABILITIES);
    assert!(
        first.contains(r#""revision":1"#),
        "initial snapshot: {first}"
    );
    provider.kill();

    // The capability changes at launch: revision 2 and one provider-origin
    // event, at the stream's first position.
    let unknown = config("unknown", Some("unknown"));
    let mut provider = Provider::start(directory.path(), Some(&unknown));
    provider.negotiate(CORE_WITH_CAPABILITIES);
    let changed = provider.call(CAPABILITIES);
    assert!(
        changed.contains(r#""revision":2"#),
        "changed snapshot: {changed}"
    );
    let before = provider.call(READ);
    assert_eq!(
        change_events(&before),
        vec![(1, 1, 2)],
        "one change event, at epoch 1 sequence 1, naming snapshot revision 2: {before}"
    );
    // No clean shutdown after the change was recorded.
    provider.kill();

    // Same configuration again, after SIGKILL: nothing changed, so the
    // revision holds, the event is still at its position, and there is
    // exactly one of it.
    let mut provider = Provider::start(directory.path(), Some(&unknown));
    provider.negotiate(CORE_WITH_CAPABILITIES);
    let after_snapshot = provider.call(CAPABILITIES);
    assert!(
        after_snapshot.contains(r#""revision":2"#)
            && after_snapshot.contains(r#""status":"unknown""#),
        "the snapshot survives SIGKILL: {after_snapshot}"
    );
    let after = provider.call(READ);
    assert_eq!(
        change_events(&after),
        vec![(1, 1, 2)],
        "after SIGKILL the change event is still at its position, and a restart with no \
         change records no second one: {after}"
    );
    provider.stop();
}

/// Over the real stdio binding, a consumer that stops reading standard output
/// ends the process — which is what closing the connection means there — and
/// the ending is recorded on standard error with the bound it was held to.
///
/// The session tests measure the timing precisely against a writer they
/// control. This one checks what they cannot: that the binary's wiring really
/// exits rather than leaving a writer thread blocked on a full pipe forever.
#[test]
fn a_stdio_consumer_that_stops_reading_ends_the_process_and_records_the_bound() {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("bounded.json");
    std::fs::write(
        &config,
        r#"{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"events":{"max_pending_notification_bytes":4096,"backpressure_notice_ms":300}}"#,
    )
    .expect("writes config");

    let mut child = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path())
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("provider starts");
    // Held open and never read: a stalled consumer, not a vanished one.
    let _stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");

    // A pipelined client: far more output than the pipe buffer and the bound
    // together, all requested without waiting for a response.
    let mut input = format!(
        "{}\n",
        r#"{"jsonrpc":"2.0","id":0,"method":"core.negotiate","params":{"operation":"core.negotiate","message_id":"m-n","payload":{"caller":{"name":"t","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.events.backpressure"],"optional_features":[]},{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}]}}}"#
    );
    input.push_str(r#"{"jsonrpc":"2.0","id":"s","method":"core.events.subscribe","params":{"operation":"core.events.subscribe","message_id":"m-s","payload":{"from":"now"}}}"#);
    input.push('\n');
    for n in 1..=400 {
        input.push_str(&command(
            n,
            &format!(
                r#"{{"operation":"core-test.subject.put","message_id":"m-{n}","command_id":"cmd-{n}","dedupe_generation":1,"subject":{{"kind":"core-test.subject","id":"s-{n}"}},"preconditions":[{{"subject":{{"kind":"core-test.subject","id":"s-{n}"}},"revision":0}}],"authority_epoch":0,"requires":[],"payload":{{"value":"{n}"}}}}"#
            ),
        ));
        input.push('\n');
    }
    let mut stdin = child.stdin.take().expect("stdin");
    // The provider stops reading input while it waits for room, so this may
    // block, and it ends with a broken pipe once the provider exits.
    std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });

    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("polls") {
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(20) {
            child.kill().expect("kills");
            panic!("the provider never closed a consumer that stopped reading");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let mut diagnostics = String::new();
    std::io::Read::read_to_string(&mut stderr, &mut diagnostics).expect("reads stderr");
    assert!(
        status.success(),
        "a closure is not a crash: {status}; {diagnostics}"
    );
    assert!(
        diagnostics.contains("consumer too slow") && diagnostics.contains("a bound of 4096"),
        "the ending is recorded with the bound it was held to: {diagnostics}"
    );
}
