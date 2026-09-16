//! Canonical form, RFC 8785 as narrowed by ENCODING section 2.
//!
//! Because section 1 excludes non-integers, RFC 8785's ECMAScript number
//! formatting collapses to plain integer formatting, so there is no float
//! printing here and no place for one to disagree.

use crate::value::Value;

/// Serialise a value to its canonical bytes.
///
/// Total: every [`Value`] is already inside the domain, so this cannot fail.
pub fn to_canonical(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Int(number) => out.extend_from_slice(number.to_string().as_bytes()),
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write(item, out);
            }
            out.push(b']');
        }
        Value::Object(members) => {
            // RFC 8785 sorts member names by their UTF-16 code units compared
            // as unsigned integers. That is not the same as sorting by code
            // point: a supplementary character encodes as surrogates in
            // D800..DFFF, which sort below U+E000..U+FFFF, so comparing Rust
            // `str`s directly would order some pairs the other way round.
            let mut ordered: Vec<&(String, Value)> = members.iter().collect();
            ordered.sort_by(|left, right| compare_utf16(&left.0, &right.0));
            out.push(b'{');
            for (index, (name, member)) in ordered.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_string(name, out);
                out.push(b':');
                write(member, out);
            }
            out.push(b'}');
        }
    }
}

fn compare_utf16(left: &str, right: &str) -> std::cmp::Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn write_string(text: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for ch in text.chars() {
        match ch {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{0008}' => out.extend_from_slice(b"\\b"),
            '\u{0009}' => out.extend_from_slice(b"\\t"),
            '\u{000A}' => out.extend_from_slice(b"\\n"),
            '\u{000C}' => out.extend_from_slice(b"\\f"),
            '\u{000D}' => out.extend_from_slice(b"\\r"),
            // Remaining C0 controls have no short escape and use lowercase hex.
            // U+007F is not escaped: JSON only requires escaping below U+0020.
            control if (control as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u{:04x}", control as u32).as_bytes());
            }
            // Everything else is emitted as UTF-8, unescaped. No normalisation
            // is applied, so U+00E9 and "e + combining acute" stay different.
            other => {
                let mut buffer = [0u8; 4];
                out.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out.push(b'"');
}
