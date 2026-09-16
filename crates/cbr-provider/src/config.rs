//! Launch configuration (`combraton-conformance-config/1`).
//!
//! CORE section 13.1: test control is environment-only. Everything here is
//! supplied when the process starts and can never be reached over the protocol,
//! so a production configuration cannot be talked into a test behaviour.
//!
//! Only the members this stage implements are read. A control CBR has not
//! implemented is not declared in its participant descriptor, so a fixture
//! needing one is reported `unsupported` under the runner's coverage limits
//! rather than silently passing.

use std::path::Path;

use cbr_encoding::Value;

/// Frame limit the conformance fixtures require a provider to advertise.
///
/// `stream.frame-limit-raised-after-negotiation` sends a 1,572,864-byte frame,
/// requires `frame_too_large` before negotiation under the binding's 1 MiB
/// default, and requires the same frame to be accepted afterwards against an
/// advertised `max_frame_bytes` of exactly 2,097,152. The fixture passes no
/// configuration override, so this is the default a conforming provider has.
pub const DEFAULT_MAX_FRAME_BYTES: i64 = 2_097_152;

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_frame_bytes: i64,
    pub max_payload_bytes: i64,
    pub max_string_bytes: i64,
    pub max_array_items: i64,
    pub max_depth: i64,
}

impl Default for Limits {
    fn default() -> Self {
        // Only `max_frame_bytes` is fixed by a fixture at this stage. The rest
        // are provisional and are settled by the Core suite in stage (c);
        // fixtures that exercise them supply their own overrides.
        Self {
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_payload_bytes: 1_048_576,
            max_string_bytes: 262_144,
            max_array_items: 4096,
            max_depth: 32,
        }
    }
}

impl Limits {
    pub fn to_value(&self) -> Value {
        Value::Object(vec![
            ("max_frame_bytes".into(), Value::Int(self.max_frame_bytes)),
            (
                "max_payload_bytes".into(),
                Value::Int(self.max_payload_bytes),
            ),
            ("max_string_bytes".into(), Value::Int(self.max_string_bytes)),
            ("max_array_items".into(), Value::Int(self.max_array_items)),
            ("max_depth".into(), Value::Int(self.max_depth)),
        ])
    }
}

/// Members that exist only to drive conformance fixtures. A production
/// configuration naming any of them is refused rather than ignored: silently
/// dropping a control the operator wrote down would leave them believing a
/// clock or a fault injector was in effect when it was not.
const TEST_CONTROL_MEMBERS: [&str; 11] = [
    "clock",
    "capabilities",
    "credentials",
    "test_barriers",
    "executor",
    "context",
    "knowledge",
    "evidence",
    "verifier",
    "faults",
    "events",
];

/// Which launch configuration a process was given. CORE section 13.1 makes
/// test control environment-only, so this is decided once at startup from the
/// configuration's declared format and can never be reached over the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No configuration, or a `cbr-config/1` one. `core-test/1` is not served
    /// and no test control exists.
    Production,
    /// A `combraton-conformance-config/1` configuration. `core-test/1` is
    /// served and the documented controls are honoured.
    Conformance,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    pub principal: String,
    pub authority_principals: Vec<String>,
    pub provider_id: String,
    pub limits: Limits,
    /// Generations the provider advances by when it starts, so a fixture can
    /// make it forget a command record without touching the protocol.
    pub dedupe_advance_on_start: i64,
    pub dedupe_retain_generations: i64,
    /// Start a new stream epoch on this launch, as a provider does when it can
    /// no longer vouch for continuity.
    pub events_new_epoch_on_start: bool,
    /// How many trailing events of the closing epoch are no longer vouched for.
    pub events_unvouched_last: i64,
    /// Keep only this many events, discarding earlier ones with a watermark.
    pub events_retain_last: Option<i64>,
    /// The per-connection bound on output produced but not yet written
    /// (CORE section 16.5). The conformance launch configuration documents
    /// 8 MiB as the default.
    pub events_max_pending_notification_bytes: i64,
    /// How long a stalled consumer has to drain everything pending, and the
    /// one budget every ending notice on a connection shares. Default 1000.
    pub events_backpressure_notice_ms: i64,
    /// Where protocol-visible time comes from. Always the system clock in
    /// production, because `clock` is a test control and is refused there.
    pub clock: crate::clock::Source,
    /// Capability statuses the launch configuration sets, by predicate name.
    pub capabilities: Vec<(String, String)>,
    /// Principal credentials for a shared transport (CORE section 18.1): the
    /// SHA-256 digest of each whole credential string, its principal and
    /// whether it is revoked. The credential itself is never kept.
    pub credentials: Vec<Credential>,
    /// Test barriers (decision 007): a directory and the barrier names enabled.
    pub test_barriers: Option<(std::path::PathBuf, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub principal: String,
    pub digest: String,
    pub revoked: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Production,
            principal: "caller".into(),
            authority_principals: Vec::new(),
            provider_id: "cbr".into(),
            limits: Limits::default(),
            dedupe_advance_on_start: 0,
            dedupe_retain_generations: 1,
            events_new_epoch_on_start: false,
            events_unvouched_last: 0,
            events_retain_last: None,
            events_max_pending_notification_bytes: 8 * 1024 * 1024,
            events_backpressure_notice_ms: 1000,
            clock: crate::clock::Source::System,
            capabilities: Vec::new(),
            credentials: Vec::new(),
            test_barriers: None,
        }
    }
}

fn int(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Int(number)) => Some(*number),
        _ => None,
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

impl Config {
    /// Read a launch configuration, falling back to defaults for anything the
    /// file does not set. A missing path is not an error: a provider launched
    /// without one uses defaults.
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let mut config = Config::default();
        let Some(path) = path else { return Ok(config) };
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let value =
            cbr_encoding::parse(&bytes).map_err(|e| format!("parsing {}: {e}", path.display()))?;

        config.mode = match value.get("format").and_then(Value::as_str) {
            Some("combraton-conformance-config/1") => Mode::Conformance,
            Some("cbr-config/1") => Mode::Production,
            Some(other) => return Err(format!("unknown launch configuration format {other}")),
            None => return Err("launch configuration has no format".into()),
        };

        // Refuse a test control in a production configuration, naming it. The
        // check is on the configuration's own members, so a control cannot
        // arrive unnoticed through a member this build does not otherwise read.
        if config.mode == Mode::Production {
            for member in value.keys() {
                if TEST_CONTROL_MEMBERS.contains(&member) {
                    return Err(format!(
                        "`{member}` is a conformance test control and has no effect in a \
                         production configuration; it is refused rather than ignored"
                    ));
                }
            }
            if value
                .get("dedupe")
                .and_then(|d| d.get("advance_on_start"))
                .is_some()
            {
                return Err(
                    "`dedupe.advance_on_start` is a conformance test control and has no \
                            effect in a production configuration; it is refused rather than ignored"
                        .into(),
                );
            }
        }

        if let Some(principal) = text(value.get("principal")) {
            config.principal = principal;
        }
        if let Some(Value::Array(items)) = value.get("authority_principals") {
            config.authority_principals = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
        }
        // A grant's `audience` must equal the issuing provider's own id (CORE
        // section 15.2), so this is what every fixture's grant is addressed to.
        // Single-participant fixtures never set it and always issue to
        // `conformance-provider`, so that is the conformance default; the
        // composition fixtures, which run several providers at once, set it
        // explicitly. A production launch keeps CBR's own id.
        if config.mode == Mode::Conformance {
            config.provider_id = "conformance-provider".into();
            // The runner's own default, which it writes a credential for on
            // the socket form; the stdio form never depended on the name. An
            // explicit `principal` always wins.
            if value.get("principal").is_none() {
                config.principal = "conformance-caller".into();
            }
        }
        if let Some(id) = text(value.get("provider_id")) {
            config.provider_id = id;
        }
        if let Some(limits) = value.get("limits") {
            let current = &mut config.limits;
            if let Some(v) = int(limits.get("max_frame_bytes")) {
                current.max_frame_bytes = v;
            }
            if let Some(v) = int(limits.get("max_payload_bytes")) {
                current.max_payload_bytes = v;
            }
            if let Some(v) = int(limits.get("max_string_bytes")) {
                current.max_string_bytes = v;
            }
            if let Some(v) = int(limits.get("max_array_items")) {
                current.max_array_items = v;
            }
            if let Some(v) = int(limits.get("max_depth")) {
                current.max_depth = v;
            }
        }
        if let Some(events) = value.get("events") {
            config.events_new_epoch_on_start =
                matches!(events.get("new_epoch_on_start"), Some(Value::Bool(true)));
            if let Some(v) = int(events.get("unvouched_last")) {
                config.events_unvouched_last = v;
            }
            if let Some(v) = int(events.get("retain_last")) {
                config.events_retain_last = Some(v);
            }
            if let Some(v) = int(events.get("max_pending_notification_bytes")) {
                config.events_max_pending_notification_bytes = v.max(1);
            }
            if let Some(v) = int(events.get("backpressure_notice_ms")) {
                config.events_backpressure_notice_ms = v.max(0);
            }
        }
        if let Some(Value::Array(entries)) = value.get("credentials") {
            for entry in entries {
                let Some(credential) = entry.get("credential").and_then(Value::as_str) else {
                    return Err("each credential entry needs a `credential` string".into());
                };
                // `ccred1.<principal>.<secret>`; only the digest is kept.
                let principal = credential.split('.').nth(1).unwrap_or_default().to_string();
                config.credentials.push(Credential {
                    principal,
                    digest: cbr_encoding::sha256_hex(credential.as_bytes()),
                    revoked: matches!(entry.get("revoked"), Some(Value::Bool(true))),
                });
            }
        }
        if let Some(barriers) = value.get("test_barriers") {
            let directory = barriers
                .get("directory")
                .and_then(Value::as_str)
                .ok_or("`test_barriers` needs a `directory`")?;
            let enabled = barriers
                .get("enabled")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            config.test_barriers = Some((directory.into(), enabled));
        }
        if let Some(Value::Object(members)) = value.get("capabilities") {
            for (name, status) in members {
                match status.as_str() {
                    Some(status @ ("supported" | "unsupported" | "unknown")) => {
                        config.capabilities.push((name.clone(), status.to_string()));
                    }
                    _ => {
                        return Err(format!(
                            "capability `{name}` must be supported, unsupported or unknown"
                        ));
                    }
                }
            }
        }
        if let Some(clock) = value.get("clock") {
            config.clock = match (clock.get("fixed"), clock.get("file")) {
                (Some(Value::String(instant)), None) if crate::grants::is_instant(instant) => {
                    crate::clock::Source::Fixed(instant.clone())
                }
                (None, Some(Value::String(path))) if !path.is_empty() => {
                    crate::clock::Source::File(path.into())
                }
                _ => {
                    return Err(
                        "`clock` must be exactly one of a `fixed` UTC instant or a `file` path"
                            .into(),
                    );
                }
            };
        }
        if let Some(dedupe) = value.get("dedupe") {
            if let Some(v) = int(dedupe.get("advance_on_start")) {
                config.dedupe_advance_on_start = v;
            }
            if let Some(v) = int(dedupe.get("retain_generations")) {
                config.dedupe_retain_generations = v;
            }
        }

        // The conformance launch defaults the authority set to the session
        // principal, which is what makes a fixture's ordinary commands
        // authorised without issuing a grant first.
        if config.authority_principals.is_empty() {
            config.authority_principals = vec![config.principal.clone()];
        }
        Ok(config)
    }

    pub fn is_authority(&self, principal: &str) -> bool {
        self.authority_principals
            .iter()
            .any(|name| name == principal)
    }
}
