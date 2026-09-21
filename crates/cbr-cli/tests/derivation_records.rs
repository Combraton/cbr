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
