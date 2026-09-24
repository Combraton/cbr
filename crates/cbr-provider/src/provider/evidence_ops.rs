//! `evidence/1` operations on the provider (EVIDENCE sections 4 to 10).
//!
//! Every command follows the Core command path through `admit_command`, so
//! digest, deduplication, authorization and preconditions happen in CORE
//! section 10's order before anything here reads an artifact's state. The
//! order of durable writes is the part worth reading slowly:
//!
//! - **Append** commits the bytes and the new `received` in one transaction.
//! - **Seal** verifies the staged bytes, **publishes the object and verifies
//!   it from disk first**, then commits the row that names it. A crash between
//!   the two leaves an object nothing names, which is never served.
//! - **Purge** commits the row first and deletes the object after, the
//!   reverse order for the reverse operation: a crash between leaves an object
//!   no available row names, never an available row naming a missing object.

use cbr_encoding::Value;

use super::{Epoch, Provider, Step6, accepted, key_of};
use crate::errors::ProtocolError;
use crate::evidence::{self, Child};
use crate::grants::{self, Grant};
use crate::store::{Chunks, Commit, NewEvent, SubjectKey};

fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("canonical form is UTF-8")
}

fn member<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.get(name)
}

fn set(value: &mut Value, name: &str, new: Value) {
    if let Value::Object(members) = value {
        match members.iter_mut().find(|(key, _)| key == name) {
            Some((_, slot)) => *slot = new,
            None => members.push((name.to_string(), new)),
        }
    }
}

fn remove(value: &mut Value, name: &str) {
    if let Value::Object(members) = value {
        members.retain(|(key, _)| key != name);
    }
}

fn text(value: &Value, name: &str) -> Option<String> {
    value.get(name).and_then(Value::as_str).map(str::to_string)
}

fn int(value: &Value, name: &str) -> i64 {
    match value.get(name) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    }
}

fn subject(kind: &str, id: &str) -> Value {
    Value::Object(vec![
        ("kind".into(), Value::String(kind.into())),
        ("id".into(), Value::String(id.into())),
    ])
}

fn event(event_type: &str, payload: Value, caused_by: &[String]) -> NewEvent {
    NewEvent {
        event_type: event_type.into(),
        caused_by: caused_by.to_vec(),
        payload: Box::new(move |_| payload),
    }
}

/// Add `seconds` to a UTC instant.
fn plus_seconds(instant: &str, seconds: i64) -> String {
    let parse = |from: usize, to: usize| instant[from..to].parse::<i64>().unwrap_or(0);
    let (year, month, day) = (parse(0, 4), parse(5, 7), parse(8, 10));
    let (hour, minute, second) = (parse(11, 13), parse(14, 16), parse(17, 19));
    // Days from civil, Howard Hinnant's algorithm, the inverse of `format_unix`.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let total = days * 86_400 + hour * 3600 + minute * 60 + second + seconds;
    crate::clock::format_unix(total.max(0) as u64)
}

impl Provider {
    // ---- records -----------------------------------------------------------

    pub(super) fn evidence_record(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Option<(i64, Value)>, ProtocolError> {
        let key = SubjectKey {
            kind: kind.into(),
            id: id.into(),
        };
        let Some(state) = self.store.subject(&key)? else {
            return Ok(None);
        };
        let record = cbr_encoding::parse(state.value.as_bytes())
            .map_err(|_| ProtocolError::new_internal_error())?;
        Ok(Some((state.revision, record)))
    }

    /// Every hold on an artifact, in hold id order, with its revision.
    fn holds_on(&self, artifact: &str) -> Result<Vec<(String, i64, Value)>, ProtocolError> {
        let mut holds = Vec::new();
        for (id, _) in self.store.subjects_of_kind(evidence::HOLD)? {
            if let Some((revision, record)) = self.evidence_record(evidence::HOLD, &id)?
                && text(&record, "artifact").as_deref() == Some(artifact)
            {
                holds.push((id, revision, record));
            }
        }
        Ok(holds)
    }

    /// Whether a hold is visible: to its owner, to authority principals, and to
    /// any reader of the held artifact (EVIDENCE section 9).
    pub(super) fn hold_visible(&self, in_force: Option<&Grant>, hold: &Value) -> bool {
        let Some(grant) = in_force else {
            return true;
        };
        text(hold, "owner").as_deref() == Some(self.config.principal.as_str())
            || text(hold, "artifact").is_some_and(|artifact| {
                grant.may_read(&evidence::artifact_key(&artifact), "evidence.read")
            })
    }

    /// Availability as observed now, including an integrity check of the
    /// stored bytes: a read reports a failure, it never records one.
    pub(super) fn availability_of(&self, id: &str, record: &Value) -> Result<Value, ProtocolError> {
        let state = text(record, "state").unwrap_or_default();
        if state == "abandoned" {
            let reason = text(record, "abandoned_reason");
            return Ok(evidence::availability("unavailable", reason.as_deref()));
        }
        if let Some(purge) = member(record, "purge") {
            return Ok(if purge.get("confirmed_at").is_some() {
                evidence::availability("purged", None)
            } else {
                evidence::availability("purge_pending", None)
            });
        }
        if state == "staged" {
            return Ok(evidence::availability("available", None));
        }
        if self
            .config
            .evidence_store
            .unavailable
            .iter()
            .any(|u| u == id)
        {
            return Ok(evidence::availability(
                "unavailable",
                Some("store_unavailable"),
            ));
        }
        let digest = member(record, "descriptor")
            .and_then(|d| text(d, "digest"))
            .unwrap_or_default();
        let intact = self
            .store
            .read_object(&digest)?
            .is_some_and(|bytes| cbr_encoding::digest_bytes(&bytes) == digest);
        Ok(if intact {
            evidence::availability("available", None)
        } else {
            evidence::availability("unavailable", Some("integrity_failed"))
        })
    }

    // ---- authorization -----------------------------------------------------

    /// Step 6 for a publish operation: `evidence.publish` over the artifact,
    /// then the work binding, after rights and scope (EVIDENCE section 10).
    pub(super) fn authorize_publish(
        &self,
        named: Option<&str>,
        artifact: &str,
        work: Option<&Value>,
    ) -> Result<Option<Grant>, ProtocolError> {
        let needs = [("evidence.publish", Some(evidence::artifact_key(artifact)))];
        let in_force = self.authorize(named, &needs)?;
        if let Some(grant) = &in_force {
            for constraint in &grant.constraints {
                if constraint.get("kind").and_then(Value::as_str) == Some("evidence.work_binding")
                    && constraint.get("work") != work
                {
                    return Err(grants::Denial::BindingViolation.into());
                }
            }
        }
        Ok(in_force)
    }

    /// Release authority over one hold: its owner, an authority principal, or
    /// a grant with `evidence.release` covering the hold subject.
    pub(super) fn authorize_release(
        &self,
        named: Option<&str>,
        hold: &str,
    ) -> Result<Option<Grant>, ProtocolError> {
        if let Some((_, record)) = self.evidence_record(evidence::HOLD, hold)?
            && text(&record, "owner").as_deref() == Some(self.config.principal.as_str())
        {
            return Ok(None);
        }
        self.authorize(
            named,
            &[("evidence.release", Some(evidence::hold_key(hold)))],
        )
    }

    /// Step 6 for a purge: `evidence.purge` on the artifact and release
    /// authority for **every** named hold. Naming a hold is intent, never
    /// authorization.
    pub(super) fn authorize_purge(
        &self,
        named: Option<&str>,
        artifact: &str,
        holds: &[String],
    ) -> Result<Option<Grant>, ProtocolError> {
        let in_force = self.authorize(
            named,
            &[("evidence.purge", Some(evidence::artifact_key(artifact)))],
        )?;
        for hold in holds {
            self.authorize_release(named, hold)?;
        }
        Ok(in_force)
    }

    fn retention_gate(&self) -> Result<(), ProtocolError> {
        if self.selected_feature("evidence.retention_control") {
            Ok(())
        } else {
            Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(vec![Value::String("evidence.retention_control".into())]),
            ))
        }
    }

    /// The single precondition an evidence command carries on its subject.
    fn one_precondition<T>(&self, key: &SubjectKey, kind: &str) -> Result<T, ProtocolError> {
        Err(ProtocolError::invalid_envelope(
            "/preconditions",
            &format!(
                "exactly one precondition on the {kind} subject {} is required",
                key.id
            ),
        ))
    }

    // One place every evidence command commits through; its arguments are the
    // parts of a `Commit` that differ between them.
    #[allow(clippy::too_many_arguments)]
    fn commit_for(
        &mut self,
        command: &crate::envelope::Command,
        key: &SubjectKey,
        record: &Value,
        events: Vec<NewEvent>,
        chunks: Chunks<'_>,
        also: Vec<crate::store::Change>,
        outcome: Value,
    ) -> Result<Value, ProtocolError> {
        let value = canonical(record);
        let principal = self.config.principal.clone();
        let recorded_at = self.clock.now();
        let digest = command.command_digest.clone();
        let subject = command.subject.clone();
        let command_id = command.command_id.clone();
        let mut events = events.into_iter();
        let first = events.next();
        let more: Vec<NewEvent> = events.collect();
        let result = self.store.commit_command(
            Commit {
                key,
                value: &value,
                principal: &principal,
                command_id: &command_id,
                digest: &digest,
                generation: command.dedupe_generation,
                recorded_at: &recorded_at,
                also,
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                more_events: more,
                chunks,
                event: first,
                claim_revision: None,
            },
            |revision, operation_ref| {
                accepted(
                    &command_id,
                    &digest,
                    operation_ref,
                    &subject,
                    revision,
                    outcome,
                )
            },
        )?;
        Ok(result)
    }

    /// A command that changes nothing still binds its identity, so its outcome
    /// replays; the acknowledgment names the unchanged revision.
    fn bind_unchanged(
        &mut self,
        command: &crate::envelope::Command,
        revision: i64,
        outcome: Value,
    ) -> Result<Value, ProtocolError> {
        let operation_ref = format!("unchanged-{}", command.command_id);
        let result = accepted(
            &command.command_id,
            &command.command_digest,
            &operation_ref,
            &command.subject,
            revision,
            outcome,
        );
        let principal = self.config.principal.clone();
        self.store.bind_command_only(
            &principal,
            &command.command_id,
            &command.command_digest,
            command.dedupe_generation,
            &result,
        )?;
        Ok(result)
    }

    // ---- upload and seal (section 4) ---------------------------------------

    pub(super) fn evidence_prepare(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        let key = key_of(&command.subject);
        if key.kind != evidence::ARTIFACT
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "artifact");
        }
        let descriptor = evidence::parse_descriptor(&command.payload, &self.config.principal)?;
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Publish {
                artifact: key.id.clone(),
                work: descriptor.get("work").cloned(),
            },
        )? {
            return Ok(stored);
        }
        let limits = &self.config.limits;
        let chunk_limit = evidence::chunk_limit(
            limits.max_payload_bytes,
            limits.max_frame_bytes,
            limits.max_string_bytes,
        );
        let record = Value::Object(vec![
            ("descriptor".into(), descriptor.clone()),
            ("state".into(), Value::String("staged".into())),
            ("received".into(), Value::Int(0)),
            ("staged_at".into(), Value::String(self.clock.now())),
        ]);
        let outcome = Value::Object(vec![
            ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
            ("state".into(), Value::String("staged".into())),
            ("received".into(), Value::Int(0)),
            ("chunk_limit".into(), Value::Int(chunk_limit)),
            (
                "chunk_overhead_bytes".into(),
                Value::Int(evidence::CHUNK_OVERHEAD_BYTES),
            ),
        ]);
        let staged = event(
            "evidence.artifact.staged",
            Value::Object(vec![("descriptor".into(), descriptor)]),
            &command.caused_by,
        );
        self.commit_for(
            &command,
            &key,
            &record,
            vec![staged],
            Chunks::None,
            Vec::new(),
            outcome,
        )
    }

    pub(super) fn evidence_append(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        let key = key_of(&command.subject);
        if key.kind != evidence::ARTIFACT
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "artifact");
        }
        // Step 2, with the other limits: before authorization and before the
        // preconditions, because it names no subject.
        let offset = match command.payload.get("offset") {
            Some(Value::Int(n)) if *n >= 0 => *n,
            _ => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/offset",
                    "not a non-negative integer",
                ));
            }
        };
        let Some(bytes) = command
            .payload
            .get("data_base64")
            .and_then(Value::as_str)
            .and_then(evidence::decode_base64)
        else {
            return Err(ProtocolError::invalid_envelope(
                "/payload/data_base64",
                "not valid padded base64",
            ));
        };
        let limits = &self.config.limits;
        let chunk_limit = evidence::chunk_limit(
            limits.max_payload_bytes,
            limits.max_frame_bytes,
            limits.max_string_bytes,
        );
        if bytes.len() as i64 > chunk_limit {
            return Err(ProtocolError::limit_exceeded("chunk_limit", chunk_limit));
        }
        let existing = self.evidence_record(evidence::ARTIFACT, &key.id)?;
        let work = existing
            .as_ref()
            .and_then(|(_, r)| member(r, "descriptor").and_then(|d| d.get("work")).cloned());
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Publish {
                artifact: key.id.clone(),
                work,
            },
        )? {
            return Ok(stored);
        }
        let Some((_, mut record)) = self.evidence_record(evidence::ARTIFACT, &key.id)? else {
            return Err(ProtocolError::not_found());
        };
        if text(&record, "state").as_deref() != Some("staged") {
            return Err(ProtocolError::not_found());
        }
        let received = int(&record, "received");
        let size = member(&record, "descriptor").map_or(0, |d| int(d, "size"));
        if offset != received {
            return Err(ProtocolError::new(
                "upload_offset_mismatch",
                crate::errors::Retry::AfterReconcile,
            )
            .with("received", Value::Int(received)));
        }
        if received + bytes.len() as i64 > size {
            return Err(
                ProtocolError::new("upload_size_exceeded", crate::errors::Retry::No)
                    .with("received", Value::Int(received))
                    .with("size", Value::Int(size)),
            );
        }
        let now_received = received + bytes.len() as i64;
        set(&mut record, "received", Value::Int(now_received));
        let appended = event(
            "evidence.artifact.appended",
            Value::Object(vec![("received".into(), Value::Int(now_received))]),
            &command.caused_by,
        );
        let outcome = Value::Object(vec![
            ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
            ("received".into(), Value::Int(now_received)),
        ]);
        crate::barriers::pause("evidence.append.before_commit");
        let artifact = key.id.clone();
        let result = self.commit_for(
            &command,
            &key,
            &record,
            vec![appended],
            Chunks::Append {
                artifact: &artifact,
                offset,
                bytes: &bytes,
            },
            Vec::new(),
            outcome,
        )?;
        // Committed, and the acknowledgment not yet on its way: the boundary
        // at which a producer loses the response and must inspect.
        crate::barriers::pause("evidence.append.after_commit");
        Ok(result)
    }

    pub(super) fn evidence_seal(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        let key = key_of(&command.subject);
        if key.kind != evidence::ARTIFACT
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "artifact");
        }
        let existing = self.evidence_record(evidence::ARTIFACT, &key.id)?;
        let work = existing
            .as_ref()
            .and_then(|(_, r)| member(r, "descriptor").and_then(|d| d.get("work")).cloned());
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Publish {
                artifact: key.id.clone(),
                work,
            },
        )? {
            return Ok(stored);
        }
        let Some((revision, mut record)) = self.evidence_record(evidence::ARTIFACT, &key.id)?
        else {
            return Err(ProtocolError::not_found());
        };
        let descriptor = member(&record, "descriptor")
            .cloned()
            .unwrap_or(Value::Null);
        let digest = text(&descriptor, "digest").unwrap_or_default();
        let size = int(&descriptor, "size");
        let outcome = |already: bool| {
            Value::Object(vec![
                ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
                ("state".into(), Value::String("sealed".into())),
                ("digest".into(), Value::String(digest.clone())),
                ("size".into(), Value::Int(size)),
                ("already_sealed".into(), Value::Bool(already)),
            ])
        };
        match text(&record, "state").as_deref() {
            // A new seal on a sealed artifact changes nothing and says so.
            Some("sealed") => return self.bind_unchanged(&command, revision, outcome(true)),
            Some("staged") => {}
            _ => return Err(ProtocolError::not_found()),
        }
        let received = int(&record, "received");
        if received != size {
            return Err(ProtocolError::new(
                "upload_incomplete",
                crate::errors::Retry::AfterReconcile,
            )
            .with("received", Value::Int(received))
            .with("size", Value::Int(size)));
        }
        let bytes = self.store.staged_bytes(&key.id)?;
        let computed = cbr_encoding::digest_bytes(&bytes);
        if computed != digest {
            return Err(
                ProtocolError::new("content_digest_mismatch", crate::errors::Retry::No)
                    .with("computed", Value::String(computed)),
            );
        }
        let is_manifest = text(&descriptor, "media_type").as_deref()
            == Some(evidence::MANIFEST_MEDIA_TYPE)
            && self.selected_feature("evidence.manifests");
        let children = if is_manifest {
            match evidence::parse_manifest(&bytes) {
                Some(children) => Some(children),
                None => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload",
                        "not a valid manifest of a supported format",
                    ));
                }
            }
        } else {
            None
        };

        // The object first, verified from disk, then the row that names it.
        self.store.publish_object(&digest, &bytes)?;
        crate::barriers::pause("evidence.seal.after_object_published");

        set(&mut record, "state", Value::String("sealed".into()));
        remove(&mut record, "received");
        if let Some(children) = children {
            set(
                &mut record,
                "manifest_children",
                Value::Array(children.iter().map(Child::to_value).collect()),
            );
        }
        let sealed = event(
            "evidence.artifact.sealed",
            Value::Object(vec![
                ("digest".into(), Value::String(digest.clone())),
                ("size".into(), Value::Int(size)),
            ]),
            &command.caused_by,
        );
        let artifact = key.id.clone();
        let result = self.commit_for(
            &command,
            &key,
            &record,
            vec![sealed],
            Chunks::Discard {
                artifact: &artifact,
            },
            Vec::new(),
            outcome(false),
        )?;
        if self.config.evidence_store.corrupt.contains(&key.id) {
            self.store.corrupt_object(&digest)?;
        }
        Ok(result)
    }

    pub(super) fn evidence_abandon(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        let key = key_of(&command.subject);
        if key.kind != evidence::ARTIFACT
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "artifact");
        }
        let existing = self.evidence_record(evidence::ARTIFACT, &key.id)?;
        let work = existing
            .as_ref()
            .and_then(|(_, r)| member(r, "descriptor").and_then(|d| d.get("work")).cloned());
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Publish {
                artifact: key.id.clone(),
                work,
            },
        )? {
            return Ok(stored);
        }
        let Some((_, mut record)) = self.evidence_record(evidence::ARTIFACT, &key.id)? else {
            return Err(ProtocolError::not_found());
        };
        // Only a staged upload is abandoned; sealed or held content never is.
        if text(&record, "state").as_deref() != Some("staged") {
            return Err(ProtocolError::not_found());
        }
        set(&mut record, "state", Value::String("abandoned".into()));
        set(
            &mut record,
            "abandoned_reason",
            Value::String("abandoned_by_producer".into()),
        );
        remove(&mut record, "received");
        let abandoned = event(
            "evidence.artifact.abandoned",
            Value::Object(vec![(
                "reason".into(),
                Value::String("abandoned_by_producer".into()),
            )]),
            &command.caused_by,
        );
        let outcome = Value::Object(vec![
            ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
            ("state".into(), Value::String("abandoned".into())),
        ]);
        let artifact = key.id.clone();
        self.commit_for(
            &command,
            &key,
            &record,
            vec![abandoned],
            Chunks::Discard {
                artifact: &artifact,
            },
            Vec::new(),
            outcome,
        )
    }

    // ---- inspect, query, fetch (section 5) ---------------------------------

    /// **A derivation record is readable only under the job's own view
    /// and readable claims** (READINESS section 6).
    ///
    /// Step 6 has already decided whether this caller may read artifacts
    /// at all; this is the second question, and it exists because a
    /// derivation names the paths and line ranges of repositories the
    /// job could read. Serving one to a reader who could not read those
    /// is the M3 leak arriving through a new door.
    ///
    /// **Only derivations carry the member**, so nothing else changes:
    /// an artifact sealed before m4d, or a cited source, is read exactly
    /// as it was. The answer is `permission_denied` rather than
    /// `not_found`, because the caller is authorised for the subject and
    /// what they lack is the content behind it (CORE section 15.5).
    fn readable_derivation(
        &self,
        in_force: Option<&Grant>,
        record: &Value,
    ) -> Result<(), ProtocolError> {
        if self.covers_derivation(in_force, record)? {
            return Ok(());
        }
        Err(ProtocolError::permission_denied(
            "derivation_outside_readable_set",
        ))
    }

    /// Whether this caller may be shown this record at all. Split out
    /// because a **listing** hides one where `inspect` refuses it, and
    /// hiding is not an error.
    fn covers_derivation(
        &self,
        in_force: Option<&Grant>,
        record: &Value,
    ) -> Result<bool, ProtocolError> {
        let Some(under) = member(record, "readable_under") else {
            return Ok(true);
        };
        let view: Vec<String> = crate::repositories::view(&self.store, in_force)
            .map_err(|_| ProtocolError::new_internal_error())?
            .into_iter()
            .map(|visible| visible.id)
            .collect();
        let mut claims = Vec::new();
        for (claim, _) in self
            .store
            .subjects_of_kind(crate::knowledge::CLAIM)
            .map_err(|_| ProtocolError::new_internal_error())?
        {
            // The authority principal holds no grant and reads its own
            // store; anyone else reads exactly what a grant covers.
            let permitted = match in_force {
                None => true,
                Some(grant) => grant.may_read(
                    &crate::knowledge::key(crate::knowledge::CLAIM, &claim),
                    "knowledge.read",
                ),
            };
            if permitted {
                claims.push(claim);
            }
        }
        // **Only the evidence the record names is asked about**: the
        // reader may read each of those, or may not, and nothing else in
        // the store bears on it.
        let evidence: Vec<String> = crate::derivation::evidence_under(under)
            .into_iter()
            .filter(|artifact| self.may_read(in_force, &evidence::artifact_key(artifact)))
            .collect();
        Ok(crate::derivation::covers(under, &view, &claims, &evidence))
    }

    pub(super) fn evidence_inspect(
        &self,
        payload: &Value,
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let id = artifact_id_of(payload)?;
        let Some((revision, record)) = self.evidence_record(evidence::ARTIFACT, &id)? else {
            return Err(ProtocolError::not_found());
        };
        self.readable_derivation(in_force, &record)?;
        let state = text(&record, "state").unwrap_or_default();
        let descriptor = member(&record, "descriptor")
            .cloned()
            .unwrap_or(Value::Null);
        let mut out = vec![
            ("artifact".into(), subject(evidence::ARTIFACT, &id)),
            ("revision".into(), Value::Int(revision)),
            ("descriptor".into(), descriptor.clone()),
            ("state".into(), Value::String(state.clone())),
            ("availability".into(), self.availability_of(&id, &record)?),
        ];
        if state == "staged" {
            out.push(("received".into(), Value::Int(int(&record, "received"))));
        }
        if let Some(reason) = text(&record, "abandoned_reason") {
            out.push(("abandoned_reason".into(), Value::String(reason)));
        }
        let holds = self.holds_on(&id)?;
        out.push((
            "holds".into(),
            Value::Array(
                holds
                    .iter()
                    .filter(|(_, _, hold)| self.hold_visible(in_force, hold))
                    .map(|(hold_id, hold_revision, hold)| {
                        Value::Object(vec![
                            ("hold".into(), subject(evidence::HOLD, hold_id)),
                            ("revision".into(), Value::Int(*hold_revision)),
                            (
                                "state".into(),
                                hold.get("state").cloned().unwrap_or(Value::Null),
                            ),
                            (
                                "owner".into(),
                                hold.get("owner").cloned().unwrap_or(Value::Null),
                            ),
                            (
                                "holder_ref".into(),
                                hold.get("holder_ref").cloned().unwrap_or(Value::Null),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ));
        if let Some(purge) = member(&record, "purge") {
            out.push((
                "loss".into(),
                self.loss_record(&id, &descriptor, purge, &holds, in_force)?,
            ));
        }
        if state == "sealed"
            && self.selected_feature("evidence.manifests")
            && let Some(Value::Array(children)) = member(&record, "manifest_children")
        {
            out.push((
                "completeness".into(),
                self.completeness(children, in_force)?,
            ));
        }
        Ok(Value::Object(out))
    }

    /// The proof-loss record, filtered for this reader (EVD-6).
    fn loss_record(
        &self,
        id: &str,
        descriptor: &Value,
        purge: &Value,
        holds: &[(String, i64, Value)],
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let mut affected: Vec<(String, String, String)> = Vec::new();
        let mut filtered = false;
        for (hold_id, _, hold) in holds {
            if self.hold_visible(in_force, hold) {
                affected.push((
                    "evidence.hold".into(),
                    evidence::HOLD.into(),
                    hold_id.clone(),
                ));
            } else {
                filtered = true;
            }
        }
        for (manifest_id, _) in self.store.subjects_of_kind(evidence::ARTIFACT)? {
            let Some((_, manifest)) = self.evidence_record(evidence::ARTIFACT, &manifest_id)?
            else {
                continue;
            };
            let lists = member(&manifest, "manifest_children")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(Child::from_value)
                .any(|child| child.artifact == id && child.provider.is_none());
            if !lists {
                continue;
            }
            if self.may_read(in_force, &evidence::artifact_key(&manifest_id)) {
                affected.push((
                    "evidence.manifest_child".into(),
                    evidence::ARTIFACT.into(),
                    manifest_id,
                ));
            } else {
                filtered = true;
            }
        }
        affected.sort();
        let released: Vec<Value> = purge
            .get("released_holds")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .map(|hold| subject(evidence::HOLD, hold))
            .collect();
        let mut loss = vec![
            ("artifact".into(), subject(evidence::ARTIFACT, id)),
            (
                "digest".into(),
                descriptor.get("digest").cloned().unwrap_or(Value::Null),
            ),
            (
                "requested_at".into(),
                purge.get("requested_at").cloned().unwrap_or(Value::Null),
            ),
        ];
        if let Some(confirmed) = purge.get("confirmed_at") {
            loss.push(("confirmed_at".into(), confirmed.clone()));
        }
        loss.push(("released_holds".into(), Value::Array(released)));
        loss.push((
            "affected".into(),
            Value::Array(
                affected
                    .into_iter()
                    .map(|(kind, subject_kind, subject_id)| {
                        Value::Object(vec![
                            ("kind".into(), Value::String(kind)),
                            ("subject".into(), subject(&subject_kind, &subject_id)),
                        ])
                    })
                    .collect(),
            ),
        ));
        loss.push((
            "coverage".into(),
            Value::Object(vec![
                (
                    "tracked".into(),
                    Value::Array(
                        evidence::TRACKED
                            .iter()
                            .map(|kind| Value::String((*kind).into()))
                            .collect(),
                    ),
                ),
                ("filtered".into(), Value::Bool(filtered)),
            ]),
        ));
        Ok(Value::Object(loss))
    }

    /// Manifest completeness for this reader (EVIDENCE section 7). A child the
    /// reader may not read is `withheld` whether or not it exists; one held
    /// elsewhere or not currently available is `unverified`. Neither is ever
    /// turned into `missing`, which would leak existence or claim knowledge
    /// the provider lacks.
    fn completeness(
        &self,
        children: &[Value],
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let mut states = Vec::new();
        let mut out = Vec::new();
        for value in children {
            let Some(child) = Child::from_value(value) else {
                continue;
            };
            let state = if child
                .provider
                .as_ref()
                .is_some_and(|provider| *provider != self.config.provider_id)
            {
                "unverified"
            } else if !self.may_read(in_force, &evidence::artifact_key(&child.artifact)) {
                "withheld"
            } else {
                match self.evidence_record(evidence::ARTIFACT, &child.artifact)? {
                    None => "missing",
                    Some((_, record)) => {
                        let sealed = text(&record, "state").as_deref() == Some("sealed");
                        let digest = member(&record, "descriptor").and_then(|d| text(d, "digest"));
                        if !sealed
                            || member(&record, "purge").is_some()
                            || digest.as_deref() != Some(child.digest.as_str())
                        {
                            "missing"
                        } else {
                            let availability = self.availability_of(&child.artifact, &record)?;
                            if availability.get("state").and_then(Value::as_str)
                                == Some("available")
                            {
                                "present"
                            } else {
                                "unverified"
                            }
                        }
                    }
                }
            };
            states.push((child.required, state));
            let mut entry = child.to_value();
            set(&mut entry, "state", Value::String(state.into()));
            out.push(entry);
        }
        let required: Vec<&str> = states.iter().filter(|(r, _)| *r).map(|(_, s)| *s).collect();
        let overall = if required.iter().all(|s| *s == "present") {
            "complete"
        } else if required.contains(&"missing") {
            "incomplete"
        } else {
            "undetermined"
        };
        Ok(Value::Object(vec![
            ("state".into(), Value::String(overall.into())),
            ("children".into(), Value::Array(out)),
        ]))
    }

    pub(super) fn evidence_query(
        &self,
        payload: &Value,
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let filters = payload
            .get("filters")
            .cloned()
            .unwrap_or(Value::Object(vec![]));
        let limit = match payload.get("limit") {
            None => 100,
            Some(Value::Int(n)) if (1..=1000).contains(n) => *n as usize,
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/limit",
                    "not an integer between 1 and 1000",
                ));
            }
        };
        let stream = self.store.stream_id()?;
        let after = match payload.get("cursor").and_then(Value::as_str) {
            None => None,
            Some(cursor) => Some(decode_query_cursor(&stream, cursor).ok_or_else(|| {
                ProtocolError::new("invalid_cursor", crate::errors::Retry::No).with(
                    "reason",
                    Value::String("not issued by this provider".into()),
                )
            })?),
        };
        let mut items = Vec::new();
        let mut filtered = false;
        let mut next = None;
        for (id, _) in self.store.subjects_of_kind(evidence::ARTIFACT)? {
            // `filtered` says whether authorization hid anything at all, never
            // whether something hidden matched, so it is decided before filters.
            if !self.may_read(in_force, &evidence::artifact_key(&id)) {
                filtered = true;
                continue;
            }
            if after.as_ref().is_some_and(|last| id <= *last) {
                continue;
            }
            let Some((_, record)) = self.evidence_record(evidence::ARTIFACT, &id)? else {
                continue;
            };
            // A derivation whose readable set this caller does not cover
            // is hidden here exactly as `inspect` refuses it: a listing
            // hands back the same descriptor, so a gate on one door and
            // not the other is a gate with a second entrance. Counted as
            // filtered, because authorization hid something.
            if !self.covers_derivation(in_force, &record)? {
                filtered = true;
                continue;
            }
            let descriptor = member(&record, "descriptor")
                .cloned()
                .unwrap_or(Value::Null);
            if !matches_filters(&descriptor, &filters) {
                continue;
            }
            if items.len() == limit {
                next = Some(encode_query_cursor(&stream, &items_last(&items)));
                break;
            }
            items.push((
                id.clone(),
                Value::Object(vec![
                    ("artifact".into(), subject(evidence::ARTIFACT, &id)),
                    (
                        "digest".into(),
                        descriptor.get("digest").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "state".into(),
                        record.get("state").cloned().unwrap_or(Value::Null),
                    ),
                    ("availability".into(), self.availability_of(&id, &record)?),
                ]),
            ));
        }
        let mut out = vec![
            (
                "items".into(),
                Value::Array(items.into_iter().map(|(_, item)| item).collect()),
            ),
            ("filtered".into(), Value::Bool(filtered)),
        ];
        if let Some(cursor) = next {
            out.push(("next_cursor".into(), Value::String(cursor)));
        }
        Ok(Value::Object(out))
    }

    pub(super) fn evidence_fetch(
        &self,
        payload: &Value,
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let id = artifact_id_of(payload)?;
        let requested = text(payload, "digest").unwrap_or_default();
        let Some((_, record)) = self.evidence_record(evidence::ARTIFACT, &id)? else {
            return Err(ProtocolError::not_found());
        };
        self.readable_derivation(in_force, &record)?;
        // Staged or abandoned: no sealed content exists.
        if text(&record, "state").as_deref() != Some("sealed") {
            return Err(ProtocolError::not_found());
        }
        let descriptor = member(&record, "descriptor")
            .cloned()
            .unwrap_or(Value::Null);
        let digest = text(&descriptor, "digest").unwrap_or_default();
        if requested != digest {
            return Err(ProtocolError::new(
                "artifact_digest_mismatch",
                crate::errors::Retry::No,
            ));
        }
        let size = int(&descriptor, "size");
        let offset = match payload.get("offset") {
            None => 0,
            Some(Value::Int(n)) if *n >= 0 => (*n).min(size),
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/offset",
                    "not a non-negative integer",
                ));
            }
        };
        let header = |availability: Value, data: String, next: i64| {
            Value::Object(vec![
                ("artifact".into(), subject(evidence::ARTIFACT, &id)),
                ("digest".into(), Value::String(digest.clone())),
                ("availability".into(), availability),
                ("offset".into(), Value::Int(offset)),
                ("data_base64".into(), Value::String(data)),
                ("next_offset".into(), Value::Int(next)),
                ("size".into(), Value::Int(size)),
            ])
        };
        let availability = self.availability_of(&id, &record)?;
        if availability.get("state").and_then(Value::as_str) != Some("available") {
            return Ok(header(availability, String::new(), offset));
        }
        // Verified above from the bytes on disk; read again to serve exactly
        // what was verified, never regenerated or normalized.
        let Some(mut bytes) = self.store.read_object(&digest)? else {
            return Ok(header(
                evidence::availability("unavailable", Some("integrity_failed")),
                String::new(),
                offset,
            ));
        };
        if cbr_encoding::digest_bytes(&bytes) != digest {
            return Ok(header(
                evidence::availability("unavailable", Some("integrity_failed")),
                String::new(),
                offset,
            ));
        }
        if self.config.evidence_store.serve_altered_bytes.contains(&id)
            && let Some(first) = bytes.first_mut()
        {
            // Adversarial test control: altered bytes behind honest metadata,
            // so readers are tested on verifying what they assemble.
            *first ^= 0x01;
        }
        let max_bytes = match payload.get("max_bytes") {
            None => size,
            Some(Value::Int(n)) if *n >= 1 => *n,
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/max_bytes",
                    "not a positive integer",
                ));
            }
        };
        // Fewer bytes than asked when the encoded response would not fit the
        // caller's receive limit, and at least one whenever one fits.
        let fits = ((self.receive_budget() / 4) * 3).max(1) as i64;
        let end = (offset + max_bytes.min(fits)).min(size);
        let slice = &bytes[offset as usize..end as usize];
        Ok(header(availability, evidence::encode_base64(slice), end))
    }

    // ---- holds and purge (section 9) ---------------------------------------

    pub(super) fn evidence_hold(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        self.retention_gate()?;
        let key = key_of(&command.subject);
        if key.kind != evidence::HOLD
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "hold");
        }
        let artifact = artifact_id_of(&command.payload)?;
        let Some(holder_ref) = command
            .payload
            .get("holder_ref")
            .filter(|v| v.is_object())
            .cloned()
        else {
            return Err(ProtocolError::invalid_envelope(
                "/payload/holder_ref",
                "not a subject",
            ));
        };
        let reason = text(&command.payload, "reason")
            .filter(|r| !r.is_empty())
            .ok_or_else(|| {
                ProtocolError::invalid_envelope("/payload/reason", "required non-empty string")
            })?;
        let expires_at = match command.payload.get("expires_at") {
            None => None,
            Some(Value::String(instant)) if grants::is_instant(instant) => Some(instant.clone()),
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/expires_at",
                    "not a UTC instant",
                ));
            }
        };
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Rights(vec![(
                "evidence.hold",
                Some(evidence::artifact_key(&artifact)),
            )]),
        )? {
            return Ok(stored);
        }
        if let Some(instant) = &expires_at
            && instant.as_str() <= self.clock.now().as_str()
        {
            return Err(ProtocolError::invalid_envelope(
                "/payload/expires_at",
                "not later than the provider clock",
            ));
        }
        // Nothing would be retained: not sealed, or its purge already requested.
        let retainable = self
            .evidence_record(evidence::ARTIFACT, &artifact)?
            .is_some_and(|(_, record)| {
                text(&record, "state").as_deref() == Some("sealed")
                    && member(&record, "purge").is_none()
            });
        if !retainable {
            return Err(ProtocolError::not_found());
        }
        let owner = self.config.principal.clone();
        let mut record = vec![
            ("artifact".into(), Value::String(artifact.clone())),
            ("holder_ref".into(), holder_ref.clone()),
            ("reason".into(), Value::String(reason)),
            ("owner".into(), Value::String(owner.clone())),
            ("state".into(), Value::String("active".into())),
        ];
        if let Some(instant) = expires_at {
            record.push(("expires_at".into(), Value::String(instant)));
        }
        let placed = event(
            "evidence.hold.placed",
            Value::Object(vec![
                ("artifact".into(), subject(evidence::ARTIFACT, &artifact)),
                ("holder_ref".into(), holder_ref),
            ]),
            &command.caused_by,
        );
        let outcome = Value::Object(vec![
            ("hold".into(), subject(evidence::HOLD, &key.id)),
            ("state".into(), Value::String("active".into())),
            ("owner".into(), Value::String(owner)),
        ]);
        self.commit_for(
            &command,
            &key,
            &Value::Object(record),
            vec![placed],
            Chunks::None,
            Vec::new(),
            outcome,
        )
    }

    pub(super) fn evidence_release(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        self.retention_gate()?;
        let key = key_of(&command.subject);
        if key.kind != evidence::HOLD
            || command.preconditions.len() != 1
            || key_of(&command.preconditions[0].0) != key
        {
            return self.one_precondition(&key, "hold");
        }
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Release {
                hold: key.id.clone(),
            },
        )? {
            return Ok(stored);
        }
        let Some((_, mut record)) = self.evidence_record(evidence::HOLD, &key.id)? else {
            return Err(ProtocolError::not_found());
        };
        if text(&record, "state").as_deref() != Some("active") {
            return Err(ProtocolError::not_found());
        }
        set(&mut record, "state", Value::String("released".into()));
        let released = event(
            "evidence.hold.released",
            hold_event_payload(&record),
            &command.caused_by,
        );
        let outcome = Value::Object(vec![
            ("hold".into(), subject(evidence::HOLD, &key.id)),
            ("state".into(), Value::String("released".into())),
        ]);
        self.commit_for(
            &command,
            &key,
            &record,
            vec![released],
            Chunks::None,
            Vec::new(),
            outcome,
        )
    }

    pub(super) fn evidence_purge(
        &mut self,
        params: &Value,
        command: crate::envelope::Command,
    ) -> Result<Value, ProtocolError> {
        self.retention_gate()?;
        let key = key_of(&command.subject);
        if key.kind != evidence::ARTIFACT {
            return self.one_precondition(&key, "artifact");
        }
        let release_holds: Vec<String> = match command.payload.get("release_holds") {
            None => Vec::new(),
            Some(Value::Array(items)) => {
                let mut holds = Vec::new();
                for (index, item) in items.iter().enumerate() {
                    match item.as_str() {
                        Some(id)
                            if crate::envelope::is_identifier(id)
                                && !holds.contains(&id.to_string()) =>
                        {
                            holds.push(id.to_string());
                        }
                        _ => {
                            return Err(ProtocolError::invalid_envelope(
                                &format!("/payload/release_holds/{index}"),
                                "not a unique hold id",
                            ));
                        }
                    }
                }
                holds
            }
            Some(_) => {
                return Err(ProtocolError::invalid_envelope(
                    "/payload/release_holds",
                    "not an array",
                ));
            }
        };
        // Exactly the artifact's precondition and one per named hold, so a
        // concurrent change to any hold being released is detected (step 2).
        let mut expected: Vec<SubjectKey> = vec![key.clone()];
        expected.extend(release_holds.iter().map(|hold| evidence::hold_key(hold)));
        let mut named: Vec<SubjectKey> = command
            .preconditions
            .iter()
            .map(|(subject, _)| key_of(subject))
            .collect();
        named.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
        expected.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
        if named != expected {
            return Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "a purge carries a precondition on the artifact and on each named hold",
            ));
        }
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Purge {
                artifact: key.id.clone(),
                holds: release_holds.clone(),
            },
        )? {
            return Ok(stored);
        }
        let Some((revision, mut record)) = self.evidence_record(evidence::ARTIFACT, &key.id)?
        else {
            return Err(ProtocolError::not_found());
        };
        if text(&record, "state").as_deref() != Some("sealed") {
            return Err(ProtocolError::not_found());
        }
        let in_force = self.authorize(command.grant.as_deref(), &[])?;

        // A repeated purge: the current availability, nothing released,
        // nothing changed.
        if member(&record, "purge").is_some() {
            let availability = self.availability_of(&key.id, &record)?;
            let outcome = Value::Object(vec![
                ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
                (
                    "availability".into(),
                    availability.get("state").cloned().unwrap_or(Value::Null),
                ),
                ("released_holds".into(), Value::Array(vec![])),
            ]);
            return self.bind_unchanged(&command, revision, outcome);
        }

        let holds = self.holds_on(&key.id)?;
        for hold in &release_holds {
            let active = holds.iter().any(|(id, _, record)| {
                id == hold && text(record, "state").as_deref() == Some("active")
            });
            if !active {
                return Err(ProtocolError::not_found());
            }
        }
        let blocking: Vec<&(String, i64, Value)> = holds
            .iter()
            .filter(|(id, _, record)| {
                text(record, "state").as_deref() == Some("active") && !release_holds.contains(id)
            })
            .collect();
        if !blocking.is_empty() {
            let visible: Vec<Value> = blocking
                .iter()
                .filter(|(_, _, record)| self.hold_visible(in_force.as_ref(), record))
                .map(|(id, _, _)| Value::String(id.clone()))
                .collect();
            let filtered = visible.len() != blocking.len();
            return Err(
                ProtocolError::new("hold_active", crate::errors::Retry::AfterReconcile)
                    .with("holds", Value::Array(visible))
                    .with("filtered", Value::Bool(filtered)),
            );
        }

        let now = self.clock.now();
        let delay = self
            .config
            .evidence_store
            .deletion_delay_seconds
            .unwrap_or(0);
        let digest = member(&record, "descriptor")
            .and_then(|d| text(d, "digest"))
            .unwrap_or_default();
        let mut purge = vec![
            ("requested_at".into(), Value::String(now.clone())),
            (
                "released_holds".into(),
                Value::Array(
                    release_holds
                        .iter()
                        .map(|h| Value::String(h.clone()))
                        .collect(),
                ),
            ),
        ];
        let immediate = delay <= 0;
        if immediate {
            purge.push(("confirmed_at".into(), Value::String(now.clone())));
        } else {
            purge.push(("due_at".into(), Value::String(plus_seconds(&now, delay))));
        }
        set(&mut record, "purge", Value::Object(purge));

        let mut events = vec![event(
            "evidence.artifact.purge_requested",
            Value::Object(vec![("requested_at".into(), Value::String(now.clone()))]),
            &command.caused_by,
        )];
        if immediate {
            events.push(event(
                "evidence.artifact.purged",
                Value::Object(vec![("confirmed_at".into(), Value::String(now.clone()))]),
                &command.caused_by,
            ));
        }
        // The artifact's own events first, then each released hold's.
        let mut also = Vec::new();
        for hold in &release_holds {
            let (_, _, hold_record) = holds.iter().find(|(id, _, _)| id == hold).expect("checked");
            let mut released = hold_record.clone();
            set(&mut released, "state", Value::String("released".into()));
            also.push(crate::store::Change {
                key: evidence::hold_key(hold),
                value: canonical(&released),
                event: Some(event(
                    "evidence.hold.released",
                    hold_event_payload(&released),
                    &command.caused_by,
                )),
            });
        }
        let outcome = Value::Object(vec![
            ("artifact".into(), subject(evidence::ARTIFACT, &key.id)),
            (
                "availability".into(),
                Value::String(if immediate { "purged" } else { "purge_pending" }.into()),
            ),
            (
                "released_holds".into(),
                Value::Array(
                    release_holds
                        .iter()
                        .map(|h| subject(evidence::HOLD, h))
                        .collect(),
                ),
            ),
        ]);
        let result =
            self.commit_for(&command, &key, &record, events, Chunks::None, also, outcome)?;
        crate::barriers::pause("evidence.purge.after_commit");
        if immediate {
            self.delete_if_unreferenced(&digest)?;
        }
        Ok(result)
    }

    /// Delete an object once no sealed, unpurged artifact still names it.
    /// Equal bytes are one object, and another artifact's retention must not
    /// be ended by this one's purge.
    fn delete_if_unreferenced(&self, digest: &str) -> Result<(), ProtocolError> {
        if self.object_roots()?.contains(digest) {
            return Ok(());
        }
        self.store.delete_object(digest)?;
        Ok(())
    }

    /// Every object digest some artifact still keeps alive.
    fn object_roots(&self) -> Result<std::collections::BTreeSet<String>, crate::store::StoreError> {
        let mut roots = std::collections::BTreeSet::new();
        for (_, value) in self.store.subjects_of_kind(evidence::ARTIFACT)? {
            let record = cbr_encoding::parse(value.as_bytes()).map_err(|_| {
                crate::store::StoreError::Corrupt("an artifact record does not parse")
            })?;
            if let Some(digest) = evidence::object_root(&record) {
                roots.insert(digest.to_string());
            }
        }
        Ok(roots)
    }

    /// The start-time collection pass (STORAGE sections 2 and 5, "during
    /// collection"): recheck the roots, then delete every published object no
    /// root names and every staging file a crashed publication left behind.
    ///
    /// Two crashes leave such objects, and both orders are deliberate. A seal
    /// killed after publishing its object and before committing the row that
    /// names it leaves an orphan: the artifact is still staged with its bytes
    /// in the chunk table, and a retried seal publishes again. A purge killed
    /// after committing and before deleting leaves the object of an artifact
    /// already recorded as purged: nothing serves it, and this pass finishes
    /// the deletion. It runs once per process start, before any session
    /// exists, so no reader can observe `purged` while those bytes remain.
    ///
    /// Only evidence artifacts own objects in M1. Anything that later
    /// publishes into the object store must become a root here first.
    pub(super) fn collect_unreferenced_objects(&self) -> Result<usize, crate::store::StoreError> {
        let roots = self.object_roots()?;
        let inventory = self.store.object_inventory()?;
        let mut collected = 0;
        for digest in inventory.objects.iter().filter(|d| !roots.contains(*d)) {
            self.store.delete_object(digest)?;
            collected += 1;
        }
        for staging in &inventory.staging {
            self.store.discard_staging(staging)?;
            collected += 1;
        }
        Ok(collected)
    }

    // ---- time-driven changes -----------------------------------------------

    /// Staging timeouts, hold expiry and confirmed deletions, each a
    /// provider-origin change with its event. Run with effect ticks.
    pub(super) fn tick_evidence(&mut self) -> Result<(), ProtocolError> {
        let now = self.clock.now();
        let timeout = self.config.evidence_store.staging_timeout_seconds;
        for (id, _) in self.store.subjects_of_kind(evidence::ARTIFACT)? {
            let Some((_, mut record)) = self.evidence_record(evidence::ARTIFACT, &id)? else {
                continue;
            };
            let key = evidence::artifact_key(&id);
            if let Some(seconds) = timeout
                && text(&record, "state").as_deref() == Some("staged")
                && text(&record, "staged_at")
                    .is_some_and(|staged| plus_seconds(&staged, seconds) <= now)
            {
                set(&mut record, "state", Value::String("abandoned".into()));
                set(
                    &mut record,
                    "abandoned_reason",
                    Value::String("staging_expired".into()),
                );
                remove(&mut record, "received");
                let abandoned = event(
                    "evidence.artifact.abandoned",
                    Value::Object(vec![(
                        "reason".into(),
                        Value::String("staging_expired".into()),
                    )]),
                    &[],
                );
                self.store.commit_provider_change(
                    &key,
                    &canonical(&record),
                    vec![abandoned],
                    &now,
                    true,
                )?;
                continue;
            }
            let due = member(&record, "purge")
                .filter(|purge| purge.get("confirmed_at").is_none())
                .and_then(|purge| text(purge, "due_at"));
            if let Some(due) = due
                && due <= now
            {
                if let Some(Value::Object(purge)) = match &mut record {
                    Value::Object(members) => members
                        .iter_mut()
                        .find(|(name, _)| name == "purge")
                        .map(|(_, value)| value),
                    _ => None,
                } {
                    purge.retain(|(name, _)| name != "due_at");
                    purge.push(("confirmed_at".into(), Value::String(now.clone())));
                }
                let purged = event(
                    "evidence.artifact.purged",
                    Value::Object(vec![("confirmed_at".into(), Value::String(now.clone()))]),
                    &[],
                );
                self.store.commit_provider_change(
                    &key,
                    &canonical(&record),
                    vec![purged],
                    &now,
                    false,
                )?;
                let digest = member(&record, "descriptor")
                    .and_then(|d| text(d, "digest"))
                    .unwrap_or_default();
                self.delete_if_unreferenced(&digest)?;
            }
        }
        for (id, _) in self.store.subjects_of_kind(evidence::HOLD)? {
            let Some((_, mut record)) = self.evidence_record(evidence::HOLD, &id)? else {
                continue;
            };
            if text(&record, "state").as_deref() == Some("active")
                && text(&record, "expires_at").is_some_and(|expiry| expiry <= now)
            {
                set(&mut record, "state", Value::String("expired".into()));
                let expired = event("evidence.hold.expired", hold_event_payload(&record), &[]);
                self.store.commit_provider_change(
                    &evidence::hold_key(&id),
                    &canonical(&record),
                    vec![expired],
                    &now,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

fn hold_event_payload(hold: &Value) -> Value {
    Value::Object(vec![
        (
            "artifact".into(),
            subject(
                evidence::ARTIFACT,
                &text(hold, "artifact").unwrap_or_default(),
            ),
        ),
        (
            "holder_ref".into(),
            hold.get("holder_ref").cloned().unwrap_or(Value::Null),
        ),
    ])
}

/// The artifact id an evidence payload names.
pub(super) fn artifact_id_of(payload: &Value) -> Result<String, ProtocolError> {
    let artifact = payload.get("artifact").ok_or_else(|| {
        ProtocolError::invalid_envelope("/payload/artifact", "required field absent")
    })?;
    if artifact.get("kind").and_then(Value::as_str) != Some(evidence::ARTIFACT) {
        return Err(ProtocolError::invalid_envelope(
            "/payload/artifact/kind",
            "not evidence.artifact",
        ));
    }
    match artifact.get("id").and_then(Value::as_str) {
        Some(id) if crate::envelope::is_identifier(id) => Ok(id.to_string()),
        _ => Err(ProtocolError::invalid_envelope(
            "/payload/artifact/id",
            "not an identifier",
        )),
    }
}

fn matches_filters(descriptor: &Value, filters: &Value) -> bool {
    let equal = |filter: &str, actual: Option<&Value>| {
        filters
            .get(filter)
            .is_none_or(|wanted| Some(wanted) == actual)
    };
    equal(
        "producer_principal",
        descriptor.get("producer").and_then(|p| p.get("principal")),
    ) && equal("media_type", descriptor.get("media_type"))
        && equal("digest", descriptor.get("digest"))
        && equal("scope", descriptor.get("scope"))
        && equal("source", descriptor.get("source"))
        && equal("work", descriptor.get("work"))
}

fn items_last(items: &[(String, Value)]) -> String {
    items.last().map(|(id, _)| id.clone()).unwrap_or_default()
}

fn encode_query_cursor(stream: &str, last: &str) -> String {
    let hex: String = last.bytes().map(|b| format!("{b:02x}")).collect();
    format!("eq1.{stream}.{hex}")
}

fn decode_query_cursor(stream: &str, cursor: &str) -> Option<String> {
    let rest = cursor.strip_prefix("eq1.")?;
    let (issued_by, hex) = rest.split_once('.')?;
    if issued_by != stream || hex.is_empty() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_add_seconds_across_day_and_month_boundaries() {
        assert_eq!(
            plus_seconds("2030-01-01T00:00:00Z", 30),
            "2030-01-01T00:00:30Z"
        );
        assert_eq!(
            plus_seconds("2030-01-31T23:59:30Z", 60),
            "2030-02-01T00:00:30Z"
        );
        assert_eq!(
            plus_seconds("2028-02-28T23:00:00Z", 3600),
            "2028-02-29T00:00:00Z"
        );
    }

    #[test]
    fn a_query_cursor_is_refused_unless_this_provider_issued_it() {
        let cursor = encode_query_cursor("stream-a", "ok-1");
        assert_eq!(
            decode_query_cursor("stream-a", &cursor).as_deref(),
            Some("ok-1")
        );
        assert!(decode_query_cursor("stream-b", &cursor).is_none());
        assert!(decode_query_cursor("stream-a", "not-a-cursor-from-this-provider").is_none());
    }
}
