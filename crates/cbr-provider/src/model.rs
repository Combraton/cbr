//! The call path, with no transport in it.
//!
//! M4a builds the half of a model call that decides whether it may happen:
//! the local estimate, the ledger, the two-step admission, the typed
//! outcomes. What it does **not** build is the half that opens a socket —
//! there is no HTTP, no TLS and no credential anywhere in this crate, and
//! [`budget::tests`] asserts that against the workspace lock file.
//!
//! So a [`Transport`] is a trait with exactly one implementation, [`Recorder`],
//! which records what it was asked to send and returns what a test told it to.
//! It is reachable only under the `model.fake` test control, which a
//! production launch configuration refuses like every other control.
//!
//! # The count call is a send
//!
//! `POST /v1/responses/input_tokens` carries the fully serialized request to
//! the provider. [`Runtime::call`] therefore treats it as one: it is admitted
//! against the ledger, it is recorded, and [`Recorder`] counts it among the
//! sends it saw. The local estimate runs **before** it and can refuse alone,
//! which is the whole reason the admission path is complete without a network.

use std::sync::Mutex;

use crate::budget::{self, Ledger, Refusal, Reservation, Settlement};

/// Which of the two calls a send is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    /// `POST /v1/responses/input_tokens`. A send like any other.
    Count,
    /// The model call itself.
    Completion,
}

/// What a transport gave back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The provider's own token count, from a [`Call::Count`].
    Counted(u64),
    /// A completion, with the provider's accounting of what it cost.
    Completed { body: Vec<u8>, usage: u64 },
    /// The provider says **its** quota is exhausted. Distinct from CBR's own
    /// envelope: CBR sees only its own spending and the quota is shared.
    ProviderExhausted,
    /// The call failed **after the body went out**. `usage` is what the
    /// provider said it charged, when it said anything at all.
    Failed { reason: String, usage: Option<u64> },
    /// The call failed **before anything left the process**.
    NotSent(String),
}

/// The one thing that could open a socket, and does not.
pub trait Transport {
    fn send(&self, call: Call, body: &[u8]) -> Answer;
}

/// The fake. It records the exact bytes it was asked to send, counts a count
/// call as a send, and returns what it was scripted to return.
#[derive(Debug, Default)]
pub struct Recorder {
    sent: Mutex<Vec<(Call, Vec<u8>)>>,
    answers: Mutex<Vec<Answer>>,
}

impl Recorder {
    /// `answers` are returned in order; when they run out the call is
    /// answered as a count of the body's byte length, which is what a
    /// provider that agreed exactly with the local bound would say.
    pub fn new(answers: Vec<Answer>) -> Self {
        Recorder {
            sent: Mutex::new(Vec::new()),
            answers: Mutex::new(answers),
        }
    }

    /// Everything it was asked to send, in order, exactly as serialized.
    // The assertion surface: every test that says "nothing was sent" or
    // "these exact bytes were" reads this. Nothing in a serving path does,
    // because nothing in a serving path calls a model yet.
    #[allow(dead_code)]
    pub fn sent(&self) -> Vec<(Call, Vec<u8>)> {
        self.sent.lock().expect("not poisoned").clone()
    }
}

impl Transport for Recorder {
    fn send(&self, call: Call, body: &[u8]) -> Answer {
        self.sent
            .lock()
            .expect("not poisoned")
            .push((call, body.to_vec()));
        let mut answers = self.answers.lock().expect("not poisoned");
        if answers.is_empty() {
            return match call {
                Call::Count => Answer::Counted(body.len() as u64),
                Call::Completion => Answer::Completed {
                    body: Vec::new(),
                    usage: body.len() as u64,
                },
            };
        }
        answers.remove(0)
    }
}

/// How a call ended, as a caller sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    Completed {
        body: Vec<u8>,
        usage: u64,
    },
    /// Refused by CBR's own envelope or a ceiling, before anything was sent.
    Refused(Refusal),
    /// The provider refused, or the call failed. Both carry the reason a
    /// context item reports.
    Unmet(&'static str),
}

impl Ended {
    /// The reason a context item carries when a call did not complete.
    #[allow(dead_code)]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Ended::Completed { .. } => None,
            Ended::Refused(refusal) => Some(refusal.reason()),
            Ended::Unmet(reason) => Some(reason),
        }
    }
}

/// One attempt at a model call: the body, and everything about it the
/// envelope has to know.
///
/// A struct rather than a parameter list because **`body` and `generation`
/// are not independent**: the reservation covers the generation, and the
/// body has to declare it, so the two travel together or one of them gets
/// forgotten. That is the shape of the defect this replaced.
pub struct Attempt<'a> {
    pub job: &'a str,
    pub request: &'a str,
    pub body: &'a [u8],
    pub messages: usize,
    /// The generation limit the body declares and the reservation covers.
    pub generation: u64,
}

/// One model call, from admission to settlement.
pub struct Runtime<'a> {
    pub ledger: Ledger<'a>,
    pub transport: &'a dyn Transport,
}

impl<'a> Runtime<'a> {
    /// The whole path, in the order READINESS section 3 fixes it.
    ///
    /// 1. The **local** estimate decides. It alone can refuse, and when it
    ///    does nothing leaves the process.
    /// 2. The provider count runs only for a request already admitted, and
    ///    is itself admitted, sent and settled.
    /// 3. The completion is admitted against the provider's own figure.
    ///
    /// `barrier` is called at each boundary the crash matrix kills at.
    pub fn call(&self, now: &str, attempt: &Attempt<'_>, barrier: &dyn Fn(&'static str)) -> Ended {
        let Attempt {
            job,
            request,
            body,
            messages,
            generation,
        } = *attempt;
        // **The reservation covers a generation the request actually asks
        // for.** A body that does not declare its limit could spend more
        // than was reserved, so it does not leave the process. This checks
        // that the figure is in the body, not that it is bound to the right
        // field — m4b's serializer owns that, and this is what stops m4b
        // building one that forgets.
        if !declares_generation(body, generation) {
            return Ended::Unmet("generation_limit_not_declared");
        }
        let local = budget::estimate(body, messages);

        // Step 1: the count call is a send, so it is admitted first.
        let counting = match self
            .ledger
            .admit(now, job, &format!("{request}.count"), local)
        {
            Ok(Ok(reservation)) => reservation,
            Ok(Err(refusal)) => return Ended::Refused(refusal),
            Err(_) => return Ended::Unmet("ledger_unavailable"),
        };
        barrier(COUNT_AFTER_RESERVATION);
        let counted = self.transport.send(Call::Count, body);
        barrier(COUNT_AFTER_SEND);
        let refined = match counted {
            Answer::Counted(tokens) => {
                // **A count far below the local bound is not a tighter
                // count.** The bound runs three to four times the real
                // figure for prose; below an eighth of it, the local figure
                // stands and the anomaly is recorded.
                let floor = budget::worst_case_tokens(body) / budget::IMPLAUSIBLE_RATIO;
                let believed = if tokens < floor {
                    let _ = self
                        .ledger
                        .note(now, job, request, "anomaly", tokens, local);
                    local
                } else {
                    tokens
                };
                self.finish(
                    now,
                    &counting,
                    Settlement::Usage(tokens.min(local)),
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                );
                believed
            }
            Answer::ProviderExhausted => {
                self.finish(
                    now,
                    &counting,
                    Settlement::ProviderExhausted,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                );
                return Ended::Unmet(Settlement::ProviderExhausted.reason().unwrap_or("unknown"));
            }
            Answer::NotSent(_) => {
                self.finish(
                    now,
                    &counting,
                    Settlement::NothingSpent,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                );
                return Ended::Unmet(Settlement::NothingSpent.reason().unwrap_or("unknown"));
            }
            Answer::Failed { usage, .. } => {
                self.finish(
                    now,
                    &counting,
                    settlement_for(usage),
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                );
                return Ended::Unmet(Settlement::UsageUnknown.reason().unwrap_or("unknown"));
            }
            // A completion answered to a count is not a transport error, it
            // is a transport that does not do what it says. Silently taking
            // the failure arm for it is how the crash matrix came to believe
            // it had reached a boundary it never did.
            Answer::Completed { .. } => {
                let _ = self.ledger.note(now, job, request, "mismatch", 0, local);
                self.finish(
                    now,
                    &counting,
                    Settlement::UsageUnknown,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                );
                return Ended::Unmet("model_answer_mismatched");
            }
        };

        // Step 3: the completion, reserved for **everything it can cost** —
        // the refined input count, the generation the request asked for, and
        // the margin. Reserving the input count alone drops the two parts
        // that are not in it and trusts the provider's figure to bound a
        // cost the provider has not incurred yet.
        let wanted = refined
            .saturating_add(generation)
            .saturating_add(budget::SAFETY_MARGIN_TOKENS);
        let reservation = match self.ledger.admit(now, job, request, wanted) {
            Ok(Ok(reservation)) => reservation,
            Ok(Err(refusal)) => return Ended::Refused(refusal),
            Err(_) => return Ended::Unmet("ledger_unavailable"),
        };
        barrier(COMPLETION_AFTER_RESERVATION);
        let answer = self.transport.send(Call::Completion, body);
        barrier(COMPLETION_AFTER_SEND);
        match answer {
            Answer::Completed { body, usage } => {
                self.finish(
                    now,
                    &reservation,
                    Settlement::Usage(usage),
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                Ended::Completed { body, usage }
            }
            Answer::ProviderExhausted => {
                self.finish(
                    now,
                    &reservation,
                    Settlement::ProviderExhausted,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                Ended::Unmet(Settlement::ProviderExhausted.reason().unwrap_or("unknown"))
            }
            Answer::NotSent(_) => {
                self.finish(
                    now,
                    &reservation,
                    Settlement::NothingSpent,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                Ended::Unmet(Settlement::NothingSpent.reason().unwrap_or("unknown"))
            }
            Answer::Failed { usage, .. } => {
                self.finish(
                    now,
                    &reservation,
                    settlement_for(usage),
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                Ended::Unmet(Settlement::UsageUnknown.reason().unwrap_or("unknown"))
            }
            Answer::Counted(_) => {
                let _ = self.ledger.note(now, job, request, "mismatch", 0, wanted);
                self.finish(
                    now,
                    &reservation,
                    Settlement::UsageUnknown,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                Ended::Unmet("model_answer_mismatched")
            }
        }
    }

    fn finish(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
        barrier: &dyn Fn(&'static str),
        boundary: &'static str,
    ) {
        barrier(boundary);
        let _ = self.ledger.settle(now, reservation, settlement);
    }
}

/// Per call, so a crash-matrix row cannot pass at the wrong boundary.
pub const COUNT_AFTER_RESERVATION: &str = "model.count.after_reservation";
pub const COUNT_AFTER_SEND: &str = "model.count.after_send";
pub const COUNT_DURING_RECONCILIATION: &str = "model.count.during_reconciliation";
pub const COMPLETION_AFTER_RESERVATION: &str = "model.completion.after_reservation";
pub const COMPLETION_AFTER_SEND: &str = "model.completion.after_send";
pub const COMPLETION_DURING_RECONCILIATION: &str = "model.completion.during_reconciliation";

/// A failure after the send, settled by whether the provider said anything
/// about what it charged.
fn settlement_for(usage: Option<u64>) -> Settlement {
    match usage {
        Some(usage) => Settlement::Usage(usage),
        None => Settlement::UsageUnknown,
    }
}

/// Whether `body` declares the generation limit it was reserved for.
///
/// A textual check, and deliberately a narrow one: it says the figure is in
/// the body, not that it is bound to the field the provider reads. m4b's
/// serializer owns that; this exists so m4b cannot build a body that leaves
/// the limit out altogether, which would let a completion spend more than
/// its reservation covers.
fn declares_generation(body: &[u8], generation: u64) -> bool {
    let body = String::from_utf8_lossy(body);
    let wanted = generation.to_string();
    body.match_indices(&wanted).any(|(at, _)| {
        let before = body[..at].chars().next_back();
        let after = body[at + wanted.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// No barrier at all, which is every path but the crash matrix.
#[allow(dead_code)]
pub fn no_barrier(_: &'static str) {}

#[cfg(test)]
mod tests;
