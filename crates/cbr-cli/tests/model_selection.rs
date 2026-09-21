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

use std::path::Path;
use std::time::{Duration, Instant};

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, OUTSIDE, UNASKED};

/// Submit one request and poll until it is no longer preparing.
fn prepared(fixture: &Fixture, request: &str, investigation: &str) -> Value {
    let submitted = fixture.cbr(&[
        "context",
        request,
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
        investigation,
        "--want",
        "q=source:queue.md",
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = fixture.cbr(&["request", request]);
        assert!(
            polled.status.success(),
            "inspect: {}",
            String::from_utf8_lossy(&polled.stderr)
        );
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            return inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{request} never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// **Where the packet says the cited span is**, which is the first line
/// of the section's own content: `app:queue.md lines 21-40 at tree ...`.
/// The span is not a member of the item; it is what the section says
/// about itself, and that is the thing a consumer reads.
fn cited_span(fixture: &Fixture, request: &str) -> String {
    let printed = fixture.cbr(&["packet", request, "--excerpt", "1000000"]);
    assert!(
        printed.status.success(),
        "packet: {}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let packet = cbr_encoding::parse(String::from_utf8_lossy(&printed.stdout).trim().as_bytes())
        .expect("canonical JSON");
    let data = packet
        .get("excerpt")
        .and_then(|excerpt| excerpt.get("data_base64"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no excerpt in {packet:?}"));
    let sealed = cbr_encoding::parse(&cbr_encoding::decode_base64(data).expect("base64"))
        .expect("the sealed packet is canonical JSON");
    sealed
        .get("sections")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|section| section.get("section_id").and_then(Value::as_str) == Some("s-q"))
        .and_then(|section| section.get("content"))
        .and_then(Value::as_str)
        .and_then(|content| content.lines().next())
        .unwrap_or_else(|| panic!("no section s-q in {sealed:?}"))
        .to_string()
}

/// The item's result, and its reason when it has one.
fn result(inspected: &Value, item_id: &str) -> (String, String) {
    let item = inspected
        .get("items")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|item| item.get("item_id").and_then(Value::as_str) == Some(item_id))
        .cloned()
        .unwrap_or_else(|| panic!("no item {item_id} in {inspected:?}"));
    (
        item.get("result")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        item.get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}

/// Every request body the provider recorded, as text.
fn bodies_sent(data: &Path) -> Vec<String> {
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let mut statement = connection
        .prepare("SELECT sent FROM model_calls ORDER BY id")
        .expect("the recordings table exists");
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .expect("queries")
        .map(|row| String::from_utf8_lossy(&row.expect("row")).to_string())
        .collect()
}

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
