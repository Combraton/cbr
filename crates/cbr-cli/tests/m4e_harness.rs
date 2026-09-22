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
//! gate, the ceiling each launch is actually given, and the gate's
//! report of what could not be replayed.
//!
//! **No model is called and no socket is opened.** The dry run writes a
//! conformance configuration carrying the fake, which a production
//! launch refuses by name.

use std::path::{Path, PathBuf};
use std::process::Command;

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, prepared, result};

/// What a checkout of this repository says it is. The harness reads a
/// checkout's origin rather than trusting the id a manifest typed, so a
/// fixture standing in for `cbr` carries `cbr`'s origin — which is the
/// check, made against the fixture rather than around it.
const CBR_ORIGIN: &str = "https://github.com/Combraton/cbr.git";

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

/// One run of the manifest, as its JSON.
fn run_of(id: &str, checkout: &Path, repository: &str, model: &str, extra: &str) -> String {
    format!(
        r#"{{"id":"{id}","repository":{{"id":"{repository}","path":"{checkout}"{extra}}},
           "model":"{model}","task":"what drains the queue",
           "selector":"queue drains shutdown","capacity":65536,"investigation":3,
           "wants":["q=source:queue.md"],
           "dry_answers":["choose:c2","terms:tombstone","ids:d1"]}}"#,
        checkout = checkout.display()
    )
}

/// A manifest of `runs`, written where the harness can read it.
fn written(directory: &Path, name: &str, runs: &[String]) -> PathBuf {
    let path = directory.join(format!("{name}.json"));
    std::fs::write(&path, format!(r#"{{"runs":[{}]}}"#, runs.join(","))).expect("manifest");
    path
}

/// A manifest naming one run over `checkout`.
fn manifest(directory: &Path, checkout: &Path, repository: &str, model: &str) -> PathBuf {
    written(
        directory,
        &format!("{repository}-{model}"),
        &[run_of("dry", checkout, repository, model, "")],
    )
}

/// A fixture whose checkout carries `cbr`'s origin, which is what the
/// harness identifies a repository by.
fn standing_in_for_cbr() -> Fixture {
    let fixture = Fixture::answering(&["choose:c1"]);
    serving::set_origin(&fixture.checkout, CBR_ORIGIN);
    fixture
}

#[test]
fn the_harness_refuses_every_run_it_should_before_anything_is_spent() {
    // **Each of these is a rule the owner set, made into a refusal
    // rather than a sentence in a document.** They are checked before a
    // provider is launched, so a run that breaks one costs nothing.
    let fixture = standing_in_for_cbr();
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

    // **A ceiling above m4e's hard cap, and a stop above its stop.**
    // Neither refusal has anything to do with `--live`: the cap is the
    // cap in both modes, which is why this can be asserted on every
    // machine rather than only where a Keychain cannot be read.
    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
        "--run-ceiling",
        "6000000",
    ]);
    assert!(!ok, "a ceiling above the hard cap was admitted");
    assert!(stderr.contains("hard cap"), "{stderr}");

    let (ok, _, stderr) = harness(&[
        "--manifest",
        good.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
        "--stop-tokens",
        "3000000",
    ]);
    assert!(!ok, "a stop above the stop was admitted");
    assert!(stderr.contains("estimate plus half"), "{stderr}");

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
#[cfg(not(target_os = "macos"))]
fn the_two_refusals_that_can_only_be_asked_with_the_live_flag() {
    // **Gated off macOS, with every other test that names the permit
    // flag.** `--live` makes the harness write a `model_runtime`
    // configuration, and a production launch carrying one plus
    // `--permit-model-network` reads the owner's Keychain. These two
    // refusals happen inside `check` before a provider is launched, so
    // nothing is read here — and "before" is a property of a statement
    // order, which is exactly the kind of thing that should not be the
    // only thing standing between a test and the owner's key.
    let fixture = standing_in_for_cbr();
    let directory = fixture.directory.path();
    let out = directory.join("out");
    let good = manifest(directory, &fixture.checkout, "cbr", "MiniMax-M3");

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

    // And with it, a ceiling above the cap — the refusal that has to
    // hold when the flags that could spend are all present.
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

    assert!(
        !out.exists(),
        "a refused run created its output directory anyway"
    );
}

#[test]
fn a_repository_is_what_its_origin_says_and_not_what_the_manifest_called_it() {
    // **The id is a label somebody typed.** "Public repositories only"
    // enforced against the label admits any checkout on the disk under
    // any of the three permitted names — a private tree registered as
    // `brian2` would have been read, indexed and sent. So the checkout
    // is asked what it is, before anything is launched, in both modes:
    // a dry run reads the same bytes off the same disk as a live one.
    let fixture = standing_in_for_cbr();
    let directory = fixture.directory.path();
    let out = directory.join("out");

    let mislabelled = manifest(directory, &fixture.checkout, "brian2", "MiniMax-M3");
    let (ok, _, stderr) = harness(&[
        "--manifest",
        mislabelled.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a checkout was admitted under a name it is not");
    assert!(
        stderr.contains("brian2") && stderr.contains("origin"),
        "and not for that reason: {stderr}"
    );

    // A checkout with no origin at all says nothing about itself, which
    // is not the same as saying the right thing.
    let anonymous = Fixture::answering(&["choose:c1"]);
    let nameless = manifest(
        anonymous.directory.path(),
        &anonymous.checkout,
        "cbr",
        "MiniMax-M3",
    );
    let (ok, _, stderr) = harness(&[
        "--manifest",
        nameless.to_str().expect("utf-8"),
        "--out",
        anonymous
            .directory
            .path()
            .join("out")
            .to_str()
            .expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a checkout with no origin was admitted");
    assert!(stderr.contains("no origin remote"), "{stderr}");

    // And the commit, when the manifest pins one: a pilot question was
    // sealed against a tree, and answering it over another tree
    // measures something else.
    let moved = written(
        directory,
        "pinned",
        &[run_of(
            "dry",
            &fixture.checkout,
            "cbr",
            "MiniMax-M3",
            r#","commit":"0000000000000000000000000000000000000000""#,
        )],
    );
    let (ok, _, stderr) = harness(&[
        "--manifest",
        moved.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a checkout off the pinned commit was admitted");
    assert!(stderr.contains("the manifest pins"), "{stderr}");

    assert!(!out.exists(), "a refused run created its directory anyway");
}

#[test]
fn a_run_whose_items_would_eat_the_budget_never_reaches_discovery() {
    // **Silence is the failure mode this refuses.** Items are asked
    // first and a flow that cannot finish is not started, so a run whose
    // investigation the items exhaust asks discovery nothing — and
    // produces a packet that looks exactly like one the model was asked
    // about and did not widen. A live run of six could have measured
    // nothing and said so nowhere.
    let fixture = standing_in_for_cbr();
    let directory = fixture.directory.path();
    let out = directory.join("out");
    let path = directory.join("tight.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"runs":[{{"id":"tight","repository":{{"id":"cbr","path":"{checkout}"}},
                 "model":"MiniMax-M3","task":"what drains the queue",
                 "selector":"queue drains shutdown","capacity":65536,"investigation":5,
                 "wants":["q=source:queue.md","c=source:cache.md","i=source:index.md",
                          "u=source:unasked.md"],
                 "dry_answers":["choose:c1"]}}]}}"#,
            checkout = fixture.checkout.display()
        ),
    )
    .expect("manifest");

    let (ok, _, stderr) = harness(&[
        "--manifest",
        path.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(!ok, "a run that could never reach discovery was admitted");
    assert!(
        stderr.contains("leaves no room for discovery") && stderr.contains("at least 6"),
        "and not for that reason: {stderr}"
    );
    assert!(!out.exists(), "a refused run created its directory anyway");
}

#[test]
fn a_dry_run_drives_every_stage_and_reports_what_it_found() {
    // **The whole harness, once.** Submit, settle, write the packet,
    // read the ledger, relaunch as a rebuild, compare sections. If any
    // stage of this is wrong, it is wrong now rather than on the day the
    // owner's quota is being spent.
    let fixture = standing_in_for_cbr();
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
    // **And two of them were discovery's**, which is what says the
    // question m4e exists to ask was actually asked. A run that skipped
    // it would report one satisfied item and nothing else amiss.
    assert_eq!(
        run.get("discovery_records"),
        Some(&Value::Int(2)),
        "discovery's two steps did not both seal a record: {run:?}"
    );
    assert!(
        run.get("discovery_note").is_none(),
        "the run noted a discovery problem it should not have: {run:?}"
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

    // **What the checkout said it was**, recorded beside what was found
    // in it, so a reviewer scoring a packet knows which bytes made it.
    assert_eq!(
        run.get("origin").and_then(Value::as_str),
        Some("github.com/combraton/cbr"),
        "{run:?}"
    );
    assert!(
        run.get("head")
            .and_then(Value::as_str)
            .is_some_and(|head| head.len() == 40),
        "the commit that was read is not in the report: {run:?}"
    );

    // **The replay gate.** The same packet, rebuilt offline from the
    // records, section for section.
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

    // **And the store the run was made over is under `--out` too.** It
    // holds the ledger the spend was read from and the records the gate
    // replayed, so it is evidence, and evidence does not live in a
    // temporary directory a reboot empties.
    let data = run.get("data").and_then(Value::as_str).expect("a data dir");
    assert!(
        // The harness resolves `--out`, and on macOS the temp directory
        // is reached through a symlink, so the comparison is between two
        // resolved paths or between a path and a different spelling of
        // itself.
        Path::new(data).starts_with(out.canonicalize().expect("--out exists")),
        "the run's store is outside --out: {data}"
    );
    assert!(Path::new(data).join("cbr.sqlite").exists(), "{data}");

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
fn each_launch_is_given_what_is_left_of_the_run_and_not_the_whole_of_it() {
    // **The hard cap is a property of the run, and the ledger counts a
    // store.** `Ledger::run_spend` sums the store it was opened over and
    // every run opens a new one, so a ceiling handed to each launch
    // unreduced is the cap once per run rather than once — six runs
    // under a 5,000,000 cap could spend six times it, held back only by
    // the per-job ceiling and a stop checked after the fact.
    //
    // So: two runs, and the second is given what the first left.
    let fixture = standing_in_for_cbr();
    let directory = fixture.directory.path();
    let out = directory.join("out");
    let both = written(
        directory,
        "two",
        &[
            run_of("one", &fixture.checkout, "cbr", "MiniMax-M3", ""),
            run_of("two", &fixture.checkout, "cbr", "MiniMax-M3", ""),
        ],
    );

    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        both.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(ok, "the dry run failed:\n{stdout}\n{stderr}");
    let report: Value =
        cbr_encoding::parse(&std::fs::read(out.join("report.json")).expect("a report"))
            .expect("JSON");
    let runs = report
        .get("runs")
        .and_then(Value::as_array)
        .expect("runs")
        .to_vec();
    assert_eq!(runs.len(), 2, "{report:?}");

    let ceiling = |run: &Value| match run.get("launch_ceiling") {
        Some(Value::Int(tokens)) => *tokens,
        other => panic!("no launch ceiling: {other:?}"),
    };
    let spent = |run: &Value| match run.get("tokens") {
        Some(Value::Int(tokens)) => *tokens,
        other => panic!("nothing was charged: {other:?}"),
    };
    assert_eq!(ceiling(&runs[0]), 5_000_000, "{:?}", runs[0]);
    assert!(spent(&runs[0]) > 0, "{:?}", runs[0]);
    assert_eq!(
        ceiling(&runs[1]),
        5_000_000 - spent(&runs[0]),
        "the second launch was not given what the first left: {:?}",
        runs[1]
    );

    // **And the stop is checked before a run rather than after one.**
    // Lowered to a little over one flow's worst case, the first run is
    // started and the second is not — because what it *could* cost
    // would cross it, which is knowable before any of it is spent.
    let out = directory.join("stopped");
    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        both.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
        "--stop-tokens",
        "370000",
    ]);
    assert!(ok, "the dry run failed:\n{stdout}\n{stderr}");
    let report: Value =
        cbr_encoding::parse(&std::fs::read(out.join("report.json")).expect("a report"))
            .expect("JSON");
    assert_eq!(
        report
            .get("runs")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(1),
        "the second run was started with no room for its worst case: {report:?}"
    );
    assert!(
        report
            .get("stopped")
            .and_then(Value::as_str)
            .is_some_and(|why| why.contains("was not started") && why.contains("worst case")),
        "and the report does not say it stopped, or why: {report:?}"
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
