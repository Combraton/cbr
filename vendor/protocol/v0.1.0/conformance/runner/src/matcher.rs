//! Fixture patterns (conformance/schemas/fixture.schema.json) and outgoing templates.

use std::collections::BTreeMap;

use serde_json::Value;
use sha2::Digest;

pub type Vars = BTreeMap<String, Value>;

/// Match `actual` against `pattern`. Returns a human-readable mismatch.
pub fn matches(
    pattern: &Value,
    actual: Option<&Value>,
    vars: &Vars,
    path: &str,
) -> Result<(), String> {
    if let Value::Object(map) = pattern
        && let Some(directive) = map.keys().find(|key| key.starts_with('$'))
    {
        return directive_match(directive, map, actual, vars, path);
    }
    let Some(actual) = actual else {
        return Err(format!("{path}: missing"));
    };
    match (pattern, actual) {
        (Value::Object(expected), Value::Object(found)) => {
            for (key, sub) in expected {
                matches(sub, found.get(key), vars, &format!("{path}/{key}"))?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(found)) => {
            if expected.len() != found.len() {
                return Err(format!(
                    "{path}: expected {} items, found {}",
                    expected.len(),
                    found.len()
                ));
            }
            for (index, (sub, item)) in expected.iter().zip(found).enumerate() {
                matches(sub, Some(item), vars, &format!("{path}/{index}"))?;
            }
            Ok(())
        }
        (expected, found) if expected == found => Ok(()),
        (expected, found) => Err(format!("{path}: expected {expected}, found {found}")),
    }
}

fn directive_match(
    directive: &str,
    map: &serde_json::Map<String, Value>,
    actual: Option<&Value>,
    vars: &Vars,
    path: &str,
) -> Result<(), String> {
    let argument = &map[directive];
    match directive {
        "$absent" => match actual {
            None => Ok(()),
            Some(found) => Err(format!("{path}: expected absent, found {found}")),
        },
        "$any" => actual.map(|_| ()).ok_or_else(|| format!("{path}: missing")),
        "$var" | "$ne_var" => {
            let name = argument.as_str().unwrap_or_default();
            let expected = vars
                .get(name)
                .ok_or_else(|| format!("{path}: unknown variable {name}"))?;
            let found = actual.ok_or_else(|| format!("{path}: missing"))?;
            match (directive, expected == found) {
                ("$var", true) | ("$ne_var", false) => Ok(()),
                ("$var", false) => Err(format!(
                    "{path}: expected ${name} = {expected}, found {found}"
                )),
                _ => Err(format!(
                    "{path}: expected a value different from ${name} = {expected}"
                )),
            }
        }
        "$exact" => {
            let found = actual.ok_or_else(|| format!("{path}: missing"))?;
            let expected = render(argument, vars, &mut 0)?;
            if &expected == found {
                Ok(())
            } else {
                Err(format!(
                    "{path}: expected exactly {expected}, found {found}"
                ))
            }
        }
        "$type" => {
            let found = actual.ok_or_else(|| format!("{path}: missing"))?;
            let kind = match found {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "integer",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            if argument.as_str() == Some(kind) {
                Ok(())
            } else {
                Err(format!("{path}: expected {argument}, found {kind}"))
            }
        }
        "$contains" => {
            let found = actual
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{path}: expected an array"))?;
            if found
                .iter()
                .any(|item| matches(argument, Some(item), vars, path).is_ok())
            {
                Ok(())
            } else {
                Err(format!("{path}: no element matches {argument}"))
            }
        }
        "$len" => {
            let found = actual
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{path}: expected an array"))?;
            if Some(found.len() as u64) == argument.as_u64() {
                Ok(())
            } else {
                Err(format!(
                    "{path}: expected length {argument}, found {}",
                    found.len()
                ))
            }
        }
        "$sha256_base64" => {
            // The decoded bytes' sha256 digest matches the argument pattern (for example a
            // captured digest): how a reader checks fetched bytes against a reference.
            let found = actual
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{path}: expected a base64 string"))?;
            let bytes = decode_base64(found).ok_or_else(|| format!("{path}: not base64"))?;
            let digest = format!("sha256:{}", hex::encode(sha2::Sha256::digest(&bytes)));
            matches(
                argument,
                Some(&Value::String(digest)),
                vars,
                &format!("{path}(sha256)"),
            )
        }
        "$base64_json" => {
            // The decoded bytes parse as JSON matching the argument pattern: how a reader checks
            // what a fetched document says, such as a packet's format.
            let found = actual
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{path}: expected a base64 string"))?;
            let bytes = decode_base64(found).ok_or_else(|| format!("{path}: not base64"))?;
            let document: Value = serde_json::from_slice(&bytes)
                .map_err(|_| format!("{path}: decoded bytes are not JSON"))?;
            matches(argument, Some(&document), vars, &format!("{path}(json)"))
        }
        "$canonical_sha256" => {
            // The sha256 digest of the value's canonical encoding matches the argument pattern:
            // how a reader recomputes a record digest (KNOWLEDGE section 3).
            let found = actual.ok_or_else(|| format!("{path}: missing"))?;
            let digest = crate::strict::sha256(&crate::strict::canonical(found));
            matches(
                argument,
                Some(&Value::String(digest)),
                vars,
                &format!("{path}(canonical sha256)"),
            )
        }
        other => Err(format!("{path}: unknown pattern directive {other}")),
    }
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let input = text.as_bytes();
    if !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks(4) {
        let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
        let mut n = 0u32;
        for c in chunk {
            n = (n << 6) | if *c == b'=' { 0 } else { value(*c)? };
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Expand outgoing templates: `{"$var": name}` and `{"$unique": prefix}`.
pub fn render(template: &Value, vars: &Vars, counter: &mut u64) -> Result<Value, String> {
    match template {
        Value::Object(map) if map.len() == 1 && map.contains_key("$var") => {
            let name = map["$var"].as_str().unwrap_or_default();
            vars.get(name)
                .cloned()
                .ok_or_else(|| format!("unknown variable {name}"))
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("$repeat") => {
            match map["$repeat"].as_array().map(Vec::as_slice) {
                Some([Value::String(text), count]) if count.is_u64() => Ok(Value::String(
                    text.repeat(count.as_u64().unwrap_or_default() as usize),
                )),
                _ => Err("$repeat takes [text, count]".into()),
            }
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("$unique") => {
            *counter += 1;
            Ok(Value::String(format!(
                "{}-{}",
                map["$unique"].as_str().unwrap_or("u"),
                counter
            )))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                out.insert(key.clone(), render(value, vars, counter)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => items
            .iter()
            .map(|item| render(item, vars, counter))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn subset_directives_and_variables() {
        let mut vars = Vars::new();
        vars.insert("ack".into(), json!({"revision": 1}));
        let actual = json!({"a": 1, "b": [1, 2], "ack": {"revision": 1}});
        assert!(
            matches(
                &json!({"a": 1, "ack": {"$var": "ack"}, "c": {"$absent": true}}),
                Some(&actual),
                &vars,
                ""
            )
            .is_ok()
        );
        assert!(
            matches(
                &json!({"b": {"$contains": 2}, "a": {"$type": "integer"}}),
                Some(&actual),
                &vars,
                ""
            )
            .is_ok()
        );
        assert!(matches(&json!({"a": 2}), Some(&actual), &vars, "").is_err());
        assert!(matches(&json!({"b": [1]}), Some(&actual), &vars, "").is_err());
        assert!(matches(&json!({"a": {"$absent": true}}), Some(&actual), &vars, "").is_err());
    }
}
