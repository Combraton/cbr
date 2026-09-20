//! The one credential CBR ever reads, and the only way it is read.
//!
//! Named for the store rather than for the secret, so that it is never
//! mistaken for [`crate::credentials`], which is the protocol's own
//! principal credentials and has nothing to do with a provider key.
//!
//! # Why a child process
//!
//! The owner's decision, recorded in [ADR 001 question 12]: the `security`
//! tool at a fixed path with fixed arguments and **no shell**, rather than a
//! Keychain crate. Keychain access control is **per program**, and this
//! binary is unsigned and rebuilt constantly, so an in-process read means
//! either a prompt after every rebuild or an *Always Allow* granted to an
//! unsigned binary — a worse position than trusting Apple's own tool. It is
//! also how the owner's other tools already read this same key, and it adds
//! no dependency to the credential path, which is the last place a
//! dependency is cheap.
//!
//! **"Never passed to a child process" is not violated by this.** The
//! constraint is that the secret is never *handed to* a child, by argument
//! vector, environment or standard input. It is read *back from* one, over a
//! private pipe, and the arguments name only the service.
//!
//! [ADR 001 question 12]: ../../docs/decisions/001-standalone-v0.1-scope-and-stack.md
//!
//! # What this module refuses to do
//!
//! There is **no other source**. Not an environment variable, not a file,
//! not a prompt, on any platform. A configured model whose credential cannot
//! be read is a **refused launch** with a typed reason, which is why every
//! failure below is a [`Refused`] and none of them is a fallback.

use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The Keychain service the owner named. **A constant, not configuration.**
pub const SERVICE: &str = "minimax_api_key";

/// The tool, at an absolute path, never a `PATH` lookup: `PATH` is the
/// operator's to set and an attacker's to prepend to.
pub const TOOL: &str = "/usr/bin/security";

/// Fixed at compile time. No shell, no interpolation, no configuration.
pub const ARGUMENTS: [&str; 4] = ["find-generic-password", "-w", "-s", SERVICE];

/// How long the tool has before it is killed and the launch refused. A
/// Keychain read can block on a prompt nobody is there to answer, and a
/// launch that blocks for ever is indistinguishable from a hung process.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// How often the child is checked while the deadline runs.
const POLL: Duration = Duration::from_millis(5);

/// The most output the tool is allowed to produce. A password is tens of
/// bytes; this is a bound on a stranger's output, not a size expectation.
const MAX_OUTPUT: u64 = 64 * 1024;

/// Why a credential could not be read. **Every one refuses the launch**;
/// none of them is a reason to look somewhere else for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// No Keychain on this platform. Linux and CI land here, and a
    /// configured model there is a refused launch, never a fallback.
    NoKeychain,
    /// The tool exited non-zero. Its own message is discarded unread.
    NotFound,
    /// The tool succeeded and said nothing.
    Empty,
    /// The output was not a single line of text.
    Malformed,
    /// The tool outlived its deadline and was killed.
    TimedOut,
    /// The tool could not be run at all.
    Unavailable,
}

impl Refused {
    /// The typed reason a refused launch reports. **None of these carries a
    /// byte the tool produced** — see [`Self::NotFound`].
    pub fn reason(self) -> &'static str {
        match self {
            Refused::NoKeychain => "keychain_unavailable",
            Refused::NotFound => "credential_not_found",
            Refused::Empty => "credential_empty",
            Refused::Malformed => "credential_malformed",
            Refused::TimedOut => "credential_read_timed_out",
            Refused::Unavailable => "keychain_tool_unavailable",
        }
    }
}

/// Bytes that are overwritten before they are released.
///
/// No `Display`, no `Clone`, no `PartialEq`, no serialization, and a `Debug`
/// that names nothing: [CORE §18.1] forbids a credential in a log, an error,
/// an event or a result, and those are the four traits by which it would
/// reach all of them.
///
/// [CORE §18.1]: https://github.com/Combraton/combraton/blob/main/docs/spec/protocol/CORE.md
pub struct Secret {
    bytes: Vec<u8>,
}

/// The zeroing, as a guard over bytes somebody else owns.
///
/// It is a *borrowing* guard on purpose. [`Secret`]'s own drop delegates to
/// it, and holding one over a buffer on the stack is the only way to watch a
/// drop zero something without reading memory that has already been freed —
/// so this is the shape in which the behaviour can actually be tested.
struct Zeroed<'a>(&'a mut [u8]);

impl Drop for Zeroed<'_> {
    fn drop(&mut self) {
        // Volatile, because a write to a buffer that is about to die is
        // exactly the write an optimiser is entitled to remove.
        for byte in self.0.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        drop(Zeroed(&mut self.bytes));
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(..)")
    }
}

impl Secret {
    fn empty() -> Self {
        Secret { bytes: Vec::new() }
    }

    /// The credential as text, for the **one** place it may go: the
    /// `Authorization` header of a request this process is about to make.
    /// Named so that a reader of any other call site asks why.
    /// **Private on purpose.** The credential leaves this module in
    /// exactly one shape, [`Authorization`], and that shape zeroes itself.
    fn expose(&self) -> &str {
        // Validated as UTF-8 when it was read, so this cannot fail; an
        // empty string rather than a panic if that ever stops being true.
        std::str::from_utf8(&self.bytes).unwrap_or_default()
    }

    /// The `Authorization` header value, and **the only way the credential
    /// leaves this module**.
    ///
    /// It is its own type because `format!("Bearer {}", …)` would put the
    /// credential in an ordinary `String` that is dropped without being
    /// overwritten — a second copy of the secret, with none of the care
    /// taken over the first.
    #[allow(dead_code)]
    pub fn authorization(&self) -> Authorization {
        Authorization(format!("Bearer {}", self.expose()))
    }
}

/// A header value holding the credential, zeroed when it is dropped.
pub struct Authorization(String);

impl Authorization {
    pub fn value(&self) -> &str {
        &self.0
    }
}

impl Drop for Authorization {
    fn drop(&mut self) {
        // Safety: the bytes are overwritten in place and the string is
        // dropped immediately afterwards, so no code observes it as text.
        drop(Zeroed(unsafe { self.0.as_bytes_mut() }));
    }
}

impl std::fmt::Debug for Authorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Authorization(..)")
    }
}

/// Whether this build can reach a Keychain at all.
///
/// macOS has one and nothing else does. This is not a capability check at
/// run time: on every other platform a configured model is refused, so there
/// is no path on which CBR looks for the key somewhere else.
pub fn supported() -> bool {
    cfg!(target_os = "macos")
}

/// The command, built in one place so a test can read back exactly what a
/// child would be given without running one.
fn command(tool: &Path) -> Command {
    let mut command = Command::new(tool);
    command
        .args(ARGUMENTS)
        // Nothing this process inherited is any of the child's business,
        // and an inherited variable is a way to steer a tool from outside.
        .env_clear()
        // Nothing is ever written to it, so it is given nothing to read.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // **Discarded, not captured.** A failure message from that tool can
        // name a keychain, a path or an item, and anything captured here
        // ends up in the refusal and from there in a log.
        .stderr(Stdio::null());
    command
}

/// Read the credential from `tool`, killing it after `timeout`.
///
/// # Why the reader is never waited on without a deadline
///
/// Killing the child does **not** close the pipe if the child left a
/// grandchild holding it. A first version joined the reader thread
/// unconditionally after the kill and blocked for the grandchild's full
/// lifetime — a launch that hangs, which is the exact failure the timeout
/// exists to prevent, arriving one step further along. So every wait here
/// is bounded by the same deadline, and a reader still running at it is
/// **detached rather than joined**: it owns a [`Secret`], so whatever it
/// read is zeroed when it finally ends.
fn read_from(tool: &Path, timeout: Duration) -> Result<Secret, Refused> {
    let deadline = Instant::now() + timeout;
    let mut child = command(tool).spawn().map_err(|_| Refused::Unavailable)?;
    let stdout = child.stdout.take().ok_or(Refused::Unavailable)?;
    // Drained on its own thread so that a tool which fills the pipe cannot
    // deadlock against a parent waiting for it to exit.
    let reader = std::thread::spawn(move || {
        let mut secret = Secret::empty();
        let _ = stdout.take(MAX_OUTPUT).read_to_end(&mut secret.bytes);
        secret
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                drop(reader);
                return Err(Refused::TimedOut);
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => return Err(Refused::Unavailable),
        }
    };
    // **From here every exit zeroes the bytes**, because they are already a
    // `Secret` and a `Secret` zeroes when it is dropped. A refusal below
    // drops it; only the last line hands it on.
    let mut secret = loop {
        if reader.is_finished() {
            break reader.join().unwrap_or_else(|_| Secret::empty());
        }
        if Instant::now() >= deadline {
            drop(reader);
            return Err(Refused::TimedOut);
        }
        std::thread::sleep(POLL);
    };
    if !status.success() {
        return Err(Refused::NotFound);
    }
    // Exactly one trailing newline, which is the tool's framing rather than
    // part of the password. Nothing else is trimmed: a trailing space is a
    // character of the secret.
    if secret.bytes.last() == Some(&b'\n') {
        secret.bytes.pop();
    }
    if secret.bytes.is_empty() {
        return Err(Refused::Empty);
    }
    // A second line means this is not a password. Taking the first would be
    // guessing which line the secret is on.
    if secret.bytes.contains(&b'\n') || std::str::from_utf8(&secret.bytes).is_err() {
        return Err(Refused::Malformed);
    }
    Ok(secret)
}

/// The credential this launch holds, if any.
///
/// **The only place a credential read is decided.** `read` is taken by
/// value, so there is no second call to make, and it is not called at all
/// unless a model is configured: a launch with none never touches the
/// Keychain, which is what CI, every conformance run and every developer
/// running this suite are.
#[allow(dead_code)]
pub fn for_launch(
    model_configured: bool,
    read: impl FnOnce() -> Result<Secret, Refused>,
) -> Result<Option<Secret>, Refused> {
    if !model_configured {
        return Ok(None);
    }
    read().map(Some)
}

/// Read the credential. Called **once, at construction**, and only when a
/// model is configured.
#[allow(dead_code)]
pub fn read() -> Result<Secret, Refused> {
    if !supported() {
        return Err(Refused::NoKeychain);
    }
    read_from(Path::new(TOOL), TIMEOUT)
}

#[cfg(test)]
mod tests;
