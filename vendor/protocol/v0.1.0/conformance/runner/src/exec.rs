//! Executing one fixture against one participant.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::matcher::{Vars, matches, render};
use crate::participant::{Descriptor, Launch, Process, Received, Session};
use crate::schemas::{Schemas, jsonrpc_code, retry_class};
use crate::strict;

const RESPONSE_TIMEOUT: Duration = Duration::from_millis(5000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Timeout,
    HarnessError,
    /// The fixture needs a profile or feature the participant does not claim.
    Unsupported,
    /// The fixture targets another transport binding than the participant's.
    Skipped,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::Timeout => "timeout",
            Outcome::HarnessError => "harness_error",
            Outcome::Unsupported => "unsupported",
            Outcome::Skipped => "skipped",
        }
    }
}

pub struct CaseResult {
    pub outcome: Outcome,
    pub step: Option<usize>,
    pub reason: Option<String>,
    pub transcript: Vec<Value>,
}

enum StepError {
    Fail(String),
    Timeout(String),
    Harness(String),
}

use StepError::{Fail, Harness, Timeout};

pub struct Context<'a> {
    pub descriptor: &'a Descriptor,
    pub schemas: &'a Schemas,
    pub repo: &'a Path,
    pub mutant: Option<&'a str>,
    /// A mutant of one client implementation: (implementation, mutant).
    pub client_mutant: Option<(&'a str, &'a str)>,
    pub work_dir: PathBuf,
}

struct State<'a> {
    ctx: &'a Context<'a>,
    fixture: &'a Value,
    vars: Vars,
    counter: u64,
    next_id: i64,
    sessions: BTreeMap<String, Session>,
    active: String,
    process: Option<Process>,
    socket_path: Option<PathBuf>,
    launches: u32,
    config_principal: String,
    notifications: BTreeMap<String, VecDeque<Value>>,
    awaiting_command: Option<Value>,
    data_generation: u32,
    transcript: Vec<Value>,
    started: Instant,
    /// Responses that arrived for requests sent with `await: false`, by session.
    responses: Vec<(String, Value)>,
    /// Requests sent with `await: false`, by name.
    pending: BTreeMap<String, Pending>,
    /// The last instant written to the controlled clock file.
    clock_value: Option<String>,
    /// Separately launched participants of a composition, by name (unix binding only).
    participants: BTreeMap<String, Named>,
    /// The named participant each session is connected to.
    session_participant: BTreeMap<String, String>,
    /// Client-only implementations launched by `start_client`, by name.
    clients: BTreeMap<String, Process>,
}

/// One separately running participant: its own process, data directory, configuration and socket.
struct Named {
    process: Process,
    socket: PathBuf,
}

struct Pending {
    session: String,
    id: Value,
    method: String,
}

const CONTROL_WAIT: Duration = Duration::from_millis(10_000);

/// Write a file so a reader never observes a partial write (decision 007).
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StepError> {
    let temporary = path.with_extension("tmp-write");
    std::fs::write(&temporary, bytes).map_err(|e| Harness(e.to_string()))?;
    std::fs::rename(&temporary, path).map_err(|e| Harness(e.to_string()))
}

/// Deterministic per-run credential for a principal (CORE section 18.1 format).
fn credential_for(work_dir: &Path, principal: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(format!("{}:{principal}", work_dir.display()).as_bytes());
    format!("ccred1.{principal}.{}", base64_url_nopad(&digest))
}

fn base64_url_nopad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        let symbols = chunk.len() + 1;
        for index in 0..symbols {
            out.push(ALPHABET[((value >> (18 - 6 * index)) & 63) as usize] as char);
        }
    }
    out
}

/// Whether a fixture applies to a participant (binding, profiles, features, test controls), and if not, why.
pub fn applicable(fixture: &Value, descriptor: &Descriptor) -> Result<(), (Outcome, String)> {
    // A pinned older participant runs only the fixtures written for it, and they run only there.
    let pinned = descriptor.raw["pinned"].as_str();
    match (fixture["requires_pinned"].as_str(), pinned) {
        (Some(wanted), Some(have)) if wanted == have => {}
        (Some(wanted), _) => {
            return Err((
                Outcome::Skipped,
                format!("fixture targets the pinned participant {wanted}"),
            ));
        }
        (None, Some(have)) => {
            return Err((
                Outcome::Skipped,
                format!("pinned participant {have} runs only fixtures that target it"),
            ));
        }
        (None, None) => {}
    }
    if let Some(binding) = fixture["binding"].as_str()
        && binding != descriptor.binding
    {
        return Err((
            Outcome::Skipped,
            format!("fixture requires the {binding} binding"),
        ));
    }
    for profile in fixture["profiles"].as_array().into_iter().flatten() {
        let wanted = (
            profile["name"].as_str().unwrap_or_default().to_string(),
            profile["major"].as_i64().unwrap_or_default(),
        );
        if !descriptor.profiles.contains(&wanted) {
            return Err((
                Outcome::Unsupported,
                format!("participant does not claim {}/{}", wanted.0, wanted.1),
            ));
        }
    }
    let claimed = |key: &str| -> Vec<String> {
        descriptor.raw["claims"][key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect()
    };
    let required = |key: &str| -> Vec<String> {
        fixture[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect()
    };
    let features = claimed("features");
    for feature in required("features") {
        if !features.contains(&feature) {
            return Err((
                Outcome::Unsupported,
                format!("participant does not claim feature {feature}"),
            ));
        }
    }
    let controls = claimed("test_controls");
    for control in required("requires_controls") {
        if !controls.contains(&control) {
            return Err((
                Outcome::Unsupported,
                format!("coverage limit: participant does not declare test control {control}"),
            ));
        }
    }
    let barriers = claimed("test_barriers");
    for barrier in required("requires_barriers") {
        if !barriers.contains(&barrier) {
            return Err((
                Outcome::Unsupported,
                format!("coverage limit: participant does not declare barrier {barrier}"),
            ));
        }
    }
    Ok(())
}

pub fn run_fixture(fixture: &Value, ctx: &Context) -> CaseResult {
    if let Err((outcome, reason)) = applicable(fixture, ctx.descriptor) {
        return CaseResult {
            outcome,
            step: None,
            reason: Some(reason),
            transcript: vec![],
        };
    }
    if let Err(error) = std::fs::create_dir_all(ctx.work_dir.join("data")) {
        return CaseResult {
            outcome: Outcome::HarnessError,
            step: None,
            reason: Some(error.to_string()),
            transcript: vec![],
        };
    }
    let mut state = State {
        ctx,
        fixture,
        vars: Vars::new(),
        counter: 0,
        next_id: 0,
        sessions: BTreeMap::new(),
        active: "main".into(),
        process: None,
        socket_path: None,
        launches: 0,
        config_principal: "conformance-caller".into(),
        notifications: BTreeMap::new(),
        awaiting_command: None,
        data_generation: 0,
        transcript: vec![],
        started: Instant::now(),
        responses: Vec::new(),
        pending: BTreeMap::new(),
        clock_value: None,
        participants: BTreeMap::new(),
        session_participant: BTreeMap::new(),
        clients: BTreeMap::new(),
    };
    // Named participants' sockets and principals' credentials are known before any starts, so
    // participants that reach each other can be configured in any order.
    if ctx.descriptor.binding == "unix" {
        for step in fixture["steps"].as_array().into_iter().flatten() {
            if step["step"] != "start_participant" {
                continue;
            }
            if let Some(name) = step["name"].as_str() {
                let socket = ctx.work_dir.join(format!("n-{name}")).join("p.sock");
                state.vars.insert(
                    format!("socket.{name}"),
                    json!(socket.display().to_string()),
                );
            }
            for principal in step["credential_principals"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                state.vars.insert(
                    format!("credential.{principal}"),
                    json!(credential_for(&ctx.work_dir, principal)),
                );
            }
        }
    }
    let mut failure = None;
    for (index, step) in fixture["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if let Err(error) = state.step(step) {
            failure = Some((index, error));
            break;
        }
    }
    state.shutdown();
    let (outcome, step, reason) = match failure {
        None => (Outcome::Pass, None, None),
        Some((index, Fail(reason))) => (Outcome::Fail, Some(index), Some(reason)),
        Some((index, Timeout(reason))) => (Outcome::Timeout, Some(index), Some(reason)),
        Some((index, Harness(reason))) => (Outcome::HarnessError, Some(index), Some(reason)),
    };
    CaseResult {
        outcome,
        step,
        reason,
        transcript: state.transcript,
    }
}

fn deep_merge(base: &mut Value, overlay: &Value) {
    if overlay.is_null() {
        return;
    }
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                deep_merge(base.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

impl State<'_> {
    fn session(&mut self) -> Result<&mut Session, StepError> {
        let name = self.active.clone();
        self.sessions.get_mut(&name).ok_or_else(|| {
            Harness(format!(
                "no participant session {name:?}; add a start or connect step"
            ))
        })
    }

    fn unix(&self) -> bool {
        self.ctx.descriptor.binding == "unix"
    }

    /// Close every session and stop a Unix-socket process; used before relaunching and at the end.
    fn shutdown(&mut self) {
        let names: Vec<String> = self.sessions.keys().cloned().collect();
        for name in names {
            if let Some(session) = self.sessions.remove(&name) {
                self.transcript.extend(session.finish());
            }
        }
        if let Some(mut process) = self.process.take()
            && process.stop(Duration::from_millis(3000)).is_none()
        {
            process.kill();
        }
        for (_, mut client) in std::mem::take(&mut self.clients) {
            if client.stop(Duration::from_millis(3000)).is_none() {
                client.kill();
            }
        }
        for (_, mut named) in std::mem::take(&mut self.participants) {
            if named.process.stop(Duration::from_millis(3000)).is_none() {
                named.process.kill();
            }
        }
        self.session_participant.clear();
    }

    fn authenticate_session(&mut self, principal: &str) -> Result<(), StepError> {
        let credential = credential_for(&self.ctx.work_dir, principal);
        let params = json!({"operation": "core.authenticate", "message_id": self.unique("auth"), "payload": {"credential": credential}});
        self.call(
            "core.authenticate",
            params,
            &json!({"expect": {"ok": {"principal": principal}}}),
        )
        .map(|_| ())
    }

    fn render(&mut self, template: &Value) -> Result<Value, StepError> {
        render(template, &self.vars, &mut self.counter).map_err(Harness)
    }

    fn step(&mut self, step: &Value) -> Result<(), StepError> {
        let kind = step["step"].as_str().unwrap_or_default();
        self.active = step["session"].as_str().unwrap_or("main").to_string();
        match kind {
            "connect" => {
                if !self.unix() {
                    return Err(Harness("connect needs a unix-binding participant".into()));
                }
                let path = match step["participant"].as_str() {
                    Some(name) => self
                        .participants
                        .get(name)
                        .map(|named| named.socket.clone())
                        .ok_or_else(|| {
                            Harness(format!("connect to {name:?} before start_participant"))
                        })?,
                    None => self
                        .socket_path
                        .clone()
                        .ok_or_else(|| Harness("connect before start".into()))?,
                };
                let session = Session::connect(&path, &self.active, self.started).map_err(Fail)?;
                self.sessions.insert(self.active.clone(), session);
                match step["participant"].as_str() {
                    Some(name) => self
                        .session_participant
                        .insert(self.active.clone(), name.to_string()),
                    None => self.session_participant.remove(&self.active),
                };
                if step["auto_authenticate"].as_bool().unwrap_or(true) {
                    let principal = step["principal"]
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| self.config_principal.clone());
                    self.authenticate_session(&principal)?;
                }
                Ok(())
            }
            "disconnect" => {
                let name = self.active.clone();
                if let Some(session) = self.sessions.remove(&name) {
                    self.transcript.extend(session.finish());
                }
                Ok(())
            }
            "expect_start_failure" => self.expect_start_failure(step),
            "start" => self.start(step),
            "stop" => self.stop(),
            "start_participant" => self.start_participant(step),
            "start_client" => self.start_client(step),
            "stop_client" => self.stop_client(step),
            "await_client_exit" => self.await_client_exit(step),
            "stop_participant" => self.stop_participant(step, false),
            "kill_participant" => self.stop_participant(step, true),
            "describe" => {
                let params = json!({"operation": "core.describe", "message_id": self.unique("msg"), "payload": {}});
                let frame = self.call("core.describe", params, step)?;
                if let Some(result) = frame.get("result") {
                    self.vars.insert("describe".into(), result.clone());
                }
                Ok(())
            }
            "negotiate" => self.negotiate(step),
            "request" => {
                let params = self.render(&step["params"])?;
                let method = step["method"].as_str().unwrap_or_default().to_string();
                self.call(&method, params, step).map(|_| ())
            }
            "command" => {
                let params = self.command_envelope(step)?;
                let method = step["method"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| {
                        params["operation"].as_str().unwrap_or_default().to_string()
                    });
                if step["await"] == json!(false) {
                    return self.send_pending(&method, params, step);
                }
                self.call(&method, params, step).map(|_| ())
            }
            "query" => {
                let params = self.query_envelope(step)?;
                let method = step["method"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| {
                        params["operation"].as_str().unwrap_or_default().to_string()
                    });
                if step["await"] == json!(false) {
                    return self.send_pending(&method, params, step);
                }
                if let Some(bound) = step["eventually_ms"].as_u64() {
                    // For asynchronous compositions: repeat the query until the expectation holds
                    // or the real-time bound passes; only the last mismatch is reported.
                    let deadline = Instant::now() + Duration::from_millis(bound);
                    loop {
                        let params = self.query_envelope(step)?;
                        match self.call(&method, params, step) {
                            Ok(_) => return Ok(()),
                            Err(Fail(reason)) if Instant::now() < deadline => {
                                self.note("eventually_retry", json!({"reason": reason}));
                                std::thread::sleep(Duration::from_millis(50));
                            }
                            Err(Fail(reason)) => {
                                return Err(Fail(format!("not within {bound} ms: {reason}")));
                            }
                            Err(other) => return Err(other),
                        }
                    }
                }
                self.call(&method, params, step).map(|_| ())
            }
            "notify" => {
                let params = self.command_envelope(&step["command"])?;
                let method = params["operation"].as_str().unwrap_or_default().to_string();
                let bytes = frame_bytes(
                    &json!({"jsonrpc": "2.0", "method": method, "params": params}),
                    true,
                );
                self.session()?.send(&bytes);
                Ok(())
            }
            "raw" => self.raw(step),
            "expect_frame" => {
                let frame = self.receive_frame(duration(step, RESPONSE_TIMEOUT))?;
                self.check(&frame, &step["expect"], None)?;
                self.capture(&frame, step)
            }
            "expect_close" => {
                let timeout = duration(step, Duration::from_millis(3000));
                match self.session()?.receive(timeout) {
                    Received::Closed => {
                        // The connection is over: a later start relaunches the participant.
                        let name = self.active.clone();
                        if let Some(session) = self.sessions.remove(&name) {
                            self.transcript.extend(session.finish());
                        }
                        Ok(())
                    }
                    Received::Timeout => Err(Fail("connection was not closed".into())),
                    Received::Frame(frame) => Err(Fail(format!(
                        "expected the connection to close, received {}",
                        String::from_utf8_lossy(&frame[..frame.len().min(300)])
                    ))),
                }
            }
            "expect_notification" => {
                let frame = self.next_notification(duration(step, RESPONSE_TIMEOUT))?;
                if let Some(pattern) = step["expect"].get("params") {
                    matches(pattern, frame.get("params"), &self.vars, "params").map_err(Fail)?;
                }
                if let Some(pattern) = step["expect"].get("frame") {
                    matches(pattern, Some(&frame), &self.vars, "frame").map_err(Fail)?;
                }
                self.capture(&frame, step)
            }
            "expect_no_notification" => {
                // Barrier: a describe round trip flushes any notification owed before it.
                let params = json!({"operation": "core.describe", "message_id": self.unique("barrier"), "payload": {}});
                self.call("core.describe", params, &json!({}))?;
                match self
                    .notifications
                    .entry(self.active.clone())
                    .or_default()
                    .pop_front()
                {
                    None => Ok(()),
                    Some(frame) => Err(Fail(format!("unexpected notification {frame}"))),
                }
            }
            "close_input" => {
                self.session()?.close_input();
                Ok(())
            }
            "set_clock" => self.set_clock(step),
            "kill" => self.kill(),
            "expect_response" => self.expect_response(step),
            "await_barrier" => self.await_barrier(step),
            "release_barrier" => {
                let name = step["name"].as_str().unwrap_or_default();
                let path = self.barrier_directory().join(format!("{name}.release"));
                write_atomic(&path, b"")?;
                self.note("release_barrier", json!({"name": name}));
                Ok(())
            }
            "await_any" => self.await_any(step),
            "pause_reading" => {
                self.session()?.pause_reading();
                Ok(())
            }
            "resume_reading" => {
                self.session()?.resume_reading();
                Ok(())
            }
            "collect_until_close" => self.collect_until_close(step),
            "expect_signal_gap" => self.expect_signal_gap(step),
            other => Err(Harness(format!("unknown step {other:?}"))),
        }
    }

    fn note(&mut self, event: &str, detail: Value) {
        self.transcript.push(json!({"t_ms": self.started.elapsed().as_millis() as u64, "event": event, "session": self.active, "detail": detail}));
    }

    fn barrier_directory(&self) -> PathBuf {
        self.ctx.work_dir.join("test-controls").join("barriers")
    }

    fn next_request_id(&mut self, step: &Value) -> Result<Value, StepError> {
        match step.get("id") {
            Some(id) => self.render(id),
            None => {
                self.next_id += 1;
                Ok(json!(self.next_id))
            }
        }
    }

    /// Send without waiting (decision 007 §5). Pipelining does not establish commit order.
    fn send_pending(&mut self, method: &str, params: Value, step: &Value) -> Result<(), StepError> {
        let name = step["name"]
            .as_str()
            .ok_or_else(|| Harness("a step with await false needs a name".into()))?
            .to_string();
        let id = self.next_request_id(step)?;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.session()?.send(&frame_bytes(&request, true));
        let session = self.active.clone();
        self.pending.insert(
            name,
            Pending {
                session,
                id,
                method: method.to_string(),
            },
        );
        Ok(())
    }

    /// Keep a response that belongs to a pending request on the active session.
    fn buffer_if_pending(&mut self, frame: &Value) -> bool {
        let belongs = self
            .pending
            .values()
            .any(|p| p.session == self.active && frame.get("id") == Some(&p.id));
        if belongs {
            self.responses.push((self.active.clone(), frame.clone()));
        }
        belongs
    }

    fn buffered_response(&self, pending: &Pending) -> Option<usize> {
        self.responses.iter().position(|(session, frame)| {
            *session == pending.session && frame.get("id") == Some(&pending.id)
        })
    }

    fn expect_response(&mut self, step: &Value) -> Result<(), StepError> {
        let name = step["name"].as_str().unwrap_or_default().to_string();
        let pending = self
            .pending
            .remove(&name)
            .ok_or_else(|| Harness(format!("expect_response: no pending request named {name}")))?;
        self.active = pending.session.clone();
        let deadline = Instant::now() + duration(step, RESPONSE_TIMEOUT);
        let frame = loop {
            if let Some(index) = self.buffered_response(&pending) {
                break self.responses.remove(index).1;
            }
            let frame = self.receive_frame(deadline.saturating_duration_since(Instant::now()))?;
            if frame.get("id") == Some(&pending.id) {
                break frame;
            }
            if !self.buffer_if_pending(&frame) {
                return Err(Fail(format!(
                    "unexpected response {frame} while waiting for {name}"
                )));
            }
        };
        self.check(
            &frame,
            step.get("expect").unwrap_or(&json!({"ok": {}})),
            Some(&pending.method),
        )?;
        self.capture(&frame, step)
    }

    fn set_clock(&mut self, step: &Value) -> Result<(), StepError> {
        let file = self.ctx.work_dir.join("test-controls").join("clock");
        if self.clock_value.is_none() {
            return Err(Harness(
                "set_clock needs a start step with clock.controlled".into(),
            ));
        }
        if let Some(raw) = step["raw"].as_str() {
            write_atomic(&file, raw.as_bytes())?;
            self.note("set_clock", json!({"raw": raw}));
            return Ok(());
        }
        let instant = step["instant"]
            .as_str()
            .ok_or_else(|| Harness("set_clock needs instant or raw".into()))?
            .to_string();
        let last = self.clock_value.clone().unwrap_or_default();
        if instant < last && step["allow_backward"] != json!(true) {
            return Err(Harness(format!(
                "set_clock would move the test clock backward from {last} to {instant}"
            )));
        }
        write_atomic(&file, instant.as_bytes())?;
        self.note("set_clock", json!({"instant": instant}));
        if instant > last {
            self.clock_value = Some(instant);
        }
        Ok(())
    }

    fn kill(&mut self) -> Result<(), StepError> {
        if self.unix() {
            let names: Vec<String> = self.sessions.keys().cloned().collect();
            if let Some(mut process) = self.process.take() {
                process.kill();
            }
            for name in names {
                if let Some(session) = self.sessions.remove(&name) {
                    self.transcript.extend(session.finish());
                }
            }
        } else if let Some(mut session) = self.sessions.remove("main") {
            session.kill();
            self.transcript.extend(session.finish());
        }
        self.note("kill", json!({}));
        Ok(())
    }

    fn await_barrier(&mut self, step: &Value) -> Result<(), StepError> {
        let name = step["name"].as_str().unwrap_or_default().to_string();
        let directory = self.barrier_directory();
        let reached = directory.join(format!("{name}.reached"));
        let bound = duration(step, CONTROL_WAIT);
        let deadline = Instant::now() + bound;
        while !reached.exists() {
            if Instant::now() > deadline {
                return Err(Timeout(format!(
                    "barrier {name} was not reached within {} ms",
                    bound.as_millis()
                )));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        // Signals emitted before the pause are stale for what follows.
        for entry in std::fs::read_dir(&directory).map_err(|e| Harness(e.to_string()))? {
            let path = entry.map_err(|e| Harness(e.to_string()))?.path();
            if path.extension().is_some_and(|ext| ext == "signal") {
                let _ = std::fs::remove_file(path);
            }
        }
        self.note("await_barrier", json!({"name": name}));
        Ok(())
    }

    /// Read everything a connection still delivers until it closes (TRN-4). The named
    /// subscription's notifications must carry contiguous event sequences, only the last may end
    /// it, and `expect_last` is matched against that last notification's params. Captures the
    /// last `next_cursor` (`capture_cursor`) and the sequence after the last event
    /// (`capture_next_sequence`). Responses to pending requests are buffered; others are noted.
    fn collect_until_close(&mut self, step: &Value) -> Result<(), StepError> {
        let bound = duration(step, Duration::from_millis(10_000));
        let deadline = Instant::now() + bound;
        let subscription = self.render(&step["subscription"])?;
        let mut notifications: Vec<Value> = self
            .notifications
            .entry(self.active.clone())
            .or_default()
            .drain(..)
            .collect();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let received = self.session()?.receive(remaining);
            let bytes = match received {
                Received::Frame(bytes) => bytes,
                Received::Closed => break,
                Received::Timeout => {
                    return Err(Timeout(format!(
                        "connection was not closed within {} ms",
                        bound.as_millis()
                    )));
                }
            };
            let frame = strict::parse(&bytes).map_err(|e| {
                Fail(format!(
                    "participant sent a frame outside the value domain: {e}"
                ))
            })?;
            if frame.get("id").is_none() && frame.get("method").is_some() {
                self.ctx.schemas.check_notification(&frame).map_err(Fail)?;
                notifications.push(frame);
            } else if !self.buffer_if_pending(&frame) {
                self.note("collected_response", json!({"id": frame.get("id")}));
            }
        }
        let name = self.active.clone();
        if let Some(session) = self.sessions.remove(&name) {
            self.transcript.extend(session.finish());
        }
        let mine: Vec<&Value> = notifications
            .iter()
            .filter(|frame| frame["params"]["subscription"] == subscription)
            .collect();
        let mut previous: Option<(i64, i64)> = None;
        let mut events = 0;
        for (index, frame) in mine.iter().enumerate() {
            if frame["params"].get("ended").is_some() && index + 1 != mine.len() {
                return Err(Fail(format!(
                    "notification {index} ended the subscription but more followed"
                )));
            }
            if let Some(ended) = frame["params"].get("ended")
                && step["forbid_ended"] == true
            {
                return Err(Fail(format!(
                    "params/ended: the subscription was ended with {ended}"
                )));
            }
            for item in frame["params"]["items"].as_array().into_iter().flatten() {
                let Some(event) = item.get("event") else {
                    previous = None;
                    continue;
                };
                let position = (
                    event["epoch"].as_i64().unwrap_or_default(),
                    event["sequence"].as_i64().unwrap_or_default(),
                );
                if let Some((epoch, sequence)) = previous
                    && epoch == position.0
                    && position.1 != sequence + 1
                {
                    return Err(Fail(format!(
                        "subscription skipped from sequence {sequence} to {}",
                        position.1
                    )));
                }
                previous = Some(position);
                events += 1;
            }
        }
        self.note(
            "collect_until_close",
            json!({"notifications": mine.len(), "events": events}),
        );
        if let Some(pattern) = step.get("expect_last") {
            let last = mine
                .last()
                .ok_or_else(|| Fail("no notification for the subscription before close".into()))?;
            matches(pattern, last.get("params"), &self.vars, "params").map_err(Fail)?;
        }
        // Without a complete notification, the consumer's last durable cursor is unchanged.
        if let (Some(name), Some(last)) = (step["capture_cursor"].as_str(), mine.last()) {
            self.vars
                .insert(name.to_string(), last["params"]["next_cursor"].clone());
        }
        if let Some(name) = step["capture_next_sequence"].as_str() {
            let (_, sequence) =
                previous.ok_or_else(|| Fail("no event before close to resume after".into()))?;
            self.vars.insert(name.to_string(), json!(sequence + 1));
        }
        Ok(())
    }

    /// Check the time between two signals the participant emitted, from their files' modification
    /// times (decision 007 §4): `min_ms <= to - from <= max_ms`.
    fn expect_signal_gap(&mut self, step: &Value) -> Result<(), StepError> {
        let directory = self.barrier_directory();
        let modified = |name: &str| -> Result<std::time::SystemTime, StepError> {
            std::fs::metadata(directory.join(format!("{name}.signal")))
                .and_then(|m| m.modified())
                .map_err(|_| Fail(format!("signal {name} was not emitted")))
        };
        let from = step["from"].as_str().unwrap_or_default();
        let to = step["to"].as_str().unwrap_or_default();
        let gap = modified(to)?
            .duration_since(modified(from)?)
            .map_err(|_| Fail(format!("signal {to} was emitted before {from}")))?
            .as_millis() as u64;
        self.note(
            "expect_signal_gap",
            json!({"from": from, "to": to, "gap_ms": gap}),
        );
        if let Some(max) = step["max_ms"].as_u64()
            && gap > max
        {
            return Err(Fail(format!(
                "signal gap {from} to {to} was {gap} ms, above the bound {max} ms"
            )));
        }
        if let Some(min) = step["min_ms"].as_u64()
            && gap < min
        {
            return Err(Fail(format!(
                "signal gap {from} to {to} was {gap} ms, below {min} ms"
            )));
        }
        Ok(())
    }

    /// Wait until any named signal exists or every named response has arrived (decision 007 §4).
    fn await_any(&mut self, step: &Value) -> Result<(), StepError> {
        let strings = |key: &str| -> Vec<String> {
            step[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        };
        let signals = strings("signals");
        let responses = strings("responses");
        let directory = self.barrier_directory();
        let bound = duration(step, CONTROL_WAIT);
        let deadline = Instant::now() + bound;
        loop {
            if let Some(signal) = signals
                .iter()
                .find(|s| directory.join(format!("{s}.signal")).exists())
            {
                self.note(
                    "await_any",
                    json!({"satisfied_by": format!("signal {signal}")}),
                );
                return Ok(());
            }
            let mut waiting = Vec::new();
            for name in &responses {
                let pending = self.pending.get(name).ok_or_else(|| {
                    Harness(format!("await_any: no pending request named {name}"))
                })?;
                if self.buffered_response(pending).is_none() {
                    waiting.push(pending.session.clone());
                }
            }
            if !responses.is_empty() && waiting.is_empty() {
                self.note(
                    "await_any",
                    json!({"satisfied_by": "responses", "responses": responses}),
                );
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(Fail(format!(
                    "await_any: no signal {signals:?} and not every response {responses:?} within {} ms",
                    bound.as_millis()
                )));
            }
            if waiting.is_empty() {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            for session in waiting {
                self.active = session;
                match self.receive_frame(Duration::from_millis(5)) {
                    Ok(frame) => {
                        if !self.buffer_if_pending(&frame) {
                            return Err(Fail(format!(
                                "unexpected response {frame} during await_any"
                            )));
                        }
                    }
                    Err(Timeout(_)) => {}
                    Err(other) => return Err(other),
                }
            }
        }
    }

    fn unique(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}-{}", self.counter)
    }

    fn launch_config(&mut self, step: &Value) -> Result<Value, StepError> {
        let mut config =
            json!({"format": "combraton-conformance-config/1", "principal": "conformance-caller"});
        deep_merge(&mut config, &self.fixture["config"]);
        if step.get("config").is_some() {
            deep_merge(&mut config, &step["config"]);
        }
        self.config_principal = config["principal"]
            .as_str()
            .unwrap_or("conformance-caller")
            .to_string();
        if self.unix() {
            let mut principals: Vec<String> = vec![self.config_principal.clone()];
            for list in [
                &config["authority_principals"],
                &step["credential_principals"],
                &step["revoked_principals"],
            ] {
                principals.extend(
                    list.as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(String::from),
                );
            }
            principals.sort();
            principals.dedup();
            let revoked: Vec<&str> = step["revoked_principals"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut credentials = Vec::new();
            for principal in &principals {
                let credential = credential_for(&self.ctx.work_dir, principal);
                self.vars
                    .insert(format!("credential.{principal}"), json!(credential));
                credentials.push(json!({"credential": credential, "revoked": revoked.contains(&principal.as_str())}));
            }
            config["credentials"] = json!(credentials);
        }
        let controls = self.ctx.work_dir.join("test-controls");
        if let Some(instant) = config["clock"]["controlled"].as_str().map(String::from) {
            std::fs::create_dir_all(&controls).map_err(|e| Harness(e.to_string()))?;
            let file = controls.join("clock");
            write_atomic(&file, instant.as_bytes())?;
            // Clients read the same controlled clock (M6 third-party compositions).
            self.vars
                .insert("clock_file".into(), json!(file.display().to_string()));
            self.clock_value = Some(instant);
            config["clock"] = json!({"file": file.display().to_string()});
        } else if let Some(raw) = config["clock"]["controlled_raw"].as_str().map(String::from) {
            // Deliberately unusable clock content, to test refusal at start.
            std::fs::create_dir_all(&controls).map_err(|e| Harness(e.to_string()))?;
            let file = controls.join("clock");
            write_atomic(&file, raw.as_bytes())?;
            config["clock"] = json!({"file": file.display().to_string()});
        }
        if let Some(enabled) = step["barriers"].as_array() {
            let directory = controls.join("barriers");
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).map_err(|e| Harness(e.to_string()))?;
            config["test_barriers"] =
                json!({"directory": directory.display().to_string(), "enabled": enabled});
        }
        if let Some(error) = self.ctx.schemas.launch_config.iter_errors(&config).next() {
            return Err(Harness(format!(
                "fixture launch configuration is invalid at {}: {error}",
                error.instance_path()
            )));
        }
        Ok(config)
    }

    fn prepare_socket_directory(&mut self, unsafe_mode: bool) -> Result<PathBuf, StepError> {
        use std::os::unix::fs::PermissionsExt;
        self.launches += 1;
        let directory = self.ctx.work_dir.join(format!("s{}", self.launches));
        std::fs::create_dir_all(&directory).map_err(|e| Harness(e.to_string()))?;
        let mode = if unsafe_mode { 0o777 } else { 0o700 };
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(mode))
            .map_err(|e| Harness(e.to_string()))?;
        Ok(directory.join("p.sock"))
    }

    fn start(&mut self, step: &Value) -> Result<(), StepError> {
        // Launch configuration may name captured values, as start_participant's does.
        let mut rendered = step.clone();
        if step.get("config").is_some() {
            rendered["config"] = self.render(&step["config"])?;
        }
        let step = &rendered;
        let config = self.launch_config(step)?;
        let config_file = self.ctx.work_dir.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec_pretty(&config).unwrap())
            .map_err(|e| Harness(e.to_string()))?;
        if step["fresh_data_directory"].as_bool().unwrap_or(false) {
            self.data_generation += 1;
        }
        let data_dir = if self.data_generation == 0 {
            self.ctx.work_dir.join("data")
        } else {
            self.ctx
                .work_dir
                .join(format!("data-{}", self.data_generation))
        };
        let stderr_file = self.ctx.work_dir.join("participant-stderr.log");
        self.notifications.clear();
        self.vars.remove("negotiation");
        if !self.unix() {
            if self.sessions.contains_key("main") {
                return Err(Harness("participant already started".into()));
            }
            let launch = Launch {
                descriptor: self.ctx.descriptor,
                repo: self.ctx.repo,
                data_dir: &data_dir,
                config_file: &config_file,
                socket_path: None,
                stderr_file,
                mutant: self.ctx.mutant,
            };
            self.sessions.insert(
                "main".into(),
                Session::start(&launch, self.started).map_err(Harness)?,
            );
            return Ok(());
        }
        self.shutdown();
        let socket = self.prepare_socket_directory(false)?;
        let launch = Launch {
            descriptor: self.ctx.descriptor,
            repo: self.ctx.repo,
            data_dir: &data_dir,
            config_file: &config_file,
            socket_path: Some(&socket),
            stderr_file,
            mutant: self.ctx.mutant,
        };
        let mut process = Process::spawn(&launch).map_err(Harness)?;
        let deadline = Instant::now() + Duration::from_millis(5000);
        while !socket.exists() {
            if let Some(code) = process.exited() {
                return Err(Fail(format!(
                    "participant exited with status {code} before listening"
                )));
            }
            if Instant::now() > deadline {
                return Err(Timeout(
                    "participant socket did not appear within 5000 ms".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.transcript.push(json!({"t_ms": self.started.elapsed().as_millis() as u64, "event": "start", "detail": {"argv": process.argv}}));
        self.process = Some(process);
        self.socket_path = Some(socket.clone());
        self.active = "main".into();
        // The socket file can exist a moment before the provider accepts connections.
        let session = loop {
            match Session::connect(&socket, "main", self.started) {
                Ok(session) => break session,
                Err(error) if Instant::now() > deadline => return Err(Fail(error)),
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        self.sessions.insert("main".into(), session);
        if step["auto_authenticate"].as_bool().unwrap_or(true) {
            let principal = self.config_principal.clone();
            self.authenticate_session(&principal)?;
        }
        Ok(())
    }

    /// Launch a client-only implementation (M6-Q2) from `conformance/thirdparty/<implementation>/`
    /// with a rendered configuration. It runs until its standard input closes or it exits.
    fn start_client(&mut self, step: &Value) -> Result<(), StepError> {
        let name = step["name"]
            .as_str()
            .ok_or_else(|| Harness("start_client needs a name".into()))?
            .to_string();
        let implementation = step["implementation"]
            .as_str()
            .ok_or_else(|| Harness("start_client needs an implementation".into()))?
            .to_string();
        if self.clients.contains_key(&name) {
            return Err(Harness(format!("client {name:?} is already running")));
        }
        let path = self
            .ctx
            .repo
            .join("conformance/thirdparty")
            .join(&implementation)
            .join("participant.json");
        let descriptor = Descriptor::load_client(&path).map_err(Harness)?;
        let config = self.render(&step["config"])?;
        let directory = self.ctx.work_dir.join("clients").join(&name);
        std::fs::create_dir_all(directory.join("data")).map_err(|e| Harness(e.to_string()))?;
        let config_file = directory.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec_pretty(&config).unwrap())
            .map_err(|e| Harness(e.to_string()))?;
        let mutant = self
            .ctx
            .client_mutant
            .filter(|(target, _)| *target == implementation)
            .map(|(_, mutant)| mutant);
        let data_dir = directory.join("data");
        let launch = Launch {
            descriptor: &descriptor,
            repo: self.ctx.repo,
            data_dir: &data_dir,
            config_file: &config_file,
            socket_path: None,
            stderr_file: directory.join("stderr.log"),
            mutant,
        };
        let process = Process::spawn(&launch).map_err(Harness)?;
        self.note(
            "start_client",
            json!({"name": name, "implementation": implementation, "argv": process.argv, "mutant": mutant}),
        );
        self.clients.insert(name, process);
        Ok(())
    }

    /// End a client's input and require the exit status (default 0).
    fn stop_client(&mut self, step: &Value) -> Result<(), StepError> {
        let name = step["name"].as_str().unwrap_or_default().to_string();
        let mut client = self
            .clients
            .remove(&name)
            .ok_or_else(|| Harness(format!("no running client {name:?}")))?;
        let expected = step["exit_code"].as_i64().unwrap_or(0);
        self.note("stop_client", json!({"name": name}));
        match client.stop(Duration::from_millis(5000)) {
            Some(code) if i64::from(code) == expected => Ok(()),
            Some(code) => Err(Fail(format!(
                "client {name:?} exited with status {code}; {expected} is required"
            ))),
            None => {
                client.kill();
                Err(Fail(format!(
                    "client {name:?} did not exit after its input ended"
                )))
            }
        }
    }

    /// Wait for a client that ends by itself, and require its exit status (default 0).
    fn await_client_exit(&mut self, step: &Value) -> Result<(), StepError> {
        let name = step["name"].as_str().unwrap_or_default().to_string();
        let expected = step["exit_code"].as_i64().unwrap_or(0);
        let bound = step["within_ms"].as_u64().unwrap_or(10_000);
        let deadline = Instant::now() + Duration::from_millis(bound);
        loop {
            let client = self
                .clients
                .get_mut(&name)
                .ok_or_else(|| Harness(format!("no running client {name:?}")))?;
            if let Some(code) = client.exited() {
                self.clients.remove(&name);
                self.note("client_exit", json!({"name": name, "code": code}));
                return if i64::from(code) == expected {
                    Ok(())
                } else {
                    Err(Fail(format!(
                        "client {name:?} exited with status {code}; {expected} is required"
                    )))
                };
            }
            if Instant::now() > deadline {
                return Err(Timeout(format!(
                    "client {name:?} did not exit within {bound} ms"
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Launch a named participant as its own process with its own data directory, configuration
    /// and socket (owner decision M4-Q3). Sessions reach it with `connect` and `participant`.
    fn start_participant(&mut self, step: &Value) -> Result<(), StepError> {
        if !self.unix() {
            return Err(Harness(
                "start_participant needs a unix-binding participant".into(),
            ));
        }
        let name = step["name"]
            .as_str()
            .ok_or_else(|| Harness("start_participant needs a name".into()))?
            .to_string();
        if self.participants.contains_key(&name) {
            return Err(Harness(format!("participant {name:?} is already running")));
        }
        let mut launch_step = step.clone();
        launch_step["config"] = self.render(&step["config"])?;
        let caller_principal = self.config_principal.clone();
        let config = self.launch_config(&launch_step)?;
        self.config_principal = caller_principal;
        let directory = self.ctx.work_dir.join("participants").join(&name);
        std::fs::create_dir_all(&directory).map_err(|e| Harness(e.to_string()))?;
        let config_file = directory.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec_pretty(&config).unwrap())
            .map_err(|e| Harness(e.to_string()))?;
        let data_dir = directory.join("data");
        // A stable socket path per participant, so peers configured with it reach a restart.
        let socket_directory = self.ctx.work_dir.join(format!("n-{name}"));
        std::fs::create_dir_all(&socket_directory).map_err(|e| Harness(e.to_string()))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&socket_directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| Harness(e.to_string()))?;
        }
        let socket = socket_directory.join("p.sock");
        let _ = std::fs::remove_file(&socket);
        let launch = Launch {
            descriptor: self.ctx.descriptor,
            repo: self.ctx.repo,
            data_dir: &data_dir,
            config_file: &config_file,
            socket_path: Some(&socket),
            stderr_file: directory.join("stderr.log"),
            mutant: self.ctx.mutant,
        };
        let mut process = Process::spawn(&launch).map_err(Harness)?;
        let deadline = Instant::now() + Duration::from_millis(5000);
        while !(socket.exists() && std::os::unix::net::UnixStream::connect(&socket).is_ok()) {
            if let Some(code) = process.exited() {
                return Err(Fail(format!(
                    "participant {name:?} exited with status {code} before listening"
                )));
            }
            if Instant::now() > deadline {
                return Err(Timeout(format!(
                    "participant {name:?} did not listen within 5000 ms"
                )));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.note(
            "start_participant",
            json!({"name": name, "argv": process.argv}),
        );
        self.vars.insert(
            format!("socket.{name}"),
            json!(socket.display().to_string()),
        );
        self.participants.insert(name, Named { process, socket });
        Ok(())
    }

    /// Stop (end of input, exit status 0 required) or kill a named participant and its sessions.
    fn stop_participant(&mut self, step: &Value, kill: bool) -> Result<(), StepError> {
        let name = step["name"].as_str().unwrap_or_default().to_string();
        let mut named = self
            .participants
            .remove(&name)
            .ok_or_else(|| Harness(format!("no running participant {name:?}")))?;
        let sessions: Vec<String> = self
            .session_participant
            .iter()
            .filter(|(_, participant)| **participant == name)
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            self.session_participant.remove(&session);
            if let Some(session) = self.sessions.remove(&session) {
                self.transcript.extend(session.finish());
            }
        }
        self.note(
            if kill {
                "kill_participant"
            } else {
                "stop_participant"
            },
            json!({"name": name}),
        );
        if kill {
            named.process.kill();
            return Ok(());
        }
        match named.process.stop(Duration::from_millis(5000)) {
            Some(0) => Ok(()),
            Some(code) => Err(Fail(format!(
                "participant {name:?} exited with status {code}; 0 is required"
            ))),
            None => {
                named.process.kill();
                Err(Fail(format!(
                    "participant {name:?} did not exit after its input ended"
                )))
            }
        }
    }

    fn expect_start_failure(&mut self, step: &Value) -> Result<(), StepError> {
        if !self.unix() {
            return Err(Harness(
                "expect_start_failure needs a unix-binding participant".into(),
            ));
        }
        self.shutdown();
        let config = self.launch_config(step)?;
        let config_file = self.ctx.work_dir.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec_pretty(&config).unwrap())
            .map_err(|e| Harness(e.to_string()))?;
        let socket = self
            .prepare_socket_directory(step["unsafe_socket_directory"].as_bool().unwrap_or(false))?;
        if step["fresh_data_directory"].as_bool().unwrap_or(false) {
            self.data_generation += 1;
        }
        let data_dir = if self.data_generation == 0 {
            self.ctx.work_dir.join("data")
        } else {
            self.ctx
                .work_dir
                .join(format!("data-{}", self.data_generation))
        };
        let launch = Launch {
            descriptor: self.ctx.descriptor,
            repo: self.ctx.repo,
            data_dir: &data_dir,
            config_file: &config_file,
            socket_path: Some(&socket),
            stderr_file: self.ctx.work_dir.join("participant-stderr.log"),
            mutant: self.ctx.mutant,
        };
        let mut process = Process::spawn(&launch).map_err(Harness)?;
        let deadline = Instant::now() + Duration::from_millis(5000);
        loop {
            if let Some(code) = process.exited() {
                return if code != 0 && !socket.exists() {
                    Ok(())
                } else {
                    Err(Fail(format!(
                        "participant exited with status {code}; expected a refusal to start"
                    )))
                };
            }
            if socket.exists() && std::os::unix::net::UnixStream::connect(&socket).is_ok() {
                process.kill();
                return Err(Fail(
                    "participant listened although the fixture expects it to refuse to start"
                        .into(),
                ));
            }
            if Instant::now() > deadline {
                process.kill();
                return Err(Fail(
                    "participant neither refused to start nor listened".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&mut self) -> Result<(), StepError> {
        if self.unix() {
            let names: Vec<String> = self.sessions.keys().cloned().collect();
            for name in names {
                if let Some(session) = self.sessions.remove(&name) {
                    self.transcript.extend(session.finish());
                }
            }
            let mut process = self
                .process
                .take()
                .ok_or_else(|| Harness("stop before start".into()))?;
            return match process.stop(Duration::from_millis(5000)) {
                Some(0) => Ok(()),
                Some(code) => Err(Fail(format!(
                    "participant exited with status {code} after its input ended; 0 is required"
                ))),
                None => {
                    process.kill();
                    Err(Fail(
                        "participant did not exit after its input ended".into(),
                    ))
                }
            };
        }
        self.active = "main".into();
        let session = self.session()?;
        session.close_input();
        match session.receive(Duration::from_millis(5000)) {
            Received::Closed => {}
            Received::Timeout => {
                return Err(Fail(
                    "participant did not close its output after input ended".into(),
                ));
            }
            Received::Frame(_) => return Err(Fail("unexpected frame after input ended".into())),
        }
        match session.wait_exit(Duration::from_millis(5000)) {
            None => return Err(Fail("participant did not exit after input ended".into())),
            Some(0) => {}
            Some(code) => {
                return Err(Fail(format!(
                    "participant exited with status {code} after input ended; STREAM section 5 requires 0"
                )));
            }
        }
        let session = self.sessions.remove("main").unwrap();
        self.transcript.extend(session.finish());
        Ok(())
    }

    fn negotiate(&mut self, step: &Value) -> Result<(), StepError> {
        let payload = if step.get("payload").is_some() {
            self.render(&step["payload"])?
        } else {
            let profiles: Vec<Value> = self.fixture["profiles"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| json!({"name": p["name"], "majors": [p["major"]], "required": true, "required_features": [], "optional_features": []}))
                .collect();
            json!({
                "caller": {"name": "combraton-conformance", "version": env!("CARGO_PKG_VERSION")},
                "receive_limits": {"max_frame_bytes": 1_048_576},
                "profiles": profiles,
            })
        };
        let params = json!({"operation": "core.negotiate", "message_id": self.unique("msg"), "payload": payload});
        let frame = self.call("core.negotiate", params, step)?;
        if let Some(result) = frame.get("result") {
            if let Some(limit) = payload["receive_limits"]["max_frame_bytes"].as_u64() {
                self.session()?.receive_limit = limit as usize;
            }
            self.vars.insert(
                "generation".into(),
                result["dedupe_window"]["current"].clone(),
            );
            self.vars.insert("negotiation".into(), result.clone());
        }
        Ok(())
    }

    fn apply_overrides(&mut self, envelope: &mut Value, step: &Value) -> Result<(), StepError> {
        if let Some(set) = step.get("set").and_then(Value::as_object) {
            for (key, value) in set.clone() {
                let rendered = self.render(&value)?;
                envelope[key] = rendered;
            }
        }
        for key in step
            .get("remove")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            envelope
                .as_object_mut()
                .unwrap()
                .remove(key.as_str().unwrap_or_default());
        }
        Ok(())
    }

    fn command_envelope(&mut self, step: &Value) -> Result<Value, StepError> {
        let generation = match step.get("dedupe_generation") {
            Some(value) => self.render(value)?,
            None => self.vars.get("generation").cloned().ok_or_else(|| {
                Harness("command before negotiation; set dedupe_generation".into())
            })?,
        };
        let mut envelope = Map::new();
        envelope.insert("operation".into(), step["operation"].clone());
        envelope.insert("message_id".into(), json!(self.unique("msg")));
        envelope.insert("command_id".into(), self.render(&step["command_id"])?);
        envelope.insert("dedupe_generation".into(), generation);
        envelope.insert("subject".into(), self.render(&step["subject"])?);
        envelope.insert("preconditions".into(), self.render(&step["preconditions"])?);
        if let Some(epoch) = step.get("authority_epoch") {
            envelope.insert("authority_epoch".into(), self.render(epoch)?);
        }
        envelope.insert(
            "requires".into(),
            step.get("requires")
                .map_or(Ok(json!([])), |r| self.render(r))?,
        );
        if let Some(extensions) = step.get("extensions") {
            envelope.insert("extensions".into(), self.render(extensions)?);
        }
        envelope.insert("payload".into(), self.render(&step["payload"])?);
        let mut envelope = Value::Object(envelope);
        self.apply_overrides(&mut envelope, step)?;
        let digest = match step.get("digest") {
            Some(Value::String(literal)) => json!(literal),
            _ => json!(strict::intent_digest(&envelope)),
        };
        if envelope.get("command_digest").is_none() && step.get("digest") != Some(&json!(false)) {
            envelope["command_digest"] = digest;
        }
        Ok(envelope)
    }

    fn query_envelope(&mut self, step: &Value) -> Result<Value, StepError> {
        let mut envelope = json!({"operation": step["operation"], "message_id": self.unique("msg"), "payload": self.render(&step["payload"])?});
        self.apply_overrides(&mut envelope, step)?;
        Ok(envelope)
    }

    fn raw(&mut self, step: &Value) -> Result<(), StepError> {
        let terminate = step["terminate"].as_bool().unwrap_or(true);
        let mut bytes = if let Some(text) = step["text"].as_str() {
            text.as_bytes().to_vec()
        } else if let Some(hex_text) = step["hex"].as_str() {
            hex::decode(hex_text).map_err(|e| Harness(e.to_string()))?
        } else if step.get("command").is_some() {
            let params = self.command_envelope(&step["command"])?;
            self.next_id += 1;
            let request = json!({"jsonrpc": "2.0", "id": self.next_id, "method": params["operation"], "params": params});
            frame_bytes(&request, false)
        } else if let Some(padded) = step.get("padded") {
            let message = self.render(&padded["message"])?;
            let target = padded["frame_bytes"].as_u64().unwrap_or_default() as usize;
            let compact = frame_bytes(&message, false);
            if compact.len() > target || compact.first() != Some(&b'{') {
                return Err(Harness(format!(
                    "padded message is {} bytes, longer than {target}",
                    compact.len()
                )));
            }
            let mut padded_bytes = vec![b'{'];
            padded_bytes.extend(std::iter::repeat_n(b' ', target - compact.len()));
            padded_bytes.extend_from_slice(&compact[1..]);
            padded_bytes
        } else {
            return Err(Harness(
                "raw step needs text, hex, command or padded".into(),
            ));
        };
        if terminate {
            bytes.push(b'\n');
        }
        self.session()?.send(&bytes);
        Ok(())
    }

    /// Next frame that is not a provider notification; notifications are validated and queued.
    fn receive_frame(&mut self, timeout: Duration) -> Result<Value, StepError> {
        let deadline = Instant::now() + timeout;
        loop {
            let frame =
                self.receive_raw_frame(deadline.saturating_duration_since(Instant::now()))?;
            if frame.get("id").is_none() && frame.get("method").is_some() {
                self.ctx.schemas.check_notification(&frame).map_err(Fail)?;
                if let Some(command_id) = &self.awaiting_command {
                    let early = frame["params"]["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|item| {
                            item["event"]["origin"] == "command"
                                && &item["event"]["command_id"] == command_id
                        });
                    if early {
                        return Err(Fail(format!(
                            "notification for command {command_id} arrived before its response (CORE section 16.5)"
                        )));
                    }
                }
                self.notifications
                    .entry(self.active.clone())
                    .or_default()
                    .push_back(frame);
                continue;
            }
            return Ok(frame);
        }
    }

    fn next_notification(&mut self, timeout: Duration) -> Result<Value, StepError> {
        if let Some(frame) = self
            .notifications
            .entry(self.active.clone())
            .or_default()
            .pop_front()
        {
            return Ok(frame);
        }
        let frame = self.receive_raw_frame(timeout)?;
        if frame.get("id").is_none() && frame.get("method").is_some() {
            self.ctx.schemas.check_notification(&frame).map_err(Fail)?;
            return Ok(frame);
        }
        Err(Fail(format!("expected a notification, received {frame}")))
    }

    fn receive_raw_frame(&mut self, timeout: Duration) -> Result<Value, StepError> {
        let session = self.session()?;
        let limit = session.receive_limit;
        let frame = match session.receive(timeout) {
            Received::Frame(frame) if frame.len() > limit => {
                return Err(Fail(format!(
                    "participant sent a {} byte frame; the caller's receive limit is {limit} (STREAM section 1)",
                    frame.len()
                )));
            }
            Received::Frame(frame) => frame,
            Received::Closed => return Err(Fail("participant closed the connection".into())),
            Received::Timeout => {
                return Err(Timeout(format!(
                    "no frame within {} ms",
                    timeout.as_millis()
                )));
            }
        };
        strict::parse(&frame).map_err(|e| {
            Fail(format!(
                "participant sent a frame outside the value domain: {e}"
            ))
        })
    }

    fn call(&mut self, method: &str, params: Value, step: &Value) -> Result<Value, StepError> {
        let id = self.next_request_id(step)?;
        self.awaiting_command = params.get("command_id").cloned();
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let bytes = frame_bytes(&request, true);
        self.session()?.send(&bytes);
        let deadline = Instant::now() + duration(step, RESPONSE_TIMEOUT);
        let received = loop {
            match self.receive_frame(deadline.saturating_duration_since(Instant::now())) {
                Ok(frame) if frame.get("id") != Some(&id) && self.buffer_if_pending(&frame) => {}
                other => break other,
            }
        };
        self.awaiting_command = None;
        let frame = received?;
        if frame.get("id") != Some(&id) {
            return Err(Fail(format!(
                "response id {} does not match request id {id}",
                frame.get("id").unwrap_or(&Value::Null)
            )));
        }
        self.check(
            &frame,
            step.get("expect").unwrap_or(&json!({"ok": {}})),
            Some(method),
        )?;
        self.capture(&frame, step)?;
        Ok(frame)
    }

    fn check(&self, frame: &Value, expect: &Value, method: Option<&str>) -> Result<(), StepError> {
        if let Some(alternatives) = expect.get("any_of").and_then(Value::as_array) {
            let mut reasons = Vec::new();
            for alternative in alternatives {
                match self.check(frame, alternative, method) {
                    Ok(()) => return Ok(()),
                    Err(Fail(reason)) => reasons.push(reason),
                    Err(other) => return Err(other),
                }
            }
            return Err(Fail(format!(
                "no alternative matched: {}",
                reasons.join(" | ")
            )));
        }
        self.ctx
            .schemas
            .check_response(frame, method)
            .map_err(Fail)?;
        if let Some(excluded) = expect.get("frame_text_excludes").and_then(Value::as_array) {
            let text = frame.to_string();
            for item in excluded {
                let rendered = render(item, &self.vars, &mut 0).map_err(Harness)?;
                if let Some(needle) = rendered.as_str()
                    && text.contains(needle)
                {
                    return Err(Fail("frame contains text it must not reveal".into()));
                }
            }
        }
        if let Some(pattern) = expect.get("ok") {
            let result = frame.get("result").ok_or_else(|| {
                Fail(format!(
                    "expected success, received error {}",
                    frame["error"]["data"]
                ))
            })?;
            return matches(pattern, Some(result), &self.vars, "result").map_err(Fail);
        }
        if let Some(code) = expect.get("error").and_then(Value::as_str) {
            let error = frame.get("error").ok_or_else(|| {
                Fail(format!(
                    "expected error {code}, received success {}",
                    frame["result"]
                ))
            })?;
            let data = &error["data"];
            if data["code"] != code {
                return Err(Fail(format!(
                    "expected error {code}, received {} ({})",
                    data["code"], data["details"]
                )));
            }
            if error["code"] != jsonrpc_code(code) {
                return Err(Fail(format!(
                    "error {code} must use JSON-RPC code {}, received {}",
                    jsonrpc_code(code),
                    error["code"]
                )));
            }
            if data["retry"] != retry_class(code) {
                return Err(Fail(format!(
                    "error {code} must have retry {}, received {}",
                    retry_class(code),
                    data["retry"]
                )));
            }
            if let Some(details) = expect.get("details") {
                matches(details, Some(&data["details"]), &self.vars, "details").map_err(Fail)?;
            }
            if expect.get("id_null") == Some(&json!(true)) && frame["id"] != Value::Null {
                return Err(Fail("error must have id null".into()));
            }
            if let Some(pattern) = expect.get("frame") {
                matches(pattern, Some(frame), &self.vars, "frame").map_err(Fail)?;
            }
            return Ok(());
        }
        if let Some(pattern) = expect.get("frame") {
            return matches(pattern, Some(frame), &self.vars, "frame").map_err(Fail);
        }
        Ok(())
    }

    fn capture(&mut self, frame: &Value, step: &Value) -> Result<(), StepError> {
        for (name, pointer) in step
            .get("capture")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let pointer = pointer.as_str().unwrap_or_default();
            let value = frame
                .pointer(pointer)
                .ok_or_else(|| Fail(format!("capture {name}: nothing at {pointer}")))?;
            self.vars.insert(name.clone(), value.clone());
        }
        Ok(())
    }
}

fn frame_bytes(message: &Value, terminate: bool) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).expect("serializable");
    if terminate {
        bytes.push(b'\n');
    }
    bytes
}

fn duration(step: &Value, default: Duration) -> Duration {
    step.get("within_ms")
        .and_then(Value::as_u64)
        .map_or(default, Duration::from_millis)
}
