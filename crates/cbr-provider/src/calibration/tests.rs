//! The gate for the calibration.
//!
//! **It is not run in this pull request.** What is built here is the thing
//! that will be run, so that the first live call runs code that has been
//! reviewed rather than plumbing written on the day.

use rusqlite::Connection;

use super::*;
use crate::budget::{self, Ledger};
use crate::model::{Answer, Recorder};
use crate::wire::Dialect;

const T0: &str = "2026-09-20T12:00:00Z";

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    Ledger::migrate(&connection).expect("migrates");
    crate::wire::record::migrate(&connection).expect("records");
    connection
}

/// A provider that answers every count with a quarter of the local bound,
/// which is what a real tokenizer does to prose, and the completion with
/// sixteen tokens.
fn believable() -> Recorder {
    let mut answers = Vec::new();
    for (_, text) in corpus() {
        answers.push(Answer::Counted((text.len() / 4) as u64));
    }
    // The completion is a count and then the completion itself.
    answers.push(Answer::Counted(64));
    answers.push(Answer::Completed {
        body: br#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"total_tokens":80}}"#.to_vec(),
        usage: Some(80),
    });
    Recorder::new(answers)
}

#[test]
fn the_corpus_is_this_repositorys_own_sources_and_one_non_latin_text() {
    // READINESS section 10: CBR's own **public** source files, so nothing
    // here sends anyone else's text, plus one non-Latin text, which is the
    // case a character-count bound would get wrong.
    let files = corpus();
    assert!(files.len() >= 5, "a corpus, not a file: {}", files.len());
    assert!(
        files.iter().any(|(name, _)| name.contains("non-latin")),
        "the non-Latin text is in it: {:?}",
        files.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (name, text) in &files {
        assert!(!text.is_empty(), "{name} is empty");
    }
    // It is fixed: the same corpus every run, so two runs are comparable.
    assert_eq!(
        files.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        corpus().iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
}

#[test]
fn the_whole_corpus_fits_under_the_run_ceiling() {
    // The protocol's number is a limit rather than a hope, so the corpus
    // has to be small enough for the run to finish inside it. If this ever
    // fails the corpus grew, and the answer is a smaller corpus rather
    // than a larger ceiling.
    let total: u64 = corpus()
        .iter()
        .map(|(_, text)| budget::estimate(text.as_bytes(), 2))
        .sum();
    assert!(
        total < CEILING,
        "the corpus reserves {total}, and the ceiling is {CEILING}"
    );
}

#[test]
fn the_run_counts_every_file_and_completes_once() {
    let connection = database();
    let transport = believable();
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    assert_eq!(report.rows.len(), corpus().len(), "one row per file");
    assert!(report.stopped.is_none(), "{:?}", report.stopped);
    assert!(report.completed(), "the completion path ran once");
    // Every file counted, and the completion: one send each way.
    assert_eq!(
        transport.sent().len(),
        corpus().len() + 2,
        "a count per file, plus the completion's own count and the completion"
    );
}

#[test]
fn every_row_carries_the_local_estimate_and_the_provider_count() {
    let connection = database();
    let transport = believable();
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    for row in &report.rows {
        assert!(row.local > 0, "{}: no local estimate", row.name);
        let provider = row.provider.expect("the provider answered");
        assert!(
            provider <= row.local,
            "{}: provider {provider} above local {}",
            row.name,
            row.local
        );
    }
    let table = report.table();
    for row in &report.rows {
        assert!(table.contains(&row.name), "the table names {}", row.name);
    }
    assert!(table.contains("local"), "and says which column is which");
}

#[test]
fn a_provider_count_above_its_local_estimate_stops_the_run() {
    // **The whole point of the exercise.** The admission design rests on
    // the local bound never falling below the truth, and one counter-example
    // says it does. Not "note it and widen the margin": stop, report, and
    // nothing else in M4 proceeds.
    let connection = database();
    let mut answers = vec![Answer::Counted(10)];
    // The second file's count comes back above anything the bound allows.
    answers.push(Answer::Counted(u32::MAX as u64));
    for _ in 0..corpus().len() {
        answers.push(Answer::Counted(10));
    }
    let transport = Recorder::new(answers);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    assert!(!report.completed(), "and did not go on to the completion");
    let stopped = report.stopped.clone().expect("the run stopped");
    assert!(
        stopped.contains("above its local estimate"),
        "and said why: {stopped}"
    );
    assert!(
        report.rows.len() < corpus().len(),
        "and did not count the rest: {} of {}",
        report.rows.len(),
        corpus().len()
    );
    assert!(
        transport.sent().len() <= 2,
        "nothing was sent after the stop: {}",
        transport.sent().len()
    );
}

#[test]
fn every_exchange_is_recorded_through_the_redaction_boundary() {
    // The calibration is not exempt from section 6 because it is a
    // measurement: every call is recorded and redacted like any other.
    let connection = database();
    let transport = believable();
    let recording = crate::wire::record::Recording {
        inner: &transport,
        store: &connection,
        now: T0,
        job: "calibration",
        request: "count",
        model: "MiniMax-M2.7",
        dialect: Dialect::OpenAi,
        scrubber: None,
    };
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &recording,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    assert!(report.stopped.is_none());
    let rows = crate::wire::record::rows(&connection).expect("rows");
    assert_eq!(
        rows.len(),
        corpus().len() + 2,
        "every exchange reached the store"
    );
}

#[test]
fn the_completion_is_capped_at_sixteen_generated_tokens() {
    // Small enough that the completion path can be exercised once end to
    // end and still cost almost nothing if everything else is wrong.
    assert_eq!(GENERATION, 16);
    let connection = database();
    let transport = believable();
    run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    let sent = transport.sent();
    let completion = String::from_utf8_lossy(&sent[sent.len() - 1].1).to_string();
    let body = cbr_encoding::parse(completion.as_bytes()).expect("its own body");
    assert_eq!(
        body.get("max_tokens"),
        Some(&cbr_encoding::Value::Int(16)),
        "{completion}"
    );
}

#[test]
fn the_run_is_bounded_by_the_ceiling_it_was_given() {
    // Enforced by the ledger rather than by intention: a ceiling too low
    // for the corpus stops the run with a refusal rather than overspending.
    let connection = database();
    let transport = believable();
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(1_000)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    let stopped = report.stopped.expect("the ceiling stopped it");
    assert!(stopped.contains("run_over_ceiling"), "{stopped}");
}

#[test]
fn a_count_below_the_implausibility_floor_is_still_reported_as_the_providers_own() {
    // The call path **disbelieves** a count far below the local bound and
    // uses the local figure instead, which is the conservative thing to do
    // when about to spend. The table is not spending: it is measuring how
    // loose the bound is, and substituting the local estimate there would
    // record that the bound was exactly right in the one case where it was
    // furthest from it.
    let connection = database();
    // One count per file, then the completion's own count.
    let mut answers: Vec<Answer> = (0..corpus().len() + 1)
        .map(|_| Answer::Counted(1))
        .collect();
    answers.push(Answer::Completed {
        body: br#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"total_tokens":8}}"#.to_vec(),
        usage: Some(8),
    });
    let transport = Recorder::new(answers);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::OpenAi,
        "MiniMax-M2.7",
    );
    assert!(report.stopped.is_none(), "{:?}", report.stopped);
    for row in &report.rows {
        assert_eq!(
            row.provider,
            Some(1),
            "{}: the provider said 1 and the table says so",
            row.name
        );
        assert!(row.local > 1, "{}: and the local bound was not 1", row.name);
    }
    // The ledger, separately, recorded the anomaly rather than acting on it.
    let rows = Ledger::new(&connection).rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "anomaly"),
        "the disbelief is a recorded fact: {rows:?}"
    );
}
