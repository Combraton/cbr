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
//! - **A compiled evidence item is read only at the provider it names.**
//!   An item naming another provider's artifact is `evidence_unavailable`
//!   even when this store holds one of the same id and digest, and the
//!   citation of one that named no provider names this one (EVIDENCE
//!   section 2).

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

    // A scripted packet says so where a consumer reads it. The compiled
    // compiler has the matching assertion in J1; between them, a packet's
    // provenance is read by a test rather than only written by one. It is
    // not in the sealed bytes: the packet format carries content, and
    // provenance belongs to the revision that carries the content.
    let inspected = result(&restarted.call(
        "context.packet.inspect",
        None,
        r#"{"packet":"r-1","revision":1}"#,
    ))
    .clone();
    assert_eq!(
        text(&inspected, &["provenance", "compiler"]),
        "cbr-context-script",
        "{inspected:?}"
    );

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

#[test]
fn a_conformance_launch_prepares_nothing_unless_it_asks_to_compile() {
    // **The rule m4c loosened, and the half of it that did not move.**
    //
    // Compiling is a production capability: a conformance launch is a test
    // harness, and there a request with no script is a request nothing
    // prepares — which is what every context fixture was measured
    // against. m4c added one way to ask for it, `context.compile`, because
    // the model call site lives inside compiling and reaching it needs the
    // fake transport, which only a conformance launch may have.
    //
    // So the thing worth holding is that **the default did not change**.
    // No fixture sets the member; this says what a launch that does not
    // set it still does, and what one that does still does differently, so
    // neither can drift into the other.
    let directory = tempfile::tempdir().expect("temp dir");
    let base = r#""format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner""#;

    let mut plain = ContextProvider::start(directory.path(), &format!("{{{base}]}}"));
    assert_eq!(
        text(
            result(&plain.submit("r-1", "2030-01-01T01:00:00Z")),
            &["outcome", "state"]
        ),
        "preparing"
    );
    // Several ticks, so this is "nothing prepares it" rather than "not
    // yet". Each `inspect` is a request, and a tick runs at the start of
    // every one.
    for _ in 0..5 {
        let inspected = plain.inspect("r-1");
        assert_eq!(text(&inspected, &["state"]), "preparing", "{inspected:?}");
        assert_eq!(
            at(&inspected, &["packets"]).as_array().map(<[_]>::len),
            Some(0),
            "a conformance launch that did not ask to compile published a packet"
        );
    }
    plain.kill();

    let asking = directory.path().join("asking");
    std::fs::create_dir(&asking).expect("dir");
    let mut compiling = ContextProvider::start(
        &asking,
        &format!(r#"{{{base}],"context":{{"compile":true}}}}"#),
    );
    compiling.submit("r-1", "2030-01-01T01:00:00Z");
    let published = (0..20).find_map(|_| {
        let inspected = compiling.inspect("r-1");
        (at(&inspected, &["packets"])
            .as_array()
            .is_some_and(|packets| !packets.is_empty()))
        .then_some(inspected)
    });
    let published = published.expect("a launch that asked to compile publishes a packet");
    // Its basis names a repository this launch never registered, so the
    // item is unmet rather than satisfied. What matters is that something
    // compiled it at all.
    let item = at(&published, &["items"])
        .as_array()
        .and_then(<[Value]>::first)
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(
        text(&item, &["reason"]),
        "source_unavailable",
        "{published:?}"
    );
    compiling.kill();
}

/// Seal `bytes` as artifact `id` in the provider's own store, over the
/// same session.
fn seal(ctx: &mut ContextProvider, id: &str, bytes: &[u8]) -> String {
    let digest = cbr_encoding::digest_bytes(bytes);
    result(&ctx.call(
        "evidence.upload.prepare",
        Some((&format!("prepare-{id}"), ("evidence.artifact", id), 0)),
        &format!(
            r#"{{"digest":"{digest}","size":{},"media_type":"text/plain","producer":{{"producer_id":"context-tests"}},"source":{{"kind":"file","id":"{id}"}},"scope":"local","capture":{{"captured_at":"2030-01-01T00:00:00Z","anchors":[]}},"coverage":{{"completeness":"complete"}},"retention_class":"standard"}}"#,
            bytes.len()
        ),
    ));
    result(&ctx.call(
        "evidence.upload.append",
        Some((&format!("append-{id}"), ("evidence.artifact", id), 1)),
        &format!(
            r#"{{"offset":0,"data_base64":"{}"}}"#,
            cbr_encoding::encode_base64(bytes)
        ),
    ));
    result(&ctx.call(
        "evidence.seal",
        Some((&format!("seal-{id}"), ("evidence.artifact", id), 2)),
        "{}",
    ));
    digest
}

#[test]
fn a_compiled_evidence_item_is_read_only_at_the_provider_its_reference_names() {
    // **An evidence reference names the provider holding the artifact**,
    // and one that names none means the provider being asked (EVIDENCE
    // section 2). Compiling reads this provider's store and no other, so
    // an item naming another provider's artifact is `evidence_unavailable`
    // — here an artifact of the same id and digest *is* sealed in this
    // store, and it is still not the one the item named: equal bytes never
    // establish equal provenance or permission. `context.expand` already
    // refused such a citation; compiling used to copy the bytes into the
    // packet regardless, under a citation its own expand would refuse.
    //
    // And a reference a packet carries always names its provider, so an
    // item that named none is cited under this provider's id.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start(
        directory.path(),
        r#"{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"],"context":{"compile":true}}"#,
    );
    let log = b"running 1 test\ntest tests::holds ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    let digest = seal(&mut ctx, "log-1", log);
    let item = |provider: &str| {
        format!(
            r#"{{"item_id":"log","selector":{{"kind":"evidence","value":"log-1"}},"obligation":"required_before_start","reliance":"evidence","selected_by":"owner","check":{{"kind":"evidence_included","evidence":{{{provider}"artifact":{{"kind":"evidence.artifact","id":"log-1"}},"digest":"{digest}"}}}}}}"#
        )
    };
    let requests = [
        ("r-foreign", r#""provider":"elsewhere","#),
        ("r-own", r#""provider":"context-1","#),
        ("r-bare", ""),
    ];
    for (request, provider) in requests {
        let submitted = ctx.submit_with(
            request,
            &item(provider),
            "proceed_with_gap",
            "2030-01-01T01:00:00Z",
        );
        assert_eq!(
            text(result(&submitted), &["outcome", "state"]),
            "preparing",
            "{submitted:?}"
        );
    }
    let mut settled = Vec::new();
    for (request, _) in requests {
        let published = (0..50).find_map(|_| {
            let inspected = ctx.inspect(request);
            (at(&inspected, &["packets"])
                .as_array()
                .is_some_and(|packets| !packets.is_empty()))
            .then_some(inspected)
        });
        let published = published.unwrap_or_else(|| panic!("{request} never published"));
        let item = at(&published, &["items"]).as_array().expect("items")[0].clone();
        let reason = item.get("reason").and_then(Value::as_str).unwrap_or("");
        settled.push((
            request,
            text(&item, &["result"]).to_string(),
            reason.to_string(),
        ));
    }
    assert_eq!(
        settled,
        vec![
            ("r-foreign", "unmet".into(), "evidence_unavailable".into()),
            ("r-own", "satisfied".into(), String::new()),
            ("r-bare", "satisfied".into(), String::new()),
        ]
    );

    // The citation a packet carries, read from the sealed bytes.
    let mut citation = |request: &str| {
        let inspected = ctx.call(
            "context.packet.inspect",
            None,
            &format!(r#"{{"packet":"{request}","revision":1,"max_bytes":1000000}}"#),
        );
        let data = text(result(&inspected), &["excerpt", "data_base64"]);
        let sealed = parse(
            &String::from_utf8(cbr_encoding::decode_base64(data).expect("base64")).expect("utf-8"),
        );
        let cited: Vec<Value> = at(&sealed, &["sections"])
            .as_array()
            .expect("sections")
            .iter()
            .flat_map(|section| {
                at(section, &["citations"])
                    .as_array()
                    .expect("citations")
                    .to_vec()
            })
            .map(|citation| at(&citation, &["evidence"]).clone())
            .collect();
        cited
    };
    let named = format!(
        r#"{{"artifact":{{"id":"log-1","kind":"evidence.artifact"}},"digest":"{digest}","provider":"context-1"}}"#
    );
    for request in ["r-own", "r-bare"] {
        let cited: Vec<String> = citation(request)
            .iter()
            .map(|evidence| String::from_utf8(cbr_encoding::to_canonical(evidence)).expect("utf-8"))
            .collect();
        assert_eq!(cited, vec![named.clone()], "{request}");
    }
    assert!(
        citation("r-foreign").is_empty(),
        "another provider's artifact was cited"
    );
    ctx.kill();
}

/// The protocol's identifier grammar (`core/1/common.schema.json`), which
/// the `context` schemas require of a section id, a citation id and an
/// artifact id. Written out because the provider is a binary with no
/// library target.
fn is_identifier(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'~' | b'-'))
}

/// Every object file the store holds.
fn objects(data: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                walk(&entry.path(), found);
            } else {
                found.push(entry.path());
            }
        }
    }
    let mut found = Vec::new();
    walk(&data.join("objects").join("sha256"), &mut found);
    found
}

#[test]
fn a_packet_holding_an_id_outside_the_identifier_grammar_is_never_published() {
    // **The last door before a packet is published.** The compiler builds
    // ids inside the grammar now, and a test control can still script any
    // string at all: `context.script` checks nothing about a section's id.
    // So the packet is checked where it is sealed, before anything leaves
    // the tick — no capture, no object, no artifact, no event — and a job
    // whose packet fails is logged and passed over, while every other job
    // on the provider carries on.
    //
    // The log names **where** each bad id is, as a JSON pointer, and never
    // the id itself: the id is often a path, and a provider's log is the
    // kind of thing that ends up in a committed record.
    let directory = tempfile::tempdir().expect("temp dir");
    let section = |id: &str| {
        format!(
            r#"[{{"section":{{"section_id":"{id}","item_id":"i-1","label":"source_inspected","content":"fn main() {{}}","source":{{"repository":"repo-a","path":"src/main.rs","tree":"tree-1"}}}}}},{{"publish":{{}}}}]"#
        )
    };
    let mut ctx = ContextProvider::start(
        directory.path(),
        &format!(
            r#"{{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"],"context":{{"scripts":{{"r-bad":{},"r-good":{}}}}}}}"#,
            section("s/x-secret-path"),
            section("s-1"),
        ),
    );
    // The bad request first, so its job is the first the tick walks: a
    // refusal that stopped the tick would stop the good one with it.
    for request in ["r-bad", "r-good"] {
        assert_eq!(
            text(
                result(&ctx.submit(request, "2030-01-01T01:00:00Z")),
                &["outcome", "state"]
            ),
            "preparing"
        );
    }
    let published = (0..50).find_map(|_| {
        let inspected = ctx.inspect("r-good");
        (at(&inspected, &["packets"])
            .as_array()
            .is_some_and(|packets| !packets.is_empty()))
        .then_some(inspected)
    });
    assert!(
        published.is_some(),
        "a valid job beside the refused one never published"
    );
    for _ in 0..5 {
        let refused = ctx.inspect("r-bad");
        assert_eq!(
            at(&refused, &["packets"]).as_array().map(<[_]>::len),
            Some(0),
            "a packet with an id outside the grammar was published: {refused:?}"
        );
        assert_eq!(text(&refused, &["state"]), "preparing", "{refused:?}");
    }

    let data = directory.path().join("context-data");
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let artifacts: Vec<String> = connection
        .prepare("SELECT id FROM subjects WHERE kind = 'evidence.artifact'")
        .expect("prepares")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    assert_eq!(artifacts, ["packet.r-good.1"], "no artifact for r-bad");
    let published_events: Vec<String> = connection
        .prepare("SELECT subject_id FROM events WHERE type = 'context.packet.published'")
        .expect("prepares")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    assert_eq!(
        published_events,
        ["r-good"],
        "no publication event for r-bad"
    );
    // Not even the object: the check runs before the packet's bytes are
    // written anywhere, so a refused scripted packet leaves nothing to
    // collect. (A compiled one would leave the objects of the sources it
    // sealed earlier in the tick, uncommitted, for the start-time
    // collection pass: `publish_one` says so.)
    assert_eq!(
        objects(&data).len(),
        1,
        "only r-good's packet has an object: {:?}",
        objects(&data)
    );

    let logged =
        std::fs::read_to_string(directory.path().join("context-stderr.log")).expect("the log");
    assert!(
        logged.contains("r-bad") && logged.contains("/sections/0/section_id"),
        "the log names the request and where the id is: {logged}"
    );
    assert!(
        !logged.contains("secret-path"),
        "the log repeats the id itself: {logged}"
    );
    ctx.kill();
}

#[test]
fn a_request_id_of_127_characters_is_published_under_an_artifact_id_inside_the_grammar() {
    // **`packet.<request>.<revision>` is a convention, not a guarantee.**
    // A request id may be 128 characters, and `packet.` and `.1` take
    // nine more, so the artifact a packet is sealed as would be outside
    // the grammar its own reference is checked against. Past 128 the id
    // keeps the `packet.` prefix a grant's `id_prefix` names, then a
    // digest of the rest and as much of its tail as fits.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        r#"{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"],"context":{"compile":true}}"#,
        &["context.required_before_start", "context.expand"],
    );
    let log = b"running 1 test\ntest tests::holds ... ok\n";
    let digest = seal(&mut ctx, "log-1", log);
    let request = format!("r{}", "x".repeat(126));
    assert_eq!(request.len(), 127);
    let payload = format!(
        r#"{{"consumer":{{"task":"fix the build","principal":"owner"}},"basis":{{"repositories":[{{"id":"repo-a","tree":"tree-1","workspace":"clean","dirty":null}}],"completeness":"complete"}},"items":[{{"item_id":"log","selector":{{"kind":"evidence","value":"log-1"}},"obligation":"required_before_start","reliance":"evidence","selected_by":"owner","check":{{"kind":"evidence_included","evidence":{{"artifact":{{"kind":"evidence.artifact","id":"log-1"}},"digest":"{digest}"}}}}}}],"fallback":"proceed_with_gap","limits":{{"deadline":"2030-01-01T01:00:00Z","investigation":{{"units":"queries","amount":0}},"output_capacity":{{"units":"bytes","amount":4096}}}}}}"#
    );
    // A short command id: the request's own id is already 127, and a
    // command id is an identifier too.
    let submitted = ctx.call(
        "context.request.submit",
        Some(("submit-long", ("context.request", &request), 0)),
        &payload,
    );
    assert_eq!(
        text(result(&submitted), &["outcome", "state"]),
        "preparing",
        "{submitted:?}"
    );
    let published = (0..50)
        .find_map(|_| {
            let inspected = ctx.inspect(&request);
            at(&inspected, &["packets"])
                .as_array()
                .and_then(<[Value]>::first)
                .cloned()
        })
        .expect("the long request published");
    let artifact = text(&published, &["reference", "artifact", "artifact", "id"]).to_string();
    assert!(
        is_identifier(&artifact) && artifact.starts_with("packet."),
        "the packet artifact id {artifact} ({} bytes) is outside the grammar",
        artifact.len()
    );

    let expanded = ctx.call(
        "context.expand",
        None,
        &format!(r#"{{"packet":"{request}","revision":1,"citation":"c-log","max_bytes":4096}}"#),
    );
    let data = text(result(&expanded), &["excerpt", "data_base64"]);
    assert_eq!(
        cbr_encoding::decode_base64(data).expect("base64"),
        log,
        "the long request's citation expands"
    );
    ctx.kill();
}

#[test]
fn a_scripted_packet_missing_a_required_id_is_never_published() {
    // **A missing id is not let through as an absent one.** A script can
    // leave a section's `section_id` out, which the facts then carry as
    // `null`, or a citation's `citation_id`, which they then do not carry
    // at all; the inspect schema requires both. Each is refused at the same
    // door as an id outside the grammar, and the log names where it is
    // missing.
    let directory = tempfile::tempdir().expect("temp dir");
    let evidence = r#""evidence":{"provider":"context-1","artifact":{"kind":"evidence.artifact","id":"log-1"},"digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}"#;
    let script = |section: &str| format!(r#"[{{"section":{{{section}}}}},{{"publish":{{}}}}]"#);
    let body = r#""item_id":"i-1","label":"source_inspected","content":"fn main() {}""#;
    let mut ctx = ContextProvider::start(
        directory.path(),
        &format!(
            r#"{{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"],"context":{{"scripts":{{"r-no-section-id":{},"r-no-citation-id":{},"r-good":{}}}}}}}"#,
            script(body),
            script(&format!(
                r#""section_id":"s-1",{body},"citations":[{{{evidence}}}]"#
            )),
            script(&format!(r#""section_id":"s-1",{body}"#)),
        ),
    );
    for request in ["r-no-section-id", "r-no-citation-id", "r-good"] {
        assert_eq!(
            text(
                result(&ctx.submit(request, "2030-01-01T01:00:00Z")),
                &["outcome", "state"]
            ),
            "preparing"
        );
    }
    let published = (0..50).find_map(|_| {
        let inspected = ctx.inspect("r-good");
        (at(&inspected, &["packets"])
            .as_array()
            .is_some_and(|packets| !packets.is_empty()))
        .then_some(inspected)
    });
    assert!(
        published.is_some(),
        "a valid job beside the refused ones never published"
    );
    for _ in 0..5 {
        for request in ["r-no-section-id", "r-no-citation-id"] {
            let refused = ctx.inspect(request);
            assert_eq!(
                at(&refused, &["packets"]).as_array().map(<[_]>::len),
                Some(0),
                "{request}: a packet missing a required id was published: {refused:?}"
            );
        }
    }

    let logged =
        std::fs::read_to_string(directory.path().join("context-stderr.log")).expect("the log");
    let line = |request: &str| {
        logged
            .lines()
            .find(|line| line.contains(&format!("request {request}:")))
            .unwrap_or_else(|| panic!("nothing logged for {request}: {logged}"))
            .to_string()
    };
    assert!(
        line("r-no-section-id").contains("/sections/0/section_id"),
        "the log names the missing section id: {logged}"
    );
    let citation = line("r-no-citation-id");
    for pointer in [
        "/sections/0/citations/0",
        "/citations/0/citation_id",
        "/body/sections/0/citations/0/citation_id",
    ] {
        assert!(
            citation.contains(pointer),
            "the log names the missing citation id at {pointer}: {citation}"
        );
    }
    ctx.kill();
}
