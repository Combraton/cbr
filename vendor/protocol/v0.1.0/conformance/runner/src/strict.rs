//! The runner's strict value domain (ENCODING section 1) and canonical digests.
//!
//! serde_json alone silently keeps duplicate members and accepts numbers outside
//! the protocol domain, so this module wraps it with an explicit visitor and a
//! token pre-scan. The reference provider uses an unrelated hand-written parser.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err("byte-order mark".into());
    }
    std::str::from_utf8(bytes).map_err(|e| format!("invalid UTF-8: {e}"))?;
    reject_negative_zero(bytes)?;
    let strict: Strict =
        serde_json::from_slice(bytes).map_err(|e| format!("not in the JSON value domain: {e}"))?;
    Ok(strict.0)
}

/// `-0` is valid JSON but outside the domain; serde_json does not report it distinctly.
fn reject_negative_zero(bytes: &[u8]) -> Result<(), String> {
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'-' if bytes.get(index + 1) == Some(&b'0')
                && !matches!(bytes.get(index + 2), Some(b'0'..=b'9' | b'.' | b'e' | b'E')) =>
            {
                return Err("negative zero".into());
            }
            _ => {}
        }
    }
    Ok(())
}

struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

fn check_string<E: de::Error>(text: &str) -> Result<(), E> {
    for ch in text.chars() {
        let code = ch as u32;
        if (0xFDD0..=0xFDEF).contains(&code) || (code & 0xFFFE) == 0xFFFE {
            return Err(E::custom(format!("noncharacter U+{code:04X}")));
        }
    }
    Ok(())
}

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Strict;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value in the protocol domain")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
        Ok(Strict(Value::Null))
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Strict, E> {
        Ok(Strict(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Strict, E> {
        if (-SAFE_INTEGER..=SAFE_INTEGER).contains(&value) {
            Ok(Strict(Value::from(value)))
        } else {
            Err(E::custom("integer outside the safe range"))
        }
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Strict, E> {
        if value <= SAFE_INTEGER as u64 {
            Ok(Strict(Value::from(value as i64)))
        } else {
            Err(E::custom("integer outside the safe range"))
        }
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Strict, E> {
        Err(E::custom("non-integer number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Strict, E> {
        check_string(value)?;
        Ok(Strict(Value::String(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Strict, E> {
        check_string(&value)?;
        Ok(Strict(Value::String(value)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
        let mut items = Vec::new();
        while let Some(Strict(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Strict(Value::Array(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Strict, A::Error> {
        let mut map = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            check_string(&key)?;
            let Strict(value) = access.next_value()?;
            if map.insert(key.clone(), value).is_some() {
                return Err(de::Error::custom(format!("duplicate member name {key:?}")));
            }
        }
        Ok(Strict(Value::Object(map)))
    }
}

/// RFC 8785 canonical bytes, using the serde_json_canonicalizer crate.
pub fn canonical(value: &Value) -> Vec<u8> {
    serde_json_canonicalizer::to_vec(value).expect("domain values are canonicalizable")
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Command intent digest (CORE section 6.1).
pub fn intent_digest(envelope: &Value) -> String {
    let requires: Vec<&str> = envelope["requires"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut intent = Map::new();
    for member in [
        "operation",
        "subject",
        "preconditions",
        "requires",
        "payload",
    ] {
        if let Some(value) = envelope.get(member) {
            intent.insert(member.into(), value.clone());
        }
    }
    let extensions: Map<String, Value> = envelope
        .get("extensions")
        .and_then(Value::as_object)
        .map(|all| {
            all.iter()
                .filter(|(key, _)| requires.contains(&key.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    intent.insert("extensions".into(), Value::Object(extensions));
    sha256(&canonical(&Value::Object(intent)))
}

/// Check the runner against the published encoding vectors.
pub fn self_test(vectors: &Value) -> Result<usize, String> {
    let mut checked = 0;
    for case in vectors["canonical"]
        .as_array()
        .ok_or("vectors: canonical missing")?
    {
        let id = &case["id"];
        let input = hex::decode(case["input_hex"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let value = parse(&input).map_err(|e| format!("{id}: rejected valid input: {e}"))?;
        let canon = canonical(&value);
        if hex::encode(&canon) != case["canonical_hex"].as_str().unwrap_or_default() {
            return Err(format!("{id}: canonical bytes differ"));
        }
        if sha256(&canon) != case["digest"].as_str().unwrap_or_default() {
            return Err(format!("{id}: digest differs"));
        }
        checked += 1;
    }
    for case in vectors["rejected"]
        .as_array()
        .ok_or("vectors: rejected missing")?
    {
        let input = hex::decode(case["input_hex"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        if parse(&input).is_ok() {
            return Err(format!(
                "{}: accepted an input outside the domain",
                case["id"]
            ));
        }
        checked += 1;
    }
    for case in vectors["intent"]
        .as_array()
        .ok_or("vectors: intent missing")?
    {
        if intent_digest(&case["envelope"]) != case["command_digest"].as_str().unwrap_or_default() {
            return Err(format!("{}: intent digest differs", case["id"]));
        }
        checked += 1;
    }
    Ok(checked)
}
