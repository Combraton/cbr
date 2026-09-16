//! The JSON value domain of ENCODING section 1.
//!
//! Deliberately narrower than JSON: integers only, no duplicate member names,
//! and no code points that the profile excludes. A value that exists here has
//! already been proven to be inside the domain, so serialising it cannot fail.

/// A value inside the protocol's JSON domain.
///
/// Object members keep their parsed order. Canonical form sorts them
/// ([`crate::canonical`]), so the stored order matters only for round-tripping
/// a value back to a caller in the shape it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    /// An integer within +/- (2^53 - 1). The domain has no other numbers.
    Int(i64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// The largest magnitude the domain admits, from ENCODING section 1 rule 3.
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

impl Value {
    /// Look a member up by name. Object member names are unique, so at most one
    /// can match.
    pub fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The member names of an object, in parsed order; empty for other values.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        let members: &[(String, Value)] = match self {
            Value::Object(members) => members,
            _ => &[],
        };
        members.iter().map(|(key, _)| key.as_str())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
}

/// True for code points ENCODING section 1 rule 2 excludes: the U+FDD0..=U+FDEF
/// block, and every code point whose low 16 bits are FFFE or FFFF.
pub(crate) fn is_noncharacter(code_point: u32) -> bool {
    (0xFDD0..=0xFDEF).contains(&code_point) || (code_point & 0xFFFE) == 0xFFFE
}
