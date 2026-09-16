//! Envelope validation: CORE section 5, and the parts of section 9 that apply
//! to a message rather than to a subject.
//!
//! Objects are closed (section 5.1): a member the negotiated profile and
//! features do not define makes the message invalid. An unknown field cannot
//! be classified as safe to ignore, so it is treated as required semantics and
//! refused.

use cbr_encoding::Value;

use crate::config::Limits;
use crate::errors::ProtocolError;

const COMMAND_MEMBERS: [&str; 14] = [
    "operation",
    "message_id",
    "command_id",
    "dedupe_generation",
    "subject",
    "preconditions",
    "authority_epoch",
    "requires",
    "correlation",
    "caused_by",
    "extensions",
    "grant",
    "command_digest",
    "payload",
];

const COMMAND_REQUIRED: [&str; 9] = [
    "operation",
    "message_id",
    "command_id",
    "dedupe_generation",
    "subject",
    "preconditions",
    "requires",
    "command_digest",
    "payload",
];

const QUERY_MEMBERS: [&str; 6] = [
    "operation",
    "message_id",
    "requires",
    "extensions",
    "grant",
    "payload",
];

const QUERY_REQUIRED: [&str; 3] = ["operation", "message_id", "payload"];

/// An identifier: printable ASCII, first character alphanumeric, 1..=128 bytes.
/// The restricted set avoids Unicode normalisation ambiguity in an identity.
pub fn is_identifier(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'~' | b'-'))
}

/// A dotted name: `<head>.<tail>`, lowercase, never containing a slash.
pub fn is_dotted_name(text: &str) -> bool {
    if text.len() > 128 || !text.contains('.') {
        return false;
    }
    let mut parts = text.split('.');
    let head = parts.next().unwrap_or_default();
    let head_ok = !head.is_empty()
        && head.as_bytes()[0].is_ascii_lowercase()
        && head
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !head_ok {
        return false;
    }
    let mut any_tail = false;
    for part in parts {
        any_tail = true;
        let ok = !part.is_empty()
            && part.as_bytes()[0].is_ascii_lowercase()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !ok {
            return false;
        }
    }
    any_tail
}

/// Measure the limits of CORE section 9 over the whole `params` object.
///
/// `params` is depth 1; each nested object or array adds one. Every string,
/// member names included, counts toward the string limit, and every array
/// counts toward the array limit. A value exactly at a limit is within it.
pub fn check_limits(params: &Value, limits: &Limits) -> Result<(), ProtocolError> {
    fn walk(value: &Value, depth: i64, limits: &Limits, worst: &mut Option<ProtocolError>) {
        if worst.is_some() {
            return;
        }
        if depth > limits.max_depth {
            *worst = Some(ProtocolError::limit_exceeded("max_depth", limits.max_depth));
            return;
        }
        match value {
            Value::String(text) => {
                if text.len() as i64 > limits.max_string_bytes {
                    *worst = Some(ProtocolError::limit_exceeded(
                        "max_string_bytes",
                        limits.max_string_bytes,
                    ));
                }
            }
            Value::Array(items) => {
                if items.len() as i64 > limits.max_array_items {
                    *worst = Some(ProtocolError::limit_exceeded(
                        "max_array_items",
                        limits.max_array_items,
                    ));
                    return;
                }
                for item in items {
                    walk(item, depth + 1, limits, worst);
                }
            }
            Value::Object(members) => {
                for (name, member) in members {
                    if name.len() as i64 > limits.max_string_bytes {
                        *worst = Some(ProtocolError::limit_exceeded(
                            "max_string_bytes",
                            limits.max_string_bytes,
                        ));
                        return;
                    }
                    walk(member, depth + 1, limits, worst);
                }
            }
            _ => {}
        }
    }

    // Order matters: depth, array items, string bytes, then payload bytes.
    let mut worst = None;
    walk(params, 1, limits, &mut worst);
    if let Some(error) = worst {
        return Err(error);
    }
    if let Some(payload) = params.get("payload") {
        let encoded = cbr_encoding::to_canonical(payload).len() as i64;
        if encoded > limits.max_payload_bytes {
            return Err(ProtocolError::limit_exceeded(
                "max_payload_bytes",
                limits.max_payload_bytes,
            ));
        }
    }
    Ok(())
}

fn check_members(
    envelope: &Value,
    allowed: &[&str],
    required: &[&str],
) -> Result<(), ProtocolError> {
    let members = match envelope {
        Value::Object(members) => members,
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "",
                "envelope is not an object",
            ));
        }
    };
    for (name, _) in members {
        if !allowed.contains(&name.as_str()) {
            return Err(ProtocolError::invalid_envelope(
                &format!("/{name}"),
                "unknown field",
            ));
        }
    }
    for name in required {
        if envelope.get(name).is_none() {
            return Err(ProtocolError::invalid_envelope(
                &format!("/{name}"),
                "required field absent",
            ));
        }
    }
    Ok(())
}

fn check_identifier(envelope: &Value, name: &str) -> Result<(), ProtocolError> {
    match envelope.get(name) {
        Some(Value::String(text)) if is_identifier(text) => Ok(()),
        Some(_) => Err(ProtocolError::invalid_envelope(
            &format!("/{name}"),
            "not an identifier",
        )),
        None => Ok(()),
    }
}

fn check_subject(value: &Value, path: &str) -> Result<(), ProtocolError> {
    let Value::Object(members) = value else {
        return Err(ProtocolError::invalid_envelope(
            path,
            "subject is not an object",
        ));
    };
    for (name, _) in members {
        if name != "kind" && name != "id" {
            return Err(ProtocolError::invalid_envelope(
                &format!("{path}/{name}"),
                "unknown field",
            ));
        }
    }
    match value.get("kind") {
        Some(Value::String(kind)) if is_dotted_name(kind) => {}
        _ => {
            return Err(ProtocolError::invalid_envelope(
                &format!("{path}/kind"),
                "not a subject kind",
            ));
        }
    }
    match value.get("id") {
        Some(Value::String(id)) if is_identifier(id) => Ok(()),
        _ => Err(ProtocolError::invalid_envelope(
            &format!("{path}/id"),
            "not an identifier",
        )),
    }
}

/// Validate the `requires` array: unique entries, and an entry containing a
/// slash must also be present in `extensions` (CORE section 5.1).
fn check_requires(envelope: &Value) -> Result<Vec<String>, ProtocolError> {
    let entries = match envelope.get("requires") {
        None => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err(ProtocolError::invalid_envelope("/requires", "not an array"));
        }
    };
    let mut seen: Vec<String> = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let Some(name) = entry.as_str() else {
            return Err(ProtocolError::invalid_envelope(
                &format!("/requires/{index}"),
                "not a string",
            ));
        };
        if seen.iter().any(|existing| existing == name) {
            return Err(ProtocolError::invalid_envelope(
                "/requires",
                "duplicate entry",
            ));
        }
        if name.contains('/')
            && envelope
                .get("extensions")
                .and_then(|e| e.get(name))
                .is_none()
        {
            return Err(ProtocolError::invalid_envelope(
                "/requires",
                "required extension absent from extensions",
            ));
        }
        seen.push(name.to_string());
    }
    Ok(seen)
}

pub struct Command {
    pub operation: String,
    pub command_id: String,
    pub dedupe_generation: i64,
    pub subject: Value,
    pub preconditions: Vec<(Value, i64)>,
    pub authority_epoch: Option<i64>,
    pub requires: Vec<String>,
    pub command_digest: String,
    pub payload: Value,
}

/// Validate a command envelope's shape and semantics (CORE section 10 step 2).
pub fn parse_command(envelope: &Value) -> Result<Command, ProtocolError> {
    check_members(envelope, &COMMAND_MEMBERS, &COMMAND_REQUIRED)?;
    check_identifier(envelope, "message_id")?;
    check_identifier(envelope, "command_id")?;
    check_identifier(envelope, "grant")?;

    let operation = match envelope.get("operation") {
        Some(Value::String(name)) if is_dotted_name(name) => name.clone(),
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/operation",
                "not an operation name",
            ));
        }
    };
    let dedupe_generation = match envelope.get("dedupe_generation") {
        Some(Value::Int(number)) if *number >= 0 => *number,
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/dedupe_generation",
                "not a non-negative integer",
            ));
        }
    };
    let subject = envelope
        .get("subject")
        .expect("required member present")
        .clone();
    check_subject(&subject, "/subject")?;

    let mut preconditions = Vec::new();
    match envelope.get("preconditions") {
        Some(Value::Array(items)) => {
            if items.len() > 16 {
                return Err(ProtocolError::invalid_envelope(
                    "/preconditions",
                    "too many entries",
                ));
            }
            for (index, item) in items.iter().enumerate() {
                let path = format!("/preconditions/{index}");
                let Value::Object(members) = item else {
                    return Err(ProtocolError::invalid_envelope(&path, "not an object"));
                };
                for (name, _) in members {
                    if name != "subject" && name != "revision" {
                        return Err(ProtocolError::invalid_envelope(
                            &format!("{path}/{name}"),
                            "unknown field",
                        ));
                    }
                }
                let Some(entry_subject) = item.get("subject") else {
                    return Err(ProtocolError::invalid_envelope(
                        &format!("{path}/subject"),
                        "required field absent",
                    ));
                };
                check_subject(entry_subject, &format!("{path}/subject"))?;
                let revision = match item.get("revision") {
                    Some(Value::Int(number)) if *number >= 0 => *number,
                    _ => {
                        return Err(ProtocolError::invalid_envelope(
                            &format!("{path}/revision"),
                            "not a non-negative integer",
                        ));
                    }
                };
                // Two entries naming the same subject are invalid.
                if preconditions
                    .iter()
                    .any(|(existing, _): &(Value, i64)| existing == entry_subject)
                {
                    return Err(ProtocolError::invalid_envelope(
                        "/preconditions",
                        "duplicate precondition subject",
                    ));
                }
                preconditions.push((entry_subject.clone(), revision));
            }
        }
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "not an array",
            ));
        }
    }

    let authority_epoch = match envelope.get("authority_epoch") {
        None => None,
        Some(Value::Int(number)) if *number >= 0 => Some(*number),
        Some(_) => {
            return Err(ProtocolError::invalid_envelope(
                "/authority_epoch",
                "not a non-negative integer",
            ));
        }
    };

    let requires = check_requires(envelope)?;

    let command_digest = match envelope.get("command_digest") {
        Some(Value::String(text)) => text.clone(),
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/command_digest",
                "not a digest",
            ));
        }
    };

    let payload = envelope
        .get("payload")
        .expect("required member present")
        .clone();
    if !payload.is_object() {
        return Err(ProtocolError::invalid_envelope("/payload", "not an object"));
    }

    Ok(Command {
        operation,
        command_id: envelope
            .get("command_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        dedupe_generation,
        subject,
        preconditions,
        authority_epoch,
        requires,
        command_digest,
        payload,
    })
}

pub struct Query {
    pub operation: String,
    pub requires: Vec<String>,
    pub payload: Value,
}

/// Validate a query envelope. A query has no command identity, preconditions
/// or digest, so it is checked for shape and `requires` only.
pub fn parse_query(envelope: &Value) -> Result<Query, ProtocolError> {
    check_members(envelope, &QUERY_MEMBERS, &QUERY_REQUIRED)?;
    check_identifier(envelope, "message_id")?;
    check_identifier(envelope, "grant")?;
    let operation = match envelope.get("operation") {
        Some(Value::String(name)) if is_dotted_name(name) => name.clone(),
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/operation",
                "not an operation name",
            ));
        }
    };
    let requires = check_requires(envelope)?;
    let payload = envelope
        .get("payload")
        .expect("required member present")
        .clone();
    if !payload.is_object() {
        return Err(ProtocolError::invalid_envelope("/payload", "not an object"));
    }
    Ok(Query {
        operation,
        requires,
        payload,
    })
}
