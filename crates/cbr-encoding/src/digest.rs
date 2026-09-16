//! Digest strings, ENCODING section 3.
//!
//! A digest here is always `<algorithm>:<lowercase hex>`. `sha256` is mandatory
//! to implement; `sha512` is optional and gated on a negotiated Core feature,
//! so parsing accepts it while callers decide whether it is selected. MD5 and
//! SHA-1 are never supported.

use sha2::{Digest, Sha256};

use crate::canonical::to_canonical;
use crate::value::Value;

/// `sha256:` followed by 64 lowercase hex characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in out {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The algorithm-qualified digest of exact bytes.
///
/// Content digests are always over bytes. Re-serialising JSON content produces
/// different content, not the same content in another shape.
pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

/// The digest of a value's canonical form.
///
/// Used for command intent and for record digests such as a knowledge claim
/// revision, where the digest is defined over the value rather than over the
/// bytes that happened to arrive.
pub fn digest_canonical(value: &Value) -> String {
    digest_bytes(&to_canonical(value))
}

/// Algorithms this build knows about. Whether one may be *used* is a
/// negotiation question, not a parsing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Sha256,
    Sha512,
}

impl Algorithm {
    fn hex_length(self) -> usize {
        match self {
            Algorithm::Sha256 => 64,
            Algorithm::Sha512 => 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestError {
    /// Does not match the digest grammar at all.
    Malformed,
    /// Well formed, but names an algorithm this build does not support. The
    /// protocol layer reports this as `unsupported_digest_algorithm`, never as
    /// a silent pass.
    UnsupportedAlgorithm(String),
    /// Right algorithm, wrong encoded length.
    WrongLength,
}

/// Validate a digest string and report which algorithm it names.
///
/// Grammar: `^[a-z0-9]+([+._-][a-z0-9]+)*:[0-9a-f]+$`, with the encoded length
/// checked against the named algorithm.
pub fn parse_digest(text: &str) -> Result<(Algorithm, &str), DigestError> {
    let (algorithm, encoded) = text.split_once(':').ok_or(DigestError::Malformed)?;
    if !valid_algorithm_name(algorithm) {
        return Err(DigestError::Malformed);
    }
    if encoded.is_empty()
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(DigestError::Malformed);
    }
    let named = match algorithm {
        "sha256" => Algorithm::Sha256,
        "sha512" => Algorithm::Sha512,
        other => return Err(DigestError::UnsupportedAlgorithm(other.to_string())),
    };
    if encoded.len() != named.hex_length() {
        return Err(DigestError::WrongLength);
    }
    Ok((named, encoded))
}

fn valid_algorithm_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // One or more `[a-z0-9]+` groups joined by a single separator.
    name.split(['+', '.', '_', '-']).all(|group| {
        !group.is_empty()
            && group
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}
