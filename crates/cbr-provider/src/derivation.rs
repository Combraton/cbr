//! What one model exchange came to, in the form it is sealed and replayed
//! from.
//!
//! # Why the record holds no repository text
//!
//! A derivation record says **which** spans were offered and which was
//! chosen; the spans themselves are the source artifacts the packet
//! already cites, sealed once and shared. So the record identifies what
//! was shown by digest, and a store that keeps derivations forever keeps
//! no second copy of anybody's repository. The redacted exchange bytes —
//! the request as it went out and the response as it came back — are the
//! `model_calls` rows m4b writes at the recording boundary, and they are
//! the only place a request body is kept. [READINESS §6] says what is
//! retained and for how long.
//!
//! # Why the question has a digest
//!
//! A retained answer is **an id out of a closed set**, and reusing it for
//! a different set would be choosing from a list the model never saw. So
//! a record is found by the digest of its whole question — the model, the
//! task, the selector, and every candidate that was offered, in the order
//! they were offered, each identified by its path, its lines and the
//! digest of the text shown. A file edited since the call is a different
//! question and finds nothing, which is the answer a replay should give.
//!
//! # Why the format is `/3`, and what becomes of a `/2` record
//!
//! **`/3` accounts for every attempt.** A record's `usage` is the whole
//! question's cost, each attempt as the ledger settled it, so its `tokens`
//! equals the question's ledger rows; `usage.attempts` keeps the attempts
//! one by one. A `/2` record carried the usage of the attempt that ended
//! the question and nothing of a repair before it, which is how live run 3
//! came to hold a record saying 4,827 tokens for a step the ledger charged
//! 5,222 and 4,827.
//!
//! **A `/2` record still replays, unchanged.** A rebuild finds a record by
//! its question's digest and reads its answer; it reads neither `format`
//! nor `usage`, and the question is built exactly as it was. So the stores
//! of the three live runs, which hold nothing but `/2` records, rebuild
//! under this build as they did under the one that wrote them. What a `/2`
//! record cannot say is what its repairs cost: for that, its ledger is the
//! authority, as it always was.
//!
//! `/2` was m4e's: m4e offers a model **claims** as well as spans, and a
//! claim has no line range, so a candidate says which kind it is and that
//! member is part of the question's digest. A record sealed by m4d never
//! answers a question asked since, which is the honest answer, because the
//! two questions showed the model different things. No `/1` record exists
//! anywhere but in a test store.
//!
//! # What a record cannot be used for
//!
//! Widening. Everything about the record is a fact about a question CBR
//! composed out of its own candidates ([`crate::selection`]), so a record
//! written by anyone, at any time, still cannot name a path, a span or a
//! label the deterministic path did not already produce.
//!
//! [READINESS §6]: ../../docs/work/m4/READINESS.md

use cbr_encoding::Value;

use crate::context::{at, canonical, list, object, string, text};

/// The retention class every derivation record is sealed under, so a
/// retention policy has one name to key on. **The policy itself is M6.**
pub const RETENTION_CLASS: &str = "model-derivation";
pub const MEDIA_TYPE: &str = "application/json";
pub const SOURCE_KIND: &str = "model_derivation";
pub const PRODUCER: &str = "cbr-model-runtime/2";

/// Every typed unmet reason a record may carry.
///
/// A record written by a later build can name one this build has never
/// heard of. It is **not** mapped to the nearest thing that sounds like
/// it: [`reason`] answers `None`, and a replay that cannot read a record
/// says so rather than guessing. The test beside this constructs every
/// refusal, settlement and unusable response the runtime has and asserts
/// each one is here, so a new reason cannot be added elsewhere and
/// quietly become unreadable.
pub const REASONS: &[&str] = &[
    "budget_exhausted",
    "model_answer_ambiguous",
    "index_unavailable",
    "job_over_ceiling",
    "local_bound_unsound",
    "model_answer_malformed",
    "model_answer_over_bound",
    "model_answer_not_structured",
    "model_answer_truncated",
    "model_call_failed",
    "model_call_timed_out",
    "model_choice_not_offered",
    "model_network_not_permitted",
    "model_not_sent",
    "model_output_unstructured",
    "model_reasoning_leaked",
    "model_tool_call_missing",
    "provider_quota_exhausted",
    "request_over_ceiling",
    "reservation_overrun",
    "run_over_ceiling",
    "store_unavailable",
    "work_panicked",
];

/// A record naming a reason this build cannot read is its own failure,
/// with its own name, rather than a reason guessed at.
pub const UNREADABLE: &str = "model_record_unreadable";
/// Nothing retained answers this question.
pub const NOT_RETAINED: &str = "model_answer_not_retained";

/// **Two retained records answer this question differently.**
///
/// A model is not a function: two calls can ask the same thing and be
/// told different things, and m4e reruns the same questions live, so a
/// store holding both is the ordinary state rather than a corner case.
/// A rebuild must be a function of the records rather than of the order
/// they are stored in — the artifact id is a digest covering the
/// instant each was made, so "the first one" is a hash of a timestamp.
/// So disagreement is the item's own reason, and the rebuild declines.
pub const AMBIGUOUS: &str = "model_answer_ambiguous";

/// The admission a call that never reached one had: CBR's own envelope
/// refused it, so nothing was counted and nothing was sent.
pub const NOT_ADMITTED: &str = "refused";

/// The typed reason `text` names, when this build knows it.
pub fn reason(text: &str) -> Option<&'static str> {
    REASONS.iter().copied().find(|known| *known == text)
}

/// One candidate as the record remembers it: what it was, where it was,
/// and the digest of the text that was shown — never the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub id: String,
    /// [`crate::selection::KIND_SPAN`] or [`crate::selection::KIND_CLAIM`].
    ///
    /// **Added at m4e, which is why the format is `/2`.** A claim has no
    /// line range, so without this a record would say `lines 0-0` of a
    /// path that is a claim id and read as a span of a file. It is part
    /// of the question's digest, so a `/1` record never answers a `/2`
    /// question — which is correct, because the two showed the model
    /// different things.
    pub kind: String,
    /// The path of the span, or the id of the claim.
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub digest: String,
}

/// What the model was shown, remembered.
pub fn offered(candidates: &[crate::selection::Candidate]) -> Vec<Offered> {
    candidates
        .iter()
        .map(|candidate| Offered {
            id: candidate.id.clone(),
            kind: candidate.kind.to_string(),
            path: candidate.path.clone(),
            start_line: candidate.start_line,
            end_line: candidate.end_line,
            digest: cbr_encoding::digest_bytes(candidate.text.as_bytes()),
        })
        .collect()
}

/// The question a model was asked, in the form that decides whether a
/// retained answer may be reused for it.
#[derive(Debug, Clone)]
pub struct Question<'a> {
    pub model: &'a str,
    pub dialect: &'a str,
    pub task: &'a str,
    pub selector: &'a str,
    pub offered: &'a [Offered],
}

impl Question<'_> {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("model", string(self.model)),
            ("dialect", string(self.dialect)),
            ("task", string(self.task)),
            ("selector", string(self.selector)),
            (
                "offered",
                Value::Array(
                    self.offered
                        .iter()
                        .map(|offered| {
                            object(vec![
                                ("id", string(&offered.id)),
                                ("kind", string(&offered.kind)),
                                ("path", string(&offered.path)),
                                ("start_line", Value::Int(offered.start_line as i64)),
                                ("end_line", Value::Int(offered.end_line as i64)),
                                ("digest", string(&offered.digest)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    /// The digest a retained answer is found by.
    pub fn digest(&self) -> String {
        cbr_encoding::digest_canonical(&self.to_value())
    }
}

/// What the model answered, or why nothing usable came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// One of the ids that were offered. Never a path, a line or a label.
    Chose(String),
    /// **The search terms a discovery step proposed, as the model wrote
    /// them**, every bound having held.
    ///
    /// The raw strings rather than the words CBR tokenised them into:
    /// tokenisation is a function of these, so a replay re-derives it,
    /// and a record of what CBR made of an answer is not a record of the
    /// answer. It is also the evidence that a planted file's instruction
    /// reached no further than a term.
    Proposed(Vec<String>),
    /// Several of the ids that were offered, in the order they were
    /// offered. Never a path, a line or a label.
    ChoseMany(Vec<String>),
    Unmet(&'static str),
}

/// What the question cost, whatever it came to: **every attempt**, as the
/// ledger settled each.
#[derive(Debug, Clone)]
pub struct Spend {
    pub cost: crate::model::Cost,
    pub latency_ms: u64,
}

/// One exchange, ready to seal.
pub struct Made<'a> {
    pub question: &'a Question<'a>,
    pub answer: Answer,
    pub spend: Spend,
    pub job: &'a str,
    pub request: &'a str,
    pub item: &'a str,
    pub made_at: &'a str,
}

fn tokens(value: Option<u64>) -> Value {
    match value {
        Some(tokens) => Value::Int(tokens as i64),
        None => Value::Null,
    }
}

/// **What a question's outcome becomes in its record**: the answer, and
/// every attempt it cost, however it ended.
///
/// Both call sites come through here, so the rule has one door: the cost is
/// passed through whole whether the question was answered, left unmet or
/// refused, and only the reading of a usable reply is the caller's, because
/// that is the only part a selection and a discovery step do differently.
/// Round 56 found why it matters: the rule was tested at selection's door and
/// not at discovery's, which is the door live run 3's repairs went through.
pub fn taken(
    outcome: crate::model::Outcome,
    read: impl FnOnce(&crate::wire::response::Reply) -> Answer,
) -> (Answer, crate::model::Cost) {
    match outcome {
        crate::model::Outcome::Answered { reply, cost } => (read(&reply), cost),
        crate::model::Outcome::Unmet { reason, cost } => (Answer::Unmet(reason), cost),
        // Refused by CBR's own envelope. The first ask of a question spent
        // nothing; a refused repair carries what the attempts before it were
        // charged.
        crate::model::Outcome::Refused { refusal, cost } => (Answer::Unmet(refusal.reason()), cost),
    }
}

/// The format a record is sealed under. Written and never read: see the
/// module's own account of what a `/2` record still does.
pub const FORMAT: &str = "cbr-model-derivation/3";

/// The sealed bytes of a derivation, as a value.
pub fn record(made: &Made<'_>) -> Value {
    let mut question = made.question.to_value();
    crate::context::set(&mut question, "digest", string(&made.question.digest()));
    let cost = &made.spend.cost;
    object(vec![
        ("format", string(FORMAT)),
        ("model", string(made.question.model)),
        (
            "admission",
            string(cost.admission().unwrap_or(NOT_ADMITTED)),
        ),
        (
            "usage",
            object(vec![
                ("tokens", tokens(cost.charged())),
                ("input_tokens", tokens(cost.input_charged())),
                ("counted_tokens", tokens(cost.last().counted)),
                ("repairs", Value::Int(cost.repairs as i64)),
                (
                    "attempts",
                    Value::Array(
                        cost.attempts
                            .iter()
                            .map(|attempt| {
                                object(vec![
                                    (
                                        "admission",
                                        attempt.admission.map(string).unwrap_or(Value::Null),
                                    ),
                                    ("tokens", tokens(attempt.tokens)),
                                    ("input_tokens", tokens(attempt.input_tokens)),
                                    ("count_tokens", tokens(attempt.count_tokens)),
                                    ("counted_tokens", tokens(attempt.counted)),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
        ),
        ("latency_ms", Value::Int(made.spend.latency_ms as i64)),
        ("question", question),
        (
            "answer",
            match &made.answer {
                Answer::Chose(id) => object(vec![("chose", string(id))]),
                Answer::Proposed(terms) => object(vec![(
                    "proposed",
                    Value::Array(terms.iter().map(|term| string(term)).collect()),
                )]),
                Answer::ChoseMany(ids) => object(vec![(
                    "chose_ids",
                    Value::Array(ids.iter().map(|id| string(id)).collect()),
                )]),
                Answer::Unmet(reason) => object(vec![("unmet", string(reason))]),
            },
        ),
        ("job", string(made.job)),
        ("request", string(made.request)),
        ("item", string(made.item)),
        ("made_at", string(made.made_at)),
    ])
}

/// The bytes that are sealed, and the digest they are sealed under.
pub fn bytes(record: &Value) -> Vec<u8> {
    canonical(record).into_bytes()
}

/// Content-addressed, like a cited blob: the same record sealed twice is
/// one artifact.
pub fn artifact_id(digest: &str) -> String {
    format!("der.{}", digest.rsplit(':').next().unwrap_or(digest))
}

/// The digest of the question this record answers.
pub fn question_digest_of(record: &Value) -> &str {
    text(record, &["question", "digest"])
}

/// What this record answered, when this build can read it.
pub fn answer_of(record: &Value) -> Option<Answer> {
    let answer = at(record, &["answer"]);
    if let Some(chose) = answer.get("chose").and_then(Value::as_str) {
        return Some(Answer::Chose(chose.to_string()));
    }
    // **A list whose members are not strings is not a list this build can
    // read**, and is the unreadable record's own reason rather than a
    // shorter list with the unreadable members dropped.
    if let Some(terms) = answer.get("proposed").and_then(Value::as_array) {
        return strings(terms).map(Answer::Proposed);
    }
    if let Some(ids) = answer.get("chose_ids").and_then(Value::as_array) {
        return strings(ids).map(Answer::ChoseMany);
    }
    let unmet = answer.get("unmet").and_then(Value::as_str)?;
    Some(Answer::Unmet(reason(unmet)?))
}

fn strings(values: &[Value]) -> Option<Vec<String>> {
    values
        .iter()
        .map(|value| value.as_str().map(str::to_string))
        .collect()
}

/// What a reader must be able to read before this derivation is served to
/// them: the job's own view and readable claims, and **the evidence the
/// question showed**.
///
/// The third was added at m5a. A part of a J2 projection shows a model
/// excerpts of an evidence artifact, and its record identifies each by
/// range and by the digest of its bytes — which a reader can confirm a
/// guess against. So a part's record is sealed under the artifact it was
/// cut from. **Only that one**, where the view and the claims are the
/// job's whole: a part of one artifact's projection reveals nothing of
/// another the same request named, and a reader who may read it is owed
/// it. A selection or a discovery step shows no evidence and names none,
/// and a record sealed before m5a has no list at all; both cover every
/// reader exactly as they did.
///
/// **Kept on the artifact record rather than in the sealed bytes.** Who
/// may read a thing is a fact about this store, and the sealed bytes are
/// the derivation itself — the same bytes, from the same question, would
/// be sealed under a different view in a different store.
pub fn readable_under(view: &[String], claims: &[String], evidence: &[String]) -> Value {
    object(vec![
        (
            "view",
            Value::Array(view.iter().map(|id| string(id)).collect()),
        ),
        (
            "readable_claims",
            Value::Array(claims.iter().map(|id| string(id)).collect()),
        ),
        (
            "readable_evidence",
            Value::Array(evidence.iter().map(|id| string(id)).collect()),
        ),
    ])
}

/// Whether a reader who can read exactly `view`, `claims` and `evidence`
/// may be served a derivation made under `under`.
///
/// The rule is **no wider than the reader**, not equal to them: an
/// authority reads everything, and a reader whose grant has since been
/// widened is not locked out of a derivation made under a narrower one.
pub fn covers(under: &Value, view: &[String], claims: &[String], evidence: &[String]) -> bool {
    let within = |path: &str, readable: &[String]| {
        list(under, &[path])
            .iter()
            .filter_map(Value::as_str)
            .all(|needed| readable.iter().any(|have| have == needed))
    };
    within("view", view)
        && within("readable_claims", claims)
        && within("readable_evidence", evidence)
}

/// The evidence a record was sealed under, which is all a door has to ask
/// a reader about: whether they may read each of these.
pub fn evidence_under(under: &Value) -> Vec<String> {
    list(under, &["readable_evidence"])
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// The descriptor a derivation record is sealed with. The anchors are the
/// job and the request it was made for, which is what makes it findable
/// from the thing it explains.
pub fn descriptor(record: &Value, digest: &str, size: usize) -> Value {
    object(vec![
        ("digest", string(digest)),
        ("size", Value::Int(size as i64)),
        ("media_type", string(MEDIA_TYPE)),
        ("producer", object(vec![("producer_id", string(PRODUCER))])),
        (
            "source",
            object(vec![
                ("kind", string(SOURCE_KIND)),
                ("id", string(question_digest_of(record))),
            ]),
        ),
        ("scope", string(text(record, &["job"]))),
        (
            "capture",
            object(vec![
                ("captured_at", string(text(record, &["made_at"]))),
                (
                    "anchors",
                    Value::Array(vec![
                        object(vec![
                            ("kind", string("context_job")),
                            ("id", string(text(record, &["job"]))),
                        ]),
                        object(vec![
                            ("kind", string("context_request")),
                            ("id", string(text(record, &["request"]))),
                        ]),
                        object(vec![
                            ("kind", string("model")),
                            ("id", string(text(record, &["model"]))),
                        ]),
                    ]),
                ),
            ]),
        ),
        (
            "coverage",
            object(vec![("completeness", string("complete"))]),
        ),
        ("retention_class", string(RETENTION_CLASS)),
    ])
}

#[cfg(test)]
mod tests;
