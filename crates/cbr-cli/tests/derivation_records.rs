//! **Every model exchange a request takes an answer from is sealed as
//! evidence, and nothing else is.**
//!
//! [READINESS §6] asks for a record that can be inspected long after the
//! call: the model, the admission path, what it cost, how long it took,
//! the candidate set it was offered and the id it chose. These tests are
//! about that record existing, saying those things, and — the part with
//! teeth — **not** existing when nobody took the answer.
//!
//! A derivation record holds **no repository text**. What was shown is
//! identified by path, lines and digest; the bytes are the source
//! artifact the packet already cites. So the question "what does a store
//! keep of a repository" has the same answer after m4d as before it.
//!
//! **No model is called.** The transport is the `model.fake` control.
//!
//! [READINESS §6]: ../../../docs/work/m4/READINESS.md

use std::time::{Duration, Instant};

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, derivations, ledger, many_candidates, prepared, result, submit};

/// The sealed bytes of a derivation, **fetched the way a reader would**.
fn sealed(fixture: &Fixture, id: &str) -> Value {
    let record = derivations(&fixture.data())
        .into_iter()
        .find(|(found, _)| found == id)
        .unwrap_or_else(|| panic!("no derivation {id}"))
        .1;
    let digest = record
        .get("descriptor")
        .and_then(|descriptor| descriptor.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no digest in {record:?}"))
        .to_string();
    let out = fixture.directory.path().join("derivation.json");
    let fetched = fixture.cbr(&[
        "fetch",
        id,
        "--digest",
        &digest,
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    assert!(
        fetched.status.success(),
        "fetch {id}: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    let bytes = std::fs::read(&out).expect("the fetched file");
    assert_eq!(
        cbr_encoding::digest_bytes(&bytes),
        digest,
        "what was served is what was sealed"
    );
    cbr_encoding::parse(&bytes).expect("a canonical derivation record")
}

fn number(value: &Value, path: &[&str]) -> Option<i64> {
    let mut at = value;
    for name in path {
        at = at.get(name)?;
    }
    match at {
        Value::Int(number) => Some(*number),
        _ => None,
    }
}

fn text(value: &Value, path: &[&str]) -> String {
    let mut at = value;
    for name in path {
        at = at.get(name).unwrap_or(&Value::Null);
    }
    at.as_str().unwrap_or_default().to_string()
}

#[test]
fn a_call_a_request_took_an_answer_from_is_sealed_with_what_it_asked() {
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    prepared(&fixture, "sealed", "1");

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "one call, one record: {found:?}");
    let record = sealed(&fixture, &found[0].0);

    assert_eq!(text(&record, &["model"]), "MiniMax-M2.7-highspeed");
    assert_eq!(text(&record, &["answer", "chose"]), "c2");
    assert_eq!(text(&record, &["request"]), "sealed");
    assert_eq!(text(&record, &["item"]), "q");
    assert_eq!(
        number(&record, &["usage", "tokens"]),
        Some(5_000),
        "what the provider said it cost"
    );
    assert!(
        matches!(
            text(&record, &["admission"]).as_str(),
            "admitted_local" | "admitted_count"
        ),
        "the admission path: {record:?}"
    );
    assert!(
        number(&record, &["latency_ms"]).is_some(),
        "how long it took"
    );
    let offered = record
        .get("question")
        .and_then(|question| question.get("offered"))
        .and_then(Value::as_array)
        .unwrap_or_default()
        .to_vec();
    assert!(
        offered.len() > 1,
        "the candidate set it was offered: {offered:?}"
    );
    assert!(
        offered
            .iter()
            .any(|candidate| candidate.get("id").and_then(Value::as_str) == Some("c2")),
        "including the one it chose"
    );
    provider.stop();
}

#[test]
fn a_record_quotes_no_repository_text() {
    // The record is an audit record, not a second copy of the
    // repository. A store that keeps derivations for ever keeps no more
    // of anybody's source than it did before m4d.
    let fixture = Fixture::answering(&["choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "quoting", "1");
    let found = derivations(&fixture.data());
    let record = sealed(&fixture, &found[0].0);
    let canonical = String::from_utf8(cbr_encoding::to_canonical(&record)).expect("utf-8");
    let repository = many_candidates();
    for line in repository.lines().filter(|line| line.len() > 30) {
        assert!(!canonical.contains(line), "the record quotes `{line}`");
    }
    assert!(canonical.contains("queue.md"), "but it says where it was");
    provider.stop();
}

#[test]
fn an_answer_that_was_never_offered_is_sealed_as_the_failure_it_was() {
    // A call that was made and could not be used is still a charge
    // against a shared quota and still a fact about the request.
    // Recording only the answers that worked would make the ledger and
    // the derivations disagree about how many calls there were.
    let fixture = Fixture::answering(&["choose:c99"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "unoffered", "1");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "model_choice_not_offered".to_string())
    );

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "the call happened: {found:?}");
    let record = sealed(&fixture, &found[0].0);
    assert_eq!(
        text(&record, &["answer", "unmet"]),
        "model_choice_not_offered"
    );
    assert!(
        record
            .get("answer")
            .and_then(|answer| answer.get("chose"))
            .is_none(),
        "and nothing was chosen"
    );
    provider.stop();
}

#[test]
fn two_answers_to_one_question_are_two_records() {
    // **The record is addressed by its own bytes, not by its
    // question.** A model is not a function: two jobs can ask the same
    // thing and be told different things, and a store that kept the
    // first and dropped the second would have a ledger saying two calls
    // happened and a derivation saying one did.
    let fixture = Fixture::answering(&["choose:c2", "choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "first", "1");
    prepared(&fixture, "second", "1");

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 2, "two calls, two records: {found:?}");
    let chosen: Vec<String> = found
        .iter()
        .map(|(id, _)| text(&sealed(&fixture, id), &["answer", "chose"]))
        .collect();
    assert!(
        chosen.contains(&"c2".to_string()) && chosen.contains(&"c1".to_string()),
        "and they say what each was told: {chosen:?}"
    );
    provider.stop();
}

#[test]
fn a_request_that_authorises_no_investigation_seals_no_derivation() {
    let fixture = Fixture::answering(&["choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "deterministic", "0");
    assert_eq!(
        derivations(&fixture.data()),
        Vec::new(),
        "no call, no record"
    );
    provider.stop();
}

#[test]
fn a_cancelled_request_seals_nothing_and_still_owes_what_it_spent() {
    // **The case with teeth.** The call is held after it sent, the
    // request is cancelled, and only then is the call let go. It
    // completes and settles in the pool — and nobody ever takes the
    // answer, so nothing is sealed: *a cancelled call leaves no partial
    // derivation record*.
    //
    // What it spent is a different question and has the opposite
    // answer. A call that went out was charged, and a ledger that
    // forgot it would overspend a quota shared with the owner's other
    // tools.
    let fixture = Fixture::paused_at("model.completion.after_send");
    let provider = fixture.start();
    submit(&fixture, "abandoned", "queue.md");

    let reached = fixture.barriers.join("model.completion.after_send.reached");
    let started = Instant::now();
    while !reached.exists() {
        let polled = fixture.cbr(&["request", "abandoned"]);
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the completion was never held; the request said {}",
            String::from_utf8_lossy(&polled.stdout)
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let cancelled = fixture.cbr(&["cancel", "abandoned"]);
    assert!(
        cancelled.status.success(),
        "cancel: {}",
        String::from_utf8_lossy(&cancelled.stderr)
    );
    // Let the held call finish. Its answer settles into a slot nobody
    // will ever ask about again.
    std::fs::write(
        fixture.barriers.join("model.completion.after_send.release"),
        b"",
    )
    .expect("release");

    let spent: i64 = {
        let started = Instant::now();
        loop {
            let rows = ledger(&fixture.data());
            let spent: i64 = rows
                .iter()
                .filter(|(request, ..)| request == "abandoned")
                .map(|(_, _, tokens)| tokens)
                .sum();
            if spent > 0 && rows.iter().any(|(_, kind, _)| kind == "usage") {
                break spent;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the released call never settled: {rows:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    assert!(spent > 0, "the call was charged");

    // And give the provider every chance to seal one anyway.
    for _ in 0..25 {
        let _ = fixture.cbr(&["request", "abandoned"]);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        derivations(&fixture.data()),
        Vec::new(),
        "an answer nobody took was sealed anyway"
    );
    provider.stop();
}

/// What the ledger charged for `request`, row by row in the order the
/// rows were settled: its completions, and its count calls, which the
/// ledger names `<request>.count`. Notes are not charges.
fn charged_for(fixture: &Fixture, request: &str) -> Vec<i64> {
    let count = format!("{request}.count");
    ledger(&fixture.data())
        .into_iter()
        .filter(|(for_request, kind, _)| {
            (for_request == request || *for_request == count)
                && matches!(
                    kind.as_str(),
                    "reservation" | "usage" | "unknown" | "provider_exhausted" | "not_sent"
                )
        })
        .map(|(_, _, tokens)| tokens)
        .collect()
}

/// Each attempt a record kept, as the charge it made: its completion and
/// its count call, if it had one.
fn attempts_charged(record: &Value) -> Vec<i64> {
    record
        .get("usage")
        .and_then(|usage| usage.get("attempts"))
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .map(|attempt| {
            number(attempt, &["tokens"]).unwrap_or(0)
                + number(attempt, &["count_tokens"]).unwrap_or(0)
        })
        .collect()
}

#[test]
fn a_repaired_steps_record_accounts_for_every_attempt_as_its_ledger_does() {
    // **Live run 3's finding, as a test.** A choice step answered in prose
    // was charged twice — the prose, then the repair — and its sealed
    // record said what the repair cost and nothing of the prose. The
    // ledger is the spend; a record of the question has to agree with it,
    // or a reader adding up records under-counts every repaired step.
    let fixture = Fixture::answering(&["text:the second span looks right", "choose:c2"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "repaired", "1");
    assert_eq!(
        result(&inspected, "q"),
        ("satisfied".to_string(), String::new()),
        "the repair answered: {inspected:?}"
    );

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "one question, one record: {found:?}");
    let record = sealed(&fixture, &found[0].0);
    let charged = charged_for(&fixture, "repaired");
    assert_eq!(
        charged.len(),
        2,
        "the prose and its repair were both charged: {charged:?}"
    );
    assert_eq!(number(&record, &["usage", "repairs"]), Some(1));
    assert_eq!(
        number(&record, &["usage", "tokens"]),
        Some(charged.iter().sum()),
        "the record's cost is the question's ledger rows: {record:?}"
    );
    assert_eq!(
        attempts_charged(&record),
        charged,
        "attempt by attempt, in the order the ledger settled them"
    );
    provider.stop();
}

#[test]
fn a_step_left_unmet_after_its_repair_still_accounts_for_both_attempts() {
    // The same rule on the way out that is not an answer. A step that ends
    // unmet after a repair was charged for both attempts, and a record
    // that kept only the last would say the question cost half of what it
    // did — on exactly the steps an operator most wants to count.
    let fixture = Fixture::answering(&["text:still no id in this answer"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "abandoned", "1");
    assert_eq!(
        result(&inspected, "q").0,
        "unmet",
        "prose twice is unmet: {inspected:?}"
    );

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "the call happened: {found:?}");
    let record = sealed(&fixture, &found[0].0);
    assert!(
        record
            .get("answer")
            .and_then(|answer| answer.get("unmet"))
            .is_some(),
        "sealed as the failure it was: {record:?}"
    );
    let charged = charged_for(&fixture, "abandoned");
    assert_eq!(charged.len(), 2, "both attempts were charged: {charged:?}");
    assert_eq!(number(&record, &["usage", "repairs"]), Some(1));
    assert_eq!(
        number(&record, &["usage", "tokens"]),
        Some(charged.iter().sum()),
        "the record's cost is the question's ledger rows: {record:?}"
    );
    assert_eq!(attempts_charged(&record), charged);
    provider.stop();
}

#[test]
fn a_repaired_discovery_choice_is_sealed_with_every_attempt_as_its_ledger_charged_them() {
    // **The door live run 3's repairs went through.** Both repaired steps
    // of that run were discovery's choice, answered in prose and then in
    // shape, and the rule was tested at selection's door and not at this
    // one — which is how the reviewer's mutant survived here while the
    // same edit at selection died. So: run 3's shape, end to end.
    let fixture = Fixture::answering(&[
        "choose:c1",
        "terms:tombstone,merger",
        "text:I would keep d1 and d3",
        "ids:d1,d3",
    ]);
    let provider = fixture.start();
    prepared(&fixture, "shaped-like-run-3", "3");

    let found = derivations(&fixture.data());
    assert_eq!(
        found.len(),
        3,
        "the item, the terms and the choice: {found:?}"
    );
    let records: Vec<Value> = found.iter().map(|(id, _)| sealed(&fixture, id)).collect();
    let choice = records
        .iter()
        .find(|record| text(record, &["question", "selector"]).starts_with("discovery.choose"))
        .unwrap_or_else(|| panic!("no choice record in {records:?}"));
    assert!(
        choice
            .get("answer")
            .and_then(|answer| answer.get("chose_ids"))
            .is_some(),
        "the repair answered in shape: {choice:?}"
    );

    let charged = charged_for(&fixture, "shaped-like-run-3");
    assert_eq!(
        charged.len(),
        4,
        "the item, the terms, the prose and its repair: {charged:?}"
    );
    assert_eq!(number(choice, &["usage", "repairs"]), Some(1));
    assert_eq!(
        attempts_charged(choice),
        charged[2..].to_vec(),
        "the choice's attempts are the last two rows the ledger charged: {choice:?}"
    );
    assert_eq!(
        number(choice, &["usage", "tokens"]),
        Some(charged[2..].iter().sum()),
        "the choice's record is its question's ledger rows"
    );
    let recorded: i64 = records
        .iter()
        .map(|record| number(record, &["usage", "tokens"]).unwrap_or(0))
        .sum();
    assert_eq!(
        recorded,
        charged.iter().sum::<i64>(),
        "the request's records account for every row its ledger charged"
    );
    provider.stop();
}

/// Each completion the ledger reserved for a request, as (what it was
/// reserved at, what it settled to), in order.
fn reservations(fixture: &Fixture, request: &str) -> Vec<(i64, i64)> {
    let connection =
        rusqlite::Connection::open(fixture.data().join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare(
            "SELECT estimate, tokens FROM model_ledger
             WHERE request = ?1 AND kind IN ('usage', 'unknown', 'not_sent', 'provider_exhausted')
             ORDER BY id",
        )
        .expect("the ledger table exists");
    statement
        .query_map([request], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("queries")
        .map(|row| row.expect("row"))
        .collect()
}

#[test]
fn a_repair_the_envelope_refuses_is_sealed_with_what_its_first_attempt_cost() {
    // **Refused is not "nothing happened" when the repair is what was
    // refused.** The first attempt was admitted and charged; the repair
    // asked for more than the run had left. The record of that question
    // has to carry the first attempt's charge, or a refused repair reads
    // as a question that cost nothing.
    //
    // The ceiling is measured, not guessed: a first store answers the
    // same question with no ceiling, which says what the first attempt
    // and the repair each reserve. A ceiling of exactly the first
    // reservation admits the first attempt in a fresh store, and cannot
    // admit the repair once the first has settled.
    let script = ["text:the second span looks right", "choose:c2"];
    let measuring = Fixture::answering(&script);
    let provider = measuring.start();
    prepared(&measuring, "measured", "1");
    provider.stop();
    let reserved = reservations(&measuring, "measured");
    assert_eq!(
        reserved.len(),
        2,
        "a first attempt and a repair: {reserved:?}"
    );
    let (first_reserved, first_charged) = reserved[0];
    let (repair_reserved, _) = reserved[1];
    let ceiling = first_reserved;
    assert!(
        first_charged + repair_reserved > ceiling,
        "that ceiling would admit the repair: {reserved:?}"
    );

    let fixture = Fixture::answering(&script);
    let provider = fixture.start_with(&["--model-run-ceiling", &ceiling.to_string()]);
    let inspected = prepared(&fixture, "refused", "1");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "run_over_ceiling".to_string()),
        "the repair was refused by the ceiling: {inspected:?}"
    );

    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "the question is recorded: {found:?}");
    let record = sealed(&fixture, &found[0].0);
    assert_eq!(text(&record, &["answer", "unmet"]), "run_over_ceiling");
    let charged = charged_for(&fixture, "refused");
    assert_eq!(
        charged,
        vec![first_charged],
        "the first attempt was charged and the repair never reserved"
    );
    assert_eq!(
        number(&record, &["usage", "tokens"]),
        Some(first_charged),
        "the record carries the first attempt's charge: {record:?}"
    );
    assert_eq!(
        text(&record, &["admission"]),
        "admitted_local",
        "and says the first attempt was admitted"
    );
    let attempts = record
        .get("usage")
        .and_then(|usage| usage.get("attempts"))
        .and_then(Value::as_array)
        .unwrap_or_default()
        .to_vec();
    assert_eq!(
        attempts.len(),
        2,
        "the refused repair is an attempt: {attempts:?}"
    );
    assert_eq!(
        attempts[1].get("admission"),
        Some(&Value::Null),
        "and it was not admitted"
    );
    provider.stop();
}
