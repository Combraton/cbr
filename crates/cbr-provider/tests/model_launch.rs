//! What a launch does about a configured model, before it serves anything.
//!
//! **No test here reads the owner's Keychain.** The four configuration tests
//! are refused during parsing, which happens before any credential read, so
//! they behave the same on every machine. The one test that reaches the
//! credential is gated to platforms that have no Keychain at all — which is
//! CI, and is where the refused-launch rule has to hold.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cbr-provider")
}

/// Launch with this `model_runtime` member and return what it said and
/// whether it started.
fn launch(model_runtime: &str) -> (bool, String) {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"cbr-config/1","principal":"owner","model_runtime":{model_runtime}}}"#
        ),
    )
    .expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("data"))
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Launch with these arguments and this `model_runtime`, and say what
/// happened. Every case below is refused **before** the credential is read,
/// which is why they are safe to run on the owner's own machine.
fn launch_with(arguments: &[&str], model_runtime: &str) -> (bool, String) {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{"format":"cbr-config/1","principal":"owner","model_runtime":{model_runtime}}}"#
        ),
    )
    .expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("data"))
        .arg("--config")
        .arg(&config)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

const CONFIGURED: &str = r#"{"provider":"minimax","dialect":"openai","model":"MiniMax-M2.7"}"#;

#[test]
fn a_provider_other_than_the_one_the_owner_named_refuses_the_launch() {
    // MiniMax only. A second provider is a decision, and a decision is not
    // something a configuration file gets to make.
    let (started, message) =
        launch(r#"{"provider":"openai","dialect":"openai","model":"MiniMax-M2.7"}"#);
    assert!(!started, "it started: {message}");
    assert!(message.contains("openai"), "and named it: {message}");
}

#[test]
fn a_model_outside_the_three_refuses_the_launch() {
    // The three are the owner's, chosen per task. A fourth is not a
    // smaller decision because its name looks similar.
    let (started, message) =
        launch(r#"{"provider":"minimax","dialect":"openai","model":"MiniMax-M4"}"#);
    assert!(!started, "it started: {message}");
    assert!(message.contains("MiniMax-M4"), "and named it: {message}");
}

#[test]
fn an_unknown_dialect_refuses_the_launch() {
    let (started, message) =
        launch(r#"{"provider":"minimax","dialect":"grpc","model":"MiniMax-M2.7"}"#);
    assert!(!started, "it started: {message}");
    assert!(message.contains("grpc"), "and named it: {message}");
}

#[test]
fn an_endpoint_in_the_configuration_refuses_the_launch_whatever_it_says() {
    // **"MiniMax only" was a label.** Each of these names the provider
    // correctly and addresses somebody else, and each one would have sent
    // the owner's credential and repository text there:
    //
    //   - another host outright;
    //   - a lookalike that starts with the pinned host;
    //   - the userinfo form, which reads like the pinned host and is not;
    //   - and the counting endpoint, which is a second address to move.
    //
    // The fix is not to validate them. It is that **an endpoint is not
    // configuration**: the host is a constant and the path is the
    // dialect's, so there is nothing here to point anywhere. Refused
    // rather than ignored, so nobody believes they set one.
    for endpoint in [
        r#""endpoint":"https://collector.example/v1""#,
        r#""endpoint":"https://api.minimax.io.collector.example/v1""#,
        r#""endpoint":"https://api.minimax.io@collector.example/v1""#,
        r#""endpoint":"http://api.minimax.io/v1""#,
        r#""endpoint":"https://api.minimax.io:8443/v1""#,
        r#""count_endpoint":"https://collector.example/count""#,
        r#""host":"collector.example""#,
        r#""base_url":"https://collector.example""#,
    ] {
        let (started, message) = launch(&format!(
            r#"{{"provider":"minimax","dialect":"openai","model":"MiniMax-M2.7",{endpoint}}}"#
        ));
        assert!(!started, "{endpoint} started the launch: {message}");
        assert!(
            message.contains("not configuration") && message.contains("api.minimax.io"),
            "{endpoint}: {message}"
        );
    }
}

#[test]
fn a_model_runtime_with_only_a_dialect_is_all_a_launch_may_choose() {
    // The positive half, so the refusals above cannot pass by everything
    // being refused. Both dialects are accepted with no address at all.
    //
    // Gated off macOS: on the owner's machine this configuration is valid
    // and the launch would read their real Keychain entry. See
    // VERIFICATION, "The model runtime".
    for dialect in ["openai", "anthropic"] {
        let (started, message) = launch(&format!(
            r#"{{"provider":"minimax","dialect":"{dialect}","model":"MiniMax-M2.7"}}"#
        ));
        assert!(!started, "{dialect}: {message}");
        // It reaches the network gate, which is **after** the configuration
        // was accepted and **before** the credential is read. So this says
        // the configuration is valid without touching a Keychain, on every
        // platform including the owner's own machine.
        assert!(
            message.contains("--permit-model-network was not given"),
            "{dialect}: {message}"
        );
        assert!(
            !message.contains("keychain") && !message.contains("credential"),
            "{dialect}: it reached the credential: {message}"
        );
    }
}

#[test]
#[cfg(not(target_os = "macos"))]
fn a_configured_model_with_no_keychain_is_a_refused_launch() {
    // **The rule this holds:** on Linux and in CI, a configured model is a
    // refused launch with a typed reason — never a fallback to an
    // environment variable, a file, or an unauthenticated call. This runs
    // where there is no Keychain, which is the only place it can be true.
    //
    // **And it is the only test that passes `--permit-model-network`**,
    // which is why it is gated: on a machine with a Keychain this launch
    // would read the owner's real key. Off macOS there is nothing to read.
    let (started, message) = launch_with(&["--permit-model-network"], CONFIGURED);
    assert!(!started, "it started without a credential: {message}");
    assert!(
        message.contains("keychain_unavailable"),
        "and gave the typed reason: {message}"
    );
}

#[test]
fn no_model_configured_starts_and_never_mentions_a_credential() {
    // The ordinary case, and the one every conformance run and every
    // developer is: no model, no Keychain, no credential anywhere.
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("config.json");
    std::fs::write(&config, r#"{"format":"cbr-config/1","principal":"owner"}"#).expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("data"))
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert!(
        output.status.success(),
        "a launch with no model configured serves: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let message = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        !message.contains("credential") && !message.contains("keychain"),
        "a launch with no model said something about a credential: {message}"
    );
}

#[test]
fn the_calibration_refuses_every_way_of_being_asked_for_carelessly() {
    // **Each of these is checked before the Keychain is touched**, which is
    // what makes them safe to run on the owner's machine: a launch that was
    // never going to run does not read their key to find that out.
    let table = "/dev/null";
    for (arguments, expected) in [
        (vec!["--calibrate", table], "needs --permit-model-network"),
        (
            vec!["--calibrate", table, "--permit-model-network"],
            "needs --model-run-ceiling",
        ),
        (
            vec![
                "--calibrate",
                table,
                "--permit-model-network",
                "--model-run-ceiling",
                "100001",
            ],
            "above the calibration's cap",
        ),
    ] {
        let (started, message) = launch_with(&arguments, CONFIGURED);
        assert!(!started, "{arguments:?} ran: {message}");
        assert!(message.contains(expected), "{arguments:?}: {message}");
        assert!(
            !message.contains("keychain") && !message.contains("credential"),
            "{arguments:?} reached the credential: {message}"
        );
    }
}

#[test]
fn the_calibration_needs_a_configured_model() {
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("config.json");
    std::fs::write(&config, r#"{"format":"cbr-config/1","principal":"owner"}"#).expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("data"))
        .arg("--config")
        .arg(&config)
        .args(["--calibrate", "/dev/null", "--permit-model-network"])
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("needs a configured model"), "{message}");
}

#[test]
fn permitting_the_network_with_no_model_configured_is_refused() {
    // Permitting calls to nothing is a configuration mistake rather than a
    // safe default, and a launch that says it is about to make live calls
    // and then does not is one nobody can reason about.
    let directory = tempfile::tempdir().expect("temp dir");
    let config = directory.path().join("config.json");
    std::fs::write(&config, r#"{"format":"cbr-config/1","principal":"owner"}"#).expect("config");
    let output = Command::new(binary())
        .arg("--data-dir")
        .arg(directory.path().join("data"))
        .arg("--config")
        .arg(&config)
        .arg("--permit-model-network")
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("needs a configured model"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
