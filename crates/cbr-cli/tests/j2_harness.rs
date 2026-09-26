//! **J2's live harness, run end to end before it is ever run live.**
//!
//! `scripts/j2_run.py` is Journey 2's live acceptance written down as code
//! (m5 READINESS §3 and §10): CBR's own test output, sealed whole,
//! projected with no investigation and then with a model's help, checked
//! against its own bytes, and rebuilt from its records. Its dry-run mode
//! drives every stage against the `model.fake` control, and this is the
//! test that says the stages work.
//!
//! What is exercised: every refusal it makes before anything is spent —
//! **above all, an input that carries a machine path** — a whole run
//! through ingest, both requests, the projection's own checks and the
//! replay gate, an input over the projection's capacity, and the stop
//! before a run the ceiling could not cover.
//!
//! **No model is called and no socket is opened** but the provider's. The
//! dry run writes a conformance configuration carrying the fake, which a
//! production launch refuses by name, and no test here passes
//! `--permit-model-network`.

use std::path::{Path, PathBuf};
use std::process::Command;

use cbr_encoding::Value;

/// The repository root, which is the checkout the harness reads.
fn root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn script() -> PathBuf {
    let script = root().join("scripts").join("j2_run.py");
    assert!(script.exists(), "{} is missing", script.display());
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

/// The commit this checkout is at, which a manifest pins its inputs to.
fn head() -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root())
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git runs");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A manifest over `runs`, each `(id, input, extra members)`.
fn manifest(directory: &Path, name: &str, commit: &str, runs: &[(&str, &Path, &str)]) -> PathBuf {
    let runs: Vec<String> = runs
        .iter()
        .map(|(id, input, extra)| {
            format!(
                r#"{{"id":"{id}","input":"{}","dry_answers":["ids:u1"]{extra}}}"#,
                input.display()
            )
        })
        .collect();
    let path = directory.join(format!("{name}.json"));
    std::fs::write(
        &path,
        format!(
            r#"{{"commit":"{commit}","model":"MiniMax-M3","runs":[{}]}}"#,
            runs.join(",")
        ),
    )
    .expect("manifest");
    path
}

/// Cargo's test output, as the suite prints it, with `failing` failing.
fn test_log(binaries: usize, tests: usize, failing: &[(usize, usize)]) -> String {
    let mut log = String::new();
    for binary in 0..binaries {
        let name = |test: usize| format!("module_{binary}::tests::case_{test}_holds_under_load");
        log.push_str(&format!(
            "     Running unittests src/lib.rs (target/debug/deps/crate_{binary}-0123456789abcdef)\n\nrunning {tests} tests\n"
        ));
        let mut failed = Vec::new();
        for test in 0..tests {
            let fails = failing.contains(&(binary, test));
            log.push_str(&format!(
                "test {} ... {}\n",
                name(test),
                if fails { "FAILED" } else { "ok" }
            ));
            if fails {
                failed.push(test);
            }
        }
        log.push('\n');
        for test in &failed {
            log.push_str(&format!(
                "---- {} stdout ----\nthread panicked at src/lib.rs:9:5:\nassertion `left == right` failed\n  left: 1\n right: 2\n\n",
                name(*test)
            ));
        }
        log.push_str(&format!(
            "test result: {}. {} passed; {} failed; 0 ignored\n\n",
            if failed.is_empty() { "ok" } else { "FAILED" },
            tests - failed.len(),
            failed.len()
        ));
    }
    log
}

fn report(out: &Path) -> Value {
    cbr_encoding::parse(&std::fs::read(out.join("report.json")).expect("a report"))
        .expect("the report is JSON")
}

/// A figure the harness declares, read from its source rather than typed
/// here: `WORST_CASE_PROJECTION_TOKENS`, which `projection::tests` in turn
/// holds to the arithmetic.
fn declared(name: &str) -> u64 {
    declared_in("j2_run.py", name)
}

/// The same, from any of the scripts: `RUN_CEILING_TOKENS` is m4e's.
fn declared_in(script: &str, name: &str) -> u64 {
    let source = std::fs::read_to_string(root().join("scripts").join(script)).expect("the script");
    let at = source
        .find(&format!("\n{name} = "))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("{script} no longer declares {name}"));
    source[at + name.len() + 3..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .filter(|c| *c != '_')
        .collect::<String>()
        .parse()
        .expect("a number")
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut current = value;
    for name in path {
        current = current.get(name).unwrap_or(&Value::Null);
    }
    current
}

fn int(value: &Value, path: &[&str]) -> i64 {
    match at(value, path) {
        Value::Int(number) => *number,
        other => panic!("{path:?} is not a number: {other:?}"),
    }
}

// ---- refusals ---------------------------------------------------------------

#[test]
fn the_harness_refuses_an_input_that_carries_a_machine_path_before_anything_starts() {
    // **The one refusal READINESS §9 names for m5a's harness.** An input is
    // sealed whole and never scrubbed, so output that names the machine it
    // was produced on is refused, and the refusal says where without
    // repeating what. The paths are built from pieces so that this file
    // carries none.
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    for root in [
        ["", "Users", "someone", "cbr"].join("/"),
        ["", "Volumes", "Disk", "cbr"].join("/"),
        ["", "home", "runner", "work"].join("/"),
        ["", "var", "folders", "xy", "T"].join("/"),
        ["", "private", "tmp", "scratch"].join("/"),
    ] {
        let log = format!(
            "running 1 test\ntest a::b ... FAILED\n---- a::b stdout ----\nthread panicked at {root}/src/lib.rs:9:5\n\ntest result: FAILED. 0 passed; 1 failed\n"
        );
        let input = directory.path().join("marked.log");
        std::fs::write(&input, &log).expect("input");
        let written = manifest(
            directory.path(),
            "marked",
            &head(),
            &[("marked", &input, "")],
        );
        let (ok, _, stderr) = harness(&[
            "--manifest",
            written.to_str().expect("utf-8"),
            "--out",
            out.to_str().expect("utf-8"),
            "--dry-run",
        ]);
        assert!(!ok, "an input carrying {root} was admitted");
        let at = log.find(&root).expect("the path is in the log");
        assert!(
            stderr.contains(&format!("machine path at byte {at}")),
            "refused for another reason, or at another byte: {stderr}"
        );
        assert!(
            !stderr.contains(&root),
            "the refusal repeated the path: {stderr}"
        );
        assert!(!out.join("work").exists(), "something was launched first");
    }
}

#[test]
fn the_harness_refuses_every_run_it_should_before_anything_is_spent() {
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let input = directory.path().join("small.log");
    std::fs::write(&input, test_log(1, 3, &[])).expect("input");
    let good = manifest(directory.path(), "good", &head(), &[("small", &input, "")]);
    let refused = |manifest: &Path, extra: &[&str], out: &Path| {
        let mut arguments = vec![
            "--manifest",
            manifest.to_str().expect("utf-8"),
            "--out",
            out.to_str().expect("utf-8"),
        ];
        arguments.extend_from_slice(extra);
        let (ok, _, stderr) = harness(&arguments);
        assert!(!ok, "admitted: {extra:?}");
        stderr
    };

    // An output directory inside this repository.
    let inside = root().join("target").join("j2-out");
    assert!(refused(&good, &["--dry-run"], &inside).contains("is inside"));
    // `--live` without a cap, and without the permit. Neither
    // passes `--permit-model-network`: the cap is refused first.
    assert!(refused(&good, &["--live"], &out).contains("--live needs --run-ceiling"));
    assert!(
        refused(&good, &["--live", "--run-ceiling", "1000000"], &out)
            .contains("--live needs --permit-model-network")
    );
    // A model outside the three.
    let other = manifest(
        directory.path(),
        "other",
        &head(),
        &[("small", &input, r#","model":"gpt-4""#)],
    );
    assert!(refused(&other, &["--dry-run"], &out).contains("is not one of the three models"));
    // Inputs that are not what the manifest says.
    let missing = directory.path().join("absent.log");
    let absent = manifest(
        directory.path(),
        "absent",
        &head(),
        &[("gone", &missing, "")],
    );
    assert!(refused(&absent, &["--dry-run"], &out).contains("its input is not a file"));
    let pinned = manifest(
        directory.path(),
        "pinned",
        &head(),
        &[(
            "small",
            &input,
            &format!(r#","digest":"sha256:{}""#, "0".repeat(64)),
        )],
    );
    assert!(refused(&pinned, &["--dry-run"], &out).contains("is not the bytes the manifest pins"));
    let twice = manifest(
        directory.path(),
        "twice",
        &head(),
        &[("same", &input, ""), ("same", &input, "")],
    );
    assert!(refused(&twice, &["--dry-run"], &out).contains("two runs share the id"));
    // A commit this checkout does not have.
    let nowhere = manifest(
        directory.path(),
        "nowhere",
        &"f".repeat(40),
        &[("small", &input, "")],
    );
    assert!(refused(&nowhere, &["--dry-run"], &out).contains("is not in"));
    assert!(
        !out.join("work").exists(),
        "a refusal launched something first"
    );
}

// ---- runs -----------------------------------------------------------------------

#[test]
fn a_dry_run_projects_a_test_log_checks_it_against_its_bytes_and_rebuilds_it() {
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let log = test_log(8, 100, &[(1, 17), (5, 2)]);
    let input = directory.path().join("cargo-test.log");
    std::fs::write(&input, &log).expect("input");
    let written = manifest(
        directory.path(),
        "run",
        &head(),
        &[("cargo-test", &input, "")],
    );
    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        written.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(ok, "the dry run failed: {stdout}\n{stderr}");

    let report = report(&out);
    let run = &at(&report, &["runs"]).as_array().expect("runs")[0];
    // Named by its file and its digest, never by the manifest's path.
    assert_eq!(at(run, &["input", "name"]).as_str(), Some("cargo-test.log"));
    assert_eq!(
        at(run, &["input", "digest"]).as_str(),
        Some(cbr_encoding::digest_bytes(log.as_bytes()).as_str())
    );
    let report_text = std::fs::read_to_string(out.join("report.json")).expect("report");
    assert!(
        !report_text.contains(
            directory
                .path()
                .join("cargo-test.log")
                .to_str()
                .expect("utf-8")
        ),
        "the report carries the manifest's path"
    );

    // The baseline: satisfied, the rule, nothing asked, every check holds.
    assert_eq!(
        at(run, &["baseline", "item", "result"]).as_str(),
        Some("satisfied")
    );
    assert!(
        at(run, &["baseline", "how"])
            .as_str()
            .is_some_and(|how| how.contains("the deterministic rule")),
        "{run:?}"
    );
    assert_eq!(
        at(run, &["baseline", "problems"]),
        &Value::Array(Vec::new())
    );
    let parts = int(run, &["baseline", "parts"]);
    assert!(parts >= 2, "{run:?}");
    assert_eq!(int(run, &["baseline", "named_failures"]), 2);

    // The assisted request: given exactly that many questions, one record
    // and one charge per part, and its projection checked the same way.
    assert_eq!(int(run, &["assisted", "investigation"]), parts);
    assert_eq!(
        at(run, &["assisted", "item", "result"]).as_str(),
        Some("satisfied")
    );
    assert_eq!(
        at(run, &["assisted", "problems"]),
        &Value::Array(Vec::new())
    );
    assert!(
        at(run, &["assisted", "how"])
            .as_str()
            .is_some_and(|how| how.contains("chosen by the model"))
    );
    assert_eq!(int(run, &["part_records"]), parts);
    assert_eq!(int(run, &["tokens"]), parts * 5000, "{run:?}");
    // **The floor was checked**, not merely unbroken: every excerpt the
    // baseline carries is counted as carried by the assisted projection.
    assert_eq!(
        int(run, &["assisted", "baseline_excerpts_carried"]),
        int(run, &["baseline", "excerpts"]),
        "{run:?}"
    );

    // The replay gate: rebuilt from the records, the same sections.
    assert_eq!(
        at(run, &["replay", "item", "result"]).as_str(),
        Some("satisfied")
    );
    assert_eq!(
        at(run, &["replay", "sections_identical"]),
        &Value::Bool(true),
        "{run:?}"
    );
    assert_eq!(at(run, &["replay", "ambiguous"]), &Value::Array(Vec::new()));
    assert!(out.join("cargo-test.assisted.packet.json").exists());
}

#[test]
fn an_input_over_the_projections_capacity_is_reported_as_such_and_nothing_is_asked() {
    // **Both of the capacity's bounds**: a log over a mebibyte, which the
    // size check refuses before reading, and plain text under the size
    // bound whose units need one part more than a projection may ask.
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let log = test_log(60, 400, &[(3, 3)]);
    assert!(
        log.len() > 1 << 20,
        "the fixture is meant to be over a mebibyte"
    );
    let input = directory.path().join("oversize.log");
    std::fs::write(&input, &log).expect("input");
    let parts = format!("{}\n", "z".repeat(89)).repeat(20 * 69);
    assert!(
        parts.len() <= 131_072,
        "the fixture is meant to pass the size check, INPUT_BYTES"
    );
    let many = directory.path().join("many-parts.log");
    std::fs::write(&many, &parts).expect("input");
    let written = manifest(
        directory.path(),
        "big",
        &head(),
        &[("oversize", &input, ""), ("many-parts", &many, "")],
    );
    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        written.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(ok, "{stdout}\n{stderr}");
    let report = report(&out);
    let runs = at(&report, &["runs"]).as_array().expect("runs");
    assert_eq!(runs.len(), 2, "{report:?}");
    for run in runs {
        assert_eq!(
            at(run, &["baseline", "item", "result"]).as_str(),
            Some("unmet"),
            "{run:?}"
        );
        assert_eq!(
            at(run, &["baseline", "item", "reason"]).as_str(),
            Some("insufficient_capacity"),
            "{run:?}"
        );
        assert_ne!(
            at(run, &["baseline", "section"]),
            &Value::Bool(true),
            "a section was published: {run:?}"
        );
        assert!(
            at(run, &["assisted", "skipped"]).as_str().is_some(),
            "{run:?}"
        );
        assert_eq!(int(run, &["tokens"]), 0);
        assert_eq!(int(run, &["part_records"]), 0);
    }
}

#[test]
fn a_run_that_offers_nothing_asks_nothing_live_or_dry() {
    // **A question no answer can change is not asked, and the harness does
    // not ask it either.** Thirty failures between passing tests fill every
    // excerpt a projection holds, so the rule's floor leaves a model
    // nothing it could add and the baseline says so with no parts. The
    // assisted request is still made — given every part a projection may
    // ask, so that nothing but the projection stops a call — and it
    // spends nothing and records nothing. `one_run` decides this without
    // looking at the mode, so a live run takes the same path.
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let failing: Vec<(usize, usize)> = (0..30).map(|test| (0, test * 2 + 1)).collect();
    let log = test_log(1, 400, &failing);
    assert!(
        log.len() > 16 * 1024,
        "the fixture is meant not to fit whole"
    );
    let input = directory.path().join("full.log");
    std::fs::write(&input, &log).expect("input");
    let written = manifest(directory.path(), "full", &head(), &[("full", &input, "")]);
    let (ok, stdout, stderr) = harness(&[
        "--manifest",
        written.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
    ]);
    assert!(ok, "{stdout}\n{stderr}");
    let report = report(&out);
    let run = &at(&report, &["runs"]).as_array().expect("runs")[0];
    assert_eq!(int(run, &["baseline", "parts"]), 0, "{run:?}");
    assert!(
        at(run, &["baseline", "how"])
            .as_str()
            .is_some_and(|how| how.contains("the deterministic rule")),
        "{run:?}"
    );
    assert_eq!(
        int(run, &["assisted", "investigation"]),
        declared("MAX_PARTS") as i64,
        "{run:?}"
    );
    assert_eq!(
        at(run, &["assisted", "item", "result"]).as_str(),
        Some("satisfied"),
        "{run:?}"
    );
    assert_eq!(
        at(run, &["assisted", "problems"]),
        &Value::Array(Vec::new()),
        "{run:?}"
    );
    assert!(
        at(run, &["assisted", "how"])
            .as_str()
            .is_some_and(|how| !how.contains("the model")),
        "{run:?}"
    );
    assert_eq!(int(run, &["part_records"]), 0, "{run:?}");
    assert_eq!(int(run, &["tokens"]), 0, "{run:?}");
    assert!(out.join("full.assisted.packet.json").exists());
    // **Nothing was answered, so nothing is replayed**: a replay gate run
    // here would report a rebuild of a question nobody asked.
    assert!(run.get("replay").is_none(), "{run:?}");
}

#[test]
fn the_harness_names_a_model_arm_that_drops_a_baseline_excerpt() {
    // **The floor, checked by the harness as well as built by the
    // provider**: a model-assisted projection carries every byte the
    // baseline carries, whatever else it carries. Given one that dropped a
    // baseline excerpt, or kept only the start of one, `checked` names it;
    // given one that carries the baseline's excerpts and more, or joins two
    // of them, it says nothing.
    let program = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import j2_run
source = b"line one\nline two\nline three\n"
head = ("projection x\nread as lines: 29 bytes, 3 lines, 3 units in 1 parts; "
        "excerpts chosen by the model\nfailures named: 0\n")
def excerpt(number, start, end):
    return (f"[e{number}] bytes {start}-{end}, lines 1-1, lines\n"
            + source[start:end].decode() + f"\n[end e{number}]\n")
def packet(content, omissions):
    return {"sections": [{"item_id": "log", "content": content}],
            "omissions": [{"item_id": "log", "section_id": f"s-log.o{n}", "reason": "applicability"}
                          for n in range(1, omissions + 1)]}
floor = [(0, 9), (18, 29)]
cases = {
    "dropped": packet(head + excerpt(1, 0, 9)
                      + "[o1] bytes 9-29, lines 2-3, omitted: not_selected\n", 1),
    "partial": packet(head + excerpt(1, 0, 9)
                      + "[o1] bytes 9-18, lines 2-2, omitted: not_selected\n"
                      + excerpt(2, 18, 25)
                      + "[o2] bytes 25-29, lines 3-3, omitted: not_selected\n", 2),
    "added": packet(head + excerpt(1, 0, 9) + excerpt(2, 9, 18) + excerpt(3, 18, 29), 0),
    "joined": packet(head + excerpt(1, 0, 29), 0),
}
print(json.dumps({name: j2_run.checked(one, source, floor)["problems"] for name, one in cases.items()}))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(root().join("scripts"))
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found = cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("json");
    let problems = |case: &str| -> Vec<String> {
        at(&found, &[case])
            .as_array()
            .unwrap_or_else(|| panic!("no {case} in {found:?}"))
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    };
    assert!(
        problems("dropped")
            .iter()
            .any(|problem| problem.contains("drops the baseline's excerpt at 18-29")),
        "{:?}",
        problems("dropped")
    );
    // **A baseline excerpt carried in part is dropped too**: its first
    // bytes are here and its tail is not.
    assert!(
        problems("partial")
            .iter()
            .any(|problem| problem.contains("drops the baseline's excerpt at 18-29")),
        "{:?}",
        problems("partial")
    );
    assert!(problems("added").is_empty(), "{:?}", problems("added"));
    assert!(problems("joined").is_empty(), "{:?}", problems("joined"));
}

#[test]
fn the_harness_refuses_a_ceiling_above_the_hard_cap_and_admits_the_cap_itself() {
    // **m4e's hard cap is J2's too**, and it is read from m4e's harness
    // rather than typed here. Above it is refused before anything starts,
    // in a dry run as in a live one; the cap itself is a ceiling a run may
    // have.
    let cap = declared_in("m4e_run.py", "RUN_CEILING_TOKENS");
    let directory = tempfile::tempdir().expect("temp dir");
    let input = directory.path().join("small.log");
    std::fs::write(&input, test_log(1, 3, &[])).expect("input");
    let written = manifest(
        directory.path(),
        "capped",
        &head(),
        &[("small", &input, "")],
    );
    let run = |ceiling: u64, out: &Path| {
        harness(&[
            "--manifest",
            written.to_str().expect("utf-8"),
            "--out",
            out.to_str().expect("utf-8"),
            "--dry-run",
            "--run-ceiling",
            &ceiling.to_string(),
        ])
    };

    let above = directory.path().join("above");
    let (ok, _, stderr) = run(cap + 1, &above);
    assert!(!ok, "a ceiling above the cap was admitted");
    assert!(
        stderr.contains(&format!("above the hard cap of {cap}")),
        "refused for another reason: {stderr}"
    );
    assert!(!above.exists(), "something was started first");

    let at_cap = directory.path().join("at-cap");
    let (ok, stdout, stderr) = run(cap, &at_cap);
    assert!(ok, "the cap itself was refused: {stdout}\n{stderr}");
    assert_eq!(int(&report(&at_cap), &["run_ceiling_tokens"]), cap as i64);
}

#[test]
fn the_harness_stops_before_a_run_its_ceiling_could_not_cover() {
    // **The stop is checked before a run, against its worst case**, and
    // the worst case is the bound `projection::tests` holds at or above
    // what a projection can reserve.
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let input = directory.path().join("small.log");
    std::fs::write(&input, test_log(1, 3, &[])).expect("input");
    let written = manifest(directory.path(), "tight", &head(), &[("small", &input, "")]);
    let short = (declared("WORST_CASE_PROJECTION_TOKENS") - 1).to_string();
    let (ok, _, stderr) = harness(&[
        "--manifest",
        written.to_str().expect("utf-8"),
        "--out",
        out.to_str().expect("utf-8"),
        "--dry-run",
        "--run-ceiling",
        &short,
    ]);
    assert!(ok, "{stderr}");
    assert!(stderr.contains("STOPPED"), "{stderr}");
    let report = report(&out);
    assert_eq!(at(&report, &["runs"]), &Value::Array(Vec::new()));
    assert!(!out.join("work").exists(), "a run was started");
}

#[test]
fn the_harness_names_every_way_a_projection_can_fail_its_checks() {
    // **A check that has only ever passed has not been shown to check
    // anything.** Every packet a dry run produces is honest, so the
    // harness's own checks are given dishonest ones here: an excerpt that
    // is not the source's bytes, an omission the packet does not declare,
    // a gap in the ledger, a ledger that stops short, and a failure named
    // by bytes that are not at its range. Each must be named.
    let program = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import j2_run
source = b"line one\nline two\nline three\n"
head = ("projection x\nread as lines: 29 bytes, 3 lines, 1 units in 1 parts; "
        "excerpts chosen by the model\nfailures named: 0\n")
excerpt = "[e1] bytes 0-9, lines 1-1, lines\nline one\n\n[end e1]\n"
omitted = "[o1] bytes 9-29, lines 2-3, omitted: not_selected\n"
declared = [{"item_id": "log", "section_id": "s-log.o1", "reason": "applicability"}]
def packet(content, omissions=declared):
    return {"sections": [{"item_id": "log", "content": content}], "omissions": omissions}
cases = {
    "honest": packet(head + excerpt + omitted),
    "altered": packet(head + excerpt.replace("line one", "line 0ne") + omitted),
    "undeclared": packet(head + excerpt + omitted, []),
    "gap": packet(head + excerpt + omitted.replace("9-29", "10-29")),
    "short": packet(head + excerpt + omitted.replace("9-29", "9-20")),
    "misnamed": packet(head.replace("named: 0", "named: 1\n  line two at bytes 0-8")
                       + excerpt + omitted),
}
print(json.dumps({name: j2_run.checked(one, source)["problems"] for name, one in cases.items()}))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(root().join("scripts"))
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found = cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("json");
    let problems = |case: &str| -> Vec<String> {
        at(&found, &[case])
            .as_array()
            .unwrap_or_else(|| panic!("no {case} in {found:?}"))
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    };
    assert!(problems("honest").is_empty(), "{:?}", problems("honest"));
    for (case, named) in [
        ("altered", "is not the source's bytes"),
        (
            "undeclared",
            "the packet's omissions are not the projection's",
        ),
        ("gap", "bytes 9-10 are neither carried nor declared"),
        ("short", "the ledger stops at byte 20 of 29"),
        (
            "misnamed",
            "a named failure at 0-8 is not the source's bytes",
        ),
    ] {
        assert!(
            problems(case).iter().any(|problem| problem.contains(named)),
            "{case}: {:?}",
            problems(case)
        );
    }
}

/// Run the harness **with the fake's usage raised** for any run whose dry
/// answers include `overbilled:`, which is how a dry run's store records
/// an overrun: the fake bills that answer's usage whole.
fn overbilling_harness(usage: u64, arguments: &[&str]) -> (bool, String, String) {
    let program = r#"
import importlib.util, sys
spec = importlib.util.spec_from_file_location("j2_run", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
m4e = sys.modules["m4e_run"]
original = m4e.config_of
def config_of(run, live, dry_answers, replay=False):
    body = original(run, live, dry_answers, replay)
    if "model" in body and any(answer.startswith("overbilled:") for answer in dry_answers):
        body["model"]["usage"] = int(sys.argv[2])
    return body
m4e.config_of = config_of
sys.exit(module.main(sys.argv[3:]))
"#;
    let output = Command::new("python3")
        .args(["-c", program])
        .arg(script())
        .arg(usage.to_string())
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

#[test]
fn no_run_starts_after_a_store_that_recorded_a_stop() {
    // **J2's twin of m4e's rule.** A store that recorded an overrun admits
    // nothing again, so the sequence ends there rather than opening a new
    // store for the next run on the assumption that just failed. The
    // first run's fake overbills its first part.
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("out");
    let input = directory.path().join("cargo-test.log");
    std::fs::write(&input, test_log(8, 100, &[(1, 17), (5, 2)])).expect("input");
    let written = manifest(
        directory.path(),
        "stopped",
        &head(),
        &[("first", &input, ""), ("second", &input, "")],
    );
    let text = std::fs::read_to_string(&written).expect("manifest");
    let first = text.replacen(
        r#""dry_answers":["ids:u1"]"#,
        r#""dry_answers":["overbilled:ids:u1","ids:u1"]"#,
        1,
    );
    assert_ne!(first, text);
    std::fs::write(&written, first).expect("manifest");
    let ceiling = declared_in("m4e_run.py", "RUN_CEILING_TOKENS").to_string();
    let (ok, stdout, stderr) = overbilling_harness(
        200_000,
        &[
            "--manifest",
            written.to_str().expect("utf-8"),
            "--out",
            out.to_str().expect("utf-8"),
            "--dry-run",
            "--run-ceiling",
            &ceiling,
        ],
    );
    assert!(ok, "the dry run failed: {stdout}\n{stderr}");
    let report = report(&out);
    let runs = at(&report, &["runs"]).as_array().expect("runs").len();
    assert_eq!(
        runs, 1,
        "the run after `first` was started; a store that records a stop ends the sequence:\n{stderr}\n{report:?}"
    );
    let stopped = at(&report, &["stopped"]).as_str().unwrap_or_default();
    assert!(
        stopped.contains("first") && stopped.contains("overrun"),
        "the stop does not name the store and what it recorded: {stopped:?}"
    );
    assert!(
        stderr.contains("STOPPED") && stderr.contains("no further run is started"),
        "{stderr}"
    );
}
