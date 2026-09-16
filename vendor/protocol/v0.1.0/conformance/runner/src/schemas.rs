//! Normative schemas used to validate every frame the participant sends.

use std::collections::HashMap;
use std::path::Path;

use jsonschema::Validator;
use serde_json::{Value, json};

const BASE: &str = "https://github.com/Combraton/protocol/schemas";

pub struct Schemas {
    response: Validator,
    results: HashMap<String, Validator>,
    notifications: HashMap<String, Validator>,
    pub fixture: Validator,
    pub launch_config: Validator,
}

fn load_dir(dir: &Path, out: &mut Vec<(String, Value)>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            load_dir(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let value: Value =
                serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            let id = value["$id"]
                .as_str()
                .ok_or_else(|| format!("{}: missing $id", path.display()))?
                .to_string();
            out.push((id, value));
        }
    }
    Ok(())
}

impl Schemas {
    pub fn load(repo: &Path) -> Result<Self, String> {
        let mut resources = Vec::new();
        load_dir(&repo.join("schemas"), &mut resources)?;
        load_dir(&repo.join("conformance/schemas"), &mut resources)?;
        let ids: Vec<String> = resources.iter().map(|(id, _)| id.clone()).collect();
        let registry = jsonschema::Registry::new()
            .extend(resources)
            .map_err(|e| e.to_string())?
            .prepare()
            .map_err(|e| e.to_string())?;
        let build = |id: &str| {
            jsonschema::options()
                .with_registry(&registry)
                .build(&json!({"$ref": id}))
                .map_err(|e| format!("{id}: {e}"))
        };
        let mut notifications = HashMap::new();
        for id in &ids {
            if let Some(rest) = id.strip_suffix(".notification.schema.json") {
                let method = rest.rsplit('/').next().unwrap_or_default().to_string();
                notifications.insert(method, build(id)?);
            }
        }
        let mut results = HashMap::new();
        for id in &ids {
            if let Some(rest) = id.strip_suffix(".result.schema.json") {
                let operation = rest.rsplit('/').next().unwrap_or_default().to_string();
                results.insert(operation, build(id)?);
            }
        }
        Ok(Self {
            response: build(&format!(
                "{BASE}/stream/1/jsonrpc.schema.json#/$defs/response"
            ))?,
            results,
            notifications,
            fixture: build(
                "https://github.com/Combraton/protocol/conformance/schemas/fixture.schema.json",
            )?,
            launch_config: build(
                "https://github.com/Combraton/protocol/conformance/schemas/launch-config.schema.json",
            )?,
        })
    }

    /// Validate a provider notification frame against its method's schema.
    pub fn check_notification(&self, frame: &Value) -> Result<(), String> {
        let method = frame["method"].as_str().unwrap_or_default();
        let validator = self
            .notifications
            .get(method)
            .ok_or_else(|| format!("notification with unknown method {method:?}"))?;
        if let Some(error) = validator.iter_errors(frame).next() {
            return Err(format!(
                "{method} notification violates its schema at {}: {error}",
                error.instance_path()
            ));
        }
        Ok(())
    }

    /// Validate a response frame, including the result schema of a known operation.
    pub fn check_response(&self, frame: &Value, method: Option<&str>) -> Result<(), String> {
        if let Some(error) = self.response.iter_errors(frame).next() {
            return Err(format!(
                "response violates stream/1 JSON-RPC schema at {}: {error}",
                error.instance_path()
            ));
        }
        if let (Some(result), Some(method)) = (frame.get("result"), method)
            && let Some(validator) = self.results.get(method)
            && let Some(error) = validator.iter_errors(result).next()
        {
            return Err(format!(
                "{method} result violates its schema at {}: {error}",
                error.instance_path()
            ));
        }
        Ok(())
    }
}

/// Binding numeric codes, typed from STREAM section 3 independently of the provider.
pub fn jsonrpc_code(code: &str) -> i64 {
    match code {
        "parse_error" | "invalid_utf8" => -32700,
        "frame_too_large" => -32010,
        "invalid_request" => -32600,
        "method_not_found" => -32601,
        "overloaded" => -32011,
        _ => 1,
    }
}

/// Retry classes, typed from CORE section 12 independently of the provider.
pub fn retry_class(code: &str) -> &'static str {
    match code {
        "negotiation_required" | "profile_not_negotiated" => "after_renegotiate",
        "dedupe_history_unavailable"
        | "effect_history_unavailable"
        | "stale_authority_epoch"
        | "precondition_failed"
        | "internal_error"
        | "upload_offset_mismatch"
        | "upload_incomplete"
        | "hold_active" => "after_reconcile",
        "unavailable" | "overloaded" => "same_command",
        "capability_unavailable" | "contract_unavailable" => "after_reconcile",
        _ => "no",
    }
}
