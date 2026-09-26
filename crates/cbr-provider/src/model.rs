//! The call path, with no transport in it.
//!
//! M4a builds the half of a model call that decides whether it may happen:
//! the local estimate, the ledger, the two-step admission, the typed
//! outcomes. What it does **not** build is the half that opens a socket —
//! there is no HTTP, no TLS and no credential anywhere in this crate, and
//! [`budget::tests`] asserts that against the workspace lock file.
//!
//! So at m4a a [`Transport`] was a trait with exactly one
//! implementation, `Recorder`, which records what it was asked to send and
//! returns what a test told it to. m4b added the live one and m4c the
//! serving call site, and this module still has no HTTP in it: what it
//! knows is admission, settlement and the typed outcomes.
//!
//! # The count call is a send
//!
//! `POST /v1/responses/input_tokens` carries the fully serialized request to
//! the provider. [`Runtime::call`] therefore treats it as one: it is admitted
//! against the ledger, it is recorded, and [`Recorder`] counts it among the
//! sends it saw. The local estimate runs **before** it and can refuse alone,
//! which is the whole reason the admission path is complete without a network.
//!
//! # No attempt reserves more than its send (m5-settle)
//!
//! A count reserves the completion body's input bound, where its settlement
//! is capped, and a completion admitted on a count is refused when the
//! count's charge and its own reservation would pass the send the local
//! bound refused. A provider billing above a reservation is caught, not
//! prevented: the bill is charged whole, the call ends
//! `reservation_overrun`, and the ledger admits nothing again.

use std::cell::Cell;
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

/// The fake a **unit test** scripts: it records the exact bytes it was
/// asked to send, counts a count call as a send, and returns the exact
/// answers it was given, in order, whichever call asks.
///
/// Test-only since m4c, and that is the whole story of this milestone in
/// one type. Until then it was the only thing in the build that reached a
/// transport, because there was no other; now [`Fake`] serves the call
/// site and [`crate::wire::http::Http`] serves a live one, and this is
/// what is left: the transport a test scripts an exact exchange with.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct Recorder {
    sent: Mutex<Vec<(Call, Vec<u8>)>>,
    answers: Mutex<Vec<(Answer, Vec<u8>)>>,
}

#[cfg(test)]
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

#[cfg(test)]
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
        /// What the counting endpoint predicted, when there was one.
        counted: Option<u64>,
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

/// When the provider's count is worth the call it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counting {
    /// **The default, and what every serving path uses.** Only when the
    /// local bound refuses on a counter a tighter figure could satisfy.
    // Constructed by m4c's serving call site; until then the only callers
    // are the calibration and the fault injector, whose purpose is the
    // count itself.
    #[cfg_attr(not(test), allow(dead_code))]
    WhenItCouldAdmit,
    /// Always, for a caller whose purpose **is** the count: the
    /// calibration compares it against what the provider then charges, and
    /// a comparison with no count is no comparison. The count it believes
    /// is capped at the local bound, so an attempt reserves at most
    /// `local + W`, which [`question_worst`] does not price; and a bill
    /// above the count is the calibration's finding, not a stop.
    Always,
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
    /// The body the **counting endpoint** is sent, when there is one.
    ///
    /// `None` for a dialect the counting endpoint does not describe: the
    /// local bound alone admits those, which is what it was built to be
    /// able to do. It is a separate body because the two differ — a count
    /// of one serialization says nothing about the cost of another, and
    /// the counting endpoint documents fewer members than the completion.
    pub count_body: Option<&'a [u8]>,
    /// The generation limit the body declares and the reservation covers.
    pub generation: u64,
    /// Which dialect framed `body`, and therefore which member the limit
    /// has to be bound to for this request to be allowed out.
    pub dialect: Dialect,
    /// Whether the provider's count is worth making for this call.
    pub counting: Counting,
}

/// One model call, from admission to settlement.
pub struct Runtime<'a> {
    pub ledger: Ledger<'a>,
    pub transport: &'a dyn Transport,
}

impl<'a> Runtime<'a> {
    /// The whole path, in the order READINESS section 3 fixes it.
    ///
    /// 1. The **local** estimate decides first. When it admits, the
    ///    completion is sent on it and no count is made. When it refuses on
    ///    the per-request ceiling, or the dialect has no counting endpoint,
    ///    the refusal stands and nothing leaves the process.
    /// 2. When it refuses on the job, the window, the month or the run —
    ///    a counter a tighter figure could satisfy — and the store has not
    ///    recorded a count unsound, the provider count is made, and is
    ///    itself admitted, sent and settled. It reserves the completion
    ///    body's input bound, the most its settlement can charge.
    /// 3. The completion is then admitted on the provider's own figure —
    ///    or on the local one again, when the count is implausibly low —
    ///    against the ledger as it stands after the count, and only when
    ///    the count's charge and the completion's reservation together fit
    ///    within the send the local bound refused (m5-settle).
    ///
    /// Under [`Counting::Always`], whose purpose is the count, the count
    /// comes first and the completion is admitted on it alone, capped at
    /// the local bound.
    ///
    /// A bill above its reservation ends its call `reservation_overrun`,
    /// and the ledger then admits nothing ([`budget`]).
    ///
    /// `barrier` is called at each boundary the crash matrix kills at.
    pub fn call(
        &self,
        now: &str,
        attempt: &Attempt<'_>,
        barrier: &dyn Fn(&'static str),
        charges: &Charges,
    ) -> Ended {
        let Attempt {
            job,
            request,
            body,
            count_body,
            messages,
            generation,
            dialect,
            counting,
        } = *attempt;
        // **The reservation covers a generation the request actually asks
        // for.** A body that does not bind its limit could spend more than
        // was reserved, so it does not leave the process.
        if !wire::request::declares_generation(dialect, body, generation) {
            return Ended::Unmet("generation_limit_not_declared");
        }
        // A caller whose purpose is the count gets it before anything is
        // admitted, because the figure is the point rather than a way of
        // being allowed to send.
        if counting == Counting::Always && count_body.is_some() {
            let counted = match self.count(now, attempt, barrier, charges) {
                Ok(counted) => counted,
                Err(ended) => return ended,
            };
            let wanted = counted
                .believed
                .saturating_add(generation)
                .saturating_add(budget::SAFETY_MARGIN_TOKENS);
            let reservation = match self.ledger.admit(now, job, request, wanted) {
                Ok(Ok(reservation)) => reservation,
                Ok(Err(refusal)) => return Ended::Refused(refusal),
                Err(_) => return Ended::Unmet("ledger_unavailable"),
            };
            let _ = self
                .ledger
                .note(now, job, request, ADMITTED_COUNT, 0, wanted);
            charges.admission.set(Some(ADMITTED_COUNT));
            barrier(COMPLETION_AFTER_RESERVATION);
            let answer = self.transport.send(Call::Completion, body).answer;
            barrier(COMPLETION_AFTER_SEND);
            // The count is the measurement here, so a bill above it is the
            // calibration's finding and closes nothing: no `admitted_on`.
            return self.settle_completion(
                now,
                attempt,
                &reservation,
                answer,
                barrier,
                wanted,
                counted.reported,
                None,
                charges,
            );
        }
        // Once the bound is known to be wrong, nothing is admitted on it.
        if self.ledger.bound_is_unsound().unwrap_or(false) {
            return Ended::Unmet(BOUND_UNSOUND_REASON);
        }
        // **The local bound decides, and usually decides alone.**
        //
        // Calibration run 2 measured what a count costs: it sends the
        // repository text a second time, the provider prices it at
        // nothing so CBR charges itself for it — 15,538 of the 15,582
        // tokens that run — and the one comparison available had it
        // over-predict the bill by 4.4 times, which is conservatism the
        // local bound already provides. So the count is made only where it
        // can change the answer, and the answer it can change is a refusal
        // on a counter with room for what the request really costs.
        //
        // **One data point is not a rule.** m4e's requests are realistic
        // sizes and will say more; until then this is the reading of one
        // measurement, and it is the reading that spends less.
        let send = budget::reservation(body, messages, generation);
        let mut admitted_on = None;
        let (reservation, admission, counted) = match self.ledger.admit(now, job, request, send) {
            Ok(Ok(reservation)) => (reservation, ADMITTED_LOCAL, None),
            Ok(Err(refusal)) => {
                // Nothing tighter exists, or nothing tighter would
                // help: the refusal stands and nothing is sent. Nor once a
                // count has been shown unsound (below), in this store.
                if count_body.is_none()
                    || !refusal.a_tighter_figure_could_admit()
                    || self.count_path_closed()
                {
                    return Ended::Refused(refusal);
                }
                let counted = match self.count(now, attempt, barrier, charges) {
                    Ok(counted) => counted,
                    Err(ended) => return ended,
                };
                // Read again: another job may have closed it while this
                // count was out.
                if self.count_path_closed() {
                    return Ended::Refused(refusal);
                }
                let wanted = counted
                    .believed
                    .saturating_add(generation)
                    .saturating_add(budget::SAFETY_MARGIN_TOKENS);
                // **The per-attempt rule.** The count's charge and the
                // completion's reservation together hold no more than the
                // send the local bound refused, however much room was freed
                // on the counter since. Without freed room the ledger
                // already forces this.
                let attempt_holds = charges.count.get().unwrap_or(0).saturating_add(wanted);
                if attempt_holds > send {
                    let _ =
                        self.ledger
                            .note(now, job, request, ATTEMPT_OVER_SEND, 0, attempt_holds);
                    return Ended::Refused(refusal);
                }
                admitted_on = Some(counted.believed);
                match self.ledger.admit(now, job, request, wanted) {
                    Ok(Ok(reservation)) => (reservation, ADMITTED_COUNT, counted.reported),
                    // The count was worth asking for and did not
                    // answer for this request. The refusal stands.
                    Ok(Err(refusal)) => return Ended::Refused(refusal),
                    Err(_) => return Ended::Unmet("ledger_unavailable"),
                }
            }
            Err(_) => return Ended::Unmet("ledger_unavailable"),
        };
        // **Which path admitted it is a fact about the call**, and a
        // derivation record carries it: a selection admitted on the local
        // bound and one admitted on the provider's count were decided by
        // different evidence.
        let _ = self
            .ledger
            .note(now, job, request, admission, 0, reservation.estimate);
        charges.admission.set(Some(admission));
        barrier(COMPLETION_AFTER_RESERVATION);
        let answer = self.transport.send(Call::Completion, body).answer;
        barrier(COMPLETION_AFTER_SEND);
        self.settle_completion(
            now,
            attempt,
            &reservation,
            answer,
            barrier,
            reservation.estimate,
            counted,
            admitted_on,
            charges,
        )
    }

    /// Whether this store has closed the serving count path. A ledger that
    /// cannot say has not said it is open, so the local refusal stands.
    fn count_path_closed(&self) -> bool {
        !matches!(self.ledger.count_is_unsound(), Ok(false))
    }

    /// **The count half alone**, which is a complete send in its own right:
    /// the local estimate decides first, then the provider count is
    /// admitted, sent and settled like anything else. It reserves the
    /// completion body's input bound and settles at no more than it, and
    /// the figure it believes is capped at that bound too.
    pub fn count(
        &self,
        now: &str,
        attempt: &Attempt<'_>,
        barrier: &dyn Fn(&'static str),
        charges: &Charges,
    ) -> Result<Counted, Ended> {
        let Attempt {
            job,
            request,
            body,
            count_body,
            messages,
            generation,
            dialect,
            ..
        } = *attempt;
        // **The reservation covers a generation the request actually asks
        // for.** A body that does not bind its limit could spend more than
        // was reserved, so it does not leave the process. m4a asked only
        // whether the figure appeared in the body, which a body capped at
        // 4,096 that mentions 16 in prose satisfies; this parses the body
        // and reads the member the dialect's provider reads.
        if !wire::request::declares_generation(dialect, body, generation) {
            return Err(Ended::Unmet("generation_limit_not_declared"));
        }
        let local = budget::input_bound(body, messages);
        // No counting endpoint for this dialect, so nothing is sent and
        // the local bound stands. Admission is complete without it, which
        // is the property the byte bound exists for.
        let Some(count_body) = count_body else {
            return Ok(Counted {
                local,
                reported: None,
                believed: local,
            });
        };
        // **A count call generates nothing**, so it reserves input and not a
        // generation it will never use: the completion body's input bound,
        // which is where its settlement is capped, so a count never settles
        // above its reservation (m5-settle). Its own body's bound omits
        // members the completion carries, and settled above it.
        let counting_costs = local;

        // Step 1: the count call is a send, so it is admitted first.
        let counting =
            match self
                .ledger
                .admit(now, job, &format!("{request}.count"), counting_costs)
            {
                Ok(Ok(reservation)) => reservation,
                Ok(Err(refusal)) => return Err(Ended::Refused(refusal)),
                Err(_) => return Err(Ended::Unmet("ledger_unavailable")),
            };
        barrier(COUNT_AFTER_RESERVATION);
        let counted = self.transport.send(Call::Count, count_body).answer;
        barrier(COUNT_AFTER_SEND);
        let counted = match counted {
            Answer::Counted(tokens) => {
                // **A count far below the local bound is not a tighter
                // count.** The bound runs three to four times the real
                // figure for prose; below an eighth of it, the local figure
                // stands and the anomaly is recorded.
                let floor = budget::worst_case_tokens(count_body) / budget::IMPLAUSIBLE_RATIO;
                let believed = if tokens < floor {
                    let _ = self
                        .ledger
                        .note(now, job, request, "anomaly", tokens, local);
                    local
                } else {
                    // Never above the local bound, so a completion
                    // admitted on it reserves no more than its send. On the
                    // serving path the per-attempt rule refuses such a
                    // completion anyway; under `Counting::Always` this is
                    // what caps it.
                    tokens.min(local)
                };
                charges.count.set(Some(self.finish(
                    now,
                    &counting,
                    Settlement::Usage(tokens.min(local)),
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                )));
                Counted {
                    local,
                    reported: Some(tokens),
                    believed,
                }
            }
            Answer::ProviderExhausted => {
                charges.count.set(Some(self.finish(
                    now,
                    &counting,
                    Settlement::ProviderExhausted,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                )));
                return Err(Ended::Unmet(
                    Settlement::ProviderExhausted.reason().unwrap_or("unknown"),
                ));
            }
            Answer::NotSent(_) => {
                charges.count.set(Some(self.finish(
                    now,
                    &counting,
                    Settlement::NothingSpent,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                )));
                return Err(Ended::Unmet(
                    Settlement::NothingSpent.reason().unwrap_or("unknown"),
                ));
            }
            Answer::Failed { usage, .. } => {
                charges.count.set(Some(self.finish(
                    now,
                    &counting,
                    settlement_for(usage),
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                )));
                if overran(usage, &counting) {
                    return Err(Ended::Unmet(OVERRUN_REASON));
                }
                return Err(Ended::Unmet(
                    Settlement::UsageUnknown.reason().unwrap_or("unknown"),
                ));
            }
            // A completion answered to a count is not a transport error, it
            // is a transport that does not do what it says. Silently taking
            // the failure arm for it is how the crash matrix came to believe
            // it had reached a boundary it never did.
            Answer::Completed { .. } => {
                let _ = self.ledger.note(now, job, request, "mismatch", 0, local);
                charges.count.set(Some(self.finish(
                    now,
                    &counting,
                    Settlement::UsageUnknown,
                    barrier,
                    COUNT_DURING_RECONCILIATION,
                )));
                return Err(Ended::Unmet("model_answer_mismatched"));
            }
        };
        Ok(counted)
    }

    /// Whether what the provider charged for the input exceeds what the
    /// local bound said it could, and if so, shut the process's model
    /// calls down.
    ///
    /// Checked at settlement because that is where the provider's own
    /// figure arrives. `input_bound` is the same function admission used,
    /// so the comparison is against the number that admitted this call.
    fn check_the_bound(
        &self,
        now: &str,
        job: &str,
        request: &str,
        body: &[u8],
        messages: usize,
        charged: Option<u64>,
    ) -> bool {
        let bound = budget::input_bound(body, messages);
        // Silence is not evidence: a provider that did not say what the
        // input cost has not said the bound is wrong.
        let Some(charged) = charged else { return false };
        if charged <= bound {
            return false;
        }
        let _ = self
            .ledger
            .note(now, job, request, BOUND_UNSOUND, charged, bound);
        true
    }

    /// What a completion's answer settles to, and what the caller is told.
    ///
    /// `admitted_on` is the count a serving completion was admitted on,
    /// when it was: input billed above it and the margin records
    /// `count_unsound`, which closes the count path. The answer stands
    /// unless the bill also passed the reservation, because the byte bound
    /// held.
    #[allow(clippy::too_many_arguments)]
    fn settle_completion(
        &self,
        now: &str,
        attempt: &Attempt<'_>,
        reservation: &Reservation,
        answer: Answer,
        barrier: &dyn Fn(&'static str),
        wanted: u64,
        counted: Option<u64>,
        admitted_on: Option<u64>,
        charges: &Charges,
    ) -> Ended {
        let (job, request) = (attempt.job, attempt.request);
        match answer {
            Answer::Completed { body, usage } => {
                charges.completion.set(Some(self.finish(
                    now,
                    reservation,
                    settlement_for(usage),
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                )));
                // **The tripwire**, read from the response itself rather
                // than from a second figure that could disagree with it.
                let charged = wire::response::accounting(attempt.dialect, &body)
                    .0
                    .and(wire::response::input_usage_of(attempt.dialect, &body));
                if self.check_the_bound(now, job, request, attempt.body, attempt.messages, charged)
                {
                    return Ended::Unmet(BOUND_UNSOUND_REASON);
                }
                if let (Some(on), Some(charged)) = (admitted_on, charged)
                    && charged > on.saturating_add(budget::SAFETY_MARGIN_TOKENS)
                {
                    let _ = self
                        .ledger
                        .note(now, job, request, COUNT_UNSOUND, charged, on);
                }
                if overran(usage, reservation) {
                    return Ended::Unmet(OVERRUN_REASON);
                }
                // An unpriced completion is reported at what it was
                // reserved for, so the caller is told what it is being
                // charged rather than told it was free.
                Ended::Completed {
                    body,
                    usage: usage.unwrap_or(reservation.estimate),
                    counted,
                }
            }
            Answer::ProviderExhausted => {
                charges.completion.set(Some(self.finish(
                    now,
                    reservation,
                    Settlement::ProviderExhausted,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                )));
                Ended::Unmet(Settlement::ProviderExhausted.reason().unwrap_or("unknown"))
            }
            Answer::NotSent(_) => {
                charges.completion.set(Some(self.finish(
                    now,
                    reservation,
                    Settlement::NothingSpent,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                )));
                Ended::Unmet(Settlement::NothingSpent.reason().unwrap_or("unknown"))
            }
            Answer::Failed { usage, .. } => {
                charges.completion.set(Some(self.finish(
                    now,
                    reservation,
                    settlement_for(usage),
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                )));
                if overran(usage, reservation) {
                    return Ended::Unmet(OVERRUN_REASON);
                }
                Ended::Unmet(Settlement::UsageUnknown.reason().unwrap_or("unknown"))
            }
            Answer::Counted(_) => {
                let _ = self.ledger.note(now, job, request, "mismatch", 0, wanted);
                charges.completion.set(Some(self.finish(
                    now,
                    reservation,
                    Settlement::UsageUnknown,
                    barrier,
                    COMPLETION_DURING_RECONCILIATION,
                )));
                Ended::Unmet("model_answer_mismatched")
            }
        }
    }

    /// Settle a reservation, and say **what the ledger now holds for it**.
    ///
    /// That figure is what a record of the call carries, so the record and
    /// the ledger cannot disagree: live run 3 found a repaired step whose
    /// sealed record carried its last exchange alone while the ledger held
    /// both.
    fn finish(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
        barrier: &dyn Fn(&'static str),
        boundary: &'static str,
    ) -> u64 {
        barrier(boundary);
        let holds = match settlement {
            Settlement::Usage(tokens) => tokens,
            // Unpriced, so the estimate stands: the conservative reading
            // of silence, and the figure already on the row.
            Settlement::UsageUnknown => reservation.estimate,
            Settlement::NothingSpent | Settlement::ProviderExhausted => 0,
        };
        match self.ledger.settle(now, reservation, settlement) {
            Ok(()) => holds,
            // A settlement that did not land leaves the reservation, which
            // counts at its estimate.
            Err(_) => reservation.estimate,
        }
    }
}

/// What the count half of a call produced.
///
/// Split out because the calibration needs the provider's **reported**
/// figure rather than the one the call path goes on to believe: the whole
/// comparison is between the local bound and what the provider said, and
/// the disbelief rule that protects the call path would hide exactly the
/// case the calibration exists to find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counted {
    /// The local, conservative bound. It alone can refuse.
    pub local: u64,
    /// What the provider said, when it said anything.
    pub reported: Option<u64>,
    /// What the call path goes on to reserve against.
    pub believed: u64,
}

/// How many times a repairable outcome may be asked about again.
///
/// **One.** The two outcomes that are repairable are the provider's own
/// documented behaviour, so a second ask is worth its tokens; a third is a
/// loop, and an unbounded loop against a shared quota is the overspend the
/// envelope exists to prevent, arriving one polite retry at a time.
pub const REPAIRS: u32 = 1;

/// The counting every serving launch uses.
///
/// **Named once, because the arithmetic rests on it.** Under it a count is
/// made only after the local bound is refused on a counter a tighter
/// figure could satisfy, and while the store has recorded no count
/// unsound, and its completion is held to the send the local bound
/// refused: that is what lets [`question_worst`] bound a serving question
/// from its body alone. `main.rs` names this rather than
/// a variant, and a test holds it to that.
pub const SERVING_COUNTING: Counting = Counting::WhenItCouldAdmit;

/// The request a repair after `unusable` sends: the same question with
/// **more room** after a truncation, and with **CBR's own sentence** after
/// any other repairable outcome.
///
/// The one place a repair is built. [`Runtime::ask`] sends it and
/// [`question_worst`] prices it, so the repair the arithmetic bounds is the
/// repair that goes out. `unusable` is one of
/// [`wire::response::REPAIRABLE`]: nothing is repaired after any other.
pub fn repaired(
    body: &wire::request::Request,
    unusable: wire::response::Unusable,
) -> wire::request::Request {
    let mut again = body.clone();
    // **A truncation is repaired with more room, not more words.** The
    // answer was not wrong; there was nowhere to put it, and on these
    // models reasoning spends the same budget. Saying it again in a smaller
    // space would waste a second call exactly as the first was wasted.
    if unusable == wire::response::Unusable::Truncated {
        again.generation = wire::request::widened(again.generation);
    } else {
        // **The model's own answer is not sent back.** Repository text is
        // untrusted and so is what a model made of it; echoing it into the
        // next request gives text that arrived from a repository a second
        // chance to be read as an instruction, and charges for the
        // privilege. The repair is CBR's own sentence.
        again.messages.push(wire::request::Message {
            role: wire::request::Role::User,
            text: wire::request::repair_instruction(&again.want).to_string(),
        });
    }
    again
}

/// **What admission reserves for one send of `body`** on the local bound:
/// [`budget::reservation`] of the body as serialized, over the messages the
/// provider frames, the instruction among them.
// Called by the arithmetic's tests, and by m5b's loop once it lands; no
// serving path needs the figure before then.
#[cfg_attr(not(test), allow(dead_code))]
pub fn send_worst(body: &wire::request::Request, dialect: Dialect) -> u64 {
    budget::reservation(
        &body.serialize(dialect),
        body.framed_messages(dialect),
        body.generation,
    )
}

/// **The most one question can hold on the serving path**: its first send
/// and the widest repair after it, each send priced by [`send_worst`] and
/// each repair built by [`repaired`], over every outcome in
/// [`wire::response::REPAIRABLE`] and [`REPAIRS`] deep.
///
/// Why no more. A question is one send and at most [`REPAIRS`] repairs. An
/// attempt admitted on the local bound holds what it reserved. Under
/// [`SERVING_COUNTING`] an attempt makes a count only after the local
/// bound is refused on a counter a tighter figure could satisfy, and its
/// completion is admitted only when the count's charge and the
/// completion's own reservation fit within the send it refused. The sweep
/// in `model::harness` measures this on the real call path rather than
/// arguing it.
///
/// **What it assumes. Only that bills are within their reservations**:
/// the byte bound holds, a completion admitted on a count is billed input
/// within the count and the margin, `max_output_tokens` is honoured, and a
/// failed call reports no more than it reserved.
///
/// **Room freed changes nothing.** A count reserves the completion body's
/// input bound, where its settlement is capped. A completion is admitted
/// on a count only when the count's charge and its own reservation fit
/// within the send the local bound refused. So an attempt reserves at most
/// `W` however room is freed.
///
/// **A bill above its reservation** is charged whole, recorded as an
/// `overrun` naming its job and request, and ends its call, and the store
/// admits nothing again. So a question can hold more than this only by
/// what a provider billed above a reservation, and only calls already
/// admitted when the first overrun settles can add to it.
///
/// **Under [`Counting::Always`]** the believed count is capped at the
/// local bound, so an attempt reserves at most `local + W`. This function
/// does not price that, because no serving launch counts that way.
// Called by the arithmetic's tests, and by m5b's loop once it lands; no
// serving path needs the figure before then.
#[cfg_attr(not(test), allow(dead_code))]
pub fn question_worst(body: &wire::request::Request, dialect: Dialect) -> u64 {
    fn worst(body: &wire::request::Request, dialect: Dialect, repairs: u32) -> u64 {
        let send = send_worst(body, dialect);
        if repairs == 0 {
            return send;
        }
        let widest = wire::response::REPAIRABLE
            .iter()
            .map(|unusable| worst(&repaired(body, *unusable), dialect, repairs - 1))
            .max()
            .unwrap_or(0);
        send.saturating_add(widest)
    }
    worst(body, dialect, REPAIRS)
}

/// **The most `price` comes to over every dialect a launch can
/// configure**, [`Dialect::ALL`]: the one place a published worst case
/// takes its maximum.
///
/// A body's bytes depend on the dialect that frames it — the chat
/// dialects carry members the Responses dialect does not, and the OpenAI
/// one the schema a structure asks for — and which dialect serves is a
/// launch's configuration. So a figure that bounds every launch is the
/// most over all of them, and a dialect added to [`Dialect::ALL`] is
/// priced here without anyone remembering to.
// Called by the arithmetic's tests; no serving path needs the figure.
#[cfg_attr(not(test), allow(dead_code))]
pub fn over_every_dialect(price: impl Fn(Dialect) -> u64) -> u64 {
    Dialect::ALL.into_iter().map(price).max().unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod harness;

/// One question for a model, with the bounded repair that goes with it.
pub struct Ask<'a> {
    pub job: &'a str,
    pub request: &'a str,
    pub dialect: Dialect,
    pub body: &'a wire::request::Request,
    pub counting: Counting,
}

/// What a question cost, **whatever it ended as**.
///
/// Carried by every outcome and not only by the answered one: an answer
/// that could not be used is still a charge against a shared quota, and a
/// completion whose whole output budget went on reasoning has a cost and
/// no text. Collapsing those into "it failed" loses the number, which is
/// what calibration run 2 found this code doing.
///
/// **Attempt by attempt, and every attempt.** A repair is a whole call with
/// its own charge, so a question's cost is the sum of its attempts, and the
/// attempt that ended it is only the last of them. Live run 3 measured the
/// difference: a choice step answered in prose and then repaired was
/// charged 5,222 and 4,827 tokens, and its record said 4,827.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cost {
    /// Every attempt, in the order it was made: the first ask and each
    /// repair, including one the envelope then refused.
    pub attempts: Vec<Attempted>,
    /// How many repairs it took. Recorded, because a selection that
    /// needed repairing is a fact about the request.
    pub repairs: u32,
}

impl Cost {
    /// **Everything the question was charged**: every attempt's completion
    /// and count call, each as the ledger settled it, which is the sum of
    /// the question's ledger rows. `None` when nothing was ever reserved.
    pub fn charged(&self) -> Option<u64> {
        self.attempts
            .iter()
            .flat_map(|attempt| [attempt.tokens, attempt.count_tokens])
            .flatten()
            .reduce(|total, tokens| total + tokens)
    }

    /// What the provider said the **input** cost, over every attempt whose
    /// completion was reserved. `None` unless each of them said, because a
    /// sum missing a term would read as a whole.
    pub fn input_charged(&self) -> Option<u64> {
        let mut reserved = self
            .attempts
            .iter()
            .filter(|attempt| attempt.tokens.is_some())
            .peekable();
        reserved.peek()?;
        reserved.map(|attempt| attempt.input_tokens).sum()
    }

    /// The attempt that ended the question.
    pub fn last(&self) -> Attempted {
        self.attempts.last().copied().unwrap_or_default()
    }

    /// Which evidence admitted it: the provider's count when any attempt
    /// was admitted on one, the local bound when any was admitted at all.
    pub fn admission(&self) -> Option<&'static str> {
        let admitted = || self.attempts.iter().filter_map(|attempt| attempt.admission);
        admitted()
            .find(|admission| *admission == ADMITTED_COUNT)
            .or_else(|| admitted().next())
    }
}

/// One attempt at a question, **as the ledger settled it**, with what the
/// provider said about it.
///
/// The charges are the settlement's rather than the response's, so a record
/// built from them agrees with the ledger by construction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attempted {
    /// Which evidence admitted the completion, when one was admitted.
    pub admission: Option<&'static str>,
    /// What the ledger settled the completion at, when one was reserved.
    pub tokens: Option<u64>,
    /// What the provider said the completion's **input** cost.
    pub input_tokens: Option<u64>,
    /// What the ledger settled a count call at, when one was made.
    pub count_tokens: Option<u64>,
    /// What the counting endpoint predicted, when it said anything.
    pub counted: Option<u64>,
}

/// What one call has been charged so far, **written where the ledger is
/// settled**.
///
/// Passed in rather than returned, because a call can end many ways after
/// something was charged — a count settled and then a refusal, a completion
/// settled and then the tripwire — and every one of them still has to say
/// what it spent.
#[derive(Debug, Default)]
pub struct Charges {
    admission: Cell<Option<&'static str>>,
    count: Cell<Option<u64>>,
    completion: Cell<Option<u64>>,
}

impl Charges {
    /// The attempt these charges belong to, before anything is read from
    /// its answer.
    fn attempted(&self) -> Attempted {
        Attempted {
            admission: self.admission.get(),
            tokens: self.completion.get(),
            count_tokens: self.count.get(),
            ..Attempted::default()
        }
    }
}

/// How a question ended.
#[derive(Debug)]
pub enum Outcome {
    Answered {
        reply: wire::response::Reply,
        cost: Cost,
    },
    /// A typed reason, never retried past the bound above — **with what it
    /// cost**, which is not nothing.
    Unmet { reason: &'static str, cost: Cost },
    /// Refused by CBR's own envelope before this attempt's completion was
    /// sent — **with what the question had already cost**, which is not
    /// nothing when the refusal came at a repair or after a count.
    Refused { refusal: Refusal, cost: Cost },
}

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
        // **Every attempt is kept, and every way out carries all of them.**
        // A question's cost is what each of its attempts was charged, so a
        // record of the question has to account for the repair it made as
        // well as the answer the repair got.
        let mut cost = Cost::default();
        loop {
            let serialized = body.serialize(ask.dialect);
            let counting = body.serialize_count(ask.dialect);
            let charges = Charges::default();
            let ended = self.call(
                now,
                &Attempt {
                    job: ask.job,
                    request: ask.request,
                    body: &serialized,
                    count_body: counting.as_deref(),
                    messages: body.framed_messages(ask.dialect),
                    generation: body.generation,
                    dialect: ask.dialect,
                    counting: ask.counting,
                },
                barrier,
                &charges,
            );
            let mut attempted = charges.attempted();
            let (answered, counted) = match ended {
                Ended::Completed { body, counted, .. } => (body, counted),
                Ended::Refused(refusal) => {
                    cost.attempts.push(attempted);
                    return Outcome::Refused { refusal, cost };
                }
                // Something may already have been charged: a count, or a
                // completion the tripwire stopped. Whatever the ledger
                // settled is in `attempted`.
                Ended::Unmet(reason) => {
                    cost.attempts.push(attempted);
                    return Outcome::Unmet { reason, cost };
                }
            };
            let read = wire::response::read_completion(ask.dialect, &body.want, &answered);
            attempted.input_tokens = read.input_usage;
            attempted.counted = counted;
            cost.attempts.push(attempted);
            let unusable = match read.reply {
                Ok(reply) => return Outcome::Answered { reply, cost },
                Err(unusable) => unusable,
            };
            if !unusable.repairable() || cost.repairs >= REPAIRS {
                return Outcome::Unmet {
                    reason: unusable.reason(),
                    cost,
                };
            }
            // **Built where the arithmetic builds it**, so the repair
            // `question_worst` prices is the repair that is sent.
            body = repaired(&body, unusable);
            cost.repairs += 1;
        }
    }
}

/// A transport that is **never called**, and says so loudly if it is.
///
/// Held by a replay launch, which answers every question from retained
/// derivation records and reaches no wire at all. Reaching this is a
/// bug in the call site rather than a configuration mistake, and the
/// panic is contained: model work runs on the pool, which turns a
/// panicking unit into the typed `work_panicked`. So the worst case is
/// a failure with a name, and never a call to a provider that an
/// offline rebuild was promised not to make.
pub struct Panicking;

/// One of them, because it has nothing in it and a pool thread needs a
/// reference that outlives the call.
pub static PANICKING: Panicking = Panicking;

impl Transport for Panicking {
    fn send(&self, _call: Call, _body: &[u8]) -> Exchange {
        panic!(
            "a replay launch reached the transport: an offline rebuild answers from retained \
             derivation records and may make no call"
        )
    }
}

/// Per call, so a crash-matrix row cannot pass at the wrong boundary.
pub const COUNT_AFTER_RESERVATION: &str = "model.count.after_reservation";
pub const COUNT_AFTER_SEND: &str = "model.count.after_send";
pub const COUNT_DURING_RECONCILIATION: &str = "model.count.during_reconciliation";
pub const COMPLETION_AFTER_RESERVATION: &str = "model.completion.after_reservation";
pub const COMPLETION_AFTER_SEND: &str = "model.completion.after_send";
pub const COMPLETION_DURING_RECONCILIATION: &str = "model.completion.during_reconciliation";

/// Ledger note kinds recording **which evidence admitted a call**. Neither
/// is a spend; both are facts a derivation record carries.
pub const ADMITTED_LOCAL: &str = "admitted_local";
pub const ADMITTED_COUNT: &str = "admitted_count";

/// The ledger kind for the one observation that invalidates every
/// admission CBR has ever made. Its own kind, not an anomaly among others.
///
/// **The calibration's stop condition, kept alive in production.**
/// [READINESS §10](../../docs/work/m4/READINESS.md) stops M4 if one
/// provider count comes in above its local estimate. That run measured six
/// files, which cannot prove a bound; this holds it. Every admission rests
/// on the byte bound never falling below the truth, so the first time a
/// provider charges more for an input than the bound said it could cost,
/// no further model call is admitted.
///
/// **Held in the ledger rather than in a process flag**, which is stronger
/// than the rule asked for: the bound is a property of the code, so a
/// restart with the same code has the same bound, and forgetting at
/// restart would forget the one observation that invalidates every
/// admission the store has ever made.
pub const BOUND_UNSOUND: &str = "bound_unsound";

/// The reason a call reports once the bound is known to be wrong.
pub const BOUND_UNSOUND_REASON: &str = "local_bound_unsound";

/// The ledger kind recording a completion admitted on a count and billed
/// input above the count and the margin. Durable: the store makes no
/// serving count again, and local admissions carry on.
pub const COUNT_UNSOUND: &str = "count_unsound";
/// The reason a call reports when its bill passed its reservation, and the
/// reason every later admission on that store is refused with.
pub const OVERRUN_REASON: &str = "reservation_overrun";
/// The ledger note recording a count-path completion refused because its
/// count and its reservation would pass the send the local bound refused.
pub const ATTEMPT_OVER_SEND: &str = "attempt_over_send";

/// Whether a provider billed above a reservation. The ledger has charged it
/// whole and recorded the `overrun`; the call ends on it, whatever the
/// answer was.
fn overran(usage: Option<u64>, reservation: &Reservation) -> bool {
    usage.is_some_and(|usage| usage > reservation.estimate)
}

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

/// The fake transport **for the serving call site**, under the `model.fake`
/// control.
///
/// Separate from [`Recorder`], which answers from one list whatever it is
/// asked: that is right for a unit test scripting an exact exchange, and
/// wrong here, because a count call would eat the completion's answer and
/// the caller would believe it had reached a completion it never made.
/// This one answers a count **as a count**, always, and scripts only the
/// completion — which is the answer a caller reads.
///
/// Nothing it returns is recorded by it. The recording boundary above it
/// writes every exchange to the store, redacted, and that is what a test
/// asserts over: a fake that kept its own second copy would let an
/// assertion pass against bytes the store never saw.
pub struct Fake {
    dialect: Dialect,
    answers: Vec<String>,
    usage: Option<u64>,
    next: Mutex<usize>,
}

impl Fake {
    pub fn new(dialect: Dialect, answers: Vec<String>, usage: Option<u64>) -> Self {
        Fake {
            dialect,
            answers,
            usage,
            next: Mutex::new(0),
        }
    }

    /// The script, one answer per completion, **the last one repeated**. A
    /// repair is a second completion, and a control that ran out would
    /// turn every repair into a different failure from the one under test.
    ///
    /// **Billed within the limits `body` declared** (m5-settle): the
    /// configured usage is billed as input up to the body's bytes, then as
    /// output up to the generation limit the body binds. So no honest
    /// answer passes its reservation or trips a stop. `overbilled:<answer>`
    /// answers `<answer>` and bills the whole usage as output with no cap:
    /// the one way a fixture overruns.
    fn scripted(&self, body: &[u8]) -> Answer {
        let mut next = self.next.lock().expect("not poisoned");
        let answer = self
            .answers
            .get(*next)
            .or_else(|| self.answers.last())
            .cloned()
            .unwrap_or_default();
        *next += 1;
        drop(next);
        if let Some(answer) = answer.strip_prefix("overbilled:") {
            return self.answer(answer, self.usage.map(|usage| (0, usage)));
        }
        let limit = wire::request::generation_of(self.dialect, body).unwrap_or(0);
        let honest = self.usage.map(|usage| {
            let input = usage.min(body.len() as u64);
            (input, (usage - input).min(limit))
        });
        self.answer(&answer, honest)
    }

    /// One scripted answer, billed `(input, output)` when it is billed.
    fn answer(&self, answer: &str, bill: Option<(u64, u64)>) -> Answer {
        let usage = bill.map(|(input, output)| input.saturating_add(output));
        let (input, output) = bill.unwrap_or((0, 0));
        let completed = |text: &str| Answer::Completed {
            body: wire::response::billed(self.dialect, text, input, output),
            usage,
        };
        match answer.split_once(':') {
            Some(("choose", id)) => completed(&format!("{{\"id\":\"{id}\"}}")),
            // **Discovery's two answers.** `terms:` proposes, `ids:`
            // chooses several; both are written the long way so that a
            // test can script an answer that breaks a bound — an empty
            // list, a term with a space in it, more ids than were
            // offered — which is the control negative control 5 needs.
            Some(("terms", list)) => completed(&format!("{{\"terms\":{}}}", Fake::quoted(list))),
            Some(("ids", list)) => completed(&format!("{{\"ids\":{}}}", Fake::quoted(list))),
            Some(("text", text)) => completed(text),
            _ if answer == "provider_exhausted" => Answer::ProviderExhausted,
            _ if answer == "failed" => Answer::Failed {
                reason: "scripted".into(),
                usage,
            },
            _ if answer == "not_sent" => Answer::NotSent("scripted".into()),
            // Anything else is a malformed body, which is a real outcome
            // and not a reason to panic in a provider.
            _ => Answer::Completed {
                body: Vec::new(),
                usage,
            },
        }
    }
}

impl Fake {
    /// A comma-separated script as a JSON array of strings. An empty
    /// script is an empty array, which is a model answering "no terms"
    /// and is an ordinary answer rather than a malformed one.
    fn quoted(list: &str) -> String {
        let members: Vec<String> = list
            .split(',')
            .filter(|member| !member.is_empty())
            .map(|member| format!("\"{member}\""))
            .collect();
        format!("[{}]", members.join(","))
    }
}

impl Transport for Fake {
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        match call {
            // What a provider that agreed exactly with the local bound
            // would say. The count is never the scripted answer.
            Call::Count => Exchange {
                answer: Answer::Counted(body.len() as u64),
                raw: Vec::new(),
            },
            Call::Completion => {
                let answer = self.scripted(body);
                let raw = match &answer {
                    Answer::Completed { body, .. } => body.clone(),
                    _ => Vec::new(),
                };
                Exchange { answer, raw }
            }
        }
    }
}
