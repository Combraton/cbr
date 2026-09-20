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

#[test]
fn a_provider_other_than_the_one_the_owner_named_refuses_the_launch() {
    // MiniMax only. A second provider is a decision, and a decision is not
    // something a configuration file gets to make.
    let (started, message) = launch(
        r#"{"provider":"openai","dialect":"openai","model":"MiniMax-M2.7","endpoint":"https://api.openai.com/v1"}"#,
    );
    assert!(!started, "it started: {message}");
    assert!(message.contains("openai"), "and named it: {message}");
}

#[test]
fn a_model_outside_the_three_refuses_the_launch() {
    // The three are the owner's, chosen per task. A fourth is not a
    // smaller decision because its name looks similar.
    let (started, message) = launch(
        r#"{"provider":"minimax","dialect":"openai","model":"MiniMax-M4","endpoint":"https://api.minimax.io/v1"}"#,
    );
    assert!(!started, "it started: {message}");
    assert!(message.contains("MiniMax-M4"), "and named it: {message}");
}

#[test]
fn an_unknown_dialect_refuses_the_launch() {
    let (started, message) = launch(
        r#"{"provider":"minimax","dialect":"grpc","model":"MiniMax-M2.7","endpoint":"https://api.minimax.io/v1"}"#,
    );
    assert!(!started, "it started: {message}");
    assert!(message.contains("grpc"), "and named it: {message}");
}

#[test]
fn an_endpoint_that_is_not_https_refuses_the_launch() {
    // Repository text goes over this. A plaintext endpoint is a
    // configuration mistake that cannot be detected once it has been made.
    let (started, message) = launch(
        r#"{"provider":"minimax","dialect":"openai","model":"MiniMax-M2.7","endpoint":"http://api.minimax.io/v1"}"#,
    );
    assert!(!started, "it started: {message}");
    assert!(message.contains("http://"), "and named it: {message}");
}

#[test]
#[cfg(not(target_os = "macos"))]
fn a_configured_model_with_no_keychain_is_a_refused_launch() {
    // **The rule this holds:** on Linux and in CI, a configured model is a
    // refused launch with a typed reason — never a fallback to an
    // environment variable, a file, or an unauthenticated call. This runs
    // where there is no Keychain, which is the only place it can be true.
    let (started, message) = launch(
        r#"{"provider":"minimax","dialect":"openai","model":"MiniMax-M2.7","endpoint":"https://api.minimax.io/v1"}"#,
    );
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
