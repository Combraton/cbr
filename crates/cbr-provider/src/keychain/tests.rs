//! The gate for the credential path.
//!
//! **No test here reads the owner's key.** Every test that exercises the
//! mechanism injects a fake tool — a script this test wrote — so the suite
//! behaves identically on the owner's machine and in CI, and the real
//! `/usr/bin/security` is never spawned. The two tests that pin the real
//! program and its arguments read them off a built [`Command`] instead of
//! running it.

use std::io::Write as _;
use std::path::PathBuf;

use super::*;

/// A fake Keychain tool: an executable script with `body` in it. The script
/// ignores its arguments, exactly as a stand-in should, so that the fixed
/// arguments are pinned by the command tests rather than by this one.
fn fake_tool(directory: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    let mut file = std::fs::File::create(&path).expect("the fake tool is created");
    write!(file, "#!/bin/sh\n{body}\n").expect("the fake tool is written");
    drop(file);
    let mut mode = std::fs::metadata(&path).expect("stat").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o700);
    std::fs::set_permissions(&path, mode).expect("the fake tool is executable");
    path
}

fn temporary() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

/// The refusal from a read that must not have produced a secret. A `Secret`
/// deliberately has no `PartialEq`: comparing credential bytes is a habit
/// worth not having, and no test here needs one.
fn refusal(read: Result<Secret, Refused>) -> Refused {
    match read {
        Ok(_) => panic!("a credential was produced where the launch should have been refused"),
        Err(refused) => refused,
    }
}

// --- what the child is given -------------------------------------------

#[test]
fn the_tool_is_an_absolute_path_and_never_a_path_lookup() {
    // A relative name would be resolved through `PATH`, which the owner's
    // shell configuration controls and an attacker's `PATH` entry would too.
    assert!(
        std::path::Path::new(TOOL).is_absolute(),
        "{TOOL} is not an absolute path"
    );
    assert_eq!(TOOL, "/usr/bin/security");
    let built = command(std::path::Path::new(TOOL));
    assert_eq!(built.get_program(), TOOL);
}

#[test]
fn the_arguments_are_fixed_and_name_the_owners_service() {
    // The service name is the owner's decision and a constant. If it ever
    // becomes configuration, this test is what has to be deleted first.
    assert_eq!(SERVICE, "minimax_api_key");
    let built = command(std::path::Path::new(TOOL));
    let arguments: Vec<_> = built.get_args().map(|a| a.to_string_lossy()).collect();
    assert_eq!(
        arguments,
        ["find-generic-password", "-w", "-s", "minimax_api_key"],
        "the arguments a child would receive"
    );
}

#[test]
fn the_child_gets_no_environment() {
    // The environment of this process holds whatever the operator's shell
    // put there. The child is given none of it, so that nothing it or its
    // dependencies read can be influenced from outside this code.
    let directory = temporary();
    let witness = directory.path().join("environment");
    // The witness path is baked into the script because the script cannot
    // be told it: that is the property under test.
    let tool = fake_tool(
        directory.path(),
        "security",
        &format!("env > {} \necho the-key", witness.display()),
    );
    // SAFETY: single-threaded test setup, before the child is spawned.
    unsafe { std::env::set_var("CBR_KEYCHAIN_TEST_MARKER", "must-not-reach-the-child") };
    let read = read_from(&tool, Duration::from_secs(5));
    unsafe { std::env::remove_var("CBR_KEYCHAIN_TEST_MARKER") };
    assert!(read.is_ok(), "the fake tool answered: {read:?}");
    let seen = std::fs::read_to_string(&witness).expect("the child wrote its environment");
    assert!(
        !seen.contains("CBR_KEYCHAIN_TEST_MARKER"),
        "the child inherited the environment: {seen:?}"
    );
}

#[test]
fn the_child_gets_nothing_on_standard_input() {
    // Nothing is ever written to this child, so its standard input is the
    // null device rather than an inherited handle. The child therefore
    // reads zero bytes. (What this cannot show on its own is that the
    // parent's own standard input was not already empty; it does show that
    // the child is given no channel to read a secret from.)
    let directory = temporary();
    let witness = directory.path().join("stdin");
    let tool = fake_tool(
        directory.path(),
        "security",
        &format!("cat > {} \necho the-key", witness.display()),
    );
    let read = read_from(&tool, Duration::from_secs(5));
    assert!(read.is_ok(), "the fake tool answered: {read:?}");
    let seen = std::fs::read(&witness).expect("the child wrote what it read");
    assert!(seen.is_empty(), "the child read {} bytes", seen.len());
}

// --- what comes back ----------------------------------------------------

#[test]
fn the_trailing_newline_is_stripped_and_nothing_else_is() {
    // `security -w` prints the password and a newline. The newline is not
    // part of the secret, and everything else is — including any trailing
    // space, which is why this key has one.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "printf 'sk-abc 123\\n'");
    let secret = read_from(&tool, Duration::from_secs(5)).expect("the fake tool answered");
    assert_eq!(secret.expose(), "sk-abc 123");
}

#[test]
fn a_message_on_standard_error_never_reaches_the_refusal() {
    // A failure message from that tool is not ours to vet: it can name a
    // keychain, a path, or an item, and it reaches a log if it reaches an
    // error. So it is discarded at the pipe and the refusal is typed.
    let directory = temporary();
    let tool = fake_tool(
        directory.path(),
        "security",
        "echo 'SecKeychainSearchCopyNext: SECRET-LEAK-CANARY' >&2\nexit 44",
    );
    let refused = read_from(&tool, Duration::from_secs(5)).expect_err("a failing tool refuses");
    assert_eq!(refused, Refused::NotFound);
    let rendered = format!("{refused:?} {}", refused.reason());
    assert!(
        !rendered.contains("CANARY"),
        "the tool's own message reached the refusal: {rendered}"
    );
}

#[test]
fn a_non_zero_exit_refuses_the_launch() {
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "exit 44");
    assert_eq!(
        refusal(read_from(&tool, Duration::from_secs(5))),
        Refused::NotFound
    );
}

#[test]
fn empty_output_refuses_the_launch() {
    // A tool that succeeds and says nothing has not given us a credential,
    // and an empty Authorization header is a request that fails at the
    // provider rather than at the launch, which is the wrong place.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "exit 0");
    assert_eq!(
        refusal(read_from(&tool, Duration::from_secs(5))),
        Refused::Empty
    );
}

#[test]
fn output_that_is_not_a_single_line_refuses_the_launch() {
    // Two lines is not a password. Taking the first would be guessing at
    // which line the secret is, and guessing is how a wrong credential
    // reaches a header.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "printf 'one\\ntwo\\n'");
    assert_eq!(
        refusal(read_from(&tool, Duration::from_secs(5))),
        Refused::Malformed
    );
}

#[test]
fn a_tool_that_hangs_is_killed_and_the_launch_refused() {
    // Without this the launch blocks for ever on a Keychain prompt nobody
    // is there to answer, which on a server is indistinguishable from a
    // hung process.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "sleep 30\necho the-key");
    let started = std::time::Instant::now();
    let refused = read_from(&tool, Duration::from_millis(300)).expect_err("a hung tool refuses");
    assert_eq!(refused, Refused::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the tool was waited on for {:?} rather than killed",
        started.elapsed()
    );
}

#[test]
fn a_tool_that_cannot_be_run_refuses_the_launch() {
    let directory = temporary();
    let missing = directory.path().join("not-installed");
    assert_eq!(
        refusal(read_from(&missing, Duration::from_secs(5))),
        Refused::Unavailable
    );
}

// --- no other source, ever ---------------------------------------------

#[test]
fn there_is_no_fallback_to_the_environment_when_the_tool_fails() {
    // The mutant this kills: a `read()` that falls back to
    // `std::env::var("MINIMAX_API_KEY")`, which is the single most common
    // way a key ends up in a shell history, a CI log and a child process.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "exit 44");
    // SAFETY: single-threaded test setup, before the child is spawned.
    unsafe { std::env::set_var("MINIMAX_API_KEY", "sk-from-the-environment") };
    unsafe { std::env::set_var("minimax_api_key", "sk-from-the-environment") };
    let read = read_from(&tool, Duration::from_secs(5));
    unsafe { std::env::remove_var("MINIMAX_API_KEY") };
    unsafe { std::env::remove_var("minimax_api_key") };
    assert_eq!(
        refusal(read),
        Refused::NotFound,
        "a credential appeared from somewhere other than the Keychain"
    );
}

#[test]
fn a_build_without_a_keychain_refuses_a_configured_model() {
    // macOS has the Keychain and nothing else does. On Linux and in CI a
    // configured model is a refused launch with a typed reason, never a
    // fallback to a file, an environment variable or an unauthenticated
    // call.
    assert_eq!(supported(), cfg!(target_os = "macos"));
    if !supported() {
        assert_eq!(refusal(read()), Refused::NoKeychain);
    }
}

// --- the secret itself --------------------------------------------------

#[test]
fn the_zeroing_guard_overwrites_the_bytes_when_it_is_dropped() {
    // The guard is what both `Secret`'s drop and `Authorization`'s delegate
    // to, and holding it over a buffer the test owns is the only way to
    // watch a drop zero something without reading memory that has already
    // been freed.
    //
    // **The gap this leaves, stated rather than papered over:** each of
    // those two `Drop` bodies is one statement that constructs a guard, and
    // deleting either survives every test here. Nothing in safe Rust can
    // observe a heap buffer after it is released, so that mutant cannot be
    // killed; what can be killed, and is, is the guard doing nothing.
    let mut buffer = *b"sk-minimax-0123456789";
    {
        let _guard = Zeroed(&mut buffer);
    }
    assert_eq!(buffer, [0u8; 21], "the guard left the bytes in place");
}

#[test]
fn the_secret_never_prints_itself() {
    // CORE section 18.1: a credential must not appear in a log, an error,
    // an event or a result. `Debug` is how it would get into all four.
    let directory = temporary();
    let tool = fake_tool(
        directory.path(),
        "security",
        "printf 'sk-PRINTED-CANARY\\n'",
    );
    let secret = read_from(&tool, Duration::from_secs(5)).expect("the fake tool answered");
    let rendered = format!("{secret:?}");
    assert!(
        !rendered.contains("CANARY"),
        "the secret printed itself: {rendered}"
    );
    // And the error type that carries it around is no better a hiding place.
    let carried: Result<Secret, Refused> = Ok(secret);
    assert!(!format!("{carried:?}").contains("CANARY"));
}

// --- only when a model is configured ------------------------------------

#[test]
fn no_model_configured_never_reaches_the_keychain() {
    // **The mutant this kills:** a launch that reads the key and then does
    // not use it. A Keychain read is a prompt, an audit entry and a secret
    // in a process that had no use for one — which is what CI is, what
    // every conformance run is, and what a developer running this suite is.
    //
    // The assertion is that **no child process was spawned at all**: the
    // fake tool creates the witness the moment it runs, so the witness is
    // the spawn, not a counter this code keeps about itself.
    let directory = temporary();
    let witness = directory.path().join("spawned");
    let tool = fake_tool(
        directory.path(),
        "security",
        &format!("echo ran >> {} \necho the-key", witness.display()),
    );
    let held = for_launch(false, || read_from(&tool, Duration::from_secs(5)))
        .expect("no model configured is not a refusal");
    assert!(
        held.is_none(),
        "a credential was read with no model to use it"
    );
    assert!(
        !witness.exists(),
        "a child process was spawned with no model configured"
    );
}

#[test]
fn a_configured_model_reads_the_keychain_once() {
    // The converse, so that the test above cannot pass by the read never
    // happening at all. Once, because the reader is consumed: it is taken
    // by value and there is no second call to make.
    let directory = temporary();
    let witness = directory.path().join("spawned");
    let tool = fake_tool(
        directory.path(),
        "security",
        &format!("echo ran >> {} \necho the-key", witness.display()),
    );
    let held = for_launch(true, || read_from(&tool, Duration::from_secs(5)))
        .expect("the fake tool answered");
    assert!(held.is_some(), "a configured model needs its credential");
    let ran = std::fs::read_to_string(&witness).expect("the child ran");
    assert_eq!(ran.lines().count(), 1, "the tool ran {ran:?}");
}

#[test]
fn a_configured_model_whose_credential_is_unreadable_refuses_the_launch() {
    // Not a warning, not a degraded start: a launch. The alternative is a
    // process that serves requests and fails every model call, which is a
    // configuration error discovered one call at a time.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "exit 44");
    let refused = for_launch(true, || read_from(&tool, Duration::from_secs(5)))
        .expect_err("an unreadable credential refuses the launch");
    assert_eq!(refused, Refused::NotFound);
}

#[test]
fn the_credential_leaves_this_module_as_a_header_value_and_nothing_else() {
    // `expose` is private, so the one shape the secret can be seen in
    // outside this module is the header value it is going into.
    let directory = temporary();
    let tool = fake_tool(directory.path(), "security", "printf 'sk-HEADERCANARY\\n'");
    let secret = read_from(&tool, Duration::from_secs(5)).expect("the fake tool answered");
    let authorization = secret.authorization();
    assert_eq!(authorization.value(), "Bearer sk-HEADERCANARY");
    assert!(!format!("{authorization:?}").contains("CANARY"));
}
