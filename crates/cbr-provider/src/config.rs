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
const TEST_CONTROL_MEMBERS: [&str; 13] = [
    "model",
    "evidence_store",
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
    /// The `evidence.store` test control (EVIDENCE section 14).
    pub evidence_store: EvidenceStore,
    /// The applicability evaluator pinned in evaluations, and the
    /// `knowledge.store` test control (KNOWLEDGE section 13).
    pub knowledge: KnowledgeStore,
    /// A ceiling for this whole run, from the launch. It can only **lower**
    /// the owner's envelope: it is checked after the two counters, so a
    /// number above them changes nothing. m4e's five-million cap across
    /// three journeys is this, enforced rather than intended.
    pub model_run_ceiling: Option<u64>,
    /// The model this launch may call, if any. **Absent is the default and
    /// the ordinary case**; present is what makes the launch read a
    /// credential and refuse to start if it cannot.
    pub model_runtime: Option<ModelRuntime>,
    /// The `model.fake` test control: one scripted call through
    /// [`crate::model::Runtime`] at startup, so the ledger's crash
    /// boundaries are reachable by a process that can be killed at them.
    /// `None` in production, where the member is refused outright — there is
    /// no transport in this build and nothing else may pretend there is.
    pub model: Option<FakeModel>,
    /// The `context.script` test control (CONTEXT section 12): scripted
    /// preparation per request, and the peers a context provider reaches over
    /// the public protocol. `Null` when absent. It holds peer credentials, so
    /// it is never logged or echoed.
    pub context: ContextControl,
}

/// A model this launch is configured to call.
///
/// **Its presence is the only thing that makes CBR read a credential.** A
/// launch without it never touches the Keychain, which is what CI, every
/// conformance run and every developer running the suite are. It is
/// ordinary configuration, not a test control: the `model` member beside it
/// is the fake transport's control and is refused in production.
// The serializer that reads these is the next commit's; the launch that
// validates them is this one's, and the validation is what has to exist
// before a credential is ever read.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ModelRuntime {
    /// The provider id, which may only ever be [`crate::wire::PROVIDER_ID`].
    pub provider: String,
    /// The endpoint, from configuration rather than a literal in the call
    /// path. Always `https`.
    pub endpoint: String,
    /// Where admission counts go. **Not part of either compatibility
    /// surface**: both dialects count at MiniMax's own endpoint, so this is
    /// configured separately rather than derived from the one above.
    pub count_endpoint: String,
    pub dialect: crate::wire::Dialect,
    /// One of [`crate::wire::MODELS`], recorded in every derivation record.
    pub model: String,
}

/// The `model.fake` control's value: what to call with, and what the fake
/// transport should answer.
#[derive(Debug, Clone)]
pub struct FakeModel {
    pub job: String,
    pub request: String,
    pub body: String,
    /// `usage:<n>`, `provider_exhausted`, `failed` or `not_sent`. It is the
    /// **completion's** answer; the count is always answered as a count.
    pub answer: String,
    /// The generation limit the body declares and the reservation covers.
    pub generation: u64,
    /// Which dialect the scripted body is framed in, so that the guard on
    /// the generation limit reads the member that dialect binds it to.
    pub dialect: crate::wire::Dialect,
}

/// The `context.script` control's value. Its `Debug` form names nothing it
/// holds, because peers in it carry credentials (CORE section 18.1).
#[derive(Clone)]
pub struct ContextControl(pub Value);

impl std::fmt::Debug for ContextControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContextControl(..)")
    }
}

/// The evaluator identity and the knowledge test control.
#[derive(Debug, Clone)]
pub struct KnowledgeStore {
    pub evaluator_id: String,
    pub evaluator_version: String,
    /// The condition kinds the evaluator implements; any other is
    /// `unsupported`, never guessed.
    pub condition_kinds: Vec<String>,
    /// Claims whose inspected record is altered while the reference and
    /// digest stay the same, so readers are tested on recomputing digests.
    pub serve_altered_claims: Vec<String>,
}

impl KnowledgeStore {
    /// CBR's own evaluator. A conformance launch starts from the control's
    /// documented default instead (below).
    fn production() -> Self {
        Self {
            evaluator_id: "cbr-conditions".into(),
            evaluator_version: "1".into(),
            condition_kinds: crate::knowledge::CONDITION_KINDS
                .iter()
                .map(|k| k.to_string())
                .collect(),
            serve_altered_claims: Vec::new(),
        }
    }
}

/// Scripted store behaviour for evidence conformance. Every member is empty
/// or absent outside a conformance launch, which refuses it.
#[derive(Debug, Clone, Default)]
pub struct EvidenceStore {
    /// Artifacts whose stored bytes are damaged after sealing, so the
    /// integrity check that refuses to serve them runs against real bytes.
    pub corrupt: Vec<String>,
    /// Artifacts the store cannot serve.
    pub unavailable: Vec<String>,
    /// Artifacts whose fetched bytes are altered behind honest metadata.
    pub serve_altered_bytes: Vec<String>,
    /// A staged upload untouched this long is abandoned as `staging_expired`.
    pub staging_timeout_seconds: Option<i64>,
    /// Physical deletion is confirmed this long after the purge request.
    pub deletion_delay_seconds: Option<i64>,
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
            model: None,
            model_runtime: None,
            model_run_ceiling: None,
            test_barriers: None,
            evidence_store: EvidenceStore::default(),
            knowledge: KnowledgeStore::production(),
            context: ContextControl(Value::Null),
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
        if config.mode == Mode::Conformance {
            // The conformance README documents the control's default: the
            // `reference-conditions` evaluator, version 1, all three kinds.
            config.knowledge.evaluator_id = "reference-conditions".into();
        }
        if let Some(knowledge) = value.get("knowledge") {
            if let Some(evaluator) = knowledge.get("evaluator") {
                if let Some(id) = text(evaluator.get("id")) {
                    config.knowledge.evaluator_id = id;
                }
                if let Some(version) = text(evaluator.get("version")) {
                    config.knowledge.evaluator_version = version;
                }
                if let Some(Value::Array(kinds)) = evaluator.get("condition_kinds") {
                    config.knowledge.condition_kinds = kinds
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect();
                }
            }
            if let Some(Value::Array(claims)) = knowledge.get("serve_altered_claims") {
                config.knowledge.serve_altered_claims = claims
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
            }
        }
        if let Some(context) = value.get("context") {
            if !context.is_object() {
                return Err("`context` must be an object".into());
            }
            config.context = ContextControl(context.clone());
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
        if let Some(store) = value.get("evidence_store") {
            let names = |member: &str| -> Vec<String> {
                store
                    .get(member)
                    .and_then(Value::as_array)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            };
            config.evidence_store = EvidenceStore {
                corrupt: names("corrupt"),
                unavailable: names("unavailable"),
                serve_altered_bytes: names("serve_altered_bytes"),
                staging_timeout_seconds: int(store.get("staging_timeout_seconds")),
                deletion_delay_seconds: int(store.get("deletion_delay_seconds")),
            };
        }
        if let Some(model) = value.get("model") {
            config.model = Some(FakeModel {
                job: model
                    .get("job")
                    .and_then(Value::as_str)
                    .unwrap_or("model-fake")
                    .to_string(),
                request: model
                    .get("request")
                    .and_then(Value::as_str)
                    .unwrap_or("call")
                    .to_string(),
                body: model
                    .get("body")
                    .and_then(Value::as_str)
                    .unwrap_or("{\"max_tokens\":64}")
                    .to_string(),
                answer: model
                    .get("answer")
                    .and_then(Value::as_str)
                    .unwrap_or("usage:64")
                    .to_string(),
                generation: match model.get("generation") {
                    Some(Value::Int(generation)) => (*generation).max(0) as u64,
                    _ => 64,
                },
                dialect: model
                    .get("dialect")
                    .and_then(Value::as_str)
                    .and_then(crate::wire::Dialect::parse)
                    .unwrap_or(crate::wire::Dialect::OpenAi),
            });
        }
        // **Validated here, which is before any credential is read.** A
        // configuration mistake is then refused identically on every
        // machine, and the Keychain is never touched to discover that the
        // launch was never going to work.
        if let Some(runtime) = value.get("model_runtime") {
            let provider = text(runtime.get("provider"))
                .ok_or("`model_runtime` needs a `provider`".to_string())?;
            if provider != crate::wire::PROVIDER_ID {
                return Err(format!(
                    "model provider `{provider}` is not configured; CBR admits `{}` and                      no other, which is the owner's decision rather than a default",
                    crate::wire::PROVIDER_ID
                ));
            }
            let named = text(runtime.get("dialect"))
                .ok_or("`model_runtime` needs a `dialect`".to_string())?;
            let dialect = crate::wire::Dialect::parse(&named)
                .ok_or(format!("unknown model dialect `{named}`"))?;
            let model =
                text(runtime.get("model")).ok_or("`model_runtime` needs a `model`".to_string())?;
            if !crate::wire::MODELS.contains(&model.as_str()) {
                return Err(format!(
                    "model `{model}` is outside the three the owner named: {}",
                    crate::wire::MODELS.join(", ")
                ));
            }
            let endpoint = text(runtime.get("endpoint"))
                .unwrap_or_else(|| dialect.default_endpoint().to_string());
            let count_endpoint = text(runtime.get("count_endpoint"))
                .unwrap_or_else(|| crate::wire::COUNT_ENDPOINT.to_string());
            for named in [&endpoint, &count_endpoint] {
                if !named.starts_with("https://") {
                    return Err(format!(
                        "model endpoint `{named}` is not https; repository text goes over it"
                    ));
                }
            }
            config.model_runtime = Some(ModelRuntime {
                provider,
                endpoint,
                count_endpoint,
                dialect,
                model,
            });
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
