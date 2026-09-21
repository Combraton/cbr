//! The one hand launch [VERIFICATION rule 4] permits.
//!
//! A rule repeated twice and broken twice is not working, so the fix is
//! not a third repetition: it is making the permitted path easier than
//! the forbidden one. `scripts/debug_launch.sh` starts a provider a
//! person can poke at — a throwaway data directory, a socket, a
//! credential — and **cannot be talked into calling a model**: it passes
//! no configuration at all, so there is no file to name a
//! `model_runtime` in, and it refuses `--permit-model-network` and
//! `--calibrate` by name before it starts anything.
//!
//! The tests below are what make that a property rather than a promise.
//! The argument vector is asserted against a stub that records what it
//! was given, so "passes no configuration" is checked at the exec rather
//! than by reading the script for reassuring text.
//!
//! [VERIFICATION rule 4]: ../../../docs/VERIFICATION.md

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod serving;

fn script() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    let script = path.join("scripts").join("debug_launch.sh");
    assert!(script.exists(), "{} is missing", script.display());
    script
}

/// Run the script to completion with whatever binary it should exec.
fn run(binary: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new("sh")
        .arg(script())
        .args(arguments)
        .env("CBR_PROVIDER_BIN", binary)
        .output()
        .expect("the script runs")
}

/// A stand-in for the provider that records the argument vector it was
/// given and exits. Nothing about the launch decision is simulated: the
/// point is exactly what was passed.
fn stub(directory: &std::path::Path) -> PathBuf {
    let record = directory.join("argv");
    let binary = directory.join("stub.sh");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{}'; done\n",
            record.display()
        ),
    )
    .expect("stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).expect("0700");
    }
    binary
}

/// A running child, killed and reaped when the test lets go of it.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_hand_launch_refuses_to_permit_model_network() {
    let directory = tempfile::tempdir().expect("temp dir");
    let output = run(&stub(directory.path()), &["--permit-model-network"]);
    assert!(!output.status.success(), "it started anyway");
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        said.contains("--permit-model-network"),
        "it did not name the flag: {said}"
    );
    assert!(
        said.contains("rule 4"),
        "it did not say where the rule is: {said}"
    );
    assert!(
        !directory.path().join("argv").exists(),
        "it refused and started the provider anyway"
    );
}

#[test]
fn the_hand_launch_refuses_to_calibrate() {
    let directory = tempfile::tempdir().expect("temp dir");
    let output = run(&stub(directory.path()), &["--calibrate", "/dev/null"]);
    assert!(!output.status.success(), "it started anyway");
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        said.contains("--calibrate"),
        "it did not name the flag: {said}"
    );
    assert!(
        !directory.path().join("argv").exists(),
        "it refused and started the provider anyway"
    );
}

#[test]
fn the_hand_launch_refuses_a_configuration_it_was_handed() {
    // The flag that could name a `model_runtime`. Refusing it is what
    // makes "cannot be given one" true of the script rather than of the
    // operator's intentions.
    let directory = tempfile::tempdir().expect("temp dir");
    let output = run(&stub(directory.path()), &["--config", "/dev/null"]);
    assert!(!output.status.success(), "it started anyway");
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        said.contains("--config"),
        "it did not name the flag: {said}"
    );
    assert!(!directory.path().join("argv").exists());
}

#[test]
fn the_configuration_this_launch_runs_under_names_no_model() {
    // Asserted at the exec, over the argument vector the binary was
    // actually given and the file that vector points at, because a
    // script that merely *says* it configures no model is the kind of
    // reassurance this whole rule exists because of.
    let directory = tempfile::tempdir().expect("temp dir");
    let output = run(&stub(directory.path()), &[]);
    assert!(
        output.status.success(),
        "the permitted launch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let argv = std::fs::read_to_string(directory.path().join("argv")).expect("the stub ran");
    let given: Vec<&str> = argv.lines().collect();
    for forbidden in ["--permit-model-network", "--calibrate"] {
        assert!(
            !given.contains(&forbidden),
            "{forbidden} reached the provider: {given:?}"
        );
    }
    assert!(
        given.contains(&"--data-dir"),
        "no data directory: {given:?}"
    );
    assert!(given.contains(&"--socket"), "no socket: {given:?}");

    let at = given
        .iter()
        .position(|argument| *argument == "--config")
        .expect("a configuration");
    let config = std::fs::read(given[at + 1]).expect("the script wrote it");
    let config = cbr_encoding::parse(&config).expect("canonical");
    assert_eq!(
        config.get("format").and_then(cbr_encoding::Value::as_str),
        Some("cbr-config/1"),
        "a conformance format would admit the fake transport"
    );
    for member in ["model_runtime", "model"] {
        assert!(
            config.get(member).is_none(),
            "the launch configuration names {member}"
        );
    }

    // And the temp directory it wrote that into is the operator's to
    // look at, so the suite takes it away again.
    let _ = std::fs::remove_dir_all(
        std::path::Path::new(given[at + 1])
            .parent()
            .expect("directory"),
    );
}

#[test]
fn the_hand_launch_serves_and_prints_what_a_client_needs() {
    // The whole point: what it prints is enough to talk to it. If this
    // is harder than typing a launch by hand, the rule will be broken a
    // third time.
    let mut child = Command::new("sh")
        .arg(script())
        .env("CBR_PROVIDER_BIN", serving::provider_binary())
        // **Held open, because standard input ending is how the provider
        // is stopped** (`socket::serve`). An operator has a terminal; a
        // test has to hold one deliberately or the launch ends before it
        // has listened.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the script runs");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut named = |name: &str| {
        let line = lines
            .next()
            .unwrap_or_else(|| panic!("no `{name}` line at all"))
            .expect("a line");
        PathBuf::from(
            line.strip_prefix(&format!("{name} "))
                .unwrap_or_else(|| panic!("expected a `{name}` line, got {line}"))
                .to_string(),
        )
    };
    let socket = named("socket");
    let credential = named("credential");
    let running = Running(child);

    let started = Instant::now();
    while std::os::unix::net::UnixStream::connect(&socket).is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "it never listened on {}",
            socket.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        credential.exists(),
        "no credential at {}",
        credential.display()
    );

    let input = socket.parent().expect("directory").join("input.txt");
    std::fs::write(&input, b"a file to poke at").expect("input");
    let ingested = Command::new(env!("CARGO_BIN_EXE_cbr"))
        .args(["ingest", input.to_str().expect("utf-8")])
        .arg("--socket")
        .arg(&socket)
        .arg("--credential-file")
        .arg(&credential)
        .output()
        .expect("cbr runs");
    assert!(
        ingested.status.success(),
        "the launch served nothing: {}",
        String::from_utf8_lossy(&ingested.stderr)
    );
    drop(running);
    // The script leaves its throwaway directory for the operator to look
    // at; a suite that did the same would fill the machine with them.
    let _ = std::fs::remove_dir_all(socket.parent().expect("directory"));
}
