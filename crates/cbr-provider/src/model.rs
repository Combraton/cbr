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
use crate::wire::{self, Dialect};

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
    /// A completion, with the provider's accounting of what it cost —
    /// **when it gave one.** `None` is a successful call the provider did
    /// not price, which settles to the reservation's estimate rather than
    /// to zero: losing a spend against a shared quota is the one direction
    /// that cannot be corrected later.
    Completed { body: Vec<u8>, usage: Option<u64> },
    /// The provider says **its** quota is exhausted. Distinct from CBR's own
    /// envelope: CBR sees only its own spending and the quota is shared.
    ProviderExhausted,
    /// The call failed **after the body went out**. `usage` is what the
    /// provider said it charged, when it said anything at all.
    Failed { reason: String, usage: Option<u64> },
    /// The call failed **before anything left the process**.
    NotSent(String),
}

/// What one send produced: the typed answer, and **the exact bytes the
/// provider sent back**.
///
/// The raw bytes travel with the answer because the record is the exchange
/// rather than CBR's reading of it, and because the recording boundary sits
/// between the transport and the store and has nothing else to redact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    pub answer: Answer,
    pub raw: Vec<u8>,
}

/// The one thing that could open a socket.
pub trait Transport {
    fn send(&self, call: Call, body: &[u8]) -> Exchange;
}

/// The fake. It records the exact bytes it was asked to send, counts a count
/// call as a send, and returns what it was scripted to return.
#[derive(Debug, Default)]
pub struct Recorder {
    sent: Mutex<Vec<(Call, Vec<u8>)>>,
    answers: Mutex<Vec<(Answer, Vec<u8>)>>,
}

impl Recorder {
    /// `answers` are returned in order; when they run out the call is
    /// answered as a count of the body's byte length, which is what a
    /// provider that agreed exactly with the local bound would say.
    pub fn new(answers: Vec<Answer>) -> Self {
        Recorder::answering_with_bytes(answers.into_iter().map(|a| (a, Vec::new())).collect())
    }

    /// Scripted answers **with the bytes they came in**, for the tests that
    /// care what reached the recording boundary rather than what CBR made
    /// of it.
    pub fn answering_with_bytes(answers: Vec<(Answer, Vec<u8>)>) -> Self {
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
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        self.sent
            .lock()
            .expect("not poisoned")
            .push((call, body.to_vec()));
        let mut answers = self.answers.lock().expect("not poisoned");
        if answers.is_empty() {
            let answer = match call {
                Call::Count => Answer::Counted(body.len() as u64),
                Call::Completion => Answer::Completed {
                    body: Vec::new(),
                    usage: Some(body.len() as u64),
                },
            };
            return Exchange {
                answer,
                raw: Vec::new(),
            };
        }
        let (answer, raw) = answers.remove(0);
        Exchange { answer, raw }
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
    /// Which dialect framed `body`, and therefore which member the limit
    /// has to be bound to for this request to be allowed out.
    pub dialect: Dialect,
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
            dialect,
        } = *attempt;
        // **The reservation covers a generation the request actually asks
        // for.** A body that does not bind its limit could spend more than
        // was reserved, so it does not leave the process. m4a asked only
        // whether the figure appeared in the body, which a body capped at
        // 4,096 that mentions 16 in prose satisfies; this parses the body
        // and reads the member the dialect's provider reads.
        if !wire::request::declares_generation(dialect, body, generation) {
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
        let counted = self.transport.send(Call::Count, body).answer;
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
        let answer = self.transport.send(Call::Completion, body).answer;
        barrier(COMPLETION_AFTER_SEND);
        match answer {
            Answer::Completed { body, usage } => {
                self.finish(
                    now,
                    &reservation,
                    settlement_for(usage),
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                );
                // An unpriced completion is reported at what it was
                // reserved for, so the caller is told what it is being
                // charged rather than told it was free.
                Ended::Completed {
                    body,
                    usage: usage.unwrap_or(reservation.estimate),
                }
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

/// How many times a repairable outcome may be asked about again.
///
/// **One.** The two outcomes that are repairable are the provider's own
/// documented behaviour, so a second ask is worth its tokens; a third is a
/// loop, and an unbounded loop against a shared quota is the overspend the
/// envelope exists to prevent, arriving one polite retry at a time.
// Reached by the tests, and by m4c's call site: this milestone builds the
// wire and gives it no consumer, exactly as the fake transport had none
// before it. The allowance goes when m4c connects selection to it.
#[allow(dead_code)]
pub const REPAIRS: u32 = 1;

/// One question for a model, with the bounded repair that goes with it.
#[allow(dead_code)]
pub struct Ask<'a> {
    pub job: &'a str,
    pub request: &'a str,
    pub dialect: Dialect,
    pub body: &'a wire::request::Request,
}

/// How a question ended.
#[allow(dead_code)]
#[derive(Debug)]
pub enum Outcome {
    Answered {
        reply: wire::response::Reply,
        usage: u64,
        /// How many repairs it took. Recorded, because a selection that
        /// needed repairing is a fact about the request.
        repairs: u32,
    },
    /// A typed reason, never retried past the bound above.
    Unmet { reason: &'static str, repairs: u32 },
    /// Refused by CBR's own envelope, before anything was sent.
    Refused(Refusal),
}

#[allow(dead_code)]
impl Runtime<'_> {
    /// Serialize, send, read, and — for the two outcomes this provider's
    /// own behaviour produces — ask once more.
    ///
    /// **A repair is a whole call**: its own admission, its own count, its
    /// own reservation, debited from the same ledger. There is no separate
    /// repair allowance, so a repair the envelope has no room for does not
    /// happen.
    pub fn ask(&self, now: &str, ask: &Ask<'_>, barrier: &dyn Fn(&'static str)) -> Outcome {
        let mut body = ask.body.clone();
        let mut repairs = 0;
        loop {
            let serialized = body.serialize(ask.dialect);
            let ended = self.call(
                now,
                &Attempt {
                    job: ask.job,
                    request: ask.request,
                    body: &serialized,
                    messages: body.framed_messages(ask.dialect),
                    generation: body.generation,
                    dialect: ask.dialect,
                },
                barrier,
            );
            let (answered, usage) = match ended {
                Ended::Completed { body, usage } => (body, usage),
                Ended::Refused(refusal) => return Outcome::Refused(refusal),
                Ended::Unmet(reason) => return Outcome::Unmet { reason, repairs },
            };
            let read = wire::response::read_completion(ask.dialect, &body.want, &answered);
            let unusable = match read.reply {
                Ok(reply) => {
                    return Outcome::Answered {
                        reply,
                        usage: read.usage.unwrap_or(usage),
                        repairs,
                    };
                }
                Err(unusable) => unusable,
            };
            if !unusable.repairable() || repairs >= REPAIRS {
                return Outcome::Unmet {
                    reason: unusable.reason(),
                    repairs,
                };
            }
            // **The model's own answer is not sent back.** Repository text
            // is untrusted and so is what a model made of it; echoing it
            // into the next request gives text that arrived from a
            // repository a second chance to be read as an instruction, and
            // charges for the privilege. The repair is CBR's own sentence.
            body.messages.push(wire::request::Message {
                role: wire::request::Role::User,
                text: wire::request::repair_instruction(&body.want).to_string(),
            });
            repairs += 1;
        }
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

/// No barrier at all, which is every path but the crash matrix.
#[allow(dead_code)]
pub fn no_barrier(_: &'static str) {}

#[cfg(test)]
mod tests;
