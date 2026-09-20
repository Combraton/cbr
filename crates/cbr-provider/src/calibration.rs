//! The calibration: the first live calls, and the only ones before m4e.
//!
//! **It is not run by this milestone.** It is built here so that the first
//! live call runs code that has been reviewed, rather than plumbing written
//! on the day it is needed. It runs only when the owner passes both
//! `--calibrate` and `--permit-model-network`, and only with a run ceiling
//! set, which the ledger enforces.
//!
//! # What it is for
//!
//! The local estimate is sound by construction — a byte-level BPE emits at
//! most one token per byte — but **byte-level is an assumption about the
//! provider's tokenizer, not a fact CBR has checked**
//! ([READINESS §10](../../docs/work/m4/READINESS.md)). This is the only
//! thing that can check it.
//!
//! # What ends it
//!
//! **One provider count above its local estimate means the bound is
//! unsound.** Then: stop, report, and nothing else in M4 proceeds until it
//! is fixed. Not "note it and widen the margin" — the whole admission
//! design rests on the estimate never falling below the truth, and one
//! counter-example says it does.
//!
//! # Reported, not believed
//!
//! The table carries the provider's **reported** figure rather than the one
//! the call path goes on to believe. For the *stop condition* the two are
//! equivalent — a count above the local bound is necessarily above the
//! implausibility floor, so the disbelief rule never fires on the case
//! being looked for — and that mutant is therefore equivalent and not
//! counted. For the *table* they are not: a count below the floor is
//! replaced by the local estimate for the call path's purposes, and
//! recording that would hide exactly how loose the bound is, which is the
//! other thing this run is for.

use crate::budget::Ledger;
use crate::model::{Ask, Attempt, Outcome, Runtime, Transport, no_barrier};
use crate::wire::Dialect;
use crate::wire::request::{Message, Request, Role, Want};

/// The hard ceiling for the run, from [READINESS §10]. Enforced through the
/// ledger's run-level ceiling rather than by intention.
///
/// [READINESS §10]: ../../docs/work/m4/READINESS.md
pub const CEILING: u64 = 100_000;

/// The completion's generation limit. Small enough that the completion path
/// is exercised once end to end and cannot cost much even if everything
/// else is wrong.
pub const GENERATION: u64 = 16;

/// A non-Latin text, the case a character-count bound gets wrong.
const NON_LATIN: &str = "\
एक प्रदाता की प्रतिक्रिया में क्रेडेंशियल हो सकते हैं, इसलिए रिकॉर्डिंग की सीमा पर ही उन्हें हटाया जाता है।
プロバイダーの応答には第三者の資格情報が含まれることがあるため、記録の境界で削除する。
提供方的响应中可能包含第三方凭据，因此在记录边界处将其删除。";

/// The fixed corpus: **this repository's own public sources**, so nothing
/// here sends anyone else's text, plus the text above.
///
/// Compiled in rather than read from disk, so the run does not depend on
/// the binary being started inside a checkout, and so two runs compare the
/// same bytes.
pub fn corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("clock.rs", include_str!("clock.rs")),
        ("keychain.rs", include_str!("keychain.rs")),
        ("wire/endpoint.rs", include_str!("wire/endpoint.rs")),
        ("wire/json.rs", include_str!("wire/json.rs")),
        ("wire/redact.rs", include_str!("wire/redact.rs")),
        ("non-latin", NON_LATIN),
    ]
}

/// One file's row of the table.
pub struct Row {
    pub name: String,
    /// CBR's own conservative bound.
    pub local: u64,
    /// What the provider said, when it said anything.
    pub provider: Option<u64>,
}

/// What the one completion cost, and what it took.
pub struct Completion {
    /// What the provider said it cost, when it said anything.
    pub usage: Option<u64>,
    /// How many repairs it needed. Recorded, because a completion that
    /// needed repairing is a fact about the request rather than noise.
    pub repairs: u32,
    /// The first line of what came back, so the table shows the run reached
    /// a model rather than merely reached the end.
    pub answer: String,
}

pub struct Report {
    pub rows: Vec<Row>,
    /// The one completion, if it ran.
    pub completion: Option<Completion>,
    /// Why the run ended early, if it did.
    pub stopped: Option<String>,
}

impl Report {
    // Read by the tests and by the report the launch prints; the table
    // below says the same thing to whoever reads the file.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn completed(&self) -> bool {
        self.completion.is_some()
    }
}

/// One request carrying `text`, framed for `dialect`.
fn asking(model: &str, text: &str) -> Request {
    Request {
        model: model.to_string(),
        system: Some("Answer in one short sentence.".into()),
        messages: vec![Message {
            role: Role::User,
            text: text.to_string(),
        }],
        generation: GENERATION,
        want: Want::Text,
    }
}

/// Run the protocol of READINESS section 10, exactly.
pub fn run(
    now: &str,
    ledger: Ledger<'_>,
    transport: &dyn Transport,
    dialect: Dialect,
    model: &str,
) -> Report {
    let runtime = Runtime { ledger, transport };
    let mut rows = Vec::new();
    for (name, text) in corpus() {
        let request = asking(model, text);
        let body = request.serialize(dialect);
        let counted = runtime.count(
            now,
            &Attempt {
                job: "calibration",
                request: name,
                body: &body,
                messages: request.framed_messages(dialect),
                generation: GENERATION,
                dialect,
            },
            &no_barrier,
        );
        let counted = match counted {
            Ok(counted) => counted,
            Err(ended) => {
                return Report {
                    rows,
                    completion: None,
                    stopped: Some(format!(
                        "{name}: the count did not happen: {}",
                        ended.reason().unwrap_or("unknown")
                    )),
                };
            }
        };
        rows.push(Row {
            name: name.to_string(),
            local: counted.local,
            provider: counted.reported,
        });
        // **The stop condition.** Checked against what the provider
        // reported, before anything else is sent.
        if let Some(reported) = counted.reported
            && reported > counted.local
        {
            return Report {
                rows,
                completion: None,
                stopped: Some(format!(
                    "{name}: the provider counted {reported}, which is above its local \
                     estimate of {}. The byte bound is unsound and nothing else in M4 \
                     proceeds until it is fixed.",
                    counted.local
                )),
            };
        }
    }
    // One completion, capped at sixteen generated tokens, **through the
    // same entry point the rest of M4 will use** rather than a path built
    // for this run: admission, count, completion, parse, and the bounded
    // repair if the provider answers with something unusable.
    let request = asking(model, "Reply with the single word: calibrated.");
    let outcome = runtime.ask(
        now,
        &Ask {
            job: "calibration",
            request: "completion",
            dialect,
            body: &request,
        },
        &no_barrier,
    );
    match outcome {
        Outcome::Answered {
            reply,
            usage,
            repairs,
        } => Report {
            rows,
            completion: Some(Completion {
                usage: Some(usage),
                repairs,
                answer: first_line(&reply),
            }),
            stopped: None,
        },
        Outcome::Unmet { reason, repairs } => Report {
            rows,
            completion: None,
            stopped: Some(format!(
                "the completion did not happen: {reason} (after {repairs} repair(s))"
            )),
        },
        Outcome::Refused(refusal) => Report {
            rows,
            completion: None,
            stopped: Some(format!("the completion was refused: {}", refusal.reason())),
        },
    }
}

/// The first line of an answer, bounded, for the table.
fn first_line(reply: &crate::wire::response::Reply) -> String {
    let text = match reply {
        crate::wire::response::Reply::Text(text) => text.clone(),
        other => format!("{other:?}"),
    };
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect()
}

impl Report {
    /// The table READINESS section 10 asks for: the local estimate against
    /// the provider's count, per file.
    pub fn table(&self) -> String {
        let mut out = String::new();
        out.push_str("# Calibration: the local estimate against the provider's count\n\n");
        out.push_str(
            "Written by `cbr-provider --calibrate`. Every figure is a token count.\n\
             `ratio` is local / provider: how loose the bound was for that file.\n\n",
        );
        out.push_str("| file | local | provider | ratio |\n|---|---:|---:|---:|\n");
        for row in &self.rows {
            let provider = row
                .provider
                .map_or_else(|| "-".to_string(), |count| count.to_string());
            let ratio = match row.provider {
                Some(count) if count > 0 => format!("{:.2}", row.local as f64 / count as f64),
                _ => "-".to_string(),
            };
            out.push_str(&format!(
                "| {} | {} | {provider} | {ratio} |\n",
                row.name, row.local
            ));
        }
        out.push('\n');
        match (&self.stopped, &self.completion) {
            (Some(reason), _) => out.push_str(&format!("**STOPPED.** {reason}\n")),
            (None, Some(completion)) => {
                out.push_str(
                    "Every count was at or below its local estimate, and the completion ran.\n\n",
                );
                out.push_str("| completion | usage | repairs | answer |\n|---|---:|---:|---|\n");
                out.push_str(&format!(
                    "| 16-token | {} | {} | {} |\n",
                    completion
                        .usage
                        .map_or_else(|| "-".to_string(), |usage| usage.to_string()),
                    completion.repairs,
                    completion.answer
                ));
            }
            (None, None) => out.push_str("The counts finished; the completion did not run.\n"),
        }
        out
    }

    /// What the whole run reserved, for the report beside the table.
    pub fn local_total(&self) -> u64 {
        self.rows.iter().map(|row| row.local).sum()
    }
}

#[cfg(test)]
mod tests;
