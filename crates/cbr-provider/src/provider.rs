//! The provider: negotiation and the Core command path.
//!
//! CORE section 10 fixes the order of checks, and the first failing step
//! determines the error. That order is not an implementation detail: it is what
//! stops an unauthorised caller learning whether a subject exists, and what
//! lets a caller retransmit a command after its authority epoch changed. The
//! steps below are numbered to match the specification.

use cbr_encoding::Value;

use crate::config::Config;
use crate::effects;
use crate::envelope::{self, Command, Query};
use crate::errors::ProtocolError;
use crate::grants::{self, Grant};
use crate::store::{Store, SubjectKey};

/// Profiles this build serves, with the majors and features it implements.
/// A profile is listed here only when it is implemented: over-claiming would
/// make negotiation succeed and then fail at the first operation.
const SERVED: &[(&str, i64, &[&str], &[&str])] = &[
    (
        "core",
        1,
        &[
            "core.events",
            "core.grants",
            "core.capabilities",
            "core.events.backpressure",
            "core.effects",
        ],
        &[],
    ),
    ("core-test", 1, &[], &["core"]),
];

/// Profiles this provider *declares* unsupported, which is a narrower thing
/// than "does not serve".
///
/// CORE section 4.1 requires a 0.1 release to list at least `coordination` and
/// `remote-trust`, and those are genuinely out of the release. Nothing else
/// belongs here. A declaration makes negotiation report `declared_unsupported`,
/// whereas a profile the provider simply does not serve is reported
/// `unknown_profile`, and the fixture
/// `core.negotiation.execution-requests-against-any-provider` accepts only the
/// latter for `execution`: its title is "an optional execution/1 request is
/// selected by providers that support it and reported unknown by older ones".
///
/// So `execution` is absent here even though CBR will never serve it, and
/// `evidence`, `context` and `knowledge` are absent because later milestones
/// implement them. Declaring either kind would be a statement this provider is
/// not entitled to make.
const UNSUPPORTED: &[(&str, &str)] = &[
    ("coordination", "not_in_release"),
    ("remote-trust", "not_in_release"),
];

/// Operations answerable before negotiation (CORE section 3).
const PRE_NEGOTIATION: [&str; 4] = [
    "core.describe",
    "core.negotiate",
    "core.feature_dependencies",
    "core.authenticate",
];

/// Operations no profile protects (CORE section 15.5). Everything else needs
/// authorization at step 6, **before** any check that depends on whether a
/// subject exists, so an unauthorised principal cannot learn that a subject is
/// absent (CORE-12).
///
/// `core.events.unsubscribe` is here because it removes only the session's own
/// subscriptions, so there is nothing to authorize that the session does not
/// already hold. `core.capabilities` is here because CORE section 15.5 lists
/// it: what a provider can do right now is not a secret of any subject.
const UNPROTECTED: [&str; 6] = [
    "core.describe",
    "core.negotiate",
    "core.feature_dependencies",
    "core.authenticate",
    "core.events.unsubscribe",
    "core.capabilities",
];

/// Every operation that is a command rather than a query. A command carries a
/// command identity, so its step 6 runs after deduplication; a query has none,
/// so its step 6 runs first.
const COMMANDS: [&str; 5] = [
    "core-test.subject.put",
    "core-test.authority.claim",
    "core.grant.issue",
    "core.grant.revoke",
    "core.effects.abort_obligation",
];

/// What step 6 means for one command (CORE section 10).
enum Step6 {
    /// An operation a profile protects: these rights over these subjects,
    /// evaluated under CORE section 15.5.
    Rights(Vec<grants::Need>),
    /// `core.grant.issue`, which has its own rules (CORE section 15.3).
    Issue(Box<Grant>),
    /// `core.grant.revoke`, likewise.
    Revoke,
    /// `core.effects.abort_obligation` (CORE section 19.4).
    Abort,
}

/// A subject key from an envelope's subject object.
fn key_of(subject: &Value) -> SubjectKey {
    SubjectKey {
        kind: subject
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        id: subject
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
    }
}

/// The `core.grant.revoked` event: one per revoked grant, payload fixed by
/// CORE section 16.2.
fn revoked_event(caused_by: &[String]) -> crate::store::NewEvent {
    crate::store::NewEvent {
        event_type: "core.grant.revoked".into(),
        caused_by: caused_by.to_vec(),
        payload: Box::new(|_| {
            Value::Object(vec![("state".into(), Value::String("revoked".into()))])
        }),
    }
}

/// Compare two byte strings in time that depends only on their length.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// The effect id a `core.effects.*` payload names.
fn effect_id_of(payload: &Value) -> Result<String, ProtocolError> {
    match payload.get("effect").and_then(Value::as_str) {
        Some(id) if crate::envelope::is_identifier(id) => Ok(id.to_string()),
        _ => Err(ProtocolError::invalid_envelope(
            "/payload/effect",
            "not an identifier",
        )),
    }
}

/// The capability an operation depends on, if any (CORE section 17.4). Only a
/// write depends on `core-test.writes`: claiming an authority epoch does not,
/// which `core.capabilities.claim-does-not-depend-on-writes` pins.
fn capability_dependency(operation: &str) -> Option<&'static str> {
    match operation {
        "core-test.subject.put" => Some("core-test.writes"),
        _ => None,
    }
}

/// The response to an accepted command: the acknowledgment, the outcome and
/// `replay: false` (CORE section 11).
fn accepted(
    command_id: &str,
    digest: &str,
    operation_ref: &str,
    subject: &Value,
    revision: i64,
    outcome: Value,
) -> Value {
    Value::Object(vec![
        (
            "acknowledgment".into(),
            Value::Object(vec![
                ("command_id".into(), Value::String(command_id.to_string())),
                ("command_digest".into(), Value::String(digest.to_string())),
                (
                    "operation_ref".into(),
                    Value::String(operation_ref.to_string()),
                ),
                ("subject".into(), subject.clone()),
                ("revision".into(), Value::Int(revision)),
                ("effect_refs".into(), Value::Array(vec![])),
            ]),
        ),
        ("outcome".into(), outcome),
        ("replay".into(), Value::Bool(false)),
    ])
}

/// The `core.grant.*` operations are protected, but by their own rules rather
/// than by section 15.5's, and a `grant` field on them is validated and not
/// evaluated. They are therefore neither "unprotected" nor ordinary.
fn is_grant_operation(operation: &str) -> bool {
    operation.starts_with("core.grant.")
}

/// One process-wide lock serializing request processing and subscription
/// re-checks across every session (STACK section 2). A re-check re-authorizes
/// a subscription and reads the events to deliver under it, so no command can
/// commit between the two: an item committed after a revocation is never
/// delivered under the revoked grant (CORE section 16.5).
static PROCESSING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn processing() -> std::sync::MutexGuard<'static, ()> {
    match PROCESSING.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            crate::barriers::signal(crate::barriers::LOCK_CONTENDED);
            PROCESSING
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
    }
}

pub struct Provider {
    pub config: Config,
    clock: std::sync::Arc<crate::clock::Clock>,
    store: Store,
    /// Whether this session is on a transport many processes can reach, where
    /// it starts unauthenticated (CORE section 18.2).
    shared_transport: bool,
    /// Whether the session has a principal. Always true on stdio, where the
    /// launching process assigns it.
    authenticated: bool,
    negotiated: Option<Vec<(String, i64, Vec<String>)>>,
    dedupe_current: i64,
    dedupe_oldest: i64,
    /// The largest frame this caller accepts, from its negotiation request.
    caller_receive_limit: usize,
    subscriptions: Vec<Subscription>,
    next_subscription: u64,
}

impl Provider {
    /// Open the provider over its data directory, advancing the deduplication
    /// generation for this process.
    pub fn open(
        config: Config,
        clock: crate::clock::Clock,
        data_dir: &std::path::Path,
    ) -> Result<Self, crate::store::StoreError> {
        let mut store = Store::open(data_dir)?;
        // Epoch and retention changes belong to process start, before anything
        // is read, so a consumer never sees the stream change under it mid-read.
        if config.events_new_epoch_on_start {
            store.start_new_epoch(config.events_unvouched_last)?;
        }
        if let Some(retain) = config.events_retain_last {
            store.retain_last_events(retain)?;
        }
        // The capability snapshot is reconciled at launch, after any epoch
        // change, so its change event lands in the epoch the process serves.
        //
        // Recording is not gated on negotiation (CORE section 16.3): the
        // snapshot and its change event exist whether or not any session
        // negotiates `core.capabilities`, which is why
        // `core.events.visibility-follows-direct-read-authority` needed them in
        // c3 while declaring only `core.events` and `core.grants`.
        let predicates = if config.mode == crate::config::Mode::Conformance {
            let status = config
                .capabilities
                .iter()
                .find(|(name, _)| name == "core-test.writes")
                .map_or("supported", |(_, status)| status.as_str());
            Value::Array(vec![Value::Object(vec![
                ("name".into(), Value::String("core-test.writes".into())),
                ("status".into(), Value::String(status.into())),
                (
                    "evidence".into(),
                    // In a conformance launch this predicate is whatever the
                    // launch configuration says, defaulted or explicit, so the
                    // source names that rather than claiming a probe.
                    Value::Object(vec![(
                        "source".into(),
                        Value::String("launch-configuration".into()),
                    )]),
                ),
            ])])
        } else {
            // A production launch serves no `core-test`, and CBR has no other
            // predicate it can yet state with evidence. An empty snapshot is
            // the honest answer; a predicate asserted without evidence would
            // have to be `unknown`, and listing one only to say so adds nothing.
            Value::Array(vec![])
        };
        store.reconcile_capabilities(&config.provider_id, &predicates, &clock.now())?;
        let (current, oldest) = store.start_generation(
            config.dedupe_advance_on_start,
            config.dedupe_retain_generations,
        )?;
        Ok(Self {
            config,
            clock: std::sync::Arc::new(clock),
            store,
            shared_transport: false,
            authenticated: true,
            negotiated: None,
            dedupe_current: current,
            dedupe_oldest: oldest,
            caller_receive_limit: crate::frames::PRE_NEGOTIATION_LIMIT,
            subscriptions: Vec::new(),
            next_subscription: 0,
        })
    }

    /// A further session over a store a started process has already opened:
    /// its own connection to the store, the process's clock, and **none** of
    /// the start-time effects — no epoch change, no retention pass, no
    /// capability reconcile, no generation advance. A connection is not a
    /// restart. The session starts unauthenticated.
    pub fn connect(
        config: Config,
        clock: std::sync::Arc<crate::clock::Clock>,
        data_dir: &std::path::Path,
    ) -> Result<Self, crate::store::StoreError> {
        let store = Store::open(data_dir)?;
        let (current, oldest) = store.current_generation(config.dedupe_retain_generations)?;
        Ok(Self {
            config,
            clock,
            store,
            shared_transport: true,
            authenticated: false,
            negotiated: None,
            dedupe_current: current,
            dedupe_oldest: oldest,
            caller_receive_limit: crate::frames::PRE_NEGOTIATION_LIMIT,
            subscriptions: Vec::new(),
            next_subscription: 0,
        })
    }

    /// The clock every session of this process shares.
    pub fn shared_clock(&self) -> std::sync::Arc<crate::clock::Clock> {
        self.clock.clone()
    }

    /// `core.authenticate` (CORE section 18.2).
    ///
    /// The presented credential is digested and compared with **every** stored
    /// digest in constant time, without stopping at a match, so the time taken
    /// does not say how far a guess got. Unknown, malformed and revoked
    /// credentials are one error with empty details, and the credential is
    /// never echoed, logged or kept.
    fn authenticate(&mut self, payload: &Value) -> Result<Value, ProtocolError> {
        if !self.shared_transport || self.authenticated {
            return Err(ProtocolError::already_authenticated());
        }
        let presented = payload
            .get("credential")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let digest = cbr_encoding::sha256_hex(presented.as_bytes());
        let mut matched: Option<(String, bool)> = None;
        for credential in &self.config.credentials {
            if constant_time_eq(credential.digest.as_bytes(), digest.as_bytes())
                && matched.is_none()
            {
                matched = Some((credential.principal.clone(), credential.revoked));
            }
        }
        match matched {
            Some((principal, false)) if presented.starts_with("ccred1.") => {
                self.authenticated = true;
                self.config.principal = principal.clone();
                Ok(Value::Object(vec![(
                    "principal".into(),
                    Value::String(principal),
                )]))
            }
            _ => Err(ProtocolError::authentication_failed()),
        }
    }

    /// The receive limit for the next frame. The binding's 1 MiB default holds
    /// until negotiation completes; afterwards the provider's advertised value
    /// applies (STREAM section 1.5).
    pub fn frame_limit(&self) -> usize {
        match self.negotiated {
            None => crate::frames::PRE_NEGOTIATION_LIMIT,
            Some(_) => self.config.limits.max_frame_bytes as usize,
        }
    }

    fn dedupe_oldest(&self) -> i64 {
        self.dedupe_oldest
    }

    fn dedupe_window(&self) -> Value {
        Value::Object(vec![
            ("oldest_retained".into(), Value::Int(self.dedupe_oldest())),
            ("current".into(), Value::Int(self.dedupe_current)),
        ])
    }

    /// The profiles this process serves.
    ///
    /// `core-test/1` is conformance-only and MUST NOT be exposed outside a test
    /// configuration (CORE section 13). Filtering here rather than at each use
    /// means `core.describe`, negotiation and operation dispatch cannot
    /// disagree about whether it exists.
    fn served(
        &self,
    ) -> impl Iterator<
        Item = &'static (
            &'static str,
            i64,
            &'static [&'static str],
            &'static [&'static str],
        ),
    > {
        let conformance = self.config.mode == crate::config::Mode::Conformance;
        SERVED
            .iter()
            .filter(move |(name, _, _, _)| conformance || *name != "core-test")
    }

    /// Whether a feature was selected in this session.
    fn selected_feature(&self, feature: &str) -> bool {
        self.negotiated.as_ref().is_some_and(|selected| {
            selected
                .iter()
                .any(|(_, _, features)| features.iter().any(|f| f == feature))
        })
    }

    fn selected_major(&self, profile: &str) -> Option<i64> {
        self.negotiated
            .as_ref()?
            .iter()
            .find(|(name, _, _)| name == profile)
            .map(|(_, major, _)| *major)
    }

    fn known_operation(&self, operation: &str) -> bool {
        matches!(
            operation,
            "core.describe"
                | "core.negotiate"
                | "core.feature_dependencies"
                // The whole of core-test/1. Listing only the operations a
                // fixture happens to call would make step 1 report
                // `method_not_found` for a known operation, hiding the
                // method/operation mismatch that step 2 owns.
                | "core.authenticate"
                | "core.grant.issue"
                | "core.grant.revoke"
                | "core.grant.get"
                | "core.capabilities"
                | "core.effects.get"
                | "core.effects.abort_obligation"
                | "core.events.read"
                | "core.events.subscribe"
                | "core.events.unsubscribe"
                | "core-test.subject.put"
                | "core-test.subject.get"
                | "core-test.subject.applied_count"
                | "core-test.authority.claim"
        )
    }

    fn profile_of(operation: &str) -> &str {
        match operation.split_once('.') {
            Some((head, _)) => head,
            None => operation,
        }
    }

    // ---- describe and negotiate -------------------------------------------

    pub fn describe(&self) -> Value {
        let profiles = self
            .served()
            .map(|(name, major, features, depends)| {
                Value::Object(vec![
                    ("name".into(), Value::String((*name).into())),
                    ("majors".into(), Value::Array(vec![Value::Int(*major)])),
                    (
                        "features".into(),
                        Value::Array(
                            features
                                .iter()
                                .map(|f| Value::String((*f).into()))
                                .collect(),
                        ),
                    ),
                    (
                        "depends_on".into(),
                        Value::Array(depends.iter().map(|d| Value::String((*d).into())).collect()),
                    ),
                ])
            })
            .collect();
        let unsupported = UNSUPPORTED
            .iter()
            .map(|(name, reason)| {
                Value::Object(vec![
                    ("name".into(), Value::String((*name).into())),
                    ("reason".into(), Value::String((*reason).into())),
                ])
            })
            .collect();
        Value::Object(vec![
            (
                "provider".into(),
                Value::Object(vec![
                    ("name".into(), Value::String("cbr".into())),
                    (
                        "version".into(),
                        Value::String(env!("CARGO_PKG_VERSION").into()),
                    ),
                ]),
            ),
            ("profiles".into(), Value::Array(profiles)),
            ("unsupported_profiles".into(), Value::Array(unsupported)),
            ("limits".into(), self.config.limits.to_value()),
            ("dedupe_window".into(), self.dedupe_window()),
            // Unknown extensions are preserved rather than dropped: discarding
            // an extension a producer attached to stored content would lose
            // provenance CBR cannot reconstruct.
            (
                "unknown_extensions".into(),
                Value::String("preserve".into()),
            ),
        ])
    }

    /// In-session dependencies negotiation enforces (CORE section 4.3).
    pub fn feature_dependencies(&self) -> Value {
        let entries = self
            .served()
            .filter(|(name, _, _, _)| *name != "core")
            .map(|(name, major, _, depends)| {
                Value::Object(vec![
                    ("profile".into(), Value::String((*name).into())),
                    ("major".into(), Value::Int(*major)),
                    (
                        "trigger".into(),
                        Value::Object(vec![("kind".into(), Value::String("profile".into()))]),
                    ),
                    (
                        "requires".into(),
                        Value::Array(
                            depends
                                .iter()
                                .map(|d| {
                                    Value::Object(vec![
                                        ("profile".into(), Value::String((*d).into())),
                                        ("major".into(), Value::Int(1)),
                                        ("features".into(), Value::Array(vec![])),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        Value::Object(vec![("dependencies".into(), Value::Array(entries))])
    }

    fn negotiate(&mut self, payload: &Value) -> Result<Value, ProtocolError> {
        if self.negotiated.is_some() {
            return Err(ProtocolError::already_negotiated());
        }
        if let Some(Value::Int(limit)) = payload
            .get("receive_limits")
            .and_then(|l| l.get("max_frame_bytes"))
        {
            self.caller_receive_limit = *limit as usize;
        }
        let requested = match payload.get("profiles") {
            Some(Value::Array(items)) if !items.is_empty() => items.clone(),
            _ => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/profiles",
                    "not a non-empty array",
                ));
            }
        };

        let mut seen: Vec<String> = Vec::new();
        let mut selected: Vec<(String, i64, Vec<String>)> = Vec::new();
        let mut unselected: Vec<Value> = Vec::new();
        let mut unsatisfied: Vec<(Value, u8)> = Vec::new();

        for entry in &requested {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if seen.contains(&name) {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/profiles",
                    "duplicate profile",
                ));
            }
            seen.push(name.clone());

            // A `core` entry is always treated as required, whatever its flag.
            let required =
                name == "core" || matches!(entry.get("required"), Some(Value::Bool(true)));
            let majors: Vec<i64> = match entry.get("majors") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| match v {
                        Value::Int(n) => Some(*n),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };

            let served = self
                .served()
                .find(|(served_name, _, _, _)| *served_name == name);
            let declared_unsupported = UNSUPPORTED
                .iter()
                .find(|(unsupported_name, _)| *unsupported_name == name);

            let Some((_, major, features, _)) = served else {
                let reason = if declared_unsupported.is_some() {
                    "declared_unsupported"
                } else {
                    "unknown_profile"
                };
                let item = Value::Object(vec![
                    ("profile".into(), Value::String(name.clone())),
                    ("reason".into(), Value::String(reason.into())),
                ]);
                if required {
                    // unsupported_profile outranks the other refusal codes.
                    unsatisfied.push((item, 0));
                } else {
                    unselected.push(item);
                }
                continue;
            };

            if !majors.contains(major) {
                let item = Value::Object(vec![
                    ("profile".into(), Value::String(name.clone())),
                    ("reason".into(), Value::String("no_common_major".into())),
                ]);
                if required {
                    unsatisfied.push((item, 1));
                } else {
                    unselected.push(item);
                }
                continue;
            }

            let wanted_required: Vec<String> = match entry.get("required_features") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                _ => Vec::new(),
            };
            let wanted_optional: Vec<String> = match entry.get("optional_features") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                _ => Vec::new(),
            };

            let mut missing_required = false;
            for feature in &wanted_required {
                if !features.contains(&feature.as_str()) {
                    let item = Value::Object(vec![
                        ("profile".into(), Value::String(name.clone())),
                        ("feature".into(), Value::String(feature.clone())),
                        ("reason".into(), Value::String("unknown_feature".into())),
                    ]);
                    if required {
                        unsatisfied.push((item, 2));
                    } else {
                        unselected.push(item);
                    }
                    missing_required = true;
                }
            }
            if missing_required {
                // An optional profile whose required feature is missing is not
                // selected; a required one has already refused the negotiation.
                continue;
            }

            let mut chosen: Vec<String> = Vec::new();
            for feature in &wanted_optional {
                if features.contains(&feature.as_str()) {
                    chosen.push(feature.clone());
                } else {
                    unselected.push(Value::Object(vec![
                        ("profile".into(), Value::String(name.clone())),
                        ("feature".into(), Value::String(feature.clone())),
                        ("reason".into(), Value::String("unknown_feature".into())),
                    ]));
                }
            }
            chosen.extend(wanted_required.iter().cloned());
            selected.push((name, *major, chosen));
        }

        if !unsatisfied.is_empty() {
            let rank = unsatisfied.iter().map(|(_, rank)| *rank).min().unwrap_or(0);
            let items: Vec<Value> = unsatisfied.into_iter().map(|(item, _)| item).collect();
            let unsatisfied = Value::Array(items);
            return Err(match rank {
                0 => ProtocolError::unsupported_profile(unsatisfied),
                1 => ProtocolError::unsupported_version(unsatisfied),
                _ => ProtocolError::unsupported_required_feature_negotiation(unsatisfied),
            });
        }

        // `core` is implicit and cannot be deselected.
        if !selected.iter().any(|(name, _, _)| name == "core") {
            selected.insert(0, ("core".into(), 1, Vec::new()));
        }

        // Profile-triggered dependencies, applied after selection.
        let names: Vec<String> = selected.iter().map(|(name, _, _)| name.clone()).collect();
        for (name, _, _, depends) in self.served() {
            if !names.contains(&(*name).to_string()) {
                continue;
            }
            for dependency in *depends {
                if !names
                    .iter()
                    .any(|selected_name| selected_name == dependency)
                {
                    return Err(ProtocolError::unsupported_profile(Value::Array(vec![
                        Value::Object(vec![
                            ("profile".into(), Value::String((*name).into())),
                            (
                                "reason".into(),
                                Value::String("dependency_not_selected".into()),
                            ),
                        ]),
                    ])));
                }
            }
        }

        let result = Value::Object(vec![
            (
                "selected".into(),
                Value::Array(
                    selected
                        .iter()
                        .map(|(name, major, features)| {
                            Value::Object(vec![
                                ("name".into(), Value::String(name.clone())),
                                ("major".into(), Value::Int(*major)),
                                (
                                    "features".into(),
                                    Value::Array(
                                        features.iter().map(|f| Value::String(f.clone())).collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("unselected".into(), Value::Array(unselected)),
            ("limits".into(), {
                // CORE section 16.5: negotiating backpressure adds its two
                // bounds to `limits`; without the feature they are absent.
                let mut limits = self.config.limits.to_value();
                let backpressure = selected.iter().any(|(name, _, features)| {
                    name == "core" && features.iter().any(|f| f == "core.events.backpressure")
                });
                if backpressure && let Value::Object(members) = &mut limits {
                    members.push((
                        "max_pending_notification_bytes".into(),
                        Value::Int(self.config.events_max_pending_notification_bytes),
                    ));
                    members.push((
                        "backpressure_notice_ms".into(),
                        Value::Int(self.config.events_backpressure_notice_ms),
                    ));
                }
                limits
            }),
            ("dedupe_window".into(), self.dedupe_window()),
        ]);
        self.negotiated = Some(selected);
        Ok(result)
    }

    // ---- dispatch ---------------------------------------------------------

    /// Handle one request. `method` is the transport method, which the envelope
    /// must agree with.
    pub fn handle(&mut self, method: &str, params: &Value) -> Result<Value, ProtocolError> {
        let _processing = processing();
        // Step 1: operation known, session negotiated, profile selected. These
        // are decided from the transport method, before the envelope is read.
        // Time-driven effect state first, so an obligation whose deadline has
        // passed is overdue before anything in this request reads it. A
        // failure here must not fail an unrelated request; it is recorded.
        if let Err(error) = self.tick_effects() {
            eprintln!(
                "cbr-provider: marking overdue obligations failed: {}",
                error.code
            );
        }
        if !self.known_operation(method) {
            return Err(ProtocolError::method_not_found(method));
        }
        // CORE section 18.2: on a shared transport, before authentication only
        // `core.describe` and `core.authenticate` are allowed, and anything else
        // is refused here — before the negotiation check, so an unauthenticated
        // caller cannot learn what negotiating would have said.
        if !self.authenticated && !matches!(method, "core.describe" | "core.authenticate") {
            return Err(ProtocolError::authentication_required());
        }
        // A conformance-only operation does not exist in a production
        // configuration, so it is unknown rather than merely unauthorised.
        if method.starts_with("core-test.") && self.config.mode != crate::config::Mode::Conformance
        {
            return Err(ProtocolError::method_not_found(method));
        }
        let pre_negotiation = PRE_NEGOTIATION.contains(&method);
        if self.negotiated.is_none() && !pre_negotiation {
            return Err(ProtocolError::negotiation_required());
        }
        if !pre_negotiation {
            let profile = Self::profile_of(method);
            if self.selected_major(profile).is_none() {
                return Err(ProtocolError::profile_not_negotiated(profile));
            }
        }

        // Step 2: limits, then the envelope's shape, then step 3's
        // `requires`. All three are decided before step 6, so a malformed
        // envelope is `invalid_envelope` whoever sent it.
        envelope::check_limits(params, &self.config.limits)?;

        // The `grant` envelope field exists only for a session that negotiated
        // `core.grants` (CORE section 15.5). Accepting it from a session that
        // did not would let a caller believe it was acting under a grant while
        // the provider ignored the field entirely.
        if params.get("grant").is_some() && !self.selected_feature("core.grants") {
            return Err(ProtocolError::invalid_envelope(
                "/grant",
                "core.grants was not negotiated",
            ));
        }

        if COMMANDS.contains(&method) {
            let command = envelope::parse_command(params)?;
            self.check_method_matches(method, &command.operation)?;
            self.check_requires(&command.requires)?;
            // Step 6 for a command runs inside `admit_command`, **after**
            // deduplication: CORE section 15.5 has a replay skip step 6, so a
            // holder replaying its own bound command still receives its stored
            // result once the grant has been revoked.
            return match method {
                "core-test.subject.put" => self.subject_put(params, command),
                "core-test.authority.claim" => self.authority_claim(params, command),
                "core.grant.issue" => self.grant_issue(params, command),
                "core.grant.revoke" => self.grant_revoke(params, command),
                "core.effects.abort_obligation" => self.effects_abort(params, command),
                _ => Err(ProtocolError::method_not_found(method)),
            };
        }

        let query = envelope::parse_query(params)?;
        self.check_method_matches(method, &query.operation)?;
        // `already_negotiated` is decided after steps 1 to 3.
        self.check_requires(&query.requires)?;

        // Step 6 for a query, which has no command identity and so nothing to
        // deduplicate first. It runs before the operation looks anything up,
        // because CORE section 15.5 requires an unauthorised principal to get
        // the same `permission_denied` whether or not the subject exists.
        // Doing it inside each operation would leak existence through
        // `not_found`, which is how `core.grants.existence-not-leaked` and
        // `core.grants.authorization-without-grants-feature` catch it.
        let in_force = if UNPROTECTED.contains(&method) || is_grant_operation(method) {
            None
        } else if method == "core.effects.get" {
            self.authorize_effect_read(&query)?
        } else {
            let needs = self.query_needs(&query)?;
            self.authorize(query.grant.as_deref(), &needs)?
        };

        match method {
            "core.describe" => Ok(self.describe()),
            // CORE section 18: on stdio the launching process assigns the
            // principal, so every stdio session is already authenticated.
            "core.authenticate" => self.authenticate(&query.payload),
            "core.feature_dependencies" => Ok(self.feature_dependencies()),
            "core.negotiate" => self.negotiate(&query.payload),
            "core-test.subject.applied_count" => self.applied_count(&query),
            "core.events.subscribe" => self.events_subscribe(&query),
            "core.events.unsubscribe" => self.events_unsubscribe(&query),
            "core.events.read" => self.events_read(&query, in_force.as_ref()),
            "core-test.subject.get" => self.subject_get(&query),
            "core.grant.get" => self.grant_get(&query),
            "core.capabilities" => self.capabilities(),
            "core.effects.get" => self.effects_get(&query),
            _ => Err(ProtocolError::method_not_found(method)),
        }
    }

    // ---- grants (CORE section 15) -----------------------------------------

    /// The grant record stored under this id.
    ///
    /// A grant is a subject like any other, so its record lives in the subject
    /// row and inherits revisions, the command transaction and durability for
    /// free. A record that will not parse is this provider's own corruption,
    /// never a caller's mistake, so it is `internal_error` and not
    /// `invalid_envelope`.
    fn grant(&self, id: &str) -> Result<Option<Grant>, ProtocolError> {
        let Some(state) = self.store.subject(&Grant::key(id))? else {
            return Ok(None);
        };
        let value = cbr_encoding::parse(state.value.as_bytes())
            .map_err(|_| ProtocolError::new_internal_error())?;
        match Grant::from_value(&value) {
            Some(grant) => Ok(Some(grant)),
            None => Err(ProtocolError::new_internal_error()),
        }
    }

    /// Every grant delegated from `root`, directly or transitively, that is
    /// still active, in a stable breadth-first order.
    ///
    /// The walk passes **through** revoked grants rather than stopping at
    /// them. Today that changes nothing, because a revoked grant cannot gain a
    /// child and its children were revoked with it; but stopping would make
    /// the cascade depend on that invariant holding forever, and a cascade that
    /// silently leaves a grant authorizing is the failure this exists to rule
    /// out.
    fn active_descendants(&self, root: &str) -> Result<Vec<Grant>, ProtocolError> {
        let mut all = Vec::new();
        for (_, value) in self.store.subjects_of_kind(grants::KIND)? {
            let parsed = cbr_encoding::parse(value.as_bytes())
                .ok()
                .and_then(|value| Grant::from_value(&value))
                .ok_or_else(ProtocolError::new_internal_error)?;
            all.push(parsed);
        }
        let mut frontier = vec![root.to_string()];
        let mut seen = std::collections::HashSet::from([root.to_string()]);
        let mut active = Vec::new();
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for grant in &all {
                if let Some(parent) = &grant.parent
                    && frontier.contains(parent)
                    && seen.insert(grant.id.clone())
                {
                    next.push(grant.id.clone());
                    if !grant.revoked {
                        active.push(grant.clone());
                    }
                }
            }
            frontier = next;
        }
        Ok(active)
    }

    /// The current epoch of every authority scope this provider tracks.
    fn authority_context(&self) -> Result<grants::Context, ProtocolError> {
        let mut epochs = Vec::new();
        for scope in grants::KNOWN_SCOPES {
            epochs.push((scope.to_string(), self.store.epoch(scope)?));
        }
        Ok(grants::Context {
            now: self.clock.now(),
            epochs,
        })
    }

    /// Step 6 for an operation a profile protects (CORE section 15.5).
    /// Returns the grant in force, or `None` when the caller acted as an
    /// authority principal without naming one.
    fn authorize(
        &self,
        named: Option<&str>,
        needs: &[grants::Need],
    ) -> Result<Option<Grant>, ProtocolError> {
        let Some(id) = named else {
            return if self.config.is_authority(&self.config.principal) {
                Ok(None)
            } else {
                Err(grants::Denial::GrantRequired.into())
            };
        };
        // The holder check is part of finding the grant, not a property read
        // from one that was found: CORE section 15.5 makes a grant held by
        // someone else indistinguishable from one that never existed, so a
        // caller cannot probe for grant ids or learn that one was revoked.
        let grant = self
            .grant(id)?
            .filter(|grant| grant.holder == self.config.principal)
            .ok_or(grants::Denial::GrantNotFound)?;
        grant.usable(&self.authority_context()?)?;
        grant.permits(needs)?;
        Ok(Some(grant))
    }

    /// Whether a subscription may still deliver, and the grant in force if it
    /// named one. `None` means it may not, and the subscription ends.
    ///
    /// This mirrors `authorize`, but takes the principal rather than reading
    /// the session's, because it is asked on behalf of a subscription created
    /// earlier rather than of the frame being processed.
    fn events_authorization(&self, principal: &str, named: Option<&str>) -> Option<Option<Grant>> {
        let Some(id) = named else {
            return self.config.is_authority(principal).then_some(None);
        };
        let grant = self
            .grant(id)
            .ok()
            .flatten()
            .filter(|grant| grant.holder == principal)?;
        grant.usable(&self.authority_context().ok()?).ok()?;
        grant.permits(&[("core.events.read", None)]).ok()?;
        Some(Some(grant))
    }

    /// Whether the caller may read a subject directly, which is what decides
    /// disclosure in an error: a current revision in `precondition_failed` and
    /// a current epoch in `stale_authority_epoch` are shown only to a principal
    /// that could have read the subject anyway (CORE sections 15.5 and 16.6).
    fn may_read(&self, in_force: Option<&Grant>, key: &SubjectKey) -> bool {
        let Some(grant) = in_force else {
            // Only an authority principal reaches an operation without a grant
            // in force, and an authority sees every subject.
            return true;
        };
        match key.kind.as_str() {
            grants::KIND => self.grant(&key.id).ok().flatten().is_some_and(|other| {
                other.holder == self.config.principal || other.issuer == self.config.principal
            }),
            kind if kind.starts_with("core-test.") => grant.may_read(key, "core-test.read"),
            // No profile defines a read right for capabilities, so resources
            // alone decide (CORE section 16.6).
            "core.capabilities" => grant.covers(key),
            // "An effect subject is visible in events exactly when its target
            // is" (CORE section 19.4).
            effects::KIND => self
                .effect(&key.id)
                .ok()
                .flatten()
                .and_then(|(_, record)| effects::target(&record))
                .is_some_and(|target| self.may_read(Some(grant), &target)),
            // A kind no profile defines is never readable under a grant.
            _ => false,
        }
    }

    /// The rights a query needs, and the subjects it needs them over.
    fn query_needs(&self, query: &Query) -> Result<Vec<grants::Need>, ProtocolError> {
        Ok(match query.operation.as_str() {
            "core-test.subject.get" | "core-test.subject.applied_count" => {
                vec![("core-test.read", Some(self.subject_key(&query.payload)?))]
            }
            // CORE section 16.6: reading or subscribing needs the right
            // itself. Which events it then shows is decided per subject as
            // they are read, so there is no subject to name here.
            "core.events.read" | "core.events.subscribe" => vec![("core.events.read", None)],
            _ => Vec::new(),
        })
    }

    /// The rights a command needs, and the subjects it needs them over
    /// (CORE section 13).
    fn command_needs(command: &Command) -> Vec<grants::Need> {
        let primary = key_of(&command.subject);
        match command.operation.as_str() {
            "core-test.subject.put" => {
                let mut needs: Vec<grants::Need> = vec![("core-test.write", Some(primary.clone()))];
                // Read authority on every **other** subject the preconditions
                // name. The primary subject's own precondition rides on the
                // write right, which is why a write-only grant can still
                // create a subject — and why `core.grants.precondition-
                // current-needs-read` can reach a precondition failure at all.
                for (subject, _) in &command.preconditions {
                    let key = key_of(subject);
                    if key != primary {
                        needs.push(("core-test.read", Some(key)));
                    }
                }
                needs
            }
            "core-test.authority.claim" => vec![("core-test.claim", Some(primary))],
            _ => Vec::new(),
        }
    }

    /// Step 6 for `core.grant.issue` (CORE section 15.3), in the order that
    /// section fixes: the three validity checks first — audience, expiry, then
    /// binding scope — and only then the issuing rules. So a non-authority
    /// issuing a malformed grant learns that it was malformed, not that it
    /// lacked authority, and the two checks cannot be swapped without
    /// `core.grants.issue-validity-check-order` failing at a named step.
    fn authorize_issue(&self, grant: &Grant) -> Result<Option<Grant>, ProtocolError> {
        // One instant for the whole decision, so the expiry check below and a
        // parent's own expiry are judged against the same time.
        let context = self.authority_context()?;
        if grant.audience != self.config.provider_id {
            return Err(ProtocolError::invalid_envelope(
                "/payload/audience",
                "not this provider",
            ));
        }
        // "Not after the provider's current time": an expiry equal to now is
        // already expired, so issuing it would mint a grant that never worked.
        if let Some(expiry) = &grant.expires_at
            && expiry.as_str() <= context.now.as_str()
        {
            return Err(ProtocolError::invalid_envelope(
                "/payload/expires_at",
                "not after the provider's current time",
            ));
        }
        if let Some(binding) = &grant.authority_binding
            && !grants::KNOWN_SCOPES.contains(&binding.scope.as_str())
        {
            return Err(ProtocolError::invalid_envelope(
                "/payload/authority_binding/scope",
                "not an authority scope this provider tracks",
            ));
        }

        let Some(parent_id) = &grant.parent else {
            // A grant without a parent may be issued only by an authority.
            if !self.config.is_authority(&self.config.principal) {
                return Err(grants::Denial::NotAuthority.into());
            }
            return Ok(None);
        };
        // A delegated grant may be issued only by the parent's holder. The
        // parent is found the way `authorize` finds a grant, so a parent held
        // by someone else is `grant_not_found`, not a hint that it exists.
        let parent = self
            .grant(parent_id)?
            .filter(|parent| parent.holder == self.config.principal)
            .ok_or(grants::Denial::GrantNotFound)?;
        // Then usable — active, unexpired, bound to a current epoch — before
        // its delegation terms are read at all, so a revoked parent says
        // `revoked` rather than `delegation_exceeded`.
        parent.usable(&context)?;
        parent.may_delegate_to(grant)?;
        Ok(None)
    }

    /// Step 6 for `core.grant.revoke` (CORE section 15.3): the issuer or an
    /// authority principal, decided **before** whether the grant is already
    /// revoked, so a stranger learns nothing about it either way.
    fn authorize_revoke(&self, command: &Command) -> Result<Option<Grant>, ProtocolError> {
        let existing = self.grant(&key_of(&command.subject).id)?;
        let permitted = self.config.is_authority(&self.config.principal)
            || existing
                .as_ref()
                .is_some_and(|grant| grant.issuer == self.config.principal);
        if !permitted {
            return Err(grants::Denial::NotAuthority.into());
        }
        if existing.is_some_and(|grant| grant.revoked) {
            return Err(grants::Denial::Revoked.into());
        }
        Ok(None)
    }

    /// `core.grant.get`: visible to its holder, its issuer and authority
    /// principals; anyone else gets `not_found` (CORE section 15.3).
    fn grant_get(&self, query: &Query) -> Result<Value, ProtocolError> {
        let Some(id) = query.payload.get("grant").and_then(Value::as_str) else {
            return Err(ProtocolError::invalid_envelope(
                "/payload/grant",
                "not a grant id",
            ));
        };
        let Some(grant) = self.grant(id)? else {
            return Err(ProtocolError::not_found());
        };
        let principal = &self.config.principal;
        if grant.holder != *principal
            && grant.issuer != *principal
            && !self.config.is_authority(principal)
        {
            return Err(ProtocolError::not_found());
        }
        Ok(Value::Object(vec![
            (
                "revision".into(),
                Value::Int(self.store.revision(&Grant::key(id))?),
            ),
            ("grant".into(), grant.to_value()),
        ]))
    }

    fn grant_issue(&mut self, params: &Value, command: Command) -> Result<Value, ProtocolError> {
        let id = self.grant_subject(&command, 0)?;
        let issuer = self.config.principal.clone();
        // Step 2: the payload's shape. Nothing here reads the clock, the store
        // or the principal; the checks that do run at step 6, so an
        // already-bound issue still replays past them.
        let grant = grants::parse_issue(&id, &issuer, &command.payload)?;
        if let Some(stored) = self.admit_command(
            params,
            &command,
            "core-test",
            false,
            Step6::Issue(Box::new(grant.clone())),
        )? {
            return Ok(stored);
        }

        let record = grant.to_value();
        let value = String::from_utf8(cbr_encoding::to_canonical(&record))
            .expect("canonical form is UTF-8");
        let key = Grant::key(&id);
        let principal = self.config.principal.clone();
        let digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let event_record = record.clone();
        let recorded_at = self.clock.now();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also: Vec::new(),
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                event: Some(crate::store::NewEvent {
                    event_type: "core.grant.issued".into(),
                    caused_by: command.caused_by.clone(),
                    payload: Box::new(move |_| Value::Object(vec![("grant".into(), event_record)])),
                }),
            },
            |revision, operation_ref| {
                accepted(
                    &command_id,
                    &digest,
                    operation_ref,
                    &subject,
                    revision,
                    Value::Object(vec![("grant".into(), record.clone())]),
                )
            },
        )?;
        Ok(result)
    }

    fn grant_revoke(&mut self, params: &Value, command: Command) -> Result<Value, ProtocolError> {
        let id = self.grant_subject(&command, 1)?;
        if let Some(stored) =
            self.admit_command(params, &command, "core-test", false, Step6::Revoke)?
        {
            return Ok(stored);
        }

        // Step 7's precondition, on a revision of at least 1, has already
        // refused a grant that does not exist.
        let Some(mut grant) = self.grant(&id)? else {
            return Err(ProtocolError::new_internal_error());
        };
        grant.revoked = true;
        let value = String::from_utf8(cbr_encoding::to_canonical(&grant.to_value()))
            .expect("canonical form is UTF-8");
        let key = Grant::key(&id);
        let principal = self.config.principal.clone();
        let digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();

        // Revocation cascades to every grant delegated from this one, directly
        // or transitively, in the same transaction: a crash cannot leave a
        // parent revoked and a child still authorizing. Descendants already
        // revoked are neither listed nor changed (CORE section 15.3).
        let descendants = self.active_descendants(&id)?;
        let mut revoked = vec![Value::String(id.clone())];
        let mut also = Vec::new();
        for mut child in descendants {
            revoked.push(Value::String(child.id.clone()));
            child.revoked = true;
            also.push(crate::store::Change {
                key: Grant::key(&child.id),
                value: String::from_utf8(cbr_encoding::to_canonical(&child.to_value()))
                    .expect("canonical form is UTF-8"),
                event: revoked_event(&command.caused_by),
            });
        }
        let revoked = Value::Array(revoked);
        let recorded_at = self.clock.now();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also,
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                event: Some(revoked_event(&command.caused_by)),
            },
            |revision, operation_ref| {
                accepted(
                    &command_id,
                    &digest,
                    operation_ref,
                    &subject,
                    revision,
                    Value::Object(vec![("revoked".into(), revoked.clone())]),
                )
            },
        )?;
        Ok(result)
    }

    // ---- effects (CORE section 19) ------------------------------------------

    /// The effect record stored under this id, with its revision.
    fn effect(&self, id: &str) -> Result<Option<(i64, Value)>, ProtocolError> {
        let key = SubjectKey {
            kind: effects::KIND.into(),
            id: id.into(),
        };
        let Some(state) = self.store.subject(&key)? else {
            return Ok(None);
        };
        let record = cbr_encoding::parse(state.value.as_bytes())
            .map_err(|_| ProtocolError::new_internal_error())?;
        Ok(Some((state.revision, record)))
    }

    /// Step 6 for `core.effects.get`.
    ///
    /// Reading an effect needs read authority on its **target**, and CORE-12
    /// requires the same refusal for an existing effect and for one that does
    /// not exist. The reference provider resolves the reason from its single
    /// target family's read right. CBR's effects will not share one family,
    /// and a missing effect has no target whose right could be evaluated, so
    /// under a grant CBR reports the one reason true in both cases —
    /// `out_of_scope` — whenever the target is not readable, including when
    /// there is no target. Distinguishing a missing right would reveal that
    /// the effect exists.
    fn authorize_effect_read(&self, query: &Query) -> Result<Option<Grant>, ProtocolError> {
        let id = effect_id_of(&query.payload)?;
        let Some(named) = query.grant.as_deref() else {
            return if self.config.is_authority(&self.config.principal) {
                Ok(None)
            } else {
                Err(grants::Denial::GrantRequired.into())
            };
        };
        let grant = self
            .grant(named)?
            .filter(|grant| grant.holder == self.config.principal)
            .ok_or(grants::Denial::GrantNotFound)?;
        grant.usable(&self.authority_context()?)?;
        let readable = self
            .effect(&id)?
            .and_then(|(_, record)| effects::target(&record))
            .is_some_and(|target| self.may_read(Some(&grant), &target));
        if !readable {
            return Err(grants::Denial::OutOfScope.into());
        }
        Ok(Some(grant))
    }

    /// Step 6 for `core.effects.abort_obligation`: right
    /// `core.effects.abort_obligation` on the effect's target. The right name
    /// is fixed, so the ordinary order holds — `right_missing` before
    /// `out_of_scope` — and a missing right reveals nothing about existence.
    /// An effect that does not exist is `out_of_scope`, like one whose target
    /// the grant does not cover.
    fn authorize_abort(&self, command: &Command) -> Result<Option<Grant>, ProtocolError> {
        let Some(named) = command.grant.as_deref() else {
            return if self.config.is_authority(&self.config.principal) {
                Ok(None)
            } else {
                Err(grants::Denial::GrantRequired.into())
            };
        };
        let grant = self
            .grant(named)?
            .filter(|grant| grant.holder == self.config.principal)
            .ok_or(grants::Denial::GrantNotFound)?;
        grant.usable(&self.authority_context()?)?;
        if !grant
            .rights
            .iter()
            .any(|right| right == effects::ABORT_RIGHT)
        {
            return Err(grants::Denial::RightMissing.into());
        }
        let covered = self
            .effect(&key_of(&command.subject).id)?
            .and_then(|(_, record)| effects::target(&record))
            .is_some_and(|target| grant.covers(&target));
        if !covered {
            return Err(grants::Denial::OutOfScope.into());
        }
        Ok(Some(grant))
    }

    /// `core.effects.get` (CORE section 19.2). An effect this provider recorded
    /// is never reported `not_found`: its record is retained, and an outcome
    /// that could not be established is `unknown` with the wait still open.
    fn effects_get(&self, query: &Query) -> Result<Value, ProtocolError> {
        if !self.selected_feature("core.effects") {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("core.effects".into())]),
            ));
        }
        let id = effect_id_of(&query.payload)?;
        match self.effect(&id)? {
            Some((revision, record)) => Ok(effects::get_result(&record, revision)),
            None => Err(ProtocolError::not_found()),
        }
    }

    /// `core.effects.abort_obligation` (CORE section 19.4): closes a wait and
    /// never changes the effect's status.
    fn effects_abort(&mut self, params: &Value, command: Command) -> Result<Value, ProtocolError> {
        if !self.selected_feature("core.effects") {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("core.effects".into())]),
            ));
        }
        let key = key_of(&command.subject);
        if key.kind != effects::KIND
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "exactly one precondition on the effect subject is required",
            ));
        }
        let obligation = match command.payload.get("obligation").and_then(Value::as_str) {
            Some(id) if crate::envelope::is_identifier(id) => id.to_string(),
            _ => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/obligation",
                    "not an identifier",
                ));
            }
        };
        if let Some(stored) =
            self.admit_command(params, &command, "core-test", false, Step6::Abort)?
        {
            return Ok(stored);
        }
        let Some((_, mut record)) = self.effect(&key.id)? else {
            return Err(ProtocolError::not_found());
        };
        // Only a waiting obligation can be aborted; anything else is not found.
        if !effects::obligation_waiting(&record, &obligation) {
            return Err(ProtocolError::not_found());
        }
        effects::abort(&mut record, &obligation);
        let status = effects::status(&record);
        let target = record
            .get("descriptor")
            .and_then(|d| d.get("target"))
            .cloned()
            .unwrap_or(Value::Null);
        let value = String::from_utf8(cbr_encoding::to_canonical(&record))
            .expect("canonical form is UTF-8");
        let principal = self.config.principal.clone();
        let digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let effect_id = key.id.clone();
        let payload = Value::Object(vec![
            ("effect".into(), Value::String(effect_id.clone())),
            ("obligation".into(), Value::String(obligation.clone())),
            ("target".into(), target),
        ]);
        let recorded_at = self.clock.now();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also: Vec::new(),
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                event: Some(crate::store::NewEvent {
                    event_type: "core.effect.obligation.aborted".into(),
                    caused_by: command.caused_by.clone(),
                    payload: Box::new(move |_| payload),
                }),
            },
            |revision, operation_ref| {
                accepted(
                    &command_id,
                    &digest,
                    operation_ref,
                    &subject,
                    revision,
                    Value::Object(vec![
                        ("effect".into(), Value::String(effect_id.clone())),
                        (
                            "obligation".into(),
                            Value::Object(vec![
                                ("id".into(), Value::String(obligation.clone())),
                                ("state".into(), Value::String("aborted".into())),
                            ]),
                        ),
                        ("status".into(), Value::String(status.clone())),
                    ]),
                )
            },
        )?;
        Ok(result)
    }

    /// Mark obligations whose deadline has passed as `overdue`, each with a
    /// provider-origin `core.effect.obligation.overdue` event on the effect.
    ///
    /// Run at the start of every request. On the stdio binding nothing else can
    /// happen while a session is idle, so this is when the passing becomes
    /// observable, as for grant expiry (CORE section 16.5). CORE section 19.4
    /// does not name the overdue event's subject or payload; CBR uses the
    /// effect subject and the same `{ effect, obligation, target }` payload as
    /// the aborted event, so a consumer reads both the same way.
    /// Idle maintenance for a session with no request in hand: mark passed
    /// obligations overdue under the processing lock. Subscription re-checks
    /// happen in `drain_subscriptions`, which the session calls next.
    pub fn tick(&mut self) {
        let _processing = processing();
        if let Err(error) = self.tick_effects() {
            eprintln!(
                "cbr-provider: marking overdue obligations failed: {}",
                error.code
            );
        }
    }

    fn tick_effects(&mut self) -> Result<(), ProtocolError> {
        let now = self.clock.now();
        for (id, value) in self.store.subjects_of_kind(effects::KIND)? {
            let mut record = cbr_encoding::parse(value.as_bytes())
                .map_err(|_| ProtocolError::new_internal_error())?;
            let marked = effects::mark_overdue(&mut record, &now);
            if marked.is_empty() {
                continue;
            }
            let target = record
                .get("descriptor")
                .and_then(|d| d.get("target"))
                .cloned()
                .unwrap_or(Value::Null);
            let events = marked
                .into_iter()
                .map(|obligation| {
                    let payload = Value::Object(vec![
                        ("effect".into(), Value::String(id.clone())),
                        ("obligation".into(), Value::String(obligation)),
                        ("target".into(), target.clone()),
                    ]);
                    crate::store::NewEvent {
                        event_type: "core.effect.obligation.overdue".into(),
                        caused_by: Vec::new(),
                        payload: Box::new(move |_| payload),
                    }
                })
                .collect();
            let value = String::from_utf8(cbr_encoding::to_canonical(&record))
                .expect("canonical form is UTF-8");
            let key = SubjectKey {
                kind: effects::KIND.into(),
                id,
            };
            self.store
                .commit_provider_change(&key, &value, events, &now)?;
        }
        Ok(())
    }

    /// Record an observation of an effect's outcome. The producer API: nothing
    /// in M1 produces effects, so only tests call it until M4.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn observe_effect(
        &mut self,
        id: &str,
        status: &str,
        class: &str,
        source: &str,
    ) -> Result<(), ProtocolError> {
        let Some((_, mut record)) = self.effect(id)? else {
            return Err(ProtocolError::not_found());
        };
        let now = self.clock.now();
        effects::observe(&mut record, status, class, source, &now);
        let value = String::from_utf8(cbr_encoding::to_canonical(&record))
            .expect("canonical form is UTF-8");
        let key = SubjectKey {
            kind: effects::KIND.into(),
            id: id.into(),
        };
        self.store
            .commit_provider_change(&key, &value, Vec::new(), &now)?;
        Ok(())
    }

    /// `core.capabilities` (CORE section 17.1): the current snapshot.
    fn capabilities(&self) -> Result<Value, ProtocolError> {
        if !self.selected_feature("core.capabilities") {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("core.capabilities".into())]),
            ));
        }
        let (revision, predicates) = self
            .store
            .capabilities()?
            .ok_or_else(ProtocolError::new_internal_error)?;
        Ok(Value::Object(vec![
            ("revision".into(), Value::Int(revision)),
            ("predicates".into(), predicates),
        ]))
    }

    /// A predicate's current status. A predicate the snapshot does not name is
    /// `unknown`, never `supported`: CORE section 17.1 allows `supported` only
    /// with evidence.
    fn capability_status(&self, name: &str) -> Result<String, ProtocolError> {
        let predicates = self
            .store
            .capabilities()?
            .map(|(_, predicates)| predicates)
            .unwrap_or(Value::Array(vec![]));
        Ok(predicates
            .as_array()
            .unwrap_or_default()
            .iter()
            .find(|p| p.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|p| p.get("status").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string())
    }

    /// The grant id a `core.grant.*` command names, with the single
    /// precondition CORE section 15.3 requires: revision 0 for an issue and at
    /// least 1 for a revoke. Anything else is `invalid_envelope` at step 2,
    /// before the command is bound.
    fn grant_subject(&self, command: &Command, expected: i64) -> Result<String, ProtocolError> {
        let key = key_of(&command.subject);
        if key.kind != grants::KIND {
            return Err(ProtocolError::invalid_envelope(
                "/subject/kind",
                "not core.grant",
            ));
        }
        let matching = command.preconditions.len() == 1
            && key_of(&command.preconditions[0].0) == key
            && if expected == 0 {
                command.preconditions[0].1 == 0
            } else {
                command.preconditions[0].1 >= 1
            };
        if !matching {
            return Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "exactly one precondition on the grant subject is required",
            ));
        }
        Ok(key.id)
    }

    fn check_method_matches(&self, method: &str, operation: &str) -> Result<(), ProtocolError> {
        if method == operation {
            Ok(())
        } else {
            Err(ProtocolError::invalid_envelope(
                "/operation",
                "operation does not equal the transport method",
            ))
        }
    }

    /// Step 3: every `requires` entry must be negotiated and understood.
    fn check_requires(&self, requires: &[String]) -> Result<(), ProtocolError> {
        let mut unknown = Vec::new();
        for entry in requires {
            let understood = if entry.contains('/') {
                // An extension key. This build understands none, so naming one
                // in `requires` is a refusal rather than a silent acceptance.
                false
            } else {
                self.negotiated.as_ref().is_some_and(|selected| {
                    selected
                        .iter()
                        .any(|(_, _, features)| features.iter().any(|f| f == entry))
                })
            };
            if !understood {
                unknown.push(Value::String(entry.clone()));
            }
        }
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(unknown),
            ))
        }
    }

    // ---- core-test --------------------------------------------------------

    fn applied_count(&self, query: &Query) -> Result<Value, ProtocolError> {
        let subject = query
            .payload
            .get("subject")
            .ok_or_else(|| ProtocolError::invalid_envelope("/payload/subject", "absent"))?;
        let key = SubjectKey {
            kind: subject
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            id: subject
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        };
        if key.kind != "core-test.subject" {
            return Err(ProtocolError::invalid_envelope(
                "/payload/subject/kind",
                "not core-test.subject",
            ));
        }
        Ok(Value::Object(vec![
            ("subject".into(), subject.clone()),
            (
                "applied_count".into(),
                Value::Int(self.store.applied_count(&key)?),
            ),
        ]))
    }

    /// A cursor: opaque to callers, but it carries the stream it came from, so
    /// a well-formed cursor minted by another store is refused rather than
    /// silently read against this one.
    fn encode_cursor(&self, position: crate::store::Position) -> Result<String, ProtocolError> {
        let stream = self.store.stream_id()?;
        Ok(format!(
            "c1.{stream}.{}.{}",
            position.epoch, position.sequence
        ))
    }

    fn decode_cursor(&self, text: &str) -> Result<crate::store::Position, ProtocolError> {
        let invalid = |reason: &str| ProtocolError::new_invalid_cursor(reason);
        let rest = text
            .strip_prefix("c1.")
            .ok_or_else(|| invalid("malformed"))?;
        let mut parts = rest.rsplitn(3, '.');
        let sequence = parts.next().ok_or_else(|| invalid("malformed"))?;
        let epoch = parts.next().ok_or_else(|| invalid("malformed"))?;
        let stream = parts.next().ok_or_else(|| invalid("malformed"))?;
        if stream != self.store.stream_id()? {
            return Err(invalid("from another stream"));
        }
        let epoch: i64 = epoch.parse().map_err(|_| invalid("malformed"))?;
        let sequence: i64 = sequence.parse().map_err(|_| invalid("malformed"))?;
        let position = crate::store::Position { epoch, sequence };
        // A cursor past the current epoch's head was never issued by this
        // stream. A cursor into an earlier epoch is valid even past that
        // epoch's end: that is exactly what epochs exist to report.
        let current = self.store.current_epoch()?;
        if position.epoch > current
            || (position.epoch == current
                && position.sequence > self.store.last_sequence(current)? + 1)
        {
            return Err(invalid("beyond the end of the stream"));
        }
        Ok(position)
    }

    fn events_subscribe(&mut self, query: &Query) -> Result<Value, ProtocolError> {
        if !self.selected_feature("core.events") {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("core.events".into())]),
            ));
        }
        let start = self.read_start(&query.payload)?;
        let kinds = self.read_kinds(&query.payload)?;
        self.next_subscription += 1;
        let id = format!("sub-{}", self.next_subscription);
        self.subscriptions.push(Subscription {
            id: id.clone(),
            cursor: start,
            kinds,
            // The principal is captured now and re-checked before every
            // delivery, so a subscription cannot outlive the authority that
            // created it.
            principal: self.config.principal.clone(),
            grant: query.grant.clone(),
            ended: false,
        });
        Ok(Value::Object(vec![
            ("subscription".into(), Value::String(id)),
            (
                "stream".into(),
                Value::Object(vec![
                    ("id".into(), Value::String(self.store.stream_id()?)),
                    ("epoch".into(), Value::Int(self.store.current_epoch()?)),
                ]),
            ),
        ]))
    }

    fn events_unsubscribe(&mut self, query: &Query) -> Result<Value, ProtocolError> {
        let id = query
            .payload
            .get("subscription")
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::invalid_envelope("/payload/subscription", "absent"))?;
        let before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.id != id);
        if self.subscriptions.len() == before {
            return Err(ProtocolError::not_found());
        }
        Ok(Value::Object(vec![]))
    }

    /// Frames to deliver for every live subscription, in order.
    ///
    /// Called after each request has produced its response, so a notification
    /// carrying events caused by a command on this connection follows that
    /// command's response (CORE section 16.5).
    /// `pending` is what the connection has produced but not yet written.
    /// When it is over the bound, no further items are produced: the withheld
    /// items stay undelivered and are produced on a later pass. They are never
    /// skipped, because a semantic event that vanishes is indistinguishable
    /// from one that never happened.
    /// The per-connection bound on output produced but not yet written.
    pub fn pending_output_bound(&self) -> usize {
        self.config.events_max_pending_notification_bytes.max(1) as usize
    }

    /// The room deadline for a stalled consumer, and the one budget all its
    /// ending notices share.
    pub fn backpressure_notice(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.config.events_backpressure_notice_ms.max(0) as u64)
    }

    /// Whether this session negotiated `core.events.backpressure`. A session
    /// that did not is bounded and closed the same way, but is never sent the
    /// `consumer_too_slow` reason it did not agree to understand.
    pub fn backpressure_negotiated(&self) -> bool {
        self.selected_feature("core.events.backpressure")
    }

    /// Produce the notifications owed to this connection's subscriptions,
    /// without producing more than `room` bytes of item-carrying frames.
    ///
    /// `whole` lets the first notification exceed `room` when nothing else is
    /// pending, so an item that fits the caller's frame limit is never withheld
    /// forever by a bound smaller than itself. Returns the notifications and
    /// whether one was withheld for lack of room. A withheld notification is
    /// not produced and its subscription's cursor does not move, so its items
    /// are produced again later: **withheld, never skipped** (CORE 16.5).
    pub fn drain_subscriptions(&mut self, room: usize, whole: bool) -> (Vec<Value>, bool) {
        let _processing = processing();
        let mut frames = Vec::new();
        let mut produced = 0usize;
        let mut withheld = false;
        let budget = self.receive_budget();
        'subscriptions: for index in 0..self.subscriptions.len() {
            loop {
                let subscription = &self.subscriptions[index];
                if subscription.ended {
                    break;
                }
                // Authorization is re-checked before **each** delivery, not
                // only at subscribe: a subscription must not outlive the
                // authority it was created under, and the grant it named can
                // be revoked, expire, or lose its authority epoch between two
                // notifications (CORE section 16.6).
                let Some(in_force) = self
                    .events_authorization(&subscription.principal, subscription.grant.as_deref())
                else {
                    let cursor = subscription.cursor;
                    let id = subscription.id.clone();
                    self.subscriptions[index].ended = true;
                    frames.push(self.ending_notification(&id, cursor, "authorization_lost"));
                    break;
                };
                // Still under the processing lock: nothing can commit between
                // the re-authorization above and the read below.
                crate::barriers::pause(crate::barriers::RECHECK_AFTER_AUTHORIZATION);
                let (id, cursor, kinds) = (
                    subscription.id.clone(),
                    subscription.cursor,
                    subscription.kinds.clone(),
                );
                let visible = |key: &SubjectKey| self.may_read(in_force.as_ref(), key);
                let Ok(read) = self
                    .store
                    .read_events(cursor, 1000, &kinds, budget, &visible)
                else {
                    break;
                };
                if read.first_item_too_large {
                    // The item is never skipped: the subscription ends and says
                    // where delivery stopped, so the consumer knows exactly what
                    // it has not seen.
                    self.subscriptions[index].ended = true;
                    frames.push(self.ending_notification(&id, read.next_cursor, "item_too_large"));
                    break;
                }
                if read.items.is_empty() {
                    break;
                }
                let Ok(next_cursor) = self.encode_cursor(read.next_cursor) else {
                    break;
                };
                let frame = notification(&id, read.items, &next_cursor, None);
                let size = crate::frames::encode(&frame).len();
                if produced + size > room && !(produced == 0 && whole) {
                    withheld = true;
                    break 'subscriptions;
                }
                produced += size;
                self.subscriptions[index].cursor = read.next_cursor;
                frames.push(frame);
            }
        }
        self.subscriptions.retain(|s| !s.ended);
        (frames, withheld)
    }

    /// End every subscription on the connection, returning one ending notice
    /// each, at the position delivery stopped. Used when the connection closes
    /// for backpressure: every subscription on it ends whether or not a notice
    /// is owed or written.
    pub fn end_subscriptions(&mut self, reason: &str) -> Vec<Value> {
        let subscriptions = std::mem::take(&mut self.subscriptions);
        subscriptions
            .iter()
            .filter(|subscription| !subscription.ended)
            .map(|subscription| {
                self.ending_notification(&subscription.id, subscription.cursor, reason)
            })
            .collect()
    }

    fn ending_notification(&self, id: &str, cursor: crate::store::Position, reason: &str) -> Value {
        let encoded = self.encode_cursor(cursor).unwrap_or_default();
        notification(id, Vec::new(), &encoded, Some(reason))
    }

    fn read_start(&self, payload: &Value) -> Result<crate::store::Position, ProtocolError> {
        match (payload.get("cursor"), payload.get("from")) {
            (Some(Value::String(cursor)), None) => self.decode_cursor(cursor),
            (None, Some(Value::String(from))) => match from.as_str() {
                "start" => Ok(self.store.stream_start()?),
                "now" => {
                    let epoch = self.store.current_epoch()?;
                    Ok(crate::store::Position {
                        epoch,
                        sequence: self.store.last_sequence(epoch)? + 1,
                    })
                }
                _ => Err(ProtocolError::invalid_envelope(
                    "/payload/from",
                    "unknown value",
                )),
            },
            _ => Err(ProtocolError::invalid_envelope(
                "/payload",
                "exactly one of cursor or from is required",
            )),
        }
    }

    fn read_kinds(&self, payload: &Value) -> Result<Vec<String>, ProtocolError> {
        match payload.get("kinds") {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => Ok(items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()),
            Some(_) => Err(ProtocolError::invalid_envelope(
                "/payload/kinds",
                "not an array",
            )),
        }
    }

    fn events_read(&self, query: &Query, in_force: Option<&Grant>) -> Result<Value, ProtocolError> {
        if !self.selected_feature("core.events") {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("core.events".into())]),
            ));
        }
        let payload = &query.payload;
        let limit = match payload.get("limit") {
            Some(Value::Int(n)) if (1..=1000).contains(n) => *n,
            _ => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/limit",
                    "not an integer between 1 and 1000",
                ));
            }
        };
        // Exactly one of `cursor` or `from`.
        let start = match (payload.get("cursor"), payload.get("from")) {
            (Some(Value::String(cursor)), None) => self.decode_cursor(cursor)?,
            (None, Some(Value::String(from))) => match from.as_str() {
                "start" => self.store.stream_start()?,
                "now" => {
                    let epoch = self.store.current_epoch()?;
                    crate::store::Position {
                        epoch,
                        sequence: self.store.last_sequence(epoch)? + 1,
                    }
                }
                _ => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload/from",
                        "unknown value",
                    ));
                }
            },
            _ => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload",
                    "exactly one of cursor or from is required",
                ));
            }
        };
        let kinds: Vec<String> = match payload.get("kinds") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/kinds",
                    "not an array",
                ));
            }
        };

        let visible = |key: &SubjectKey| self.may_read(in_force, key);
        let read = self
            .store
            .read_events(start, limit, &kinds, self.receive_budget(), &visible)?;
        if read.first_item_too_large {
            // No honest partial answer exists: the caller cannot be handed a
            // view that silently omits the item it asked for.
            return Err(ProtocolError::new_internal_error());
        }
        Ok(Value::Object(vec![
            (
                "stream".into(),
                Value::Object(vec![
                    ("id".into(), Value::String(self.store.stream_id()?)),
                    ("epoch".into(), Value::Int(read.stream_epoch)),
                ]),
            ),
            ("items".into(), Value::Array(read.items)),
            (
                "next_cursor".into(),
                Value::String(self.encode_cursor(read.next_cursor)?),
            ),
            ("filtered".into(), Value::Bool(read.filtered)),
        ]))
    }

    /// How many bytes of items one response may carry, leaving room for the
    /// envelope around them.
    fn receive_budget(&self) -> usize {
        self.caller_receive_limit.saturating_sub(4096)
    }

    fn subject_get(&self, query: &Query) -> Result<Value, ProtocolError> {
        let key = self.subject_key(&query.payload)?;
        // The result requires a revision of at least 1, so a subject that does
        // not exist is absent rather than a zero-revision reading.
        match self.store.subject(&key)? {
            None => Err(ProtocolError::not_found()),
            Some(state) => Ok(Value::Object(vec![
                (
                    "subject".into(),
                    query.payload.get("subject").expect("checked").clone(),
                ),
                ("revision".into(), Value::Int(state.revision)),
                ("value".into(), Value::String(state.value.clone())),
            ])),
        }
    }

    fn subject_key(&self, payload: &Value) -> Result<SubjectKey, ProtocolError> {
        let subject = payload
            .get("subject")
            .ok_or_else(|| ProtocolError::invalid_envelope("/payload/subject", "absent"))?;
        let key = SubjectKey {
            kind: subject
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            id: subject
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        };
        if key.kind != "core-test.subject" {
            return Err(ProtocolError::invalid_envelope(
                "/payload/subject/kind",
                "not core-test.subject",
            ));
        }
        Ok(key)
    }

    /// Steps 4 to 7 of the command path, shared by every command so the order
    /// cannot drift between operations. Returns the stored result when the
    /// command was already bound.
    fn admit_command(
        &mut self,
        params: &Value,
        command: &Command,
        scope: &str,
        epoch_checked: bool,
        step6: Step6,
    ) -> Result<Option<Value>, ProtocolError> {
        // Step 4: digest algorithm, then the recomputed digest.
        match cbr_encoding::parse_digest(&command.command_digest) {
            Ok((cbr_encoding::Algorithm::Sha256, _)) => {}
            // sha512 is available only when `core.digest-sha512` is negotiated,
            // and this build offers no features.
            Ok((cbr_encoding::Algorithm::Sha512, _)) => {
                return Err(ProtocolError::unsupported_digest_algorithm(
                    "sha512",
                    vec!["sha256"],
                ));
            }
            Err(cbr_encoding::DigestError::UnsupportedAlgorithm(algorithm)) => {
                return Err(ProtocolError::unsupported_digest_algorithm(
                    &algorithm,
                    vec!["sha256"],
                ));
            }
            Err(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/command_digest",
                    "malformed digest",
                ));
            }
        }
        let expected = cbr_encoding::command_digest(params).map_err(|error| match error {
            cbr_encoding::IntentError::RequiredExtensionMissing(key) => {
                ProtocolError::invalid_envelope("/requires", &format!("extension {key} absent"))
            }
            cbr_encoding::IntentError::DuplicateRequires(_) => {
                ProtocolError::invalid_envelope("/requires", "duplicate entry")
            }
            _ => ProtocolError::invalid_envelope("", "envelope cannot form a command intent"),
        })?;
        if expected != command.command_digest {
            return Err(ProtocolError::digest_mismatch(&expected));
        }

        // Step 5: deduplication, before authorization, so a replay of a
        // caller's own bound command still returns its stored result.
        let principal = self.config.principal.clone();
        if command.dedupe_generation > self.dedupe_current {
            return Err(ProtocolError::invalid_envelope(
                "/dedupe_generation",
                "generation was never issued",
            ));
        }
        if let Some(record) = self.store.command(&principal, &command.command_id)? {
            if record.digest == command.command_digest {
                return Ok(Some(replayed(record.result.clone())));
            }
            return Err(ProtocolError::idempotency_conflict(&command.command_id));
        }
        if command.dedupe_generation < self.dedupe_oldest() {
            return Err(ProtocolError::dedupe_history_unavailable(
                self.dedupe_oldest(),
            ));
        }

        // Step 6: authorization. It sits **between** deduplication and every
        // check that depends on whether a subject exists. Both sides matter:
        // before it, a replay of a bound command returns its stored result
        // even after the grant was revoked (CORE section 15.5); after it, an
        // unauthorised principal cannot tell an existing subject from an
        // absent one, because the first lookup is step 7's preconditions.
        let in_force = match step6 {
            Step6::Rights(needs) => self.authorize(command.grant.as_deref(), &needs)?,
            Step6::Issue(grant) => self.authorize_issue(&grant)?,
            Step6::Revoke => self.authorize_revoke(command)?,
            Step6::Abort => self.authorize_abort(command)?,
        };

        // Step 7: capabilities, then the authority epoch, then preconditions.
        //
        // A capability refusal comes first so a command whose capability is
        // lost changes nothing and names no precondition. It follows step 5, so
        // a command bound before the loss still replays its stored outcome
        // (CORE section 17.2): loss does not rewrite accepted history.
        if let Some(capability) = capability_dependency(&command.operation) {
            let status = self.capability_status(capability)?;
            if status != "supported" {
                return Err(ProtocolError::capability_unavailable(capability, &status));
            }
        }
        //
        // CORE section 8 applies to operations that *act under* an authority
        // "that can be taken over", and only to those. `core-test.authority.claim`
        // is the takeover itself, not an operation performed under one, so it
        // carries no `authority_epoch` and is not epoch-checked. Across the
        // whole pinned fixture corpus this is unambiguous: all 198
        // `core-test.subject.put` commands carry `authority_epoch` and none
        // omits it, while all 16 `core-test.authority.claim` commands omit it.
        // A claim is ordered by its precondition on the authority subject
        // instead, which is why `core.authority.claim-requires-current-epoch`
        // expects `precondition_failed` rather than `stale_authority_epoch`.
        //
        // An absent epoch on an operation that does require one is
        // `invalid_envelope`. Section 8 permits a profile to treat it as epoch
        // 0 instead -- "A profile whose epoch exists only once claimed may
        // treat an absent epoch as epoch 0, so it is `stale_authority_epoch`
        // once any epoch exists (EXECUTION section 11.3)" -- but that is
        // something a profile must state, and `core-test` does not. No fixture
        // exercises the absent case either way, so this is the specification's
        // default rather than a measured behaviour.
        if epoch_checked {
            let current_epoch = self.store.epoch(scope)?;
            let Some(epoch) = command.authority_epoch else {
                return Err(ProtocolError::invalid_envelope(
                    "/authority_epoch",
                    "required by this operation",
                ));
            };
            if epoch < current_epoch {
                // The current epoch is disclosed only to a principal that
                // could read the authority subject anyway; to anyone else the
                // refusal names no number (CORE section 15.5).
                let readable = self.may_read(in_force.as_ref(), &Store::authority_key(scope));
                return Err(ProtocolError::new_stale_epoch(
                    readable.then_some(current_epoch),
                ));
            }
            if epoch > current_epoch {
                return Err(ProtocolError::new_unknown_epoch());
            }
        }

        // A command must carry a precondition on its own subject: without one
        // it would apply against an unknown starting revision.
        if !command
            .preconditions
            .iter()
            .any(|(subject, _)| *subject == command.subject)
        {
            return Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "no precondition on the command's own subject",
            ));
        }

        let mut failed = Vec::new();
        for (subject, expected_revision) in &command.preconditions {
            let key = key_of(subject);
            let current = self.store.revision(&key)?;
            if current != *expected_revision {
                let mut entry = vec![
                    ("subject".into(), subject.clone()),
                    ("expected".into(), Value::Int(*expected_revision)),
                ];
                // Same rule as the epoch above: a current revision is a fact
                // about the subject, so it is shown only to a principal that
                // may read it.
                if self.may_read(in_force.as_ref(), &key) {
                    entry.push(("current".into(), Value::Int(current)));
                }
                failed.push(Value::Object(entry));
            }
        }
        if !failed.is_empty() {
            return Err(ProtocolError::precondition_failed(Value::Array(failed)));
        }
        Ok(None)
    }

    /// Claim the next authority epoch for the `core-test` scope.
    ///
    /// Implemented from the profile's schemas; **no fixture in the `stream`
    /// suite exercises it**, so it is unverified until the Core suite runs.
    fn authority_claim(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        if let Some(stored) = self.admit_command(
            params,
            &command,
            "core-test",
            false,
            Step6::Rights(Self::command_needs(&command)),
        )? {
            return Ok(stored);
        }
        // The epoch and the authority subject's revision are the same number by
        // construction, so the acknowledgment's revision and the outcome's
        // epoch cannot disagree. The state change and the command record commit
        // in one transaction.
        let principal = self.config.principal.clone();
        let key = Store::authority_key("core-test");
        let command_digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let caused_by = command.caused_by.clone();
        let recorded_at = self.clock.now();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: "",
                principal: &principal,
                command_id: &command_id,
                digest: &command_digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also: Vec::new(),
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                event: Some(crate::store::NewEvent {
                    event_type: "core-test.authority.claimed".into(),
                    payload: Box::new(|epoch| {
                        Value::Object(vec![("epoch".into(), Value::Int(epoch))])
                    }),
                    caused_by: caused_by.clone(),
                }),
            },
            |epoch, operation_ref| {
                accepted(
                    &command_id,
                    &command_digest,
                    operation_ref,
                    &subject,
                    epoch,
                    Value::Object(vec![("epoch".into(), Value::Int(epoch))]),
                )
            },
        )?;
        Ok(result)
    }

    fn subject_put(&mut self, params: &Value, command: Command) -> Result<Value, ProtocolError> {
        if let Some(stored) = self.admit_command(
            params,
            &command,
            "core-test",
            true,
            Step6::Rights(Self::command_needs(&command)),
        )? {
            return Ok(stored);
        }

        // Step 8: commit the command record, the state change and the result in
        // one transaction, so a crash between them is not representable.
        let value = command
            .payload
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::invalid_envelope("/payload/value", "not a string"))?
            .to_string();
        let key = key_of(&command.subject);
        let principal = self.config.principal.clone();
        let command_digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let caused_by = command.caused_by.clone();
        let stored_value = value.clone();
        let event_value = value.clone();
        let recorded_at = self.clock.now();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &command_digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also: Vec::new(),
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                event: Some(crate::store::NewEvent {
                    event_type: "core-test.subject.changed".into(),
                    caused_by: caused_by.clone(),
                    payload: Box::new(move |_| {
                        Value::Object(vec![("value".into(), Value::String(event_value))])
                    }),
                }),
            },
            |revision, operation_ref| {
                accepted(
                    &command_id,
                    &command_digest,
                    operation_ref,
                    &subject,
                    revision,
                    Value::Object(vec![("value".into(), Value::String(stored_value.clone()))]),
                )
            },
        )?;
        Ok(result)
    }
}

struct Subscription {
    id: String,
    cursor: crate::store::Position,
    kinds: Vec<String>,
    principal: String,
    /// The grant the subscription was created under, re-resolved before every
    /// delivery rather than captured: a grant revoked or expired after
    /// `subscribe` must stop the stream, and a captured copy could not.
    grant: Option<String>,
    ended: bool,
}

/// A `core.events.notify` notification frame.
fn notification(id: &str, items: Vec<Value>, next_cursor: &str, ended: Option<&str>) -> Value {
    let mut params = vec![
        ("subscription".into(), Value::String(id.to_string())),
        ("items".into(), Value::Array(items)),
        ("next_cursor".into(), Value::String(next_cursor.to_string())),
    ];
    if let Some(reason) = ended {
        params.push((
            "ended".into(),
            Value::Object(vec![("reason".into(), Value::String(reason.to_string()))]),
        ));
    }
    Value::Object(vec![
        ("jsonrpc".into(), Value::String("2.0".into())),
        ("method".into(), Value::String("core.events.notify".into())),
        ("params".into(), Value::Object(params)),
    ])
}

/// A replay returns the stored acknowledgment and outcome unchanged, with
/// `replay` set to true.
fn replayed(mut result: Value) -> Value {
    if let Value::Object(members) = &mut result {
        for (name, value) in members.iter_mut() {
            if name == "replay" {
                *value = Value::Bool(true);
            }
        }
    }
    result
}

impl ProtocolError {
    fn new_stale_epoch(current: Option<i64>) -> Self {
        Self {
            code: "stale_authority_epoch",
            retry: crate::errors::Retry::AfterReconcile,
            details: current
                .map(|epoch| vec![("current_epoch".into(), Value::Int(epoch))])
                .unwrap_or_default(),
        }
    }

    fn new_invalid_cursor(reason: &str) -> Self {
        Self {
            code: "invalid_cursor",
            retry: crate::errors::Retry::No,
            details: vec![("reason".into(), Value::String(reason.to_string()))],
        }
    }

    fn new_internal_error() -> Self {
        Self {
            code: "internal_error",
            retry: crate::errors::Retry::AfterReconcile,
            details: Vec::new(),
        }
    }

    fn new_unknown_epoch() -> Self {
        Self {
            code: "unknown_authority_epoch",
            retry: crate::errors::Retry::No,
            details: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mode};

    fn provider(directory: &std::path::Path) -> Provider {
        let config = Config {
            mode: Mode::Conformance,
            principal: "owner".into(),
            authority_principals: vec!["owner".into()],
            ..Config::default()
        };
        let clock = crate::clock::Clock::open(crate::clock::Source::System).expect("clock");
        Provider::open(config, clock, directory).expect("opens")
    }

    /// A subscription must not outlive the authority it was created under.
    ///
    /// The fixture-level kill for the grant form of this guard landed in c3:
    /// `core.events.subscription-ends-at-grant-expiry`,
    /// `core.events.subscription-ends-when-grant-revoked` and
    /// `core.events.subscription-ends-when-grant-stops-authorizing`. This test
    /// covers the authority-principal form, which no fixture reaches.
    #[test]
    fn a_subscription_ends_when_its_principal_stops_being_an_authority() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut provider = provider(directory.path());
        provider.negotiated = Some(vec![("core".into(), 1, vec!["core.events".into()])]);

        let subscribe = cbr_encoding::parse(
            br#"{"operation":"core.events.subscribe","message_id":"m","payload":{"from":"start"}}"#,
        )
        .unwrap();
        let query = envelope::parse_query(&subscribe).unwrap();
        provider.events_subscribe(&query).expect("subscribes");
        assert_eq!(provider.subscriptions.len(), 1);

        // Nothing to deliver yet, and the subscription survives.
        assert!(provider.drain_subscriptions(usize::MAX, true).0.is_empty());
        assert_eq!(provider.subscriptions.len(), 1);

        // The principal stops being an authority. The next delivery ends the
        // subscription rather than continuing to serve it.
        provider.config.authority_principals = vec!["someone-else".into()];
        let (frames, _) = provider.drain_subscriptions(usize::MAX, true);
        assert_eq!(frames.len(), 1, "one ending notification: {frames:?}");
        let params = frames[0].get("params").expect("params");
        assert_eq!(
            params
                .get("ended")
                .and_then(|e| e.get("reason"))
                .and_then(Value::as_str),
            Some("authorization_lost")
        );
        assert_eq!(params.get("items"), Some(&Value::Array(vec![])));
        assert!(provider.subscriptions.is_empty(), "and it is gone");
    }

    // ---- effects: no fixture CBR can run exercises these ------------------

    fn negotiated_with_effects(provider: &mut Provider) {
        provider.negotiated = Some(vec![
            (
                "core".into(),
                1,
                vec![
                    "core.events".into(),
                    "core.grants".into(),
                    "core.effects".into(),
                ],
            ),
            ("core-test".into(), 1, vec![]),
        ]);
    }

    /// Send a query or command through `handle`, computing a command's digest.
    fn call(provider: &mut Provider, envelope: &str) -> Result<Value, ProtocolError> {
        let mut value = cbr_encoding::parse(envelope.as_bytes()).expect("envelope parses");
        if value.get("command_id").is_some() {
            let digest = cbr_encoding::command_digest(&value).expect("intent");
            if let Value::Object(members) = &mut value {
                members.push(("command_digest".into(), Value::String(digest)));
            }
        }
        let method = value
            .get("operation")
            .and_then(Value::as_str)
            .expect("operation")
            .to_string();
        provider.handle(&method, &value)
    }

    /// Record an effect the way a producer will: in the authorizing command's
    /// own transaction, before any external action. Returns its id.
    fn record_effect(provider: &mut Provider, target: &str, deadline: Option<&str>) -> String {
        let key = SubjectKey {
            kind: "core-test.subject".into(),
            id: target.into(),
        };
        let recorded_at = provider.clock.now();
        let result = provider
            .store
            .commit_command(
                crate::store::Commit {
                    key: &key,
                    value: "v",
                    principal: "owner",
                    command_id: &format!("produce-{target}"),
                    digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    generation: 1,
                    event: None,
                    also: Vec::new(),
                    effects: vec![effects::NewEffect {
                        kind: "cbr.model_call".into(),
                        target: key.clone(),
                        payload_digest: format!("sha256:{}", "b".repeat(64)),
                        retry_class: effects::RetryClass::NonRepeatable,
                        idempotency_key: None,
                        obligations: vec![(
                            "o-outcome".into(),
                            "outcome".into(),
                            deadline.map(str::to_string),
                        )],
                    }],
                    grant: None,
                    recorded_at: &recorded_at,
                },
                |_, operation_ref| Value::String(effects::effect_id(operation_ref, 0)),
            )
            .expect("commits");
        result.as_str().expect("effect id").to_string()
    }

    fn get(
        provider: &mut Provider,
        effect: &str,
        grant: Option<&str>,
    ) -> Result<Value, ProtocolError> {
        let grant = grant
            .map(|g| format!(r#","grant":"{g}""#))
            .unwrap_or_default();
        call(
            provider,
            &format!(
                r#"{{"operation":"core.effects.get","message_id":"m-g"{grant},"payload":{{"effect":"{effect}"}}}}"#
            ),
        )
    }

    #[test]
    fn an_effect_is_recorded_with_its_command_and_read_back_whole() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut provider = provider(directory.path());
        negotiated_with_effects(&mut provider);
        let id = record_effect(&mut provider, "s-1", None);

        let result = get(&mut provider, &id, None).expect("an authority reads it");
        assert_eq!(result.get("revision"), Some(&Value::Int(1)));
        assert_eq!(
            result.get("status").and_then(Value::as_str),
            Some("pending")
        );
        let descriptor = result.get("effect").expect("descriptor");
        assert_eq!(
            descriptor.get("id").and_then(Value::as_str),
            Some(id.as_str())
        );
        assert_eq!(
            descriptor.get("retry_class").and_then(Value::as_str),
            Some("non_repeatable")
        );
        // The operation that recorded it is named, and is the id's own prefix.
        let operation_ref = descriptor
            .get("operation_ref")
            .and_then(Value::as_str)
            .unwrap();
        assert!(id.starts_with(&format!("{operation_ref}.")));

        // An id never recorded is not_found for an authority.
        let missing = get(&mut provider, "op-999.e1", None).expect_err("absent");
        assert_eq!(missing.code, "not_found");
    }

    #[test]
    fn reading_an_effect_under_a_grant_does_not_reveal_whether_it_exists() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut provider = provider(directory.path());
        negotiated_with_effects(&mut provider);
        let id = record_effect(&mut provider, "s-1", None);
        for (grant, resource) in [("g-elsewhere", "s-9"), ("g-here", "s-1")] {
            call(
                &mut provider,
                &format!(
                    r#"{{"operation":"core.grant.issue","message_id":"m-{grant}","command_id":"issue-{grant}","dedupe_generation":1,"subject":{{"kind":"core.grant","id":"{grant}"}},"preconditions":[{{"subject":{{"kind":"core.grant","id":"{grant}"}},"revision":0}}],"requires":[],"payload":{{"holder":"agent-1","audience":"cbr","rights":["core-test.read"],"resources":[{{"kind":"core-test.subject","id":"{resource}"}}],"delegation":{{"allowed":false,"max_depth":0}}}}}}"#
                ),
            )
            .expect("issued");
        }
        provider.config.principal = "agent-1".into();

        let existing = get(&mut provider, &id, Some("g-elsewhere")).expect_err("not readable");
        let absent = get(&mut provider, "op-999.e1", Some("g-elsewhere")).expect_err("absent");
        assert_eq!(existing.code, "permission_denied");
        assert_eq!(
            existing.to_data(),
            absent.to_data(),
            "an existing effect and an absent one are refused identically (CORE-12)"
        );
        // A grant that can read the target reads the effect.
        assert!(get(&mut provider, &id, Some("g-here")).is_ok());
        // And an effect that does not exist is refused alike even under that
        // grant, because there is no target it could read.
        let absent_here = get(&mut provider, "op-999.e1", Some("g-here")).expect_err("absent");
        assert_eq!(absent_here.to_data(), existing.to_data());
    }

    #[test]
    fn an_overdue_obligation_is_announced_and_aborting_it_leaves_the_status_alone() {
        let directory = tempfile::tempdir().expect("temp dir");
        let clock_file = directory.path().join("clock");
        std::fs::write(&clock_file, "2030-01-01T00:00:00Z").expect("clock");
        let config = Config {
            mode: Mode::Conformance,
            principal: "owner".into(),
            authority_principals: vec!["owner".into()],
            ..Config::default()
        };
        let clock = crate::clock::Clock::open(crate::clock::Source::File(clock_file.clone()))
            .expect("clock");
        let data = directory.path().join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        let mut provider = Provider::open(config, clock, &data).expect("opens");
        negotiated_with_effects(&mut provider);
        let id = record_effect(&mut provider, "s-1", Some("2030-01-01T00:01:00Z"));

        // Before the deadline: open. At the deadline, the next request marks it.
        let before = get(&mut provider, &id, None).expect("reads");
        assert!(format!("{before:?}").contains(r#"String("open")"#));
        std::fs::write(&clock_file, "2030-01-01T00:01:00Z").expect("advances");
        let after = get(&mut provider, &id, None).expect("reads");
        assert!(
            format!("{after:?}").contains(r#"String("overdue")"#),
            "{after:?}"
        );
        assert_eq!(after.get("revision"), Some(&Value::Int(2)));
        assert_eq!(
            after.get("status").and_then(Value::as_str),
            Some("pending"),
            "an ended wait is not an observation"
        );
        let read = call(
            &mut provider,
            r#"{"operation":"core.events.read","message_id":"m-r","payload":{"limit":100,"from":"start"}}"#,
        )
        .expect("reads events");
        let rendered = String::from_utf8(cbr_encoding::to_canonical(&read)).unwrap();
        assert_eq!(
            rendered.matches("core.effect.obligation.overdue").count(),
            1,
            "one provider-origin overdue event, not one per request: {rendered}"
        );

        // The outcome cannot be established: unknown, and still waiting.
        provider
            .observe_effect(&id, "unknown", "provider", "response lost")
            .expect("observes");
        let abort = |provider: &mut Provider, command: &str, revision: i64| {
            call(
                provider,
                &format!(
                    r#"{{"operation":"core.effects.abort_obligation","message_id":"m-{command}","command_id":"{command}","dedupe_generation":1,"subject":{{"kind":"core.effect","id":"{id}"}},"preconditions":[{{"subject":{{"kind":"core.effect","id":"{id}"}},"revision":{revision}}}],"requires":[],"payload":{{"obligation":"o-outcome"}}}}"#
                ),
            )
        };
        let aborted = abort(&mut provider, "abort-1", 3).expect("aborts");
        let outcome = aborted.get("outcome").expect("outcome");
        assert_eq!(
            outcome.get("status").and_then(Value::as_str),
            Some("unknown"),
            "EFF-4: aborting the wait leaves the effect unknown"
        );
        assert_eq!(
            aborted
                .get("acknowledgment")
                .and_then(|a| a.get("effect_refs")),
            Some(&Value::Array(vec![])),
            "the abort records no effect of its own"
        );
        let final_state = get(&mut provider, &id, None).expect("reads");
        assert_eq!(
            final_state.get("status").and_then(Value::as_str),
            Some("unknown")
        );

        // An obligation no longer waiting is not found.
        let again = abort(&mut provider, "abort-2", 4).expect_err("not waiting");
        assert_eq!(again.code, "not_found");
    }
}
