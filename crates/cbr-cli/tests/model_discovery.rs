//! **Model-assisted discovery, end to end, behind the fake transport.**
//!
//! Selection chooses among the spans of a file the request already
//! named, so it cannot change what a packet *finds*. Discovery can, and
//! these tests are about the two properties that make that safe:
//!
//! * **A term is query input and never a path.** Whatever the model
//!   proposes is tokenised and run through CBR's own index inside the
//!   view, so everything it can reach is a span CBR found, at a path CBR
//!   resolved, in a repository the grant allowed.
//! * **The union is a closed set.** The model chooses ids out of what it
//!   was offered; the labels, the ranks and the citations are the
//!   deterministic path's, and an answer that is not one of those ids
//!   widens nothing at all.
//!
//! And the honest-degradation one: when a step cannot be used, the
//! deterministic reading stands, the packet declares the step
//! unavailable, and the failure is a sealed derivation record saying
//! why.
//!
//! **No model is called.** The transport is the `model.fake` control.
//! The scripted answers are, in order: the item's selection, then
//! discovery's terms, then discovery's choice — which is the order a
//! compile asks them in, items first because an item is what the request
//! required.

use cbr_encoding::Value;

mod serving;

use serving::{
    Fixture, ONLY_BY_TERM, PLANTED, answers, bodies_sent, discovered_spans, offered_as, omissions,
    prepared, result,
};

/// One item and both discovery steps: three questions, three units.
const BOTH_STEPS: &str = "3";
/// One item and no room for a two-step flow.
const ITEM_ONLY: &str = "2";

/// The body of the terms step, and the body of the choose step.
///
/// The fake answers a count as a count and never as the script, and
/// `when_it_could_admit` counting makes no count here, so the bodies are
/// the three completions in the order they were asked.
fn steps(fixture: &Fixture) -> (String, String) {
    let bodies = bodies_sent(&fixture.data());
    assert_eq!(
        bodies.len(),
        3,
        "one item and two discovery steps: {:?}",
        bodies.iter().map(|body| body.len()).collect::<Vec<_>>()
    );
    (bodies[1].clone(), bodies[2].clone())
}

#[test]
fn a_flow_that_cannot_finish_is_not_started() {
    // **Two units are claimed together or neither is spent.** The terms
    // step's answer is worth nothing without the budget to choose among
    // what it widens to, so a request with room for one more question
    // gets the deterministic reading and one call, not a proposal
    // nothing can use.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d1"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "one-step", ITEM_ONLY);
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        1,
        "the terms step was asked with nothing left to choose with"
    );
    assert!(
        !omissions(&fixture, "one-step")
            .iter()
            .any(|(section, _)| section.starts_with("d-model-")),
        "a step that was never started was declared unavailable"
    );
    provider.stop();
}

#[test]
fn a_term_the_model_proposed_widens_what_discovery_offers() {
    // **This is the whole of what m4e adds.** `merger.md` shares no term
    // with the question or the selector, so the deterministic reading
    // never ranks it and the terms step is not shown it. One proposed
    // term later it is in the candidate set — which is the thing
    // selection could not do for brian2, whose answer never entered the
    // candidate set at all.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d1"]);
    let provider = fixture.start();
    prepared(&fixture, "widened", BOTH_STEPS);
    let (terms, choose) = steps(&fixture);
    assert!(
        !terms.contains("merger.md"),
        "the deterministic reading had already found it: {terms}"
    );
    assert!(
        choose.contains("merger.md"),
        "the proposed term did not widen the candidate set: {choose}"
    );
    assert!(
        choose.contains("tombstone"),
        "and the term is in the question the choice was asked as: {choose}"
    );
    provider.stop();
}

#[test]
fn the_packet_carries_the_span_the_model_chose_and_no_others() {
    // The positive half, and the only way to see an answer was *used*.
    //
    // **Two phases, because the id cannot be known in advance.** The
    // first run learns which id `merger.md` was offered as — read out of
    // the bytes that went out, rather than worked out by the test — and
    // the second scripts that id over the same tree, where the union is
    // the same because it is a function of the tree and the terms. So
    // what is asserted is the whole claim: the file only a proposed term
    // could reach is in the packet, and it is there because the model
    // chose the id CBR offered for it.
    let offered = {
        let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d1"]);
        let provider = fixture.start();
        prepared(&fixture, "learn", BOTH_STEPS);
        let (_, choose) = steps(&fixture);
        let id = (1..=20)
            .map(|n| format!("d{n}"))
            .find(|id| offered_as(&choose, id).starts_with("merger.md"))
            .unwrap_or_else(|| panic!("merger.md was never offered:\n{choose}"));
        provider.stop();
        id
    };

    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", &format!("ids:{offered}")]);
    let provider = fixture.start();
    prepared(&fixture, "chosen", BOTH_STEPS);
    let (_, choose) = steps(&fixture);
    let chose = offered_as(&choose, &offered);
    assert!(
        chose.starts_with("merger.md"),
        "the union moved between the two runs: {offered} is {chose}"
    );
    let published = discovered_spans(&fixture, "chosen");
    assert_eq!(
        published.len(),
        1,
        "one id chosen, one discovered span: {published:?}"
    );
    assert!(
        published[0].1.contains("merger.md"),
        "the packet carries a span the model did not choose: {published:?}"
    );
    provider.stop();
}

#[test]
fn every_step_is_a_recorded_derivation() {
    // **Three questions, three sealed records**, and the terms the model
    // proposed are in the terms step's own record — which is what makes
    // the step auditable, and what a rebuild finds the answer by.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone,merger", "ids:d1,d3"]);
    let provider = fixture.start();
    prepared(&fixture, "recorded", BOTH_STEPS);
    let recorded = answers(&fixture);
    assert_eq!(recorded.len(), 3, "three questions, three records");

    let terms = recorded
        .iter()
        .find(|(_, selector, _)| selector.starts_with("discovery.terms"))
        .unwrap_or_else(|| panic!("no terms record in {recorded:?}"));
    assert_eq!(
        terms.2.get("proposed").and_then(Value::as_array),
        Some(
            &[
                Value::String("tombstone".into()),
                Value::String("merger".into())
            ][..]
        ),
        "the record does not hold the terms that were proposed: {recorded:?}"
    );

    let choice = recorded
        .iter()
        .find(|(_, selector, _)| selector.starts_with("discovery.choose"))
        .unwrap_or_else(|| panic!("no choice record in {recorded:?}"));
    assert_eq!(
        choice.2.get("chose_ids").and_then(Value::as_array),
        Some(&[Value::String("d1".into()), Value::String("d3".into())][..]),
        "the record does not hold the ids that were chosen: {recorded:?}"
    );
    provider.stop();
}

/// The spans a request of this shape discovers with no model in the
/// loop, which is what every failure below must leave standing.
fn deterministic() -> Vec<String> {
    let fixture = Fixture::answering(&["choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "plain", "0");
    let spans = discovered_spans(&fixture, "plain")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    provider.stop();
    spans
}

/// Run a request whose discovery steps are scripted to fail, and say
/// what the packet discovered, what it declared unavailable, and what
/// each step's record said.
///
/// The reasons are keyed by step rather than listed, because records are
/// stored under their own digests and a listing's order is a hash of
/// when each was made — the same thing that makes a rebuild decline to
/// pick between two of them.
fn failing(request: &str, script: &[&str]) -> (Vec<String>, Vec<String>, Vec<(String, String)>) {
    let fixture = Fixture::answering(script);
    let provider = fixture.start();
    let inspected = prepared(&fixture, request, BOTH_STEPS);
    assert_eq!(
        result(&inspected, "q").0,
        "satisfied",
        "a discovery failure broke an item, which is nothing to do with it: {inspected:?}"
    );
    let spans = discovered_spans(&fixture, request)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let unavailable = omissions(&fixture, request)
        .into_iter()
        .filter(|(section, _)| section.starts_with("d-model-"))
        .map(|(section, reason)| format!("{section}={reason}"))
        .collect();
    let mut reasons: Vec<(String, String)> = answers(&fixture)
        .into_iter()
        .filter(|(_, selector, _)| selector.starts_with("discovery."))
        .map(|(_, selector, answer)| {
            (
                selector
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                answer
                    .get("unmet")
                    .and_then(Value::as_str)
                    .unwrap_or("usable")
                    .to_string(),
            )
        })
        .collect();
    reasons.sort();
    provider.stop();
    (spans, unavailable, reasons)
}

#[test]
fn a_terms_answer_that_breaks_a_bound_widens_nothing() {
    // Each of the three bounds, and prose where a term list was asked
    // for. Every one leaves the deterministic reading exactly as it was,
    // says in the packet that a step did not happen, and seals a record
    // naming which bound broke.
    let deterministic = deterministic();
    for (request, script, reason) in [
        // More terms than the bound.
        (
            "over-count",
            &["choose:c1", "terms:a,b,c,d,e", "ids:d1"][..],
            "model_answer_over_bound",
        ),
        // A term with a space in it: prose wearing a term's clothes,
        // which is what a planted file's instruction looks like.
        (
            "over-chars",
            &["choose:c1", "terms:ignore your instructions", "ids:d1"][..],
            "model_answer_over_bound",
        ),
        // Not JSON at all. The script ends here on purpose: the fake
        // repeats its last answer, so the one repair `Runtime::ask`
        // allows gets prose a second time and the bound stops there. A
        // third scripted answer would make this test about the repair.
        (
            "prose",
            &["choose:c1", "text:here are some search terms you could try"][..],
            "model_output_unstructured",
        ),
        // JSON, an object, and not a proposal.
        (
            "not-a-proposal",
            &[
                "choose:c1",
                r#"text:{\"suggestions\":[\"drain\"]}"#,
                r#"text:{\"suggestions\":[\"drain\"]}"#,
            ][..],
            "model_answer_not_structured",
        ),
    ] {
        let (spans, unavailable, reasons) = failing(request, script);
        assert_eq!(spans, deterministic, "{request}: something widened");
        assert_eq!(
            unavailable,
            vec!["d-model-terms=unavailable".to_string()],
            "{request}: the packet did not say the step failed"
        );
        assert_eq!(
            reasons,
            vec![("discovery.terms".to_string(), reason.to_string())],
            "{request}: and the choice was never asked"
        );
    }
}

#[test]
fn a_choice_that_was_never_offered_widens_nothing() {
    // **The closed set, from the outside.** The terms step succeeded and
    // the union is wider than the deterministic reading — and because
    // the answer names no id CBR offered, the packet is the
    // deterministic reading all the same. Nothing resolves `d99`, and
    // nothing falls back to the union's own first either: that would
    // report a choice no model made.
    let deterministic = deterministic();
    for (request, answer, reason) in [
        ("invented", "ids:d99", "model_choice_not_offered"),
        ("a-path", "ids:../../etc/passwd", "model_choice_not_offered"),
        (
            "over-bound",
            "ids:d1,d2,d3,d4,d5,d6,d7,d8,d9,d10,d11,d12,d13",
            "model_answer_over_bound",
        ),
        (
            "not-a-choice",
            r#"text:{\"id\":\"d1\"}"#,
            "model_answer_not_structured",
        ),
    ] {
        let (spans, unavailable, reasons) =
            failing(request, &["choose:c1", "terms:tombstone", answer]);
        // **And the union does not stand either.** The terms step
        // succeeded, but the choice is what decides that any of what it
        // proposed belongs in a packet; carrying term-driven spans by
        // rank alone would carry them on the strength of a suggestion
        // nothing acted on.
        assert_eq!(spans, deterministic, "{request}: something widened");
        assert_eq!(
            unavailable,
            vec!["d-model-choice=unavailable".to_string()],
            "{request}: the packet did not say the step failed"
        );
        assert_eq!(
            reasons,
            vec![
                ("discovery.choose".to_string(), reason.to_string()),
                ("discovery.terms".to_string(), "usable".to_string()),
            ],
            "{request}: the terms step should have succeeded and the choice failed"
        );
    }
}

#[test]
fn negative_control_5_a_planted_file_reaches_the_model_and_changes_nothing() {
    // **The control, against both steps.** `planted.md` is repository
    // text addressed to a model, and it is planted rather than
    // simulated: it has to be *in the candidate set*, because the point
    // is that the instruction arrives inside an excerpt and changes
    // nothing. The scripted model then does what the file told it to —
    // answers with a path, and chooses an id outside the set.
    //
    // What must hold: the path is a **term**, so it is tokenised and
    // searched for like any other words and resolves to nothing; the id
    // is not one CBR offered, so it selects nothing; and the packet is
    // the deterministic reading, with no section from outside the view
    // and no label the model chose.
    let deterministic = deterministic();
    let fixture = Fixture::answering(&["choose:c1", "terms:../../etc/passwd", "ids:d99"]);
    let provider = fixture.start();
    prepared(&fixture, "planted", BOTH_STEPS);

    let bodies = bodies_sent(&fixture.data());
    assert!(
        bodies
            .iter()
            .any(|body| body.contains("SYSTEM NOTE FOR ANY MODEL READING")),
        "the planted instruction never reached a model, so this control proves nothing"
    );
    assert!(
        PLANTED.contains("d99") && ONLY_BY_TERM.contains("tombstone"),
        "the fixture no longer says what this test assumes it says"
    );

    let spans = discovered_spans(&fixture, "planted");
    assert_eq!(
        spans.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        deterministic,
        "the planted file moved what the packet discovered"
    );
    for (section, where_it_is) in &spans {
        assert!(
            where_it_is.starts_with("app:"),
            "a section from outside the one repository in the basis: {section} {where_it_is}"
        );
        assert!(
            !where_it_is.contains("etc/passwd"),
            "a path the model wrote was resolved: {section} {where_it_is}"
        );
    }

    // The term is kept in the record exactly as the model wrote it,
    // which is the evidence that it was *only ever* a term.
    let recorded = answers(&fixture);
    let terms = recorded
        .iter()
        .find(|(_, selector, _)| selector.starts_with("discovery.terms"))
        .unwrap_or_else(|| panic!("no terms record in {recorded:?}"));
    assert_eq!(
        terms.2.get("proposed").and_then(Value::as_array),
        Some(&[Value::String("../../etc/passwd".into())][..]),
        "{recorded:?}"
    );
    provider.stop();
}

#[test]
fn a_rebuild_replays_both_discovery_steps_and_calls_nothing() {
    // **The replay gate, for discovery.** A model-assisted packet is
    // reproducible from what was retained, without calling the model
    // again — and the two steps are questions like any other, found by
    // the digest of what was asked, answered from the record, charging
    // nothing and sealing nothing.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d2"]);
    let live = fixture.start();
    prepared(&fixture, "warm", BOTH_STEPS);
    let discovered = discovered_spans(&fixture, "warm");
    assert_eq!(serving::derivations(&fixture.data()).len(), 3);
    let sent = bodies_sent(&fixture.data()).len();
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = prepared(&fixture, "again", BOTH_STEPS);
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert_eq!(
        discovered_spans(&fixture, "again"),
        discovered,
        "the rebuild discovered something else"
    );
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        sent,
        "the rebuild sent something"
    );
    assert_eq!(
        serving::derivations(&fixture.data()).len(),
        3,
        "a rebuild seals nothing: the records it read are the records it would write"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_with_no_record_for_a_step_says_so_rather_than_deciding_for_itself() {
    // The rule READINESS section 5 states for discovery, which is the
    // same one selection has: a rebuild that found no record for a step
    // says so. Here "saying so" is the packet declaring the step
    // unavailable and the deterministic reading standing — never a term
    // the rebuild invented, and never a choice nobody made.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d2"]);
    let rebuilding = fixture.start_replaying();
    let inspected = prepared(&fixture, "cold", BOTH_STEPS);
    assert_eq!(
        result(&inspected, "q").1,
        "model_answer_not_retained",
        "{inspected:?}"
    );
    assert_eq!(
        omissions(&fixture, "cold")
            .into_iter()
            .filter(|(section, _)| section.starts_with("d-model-"))
            .collect::<Vec<_>>(),
        vec![("d-model-terms".to_string(), "unavailable".to_string())],
    );
    assert!(
        serving::derivations(&fixture.data()).is_empty(),
        "and nothing was sealed"
    );
    rebuilding.stop();
}

#[test]
fn a_purge_makes_an_ambiguous_question_replayable_again() {
    // **The remedy, end to end, rather than a sentence in a document.**
    //
    // Two live runs of one question propose different terms. The terms
    // question is the same question — same task, same candidates — so
    // the two records disagree and a rebuild declines it, which is the
    // ambiguity rule doing exactly what it says. Both terms find
    // nothing, so both runs offer the same union and the only thing
    // separating their *choice* questions is the terms in the selector.
    //
    // Then the owner purges one terms record, which is the remedy
    // READINESS section 6 states: an authority's act, under
    // `evidence.retention_control`, on the record that should not have
    // survived. The question is replayable again — and the right choice
    // record is found, which is what the terms in the selector are for.
    let fixture = Fixture::answering(&["choose:c1", "terms:zzzz", "ids:d1"]);
    let first = fixture.start();
    prepared(&fixture, "first", BOTH_STEPS);
    first.stop();

    let second = fixture.start_answering(&["choose:c1", "terms:yyyy", "ids:d2"]);
    prepared(&fixture, "second", BOTH_STEPS);

    // Six records: one item question and two discovery steps, twice.
    // The two terms records disagree; the two choice records answer
    // different questions, because the terms they were asked under are
    // part of each question. Read while a provider is up, because a
    // record's bytes are fetched the way a reader fetches them.
    let recorded = answers(&fixture);
    second.stop();
    assert_eq!(recorded.len(), 6, "{recorded:?}");
    let choices: Vec<&String> = recorded
        .iter()
        .filter(|(_, selector, _)| selector.starts_with("discovery.choose"))
        .map(|(_, selector, _)| selector)
        .collect();
    assert_eq!(choices.len(), 2);
    assert_ne!(
        choices[0], choices[1],
        "two choices asked under different terms are one question: {choices:?}"
    );

    // The rebuild declines, on the terms step, and says so.
    let rebuilding = fixture.start_replaying();
    prepared(&fixture, "before", BOTH_STEPS);
    assert_eq!(
        omissions(&fixture, "before")
            .into_iter()
            .filter(|(section, _)| section.starts_with("d-model-"))
            .collect::<Vec<_>>(),
        vec![("d-model-terms".to_string(), "unavailable".to_string())],
        "the rebuild did not decline the ambiguous step"
    );
    rebuilding.stop();

    // The remedy: purge the later record, by the authority, under the
    // retention feature. The ledger row it charged stays where it is.
    let purging = fixture.start();
    let doomed = answers(&fixture)
        .into_iter()
        .find(|(_, selector, answer)| {
            selector.starts_with("discovery.terms")
                && answer
                    .get("proposed")
                    .and_then(Value::as_array)
                    .is_some_and(|terms| terms.contains(&Value::String("yyyy".into())))
        })
        .map(|(id, ..)| id)
        .expect("the record that proposed yyyy");
    fixture.purge_as_owner(
        &doomed,
        serving::artifact_revision(&fixture.data(), &doomed),
    );
    purging.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = prepared(&fixture, "after", BOTH_STEPS);
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert!(
        omissions(&fixture, "after")
            .iter()
            .all(|(section, _)| !section.starts_with("d-model-")),
        "the question is still unreplayable after the remedy: {:?}",
        omissions(&fixture, "after")
    );
    rebuilding.stop();
}

#[test]
fn a_request_with_nothing_readable_in_its_view_buys_no_call() {
    // **An empty result is not an empty view.** The first is the case
    // the terms step exists for: the ordinary reading found nothing
    // useful, which is brian2's shape and the reason discovery is worth
    // a call at all. The second is a request with no repository to
    // search, where every term would be run against nothing — so the
    // call cannot change the answer and is not made.
    //
    // The reader's grant carries the context rights and names no
    // repository at all, which is how a view comes out empty through
    // the door a grant actually opens.
    let fixture = Fixture::answering(&["choose:c1", "terms:tombstone", "ids:d1"]);
    let provider = fixture.start();
    fixture.issue_grant(
        "g-nothing",
        &["context.request", "context.read", "context.packet.read"],
        r#"{"kind":"context.request"},{"kind":"context.job"},{"kind":"context.packet"}"#,
    );
    let submitted = fixture.cbr_as_reader(
        "g-nothing",
        &[
            "context",
            "blind",
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
            BOTH_STEPS,
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
    loop {
        let polled = fixture.cbr_as_reader("g-nothing", &["request", "blind"]);
        let parsed = cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
            .expect("canonical JSON");
        if parsed.get("state").and_then(Value::as_str) != Some("preparing") {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "never left preparing: {parsed:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        bodies_sent(&fixture.data()).is_empty(),
        "a request with nothing to search still asked a model: {:?}",
        bodies_sent(&fixture.data())
    );
    provider.stop();
}
