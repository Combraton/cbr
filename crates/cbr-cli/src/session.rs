//! One authenticated, negotiated session over the provider's Unix socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use cbr_encoding::Value;

/// Every `context/1` feature, asked for as optional. A client that asks for
/// none of them cannot submit an advisory item, which is most of them.
const CONTEXT_FEATURES: [&str; 7] = [
    "context.advisory",
    "context.required_before_start",
    "context.required_before_transition",
    "context.shared_jobs",
    "context.updates",
    "context.expand",
    "context.claims",
];

pub fn string(text: &str) -> Value {
    Value::String(text.into())
}

pub fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

pub fn subject(kind: &str, id: &str) -> Value {
    object(vec![("kind", string(kind)), ("id", string(id))])
}

pub fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("canonical form is UTF-8")
}

/// A value at a path of member names, or `Null`.
/// Set or replace one member of an object, keeping the members sorted as the
/// canonical form needs them.
pub fn set(value: &mut Value, name: &str, member: Value) {
    if let Value::Object(members) = value {
        members.retain(|(existing, _)| existing != name);
        members.push((name.to_string(), member));
        members.sort_by(|left, right| left.0.cmp(&right.0));
    }
}

pub fn at(value: &Value, path: &[&str]) -> Value {
    let mut current = value;
    for name in path {
        match current.get(name) {
            Some(next) => current = next,
            None => return Value::Null,
        }
    }
    current.clone()
}

pub struct Session {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: i64,
    generation: i64,
    /// The grant every protected request names, when the principal acts under
    /// one rather than as an authority principal.
    grant: Option<String>,
}

impl Session {
    /// Authenticate with the handed-off credential and negotiate `core/1`
    /// with `core.events` (and `core.grants` when it is served) plus each of
    /// `profiles`.
    pub fn open(
        socket: &Path,
        credential_file: &Path,
        profiles: &[&str],
        grant: Option<String>,
    ) -> Result<Self, String> {
        let credential = std::fs::read_to_string(credential_file)
            .map_err(|error| format!("reading the credential file: {error}"))?;
        let credential = credential.trim_end_matches('\n');
        let stream = UnixStream::connect(socket)
            .map_err(|error| format!("connecting to {}: {error}", socket.display()))?;
        let mut session = Self {
            reader: BufReader::new(stream.try_clone().map_err(|e| e.to_string())?),
            writer: stream,
            next_id: 1,
            generation: 1,
            grant: None,
        };
        // Never echoed: the credential goes only into this one frame.
        session.query(
            "core.authenticate",
            object(vec![("credential", string(credential))]),
        )?;
        let mut requested = vec![object(vec![
            ("name", string("core")),
            ("majors", Value::Array(vec![Value::Int(1)])),
            ("required", Value::Bool(true)),
            (
                "required_features",
                Value::Array(vec![string("core.events")]),
            ),
            (
                "optional_features",
                Value::Array(vec![string("core.grants")]),
            ),
        ])];
        for profile in profiles {
            // Optional, always: a provider that does not serve one of these
            // leaves it unselected rather than refusing the session, and the
            // verb that needed it gets `unsupported_required_feature` with
            // the feature named.
            let optional: Vec<Value> = match *profile {
                "context" => CONTEXT_FEATURES.iter().map(|f| string(f)).collect(),
                _ => Vec::new(),
            };
            requested.push(object(vec![
                ("name", string(profile)),
                ("majors", Value::Array(vec![Value::Int(1)])),
                ("required", Value::Bool(true)),
                ("required_features", Value::Array(vec![])),
                ("optional_features", Value::Array(optional)),
            ]));
        }
        let negotiated = session.query(
            "core.negotiate",
            object(vec![
                (
                    "caller",
                    object(vec![
                        ("name", string("cbr")),
                        ("version", string(env!("CARGO_PKG_VERSION"))),
                    ]),
                ),
                (
                    "receive_limits",
                    object(vec![("max_frame_bytes", Value::Int(1_048_576))]),
                ),
                ("profiles", Value::Array(requested)),
            ]),
        )?;
        if let Value::Int(current) = at(&negotiated, &["dedupe_window", "current"]) {
            session.generation = current;
        }
        session.grant = grant;
        Ok(session)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let frame = object(vec![
            ("jsonrpc", string("2.0")),
            ("id", Value::Int(id)),
            ("method", string(method)),
            ("params", params),
        ]);
        let mut bytes = cbr_encoding::to_canonical(&frame);
        bytes.push(b'\n');
        self.writer.write_all(&bytes).map_err(|e| e.to_string())?;
        loop {
            let mut line = String::new();
            if self
                .reader
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                return Err("the provider closed the connection".into());
            }
            let response = cbr_encoding::parse(line.trim_end().as_bytes())
                .map_err(|error| format!("unparseable response: {error}"))?;
            // Notifications carry no id and are not this call's answer.
            if response.get("id") != Some(&Value::Int(id)) {
                continue;
            }
            if let Some(result) = response.get("result") {
                return Ok(result.clone());
            }
            let data = at(&response, &["error", "data"]);
            let code = data.get("code").and_then(Value::as_str).unwrap_or("error");
            let details = data.get("details").map(canonical).unwrap_or_default();
            return Err(format!("{method}: {code} {details}"));
        }
    }

    pub fn query(&mut self, operation: &str, payload: Value) -> Result<Value, String> {
        let id = self.next_id;
        let mut params = vec![
            ("operation", string(operation)),
            ("message_id", string(&format!("cbr-{id}"))),
        ];
        if let Some(grant) = &self.grant {
            params.push(("grant", string(grant)));
        }
        params.push(("payload", payload));
        self.call(operation, object(params))
    }

    /// A command, with the digest the provider will recompute. Command ids are
    /// deterministic in what the command names, so rerunning a command whose
    /// response was lost replays rather than applying twice.
    pub fn command(
        &mut self,
        operation: &str,
        command_id: &str,
        subject: Value,
        revision: i64,
        authority_epoch: Option<i64>,
        payload: Value,
    ) -> Result<Value, String> {
        let id = self.next_id;
        let mut envelope = vec![
            ("operation", string(operation)),
            ("message_id", string(&format!("cbr-{id}"))),
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
        if let Some(epoch) = authority_epoch {
            envelope.push(("authority_epoch", Value::Int(epoch)));
        }
        envelope.push(("payload", payload));
        let mut envelope = object(envelope);
        let digest = cbr_encoding::command_digest(&envelope).map_err(|e| format!("{e:?}"))?;
        if let Value::Object(members) = &mut envelope {
            members.push(("command_digest".into(), string(&digest)));
        }
        self.call(operation, envelope)
    }
}
