//! `knowledge/1` operations on the provider (KNOWLEDGE sections 3 to 11).
//!
//! Every command follows the Core command path through `admit_command`:
//! digest, deduplication, authorization, then the authority epoch and the
//! preconditions, and only then this profile's own checks (CORE section 10).
//! What the order protects:
//!
//! - **Authorization before existence.** Step 6 runs before anything here
//!   looks a claim up, so a principal without `knowledge.read` cannot tell an
//!   existing revision from a missing one.
//! - **The epoch before the preconditions.** A decision or resolution acting
//!   under a transferred binding is `stale_authority_epoch` even when it also
//!   reuses a decision id.
//! - **Nothing decides but the bound authority.** A grant, a derivation label
//!   and a model producer's own opinion never record reliance; only a decision
//!   by the principal the scope is bound to does.
//!
//! A claim revision's record is written once, in the same transaction as the
//! claim subject it numbers, into an insert-only table.

use cbr_encoding::Value;

use super::{Epoch, Provider, Step6, accepted};
use crate::envelope::Command;
use crate::errors::{ProtocolError, Retry};
use crate::knowledge::{self, Observed, canonical, key};
use crate::store::{ClaimRevision, ClaimRevisionRow, Commit, NewEvent, SubjectKey};

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn string(text: &str) -> Value {
    Value::String(text.into())
}

fn subject(kind: &str, id: &str) -> Value {
    object(vec![("kind", string(kind)), ("id", string(id))])
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for name in path {
        match current.get(name) {
            Some(next) => current = next,
            None => return "",
        }
    }
    current.as_str().unwrap_or_default()
}

fn event(event_type: &str, payload: Value, caused_by: &[String]) -> NewEvent {
    NewEvent {
        event_type: event_type.into(),
        caused_by: caused_by.to_vec(),
        payload: Box::new(move |_| payload),
    }
}

fn parse_record(value: &str) -> Result<Value, ProtocolError> {
    cbr_encoding::parse(value.as_bytes()).map_err(|_| ProtocolError::new_internal_error())
}

fn not_authority() -> ProtocolError {
    ProtocolError::permission_denied("not_authority")
}

impl Provider {
    // ---- shared ------------------------------------------------------------

    /// The subject kind and the single precondition every knowledge command
    /// carries (step 2): revision 0 for a command that creates its subject,
    /// at least 1 for one that changes an existing subject.
    fn knowledge_shape(
        &self,
        command: &Command,
        kind: &str,
        existing: bool,
    ) -> Result<SubjectKey, ProtocolError> {
        let key = super::key_of(&command.subject);
        if key.kind != kind {
            return Err(ProtocolError::invalid_envelope(
                "/subject/kind",
                &format!("this operation's subject is a {kind}"),
            ));
        }
        match command.preconditions.as_slice() {
            [(on, revision)] if super::key_of(on) == key => {
                if existing && *revision < 1 {
                    return Err(ProtocolError::invalid_envelope(
                        "/preconditions/0/revision",
                        "an existing subject is at least revision 1",
                    ));
                }
                if !existing && *revision != 0 {
                    return Err(ProtocolError::invalid_envelope(
                        "/preconditions/0/revision",
                        "this operation creates its subject at revision 0",
                    ));
                }
                Ok(key)
            }
            _ => Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "exactly one precondition, on the command's own subject",
            )),
        }
    }

    fn require_epoch(command: &Command) -> Result<(), ProtocolError> {
        if command.authority_epoch.is_none() {
            return Err(ProtocolError::invalid_envelope(
                "/authority_epoch",
                "deciding needs the binding's authority epoch",
            ));
        }
        Ok(())
    }

    fn provider_id(&self) -> String {
        self.config.provider_id.clone()
    }

    /// A revision this provider holds, by a reference naming it. A reference
    /// under another provider is never held here, even when a local claim has
    /// the same id and revision (KNOWLEDGE section 2).
    fn held_revision(&self, reference: &Value) -> Result<Option<ClaimRevisionRow>, ProtocolError> {
        let parts = knowledge::reference_parts(reference);
        if parts.provider != self.config.provider_id {
            return Ok(None);
        }
        Ok(self.store.claim_revision(parts.claim, parts.revision)?)
    }

    /// The authority binding of a scope: `(revision, record)`, where the
    /// revision is the binding's epoch.
    fn binding(&self, scope: &str) -> Result<Option<(i64, Value)>, ProtocolError> {
        let Some(state) = self.store.subject(&key(knowledge::AUTHORITY, scope))? else {
            return Ok(None);
        };
        Ok(Some((state.revision, parse_record(&state.value)?)))
    }

    /// The epoch check for a command acting under the binding of the scope of
    /// `reference`'s revision, when that revision exists here and its scope is
    /// bound (KNOWLEDGE section 6).
    fn epoch_for(&self, reference: &Value) -> Result<Epoch, ProtocolError> {
        let Some(row) = self.held_revision(reference)? else {
            return Ok(Epoch::Unchecked);
        };
        let scope = text(&row.record, &["scope", "id"]);
        Ok(Epoch::Binding(key(knowledge::AUTHORITY, scope)))
    }

    /// Checks 2 and 3 of section 6: the scope is bound, to this principal.
    fn bound_to_caller(&self, scope: &str) -> Result<i64, ProtocolError> {
        match self.binding(scope)? {
            Some((epoch, binding)) if text(&binding, &["authority"]) == self.config.principal => {
                Ok(epoch)
            }
            _ => Err(not_authority()),
        }
    }

    /// Every record of a kind, in recorded order, parsed.
    fn knowledge_records(&self, kind: &str) -> Result<Vec<(String, i64, Value)>, ProtocolError> {
        let mut out = Vec::new();
        for (id, revision, value) in self.store.subjects_in_recorded_order(kind)? {
            out.push((id, revision, parse_record(&value)?));
        }
        Ok(out)
    }

    /// The latest decision about exactly this revision reference.
    fn latest_decision(&self, reference: &Value) -> Result<Option<(String, Value)>, ProtocolError> {
        Ok(self
            .knowledge_records(knowledge::DECISION)?
            .into_iter()
            .filter(|(_, _, d)| {
                d.get("claim")
                    .is_some_and(|c| knowledge::same_reference(c, reference))
            })
            .map(|(id, _, d)| (id, d))
            .next_back())
    }

    /// The latest evaluation of exactly this reference for a target equal to
    /// `target` in canonical JSON.
    fn latest_evaluation(
        &self,
        reference: &Value,
        target: &Value,
    ) -> Result<Option<(String, Value)>, ProtocolError> {
        let wanted = canonical(target);
        Ok(self
            .knowledge_records(knowledge::EVALUATION)?
            .into_iter()
            .filter(|(_, _, e)| {
                e.get("claim")
                    .is_some_and(|c| knowledge::same_reference(c, reference))
                    && e.get("target").map(canonical).as_deref() == Some(wanted.as_str())
            })
            .map(|(id, _, e)| (id, e))
            .next_back())
    }

    /// Commit a knowledge command's one subject change and its event.
    #[allow(clippy::too_many_arguments)] // one call shape shared by every command
    fn commit_knowledge(
        &mut self,
        command: &Command,
        key: &SubjectKey,
        value: &Value,
        new_event: NewEvent,
        claim_revision: Option<ClaimRevision<'_>>,
        recorded_at: &str,
        outcome: Value,
    ) -> Result<Value, ProtocolError> {
        let principal = self.config.principal.clone();
        let value = canonical(value);
        let result = self.store.commit_command(
            Commit {
                key,
                value: &value,
                principal: &principal,
                command_id: &command.command_id,
                digest: &command.command_digest,
                generation: command.dedupe_generation,
                event: Some(new_event),
                also: Vec::new(),
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                more_events: Vec::new(),
                chunks: crate::store::Chunks::None,
                recorded_at,
                claim_revision,
            },
            |revision, operation_ref| {
                accepted(
                    &command.command_id,
                    &command.command_digest,
                    operation_ref,
                    &command.subject,
                    revision,
                    outcome,
                )
            },
        )?;
        Ok(result)
    }

    // ---- claims (section 3) --------------------------------------------------

    pub(super) fn knowledge_claim(
        &mut self,
        params: &Value,
        command: Command,
        revise: bool,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::CLAIM, revise)?;
        knowledge::check_claim_payload(&command.payload, revise)?;
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Rights(vec![("knowledge.propose", Some(key.clone()))]),
        )? {
            return Ok(stored);
        }

        // The base check (KNW-1). The Core precondition has already matched
        // the lineage's current revision; `supersedes` must name that same
        // revision, by number and digest, so a revise can never silently
        // replace or merge a revision its producer did not see.
        let current = self.store.revision(&key)?;
        if revise {
            let Some(base) = self.store.claim_revision(&key.id, current)? else {
                return Err(ProtocolError::not_found());
            };
            let supersedes = command.payload.get("supersedes").unwrap_or(&Value::Null);
            if supersedes.get("revision") != Some(&Value::Int(current)) {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/supersedes/revision",
                    "not the lineage's current revision",
                ));
            }
            if supersedes.get("digest").and_then(Value::as_str) != Some(base.digest.as_str()) {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/supersedes/digest",
                    "not the current revision's digest",
                ));
            }
        }

        let provider = self.provider_id();
        let revision = current + 1;
        let record = knowledge::build_record(
            &provider,
            &key.id,
            revision,
            &self.config.principal,
            &command.payload,
        );
        let digest = knowledge::record_digest(&record);
        let reference = knowledge::reference(&provider, &key.id, revision, &digest);
        let record_text = canonical(&record);
        let recorded_at = self.clock.now();
        let revised = event(
            "knowledge.claim.revised",
            object(vec![("reference", reference.clone())]),
            &command.caused_by,
        );
        self.commit_knowledge(
            &command,
            &key,
            &object(vec![("current", reference.clone())]),
            revised,
            Some(ClaimRevision {
                claim: &key.id,
                revision,
                digest: &digest,
                record: &record_text,
            }),
            &recorded_at,
            object(vec![("reference", reference)]),
        )
    }

    // ---- authority bindings (section 6) --------------------------------------

    pub(super) fn knowledge_authority(
        &mut self,
        params: &Value,
        command: Command,
        transfer: bool,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::AUTHORITY, transfer)?;
        knowledge::check_authority_payload(&command.payload)?;
        if let Some(stored) = self.admit_command(params, &command, Epoch::Unchecked, Step6::Bind)? {
            return Ok(stored);
        }
        // The epoch is the binding subject's revision: bind records 1, each
        // transfer raises it by one, and the two can never disagree.
        let epoch = self.store.revision(&key)? + 1;
        let authority = text(&command.payload, &["authority"]).to_string();
        let value = object(vec![
            ("scope", string(&key.id)),
            ("authority", string(&authority)),
            ("epoch", Value::Int(epoch)),
        ]);
        let recorded_at = self.clock.now();
        let recorded = event(
            if transfer {
                "knowledge.authority.transferred"
            } else {
                "knowledge.authority.bound"
            },
            object(vec![
                ("authority", string(&authority)),
                ("epoch", Value::Int(epoch)),
            ]),
            &command.caused_by,
        );
        self.commit_knowledge(
            &command,
            &key,
            &value,
            recorded,
            None,
            &recorded_at,
            value.clone(),
        )
    }

    pub(super) fn knowledge_authority_get(&self, payload: &Value) -> Result<Value, ProtocolError> {
        let scope = knowledge::check_authority_get_payload(payload)?;
        let Some((revision, binding)) = self.binding(&scope)? else {
            return Err(ProtocolError::not_found());
        };
        Ok(object(vec![
            ("scope", string(&scope)),
            (
                "authority",
                binding.get("authority").cloned().unwrap_or(Value::Null),
            ),
            (
                "epoch",
                binding.get("epoch").cloned().unwrap_or(Value::Null),
            ),
            ("revision", Value::Int(revision)),
        ]))
    }

    // ---- reliance decisions (section 6) --------------------------------------

    pub(super) fn knowledge_decision(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::DECISION, false)?;
        knowledge::check_decision_payload(&command.payload)?;
        Self::require_epoch(&command)?;
        let reference = command.payload.get("claim").cloned().unwrap_or(Value::Null);
        let claim_key = key_of_claim(&reference);
        let epoch = self.epoch_for(&reference)?;
        if let Some(stored) = self.admit_command(
            params,
            &command,
            epoch,
            Step6::Rights(vec![
                ("knowledge.decide", Some(key.clone())),
                ("knowledge.read", Some(claim_key)),
            ]),
        )? {
            return Ok(stored);
        }

        // Step 7, after the epoch and the preconditions, in section 6's order.
        let Some(row) = self.held_revision(&reference)? else {
            return Err(ProtocolError::not_found());
        };
        let scope = text(&row.record, &["scope", "id"]).to_string();
        // Only the bound authority decides. A grant never makes a principal
        // an authority, and a derivation label is never read here.
        let epoch = self.bound_to_caller(&scope)?;
        if knowledge::reference_parts(&reference).digest != row.digest {
            return Err(ProtocolError::invalid_envelope(
                "/payload/claim/digest",
                "the digest differs from the revision's digest",
            ));
        }
        let latest = self.latest_decision(&reference)?.map(|(id, _)| id);
        let named = command
            .payload
            .get("supersedes_decision")
            .and_then(Value::as_str)
            .map(str::to_string);
        if latest != named {
            return Err(
                ProtocolError::new("precondition_failed", Retry::AfterReconcile)
                    .with("latest_decision", latest.map_or(Value::Null, Value::String)),
            );
        }

        let decision = text(&command.payload, &["decision"]).to_string();
        let permitted_use = command.payload.get("permitted_use").cloned();
        let supersedes = command
            .payload
            .get("supersedes_decision")
            .cloned()
            .unwrap_or(Value::Null);
        // The bound authority may adopt its own claim, and it is recorded as
        // exactly that: an adoption by its author, never a separate review.
        let author_is_decider = text(&row.record, &["producer"]) == self.config.principal;
        let recorded_at = self.clock.now();
        let value = object(vec![
            ("claim", reference.clone()),
            ("value", string(&decision)),
            (
                "permitted_use",
                permitted_use.clone().unwrap_or(Value::Null),
            ),
            ("supersedes_decision", supersedes.clone()),
            (
                "validation_basis",
                command
                    .payload
                    .get("validation_basis")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            (
                "rationale",
                command
                    .payload
                    .get("rationale")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            ("decider", string(&self.config.principal)),
            ("author_is_decider", Value::Bool(author_is_decider)),
            ("epoch", Value::Int(epoch)),
            ("scope", string(&scope)),
            ("recorded_at", string(&recorded_at)),
        ]);
        let mut payload = vec![
            ("claim", reference.clone()),
            ("decision", string(&decision)),
        ];
        let mut outcome = vec![
            ("decision", subject(knowledge::DECISION, &key.id)),
            ("claim", reference),
            ("value", string(&decision)),
        ];
        if let Some(used) = &permitted_use {
            payload.push(("permitted_use", used.clone()));
            outcome.push(("permitted_use", used.clone()));
        }
        payload.push(("author_is_decider", Value::Bool(author_is_decider)));
        payload.push(("supersedes_decision", supersedes));
        outcome.push(("author_is_decider", Value::Bool(author_is_decider)));
        outcome.push(("epoch", Value::Int(epoch)));
        let recorded = event(
            "knowledge.decision.recorded",
            object(payload),
            &command.caused_by,
        );
        self.commit_knowledge(
            &command,
            &key,
            &value,
            recorded,
            None,
            &recorded_at,
            object(outcome),
        )
    }

    // ---- conflicts (section 7) -----------------------------------------------

    pub(super) fn knowledge_conflict_open(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::CONFLICT, false)?;
        knowledge::check_conflict_open_payload(&command.payload)?;
        let revisions: Vec<Value> = command
            .payload
            .get("revisions")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .to_vec();
        let mut needs = vec![("knowledge.propose", Some(key.clone()))];
        for reference in &revisions {
            needs.push(("knowledge.read", Some(key_of_claim(reference))));
        }
        if let Some(stored) =
            self.admit_command(params, &command, Epoch::Unchecked, Step6::Rights(needs))?
        {
            return Ok(stored);
        }

        let mut records = Vec::new();
        for (index, reference) in revisions.iter().enumerate() {
            let Some(row) = self.held_revision(reference)? else {
                return Err(ProtocolError::not_found());
            };
            if knowledge::reference_parts(reference).digest != row.digest {
                return Err(ProtocolError::invalid_envelope(
                    &format!("/payload/revisions/{index}/digest"),
                    "the digest differs from the revision's digest",
                ));
            }
            records.push(row.record);
        }
        let comparison = knowledge::compare(&records[0], &records[1]).map_err(|reason| {
            ProtocolError::new("claims_not_comparable", Retry::No).with("reason", string(reason))
        })?;
        let uncertain = Value::Array(comparison.uncertain.iter().map(|u| string(u)).collect());
        let recorded_at = self.clock.now();
        let value = object(vec![
            ("kind", string(comparison.kind)),
            ("status", string(comparison.status)),
            ("uncertain", uncertain.clone()),
            ("revisions", Value::Array(revisions.clone())),
            // While a record is open its competing values are all listed.
            (
                "values",
                Value::Array(
                    records
                        .iter()
                        .map(|r| {
                            r.get("statement")
                                .and_then(|s| s.get("value"))
                                .cloned()
                                .unwrap_or(Value::Null)
                        })
                        .collect(),
                ),
            ),
            (
                "note",
                command.payload.get("note").cloned().unwrap_or(Value::Null),
            ),
            ("state", string("open")),
            ("resolution", Value::Null),
            ("selected", Value::Null),
            ("rationale", Value::Null),
            ("recorded_at", string(&recorded_at)),
        ]);
        let opened = event(
            "knowledge.conflict.opened",
            object(vec![
                ("kind", string(comparison.kind)),
                ("status", string(comparison.status)),
                ("revisions", Value::Array(revisions)),
            ]),
            &command.caused_by,
        );
        let outcome = object(vec![
            ("conflict", subject(knowledge::CONFLICT, &key.id)),
            ("kind", string(comparison.kind)),
            ("status", string(comparison.status)),
            ("uncertain", uncertain),
        ]);
        self.commit_knowledge(&command, &key, &value, opened, None, &recorded_at, outcome)
    }

    pub(super) fn knowledge_conflict_resolve(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::CONFLICT, true)?;
        knowledge::check_conflict_resolve_payload(&command.payload)?;
        Self::require_epoch(&command)?;
        let existing = self
            .store
            .subject(&key)?
            .map(|state| parse_record(&state.value))
            .transpose()?;
        let first = existing
            .as_ref()
            .and_then(|c| c.get("revisions"))
            .and_then(Value::as_array)
            .and_then(|r| r.first())
            .cloned();
        let epoch = match &first {
            Some(reference) => self.epoch_for(reference)?,
            None => Epoch::Unchecked,
        };
        if let Some(stored) = self.admit_command(
            params,
            &command,
            epoch,
            Step6::Rights(vec![("knowledge.decide", Some(key.clone()))]),
        )? {
            return Ok(stored);
        }

        let Some(mut conflict) = existing.filter(|c| text(c, &["state"]) == "open") else {
            return Err(ProtocolError::not_found());
        };
        let scope = match first.as_ref().map(|r| self.held_revision(r)).transpose()? {
            Some(Some(row)) => text(&row.record, &["scope", "id"]).to_string(),
            _ => String::new(),
        };
        self.bound_to_caller(&scope)?;
        let resolution = text(&command.payload, &["resolution"]).to_string();
        let selected = command.payload.get("selected");
        let names_one = selected.is_some_and(|s| {
            conflict
                .get("revisions")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .any(|r| canonical(r) == canonical(s))
        });
        let valid = if resolution == "select" {
            names_one
        } else {
            selected.is_none()
        };
        if !valid {
            return Err(ProtocolError::invalid_envelope(
                "/payload/selected",
                "select names one of the two revisions, and only select names one",
            ));
        }

        let recorded_at = self.clock.now();
        if let Value::Object(members) = &mut conflict {
            for (name, value) in [
                ("state", string("resolved")),
                ("resolution", string(&resolution)),
                ("selected", selected.cloned().unwrap_or(Value::Null)),
                (
                    "rationale",
                    command
                        .payload
                        .get("rationale")
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
                ("resolved_by", string(&self.config.principal)),
                ("resolved_at", string(&recorded_at)),
            ] {
                match members.iter_mut().find(|(n, _)| n == name) {
                    Some((_, slot)) => *slot = value,
                    None => members.push((name.to_string(), value)),
                }
            }
        }
        let resolved = event(
            "knowledge.conflict.resolved",
            object(vec![("resolution", string(&resolution))]),
            &command.caused_by,
        );
        let outcome = object(vec![
            ("conflict", subject(knowledge::CONFLICT, &key.id)),
            ("state", string("resolved")),
            ("resolution", string(&resolution)),
        ]);
        self.commit_knowledge(
            &command,
            &key,
            &conflict,
            resolved,
            None,
            &recorded_at,
            outcome,
        )
    }

    // ---- applicability (section 8) -------------------------------------------

    pub(super) fn knowledge_evaluate(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let key = self.knowledge_shape(&command, knowledge::EVALUATION, false)?;
        knowledge::check_evaluate_payload(&command.payload)?;
        let reference = command.payload.get("claim").cloned().unwrap_or(Value::Null);
        let target = command
            .payload
            .get("target")
            .cloned()
            .unwrap_or(Value::Null);

        // Evaluating reads every local claim its dependencies name, because
        // the findings reveal whether those revisions exist here (section 10).
        let mut needs = vec![
            ("knowledge.propose", Some(key.clone())),
            ("knowledge.read", Some(key_of_claim(&reference))),
        ];
        let named = self.held_revision(&reference)?;
        if let Some(row) = &named {
            for dependency in row
                .record
                .get("dependencies")
                .and_then(Value::as_array)
                .unwrap_or_default()
            {
                if text(dependency, &["provider"]) == self.config.provider_id {
                    needs.push(("knowledge.read", Some(key_of_claim(dependency))));
                }
            }
        }
        if let Some(stored) =
            self.admit_command(params, &command, Epoch::Unchecked, Step6::Rights(needs))?
        {
            return Ok(stored);
        }

        let Some(row) = self.held_revision(&reference)? else {
            return Err(ProtocolError::not_found());
        };
        if knowledge::reference_parts(&reference).digest != row.digest {
            return Err(ProtocolError::invalid_envelope(
                "/payload/claim/digest",
                "the digest differs from the revision's digest",
            ));
        }

        let mut findings = knowledge::condition_findings(
            &row.record,
            &target,
            &self.config.knowledge.condition_kinds,
        );
        for dependency in row
            .record
            .get("dependencies")
            .and_then(Value::as_array)
            .unwrap_or_default()
        {
            let (finding, reason) = self.dependency_finding(&row.record, dependency, &target)?;
            let mut entry = vec![
                ("dependency", dependency.clone()),
                ("finding", string(finding)),
            ];
            if let Some(reason) = reason {
                entry.push(("reason", string(reason)));
            }
            findings.push(object(entry));
        }
        let result = knowledge::result_of(&findings);
        let evaluator = object(vec![
            ("id", string(&self.config.knowledge.evaluator_id)),
            ("version", string(&self.config.knowledge.evaluator_version)),
        ]);
        let supersedes = self
            .latest_evaluation(&reference, &target)?
            .map_or(Value::Null, |(id, _)| Value::String(id));
        let recorded_at = self.clock.now();
        let value = object(vec![
            ("claim", reference.clone()),
            ("target", target),
            ("evaluator", evaluator.clone()),
            ("result", string(result)),
            ("findings", Value::Array(findings.clone())),
            ("supersedes_evaluation", supersedes.clone()),
            ("recorded_at", string(&recorded_at)),
        ]);
        let recorded = event(
            "knowledge.evaluation.recorded",
            object(vec![
                ("claim", reference.clone()),
                ("result", string(result)),
                ("evaluator", evaluator.clone()),
                ("supersedes_evaluation", supersedes.clone()),
            ]),
            &command.caused_by,
        );
        let outcome = object(vec![
            ("evaluation", subject(knowledge::EVALUATION, &key.id)),
            ("claim", reference),
            ("result", string(result)),
            ("evaluator", evaluator),
            ("findings", Value::Array(findings)),
            ("supersedes_evaluation", supersedes),
        ]);
        self.commit_knowledge(
            &command,
            &key,
            &value,
            recorded,
            None,
            &recorded_at,
            outcome,
        )
    }

    /// One dependency's finding. A dependency resolves only as an **exact
    /// local reference**, and gets its finding only from the latest evaluation
    /// of that exact reference for an equal target; the reasons are checked in
    /// the order of section 8's table.
    fn dependency_finding(
        &self,
        record: &Value,
        dependency: &Value,
        target: &Value,
    ) -> Result<(&'static str, Option<&'static str>), ProtocolError> {
        let parts = knowledge::reference_parts(dependency);
        if parts.provider != self.config.provider_id {
            return Ok(("unchecked", Some("remote_dependency")));
        }
        let own_revision = match record.get("revision") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        };
        if parts.claim == text(record, &["claim"]) && parts.revision == own_revision {
            return Ok(("unchecked", Some("self_reference")));
        }
        let held = self
            .store
            .claim_revision(parts.claim, parts.revision)?
            .is_some_and(|row| row.digest == parts.digest);
        if !held {
            return Ok(("unchecked", Some("unresolved")));
        }
        Ok(match self.latest_evaluation(dependency, target)? {
            None => ("unchecked", Some("not_evaluated")),
            Some((_, evaluation)) => match text(&evaluation, &["result"]) {
                "applicable" => ("match", None),
                "invalid_for_target" => ("mismatch", None),
                _ => ("unchecked", Some("not_established")),
            },
        })
    }

    // ---- reads (sections 4 and 9) --------------------------------------------

    pub(super) fn knowledge_inspect(&self, payload: &Value) -> Result<Value, ProtocolError> {
        let (claim, revision) = knowledge::check_inspect_payload(payload)?;
        let current = self.store.revision(&key(knowledge::CLAIM, &claim))?;
        if current == 0 {
            return Err(ProtocolError::not_found());
        }
        let Some(row) = self
            .store
            .claim_revision(&claim, revision.unwrap_or(current))?
        else {
            return Err(ProtocolError::not_found());
        };
        let reference = knowledge::reference(
            text(&row.record, &["provider"]),
            &claim,
            row.revision,
            &row.digest,
        );

        // Each facet from its own records; none is derived from another.
        let reliance = match self.latest_decision(&reference)? {
            Some((id, decision)) => {
                let mut members = vec![
                    (
                        "state",
                        decision.get("value").cloned().unwrap_or(Value::Null),
                    ),
                    ("decision", string(&id)),
                ];
                if let Some(used @ Value::String(_)) = decision.get("permitted_use") {
                    members.push(("permitted_use", used.clone()));
                }
                members.push((
                    "author_is_decider",
                    decision
                        .get("author_is_decider")
                        .cloned()
                        .unwrap_or(Value::Bool(false)),
                ));
                object(members)
            }
            None => object(vec![("state", string("proposed"))]),
        };

        let mut applicability: Vec<Value> = Vec::new();
        for (id, _, evaluation) in self.knowledge_records(knowledge::EVALUATION)? {
            if !evaluation
                .get("claim")
                .is_some_and(|c| knowledge::same_reference(c, &reference))
            {
                continue;
            }
            let target = evaluation.get("target").cloned().unwrap_or(Value::Null);
            let item = object(vec![
                ("evaluation", string(&id)),
                ("target", target.clone()),
                (
                    "result",
                    evaluation.get("result").cloned().unwrap_or(Value::Null),
                ),
                (
                    "evaluator",
                    evaluation.get("evaluator").cloned().unwrap_or(Value::Null),
                ),
            ]);
            let wanted = canonical(&target);
            match applicability
                .iter_mut()
                .find(|a| a.get("target").map(canonical).as_deref() == Some(wanted.as_str()))
            {
                Some(slot) => *slot = item,
                None => applicability.push(item),
            }
        }

        let support = row
            .record
            .get("support")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .to_vec();
        let (class, unknown_ancestry) = knowledge::support_class(&support);
        let mut observed = Vec::new();
        for entry in &support {
            observed.push(self.observe_support(entry)?);
        }

        let mut conflicts = Vec::new();
        for (id, _, conflict) in self.knowledge_records(knowledge::CONFLICT)? {
            let names_this = conflict
                .get("revisions")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .any(|r| knowledge::same_reference(r, &reference));
            if names_this {
                conflicts.push(object(vec![
                    ("conflict", string(&id)),
                    ("kind", conflict.get("kind").cloned().unwrap_or(Value::Null)),
                    (
                        "status",
                        conflict.get("status").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "state",
                        conflict.get("state").cloned().unwrap_or(Value::Null),
                    ),
                ]));
            }
        }

        let mut record = row.record.clone();
        if self.config.knowledge.serve_altered_claims.contains(&claim) {
            // Adversarial test control: an altered record behind an honest
            // reference and digest, so readers are tested on recomputing.
            alter_statement_value(&mut record);
        }
        Ok(object(vec![
            ("reference", reference),
            ("record", record),
            ("current_revision", Value::Int(current)),
            ("reliance", reliance),
            ("applicability", Value::Array(applicability)),
            (
                "health",
                row.record.get("health").cloned().unwrap_or(Value::Null),
            ),
            ("availability", knowledge::availability(&observed)),
            (
                "support",
                object(vec![
                    ("class", string(class)),
                    (
                        "unknown_ancestry",
                        Value::Array(unknown_ancestry.iter().map(|s| string(s)).collect()),
                    ),
                ]),
            ),
            ("conflicts", Value::Array(conflicts)),
        ]))
    }

    /// One support artifact as this provider observes it at the read.
    fn observe_support(&self, entry: &Value) -> Result<Observed, ProtocolError> {
        let evidence = entry.get("evidence").unwrap_or(&Value::Null);
        if text(evidence, &["provider"]) != self.config.provider_id {
            return Ok(Observed::Unknown);
        }
        let id = text(evidence, &["artifact", "id"]);
        let Some((_, record)) = self.evidence_record(crate::evidence::ARTIFACT, id)? else {
            return Ok(Observed::Unavailable);
        };
        if text(&record, &["descriptor", "digest"]) != text(evidence, &["digest"]) {
            return Ok(Observed::Unavailable);
        }
        let availability = self.availability_of(id, &record)?;
        Ok(match text(&availability, &["state"]) {
            "available" => Observed::Available,
            "purged" => Observed::Purged,
            _ => Observed::Unavailable,
        })
    }

    pub(super) fn knowledge_history(&self, payload: &Value) -> Result<Value, ProtocolError> {
        let claim = knowledge::check_history_payload(payload)?;
        let rows = self.store.claim_revisions(&claim)?;
        if rows.is_empty() {
            return Err(ProtocolError::not_found());
        }
        let position = |event_type: &str, subject_key: &SubjectKey, revision: i64| {
            self.store
                .event_position(event_type, subject_key, revision)
                .map(|p| p.map_or(Value::Null, |p| p.to_value()))
        };
        let claim_key = key(knowledge::CLAIM, &claim);
        let mut revisions = Vec::new();
        for row in &rows {
            revisions.push(object(vec![
                (
                    "reference",
                    knowledge::reference(
                        text(&row.record, &["provider"]),
                        &claim,
                        row.revision,
                        &row.digest,
                    ),
                ),
                (
                    "producer",
                    row.record.get("producer").cloned().unwrap_or(Value::Null),
                ),
                (
                    "supersedes",
                    row.record.get("supersedes").cloned().unwrap_or(Value::Null),
                ),
                ("recorded_at", string(&row.recorded_at)),
                (
                    "position",
                    position("knowledge.claim.revised", &claim_key, row.revision)?,
                ),
            ]));
        }
        let about_this = |reference: Option<&Value>| {
            reference.is_some_and(|r| {
                text(r, &["claim"]) == claim && text(r, &["provider"]) == self.config.provider_id
            })
        };
        let field = |record: &Value, name: &str| record.get(name).cloned().unwrap_or(Value::Null);

        let mut decisions = Vec::new();
        for (id, _, d) in self.knowledge_records(knowledge::DECISION)? {
            if !about_this(d.get("claim")) {
                continue;
            }
            decisions.push(object(vec![
                ("decision", string(&id)),
                ("claim", field(&d, "claim")),
                ("value", field(&d, "value")),
                ("permitted_use", field(&d, "permitted_use")),
                ("decider", field(&d, "decider")),
                ("author_is_decider", field(&d, "author_is_decider")),
                ("epoch", field(&d, "epoch")),
                ("supersedes_decision", field(&d, "supersedes_decision")),
                ("recorded_at", field(&d, "recorded_at")),
                (
                    "position",
                    position(
                        "knowledge.decision.recorded",
                        &key(knowledge::DECISION, &id),
                        1,
                    )?,
                ),
            ]));
        }
        let mut evaluations = Vec::new();
        for (id, _, e) in self.knowledge_records(knowledge::EVALUATION)? {
            if !about_this(e.get("claim")) {
                continue;
            }
            evaluations.push(object(vec![
                ("evaluation", string(&id)),
                ("claim", field(&e, "claim")),
                ("target", field(&e, "target")),
                ("result", field(&e, "result")),
                ("evaluator", field(&e, "evaluator")),
                ("supersedes_evaluation", field(&e, "supersedes_evaluation")),
                ("recorded_at", field(&e, "recorded_at")),
                (
                    "position",
                    position(
                        "knowledge.evaluation.recorded",
                        &key(knowledge::EVALUATION, &id),
                        1,
                    )?,
                ),
            ]));
        }
        let mut conflicts = Vec::new();
        for (id, _, c) in self.knowledge_records(knowledge::CONFLICT)? {
            let names_this = c
                .get("revisions")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .any(|r| about_this(Some(r)));
            if !names_this {
                continue;
            }
            // A resolved conflict appears once, with its resolution.
            conflicts.push(object(vec![
                ("conflict", string(&id)),
                ("kind", field(&c, "kind")),
                ("status", field(&c, "status")),
                ("revisions", field(&c, "revisions")),
                ("state", field(&c, "state")),
                ("resolution", field(&c, "resolution")),
                ("recorded_at", field(&c, "recorded_at")),
                (
                    "position",
                    position(
                        "knowledge.conflict.opened",
                        &key(knowledge::CONFLICT, &id),
                        1,
                    )?,
                ),
            ]));
        }
        Ok(object(vec![
            ("claim", string(&claim)),
            ("revisions", Value::Array(revisions)),
            ("decisions", Value::Array(decisions)),
            ("evaluations", Value::Array(evaluations)),
            ("conflicts", Value::Array(conflicts)),
        ]))
    }
}

/// The claim subject a reference names.
pub(super) fn key_of_claim(reference: &Value) -> SubjectKey {
    key(knowledge::CLAIM, text(reference, &["claim"]))
}

fn alter_statement_value(record: &mut Value) {
    if let Value::Object(members) = record
        && let Some((_, Value::Object(statement))) =
            members.iter_mut().find(|(name, _)| name == "statement")
        && let Some((_, value)) = statement.iter_mut().find(|(name, _)| name == "value")
    {
        let original = std::mem::replace(value, Value::Null);
        *value = Value::Object(vec![("altered".into(), original)]);
    }
}
