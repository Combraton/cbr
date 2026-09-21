//! **A model-assisted packet, rebuilt offline from retained records.**
//!
//! [INTERNALS §5] asks that a packet built with a model be reproducible
//! from what was retained, without calling the model again. This is that
//! test, and the comparison is between digests rather than between
//! descriptions: the packet a replay produces must be **byte-identical**
//! to the one the call produced.
//!
//! # How two packets are made comparable
//!
//! A sealed packet names its own request and its own job, so two
//! requests can never have equal bytes. The pair compared here is
//! therefore the same request id in two stores:
//!
//! * one store where a **different** request asked the question first
//!   and its derivation was retained, and the rebuild answers `probe`
//!   from that record with a transport that panics if called;
//! * one store where `probe` asked it live, through the fake.
//!
//! That works because a question's digest is the model, the task, the
//! selector and the candidates — **never the request that asked it**.
//! Two requests about the same file with the same selector are one
//! question, which is the same rule that makes the work pool ask it
//! once.
//!
//! **No model is called, in either run.** The live run is the
//! `model.fake` control; the rebuild's transport panics if reached, and
//! the panic is contained by the work pool as a typed failure rather
//! than becoming a call nobody asked for.
//!
//! [INTERNALS §5]: ../../../docs/spec/INTERNALS.md

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, artifact_revision, cited_span, derivations, prepared, result};

/// The sealed packet's digest, from the reference the request carries.
fn packet_digest(fixture: &Fixture, request: &str) -> String {
    let inspected = fixture.cbr(&["request", request]);
    assert!(
        inspected.status.success(),
        "inspect: {}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let parsed = cbr_encoding::parse(String::from_utf8_lossy(&inspected.stdout).trim().as_bytes())
        .expect("canonical JSON");
    parsed
        .get("packets")
        .and_then(Value::as_array)
        .and_then(<[Value]>::last)
        .and_then(|packet| packet.get("reference"))
        .and_then(|reference| reference.get("artifact"))
        .and_then(|artifact| artifact.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no packet in {parsed:?}"))
        .to_string()
}

/// Submit `request` about `queue.md` with the same selector and task
/// every other request here uses, and wait for it to settle.
fn ask(fixture: &Fixture, request: &str) -> Value {
    prepared(fixture, request, "1")
}

#[test]
fn a_rebuilt_packet_is_byte_identical_to_the_one_the_call_produced() {
    // The store that will rebuild: one request asks live, and then a
    // second provider over the same data directory answers a different
    // request from what the first one retained.
    let replayed = Fixture::answering(&["choose:c2"]);
    let live = replayed.start();
    ask(&replayed, "warm");
    assert_eq!(
        derivations(&replayed.data()).len(),
        1,
        "the live run retained one"
    );
    live.stop();

    let rebuilding = replayed.start_replaying();
    let inspected = ask(&replayed, "probe");
    assert_eq!(
        result(&inspected, "q"),
        ("satisfied".to_string(), String::new()),
        "the rebuild could not answer: {inspected:?}"
    );
    let rebuilt = packet_digest(&replayed, "probe");
    assert_eq!(
        derivations(&replayed.data()).len(),
        1,
        "a rebuild seals nothing: the record it read is the record it would write"
    );
    rebuilding.stop();

    // The store that asks the same request live.
    let direct = Fixture::answering(&["choose:c2"]);
    let running = direct.start();
    ask(&direct, "probe");
    let called = packet_digest(&direct, "probe");
    running.stop();

    assert_eq!(
        rebuilt, called,
        "the rebuilt packet is not the packet the call produced"
    );
}

#[test]
fn a_rebuild_with_nothing_retained_says_so_rather_than_choosing_for_itself() {
    // **The failure that matters most.** A rebuild that quietly fell
    // back to BM25's own first would produce a packet that looks
    // model-assisted and is not, and the digest comparison above would
    // pass for the wrong reason on any question nobody retained.
    let fixture = Fixture::answering(&["choose:c2"]);
    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "cold");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "model_answer_not_retained".to_string()),
        "a question nothing answered was answered anyway: {inspected:?}"
    );
    assert!(
        derivations(&fixture.data()).is_empty(),
        "and nothing was sealed"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_reaches_no_transport_at_all() {
    // The panicking transport is the point: if anything ever did reach
    // it, the work pool would turn the panic into `work_panicked`, and
    // the item would say so. It does not, and nothing is recorded as
    // having been sent.
    let fixture = Fixture::answering(&["choose:c1"]);
    let live = fixture.start();
    ask(&fixture, "warm");
    let sent_before = serving::bodies_sent(&fixture.data()).len();
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "again");
    assert_eq!(result(&inspected, "q").0, "satisfied");
    assert_eq!(
        serving::bodies_sent(&fixture.data()).len(),
        sent_before,
        "the rebuild sent something"
    );
    rebuilding.stop();
}

#[test]
fn a_question_whose_candidates_changed_is_not_answered_from_an_old_record() {
    // A retained answer is an id out of a closed set. The file is
    // edited between the call and the rebuild, so the candidates differ
    // and the question is a different question — which must find
    // nothing rather than an answer chosen from a list that no longer
    // exists.
    let fixture = Fixture::answering(&["choose:c2"]);
    let live = fixture.start();
    ask(&fixture, "warm");
    live.stop();

    // A new tree, so the request's basis moves with it.
    std::fs::write(
        fixture.checkout.join("queue.md"),
        format!(
            "## An opening nobody ranked\n\n{}",
            serving::many_candidates()
        ),
    )
    .expect("writes");
    serving::commit(&fixture.checkout, "edited");

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "after");
    assert_eq!(
        result(&inspected, "q").1,
        "model_answer_not_retained",
        "an edited file was answered from a record about the old one: {inspected:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_does_not_answer_from_a_record_made_under_a_wider_view() {
    // **The readable-set gate, walked around from the inside.** The
    // authority's job could read every registered repository, so its
    // derivation was sealed under that view. A reader whose grant
    // covers one of them must not get that answer back by asking the
    // same question under a rebuild — that would be `evidence.fetch`'s
    // refusal with an extra step in front of it.
    let fixture = Fixture::answering(&["choose:c2"]);
    let live = fixture.start();
    ask(&fixture, "warm");
    assert_eq!(derivations(&fixture.data()).len(), 1);
    fixture.issue_grant(
        "g-app",
        &["context.request", "context.read", "context.packet.read"],
        r#"{"kind":"context.request"},{"kind":"context.job"},{"kind":"context.packet"},{"kind":"cbr.repository","id":"app"}"#,
    );
    live.stop();

    let rebuilding = fixture.start_replaying();
    let submitted = fixture.cbr_as_reader(
        "g-app",
        &[
            "context",
            "narrow",
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
        ],
    );
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = std::time::Instant::now();
    let inspected = loop {
        let polled = fixture.cbr_as_reader("g-app", &["request", "narrow"]);
        let parsed = cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
            .expect("canonical JSON");
        if parsed.get("state").and_then(Value::as_str) != Some("preparing") {
            break parsed;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "never left preparing: {parsed:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(
        result(&inspected, "q").1,
        "model_answer_not_retained",
        "a narrower reader was handed a wider job's answer: {inspected:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_whose_records_disagree_says_so_rather_than_picking_one() {
    // **A rebuild must be a function of the records, not of their
    // order.** Two calls asked one question and were told different
    // things; both are retained, and the id they are stored under is a
    // digest that covers the instant each was made. Taking the first
    // match makes the rebuilt packet depend on a hash of a timestamp —
    // the same history producing either of two packets.
    //
    // m4e reruns these same questions live, so this state is not
    // hypothetical: it is what a second run of a pilot leaves behind.
    let fixture = Fixture::answering(&["choose:c2", "choose:c1"]);
    let live = fixture.start();
    ask(&fixture, "first");
    ask(&fixture, "second");
    assert_eq!(
        derivations(&fixture.data()).len(),
        2,
        "two records that disagree"
    );
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "third");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "model_answer_ambiguous".to_string()),
        "a rebuild chose between two disagreeing records: {inspected:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_uses_an_answer_two_records_agree_on() {
    // The other arm, and the one that stops the rule above from being
    // "more than one record is ambiguous". Two calls, one question, the
    // same answer: there is nothing to choose between, so the rebuild
    // answers.
    let fixture = Fixture::answering(&["choose:c2"]);
    let live = fixture.start();
    ask(&fixture, "first");
    ask(&fixture, "second");
    assert_eq!(
        derivations(&fixture.data()).len(),
        2,
        "two records that agree"
    );
    let cited = cited_span(&fixture, "first");
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "third");
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert_eq!(
        cited_span(&fixture, "third"),
        cited,
        "and it is the span both records name"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_does_not_answer_from_a_record_that_was_purged() {
    // **A purge is an order to destroy evidence**, and a rebuild that
    // answered from a purged record would serve what somebody asked to
    // be destroyed. `state` stays `sealed` through a purge — only the
    // `purge` member and the availability change — so a check on
    // `state` alone does not notice.
    //
    // The object has to survive the purge for this test to mean
    // anything: collection deletes it, and a record whose object is
    // gone is skipped for that reason instead. So the record's own
    // bytes are ingested as a second artifact first, and **an object
    // another sealed artifact names is never collected**. What is left
    // is exactly the case in question: a purged record whose bytes are
    // still on disk.
    let fixture = Fixture::answering(&["choose:c2"]);
    let live = fixture.start();
    ask(&fixture, "warm");
    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1);
    let artifact = found[0].0.clone();
    let digest = found[0]
        .1
        .get("descriptor")
        .and_then(|descriptor| descriptor.get("digest"))
        .and_then(Value::as_str)
        .expect("a digest")
        .to_string();

    let copy = fixture.directory.path().join("derivation.bytes");
    let fetched = fixture.cbr(&[
        "fetch",
        &artifact,
        "--digest",
        &digest,
        "--out",
        copy.to_str().expect("utf-8"),
    ]);
    assert!(
        fetched.status.success(),
        "fetch: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    let ingested = fixture.cbr(&["ingest", copy.to_str().expect("utf-8")]);
    assert!(
        ingested.status.success(),
        "ingest: {}",
        String::from_utf8_lossy(&ingested.stderr)
    );

    fixture.purge_as_owner(&artifact, artifact_revision(&fixture.data(), &artifact));
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "after");
    assert_eq!(
        result(&inspected, "q").1,
        "model_answer_not_retained",
        "a purged record answered a rebuild: {inspected:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_retained_failure_beside_a_retained_choice_is_a_disagreement() {
    // **The decision, tested rather than only written down.** One call
    // answered with an id that was never offered and is retained as
    // that failure; a second answered with a candidate. Preferring the
    // usable one would be the rebuild deciding which of two histories
    // to reproduce — improving on the past rather than replaying it,
    // which is the same fault as taking the first match with better
    // manners.
    let fixture = Fixture::answering(&["choose:c99", "choose:c2"]);
    let live = fixture.start();
    let first = ask(&fixture, "first");
    assert_eq!(
        result(&first, "q").1,
        "model_choice_not_offered",
        "the first call failed, as the fixture scripted"
    );
    assert_eq!(result(&ask(&fixture, "second"), "q").0, "satisfied");
    assert_eq!(derivations(&fixture.data()).len(), 2);
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = ask(&fixture, "third");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "model_answer_ambiguous".to_string()),
        "a rebuild preferred the answer that worked: {inspected:?}"
    );
    rebuilding.stop();
}
