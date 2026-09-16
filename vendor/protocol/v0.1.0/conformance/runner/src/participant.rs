//! Launching a participant under test over the stdio binding, with a transcript.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const RUNNER_FRAME_LIMIT: usize = 64 * 1024 * 1024;

pub struct Descriptor {
    pub raw: Value,
    pub binding: String,
    pub name: String,
    pub version: String,
    pub argv: Vec<String>,
    pub mutant_argv: Vec<String>,
    pub mutants: Vec<String>,
    pub profiles: Vec<(String, i64)>,
    pub digest: String,
}

impl Descriptor {
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::load_role(path, "provider")
    }

    /// A client-only implementation (M6-Q2): it serves nothing and reaches providers named in its
    /// configuration. Composition fixtures launch it with `start_client`.
    pub fn load_client(path: &Path) -> Result<Self, String> {
        Self::load_role(path, "client")
    }

    fn load_role(path: &Path, role: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw = crate::strict::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if raw["format"] != "combraton-conformance-participant/1" {
            return Err(format!(
                "{}: format must be combraton-conformance-participant/1",
                path.display()
            ));
        }
        if !matches!(raw["binding"].as_str(), Some("stdio" | "unix")) || raw["role"] != role {
            return Err(format!(
                "{}: expected a {role} descriptor with binding stdio or unix",
                path.display()
            ));
        }
        let strings = |value: &Value| -> Vec<String> {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        };
        Ok(Self {
            binding: raw["binding"].as_str().unwrap_or("stdio").to_string(),
            name: raw["name"].as_str().unwrap_or_default().into(),
            version: raw["version"].as_str().unwrap_or_default().into(),
            argv: strings(&raw["launch"]["argv"]),
            mutant_argv: strings(&raw["mutants"]["argv"]),
            mutants: strings(&raw["mutants"]["names"]),
            profiles: raw["claims"]["profiles"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| {
                    (
                        p["name"].as_str().unwrap_or_default().to_string(),
                        p["major"].as_i64().unwrap_or_default(),
                    )
                })
                .collect(),
            digest: crate::strict::sha256(&crate::strict::canonical(&raw)),
            raw,
        })
    }
}

pub enum Received {
    Frame(Vec<u8>),
    Closed,
    Timeout,
}

enum Event {
    Frame(Vec<u8>),
    Eof,
}

/// One connection to a participant: the stdio pipes of a process it owns, or a Unix socket.
pub struct Session {
    child: Option<Child>,
    writer: Option<Box<dyn Write + Send>>,
    stream: Option<UnixStream>,
    events: Receiver<Event>,
    gate: ReadGate,
    closed: bool,
    started: Instant,
    label: String,
    transcript: Vec<Value>,
    /// Largest frame this caller accepts: 1 MiB until a negotiation advertises another value.
    pub receive_limit: usize,
}

pub struct Launch<'a> {
    pub descriptor: &'a Descriptor,
    pub repo: &'a Path,
    pub data_dir: &'a Path,
    pub config_file: &'a Path,
    pub socket_path: Option<&'a Path>,
    pub stderr_file: PathBuf,
    pub mutant: Option<&'a str>,
}

fn spawn(launch: &Launch, stdout: Stdio) -> Result<(Child, Vec<String>), String> {
    let socket = launch
        .socket_path
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let substitute = |arg: &str| {
        arg.replace("{repo}", &launch.repo.display().to_string())
            .replace("{data_dir}", &launch.data_dir.display().to_string())
            .replace("{config_file}", &launch.config_file.display().to_string())
            .replace("{socket_path}", &socket)
            .replace("{mutant}", launch.mutant.unwrap_or_default())
    };
    let mut argv: Vec<String> = launch
        .descriptor
        .argv
        .iter()
        .map(|a| substitute(a))
        .collect();
    if launch.mutant.is_some() {
        argv.extend(launch.descriptor.mutant_argv.iter().map(|a| substitute(a)));
    }
    let (program, rest) = argv
        .split_first()
        .ok_or("participant launch argv is empty")?;
    let stderr = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&launch.stderr_file)
        .map_err(|e| e.to_string())?;
    let child = Command::new(program)
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| format!("cannot launch {program}: {e}"))?;
    Ok((child, argv))
}

/// Whether the runner is reading a connection; paused reading models a slow consumer (TRN-4).
type ReadGate = Arc<(Mutex<bool>, Condvar)>;

fn frame_reader(mut input: impl Read + Send + 'static, gate: ReadGate) -> Receiver<Event> {
    let (sender, events) = channel();
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 65_536];
        loop {
            {
                let (paused, resumed) = &*gate;
                let mut paused = paused.lock().unwrap_or_else(|e| e.into_inner());
                while *paused {
                    paused = resumed.wait(paused).unwrap_or_else(|e| e.into_inner());
                }
            }
            match input.read(&mut chunk) {
                Ok(0) | Err(_) => {
                    let _ = sender.send(Event::Eof);
                    return;
                }
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    while let Some(newline) = buffer.iter().position(|b| *b == b'\n') {
                        let frame: Vec<u8> = buffer.drain(..=newline).collect();
                        if sender
                            .send(Event::Frame(frame[..frame.len() - 1].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    if buffer.len() > RUNNER_FRAME_LIMIT {
                        let _ = sender.send(Event::Eof);
                        return;
                    }
                }
            }
        }
    });
    events
}

/// A Unix-socket participant process; it runs until its standard input is closed.
pub struct Process {
    child: Child,
    stdin: Option<ChildStdin>,
    pub argv: Vec<String>,
}

impl Process {
    pub fn spawn(launch: &Launch) -> Result<Self, String> {
        let (mut child, argv) = spawn(launch, Stdio::null())?;
        let stdin = child.stdin.take();
        Ok(Self { child, stdin, argv })
    }

    pub fn exited(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
            _ => None,
        }
    }

    /// Close the lifecycle pipe and wait; returns the exit status if the process exited in time.
    pub fn stop(&mut self, timeout: Duration) -> Option<i32> {
        self.stdin = None;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(code) = self.exited() {
                return Some(code);
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if self.stop(Duration::from_millis(500)).is_none() {
            self.kill();
        }
    }
}

impl Session {
    pub fn start(launch: &Launch, started: Instant) -> Result<Self, String> {
        let (mut child, argv) = spawn(launch, Stdio::piped())?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stdin = child
            .stdin
            .take()
            .map(|stdin| Box::new(stdin) as Box<dyn Write + Send>);
        let gate: ReadGate = Arc::new((Mutex::new(false), Condvar::new()));
        let mut session = Self {
            child: Some(child),
            writer: stdin,
            stream: None,
            events: frame_reader(stdout, gate.clone()),
            gate,
            closed: false,
            started,
            label: "main".into(),
            transcript: Vec::new(),
            receive_limit: 1_048_576,
        };
        session.note("start", &json!({"argv": argv}));
        Ok(session)
    }

    pub fn connect(path: &Path, label: &str, started: Instant) -> Result<Self, String> {
        let stream = UnixStream::connect(path)
            .map_err(|e| format!("cannot connect to {}: {e}", path.display()))?;
        let reader = stream.try_clone().map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let gate: ReadGate = Arc::new((Mutex::new(false), Condvar::new()));
        let mut session = Self {
            child: None,
            writer: Some(Box::new(writer)),
            stream: Some(stream),
            events: frame_reader(reader, gate.clone()),
            gate,
            closed: false,
            started,
            label: label.to_string(),
            transcript: Vec::new(),
            receive_limit: 1_048_576,
        };
        session.note("connect", &json!({"socket": path.display().to_string()}));
        Ok(session)
    }

    fn elapsed(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    pub fn note(&mut self, event: &str, detail: &Value) {
        let entry = json!({"t_ms": self.elapsed() as u64, "session": self.label, "event": event, "detail": detail});
        self.transcript.push(entry);
    }

    fn record_bytes(&mut self, direction: &str, bytes: &[u8]) {
        const SHOWN: usize = 2048;
        let shown = &bytes[..bytes.len().min(SHOWN)];
        let body = match std::str::from_utf8(shown) {
            Ok(text) => json!({"text": text}),
            Err(_) => json!({"hex": hex::encode(shown)}),
        };
        let mut detail = json!({"bytes": bytes.len()});
        detail
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        if bytes.len() > SHOWN {
            detail["truncated"] = json!(true);
        }
        self.note(direction, &detail);
    }

    /// Write bytes; a peer that has already closed is not a runner error.
    pub fn send(&mut self, bytes: &[u8]) {
        self.record_bytes("send", bytes);
        let failed = match self.writer.as_mut() {
            Some(writer) => writer
                .write_all(bytes)
                .and_then(|()| writer.flush())
                .is_err(),
            None => true,
        };
        if failed {
            self.note("send_failed", &json!({"reason": "peer input closed"}));
        }
    }

    /// Stop reading from the connection, so its buffers fill (runner step `pause_reading`).
    pub fn pause_reading(&mut self) {
        self.set_paused(true);
    }

    pub fn resume_reading(&mut self) {
        self.set_paused(false);
    }

    fn set_paused(&mut self, paused: bool) {
        let (state, resumed) = &*self.gate;
        *state.lock().unwrap_or_else(|e| e.into_inner()) = paused;
        resumed.notify_all();
        self.note(
            if paused {
                "pause_reading"
            } else {
                "resume_reading"
            },
            &json!({}),
        );
    }

    pub fn close_input(&mut self) {
        self.note("close_input", &json!({}));
        self.writer = None;
        if let Some(stream) = &self.stream {
            let _ = stream.shutdown(Shutdown::Write);
        }
    }

    pub fn receive(&mut self, timeout: Duration) -> Received {
        if self.closed {
            return Received::Closed;
        }
        match self.events.recv_timeout(timeout) {
            Ok(Event::Frame(frame)) => {
                self.record_bytes("recv", &frame);
                Received::Frame(frame)
            }
            Ok(Event::Eof) | Err(RecvTimeoutError::Disconnected) => {
                self.closed = true;
                self.note("peer_closed", &json!({}));
                Received::Closed
            }
            Err(RecvTimeoutError::Timeout) => Received::Timeout,
        }
    }

    /// Exit status of the owned stdio process, if it exits within `timeout`.
    pub fn wait_exit(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = self.child.as_mut().map(Child::try_wait) {
                self.note("exit", &json!({"code": status.code()}));
                return Some(status.code().unwrap_or(-1));
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }

    /// SIGKILL an owned stdio process (runner step `kill`, decision 007).
    pub fn kill(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
            self.note("killed", &json!({"reason": "kill step"}));
        }
    }

    /// Close the connection (and stop an owned stdio process); return the transcript.
    pub fn finish(mut self) -> Vec<Value> {
        self.set_paused(false);
        self.writer = None;
        if let Some(stream) = &self.stream {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if self.child.is_some() && self.wait_exit(Duration::from_millis(2000)).is_none() {
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.note(
                "killed",
                &json!({"reason": "did not exit after input closed"}),
            );
        }
        std::mem::take(&mut self.transcript)
    }
}
