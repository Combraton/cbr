//! **No test in this suite launches a serving provider with a real model.**
//!
//! Until m4c the rule held itself: while `launch::SERVING_CALLS_A_MODEL`
//! was false, a serving launch with a model configured was *refused*, so
//! the worst a careless test could do was fail. m4c set that constant
//! true, and with it `--permit-model-network` plus a valid `model_runtime`
//! and no `--calibrate` became a launch that **reads the owner's Keychain
//! and serves**.
//!
//! That is the exact shape of the lapse recorded in
//! [VERIFICATION](../../../docs/VERIFICATION.md): a careless probe of the
//! binary with a valid model configuration and the permit flag, which read
//! the owner's real key for a process that could not use it. Nothing was
//! sent and the bytes were zeroed, and it was still a read that should not
//! have been possible.
//!
//! So the rule moves from the constant into a test that reads the sources.
//! Every place a test passes `--permit-model-network` must be one of:
//!
//! * gated off macOS, where there is no Keychain to read; or
//! * asking for `--calibrate`, which is refused before the credential
//!   unless it was given a ceiling as well, and which the owner runs
//!   deliberately; or
//! * carrying no configured model at all, which is refused outright —
//!   and a piece that passes `--live` to the m4e harness is not one of
//!   these, because that flag makes the harness write a configuration
//!   naming a model before it launches.
//!
//! **What this does not do.** It reads text, so it can be fooled by a
//! program that builds the flag out of pieces. It is a guard against
//! carelessness, which is what the lapse was, and not against intent.

use std::path::{Path, PathBuf};

/// The literal as an argument, which is what a launch passes. The same
/// characters inside a longer string — an assertion about a refusal
/// message, say — are not a launch and are not matched.
const FLAG: &str = "\"--permit-model-network\"";
const CALIBRATE: &str = "\"--calibrate\"";

fn workspace() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// Every `tests/*.rs` of every crate, and every `src` file that could
/// start a process.
fn sources() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let crates = workspace().join("crates");
    for entry in std::fs::read_dir(&crates).expect("crates") {
        let directory = entry.expect("entry").path();
        for sub in ["tests", "src"] {
            collect(&directory.join(sub), &mut found);
        }
    }
    assert!(found.len() > 10, "found almost nothing: {found:?}");
    found
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// Split a source into `#[test]`-sized pieces, each carrying the
/// attributes that precede it.
fn tests(text: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("#[test]") {
        // Back up over the attributes and doc comments above it, which is
        // where a `#[cfg(...)]` gate sits.
        let head = &rest[..at];
        let from = head.rfind("\n\n").map_or(0, |blank| blank + 1);
        let piece = &rest[from..];
        let to = piece[7..]
            .find("\n#[test]")
            .map_or(piece.len(), |next| next + 7);
        pieces.push(&piece[..to]);
        rest = &rest[at + 7..];
    }
    pieces
}

#[test]
fn no_test_launches_a_serving_provider_with_a_real_model() {
    let mut checked = 0;
    for path in sources() {
        let text = std::fs::read_to_string(&path).expect("reads");
        if !text.contains(FLAG) {
            continue;
        }
        for piece in tests(&text) {
            if !piece.contains(FLAG) {
                continue;
            }
            checked += 1;
            let gated_off_macos = piece.contains("#[cfg(not(target_os = \"macos\"))]");
            let asks_for_the_calibration = piece.contains(CALIBRATE);
            // A configuration with no `model_runtime` member cannot reach
            // a credential: the launch is refused for permitting calls to
            // nothing.
            //
            // **`--live` counts as one**, and the reason is the hole this
            // rule had at m4e. `scripts/m4e_run.py --live` *writes* a
            // production configuration naming a `model_runtime` and then
            // launches under it, so a test driving the harness configures
            // a model without the word appearing anywhere in the piece.
            // The escape above was written for a Rust launch whose whole
            // configuration is visible; it is not a licence for a
            // launcher that builds its own.
            let configures_a_model = piece.contains("model_runtime")
                || piece.contains("CONFIGURED")
                || piece.contains("\"--live\"");
            assert!(
                gated_off_macos || asks_for_the_calibration || !configures_a_model,
                "{} passes --permit-model-network with a configured model, not gated off \
                 macOS and not asking for the calibration. On the owner's machine that \
                 launch now reads their Keychain and serves. The piece was:\n{piece}",
                path.display()
            );
        }
    }
    assert!(
        checked >= 4,
        "the rule found almost nothing to hold; has the flag been renamed? {checked}"
    );
}

#[test]
fn the_only_place_a_credential_is_read_is_the_one_function_that_reads_it() {
    // The other half, and the reason the rule above is worth stating: a
    // credential comes from one place, guarded by one decision. A second
    // reader — an environment variable, a file, a second Keychain call —
    // would make every rule about launches a rule about only one of them.
    let text = std::fs::read_to_string(workspace().join("crates/cbr-provider/src/main.rs"))
        .expect("reads main");
    assert_eq!(
        text.matches("keychain::for_launch").count(),
        1,
        "a credential is read in exactly one place"
    );
    assert!(
        text.contains("keychain::for_launch(decision.reads_credential()"),
        "and only when the launch decision says so"
    );
    for forbidden in [
        "std::env::var(\"MINIMAX",
        "env::var(\"MINIMAX",
        "MINIMAX_API_KEY",
    ] {
        assert!(
            !text.contains(forbidden),
            "{forbidden} is a second way to a credential"
        );
    }
}
