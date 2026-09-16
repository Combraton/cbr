//! `context/1` operations on the provider (CONTEXT sections 3 to 14).
//!
//! Commands follow the Core command path through `admit_command`, after this
//! profile's step 2 checks and its step 3 features: a request is shaped, then
//! its timing semantics must have been negotiated, then it is authorized, and
//! only then does anything look at whether it exists.
//!
//! Preparation is the provider's own work. Each running job advances against
//! the provider clock before every request this provider handles, in one
//! owner transaction per job, and what it publishes is recorded as
//! provider-origin events. In a conformance launch the steps come from the
//! `context.script` test control; nothing else prepares content yet, so a
//! production request with no script waits for its deadline and is published
//! with its items unmet, which is the honest answer until CBR compiles packets.
//!
//! A packet revision is never reported before its bytes are sealed: locally,
//! the object is published and verified before the batch that names it
//! commits; at a separate evidence provider, the seal must succeed first, and
//! if it does not, nothing of that job's step is committed and it is retried
//! at a later tick.

use cbr_encoding::Value;

use super::{Epoch, Provider, Step6, accepted};
use crate::context::{
    self, JOB, PACKET, REQUEST, at, canonical, int, key, list, object, set, string, subject, text,
};
use crate::envelope::Command;
use crate::errors::ProtocolError;
use crate::grants::Grant;
use crate::peer::{self, PeerConfig};
use crate::store::{Change, Commit, NewEvent, ProviderEvent, ProviderWrite, SubjectKey};

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

/// Subject changes and provider-origin events of one job's tick, applied in
/// memory first and committed together. A subject saved twice in one tick
/// passes through two revisions, and each event names the one it records.
#[derive(Default)]
struct Batch {
    writes: Vec<ProviderWrite>,
    events: Vec<ProviderEvent>,
}

impl Batch {
    fn written(&self, key: &SubjectKey) -> Option<&ProviderWrite> {
        self.writes.iter().find(|w| &w.key == key)
    }

    fn event(&mut self, key: &SubjectKey, revision: i64, event_type: &str, payload: Value) {
        self.events.push(ProviderEvent {
            key: key.clone(),
            revision,
            event_type: event_type.into(),
            payload,
        });
    }
}

/// A publication at a separate evidence provider did not seal. The capture
/// instants of every packet attempted in the tick are kept, so the retry
/// submits the same descriptors and replays what already applied.
struct NotSealed {
    artifact: String,
    failure: peer::Failure,
    captures: Vec<(String, String)>,
}

/// One job's tick in progress.
struct Tick {
    now: String,
    batch: Batch,
    /// `(artifact, captured_at)` for every packet sent to an evidence peer.
    captures: Vec<(String, String)>,
}

enum TickError {
    Protocol(ProtocolError),
    NotSealed(NotSealed),
}

impl From<ProtocolError> for TickError {
    fn from(error: ProtocolError) -> Self {
        TickError::Protocol(error)
    }
}

impl From<crate::store::StoreError> for TickError {
    fn from(error: crate::store::StoreError) -> Self {
        TickError::Protocol(error.into())
    }
}

impl Provider {
    // ---- shared ------------------------------------------------------------

    fn context_record(&self, kind: &str, id: &str) -> Result<Option<(i64, Value)>, ProtocolError> {
        let Some(state) = self.store.subject(&key(kind, id))? else {
            return Ok(None);
        };
        Ok(Some((state.revision, parse_record(&state.value)?)))
    }

    /// A record as the tick sees it: its latest in-memory change, else the
    /// store.
    fn tick_record(
        &self,
        tick: &Tick,
        kind: &str,
        id: &str,
    ) -> Result<Option<(i64, Value)>, ProtocolError> {
        let subject_key = key(kind, id);
        if let Some(write) = tick.batch.written(&subject_key) {
            return Ok(Some((write.revision, parse_record(&write.value)?)));
        }
        self.context_record(kind, id)
    }

    /// Save a record in the tick, returning the revision it now has.
    fn tick_save(
        &self,
        tick: &mut Tick,
        subject_key: &SubjectKey,
        value: &Value,
    ) -> Result<i64, ProtocolError> {
        let value = canonical(value);
        if let Some(write) = tick.batch.writes.iter_mut().find(|w| &w.key == subject_key) {
            write.revision += 1;
            write.value = value;
            return Ok(write.revision);
        }
        let base = self.store.revision(subject_key)?;
        tick.batch.writes.push(ProviderWrite {
            key: subject_key.clone(),
            base,
            revision: base + 1,
            value,
        });
        Ok(base + 1)
    }

    /// The `context.request` subject and its one precondition (step 2).
    fn context_shape(&self, command: &Command) -> Result<SubjectKey, ProtocolError> {
        let subject_key = super::key_of(&command.subject);
        if subject_key.kind != REQUEST {
            return Err(ProtocolError::invalid_envelope(
                "/subject/kind",
                "this operation's subject is a context.request",
            ));
        }
        match command.preconditions.as_slice() {
            [(on, _)] if super::key_of(on) == subject_key => Ok(subject_key),
            _ => Err(ProtocolError::invalid_envelope(
                "/preconditions",
                "exactly one precondition, on the request",
            )),
        }
    }

    fn commit_context(
        &mut self,
        command: &Command,
        subject_key: &SubjectKey,
        value: &Value,
        new_event: NewEvent,
        also: Vec<Change>,
        outcome: Value,
    ) -> Result<Value, ProtocolError> {
        let principal = self.config.principal.clone();
        let value = canonical(value);
        let recorded_at = self.clock.now();
        Ok(self.store.commit_command(
            Commit {
                key: subject_key,
                value: &value,
                principal: &principal,
                command_id: &command.command_id,
                digest: &command.command_digest,
                generation: command.dedupe_generation,
                event: Some(new_event),
                also,
                effects: Vec::new(),
                grant: command.grant.as_deref(),
                more_events: Vec::new(),
                chunks: crate::store::Chunks::None,
                recorded_at: &recorded_at,
                claim_revision: None,
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
        )?)
    }

    /// The script a job started by `request` follows: the `context.script`
    /// control's entry for it, else its default, else nothing.
    fn script_for(&self, request: &str) -> Vec<Value> {
        let control = &self.config.context.0;
        let script = match at(control, &["scripts", request]) {
            Value::Null => at(control, &["default_script"]),
            script => script,
        };
        script.as_array().unwrap_or_default().to_vec()
    }

    // ---- requests (sections 3 and 4) ---------------------------------------

    pub(super) fn context_submit(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let request_key = self.context_shape(&command)?;
        context::check_submit_payload(&command.payload)?;
        // Step 3: an obligation, or a claim check, whose feature this session
        // did not negotiate is never accepted under another meaning.
        let missing: Vec<Value> = context::needed_features(&command.payload)
            .into_iter()
            .filter(|feature| !self.selected_feature(feature))
            .map(Value::String)
            .collect();
        if !missing.is_empty() {
            return Err(ProtocolError::unsupported_required_feature_message(
                Value::Array(missing),
            ));
        }
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Rights(vec![("context.request", Some(request_key.clone()))]),
        )? {
            return Ok(stored);
        }

        let id = request_key.id.clone();
        let principal = self.config.principal.clone();
        let payload = &command.payload;
        let request_subject = subject(REQUEST, &id);
        let mut record = context::new_request(
            payload,
            &principal,
            self.selected_feature("context.updates"),
            self.selected_feature("context.claims"),
        );
        let script = self.script_for(&id);

        // Mandatory items that cannot fit are refused with what they need,
        // before any job exists (CONTEXT section 3).
        let needed = context::mandatory_size(&script, list(payload, &["items"]));
        if needed > int(payload, &["limits", "output_capacity", "amount"]) {
            let needed = object(vec![
                ("units", string("bytes")),
                ("amount", Value::Int(needed)),
            ]);
            set(&mut record, "state", string("refused"));
            set(&mut record, "reason", string("budget_insufficient"));
            set(&mut record, "needed", needed.clone());
            let changed = event(
                "context.request.changed",
                object(vec![
                    ("state", string("refused")),
                    ("reason", string("budget_insufficient")),
                ]),
                &command.caused_by,
            );
            let outcome = object(vec![
                ("request", request_subject),
                ("state", string("refused")),
                ("reason", string("budget_insufficient")),
                ("needed", needed),
            ]);
            return self.commit_context(
                &command,
                &request_key,
                &record,
                changed,
                Vec::new(),
                outcome,
            );
        }

        let mut joined = None;
        if self.selected_feature("context.shared_jobs") {
            for (job_id, _, value) in self.store.subjects_in_recorded_order(JOB)? {
                let mut job = parse_record(&value)?;
                if context::may_join(&job, &principal, payload) {
                    context::push(&mut job, "requests", string(&id));
                    joined = Some((job_id, job));
                    break;
                }
            }
        }
        let (job_id, job) = joined.unwrap_or_else(|| {
            (
                id.clone(),
                context::new_job(&principal, payload, &script, &id),
            )
        });
        let job_subject = subject(JOB, &job_id);
        set(&mut record, "state", string("preparing"));
        set(&mut record, "job", string(&job_id));
        let changed = event(
            "context.request.changed",
            object(vec![
                ("state", string("preparing")),
                ("job", job_subject.clone()),
            ]),
            &command.caused_by,
        );
        // A job gaining a subscriber is not itself an event: what a reader
        // observes is the request's own change.
        let also = vec![Change {
            key: key(JOB, &job_id),
            value: canonical(&job),
            event: None,
        }];
        let outcome = object(vec![
            ("request", request_subject),
            ("state", string("preparing")),
            ("job", job_subject),
        ]);
        self.commit_context(&command, &request_key, &record, changed, also, outcome)
    }

    /// `context.request.cancel`: that request only (CONTEXT section 4).
    pub(super) fn context_cancel(
        &mut self,
        params: &Value,
        command: Command,
    ) -> Result<Value, ProtocolError> {
        let request_key = self.context_shape(&command)?;
        context::check_cancel_payload(&command.payload)?;
        if let Some(stored) = self.admit_command(
            params,
            &command,
            Epoch::Unchecked,
            Step6::Rights(vec![("context.request", Some(request_key.clone()))]),
        )? {
            return Ok(stored);
        }
        // Nothing is being prepared for a request that is not preparing.
        let Some((_, mut record)) = self.context_record(REQUEST, &request_key.id)? else {
            return Err(ProtocolError::not_found());
        };
        if text(&record, &["state"]) != "preparing" {
            return Err(ProtocolError::not_found());
        }
        let job_id = text(&record, &["job"]).to_string();
        let Some((_, mut job)) = self.context_record(JOB, &job_id)? else {
            return Err(ProtocolError::new_internal_error());
        };
        let others: Vec<Value> = list(&job, &["requests"])
            .iter()
            .filter(|r| r.as_str() != Some(request_key.id.as_str()))
            .cloned()
            .collect();
        let job_continues = !others.is_empty();
        set(&mut job, "requests", Value::Array(others));
        let ended = if job_continues {
            None
        } else {
            // The provider ends a job nobody needs any more.
            set(&mut job, "state", string("ended"));
            set(&mut job, "reason", string("no_subscribers"));
            Some(event(
                "context.job.ended",
                object(vec![("reason", string("no_subscribers"))]),
                &command.caused_by,
            ))
        };
        set(&mut record, "state", string("cancelled"));
        let cancelled = event(
            "context.request.cancelled",
            object(vec![("job_continues", Value::Bool(job_continues))]),
            &command.caused_by,
        );
        let also = vec![Change {
            key: key(JOB, &job_id),
            value: canonical(&job),
            event: ended,
        }];
        let outcome = object(vec![
            ("request", subject(REQUEST, &request_key.id)),
            ("state", string("cancelled")),
            ("job", subject(JOB, &job_id)),
            ("job_continues", Value::Bool(job_continues)),
        ]);
        self.commit_context(&command, &request_key, &record, cancelled, also, outcome)
    }

    pub(super) fn context_request_inspect(&self, payload: &Value) -> Result<Value, ProtocolError> {
        let id = context::check_request_inspect_payload(payload)?;
        let Some((revision, record)) = self.context_record(REQUEST, &id)? else {
            return Err(ProtocolError::not_found());
        };
        let job = match record.get("job").and_then(Value::as_str) {
            Some(job_id) => self
                .context_record(JOB, job_id)?
                .map_or(Value::Null, |(_, job)| job),
            None => Value::Null,
        };
        let packets = list(&record, &["packets"]);
        // Once published, the item results are the current revision's.
        let items = match packets.last() {
            Some(facts) if text(&record, &["state"]) != "preparing" => {
                list(facts, &["items"]).to_vec()
            }
            _ => context::item_results(&record, &job, false, None),
        };
        let listed = packets
            .iter()
            .enumerate()
            .map(|(index, facts)| {
                let mut entry = object(vec![
                    ("reference", at(facts, &["reference"]).clone()),
                    ("current", Value::Bool(index + 1 == packets.len())),
                ]);
                if let Some(supersedes) = facts.get("supersedes") {
                    set(&mut entry, "supersedes", supersedes.clone());
                }
                entry
            })
            .collect();
        let mut result = object(vec![
            ("request", subject(REQUEST, &id)),
            ("revision", Value::Int(revision)),
            ("state", at(&record, &["state"]).clone()),
            ("consumer", at(&record, &["consumer"]).clone()),
            ("limits", at(&record, &["limits"]).clone()),
            ("items", Value::Array(items)),
            ("packets", Value::Array(listed)),
        ]);
        for member in ["reason", "needed"] {
            if let Some(value) = record.get(member) {
                set(&mut result, member, value.clone());
            }
        }
        if let Some(job_id) = record.get("job").and_then(Value::as_str) {
            set(&mut result, "job", subject(JOB, job_id));
        }
        Ok(result)
    }

    // ---- packets (sections 5, 6, 8 and 14) ---------------------------------

    /// A published revision's facts as stored, with its request record.
    fn published(
        &self,
        packet: &str,
        revision: i64,
    ) -> Result<Option<(Value, Value)>, ProtocolError> {
        let Some((_, record)) = self.context_record(REQUEST, packet)? else {
            return Ok(None);
        };
        let facts = usize::try_from(revision - 1)
            .ok()
            .and_then(|index| list(&record, &["packets"]).get(index).cloned());
        Ok(facts.map(|facts| (facts, record)))
    }

    /// The exact bytes of a published revision. They are the canonical form of
    /// the body recorded at publication, and they are served only when they
    /// still digest to the reference, so they can never differ from what was
    /// sealed.
    fn packet_bytes(facts: &Value) -> Result<Vec<u8>, ProtocolError> {
        let bytes = cbr_encoding::to_canonical(at(facts, &["body"]));
        if cbr_encoding::digest_bytes(&bytes) != text(facts, &["reference", "artifact", "digest"]) {
            return Err(ProtocolError::new_internal_error());
        }
        Ok(bytes)
    }

    pub(super) fn context_packet_inspect(&self, payload: &Value) -> Result<Value, ProtocolError> {
        let (packet, revision, _) = context::check_packet_payload(payload, false)?;
        let Some((mut facts, record)) = self.published(&packet, revision)? else {
            return Err(ProtocolError::not_found());
        };
        let count = list(&record, &["packets"]).len() as i64;
        let current = revision == count;
        let job = match record.get("job").and_then(Value::as_str) {
            Some(job_id) => self
                .context_record(JOB, job_id)?
                .map_or(Value::Null, |(_, job)| job),
            None => Value::Null,
        };
        let data = Self::packet_bytes(&facts)?;
        let mut invalidated = context::invalidated_by_corrections(&facts, &job);
        let mut claim_facts = None;
        if self.selected_feature("context.claims") {
            let basis = at(&record, &["basis"]).clone();
            claim_facts = Some(context::read_time_claims(
                &record,
                &facts,
                &mut invalidated,
                |reference| {
                    let read = self.read_claim(reference);
                    context::claim_snapshot(read.as_ref().map_err(|e| *e), reference, &basis)
                },
            ));
        } else if let Some(Value::Array(sections)) = member_mut(&mut facts, "sections") {
            // Sessions without context.claims see M4 facts.
            for section in sections {
                context::remove(section, "claim");
            }
        }
        context::remove(&mut facts, "body");
        set(&mut facts, "current", Value::Bool(current));
        set(&mut facts, "invalidated_items", Value::Array(invalidated));
        if let Some((changes, unverified)) = claim_facts {
            set(&mut facts, "claim_changes", Value::Array(changes));
            set(&mut facts, "unverified_items", Value::Array(unverified));
        }
        if !current {
            set(
                &mut facts,
                "superseded_by",
                object(vec![("revision", Value::Int(count))]),
            );
        }
        set(
            &mut facts,
            "excerpt",
            context::excerpt(&data, payload, self.caller_receive_limit),
        );
        Ok(facts)
    }

    /// `context.expand` after step 6 authorized the packet read. A citation
    /// the principal may not read, one that does not exist and one held at
    /// another provider are one refusal (CONTEXT section 6).
    pub(super) fn context_expand(
        &self,
        payload: &Value,
        in_force: Option<&Grant>,
    ) -> Result<Value, ProtocolError> {
        let (packet, revision, citation) = context::check_packet_payload(payload, true)?;
        let denied = || ProtocolError::permission_denied("out_of_scope");
        let Some((facts, _)) = self.published(&packet, revision)? else {
            return Err(if self.context_record(REQUEST, &packet)?.is_some() {
                denied()
            } else {
                ProtocolError::not_found()
            });
        };
        let Some(reference) = list(&facts, &["citations"])
            .iter()
            .find(|c| text(c, &["citation_id"]) == citation)
            .map(|c| at(c, &["evidence"]).clone())
        else {
            return Err(denied());
        };
        let provider = text(&reference, &["provider"]);
        if !provider.is_empty() && provider != self.config.provider_id {
            return Err(denied());
        }
        let artifact = text(&reference, &["artifact", "id"]).to_string();
        if !self.may_read(in_force, &crate::evidence::artifact_key(&artifact)) {
            return Err(denied());
        }
        let Some((_, record)) = self.evidence_record(crate::evidence::ARTIFACT, &artifact)? else {
            return Err(denied());
        };
        if text(&record, &["state"]) != "sealed" || record.get("purge").is_some() {
            return Err(denied());
        }
        let sealed = text(&record, &["descriptor", "digest"]);
        if sealed != text(&reference, &["digest"]) {
            return Err(ProtocolError::new(
                "artifact_digest_mismatch",
                crate::errors::Retry::No,
            ));
        }
        let data = self
            .store
            .read_object(sealed)?
            .filter(|bytes| cbr_encoding::digest_bytes(bytes) == sealed)
            .ok_or_else(ProtocolError::new_internal_error)?;
        Ok(object(vec![
            ("citation", string(&citation)),
            ("evidence", reference),
            (
                "excerpt",
                context::excerpt(&data, payload, self.caller_receive_limit),
            ),
        ]))
    }

    /// Read a claim revision as an ordinary reader (CONTEXT section 14): at
    /// the configured knowledge provider when the reference names it, or in
    /// this provider's own knowledge store when it names this provider.
    /// Anything else, and any failure, is unavailable knowledge.
    fn read_claim(&self, reference: &Value) -> Result<Value, &'static str> {
        let payload = object(vec![
            ("claim", at(reference, &["claim"]).clone()),
            ("revision", at(reference, &["revision"]).clone()),
        ]);
        let provider = text(reference, &["provider"]);
        match PeerConfig::from_value(self.config.context.0.get("knowledge_provider")) {
            Some(knowledge) if knowledge.provider_id == provider => {
                let mut peer = peer::Peer::connect(&knowledge, "knowledge")
                    .map_err(|_| context::KNOWLEDGE_UNAVAILABLE)?;
                peer.query("knowledge.claim.inspect", payload)
                    .map_err(|_| context::KNOWLEDGE_UNAVAILABLE)
            }
            None if provider == self.config.provider_id => self
                .knowledge_inspect(&payload)
                .map_err(|_| context::KNOWLEDGE_UNAVAILABLE),
            _ => Err(context::KNOWLEDGE_UNAVAILABLE),
        }
    }

    // ---- preparation (sections 4, 5, 8 and 12) -----------------------------

    /// Advance every running job against the provider clock, one owner
    /// transaction per job.
    pub(super) fn tick_context(&mut self) -> Result<(), ProtocolError> {
        let jobs = self.store.subjects_in_recorded_order(JOB)?;
        for (job_id, _, value) in jobs {
            let mut job = parse_record(&value)?;
            if text(&job, &["state"]) != "running" {
                continue;
            }
            let before = canonical(&job);
            let mut tick = Tick {
                now: self.clock.now(),
                batch: Batch::default(),
                captures: Vec::new(),
            };
            match self.advance(&mut job, &mut tick) {
                Ok(ended) => {
                    if canonical(&job) != before {
                        let job_key = key(JOB, &job_id);
                        let revision = self.tick_save(&mut tick, &job_key, &job)?;
                        if let Some(reason) = ended {
                            tick.batch.event(
                                &job_key,
                                revision,
                                "context.job.ended",
                                object(vec![("reason", string(&reason))]),
                            );
                        }
                    }
                    if !tick.batch.writes.is_empty() {
                        self.store.commit_provider_batch(
                            &tick.batch.writes,
                            &tick.batch.events,
                            &tick.now,
                        )?;
                    }
                }
                Err(TickError::NotSealed(not_sealed)) => {
                    // Nothing of this step is committed; only the capture
                    // instants are kept, so the retry replays.
                    eprintln!(
                        "cbr-provider: context job {job_id}: packet {} not sealed at the \
                         evidence provider ({}); retrying at a later tick",
                        not_sealed.artifact,
                        not_sealed.failure.describe()
                    );
                    self.keep_captures(&job_id, &not_sealed.captures)?;
                }
                Err(TickError::Protocol(error)) => return Err(error),
            }
        }
        Ok(())
    }

    fn keep_captures(
        &mut self,
        job_id: &str,
        captures: &[(String, String)],
    ) -> Result<(), ProtocolError> {
        let Some((revision, mut job)) = self.context_record(JOB, job_id)? else {
            return Ok(());
        };
        let before = canonical(&job);
        for (artifact, captured_at) in captures {
            if context::is_null(at(&job, &["captures", artifact])) {
                context::set_in(&mut job, "captures", artifact, string(captured_at));
            }
        }
        if canonical(&job) == before {
            return Ok(());
        }
        let write = ProviderWrite {
            key: key(JOB, job_id),
            base: revision,
            revision: revision + 1,
            value: canonical(&job),
        };
        let now = self.clock.now();
        Ok(self.store.commit_provider_batch(&[write], &[], &now)?)
    }

    /// Run a job's steps until one waits. Returns the reason the job ended,
    /// if it did.
    fn advance(&self, job: &mut Value, tick: &mut Tick) -> Result<Option<String>, TickError> {
        loop {
            let cursor = int(job, &["cursor"]);
            let Some(step) = usize::try_from(cursor)
                .ok()
                .and_then(|index| list(job, &["script"]).get(index).cloned())
            else {
                break;
            };
            let (name, argument) = context::step(&step);
            match name {
                "wait_until" => {
                    if tick.now.as_str() < argument.as_str().unwrap_or_default() {
                        break;
                    }
                }
                "stall" => break,
                "investigate" => {
                    let spent = int(job, &["spent"]) + argument_int(argument);
                    set(job, "spent", Value::Int(spent));
                    // The investigation budget is its own limit: exhausting it
                    // ends the job with that reason and no other.
                    if spent > int(job, &["limits", "investigation", "amount"]) {
                        let reason = "investigation_budget_exhausted";
                        self.finish(job, tick, reason)?;
                        return Ok(Some(reason.into()));
                    }
                }
                "section" => {
                    let mut section = argument.clone();
                    set(&mut section, "historical", Value::Bool(false));
                    if let Some(reference) = argument.get("claim") {
                        let read = self.read_claim(reference);
                        let snapshot = context::claim_snapshot(
                            read.as_ref().map_err(|e| *e),
                            reference,
                            at(job, &["basis"]),
                        );
                        context::attach_claim(&mut section, snapshot);
                    }
                    // Content derived from an authority revision already
                    // corrected is never current for its item.
                    if let (Some(item), Some(Value::Int(derived))) = (
                        argument.get("item_id").and_then(Value::as_str),
                        argument.get("authority_revision"),
                    ) && matches!(at(job, &["corrections", item]), Value::Int(c) if c > derived)
                    {
                        set(&mut section, "historical", Value::Bool(true));
                        set(&mut section, "label", string("stale"));
                    }
                    context::push(job, "sections", section);
                }
                "coverage" => context::push(job, "coverage", argument.clone()),
                "unmet" => {
                    let item = text(argument, &["item_id"]).to_string();
                    context::set_in(job, "unmet", &item, at(argument, &["reason"]).clone());
                }
                "omit" => context::push(job, "omissions", argument.clone()),
                "correction" => {
                    let item = text(argument, &["item_id"]).to_string();
                    let corrected = int(argument, &["authority_revision"]);
                    context::set_in(job, "corrections", &item, Value::Int(corrected));
                    if let Some(Value::Array(sections)) = member_mut(job, "sections") {
                        for section in sections {
                            let older = section.get("item_id").and_then(Value::as_str)
                                == Some(item.as_str())
                                && matches!(section.get("authority_revision"), Some(Value::Int(r)) if *r < corrected);
                            if older {
                                set(section, "historical", Value::Bool(true));
                                set(section, "label", string("stale"));
                            }
                        }
                    }
                }
                "conditions" => set(job, "conditions", argument.clone()),
                "publish" => {
                    if !self.publish_all(job, tick, None)? {
                        break;
                    }
                }
                "end" => {
                    let reason = argument.as_str().unwrap_or("ended").to_string();
                    self.finish(job, tick, &reason)?;
                    return Ok(Some(reason));
                }
                // A step this provider does not perform, such as an
                // investigation execution, holds the job where it is rather
                // than pretending it ran.
                _ => break,
            }
            set(job, "cursor", Value::Int(cursor + 1));
        }
        // The deadline: whatever a preparing request still lacks is published
        // now, required items unmet and advisory ones degraded.
        let requests: Vec<String> = list(job, &["requests"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        for request in requests {
            if let Some((_, record)) = self.tick_record(tick, REQUEST, &request)?
                && text(&record, &["state"]) == "preparing"
                && tick.now.as_str() >= text(&record, &["limits", "deadline"])
            {
                self.publish_one(job, tick, &request, Some("deadline_passed"))?;
            }
        }
        Ok(None)
    }

    fn finish(&self, job: &mut Value, tick: &mut Tick, reason: &str) -> Result<(), TickError> {
        self.publish_all_finishing(job, tick, reason)?;
        set(job, "state", string("ended"));
        set(job, "reason", string(reason));
        Ok(())
    }

    fn publish_all_finishing(
        &self,
        job: &mut Value,
        tick: &mut Tick,
        reason: &str,
    ) -> Result<(), TickError> {
        for request in subscribers(job) {
            self.publish_one(job, tick, &request, Some(reason))?;
        }
        set(job, "published", Value::Bool(true));
        Ok(())
    }

    /// A scripted `publish`. It waits instead while a subscriber's advisory
    /// items are unsatisfied under `wait_until_deadline` and the deadline has
    /// not passed (CONTEXT section 12).
    fn publish_all(
        &self,
        job: &mut Value,
        tick: &mut Tick,
        reason: Option<&str>,
    ) -> Result<bool, TickError> {
        let requests = subscribers(job);
        for request in &requests {
            if let Some((_, record)) = self.tick_record(tick, REQUEST, request)?
                && text(&record, &["state"]) == "preparing"
                && text(&record, &["fallback"]) == "wait_until_deadline"
                && tick.now.as_str() < text(&record, &["limits", "deadline"])
                && context::item_results(&record, job, false, None)
                    .iter()
                    .any(|i| {
                        text(i, &["obligation"]) == "advisory"
                            && text(i, &["result"]) != "satisfied"
                    })
            {
                return Ok(false);
            }
        }
        for request in &requests {
            self.publish_one(job, tick, request, reason)?;
        }
        set(job, "published", Value::Bool(true));
        Ok(true)
    }

    /// Publish the next packet revision for one request, if it is due one: a
    /// preparing request always is, and a published one only when it was
    /// submitted under `context.updates` (CONTEXT section 8).
    fn publish_one(
        &self,
        job: &Value,
        tick: &mut Tick,
        request: &str,
        reason: Option<&str>,
    ) -> Result<(), TickError> {
        let Some((_, mut record)) = self.tick_record(tick, REQUEST, request)? else {
            return Ok(());
        };
        let due = match text(&record, &["state"]) {
            "preparing" => true,
            "ready" | "partial" | "unmet" => at(&record, &["updates"]) == &Value::Bool(true),
            _ => false,
        };
        if !due {
            return Ok(());
        }
        let past_deadline = tick.now.as_str() >= text(&record, &["limits", "deadline"]);
        let reason = reason.or(past_deadline.then_some("deadline_passed"));
        let packet =
            context::compile_packet(request, &record, job, reason, context::SCRIPT_COMPILER);

        let provider = match PeerConfig::from_value(self.config.context.0.get("evidence_provider"))
        {
            Some(evidence) => {
                let captured_at = match at(job, &["captures", &packet.artifact]).as_str() {
                    Some(kept) => kept.to_string(),
                    None => tick.now.clone(),
                };
                tick.captures
                    .push((packet.artifact.clone(), captured_at.clone()));
                let descriptor = context::packet_descriptor(
                    request,
                    &packet.digest,
                    packet.content.len(),
                    &captured_at,
                );
                if let Err(failure) =
                    peer::publish_artifact(&evidence, &packet.artifact, descriptor, &packet.content)
                {
                    return Err(TickError::NotSealed(NotSealed {
                        artifact: packet.artifact,
                        failure,
                        captures: tick.captures.clone(),
                    }));
                }
                evidence.provider_id
            }
            None => {
                self.seal_locally(tick, request, &packet)?;
                self.config.provider_id.clone()
            }
        };

        let reference = context::packet_reference(
            request,
            packet.revision,
            &provider,
            &packet.artifact,
            &packet.digest,
        );
        let mut facts = packet.facts;
        set(&mut facts, "reference", reference.clone());
        context::push(&mut record, "packets", facts);
        let changed = text(&record, &["state"]) != packet.state;
        set(&mut record, "state", string(packet.state));
        let request_key = key(REQUEST, request);
        // One publication raises the request's revision once; its events share
        // that revision.
        let revision = self.tick_save(tick, &request_key, &record)?;
        tick.batch.event(
            &request_key,
            revision,
            "context.packet.published",
            object(vec![
                ("reference", reference),
                ("items", Value::Array(packet.items)),
            ]),
        );
        if changed {
            tick.batch.event(
                &request_key,
                revision,
                "context.request.changed",
                object(vec![("state", string(packet.state))]),
            );
        }
        Ok(())
    }

    /// Seal a packet revision in this provider's own store, as an artifact
    /// whose producer principal is this provider (CONTEXT section 5). The
    /// object is published and verified from disk before the batch naming it
    /// commits; if the commit never happens, the start-time collection pass
    /// removes the object.
    fn seal_locally(
        &self,
        tick: &mut Tick,
        request: &str,
        packet: &context::Packet,
    ) -> Result<(), TickError> {
        let artifact_key = crate::evidence::artifact_key(&packet.artifact);
        if self.store.revision(&artifact_key)? != 0 || tick.batch.written(&artifact_key).is_some() {
            // An artifact already holds the conventional id. Overwriting it
            // would rewrite sealed evidence, so the packet is not published.
            eprintln!(
                "cbr-provider: context request {request}: artifact {} already exists; the \
                 packet is not published",
                packet.artifact
            );
            return Err(TickError::Protocol(ProtocolError::new_internal_error()));
        }
        let payload =
            context::packet_descriptor(request, &packet.digest, packet.content.len(), &tick.now);
        let descriptor = crate::evidence::parse_descriptor(&payload, &self.config.provider_id)?;
        self.store.publish_object(&packet.digest, &packet.content)?;
        let staged = object(vec![
            ("descriptor", descriptor.clone()),
            ("state", string("staged")),
            ("received", Value::Int(packet.content.len() as i64)),
            ("staged_at", string(&tick.now)),
        ]);
        let staged_revision = self.tick_save(tick, &artifact_key, &staged)?;
        tick.batch.event(
            &artifact_key,
            staged_revision,
            "evidence.artifact.staged",
            object(vec![("descriptor", descriptor.clone())]),
        );
        let sealed = object(vec![
            ("descriptor", descriptor),
            ("state", string("sealed")),
            ("staged_at", string(&tick.now)),
        ]);
        let sealed_revision = self.tick_save(tick, &artifact_key, &sealed)?;
        tick.batch.event(
            &artifact_key,
            sealed_revision,
            "evidence.artifact.sealed",
            object(vec![
                ("digest", string(&packet.digest)),
                ("size", Value::Int(packet.content.len() as i64)),
            ]),
        );
        Ok(())
    }

    /// Whether a context subject is visible under a grant (CONTEXT section
    /// 10): a request with `context.read` over it, a job when any of its
    /// requests is.
    pub(super) fn context_visible(&self, grant: &Grant, subject_key: &SubjectKey) -> bool {
        match subject_key.kind.as_str() {
            REQUEST => grant.may_read(subject_key, "context.read"),
            PACKET => grant.may_read(subject_key, "context.packet.read"),
            JOB => {
                let mut requests = vec![subject_key.id.clone()];
                if let Ok(Some((_, job))) = self.context_record(JOB, &subject_key.id) {
                    requests.extend(
                        list(&job, &["requests"])
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string),
                    );
                }
                requests
                    .iter()
                    .any(|request| grant.may_read(&key(REQUEST, request), "context.read"))
            }
            _ => false,
        }
    }
}

fn argument_int(value: &Value) -> i64 {
    match value {
        Value::Int(number) => *number,
        _ => 0,
    }
}

fn subscribers(job: &Value) -> Vec<String> {
    list(job, &["requests"])
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn member_mut<'a>(target: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    match target {
        Value::Object(members) => members
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, value)| value),
        _ => None,
    }
}
