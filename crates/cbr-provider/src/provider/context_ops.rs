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
use crate::compiler::{self, Decided, Reach, Selection, Unmet};
use crate::context::{
    self, JOB, PACKET, REQUEST, at, canonical, int, key, list, object, set, string, subject, text,
};
use crate::envelope::Command;
use crate::errors::ProtocolError;
use crate::grants::Grant;
use crate::peer::{self, PeerConfig};
use crate::store::{Change, Commit, NewEvent, ProviderEvent, ProviderWrite, SubjectKey};

/// One repository the compiler may read, at the tree the basis names.
struct Frontier {
    repository: String,
    tree: String,
    checkout: std::path::PathBuf,
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

    /// The repositories this session may read, as
    /// [`crate::repositories::view`] resolves them from the grant the command
    /// named. Only the ids travel into the job: a path is local.
    fn readable_repositories(&self, params: &Value) -> Result<Vec<String>, ProtocolError> {
        let named = params.get("grant").and_then(Value::as_str);
        let grant = self.authorize(named, &[])?;
        Ok(crate::repositories::view(&self.store, grant.as_ref())
            .map_err(|_| ProtocolError::new_internal_error())?
            .into_iter()
            .map(|visible| visible.id)
            .collect())
    }

    /// The claims this session may read, resolved **at the command, where
    /// the grant is**.
    ///
    /// Preparation runs later, on the provider's own authority, and
    /// `knowledge_inspect` authorizes nothing: it is the body of an
    /// operation whose authorization happened at step 6 of the command that
    /// called it. Discovery called it directly and so read every claim in
    /// the store. The repository view had this shape from the start; claims
    /// did not, and nothing covered them.
    fn readable_claims(&self, params: &Value) -> Result<Vec<String>, ProtocolError> {
        let named = params.get("grant").and_then(Value::as_str);
        let grant = self.authorize(named, &[])?;
        let mut readable = Vec::new();
        for (claim, _) in self.store.subjects_of_kind(crate::knowledge::CLAIM)? {
            let permitted = match &grant {
                // The authority principal holds no grant and reads its own
                // store; anyone else reads exactly what a grant covers.
                None => true,
                Some(grant) => {
                    grant.may_read(&key(crate::knowledge::CLAIM, &claim), "knowledge.read")
                }
            };
            if permitted {
                readable.push(claim);
            }
        }
        readable.sort();
        Ok(readable)
    }

    /// The deterministic compiler, run as the job's first step
    /// (INTERNALS section 5). Returns the steps that replace the marker.
    fn compile(&self, job: &Value, tick: &mut Tick) -> Result<Vec<Value>, TickError> {
        let view: Vec<String> = list(job, &["view"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let mut decided = Decided::default();
        let mut trees: Vec<Frontier> = Vec::new();

        // Steps 1 and 2: the view, and a frontier for each repository in it.
        for repository in list(job, &["basis", "repositories"]) {
            let id = text(repository, &["id"]).to_string();
            let tree = text(repository, &["tree"]).to_string();
            if !view.contains(&id) {
                continue;
            }
            let Some(checkout) = self
                .store
                .repository_checkout(&id)
                .map_err(|_| ProtocolError::new_internal_error())?
            else {
                continue;
            };
            let reach = self.ensure_index(&id, &checkout, &tree)?;
            decided.reach.push(reach);
            trees.push(Frontier {
                repository: id,
                tree,
                checkout,
            });
        }

        // Steps 3 and 4: one selection per item, inside the view.
        let request = list(job, &["requests"])
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let record = self
            .tick_record(tick, REQUEST, &request)?
            .map(|(_, record)| record)
            .unwrap_or(Value::Null);
        for item in list(job, &["items"]) {
            self.decide_item(item, &record, &trees, &mut decided, tick)?;
        }

        // Step 3 again, for what no item named: the task is a question, and
        // a packet that answers only what was already located is a pointer
        // list. Everything here is advisory and is dropped by capacity
        // before anything an item required.
        self.discover(&record, job, &trees, &mut decided, tick)?;

        let mut steps = compiler::steps(&decided);
        // Step 5's refusal, moved to where a compiled request can know it:
        // the sizes exist only once something has been selected.
        let capacity = int(job, &["limits", "output_capacity", "amount"]);
        if context::mandatory_size(&steps, list(job, &["items"])) > capacity {
            steps = vec![object(vec![("end", string("budget_insufficient"))])];
        }
        Ok(steps)
    }

    /// The words a request is about: its task, and every selector.
    fn question(record: &Value, job: &Value) -> String {
        let mut words = vec![text(record, &["consumer", "task"]).to_string()];
        for item in list(job, &["items"]) {
            words.push(text(at(item, &["selector"]), &["value"]).to_string());
        }
        words.join(" ")
    }

    /// The names in a request that could be code.
    ///
    /// **Not every word of the task.** A prose question contains ordinary
    /// words, and ordinary words are function names somewhere in any large
    /// repository: asking about "the git binary" pulled in every helper
    /// called `git` and every one called `binary`, which is noise wearing
    /// the clothes of code flow. So a name comes from a selector, which is
    /// chosen words, or from a word in the task that is shaped like an
    /// identifier -- `snake_case`, `camelCase`, or dotted.
    fn symbols(record: &Value, job: &Value) -> Vec<String> {
        let identifier = |word: &str| {
            word.contains('_')
                || word.contains('.')
                || (word.chars().next().is_some_and(char::is_lowercase)
                    && word.chars().any(char::is_uppercase))
        };
        let mut names: Vec<String> = Vec::new();
        for item in list(job, &["items"]) {
            names.extend(
                text(at(item, &["selector"]), &["value"])
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|word| word.len() > 2)
                    .map(str::to_string),
            );
        }
        for word in text(record, &["consumer", "task"]).split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.');
            if trimmed.len() > 2 && identifier(trimmed) {
                names.push(trimmed.to_string());
                // A dotted name is also its parts: `np.concatenate` is worth
                // looking up as `concatenate` when the module is not anchored.
                names.extend(
                    trimmed
                        .split('.')
                        .filter(|part| part.len() > 2)
                        .map(str::to_string),
                );
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Sections no item asked for (CONTEXT section 6): spans the question
    /// finds, the definitions and callers of the names in it, and the claims
    /// that bear on the basis.
    fn discover(
        &self,
        record: &Value,
        job: &Value,
        trees: &[Frontier],
        decided: &mut Decided,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        let question = Self::question(record, job);
        self.discover_spans(&question, trees, decided, tick)?;
        self.discover_anchors(&Self::symbols(record, job), trees, decided)?;
        self.discover_claims(record, job, decided)?;
        Ok(())
    }

    /// Retrieval over the whole view, from the question rather than from a
    /// path. Spans an item already cited are skipped: a packet should not
    /// pay twice for the same bytes.
    fn discover_spans(
        &self,
        question: &str,
        trees: &[Frontier],
        decided: &mut Decided,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        use cbr_memory::retrieval::{Ask, Bounds, Readable};
        let readable: Vec<Readable<'_>> = trees
            .iter()
            .map(|frontier| Readable {
                id: &frontier.repository,
                checkout: &frontier.checkout,
                basis: &frontier.tree,
            })
            .collect();
        if readable.is_empty() {
            return Ok(());
        }
        // Ask for more rows than the packet will carry: the per-path cap
        // below discards some of them, and asking for exactly the budget
        // would leave the packet short whenever one file ranked twice.
        let bounds = Bounds {
            rows: compiler::DISCOVERED_SPANS * 4,
            batch_bytes: usize::MAX,
            ..Bounds::default()
        };
        // **A question is prose, and every-term over prose is the wrong
        // reading.** Asking for every term of "why does source identity use
        // gix rather than the git binary" returns only chunks that quote the
        // question -- which, in a repository that contains the test asking
        // it, is exactly what the first version of this returned, while the
        // answer went unfound. The ranked partial reading is the right one
        // here: BM25 already weights the rare terms that carry the question.
        // A selector inside a named path keeps the every-term reading, which
        // is what `select_source` uses, because a selector is chosen words.
        let answer = cbr_memory::retrieval::search(
            self.store.connection(),
            &readable,
            &Ask::all(question).partial(),
            &bounds,
        )
        .map_err(|_| ProtocolError::new_internal_error())?;

        let mut per_path: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut taken = 0;
        for found in answer.found {
            if taken >= compiler::DISCOVERED_SPANS {
                break;
            }
            let seen = per_path.entry(found.path.clone()).or_default();
            if *seen >= compiler::DISCOVERED_PER_PATH {
                continue;
            }
            let already = decided.selections.iter().any(|selection| {
                selection.blob == found.blob && selection.start_byte == found.start_byte
            });
            if already {
                continue;
            }
            *seen += 1;
            let Some(frontier) = trees
                .iter()
                .find(|frontier| frontier.repository == found.repository)
            else {
                continue;
            };
            let Ok(Some(bytes)) = cbr_identity::read_blob_bounded(
                &frontier.checkout,
                &found.blob,
                cbr_memory::index::MAX_BLOB_BYTES as u64,
            ) else {
                continue;
            };
            let selection = Selection {
                item: String::new(),
                repository: found.repository.clone(),
                tree: frontier.tree.clone(),
                path: found.path.clone(),
                blob: found.blob.clone(),
                digest: cbr_encoding::digest_bytes(&bytes),
                size: bytes.len(),
                start_byte: found.start_byte,
                end_byte: found.end_byte,
                start_line: found.start_line,
                end_line: found.end_line,
                origin: "index",
                excerpt: compiler::excerpt(&bytes, found.start_byte, found.end_byte),
            };
            self.seal_source(tick, &selection, &bytes)?;
            decided.discovered.push(compiler::Discovered {
                id: format!("span-{}-{}", selection.path, selection.start_byte),
                rank: compiler::Rank::Span,
                // Retrieval's own order, kept: under capacity pressure a
                // span drops by rank, never by path name.
                order: taken,
                claim: None,
                label: "source_inspected",
                historical: false,
                content: selection.content(),
                citation: Some(object(vec![
                    ("provider", string(&self.config.provider_id)),
                    (
                        "artifact",
                        subject(crate::evidence::ARTIFACT, &selection.artifact()),
                    ),
                    ("digest", string(&selection.digest)),
                ])),
            });
            taken += 1;
        }
        Ok(())
    }

    /// Code flow, as far as tags can honestly take it: where a name in the
    /// question is defined, and where it is used. Tags say a name appears,
    /// never which definition a use means, so ambiguity is stated and
    /// nothing is narrowed.
    fn discover_anchors(
        &self,
        names: &[String],
        trees: &[Frontier],
        decided: &mut Decided,
    ) -> Result<(), TickError> {
        for frontier in trees {
            for name in names {
                let resolution =
                    cbr_memory::anchors::resolve(self.store.connection(), &frontier.tree, name)
                        .map_err(|_| ProtocolError::new_internal_error())?;
                if resolution.candidates.is_empty() {
                    continue;
                }
                let where_defined: Vec<String> = resolution
                    .candidates
                    .iter()
                    .map(|candidate| {
                        format!(
                            "{} {}:{} line {}",
                            candidate.kind,
                            frontier.repository,
                            candidate.path,
                            candidate.start_line
                        )
                    })
                    .collect();
                let callers =
                    cbr_memory::anchors::references(self.store.connection(), &frontier.tree, name)
                        .map_err(|_| ProtocolError::new_internal_error())?;
                let used_at: Vec<String> = callers
                    .iter()
                    .take(16)
                    .map(|candidate| {
                        format!(
                            "{}:{} line {}",
                            frontier.repository, candidate.path, candidate.start_line
                        )
                    })
                    .collect();
                let ambiguity = if resolution.ambiguous {
                    "\nambiguous: every candidate is listed; none is chosen"
                } else {
                    ""
                };
                let uses = if used_at.is_empty() {
                    "no use of this name is anchored at this tree".to_string()
                } else {
                    format!("used at:\n  {}", used_at.join("\n  "))
                };
                decided.discovered.push(compiler::Discovered {
                    id: format!("anchor-{}-{name}", frontier.repository),
                    rank: compiler::Rank::Anchor,
                    order: 0,
                    claim: None,
                    label: "inferred",
                    historical: false,
                    content: format!(
                        "{name} defined at:\n  {}\n{uses}{ambiguity}",
                        where_defined.join("\n  ")
                    ),
                    citation: None,
                });
            }
        }
        Ok(())
    }

    /// The claims that bear on this request, from the set this session may
    /// read.
    ///
    /// **Authorization happened at the command.** `knowledge_inspect` is the
    /// body of an operation whose step 6 ran before it; calling it here, on
    /// the provider's own authority, authorizes nothing. So the job carries
    /// the claims the submitting grant could read, resolved where the grant
    /// was, and a claim outside that set is never read and never named —
    /// identical to a claim that was never proposed.
    ///
    /// **Relevance, stated rather than implied.** Every claim in the store
    /// is not context for every request: a pilot repository with twenty-two
    /// decisions would put all twenty-two in a packet about one of them. A
    /// claim is carried when either
    /// 1. one of its conditions names a repository the basis names — it is
    ///    about the code this request is about; or
    /// 2. its scope or its statement shares a term with the question.
    ///
    /// Anything else is **omitted with reason `applicability`**, one
    /// omission each, so a caller counts what it did not get.
    ///
    /// A claim is labelled by what the authority permitted it for, so
    /// `binding` means an authority said so. A rejected claim, or one not
    /// applicable at this basis, is carried as historical: CONTEXT section
    /// 14 makes a historical section never current for an item and reports
    /// it at the read, which is what INTERNALS section 5 step 3 means by
    /// keeping rejected alternatives distinguishable rather than absent.
    fn discover_claims(
        &self,
        record: &Value,
        job: &Value,
        decided: &mut Decided,
    ) -> Result<(), TickError> {
        let basis = at(record, &["basis"]);
        let repositories: Vec<String> = list(basis, &["repositories"])
            .iter()
            .map(|repository| text(repository, &["id"]).to_string())
            .collect();
        let question = cbr_memory::lexical::query_terms(&Self::question(record, job));
        let kinds: Vec<String> = crate::knowledge::CONDITION_KINDS
            .iter()
            .map(|kind| (*kind).to_string())
            .collect();

        let readable: Vec<String> = list(job, &["readable_claims"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        for claim in readable {
            let payload = object(vec![("claim", string(&claim))]);
            let Ok(inspected) = self.knowledge_inspect(&payload) else {
                continue;
            };
            let claim_record = at(&inspected, &["record"]);

            let about_this_code = list(claim_record, &["conditions"]).iter().any(|condition| {
                repositories.contains(&text(condition, &["repository"]).to_string())
            });
            let words = cbr_memory::lexical::query_terms(&format!(
                "{} {}",
                text(claim_record, &["scope", "id"]),
                crate::context::canonical(at(claim_record, &["statement"]))
            ));
            let shared = question.iter().filter(|term| words.contains(term)).count();
            if !about_this_code && shared == 0 {
                decided.omitted.push(object(vec![
                    ("section_id", string(&format!("d-claim-{claim}"))),
                    ("reason", string("applicability")),
                ]));
                continue;
            }

            let findings = crate::knowledge::condition_findings(claim_record, basis, &kinds);
            let applicability = crate::knowledge::result_of(&findings);
            let state = text(&inspected, &["reliance", "state"]).to_string();
            let permitted = text(&inspected, &["reliance", "permitted_use"]).to_string();
            let decision = text(&inspected, &["reliance", "decision"]).to_string();
            let current = state == "accepted_for_use" && applicability == "applicable";
            let (rank, label) = match permitted.as_str() {
                _ if !current => (compiler::Rank::Historical, "stale"),
                "binding" => (compiler::Rank::BindingClaim, "binding"),
                "evidence" => (compiler::Rank::CurrentClaim, "observation"),
                "hypothesis" => (compiler::Rank::CurrentClaim, "hypothesis"),
                _ => (compiler::Rank::CurrentClaim, "unknown"),
            };
            // "stale" is the nearest label CONTEXT section 14 has, and it
            // says "was once valid", which a rejected claim never was. The
            // content says what actually happened and names the decision
            // that did it.
            let standing = if state == "rejected" {
                match decision.as_str() {
                    "" => "rejected by an authority".to_string(),
                    decision => format!("rejected by decision {decision}"),
                }
            } else if state != "accepted_for_use" {
                format!("not accepted for use: {state}")
            } else if current {
                format!("accepted for use as {permitted}, by decision {decision}")
            } else {
                format!("accepted as {permitted}, but {applicability} at this basis")
            };
            let why = if about_this_code {
                "names a repository of this basis"
            } else {
                "shares terms with the question"
            };

            decided.discovered.push(compiler::Discovered {
                id: format!("claim-{claim}"),
                rank,
                order: usize::MAX - shared,
                claim: Some(at(&inspected, &["reference"]).clone()),
                label,
                historical: !current,
                content: format!(
                    "claim {claim} revision {}: {standing}\nselected because it {why}\n{}",
                    int(&inspected, &["current_revision"]),
                    crate::context::canonical(at(claim_record, &["statement"]))
                ),
                // The claim's own support, where the reader may read it: a
                // binding statement a reader cannot check against evidence
                // is an assertion, not a citation.
                citation: list(claim_record, &["support"])
                    .first()
                    .map(|support| at(support, &["evidence"]).clone()),
            });
        }
        Ok(())
    }

    /// Build the index for `repository` at `tree` unless a manifest already
    /// says it is there, and report what that projection reaches.
    fn ensure_index(
        &self,
        repository: &str,
        checkout: &std::path::Path,
        tree: &str,
    ) -> Result<Reach, TickError> {
        use cbr_memory::retrieval;
        let connection = self.store.connection();
        let position = self
            .store
            .current_epoch()
            .and_then(|epoch| self.store.last_sequence(epoch))
            .unwrap_or(0);
        let existing = retrieval::manifest(connection, repository).ok().flatten();
        let manifest = match existing {
            Some(manifest) if manifest.frontier == tree => Ok(manifest),
            _ => retrieval::build(connection, repository, checkout, tree, position),
        };
        Ok(match manifest {
            Ok(manifest) => {
                let coverage = &manifest.coverage;
                let mut gaps = Vec::new();
                for (count, what) in [
                    (coverage.binary, "blobs were not text"),
                    (coverage.too_large, "blobs were over the size cap"),
                    (
                        coverage.unanchored_language,
                        "blobs are in a language with no anchors",
                    ),
                ] {
                    if count > 0 {
                        gaps.push(format!("{count} {what}"));
                    }
                }
                Reach {
                    repository: repository.to_string(),
                    frontier: manifest.frontier,
                    state: "complete".into(),
                    gaps,
                }
            }
            // A projection that cannot be built is unavailable and says so.
            // It is never silently an empty one.
            Err(error) => Reach {
                repository: repository.to_string(),
                frontier: tree.to_string(),
                state: "unavailable".into(),
                gaps: vec![format!("the index could not be built: {error}")],
            },
        })
    }

    /// One item: what satisfies it, or why nothing does.
    fn decide_item(
        &self,
        item: &Value,
        record: &Value,
        trees: &[Frontier],
        decided: &mut Decided,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        let item_id = text(item, &["item_id"]).to_string();
        let check = at(item, &["check"]);
        match text(check, &["kind"]) {
            "source_included" => {
                let wanted = text(check, &["repository"]);
                let path = text(check, &["path"]).to_string();
                // A repository outside the view is indistinguishable here
                // from one that was never registered: one reason covers
                // both, so nothing leaks through the difference.
                let Some(frontier) = trees.iter().find(|f| f.repository == wanted) else {
                    decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "source_unavailable".into(),
                    });
                    return Ok(());
                };
                match self.select_source(frontier, item, &path, tick)? {
                    Some(selection) => decided.selections.push(selection),
                    None => decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "source_absent_at_basis".into(),
                    }),
                }
            }
            "evidence_included" => {
                let reference = at(check, &["evidence"]).clone();
                let artifact = text(&reference, &["artifact", "id"]).to_string();
                let sealed = self
                    .store
                    .subject(&crate::evidence::artifact_key(&artifact))
                    .map_err(|_| ProtocolError::new_internal_error())?;
                let holds = match &sealed {
                    Some(state) => {
                        let value = parse_record(&state.value)?;
                        text(&value, &["state"]) == "sealed"
                            && text(&value, &["descriptor", "digest"])
                                == text(&reference, &["digest"])
                    }
                    None => false,
                };
                if holds {
                    decided.evidence.push(compiler::EvidenceSection {
                        item: item_id,
                        summary: format!("evidence {artifact}"),
                        evidence: reference,
                    });
                } else {
                    decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "evidence_unavailable".into(),
                    });
                }
            }
            "claim_included" => {
                let reference = at(check, &["claim"]).clone();
                let read = self.read_claim(&reference);
                decided.claims.push(compiler::ClaimSection {
                    item: item_id,
                    reference,
                    read,
                });
            }
            "authority_content_included" => {
                match list(record, &["authority_content"])
                    .iter()
                    .find(|content| text(content, &["item_id"]) == item_id)
                {
                    Some(content) => decided.authority.push(compiler::AuthoritySection {
                        item: item_id,
                        revision: int(content, &["authority_revision"]),
                        evidence: at(content, &["evidence"]).clone(),
                    }),
                    None => decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "authority_content_absent".into(),
                    }),
                }
            }
            _ => decided.unmet.push(Unmet {
                item: item_id,
                reason: "uncheckable_item".into(),
            }),
        }
        Ok(())
    }

    /// The span of `path` this item gets, and the artifact that holds it.
    ///
    /// **Retrieval is scoped to the named path, in SQL.** An unscoped search
    /// ranks this file's best span against every other file's, and in a real
    /// repository the file's own span loses: the first version of this took
    /// rows from the whole repository and then filtered by path, and cited a
    /// file's first twenty lines while the answer was on line 24.
    ///
    /// **A multi-term selector has a recorded ladder.** INTERNALS section 5
    /// leaves the reading to the implementation, so it is written down here
    /// rather than left to whatever the query engine happens to do:
    /// 1. every term inside one chunk;
    /// 2. failing that, the best partial match, ranked;
    /// 3. failing that, the file's first chunk, marked `first-chunk` so a
    ///    reader knows no term was found in the file at all.
    fn select_source(
        &self,
        frontier: &Frontier,
        item: &Value,
        path: &str,
        tick: &mut Tick,
    ) -> Result<Option<Selection>, TickError> {
        use cbr_memory::retrieval::{Ask, Bounds, Origin, Readable};
        let entries = match cbr_identity::tree_entries(&frontier.checkout, &frontier.tree) {
            Ok(entries) => entries,
            Err(_) => return Ok(None),
        };
        let Some(entry) = entries.into_iter().find(|entry| entry.path == path) else {
            return Ok(None);
        };
        // Bounded before the read: a repository nobody in this process
        // controls should not be able to make it allocate a blob in order to
        // decide the blob is too big. A file too large to cite is a file that
        // was not found.
        let Ok(Some(bytes)) = cbr_identity::read_blob_bounded(
            &frontier.checkout,
            &entry.blob,
            cbr_memory::index::MAX_BLOB_BYTES as u64,
        ) else {
            return Ok(None);
        };

        let readable = [Readable {
            id: &frontier.repository,
            checkout: &frontier.checkout,
            basis: &frontier.tree,
        }];
        let query = text(at(item, &["selector"]), &["value"]).to_string();
        let bounds = Bounds {
            rows: 8,
            ..Bounds::default()
        };
        let mut answer = cbr_memory::retrieval::search(
            self.store.connection(),
            &readable,
            &Ask::within(&query, path),
            &bounds,
        )
        .map_err(|_| ProtocolError::new_internal_error())?;
        if answer.found.is_empty() {
            answer = cbr_memory::retrieval::search(
                self.store.connection(),
                &readable,
                &Ask::within(&query, path).partial(),
                &bounds,
            )
            .map_err(|_| ProtocolError::new_internal_error())?;
        }
        let (start_byte, end_byte, start_line, end_line, origin) = match answer.found.first() {
            Some(found) => (
                found.start_byte,
                found.end_byte,
                found.start_line,
                found.end_line,
                match found.origin {
                    Origin::Index => "index",
                    Origin::Canonical => "canonical",
                },
            ),
            None => {
                let contents = String::from_utf8_lossy(&bytes);
                let chunk = cbr_memory::lexical::chunks(&contents, cbr_memory::index::CHUNK_LINES)
                    .into_iter()
                    .next();
                match chunk {
                    Some(chunk) => (
                        chunk.start_byte,
                        chunk.end_byte,
                        chunk.start_line,
                        chunk.end_line,
                        "first-chunk",
                    ),
                    None => (0, 0, 1, 1, "first-chunk"),
                }
            }
        };

        let digest = cbr_encoding::digest_bytes(&bytes);
        let selection = Selection {
            item: text(item, &["item_id"]).to_string(),
            repository: frontier.repository.clone(),
            tree: frontier.tree.clone(),
            path: path.to_string(),
            blob: entry.blob.clone(),
            digest: digest.clone(),
            size: bytes.len(),
            start_byte,
            end_byte,
            start_line,
            end_line,
            origin,
            excerpt: compiler::excerpt(&bytes, start_byte, end_byte),
        };
        self.seal_source(tick, &selection, &bytes)?;
        Ok(Some(selection))
    }

    /// Seal a cited file as evidence, unless an artifact already holds it.
    /// The id is the blob, so citing the same file twice is one artifact.
    fn seal_source(
        &self,
        tick: &mut Tick,
        selection: &Selection,
        bytes: &[u8],
    ) -> Result<(), TickError> {
        let artifact = selection.artifact();
        let artifact_key = crate::evidence::artifact_key(&artifact);
        if self.store.revision(&artifact_key)? != 0 || tick.batch.written(&artifact_key).is_some() {
            return Ok(());
        }
        let payload = compiler::source_descriptor(
            &selection.repository,
            &selection.tree,
            &selection.path,
            &selection.digest,
            selection.size,
            &tick.now,
        );
        let descriptor = crate::evidence::parse_descriptor(&payload, &self.config.principal)?;
        self.store.publish_object(&selection.digest, bytes)?;
        let sealed = object(vec![
            ("descriptor", descriptor.clone()),
            ("state", string("sealed")),
            ("staged_at", string(&tick.now)),
        ]);
        let revision = self.tick_save(tick, &artifact_key, &sealed)?;
        tick.batch.event(
            &artifact_key,
            revision,
            "evidence.artifact.sealed",
            object(vec![
                ("digest", string(&selection.digest)),
                ("size", Value::Int(selection.size as i64)),
            ]),
        );
        Ok(())
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

        // Step 1 of INTERNALS section 5, at the only place it can honestly
        // happen: the command, where the grant is. Preparation runs later and
        // on the provider's own authority, so it is handed the view rather
        // than allowed to compute one, and cannot widen it.
        let view = self.readable_repositories(params)?;
        let claims = self.readable_claims(params)?;
        let mut joined = None;
        if self.selected_feature("context.shared_jobs") {
            for (job_id, _, value) in self.store.subjects_in_recorded_order(JOB)? {
                let mut job = parse_record(&value)?;
                if context::may_join(&job, &principal, payload, &view, &claims) {
                    context::push(&mut job, "requests", string(&id));
                    joined = Some((job_id, job));
                    break;
                }
            }
        }
        let (job_id, job) = joined.unwrap_or_else(|| {
            let mut job = context::new_job(&principal, payload, &script, &id);
            set(
                &mut job,
                "view",
                Value::Array(view.iter().map(|id| string(id)).collect()),
            );
            set(
                &mut job,
                "readable_claims",
                Value::Array(claims.iter().map(|id| string(id)).collect()),
            );
            // Compiling is a production capability, and a conformance
            // launch is a test harness: there, a request with no script is
            // still a request nothing prepares, which is what every context
            // fixture was measured against. The two never overlap, because a
            // production launch serves no test control at all.
            if script.is_empty()
                && self.config.mode == crate::config::Mode::Production
                && self.config.context.0 == Value::Null
            {
                // Nothing scripts this request, so it is compiled. The marker
                // is the first step, and compiling replaces it with what it
                // decided.
                set(
                    &mut job,
                    "script",
                    Value::Array(vec![object(vec![("compile", Value::Object(Vec::new()))])]),
                );
                set(&mut job, "compiler", string(compiler::COMPILER));
            }
            (id.clone(), job)
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
                "compile" => {
                    let compiled = self.compile(job, tick)?;
                    // The marker is replaced by what compiling decided, so
                    // everything after this is the same path a script takes.
                    let mut script: Vec<Value> = list(job, &["script"]).to_vec();
                    script.splice(cursor as usize..=cursor as usize, compiled);
                    set(job, "script", Value::Array(script));
                    continue;
                }
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
                    // A section is current unless it says otherwise. The
                    // compiler says otherwise for a rejected claim and for
                    // one that is not applicable at this basis; a scripted
                    // section never sets it, so this defaults as before.
                    if section.get("historical").is_none() {
                        set(&mut section, "historical", Value::Bool(false));
                    }
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
        // The compiler the job actually ran, not the one this line used to
        // assume. `job.compiler` is set at submit for a compiled request and
        // absent for a scripted one, and a packet's provenance is the only
        // place a reader can tell the two apart.
        let compiler = match at(job, &["compiler"]).as_str() {
            Some(compiler) => compiler.to_string(),
            None => context::SCRIPT_COMPILER.to_string(),
        };
        let packet = context::compile_packet(request, &record, job, reason, &compiler);

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
        crate::barriers::pause(crate::barriers::PACKET_AFTER_OBJECT_PUBLISHED);
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
