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
//! - **A packet the publication guard refuses ends its job**, with the typed
//!   reason `packet_invalid` (the owner's ruling of 2026-09-26). Every
//!   subscriber still preparing is `refused` with that reason, in one batch
//!   built from the stored records and apart from the refused tick; one
//!   already published under `context.updates` keeps its revision. It holds
//!   at the deadline, across ticks, across a restart, and across a `SIGKILL`
//!   before the refusal commits.

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
    // whose packet fails ends, its request refused as `packet_invalid`,
    // while every other job on the provider carries on.
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
        // **Refused, not left preparing** (the owner's ruling of
        // 2026-09-26): nothing will ever be published for it.
        assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
        assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
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
    // door as an id outside the grammar, ends its request the same way,
    // `refused` as `packet_invalid`, and the log names where it is missing.
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
            assert_eq!(
                text(&refused, &["state"]),
                "refused",
                "{request}: {refused:?}"
            );
            assert_eq!(
                text(&refused, &["reason"]),
                "packet_invalid",
                "{request}: {refused:?}"
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

// ---- a packet the guard refuses ends its job -------------------------------

/// A section id outside the identifier grammar, which only a script can
/// write now. `secret-path` stands for what such an id usually was, a
/// repository path, and is what the log must never repeat.
const OUTSIDE_GRAMMAR: &str = "s/x-secret-path";

/// Between building the batch that ends a refused job and committing it.
/// Named here and in no descriptor, because no fixture waits on it.
const REFUSED_BEFORE_COMMIT: &str = "context.packet.refused_before_commit";

/// [`ContextProvider::submit`]'s one item as a refusal leaves it: required,
/// so `unmet`, for the reason the job ended.
const REFUSED_ITEM: &str = r#"[{"item_id":"i-1","obligation":"required_before_start","reason":"packet_invalid","result":"unmet"}]"#;

/// A scripted section for item `i-1` of [`ContextProvider::submit`]'s
/// request. The item's check reads the section's source and never its id,
/// so the section satisfies `i-1` whatever `id` is.
fn source_section(id: &str) -> String {
    format!(
        r#"{{"section":{{"section_id":"{id}","item_id":"i-1","label":"source_inspected","content":"fn main() {{}}","source":{{"repository":"repo-a","path":"src/main.rs","tree":"tree-1"}}}}}}"#
    )
}

/// A conformance launch whose `context.script` control holds `scripts`,
/// with `extra` top-level members, such as a clock, spliced in before it.
fn scripted(scripts: &str, extra: &str) -> String {
    format!(
        r#"{{"format":"combraton-conformance-config/1","provider_id":"context-1","principal":"owner","authority_principals":["owner"]{extra},"context":{{"scripts":{{{scripts}}}}}}}"#
    )
}

fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("utf-8")
}

/// Inspect `request` until it leaves `preparing`. Each inspect is a
/// request, and a tick runs at the start of every one.
fn settled(ctx: &mut ContextProvider, request: &str) -> Value {
    let mut last = Value::Null;
    for _ in 0..50 {
        last = ctx.inspect(request);
        if text(&last, &["state"]) != "preparing" {
            return last;
        }
    }
    panic!("{request} never left preparing within 50 polls: {last:?}");
}

/// The provider log of the process last started over `directory`: each
/// start creates the file afresh.
fn log(directory: &Path) -> String {
    std::fs::read_to_string(directory.join("context-stderr.log")).expect("the log")
}

/// How many lines of `log` report a packet of `request`'s refused.
fn refusals(log: &str, request: &str) -> usize {
    log.lines()
        .filter(|line| line.contains(&format!("request {request}:")))
        .count()
}

/// One recorded event: its subject as `(kind, id)`, its type, the subject
/// revision it was recorded at and its payload as canonical JSON.
#[derive(Debug)]
struct Recorded {
    subject: (String, String),
    event: String,
    revision: i64,
    payload: String,
}

/// Every event the provider has recorded, in stream order.
fn recorded(ctx: &mut ContextProvider) -> Vec<Recorded> {
    let read = ctx.call("core.events.read", None, r#"{"from":"start","limit":1000}"#);
    at(result(&read), &["items"])
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|item| item.get("event"))
        .map(|event| Recorded {
            subject: (
                text(event, &["subject", "kind"]).to_string(),
                text(event, &["subject", "id"]).to_string(),
            ),
            event: text(event, &["type"]).to_string(),
            revision: match at(event, &["revision"]) {
                Value::Int(revision) => *revision,
                other => panic!("a revision: {other:?}"),
            },
            payload: canonical(at(event, &["payload"])),
        })
        .collect()
}

/// One subject's events as `(type, payload)`, in stream order.
fn of_subject(events: &[Recorded], kind: &str, id: &str) -> Vec<(String, String)> {
    events
        .iter()
        .filter(|e| e.subject.0 == kind && e.subject.1 == id)
        .map(|e| (e.event.clone(), e.payload.clone()))
        .collect()
}

/// A subject's record as the store holds it. No operation serves a job's
/// record, so this is how a test reads one.
fn stored(data: &Path, kind: &str, id: &str) -> Value {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let value: String = connection
        .query_row(
            "SELECT value FROM subjects WHERE kind = ?1 AND id = ?2",
            [kind, id],
            |row| row.get(0),
        )
        .unwrap_or_else(|error| panic!("no {kind} {id}: {error}"));
    parse(&value)
}

/// The ids of every evidence artifact the store holds.
fn artifacts(data: &Path) -> Vec<String> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    connection
        .prepare("SELECT id FROM subjects WHERE kind = 'evidence.artifact' ORDER BY id")
        .expect("prepares")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads")
}

/// Nothing of a publication was recorded: no seal and no packet.
fn assert_nothing_published(events: &[Recorded]) {
    for event in events {
        assert!(
            !matches!(
                event.event.as_str(),
                "context.packet.published" | "evidence.artifact.sealed"
            ),
            "a refused packet left a publication behind: {events:?}"
        );
    }
}

/// The one-step script every refusal test below starts from: a section
/// the guard refuses, then `publish`.
fn refused_script(request: &str) -> String {
    format!(
        r#""{request}":[{},{{"publish":{{}}}}]"#,
        source_section(OUTSIDE_GRAMMAR)
    )
}

#[test]
fn a_packet_the_guard_refuses_ends_its_job_and_refuses_its_request_as_packet_invalid() {
    // **The owner's ruling of 2026-09-26**: a packet the publication guard
    // refuses ends its job with the typed reason `packet_invalid`. Before
    // it, the request stayed `preparing` for ever, compiled again and
    // logged again at every tick.
    //
    // The request ends `refused`. `ready`, `partial` and `unmet` each
    // name a published revision (CONTEXT section 3), and there is none to
    // name; `refused` is the one terminal state that needs no packet. That
    // state is this session's reading within the ruling, which named the
    // job's ending and its reason.
    //
    // The scripted section would satisfy `i-1`, so an item read off the
    // job's sections would say `satisfied`, and one read as still
    // preparing `pending`. Nothing was delivered: the required item is
    // `unmet`, for the reason the job ended.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start(directory.path(), &scripted(&refused_script("r-bad"), ""));
    let submitted = ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");

    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    assert!(
        refused.get("needed").is_none(),
        "an invalid packet is not a capacity that could be raised: {refused:?}"
    );
    assert_eq!(
        at(&refused, &["packets"]).as_array().map(<[_]>::len),
        Some(0),
        "{refused:?}"
    );
    let job = r#"{"id":"r-bad","kind":"context.job"}"#;
    assert_eq!(canonical(at(&refused, &["job"])), job);
    assert_eq!(canonical(at(&refused, &["items"])), REFUSED_ITEM);

    // The request's change, then the job's ending, and nothing published.
    let events = recorded(&mut ctx);
    assert_eq!(
        of_subject(&events, "context.request", "r-bad"),
        vec![
            (
                "context.request.changed".to_string(),
                format!(r#"{{"job":{job},"state":"preparing"}}"#)
            ),
            (
                "context.request.changed".to_string(),
                format!(r#"{{"job":{job},"reason":"packet_invalid","state":"refused"}}"#)
            ),
        ],
        "{events:?}"
    );
    assert_eq!(
        of_subject(&events, "context.job", "r-bad"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    let position = |wanted: &dyn Fn(&Recorded) -> bool| {
        events
            .iter()
            .position(wanted)
            .unwrap_or_else(|| panic!("not recorded: {events:?}"))
    };
    let preparing = position(&|e| e.payload.contains(r#""state":"preparing""#));
    let changed = position(&|e| e.payload.contains(r#""state":"refused""#));
    let ended = position(&|e| e.event == "context.job.ended");
    assert!(
        changed < ended,
        "the request's change comes before the job's ending: {events:?}"
    );
    // One change raises the request's revision once, and the inspect
    // reports the revision the change was recorded at.
    assert_eq!(
        events[changed].revision,
        events[preparing].revision + 1,
        "{events:?}"
    );
    assert_eq!(
        at(&refused, &["revision"]),
        &Value::Int(events[changed].revision)
    );
    assert_nothing_published(&events);

    // Ended from the job as it was stored, not from the tick that was
    // refused: nothing that tick advanced was saved.
    let stored_job = stored(
        &directory.path().join("context-data"),
        "context.job",
        "r-bad",
    );
    assert_eq!(text(&stored_job, &["state"]), "ended", "{stored_job:?}");
    assert_eq!(text(&stored_job, &["reason"]), "packet_invalid");
    assert_eq!(
        at(&stored_job, &["cursor"]),
        &Value::Int(0),
        "{stored_job:?}"
    );
    assert_eq!(
        at(&stored_job, &["sections"]).as_array().map(<[_]>::len),
        Some(0),
        "{stored_job:?}"
    );
    ctx.kill();
}

#[test]
fn a_refused_request_stays_refused_across_ticks_and_restarts_and_its_job_is_never_compiled_again() {
    // **Refused is terminal.** Before the ruling the job was compiled
    // again at every tick, and logged again, because nothing recorded that
    // it had been tried. Now it has ended, so no tick walks it, and its
    // one log line is the only one there will be, in this process or the
    // next.
    let directory = tempfile::tempdir().expect("temp dir");
    let config = scripted(&refused_script("r-bad"), "");
    let mut ctx = ContextProvider::start(directory.path(), &config);
    ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    for _ in 0..10 {
        let again = ctx.inspect("r-bad");
        assert_eq!(
            canonical(&again),
            canonical(&refused),
            "a later tick changed a refused request"
        );
    }
    let logged = log(directory.path());
    assert_eq!(refusals(&logged, "r-bad"), 1, "{logged}");
    let data = directory.path().join("context-data");
    let objects_before = objects(&data).len();
    ctx.kill();

    let mut restarted = ContextProvider::start(directory.path(), &config);
    for _ in 0..5 {
        let again = restarted.inspect("r-bad");
        assert_eq!(
            canonical(&again),
            canonical(&refused),
            "the restart changed a refused request"
        );
    }
    let events = recorded(&mut restarted);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event == "context.job.ended")
            .count(),
        1,
        "{events:?}"
    );
    let logged = log(directory.path());
    assert_eq!(
        refusals(&logged, "r-bad"),
        0,
        "the restarted provider compiled the ended job again: {logged}"
    );
    assert_eq!(objects(&data).len(), objects_before);
    restarted.kill();
}

#[test]
fn every_request_sharing_a_job_the_guard_refuses_is_refused_with_it() {
    // **The subscribers of one job share its sections, and so its
    // refusal.** A section dropped from a narrower subscriber's packet
    // for capacity is still named there, as an omission, and the guard
    // reads omission ids too: no subscriber's own packet would have been
    // valid. Here both subscribers have one deadline, both are refused
    // with the job, and the job ends once. One whose deadline is later
    // than the one that brought the packet on is refused too:
    // `a_subscriber_with_a_later_deadline_is_refused_with_the_job_an_earlier_deadline_ended`.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let script = format!(
        r#""r-first":[{{"wait_until":"2030-01-01T00:10:00Z"}},{},{{"publish":{{}}}}]"#,
        source_section(OUTSIDE_GRAMMAR)
    );
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &scripted(
            &script,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
        &["context.required_before_start", "context.shared_jobs"],
    );
    let job = r#"{"id":"r-first","kind":"context.job"}"#;
    for request in ["r-first", "r-second"] {
        let submitted = ctx.submit(request, "2030-01-01T01:00:00Z");
        let outcome = at(result(&submitted), &["outcome"]);
        assert_eq!(text(outcome, &["state"]), "preparing", "{submitted:?}");
        // The second joins the first's job, which waits for the clock.
        assert_eq!(canonical(at(outcome, &["job"])), job, "{submitted:?}");
    }

    set_clock(&clock, "2030-01-01T00:10:00Z");
    for request in ["r-first", "r-second"] {
        let refused = settled(&mut ctx, request);
        assert_eq!(
            text(&refused, &["state"]),
            "refused",
            "{request}: {refused:?}"
        );
        assert_eq!(
            text(&refused, &["reason"]),
            "packet_invalid",
            "{request}: {refused:?}"
        );
        assert_eq!(canonical(at(&refused, &["job"])), job, "{request}");
        assert_eq!(
            canonical(at(&refused, &["items"])),
            REFUSED_ITEM,
            "{request}"
        );
        assert_eq!(
            at(&refused, &["packets"]).as_array().map(<[_]>::len),
            Some(0),
            "{request}: {refused:?}"
        );
    }
    let events = recorded(&mut ctx);
    assert_eq!(
        of_subject(&events, "context.job", "r-first"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    assert_nothing_published(&events);
    ctx.kill();
}

#[test]
fn an_update_the_guard_refuses_ends_the_job_and_leaves_the_published_revision_current() {
    // **A request already delivered keeps what it was delivered.** Under
    // `context.updates` revision 1 is published and the job carries on
    // towards revision 2, which the guard refuses. The job ends, and the
    // request is not refused: it has a packet, and that packet stays
    // current. The ending shows only on the job, as `context.job.ended`.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let script = format!(
        r#""r":[{},{{"publish":{{}}}},{{"wait_until":"2030-01-01T00:10:00Z"}},{},{{"publish":{{}}}}]"#,
        source_section("s-1"),
        source_section(OUTSIDE_GRAMMAR)
    );
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &scripted(
            &script,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
        &["context.required_before_start", "context.updates"],
    );
    ctx.submit("r", "2030-01-01T01:00:00Z");
    let published = (0..50)
        .find_map(|_| {
            let inspected = ctx.inspect("r");
            (at(&inspected, &["packets"]).as_array().map(<[_]>::len) == Some(1))
                .then_some(inspected)
        })
        .expect("revision 1 was never published");
    assert_eq!(text(&published, &["state"]), "ready", "{published:?}");

    set_clock(&clock, "2030-01-01T00:10:00Z");
    let events = (0..50)
        .find_map(|_| {
            let events = recorded(&mut ctx);
            events
                .iter()
                .any(|e| e.event == "context.job.ended")
                .then_some(events)
        })
        .expect("the job never ended: a refused update left it running");
    assert_eq!(
        of_subject(&events, "context.job", "r"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| e.payload.contains(r#""state":"refused""#)),
        "a request that was delivered a packet was refused: {events:?}"
    );

    let after = ctx.inspect("r");
    assert_eq!(text(&after, &["state"]), "ready", "{after:?}");
    assert!(after.get("reason").is_none(), "{after:?}");
    assert_eq!(
        at(&after, &["revision"]),
        at(&published, &["revision"]),
        "the request's revision moved"
    );
    let packets = at(&after, &["packets"])
        .as_array()
        .expect("packets")
        .to_vec();
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert_eq!(at(&packets[0], &["current"]), &Value::Bool(true));
    assert_eq!(
        canonical(&after),
        canonical(&published),
        "revision 1 is read as it was published"
    );
    assert_eq!(
        artifacts(&directory.path().join("context-data")),
        ["packet.r.1"],
        "no artifact for the refused revision"
    );
    ctx.kill();
}

#[test]
fn a_refusal_commits_nothing_of_the_tick_that_was_refused() {
    // **The refused tick is dropped whole, and the refusal is built apart
    // from it.** Here one tick publishes revision 1, a valid packet, then
    // refuses revision 2. Nothing of that tick commits, not revision 1's
    // artifact and not its publication, so the request was never
    // delivered anything and is refused like any other. Revision 1's
    // object was written before the refusal and no row names it; the
    // start-time collection pass removes it. Only a script publishes twice
    // in one tick.
    let directory = tempfile::tempdir().expect("temp dir");
    let script = format!(
        r#""r":[{},{{"publish":{{}}}},{},{{"publish":{{}}}}]"#,
        source_section("s-1"),
        source_section(OUTSIDE_GRAMMAR)
    );
    let config = scripted(&script, "");
    let features = ["context.required_before_start", "context.updates"];
    let mut ctx = ContextProvider::start_with(directory.path(), &config, &features);
    ctx.submit("r", "2030-01-01T01:00:00Z");
    let refused = settled(&mut ctx, "r");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    assert_eq!(
        at(&refused, &["packets"]).as_array().map(<[_]>::len),
        Some(0),
        "{refused:?}"
    );
    let data = directory.path().join("context-data");
    assert!(
        artifacts(&data).is_empty(),
        "an artifact of the refused tick was committed: {:?}",
        artifacts(&data)
    );
    assert_nothing_published(&recorded(&mut ctx));
    // The premise: revision 1 was sealed in the tick that was refused.
    assert_eq!(objects(&data).len(), 1, "{:?}", objects(&data));
    ctx.kill();

    let mut restarted = ContextProvider::start_with(directory.path(), &config, &features);
    assert_eq!(canonical(&restarted.inspect("r")), canonical(&refused));
    assert!(
        objects(&data).is_empty(),
        "an object no row names survived the restart: {:?}",
        objects(&data)
    );
    restarted.kill();
}

#[test]
fn a_request_refused_after_content_was_prepared_reports_no_item_satisfied() {
    // **What was prepared was never delivered.** The first tick commits a
    // section that satisfies `i-1`, and while the request is preparing its
    // inspect says so. The packet that would have carried the section is
    // refused, so nothing satisfied `i-1` for the consumer: it ends
    // `unmet`, for the reason the job ended, however far preparation got.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let script = format!(
        r#""r-bad":[{},{{"wait_until":"2030-01-01T00:10:00Z"}},{},{{"publish":{{}}}}]"#,
        source_section("s-1"),
        source_section(OUTSIDE_GRAMMAR)
    );
    let mut ctx = ContextProvider::start(
        directory.path(),
        &scripted(
            &script,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
    );
    ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    let preparing = ctx.inspect("r-bad");
    assert_eq!(text(&preparing, &["state"]), "preparing", "{preparing:?}");
    assert_eq!(
        text(
            &at(&preparing, &["items"]).as_array().expect("items")[0],
            &["result"]
        ),
        "satisfied",
        "the premise: what was prepared satisfies i-1: {preparing:?}"
    );

    set_clock(&clock, "2030-01-01T00:10:00Z");
    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    assert_eq!(canonical(at(&refused, &["items"])), REFUSED_ITEM);
    ctx.kill();
}

#[test]
fn a_kill_before_the_refusal_commits_leaves_nothing_and_the_restart_refuses_once() {
    // **The refusal is one batch, and its log line follows the commit.**
    // The provider is killed with the batch that ends the job built and
    // not committed. None of it is visible afterwards: the request is
    // still preparing, the job still running, and nothing was logged — a
    // line written first would report a refusal that never happened. The
    // restart refuses the request, once.
    let directory = tempfile::tempdir().expect("temp dir");
    let barriers = directory.path().join("barriers");
    std::fs::create_dir(&barriers).expect("barrier dir");
    let config = |enabled: &str| {
        scripted(
            &refused_script("r-bad"),
            &format!(
                r#","test_barriers":{{"directory":"{}","enabled":[{enabled}]}}"#,
                barriers.display()
            ),
        )
    };

    let mut ctx = ContextProvider::start(
        directory.path(),
        &config(&format!(r#""{REFUSED_BEFORE_COMMIT}""#)),
    );
    let submitted = ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");
    ctx.send("context.request.inspect", None, r#"{"request":"r-bad"}"#);
    wait_for(
        &barriers.join(format!("{REFUSED_BEFORE_COMMIT}.reached")),
        "the refusal's batch reaching its commit",
    );
    let mut killed = ctx;
    killed.child.kill().expect("SIGKILL");
    killed.child.wait().expect("reaped");
    assert!(
        killed.read().is_none(),
        "killed before the refusal committed, the provider must not have answered"
    );
    drop(killed);
    let logged = log(directory.path());
    assert_eq!(
        refusals(&logged, "r-bad"),
        0,
        "a refusal was logged before it committed: {logged}"
    );
    let data = directory.path().join("context-data");
    assert_eq!(
        text(&stored(&data, "context.request", "r-bad"), &["state"]),
        "preparing"
    );
    assert_eq!(
        text(&stored(&data, "context.job", "r-bad"), &["state"]),
        "running"
    );

    let mut restarted = ContextProvider::start(directory.path(), &config(""));
    let refused = settled(&mut restarted, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    let events = recorded(&mut restarted);
    let count = |wanted: &dyn Fn(&Recorded) -> bool| events.iter().filter(|e| wanted(e)).count();
    assert_eq!(
        count(&|e| e.payload.contains(r#""state":"refused""#)),
        1,
        "{events:?}"
    );
    assert_eq!(count(&|e| e.event == "context.job.ended"), 1, "{events:?}");
    let logged = log(directory.path());
    assert_eq!(refusals(&logged, "r-bad"), 1, "{logged}");
    restarted.kill();
}

#[test]
fn a_request_the_guard_refuses_at_its_deadline_ends_refused_too() {
    // **The deadline goes through the same door.** A request still
    // preparing at its deadline is published with whatever it has. When
    // what it has is a section the guard refuses, it ends like any other
    // refused packet, rather than staying `preparing` past its deadline.
    // The job's reason is `packet_invalid`, not `deadline_passed`: the
    // refusal is what ended it.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start(
        directory.path(),
        &scripted(
            &format!(r#""r-bad":[{}]"#, source_section(OUTSIDE_GRAMMAR)),
            r#","clock":{"fixed":"2030-01-01T00:00:00Z"}"#,
        ),
    );
    let submitted = ctx.submit("r-bad", "2030-01-01T00:00:00Z");
    assert_eq!(text(result(&submitted), &["outcome", "state"]), "preparing");
    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    assert_eq!(canonical(at(&refused, &["items"])), REFUSED_ITEM);
    assert_eq!(
        at(&refused, &["packets"]).as_array().map(<[_]>::len),
        Some(0),
        "{refused:?}"
    );
    let events = recorded(&mut ctx);
    assert_eq!(
        of_subject(&events, "context.job", "r-bad"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    ctx.kill();
}

#[test]
fn a_refusal_holds_back_no_other_job_the_same_tick_walks() {
    // **One bad packet must not stop every other job on the provider**,
    // which is why the guard's failure is not routed through `Protocol`.
    // The guard test above held that while a refused job came back at
    // every tick; now the job ends at its first refusal, and there that
    // comes before the valid request's job exists. Here both jobs wait
    // for the same instant, the refused one first in the order the tick
    // walks them, and the valid one publishes in that same tick: the one
    // request after the clock moves, and so the one tick at the new
    // instant, already sees its packet.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let wait = r#"{"wait_until":"2030-01-01T00:10:00Z"}"#;
    let scripts = format!(
        r#""r-bad":[{wait},{},{{"publish":{{}}}}],"r-good":[{wait},{},{{"publish":{{}}}}]"#,
        source_section(OUTSIDE_GRAMMAR),
        source_section("s-1"),
    );
    let mut ctx = ContextProvider::start(
        directory.path(),
        &scripted(
            &scripts,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
    );
    for request in ["r-bad", "r-good"] {
        let submitted = ctx.submit(request, "2030-01-01T01:00:00Z");
        assert_eq!(
            text(result(&submitted), &["outcome", "state"]),
            "preparing",
            "{submitted:?}"
        );
    }

    set_clock(&clock, "2030-01-01T00:10:00Z");
    let good = ctx.inspect("r-good");
    assert_eq!(
        text(&good, &["state"]),
        "ready",
        "the refusal held back the job the tick walked after it: {good:?}"
    );
    assert_eq!(
        at(&good, &["packets"]).as_array().map(<[_]>::len),
        Some(1),
        "{good:?}"
    );
    let bad = ctx.inspect("r-bad");
    assert_eq!(text(&bad, &["state"]), "refused", "{bad:?}");
    assert_eq!(text(&bad, &["reason"]), "packet_invalid", "{bad:?}");
    let logged = log(directory.path());
    assert!(
        !logged.contains("context preparation failed"),
        "the refusal stopped the tick: {logged}"
    );
    ctx.kill();
}

// ---- the cases the verification's mutants reached ---------------------------

/// The job both subscribers of [`two_subscribers_of_one_refused_job`] share,
/// named after the first of them.
const SHARED_JOB: &str = r#"{"id":"r-first","kind":"context.job"}"#;

/// `r-first` and `r-second`, both preparing in one job under
/// `context.shared_jobs`, with the same deadline. The job's script waits
/// for 00:10, then publishes a section the guard refuses. Returns the
/// provider and its clock file, at 00:00.
fn two_subscribers_of_one_refused_job(directory: &Path) -> (ContextProvider, PathBuf) {
    let clock = directory.join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let script = format!(
        r#""r-first":[{{"wait_until":"2030-01-01T00:10:00Z"}},{},{{"publish":{{}}}}]"#,
        source_section(OUTSIDE_GRAMMAR)
    );
    let mut ctx = ContextProvider::start_with(
        directory,
        &scripted(
            &script,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
        &["context.required_before_start", "context.shared_jobs"],
    );
    for request in ["r-first", "r-second"] {
        let submitted = ctx.submit(request, "2030-01-01T01:00:00Z");
        let outcome = at(result(&submitted), &["outcome"]);
        assert_eq!(text(outcome, &["state"]), "preparing", "{submitted:?}");
        assert_eq!(
            canonical(at(outcome, &["job"])),
            SHARED_JOB,
            "{submitted:?}"
        );
    }
    (ctx, clock)
}

/// The `context.request.changed` payload that refuses a subscriber of
/// [`SHARED_JOB`].
fn refused_in_shared_job() -> String {
    format!(r#"{{"job":{SHARED_JOB},"reason":"packet_invalid","state":"refused"}}"#)
}

#[test]
fn a_shared_job_whose_first_subscriber_was_cancelled_still_ends_for_the_one_left() {
    // **The job that ends is found by its own id, not by the request whose
    // packet was refused.** A shared job is named after its first
    // subscriber. Cancel that one and the job carries on for the second,
    // so when the guard refuses the packet the request being published is
    // `r-second`, in the job `r-first`. That job ends, once, and
    // `r-second`'s refusal names it.
    let directory = tempfile::tempdir().expect("temp dir");
    let (mut ctx, clock) = two_subscribers_of_one_refused_job(directory.path());
    let revision = match at(&ctx.inspect("r-first"), &["revision"]) {
        Value::Int(revision) => *revision,
        other => panic!("a revision: {other:?}"),
    };
    let cancelled = ctx.call(
        "context.request.cancel",
        Some(("cancel-r-first", ("context.request", "r-first"), revision)),
        "{}",
    );
    assert_eq!(
        at(result(&cancelled), &["outcome", "job_continues"]),
        &Value::Bool(true),
        "the premise: the job carries on for r-second: {cancelled:?}"
    );

    set_clock(&clock, "2030-01-01T00:10:00Z");
    let refused = settled(&mut ctx, "r-second");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    assert_eq!(text(&refused, &["reason"]), "packet_invalid", "{refused:?}");
    assert_eq!(canonical(at(&refused, &["job"])), SHARED_JOB, "{refused:?}");
    assert_eq!(canonical(at(&refused, &["items"])), REFUSED_ITEM);

    let events = recorded(&mut ctx);
    assert_eq!(
        of_subject(&events, "context.request", "r-second")
            .last()
            .expect("an event")
            .1,
        refused_in_shared_job(),
        "{events:?}"
    );
    assert_eq!(
        of_subject(&events, "context.job", "r-first"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    let logged = log(directory.path());
    assert_eq!(refusals(&logged, "r-second"), 1, "{logged}");
    ctx.kill();
}

#[test]
fn every_refusal_in_a_shared_job_names_the_job_and_not_the_request() {
    // **`context.request.changed` names the job a request was prepared
    // by.** For a shared job's first subscriber that is its own id as
    // well, so only the second tells the job from the request: each
    // subscriber is refused once, and each refusal names `r-first`.
    let directory = tempfile::tempdir().expect("temp dir");
    let (mut ctx, clock) = two_subscribers_of_one_refused_job(directory.path());
    set_clock(&clock, "2030-01-01T00:10:00Z");
    for request in ["r-first", "r-second"] {
        let refused = settled(&mut ctx, request);
        assert_eq!(
            text(&refused, &["state"]),
            "refused",
            "{request}: {refused:?}"
        );
    }
    let events = recorded(&mut ctx);
    for request in ["r-first", "r-second"] {
        let refusals: Vec<(String, String)> = of_subject(&events, "context.request", request)
            .into_iter()
            .filter(|(_, payload)| payload.contains(r#""state":"refused""#))
            .collect();
        assert_eq!(
            refusals,
            vec![(
                "context.request.changed".to_string(),
                refused_in_shared_job()
            )],
            "{request}: {events:?}"
        );
    }
    ctx.kill();
}

#[test]
fn a_subscriber_with_a_later_deadline_is_refused_with_the_job_an_earlier_deadline_ended() {
    // **A subscriber is refused with its job whatever brought the packet
    // on.** The job's script never publishes, so the packet the guard
    // refuses is the one `r-first`'s deadline, 00:05, publishes. `r-second`
    // joined the same job with a deadline an hour later, and is still
    // inside it when the job ends; it is refused all the same, since
    // nothing the ended job prepares will be published.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let script = format!(r#""r-first":[{}]"#, source_section(OUTSIDE_GRAMMAR));
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &scripted(
            &script,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
        &["context.required_before_start", "context.shared_jobs"],
    );
    for (request, deadline) in [
        ("r-first", "2030-01-01T00:05:00Z"),
        ("r-second", "2030-01-01T01:00:00Z"),
    ] {
        let submitted = ctx.submit(request, deadline);
        let outcome = at(result(&submitted), &["outcome"]);
        assert_eq!(text(outcome, &["state"]), "preparing", "{submitted:?}");
        assert_eq!(
            canonical(at(outcome, &["job"])),
            SHARED_JOB,
            "{submitted:?}"
        );
    }
    let before = ctx.inspect("r-second");
    assert_eq!(text(&before, &["state"]), "preparing", "{before:?}");

    set_clock(&clock, "2030-01-01T00:05:00Z");
    for request in ["r-first", "r-second"] {
        let refused = settled(&mut ctx, request);
        assert_eq!(
            text(&refused, &["state"]),
            "refused",
            "{request}: {refused:?}"
        );
        assert_eq!(
            text(&refused, &["reason"]),
            "packet_invalid",
            "{request}: {refused:?}"
        );
        assert_eq!(
            canonical(at(&refused, &["items"])),
            REFUSED_ITEM,
            "{request}"
        );
    }
    let events = recorded(&mut ctx);
    assert_eq!(
        of_subject(&events, "context.request", "r-second")
            .last()
            .expect("an event")
            .1,
        refused_in_shared_job(),
        "{events:?}"
    );
    assert_eq!(
        of_subject(&events, "context.job", "r-first"),
        vec![(
            "context.job.ended".to_string(),
            r#"{"reason":"packet_invalid"}"#.to_string()
        )],
        "{events:?}"
    );
    ctx.kill();
}

#[test]
fn a_request_submitted_after_the_refusal_never_joins_the_ended_job() {
    // **An ended job takes no new subscriber.** Under
    // `context.shared_jobs` a request joins a running job with the same
    // principal, basis, items and access scope, and `r-later` matches
    // `r-bad` in all of them. `r-bad`'s job has ended and never
    // published, so joining it would leave `r-later` `preparing` for
    // ever: no tick walks an ended job. It gets a job of its own.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &scripted(&refused_script("r-bad"), ""),
        &["context.required_before_start", "context.shared_jobs"],
    );
    ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");

    let later = ctx.submit("r-later", "2030-01-01T01:00:00Z");
    let outcome = at(result(&later), &["outcome"]);
    assert_eq!(text(outcome, &["state"]), "preparing", "{later:?}");
    assert_eq!(
        canonical(at(outcome, &["job"])),
        r#"{"id":"r-later","kind":"context.job"}"#,
        "a request joined a job that had ended: {later:?}"
    );
    ctx.kill();
}

/// Item `i-1` of [`ContextProvider::submit`]'s request, for a request
/// [`ContextProvider::submit_with`] builds.
const SOURCE_ITEM: &str = r#"{"item_id":"i-1","selector":{"kind":"path","value":"src/main.rs"},"obligation":"required_before_start","reliance":"binding","selected_by":"owner","check":{"kind":"source_included","repository":"repo-a","path":"src/main.rs"}}"#;

#[test]
fn a_request_published_unmet_or_partial_keeps_its_state_and_revision_when_a_later_update_is_refused()
 {
    // **A request delivered a packet keeps it, whatever the packet said.**
    // The update test above publishes revision 1 `ready`. Here `r-unmet`'s
    // revision 1 is `unmet`, nothing having satisfied `i-1` yet, and
    // `r-partial`'s is `partial`, `i-1` satisfied and an advisory item
    // degraded. Neither is `preparing` when its job's revision 2 is
    // refused, so neither is refused: each job ends, and each request
    // reads as it was published, state and revision included.
    let directory = tempfile::tempdir().expect("temp dir");
    let clock = directory.path().join("clock");
    set_clock(&clock, "2030-01-01T00:00:00Z");
    let then_refused = format!(
        r#"{{"wait_until":"2030-01-01T00:10:00Z"}},{},{{"publish":{{}}}}"#,
        source_section(OUTSIDE_GRAMMAR)
    );
    let scripts = format!(
        r#""r-unmet":[{{"publish":{{}}}},{then_refused}],"r-partial":[{},{{"publish":{{}}}},{then_refused}]"#,
        source_section("s-1")
    );
    let mut ctx = ContextProvider::start_with(
        directory.path(),
        &scripted(
            &scripts,
            &format!(r#","clock":{{"file":"{}"}}"#, clock.display()),
        ),
        &[
            "context.required_before_start",
            "context.advisory",
            "context.updates",
        ],
    );
    ctx.submit("r-unmet", "2030-01-01T01:00:00Z");
    ctx.submit_with(
        "r-partial",
        &format!("{SOURCE_ITEM},{}", unsatisfiable("optional", "advisory")),
        "proceed_with_gap",
        "2030-01-01T01:00:00Z",
    );
    let mut published = Vec::new();
    for (request, state) in [("r-unmet", "unmet"), ("r-partial", "partial")] {
        let first = (0..50)
            .find_map(|_| {
                let inspected = ctx.inspect(request);
                (at(&inspected, &["packets"]).as_array().map(<[_]>::len) == Some(1))
                    .then_some(inspected)
            })
            .unwrap_or_else(|| panic!("{request}: revision 1 was never published"));
        assert_eq!(
            text(&first, &["state"]),
            state,
            "the premise: {request}'s revision 1: {first:?}"
        );
        published.push((request, first));
    }

    set_clock(&clock, "2030-01-01T00:10:00Z");
    let events = (0..50)
        .find_map(|_| {
            let events = recorded(&mut ctx);
            (events
                .iter()
                .filter(|e| e.event == "context.job.ended")
                .count()
                == 2)
                .then_some(events)
        })
        .expect("the two jobs never ended: a refused update left one running");
    for (request, _) in &published {
        assert_eq!(
            of_subject(&events, "context.job", request),
            vec![(
                "context.job.ended".to_string(),
                r#"{"reason":"packet_invalid"}"#.to_string()
            )],
            "{request}: {events:?}"
        );
    }
    assert!(
        !events
            .iter()
            .any(|e| e.payload.contains(r#""state":"refused""#)),
        "a request that was delivered a packet was refused: {events:?}"
    );
    for (request, first) in &published {
        let after = ctx.inspect(request);
        assert_eq!(
            text(&after, &["state"]),
            text(first, &["state"]),
            "{request}: {after:?}"
        );
        assert_eq!(
            at(&after, &["revision"]),
            at(first, &["revision"]),
            "{request}: the request's revision moved"
        );
        assert_eq!(
            canonical(&after),
            canonical(first),
            "{request}: revision 1 is read as it was published"
        );
    }
    ctx.kill();
}

#[test]
fn the_log_line_of_a_refused_packet_says_its_job_has_ended_and_names_no_id() {
    // **The line reports what happened.** It is written after the ending
    // commits, and its last words say the job has ended, which here it
    // has. It names the job, the request and where each bad id is, and
    // never an id itself: the section id is a path.
    let directory = tempfile::tempdir().expect("temp dir");
    let mut ctx = ContextProvider::start(directory.path(), &scripted(&refused_script("r-bad"), ""));
    ctx.submit("r-bad", "2030-01-01T01:00:00Z");
    let refused = settled(&mut ctx, "r-bad");
    assert_eq!(text(&refused, &["state"]), "refused", "{refused:?}");
    let logged = log(directory.path());
    let lines: Vec<&str> = logged
        .lines()
        .filter(|line| line.contains("request r-bad:"))
        .collect();
    assert_eq!(
        lines,
        [
            "cbr-provider: context job r-bad: request r-bad: packet not published; an id at \
             /sections/0/section_id, /inclusions/0, /body/sections/0/section_id is missing, \
             outside the identifier grammar or repeated; the job has ended: packet_invalid"
        ],
        "{logged}"
    );
    assert!(
        !logged.contains("secret-path"),
        "the log repeats the id itself: {logged}"
    );
    ctx.kill();
}
