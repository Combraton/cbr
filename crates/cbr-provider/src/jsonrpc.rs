//! The JSON-RPC 2.0 mapping (STREAM section 3).
//!
//! Only the shapes the binding admits are accepted. Batches are not supported,
//! notifications are never processed, and an object carrying any member beyond
//! `jsonrpc`, `id`, `method` and `params` is invalid rather than tolerated.

use cbr_encoding::Value;

/// What a well-formed frame turned out to be.
pub enum Incoming {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A JSON-RPC notification: no `id`, `jsonrpc` 2.0, string `method`. The
    /// provider must neither process nor answer it.
    Notification,
    /// Malformed. `id` is the id to echo, or `Value::Null` when there is none
    /// that may be echoed.
    Invalid { id: Value },
}

/// A request id is a string of 1 to 128 code points, or an integer within the
/// domain's safe range. Anything else is invalid, and an invalid id may not be
/// echoed, so those answers carry `"id": null`.
fn valid_id(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let points = text.chars().count();
            (1..=128).contains(&points)
        }
        // The parser already refuses fractions and out-of-range integers at the
        // frame level, so any integer that reaches here is in range.
        Value::Int(_) => true,
        _ => false,
    }
}

pub fn classify(frame: &Value) -> Incoming {
    let members: &[(String, Value)] = match frame {
        Value::Object(members) => members,
        // A top-level array or scalar is an invalid request, answered with a
        // null id, and the connection stays open.
        _ => return Incoming::Invalid { id: Value::Null },
    };

    let known = ["jsonrpc", "id", "method", "params"];
    let has_unknown = members
        .iter()
        .any(|(name, _)| !known.contains(&name.as_str()));

    let id = frame.get("id");
    let jsonrpc_ok = matches!(frame.get("jsonrpc"), Some(Value::String(v)) if v == "2.0");
    let method = match frame.get("method") {
        Some(Value::String(name)) => Some(name.clone()),
        _ => None,
    };

    match id {
        None => {
            // No `id`. Only a well-formed notification is silently ignored;
            // anything else is an invalid request with a null id.
            if jsonrpc_ok && method.is_some() && !has_unknown {
                Incoming::Notification
            } else {
                Incoming::Invalid { id: Value::Null }
            }
        }
        Some(id) => {
            if !valid_id(id) {
                // An unusable id cannot be echoed.
                return Incoming::Invalid { id: Value::Null };
            }
            let params = frame.get("params");
            let params_ok = matches!(params, Some(value) if value.is_object());
            if !jsonrpc_ok || method.is_none() || !params_ok || has_unknown {
                return Incoming::Invalid { id: id.clone() };
            }
            Incoming::Request {
                id: id.clone(),
                method: method.expect("checked above"),
                params: params.expect("checked above").clone(),
            }
        }
    }
}

/// `invalid_request` always has retry `no` and empty details.
pub fn invalid_request_error() -> Value {
    Value::Object(vec![
        (
            "code".into(),
            Value::Int(crate::errors::jsonrpc_code::INVALID_REQUEST),
        ),
        ("message".into(), Value::String("invalid_request".into())),
        (
            "data".into(),
            Value::Object(vec![
                ("code".into(), Value::String("invalid_request".into())),
                ("retry".into(), Value::String("no".into())),
                ("details".into(), Value::Object(vec![])),
            ]),
        ),
    ])
}

pub fn response(id: Value, result: Value) -> Value {
    Value::Object(vec![
        ("jsonrpc".into(), Value::String("2.0".into())),
        ("id".into(), id),
        ("result".into(), result),
    ])
}

pub fn error_response(id: Value, error: Value) -> Value {
    Value::Object(vec![
        ("jsonrpc".into(), Value::String("2.0".into())),
        ("id".into(), id),
        ("error".into(), error),
    ])
}
