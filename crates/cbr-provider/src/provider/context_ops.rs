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

use std::collections::BTreeMap;

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
/// How many symbolic links are followed before the chain is refused. A
/// link to a link is ordinary; eight of them is a loop or a trap.
const LINK_HOPS: usize = 8;

/// The request authorised an investigation and it did not stretch this
/// far.
///
/// The same name the job's own `investigate` script step ends a job
/// with, because it is the same limit reached in two places: there by a
/// script asking for more turns than the request allowed, here by a
/// compile asking for more questions than it allowed. One limit, one
/// name.
const INVESTIGATION_EXHAUSTED: &str = "investigation_budget_exhausted";

/// The section ids discovery's two steps are omitted under when they
/// cannot run, and the `item` a derivation made for one is recorded
/// against.
///
/// **Discovery belongs to no item**, which is the whole of what it is
/// for: CONTEXT section 6 content is what nobody asked for. So the
/// record names the step rather than an item, and a reader looking for
/// why a packet discovered what it did has a name to look for.
const TERMS_SECTION: &str = "d-model-terms";
const CHOICE_SECTION: &str = "d-model-choice";
const DISCOVERY_ITEM: &str = "";

/// A symbolic link's blob holds a path. Anything longer than this is not
/// one, and is refused without being read into memory.
const LINK_TARGET_BYTES: u64 = 4096;

/// What the path an item named turned out to be.
enum Link {
    /// A regular file of this tree, at the path the item named.
    Direct,
    /// A link whose chain ended at a regular file of this tree.
    Followed { target: String },
    /// A link this tree cannot resolve: out of the tree, absolute, a
    /// loop, or pointing at something that is not a regular file.
    Broken { link: String, target: String },
    /// Nothing of that path in this tree at all.
    Absent,
}

/// Resolve `target` against the directory `from` sits in, staying inside
/// the tree. Returns `None` for an absolute path or one that climbs out,
/// because a tree has no parent to climb into.
fn join_in_tree(from: &str, target: &str) -> Option<String> {
    if target.is_empty() || target.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    let joined = parts.join("/");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

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
    /// **Every packet the tick publishes, held until the publication guard
    /// has seen the last of them** ([`Provider::seal_packets`]). A tick can
    /// publish at more than one step, and a packet at a later step does not
    /// exist until the steps before it have run, so no packet leaves the
    /// tick while any step is left.
    outgoing: Vec<Outgoing>,
}

/// A packet revision that has passed the guard and not yet left the tick.
enum Outgoing {
    /// To be captured, sent to the evidence peer and sealed there.
    Peer {
        evidence: PeerConfig,
        request: String,
        artifact: String,
        digest: String,
        content: Vec<u8>,
    },
    /// To be written as this provider's own object, which the tick's batch
    /// already names as a sealed artifact.
    Object { digest: String, content: Vec<u8> },
}

enum TickError {
    Protocol(ProtocolError),
    NotSealed(NotSealed),
    /// **Work this job needs has left the preparation tick and has not
    /// finished.** Nothing of this step is committed and the job is left
    /// exactly as it was, so the next tick asks again — which is the whole
    /// point of the work leaving: the tick that asks is the tick that
    /// returns, and every other job on this provider carries on.
    NotReady,
    /// **A packet about to be published names an id outside the protocol's
    /// identifier grammar**, leaves out one the schema requires, or names
    /// one section or citation id twice
    /// ([`context::ids_outside_grammar`]). Nothing of the job's tick is
    /// committed, and no packet of it is captured, sealed or sent, a valid
    /// one the tick published before this one included: each is held in
    /// [`Tick::outgoing`] until the tick has run. The job then ends as
    /// `packet_invalid`, from its stored records, in a batch of its own
    /// ([`Provider::end_refused_job`]), and the tick moves on to the next
    /// job, as it does for `NotSealed`: one bad packet must not stop every
    /// other job on the provider, which routing it through `Protocol` would.
    /// `pointers` say where; the values are never kept, because an id is
    /// often a path and this is logged.
    IdOutsideGrammar {
        request: String,
        pointers: Vec<String>,
    },
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

/// What a model call at this call site needs to know about the request it
/// serves: who it is charged to, what it was asked, and whether it may be
/// made at all.
struct Assist<'a> {
    job: &'a str,
    request: &'a str,
    task: &'a str,
    /// The request's investigation budget, **in questions**.
    ///
    /// **Zero is the deterministic path.** A request that authorised no
    /// investigation gets exactly the compiler M3 shipped, which is what
    /// makes the same binary the baseline m4e's journeys are scored
    /// against rather than a second build nobody ran. MODEL-RUNTIME §63
    /// is where the meaning comes from: *an investigation can use several
    /// turns: select a source, inspect it, update findings*.
    ///
    /// **It counts at m4e, where at m4c it only gated.** m4c asked a
    /// model for every item of any request whose budget was above zero,
    /// so the number said *whether* and not *how much*. m4e adds two
    /// more questions per request, and a limit that counted some kinds
    /// of call and not others would be two meanings for one number. So
    /// every distinct question a compile puts to a model spends one,
    /// items first because an item is what the request required and
    /// discovery is advisory.
    investigation: i64,
    /// Every distinct question this compile has put to a model, in the
    /// order it asked.
    ///
    /// **One compile pass, recomputed each tick.** A compile asks the
    /// same questions in the same order every tick — the basis is fixed
    /// by the job and the pool keeps a settled answer until its caller
    /// takes it — so counting inside one pass gives the same numbers on
    /// each of them. A question already in this list is free: the answer
    /// is the pool's and no second call is made for it.
    asked: std::cell::RefCell<Vec<String>>,
    /// The request's own deadline and the tick's instant, both as the
    /// protocol writes them. Their difference is what the pool is given.
    deadline: &'a str,
    /// Owned, because the tick it comes from is borrowed mutably by
    /// everything this is passed to.
    now: String,
    /// **The job's own readable set**, carried so a derivation record can
    /// be sealed under it. Resolved at the command, where the grant is;
    /// preparation never computes one and so cannot widen it.
    view: &'a [String],
    claims: &'a [String],
    /// **The evidence artifacts the job may read**, resolved at the
    /// command like the view and the claims (m5a). A projection copies an
    /// artifact's bytes into a packet, so an artifact outside this is
    /// `evidence_unavailable` exactly as one that was never sealed is.
    evidence: &'a [String],
}

impl Assist<'_> {
    /// Whether `key` may be put to a model: either this compile has
    /// asked it already, or the investigation budget has room for one
    /// more question.
    ///
    /// **Counted here rather than where the call is made**, because a
    /// question deferred by the work pool is still a question this
    /// compile asked, and a budget that only counted settled answers
    /// would admit as many calls as there are ticks.
    fn admits(&self, key: &str) -> bool {
        let mut asked = self.asked.borrow_mut();
        if asked.iter().any(|already| already == key) {
            return true;
        }
        if (asked.len() as i64) >= self.investigation {
            return false;
        }
        asked.push(key.to_string());
        true
    }

    /// Whether the budget has room for `count` more distinct questions.
    ///
    /// **A flow that cannot finish is not started.** Discovery is two
    /// questions and the second one is what turns the first one's terms
    /// into sections; asking for terms with no room left to choose from
    /// them would spend a shared quota on an answer nothing can use.
    fn room_for(&self, count: usize) -> bool {
        self.investigation
            .saturating_sub(self.asked.borrow().len() as i64)
            >= count as i64
    }
}

/// What selecting a source decided.
enum Choice {
    Selected(Box<Selection>),
    /// Nothing of that path at this basis.
    Absent,
    /// A model call this request asked for, which could not be made or
    /// could not be used. **A typed reason the item carries**, never a
    /// quiet fall back to the span BM25 ranked first: that would report a
    /// model-assisted selection no model made.
    Unmet(&'static str),
}

/// What discovery's two model steps decided.
struct Widened {
    /// The spans the packet draws its discovered sections from, in the
    /// order they were offered — which is the deterministic path's own
    /// ranking, for the part that came from it.
    spans: Vec<cbr_memory::retrieval::Found>,
    /// The claims a model chose, or `None` for the deterministic cut.
    claims: Option<Vec<String>>,
    /// The section id of a step that could not run, so the packet can
    /// say one did not happen.
    omitted: Option<String>,
}

/// Which discovery step a question is.
///
/// It decides two things and nothing else: what the request body says,
/// and how the reply is read. Everything around it — the budget, the
/// pool, the record, the rebuild — is the same for both, which is why
/// there is one `ask_step` and not two.
enum Step {
    Terms,
    /// The terms the first step proposed, which the body prints and the
    /// question's digest covers.
    Choose(Vec<String>),
    /// **One part of a J2 projection** (m5a): what the body says about
    /// the document, and how each candidate is labelled. Its selector
    /// names the artifact, its digest, the projection's format and the
    /// part, so the question's digest does too.
    Project(crate::projection::Part),
}

/// What asking a discovery step came to, this tick.
enum Asked {
    Answered(crate::derivation::Answer),
    /// Nothing yet. Ask again next tick; nothing is written meanwhile.
    NotReady,
}

/// What asking the model came to, this tick.
enum Assisted {
    /// The index into the candidates it chose.
    Chose(usize),
    /// Nothing yet. Ask again next tick; nothing is written meanwhile.
    NotReady,
    Unmet(&'static str),
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

    /// The evidence artifacts a request's items name that this session
    /// may read, resolved **at the command, where the grant is** (m5a).
    ///
    /// A J2 projection copies an artifact's bytes into a packet, so a
    /// packet is a way to read the artifact — and preparation runs later,
    /// on the provider's own authority. Before m5a an `evidence_included`
    /// section only named the artifact; it now carries its bytes, so the
    /// right to read them is checked where the view and the claims are.
    fn readable_evidence(
        &self,
        params: &Value,
        payload: &Value,
    ) -> Result<Vec<String>, ProtocolError> {
        let named = params.get("grant").and_then(Value::as_str);
        let grant = self.authorize(named, &[])?;
        let mut readable = Vec::new();
        for item in list(payload, &["items"]) {
            let check = at(item, &["check"]);
            if text(check, &["kind"]) != "evidence_included" {
                continue;
            }
            let artifact = text(check, &["evidence", "artifact", "id"]).to_string();
            if self.may_read(grant.as_ref(), &crate::evidence::artifact_key(&artifact))
                && !readable.contains(&artifact)
            {
                readable.push(artifact);
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
            let dirty = at(repository, &["dirty", "snapshot_digest"])
                .as_str()
                .map(str::to_string);
            let reach = self.ensure_index(&id, &checkout, &tree, dirty.as_deref())?;
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
        let claims: Vec<String> = list(job, &["readable_claims"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let evidence: Vec<String> = list(job, &["readable_evidence"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let assist = Assist {
            job: text(&record, &["job"]),
            request: &request,
            task: text(&record, &["consumer", "task"]),
            investigation: int(job, &["limits", "investigation", "amount"]),
            deadline: text(&record, &["limits", "deadline"]),
            now: tick.now.clone(),
            view: &view,
            claims: &claims,
            evidence: &evidence,
            asked: std::cell::RefCell::new(Vec::new()),
        };
        for item in list(job, &["items"]) {
            self.decide_item(item, &record, &trees, &assist, &mut decided, tick)?;
        }

        // Step 3 again, for what no item named: the task is a question, and
        // a packet that answers only what was already located is a pointer
        // list. Everything here is advisory and is dropped by capacity
        // before anything an item required.
        self.discover(&record, job, &trees, &mut decided, &assist, tick)?;

        // **The compile that read the answers is the one that frees
        // them.** Everything this job asked a model has now been used, and
        // compiling happens once per job. Until this point the answers
        // must stay: a key freed as soon as it was read would be asked
        // afresh by the next tick's compile, which is a second call and a
        // second charge for a question already answered.
        self.release_job(assist.job);

        let mut steps = compiler::steps(&decided, &self.config.provider_id);
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
    ///
    /// **Model-assisted discovery lives here and nowhere else.** The
    /// deterministic reading runs first and always: it is exactly what a
    /// request authorising no investigation gets, and it is what stands
    /// when a model step cannot be used. All a model may do is widen the
    /// set the sections are drawn from, and choose within it — the
    /// labels, the ranks and the citations are the deterministic path's
    /// either way.
    fn discover(
        &self,
        record: &Value,
        job: &Value,
        trees: &[Frontier],
        decided: &mut Decided,
        assist: &Assist<'_>,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        let question = Self::question(record, job);
        let found = self.ranked_spans(&question, trees)?;
        let claims = self.ranked_claims(record, job, decided)?;
        let widened = self.assist_discovery(&question, &found, &claims, trees, assist, tick)?;
        self.publish_spans(&widened.spans, &question, trees, decided, tick)?;
        self.discover_anchors(&Self::symbols(record, job), trees, decided)?;
        Self::publish_claims(claims, widened.claims.as_deref(), decided);
        if let Some(section) = widened.omitted {
            // **The packet says a step did not happen.** The omission
            // vocabulary is the protocol's four, so `unavailable` is the
            // one that fits and is literally what it was; *why* is the
            // typed reason in the step's own derivation record, which is
            // sealed whether the answer was usable or not.
            decided.omitted.push(object(vec![
                ("section_id", string(&section)),
                ("reason", string("unavailable")),
            ]));
        }
        Ok(())
    }

    /// Retrieval over the whole view, from a question rather than from a
    /// path.
    ///
    /// **Split out at m4e** because the same search runs twice: once for
    /// the request's own words, and once for the terms a model proposed
    /// — which take exactly this path, through CBR's own index, inside
    /// the view, with no branch of their own.
    fn ranked_spans(
        &self,
        question: &str,
        trees: &[Frontier],
    ) -> Result<Vec<cbr_memory::retrieval::Found>, TickError> {
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
            return Ok(Vec::new());
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
        Ok(answer.found)
    }

    /// Turn the spans discovery settled on into sections, with the
    /// per-path cap, the widening and the overlap rule that have always
    /// applied to them.
    ///
    /// `found` is the deterministic reading, or — when a model helped —
    /// the union it was offered, filtered to what it chose. **Everything
    /// below this line is the same either way**, which is what "the
    /// labels, ranks and citations stay the deterministic path's" means
    /// in code rather than in a sentence.
    fn publish_spans(
        &self,
        found: &[cbr_memory::retrieval::Found],
        question: &str,
        trees: &[Frontier],
        decided: &mut Decided,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        let mut per_path: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        // The spans this loop has already published. `decided.selections`
        // holds what the *items* took; a discovered span has to be checked
        // against both, because two discovered hits in one file are exactly
        // the pair that collide.
        let mut published: Vec<(String, i64, i64)> = decided
            .selections
            .iter()
            .map(|selection| {
                (
                    selection.blob.clone(),
                    selection.start_byte,
                    selection.end_byte,
                )
            })
            .collect();
        let mut taken = 0;
        for found in found.iter().cloned() {
            if taken >= compiler::DISCOVERED_SPANS {
                break;
            }
            if per_path
                .get(&found.path)
                .is_some_and(|seen| *seen >= compiler::DISCOVERED_PER_PATH)
            {
                continue;
            }
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
            // Discovered spans rank by definition, so every one of them
            // reaches its neighbours.
            let terms = cbr_memory::lexical::query_terms(question);
            let (start_byte, end_byte, start_line, end_line) = self.widen(
                frontier,
                &found.path,
                (
                    found.start_byte,
                    found.end_byte,
                    found.start_line,
                    found.end_line,
                ),
                &bytes,
                &terms,
            );
            // **Overlap is decided after widening, not before.** Two
            // adjacent hits in one file widen towards each other and can
            // land on the same bytes; the first form of this checked the
            // hit's own start byte and published the same span twice, which
            // brian2's rerun showed as one section repeated. A span already
            // covered by one the packet holds adds nothing, so it is
            // dropped and does not spend the budget.
            let covered = published.iter().any(|(blob, from, to)| {
                *blob == found.blob && *from < end_byte && start_byte < *to
            });
            if covered {
                continue;
            }
            published.push((found.blob.clone(), start_byte, end_byte));
            *per_path.entry(found.path.clone()).or_default() += 1;
            let selection = Selection {
                item: String::new(),
                via: None,
                repository: found.repository.clone(),
                tree: frontier.tree.clone(),
                path: found.path.clone(),
                blob: found.blob.clone(),
                digest: cbr_encoding::digest_bytes(&bytes),
                size: bytes.len(),
                start_byte,
                end_byte,
                start_line,
                end_line,
                origin: "index",
                excerpt: compiler::excerpt(&bytes, start_byte, end_byte, &terms),
            };
            self.seal_source(tick, &selection, &bytes)?;
            decided.discovered.push(compiler::Discovered {
                // The repository is in the id, because the same path at the
                // same byte in two repositories of one basis is two spans;
                // the key is the id as it was, so ties sort as they did.
                id: crate::ids::span(&selection.repository, &selection.path, selection.start_byte),
                key: format!("span-{}-{}", selection.path, selection.start_byte),
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
                let callers =
                    cbr_memory::anchors::references(self.store.connection(), &frontier.tree, name)
                        .map_err(|_| ProtocolError::new_internal_error())?;
                // A name with uses and no definition in this tree is the
                // third-party API case, and its use sites are exactly what
                // "where does this repository use X" asks for. Requiring a
                // definition first meant a question about someone else's
                // function got nothing at all.
                if resolution.candidates.is_empty() && callers.is_empty() {
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
                let definitions = if where_defined.is_empty() {
                    format!(
                        "{name} is not defined in this tree; it is used here, which is what a \
                         question about someone else's function is asking for"
                    )
                } else {
                    format!("{name} defined at:\n  {}", where_defined.join("\n  "))
                };
                decided.discovered.push(compiler::Discovered {
                    // A name is whatever the request's words were, in any
                    // script, so it is encoded; `:` ends the repository.
                    id: crate::ids::anchor(&frontier.repository, name),
                    key: format!("anchor-{}-{name}", frontier.repository),
                    rank: compiler::Rank::Anchor,
                    order: 0,
                    claim: None,
                    label: "inferred",
                    historical: false,
                    content: format!("{definitions}\n{uses}{ambiguity}"),
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
    /// **That is eligibility, not selection.** In a store holding one
    /// repository the first limb is true of every claim, so it selects
    /// nothing: M3d's Knowscroll pilot carried twenty-two of twenty-two
    /// decisions into a packet about one of them. Among eligible claims the
    /// compiler therefore **ranks** — by how many of the question's terms
    /// occur in the claim's statement, its scope, and the text of the
    /// evidence it cites — carries the top [`compiler::CARRIED_CLAIMS`],
    /// and omits the rest.
    ///
    /// **A claim that does not rank is omitted, not demoted.** A `binding`
    /// claim that ranks keeps its `BindingClaim` drop rank; one that does
    /// not is not in the packet at all, and is counted.
    ///
    /// **The cited text is read at the claim's own span when it names one.**
    /// A repository whose decisions live in one append-only file gives every
    /// claim the same artifact and the same digest — Protocol 0.1 cannot
    /// cite a span ([protocol#16](https://github.com/Combraton/protocol/issues/16))
    /// — so the range lives in the claim's scope qualifiers, which is where
    /// this reads it from. Without that, every claim over one file scores
    /// identically and the ranking is no ranking.
    ///
    /// Anything ineligible is **omitted with reason `applicability`**, one
    /// omission each, so a caller counts what it did not get.
    ///
    /// A claim is labelled by what the authority permitted it for, so
    /// `binding` means an authority said so. A rejected claim, or one not
    /// applicable at this basis, is carried as historical: CONTEXT section
    /// 14 makes a historical section never current for an item and reports
    /// it at the read, which is what INTERNALS section 5 step 3 means by
    /// keeping rejected alternatives distinguishable rather than absent.
    /// **Split at m4e into eligibility-and-ranking, here, and
    /// carrying, in [`Self::publish_claims`]** — because claims enter the
    /// candidate set a model chooses from, and a cap cannot be applied
    /// while the set is still being offered.
    fn ranked_claims(
        &self,
        record: &Value,
        job: &Value,
        decided: &mut Decided,
    ) -> Result<Vec<(usize, String, compiler::Discovered)>, TickError> {
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
        // Every eligible claim, with the score it will be ranked by. The
        // packet is built from this afterwards, because a cap cannot be
        // applied while the list is still being discovered.
        let mut ranked: Vec<(usize, String, compiler::Discovered)> = Vec::new();
        for claim in readable {
            let payload = object(vec![("claim", string(&claim))]);
            let Ok(inspected) = self.knowledge_inspect(&payload) else {
                continue;
            };
            let claim_record = at(&inspected, &["record"]);

            let about_this_code = list(claim_record, &["conditions"]).iter().any(|condition| {
                repositories.contains(&text(condition, &["repository"]).to_string())
            });
            // **Eligibility is the m3c rule, unchanged**, and is decided
            // from the claim itself: a claim whose cited artifact happens
            // to mention the question is not thereby about it, and reading
            // evidence to decide eligibility would make every claim over a
            // large shared file eligible for every question.
            let own = cbr_memory::lexical::query_terms(&format!(
                "{} {}",
                text(claim_record, &["scope", "id"]),
                crate::context::canonical(at(claim_record, &["statement"]))
            ));
            let shared = question.iter().filter(|term| own.contains(term)).count();
            if !about_this_code && shared == 0 {
                decided.omitted.push(object(vec![
                    ("section_id", string(&crate::ids::claim_section(&claim))),
                    ("reason", string("applicability")),
                ]));
                continue;
            }

            // **Ranking is wider than eligibility**, because among claims
            // that are all eligible the question has to be answered from
            // somewhere: the scope's qualifiers, and the text of the
            // evidence the claim cites at the span the claim names.
            let mut corpus = String::new();
            for (name, value) in Self::qualifiers(claim_record) {
                corpus.push_str(&name);
                corpus.push(' ');
                corpus.push_str(&value);
                corpus.push(' ');
            }
            corpus.push_str(&self.cited_text(claim_record));
            let wider = cbr_memory::lexical::query_terms(&corpus);
            let score = question
                .iter()
                .filter(|term| own.contains(term) || wider.contains(term))
                .count();
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

            ranked.push((
                score,
                claim.clone(),
                compiler::Discovered {
                    id: crate::ids::claim(&claim),
                    key: crate::ids::claim(&claim),
                    rank,
                    order: usize::MAX - score,
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
                },
            ));
        }

        // Rank, but do not cut. The tie-break is the claim id, so two
        // claims that share the question equally are ordered by something
        // stable rather than by the order the store happened to return
        // them in.
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        Ok(ranked)
    }

    /// Carry the claims the packet keeps, and say why the rest were left
    /// out.
    ///
    /// `chosen` is `None` for the deterministic cut — the first
    /// [`compiler::CARRIED_CLAIMS`] of the ranking, which is the m3c rule
    /// unchanged — and `Some` when a model chose out of the candidate
    /// set. **The cap applies either way**: a model that chose six claims
    /// does not thereby widen a packet that carries four, and a claim it
    /// did not choose is omitted for `applicability` exactly as one
    /// ranked below the cut is.
    fn publish_claims(
        ranked: Vec<(usize, String, compiler::Discovered)>,
        chosen: Option<&[String]>,
        decided: &mut Decided,
    ) {
        let mut carried = 0;
        for (_, claim, discovered) in ranked {
            // **A binding claim is carried whether a model listed it or
            // not.** `binding` is an authority's act, and the rule that
            // a model may never mark a claim binding is worth nothing if
            // a model may quietly unmark one by leaving it out of a
            // list. INTERNALS section 5 step 3 puts it plainly about
            // this rank: losing it loses the answer. The cap still
            // applies, and so does every other claim's eligibility.
            let binding = discovered.rank == compiler::Rank::BindingClaim;
            let wanted = binding
                || match chosen {
                    Some(chosen) => chosen.contains(&claim),
                    None => true,
                };
            if wanted && carried < compiler::CARRIED_CLAIMS {
                carried += 1;
                decided.discovered.push(discovered);
            } else {
                decided.omitted.push(object(vec![
                    ("section_id", string(&crate::ids::claim_section(&claim))),
                    ("reason", string("applicability")),
                ]));
            }
        }
    }

    /// **The two model steps of discovery**, or the deterministic reading
    /// when there is no model, no budget for both, or nothing to choose.
    ///
    /// Written as one function because the two steps are one flow: the
    /// terms the first proposes are what the second chooses among, and
    /// neither is worth making on its own.
    fn assist_discovery(
        &self,
        question: &str,
        found: &[cbr_memory::retrieval::Found],
        claims: &[(usize, String, compiler::Discovered)],
        trees: &[Frontier],
        assist: &Assist<'_>,
        tick: &mut Tick,
    ) -> Result<Widened, TickError> {
        let deterministic = || Widened {
            spans: found.to_vec(),
            claims: None,
            omitted: None,
        };
        let Some(serving) = self.model.clone() else {
            return Ok(deterministic());
        };
        if assist.investigation <= 0 {
            return Ok(deterministic());
        }
        // **Nothing readable is not the same as nothing found.** An
        // empty *result* is exactly the case the terms step exists for —
        // brian2's, where the ordinary reading finds nothing useful. An
        // empty *view* is a request with no repository to search, where
        // every term would be run against nothing, so the call could not
        // change the answer and is not made.
        if trees.is_empty() {
            return Ok(deterministic());
        }
        // **A flow that cannot finish is not started.** The terms step's
        // answer is only worth anything if there is budget left to choose
        // among what it widens to, so the two units are claimed together
        // or neither is spent.
        if !assist.room_for(2) {
            return Ok(deterministic());
        }
        let digest = cbr_encoding::digest_bytes(question.as_bytes());

        // ---- step one: the terms ------------------------------------
        let (seen, _) =
            self.span_candidates(&found[..found.len().min(crate::discovery::SEEN)], trees);
        let terms = match self.ask_step(
            &serving,
            assist,
            &format!(
                "model:{}:{}:discovery.terms:{digest}",
                assist.job, assist.request
            ),
            &format!("discovery.terms {question}"),
            DISCOVERY_ITEM,
            seen,
            Step::Terms,
            tick,
        )? {
            Asked::NotReady => return Err(TickError::NotReady),
            Asked::Answered(crate::derivation::Answer::Proposed(terms)) => terms,
            // Anything else — a bound broken, a call that failed, a
            // rebuild with nothing retained, a record of another shape —
            // leaves the deterministic reading standing and the packet
            // saying the step did not happen.
            Asked::Answered(_) => {
                return Ok(Widened {
                    spans: found.to_vec(),
                    claims: None,
                    omitted: Some(TERMS_SECTION.to_string()),
                });
            }
        };

        // ---- the union: CBR's own search, for the terms it was given --
        //
        // **This is the whole of what a term does.** It is tokenised and
        // run through the same index, over the same view, as the
        // request's own words; what comes back are spans CBR found, at
        // paths CBR resolved, inside repositories the grant allowed. A
        // term cannot name any of those.
        //
        // **The ordinary reading takes a reserved share and no more.**
        // On a real repository it returns more than the set can hold, so
        // without this it fills every slot and a term contributes
        // nothing — which is backwards for the question this step is
        // for, whose answer the ordinary reading missed while returning
        // plenty of confident near-misses.
        //
        // **Both halves are built under the packet's own per-path cap**,
        // and the live run of 2026-09-22 is why. Without it the question
        // half was the raw top of the ranking, so one file that
        // out-ranks the rest filled the set: J1 offered thirteen spans
        // of one file and none of the file the deterministic packet
        // cites. An offer the packet could not honour, and a fact never
        // offered that could not be kept.
        let mut taken: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut union =
            crate::discovery::per_path_capped(found, crate::discovery::FROM_QUESTION, &mut taken);
        let query = crate::discovery::query(&terms);
        let shown_claims = claims.len().min(crate::discovery::CLAIMS_SHOWN);
        let span_room = crate::discovery::CANDIDATES.saturating_sub(shown_claims);
        if !query.is_empty() {
            // The same cap, carried across the halves by the same map: a
            // file the question already reached twice is not reached a
            // third time because a term names it too.
            let extra: Vec<cbr_memory::retrieval::Found> = self
                .ranked_spans(&query, trees)?
                .into_iter()
                .filter(|extra| {
                    !union
                        .iter()
                        .any(|have| have.blob == extra.blob && have.start_byte == extra.start_byte)
                })
                .collect();
            let room = span_room.saturating_sub(union.len());
            union.extend(crate::discovery::per_path_capped(&extra, room, &mut taken));
        }
        union.truncate(span_room);

        // ---- step two: the choice ------------------------------------
        // **The union is what was offered, not what was searched.** A
        // span whose bytes could not be read is offered to nobody, so it
        // leaves both lists together and an id keeps meaning the span
        // the packet would publish.
        let (mut candidates, union) = self.span_candidates(&union, trees);
        let claim_ids: Vec<String> = claims
            .iter()
            .take(shown_claims)
            .map(|(_, claim, _)| claim.clone())
            .collect();
        for (position, (_, _, discovered)) in claims.iter().take(shown_claims).enumerate() {
            candidates.push(crate::selection::Candidate {
                id: format!("k{}", position + 1),
                kind: crate::selection::KIND_CLAIM,
                path: claim_ids[position].clone(),
                start_line: 0,
                end_line: 0,
                // **Bounded in bytes, not characters.** The
                // per-request arithmetic is computed from a byte
                // bound, and `chars().take(n)` is up to four times
                // that on non-Latin text -- which a claim's statement
                // may well be.
                text: crate::discovery::clipped(&discovered.content, compiler::EXCERPT_BYTES),
            });
        }
        if !crate::discovery::worth_choosing(&candidates) {
            // Everything offered is going in, so the call would decide
            // nothing. What the terms widened to still stands.
            return Ok(Widened {
                spans: union,
                claims: None,
                omitted: None,
            });
        }
        let chosen = match self.ask_step(
            &serving,
            assist,
            // **The key is the question**, which is m4c's own rule, and
            // this question includes the terms. Nothing today can ask
            // this key twice under different terms -- the terms answer
            // is the pool's and is stable across the ticks of one
            // compile -- and a key that disagreed with its question
            // would be a coupling waiting to be broken.
            &format!(
                "model:{}:{}:discovery.choose:{digest}:{}",
                assist.job,
                assist.request,
                cbr_encoding::digest_bytes(terms.join(" ").as_bytes())
            ),
            // **The terms are part of the question**, so a rebuild that
            // retained different terms asks a different question here and
            // does not answer it from this record.
            &format!("discovery.choose {question} | {}", terms.join(" ")),
            DISCOVERY_ITEM,
            candidates,
            Step::Choose(terms),
            tick,
        )? {
            Asked::NotReady => return Err(TickError::NotReady),
            Asked::Answered(crate::derivation::Answer::ChoseMany(ids)) => ids,
            // **A failed choice leaves the deterministic reading, not
            // the union.** The two steps are one flow: the terms step
            // proposes where to look and the choice is what decides that
            // any of it belongs in a packet. Carrying term-driven spans
            // by rank alone would be carrying them on the strength of a
            // suggestion nothing acted on — and BM25 scores from two
            // different queries are not one ranking. Skipping the choice
            // because it *could not change the answer* is a different
            // case, and the union stands there because that is what the
            // choice would have done.
            Asked::Answered(_) => {
                return Ok(Widened {
                    spans: found.to_vec(),
                    claims: None,
                    omitted: Some(CHOICE_SECTION.to_string()),
                });
            }
        };

        // **The ids are resolved against the offered list and nothing
        // else**, a second time and on the way out. A span is kept in the
        // order it was offered, which is the deterministic path's own
        // ranking; a claim id the model never saw is not a claim.
        let spans = union
            .into_iter()
            .enumerate()
            .filter(|(position, _)| chosen.iter().any(|id| *id == format!("d{}", position + 1)))
            .map(|(_, found)| found)
            .collect();
        let chosen_claims = claim_ids
            .iter()
            .enumerate()
            .filter(|(position, _)| chosen.iter().any(|id| *id == format!("k{}", position + 1)))
            .map(|(_, claim)| claim.clone())
            .collect();
        Ok(Widened {
            spans,
            claims: Some(chosen_claims),
            omitted: None,
        })
    }

    /// Every span that can actually be shown, as a candidate — **and the
    /// spans themselves, in the same order**.
    ///
    /// The two travel together because an id is a position: `d3` means
    /// the third candidate offered, and the third candidate offered is
    /// what the packet publishes if the model chooses it. Returning the
    /// candidates alone and re-deriving the spans from the input would
    /// put the two out of step the moment one was dropped — an id
    /// resolving to a span nobody was shown, which is the closed set
    /// broken from the inside.
    ///
    /// A span whose blob cannot be read is **dropped rather than offered
    /// empty**: an id whose text nobody could show is an id chosen blind,
    /// and the deterministic path would not have published it either.
    fn span_candidates(
        &self,
        found: &[cbr_memory::retrieval::Found],
        trees: &[Frontier],
    ) -> (
        Vec<crate::selection::Candidate>,
        Vec<cbr_memory::retrieval::Found>,
    ) {
        let mut candidates = Vec::new();
        let mut kept = Vec::new();
        for found in found.iter() {
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
            let text = usize::try_from(found.start_byte)
                .ok()
                .zip(usize::try_from(found.end_byte).ok())
                .and_then(|(start, end)| bytes.get(start..end))
                .map(|span| String::from_utf8_lossy(span).to_string())
                .unwrap_or_default();
            candidates.push(crate::selection::Candidate {
                id: format!("d{}", candidates.len() + 1),
                kind: crate::selection::KIND_SPAN,
                path: found.path.clone(),
                start_line: found.start_line as usize,
                end_line: found.end_line as usize,
                text,
            });
            kept.push(found.clone());
        }
        (candidates, kept)
    }

    /// Put one discovery question to a model, or answer it from a
    /// retained record on a rebuild.
    ///
    /// **The same path [`Self::assist`] takes**, for a question that is
    /// not an item's: the budget is spent on the question, a rebuild
    /// reads a record instead of calling, the answer is sealed when it is
    /// taken, and every failure is a typed reason. What differs is only
    /// what the body says and how the reply is read, which is [`Step`].
    #[allow(clippy::too_many_arguments)]
    fn ask_step(
        &self,
        serving: &std::sync::Arc<crate::provider::Serving>,
        assist: &Assist<'_>,
        key: &str,
        selector: &str,
        item: &str,
        candidates: Vec<crate::selection::Candidate>,
        step: Step,
        tick: &mut Tick,
    ) -> Result<Asked, TickError> {
        // Unreachable while `room_for(2)` guards the flow, and checked
        // anyway: this is where a unit is spent, and a step that spends
        // none has not been asked.
        if !assist.admits(key) {
            return Ok(Asked::Answered(crate::derivation::Answer::Unmet(
                INVESTIGATION_EXHAUSTED,
            )));
        }
        let offered = crate::derivation::offered(&candidates);
        // **The evidence this question showed a model**, which is what its
        // record is sealed under: a part of a projection shows one
        // artifact's bytes, and a discovery step shows none.
        let evidence: Vec<String> = match &step {
            Step::Project(part) => vec![part.artifact.clone()],
            Step::Terms | Step::Choose(_) => Vec::new(),
        };
        let deadline = crate::clock::unix_of(assist.deadline)
            .zip(crate::clock::unix_of(&assist.now))
            .map(|(deadline, now)| {
                std::time::Instant::now()
                    + std::time::Duration::from_secs(deadline.saturating_sub(now))
            });

        if matches!(serving.wire, crate::provider::Wire::Replay) {
            let wanted = crate::derivation::Question {
                model: &serving.model,
                dialect: serving.dialect.name(),
                task: assist.task,
                selector,
                offered: &offered,
            }
            .digest();
            return Ok(Asked::Answered(match self.retained(&wanted, assist)? {
                Some(answer) => answer,
                None => crate::derivation::Answer::Unmet(crate::derivation::NOT_RETAINED),
            }));
        }

        let asking = || {
            let serving = std::sync::Arc::clone(serving);
            let (job, request) = (assist.job.to_string(), assist.request.to_string());
            let (task, selector, now) = (
                assist.task.to_string(),
                selector.to_string(),
                assist.now.clone(),
            );
            let item = item.to_string();
            let beside = self.store.open_beside();
            move || {
                let beside = beside.map_err(|_| "store_unavailable")?;
                let body = match &step {
                    Step::Terms => crate::discovery::propose(&serving.model, &task, &candidates),
                    Step::Choose(terms) => {
                        crate::discovery::choose(&serving.model, &task, terms, &candidates)
                    }
                    Step::Project(part) => {
                        crate::projection::ask(&serving.model, &task, part, &candidates)
                    }
                };
                let started = std::time::Instant::now();
                let outcome = serving.ask(&beside, &now, &job, &request, &body);
                let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let (answer, cost) = crate::derivation::taken(outcome, |reply| match &step {
                    Step::Terms => match crate::discovery::proposed(reply) {
                        Ok(terms) => crate::derivation::Answer::Proposed(terms),
                        Err(reason) => crate::derivation::Answer::Unmet(reason),
                    },
                    Step::Choose(_) => match crate::discovery::chosen(reply, &candidates) {
                        Ok(picked) => crate::derivation::Answer::ChoseMany(
                            picked
                                .into_iter()
                                .map(|at| candidates[at].id.clone())
                                .collect(),
                        ),
                        Err(reason) => crate::derivation::Answer::Unmet(reason),
                    },
                    Step::Project(_) => match crate::projection::chosen(reply, &candidates) {
                        Ok(picked) => crate::derivation::Answer::ChoseMany(
                            picked
                                .into_iter()
                                .map(|at| candidates[at].id.clone())
                                .collect(),
                        ),
                        Err(reason) => crate::derivation::Answer::Unmet(reason),
                    },
                });
                Ok(crate::derivation::record(&crate::derivation::Made {
                    question: &crate::derivation::Question {
                        model: &serving.model,
                        dialect: serving.dialect.name(),
                        task: &task,
                        selector: &selector,
                        offered: &offered,
                    },
                    answer,
                    spend: crate::derivation::Spend { cost, latency_ms },
                    job: &job,
                    request: &request,
                    item: &item,
                    made_at: &now,
                }))
            }
        };
        Ok(match self.work.progress(key, deadline, asking) {
            crate::work::Progress::Running | crate::work::Progress::Deferred => Asked::NotReady,
            crate::work::Progress::Done(value) => {
                self.seal_derivation(tick, &value, assist, &evidence)?;
                Asked::Answered(crate::derivation::answer_of(&value).unwrap_or(
                    crate::derivation::Answer::Unmet(crate::derivation::UNREADABLE),
                ))
            }
            crate::work::Progress::Failed(reason) => {
                Asked::Answered(crate::derivation::Answer::Unmet(reason))
            }
            crate::work::Progress::TimedOut => {
                Asked::Answered(crate::derivation::Answer::Unmet("model_call_timed_out"))
            }
        })
    }

    /// A claim's scope qualifiers, as name and value pairs.
    fn qualifiers(claim_record: &Value) -> Vec<(String, String)> {
        let qualifiers = at(claim_record, &["scope", "qualifiers"]).clone();
        qualifiers
            .keys()
            .map(str::to_string)
            .collect::<Vec<String>>()
            .into_iter()
            .map(|name| {
                let value = text(&qualifiers, &[name.as_str()]).to_string();
                (name, value)
            })
            .collect()
    }

    /// The text of the evidence a claim cites, bounded, and narrowed to the
    /// claim's own span when its scope names one.
    ///
    /// Protocol 0.1's evidence reference is a provider, an artifact and a
    /// digest and nothing narrower, so a repository that keeps every
    /// decision in one file gives every claim identical support
    /// ([protocol#16](https://github.com/Combraton/protocol/issues/16)).
    /// Reading the whole artifact for each of them would score them all the
    /// same, which is the same as not ranking at all. A `lines` qualifier is
    /// where such a claim has to put its range today, so it is read from
    /// there — a declared qualifier, not a string parsed out of prose.
    fn cited_text(&self, claim_record: &Value) -> String {
        let Some(support) = list(claim_record, &["support"]).first().cloned() else {
            return String::new();
        };
        let digest = text(&support, &["evidence", "digest"]).to_string();
        let Ok(Some(bytes)) = self.store.read_object(&digest) else {
            return String::new();
        };
        let bytes = &bytes[..bytes.len().min(compiler::CLAIM_TEXT_BYTES)];
        let whole = String::from_utf8_lossy(bytes);
        let lines = Self::qualifiers(claim_record)
            .into_iter()
            .find(|(name, _)| name == "lines")
            .map(|(_, value)| value);
        let Some(range) = lines else {
            return whole.into_owned();
        };
        let Some((from, to)) = range.split_once('-') else {
            return whole.into_owned();
        };
        let (Ok(from), Ok(to)) = (from.trim().parse::<usize>(), to.trim().parse::<usize>()) else {
            return whole.into_owned();
        };
        whole
            .lines()
            .skip(from.saturating_sub(1))
            .take(to.saturating_sub(from.saturating_sub(1)))
            .collect::<Vec<&str>>()
            .join("\n")
    }

    /// Build the index for `repository` at `tree` unless a manifest already
    /// says it is there, and report what that projection reaches.
    fn ensure_index(
        &self,
        repository: &str,
        checkout: &std::path::Path,
        tree: &str,
        dirty: Option<&str>,
    ) -> Result<Reach, TickError> {
        use cbr_memory::retrieval;
        let connection = self.store.connection();
        let position = self
            .store
            .current_epoch()
            .and_then(|epoch| self.store.last_sequence(epoch))
            .unwrap_or(0);
        let existing = retrieval::manifest(connection, repository).ok().flatten();
        let manifest: Result<retrieval::Manifest, cbr_memory::index::IndexError> = match existing {
            // Already built for this tree: nothing leaves the tick, and
            // the overwhelmingly common case stays as fast as it was.
            Some(manifest) if manifest.frontier == tree => Ok(manifest),
            // **Not built. This is the long one, and it leaves.**
            //
            // M3 measured it holding the tick throughout — 7.2s on CBR's
            // own blobs, 12.8s on brian2's, 3.9s on Knowscroll's — with
            // every other job on the provider waiting it out. Now the tick
            // starts it and returns, and this job waits while the others
            // carry on.
            _ => {
                let key = format!("index:{repository}:{tree}");
                // **No deadline here**: the request has one of its own,
                // in protocol instants, and `advance` already publishes
                // what a job has when it passes. A second deadline in
                // monotonic time beside it would be two clocks
                // disagreeing about the same request. What this does owe
                // is the slot: a job that ends cancels its build.
                let deadline = None;
                let building = || {
                    let (repository, tree) = (repository.to_string(), tree.to_string());
                    let checkout = checkout.to_path_buf();
                    // Opened here, inside the factory, so a tick that is
                    // only asking opens nothing. A store that cannot be
                    // opened is this unit's typed reason rather than a
                    // failure of the whole tick.
                    let beside = self.store.open_beside();
                    move || {
                        let beside = beside.map_err(|_| "store_unavailable")?;
                        retrieval::build(&beside, &repository, &checkout, &tree, position)
                            // The build's product is the manifest it
                            // wrote; this job reads it back through its
                            // own connection, so there is nothing to
                            // carry out of the thread.
                            .map(|_| Value::Null)
                            .map_err(|_| "index_unavailable")
                    }
                };
                match self.work.progress(&key, deadline, building) {
                    // Started, or already running, or waiting for room.
                    // Either way this job has nothing to do this tick.
                    crate::work::Progress::Running | crate::work::Progress::Deferred => {
                        return Err(TickError::NotReady);
                    }
                    // Built. Read back what it wrote, through this
                    // connection, and carry on.
                    crate::work::Progress::Done(_) => {
                        // Taken, so the key is free. A settled answer is
                        // kept until its caller has used it, and this one
                        // has: what follows reads the manifest itself.
                        self.work.release(&key);
                        match retrieval::manifest(connection, repository).ok().flatten() {
                            Some(manifest) => Ok(manifest),
                            // It said it built and there is no manifest.
                            // Reported rather than retried: a build that
                            // succeeds and leaves nothing is a fault to
                            // see, not one to paper over with another run.
                            None => {
                                return Ok(Reach {
                                    repository: repository.to_string(),
                                    frontier: tree.to_string(),
                                    state: "unavailable".into(),
                                    gaps: vec![
                                        "the projection could not be built \
                                         (index_missing_after_build)"
                                            .into(),
                                    ],
                                });
                            }
                        }
                    }
                    // **Every failure is a typed reason an item carries.**
                    crate::work::Progress::Failed(reason) => {
                        return Ok(Reach {
                            repository: repository.to_string(),
                            frontier: tree.to_string(),
                            state: "unavailable".into(),
                            // Descriptive **and** typed: a reader of the
                            // coverage learns what happened, and a
                            // consumer gets the reason it can act on.
                            gaps: vec![format!("the projection could not be built ({reason})")],
                        });
                    }
                    crate::work::Progress::TimedOut => {
                        return Ok(Reach {
                            repository: repository.to_string(),
                            frontier: tree.to_string(),
                            state: "unavailable".into(),
                            gaps: vec![
                                "the projection could not be built (index_build_timed_out)".into(),
                            ],
                        });
                    }
                }
            }
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
                // By kind, not just by count: "991 blobs in a language with
                // no anchors" does not tell a reader whether the gap is
                // documentation or the Cython half of a scientific library.
                let mut kinds: Vec<(&String, &usize)> =
                    coverage.unanchored_by_kind.iter().collect();
                kinds.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
                for (kind, count) in kinds.into_iter().take(8) {
                    gaps.push(format!("  of those, {count} are {kind}"));
                }
                gaps.extend(self.dirty_gaps(checkout, dirty));
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

    /// What a dirty working tree keeps out of the index, as a gap.
    ///
    /// The owner's decision for M3d is that a pilot searches the tree **as
    /// committed**: a file that is not in any tree has no blob to cite and
    /// no tree to anchor a citation to, so citing one would produce a
    /// reference reproducible from a snapshot and from nothing else. So the
    /// working tree is not read — and the count of what that leaves out is
    /// declared rather than left for a reader to discover.
    ///
    /// The snapshot is recomputed here rather than taken from the basis,
    /// which carries only its digest. That is also a check: a digest that no
    /// longer matches means the working tree moved after the basis was
    /// taken, which is itself a gap.
    /// What a dirty working tree keeps out of the answer.
    ///
    /// The index is built at the **committed** tree, which is the right
    /// choice — a working tree is not a basis anyone else can resolve — but
    /// a packet that does not say so leaves a reader unable to tell
    /// "searched and not found" from "searched a version of this file that
    /// is no longer on disk". INTERNALS section 5 calls that a false
    /// absence, and it is the same rule that made the untracked gap exist.
    ///
    /// The snapshot distinguishes three states and each gets its own line,
    /// because they are different facts about the same tree: a file the
    /// index never saw, a file whose bytes on disk differ from the ones it
    /// holds, and a file that is in the tree and gone from disk. M3d's
    /// Knowscroll pilot had twenty-one of the second and reported none of
    /// them; brian2 had none, which is why nothing showed.
    fn dirty_gaps(&self, checkout: &std::path::Path, dirty: Option<&str>) -> Vec<String> {
        let Some(declared) = dirty else {
            return Vec::new();
        };
        let Ok(Some(snapshot)) = cbr_identity::dirty_snapshot(checkout) else {
            return vec![format!(
                "the basis declares a dirty snapshot {declared} that could not be recomputed"
            )];
        };
        let mut gaps = Vec::new();
        if snapshot.digest != declared {
            gaps.push(format!(
                "the working tree changed after this basis was taken: the basis declares \
                 {declared} and the tree now digests to {}",
                snapshot.digest
            ));
        }
        let entries = snapshot
            .document
            .get("entries")
            .and_then(Value::as_array)
            .map(<[Value]>::to_vec)
            .unwrap_or_default();
        // One pass, three tallies, in the order a reader cares about:
        // what was never seen, what has moved on, and what is gone.
        let mut counted: std::collections::BTreeMap<&str, (usize, BTreeMap<String, usize>)> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            let fields = entry.as_array().unwrap_or_default();
            let path = fields.first().and_then(Value::as_str).unwrap_or_default();
            let state = fields.get(1).and_then(Value::as_str).unwrap_or_default();
            let state = match state {
                "untracked" => "untracked",
                "modified" => "modified",
                "deleted" => "deleted",
                _ => continue,
            };
            let tally = counted.entry(state).or_default();
            tally.0 += 1;
            *tally
                .1
                .entry(cbr_memory::lexical::file_kind(path))
                .or_default() += 1;
        }
        for state in ["untracked", "modified", "deleted"] {
            let Some((count, kinds)) = counted.get(state) else {
                continue;
            };
            let (noun, verb) = if *count == 1 {
                ("file", "is")
            } else {
                ("files", "are")
            };
            gaps.push(match state {
                "untracked" => format!(
                    "{count} {noun} {verb} untracked and in no tree, so they are not searched"
                ),
                "modified" => format!(
                    "{count} tracked {noun} {verb} modified in the working tree, so the index \
                     holds the committed bytes and not these"
                ),
                _ => format!(
                    "{count} tracked {noun} {verb} deleted in the working tree, so the index \
                     still holds bytes that are no longer on disk"
                ),
            });
            let mut listed: Vec<(&String, &usize)> = kinds.iter().collect();
            listed.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
            for (kind, count) in listed.into_iter().take(6) {
                gaps.push(format!("  of those, {count} are {kind}"));
            }
        }
        gaps
    }

    /// One item: what satisfies it, or why nothing does.
    fn decide_item(
        &self,
        item: &Value,
        record: &Value,
        trees: &[Frontier],
        assist: &Assist<'_>,
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
                // A symbolic link never supplies content: see `follow_link`.
                let (read, via) = match self.follow_link(frontier, &path) {
                    Link::Direct | Link::Absent => (path.clone(), None),
                    Link::Followed { target } => (target, Some(path.clone())),
                    Link::Broken { link, target } => {
                        decided.unmet.push(Unmet {
                            item: item_id,
                            reason: "source_is_a_link".into(),
                        });
                        // The reason is capped at 64 characters by CONTEXT,
                        // so where it pointed goes where there is room for
                        // it, beside the other things this tree could not
                        // reach.
                        let where_to = if target.is_empty() {
                            "something that is not a regular file of this tree".to_string()
                        } else {
                            format!("{target}, which is not a regular file of this tree")
                        };
                        if let Some(reach) = decided
                            .reach
                            .iter_mut()
                            .find(|reach| reach.repository == frontier.repository)
                        {
                            reach
                                .gaps
                                .push(format!("{link} is a symbolic link to {where_to}"));
                        }
                        return Ok(());
                    }
                };
                match self.select_source(frontier, item, &read, assist, tick)? {
                    Choice::Selected(selection) => {
                        let mut selection = *selection;
                        selection.via = via;
                        decided.selections.push(selection);
                    }
                    Choice::Absent => decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "source_absent_at_basis".into(),
                    }),
                    // **Failure is an item's unmet reason, never a hang
                    // and never a silent downgrade** (READINESS §8).
                    Choice::Unmet(reason) => decided.unmet.push(Unmet {
                        item: item_id,
                        reason: reason.into(),
                    }),
                }
            }
            "evidence_included" => {
                let mut reference = at(check, &["evidence"]).clone();
                // **A reference names the provider holding the artifact, or
                // none, which is this one** (EVIDENCE section 2). Compiling
                // reads this store and no other, so another provider's
                // artifact is not here to read — even when one of the same
                // id and digest is, since equal bytes never establish equal
                // provenance or permission — and `context.expand` refuses
                // such a citation for the same reason. The citation a packet
                // carries always names its provider, so it names this one.
                let named = text(&reference, &["provider"]);
                let here = named.is_empty() || named == self.config.provider_id;
                set(&mut reference, "provider", string(&self.config.provider_id));
                let artifact = text(&reference, &["artifact", "id"]).to_string();
                let sealed = self
                    .store
                    .subject(&crate::evidence::artifact_key(&artifact))
                    .map_err(|_| ProtocolError::new_internal_error())?;
                // **Readable first**, and the answer for an artifact the job
                // may not read is the answer for one that is not there: a
                // projection copies an artifact's bytes into a packet, so
                // naming one is otherwise a way to read it, and telling the
                // two apart is a way to learn that it exists.
                let readable = here && assist.evidence.contains(&artifact);
                let descriptor = match &sealed {
                    Some(state) if readable => {
                        let value = parse_record(&state.value)?;
                        (text(&value, &["state"]) == "sealed"
                            && context::is_null(at(&value, &["purge"]))
                            && text(&value, &["descriptor", "digest"])
                                == text(&reference, &["digest"]))
                        .then(|| at(&value, &["descriptor"]).clone())
                    }
                    _ => None,
                };
                match descriptor {
                    Some(descriptor) => {
                        self.project(
                            &item_id,
                            &reference,
                            &artifact,
                            &descriptor,
                            assist,
                            decided,
                            tick,
                        )?;
                    }
                    None => decided.unmet.push(Unmet {
                        item: item_id,
                        reason: "evidence_unavailable".into(),
                    }),
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

    /// **Journey 2: the evidence an item named, projected** (m5 READINESS
    /// §3). The artifact was sealed whole before anybody asked, and the job
    /// may read it; what the item gets is one section that tiles it —
    /// carried excerpts that are its bytes at their ranges, and declared
    /// omissions — or a typed reason and no section at all.
    ///
    /// The order is [`crate::projection::next`]'s. What is this call site's
    /// own is the investigation limit: **the questions a model is offered
    /// are claimed together**, as discovery's two steps are, because a
    /// projection with a part never read would name failures from part of a
    /// log as if from all of it — and they are the offered parts, at most
    /// the planned ones, so a part with nothing a model could add is neither
    /// asked nor charged. A part that fails ends the item with that part's
    /// reason, never a fall back to the deterministic rule, which would
    /// report a model-assisted projection that no model made. What the model
    /// answers is added to the rule's floor, and never replaces it.
    #[allow(clippy::too_many_arguments)]
    fn project(
        &self,
        item: &str,
        reference: &Value,
        artifact: &str,
        descriptor: &Value,
        assist: &Assist<'_>,
        decided: &mut Decided,
        tick: &mut Tick,
    ) -> Result<(), TickError> {
        use crate::projection::{self, Next};
        let unmet = |decided: &mut Decided, reason: &str| {
            decided.unmet.push(Unmet {
                item: item.to_string(),
                reason: reason.to_string(),
            });
        };
        // **Too large is decided before a byte is read**, so a store holding
        // a very large artifact cannot make a compile allocate it only to
        // find it is too large.
        let size = int(descriptor, &["size"]);
        if projection::too_large(usize::try_from(size).unwrap_or(usize::MAX)) {
            unmet(decided, projection::INSUFFICIENT_CAPACITY);
            return Ok(());
        }
        let digest = text(reference, &["digest"]).to_string();
        let Some(bytes) = self
            .store
            .read_object(&digest)?
            .filter(|bytes| cbr_encoding::digest_bytes(bytes) == digest)
        else {
            unmet(decided, "evidence_unavailable");
            return Ok(());
        };
        let read = projection::read(&bytes);
        let capture: Vec<(String, String)> = list(descriptor, &["capture", "anchors"])
            .iter()
            .map(|anchor| {
                (
                    text(anchor, &["kind"]).to_string(),
                    text(anchor, &["id"]).to_string(),
                )
            })
            .collect();
        let subject = projection::Subject {
            artifact,
            digest: &digest,
            capture: &capture,
        };
        // **Zero is the deterministic path**, as it is for selection and
        // discovery: a request that authorised no investigation gets the
        // rule, from the same binary.
        let serving = self.model.clone().filter(|_| assist.investigation > 0);
        let plan = projection::partition(&read, &bytes, subject, serving.is_some());
        let choice = match projection::next(&read, &bytes, &plan, subject, serving.is_some()) {
            Next::Insufficient => {
                unmet(decided, projection::INSUFFICIENT_CAPACITY);
                return Ok(());
            }
            Next::Carry(choice) => choice,
            Next::Ask => {
                let Some(serving) = serving else {
                    unmet(decided, crate::derivation::UNREADABLE);
                    return Ok(());
                };
                // **The claim is the questions the model is offered**, which
                // are at most the parts the input fills: a planned part
                // holding nothing a model could add is not asked, and is not
                // charged for.
                if !assist.room_for(plan.asked.len()) {
                    unmet(decided, INVESTIGATION_EXHAUSTED);
                    return Ok(());
                }
                let of = plan.asked.len();
                let mut asked = Vec::with_capacity(of);
                for (index, units) in plan.asked.iter().enumerate() {
                    let (candidates, labels) =
                        projection::candidates(&read, &bytes, artifact, units);
                    let ids: Vec<String> = candidates
                        .iter()
                        .map(|candidate| candidate.id.clone())
                        .collect();
                    let selector = projection::selector(artifact, &digest, index + 1, of);
                    // **The key is the question**: this item, this part of
                    // this artifact under this format.
                    let key = format!(
                        "model:{}:{}:project:{item}:{}",
                        assist.job,
                        assist.request,
                        cbr_encoding::digest_bytes(selector.as_bytes())
                    );
                    let part = projection::Part {
                        artifact: artifact.to_string(),
                        format: read.format.name(),
                        size: read.size,
                        number: index + 1,
                        of,
                        labels,
                        floor: plan.floor.clone(),
                    };
                    let answer = self.ask_step(
                        &serving,
                        assist,
                        &key,
                        &selector,
                        item,
                        candidates,
                        Step::Project(part),
                        tick,
                    )?;
                    asked.push((answer, ids, units));
                }
                // **Every part is started before any is waited for**, so the
                // pool holds as many as its bound allows at once.
                if asked
                    .iter()
                    .any(|(answer, ..)| matches!(answer, Asked::NotReady))
                {
                    return Err(TickError::NotReady);
                }
                let mut picked = Vec::new();
                for (answer, ids, units) in asked {
                    let reason = match answer {
                        Asked::Answered(crate::derivation::Answer::ChoseMany(chosen)) => {
                            // The closed set, checked again on the way out:
                            // an id is only ever resolved against the part it
                            // was offered in.
                            let mut unoffered = false;
                            for id in chosen {
                                match ids.iter().position(|offered| *offered == id) {
                                    Some(position) => picked.push(units[position]),
                                    None => unoffered = true,
                                }
                            }
                            if !unoffered {
                                continue;
                            }
                            crate::selection::NOT_OFFERED
                        }
                        Asked::Answered(crate::derivation::Answer::Unmet(reason)) => reason,
                        Asked::Answered(_) | Asked::NotReady => crate::derivation::UNREADABLE,
                    };
                    unmet(decided, reason);
                    return Ok(());
                }
                // What the model added, **beside every failure the parser
                // found**: `render` carries the rule's floor first and never
                // takes any of it away.
                projection::choose_by_model(&read, &picked)
            }
        };
        // **A projection over its own bound is never published**: `render`
        // refuses a model arm that does not fit, which `offer` makes
        // unreachable, and the item says so rather than carry it.
        let Some(rendered) = projection::render(&read, &bytes, &choice, &plan, subject) else {
            unmet(decided, projection::INSUFFICIENT_CAPACITY);
            return Ok(());
        };
        decided.evidence.push(compiler::EvidenceSection {
            item: item.to_string(),
            summary: rendered.content,
            evidence: reference.clone(),
        });
        // **Every omission is a typed record**, in the packet the way M3's
        // are: under the protocol's own reason, with a section id the
        // projection's ledger names, where its extent is.
        for (number, reason) in rendered.omitted {
            decided.omitted.push(object(vec![
                ("item_id", string(item)),
                ("section_id", string(&crate::ids::omission(item, number))),
                ("reason", string(reason)),
            ]));
        }
        Ok(())
    }

    /// Extend a cited span into the chunks either side of it, while the
    /// excerpt still fits.
    ///
    /// **A chunk boundary is an artefact of indexing and a reader should
    /// not lose an answer to it.** Both M3d pilots ran into this: the span
    /// that ranked held the question's words and the sentence that answered
    /// them sat a few lines past its edge, on the far side of a fixed
    /// twenty-line boundary that has nothing to do with the question.
    ///
    /// The rule:
    ///
    /// - Only a span that **ranked** is extended. A file cited because
    ///   nothing matched in it (`first-chunk`) has no reason to prefer one
    ///   neighbour over another.
    /// - Chunks tile a file exactly, so the result is still **one
    ///   contiguous byte range** of the artifact, which is what the locator
    ///   promises and what a reader checks by fetching the citation.
    /// - Both neighbours are taken when the whole of it stays within
    ///   [`compiler::EXCERPT_BYTES`]. When only one fits, the side holding
    ///   more of the query's terms is taken; on a tie the following chunk
    ///   wins, because prose and code continue forward.
    /// - Nothing is extended when the span already fills the budget.
    fn widen(
        &self,
        frontier: &Frontier,
        path: &str,
        span: (i64, i64, u32, u32),
        bytes: &[u8],
        terms: &[String],
    ) -> (i64, i64, u32, u32) {
        let (start_byte, end_byte, start_line, end_line) = span;
        let budget = compiler::EXCERPT_BYTES as i64;
        if end_byte - start_byte >= budget {
            return span;
        }
        let Ok((before, after)) = cbr_memory::lexical::neighbours(
            self.store.connection(),
            &frontier.tree,
            path,
            start_byte,
            end_byte,
        ) else {
            return span;
        };
        let length = |one: &cbr_memory::lexical::Neighbour| one.end_byte - one.start_byte;
        let scored = |one: &cbr_memory::lexical::Neighbour| {
            let from = usize::try_from(one.start_byte)
                .unwrap_or(0)
                .min(bytes.len());
            let to = usize::try_from(one.end_byte)
                .unwrap_or(0)
                .clamp(from, bytes.len());
            let text = String::from_utf8_lossy(&bytes[from..to]).to_lowercase();
            terms
                .iter()
                .filter(|term| text.contains(term.as_str()))
                .count()
        };
        let room = budget - (end_byte - start_byte);
        let fits = |one: &Option<cbr_memory::lexical::Neighbour>| {
            one.as_ref().is_some_and(|one| length(one) <= room)
        };
        let take_before;
        let take_after;
        match (&before, &after) {
            (Some(one), Some(two)) if length(one) + length(two) <= room => {
                take_before = true;
                take_after = true;
            }
            (Some(one), Some(two)) if fits(&before) && fits(&after) => {
                let forward = scored(two) >= scored(one);
                take_before = !forward;
                take_after = forward;
            }
            _ => {
                take_before = fits(&before);
                take_after = fits(&after);
            }
        }
        let mut widened = span;
        if take_before && let Some(one) = before {
            widened.0 = one.start_byte;
            widened.2 = one.start_line;
        }
        if take_after && let Some(one) = after {
            widened.1 = one.end_byte;
            widened.3 = one.end_line;
        }
        let _ = (start_line, end_line);
        widened
    }

    /// Where an item's bytes come from when the path it named is a
    /// symbolic link.
    ///
    /// **A link never supplies content.** Its blob holds the target's
    /// *name*, not the target's text, and citing those bytes gives a reader
    /// a path where they asked for a file. M3d's Knowscroll pilot did
    /// exactly that: `AGENTS.md` is mode `120000` pointing at `CLAUDE.md`,
    /// the indexer skipped it as it should — a link's content is a path,
    /// not text of this tree — and `select_source` read the blob anyway and
    /// reported the item satisfied by nine bytes reading `CLAUDE.md`.
    ///
    /// The rule, in one piece:
    ///
    /// - A link is resolved **inside the same tree**, relative to its own
    ///   directory. An absolute target, a target that climbs out of the
    ///   tree, and a chain longer than [`LINK_HOPS`] are all refused: the
    ///   provider answers from trees, and anything outside one is not a
    ///   thing it can cite.
    /// - If that resolves to a **regular file of the same tree**, the item
    ///   is satisfied from the target and cited **at the target's path**,
    ///   so the citation a reader fetches is the bytes they were shown. The
    ///   locator names the link it came through.
    /// - Otherwise the item is **unmet**. CONTEXT bounds an item reason at
    ///   64 characters, so the reason is a code and the coverage carries
    ///   one gap line saying which link it was and where it pointed.
    fn follow_link(&self, frontier: &Frontier, path: &str) -> Link {
        let Ok(entries) = cbr_identity::tree_entries(&frontier.checkout, &frontier.tree) else {
            return Link::Absent;
        };
        let regular = |mode: &str| mode == "100644" || mode == "100755";
        let mut current = path.to_string();
        for _ in 0..LINK_HOPS {
            let Some(entry) = entries.iter().find(|entry| entry.path == current) else {
                return Link::Absent;
            };
            if regular(&entry.mode) {
                return if current == path {
                    Link::Direct
                } else {
                    Link::Followed { target: current }
                };
            }
            if entry.mode != "120000" {
                // A submodule or anything else that is not a file of this
                // tree: there is nothing here to cite.
                return Link::Broken {
                    link: current,
                    target: String::new(),
                };
            }
            let Ok(Some(bytes)) =
                cbr_identity::read_blob_bounded(&frontier.checkout, &entry.blob, LINK_TARGET_BYTES)
            else {
                return Link::Broken {
                    link: current,
                    target: String::new(),
                };
            };
            let target = String::from_utf8_lossy(&bytes).trim().to_string();
            match join_in_tree(&current, &target) {
                Some(next) => current = next,
                None => {
                    return Link::Broken {
                        link: current,
                        target,
                    };
                }
            }
        }
        Link::Broken {
            link: current,
            target: "a chain of links too long to follow".into(),
        }
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
        assist: &Assist<'_>,
        tick: &mut Tick,
    ) -> Result<Choice, TickError> {
        use cbr_memory::retrieval::{Ask, Bounds, Origin, Readable};
        let entries = match cbr_identity::tree_entries(&frontier.checkout, &frontier.tree) {
            Ok(entries) => entries,
            Err(_) => return Ok(Choice::Absent),
        };
        let Some(entry) = entries.into_iter().find(|entry| entry.path == path) else {
            return Ok(Choice::Absent);
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
            return Ok(Choice::Absent);
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
        // **Model-assisted selection, when the request asked for one.**
        // BM25 ranked these spans; which of them actually answers the
        // question is a judgement, and a request that authorised an
        // investigation is a request that asked for one to be made. Every
        // bound is already in place: a closed set of CBR's own
        // candidates, one call, this request's deadline, and the pool's
        // concurrency.
        let ranked = match self.assist(&answer.found, &bytes, path, &query, assist, item, tick)? {
            Assisted::Chose(index) => index,
            Assisted::NotReady => return Err(TickError::NotReady),
            Assisted::Unmet(reason) => return Ok(Choice::Unmet(reason)),
        };
        let (start_byte, end_byte, start_line, end_line, origin) = match answer.found.get(ranked) {
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

        // A ranked span reaches its neighbours; a first chunk does not.
        let terms = cbr_memory::lexical::query_terms(&query);
        let (start_byte, end_byte, start_line, end_line) = if origin == "first-chunk" {
            (start_byte, end_byte, start_line, end_line)
        } else {
            self.widen(
                frontier,
                path,
                (start_byte, end_byte, start_line, end_line),
                &bytes,
                &terms,
            )
        };

        let digest = cbr_encoding::digest_bytes(&bytes);
        let selection = Selection {
            item: text(item, &["item_id"]).to_string(),
            // `decide_item` sets this when the item named a link and this
            // is its target.
            via: None,
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
            excerpt: compiler::excerpt(&bytes, start_byte, end_byte, &terms),
        };
        self.seal_source(tick, &selection, &bytes)?;
        Ok(Choice::Selected(Box::new(selection)))
    }

    /// Let go of everything `job` asked a model.
    ///
    /// **This is what cancellation is at this call site.** A thread that
    /// is mid-call is not killed — Rust cannot — but its answer is never
    /// read, so nothing it chose reaches a packet: *a cancelled call
    /// leaves no partial derivation record*. What it already spent stays
    /// in the ledger, and must: a call that went out and was charged is a
    /// charge, and forgetting it would overspend a shared quota.
    ///
    /// Called when a job ends for any reason, and when the last request
    /// of one is cancelled. Without it the answers sit in the pool for as
    /// long as the process lives; with it the memory and, for anything
    /// still running, the bound come back.
    fn release_job(&self, job: &str) {
        self.work.release_all(&format!("model:{job}:"));
    }

    /// Which of the ranked spans to cite, when a model helps choose.
    ///
    /// Answers `Chose(0)` — BM25's own first — whenever there is no model
    /// to ask, no investigation authorised, or nothing to choose between,
    /// so the deterministic path is the same code rather than a branch
    /// around it.
    #[allow(clippy::too_many_arguments)]
    fn assist(
        &self,
        found: &[cbr_memory::retrieval::Found],
        bytes: &[u8],
        path: &str,
        query: &str,
        assist: &Assist<'_>,
        item: &Value,
        tick: &mut Tick,
    ) -> Result<Assisted, TickError> {
        let Some(serving) = self.model.clone() else {
            return Ok(Assisted::Chose(0));
        };
        if assist.investigation <= 0 {
            return Ok(Assisted::Chose(0));
        }
        let candidates: Vec<crate::selection::Candidate> = found
            .iter()
            .enumerate()
            .map(|(index, found)| crate::selection::Candidate {
                id: format!("c{}", index + 1),
                kind: crate::selection::KIND_SPAN,
                path: path.to_string(),
                start_line: found.start_line as usize,
                end_line: found.end_line as usize,
                text: String::from_utf8_lossy(
                    usize::try_from(found.start_byte)
                        .ok()
                        .zip(usize::try_from(found.end_byte).ok())
                        .and_then(|(start, end)| bytes.get(start..end))
                        .unwrap_or_default(),
                )
                .to_string(),
            })
            .collect();
        if !crate::selection::worth_asking(&candidates) {
            return Ok(Assisted::Chose(0));
        }

        // **The key is the question, not the file.** One key per
        // request, path *and selector*: two items citing the same file of
        // the same request are one question only when they ask the same
        // thing, and two selectors rank the file differently. Sharing a
        // key across them would hand the second item an answer chosen
        // from a candidate list it was never shown — an index into the
        // wrong spans. The selector is digested because it is free text
        // and a key is not.
        let key = format!(
            "model:{}:{}:{path}:{}",
            assist.job,
            assist.request,
            cbr_encoding::digest_bytes(query.as_bytes())
        );
        // **The budget is spent here, on a question rather than on a
        // call.** A question this compile has already asked costs
        // nothing more; a new one costs one unit, and a request whose
        // budget is gone gets the typed reason and not BM25's own first,
        // which would report a model-assisted selection no model made.
        //
        // Nothing is sealed for it: a derivation record is one model
        // exchange, and no exchange happened. Only a call that was made
        // has something to say.
        if !assist.admits(&key) {
            return Ok(Assisted::Unmet(INVESTIGATION_EXHAUSTED));
        }
        // **The request's deadline, converted once.** The pool measures
        // monotonic time and the protocol measures instants; converting
        // at the moment of asking is one clock, where keeping a second
        // deadline beside the protocol's would be two of them disagreeing
        // about the same request. A deadline CBR cannot read is no
        // deadline rather than a guess.
        let deadline = crate::clock::unix_of(assist.deadline)
            .zip(crate::clock::unix_of(&assist.now))
            .map(|(deadline, now)| {
                std::time::Instant::now()
                    + std::time::Duration::from_secs(deadline.saturating_sub(now))
            });

        // What the record will remember of the question, built before
        // the call so the thing that was asked and the thing that is
        // recorded cannot drift apart.
        let offered = crate::derivation::offered(&candidates);
        // The ids as they were offered, kept behind because the
        // candidates themselves move into the work. An answer is
        // resolved against this and nothing else.
        let ids: Vec<String> = candidates
            .iter()
            .map(|candidate| candidate.id.clone())
            .collect();
        let item = text(item, &["item_id"]).to_string();

        // **An offline rebuild answers here and asks nothing.** The
        // question's digest is what a retained record is found by — the
        // model, the task, the selector and every candidate in the
        // order it was offered — so a file edited since the call is a
        // different question and finds nothing, which is the answer a
        // rebuild should give. Nothing is charged and nothing is
        // sealed: the record it read is the record it would write.
        if matches!(serving.wire, crate::provider::Wire::Replay) {
            let wanted = crate::derivation::Question {
                model: &serving.model,
                dialect: serving.dialect.name(),
                task: assist.task,
                selector: query,
                offered: &offered,
            }
            .digest();
            return Ok(match self.retained(&wanted, assist)? {
                Some(crate::derivation::Answer::Chose(id)) => {
                    match ids.iter().position(|offered| *offered == id) {
                        Some(index) if index < found.len() => Assisted::Chose(index),
                        _ => Assisted::Unmet(crate::selection::NOT_OFFERED),
                    }
                }
                Some(crate::derivation::Answer::Unmet(reason)) => Assisted::Unmet(reason),
                // **A record of another step's question, which cannot
                // happen and is not therefore assumed away.** A
                // discovery record answers a question whose selector
                // names its step, so its digest is never this one's;
                // if one ever arrived here the record would have been
                // read and still not answer what was asked, which is
                // what `UNREADABLE` means.
                Some(_) => Assisted::Unmet(crate::derivation::UNREADABLE),
                // **Never a call, and never BM25's own first.** A
                // question nothing retained an answer to is an item
                // this rebuild cannot honestly satisfy, and saying so
                // is the difference between a rebuild and a rerun.
                None => Assisted::Unmet(crate::derivation::NOT_RETAINED),
            });
        }

        let asking = || {
            let serving = std::sync::Arc::clone(&serving);
            let (job, request) = (assist.job.to_string(), assist.request.to_string());
            let (task, selector, now) = (
                assist.task.to_string(),
                query.to_string(),
                assist.now.clone(),
            );
            // Its own connection, because it runs off the session's,
            // and opened inside the factory so a tick that is only asking
            // opens none.
            let beside = self.store.open_beside();
            move || {
                let beside = beside.map_err(|_| "store_unavailable")?;
                // Built here rather than in the caller: the pool drops an
                // unused closure, and a tick that is only asking should
                // not pay for a body nobody sends.
                let body = crate::selection::ask(&serving.model, &task, &selector, &candidates);
                let started = std::time::Instant::now();
                let outcome = serving.ask(&beside, &now, &job, &request, &body);
                let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                // **A call that was made is recorded, whatever it came
                // to.** An answer that could not be used is still a
                // charge against a shared quota and still a fact about
                // the request; only a call that never happened has
                // nothing to say.
                let (answer, cost) = crate::derivation::taken(outcome, |reply| {
                    match crate::selection::chosen(reply, &candidates) {
                        Ok(index) => crate::derivation::Answer::Chose(candidates[index].id.clone()),
                        Err(reason) => crate::derivation::Answer::Unmet(reason),
                    }
                });
                Ok(crate::derivation::record(&crate::derivation::Made {
                    question: &crate::derivation::Question {
                        model: &serving.model,
                        dialect: serving.dialect.name(),
                        task: &task,
                        selector: &selector,
                        offered: &offered,
                    },
                    answer,
                    spend: crate::derivation::Spend { cost, latency_ms },
                    job: &job,
                    request: &request,
                    item: &item,
                    made_at: &now,
                }))
            }
        };
        Ok(match self.work.progress(&key, deadline, asking) {
            // Started, or waiting for room. Nothing is written and
            // nothing is logged; the next tick asks again.
            crate::work::Progress::Running | crate::work::Progress::Deferred => Assisted::NotReady,
            // **Not released here.** The compile releases everything this
            // job asked once it has produced its script: freeing a key the
            // moment it was read would have the next tick's compile ask
            // the same question again, at a second charge.
            //
            // **Taking the answer is what seals the record**, in this
            // tick's own batch. A call whose answer is never taken —
            // cancelled, or a job that ended — seals nothing, which is
            // *a cancelled call leaves no partial derivation record*;
            // what it spent stays in the ledger, because a call that
            // went out was charged.
            crate::work::Progress::Done(value) => {
                // A selection shows spans of a repository and no evidence.
                self.seal_derivation(tick, &value, assist, &[])?;
                match crate::derivation::answer_of(&value) {
                    // The closed set, checked a second time and on the
                    // way out: an id is only ever resolved against the
                    // candidates it was offered from.
                    Some(crate::derivation::Answer::Chose(id)) => {
                        match ids.iter().position(|offered| *offered == id) {
                            Some(index) if index < found.len() => Assisted::Chose(index),
                            // It answered with something that is not one
                            // of the candidates it was offered. Nothing
                            // guesses on its behalf.
                            _ => Assisted::Unmet(crate::selection::NOT_OFFERED),
                        }
                    }
                    Some(crate::derivation::Answer::Unmet(reason)) => Assisted::Unmet(reason),
                    // As above: a record shaped for another step is one
                    // this call site cannot read as an answer.
                    Some(_) | None => Assisted::Unmet(crate::derivation::UNREADABLE),
                }
            }
            crate::work::Progress::Failed(reason) => Assisted::Unmet(reason),
            crate::work::Progress::TimedOut => Assisted::Unmet("model_call_timed_out"),
        })
    }

    /// The answer a retained derivation holds for this exact question,
    /// if this job may read it.
    ///
    /// **Scanned, and only by a replay launch.** An ordinary serving
    /// launch never reaches this; a rebuild is an offline operation
    /// where a scan per question costs nothing anybody is waiting on.
    /// An index would be a second place for the truth to live, and the
    /// truth is the sealed record.
    ///
    /// **A retained answer is reused only under a readable set this job
    /// also has.** Without that, replaying would be a way of reading a
    /// wider job's answers from a narrower one — the readable-set gate
    /// on `evidence.fetch` walked around from the inside.
    ///
    /// **Every covered record is read, not the first one found.** A
    /// model is not a function: two calls can ask one question and be
    /// told different things, and m4e reruns the same questions live, so
    /// a store holding both is the ordinary state rather than a corner
    /// case. The id they are stored under is a digest covering the
    /// instant each was made, so "the first match" would make the
    /// rebuilt packet depend on a hash of a timestamp — the same history
    /// producing either of two packets. If every covered record says the
    /// same thing there is nothing to choose between; if any two differ
    /// the item is [`crate::derivation::AMBIGUOUS`].
    ///
    /// **A retained failure beside a retained choice is a
    /// disagreement**, and deliberately so. Preferring the choice would
    /// be the rebuild deciding which of two histories to reproduce —
    /// improving on the past rather than replaying it — which is the
    /// same fault as taking the first match, with better manners.
    ///
    /// **A purged record is not read at all.** A purge is a deliberate
    /// act of destroying evidence, and its object can still be on disk
    /// until collection runs: answering from one would serve what
    /// somebody ordered destroyed.
    fn retained(
        &self,
        question: &str,
        assist: &Assist<'_>,
    ) -> Result<Option<crate::derivation::Answer>, TickError> {
        let mut agreed: Option<crate::derivation::Answer> = None;
        for (id, value) in self.store.subjects_of_kind(crate::evidence::ARTIFACT)? {
            if !id.starts_with("der.") {
                continue;
            }
            let record = parse_record(&value)?;
            if text(&record, &["state"]) != "sealed" {
                continue;
            }
            if !context::is_null(at(&record, &["purge"])) {
                continue;
            }
            if !crate::derivation::covers(
                at(&record, &["readable_under"]),
                assist.view,
                assist.claims,
                assist.evidence,
            ) {
                continue;
            }
            let Some(bytes) = self
                .store
                .read_object(text(&record, &["descriptor", "digest"]))?
            else {
                continue;
            };
            let Ok(sealed) = cbr_encoding::parse(&bytes) else {
                continue;
            };
            if crate::derivation::question_digest_of(&sealed) != question {
                continue;
            }
            // A record this build cannot read is a failure with a name,
            // not a question to ask a provider — and it takes part in
            // the agreement above, because a record nobody can read is
            // no evidence that the others are right.
            let found = crate::derivation::answer_of(&sealed).unwrap_or(
                crate::derivation::Answer::Unmet(crate::derivation::UNREADABLE),
            );
            match &agreed {
                None => agreed = Some(found),
                Some(already) if *already == found => {}
                Some(_) => {
                    return Ok(Some(crate::derivation::Answer::Unmet(
                        crate::derivation::AMBIGUOUS,
                    )));
                }
            }
        }
        Ok(agreed)
    }

    /// Seal one model exchange as evidence, unless an artifact already
    /// holds it.
    ///
    /// **In the tick's own batch**, with the same atomicity every other
    /// record gets: the object is published and verified from disk
    /// before the row that names it is committed, so a crash leaves an
    /// object no row names and never a row naming an object that is not
    /// there. There is no half-written derivation to find.
    ///
    /// The id is the record's own digest, so the same exchange sealed
    /// twice is one artifact, and two answers to one question are two
    /// records rather than one overwriting the other.
    fn seal_derivation(
        &self,
        tick: &mut Tick,
        record: &Value,
        assist: &Assist<'_>,
        evidence: &[String],
    ) -> Result<(), TickError> {
        let bytes = crate::derivation::bytes(record);
        let digest = cbr_encoding::digest_bytes(&bytes);
        let id = crate::derivation::artifact_id(&digest);
        let artifact_key = crate::evidence::artifact_key(&id);
        if self.store.revision(&artifact_key)? != 0 || tick.batch.written(&artifact_key).is_some() {
            return Ok(());
        }
        let payload = crate::derivation::descriptor(record, &digest, bytes.len());
        let descriptor = crate::evidence::parse_descriptor(&payload, &self.config.principal)?;
        self.store.publish_object(&digest, &bytes)?;
        let sealed = object(vec![
            ("descriptor", descriptor),
            ("state", string("sealed")),
            ("staged_at", string(&tick.now)),
            // **The job's own readable set, carried with the record.**
            // A derivation names paths and line ranges of repositories
            // the job could read; serving it to a reader who could not
            // read them is the M3 leak arriving through a new door.
            (
                "readable_under",
                //
                // **And the evidence the question showed**, since m5a: not
                // the job's whole set, because a part of one artifact's
                // projection reveals nothing of another the same request
                // named, and a reader who may read this one is owed it.
                crate::derivation::readable_under(assist.view, assist.claims, evidence),
            ),
        ]);
        let revision = self.tick_save(tick, &artifact_key, &sealed)?;
        tick.batch.event(
            &artifact_key,
            revision,
            "evidence.artifact.sealed",
            object(vec![
                ("digest", string(&digest)),
                ("size", Value::Int(bytes.len() as i64)),
            ]),
        );
        Ok(())
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
            let needed = context::needed_bytes(needed);
            set(&mut record, "state", string("refused"));
            set(&mut record, "reason", string(context::BUDGET_INSUFFICIENT));
            set(&mut record, "needed", needed.clone());
            let changed = event(
                "context.request.changed",
                object(vec![
                    ("state", string("refused")),
                    ("reason", string(context::BUDGET_INSUFFICIENT)),
                ]),
                &command.caused_by,
            );
            let outcome = object(vec![
                ("request", request_subject),
                ("state", string("refused")),
                ("reason", string(context::BUDGET_INSUFFICIENT)),
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
        let evidence = self.readable_evidence(params, payload)?;
        let mut joined = None;
        if self.selected_feature("context.shared_jobs") {
            for (job_id, _, value) in self.store.subjects_in_recorded_order(JOB)? {
                let mut job = parse_record(&value)?;
                if context::may_join(&job, &principal, payload, &view, &claims, &evidence) {
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
            set(
                &mut job,
                "readable_evidence",
                Value::Array(evidence.iter().map(|id| string(id)).collect()),
            );
            // Compiling is a production capability, and a conformance
            // launch is a test harness: there, a request with no script is
            // still a request nothing prepares, which is what every context
            // fixture was measured against.
            //
            // **A conformance launch may ask for it, with
            // `context.compile`.** m4c is why: the model call site lives
            // inside compiling, and reaching it needs the fake transport,
            // which only a conformance launch may have — a production one
            // serves no test control at all. Without the opt-in the two
            // could never overlap and the call site could not be tested
            // under `SIGKILL` at all. **No fixture sets it**, so no
            // fixture's expectations move; `a_conformance_launch_prepares_
            // nothing_unless_it_asks_to_compile` is what holds that.
            let compiling = match self.config.mode {
                crate::config::Mode::Production => self.config.context.0 == Value::Null,
                crate::config::Mode::Conformance => {
                    *at(&self.config.context.0, &["compile"]) == Value::Bool(true)
                }
            };
            if script.is_empty() && compiling {
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
        let committed =
            self.commit_context(&command, &request_key, &record, cancelled, also, outcome);
        // **Cancellation, at this call site — and only once it is a
        // fact.** A job nobody is waiting for any more lets go of
        // everything it asked a model: the thread is not killed, but its
        // answer is never read, so nothing it chose reaches a packet.
        //
        // After the commit, not before: a cancel that failed to commit is
        // a job still running, and freeing its answers there would have
        // the next tick ask every question again. A job that continues
        // keeps them either way, because its other requests are waiting
        // for exactly those answers.
        if committed.is_ok() && !job_continues {
            self.release_job(&job_id);
        }
        committed
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
        // Once published, the item results are the current revision's. A
        // request whose job ended without publishing it keeps the results
        // it ended with, which never read the job's sections: what the job
        // prepared was never delivered.
        let items = match (packets.last(), record.get("ended_items")) {
            (Some(facts), _) if text(&record, &["state"]) != "preparing" => {
                list(facts, &["items"]).to_vec()
            }
            (_, Some(Value::Array(ended))) => ended.clone(),
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
                outgoing: Vec::new(),
            };
            // The tick's packets leave only once it has run to the end,
            // every one of them guarded, and before its batch commits.
            let advanced = self
                .advance(&mut job, &mut tick)
                .and_then(|ended| self.seal_packets(&job, &mut tick).map(|()| ended));
            match advanced {
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
                            // **A job that has ended is a job whose
                            // answers nobody will ask for**, however it
                            // ended: published, out of investigation, or
                            // past its deadline. Here rather than in
                            // `finish`, because the job record does not
                            // carry its own id and this is where it is.
                            self.release_job(&job_id);
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
                // Nothing is written and nothing is logged: this is the
                // ordinary state of a job waiting for work that is running.
                Err(TickError::NotReady) => continue,
                Err(TickError::IdOutsideGrammar { request, pointers }) => {
                    // **The job ends, as `packet_invalid`** (the owner's
                    // ruling of 2026-09-26): nothing it prepares can be
                    // published, so waiting would leave its requests
                    // `preparing` for ever, compiled and logged again at
                    // every tick. The tick's own batch is dropped; the
                    // ending is built from the stored records and committed
                    // before anything is logged, so a line in the log is a
                    // refusal that happened. A store failure there is a
                    // store failure like any other in this loop.
                    //
                    // The job, the request and the pointers are all that is
                    // logged; the ids themselves never are.
                    let ended = self.end_refused_job(&job_id)?;
                    let suffix = if ended {
                        "; the job has ended: packet_invalid"
                    } else {
                        ""
                    };
                    eprintln!(
                        "cbr-provider: context job {job_id}: request {request}: packet not \
                         published; an id at {} is missing, outside the identifier grammar or \
                         repeated{suffix}",
                        pointers.join(", ")
                    );
                }
                Err(TickError::Protocol(error)) => return Err(error),
            }
        }
        Ok(())
    }

    /// **End a job whose packet the publication guard refused**, as
    /// `packet_invalid`, in one provider batch of its own. Returns whether
    /// it ended it: a job no longer stored as `running` is left alone.
    ///
    /// Built from the job and its requests **as stored**, never from the
    /// tick that was refused: that tick's sections, cursor, captures and
    /// any revision it published are dropped with its batch, so nothing of
    /// the refused tick is committed. A section an earlier tick committed
    /// stays in the ended job's stored record, which no operation serves.
    /// Every subscriber still `preparing` is `refused` with the reason, its
    /// items ended by [`context::refused_items`]; a subscriber already
    /// published (under `context.updates`) keeps its state and its current
    /// revision, and learns of the ending from `context.job.ended`. The
    /// requests' changes come first and the job's ending last, the order
    /// `finish` already gives them. An ended job never leaves a subscriber
    /// `preparing`: the next tick skips it.
    fn end_refused_job(&mut self, job_id: &str) -> Result<bool, ProtocolError> {
        let Some((_, mut job)) = self.context_record(JOB, job_id)? else {
            return Ok(false);
        };
        if text(&job, &["state"]) != "running" {
            return Ok(false);
        }
        let mut tick = Tick {
            now: self.clock.now(),
            batch: Batch::default(),
            captures: Vec::new(),
            outgoing: Vec::new(),
        };
        let job_subject = subject(JOB, job_id);
        for request in subscribers(&job) {
            let Some((_, record)) = self.context_record(REQUEST, &request)? else {
                continue;
            };
            if text(&record, &["state"]) != "preparing" {
                continue;
            }
            self.refuse_request(
                &mut tick,
                &request,
                record,
                &job_subject,
                context::PACKET_INVALID,
                None,
            )?;
        }
        set(&mut job, "state", string("ended"));
        set(&mut job, "reason", string(context::PACKET_INVALID));
        let job_key = key(JOB, job_id);
        let revision = self.tick_save(&mut tick, &job_key, &job)?;
        tick.batch.event(
            &job_key,
            revision,
            "context.job.ended",
            object(vec![("reason", string(context::PACKET_INVALID))]),
        );
        crate::barriers::pause(crate::barriers::PACKET_REFUSED_BEFORE_COMMIT);
        self.store
            .commit_provider_batch(&tick.batch.writes, &tick.batch.events, &tick.now)?;
        // After the commit, as a cancel does: an ending that failed to
        // commit is a job still running.
        self.release_job(job_id);
        Ok(true)
    }

    /// Refuse one request in `tick`, for `reason`: `refused`, its items
    /// ended by [`context::refused_items`], `needed` when the reason has
    /// one, one revision, and `context.request.changed` naming its job.
    fn refuse_request(
        &self,
        tick: &mut Tick,
        request: &str,
        mut record: Value,
        job_subject: &Value,
        reason: &str,
        needed: Option<i64>,
    ) -> Result<(), ProtocolError> {
        let ended_items = context::refused_items(&record, reason);
        set(&mut record, "state", string("refused"));
        set(&mut record, "reason", string(reason));
        if let Some(needed) = needed {
            set(&mut record, "needed", context::needed_bytes(needed));
        }
        set(&mut record, "ended_items", Value::Array(ended_items));
        let request_key = key(REQUEST, request);
        let revision = self.tick_save(tick, &request_key, &record)?;
        tick.batch.event(
            &request_key,
            revision,
            "context.request.changed",
            object(vec![
                ("state", string("refused")),
                ("job", job_subject.clone()),
                ("reason", string(reason)),
            ]),
        );
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
                        let reason = INVESTIGATION_EXHAUSTED;
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
        // **Checked before the packet leaves the tick**, and before any
        // packet of the tick does: nothing of a packet is captured, sent to
        // the evidence peer or written to this store until the tick has run
        // to the end ([`Provider::seal_packets`]), so a packet refused here
        // takes every other packet of its tick with it, however valid, and
        // none of them leaves anything behind. A compiled job has already
        // written the objects of the sources and derivations it sealed this
        // tick (`seal_source`, `seal_derivation`); a refusal drops the
        // tick's batch, so no row names them, and they are left, once, for
        // the start-time collection pass: the job then ends, as
        // `packet_invalid`, and is never compiled again.
        let pointers = context::ids_outside_grammar(&packet.facts, &packet.artifact);
        if !pointers.is_empty() {
            return Err(TickError::IdOutsideGrammar {
                request: request.to_string(),
                pointers,
            });
        }

        let provider = match PeerConfig::from_value(self.config.context.0.get("evidence_provider"))
        {
            Some(evidence) => {
                let provider = evidence.provider_id.clone();
                tick.outgoing.push(Outgoing::Peer {
                    evidence,
                    request: request.to_string(),
                    artifact: packet.artifact.clone(),
                    digest: packet.digest.clone(),
                    content: packet.content.clone(),
                });
                provider
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

    /// **Let the tick's packets leave**, in the order it published them:
    /// each one for an evidence peer captured, sent and sealed there, and
    /// each one of this provider's own written as an object. Called once
    /// the tick has run to the end and before its batch commits, so every
    /// packet it publishes has passed the guard before any of them is
    /// captured, sent or sealed.
    ///
    /// A peer that does not seal fails the tick as before, keeping the
    /// capture instants of the packets attempted so far, the failed one
    /// included, so the retry replays what already applied.
    fn seal_packets(&self, job: &Value, tick: &mut Tick) -> Result<(), TickError> {
        for outgoing in std::mem::take(&mut tick.outgoing) {
            match outgoing {
                Outgoing::Peer {
                    evidence,
                    request,
                    artifact,
                    digest,
                    content,
                } => {
                    let captured_at = match at(job, &["captures", &artifact]).as_str() {
                        Some(kept) => kept.to_string(),
                        None => tick.now.clone(),
                    };
                    tick.captures.push((artifact.clone(), captured_at.clone()));
                    let descriptor =
                        context::packet_descriptor(&request, &digest, content.len(), &captured_at);
                    if let Err(failure) =
                        peer::publish_artifact(&evidence, &artifact, descriptor, &content)
                    {
                        return Err(TickError::NotSealed(NotSealed {
                            artifact,
                            failure,
                            captures: tick.captures.clone(),
                        }));
                    }
                }
                Outgoing::Object { digest, content } => {
                    self.store.publish_object(&digest, &content)?;
                    crate::barriers::pause(crate::barriers::PACKET_AFTER_OBJECT_PUBLISHED);
                }
            }
        }
        Ok(())
    }

    /// Seal a packet revision in this provider's own store, as an artifact
    /// whose producer principal is this provider (CONTEXT section 5). The
    /// artifact's rows go into the tick's batch here; its object is written
    /// by [`Provider::seal_packets`] once the tick has run, published and
    /// verified from disk before the batch naming it commits. If the commit
    /// never happens, the start-time collection pass removes the object.
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
        tick.outgoing.push(Outgoing::Object {
            digest: packet.digest.clone(),
            content: packet.content.clone(),
        });
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
