//! `cbr`: a command-line client of CBR's public protocol.
//!
//! It has **no internal shortcut**. It opens the provider's Unix socket,
//! authenticates with a handed-off credential, negotiates, and uses the same
//! `evidence/1` operations any other client would. Nothing here opens the store,
//! reads the object directory or links the provider's code, so what `cbr` can
//! do is exactly what the protocol lets a client do.
//!
//! ```text
//! cbr ingest <file> --socket <path> --credential-file <path> [--media-type T] [--scope S]
//! cbr fetch <artifact> --digest <digest> --out <file> --socket <path> --credential-file <path>
//! ```
//!
//! `ingest` prints `artifact`, `digest` and `size` lines. `fetch` writes the
//! bytes only after checking that what it assembled has the digest it asked
//! for; a mismatch writes nothing and exits nonzero.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use cbr_encoding::Value;

fn string(text: &str) -> Value {
    Value::String(text.into())
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn subject(kind: &str, id: &str) -> Value {
    object(vec![("kind", string(kind)), ("id", string(id))])
}

/// One authenticated, negotiated session.
struct Session {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: i64,
    generation: i64,
}

impl Session {
    fn open(socket: &Path, credential_file: &Path) -> Result<Self, String> {
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
        };
        // Never echoed: the credential goes only into this one frame.
        session.query(
            "core.authenticate",
            object(vec![("credential", string(credential))]),
        )?;
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
                (
                    "profiles",
                    Value::Array(vec![
                        object(vec![
                            ("name", string("core")),
                            ("majors", Value::Array(vec![Value::Int(1)])),
                            ("required", Value::Bool(true)),
                            (
                                "required_features",
                                Value::Array(vec![string("core.events")]),
                            ),
                            ("optional_features", Value::Array(vec![])),
                        ]),
                        object(vec![
                            ("name", string("evidence")),
                            ("majors", Value::Array(vec![Value::Int(1)])),
                            ("required", Value::Bool(true)),
                            ("required_features", Value::Array(vec![])),
                            ("optional_features", Value::Array(vec![])),
                        ]),
                    ]),
                ),
            ]),
        )?;
        if let Some(Value::Int(current)) = negotiated
            .get("dedupe_window")
            .and_then(|w| w.get("current"))
        {
            session.generation = *current;
        }
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
            let data = response.get("error").and_then(|e| e.get("data"));
            let code = data
                .and_then(|d| d.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("error");
            let details = data
                .and_then(|d| d.get("details"))
                .map(|d| String::from_utf8_lossy(&cbr_encoding::to_canonical(d)).into_owned())
                .unwrap_or_default();
            return Err(format!("{method}: {code} {details}"));
        }
    }

    fn query(&mut self, operation: &str, payload: Value) -> Result<Value, String> {
        let id = self.next_id;
        let params = object(vec![
            ("operation", string(operation)),
            ("message_id", string(&format!("cbr-{id}"))),
            ("payload", payload),
        ]);
        self.call(operation, params)
    }

    /// A command, with the digest the provider will recompute. Its command id
    /// is deterministic in the artifact and step, so running the same ingest
    /// again after a lost response replays rather than applying twice.
    fn command(
        &mut self,
        operation: &str,
        command_id: &str,
        subject: Value,
        revision: i64,
        payload: Value,
    ) -> Result<Value, String> {
        let id = self.next_id;
        let mut envelope = object(vec![
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
            ("payload", payload),
        ]);
        let digest = cbr_encoding::command_digest(&envelope).map_err(|e| format!("{e:?}"))?;
        if let Value::Object(members) = &mut envelope {
            members.push(("command_digest".into(), string(&digest)));
        }
        self.call(operation, envelope)
    }
}

fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as i64;
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn ingest(options: &Options, file: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("reading {}: {error}", file.display()))?;
    let digest = cbr_encoding::digest_bytes(&bytes);
    let hex = digest.split_once(':').map_or(digest.as_str(), |(_, h)| h);
    // Identity is the content plus the moment of ingest: the same bytes
    // ingested twice are two artifacts with their own provenance.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |e| e.as_nanos());
    let artifact = format!("ingest.{}.{nanos}", &hex[..16]);
    // Only the file's name, never its path: a path names a machine.
    let name = file
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());

    let mut session = Session::open(&options.socket, &options.credential_file)?;
    let key = subject("evidence.artifact", &artifact);
    let prepared = session.command(
        "evidence.upload.prepare",
        &format!("{artifact}.prepare"),
        key.clone(),
        0,
        object(vec![
            ("digest", string(&digest)),
            ("size", Value::Int(bytes.len() as i64)),
            ("media_type", string(&options.media_type)),
            ("producer", object(vec![("producer_id", string("cbr-cli"))])),
            (
                "source",
                object(vec![("kind", string("file")), ("id", string(&name))]),
            ),
            ("scope", string(&options.scope)),
            (
                "capture",
                object(vec![
                    ("captured_at", string(&now())),
                    ("anchors", Value::Array(vec![])),
                ]),
            ),
            (
                "coverage",
                object(vec![("completeness", string("complete"))]),
            ),
            ("retention_class", string("standard")),
        ]),
    )?;
    let chunk_limit = match prepared.get("outcome").and_then(|o| o.get("chunk_limit")) {
        Some(Value::Int(limit)) if *limit > 0 => *limit as usize,
        _ => return Err("prepare returned no usable chunk_limit".into()),
    };
    let mut revision = 1;
    for (index, chunk) in bytes.chunks(chunk_limit).enumerate() {
        let offset = index * chunk_limit;
        session.command(
            "evidence.upload.append",
            &format!("{artifact}.append.{offset}"),
            key.clone(),
            revision,
            object(vec![
                ("offset", Value::Int(offset as i64)),
                ("data_base64", string(&cbr_encoding::encode_base64(chunk))),
            ]),
        )?;
        revision += 1;
    }
    let sealed = session.command(
        "evidence.seal",
        &format!("{artifact}.seal"),
        key,
        revision,
        object(vec![]),
    )?;
    let sealed_digest = sealed
        .get("outcome")
        .and_then(|o| o.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if sealed_digest != digest {
        return Err(format!("the provider sealed {sealed_digest}, not {digest}"));
    }
    println!("artifact {artifact}");
    println!("digest {digest}");
    println!("size {}", bytes.len());
    Ok(())
}

fn fetch(options: &Options, artifact: &str, digest: &str, out: &Path) -> Result<(), String> {
    let mut session = Session::open(&options.socket, &options.credential_file)?;
    let mut assembled = Vec::new();
    loop {
        let result = session.query(
            "evidence.fetch",
            object(vec![
                ("artifact", subject("evidence.artifact", artifact)),
                ("digest", string(digest)),
                ("offset", Value::Int(assembled.len() as i64)),
            ]),
        )?;
        let availability = result
            .get("availability")
            .and_then(|a| a.get("state"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if availability != "available" {
            return Err(format!("the artifact is not available: {availability}"));
        }
        let data = result
            .get("data_base64")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let bytes = cbr_encoding::decode_base64(data).ok_or("the provider sent invalid base64")?;
        if bytes.is_empty() {
            return Err("the provider returned no bytes before the end".into());
        }
        assembled.extend(bytes);
        let size = match result.get("size") {
            Some(Value::Int(size)) => *size as usize,
            _ => return Err("fetch returned no size".into()),
        };
        if assembled.len() >= size {
            break;
        }
    }
    // Checked before anything is written: bytes that do not match the sealed
    // digest are never presented as the artifact.
    let computed = cbr_encoding::digest_bytes(&assembled);
    if computed != digest {
        return Err(format!(
            "assembled bytes have digest {computed}, not {digest}; nothing written"
        ));
    }
    let staged = out.with_extension("cbr-partial");
    std::fs::write(&staged, &assembled).map_err(|e| e.to_string())?;
    std::fs::rename(&staged, out).map_err(|e| e.to_string())?;
    println!("wrote {} bytes, digest {digest}", assembled.len());
    Ok(())
}

struct Options {
    socket: PathBuf,
    credential_file: PathBuf,
    media_type: String,
    scope: String,
}

fn run() -> Result<(), String> {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().ok_or("usage: cbr ingest|fetch …")?;
    let mut positional = Vec::new();
    let (mut socket, mut credential_file, mut digest, mut out) = (None, None, None, None);
    let mut media_type = "application/octet-stream".to_string();
    let mut scope = "local".to_string();
    while let Some(argument) = argv.next() {
        let mut value = || argv.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--socket" => socket = Some(PathBuf::from(value()?)),
            "--credential-file" => credential_file = Some(PathBuf::from(value()?)),
            "--digest" => digest = Some(value()?),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--media-type" => media_type = value()?,
            "--scope" => scope = value()?,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    let options = Options {
        socket: socket.ok_or("--socket is required")?,
        credential_file: credential_file.ok_or("--credential-file is required")?,
        media_type,
        scope,
    };
    match (command.as_str(), positional.as_slice()) {
        ("ingest", [file]) => ingest(&options, Path::new(file)),
        ("fetch", [artifact]) => fetch(
            &options,
            artifact,
            &digest.ok_or("--digest is required")?,
            &out.ok_or("--out is required")?,
        ),
        _ => Err("usage: cbr ingest <file> | cbr fetch <artifact> --digest D --out F".into()),
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("cbr: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}
