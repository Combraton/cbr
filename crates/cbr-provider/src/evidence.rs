//! The evidence profile's records and pure rules (`evidence/1`).
//!
//! An artifact is a subject of kind `evidence.artifact` whose record is the
//! subject value, like a grant or an effect: revisions, the command transaction
//! and durability are the store's. Staged bytes live in the store beside the
//! record, committed in the same transaction as the append that brought them.
//! Sealed bytes live in the content-addressed object store, **published and
//! verified from disk before the row that names them is committed**, never the
//! reverse, so a crash can leave an object no row names but never a row naming
//! an object that is not there.
//!
//! Everything here is a pure function of its inputs; the provider owns the
//! command path, authorization and the store.

use cbr_encoding::Value;

use crate::errors::ProtocolError;
use crate::store::SubjectKey;

pub const ARTIFACT: &str = "evidence.artifact";
pub const HOLD: &str = "evidence.hold";
pub const MANIFEST_MEDIA_TYPE: &str = "application/vnd.combraton.evidence-manifest+json";
pub const MANIFEST_FORMAT: &str = "combraton-evidence-manifest/1";

/// The envelope overhead CBR declares for an append beyond its base64 data
/// (EVIDENCE section 4): the operation, ids, digest, subject and precondition
/// of an append envelope comfortably fit in it.
pub const CHUNK_OVERHEAD_BYTES: i64 = 2048;

/// Dependency kinds the proof-loss record tracks (EVIDENCE section 9).
pub const TRACKED: [&str; 2] = ["evidence.hold", "evidence.manifest_child"];

pub fn artifact_key(id: &str) -> SubjectKey {
    SubjectKey {
        kind: ARTIFACT.into(),
        id: id.into(),
    }
}

pub fn hold_key(id: &str) -> SubjectKey {
    SubjectKey {
        kind: HOLD.into(),
        id: id.into(),
    }
}

/// The largest decoded chunk one append may carry: the largest multiple of 3
/// whose base64 form fits the payload limit less the declared overhead, the
/// frame limit less the same overhead, and the string limit.
pub fn chunk_limit(max_payload: i64, max_frame: i64, max_string: i64) -> i64 {
    let encoded = (max_payload - CHUNK_OVERHEAD_BYTES)
        .min(max_frame - CHUNK_OVERHEAD_BYTES)
        .min(max_string)
        .max(0);
    // Base64 encodes 3 bytes as 4 characters.
    (encoded / 4) * 3
}

// ---- base64 --------------------------------------------------------------

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Decode padded standard base64, strictly: length a multiple of 4, padding
/// only at the end, and no stray bits in the last character. `None` for
/// anything else, which the caller reports as `invalid_envelope`.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| -> Option<u32> { ALPHABET.iter().position(|a| *a == c).map(|p| p as u32) };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, quad) in bytes.chunks(4).enumerate() {
        let last = index == bytes.len() / 4 - 1;
        let padding = quad.iter().rev().take_while(|c| **c == b'=').count();
        if padding > 2 || (padding > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in quad.iter().enumerate() {
            let digit = if i >= 4 - padding { 0 } else { value(*c)? };
            n = (n << 6) | digit;
        }
        // Canonical: bits the padding discards must be zero.
        if (padding == 1 && n & 0xff != 0) || (padding == 2 && n & 0xffff != 0) {
            return None;
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

// ---- descriptor ----------------------------------------------------------

fn invalid(path: &str, reason: &str) -> ProtocolError {
    ProtocolError::invalid_envelope(path, reason)
}

fn string_at<'a>(value: &'a Value, name: &str, path: &str) -> Result<&'a str, ProtocolError> {
    match value.get(name) {
        Some(Value::String(text)) if !text.is_empty() => Ok(text),
        _ => Err(invalid(path, "required non-empty string")),
    }
}

fn check_instant(value: Option<&Value>, path: &str) -> Result<(), ProtocolError> {
    match value {
        Some(Value::String(text)) if crate::grants::is_instant(text) => Ok(()),
        _ => Err(invalid(path, "not a UTC instant")),
    }
}

fn check_members(value: &Value, allowed: &[&str], path: &str) -> Result<(), ProtocolError> {
    let Value::Object(members) = value else {
        return Err(invalid(path, "not an object"));
    };
    for (name, _) in members {
        if !allowed.contains(&name.as_str()) {
            return Err(invalid(&format!("{path}/{name}"), "unknown field"));
        }
    }
    Ok(())
}

/// Whether a locator carries a credential: user information in its authority,
/// or a query parameter whose name says it signs or authenticates the request.
/// Checked because a locator is stored and shown to every reader, and a
/// credential there would outlive whatever issued it (EVIDENCE section 3).
pub fn locator_carries_credentials(locator: &str) -> bool {
    let after_scheme = locator.split_once("://").map_or(locator, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.contains('@') {
        return true;
    }
    let Some((_, query)) = locator.split_once('?') else {
        return false;
    };
    let query = query.split('#').next().unwrap_or_default();
    const SIGNALS: [&str; 10] = [
        "signature",
        "token",
        "credential",
        "secret",
        "password",
        "passwd",
        "sig",
        "key",
        "auth",
        "session",
    ];
    query.split('&').any(|pair| {
        let name = pair
            .split('=')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        SIGNALS.iter().any(|signal| name.contains(signal))
    })
}

/// Validate a prepare payload and return the descriptor as stored, with the
/// producer principal set to the session principal (EVD-4). Step 2: nothing
/// here consults the store or authorization.
pub fn parse_descriptor(payload: &Value, principal: &str) -> Result<Value, ProtocolError> {
    check_members(
        payload,
        &[
            "digest",
            "size",
            "media_type",
            "producer",
            "source",
            "scope",
            "capture",
            "coverage",
            "work",
            "retention_class",
            "locator",
        ],
        "/payload",
    )?;
    for required in [
        "digest",
        "size",
        "media_type",
        "producer",
        "source",
        "scope",
        "capture",
        "coverage",
        "retention_class",
    ] {
        if payload.get(required).is_none() {
            return Err(invalid(
                &format!("/payload/{required}"),
                "required field absent",
            ));
        }
    }

    let digest = string_at(payload, "digest", "/payload/digest")?;
    match cbr_encoding::parse_digest(digest) {
        Ok((cbr_encoding::Algorithm::Sha256, _)) => {}
        Ok((cbr_encoding::Algorithm::Sha512, _)) => {
            return Err(ProtocolError::unsupported_digest_algorithm(
                "sha512",
                vec!["sha256"],
            ));
        }
        Err(cbr_encoding::DigestError::UnsupportedAlgorithm(algorithm)) => {
            return Err(ProtocolError::unsupported_digest_algorithm(
                &algorithm,
                vec!["sha256"],
            ));
        }
        Err(_) => return Err(invalid("/payload/digest", "malformed digest")),
    }
    match payload.get("size") {
        Some(Value::Int(size)) if *size >= 0 => {}
        _ => return Err(invalid("/payload/size", "not a non-negative integer")),
    }
    string_at(payload, "media_type", "/payload/media_type")?;
    string_at(payload, "scope", "/payload/scope")?;
    string_at(payload, "retention_class", "/payload/retention_class")?;

    let producer = payload.get("producer").expect("checked");
    check_members(producer, &["principal", "producer_id"], "/payload/producer")?;
    if let Some(named) = producer.get("principal")
        && named.as_str() != Some(principal)
    {
        return Err(invalid(
            "/payload/producer/principal",
            "the producer is always the session principal",
        ));
    }

    let source = payload.get("source").expect("checked");
    check_members(source, &["kind", "id"], "/payload/source")?;
    let source_kind = string_at(source, "kind", "/payload/source/kind")?;
    string_at(source, "id", "/payload/source/id")?;

    let capture = payload.get("capture").expect("checked");
    check_members(
        capture,
        &["captured_at", "uncertainty", "anchors"],
        "/payload/capture",
    )?;
    check_instant(capture.get("captured_at"), "/payload/capture/captured_at")?;
    if let Some(uncertainty) = capture.get("uncertainty") {
        check_members(
            uncertainty,
            &["not_before", "not_after"],
            "/payload/capture/uncertainty",
        )?;
        check_instant(
            uncertainty.get("not_before"),
            "/payload/capture/uncertainty/not_before",
        )?;
        check_instant(
            uncertainty.get("not_after"),
            "/payload/capture/uncertainty/not_after",
        )?;
        let before = uncertainty.get("not_before").and_then(Value::as_str);
        let after = uncertainty.get("not_after").and_then(Value::as_str);
        if before > after {
            return Err(invalid(
                "/payload/capture/uncertainty",
                "not_before is after not_after",
            ));
        }
    }

    let coverage = payload.get("coverage").expect("checked");
    check_members(
        coverage,
        &["completeness", "covered", "gaps"],
        "/payload/coverage",
    )?;
    let completeness = coverage.get("completeness").and_then(Value::as_str);
    if !matches!(completeness, Some("complete" | "partial" | "unknown")) {
        return Err(invalid(
            "/payload/coverage/completeness",
            "not complete, partial or unknown",
        ));
    }
    // EVD-7: terminal output is never a complete trace of the tools behind it.
    if source_kind == "terminal_output" && completeness == Some("complete") {
        return Err(invalid(
            "/payload/coverage/completeness",
            "terminal output cannot declare complete coverage",
        ));
    }

    if let Some(work) = payload.get("work") {
        check_members(work, &["kind", "id"], "/payload/work")?;
        string_at(work, "kind", "/payload/work/kind")?;
        string_at(work, "id", "/payload/work/id")?;
    }
    if let Some(locator) = payload.get("locator") {
        let Some(text) = locator.as_str().filter(|t| !t.is_empty()) else {
            return Err(invalid("/payload/locator", "not a string"));
        };
        if locator_carries_credentials(text) {
            return Err(invalid(
                "/payload/locator",
                "a locator must not carry credentials",
            ));
        }
    }

    // The stored descriptor: the payload with the producer principal filled.
    let mut descriptor = payload.clone();
    if let Value::Object(members) = &mut descriptor
        && let Some((_, Value::Object(producer))) =
            members.iter_mut().find(|(name, _)| name == "producer")
        && !producer.iter().any(|(name, _)| name == "principal")
    {
        producer.insert(0, ("principal".into(), Value::String(principal.into())));
    }
    Ok(descriptor)
}

// ---- manifests -----------------------------------------------------------

/// One child a manifest names: its role, the exact artifact **and** digest,
/// whether it is required, and the provider if it is elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Child {
    pub role: String,
    pub provider: Option<String>,
    pub artifact: String,
    pub digest: String,
    pub required: bool,
}

impl Child {
    pub fn reference(&self) -> Value {
        let mut members = Vec::new();
        if let Some(provider) = &self.provider {
            members.push(("provider".into(), Value::String(provider.clone())));
        }
        members.push((
            "artifact".into(),
            Value::Object(vec![
                ("kind".into(), Value::String(ARTIFACT.into())),
                ("id".into(), Value::String(self.artifact.clone())),
            ]),
        ));
        members.push(("digest".into(), Value::String(self.digest.clone())));
        Value::Object(members)
    }

    pub fn to_value(&self) -> Value {
        Value::Object(vec![
            ("role".into(), Value::String(self.role.clone())),
            ("evidence".into(), self.reference()),
            ("required".into(), Value::Bool(self.required)),
        ])
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        let evidence = value.get("evidence")?;
        Some(Self {
            role: value.get("role")?.as_str()?.into(),
            provider: evidence
                .get("provider")
                .and_then(Value::as_str)
                .map(str::to_string),
            artifact: evidence.get("artifact")?.get("id")?.as_str()?.into(),
            digest: evidence.get("digest")?.as_str()?.into(),
            required: matches!(value.get("required"), Some(Value::Bool(true))),
        })
    }
}

/// Parse sealed manifest content, or `None` if it is not a valid manifest of a
/// supported format (refused at seal, EVIDENCE section 7).
pub fn parse_manifest(bytes: &[u8]) -> Option<Vec<Child>> {
    let value = cbr_encoding::parse(bytes).ok()?;
    let Value::Object(members) = &value else {
        return None;
    };
    if members
        .iter()
        .any(|(name, _)| !matches!(name.as_str(), "format" | "children"))
        || value.get("format").and_then(Value::as_str) != Some(MANIFEST_FORMAT)
    {
        return None;
    }
    let mut children = Vec::new();
    for child in value.get("children")?.as_array()? {
        let Value::Object(fields) = child else {
            return None;
        };
        if fields
            .iter()
            .any(|(name, _)| !matches!(name.as_str(), "role" | "evidence" | "required"))
        {
            return None;
        }
        if !matches!(child.get("required"), Some(Value::Bool(_))) {
            return None;
        }
        let evidence = child.get("evidence")?;
        if evidence.get("artifact")?.get("kind")?.as_str()? != ARTIFACT {
            return None;
        }
        let digest = evidence.get("digest")?.as_str()?;
        cbr_encoding::parse_digest(digest).ok()?;
        children.push(Child::from_value(child)?);
    }
    Some(children)
}

/// An availability object.
pub fn availability(state: &str, reason: Option<&str>) -> Value {
    let mut members = vec![("state".into(), Value::String(state.into()))];
    if let Some(reason) = reason {
        members.push(("reason".into(), Value::String(reason.into())));
    }
    Value::Object(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_and_refuses_what_is_not_canonical() {
        for bytes in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"exact bytes \x00\xff\xfe ",
        ] {
            let encoded = encode_base64(bytes);
            assert_eq!(decode_base64(&encoded).as_deref(), Some(bytes), "{encoded}");
        }
        assert_eq!(
            encode_base64(b"exact bytes \x00\xff\xfe "),
            "ZXhhY3QgYnl0ZXMgAP/+IA=="
        );
        for bad in ["Zg", "Zg=", "Z===", "Zg==Zg==", "Zh==", "Zm9v!", "Zm=v"] {
            assert!(decode_base64(bad).is_none(), "{bad} must be refused");
        }
    }

    #[test]
    fn a_chunk_limit_leaves_room_for_base64_and_the_declared_overhead() {
        // The fixture's own numbers: a 6144-byte payload limit less 2048 bytes
        // of overhead leaves 4096 base64 characters, which carry 3072 bytes.
        assert_eq!(chunk_limit(6144, 2_097_152, 262_144), 3072);
        assert_eq!(chunk_limit(6144, 2_097_152, 262_144) % 3, 0);
        // The string limit binds when it is the smallest.
        assert_eq!(chunk_limit(1_048_576, 2_097_152, 262_144), 196_608);
    }

    #[test]
    fn locators_with_credentials_are_recognised() {
        assert!(locator_carries_credentials(
            "https://user:secret@store.example/objects/l-1"
        ));
        assert!(locator_carries_credentials(
            "https://store.example/objects/l-1?X-Amz-Signature=abc123"
        ));
        assert!(locator_carries_credentials(
            "https://store.example/o?access_token=x"
        ));
        assert!(!locator_carries_credentials(
            "https://store.example/objects/l-1"
        ));
        assert!(!locator_carries_credentials(
            "https://store.example/objects/l-1?part=2"
        ));
        // An `@` in the path is not user information.
        assert!(!locator_carries_credentials(
            "https://store.example/objects/a@b"
        ));
    }
}
