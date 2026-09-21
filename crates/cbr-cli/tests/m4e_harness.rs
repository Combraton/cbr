//! **The live-run harness, run end to end before it is ever run live.**
//!
//! `scripts/m4e_run.py` is the first live run written down as code
//! rather than typed on the day, and the reason is the one m4b already
//! learned about `--calibrate`: *if the first live call needs new
//! plumbing, the first live call runs code nobody reviewed.* So the
//! harness exists now, its dry-run mode drives every stage against the
//! `model.fake` control, and this is the test that says the stages work.
//!
//! What is exercised here: the refusals it makes before anything is
//! spent, a whole run through submit, packet, ledger and the replay
//! gate, and the gate's report of what could not be replayed.
//!
//! **No model is called and no socket is opened.** The dry run writes a
//! conformance configuration carrying the fake, which a production
//! launch refuses by name.

use std::path::{Path, PathBuf};
use std::process::Command;

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, prepared, result};

/// The repository root, from where this crate's manifest is.
fn root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn script() -> PathBuf {
    let script = root().join("scripts").join("m4e_run.py");
    assert!(
        script.exists(),
        "{} is not where the harness lives",
        script.display()
    );
    script
}

fn binaries() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path
}

/// Run the harness, and hand back (status, stdout, stderr).
fn harness(arguments: &[&str]) -> (bool, String, String) {
    let output = Command::new("python3")
        .arg(script())
        .args(arguments)
        .args(["--binaries", binaries().to_str().expect("utf-8")])
        .output()
        .expect("python3 runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// A manifest naming one run over `checkout`, written where the harness
/// can read it.
fn manifest(directory: &Path, checkout: &Path, repository: &str, model: &str) -> PathBuf {
    let body = format!(
        r#"{{"runs":[{{"id":"dry","repository":{{"id":"{repository}","path":"{checkout}"}},
             "model":"{model}","task":"what drains the queue",
             "selector":"queue drains shutdown","capacity":65536,"investigation":3,
             "wants":["q=source:queue.md"],
             "dry_answers":["choose:c2","terms:tombstone","ids:d1"]}}]}}"#,
        checkout = checkout.display()
    );
    let path = directory.join(format!("{repository}-{model}.json"));
    std::fs::write(&path, body).expect("manifest");
    path
}

#[test]
fn the_harness_refuses_every_run_it_should_before_anything_is_spent() {
    // **Each of these is a rule the owner set, made into a refusal
    // rather than a sentence in a document.** They are checked before a
    // provider is launched, so a run that breaks one costs nothing.
    let fixture = Fixture::answering(&["choose:c1"]);
    let directory = fixture.directory.path();
    let out = directory.join("out");
    let good = manifest(directory, &fixture.checkout, "cbr", "MiniMax-M3");

    // A repository outside the three the owner's word covers.
    let private = manifest(
        directory,
        &fixture.checkout,
        "someones-private-repo",
        "MiniMax-M3",
    );
    let (ok, _, stderr) = harness(&[
        "--manifest",
        private.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a private repository was admitted");
    assert!(
        stderr.contains("owner's explicit word"),
        "and not for that reason: {stderr}"
    );

    // A model outside the three M4 may name.
    let other = manifest(directory, &fixture.checkout, "cbr", "some-other-model");
    let (ok, _, stderr) = harness(&[
        "--manifest",
        other.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a model outside the three was admitted");
    assert!(stderr.contains("three"), "{stderr}");

    // An output directory inside this repository. A pilot's packet
    // holds a third party's text and must never be committed.
    let inside = root().join("target").join("m4e-out");
    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        inside.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(
        !ok,
        "an output directory inside the repository was admitted"
    );
    assert!(stderr.contains("never be committed"), "{stderr}");

    // `--live` without the permit.
    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--live",
    ]);
    assert!(!ok, "a live run was admitted without the permit");
    assert!(
        stderr.contains("--permit-model-network"),
        "and not for that reason: {stderr}"
    );

    // A ceiling above m4e's hard cap.
    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--live",
        "--permit-model-network",
        "--run-ceiling",
        "6000000",
    ]);
    assert!(!ok, "a ceiling above the hard cap was admitted");
    assert!(stderr.contains("hard cap"), "{stderr}");

    // And neither mode at all.
    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    assert!(!ok, "a run that was neither live nor dry was admitted");
    assert!(stderr.contains("nobody asked for"), "{stderr}");

    assert!(
        !out.exists(),
        "a refused run created its output directory anyway"
    );
}

#[test]
fn a_dry_run_drives_every_stage_and_reports_what_it_found() {
    // **The whole harness, once.** Submit, settle, write the packet,
    // read the ledger, relaunch as a rebuild, compare digests. If any
    // stage of this is wrong, it is wrong now rather than on the day the
    // owner's quota is being spent.
    let fixture = Fixture::answering(&["choose:c1"]);
    let directory = fixture.directory.path();
    let out = directory.join("out");
    let manifest = manifest(
        directory,
        &fixture.checkout,
        "cbr",
        "MiniMax-M2.7-highspeed",
    );

    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        manifest.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(ok, "the dry run failed:\n{stdout}\n{stderr}");

    let report: Value =
        cbr_encoding::parse(&std::fs::read(out.join("report.json")).expect("a report was written"))
            .expect("the report is JSON");
    let runs = report
        .get("runs")
        .and_then(Value::as_array)
        .expect("runs in the report");
    assert_eq!(runs.len(), 1, "{report:?}");
    let run = &runs[0];

    // Three questions were asked and three records kept: the item's
    // selection and discovery's two steps.
    assert_eq!(
        run.get("records"),
        Some(&Value::Int(3)),
        "the run did not retain a record per question: {run:?}"
    );
    assert!(
        matches!(run.get("tokens"), Some(Value::Int(tokens)) if *tokens > 0),
        "nothing was charged, so nothing was asked: {run:?}"
    );
    assert_eq!(
        run.get("items")
            .and_then(Value::as_array)
            .and_then(<[Value]>::first)
            .and_then(|item| item.get("result"))
            .and_then(Value::as_str),
        Some("satisfied"),
        "{run:?}"
    );

    // **The replay gate.** The same packet, rebuilt offline from the
    // records, byte for byte.
    let replay = run.get("replay").expect("a replay stage");
    assert_eq!(
        replay.get("sections_identical"),
        Some(&Value::Bool(true)),
        "the rebuild did not reproduce the packet's content: {replay:?}"
    );
    assert_eq!(
        replay.get("ambiguous").and_then(Value::as_array),
        Some(&[][..]),
        "a run nobody repeated has an ambiguous question: {replay:?}"
    );
    assert_eq!(report.get("ambiguous_questions"), Some(&Value::Int(0)));

    // The estimate, the stop and the ceiling are in the report, so a
    // reader of the report knows what the run was bounded by.
    assert_eq!(report.get("estimate_tokens"), Some(&Value::Int(1_500_000)));
    assert_eq!(report.get("stop_tokens"), Some(&Value::Int(2_250_000)));
    assert_eq!(
        report.get("run_ceiling_tokens"),
        Some(&Value::Int(5_000_000))
    );

    // The packet is written where the reviewer can read it, and it is
    // not in this repository.
    let packet = run
        .get("packet_file")
        .and_then(Value::as_str)
        .expect("a packet file");
    assert!(Path::new(packet).exists(), "{packet} was not written");

    // **And the report holds no repository text.** For the pilots the
    // licence permits committing digests, paths, spans, counts and
    // costs, and nothing else; the report is the thing most likely to
    // be pasted somewhere, so the rule is a test rather than a habit.
    let text = std::fs::read_to_string(out.join("report.json")).expect("the report");
    assert!(
        !text.contains("The queue drains on shutdown"),
        "the report carries repository text"
    );
    assert!(
        !text.contains("tombstone"),
        "the report carries repository text"
    );
    assert!(
        text.contains("remedy_for_an_ambiguous_question"),
        "the report does not say what an operator does about an ambiguous question"
    );
}

#[test]
fn the_replay_gate_reports_an_ambiguous_question_rather_than_skipping_it() {
    // **The consequence of the ambiguity rule, planned for.** In a live
    // run a call that timed out and a successful rerun of the same
    // question leave two records that disagree, and that question is
    // unreplayable from then on. A gate that reported it as "nothing
    // retained" would hide the one failure an operator can act on.
    //
    // The two halves are pinned together here: the provider's own
    // rebuild declines the question, and the harness names it, on the
    // same store. If the agreement rule ever moved in one and not the
    // other, this fails.
    let fixture = Fixture::answering(&["choose:c2", "choose:c1"]);
    let live = fixture.start();
    prepared(&fixture, "first", "1");
    prepared(&fixture, "second", "1");
    assert_eq!(
        serving::derivations(&fixture.data()).len(),
        2,
        "two records that disagree"
    );
    live.stop();

    let rebuilding = fixture.start_replaying();
    let inspected = prepared(&fixture, "third", "1");
    assert_eq!(
        result(&inspected, "q"),
        ("unmet".to_string(), "model_answer_ambiguous".to_string()),
        "the provider's rebuild did not decline: {inspected:?}"
    );
    rebuilding.stop();

    let (ok, stdout, stderr) = harness(&["--ambiguity", fixture.data().to_str().expect("utf-8")]);
    assert!(ok, "the gate failed:\n{stdout}\n{stderr}");
    let found: Value = cbr_encoding::parse(stdout.trim().as_bytes()).expect("JSON");
    let ambiguous = found
        .get("ambiguous")
        .and_then(Value::as_array)
        .expect("an ambiguous list");
    assert_eq!(
        ambiguous.len(),
        1,
        "the gate skipped the question the rebuild declined: {found:?}"
    );
    assert_eq!(
        ambiguous[0]
            .get("records")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(2),
        "and did not name both records: {found:?}"
    );
    assert_eq!(
        ambiguous[0]
            .get("answers")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(2),
        "and did not say what they disagreed about: {found:?}"
    );
    assert!(
        found
            .get("remedy")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("evidence.purge"),
        "and did not say what the operator does about it: {found:?}"
    );
}
