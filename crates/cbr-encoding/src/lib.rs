//! Canonical JSON, digests and command intent for Combraton Protocol `encoding/1`.
//!
//! Pinned to Protocol v0.1.0 (`cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`). The
//! normative text is vendored at `vendor/protocol/v0.1.0/docs/spec/bindings/ENCODING.md`
//! and the test vectors at `vendor/protocol/v0.1.0/conformance/vectors/encoding.json`.
//!
//! Three things live here because they are the parts where being slightly wrong
//! is indistinguishable from being right until two participants disagree about
//! whether a command is the same command:
//!
//! - a reader for the JSON value domain that refuses everything outside it;
//! - canonical form, whose bytes must match RFC 8785 exactly;
//! - digests, over exact bytes for content and over canonical form for records.

mod base64;
mod canonical;
mod digest;
mod error;
mod intent;
mod parse;
mod value;

pub use base64::{decode_base64, encode_base64};
pub use canonical::to_canonical;
pub use digest::{
    Algorithm, DigestError, digest_bytes, digest_canonical, parse_digest, sha256_hex,
};
pub use error::{Error, ErrorKind};
pub use intent::{IntentError, command_digest, command_intent};
pub use parse::{DEFAULT_MAX_DEPTH, parse, parse_with_depth};
pub use value::{MAX_SAFE_INTEGER, Value};
