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
pub const PRODUCER: &str = "cbr-model-runtime/1";

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
    "index_unavailable",
    "job_over_ceiling",
    "model_answer_malformed",
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
    "run_over_ceiling",
    "store_unavailable",
    "work_panicked",
];

/// A record naming a reason this build cannot read is its own failure,
/// with its own name, rather than a reason guessed at.
pub const UNREADABLE: &str = "model_record_unreadable";
/// Nothing retained answers this question.
pub const NOT_RETAINED: &str = "model_answer_not_retained";

/// The admission a call that never reached one had: CBR's own envelope
/// refused it, so nothing was counted and nothing was sent.
pub const NOT_ADMITTED: &str = "refused";

/// The typed reason `text` names, when this build knows it.
pub fn reason(text: &str) -> Option<&'static str> {
    REASONS.iter().copied().find(|known| *known == text)
}

/// One candidate as the record remembers it: where it was, and the digest
/// of the text that was shown — never the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub id: String,
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
    Unmet(&'static str),
}

/// What the call cost, whatever it came to.
#[derive(Debug, Clone, Copy)]
pub struct Spend {
    /// `admitted_local` or `admitted_count`: whether the local bound
    /// alone admitted it, or the provider's count was asked for first.
    pub admission: &'static str,
    pub usage: Option<u64>,
    pub input_usage: Option<u64>,
    pub counted: Option<u64>,
    pub repairs: u32,
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

/// The sealed bytes of a derivation, as a value.
pub fn record(made: &Made<'_>) -> Value {
    let mut question = made.question.to_value();
    crate::context::set(&mut question, "digest", string(&made.question.digest()));
    object(vec![
        ("format", string("cbr-model-derivation/1")),
        ("model", string(made.question.model)),
        ("admission", string(made.spend.admission)),
        (
            "usage",
            object(vec![
                ("tokens", tokens(made.spend.usage)),
                ("input_tokens", tokens(made.spend.input_usage)),
                ("counted_tokens", tokens(made.spend.counted)),
                ("repairs", Value::Int(made.spend.repairs as i64)),
            ]),
        ),
        ("latency_ms", Value::Int(made.spend.latency_ms as i64)),
        ("question", question),
        (
            "answer",
            match &made.answer {
                Answer::Chose(id) => object(vec![("chose", string(id))]),
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
    let unmet = answer.get("unmet").and_then(Value::as_str)?;
    Some(Answer::Unmet(reason(unmet)?))
}

/// What a reader must be able to read before this derivation is served to
/// them: the job's own view and readable claims.
///
/// **Kept on the artifact record rather than in the sealed bytes.** Who
/// may read a thing is a fact about this store, and the sealed bytes are
/// the derivation itself — the same bytes, from the same question, would
/// be sealed under a different view in a different store.
pub fn readable_under(view: &[String], claims: &[String]) -> Value {
    object(vec![
        (
            "view",
            Value::Array(view.iter().map(|id| string(id)).collect()),
        ),
        (
            "readable_claims",
            Value::Array(claims.iter().map(|id| string(id)).collect()),
        ),
    ])
}

/// Whether a reader who can read exactly `view` and `claims` may be
/// served a derivation made under `under`.
///
/// The rule is **no wider than the reader**, not equal to them: an
/// authority reads everything, and a reader whose grant has since been
/// widened is not locked out of a derivation made under a narrower one.
pub fn covers(under: &Value, view: &[String], claims: &[String]) -> bool {
    let within = |path: &str, readable: &[String]| {
        list(under, &[path])
            .iter()
            .filter_map(Value::as_str)
            .all(|needed| readable.iter().any(|have| have == needed))
    };
    within("view", view) && within("readable_claims", claims)
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
