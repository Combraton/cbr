//! Test barriers and signals for deterministic interleavings (decision 007).
//!
//! Inert unless the conformance launch configuration names `test_barriers`,
//! which a production configuration refuses. A **barrier** pauses the thread
//! that reaches it, once, until the runner creates `<name>.release` in the
//! barrier directory; a **signal** marks a point by creating `<name>.signal`
//! and never pauses. A watchdog continues after 30 seconds, so a runner that
//! vanished cannot leave the provider paused forever.
//!
//! A barrier is declared in the participant descriptor only once the point it
//! names exists in this code, because a declared barrier nobody reaches would
//! turn a fixture's `await_barrier` into a timeout rather than an
//! `unsupported`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Between a subscription's re-authorization and reading the events to
/// deliver, while the processing lock is held.
pub const RECHECK_AFTER_AUTHORIZATION: &str = "subscription.recheck.after_authorization";
/// A context tick published a packet's object and has not yet committed the
/// batch that names it (CONTEXT section 5, STORAGE section 2).
pub const PACKET_AFTER_OBJECT_PUBLISHED: &str = "context.packet.after_object_published";
/// A request found the processing lock held by someone else.
pub const LOCK_CONTENDED: &str = "processing.lock.contended";
/// An idle session reserved FIFO progress while processing was busy. This is
/// distinct from command contention and never substitutes for it.
pub const IDLE_PROCESSING_QUEUED: &str = "processing.idle.queued";
/// A core-test put owns the processing lock but has not begun admission.
pub const PUT_AFTER_PROCESSING_LOCK: &str = "core-test.put.after-processing-lock";

const WATCHDOG: Duration = Duration::from_secs(30);

struct Barriers {
    directory: PathBuf,
    enabled: BTreeSet<String>,
    used: Mutex<BTreeSet<String>>,
}

static BARRIERS: OnceLock<Barriers> = OnceLock::new();

/// Enable barriers for this process. Called once at start.
pub fn init(directory: &Path, enabled: &[String]) {
    let _ = BARRIERS.set(Barriers {
        directory: directory.to_path_buf(),
        enabled: enabled.iter().cloned().collect(),
        used: Mutex::new(BTreeSet::new()),
    });
}

/// Create a file atomically, so the runner never sees a half-written marker.
fn touch(path: &Path) {
    let temporary = path.with_extension("tmp");
    if std::fs::write(&temporary, b"").is_ok() {
        let _ = std::fs::rename(&temporary, path);
    }
}

/// Mark a point without pausing.
pub fn signal(name: &str) {
    if let Some(barriers) = BARRIERS.get() {
        touch(&barriers.directory.join(format!("{name}.signal")));
    }
}

/// Pause at an enabled barrier the first time it is reached.
pub fn pause(name: &str) {
    let Some(barriers) = BARRIERS.get() else {
        return;
    };
    if !barriers.enabled.contains(name) {
        return;
    }
    {
        let mut used = barriers
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !used.insert(name.to_string()) {
            return;
        }
    }
    touch(&barriers.directory.join(format!("{name}.reached")));
    let release = barriers.directory.join(format!("{name}.release"));
    let started = Instant::now();
    while !release.exists() {
        if started.elapsed() > WATCHDOG {
            eprintln!(
                "cbr-provider: barrier {name}: no release within {} s; continuing",
                WATCHDOG.as_secs()
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
