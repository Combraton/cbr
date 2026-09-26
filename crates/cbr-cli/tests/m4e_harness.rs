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
        replay.get("differences").and_then(Value::as_array),
        Some(&[][..]),
        "a rebuild that matched still named a differing leaf: {replay:?}"
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
    // **And so is what the stop priced a run at**: the flow and the
    // selection question the harness names, and this run's worst case,
    // its flow and one selection question for its one want. A report
    // that stated a figure the stop did not use would tell its reader
    // about a bound nobody applied.
    let (flow, selection) = (
        harness_figure("WORST_CASE_FLOW_TOKENS"),
        harness_figure("WORST_CASE_SELECTION_TOKENS"),
    );
    assert_eq!(
        report.get("worst_case_flow_tokens"),
        Some(&Value::Int(flow)),
        "the report does not state the flow the stop priced"
    );
    assert_eq!(
        report.get("worst_case_selection_tokens"),
        Some(&Value::Int(selection)),
        "the report does not state the selection question the stop priced"
    );
    assert_eq!(
        run.get("worst_case_tokens"),
        Some(&Value::Int(flow + selection)),
        "the run does not say it was priced at its flow and one selection question: {run:?}"
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
    // Lowered to a little over one run's worst case, the first run is
    // started and the second is not — because what it *could* cost
    // would cross it, which is knowable before any of it is spent.
    //
    // The bound is read out of the report rather than written here: it
    // moved at m4f when the discovery steps were given room to reason,
    // and again when the stop was found to leave out the run's own
    // selection questions, and a figure typed into a test is one more
    // place for the number to drift.
    let worst = match runs[0].get("worst_case_tokens") {
        Some(Value::Int(tokens)) => *tokens,
        other => panic!("the report does not say what a run can cost: {other:?}"),
    };
    let stop = (worst + 1).to_string();
    let out = directory.join("stopped");
    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        both.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
        "--stop-tokens",
        &stop,
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

/// A figure the harness names, read out of its source rather than
/// written here, so the test prices a run the way the harness does.
fn harness_figure(name: &str) -> i64 {
    let source = std::fs::read_to_string(script()).expect("the harness");
    let marker = format!("{name} = ");
    let at = source
        .find(&marker)
        .unwrap_or_else(|| panic!("the harness does not name {name}"));
    source[at + marker.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .filter(|c| *c != '_')
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("{name} is not a figure"))
}

#[test]
fn the_stop_prices_each_selection_question_a_run_asks_as_well_as_its_flow() {
    // **Every run asks a selection question for each thing it wants**,
    // under the same job and before discovery, and the stop priced the
    // flow alone. So a run was started against a stop its own item
    // questions could carry it past. What one run can cost is its flow
    // and one selection question per want, and a run is started only
    // when that fits under the stop and under what is left of the
    // ceiling.
    //
    // Two wants, at one token short of that figure by each door, and
    // then exactly at it.
    let flow = harness_figure("WORST_CASE_FLOW_TOKENS");
    let selection = harness_figure("WORST_CASE_SELECTION_TOKENS");
    let worst = flow + 2 * selection;
    let fixture = standing_in_for_cbr();
    let directory = fixture.directory.path();
    let manifest = written(
        directory,
        "two-wants",
        &[format!(
            r#"{{"id":"wants","repository":{{"id":"cbr","path":"{checkout}"}},
               "model":"MiniMax-M3","task":"what drains the queue",
               "selector":"queue drains shutdown","capacity":65536,"investigation":4,
               "wants":["q=source:queue.md","r=source:cache.md"],
               "dry_answers":["choose:c2","choose:c2","terms:tombstone","ids:d1"]}}"#,
            checkout = fixture.checkout.display()
        )],
    );
    let started = |name: &str, flags: &[&str]| -> Value {
        let out = directory.join(name);
        let mut arguments = vec![
            "--manifest",
            manifest.to_str().expect("utf-8"),
            "--out",
            out.to_str().expect("utf-8"),
            "--dry-run",
        ];
        arguments.extend_from_slice(flags);
        let (ok, stdout, stderr) = harness(&arguments);
        assert!(ok, "{name}: the dry run failed:\n{stdout}\n{stderr}");
        cbr_encoding::parse(&std::fs::read(out.join("report.json")).expect("a report"))
            .expect("JSON")
    };
    let short = (worst - 1).to_string();
    let exact = worst.to_string();
    for (name, flags) in [
        ("stop", ["--stop-tokens", short.as_str()]),
        ("ceiling", ["--run-ceiling", short.as_str()]),
    ] {
        let report = started(name, &flags);
        assert_eq!(
            report
                .get("runs")
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(0),
            "{name}: a run was started with {short}, one short of its flow and two \
             selection questions: {report:?}"
        );
        let why = report
            .get("stopped")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{name}: the report does not say it stopped: {report:?}"));
        assert!(
            why.contains("wants was not started"),
            "{name}: the report does not name the run it did not start: {why}"
        );
        // **And it says what the run was priced at**: the whole figure,
        // not the flow alone, and the wants that make it up. A message
        // naming a figure the check did not use tells its reader about a
        // bound nobody applied.
        assert!(
            why.contains(&format!(
                "the {worst} worst case of one flow and 2 selection questions"
            )),
            "{name}: the stop does not state the {worst} it priced the run at, flow \
             {flow} and two selection questions of {selection}: {why}"
        );
    }
    let report = started(
        "exact",
        &[
            "--stop-tokens",
            exact.as_str(),
            "--run-ceiling",
            exact.as_str(),
        ],
    );
    let runs = report.get("runs").and_then(Value::as_array).expect("runs");
    assert_eq!(
        runs.len(),
        1,
        "exactly its worst case is room for it: {report:?}"
    );
    assert_eq!(
        runs[0].get("worst_case_tokens"),
        Some(&Value::Int(worst)),
        "the run does not say what it was priced at"
    );
}

/// Run `not_public` over canned `(status, body)` pairs — by importing
/// the module and calling the one function, so nothing is launched and
/// no socket is opened.
fn parser_says(cases: &[(u16, &str)]) -> Vec<Option<String>> {
    let program = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("m4e_run", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
rest = sys.argv[2:]
print(json.dumps([
    module.not_public("github.com/an/example", int(rest[at]), rest[at + 1])
    for at in range(0, len(rest), 2)
]))
"#;
    let mut command = Command::new("python3");
    command.args(["-c", program]).arg(script());
    for (status, body) in cases {
        command.arg(status.to_string()).arg(body);
    }
    let output = command.output().expect("python3 runs");
    assert!(
        output.status.success(),
        "the parser could not be called: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value =
        cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
            .expect("the answers are JSON");
    parsed
        .as_array()
        .expect("a list of answers")
        .iter()
        .map(|answer| answer.as_str().map(str::to_string))
        .collect()
}

#[test]
fn only_a_two_hundred_saying_public_admits_a_repository() {
    // **The parser, on canned bodies.** The call itself has no test: it
    // opens a socket to a third party, and a suite that did that would
    // be a suite whose result depended on GitHub being reachable. What
    // is testable is the decision made about an answer — and the
    // property worth pinning is that **unknown lands where private
    // lands**, because the failure this prevents is sending somebody
    // else's private text to a provider.
    let answers = parser_says(&[
        (200, r#"{"private": false, "name": "cbr"}"#),
        (200, r#"{"private": true, "name": "cbr"}"#),
        (404, r#"{"message": "Not Found"}"#),
        (403, r#"{"message": "API rate limit exceeded"}"#),
        (301, r#"{"message": "Moved Permanently"}"#),
        (200, "<html>not json at all</html>"),
        (200, r#"{"name": "cbr"}"#),
        (200, r#"{"private": "false"}"#),
        (200, "null"),
    ]);

    assert_eq!(answers[0], None, "a public repository was not admitted");
    for (position, why) in answers.iter().enumerate().skip(1) {
        assert!(
            why.is_some(),
            "answer {position} admitted a repository nothing said was public"
        );
    }

    // And each refusal says which it was, because "refused" on its own
    // does not tell an operator whether to wait, to ask the owner, or to
    // fix the manifest.
    let says = |position: usize, wanted: &str| {
        let why = answers[position].as_deref().unwrap_or_default();
        assert!(
            why.contains(wanted),
            "answer {position} does not say {wanted:?}: {why}"
        );
    };
    says(1, "is not public");
    says(2, "HTTP 404");
    says(3, "HTTP 403");
    says(4, "HTTP 301");
    says(5, "not JSON");
    says(6, "no `private`");
    // A string `"false"` is not the boolean, and is refused rather than
    // read as one.
    says(7, "is not public");
    says(8, "no `private`");
}

/// The harness's **code**, with its docstrings and comments gone.
///
/// A rule about what a path may name has to be asserted against what
/// runs, not against what is written about it: this file says in prose
/// that it reads no `.netrc`, and a plain text search cannot tell that
/// sentence from the thing it forbids. Python parses its own source and
/// prints it back without the prose, which is the distinction made by a
/// parser rather than by a guess.
fn code_without_prose() -> String {
    let program = r#"
import ast, sys
tree = ast.parse(open(sys.argv[1]).read())
for node in ast.walk(tree):
    if isinstance(node, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        first = node.body[0] if node.body else None
        if (isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant)
                and isinstance(first.value.value, str)):
            node.body.pop(0)
print(ast.unparse(tree))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(script())
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "the harness could not be parsed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn the_gate_derives_whether_it_matched_from_the_leaves_it_found() {
    // **One answer, reported two ways.** `sections_identical` is derived
    // from the differences, so the report cannot say "differs" and name
    // nothing, or name something and call itself identical — which is
    // what the first live run did six times over.
    //
    // Asserted by reading the source, and that is a weaker thing than
    // the rest of this file: no dry run produces a rebuild that
    // differs, because both launches are over one store and the rebuild
    // reproduces the packet. The reporting branch is therefore not
    // observable from any test here, only the function it calls is
    // (above). Recorded as a gap in STATE rather than described as
    // covered.
    let source = std::fs::read_to_string(script()).expect("the harness");
    assert!(
        source.contains("differences = differing_leaves(live_sections, rebuilt_sections)")
            && source.contains(
                "result[\"replay\"][\"sections_identical\"] = live_sections is not None \
                 and not differences"
            ),
        "the gate no longer derives `sections_identical` from the leaves it found"
    );
}

#[test]
fn the_visibility_call_is_live_only_and_carries_no_credential() {
    // **Two rules about the one path that opens a socket to anywhere
    // but the provider.** Like `credential_discipline.rs` this reads the
    // source, so it can be fooled by a program that builds the call out
    // of pieces; it is a guard against carelessness, which is what would
    // put this in the wrong place.
    //
    // *Live only*, because a dry run must send nothing off the machine
    // and no test in this suite may open a socket to GitHub. *No
    // credential*, because the question is "can anyone read this" — a
    // call carrying the owner's token asks whether **they** can, which a
    // private repository answers yes.
    let source = std::fs::read_to_string(script()).expect("the harness");

    assert!(
        source.contains("    if live:\n        refuse_unless_public(checkouts)\n"),
        "the visibility check is not guarded by live mode"
    );
    assert_eq!(
        source.matches("refuse_unless_public(").count(),
        2,
        "the definition and exactly one call site, or that guard is not the only door"
    );
    assert_eq!(
        source.matches("ask_github(").count(),
        2,
        "the definition and exactly one call site"
    );

    let code = code_without_prose();
    for credential in [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "Authorization",
        "netrc",
        "\"gh\"",
        "'gh'",
        "environ",
        "getenv",
    ] {
        assert!(
            !code.contains(credential),
            "{credential} is on a path that must carry no credential at all"
        );
    }
    // And the guard is not vacuous: the prose it ignores really does
    // name one of these, so a run that stripped too much would pass
    // this rule while proving nothing.
    assert!(
        source.contains("netrc"),
        "the prose no longer names what the code must not, so this proves less than it reads"
    );
}

/// Call one of the harness's own functions with JSON arguments, without
/// running it: the module is imported by path and the function is
/// applied, which is how a pure rule in it is tested without a socket.
fn harness_says(call: &str, arguments: &[&str]) -> Value {
    let program = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("m4e_run", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
print(json.dumps(getattr(module, sys.argv[2])(*[json.loads(a) for a in sys.argv[3:]])))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(script())
        .arg(call)
        .args(arguments)
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "{call} could not be called: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("the answer is JSON")
}

/// Call `credential_file` for one launch, over a real `work` directory,
/// and report what it decided **and what it left on disk**.
fn credential_file_for(work: &Path, live: bool, replay: bool) -> Value {
    let program = r#"
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location("m4e_run", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
work = pathlib.Path(sys.argv[2])
run = {"id": "one", "model": "MiniMax-M3", "repository": {"id": "cbr", "path": "/nowhere"}}
config = module.config_of(run, sys.argv[3] == "true", [], sys.argv[4] == "true")
path = module.credential_file(work, config)
written = work / "credential"
print(json.dumps({
    "path": str(path),
    "issued": str(path) == str(work / "data" / "credentials" / "owner"),
    "wrote_a_file": written.exists(),
    "holds": written.read_text() if written.exists() else None,
}))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(script())
        .arg(work)
        .arg(live.to_string())
        .arg(replay.to_string())
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "credential_file could not be called: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("the answer is JSON")
}

#[test]
fn credential_file_gives_each_launch_what_its_own_configuration_admits() {
    // **Asserted at the door, and the first version of this was not.**
    // It asserted a second function written so the rule could be checked
    // without a temporary directory — and the reviewer's mutant, one
    // `or DRY_RUN_CREDENTIAL` inside `credential_file`, passed every
    // test in this file. That is run 2's defect one level down: the rule
    // restated somewhere nothing calls.
    //
    // So this calls the function every launch goes through, over a real
    // directory, and looks at what it left on disk as well as what it
    // returned. A production launch must take the credential the
    // provider issued **and write nothing**: writing one would mean a
    // file on disk that a production launch might later be handed.
    //
    // **Only a live run exercises the production side for real**, since
    // both launches of a dry run are conformance. What is checked here
    // is the decision and its trace on disk, which is the most a test
    // that launches nothing can hold.
    let directory = tempfile::tempdir().expect("temp dir");

    for (live, replay) in [(true, false), (true, true)] {
        let work = directory.path().join(format!("live-{replay}"));
        std::fs::create_dir_all(&work).expect("work");
        let decided = credential_file_for(&work, live, replay);
        assert_eq!(
            decided.get("issued"),
            Some(&Value::Bool(true)),
            "a production launch (replay={replay}) did not take the issued credential: {decided:?}"
        );
        assert_eq!(
            decided.get("wrote_a_file"),
            Some(&Value::Bool(false)),
            "a production launch (replay={replay}) wrote a credential file: {decided:?}"
        );
    }

    for (live, replay) in [(false, false), (false, true)] {
        let work = directory.path().join(format!("dry-{replay}"));
        std::fs::create_dir_all(&work).expect("work");
        let decided = credential_file_for(&work, live, replay);
        assert_eq!(
            decided.get("issued"),
            Some(&Value::Bool(false)),
            "a conformance launch (replay={replay}) took the issued credential: {decided:?}"
        );
        assert_eq!(
            decided.get("wrote_a_file"),
            Some(&Value::Bool(true)),
            "a conformance launch (replay={replay}) wrote no credential: {decided:?}"
        );
        // And it holds what that configuration admits, rather than
        // whatever the harness happens to call its dry-run credential.
        let configuration = harness_says(
            "config_of",
            &[
                r#"{"id": "one", "model": "MiniMax-M3",
                    "repository": {"id": "cbr", "path": "/nowhere"}}"#,
                "false",
                "[]",
                if replay { "true" } else { "false" },
            ],
        );
        let admitted = configuration
            .get("credentials")
            .and_then(Value::as_array)
            .and_then(<[Value]>::first)
            .and_then(|named| named.get("credential"))
            .and_then(Value::as_str)
            .expect("a conformance configuration names its credential");
        assert_eq!(
            decided.get("holds").and_then(Value::as_str),
            Some(admitted),
            "the file holds something its configuration does not admit: {decided:?}"
        );
    }
}

#[test]
fn nothing_but_that_one_function_decides_a_credential() {
    // **The shape `credential_discipline.rs` uses**, for the same reason
    // it uses it: a rule about which function decides something is a
    // rule about the source, and reading the source is the only way to
    // say *nothing else does*.
    let source = std::fs::read_to_string(script()).expect("the harness");

    // The definition and exactly two call sites, one per launch.
    assert_eq!(
        source.matches("credential_file(").count(),
        3,
        "the harness no longer decides a credential in exactly two places"
    );
    // And each passes the body that came back beside the configuration
    // it just launched — which is what makes pairing the wrong two
    // impossible rather than merely discouraged.
    for (bound, presented) in [
        (
            "config, config_body = configuration(work, run, live, run.get(\"dry_answers\", []))",
            "credential_file(work, config_body)",
        ),
        (
            "replay_config, replay_config_body = configuration(",
            "credential_file(work, replay_config_body)",
        ),
    ] {
        assert!(
            source.contains(bound),
            "the harness no longer binds {bound}"
        );
        assert!(
            source.contains(presented),
            "a launch presents a credential that is not its own configuration's: {presented}"
        );
    }

    // Nothing else reads a credential out of a configuration, and the
    // dry-run credential is named only where it is defined and where a
    // conformance configuration declares it.
    assert_eq!(
        source.matches("admits(").count(),
        2,
        "something other than `credential_file` decides what a configuration admits"
    );
    assert_eq!(
        source.matches("DRY_RUN_CREDENTIAL").count(),
        2,
        "the dry-run credential is named somewhere that is not its definition or the \
         configuration that declares it"
    );
}

#[test]
fn the_gate_says_which_leaf_differs_and_carries_no_repository_text() {
    // **The live run of 2026-09-22 reported six differences and named
    // none of them.** The one differing leaf in every store was
    // `citations[].evidence.provider`, and finding that out meant
    // opening six packets by hand. A gate that reports only *that* two
    // things are unequal makes its reader do the work it existed to do.
    //
    // The two halves of the rule: the leaf is named, and the values are
    // carried only when they cannot be somebody else's source.
    let live = r#"[{"section_id": "s-q", "content": "The queue drains on shutdown, and the drain is ordered.",
                    "citations": [{"evidence": {"provider": "cbr"}}]}]"#;
    let rebuilt = r#"[{"section_id": "s-q", "content": "The queue drains on shutdown, and the drain is ordered.",
                       "citations": [{"evidence": {"provider": "conformance-provider"}}]}]"#;
    let differences = harness_says("differing_leaves", &[live, rebuilt]);
    let found = differences.as_array().expect("a list");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].get("at").and_then(Value::as_str),
        Some("[0].citations[0].evidence.provider"),
        "the differing leaf is not named: {found:?}"
    );
    // An identifier is carried as it is, because that is what makes the
    // report diagnosable and an identifier is not repository text.
    assert_eq!(found[0].get("live").and_then(Value::as_str), Some("cbr"));
    assert_eq!(
        found[0].get("rebuilt").and_then(Value::as_str),
        Some("conformance-provider")
    );

    // **And a differing excerpt is not.** The report is the thing most
    // likely to be pasted somewhere, and the licence rule for the
    // pilots is digests, paths, spans, counts and costs.
    let one =
        r#"[{"content": "The queue drains on shutdown, case 1.0, and the drain is ordered."}]"#;
    let other =
        r#"[{"content": "The queue drains on shutdown, case 2.0, and the drain is ordered."}]"#;
    let differences = harness_says("differing_leaves", &[one, other]);
    let found = differences.as_array().expect("a list");
    assert_eq!(found.len(), 1, "{found:?}");
    for side in ["live", "rebuilt"] {
        let carried = found[0]
            .get(side)
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            carried.starts_with('<') && carried.contains("sha256:"),
            "{side} carries the text itself: {carried}"
        );
        assert!(
            !carried.contains("drains on shutdown"),
            "{side} carries repository text: {carried}"
        );
    }

    // A tree that matches has nothing to say about itself.
    let same = harness_says("differing_leaves", &[live, live]);
    assert_eq!(same.as_array().map(<[Value]>::len), Some(0), "{same:?}");

    // And a missing leaf is a difference rather than a match, which is
    // the case a walk that only visited shared keys would miss.
    let short = r#"[{"section_id": "s-q"}]"#;
    let long = r#"[{"section_id": "s-q", "label": "queue"}]"#;
    let found = harness_says("differing_leaves", &[short, long]);
    let found = found.as_array().expect("a list");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].get("live").and_then(Value::as_str),
        Some("<absent>")
    );
}

#[test]
fn a_live_rebuild_is_launched_under_the_configuration_the_live_run_used() {
    // **The defect the gate's six false differences came from.** The
    // rebuild used to be launched under a conformance configuration,
    // whose `provider_id` defaults to `conformance-provider`; every
    // citation in every rebuilt packet then named a different provider
    // from the live one, and the gate dutifully reported a difference
    // the harness had created itself.
    //
    // `--replay-model` with a production configuration and no permit is
    // `ServeFromRecords`: no transport is built and no credential is
    // read, so there is nothing the live configuration buys the rebuild
    // except being the same.
    let run = r#"{"id": "one", "model": "MiniMax-M3",
                  "repository": {"id": "cbr", "path": "/nowhere"}}"#;

    let serving = harness_says("config_of", &[run, "true", "[]", "false"]);
    let rebuild = harness_says("config_of", &[run, "true", "[]", "true"]);
    assert_eq!(
        serving, rebuild,
        "a live rebuild is configured differently from the run it rebuilds"
    );

    // A dry run cannot do the same, and the asymmetry is the provider's
    // rule: `--replay-model` needs a configured model, and a *serving*
    // launch carrying one without the permit is refused. Both launches
    // are conformance either way, so both name the same provider and
    // the gate is not misled.
    let serving = harness_says("config_of", &[run, "false", "[]", "false"]);
    let rebuild = harness_says("config_of", &[run, "false", "[]", "true"]);
    assert_ne!(
        serving, rebuild,
        "the dry rebuild has no configured model, so --replay-model would be refused"
    );
    for configuration in [&serving, &rebuild] {
        assert_eq!(
            configuration.get("format").and_then(Value::as_str),
            Some("combraton-conformance-config/1"),
            "a dry run launched something that is not a conformance launch"
        );
    }
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

/// Build a store the way a run leaves one, read it through the harness's
/// own `spend`, `records` and `ambiguity`, and report what they read and
/// **every file's bytes before and after**.
///
/// `killed`: the writer is killed with its rows still in the write-ahead
/// log, as a provider killed while a session held the store leaves it.
/// `closed`: the writer closes cleanly, so there is no log and no
/// shared-memory file at all — the usual state, because a provider holds a
/// connection per session and none between them. `torn`: killed, and then
/// the shared-memory file is gone while the log remains, which is what
/// `stop` leaves when it lands inside SQLite's own close.
fn read_through_the_harness(state: &str) -> Value {
    let directory = tempfile::tempdir().expect("temp dir");
    let data = directory.path().join("data");
    let program = r#"
import hashlib, importlib.util, json, pathlib, subprocess, sys
spec = importlib.util.spec_from_file_location("m4e_run", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
data, state = pathlib.Path(sys.argv[2]), sys.argv[3]
data.mkdir(parents=True)

record = {"question": {"digest": "sha256:q1", "selector": "discovery.terms x"},
          "answer": {"proposed": ["queue"]}, "made_at": "2026-09-23T00:00:00Z"}
body = json.dumps(record).encode()
digest = "sha256:" + hashlib.sha256(body).hexdigest()
hexed = digest.split(":", 1)[1]
obj = data / "objects" / "sha256" / hexed[0:2] / hexed[2:4] / hexed[4:]
obj.parent.mkdir(parents=True)
obj.write_bytes(body)
row = json.dumps({"state": "sealed", "descriptor": {"digest": digest}})

writer = '''
import os, signal, sqlite3, sys
path, state, row = sys.argv[1], sys.argv[2], sys.argv[3]
c = sqlite3.connect(path, isolation_level=None)
c.execute("PRAGMA journal_mode=WAL")
c.execute("PRAGMA wal_autocheckpoint=0")
c.execute("CREATE TABLE model_ledger (id INTEGER PRIMARY KEY, kind TEXT, tokens INTEGER)")
c.execute("CREATE TABLE subjects (kind TEXT, id TEXT, value TEXT)")
c.execute("PRAGMA wal_checkpoint(TRUNCATE)")
c.execute("INSERT INTO model_ledger (kind, tokens) VALUES "
          "('usage', 5222), ('admitted_local', 0), ('usage', 4827), ('admitted_local', 0)")
c.execute("INSERT INTO subjects VALUES ('evidence.artifact', 'der.x', ?)", (row,))
if state in ("killed", "torn"):
    os.kill(os.getpid(), signal.SIGKILL)
c.close()
'''
subprocess.run([sys.executable, "-c", writer, str(data / "cbr.sqlite"), state, row])
# SQLite's close unlinks the shared memory only after its checkpoint, so a
# natural torn store's database is already whole. This one is not: its rows
# are only in the log, which is the case a reader could get most wrong.
if state == "torn":
    (data / "cbr.sqlite-shm").unlink()

# The precondition, read with `immutable=1`, which changes nothing: in the
# killed store the rows are in the log and not yet in the database file.
database = (data / "cbr.sqlite").resolve()
probe = __import__("sqlite3").connect(f"{database.as_uri()}?immutable=1", uri=True)
in_the_file = probe.execute("SELECT COUNT(*) FROM model_ledger").fetchone()[0]
probe.close()

def files():
    return {p.relative_to(data).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(data.rglob("*")) if p.is_file()}

before = files()
total, charges = module.spend(data)
found = module.records(data)
questions = module.ambiguity(data)["questions"]
after = files()
print(json.dumps({"in_the_file": in_the_file, "total": total, "charges": len(charges),
                  "records": len(found), "questions": questions,
                  "before": before, "after": after}))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(script())
        .arg(&data)
        .arg(state)
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "the harness could not read the store: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("the answer is JSON")
}

fn file_names(files: Option<&Value>) -> Vec<String> {
    match files {
        Some(Value::Object(members)) => members.iter().map(|(name, _)| name.clone()).collect(),
        other => panic!("not a file listing: {other:?}"),
    }
}

#[test]
fn the_harness_reads_a_killed_stores_ledger_from_its_log_and_changes_no_byte() {
    // **`stop` kills the provider**, so the ledger rows a run's report is
    // made of can be only in the write-ahead log when the harness reads
    // them. A reader that ignores the log reports a run as free; a reader
    // that writes folds the log into the database and deletes it, which is
    // the harness editing the evidence it summarises.
    let read = read_through_the_harness("killed");
    assert_eq!(
        read.get("in_the_file"),
        Some(&Value::Int(0)),
        "the rows are only in the log, so this reads what the harness must: {read:?}"
    );
    let names = file_names(read.get("before"));
    assert!(
        names.contains(&"cbr.sqlite-wal".to_string())
            && names.contains(&"cbr.sqlite-shm".to_string()),
        "a killed store keeps its log and shared memory: {names:?}"
    );
    assert_eq!(
        read.get("total"),
        Some(&Value::Int(10_049)),
        "every charge was read"
    );
    assert_eq!(
        read.get("charges"),
        Some(&Value::Int(2)),
        "and notes are not charges"
    );
    assert_eq!(read.get("records"), Some(&Value::Int(1)));
    assert_eq!(read.get("questions"), Some(&Value::Int(1)));
    assert_eq!(
        read.get("before"),
        read.get("after"),
        "reading the store changed it: {read:?}"
    );
}

#[test]
fn the_harness_reads_a_store_with_no_log_and_creates_no_file() {
    // The other state a store can be in: closed cleanly, or checkpointed by
    // anything that read it before this build. There is no log to read,
    // and `mode=ro` would create one and a shared-memory file beside it.
    let read = read_through_the_harness("closed");
    let names = file_names(read.get("before"));
    assert!(
        !names
            .iter()
            .any(|name| name.ends_with("-wal") || name.ends_with("-shm")),
        "a cleanly closed store has no log: {names:?}"
    );
    assert_eq!(read.get("total"), Some(&Value::Int(10_049)));
    assert_eq!(read.get("records"), Some(&Value::Int(1)));
    assert_eq!(
        read.get("before"),
        read.get("after"),
        "reading the store changed it: {read:?}"
    );
}

#[test]
fn the_harness_reads_a_store_with_a_log_and_no_shared_memory_from_a_copy() {
    // **Found by CI, not by a reading.** A provider opens a store
    // connection per session and closes it cleanly, and `stop` can land
    // inside that close: after SQLite unlinked the shared-memory file,
    // before it deleted the log. Opened in place that store gets a new
    // shared-memory file whichever way it is opened, or its log is missed.
    // So it is read from a private copy, and the store is never opened.
    let read = read_through_the_harness("torn");
    assert_eq!(
        read.get("in_the_file"),
        Some(&Value::Int(0)),
        "the rows are only in the log: {read:?}"
    );
    let names = file_names(read.get("before"));
    assert!(
        names.contains(&"cbr.sqlite-wal".to_string())
            && !names.contains(&"cbr.sqlite-shm".to_string()),
        "a log and no shared memory: {names:?}"
    );
    assert_eq!(
        read.get("total"),
        Some(&Value::Int(10_049)),
        "every charge was read"
    );
    assert_eq!(read.get("records"), Some(&Value::Int(1)));
    assert_eq!(
        read.get("before"),
        read.get("after"),
        "reading the store changed it: {read:?}"
    );
}
