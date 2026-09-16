//! The provider: negotiation and the Core command path.
//!
//! CORE section 10 fixes the order of checks, and the first failing step
//! determines the error. That order is not an implementation detail: it is what
//! stops an unauthorised caller learning whether a subject exists, and what
//! lets a caller retransmit a command after its authority epoch changed. The
//! steps below are numbered to match the specification.

use cbr_encoding::Value;

use crate::config::Config;
use crate::envelope::{self, Command, Query};
use crate::errors::ProtocolError;
use crate::store::{Store, SubjectKey};

/// Profiles this build serves, with the majors and features it implements.
/// A profile is listed here only when it is implemented: over-claiming would
/// make negotiation succeed and then fail at the first operation.
const SERVED: &[(&str, i64, &[&str], &[&str])] =
    &[("core", 1, &[], &[]), ("core-test", 1, &[], &["core"])];

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
const UNPROTECTED: [&str; 4] = [
    "core.describe",
    "core.negotiate",
    "core.feature_dependencies",
    "core.authenticate",
];

pub struct Provider {
    pub config: Config,
    store: Store,
    negotiated: Option<Vec<(String, i64, Vec<String>)>>,
    dedupe_current: i64,
    dedupe_oldest: i64,
    operation_counter: u64,
}

impl Provider {
    /// Open the provider over its data directory, advancing the deduplication
    /// generation for this process.
    pub fn open(
        config: Config,
        data_dir: &std::path::Path,
    ) -> Result<Self, crate::store::StoreError> {
        let mut store = Store::open(data_dir)?;
        let (current, oldest) = store.start_generation(
            config.dedupe_advance_on_start,
            config.dedupe_retain_generations,
        )?;
        Ok(Self {
            config,
            store,
            negotiated: None,
            dedupe_current: current,
            dedupe_oldest: oldest,
            operation_counter: 0,
        })
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
            ("limits".into(), self.config.limits.to_value()),
            ("dedupe_window".into(), self.dedupe_window()),
        ]);
        self.negotiated = Some(selected);
        Ok(result)
    }

    // ---- dispatch ---------------------------------------------------------

    /// Handle one request. `method` is the transport method, which the envelope
    /// must agree with.
    pub fn handle(&mut self, method: &str, params: &Value) -> Result<Value, ProtocolError> {
        // Step 1: operation known, session negotiated, profile selected. These
        // are decided from the transport method, before the envelope is read.
        if !self.known_operation(method) {
            return Err(ProtocolError::method_not_found(method));
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

        // Step 2: limits, then the method/operation agreement, then shape.
        envelope::check_limits(params, &self.config.limits)?;

        // Step 6: authorization, for every protected operation including
        // queries. This runs before the operation looks anything up, because
        // CORE section 15.5 requires an unauthorised principal to get the same
        // `permission_denied` whether or not the subject exists. Doing it
        // inside each operation would leak existence through `not_found` on the
        // query path, which is how `core.grants.authorization-without-grants-
        // feature` catches it. This build negotiates no `core.grants`, so a
        // session cannot name a grant and only an authority principal is
        // authorized.
        if !UNPROTECTED.contains(&method) && !self.config.is_authority(&self.config.principal) {
            return Err(ProtocolError::permission_denied("grant_required"));
        }

        match method {
            "core.describe" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                self.check_requires(&query.requires)?;
                Ok(self.describe())
            }
            "core.authenticate" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                self.check_requires(&query.requires)?;
                // CORE section 18: the stdio binding assigns the principal
                // through the launch configuration, so every session already
                // has one from its first frame, before and after negotiation.
                // The socket form, where a session starts unauthenticated,
                // arrives with that binding.
                Err(ProtocolError::already_authenticated())
            }
            "core.feature_dependencies" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                self.check_requires(&query.requires)?;
                Ok(self.feature_dependencies())
            }
            "core.negotiate" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                // `already_negotiated` is decided after steps 1 to 3.
                self.check_requires(&query.requires)?;
                self.negotiate(&query.payload)
            }
            "core-test.subject.applied_count" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                self.check_requires(&query.requires)?;
                self.applied_count(&query)
            }
            "core-test.subject.get" => {
                let query = envelope::parse_query(params)?;
                self.check_method_matches(method, &query.operation)?;
                self.check_requires(&query.requires)?;
                self.subject_get(&query)
            }
            "core-test.subject.put" => {
                let command = envelope::parse_command(params)?;
                self.check_method_matches(method, &command.operation)?;
                self.check_requires(&command.requires)?;
                self.subject_put(params, command)
            }
            "core-test.authority.claim" => {
                let command = envelope::parse_command(params)?;
                self.check_method_matches(method, &command.operation)?;
                self.check_requires(&command.requires)?;
                self.authority_claim(params, command)
            }
            _ => Err(ProtocolError::method_not_found(method)),
        }
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

        // Step 6 already ran in `handle`, before this operation was reached, so
        // that an unauthorised principal cannot tell an existing subject from
        // an absent one.

        // Step 7: capabilities, then the authority epoch, then preconditions.
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
                return Err(ProtocolError::new_stale_epoch(current_epoch));
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
            let current = self.store.revision(&key)?;
            if current != *expected_revision {
                failed.push(Value::Object(vec![
                    ("subject".into(), subject.clone()),
                    ("expected".into(), Value::Int(*expected_revision)),
                    ("current".into(), Value::Int(current)),
                ]));
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
        if let Some(stored) = self.admit_command(params, &command, "core-test", false)? {
            return Ok(stored);
        }
        // The epoch and the authority subject's revision are the same number by
        // construction, so the acknowledgment's revision and the outcome's
        // epoch cannot disagree. The state change and the command record commit
        // in one transaction.
        let principal = self.config.principal.clone();
        let key = Store::authority_key("core-test");
        let counter = {
            self.operation_counter += 1;
            self.operation_counter
        };
        let command_digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: "",
                principal: &principal,
                command_id: &command_id,
                digest: &command_digest,
                generation: command.dedupe_generation,
            },
            |epoch| {
                Value::Object(vec![
                    (
                        "acknowledgment".into(),
                        Value::Object(vec![
                            ("command_id".into(), Value::String(command_id.clone())),
                            (
                                "command_digest".into(),
                                Value::String(command_digest.clone()),
                            ),
                            (
                                "operation_ref".into(),
                                Value::String(format!("op-{counter}")),
                            ),
                            ("subject".into(), subject.clone()),
                            ("revision".into(), Value::Int(epoch)),
                            ("effect_refs".into(), Value::Array(vec![])),
                        ]),
                    ),
                    (
                        "outcome".into(),
                        Value::Object(vec![("epoch".into(), Value::Int(epoch))]),
                    ),
                    ("replay".into(), Value::Bool(false)),
                ])
            },
        )?;
        Ok(result)
    }

    fn subject_put(&mut self, params: &Value, command: Command) -> Result<Value, ProtocolError> {
        if let Some(stored) = self.admit_command(params, &command, "core-test", true)? {
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
        let key = SubjectKey {
            kind: command
                .subject
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            id: command
                .subject
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        };
        let principal = self.config.principal.clone();
        let counter = {
            self.operation_counter += 1;
            self.operation_counter
        };
        let command_digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let stored_value = value.clone();
        let result = self.store.commit_command(
            crate::store::Commit {
                key: &key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &command_digest,
                generation: command.dedupe_generation,
            },
            |revision| {
                Value::Object(vec![
                    (
                        "acknowledgment".into(),
                        Value::Object(vec![
                            ("command_id".into(), Value::String(command_id.clone())),
                            (
                                "command_digest".into(),
                                Value::String(command_digest.clone()),
                            ),
                            (
                                "operation_ref".into(),
                                Value::String(format!("op-{counter}")),
                            ),
                            ("subject".into(), subject.clone()),
                            ("revision".into(), Value::Int(revision)),
                            ("effect_refs".into(), Value::Array(vec![])),
                        ]),
                    ),
                    (
                        "outcome".into(),
                        Value::Object(vec![("value".into(), Value::String(stored_value.clone()))]),
                    ),
                    ("replay".into(), Value::Bool(false)),
                ])
            },
        )?;
        Ok(result)
    }
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
    fn new_stale_epoch(current: i64) -> Self {
        Self {
            code: "stale_authority_epoch",
            retry: crate::errors::Retry::AfterReconcile,
            details: vec![("current_epoch".into(), Value::Int(current))],
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
