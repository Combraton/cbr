//! The storage crash matrix (STORAGE section 5), fault-injected by killing the
//! real provider at each named boundary and restarting it over the same data
//! directory.
//!
//! Each boundary is a test barrier inside the provider (decision 007): the
//! thread that reaches it creates `<name>.reached` and waits. The test sees the
//! marker, sends `SIGKILL`, and checks that no response frame was written. So
//! the process dies at that line of code, not near it, and not after a flush a
//! graceful exit would have performed.
//!
//! | Row | Boundary | Test |
//! |---|---|---|
//! | Before command commit | `evidence.append.before_commit` | [`before_commit_nothing_is_accepted_and_the_same_command_applies`] |
//! | After accepted, before ack | `evidence.append.after_commit` | [`after_commit_before_the_acknowledgment_the_retry_replays_and_appends_nothing`] |
//! | During artifact upload | idle between appends, then `evidence.append.before_commit` | [`a_killed_upload_stays_staged_is_never_served_and_resumes`] |
//! | Orphan object (STORAGE section 2) | `evidence.seal.after_object_published` | [`an_object_published_before_its_seal_row_is_collected_and_never_served`] |
//! | During collection | `evidence.purge.after_commit` | [`a_purge_killed_before_deletion_is_finished_at_restart_and_rechecks_roots`] |
//!
//! The other rows belong to PIO or Comreton, or to parts of CBR that do not
//! exist in M1; `docs/VERIFICATION.md` says which and why.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cbr-provider")
}

const APPEND_BEFORE_COMMIT: &str = "evidence.append.before_commit";
const APPEND_AFTER_COMMIT: &str = "evidence.append.after_commit";
const SEAL_AFTER_OBJECT: &str = "evidence.seal.after_object_published";
const PURGE_AFTER_COMMIT: &str = "evidence.purge.after_commit";

/// A data directory and the barrier directory that goes with it.
struct Data {
    directory: tempfile::TempDir,
}

impl Data {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(directory.path().join("barriers")).expect("barrier dir");
        Self { directory }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Start a provider with only these barriers enabled. Each barrier pauses
    /// the first time it is reached in that process.
    fn start(&self, barriers: &[&str]) -> Provider {
        let barrier_dir = self.path().join("barriers");
        for entry in std::fs::read_dir(&barrier_dir).expect("barrier dir") {
            std::fs::remove_file(entry.expect("entry").path()).expect("clears markers");
        }
        let enabled = barriers
            .iter()
            .map(|b| format!("\"{b}\""))
            .collect::<Vec<_>>()
            .join(",");
        let config = self.path().join("config.json");
        std::fs::write(
            &config,
            format!(
                r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"test_barriers":{{"directory":"{}","enabled":[{enabled}]}}}}"#,
                barrier_dir.display()
            ),
        )
        .expect("config");
        let mut child = Command::new(binary())
            .arg("--data-dir")
            .arg(self.path().join("data"))
            .arg("--config")
            .arg(&config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("provider starts");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        let mut provider = Provider {
            child,
            reader,
            next: 1,
            barriers: barrier_dir,
        };
        let negotiated = provider.call(
            "core.negotiate",
            r#"{"caller":{"name":"crash-matrix","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events"],"optional_features":[]},{"name":"evidence","majors":[1],"required":true,"required_features":["evidence.retention_control"],"optional_features":[]}]}"#,
            None,
        );
        assert!(
            negotiated.get("result").is_some(),
            "negotiated: {negotiated:?}"
        );
        provider
    }

    /// The published object for a digest, where the store keeps it.
    fn object(&self, digest: &str) -> PathBuf {
        let hex = digest.strip_prefix("sha256:").expect("sha256");
        self.path()
            .join("data")
            .join("objects")
            .join("sha256")
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(&hex[4..])
    }
}

struct Provider {
    child: Child,
    reader: BufReader<ChildStdout>,
    next: i64,
    barriers: PathBuf,
}

impl Provider {
    fn frame(&mut self, method: &str, params: &str) -> String {
        let id = self.next;
        self.next += 1;
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
    }

    fn write(&mut self, frame: &str) {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{frame}").expect("writes");
        stdin.flush().expect("flushes");
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("reads");
        assert!(!line.is_empty(), "the provider ended without a response");
        cbr_encoding::parse(line.trim_end().as_bytes()).expect("response parses")
    }

    /// A query, or with `command` a command envelope (see [`command`]).
    fn call(&mut self, operation: &str, payload: &str, command: Option<&str>) -> Value {
        let frame = self.frame(operation, &envelope(operation, payload, command));
        self.write(&frame);
        self.read()
    }

    /// Send a command that will stop at `barrier`, kill the process there,
    /// and check it never answered.
    fn kill_at(mut self, barrier: &str, operation: &str, payload: &str, command: &str) {
        let frame = self.frame(operation, &envelope(operation, payload, Some(command)));
        self.write(&frame);
        let reached = self.barriers.join(format!("{barrier}.reached"));
        let started = Instant::now();
        while !reached.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "{barrier} was never reached"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        self.child.kill().expect("SIGKILL");
        self.child.wait().expect("reaped");
        let mut rest = String::new();
        self.reader.read_line(&mut rest).expect("reads to the end");
        assert!(
            rest.is_empty(),
            "killed at {barrier}, the provider must not have answered: {rest}"
        );
    }

    fn kill(mut self) {
        self.child.kill().expect("SIGKILL");
        self.child.wait().expect("reaped");
    }

    fn inspect(&mut self, artifact: &str) -> Value {
        let response = self.call(
            "evidence.inspect",
            &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}}}}"#),
            None,
        );
        response.get("result").cloned().unwrap_or(response)
    }

    fn fetch(&mut self, artifact: &str, digest: &str) -> Value {
        self.call(
            "evidence.fetch",
            &format!(
                r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},"digest":"{digest}"}}"#
            ),
            None,
        )
    }

    /// Every event of this artifact, as `(type, command_id)`.
    fn events_of(&mut self, artifact: &str) -> Vec<(String, String)> {
        let response = self.call("core.events.read", r#"{"limit":1000,"from":"start"}"#, None);
        let items = response
            .get("result")
            .and_then(|r| r.get("items"))
            .and_then(Value::as_array)
            .expect("events")
            .to_vec();
        items
            .iter()
            .filter_map(|item| item.get("event"))
            .filter(|event| {
                event
                    .get("subject")
                    .and_then(|s| s.get("id"))
                    .and_then(Value::as_str)
                    == Some(artifact)
            })
            .map(|event| {
                let text = |name: &str| {
                    event
                        .get(name)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                (text("type"), text("command_id"))
            })
            .collect()
    }
}

/// A command's identity: `command_id|artifact|revision`.
fn command(command_id: &str, artifact: &str, revision: i64) -> String {
    format!("{command_id}|{artifact}|{revision}")
}

/// The envelope for a query or command, with the command digest the provider
/// will recompute, so every step is the real command path.
fn envelope(operation: &str, payload: &str, command: Option<&str>) -> String {
    let Some(command) = command else {
        return format!(
            r#"{{"operation":"{operation}","message_id":"q-{}","payload":{payload}}}"#,
            std::process::id()
        );
    };
    let mut parts = command.split('|');
    let (id, artifact, revision) = (
        parts.next().expect("id"),
        parts.next().expect("artifact"),
        parts.next().expect("revision"),
    );
    let subject = format!(r#"{{"kind":"evidence.artifact","id":"{artifact}"}}"#);
    let text = format!(
        r#"{{"operation":"{operation}","message_id":"m-{id}","command_id":"{id}","dedupe_generation":1,"subject":{subject},"preconditions":[{{"subject":{subject},"revision":{revision}}}],"requires":[],"payload":{payload}}}"#
    );
    let value = cbr_encoding::parse(text.as_bytes()).expect("envelope parses");
    let digest = cbr_encoding::command_digest(&value).expect("intent");
    text.replacen(
        r#""payload""#,
        &format!(r#""command_digest":"{digest}","payload""#),
        1,
    )
}

fn prepare_payload(bytes: &[u8]) -> String {
    format!(
        r#"{{"digest":"{}","size":{},"media_type":"application/octet-stream","producer":{{"producer_id":"crash-matrix"}},"source":{{"kind":"test_report","id":"crash"}},"scope":"local","capture":{{"captured_at":"2030-01-01T00:00:00Z","anchors":[]}},"coverage":{{"completeness":"complete"}},"retention_class":"standard"}}"#,
        cbr_encoding::digest_bytes(bytes),
        bytes.len()
    )
}

fn append_payload(offset: usize, chunk: &[u8]) -> String {
    format!(
        r#"{{"offset":{offset},"data_base64":"{}"}}"#,
        cbr_encoding::encode_base64(chunk)
    )
}

fn content(size: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn ok(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected a result: {response:?}"))
}

fn int(value: &Value, path: &[&str]) -> i64 {
    let mut current = value;
    for name in path {
        current = current
            .get(name)
            .unwrap_or_else(|| panic!("no {name} in {value:?}"));
    }
    match current {
        Value::Int(n) => *n,
        other => panic!("{path:?} is not an integer: {other:?}"),
    }
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for name in path {
        current = current
            .get(name)
            .unwrap_or_else(|| panic!("no {name} in {value:?}"));
    }
    current
        .as_str()
        .unwrap_or_else(|| panic!("{path:?} is not a string"))
}

fn error_code(response: &Value) -> &str {
    response
        .get("error")
        .and_then(|e| e.get("data"))
        .and_then(|d| d.get("code"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected an error: {response:?}"))
}

/// Upload and seal a whole artifact in one call each. Returns the digest.
fn upload(provider: &mut Provider, artifact: &str, bytes: &[u8]) -> String {
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(bytes),
        Some(&command(&format!("prepare-{artifact}"), artifact, 0)),
    ));
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(0, bytes),
        Some(&command(&format!("append-{artifact}-0"), artifact, 1)),
    ));
    let sealed = provider.call(
        "evidence.seal",
        "{}",
        Some(&command(&format!("seal-{artifact}"), artifact, 2)),
    );
    text(ok(&sealed), &["outcome", "digest"]).to_string()
}

/// **Before command commit.** Durable fact: no accepted command. Recovery: the
/// caller retries the same command ID, and it applies once.
#[test]
fn before_commit_nothing_is_accepted_and_the_same_command_applies() {
    let data = Data::new();
    let bytes = content(1500, 1);

    let mut provider = data.start(&[APPEND_BEFORE_COMMIT]);
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(&bytes),
        Some(&command("prepare-a", "a", 0)),
    ));
    provider.kill_at(
        APPEND_BEFORE_COMMIT,
        "evidence.upload.append",
        &append_payload(0, &bytes),
        &command("append-a-0", "a", 1),
    );

    let mut provider = data.start(&[]);
    let inspected = provider.inspect("a");
    assert_eq!(text(&inspected, &["state"]), "staged");
    assert_eq!(int(&inspected, &["received"]), 0, "no bytes were accepted");
    assert_eq!(int(&inspected, &["revision"]), 1, "no revision was taken");
    assert!(
        !provider
            .events_of("a")
            .iter()
            .any(|(kind, _)| kind == "evidence.artifact.appended"),
        "no event was appended"
    );

    let retried = provider.call(
        "evidence.upload.append",
        &append_payload(0, &bytes),
        Some(&command("append-a-0", "a", 1)),
    );
    let result = ok(&retried);
    assert_eq!(
        result.get("replay"),
        Some(&Value::Bool(false)),
        "nothing was bound, so the same command id applies now rather than replaying"
    );
    assert_eq!(int(result, &["acknowledgment", "revision"]), 2);
    assert_eq!(int(result, &["outcome", "received"]), bytes.len() as i64);
    provider.kill();
}

/// **After the provider accepted, before the caller received the
/// acknowledgment** — `SIGKILL` between append and acknowledgment. Durable
/// fact: the provider's deduplication record and result. Recovery: the same
/// command ID replays; no bytes are appended twice.
#[test]
fn after_commit_before_the_acknowledgment_the_retry_replays_and_appends_nothing() {
    let data = Data::new();
    let bytes = content(2500, 2);
    let (first, second) = bytes.split_at(1000);

    let mut provider = data.start(&[APPEND_AFTER_COMMIT]);
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(&bytes),
        Some(&command("prepare-b", "b", 0)),
    ));
    provider.kill_at(
        APPEND_AFTER_COMMIT,
        "evidence.upload.append",
        &append_payload(0, first),
        &command("append-b-0", "b", 1),
    );

    let mut provider = data.start(&[]);
    let inspected = provider.inspect("b");
    assert_eq!(
        int(&inspected, &["received"]),
        first.len() as i64,
        "the unacknowledged append is durable"
    );
    assert_eq!(int(&inspected, &["revision"]), 2);

    let retried = provider.call(
        "evidence.upload.append",
        &append_payload(0, first),
        Some(&command("append-b-0", "b", 1)),
    );
    let result = ok(&retried);
    assert_eq!(
        result.get("replay"),
        Some(&Value::Bool(true)),
        "the retry is answered from the stored result"
    );
    assert_eq!(int(result, &["acknowledgment", "revision"]), 2);
    assert_eq!(int(result, &["outcome", "received"]), first.len() as i64);
    assert_eq!(
        int(&provider.inspect("b"), &["received"]),
        first.len() as i64,
        "and appends nothing a second time"
    );
    let appended: Vec<_> = provider
        .events_of("b")
        .into_iter()
        .filter(|(kind, _)| kind == "evidence.artifact.appended")
        .collect();
    assert_eq!(
        appended,
        vec![(
            "evidence.artifact.appended".to_string(),
            "append-b-0".to_string()
        )],
        "exactly one appended event, naming the command"
    );

    // The upload continues from where the durable record says it is, and the
    // sealed content is exactly the original: nothing was doubled.
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(first.len(), second),
        Some(&command("append-b-1", "b", 2)),
    ));
    let sealed = provider.call("evidence.seal", "{}", Some(&command("seal-b", "b", 3)));
    let digest = text(ok(&sealed), &["outcome", "digest"]).to_string();
    let fetched = provider.fetch("b", &digest);
    let data_base64 = text(ok(&fetched), &["data_base64"]);
    assert!(cbr_encoding::decode_base64(data_base64).expect("base64") == bytes);
    provider.kill();
}

/// **During artifact upload.** Durable fact: an unsealed staging object.
/// Recovery: resume the upload; the staged artifact is never served or cited
/// as sealed evidence.
#[test]
fn a_killed_upload_stays_staged_is_never_served_and_resumes() {
    let data = Data::new();
    let bytes = content(3000, 3);
    let chunks: Vec<&[u8]> = bytes.chunks(1000).collect();
    let digest = cbr_encoding::digest_bytes(&bytes);

    // Killed idle between two appends...
    let mut provider = data.start(&[]);
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(&bytes),
        Some(&command("prepare-c", "c", 0)),
    ));
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(0, chunks[0]),
        Some(&command("append-c-0", "c", 1)),
    ));
    provider.kill();

    // ...and killed again inside the next append, before it commits.
    let provider = data.start(&[APPEND_BEFORE_COMMIT]);
    provider.kill_at(
        APPEND_BEFORE_COMMIT,
        "evidence.upload.append",
        &append_payload(1000, chunks[1]),
        &command("append-c-1", "c", 2),
    );

    let mut provider = data.start(&[]);
    let inspected = provider.inspect("c");
    assert_eq!(text(&inspected, &["state"]), "staged");
    assert_eq!(int(&inspected, &["received"]), 1000);

    // Never served, never sealed, never available as sealed content.
    assert_eq!(error_code(&provider.fetch("c", &digest)), "not_found");
    let early_seal = provider.call(
        "evidence.seal",
        "{}",
        Some(&command("seal-c-early", "c", 2)),
    );
    assert_eq!(error_code(&early_seal), "upload_incomplete");
    assert!(
        !data.object(&digest).exists(),
        "no object exists for an unsealed upload"
    );
    assert!(
        !provider
            .events_of("c")
            .iter()
            .any(|(kind, _)| kind == "evidence.artifact.sealed"),
        "no sealed event"
    );

    // Resume from the durable `received`.
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(1000, chunks[1]),
        Some(&command("append-c-1", "c", 2)),
    ));
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(2000, chunks[2]),
        Some(&command("append-c-2", "c", 3)),
    ));
    let sealed = provider.call("evidence.seal", "{}", Some(&command("seal-c", "c", 4)));
    assert_eq!(text(ok(&sealed), &["outcome", "digest"]), digest);
    let fetched = provider.fetch("c", &digest);
    assert!(
        cbr_encoding::decode_base64(text(ok(&fetched), &["data_base64"])).expect("base64") == bytes
    );
    provider.kill();
}

/// **The orphan object** (STORAGE section 2): the seal publishes its object
/// and verifies it from disk, and is killed before the row naming it commits.
/// Durable fact: an unreferenced object and a still-staged artifact — never a
/// successful receipt. Recovery: the object is collected at start, and a
/// retried seal publishes again from the staged bytes.
#[test]
fn an_object_published_before_its_seal_row_is_collected_and_never_served() {
    let data = Data::new();
    let bytes = content(2048, 4);
    let digest = cbr_encoding::digest_bytes(&bytes);

    let mut provider = data.start(&[SEAL_AFTER_OBJECT]);
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(&bytes),
        Some(&command("prepare-d", "d", 0)),
    ));
    ok(&provider.call(
        "evidence.upload.append",
        &append_payload(0, &bytes),
        Some(&command("append-d-0", "d", 1)),
    ));
    provider.kill_at(
        SEAL_AFTER_OBJECT,
        "evidence.seal",
        "{}",
        &command("seal-d", "d", 2),
    );

    // The kill really landed between the two writes: the object is on disk,
    // complete and verified, and nothing names it yet.
    let object = data.object(&digest);
    assert!(
        std::fs::read(&object).is_ok_and(|on_disk| on_disk == bytes),
        "the object was published before the kill"
    );

    let mut provider = data.start(&[]);
    assert!(!object.exists(), "the orphan was collected at start");
    let inspected = provider.inspect("d");
    assert_eq!(
        text(&inspected, &["state"]),
        "staged",
        "no receipt for the seal"
    );
    assert_eq!(int(&inspected, &["revision"]), 2);
    assert_eq!(int(&inspected, &["received"]), bytes.len() as i64);
    assert_eq!(error_code(&provider.fetch("d", &digest)), "not_found");
    assert!(
        !provider
            .events_of("d")
            .iter()
            .any(|(kind, _)| kind == "evidence.artifact.sealed")
    );

    let retried = provider.call("evidence.seal", "{}", Some(&command("seal-d", "d", 2)));
    let result = ok(&retried);
    assert_eq!(
        result.get("replay"),
        Some(&Value::Bool(false)),
        "the seal was never bound, so it applies now"
    );
    assert_eq!(text(result, &["outcome", "digest"]), digest);
    assert!(
        std::fs::read(&object).is_ok_and(|on_disk| on_disk == bytes),
        "published again"
    );
    let fetched = provider.fetch("d", &digest);
    assert!(
        cbr_encoding::decode_base64(text(ok(&fetched), &["data_base64"])).expect("base64") == bytes
    );
    provider.kill();
}

/// **During collection.** The purge commits its row first and deletes the
/// object after. Killed between them, the durable fact is the tombstone: the
/// artifact is recorded as purged. Recovery rechecks the roots before
/// deleting: an object no other sealed artifact names is deleted at start;
/// one another artifact still names is kept and still served for that one.
#[test]
fn a_purge_killed_before_deletion_is_finished_at_restart_and_rechecks_roots() {
    let data = Data::new();
    let shared = content(1024, 5);
    let unique = content(1024, 6);

    let mut provider = data.start(&[]);
    let shared_digest = upload(&mut provider, "x", &shared);
    assert_eq!(upload(&mut provider, "y", &shared), shared_digest);
    let unique_digest = upload(&mut provider, "z", &unique);
    provider.kill();

    // Purge the artifact whose bytes nothing else shares, killed after commit.
    let provider = data.start(&[PURGE_AFTER_COMMIT]);
    provider.kill_at(
        PURGE_AFTER_COMMIT,
        "evidence.purge",
        "{}",
        &command("purge-z", "z", 3),
    );
    assert!(
        data.object(&unique_digest).exists(),
        "the kill landed before the deletion"
    );

    let mut provider = data.start(&[PURGE_AFTER_COMMIT]);
    assert!(
        !data.object(&unique_digest).exists(),
        "the interrupted deletion was finished at start"
    );
    let inspected = provider.inspect("z");
    assert_eq!(text(&inspected, &["availability", "state"]), "purged");
    let fetched = provider.fetch("z", &unique_digest);
    assert_eq!(text(ok(&fetched), &["availability", "state"]), "purged");
    assert_eq!(text(ok(&fetched), &["data_base64"]), "");
    let replayed = provider.call("evidence.purge", "{}", Some(&command("purge-z", "z", 3)));
    assert_eq!(ok(&replayed).get("replay"), Some(&Value::Bool(true)));
    assert_eq!(text(ok(&replayed), &["outcome", "availability"]), "purged");

    // Purge one of two artifacts sharing an object, killed after commit.
    provider.kill_at(
        PURGE_AFTER_COMMIT,
        "evidence.purge",
        "{}",
        &command("purge-x", "x", 3),
    );

    let mut provider = data.start(&[]);
    assert_eq!(
        text(&provider.inspect("x"), &["availability", "state"]),
        "purged"
    );
    assert!(
        data.object(&shared_digest).exists(),
        "an object another sealed artifact names is never collected"
    );
    let fetched = provider.fetch("y", &shared_digest);
    assert_eq!(text(ok(&fetched), &["availability", "state"]), "available");
    assert!(
        cbr_encoding::decode_base64(text(ok(&fetched), &["data_base64"])).expect("base64")
            == shared
    );
    provider.kill();
}

/// The collection pass deletes objects no committed row names, so it is only
/// safe if no other process is between publishing an object and naming it.
/// A second provider over the same data directory therefore refuses to start,
/// and a killed provider leaves nothing held.
#[test]
fn a_second_provider_over_the_same_data_directory_refuses_to_start() {
    let data = Data::new();
    let second = || {
        Command::new(binary())
            .arg("--data-dir")
            .arg(data.path().join("data"))
            .arg("--config")
            .arg(data.path().join("config.json"))
            .stdin(Stdio::null())
            .output()
            .expect("runs")
    };

    let provider = data.start(&[]);
    let refused = second();
    assert!(
        !refused.status.success(),
        "a second provider must not start"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("another provider is serving"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );

    // SIGKILL releases the lock: the next start is not refused.
    provider.kill();
    let mut provider = data.start(&[]);
    ok(&provider.call(
        "evidence.upload.prepare",
        &prepare_payload(b"after"),
        Some(&command("prepare-e", "e", 0)),
    ));
    provider.kill();
}

// ---------------------------------------------------------------------------
// The envelope's own rows (m4a)
// ---------------------------------------------------------------------------
//
// **Six rows, not three.** A call is two sends — the count and the
// completion — and the first version of this scripted one answer, which the
// count consumed, so the process paused at the count's first boundary in
// every row while the test believed it had reached the completion. Each row
// now names its call, and each asserts **which call's reservation is on
// disk, by request id**, so a row cannot pass at the wrong boundary again.
//
// | Row | Boundary | Test |
// |---|---|---|
// | Count reserved, nothing sent | `model.count.after_reservation` | [`a_kill_after_the_count_reservation_leaves_the_spend_counted`] |
// | Count sent, not reconciled | `model.count.after_send` | [`a_kill_after_the_count_send_leaves_the_estimate_counted`] |
// | Inside the count's reconciliation | `model.count.during_reconciliation` | [`a_kill_during_the_count_reconciliation_counts_once`] |
// | Completion reserved, nothing sent | `model.completion.after_reservation` | [`a_kill_after_the_completion_reservation_leaves_both_counted`] |
// | Completion sent, not reconciled | `model.completion.after_send` | [`a_kill_after_the_completion_send_leaves_the_estimate_counted`] |
// | Inside the completion's reconciliation | `model.completion.during_reconciliation` | [`a_kill_during_the_completion_reconciliation_counts_once`] |
//
// The property after every one is the same and is deliberately one-sided:
// **the spend is counted at least once and is never zero.** A ledger that
// forgets a spend overspends somebody else's quota; one that counts it twice
// only refuses a call it could have allowed.

/// Launch a provider whose `model.fake` control makes it perform one call
/// through the ledger, pausing at `barrier`. It pauses before it serves, so
/// nothing negotiates with it: the test waits for the marker and kills it.
fn start_paused_at(data: &Data, barrier: &str) -> Child {
    let barrier_dir = data.path().join("barriers");
    let config = data.path().join("model-config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"test_barriers":{{"directory":"{}","enabled":["{barrier}"]}},"model":{{"job":"m4a","request":"call","body":"{{\"max_tokens\":64}}","answer":"usage:5000","generation":64}}}}"#,
            barrier_dir.display()
        ),
    )
    .expect("config");
    Command::new(binary())
        .arg("--data-dir")
        .arg(data.path().join("data"))
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("provider starts")
}

fn wait_for_marker(data: &Data, barrier: &str) {
    let marker = data
        .path()
        .join("barriers")
        .join(format!("{barrier}.reached"));
    let started = Instant::now();
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the provider never reached {barrier}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What the ledger says, read from the store the killed process left behind:
/// the request id, the kind, the tokens and the estimate.
/// Ledger rows that are **charges**. The note recording which evidence
/// admitted a call sits beside them and is not one, so a row count that
/// includes it is counting two different things.
fn ledger_spend(data: &Data) -> Vec<(String, String, i64, i64)> {
    ledger_rows(data)
        .into_iter()
        .filter(|(_, kind, _, _)| !kind.starts_with("admitted_"))
        .collect()
}

fn ledger_rows(data: &Data) -> Vec<(String, String, i64, i64)> {
    let connection = rusqlite::Connection::open(data.path().join("data").join("cbr.sqlite"))
        .expect("opens the store");
    let mut statement = connection
        .prepare("SELECT request, kind, tokens, estimate FROM model_ledger ORDER BY id")
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("queries");
    rows.map(|row| row.expect("row")).collect()
}

/// Every row that counts as spend, and their total.
fn counted(rows: &[(String, String, i64, i64)]) -> i64 {
    rows.iter()
        .filter(|(_, kind, _, _)| {
            matches!(
                kind.as_str(),
                "reservation" | "usage" | "unknown" | "provider_exhausted" | "not_sent"
            )
        })
        .map(|(_, _, tokens, _)| tokens)
        .sum()
}

fn kill(mut child: Child) {
    child.kill().expect("kills");
    child.wait().expect("reaps");
}

/// Run one row: kill at `barrier` and hand the rows back.
fn row(barrier: &str) -> Vec<(String, String, i64, i64)> {
    let data = Data::new();
    let child = start_paused_at(&data, barrier);
    wait_for_marker(&data, barrier);
    kill(child);
    let rows = ledger_spend(&data);
    assert!(
        counted(&rows) > 0,
        "the spend is counted, never zero, at {barrier}: {rows:?}"
    );
    rows
}

#[test]
fn a_kill_after_the_count_reservation_leaves_the_spend_counted() {
    // The reservation is written *before* anything is sent, so this is the
    // moment the design exists for: the process dies holding a reservation
    // for a call that never happened, and the spend is still counted.
    let rows = row("model.count.after_reservation");
    assert_eq!(rows.len(), 1, "only the count has been reserved: {rows:?}");
    assert_eq!(rows[0].0, "call.count", "and it is the count's: {rows:?}");
    assert_eq!(rows[0].1, "reservation", "nothing settled it: {rows:?}");
}

#[test]
fn a_kill_after_the_count_send_leaves_the_estimate_counted() {
    let rows = row("model.count.after_send");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "call.count");
    assert_eq!(rows[0].1, "reservation", "still holding its estimate");
    assert_eq!(rows[0].2, rows[0].3, "which is what it reserved");
}

#[test]
fn a_kill_during_the_count_reconciliation_counts_once() {
    let rows = row("model.count.during_reconciliation");
    let spends = rows
        .iter()
        .filter(|(_, kind, _, _)| kind == "reservation" || kind == "usage")
        .count();
    assert_eq!(spends, 1, "counted once, not twice and not none: {rows:?}");
    assert_eq!(rows[0].0, "call.count");
}

#[test]
fn a_kill_after_the_completion_reservation_leaves_both_counted() {
    // The row the first version of this could never reach: the count has
    // settled and the completion is reserved and unsent.
    let rows = row("model.completion.after_reservation");
    assert_eq!(rows.len(), 2, "the count and the completion: {rows:?}");
    assert_eq!(rows[0].0, "call.count");
    assert_eq!(rows[0].1, "usage", "the count settled: {rows:?}");
    assert_eq!(rows[1].0, "call", "the completion is reserved: {rows:?}");
    assert_eq!(rows[1].1, "reservation");
    assert!(
        rows[1].2 > rows[0].2,
        "and its reservation covers the generation, so it is the larger: {rows:?}"
    );
}

#[test]
fn a_kill_after_the_completion_send_leaves_the_estimate_counted() {
    let rows = row("model.completion.after_send");
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[1].0, "call");
    assert_eq!(rows[1].1, "reservation", "sent, not reconciled");
    assert_eq!(rows[1].2, rows[1].3, "the estimate stands, over-counting");
}

#[test]
fn a_kill_during_the_completion_reconciliation_counts_once() {
    let rows = row("model.completion.during_reconciliation");
    let completion: Vec<_> = rows
        .iter()
        .filter(|(request, ..)| request == "call")
        .collect();
    assert_eq!(
        completion.len(),
        1,
        "the completion is one row, never two: {rows:?}"
    );
    assert!(
        matches!(completion[0].1.as_str(), "reservation" | "usage"),
        "either still its estimate or already its usage: {rows:?}"
    );
}

#[test]
fn with_no_model_configured_nothing_is_reserved_and_the_ledger_stays_empty() {
    // **Negative control 1, at m4a rather than at m4e.** The golden packet
    // digest already says the deterministic path is unchanged when no model
    // is configured; this says the envelope is untouched too. A ledger row
    // written without a model would mean a call happened that nobody asked
    // for, which is what "background spend is zero" forbids.
    let data = Data::new();
    let provider = data.start(&[]);
    drop(provider);
    assert!(
        ledger_spend(&data).is_empty(),
        "no model, no reservation: {:?}",
        ledger_spend(&data)
    );
}

#[test]
fn a_production_configuration_refuses_the_model_control() {
    // The fake transport is the only transport in this build, and it must be
    // unreachable outside a conformance launch. Refused, not ignored:
    // silently dropping it would leave an operator believing a model was
    // configured when none was.
    let data = Data::new();
    let config = data.path().join("production.json");
    std::fs::write(
        &config,
        r#"{"format":"cbr-config/1","principal":"owner","model":{"job":"j","request":"r","body":"{}","answer":"usage:1"}}"#,
    )
    .expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(data.path().join("data"))
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert!(!output.status.success(), "a production launch refuses it");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains("model"),
        "and says which control it refused: {message}"
    );
}
