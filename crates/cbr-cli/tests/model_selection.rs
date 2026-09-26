//! **What a serving provider asks a model, and what it does with the answer.**
//!
//! The call site m4c added lives inside compiling: `select_source` ranks
//! the spans of the file an item named, and a request that authorised an
//! investigation asks a model which of them to cite. Every test here is
//! about one of the two properties that makes that safe to do:
//!
//! * **The answer cannot widen what is cited.** The model is offered a
//!   closed set of CBR's own candidates and answers with one of their ids,
//!   so the worst any answer can do is choose a worse candidate from that
//!   list.
//! * **Every failure is an item's typed unmet reason**, never a hang and
//!   never a quiet fall back to the span BM25 ranked first — which would
//!   report a model-assisted selection that no model made.
//!
//! And the one that makes it optional: a request that authorised no
//! investigation is the deterministic compiler M3 shipped, from the same
//! binary, which is what m4e's journeys are scored against.
//!
//! **No model is called.** The transport is the `model.fake` control.

use std::time::{Duration, Instant};

use cbr_encoding::Value;

mod serving;

use serving::{
    Fixture, OUTSIDE, UNASKED, bodies_sent, calls, cited_span, ledger, poll, prepared, result,
    submit,
};

#[test]
fn a_request_that_authorises_no_investigation_calls_nothing_at_all() {
    // **The deterministic path, from the same binary.** Not a second
    // build and not a configuration without a model: a model is
    // configured here and is not called, because nothing authorised a
    // call. "Background spend is zero" is this.
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "plain", "0");
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert!(
        bodies_sent(&fixture.data()).is_empty(),
        "a model was called for a request that authorised none"
    );
    provider.stop();
}

#[test]
fn the_span_cited_is_the_one_the_model_chose() {
    // The positive half, and the only way to see that the answer was
    // *used*: the same question, the same tree and the same selector, run
    // once without an investigation and once with one whose model picks
    // the second candidate rather than the first.
    let deterministic = {
        let fixture = Fixture::answering(&["choose:c2"]);
        let provider = fixture.start();
        prepared(&fixture, "plain", "0");
        let span = cited_span(&fixture, "plain");
        provider.stop();
        span
    };
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    let inspected = prepared(&fixture, "assisted", "1");
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    let assisted = cited_span(&fixture, "assisted");
    assert!(
        deterministic.contains("queue.md") && assisted.contains("queue.md"),
        "both cite the file the item named: {deterministic:?} and {assisted:?}"
    );
    assert_ne!(
        assisted, deterministic,
        "the model chose the second candidate and the packet cites the first"
    );
    assert!(
        !bodies_sent(&fixture.data()).is_empty(),
        "and a call was recorded"
    );
    provider.stop();
}

#[test]
fn an_id_that_was_never_offered_leaves_the_item_unmet() {
    // **The property the closed set exists for.** Whatever the model
    // says, CBR reads back only which of its own candidates was chosen;
    // an answer naming anything else selects nothing at all.
    //
    // There is no repair here, and that is deliberate: the answer was
    // well formed and wrong, so asking again would spend a shared quota
    // on the same question. The repair bound in `Runtime::ask` is for an
    // answer CBR could not read, which is a different thing.
    for invented in ["choose:c99", "choose:../../etc/passwd", "choose:"] {
        let fixture = Fixture::answering(&[invented]);
        let provider = fixture.start();
        let inspected = prepared(&fixture, "invented", "1");
        let (result, reason) = result(&inspected, "q");
        assert_eq!(result, "unmet", "{invented}: {inspected:?}");
        assert_eq!(
            reason, "model_choice_not_offered",
            "{invented}: {inspected:?}"
        );
        provider.stop();
    }
}

#[test]
fn a_call_that_fails_leaves_the_item_unmet_with_the_reason_it_failed_for() {
    // READINESS §8: a provider error, a refusal at admission and an
    // invalid output all end as a typed reason a consumer can read —
    // **never a hang, and never the span BM25 would have chosen**, which
    // would report a model-assisted selection no model made.
    for (answer, reason) in [
        ("failed", "model_call_failed"),
        ("not_sent", "model_not_sent"),
        ("provider_exhausted", "provider_quota_exhausted"),
        // Not JSON at all: the parser refuses it, `Runtime::ask` repairs
        // once with CBR's own sentence, the script repeats itself, and the
        // bound stops there.
        ("text:not a json object at all", "model_output_unstructured"),
        // JSON, an object, and **not a choice**: the wire was fine and
        // the answer names nothing CBR asked about. A different reason,
        // because it is a different failure.
        (r#"text:{\"choice\":\"c1\"}"#, "model_answer_not_structured"),
    ] {
        let fixture = Fixture::answering(&[answer]);
        let provider = fixture.start();
        let inspected = prepared(&fixture, "failing", "1");
        let (result, got) = result(&inspected, "q");
        assert_eq!(result, "unmet", "{answer}: {inspected:?}");
        assert_eq!(got, reason, "{answer}: {inspected:?}");
        provider.stop();
    }
}

#[test]
fn nothing_outside_the_requests_view_is_in_a_request_body() {
    // **READINESS §7's gate, over the serialized bytes rather than over
    // the selection that preceded them.** The provider has a second
    // repository registered, readable by it and named in no basis; the
    // checkout the request *is* about has a second file no item asked
    // for. Neither may reach a model.
    let fixture = Fixture::answering(&["choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "scoped", "1");
    let bodies = bodies_sent(&fixture.data());
    assert!(!bodies.is_empty(), "nothing was sent to assert over");
    for body in &bodies {
        assert!(
            !body.contains(OUTSIDE.trim()),
            "a repository outside the view reached the wire: {body}"
        );
        assert!(
            !body.contains("elsewhere.md"),
            "its path reached the wire: {body}"
        );
        assert!(
            !body.contains(UNASKED.trim()),
            "a file no item asked for reached the wire: {body}"
        );
        assert!(
            body.contains("queue.md"),
            "and the file the item did name is there: {body}"
        );
    }
    provider.stop();
}

#[test]
fn a_request_with_two_items_asks_two_questions_and_holds_both_answers() {
    // **The case one item per request never reaches.** A compile that
    // needs two calls asks across several ticks: the first answer settles
    // while the second is still running, and the compile can only finish
    // when it holds both. Two things would break it and neither shows
    // with a single item — an answer released the moment it is read, and
    // a pool key that does not distinguish the two questions.
    //
    // Two items, two files, one call each: **exactly two completions**.
    // Fewer means one answer was reused for a question it was not asked;
    // more means an answer was released and the question asked again, at
    // a second charge.
    let fixture = Fixture::answering(&["choose:c2", "choose:c3"]);
    let provider = fixture.start();
    let submitted = fixture.cbr(&[
        "context",
        "two",
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
        "2",
        "--want",
        "q=source:queue.md",
        "--want",
        "c=source:cache.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    let inspected = loop {
        let polled = fixture.cbr(&["request", "two"]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            break inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert_eq!(result(&inspected, "c").0, "satisfied", "{inspected:?}");
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        2,
        "two items, two files, two questions — and each asked once"
    );
    provider.stop();
}

#[test]
fn the_same_question_of_the_same_file_is_asked_once() {
    // The rule from the other side: **the pool key is the question, not
    // the item.** Two items asking the same thing of the same file are
    // one call, not two charges.
    //
    // The key also carries the *selector*, and that half is not tested
    // here — it cannot be, through this client. `cbr` gives every item of
    // a request the same selector, so two items on one file always ask
    // the same question. Over the protocol an item carries its own, and
    // there a key without the selector would hand the second item a
    // choice made from a candidate list it was never shown: two selectors
    // rank a file differently, so `c2` does not mean the same span to
    // both. The key carries it for that case, which this suite reaches
    // no further than saying.
    let fixture = Fixture::answering(&["choose:c2", "choose:c2"]);
    let provider = fixture.start();
    let submitted = fixture.cbr(&[
        "context",
        "same-file",
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
        "2",
        "--want",
        "q=source:queue.md",
        "--want",
        "o=source:queue.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = fixture.cbr(&["request", "same-file"]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // The CLI gives every item the request's one selector, so these two
    // ask the same question of the same file and **are** one question.
    // What this pins is that the count follows the question rather than
    // the item: one selector, one call.
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        1,
        "the same question of the same file, asked once"
    );
    provider.stop();
}

#[test]
fn a_completion_settles_to_what_the_provider_said_it_cost() {
    // **The reservation is conservative and the settlement is the
    // truth.** The reservation is the local bound, which over-states by
    // three to four times; the row that survives the call must be the
    // provider's own figure, or a shared quota is charged for something
    // nobody spent. The fixture's model says 5,000.
    let fixture = Fixture::answering(&["choose:c1"]);
    let provider = fixture.start();
    prepared(&fixture, "settling", "1");
    let rows = ledger(&fixture.data());
    let completion: Vec<_> = rows
        .iter()
        .filter(|(request, ..)| request == "settling")
        .collect();
    assert_eq!(completion.len(), 1, "one completion: {rows:?}");
    assert_eq!(
        completion[0].1, "usage",
        "settled, not left reserved: {rows:?}"
    );
    assert_eq!(
        completion[0].2, 5_000,
        "and settled at what the provider said, not at the reservation: {rows:?}"
    );
    provider.stop();
}

#[test]
fn cancelling_a_request_frees_the_bound_its_call_was_holding() {
    // **What cancellation does, where it is observable.** A settled
    // answer costs a map entry and no more, so releasing one frees
    // memory and nothing a test can see. A call *still in flight* is the
    // case with teeth: it holds one of the two slots the concurrency
    // bound allows, and cancelling the request that owns it must give
    // that slot back rather than leave the next job waiting for work
    // nobody wants.
    //
    // Two barriers hold two calls. A third request is then deferred —
    // the bound is full — and stays deferred however often it is
    // polled. Cancelling the first request frees its slot, and the third
    // call happens. Its thread is not killed, and nothing it chose
    // reaches a packet, because its answer is never read.
    let fixture = Fixture::paused_at_two();
    let provider = fixture.start();
    submit(&fixture, "first", "queue.md");
    submit(&fixture, "second", "cache.md");

    // Both calls stop at their barriers, each having sent. Three bodies:
    // the first call's count, the second's count, and the second's
    // completion — the first is held at its count's boundary and the
    // second at its completion's.
    let held = 3;
    let started = Instant::now();
    while bodies_sent(&fixture.data()).len() < held {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "two calls were never held: {:?}",
            calls(&fixture.data()),
        );
        poll(&fixture, &["first", "second"]);
        std::thread::sleep(Duration::from_millis(20));
    }

    submit(&fixture, "third", "index.md");
    for _ in 0..25 {
        poll(&fixture, &["first", "second", "third"]);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        held,
        "the third is deferred while the bound is full"
    );

    let cancelled = fixture.cbr(&["cancel", "first"]);
    assert!(
        cancelled.status.success(),
        "cancel: {}",
        String::from_utf8_lossy(&cancelled.stderr)
    );

    let started = Instant::now();
    while bodies_sent(&fixture.data()).len() <= held {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the slot the cancelled request held was never given back"
        );
        poll(&fixture, &["second", "third"]);
        std::thread::sleep(Duration::from_millis(20));
    }
    provider.stop();
}

#[test]
fn the_investigation_limit_counts_questions_and_the_budget_runs_out() {
    // **At m4c this number gated; at m4e it counts.** Two items, two
    // questions and a budget of one: the first is asked and the second
    // is not, because the budget is spent. The second item's reason
    // says so rather than the packet quietly citing the span BM25 would
    // have chosen, which is the rule m4c wrote down about reporting a
    // model-assisted selection no model made.
    //
    // Items come first because an item is what the request *required*,
    // and discovery is advisory.
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    let submitted = fixture.cbr(&[
        "context",
        "one-unit",
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
        "--want",
        "c=source:cache.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    let inspected = loop {
        let polled = fixture.cbr(&["request", "one-unit"]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            break inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    assert_eq!(
        result(&inspected, "c"),
        (
            "unmet".to_string(),
            "investigation_budget_exhausted".to_string()
        ),
        "{inspected:?}"
    );
    assert_eq!(
        bodies_sent(&fixture.data()).len(),
        1,
        "one unit of budget, one question"
    );
    provider.stop();
}

/// How many candidates one item's selection offers at most:
/// `selection::CANDIDATES` in `cbr-provider`, which its published
/// selection figure is priced for and which the call site asks retrieval
/// for. Written out because this crate launches the provider rather than
/// linking it; `selection::tests` holds the call site to the constant.
const SELECTION_CANDIDATES: usize = 8;

#[test]
fn a_selection_offers_no_more_candidates_than_its_figure_is_priced_for() {
    // **The published selection figure prices a fixed number of
    // candidates**, and what decides how many are offered is the number
    // of rows retrieval returns at the call site. A file with more spans
    // than that, every one of which answers the selector, must still be
    // offered that many and no more: one more would be a body past the
    // figure, and a stop priced on it would start runs that could cross it.
    let fixture = Fixture::dominated(&["choose:c1"]);
    let provider = fixture.start();
    serving::submit(&fixture, "wide", "dominant.md");
    let started = Instant::now();
    let inspected = loop {
        let polled = fixture.cbr(&["request", "wide"]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            break inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(result(&inspected, "q").0, "satisfied", "{inspected:?}");
    let bodies = bodies_sent(&fixture.data());
    assert_eq!(bodies.len(), 1, "one item, one question");
    let offered: Vec<usize> = (1..=4 * SELECTION_CANDIDATES)
        .filter(|n| bodies[0].contains(&format!("\\n[c{n}] dominant.md lines ")))
        .collect();
    assert_eq!(
        offered,
        (1..=SELECTION_CANDIDATES).collect::<Vec<_>>(),
        "the selection offered other than the {SELECTION_CANDIDATES} candidates its figure prices"
    );
    // **And the file had more to offer**, or this would pin nothing: the
    // file's spans, each answering the selector, outnumber the candidates.
    let file = std::fs::read_to_string(fixture.checkout.join("dominant.md")).expect("the file");
    assert!(
        file.matches("## Draining, part").count() > SELECTION_CANDIDATES,
        "the fixture has no more spans than the candidates offered"
    );
    provider.stop();
}
