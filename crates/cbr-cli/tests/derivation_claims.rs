//! **The claims half of a derivation's readable set, end to end.**
//!
//! A derivation is sealed under the job's view *and* its readable
//! claims, and every gate compares both. Until m4d there was no test in
//! which a claim existed at all, so that half was an empty list on both
//! sides of every comparison — true by vacuity. The reviewer's mutant
//! says so exactly: sealing `readable_under(view, &[])` survived every
//! test in the suite.
//!
//! At m4d a derivation named no claim, which is why that was a
//! correction rather than a leak. **At m4e claims enter the candidate
//! sets**: discovery offers a model the eligible claims beside the
//! spans, so a record's `offered` holds claim text a job could read and
//! the gate is now load-bearing rather than merely correct. The last
//! test here is that case, and it is the one the reviewer's mutant —
//! sealing `readable_under(view, &[])` — now leaks through.
//!
//! Four doors, and both arms of each: `inspect`, a listing, `fetch`, and
//! a rebuild.
//!
//! **No model is called.** The transport is the `model.fake` control.

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, answers, bodies_sent, derivations, prepared, result};

const CLAIM: &str = "drains";

/// The one derivation the store holds, with its digest.
fn only_derivation(fixture: &Fixture) -> (String, String) {
    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "one derivation: {found:?}");
    let digest = found[0]
        .1
        .get("descriptor")
        .and_then(|descriptor| descriptor.get("digest"))
        .and_then(Value::as_str)
        .expect("a digest")
        .to_string();
    (found[0].0.clone(), digest)
}

/// A grant over **both registered repositories** and the context
/// subjects, with `evidence.read` over every artifact — and the claim
/// only when `with_claim`.
///
/// Both repositories, deliberately: the job that sealed the derivation
/// belongs to the authority, whose view is every registration, so a
/// reader missing one of them is refused for *that* reason and the
/// claim half of the comparison is never reached. The only difference
/// between the two grants here is the claim.
fn grant(fixture: &Fixture, id: &str, with_claim: bool) {
    let mut rights = vec![
        "evidence.read",
        "context.request",
        "context.read",
        "context.packet.read",
    ];
    let mut resources = vec![
        r#"{"kind":"evidence.artifact"}"#.to_string(),
        r#"{"kind":"context.request"}"#.to_string(),
        r#"{"kind":"context.job"}"#.to_string(),
        r#"{"kind":"context.packet"}"#.to_string(),
        r#"{"kind":"cbr.repository","id":"app"}"#.to_string(),
        r#"{"kind":"cbr.repository","id":"outside"}"#.to_string(),
    ];
    if with_claim {
        rights.push("knowledge.read");
        resources.push(format!(r#"{{"kind":"knowledge.claim","id":"{CLAIM}"}}"#));
    }
    fixture.issue_grant(id, &rights, &resources.join(","));
}

/// A fixture with a claim in it and one model-assisted request run.
fn with_a_claim(request: &str) -> (Fixture, serving::Running) {
    let fixture = Fixture::answering(&["choose:c2"]);
    let running = fixture.start();
    fixture.propose_claim(CLAIM);
    prepared(&fixture, request, "1");
    (fixture, running)
}

#[test]
fn a_job_that_could_read_a_claim_seals_it_into_the_readable_set() {
    // The first link, and the one the reviewer's mutant breaks: if the
    // job's claims never reach the seal, every comparison below is
    // between two empty lists and passes for no reason.
    let (fixture, running) = with_a_claim("sealed");
    let (artifact, _) = only_derivation(&fixture);
    let record = derivations(&fixture.data())
        .into_iter()
        .find(|(id, _)| *id == artifact)
        .expect("the record")
        .1;
    let claims: Vec<String> = record
        .get("readable_under")
        .and_then(|under| under.get("readable_claims"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no readable set in {record:?}"))
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    assert_eq!(
        claims,
        vec![CLAIM.to_string()],
        "the job could read the claim and the record does not say so"
    );
    running.stop();
}

#[test]
fn a_reader_without_the_claim_is_refused_at_every_door() {
    let (fixture, running) = with_a_claim("gated");
    let (artifact, digest) = only_derivation(&fixture);
    grant(&fixture, "g-no-claim", false);

    let inspected = fixture.query_as_reader(
        "g-no-claim",
        "evidence.inspect",
        &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}}}}"#),
    );
    assert_eq!(
        inspected
            .get("error")
            .and_then(|error| error.get("data"))
            .and_then(|data| data.get("code"))
            .and_then(Value::as_str),
        Some("permission_denied"),
        "inspect answered: {inspected:?}"
    );

    let listed = fixture.query_as_reader("g-no-claim", "evidence.query", "{}");
    assert!(
        !listing(&listed).contains(&artifact),
        "the listing named it: {listed:?}"
    );

    let out = fixture.directory.path().join("refused.json");
    let fetched = fixture.cbr_as_reader(
        "g-no-claim",
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(!fetched.status.success(), "it was served");
    assert!(
        String::from_utf8_lossy(&fetched.stderr).contains("permission_denied"),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert!(!out.exists(), "and wrote nothing");
    running.stop();
}

#[test]
fn a_reader_with_the_claim_passes_every_door() {
    // The other arm. Without it the rule above is satisfied by refusing
    // everybody, which is trivially safe and useless.
    let (fixture, running) = with_a_claim("allowed");
    let (artifact, digest) = only_derivation(&fixture);
    grant(&fixture, "g-claim", true);

    let inspected = fixture.query_as_reader(
        "g-claim",
        "evidence.inspect",
        &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}}}}"#),
    );
    assert!(
        inspected.get("result").is_some(),
        "inspect refused a reader who could read it: {inspected:?}"
    );

    let listed = fixture.query_as_reader("g-claim", "evidence.query", "{}");
    assert!(
        listing(&listed).contains(&artifact),
        "the listing hid it: {listed:?}"
    );

    let out = fixture.directory.path().join("served.json");
    let fetched = fixture.cbr_as_reader(
        "g-claim",
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(
        fetched.status.success(),
        "fetch: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert_eq!(
        cbr_encoding::digest_bytes(&std::fs::read(&out).expect("the file")),
        digest
    );
    running.stop();
}

#[test]
fn a_rebuild_answers_a_reader_with_the_claim_and_not_one_without() {
    // The fourth door, and both its arms in one run: the same question,
    // the same repository, the same rebuild — and the only difference
    // is whether the reader's grant covers the claim the job could
    // read.
    let (fixture, running) = with_a_claim("warm");
    grant(&fixture, "g-no-claim", false);
    grant(&fixture, "g-claim", true);
    running.stop();

    let rebuilding = fixture.start_replaying();
    assert_eq!(
        reader_asks(&fixture, "g-no-claim", "narrow").1,
        "model_answer_not_retained",
        "a reader without the claim was handed the answer"
    );
    assert_eq!(
        reader_asks(&fixture, "g-claim", "wide").0,
        "satisfied",
        "a reader with the claim was refused it"
    );
    rebuilding.stop();
}

/// Submit the same question as the reader, under `grant`, and say how
/// the item ended.
fn reader_asks(fixture: &Fixture, grant: &str, request: &str) -> (String, String) {
    let submitted = fixture.cbr_as_reader(
        grant,
        &[
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
            "1",
            "--want",
            "q=source:queue.md",
        ],
    );
    assert!(
        submitted.status.success(),
        "submit {request}: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = std::time::Instant::now();
    loop {
        let polled = fixture.cbr_as_reader(grant, &["request", request]);
        let parsed = cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
            .expect("canonical JSON");
        if parsed.get("state").and_then(Value::as_str) != Some("preparing") {
            return result(&parsed, "q");
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "{request} never left preparing: {parsed:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The artifact ids a listing named.
fn listing(answered: &Value) -> Vec<String> {
    answered
        .get("result")
        .and_then(|result| result.get("items"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no items in {answered:?}"))
        .iter()
        .filter_map(|item| item.get("artifact"))
        .filter_map(|subject| subject.get("id"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// One item and both discovery steps: three questions, three units.
const BOTH_STEPS: &str = "3";

#[test]
fn a_candidate_set_that_held_a_claim_is_sealed_under_that_claim() {
    // **Where the gate stops being a formality.** Discovery offers a
    // model the eligible claims beside the spans, so the choice step's
    // record says *this claim was shown* and its `offered` holds the
    // claim's own text by digest. A reader who cannot read the claim
    // must not be handed that record — which until now was true of
    // nothing, because no record named a claim at all.
    let fixture = Fixture::answering(&["choose:c2", "terms:tombstone", "ids:k1"]);
    let running = fixture.start();
    fixture.propose_claim(CLAIM);
    prepared(&fixture, "offered", BOTH_STEPS);

    // The claim reached a model, which is the reason any of this
    // matters: its text left the store.
    assert!(
        bodies_sent(&fixture.data())
            .iter()
            .any(|body| body.contains(&format!("claim {CLAIM}"))),
        "the claim was never offered, so this test proves nothing"
    );

    let recorded = answers(&fixture);
    let choice = recorded
        .iter()
        .find(|(_, selector, _)| selector.starts_with("discovery.choose"))
        .unwrap_or_else(|| panic!("no choice record in {recorded:?}"));
    assert_eq!(
        choice.2.get("chose_ids").and_then(Value::as_array),
        Some(&[Value::String("k1".into())][..]),
        "the claim the model chose is not the one the record says: {recorded:?}"
    );

    // Every record of this request, not only the one that held the
    // claim: the readable set is the *job's*, and the job could read it
    // throughout.
    for (id, record) in derivations(&fixture.data()) {
        let claims: Vec<&str> = record
            .get("readable_under")
            .and_then(|under| under.get("readable_claims"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("no readable set on {id}"))
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(claims, vec![CLAIM], "{id} was sealed under no claim");
    }

    // And the gate holds at the door, with the claim in the set.
    grant(&fixture, "g-no-claim", false);
    let artifact = derivations(&fixture.data())
        .into_iter()
        .map(|(id, _)| id)
        .next()
        .expect("a derivation");
    let inspected = fixture.query_as_reader(
        "g-no-claim",
        "evidence.inspect",
        &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}}}}"#),
    );
    assert_eq!(
        inspected
            .get("error")
            .and_then(|error| error.get("data"))
            .and_then(|data| data.get("code"))
            .and_then(Value::as_str),
        Some("permission_denied"),
        "a reader who cannot read the claim was served a record built from it: {inspected:?}"
    );
    running.stop();
}

#[test]
fn a_binding_claim_is_carried_whether_the_model_listed_it_or_not() {
    // **A model may not unmark a binding claim by leaving it out of a
    // list.** The rule that it may never *mark* one binding is worth
    // nothing without this one: `binding` is an authority's act, and
    // INTERNALS section 5 step 3 says of this rank that losing it loses
    // the answer.
    //
    // The model is offered the claim and chooses only a span. The
    // packet carries the claim regardless, and its label is still the
    // authority's.
    let fixture = Fixture::answering(&["choose:c2", "terms:tombstone", "ids:d1"]);
    let running = fixture.start();
    fixture.propose_checkable_claim(CLAIM);
    fixture.make_binding(CLAIM);
    prepared(&fixture, "binding", BOTH_STEPS);

    let recorded = answers(&fixture);
    let choice = recorded
        .iter()
        .find(|(_, selector, _)| selector.starts_with("discovery.choose"))
        .unwrap_or_else(|| panic!("no choice record in {recorded:?}"));
    assert_eq!(
        choice.2.get("chose_ids").and_then(Value::as_array),
        Some(&[Value::String("d1".into())][..]),
        "the model chose no claim, which is what this test needs: {recorded:?}"
    );

    let printed = fixture.cbr(&["packet", "binding", "--excerpt", "1000000"]);
    assert!(
        printed.status.success(),
        "packet: {}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let inspected = cbr_encoding::parse(String::from_utf8_lossy(&printed.stdout).trim().as_bytes())
        .expect("canonical JSON");
    let sealed = cbr_encoding::parse(
        &cbr_encoding::decode_base64(
            inspected
                .get("excerpt")
                .and_then(|excerpt| excerpt.get("data_base64"))
                .and_then(Value::as_str)
                .expect("an excerpt"),
        )
        .expect("base64"),
    )
    .expect("the sealed packet");
    let carried = sealed
        .get("sections")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|section| {
            section.get("section_id").and_then(Value::as_str) == Some(&format!("d-claim-{CLAIM}"))
        })
        .cloned()
        .unwrap_or_else(|| panic!("the binding claim was dropped: {sealed:?}"));
    assert_eq!(
        carried.get("label").and_then(Value::as_str),
        Some("binding"),
        "and it is carried as something other than binding: {carried:?}"
    );
    running.stop();
}
