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
    /// Anything else.
    Failed(String),
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
    pub fn call(
        &self,
        now: &str,
        job: &str,
        request: &str,
        body: &[u8],
        messages: usize,
        barrier: &dyn Fn(&'static str),
    ) -> Ended {
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
        barrier(AFTER_RESERVATION);
        let counted = self.transport.send(Call::Count, body);
        barrier(AFTER_SEND);
        let refined = match counted {
            Answer::Counted(tokens) => {
                self.finish(
                    now,
                    &counting,
                    Settlement::Usage(tokens.min(local)),
                    barrier,
                );
                tokens
            }
            Answer::ProviderExhausted => {
                self.finish(now, &counting, Settlement::ProviderExhausted, barrier);
                return Ended::Unmet(Settlement::ProviderExhausted.reason().unwrap_or("unknown"));
            }
            _ => {
                self.finish(now, &counting, Settlement::Failed, barrier);
                return Ended::Unmet(Settlement::Failed.reason().unwrap_or("unknown"));
            }
        };

        // Step 3: the completion, against the refined figure, but never
        // below the local bound's own reservation semantics: a provider that
        // reports less than it charges does not widen the envelope.
        let reservation = match self.ledger.admit(now, job, request, refined.max(1)) {
            Ok(Ok(reservation)) => reservation,
            Ok(Err(refusal)) => return Ended::Refused(refusal),
            Err(_) => return Ended::Unmet("ledger_unavailable"),
        };
        barrier(AFTER_RESERVATION);
        let answer = self.transport.send(Call::Completion, body);
        barrier(AFTER_SEND);
        match answer {
            Answer::Completed { body, usage } => {
                self.finish(now, &reservation, Settlement::Usage(usage), barrier);
                Ended::Completed { body, usage }
            }
            Answer::ProviderExhausted => {
                self.finish(now, &reservation, Settlement::ProviderExhausted, barrier);
                Ended::Unmet(Settlement::ProviderExhausted.reason().unwrap_or("unknown"))
            }
            _ => {
                self.finish(now, &reservation, Settlement::Failed, barrier);
                Ended::Unmet(Settlement::Failed.reason().unwrap_or("unknown"))
            }
        }
    }

    fn finish(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
        barrier: &dyn Fn(&'static str),
    ) {
        barrier(DURING_RECONCILIATION);
        let _ = self.ledger.settle(now, reservation, settlement);
    }
}

/// The reservation is written and nothing has been sent.
pub const AFTER_RESERVATION: &str = "model.after_reservation";
/// The send happened and the reservation still holds the estimate.
pub const AFTER_SEND: &str = "model.after_send";
/// Inside the reconciliation that replaces the estimate with the usage.
pub const DURING_RECONCILIATION: &str = "model.during_reconciliation";

/// No barrier at all, which is every path but the crash matrix.
#[allow(dead_code)]
pub fn no_barrier(_: &'static str) {}

#[cfg(test)]
mod tests;
