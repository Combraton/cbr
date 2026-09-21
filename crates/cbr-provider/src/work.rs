//! Work that has left the preparation tick.
//!
//! # Why anything leaves it
//!
//! M3 measured the cost of doing long work inside the tick: the index
//! build holds it throughout — **7.2s** on CBR's own 1,066 blobs, **12.8s**
//! on brian2's 553 blobs and 5.3 MB, **3.9s** on Knowscroll's 145 — with
//! every other job on that provider waiting it out
//! ([READINESS §8](../../docs/work/m4/READINESS.md)). A model call is
//! longer and less predictable than any of those.
//!
//! # Deferred, not queued
//!
//! Work beyond the bound is **refused for now** rather than put in a line.
//! The tick asks again each time it runs, and by the next one the request
//! may have been cancelled or its deadline passed — so a queue would be a
//! promise to do work nobody is waiting for any more, taken at the moment
//! CBR has least idea whether it is still wanted.
//!
//! # What a caller gets
//!
//! Every answer is a [`Progress`], and every unhappy one is typed: an item
//! carries the reason rather than a hang. Nothing here blocks the caller,
//! so the tick that asks is the tick that returns.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Instant;

/// How many units of work may be in flight at once.
///
/// Two, because the work this bounds is a **whole-repository index build**
/// or a model call, each of which is heavy on a machine the owner is also
/// using. It is a bound on simultaneous heaviness rather than a throughput
/// setting, and m4e's measurements are what would move it.
pub const CONCURRENCY: usize = 2;

/// Where a unit of work has got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Started, or already running. Ask again next tick.
    Running,
    /// Not started: the bound is full. Ask again next tick.
    Deferred,
    /// Finished, and the result is taken. **Reported once**: the slot is
    /// released with it, so a later ask starts new work rather than
    /// answering with an old result for ever.
    Done,
    /// Finished badly, with the reason an item carries.
    Failed(&'static str),
    /// The request's deadline passed while it ran.
    TimedOut,
}

struct Slot {
    handle: JoinHandle<Result<(), &'static str>>,
}

pub struct Pool {
    bound: usize,
    slots: Mutex<HashMap<String, Slot>>,
}

impl Pool {
    pub fn new(bound: usize) -> Self {
        Pool {
            bound,
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// Where `key`'s work has got to, starting it if it is not running and
    /// there is room.
    ///
    /// `work` is taken by value and dropped unused when the answer is
    /// anything but a fresh start, so a caller that builds an expensive
    /// closure pays for it either way — which is why the callers build
    /// cheap ones that capture what they need.
    pub fn progress(
        &self,
        key: &str,
        deadline: Option<Instant>,
        work: impl FnOnce() -> Result<(), &'static str> + Send + 'static,
    ) -> Progress {
        let mut slots = self.slots.lock().expect("not poisoned");
        if let Some(slot) = slots.get(key) {
            if slot.handle.is_finished() {
                let slot = slots.remove(key).expect("just seen");
                // A unit that died is a unit that did not finish, and the
                // caller is told so rather than left waiting for it.
                return match slot.handle.join() {
                    Ok(Ok(())) => Progress::Done,
                    Ok(Err(reason)) => Progress::Failed(reason),
                    Err(_) => Progress::Failed("work_panicked"),
                };
            }
            // **The deadline is the request's.** A unit that outlives one
            // is not something anybody is still waiting for, and holding
            // its slot would hold the bound against work that is.
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                slots.remove(key);
                return Progress::TimedOut;
            }
            return Progress::Running;
        }
        if slots.len() >= self.bound {
            return Progress::Deferred;
        }
        slots.insert(
            key.to_string(),
            Slot {
                handle: std::thread::spawn(work),
            },
        );
        Progress::Running
    }

    /// Stop waiting for `key`.
    ///
    /// The thread is not killed — Rust has no way to — but its result is
    /// never read and its slot is released at once. **Nothing it produced
    /// is recorded**, which is what "a cancelled call leaves no partial
    /// derivation record" means here: the record is written by whoever
    /// reads the result, and nobody does.
    ///
    /// That reasoning holds only for work which writes nothing itself,
    /// which is why this belongs to the model call site, where a request
    /// owns its call, and not to the index build. The build writes its
    /// manifest as it goes, so cancelling it would leave exactly the
    /// partial record this rules out; and it has no single owner to
    /// cancel it besides — several jobs may want the same repository at
    /// the same tree, and it is idempotent.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cancel(&self, key: &str) {
        self.slots.lock().expect("not poisoned").remove(key);
    }

    /// How many units hold a slot.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn running(&self) -> usize {
        self.slots.lock().expect("not poisoned").len()
    }
}

#[cfg(test)]
mod tests;
