//! The gate for the call path, written before it existed.

use rusqlite::Connection;

use super::*;
use crate::budget::{PER_REQUEST_TOKENS, Refusal, WINDOW_TOKENS};

const T0: &str = "2026-09-20T12:00:00Z";

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    Ledger::migrate(&connection).expect("migrates");
    connection
}

#[test]
fn the_count_call_is_a_send_and_is_admitted_recorded_and_charged_like_one() {
    let connection = database();
    let transport = Recorder::new(vec![
        Answer::Counted(120),
        Answer::Completed {
            body: b"{}".to_vec(),
            usage: 150,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert!(
        matches!(ended, Ended::Completed { usage: 150, .. }),
        "{ended:?}"
    );

    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "the count and the completion are both sends");
    assert_eq!(sent[0].0, Call::Count);
    assert_eq!(sent[1].0, Call::Completion);
    assert_eq!(sent[0].1, body_declaring(64), "the exact serialized bytes");

    let ledger = Ledger::new(&connection);
    let rows = ledger.rows().expect("rows");
    assert_eq!(rows.len(), 2, "both calls are in the ledger: {rows:?}");
    assert_eq!(
        ledger.spend(T0, "job").expect("spend").window,
        120 + 150,
        "and the count's own cost is debited beside the completion's"
    );
}

#[test]
fn a_request_the_local_estimate_refuses_never_reaches_the_count_endpoint() {
    // The mutant this exists for: reaching the provider count for a request
    // the local step refused. The count endpoint carries the whole body, so
    // that is the leak it was meant to prevent, arriving first.
    let connection = database();
    let transport = Recorder::new(vec![]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let mut huge = body_declaring(64);
    huge.extend(std::iter::repeat_n(b'x', PER_REQUEST_TOKENS as usize + 1));
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &huge,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended, Ended::Refused(Refusal::PerRequest));
    assert!(
        transport.sent().is_empty(),
        "nothing left the process: {:?}",
        transport.sent().len()
    );
    assert_eq!(
        Ledger::new(&connection)
            .spend(T0, "job")
            .expect("spend")
            .window,
        0
    );
}

#[test]
fn an_exhausted_envelope_refuses_before_the_count_call() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    // Fill the window under both ceilings.
    let mut spent = 0u64;
    let mut job = 0u64;
    while spent + PER_REQUEST_TOKENS <= WINDOW_TOKENS {
        let name = format!("filler-{job}");
        for _ in 0..4 {
            if spent + PER_REQUEST_TOKENS > WINDOW_TOKENS {
                break;
            }
            let reservation = ledger
                .admit(T0, &name, "r", PER_REQUEST_TOKENS)
                .expect("admits")
                .expect("admitted");
            ledger
                .settle(
                    T0,
                    &reservation,
                    crate::budget::Settlement::Usage(PER_REQUEST_TOKENS),
                )
                .expect("settles");
            spent += PER_REQUEST_TOKENS;
        }
        job += 1;
    }
    let transport = Recorder::new(vec![]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "fresh",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended, Ended::Refused(Refusal::WindowExhausted));
    assert_eq!(ended.reason(), Some("budget_exhausted"));
    assert!(transport.sent().is_empty(), "and nothing was sent");
}

#[test]
fn provider_exhaustion_reaches_the_caller_as_its_own_reason_and_is_not_retried() {
    let connection = database();
    let transport = Recorder::new(vec![Answer::ProviderExhausted]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("provider_quota_exhausted"));
    assert_ne!(
        ended.reason(),
        Some("budget_exhausted"),
        "a different limit"
    );
    assert_eq!(transport.sent().len(), 1, "once, not in a loop");
    assert_eq!(
        Ledger::new(&connection)
            .spend(T0, "job")
            .expect("spend")
            .window,
        0,
        "a refused call spent nothing and the ledger says so"
    );
}

#[test]
fn a_failed_call_reaches_the_caller_as_an_unmet_reason_and_never_hangs() {
    let connection = database();
    let transport = Recorder::new(vec![Answer::NotSent("connection reset".into())]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("model_not_sent"));
    assert_eq!(transport.sent().len(), 1);
}

#[test]
fn a_usage_that_differs_from_the_count_is_what_the_ledger_keeps() {
    let connection = database();
    let transport = Recorder::new(vec![
        Answer::Counted(100),
        Answer::Completed {
            body: Vec::new(),
            usage: 900,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(
        Ledger::new(&connection)
            .spend(T0, "job")
            .expect("spend")
            .window,
        1_000,
        "100 counted plus 900 spent, not the estimate"
    );
}

#[test]
fn nothing_calls_the_transport_without_a_request_behind_it() {
    // Background spend is zero: there is no path that sends without a caller.
    let connection = database();
    let transport = Recorder::new(vec![]);
    let _runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    assert!(
        transport.sent().is_empty(),
        "constructing a runtime sends nothing"
    );
    assert!(
        Ledger::new(&connection).rows().expect("rows").is_empty(),
        "and writes no ledger row"
    );
}

#[test]
fn every_crash_boundary_is_reached_in_order() {
    // The crash matrix kills at these; this is what says they exist and are
    // reached, so a row that never fires cannot pass as one that did.
    let connection = database();
    let transport = Recorder::new(vec![]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let seen = std::sync::Mutex::new(Vec::new());
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body_declaring(64),
            messages: 1,
            generation: 64,
        },
        &|name| {
            // **What is true at each boundary, not merely that it was reached.**
            // A test that only collects names passes a build that reserves after
            // it sends, which is the exact mutant this has to kill.
            let sent = transport.sent().len();
            let mut seen = seen.lock().expect("not poisoned");
            match name {
                COUNT_AFTER_RESERVATION | COMPLETION_AFTER_RESERVATION => assert_eq!(
                    sent,
                    seen.iter()
                        .filter(|n| **n == COUNT_AFTER_SEND || **n == COMPLETION_AFTER_SEND)
                        .count(),
                    "a reservation is written before its send, never after it"
                ),
                COUNT_AFTER_SEND | COMPLETION_AFTER_SEND => assert_eq!(
                    sent,
                    seen.iter()
                        .filter(|n| **n == COUNT_AFTER_SEND || **n == COMPLETION_AFTER_SEND)
                        .count()
                        + 1,
                    "one send per boundary, and it has happened by now"
                ),
                _ => {}
            }
            seen.push(name);
        },
    );
    let seen = seen.lock().expect("not poisoned").clone();
    assert_eq!(
        seen,
        vec![
            COUNT_AFTER_RESERVATION,
            COUNT_AFTER_SEND,
            COUNT_DURING_RECONCILIATION,
            COMPLETION_AFTER_RESERVATION,
            COMPLETION_AFTER_SEND,
            COMPLETION_DURING_RECONCILIATION,
        ],
        "both calls pass all three boundaries in order"
    );
}

// ---------------------------------------------------------------------------
// The review's three defects, reproduced before they were fixed.
//
// Findings 1 and 2 are both in this module — the one whose tests were written
// in the same step as the code and never ran red. These ran red first.
// ---------------------------------------------------------------------------

#[test]
fn the_completion_reserves_its_generation_and_margin_not_the_input_count_alone() {
    // **Finding 1.** The completion was admitted against the provider's
    // input count alone, which drops the reserved generation and the margin
    // the local estimate carried and trusts whatever figure came back. With
    // the window nearly full, a count of 1 admitted a completion that then
    // spent fifty thousand.
    let connection = database();
    let ledger = Ledger::new(&connection);
    fill_window_to(&ledger, WINDOW_TOKENS - 8_000);

    let transport = Recorder::new(vec![
        Answer::Counted(1),
        Answer::Completed {
            body: Vec::new(),
            usage: 50_000,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let body = body_declaring(4_096);
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 4_096,
        },
        &no_barrier,
    );

    // The reservation has to cover the generation the request asked for, so
    // a usage that large cannot be admitted against a window with 20,000
    // left in it.
    assert_eq!(
        ended,
        Ended::Refused(Refusal::WindowExhausted),
        "a completion that could spend more than the window has left is refused: {ended:?}"
    );
    assert_eq!(
        transport.sent().len(),
        1,
        "the count was sent; the completion was not"
    );
}

#[test]
fn the_completions_reservation_covers_the_margin_as_well_as_the_generation() {
    // **The mutant the review found surviving.** Removing the margin from
    // the completion's reservation passed every test at m4a's head: the
    // reservation was asserted to be "larger than the count's", which it
    // still was. The statement that distinguishes them is behavioural — with
    // exactly the input count plus the generation left in the window, a
    // reservation that omits the margin is admitted and one that includes it
    // is not.
    let body = body_declaring(64);
    let local = crate::budget::estimate(&body, 1);
    let refined = local; // the count is answered at the local figure
    let without_margin = refined + 64;

    let connection = database();
    let ledger = Ledger::new(&connection);
    // Leave room for the count, then exactly `without_margin` for the
    // completion — so the margin is the whole of the difference.
    fill_window_to(&ledger, WINDOW_TOKENS - (local + without_margin));

    let transport = Recorder::new(vec![
        Answer::Counted(refined),
        Answer::Completed {
            body: Vec::new(),
            usage: 10,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(
        ended,
        Ended::Refused(Refusal::WindowExhausted),
        "the margin is part of what the completion reserves: {ended:?}"
    );
    assert_eq!(
        transport.sent().len(),
        1,
        "the count went; the completion did not"
    );
}

#[test]
fn a_count_implausibly_below_the_local_bound_is_an_anomaly_and_the_local_figure_stands() {
    // A byte bound runs three to four times the real count for prose, so a
    // provider figure below an eighth of it is not a tighter count, it is a
    // number to disbelieve. The local figure stands and the anomaly is kept.
    let connection = database();
    let transport = Recorder::new(vec![
        Answer::Counted(1),
        Answer::Completed {
            body: Vec::new(),
            usage: 10,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let body = body_declaring(16);
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 16,
        },
        &no_barrier,
    );
    let rows = Ledger::new(&connection).rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "anomaly"),
        "the implausible count is recorded: {rows:?}"
    );
}

#[test]
fn usage_above_the_reservation_is_recorded_as_a_divergence() {
    let connection = database();
    let transport = Recorder::new(vec![
        Answer::Counted(200),
        Answer::Completed {
            body: Vec::new(),
            usage: 999_999,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let body = body_declaring(64);
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    let rows = Ledger::new(&connection).rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "divergence"),
        "spending more than was reserved is a recorded fact: {rows:?}"
    );
}

#[test]
fn a_request_that_does_not_declare_its_generation_limit_is_never_sent() {
    // The reservation covers a generation the request must actually ask for.
    // A body that does not declare it could spend more than was reserved, so
    // it does not leave the process.
    let connection = database();
    let transport = Recorder::new(vec![]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: b"{\"messages\":[]}",
            messages: 1,
            generation: 4_096,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("generation_limit_not_declared"));
    assert!(transport.sent().is_empty(), "and nothing was sent");
}

#[test]
fn a_failure_after_the_send_keeps_the_estimate_because_the_provider_may_have_charged() {
    // **Finding 2.** A failure settled to zero whatever had happened. A
    // timeout after the body went out is a call the provider may well have
    // charged for, and a ledger that records nothing for it under-counts.
    let connection = database();
    let body = body_declaring(64);
    let transport = Recorder::new(vec![
        Answer::Counted(2_000),
        Answer::Failed {
            reason: "timed out".into(),
            usage: None,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("model_call_failed"));
    let spend = Ledger::new(&connection).spend(T0, "job").expect("spend");
    assert!(
        spend.window > 2_000,
        "the completion's reservation is still counted: {spend:?}"
    );
}

#[test]
fn a_failure_before_anything_left_the_process_spends_nothing() {
    let connection = database();
    let body = body_declaring(64);
    let transport = Recorder::new(vec![Answer::NotSent("connect refused".into())]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("model_not_sent"));
    assert_eq!(
        Ledger::new(&connection)
            .spend(T0, "job")
            .expect("spend")
            .window,
        0,
        "nothing left, nothing spent"
    );
}

#[test]
fn a_failure_that_reports_usage_settles_to_what_it_reported() {
    let connection = database();
    let body = body_declaring(64);
    let transport = Recorder::new(vec![
        Answer::Counted(100),
        Answer::Failed {
            reason: "truncated".into(),
            usage: Some(777),
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(
        Ledger::new(&connection)
            .spend(T0, "job")
            .expect("spend")
            .window,
        100 + 777,
        "the provider said what it charged, so that is what is counted"
    );
}

#[test]
fn an_answer_of_the_wrong_kind_is_a_recorded_failure_and_not_a_silent_fall_through() {
    // **Finding 3's root.** A `Completed` answer returned to a count call
    // fell into the failure arm and looked like a transport error, which is
    // how the crash matrix came to pause at the count in all three rows
    // while believing it had reached the completion.
    let connection = database();
    let body = body_declaring(64);
    let transport = Recorder::new(vec![Answer::Completed {
        body: Vec::new(),
        usage: 5,
    }]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let ended = runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &no_barrier,
    );
    assert_eq!(ended.reason(), Some("model_answer_mismatched"));
    let rows = Ledger::new(&connection).rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "mismatch"),
        "and it is recorded as what it was: {rows:?}"
    );
}

#[test]
fn the_boundaries_are_named_per_call_so_a_row_cannot_pass_at_the_wrong_one() {
    let connection = database();
    let body = body_declaring(64);
    let transport = Recorder::new(vec![
        Answer::Counted(100),
        Answer::Completed {
            body: Vec::new(),
            usage: 200,
        },
    ]);
    let runtime = Runtime {
        ledger: Ledger::new(&connection),
        transport: &transport,
    };
    let seen = std::sync::Mutex::new(Vec::new());
    runtime.call(
        T0,
        &Attempt {
            job: "job",
            request: "r",
            body: &body,
            messages: 1,
            generation: 64,
        },
        &|name| seen.lock().expect("not poisoned").push(name),
    );
    assert_eq!(
        seen.lock().expect("not poisoned").clone(),
        vec![
            COUNT_AFTER_RESERVATION,
            COUNT_AFTER_SEND,
            COUNT_DURING_RECONCILIATION,
            COMPLETION_AFTER_RESERVATION,
            COMPLETION_AFTER_SEND,
            COMPLETION_DURING_RECONCILIATION,
        ],
        "each call has its own three"
    );
}

#[test]
fn a_run_ceiling_lowers_the_envelope_and_never_raises_it() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    // A run ceiling below the envelope binds.
    assert_eq!(
        ledger
            .with_run_ceiling(Some(1_000))
            .admit(T0, "job", "r", 2_000)
            .expect("admits"),
        Err(Refusal::RunCeiling)
    );
    // One above the envelope does not raise it: the window still refuses.
    let full = Ledger::new(&connection);
    fill_window_to(&full, WINDOW_TOKENS);
    assert_eq!(
        Ledger::new(&connection)
            .with_run_ceiling(Some(u64::MAX))
            .admit(T0, "fresh", "r", 100)
            .expect("admits"),
        Err(Refusal::WindowExhausted),
        "a ceiling can only lower the owner's envelope"
    );
}

#[test]
fn the_check_and_the_write_are_one_transaction() {
    // m4c adds concurrency, and a check-then-write race is an overspend: two
    // admissions could each read a spend the other was about to write and
    // both be admitted. `BEGIN IMMEDIATE` takes the write lock before the
    // read, so a second writer cannot slip between them — which is
    // observable, rather than a boolean the code asserts about itself.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("ledger.sqlite");
    let first = rusqlite::Connection::open(&path).expect("opens");
    Ledger::migrate(&first).expect("migrates");
    let second = rusqlite::Connection::open(&path).expect("opens");
    second
        .busy_timeout(std::time::Duration::from_millis(50))
        .expect("timeout");

    // Someone else holds the write lock.
    first.execute_batch("BEGIN IMMEDIATE").expect("locks");
    let blocked = Ledger::new(&second).admit(T0, "job", "r", 100);
    assert!(
        blocked.is_err(),
        "admission takes the write lock before it reads, so it waits rather than racing"
    );
    first.execute_batch("COMMIT").expect("commits");

    // And with the lock free it admits, leaving nothing half-written.
    assert!(
        Ledger::new(&second)
            .admit(T0, "job", "r", 100)
            .expect("admits")
            .is_ok()
    );
    assert_eq!(Ledger::new(&second).rows().expect("rows").len(), 1);
}

/// Fill the rolling window to `target`, under both ceilings.
fn fill_window_to(ledger: &Ledger<'_>, target: u64) {
    let mut spent = 0u64;
    let mut job = 0u64;
    while spent + PER_REQUEST_TOKENS <= target {
        let name = format!("filler-{job}");
        for _ in 0..(crate::budget::PER_JOB_TOKENS / PER_REQUEST_TOKENS) {
            if spent + PER_REQUEST_TOKENS > target {
                break;
            }
            let reservation = ledger
                .admit(T0, &name, "r", PER_REQUEST_TOKENS)
                .expect("admits")
                .expect("admitted");
            ledger
                .settle(
                    T0,
                    &reservation,
                    crate::budget::Settlement::Usage(PER_REQUEST_TOKENS),
                )
                .expect("settles");
            spent += PER_REQUEST_TOKENS;
        }
        job += 1;
    }
    // Top up to exactly `target`: the loop above steps by the per-request
    // ceiling, which rarely divides the target, and a test that meant to
    // leave 8,000 in the window must not leave 250,000.
    let remainder = target - spent;
    if remainder > 0 {
        let name = format!("filler-{job}");
        let reservation = ledger
            .admit(T0, &name, "top-up", remainder)
            .expect("admits")
            .expect("admitted");
        ledger
            .settle(
                T0,
                &reservation,
                crate::budget::Settlement::Usage(remainder),
            )
            .expect("settles");
    }
}

/// A body that declares the generation limit it was reserved for.
fn body_declaring(generation: u64) -> Vec<u8> {
    format!("{{\"messages\":[],\"max_tokens\":{generation}}}").into_bytes()
}
