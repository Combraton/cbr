//! **An idle session never waits for the processing lock.**
//!
//! A socket session with nothing to read polls: every 40 ms it ticks and then
//! re-checks its subscriptions. Both steps used to take the process-wide
//! processing lock. When another session held that lock, the idle poll queued
//! behind it — and a session queued inside its own poll reads nothing. A
//! command sent to it sat in the socket until the lock was free, and never
//! reached the point where a request finds the lock held and says so.
//!
//! That is how `socket.subscription-recheck-race-regression` failed at step 9
//! on CI, once in the workflow's history: the fixture pauses a re-check under
//! the lock, clears every signal, sends a revoke on the other session, and
//! waits for the revoke to find the lock held. It passed whenever the other
//! session's idle poll contended *after* the runner cleared the signals,
//! because the poll's own signal stood in for the revoke's; it failed when
//! the poll contended first, because the session was then parked in the poll
//! and never read the revoke.
//!
//! This test takes the losing order deliberately rather than waiting for it:
//! it lets the idle poll contend, clears the signals afterwards, and only then
//! sends the command.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const RECHECK: &str = "subscription.recheck.after_authorization";
const CONTENDED: &str = "processing.lock.contended";
const IDLE_QUEUED: &str = "processing.idle.queued";
const PUT_AFTER_PROCESSING: &str = "core-test.put.after-processing-lock";
/// CORE section 16.5's conformance bound for a lapse or delivery caused by
/// another socket session.
const DELIVERY_BOUND: Duration = Duration::from_secs(2);
const CORE_GRANTS_AND_TEST: &str = r#"{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.grants"],"optional_features":[]},{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}"#;

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cbr-provider")
}

/// A provider killed when the test ends, however it ends, so a failing
/// assertion cannot leave one paused at a barrier.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Frame a command envelope with the digest the provider will recompute.
fn command(id: &str, envelope: &str) -> String {
    let value = cbr_encoding::parse(envelope.as_bytes()).expect("envelope parses");
    let digest = cbr_encoding::command_digest(&value).expect("intent");
    let operation = value
        .get("operation")
        .and_then(|v| v.as_str())
        .expect("operation");
    format!(
        r#"{{"jsonrpc":"2.0","id":"{id}","method":"{operation}","params":{}}}"#,
        envelope.replace(
            r#""payload""#,
            &format!(r#""command_digest":"{digest}","payload""#)
        )
    )
}

struct Session {
    reader: BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
}

impl Session {
    fn connect(socket: &Path, credential: &str) -> Self {
        let started = Instant::now();
        let stream = loop {
            match std::os::unix::net::UnixStream::connect(socket) {
                Ok(stream) => break stream,
                Err(_) if started.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("the provider never listened: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("timeout");
        let mut session = Self {
            reader: BufReader::new(stream.try_clone().expect("clone")),
            writer: stream,
        };
        let authenticated = session.call(&format!(
            r#"{{"jsonrpc":"2.0","id":"auth","method":"core.authenticate","params":{{"operation":"core.authenticate","message_id":"m-auth","payload":{{"credential":"{credential}"}}}}}}"#
        ));
        let principal = credential.split('.').nth(1).expect("credential principal");
        assert!(
            authenticated.contains(&format!(r#""principal":"{principal}""#)),
            "{authenticated}"
        );
        let negotiated = session.call(&format!(
            r#"{{"jsonrpc":"2.0","id":"neg","method":"core.negotiate","params":{{"operation":"core.negotiate","message_id":"m-neg","payload":{{"caller":{{"name":"t","version":"1"}},"receive_limits":{{"max_frame_bytes":1048576}},"profiles":[{CORE_GRANTS_AND_TEST}]}}}}}}"#
        ));
        assert!(negotiated.contains(r#""result""#), "{negotiated}");
        session
    }

    fn send(&mut self, frame: &str) {
        writeln!(self.writer, "{frame}").expect("writes");
    }

    /// The next frame that is not a notification.
    fn response(&mut self) -> String {
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).expect("reads");
            assert!(read > 0, "the connection ended before a response");
            if !line.contains(r#""method":"core.events.notify""#) {
                return line;
            }
        }
    }

    fn call(&mut self, frame: &str) -> String {
        self.send(frame);
        self.response()
    }

    fn notification(&mut self, within: Duration) -> String {
        self.reader
            .get_mut()
            .set_read_timeout(Some(within))
            .expect("notification timeout");
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).unwrap_or_else(|error| {
                panic!("no notification within {} ms: {error}", within.as_millis())
            });
            assert!(read > 0, "the connection ended before a notification");
            if line.contains(r#""method":"core.events.notify""#) {
                return line;
            }
        }
    }
}

fn wait_for(path: &Path, within: Duration) -> bool {
    let started = Instant::now();
    while !path.exists() {
        if started.elapsed() > within {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    true
}

fn wait_until(within: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while !condition() {
        if started.elapsed() > within {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
    true
}

#[test]
fn a_command_sent_to_an_idle_session_reaches_the_lock_while_another_session_holds_it() {
    let directory = tempfile::tempdir().expect("temp dir");
    let sockets = directory.path().join("sockets");
    std::fs::create_dir(&sockets).expect("socket dir");
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
    let socket = sockets.join("provider.sock");
    let data = directory.path().join("data");
    std::fs::create_dir(&data).expect("data dir");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let credential = format!("ccred1.owner.{}", "A".repeat(43));
    let config = directory.path().join("socket.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{credential}"}}],"test_barriers":{{"directory":"{}","enabled":["{RECHECK}"]}}}}"#,
            barriers.display()
        ),
    )
    .expect("config");
    let _provider = Running(
        Command::new(binary())
            .arg("--data-dir")
            .arg(&data)
            .arg("--config")
            .arg(&config)
            .arg("--socket")
            .arg(&socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("provider starts"),
    );

    // The session that will be idle, connected while the lock is free.
    let mut idle = Session::connect(&socket, &credential);

    // A subscriber whose first re-check pauses under the lock.
    let mut subscriber = Session::connect(&socket, &credential);
    let subscribed = subscriber.call(r#"{"jsonrpc":"2.0","id":"sub","method":"core.events.subscribe","params":{"operation":"core.events.subscribe","message_id":"m-sub","payload":{"from":"now"}}}"#);
    assert!(subscribed.contains(r#""subscription""#), "{subscribed}");
    assert!(
        wait_for(
            &barriers.join(format!("{RECHECK}.reached")),
            Duration::from_secs(10)
        ),
        "the re-check never paused under the lock"
    );

    // Long enough for the idle session to have polled several times with the
    // lock held. Then clear every signal, as the runner does once it has seen
    // the pause: whatever the polls signalled is behind us.
    std::thread::sleep(Duration::from_millis(400));
    for entry in std::fs::read_dir(&barriers).expect("barrier dir") {
        let path = entry.expect("entry").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "signal")
        {
            std::fs::remove_file(path).expect("clears a signal");
        }
    }

    // Several idle poll intervals must remain quiet. An idle poll neither
    // waits for the processing lock nor claims command contention.
    let contention = barriers.join(format!("{CONTENDED}.signal"));
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !contention.exists(),
        "an idle poll signalled processing-lock contention"
    );

    // A command to the idle session. It must be read, and must find the lock
    // held — the only thing that can raise the signal now.
    idle.send(&command(
        "put",
        r#"{"operation":"core-test.subject.put","message_id":"m-put","command_id":"cmd-put","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-1"},"revision":0}],"authority_epoch":0,"requires":[],"payload":{"value":"after-the-pause"}}"#,
    ));
    let reached_the_lock = wait_for(
        &barriers.join(format!("{CONTENDED}.signal")),
        Duration::from_secs(5),
    );

    // Release the re-check either way, so the provider can answer.
    std::fs::write(barriers.join(format!("{RECHECK}.release")), b"").expect("releases");
    assert!(
        reached_the_lock,
        "a command sent to an idle session never reached the processing lock while another \
         session held it: the session was parked in its own idle poll"
    );
    let put = idle.response();
    assert!(
        put.contains(r#""replay":false"#),
        "the command commits once the lock is free: {put}"
    );
}

#[test]
fn a_subscription_recheck_keeps_authorization_and_event_read_under_one_guard() {
    let directory = tempfile::tempdir().expect("temp dir");
    let sockets = directory.path().join("sockets");
    std::fs::create_dir(&sockets).expect("socket dir");
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
    let socket = sockets.join("provider.sock");
    let data = directory.path().join("data");
    std::fs::create_dir(&data).expect("data dir");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let owner = format!("ccred1.owner.{}", "C".repeat(43));
    let agent = format!("ccred1.agent-1.{}", "D".repeat(43));
    let config = directory.path().join("socket.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{owner}"}},{{"credential":"{agent}"}}],"test_barriers":{{"directory":"{}","enabled":["{RECHECK}"]}}}}"#,
            barriers.display()
        ),
    )
    .expect("config");
    let _provider = Running(
        Command::new(binary())
            .arg("--data-dir")
            .arg(&data)
            .arg("--config")
            .arg(&config)
            .arg("--socket")
            .arg(&socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("provider starts"),
    );

    let mut revoker = Session::connect(&socket, &owner);
    let issued = revoker.call(&command(
        "issue",
        r#"{"operation":"core.grant.issue","message_id":"m-issue","command_id":"cmd-issue","dedupe_generation":1,"subject":{"kind":"core.grant","id":"g-1"},"preconditions":[{"subject":{"kind":"core.grant","id":"g-1"},"revision":0}],"requires":[],"payload":{"holder":"agent-1","audience":"conformance-provider","rights":["core.events.read","core-test.read"],"resources":[{"kind":"core-test.subject","id_prefix":"s-"}],"delegation":{"allowed":false,"max_depth":0}}}"#,
    ));
    assert!(issued.contains(r#""replay":false"#), "{issued}");

    let mut writer = Session::connect(&socket, &owner);
    let mut subscriber = Session::connect(&socket, &agent);
    let subscribed = subscriber.call(r#"{"jsonrpc":"2.0","id":"sub","method":"core.events.subscribe","params":{"operation":"core.events.subscribe","message_id":"m-sub","grant":"g-1","payload":{"from":"now","kinds":["core-test.subject"]}}}"#);
    assert!(subscribed.contains(r#""subscription""#), "{subscribed}");
    assert!(
        wait_for(
            &barriers.join(format!("{RECHECK}.reached")),
            Duration::from_secs(10)
        ),
        "the subscription re-check never paused after authorization"
    );

    revoker.send(&command(
        "revoke",
        r#"{"operation":"core.grant.revoke","message_id":"m-revoke","command_id":"cmd-revoke","dedupe_generation":1,"subject":{"kind":"core.grant","id":"g-1"},"preconditions":[{"subject":{"kind":"core.grant","id":"g-1"},"revision":1}],"requires":[],"payload":{}}"#,
    ));
    writer.send(&command(
        "put",
        r#"{"operation":"core-test.subject.put","message_id":"m-put","command_id":"cmd-put","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-1"},"revision":0}],"authority_epoch":0,"requires":[],"payload":{"value":"after-revoke"}}"#,
    ));
    assert!(
        wait_for(
            &barriers.join(format!("{CONTENDED}.signal")),
            Duration::from_secs(5)
        ),
        "neither command reached the held processing guard"
    );
    std::fs::write(barriers.join(format!("{RECHECK}.release")), b"").expect("releases");
    let revoked = revoker.response();
    assert!(revoked.contains(r#""revoked":["g-1"]"#), "{revoked}");
    let put = writer.response();
    assert!(put.contains(r#""replay":false"#), "{put}");

    let notification = subscriber.notification(DELIVERY_BOUND);
    assert!(
        notification.contains(r#""ended":{"reason":"authorization_lost"}"#),
        "the re-check delivered an event under authorization that was already revoked: {notification}"
    );
    assert!(
        notification.contains(r#""items":[]"#),
        "the ended subscription carried an event committed after revocation: {notification}"
    );
}

#[test]
fn an_idle_subscription_is_not_starved_by_sustained_command_contention() {
    let directory = tempfile::tempdir().expect("temp dir");
    let sockets = directory.path().join("sockets");
    std::fs::create_dir(&sockets).expect("socket dir");
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
    let socket = sockets.join("provider.sock");
    let data = directory.path().join("data");
    std::fs::create_dir(&data).expect("data dir");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let credential = format!("ccred1.owner.{}", "B".repeat(43));
    let config = directory.path().join("socket.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"credentials":[{{"credential":"{credential}"}}],"test_barriers":{{"directory":"{}","enabled":["{PUT_AFTER_PROCESSING}"]}}}}"#,
            barriers.display()
        ),
    )
    .expect("config");
    let _provider = Running(
        Command::new(binary())
            .arg("--data-dir")
            .arg(&data)
            .arg("--config")
            .arg(&config)
            .arg("--socket")
            .arg(&socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("provider starts"),
    );

    let mut subscriber = Session::connect(&socket, &credential);
    let subscribed = subscriber.call(r#"{"jsonrpc":"2.0","id":"sub","method":"core.events.subscribe","params":{"operation":"core.events.subscribe","message_id":"m-sub","payload":{"from":"now","kinds":["core-test.subject"]}}}"#);
    assert!(subscribed.contains(r#""subscription""#), "{subscribed}");

    // Connect the contending sessions before the lock is held. Blank frames
    // arrive more often than POLL, so their server-side sessions keep reading
    // rather than reserving idle tickets ahead of the subscriber.
    let running = Arc::new(AtomicBool::new(true));
    let start_writing = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicU64::new(0));
    let committed = Arc::new(AtomicU64::new(0));
    let writers: Vec<_> = (0..4)
        .map(|worker| {
            let socket = socket.clone();
            let credential = credential.clone();
            let running = Arc::clone(&running);
            let start_writing = Arc::clone(&start_writing);
            let ready = Arc::clone(&ready);
            let committed = Arc::clone(&committed);
            thread::spawn(move || {
                let mut session = Session::connect(&socket, &credential);
                ready.fetch_add(1, Ordering::Release);
                while !start_writing.load(Ordering::Acquire) {
                    session.send("");
                    thread::sleep(Duration::from_millis(10));
                }
                let mut sequence = 0u64;
                while running.load(Ordering::Relaxed) {
                    let id = format!("w-{worker}-{sequence}");
                    let response = session.call(&command(
                        &id,
                        &format!(
                            r#"{{"operation":"core-test.subject.put","message_id":"m-{id}","command_id":"cmd-{id}","dedupe_generation":1,"subject":{{"kind":"core-test.subject","id":"s-{id}"}},"preconditions":[{{"subject":{{"kind":"core-test.subject","id":"s-{id}"}},"revision":0}}],"authority_epoch":0,"requires":[],"payload":{{"value":"under-contention"}}}}"#
                        ),
                    ));
                    assert!(response.contains(r#""replay":false"#), "{response}");
                    committed.fetch_add(1, Ordering::Relaxed);
                    sequence += 1;
                }
            })
        })
        .collect();
    assert!(
        wait_until(Duration::from_secs(5), || ready.load(Ordering::Acquire)
            == 4),
        "the contending sessions did not connect"
    );

    // Hold the processing lock until the subscriber has reserved its FIFO
    // place. That reservation does not block its socket reader and uses a
    // signal distinct from command contention.
    let mut blocker = Session::connect(&socket, &credential);
    blocker.send(&command(
        "blocker",
        r#"{"operation":"core-test.subject.put","message_id":"m-blocker","command_id":"cmd-blocker","dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-blocker"},"preconditions":[{"subject":{"kind":"core-test.subject","id":"s-blocker"},"revision":0}],"authority_epoch":0,"requires":[],"payload":{"value":"starts-the-delivery"}}"#,
    ));
    assert!(
        wait_for(
            &barriers.join(format!("{PUT_AFTER_PROCESSING}.reached")),
            Duration::from_secs(5)
        ),
        "the blocker never paused while holding the processing lock"
    );
    assert!(
        wait_for(
            &barriers.join(format!("{IDLE_QUEUED}.signal")),
            Duration::from_secs(1)
        ),
        "the idle subscriber did not reserve bounded progress"
    );
    start_writing.store(true, Ordering::Release);

    assert!(
        wait_for(
            &barriers.join(format!("{CONTENDED}.signal")),
            Duration::from_secs(5)
        ),
        "the writers never contended for the processing lock"
    );
    std::fs::write(
        barriers.join(format!("{PUT_AFTER_PROCESSING}.release")),
        b"",
    )
    .expect("releases the blocker");
    let waiting = Instant::now();
    let notification = subscriber.notification(DELIVERY_BOUND);
    assert!(
        waiting.elapsed() <= DELIVERY_BOUND,
        "the idle subscription exceeded CORE section 16.5's 2 s delivery bound"
    );
    assert!(
        notification.contains(r#""kind":"core-test.subject""#),
        "the idle subscription received the wrong notification: {notification}"
    );
    let blocked = blocker.response();
    assert!(blocked.contains(r#""replay":false"#), "{blocked}");
    running.store(false, Ordering::Relaxed);
    for writer in writers {
        writer.join().expect("writer finishes");
    }
    assert!(
        committed.load(Ordering::Relaxed) > 0,
        "no contending command committed"
    );
}
