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

/// A Responses-shaped completion carrying `total_tokens`.
fn responses_completion(total: u64) -> Vec<u8> {
    format!(
        "{{\"object\":\"response\",\"status\":\"completed\",\"output\":[{{\"type\":\"message\",\
         \"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"calibrated.\"}}]}}],\
         \"output_text\":\"calibrated.\",\"error\":null,\"usage\":{{\"input_tokens\":1,\
         \"output_tokens\":1,\"total_tokens\":{total}}}}}"
    )
    .into_bytes()
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
        body: responses_completion(80),
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
        Dialect::Responses,
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
        Dialect::Responses,
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
        Dialect::Responses,
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
        dialect: Dialect::Responses,
        scrubber: None,
    };
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &recording,
        Dialect::Responses,
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
        Dialect::Responses,
        "MiniMax-M2.7",
    );
    let sent = transport.sent();
    let completion = String::from_utf8_lossy(&sent[sent.len() - 1].1).to_string();
    let body = cbr_encoding::parse(completion.as_bytes()).expect("its own body");
    assert_eq!(
        body.get("max_output_tokens"),
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
        Dialect::Responses,
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
        body: responses_completion(8),
        usage: Some(8),
    });
    let transport = Recorder::new(answers);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::Responses,
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

// --- the second comparison: does the count predict what is charged? ------

/// A Responses completion whose usage reports `input` input tokens.
fn responses_completion_costing(input: u64, total: u64) -> Vec<u8> {
    format!(
        "{{\"object\":\"response\",\"status\":\"completed\",\"error\":null,\
         \"output\":[{{\"type\":\"message\",\"role\":\"assistant\",\
         \"content\":[{{\"type\":\"output_text\",\"text\":\"calibrated.\"}}]}}],\
         \"output_text\":\"calibrated.\",\"usage\":{{\"input_tokens\":{input},\
         \"output_tokens\":1,\"total_tokens\":{total}}}}}"
    )
    .into_bytes()
}

/// A run whose completion is counted at `counted` and charged `input` for
/// the same request.
fn run_with_completion(counted: u64, input: u64) -> Report {
    let connection = database();
    let mut answers: Vec<Answer> = corpus()
        .iter()
        .map(|(_, text)| Answer::Counted((text.len() / 4) as u64))
        .collect();
    answers.push(Answer::Counted(counted));
    answers.push(Answer::Completed {
        body: responses_completion_costing(input, input + 1),
        usage: Some(input + 1),
    });
    let transport = Recorder::new(answers);
    run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::Responses,
        "MiniMax-M2.7",
    )
}

#[test]
fn the_completion_compares_its_count_against_what_its_input_actually_cost() {
    // **The measurement that says whether the count endpoint predicts what
    // is charged.** The local bound against the provider's count is one
    // question; the provider's count against the provider's own bill for
    // the same request is a different one, and only the second says the
    // count is worth making.
    let report = run_with_completion(1_000, 1_000);
    let table = report.table();
    let completion = report.completion.expect("the completion ran");
    assert_eq!(completion.counted, Some(1_000), "what was predicted");
    assert_eq!(completion.input_usage, Some(1_000), "what was charged");
    assert!(completion.prediction_finding.is_none(), "and they agree");
    assert!(table.contains("counted"), "the table carries both: {table}");
    assert!(table.contains("input charged"), "{table}");
}

#[test]
fn an_input_charged_above_its_count_beyond_the_margin_is_a_finding_not_a_stop() {
    // **Reported, not stopping.** The stop condition is the local bound
    // being wrong, because the whole admission design rests on it. A count
    // that under-predicts the bill is a fact about the count endpoint, and
    // it is reported so the owner can decide what it means.
    let over = 1_000 + 1_000 * PREDICTION_MARGIN_PERCENT / 100 + 1;
    let report = run_with_completion(1_000, over);
    let table = report.table();
    let stopped = report.stopped.clone();
    let completion = report.completion.expect("the completion ran");
    let finding = completion
        .prediction_finding
        .expect("a finding was recorded");
    assert!(finding.contains("1000"), "naming the count: {finding}");
    assert!(
        finding.contains(&over.to_string()),
        "and the charge: {finding}"
    );
    assert!(
        stopped.is_none(),
        "it is a finding and not a stop: {stopped:?}"
    );
    assert!(table.contains("FINDING"), "{table}");
}

#[test]
fn an_input_charged_within_the_margin_is_not_a_finding() {
    // The negative control: a margin that flags everything reports nothing.
    let within = 1_000 + 1_000 * PREDICTION_MARGIN_PERCENT / 100;
    let report = run_with_completion(1_000, within);
    let completion = report.completion.expect("the completion ran");
    assert!(
        completion.prediction_finding.is_none(),
        "{:?}",
        completion.prediction_finding
    );
    // And an input charged *below* its count is the expected direction.
    let report = run_with_completion(1_000, 400);
    assert!(report.completion.expect("ran").prediction_finding.is_none());
}

#[test]
fn the_margin_is_a_stated_number() {
    assert_eq!(PREDICTION_MARGIN_PERCENT, 2);
}

// --- a truncated completion is not a stop --------------------------------

/// A Responses completion that ends `incomplete`, its whole output budget
/// spent on reasoning, exactly as run 2's did.
fn responses_incomplete() -> Vec<u8> {
    br#"{"object":"response","status":"incomplete","error":null,
"incomplete_details":{"reason":"max_output_tokens"},
"output":[{"type":"reasoning","content":[{"type":"reasoning_text","text":"thinking"}],
"summary":[]}],"output_text":null,
"usage":{"input_tokens":28,"output_tokens":16,"total_tokens":44}}"#
        .to_vec()
}

fn run_ending_in(body: Vec<u8>, usage: Option<u64>) -> Report {
    let connection = database();
    let mut answers: Vec<Answer> = corpus()
        .iter()
        .map(|(_, text)| Answer::Counted((text.len() / 4) as u64))
        .collect();
    answers.push(Answer::Counted(122));
    answers.push(Answer::Completed { body, usage });
    let transport = Recorder::new(answers);
    run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::Responses,
        "MiniMax-M2.7-highspeed",
    )
}

#[test]
fn a_truncated_completion_is_an_outcome_with_a_cost_and_not_a_stop() {
    // **The defect run 2 exposed in this module.** A sixteen-token limit
    // spent entirely on reasoning is what the provider documents itself
    // doing, and the run that produced every measurement it exists for
    // reported `STOPPED` and exited non-zero.
    //
    // A stop means one of two things and no others: a count above its
    // local estimate, or a measurement that could not be obtained.
    let report = run_ending_in(responses_incomplete(), Some(44));
    assert!(
        report.stopped.is_none(),
        "a truncated completion stopped the run: {:?}",
        report.stopped
    );
    assert_eq!(report.rows.len(), corpus().len(), "every file was counted");
    let completion = report.completion.expect("the completion is recorded");
    assert_eq!(
        completion.outcome, "model_answer_truncated",
        "and its outcome is carried rather than collapsed"
    );
    assert_eq!(completion.usage, Some(44), "with its cost");
    assert_eq!(completion.input_usage, Some(28));
    assert_eq!(completion.counted, Some(122));
}

#[test]
fn the_table_of_a_truncated_run_reads_as_a_finished_run() {
    let report = run_ending_in(responses_incomplete(), Some(44));
    let table = report.table();
    assert!(!table.contains("STOPPED"), "{table}");
    assert!(table.contains("model_answer_truncated"), "{table}");
    assert!(table.contains("44"), "the cost is in it: {table}");
}

#[test]
fn a_count_that_could_not_be_obtained_is_still_a_stop() {
    // The other half: the exit stays non-zero for the two things that
    // really are stops, or the change would have made every run succeed.
    let connection = database();
    let transport = Recorder::new(vec![Answer::Failed {
        reason: "provider_status".into(),
        usage: None,
    }]);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::Responses,
        "MiniMax-M2.7-highspeed",
    );
    assert!(report.stopped.is_some(), "a measurement was not obtained");
}

#[test]
fn a_completion_the_envelope_refused_is_still_a_stop() {
    // **The completion never happened at all**, so the run did not obtain
    // the measurement it came for. That is a stop, where a completion that
    // happened and was truncated is not.
    //
    // Reached by scripting the completion's own count above the
    // per-request ceiling, so the refusal lands on the completion rather
    // than on one of the corpus counts — an earlier version set a low run
    // ceiling, which stopped the first count instead and tested nothing
    // about this path.
    let connection = database();
    let mut answers: Vec<Answer> = corpus()
        .iter()
        .map(|(_, text)| Answer::Counted((text.len() / 4) as u64))
        .collect();
    answers.push(Answer::Counted(crate::budget::PER_REQUEST_TOKENS + 1));
    let transport = Recorder::new(answers);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(CEILING)),
        &transport,
        Dialect::Responses,
        "MiniMax-M2.7-highspeed",
    );
    assert_eq!(report.rows.len(), corpus().len(), "every file was counted");
    let stopped = report.stopped.expect("the completion was refused");
    assert!(stopped.contains("refused"), "{stopped}");
    assert!(report.completion.is_none(), "and there is no completion");
}

#[test]
fn a_run_ceiling_too_low_for_the_corpus_is_a_stop_too() {
    let connection = database();
    let transport = Recorder::new(vec![Answer::Counted(10)]);
    let report = run(
        T0,
        Ledger::new(&connection).with_run_ceiling(Some(1_000)),
        &transport,
        Dialect::Responses,
        "MiniMax-M2.7-highspeed",
    );
    let stopped = report.stopped.expect("the ceiling stopped it");
    assert!(stopped.contains("run_over_ceiling"), "{stopped}");
}
