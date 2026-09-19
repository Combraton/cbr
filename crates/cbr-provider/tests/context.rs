//! Context behaviour no conformance fixture shows, driven against the real
//! provider binary.
//!
//! - **A publication interrupted at a separate evidence provider replays.**
//!   The seal is held at the evidence provider past the context provider's
//!   peer timeout, the clock moves, and the retried publication must replay
//!   the steps that already applied rather than conflict with them: the
//!   capture instant of the first attempt is kept (CONTEXT section 12).
//! - **A packet is never published before its local seal commits.** The
//!   provider is killed after the packet's object is published and before the
//!   batch that names it commits; after restart there is exactly one
//!   publication and its bytes are served (STORAGE section 2).
//! - **A request nothing prepares is published unmet at its deadline**, never
//!   left preparing and never satisfied.
//! - **A correction after publication is a read-time fact.** The published
//!   revision keeps the authority revision it was prepared against; reading it
//!   reports the correction in `invalidated_items` and the newer revision in
//!   `superseded_by`, and the current revision reports neither (CONTEXT
//!   section 8).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
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

fn parse(text: &str) -> Value {
    cbr_encoding::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{e:?}: {text}"))
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut current = value;
    for name in path {
        current = current
            .get(name)
            .unwrap_or_else(|| panic!("no {name} along {path:?} in {value:?}"));
    }
    current
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    at(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("{path:?} is not a string in {value:?}"))
}

fn result(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected a result: {response:?}"))
}

/// A query or command envelope, with the command digest the provider will
/// recompute. `command` is `(command_id, (kind, id), revision)`.
fn envelope(
    operation: &str,
    command: Option<(&str, (&str, &str), i64)>,
    payload: &str,
    grant: Option<&str>,
    message: i64,
) -> String {
    let grant = grant.map_or(String::new(), |g| format!(r#","grant":"{g}""#));
    let Some((command_id, (kind, id), revision)) = command else {
        return format!(
            r#"{{"operation":"{operation}","message_id":"q-{message}"{grant},"payload":{payload}}}"#
        );
    };
    let subject = format!(r#"{{"kind":"{kind}","id":"{id}"}}"#);
    let text = format!(
        r#"{{"operation":"{operation}","message_id":"m-{message}","command_id":"{command_id}","dedupe_generation":1,"subject":{subject},"preconditions":[{{"subject":{subject},"revision":{revision}}}],"requires":[]{grant},"payload":{payload}}}"#
    );
    let digest = cbr_encoding::command_digest(&parse(&text)).expect("intent");
    text.replacen(
        r#""payload""#,
        &format!(r#""command_digest":"{digest}","payload""#),
        1,
    )
}

/// Core with events, evidence, and context with `features`.
fn context_profiles(features: &[&str]) -> String {
    let features = features
        .iter()
        .map(|f| format!(r#""{f}""#))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"[{{"name":"core","majors":[1],"required":true,"required_features":["core.events"],"optional_features":[]}},{{"name":"context","majors":[1],"required":true,"required_features":[{features}],"optional_features":[]}},{{"name":"evidence","majors":[1],"required":true,"required_features":[],"optional_features":[]}}]"#
    )
}

/// A provider over stdio, acting as the configuration's principal.
struct ContextProvider {
    child: Child,
    reader: BufReader<ChildStdout>,
    next: i64,
}

impl ContextProvider {
    fn start(directory: &Path, config: &str) -> Self {
        Self::start_with(directory, config, &["context.required_before_start"])
    }

    fn start_with(directory: &Path, config: &str, features: &[&str]) -> Self {
        let path = directory.join("context-config.json");
        std::fs::write(&path, config).expect("config");
        let mut child = Command::new(binary())
            .arg("--data-dir")
            .arg(directory.join("context-data"))
            .arg("--config")
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(
                std::fs::File::create(directory.join("context-stderr.log")).expect("stderr file"),
            )
            .spawn()
            .expect("provider starts");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        let mut provider = Self {
            child,
            reader,
            next: 1,
        };
        let negotiated = provider.call(
            "core.negotiate",
            None,
            &format!(
                r#"{{"caller":{{"name":"context-tests","version":"1"}},"receive_limits":{{"max_frame_bytes":1048576}},"profiles":{}}}"#,
                context_profiles(features)
            ),
        );
        result(&negotiated);
        provider
    }

    fn send(&mut self, operation: &str, command: Option<(&str, (&str, &str), i64)>, payload: &str) {
        let id = self.next;
        self.next += 1;
        let params = envelope(operation, command, payload, None, id);
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{operation}","params":{params}}}"#
        )
        .expect("writes");
        stdin.flush().expect("flushes");
    }

    fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("reads");
        (!line.is_empty()).then(|| parse(line.trim_end()))
    }

    fn call(
        &mut self,
        operation: &str,
        command: Option<(&str, (&str, &str), i64)>,
        payload: &str,
    ) -> Value {
        self.send(operation, command, payload);
        self.read().expect("a response")
    }

    fn submit(&mut self, request: &str, deadline: &str) -> Value {
        let payload = format!(
            r#"{{"consumer":{{"task":"fix the build","principal":"owner"}},"basis":{{"repositories":[{{"id":"repo-a","tree":"tree-1","workspace":"clean","dirty":null}}],"completeness":"complete"}},"items":[{{"item_id":"i-1","selector":{{"kind":"path","value":"src/main.rs"}},"obligation":"required_before_start","reliance":"binding","selected_by":"owner","check":{{"kind":"source_included","repository":"repo-a","path":"src/main.rs"}}}}],"limits":{{"deadline":"{deadline}","investigation":{{"units":"queries","amount":10}},"output_capacity":{{"units":"bytes","amount":4096}}}}}}"#
        );
        self.call(
            "context.request.submit",
            Some((
                &format!("submit-{request}"),
                ("context.request", request),
                0,
            )),
            &payload,
        )
    }

    /// A request with the items, fallback and deadline a test needs, rather
    /// than the one shape `submit` builds.
    fn submit_with(&mut self, request: &str, items: &str, fallback: &str, deadline: &str) -> Value {
        let payload = format!(
            r#"{{"consumer":{{"task":"fix the build","principal":"owner"}},"basis":{{"repositories":[{{"id":"repo-a","tree":"tree-1","workspace":"clean","dirty":null}}],"completeness":"complete"}},"items":[{items}],"fallback":"{fallback}","limits":{{"deadline":"{deadline}","investigation":{{"units":"queries","amount":10}},"output_capacity":{{"units":"bytes","amount":4096}}}}}}"#
        );
        self.call(
            "context.request.submit",
            Some((
                &format!("submit-{request}"),
                ("context.request", request),
                0,
            )),
            &payload,
        )
    }

    fn inspect(&mut self, request: &str) -> Value {
        let response = self.call(
            "context.request.inspect",
            None,
            &format!(r#"{{"request":"{request}"}}"#),
        );
        result(&response).clone()
    }

    fn kill(mut self) {
        self.child.kill().expect("SIGKILL");
        self.child.wait().expect("reaped");
    }
}

impl Drop for ContextProvider {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One authenticated session on a provider's Unix socket.
struct Socket {
    reader: BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
    next: i64,
}

impl Socket {
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
            .set_read_timeout(Some(Duration::from_secs(40)))
            .expect("timeout");
        let mut session = Self {
            reader: BufReader::new(stream.try_clone().expect("clone")),
            writer: stream,
            next: 1,
        };
        result(&session.call(
            "core.authenticate",
            None,
            &format!(r#"{{"credential":"{credential}"}}"#),
            None,
        ));
        result(&session.call(
            "core.negotiate",
            None,
            r#"{"caller":{"name":"context-tests","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.grants"],"optional_features":[]},{"name":"evidence","majors":[1],"required":true,"required_features":[],"optional_features":[]}]}"#,
            None,
        ));
        session
    }

    fn call(
        &mut self,
        operation: &str,
        command: Option<(&str, (&str, &str), i64)>,
        payload: &str,
        grant: Option<&str>,
    ) -> Value {
        let id = self.next;
        self.next += 1;
        let params = envelope(operation, command, payload, grant, id);
        writeln!(
            self.writer,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{operation}","params":{params}}}"#
        )
        .expect("writes");
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("reads");
            assert!(!line.is_empty(), "the provider closed the connection");
            let frame = parse(line.trim_end());
            if frame.get("id") == Some(&Value::Int(id)) {
                return frame;
            }
        }
    }
}

fn wait_for(path: &Path, what: &str) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{what} never happened"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Replace the controlled clock the way the runner does: atomically.
fn set_clock(path: &Path, instant: &str) {
    let staged = path.with_extension("tmp");
    std::fs::write(&staged, instant).expect("clock");
    std::fs::rename(&staged, path).expect("clock moves");
}

const SCRIPT: &str = r#"{"scripts":{"r-1":[{"section":{"section_id":"s-1","item_id":"i-1","label":"source_inspected","content":"fn main() {}","source":{"repository":"repo-a","path":"src/main.rs","tree":"tree-1"}}},{"publish":{}}]}}"#;

#[test]
fn a_publication_interrupted_at_the_evidence_provider_replays_after_the_clock_moves() {
    let directory = tempfile::tempdir().expect("temp dir");
    let sockets = directory.path().join("s");
    std::fs::create_dir(&sockets).expect("socket dir");
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
    let socket = sockets.join("evd.sock");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");

    let owner = format!("ccred1.owner.{}", "A".repeat(43));
    let service = format!("ccred1.ctx-service.{}", "B".repeat(43));
    let seal_barrier = "evidence.seal.after_object_published";
    let evd_config = directory.path().join("evd.json");
    std::fs::write(
        &evd_config,
        format!(
            r#"{{"format":"combraton-conformance-config/1","provider_id":"evidence-1","principal":"owner","authority_principals":["owner"],"clock":{{"file":"{}"}},"credentials":[{{"credential":"{owner}"}},{{"credential":"{service}"}}],"test_barriers":{{"directory":"{}","enabled":["{seal_barrier}"]}}}}"#,
            clock.display(),
            barriers.display()
        ),
    )
    .expect("evd config");
    let mut evd = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("evd-data"))
        .arg("--config")
        .arg(&evd_config)
        .arg("--socket")
        .arg(&socket)
        // A socket provider serves until its standard input closes.
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("evidence provider starts");

    let mut evd_owner = Socket::connect(&socket, &owner);
    result(&evd_owner.call(
        "core.grant.issue",
        Some(("issue-g-pub", ("core.grant", "g-pub"), 0)),
        r#"{"holder":"ctx-service","audience":"evidence-1","rights":["evidence.publish"],"resources":[{"kind":"evidence.artifact","id_prefix":"packet."}],"delegation":{"allowed":false,"max_depth":0}}"#,
        None,
    ));

    let context = format!(
        r#"{{"scripts":{},"evidence_provider":{{"provider_id":"evidence-1","socket":"{}","credential":"{service}","grant":"g-pub"}}}}"#,
        &SCRIPT[r#"{"scripts":"#.len()..SCRIPT.len() - 1],
        socket.display()
    );
    let mut ctx = ContextProvider::start(
        directory.path(),
        &format!(
            r#"{{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"],"clock":{{"file":"{}"}},"context":{context}}}"#,
            clock.display()
        ),
    );
    let submitted = ctx.submit("r-1", "2030-01-01T01:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");

    // The tick before this read prepares and appends at the evidence
    // provider, whose seal is then held past the peer timeout.
    let first = ctx.inspect("r-1");
    wait_for(
        &barriers.join(format!("{seal_barrier}.reached")),
        "the seal at the evidence provider",
    );
    assert_eq!(text(&first, &["state"]), "preparing");
    assert_eq!(at(&first, &["packets"]).as_array().map(<[_]>::len), Some(0));

    // Time moves, and only then does the held seal complete.
    set_clock(&clock, "2030-01-01T00:05:00Z");
    std::fs::write(barriers.join(format!("{seal_barrier}.release")), b"").expect("release");
    let sealed = loop {
        let response = evd_owner.call(
            "evidence.inspect",
            None,
            r#"{"artifact":{"kind":"evidence.artifact","id":"packet.r-1.1"}}"#,
            None,
        );
        let inspected = result(&response).clone();
        if text(&inspected, &["state"]) == "sealed" {
            break inspected;
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let retried = ctx.inspect("r-1");
    assert_eq!(
        text(&retried, &["state"]),
        "ready",
        "the retried publication replays the steps that applied: {retried:?}"
    );
    let reference = at(&retried, &["packets"]).as_array().expect("packets")[0].clone();
    assert_eq!(
        text(&reference, &["reference", "artifact", "provider"]),
        "evidence-1"
    );
    assert_eq!(
        text(&reference, &["reference", "artifact", "digest"]),
        text(&sealed, &["descriptor", "digest"])
    );
    assert_eq!(
        text(&sealed, &["descriptor", "capture", "captured_at"]),
        "2030-01-01T00:00:00Z",
        "the descriptor is the first attempt's, so the retry could replay it"
    );
    assert_eq!(
        text(&sealed, &["descriptor", "producer", "principal"]),
        "ctx-service"
    );

    drop(ctx);
    let _ = evd.kill();
    let _ = evd.wait();

    // The failed attempt was diagnosed, and nothing it wrote names the peer
    // credential (CORE section 18.1).
    let diagnostics =
        std::fs::read_to_string(directory.path().join("context-stderr.log")).expect("stderr");
    assert!(
        diagnostics.contains("not sealed at the evidence provider"),
        "{diagnostics}"
    );
    let secret = service.rsplit('.').next().expect("secret");
    assert!(
        !diagnostics.contains(secret),
        "a diagnostic carries the peer credential"
    );
}

#[test]
fn a_packet_is_never_published_before_its_local_seal_commits() {
    let directory = tempfile::tempdir().expect("temp dir");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let barrier = "context.packet.after_object_published";
    let config = |enabled: &str| {
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"test_barriers":{{"directory":"{}","enabled":[{enabled}]}},"context":{SCRIPT}}}"#,
            barriers.display()
        )
    };

    let mut ctx = ContextProvider::start(directory.path(), &config(&format!(r#""{barrier}""#)));
    let submitted = ctx.submit("r-1", "2100-01-01T00:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");
    ctx.send("context.request.inspect", None, r#"{"request":"r-1"}"#);
    wait_for(
        &barriers.join(format!("{barrier}.reached")),
        "the packet object's publication",
    );
    let mut killed = ctx;
    killed.child.kill().expect("SIGKILL");
    killed.child.wait().expect("reaped");
    assert!(
        killed.read().is_none(),
        "killed before the batch committed, the provider must not have answered"
    );
    drop(killed);

    let mut restarted = ContextProvider::start(directory.path(), &config(""));
    let request = restarted.inspect("r-1");
    assert_eq!(text(&request, &["state"]), "ready", "{request:?}");
    let packets = at(&request, &["packets"])
        .as_array()
        .expect("packets")
        .to_vec();
    assert_eq!(packets.len(), 1, "exactly one publication: {packets:?}");
    let digest = text(&packets[0], &["reference", "artifact", "digest"]).to_string();
    let fetched = restarted.call(
        "evidence.fetch",
        None,
        &format!(
            r#"{{"artifact":{{"kind":"evidence.artifact","id":"packet.r-1.1"}},"digest":"{digest}"}}"#
        ),
    );
    let data = text(result(&fetched), &["data_base64"]);
    let bytes = cbr_encoding::decode_base64(data).expect("base64");
    assert_eq!(cbr_encoding::digest_bytes(&bytes), digest);

    let events = restarted.call("core.events.read", None, r#"{"from":"start","limit":1000}"#);
    let published: Vec<String> = at(result(&events), &["items"])
        .as_array()
        .expect("items")
        .iter()
        .map(|item| text(item, &["event", "type"]).to_string())
        .filter(|t| t == "context.packet.published" || t == "evidence.artifact.sealed")
        .collect();
    assert_eq!(
        published,
        vec!["evidence.artifact.sealed", "context.packet.published"],
        "one seal and one publication, the seal first"
    );
    restarted.kill();
}

#[test]
fn a_request_nothing_prepares_is_published_unmet_at_its_deadline() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start(
        directory.path(),
        r#"{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"clock":{"fixed":"2030-01-01T00:00:00Z"}}"#,
    );
    let submitted = ctx.submit("r-1", "2030-01-01T00:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");
    let request = ctx.inspect("r-1");
    assert_eq!(text(&request, &["state"]), "unmet", "{request:?}");
    let item = &at(&request, &["items"]).as_array().expect("items")[0];
    assert_eq!(text(item, &["result"]), "unmet");
    assert_eq!(text(item, &["reason"]), "deadline_passed");
    assert_eq!(text(item, &["obligation"]), "required_before_start");
    ctx.kill();
}

#[test]
fn a_correction_after_publication_is_reported_at_the_read_beside_supersession() {
    let directory = tempfile::tempdir().expect("temp dir");
    let script = r#"{"scripts":{"r-1":[
        {"section":{"section_id":"s-old","item_id":"i-rule","label":"binding","content":"retries are unlimited","authority_revision":1}},
        {"publish":{}},
        {"correction":{"item_id":"i-rule","authority_revision":2}},
        {"section":{"section_id":"s-late","item_id":"i-rule","label":"binding","content":"derived late from the old rule","authority_revision":1}},
        {"section":{"section_id":"s-new","item_id":"i-rule","label":"binding","content":"retries are capped","authority_revision":2}},
        {"publish":{}}]}}"#;
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"context":{script}}}"#
        ),
        &["context.required_before_start", "context.updates"],
    );
    let digest = format!("sha256:{}", "0".repeat(64));
    let submitted = ctx.call(
        "context.request.submit",
        Some(("submit-r-1", ("context.request", "r-1"), 0)),
        &format!(
            r#"{{"consumer":{{"task":"t","principal":"owner"}},"basis":{{"repositories":[{{"id":"repo-a","tree":"tree-1","workspace":"clean","dirty":null}}],"completeness":"complete"}},"items":[{{"item_id":"i-rule","selector":{{"kind":"path","value":"p"}},"obligation":"required_before_start","reliance":"binding","selected_by":"owner","check":{{"kind":"authority_content_included"}}}}],"limits":{{"deadline":"2100-01-01T00:00:00Z","investigation":{{"units":"queries","amount":10}},"output_capacity":{{"units":"bytes","amount":4096}}}},"authority_content":[{{"item_id":"i-rule","evidence":{{"artifact":{{"kind":"evidence.artifact","id":"ev-rule"}},"digest":"{digest}"}},"authority_revision":1}}]}}"#
        ),
    );
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");
    let request = ctx.inspect("r-1");
    assert_eq!(text(&request, &["state"]), "ready", "{request:?}");

    let first = ctx.call(
        "context.packet.inspect",
        None,
        r#"{"packet":"r-1","revision":1}"#,
    );
    let first = result(&first);
    assert_eq!(at(first, &["current"]), &Value::Bool(false));
    assert_eq!(at(first, &["superseded_by", "revision"]), &Value::Int(2));
    let invalidated = at(first, &["invalidated_items"]).as_array().expect("array");
    assert_eq!(invalidated.len(), 1, "{invalidated:?}");
    assert_eq!(text(&invalidated[0], &["item_id"]), "i-rule");
    assert_eq!(at(&invalidated[0], &["authority_revision"]), &Value::Int(2));
    // The published facts themselves are unchanged: revision 1 was prepared
    // against authority revision 1.
    assert_eq!(
        at(
            &at(first, &["authority"]).as_array().expect("authority")[0],
            &["authority_revision"]
        ),
        &Value::Int(1)
    );

    let second = ctx.call(
        "context.packet.inspect",
        None,
        r#"{"packet":"r-1","revision":2}"#,
    );
    let second = result(&second);
    assert_eq!(at(second, &["current"]), &Value::Bool(true));
    // Content derived from the corrected revision is never current, whether it
    // was prepared before the correction or after it.
    for section in at(second, &["sections"]).as_array().expect("sections") {
        let stale = text(section, &["section_id"]) != "s-new";
        assert_eq!(
            at(section, &["historical"]),
            &Value::Bool(stale),
            "{section:?}"
        );
        assert_eq!(
            text(section, &["label"]),
            if stale { "stale" } else { "binding" }
        );
    }
    assert!(second.get("superseded_by").is_none(), "{second:?}");
    assert_eq!(
        at(second, &["invalidated_items"])
            .as_array()
            .map(<[_]>::len),
        Some(0)
    );
    ctx.kill();
}

/// One item of a request, as J8 needs them: a path check nothing will
/// satisfy, so the only thing that can resolve the item is the deadline.
fn unsatisfiable(item: &str, obligation: &str) -> String {
    format!(
        r#"{{"item_id":"{item}","selector":{{"kind":"path","value":"src/{item}.rs"}},"obligation":"{obligation}","reliance":"evidence","selected_by":"owner","check":{{"kind":"source_included","repository":"repo-a","path":"src/{item}.rs"}}}}"#
    )
}

/// **J8.** A required item is still unmet when the deadline passes; it stays
/// unmet, and advisory items follow the fallback the request declared.
///
/// The three behaviours are separated deliberately. `proceed_with_gap`
/// publishes rather than waiting, so its advisory item is degraded straight
/// away. `wait_until_deadline` holds the publication while an advisory item
/// is unsatisfied, so the request is still preparing at the same instant.
/// When the deadline does pass, the required item is `unmet` with
/// `deadline_passed` and the advisory one is `degraded` with the same
/// reason: expiry is not evidence, and it is not consent either — the packet
/// it publishes carries no citation and no claim for those items, and the
/// request is never `ready`.
#[test]
fn j8_a_deadline_leaves_required_items_unmet_and_advisory_items_at_their_fallback() {
    let directory = tempfile::tempdir().expect("temp dir");
    let script = r#"{"default_script":[{"publish":{}}]}"#;
    let config = |now: &str| {
        format!(
            r#"{{"format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],"clock":{{"fixed":"{now}"}},"context":{script}}}"#
        )
    };
    let before = "2030-01-01T00:00:00Z";
    let deadline = "2030-01-01T00:10:00Z";
    let after = "2030-01-01T00:10:01Z";

    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &config(before),
        &["context.required_before_start", "context.advisory"],
    );

    // `proceed_with_gap`: the publication happens now, and the advisory item
    // is degraded rather than held.
    let gap = ctx.submit_with(
        "r-gap",
        &unsatisfiable("advisory", "advisory"),
        "proceed_with_gap",
        deadline,
    );
    assert_eq!(text(result(&gap), &["outcome", "state"]), "preparing");
    let request = ctx.inspect("r-gap");
    assert_eq!(text(&request, &["state"]), "partial", "{request:?}");
    let items = at(&request, &["items"]).as_array().expect("items").to_vec();
    assert_eq!(text(&items[0], &["result"]), "degraded", "{items:?}");
    assert!(
        !text(&items[0], &["reason"]).is_empty(),
        "a degraded item carries its reason: {items:?}"
    );

    // `wait_until_deadline`: the same instant, and this one is still
    // preparing, because its advisory item is unsatisfied.
    let items = format!(
        "{},{}",
        unsatisfiable("required", "required_before_start"),
        unsatisfiable("optional", "advisory")
    );
    let waiting = ctx.submit_with("r-wait", &items, "wait_until_deadline", deadline);
    assert_eq!(text(result(&waiting), &["outcome", "state"]), "preparing");
    let request = ctx.inspect("r-wait");
    assert_eq!(
        text(&request, &["state"]),
        "preparing",
        "wait_until_deadline waits: {request:?}"
    );
    assert!(
        at(&request, &["packets"])
            .as_array()
            .expect("packets")
            .is_empty(),
        "and publishes nothing while it waits: {request:?}"
    );
    ctx.kill();

    // The deadline passes. Nothing else changes.
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &config(after),
        &["context.required_before_start", "context.advisory"],
    );
    let request = ctx.inspect("r-wait");
    assert_eq!(text(&request, &["state"]), "unmet", "{request:?}");
    let items = at(&request, &["items"]).as_array().expect("items").to_vec();
    let of = |id: &str| {
        items
            .iter()
            .find(|item| text(item, &["item_id"]) == id)
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert_eq!(text(&of("required"), &["result"]), "unmet", "{items:?}");
    assert_eq!(
        text(&of("required"), &["reason"]),
        "deadline_passed",
        "{items:?}"
    );
    assert_eq!(text(&of("optional"), &["result"]), "degraded", "{items:?}");
    assert_eq!(
        text(&of("optional"), &["reason"]),
        "deadline_passed",
        "an advisory item follows its fallback at the deadline: {items:?}"
    );

    // Expiry supplies no evidence and no consent.
    let packets = at(&request, &["packets"])
        .as_array()
        .expect("packets")
        .to_vec();
    assert_eq!(packets.len(), 1, "{packets:?}");
    let revision = match at(&packets[0], &["reference", "revision"]) {
        Value::Int(revision) => *revision,
        other => panic!("a revision: {other:?}"),
    };
    let packet = result(&ctx.call(
        "context.packet.inspect",
        None,
        &format!(r#"{{"packet":"r-wait","revision":{revision}}}"#),
    ))
    .clone();
    assert!(
        at(&packet, &["citations"])
            .as_array()
            .expect("citations")
            .is_empty(),
        "a deadline cites nothing: {packet:?}"
    );
    assert!(
        at(&packet, &["sections"])
            .as_array()
            .expect("sections")
            .is_empty(),
        "and includes nothing: {packet:?}"
    );
    for item in at(&packet, &["items"]).as_array().expect("items") {
        assert_ne!(
            text(item, &["result"]),
            "satisfied",
            "expiry satisfies nothing: {item:?}"
        );
    }
    ctx.kill();
}
