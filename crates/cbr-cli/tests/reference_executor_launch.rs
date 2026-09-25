//! The reference-executor route, and the two places that keep it inside the
//! composition suite.
//!
//! `conformance/participants/cbr-with-reference-executor-unix.json` launches
//! every participant through `scripts/reference_executor_launch.py`, which
//! execs either CBR's provider or the protocol's reference provider. The
//! descriptor claims the `execution/1` profile on the reference provider's
//! behalf, so a launch outside a composition would let a CBR-named
//! participant pass an executor fixture that CBR never served: the
//! misreporting RELEASE-SCOPE §6 forbids. Two refusals prevent it, and the
//! tests below are what make them properties rather than promises:
//!
//! - the launcher sends a configuration carrying `executor` to the
//!   reference provider only when the runner launched a *named* composition
//!   participant (its `start_participant` gives participant N the data
//!   directory `<work>/participants/N/data`, the configuration
//!   `<work>/participants/N/config.json` and the socket `<work>/n-N/p.sock`),
//!   and refuses it anywhere else rather than start anything;
//! - `scripts/run_fixtures.py` refuses that descriptor, before anything
//!   runs, unless the filter selects composition fixtures only.
//!
//! Each script runs from a copy in a temporary tree, so the binaries it
//! finds are stubs that record which one was started and with what
//! arguments. The route is asserted at the exec, not by reading the script.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn executable(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).expect("0700");
}

/// A stand-in binary that records its own name and argument vector.
fn stub(path: &Path, record: &Path, label: &str) {
    executable(
        path,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{label}' >> '{record}'\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{record}'; done\n",
            record = record.display()
        ),
    );
}

fn sha256(path: &Path) -> String {
    let output = Command::new("python3")
        .args([
            "-c",
            "import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], 'rb').read()).hexdigest())",
        ])
        .arg(path)
        .output()
        .expect("python3 runs");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

/// A temporary checkout: the launcher, a stub `cbr-provider`, a stub
/// reference provider with its schemas, and the `runner.json` that
/// `build_runner.py --reference-executor` writes.
struct Tree {
    dir: tempfile::TempDir,
}

const REFERENCE: &str = "target/protocol-release/release/target/debug/combraton-reference-provider";
const SCHEMAS: &str = "target/protocol-release/release/schemas";

impl Tree {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let tree = Tree { dir };
        let scripts = tree.path().join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts");
        std::fs::copy(
            root().join("scripts").join("reference_executor_launch.py"),
            scripts.join("reference_executor_launch.py"),
        )
        .expect("copy the launcher");
        stub(
            &tree.path().join("target/debug/cbr-provider"),
            &tree.record(),
            "cbr-provider",
        );
        stub(
            &tree.path().join(REFERENCE),
            &tree.record(),
            "reference-provider",
        );
        std::fs::create_dir_all(tree.path().join(SCHEMAS)).expect("schemas");
        let digest = sha256(&tree.path().join(REFERENCE));
        tree.write_identity(&format!(
            r#"{{"format": "cbr-runner-identity/1", "reference_executor": {{"binary": "{REFERENCE}", "schemas": "{SCHEMAS}", "label": "reference executor", "sha256": "{digest}"}}}}"#
        ));
        tree
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn record(&self) -> PathBuf {
        self.path().join("started")
    }

    fn identity(&self) -> PathBuf {
        self.path().join("target/protocol-release/runner.json")
    }

    fn write_identity(&self, text: &str) {
        std::fs::create_dir_all(self.identity().parent().expect("parent")).expect("mkdir");
        std::fs::write(self.identity(), text).expect("runner.json");
    }

    /// What was started, if anything: the stub's label, then its arguments.
    fn started(&self) -> Option<Vec<String>> {
        std::fs::read_to_string(self.record())
            .ok()
            .map(|text| text.lines().map(str::to_string).collect())
    }

    fn launch(&self, arguments: &[&str]) -> Output {
        Command::new("python3")
            .arg(self.path().join("scripts/reference_executor_launch.py"))
            .args(arguments)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .expect("python3 runs")
    }

    /// The paths the pinned runner's `start_participant` gives participant
    /// `name` (exec.rs), with a configuration written where it says.
    fn named(&self, name: &str, config: &str) -> [String; 3] {
        let work = self.path().join("work");
        let directory = work.join("participants").join(name);
        std::fs::create_dir_all(&directory).expect("participant dir");
        std::fs::write(directory.join("config.json"), config).expect("config");
        [
            directory.join("data").display().to_string(),
            directory.join("config.json").display().to_string(),
            work.join(format!("n-{name}"))
                .join("p.sock")
                .display()
                .to_string(),
        ]
    }

    /// The paths the runner's `start` and `expect_start_failure` give the
    /// single participant of a fixture: `<work>/data` (or `data-G` after a
    /// fresh data directory), `<work>/config.json` and `<work>/sN/p.sock`.
    fn single(&self, data: &str, config: &str) -> [String; 3] {
        let work = self.path().join("work");
        std::fs::create_dir_all(&work).expect("work dir");
        std::fs::write(work.join("config.json"), config).expect("config");
        [
            work.join(data).display().to_string(),
            work.join("config.json").display().to_string(),
            work.join("s1").join("p.sock").display().to_string(),
        ]
    }

    fn launch_at(&self, [data, config, socket]: &[String; 3]) -> Output {
        self.launch(&["--data-dir", data, "--config", config, "--socket", socket])
    }
}

const EXECUTOR: &str = r#"{"provider_id": "executor-one", "executor": {"script": []}}"#;
const PROVIDER: &str = r#"{"provider_id": "context-one"}"#;

fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_refused(tree: &Tree, output: &Output, needle: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2, got {:?}: {}",
        output.status,
        said(output)
    );
    assert!(
        said(output).contains(needle),
        "the refusal does not say {needle:?}: {}",
        said(output)
    );
    assert_eq!(
        tree.started(),
        None,
        "it refused and started a binary anyway"
    );
}

#[test]
fn a_named_composition_executor_is_the_reference_provider() {
    let tree = Tree::new();
    let paths = tree.named("executor", EXECUTOR);
    let output = tree.launch_at(&paths);
    assert!(output.status.success(), "{}", said(&output));
    let started = tree.started().expect("something started");
    assert_eq!(started[0], "reference-provider", "{started:?}");
    // The launcher resolves its own checkout, so the schemas path is canonical.
    let schemas = tree
        .path()
        .canonicalize()
        .expect("canonical")
        .join(SCHEMAS)
        .display()
        .to_string();
    assert_eq!(
        started[1..],
        [
            "--data-dir",
            paths[0].as_str(),
            "--config",
            paths[1].as_str(),
            "--schemas",
            schemas.as_str(),
            "--socket",
            paths[2].as_str(),
        ]
    );
    assert!(
        said(&output).contains("reference executor"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_named_composition_participant_without_executor_is_cbr() {
    let tree = Tree::new();
    let paths = tree.named("context", PROVIDER);
    let output = tree.launch_at(&paths);
    assert!(output.status.success(), "{}", said(&output));
    let started = tree.started().expect("something started");
    assert_eq!(
        started,
        [
            "cbr-provider",
            "--data-dir",
            paths[0].as_str(),
            "--config",
            paths[1].as_str(),
            "--socket",
            paths[2].as_str(),
        ]
    );
}

#[test]
fn an_executor_configuration_outside_a_composition_is_refused() {
    // The misuse the verifier reproduced: execution.requires-core-features
    // through this descriptor passed on the reference provider under a
    // CBR-named participant, with CBR serving nothing.
    for data in ["data", "data-1"] {
        let tree = Tree::new();
        let output = tree.launch_at(&tree.single(data, EXECUTOR));
        assert_refused(&tree, &output, "composition");
    }
}

#[test]
fn an_executor_configuration_whose_paths_disagree_is_refused() {
    // Each of the three paths must be the one start_participant gives the
    // same participant; one that is not is not evidence of a composition.
    let tree = Tree::new();
    let [data, config, socket] = tree.named("executor", EXECUTOR);
    let work = tree.path().join("work");
    let elsewhere = work.join("config.json");
    std::fs::write(&elsewhere, EXECUTOR).expect("config");
    let cases = [
        [
            data.clone(),
            elsewhere.display().to_string(),
            socket.clone(),
        ],
        [
            data.clone(),
            config.clone(),
            work.join("s1/p.sock").display().to_string(),
        ],
        [
            data.clone(),
            config.clone(),
            work.join("n-other/p.sock").display().to_string(),
        ],
        [
            work.join("participants/executor/state")
                .display()
                .to_string(),
            config,
            socket,
        ],
    ];
    for paths in &cases {
        let output = tree.launch_at(paths);
        assert_refused(&tree, &output, "composition");
    }
}

#[test]
fn a_single_participant_without_executor_is_cbr() {
    let tree = Tree::new();
    let paths = tree.single("data", PROVIDER);
    let output = tree.launch_at(&paths);
    assert!(output.status.success(), "{}", said(&output));
    assert_eq!(tree.started().expect("started")[0], "cbr-provider");
}

#[test]
fn a_malformed_argument_vector_starts_nothing() {
    let tree = Tree::new();
    let [data, config, socket] = tree.named("executor", EXECUTOR);
    let cases: Vec<Vec<&str>> = vec![
        vec![],
        vec!["--data-dir", &data, "--config", &config],
        vec![
            "--config",
            &config,
            "--data-dir",
            &data,
            "--socket",
            &socket,
        ],
        vec![
            "--data-dir",
            &data,
            "--config",
            &config,
            "--socket",
            &socket,
            "--mutant",
            "x",
        ],
    ];
    for arguments in cases {
        let output = tree.launch(&arguments);
        assert_refused(&tree, &output, "expected exactly");
    }
}

#[test]
fn an_unreadable_configuration_starts_nothing() {
    let tree = Tree::new();
    let paths = tree.named("executor", "not json");
    assert_refused(&tree, &tree.launch_at(&paths), "launch configuration");
    let paths = tree.named("array", "[]");
    assert_refused(&tree, &tree.launch_at(&paths), "not an object");
}

#[test]
fn a_missing_runner_identity_starts_nothing() {
    let tree = Tree::new();
    std::fs::remove_file(tree.identity()).expect("remove");
    let output = tree.launch_at(&tree.named("executor", EXECUTOR));
    assert_refused(&tree, &output, "build_runner.py --reference-executor");
}

#[test]
fn a_runner_identity_without_the_reference_executor_starts_nothing() {
    let tree = Tree::new();
    tree.write_identity(r#"{"format": "cbr-runner-identity/1"}"#);
    let output = tree.launch_at(&tree.named("executor", EXECUTOR));
    assert_refused(&tree, &output, "build_runner.py --reference-executor");
}

#[test]
fn a_runner_identity_without_the_reference_digest_starts_nothing() {
    let tree = Tree::new();
    tree.write_identity(&format!(
        r#"{{"format": "cbr-runner-identity/1", "reference_executor": {{"binary": "{REFERENCE}", "schemas": "{SCHEMAS}", "label": "reference executor"}}}}"#
    ));
    let output = tree.launch_at(&tree.named("executor", EXECUTOR));
    assert_refused(&tree, &output, "records no sha256");
}

#[test]
fn a_missing_reference_provider_starts_nothing() {
    let tree = Tree::new();
    std::fs::remove_file(tree.path().join(REFERENCE)).expect("remove");
    let output = tree.launch_at(&tree.named("executor", EXECUTOR));
    assert_refused(&tree, &output, "reference executor missing");
}

#[test]
fn a_reference_provider_that_is_not_the_recorded_one_starts_nothing() {
    // runner.json records the binary's SHA-256; a rebuilt or replaced
    // binary is not the one the identity names.
    let tree = Tree::new();
    stub(
        &tree.path().join(REFERENCE),
        &tree.path().join("other"),
        "replaced",
    );
    let output = tree.launch_at(&tree.named("executor", EXECUTOR));
    assert_refused(&tree, &output, "sha256");
}

#[test]
fn a_missing_cbr_provider_starts_nothing() {
    let tree = Tree::new();
    std::fs::remove_file(tree.path().join("target/debug/cbr-provider")).expect("remove");
    let output = tree.launch_at(&tree.named("context", PROVIDER));
    assert_refused(&tree, &output, "cbr-provider missing");
}

/// `run_fixtures.py` from a temporary tree whose `vendor/` is the real
/// pinned one (read only) and which has no runner: a run that gets past
/// the descriptor check stops at "no runner" without starting anything.
fn run_fixtures(filter: &str, participant: &str) -> (Output, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let scripts = dir.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("scripts");
    for name in ["run_fixtures.py", "result_paths.py"] {
        std::fs::copy(root().join("scripts").join(name), scripts.join(name)).expect("copy");
    }
    std::os::unix::fs::symlink(root().join("vendor"), dir.path().join("vendor")).expect("vendor");
    let participants = dir.path().join("conformance/participants");
    std::fs::create_dir_all(&participants).expect("participants");
    for name in [
        "cbr-with-reference-executor-unix.json",
        "cbr-provider-unix.json",
    ] {
        std::fs::copy(
            root().join("conformance/participants").join(name),
            participants.join(name),
        )
        .expect("copy descriptor");
    }
    // A copy under another name is the same descriptor.
    std::fs::copy(
        root().join("conformance/participants/cbr-with-reference-executor-unix.json"),
        participants.join("renamed.json"),
    )
    .expect("copy descriptor");
    let out = dir.path().join("results");
    let output = Command::new("python3")
        .arg(scripts.join("run_fixtures.py"))
        .args(["--filter", filter, "--participant", participant, "--out"])
        .arg(&out)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("python3 runs");
    (output, out, dir)
}

const WITH_REFERENCE: &str = "conformance/participants/cbr-with-reference-executor-unix.json";

#[test]
fn run_fixtures_refuses_the_reference_executor_descriptor_outside_composition() {
    for participant in [WITH_REFERENCE, "conformance/participants/renamed.json"] {
        for filter in [
            "execution.",
            "execution.requires-core-features",
            "",
            "core.",
            "composition",
            "socket.composition.",
        ] {
            let (output, out, _dir) = run_fixtures(filter, participant);
            assert!(!output.status.success(), "{filter:?} was not refused");
            let text = said(&output);
            assert!(
                text.contains("composition.") && text.contains("reference executor"),
                "{participant} with {filter:?} was not refused for the reference executor: {text}"
            );
            assert!(!out.exists(), "it refused after creating {}", out.display());
        }
    }
}

#[test]
fn run_fixtures_lets_the_reference_executor_descriptor_run_composition() {
    for filter in [
        "composition.",
        "composition.executor-fetches-bound-packet-and-seals-outputs",
    ] {
        let (output, _out, _dir) = run_fixtures(filter, WITH_REFERENCE);
        let text = said(&output);
        assert!(
            text.contains("no runner") && !text.contains("reference executor"),
            "{filter:?} did not get past the descriptor check: {text}"
        );
    }
}

#[test]
fn run_fixtures_leaves_other_descriptors_alone() {
    let (output, _out, _dir) = run_fixtures(
        "execution.",
        "conformance/participants/cbr-provider-unix.json",
    );
    let text = said(&output);
    assert!(
        text.contains("no runner") && !text.contains("reference executor"),
        "an ordinary descriptor was held to the composition rule: {text}"
    );
}
