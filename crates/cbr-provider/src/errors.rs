//! Core error codes and their transport mapping.
//!
//! CORE section 12 makes the symbolic `data.code` normative and the numeric
//! transport code a property of the binding. Both are produced from one place
//! so they cannot drift apart, and every error carries a retry class, because
//! `retry` is what tells a caller whether resending the identical command is
//! safe.

use cbr_encoding::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    No,
    AfterReconcile,
    AfterRenegotiate,
}

impl Retry {
    fn wire(self) -> &'static str {
        match self {
            Retry::No => "no",
            Retry::AfterReconcile => "after_reconcile",
            Retry::AfterRenegotiate => "after_renegotiate",
        }
    }
}

/// JSON-RPC numeric codes, fixed by the stream binding (STREAM section 3).
pub mod jsonrpc_code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const FRAME_TOO_LARGE: i64 = -32010;
    pub const DOMAIN: i64 = 1;
}

/// A protocol error: a symbolic code, its retry class and its details.
#[derive(Debug, Clone)]
pub struct ProtocolError {
    pub code: &'static str,
    pub retry: Retry,
    pub details: Vec<(String, Value)>,
}

impl ProtocolError {
    fn new(code: &'static str, retry: Retry) -> Self {
        Self {
            code,
            retry,
            details: Vec::new(),
        }
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.details.push((name.to_string(), value));
        self
    }

    /// A path and a reason, the two details `invalid_envelope` always carries.
    pub fn invalid_envelope(path: &str, reason: &str) -> Self {
        Self::new("invalid_envelope", Retry::No)
            .with("path", Value::String(path.to_string()))
            .with("reason", Value::String(reason.to_string()))
    }

    pub fn method_not_found(operation: &str) -> Self {
        Self::new("method_not_found", Retry::No)
            .with("operation", Value::String(operation.to_string()))
    }

    pub fn negotiation_required() -> Self {
        Self::new("negotiation_required", Retry::AfterRenegotiate)
    }

    pub fn already_negotiated() -> Self {
        Self::new("already_negotiated", Retry::No)
    }

    pub fn profile_not_negotiated(profile: &str) -> Self {
        Self::new("profile_not_negotiated", Retry::AfterRenegotiate)
            .with("profile", Value::String(profile.to_string()))
    }

    pub fn unsupported_profile(unsatisfied: Value) -> Self {
        Self::new("unsupported_profile", Retry::No).with("unsatisfied", unsatisfied)
    }

    pub fn unsupported_version(unsatisfied: Value) -> Self {
        Self::new("unsupported_version", Retry::No).with("unsatisfied", unsatisfied)
    }

    /// At negotiation the detail is `unsatisfied`; for a message's `requires`
    /// it is `features`. CORE section 12 lists both under one code.
    pub fn unsupported_required_feature_negotiation(unsatisfied: Value) -> Self {
        Self::new("unsupported_required_feature", Retry::No).with("unsatisfied", unsatisfied)
    }

    pub fn unsupported_required_feature_message(features: Value) -> Self {
        Self::new("unsupported_required_feature", Retry::No).with("features", features)
    }

    pub fn limit_exceeded(limit: &str, maximum: i64) -> Self {
        Self::new("limit_exceeded", Retry::No)
            .with("limit", Value::String(limit.to_string()))
            .with("maximum", Value::Int(maximum))
    }

    pub fn unsupported_digest_algorithm(algorithm: &str, supported: Vec<&str>) -> Self {
        Self::new("unsupported_digest_algorithm", Retry::No)
            .with("algorithm", Value::String(algorithm.to_string()))
            .with(
                "supported",
                Value::Array(
                    supported
                        .into_iter()
                        .map(|s| Value::String(s.into()))
                        .collect(),
                ),
            )
    }

    pub fn digest_mismatch(expected: &str) -> Self {
        Self::new("digest_mismatch", Retry::No)
            .with("expected", Value::String(expected.to_string()))
    }

    pub fn idempotency_conflict(command_id: &str) -> Self {
        Self::new("idempotency_conflict", Retry::No)
            .with("command_id", Value::String(command_id.to_string()))
    }

    pub fn dedupe_history_unavailable(oldest_retained: i64) -> Self {
        Self::new("dedupe_history_unavailable", Retry::AfterReconcile)
            .with("oldest_retained", Value::Int(oldest_retained))
    }

    pub fn precondition_failed(failed: Value) -> Self {
        Self::new("precondition_failed", Retry::AfterReconcile).with("failed", failed)
    }

    /// The session already has a principal (CORE section 18). On stdio the
    /// spawner assigns it through the launch configuration, so every session
    /// is authenticated from its first frame.
    pub fn already_authenticated() -> Self {
        Self::new("already_authenticated", Retry::No)
    }

    pub fn not_found() -> Self {
        Self::new("not_found", Retry::No)
    }

    pub fn permission_denied(reason: &str) -> Self {
        Self::new("permission_denied", Retry::No).with("reason", Value::String(reason.to_string()))
    }

    /// The numeric transport code for this error.
    pub fn jsonrpc_code(&self) -> i64 {
        match self.code {
            "method_not_found" => jsonrpc_code::METHOD_NOT_FOUND,
            _ => jsonrpc_code::DOMAIN,
        }
    }

    /// The JSON-RPC `error` object for this error.
    pub fn to_error_object(&self, message: &str) -> Value {
        Value::Object(vec![
            ("code".into(), Value::Int(self.jsonrpc_code())),
            ("message".into(), Value::String(message.to_string())),
            ("data".into(), self.to_data()),
        ])
    }

    pub fn to_data(&self) -> Value {
        Value::Object(vec![
            ("code".into(), Value::String(self.code.to_string())),
            ("retry".into(), Value::String(self.retry.wire().to_string())),
            ("details".into(), Value::Object(self.details.clone())),
        ])
    }
}

/// A frame-level failure. The stream can no longer be trusted, so the receiver
/// answers once with `"id": null` and closes without reading further input
/// (STREAM section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFailure {
    TooLarge,
    InvalidUtf8,
    ParseError,
}

impl FrameFailure {
    pub fn code(self) -> &'static str {
        match self {
            FrameFailure::TooLarge => "frame_too_large",
            FrameFailure::InvalidUtf8 => "invalid_utf8",
            FrameFailure::ParseError => "parse_error",
        }
    }

    pub fn jsonrpc_code(self) -> i64 {
        match self {
            FrameFailure::TooLarge => jsonrpc_code::FRAME_TOO_LARGE,
            FrameFailure::InvalidUtf8 | FrameFailure::ParseError => jsonrpc_code::PARSE_ERROR,
        }
    }
}
