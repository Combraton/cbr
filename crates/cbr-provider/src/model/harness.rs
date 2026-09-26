//! **One question on the real call path, against a provider that answers
//! as badly as it can.** Test-only.
//!
//! Every completion comes back unpriced, so each attempt holds its whole
//! reservation, and every one of them is either truncated or not the
//! shape the question asked for, so the one repair [`super::REPAIRS`]
//! allows is always made. The count is answered however the test says,
//! and the ledger is given whatever run ceiling the test says. What the
//! ledger then holds for the question is the figure the arithmetic claims
//! to bound, measured rather than argued.
//!
//! [`sweep`] and [`boundary_sweep`] run a body across the ceilings and
//! counts where the admission path changes its mind: just below, at and
//! just above each figure it compares, and eighths of the whole question
//! between them. [`with_room_freed`] frees room on the refusing counter
//! anywhere between a send's local refusal and its completion's
//! admission: the case m5-settle's per-attempt rule closes, run so a test
//! can show it closed.

use std::cell::{Cell, RefCell};

use rusqlite::Connection;

use super::{
    Answer, Ask, COUNT_AFTER_RESERVATION, COUNT_AFTER_SEND, COUNT_DURING_RECONCILIATION, Call,
    Exchange, Outcome, Runtime, SERVING_COUNTING, Transport, repaired, send_worst,
};
use crate::budget::{self, Ledger, Reservation, Settlement};
use crate::wire::Dialect;
use crate::wire::request::Request;
use crate::wire::response::REPAIRABLE;

/// The instant every run happens at. One window, one month.
pub(crate) const T0: &str = "2026-09-20T12:00:00Z";
/// The job the question is asked for.
const JOB: &str = "job";
/// A job of somebody else's, holding room a test frees.
const OTHER: &str = "other";
/// The dialect the serving path speaks, and the only one counted.
const DIALECT: Dialect = Dialect::Responses;

/// **Where another job's room is freed**, in the window between a send's
/// local refusal and its completion's admission: each place in it the
/// call path names a boundary at, and the count's flight.
///
/// The window opens before the first of these, between the local refusal
/// and the count's admission, and runs on past the last, from the count's
/// settlement to the completion's admission. Nothing is named there to
/// stop at, and a settlement on another thread can land in either: it is
/// the same window, and admits the same completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Freed {
    /// The count has been admitted and not yet sent.
    Admitted,
    /// The count is on the wire.
    InFlight,
    /// The count has answered and is not yet settled.
    Answered,
    /// The count is being settled.
    Settling,
}

/// Every place [`Freed`] names, in the order the call path reaches them.
pub(crate) const WINDOW: [Freed; 4] = [
    Freed::Admitted,
    Freed::InFlight,
    Freed::Answered,
    Freed::Settling,
];

impl Freed {
    /// The boundary the call path names at this place, when it names one.
    fn boundary(self) -> Option<&'static str> {
        match self {
            Freed::Admitted => Some(COUNT_AFTER_RESERVATION),
            Freed::InFlight => None,
            Freed::Answered => Some(COUNT_AFTER_SEND),
            Freed::Settling => Some(COUNT_DURING_RECONCILIATION),
        }
    }
}

/// What every completion answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Completion {
    /// Out of room before the answer: repaired with a wider limit.
    Truncated,
    /// Prose, where a structure or a tool call was asked for: repaired
    /// with CBR's own sentence.
    Prose,
}

/// What the counting endpoint answers, as a function of the body it was
/// sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CountAnswer {
    /// The count body's byte length, which is what the `model.fake`
    /// control answers.
    Bytes,
    /// `n / d` of the count's own reservation: its byte bound and the
    /// framing of its messages.
    Share(u64, u64),
    /// This figure, whatever was counted.
    Tokens(u64),
}

impl CountAnswer {
    fn answer(self, counted: &[u8]) -> u64 {
        match self {
            CountAnswer::Bytes => counted.len() as u64,
            CountAnswer::Share(n, d) => reservation_of_count(counted) * n / d,
            CountAnswer::Tokens(tokens) => tokens,
        }
    }
}

/// What a count body reserves, read from the body alone: a Responses
/// body frames its instructions and each of its input messages.
fn reservation_of_count(counted: &[u8]) -> u64 {
    let parsed = cbr_encoding::parse(counted).expect("a count body is canonical JSON");
    let messages = parsed
        .get("input")
        .and_then(cbr_encoding::Value::as_array)
        .map_or(0, <[cbr_encoding::Value]>::len)
        + usize::from(parsed.get("instructions").is_some());
    budget::input_bound(counted, messages)
}

/// What one question left behind.
#[derive(Debug)]
pub(crate) struct Held {
    /// What the ledger holds for the question's job.
    pub job: u64,
    pub outcome: Outcome,
    /// How many count calls were made.
    pub counts: usize,
    /// **What the whole ledger held as each send went out**, which is
    /// after that send's own reservation was admitted: every figure here
    /// was within the ceiling when it was admitted.
    pub admitted_at: Vec<u64>,
    /// Every ledger row afterwards, as [`Ledger::rows`] reads them, for a
    /// test that asserts a note.
    pub rows: Vec<(String, String, u64)>,
}

/// One run of a sweep, and what it was run with.
#[derive(Debug)]
pub(crate) struct Run {
    pub ceiling: Option<u64>,
    pub count: CountAnswer,
    pub completion: Completion,
    pub held: Held,
}

/// The provider this module scripts.
struct Script<'c> {
    connection: &'c Connection,
    completion: Completion,
    count: CountAnswer,
    counts: Cell<usize>,
    admitted_at: RefCell<Vec<u64>>,
    /// Another job's reservation, settled at nothing the first time the
    /// call path reaches `free_at`.
    freed: RefCell<Option<Reservation>>,
    free_at: Freed,
}

impl Script<'_> {
    /// Settle the other job's reservation at nothing, once.
    fn free(&self) {
        if let Some(other) = self.freed.borrow_mut().take() {
            Ledger::new(self.connection)
                .settle(T0, &other, Settlement::Usage(0))
                .expect("settles");
        }
    }
}

impl Transport for Script<'_> {
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        let spend = Ledger::new(self.connection)
            .spend(T0, JOB)
            .expect("the ledger reads")
            .window;
        self.admitted_at.borrow_mut().push(spend);
        let answer = match call {
            Call::Count => {
                self.counts.set(self.counts.get() + 1);
                if self.free_at == Freed::InFlight {
                    self.free();
                }
                Answer::Counted(self.count.answer(body))
            }
            Call::Completion => Answer::Completed {
                body: completed(self.completion),
                usage: None,
            },
        };
        Exchange {
            answer,
            raw: Vec::new(),
        }
    }
}

/// A Responses completion that says nothing about what it cost.
fn completed(completion: Completion) -> Vec<u8> {
    match completion {
        Completion::Truncated => br#"{"object":"response","status":"incomplete","error":null,"incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"reasoning","content":[{"type":"reasoning_text","text":"..."}],"summary":[]}],"output_text":null}"#.to_vec(),
        Completion::Prose => br#"{"object":"response","status":"completed","error":null,"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The second one, I think."}]}],"output_text":"The second one, I think."}"#.to_vec(),
    }
}

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    Ledger::migrate(&connection).expect("migrates");
    connection
}

fn asked(
    connection: &Connection,
    body: &Request,
    completion: Completion,
    ceiling: Option<u64>,
    count: CountAnswer,
    freed: Option<(Reservation, Freed)>,
) -> Held {
    let free_at = freed.as_ref().map_or(Freed::InFlight, |(_, at)| *at);
    let script = Script {
        connection,
        completion,
        count,
        counts: Cell::new(0),
        admitted_at: RefCell::new(Vec::new()),
        freed: RefCell::new(freed.map(|(reservation, _)| reservation)),
        free_at,
    };
    let barrier = |boundary: &'static str| {
        if free_at.boundary() == Some(boundary) {
            script.free();
        }
    };
    let runtime = Runtime {
        ledger: Ledger::new(connection).with_run_ceiling(ceiling),
        transport: &script,
    };
    let outcome = runtime.ask(
        T0,
        &Ask {
            job: JOB,
            request: "question",
            dialect: DIALECT,
            body,
            counting: SERVING_COUNTING,
        },
        &barrier,
    );
    Held {
        job: Ledger::new(connection)
            .spend(T0, JOB)
            .expect("the ledger reads")
            .job,
        outcome,
        counts: script.counts.get(),
        admitted_at: script.admitted_at.into_inner(),
        rows: Ledger::new(connection).rows().expect("the ledger reads"),
    }
}

/// Ask `body` once, on the serving path, under `ceiling`.
pub(crate) fn holds(
    body: &Request,
    completion: Completion,
    ceiling: Option<u64>,
    count: CountAnswer,
) -> Held {
    asked(&database(), body, completion, ceiling, count, None)
}

/// Ask `body` once under `ceiling` with `other` tokens of another job's
/// already reserved, and **settle that reservation at nothing at `at`**,
/// the first time the call path reaches it: after the send was refused on
/// the local bound and before the completion is admitted on the count.
pub(crate) fn with_room_freed(
    body: &Request,
    completion: Completion,
    ceiling: u64,
    other: u64,
    count: CountAnswer,
    at: Freed,
) -> Held {
    let connection = database();
    let reserved = Ledger::new(&connection)
        .with_run_ceiling(Some(ceiling))
        .admit(T0, OTHER, "elsewhere", other)
        .expect("the ledger admits")
        .expect("the other job's reservation fits");
    asked(
        &connection,
        body,
        completion,
        Some(ceiling),
        count,
        Some((reserved, at)),
    )
}

/// The input bound of `body` as sent, and of the body its count sends.
fn bounds(body: &Request) -> (u64, u64) {
    let messages = body.framed_messages(DIALECT);
    let counted = body
        .serialize_count(DIALECT)
        .expect("the serving dialect is counted");
    (
        budget::input_bound(&body.serialize(DIALECT), messages),
        budget::input_bound(&counted, messages),
    )
}

/// Every ceiling at which the admission path decides something different
/// for `body`: none; either side of its first send; the count body's
/// bound and the completion's, which a count reserves; the first send
/// beside a count;
/// either side of the first send and each repair; and eighths of the whole
/// question between them.
pub(crate) fn ceilings(body: &Request) -> Vec<Option<u64>> {
    let first = send_worst(body, DIALECT);
    let (local, counted) = bounds(body);
    let mut ceilings = vec![first - 1, first, first + 1, counted, local, first + counted];
    let mut widest = 0;
    for unusable in REPAIRABLE {
        let repair = send_worst(&repaired(body, unusable), DIALECT);
        ceilings.extend([first + repair - 1, first + repair, first + repair + 1]);
        widest = widest.max(repair);
    }
    ceilings.extend((1..=8).map(|eighth| (first + widest) * eighth / 8));
    ceilings.sort_unstable();
    ceilings.dedup();
    std::iter::once(None)
        .chain(ceilings.into_iter().map(Some))
        .collect()
}

/// Every count answer worth making: nothing, the floor's worth, half, the
/// body's bytes, the count body's whole bound, the completion's bound —
/// which a count reserves, and settles at no more than — and ten times
/// that.
pub(crate) fn counts(body: &Request) -> Vec<CountAnswer> {
    let (local, _) = bounds(body);
    vec![
        CountAnswer::Tokens(0),
        CountAnswer::Share(1, budget::IMPLAUSIBLE_RATIO),
        CountAnswer::Share(1, 2),
        CountAnswer::Bytes,
        CountAnswer::Share(1, 1),
        CountAnswer::Tokens(local),
        CountAnswer::Tokens(local * 10),
    ]
}

const COMPLETIONS: [Completion; 2] = [Completion::Truncated, Completion::Prose];

fn every(body: &Request, ceilings: &[Option<u64>], counts: &[CountAnswer]) -> Vec<Run> {
    let mut runs = Vec::new();
    for ceiling in ceilings {
        for count in counts {
            for completion in COMPLETIONS {
                runs.push(Run {
                    ceiling: *ceiling,
                    count: *count,
                    completion,
                    held: holds(body, completion, *ceiling, *count),
                });
            }
        }
    }
    runs
}

/// The whole grid, for a body small enough to run it on.
pub(crate) fn sweep(body: &Request) -> Vec<Run> {
    every(body, &ceilings(body), &counts(body))
}

/// **The ceilings a decision turns on and nothing between them**, with
/// the count the fake makes and the one at the completion's bound:
/// thirty-six runs, for a body too large to sweep whole.
pub(crate) fn boundary_sweep(body: &Request) -> Vec<Run> {
    let first = send_worst(body, DIALECT);
    let (local, counted) = bounds(body);
    let widest = REPAIRABLE
        .iter()
        .map(|unusable| send_worst(&repaired(body, *unusable), DIALECT))
        .max()
        .unwrap_or(0);
    let ceilings = [
        None,
        Some(first - 1),
        Some(first),
        Some(counted),
        Some(local),
        Some(first + counted),
        Some(first + widest - 1),
        Some(first + widest),
        Some(first + widest + 1),
    ];
    every(
        body,
        &ceilings,
        &[CountAnswer::Bytes, CountAnswer::Tokens(local)],
    )
}
