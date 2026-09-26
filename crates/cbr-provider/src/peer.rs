//! A protocol client for reaching another participant (CONTEXT section 12).
//!
//! A context provider configured with an evidence provider or a knowledge
//! provider reaches it only as an ordinary caller would: over that
//! participant's public Unix socket, authenticated with a credential that
//! participant issued, negotiated, and acting under a grant that participant
//! issued. Nothing is shared but the protocol, so a peer's store stays its
//! own and a grant here never authorizes anything there (CONTEXT section 7).
//!
//! The credential is sent in the authenticate frame and nowhere else. No
//! failure this module reports carries it, the socket path, or a peer's
//! error details: only the peer's error code (CORE section 18.1).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use cbr_encoding::Value;

/// How long a peer may take to answer one frame before the call fails. A
/// context provider calls peers while holding its processing lock, so this
/// bounds how long its own callers can be kept waiting by a stalled peer.
const TIMEOUT: Duration = Duration::from_secs(3);

/// A peer as the `context` launch configuration names it:
/// `{ provider_id, socket, credential, grant? }`.
#[derive(Clone)]
pub struct PeerConfig {
    pub provider_id: String,
    socket: PathBuf,
    credential: String,
    grant: Option<String>,
}

impl std::fmt::Debug for PeerConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PeerConfig({})", self.provider_id)
    }
}

impl PeerConfig {
    /// Read a peer entry; `None` when it is absent or incomplete.
    pub fn from_value(value: Option<&Value>) -> Option<Self> {
        let value = value?;
        let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_string);
        Some(Self {
            provider_id: text("provider_id")?,
            socket: text("socket")?.into(),
            credential: text("credential")?,
            grant: text("grant"),
        })
    }
}

/// Why a peer call did not succeed.
#[derive(Debug)]
pub enum Failure {
    /// The peer could not be reached, or the connection failed mid-call.
    Unreachable(&'static str),
    /// The peer answered with a protocol error, named by its code.
    Refused(String),
}

impl Failure {
    pub fn describe(&self) -> String {
        match self {
            Failure::Unreachable(reason) => format!("unreachable: {reason}"),
            Failure::Refused(code) => format!("refused: {code}"),
        }
    }
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn string(text: &str) -> Value {
    Value::String(text.into())
}

fn strings(items: &[&str]) -> Value {
    Value::Array(items.iter().map(|item| string(item)).collect())
}

pub struct Peer {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: i64,
    generation: i64,
    grant: Option<String>,
}

impl Peer {
    /// Connect, authenticate and negotiate `core/1` with `core.events` (and
    /// `core.grants` when the peer entry names a grant) plus `profile`.
    pub fn connect(config: &PeerConfig, profile: &str) -> Result<Self, Failure> {
        let stream = UnixStream::connect(&config.socket)
            .map_err(|_| Failure::Unreachable("the peer socket refused the connection"))?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
            .map_err(|_| Failure::Unreachable("the peer socket could not be configured"))?;
        let writer = stream
            .try_clone()
            .map_err(|_| Failure::Unreachable("the peer socket could not be shared"))?;
        let mut peer = Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 0,
            generation: 1,
            grant: None,
        };
        peer.call(
            "core.authenticate",
            object(vec![
                ("operation", string("core.authenticate")),
                ("message_id", string("peer-authenticate")),
                (
                    "payload",
                    object(vec![("credential", string(&config.credential))]),
                ),
            ]),
        )?;
        let mut core_features = vec!["core.events"];
        if config.grant.is_some() {
            core_features.push("core.grants");
        }
        let negotiated = peer.call(
            "core.negotiate",
            object(vec![
                ("operation", string("core.negotiate")),
                ("message_id", string("peer-negotiate")),
                (
                    "payload",
                    object(vec![
                        (
                            "caller",
                            object(vec![
                                ("name", string("cbr-provider")),
                                ("version", string(env!("CARGO_PKG_VERSION"))),
                            ]),
                        ),
                        (
                            "receive_limits",
                            object(vec![("max_frame_bytes", Value::Int(1_048_576))]),
                        ),
                        (
                            "profiles",
                            Value::Array(vec![
                                object(vec![
                                    ("name", string("core")),
                                    ("majors", Value::Array(vec![Value::Int(1)])),
                                    ("required", Value::Bool(true)),
                                    ("required_features", strings(&core_features)),
                                    ("optional_features", Value::Array(vec![])),
                                ]),
                                object(vec![
                                    ("name", string(profile)),
                                    ("majors", Value::Array(vec![Value::Int(1)])),
                                    ("required", Value::Bool(true)),
                                    ("required_features", Value::Array(vec![])),
                                    ("optional_features", Value::Array(vec![])),
                                ]),
                            ]),
                        ),
                    ]),
                ),
            ]),
        )?;
        if let Some(Value::Int(current)) = negotiated
            .get("dedupe_window")
            .and_then(|window| window.get("current"))
        {
            peer.generation = *current;
        }
        peer.grant = config.grant.clone();
        Ok(peer)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        self.next_id += 1;
        let id = self.next_id;
        let frame = object(vec![
            ("jsonrpc", string("2.0")),
            ("id", Value::Int(id)),
            ("method", string(method)),
            ("params", params),
        ]);
        let mut bytes = cbr_encoding::to_canonical(&frame);
        bytes.push(b'\n');
        self.writer
            .write_all(&bytes)
            .map_err(|_| Failure::Unreachable("writing to the peer failed"))?;
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|_| Failure::Unreachable("reading from the peer failed"))?;
            if read == 0 {
                return Err(Failure::Unreachable("the peer closed the connection"));
            }
            let response = cbr_encoding::parse(line.trim_end().as_bytes())
                .map_err(|_| Failure::Unreachable("the peer sent an unparsable frame"))?;
            // A notification carries no id and is not this call's answer.
            if response.get("id") != Some(&Value::Int(id)) {
                continue;
            }
            if let Some(result) = response.get("result") {
                return Ok(result.clone());
            }
            let code = response
                .get("error")
                .and_then(|error| error.get("data"))
                .and_then(|data| data.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("error");
            return Err(Failure::Refused(code.to_string()));
        }
    }

    pub fn query(&mut self, operation: &str, payload: Value) -> Result<Value, Failure> {
        let mut params = vec![
            ("operation", string(operation)),
            (
                "message_id",
                string(&format!("peer-query-{}", self.next_id + 1)),
            ),
        ];
        if let Some(grant) = &self.grant {
            params.push(("grant", string(grant)));
        }
        params.push(("payload", payload));
        self.call(operation, object(params))
    }

    /// A command with one precondition on its own subject, carrying the digest
    /// the peer recomputes.
    pub fn command(
        &mut self,
        operation: &str,
        command_id: &str,
        subject: Value,
        revision: i64,
        payload: Value,
    ) -> Result<Value, Failure> {
        let mut envelope = vec![
            ("operation", string(operation)),
            ("message_id", string(&format!("peer-command-{command_id}"))),
            ("command_id", string(command_id)),
            ("dedupe_generation", Value::Int(self.generation)),
            ("subject", subject.clone()),
            (
                "preconditions",
                Value::Array(vec![object(vec![
                    ("subject", subject),
                    ("revision", Value::Int(revision)),
                ])]),
            ),
            ("requires", Value::Array(vec![])),
        ];
        if let Some(grant) = &self.grant {
            envelope.push(("grant", string(grant)));
        }
        envelope.push(("payload", payload));
        let mut envelope = object(envelope);
        let digest = cbr_encoding::command_digest(&envelope)
            .map_err(|_| Failure::Unreachable("a peer command could not be digested"))?;
        if let Value::Object(members) = &mut envelope {
            members.push(("command_digest".into(), string(&digest)));
        }
        self.call(operation, envelope)
    }
}

/// Publish provider-produced bytes as one sealed artifact at an evidence peer.
///
/// Command ids derive from the artifact id and offsets alone, so a publication
/// retried after a failure part-way replays the steps that already applied
/// instead of duplicating them (CORE section 6). That only works while the
/// descriptor is the same bytes, which is why its capture instant is fixed by
/// the caller across attempts, and kept durably before the first one.
pub fn publish_artifact(
    config: &PeerConfig,
    artifact: &str,
    descriptor: Value,
    content: &[u8],
) -> Result<(), Failure> {
    let mut peer = Peer::connect(config, "evidence")?;
    let subject = object(vec![
        ("kind", string(crate::evidence::ARTIFACT)),
        ("id", string(artifact)),
    ]);
    let prepared = peer.command(
        "evidence.upload.prepare",
        &format!("publish.{artifact}.prepare"),
        subject.clone(),
        0,
        descriptor,
    )?;
    let chunk_limit = match prepared
        .get("outcome")
        .and_then(|outcome| outcome.get("chunk_limit"))
    {
        Some(Value::Int(limit)) if *limit > 0 => *limit as usize,
        _ => return Err(Failure::Unreachable("the peer gave no usable chunk limit")),
    };
    let mut revision = 1;
    for (index, part) in content.chunks(chunk_limit).enumerate() {
        let offset = index * chunk_limit;
        peer.command(
            "evidence.upload.append",
            &format!("publish.{artifact}.append.{offset}"),
            subject.clone(),
            revision,
            object(vec![
                ("offset", Value::Int(offset as i64)),
                ("data_base64", string(&cbr_encoding::encode_base64(part))),
            ]),
        )?;
        revision += 1;
    }
    peer.command(
        "evidence.seal",
        &format!("publish.{artifact}.seal"),
        subject,
        revision,
        object(vec![]),
    )?;
    Ok(())
}
