//! **The ledger's crash boundaries, at the call site that serves a request.**
//!
//! Every row here kills a provider at one boundary inside a model call and
//! reads the ledger the dead process left behind. The property after every
//! one is the same and is deliberately one-sided: **the spend is counted at
//! least once and is never zero.** A ledger that forgets a spend overspends
//! a quota shared with the owner's other tools; one that counts it twice
//! only refuses a call it could have allowed.
//!
//! | Boundary | Barrier | Row |
//! |---|---|---|
//! | Count reserved, nothing sent | `model.count.after_reservation` | [`a_kill_after_the_count_reservation_leaves_the_spend_counted`] |
//! | Count sent, not reconciled | `model.count.after_send` | [`a_kill_after_the_count_send_leaves_the_estimate_counted`] |
//! | Inside the count's reconciliation | `model.count.during_reconciliation` | [`a_kill_during_the_count_reconciliation_counts_once`] |
//! | Completion reserved, nothing sent | `model.completion.after_reservation` | [`a_kill_after_the_completion_reservation_leaves_both_counted`] |
//! | Completion sent, not reconciled | `model.completion.after_send` | [`a_kill_after_the_completion_send_leaves_the_estimate_counted`] |
//! | Inside the completion's reconciliation | `model.completion.during_reconciliation` | [`a_kill_during_the_completion_reconciliation_counts_once`] |
//!
//! # Why they moved here
//!
//! Until m4c these rows were driven by a control that made **one scripted
//! call at startup**, because there was no serving call site for the
//! boundaries to belong to: the provider had no way to reach a transport
//! while serving anything. m4c gives it one, so the rows now kill a
//! provider in the middle of preparing a real context request — which is
//! the thing they were always standing in for.
//!
//! They live in this crate rather than beside the rest of the crash matrix
//! because reaching the call site means submitting a request, and `cbr` is
//! the client that submits one.
//!
//! **No model is called.** The transport is the `model.fake` control, which
//! carries its own model identity so that a launch which needs a model call
//! still needs no credential, and which a production configuration refuses.

use std::path::Path;
use std::time::{Duration, Instant};

mod serving;

use serving::Fixture;

/// The ledger rows that are **charges**. The notes recording which
/// evidence admitted a call sit beside them and are not charges, so a row
/// count that includes one is counting two different things.
fn ledger_spend(data: &Path) -> Vec<(String, String, i64, i64)> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare("SELECT request, kind, tokens, estimate FROM model_ledger ORDER BY id")
        .expect("the ledger table exists");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("queries")
        .map(|row| row.expect("row"))
        .collect::<Vec<_>>();
    rows.into_iter()
        .filter(|(_, kind, _, _)| !kind.starts_with("admitted_"))
        .collect()
}

fn counted(rows: &[(String, String, i64, i64)]) -> i64 {
    rows.iter()
        .filter(|(_, kind, _, _)| {
            matches!(
                kind.as_str(),
                "reservation" | "usage" | "unknown" | "provider_exhausted" | "not_sent"
            )
        })
        .map(|(_, _, tokens, _)| tokens)
        .sum()
}

/// Run one row: submit a request that needs a model, let the preparation
/// reach `barrier`, kill the process there, and hand the ledger back.
fn row(barrier: &str) -> Vec<(String, String, i64, i64)> {
    let fixture = Fixture::paused_at(barrier);
    let provider = fixture.start();

    // **The request authorises an investigation**, which is what makes
    // selection ask a model at all. Without one the compiler is the
    // deterministic path M3 shipped and nothing is called.
    let submitted = fixture.cbr(&[
        "context",
        "crash",
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
        "--repo-id",
        "app",
        "--selector",
        "queue drains shutdown",
        "--task",
        "what drains the queue",
        "--capacity",
        "65536",
        "--investigation",
        "1",
        "--want",
        "q=source:queue.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );

    // Each `cbr request` is a poll, and each poll is a tick. Preparation
    // takes ticks now: the index is built off the tick and the model call
    // with it, so this drives the job until the call reaches the barrier.
    let reached = fixture.barriers.join(format!("{barrier}.reached"));
    let started = Instant::now();
    let mut last = Vec::new();
    while !reached.exists() {
        let polled = fixture.cbr(&["request", "crash"]);
        last = polled.stdout.clone();
        // **What the request says, when it says nothing happened.** A
        // barrier that is never reached is almost always a request that
        // ended for its own reasons — no model configured, no
        // investigation authorised, nothing to choose between — and a
        // bare timeout hides which.
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{barrier} was never reached; the request last said {} and the provider said {}",
            String::from_utf8_lossy(&last),
            String::from_utf8_lossy(&polled.stderr)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = last;
    // Dropping it is the SIGKILL: the guard kills and reaps.
    provider.stop();

    let rows = ledger_spend(&fixture.data());
    assert!(
        counted(&rows) > 0,
        "the spend is counted, never zero, at {barrier}: {rows:?}"
    );
    rows
}

#[test]
fn a_kill_after_the_count_reservation_leaves_the_spend_counted() {
    // The reservation is written *before* anything is sent, so this is the
    // moment the design exists for: the process dies holding a reservation
    // for a call that never happened, and the spend is still counted.
    let rows = row("model.count.after_reservation");
    assert_eq!(rows.len(), 1, "only the count has been reserved: {rows:?}");
    assert_eq!(rows[0].0, "crash.count", "and it is the count's: {rows:?}");
    assert_eq!(rows[0].1, "reservation", "nothing settled it: {rows:?}");
}

#[test]
fn a_kill_after_the_count_send_leaves_the_estimate_counted() {
    let rows = row("model.count.after_send");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "crash.count");
    assert_eq!(rows[0].1, "reservation", "still holding its estimate");
    assert_eq!(rows[0].2, rows[0].3, "which is what it reserved");
}

#[test]
fn a_kill_during_the_count_reconciliation_counts_once() {
    let rows = row("model.count.during_reconciliation");
    let spends = rows
        .iter()
        .filter(|(_, kind, _, _)| kind == "reservation" || kind == "usage")
        .count();
    assert_eq!(spends, 1, "counted once, not twice and not none: {rows:?}");
    assert_eq!(rows[0].0, "crash.count");
}

#[test]
fn a_kill_after_the_completion_reservation_leaves_both_counted() {
    // The count has settled and the completion is reserved and unsent.
    let rows = row("model.completion.after_reservation");
    assert_eq!(rows.len(), 2, "the count and the completion: {rows:?}");
    assert_eq!(rows[0].0, "crash.count");
    assert_eq!(rows[0].1, "usage", "the count settled: {rows:?}");
    assert_eq!(rows[1].0, "crash", "the completion is reserved: {rows:?}");
    assert_eq!(rows[1].1, "reservation");
    assert!(
        rows[1].2 > rows[0].2,
        "and its reservation covers the generation, so it is the larger: {rows:?}"
    );
}

#[test]
fn a_kill_after_the_completion_send_leaves_the_estimate_counted() {
    let rows = row("model.completion.after_send");
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[1].0, "crash");
    assert_eq!(rows[1].1, "reservation", "sent, not reconciled");
    assert_eq!(rows[1].2, rows[1].3, "the estimate stands, over-counting");
}

#[test]
fn a_kill_during_the_completion_reconciliation_counts_once() {
    let rows = row("model.completion.during_reconciliation");
    let completion: Vec<_> = rows
        .iter()
        .filter(|(request, ..)| request == "crash")
        .collect();
    assert_eq!(
        completion.len(),
        1,
        "the completion is one row, never two: {rows:?}"
    );
    assert!(
        matches!(completion[0].1.as_str(), "reservation" | "usage"),
        "either still its estimate or already its usage: {rows:?}"
    );
}
