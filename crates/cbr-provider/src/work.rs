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

use cbr_encoding::Value;

/// How many units of work may be in flight at once.
///
/// Two, because the work this bounds is a **whole-repository index build**
/// or a model call, each of which is heavy on a machine the owner is also
/// using. It is a bound on simultaneous heaviness rather than a throughput
/// setting, and m4e's measurements are what would move it.
pub const CONCURRENCY: usize = 2;

/// Where a unit of work has got to.
///
/// The three settled answers — [`Progress::Done`], [`Progress::Failed`]
/// and [`Progress::TimedOut`] — are **kept until the caller takes them**
/// with [`Pool::release`], and every ask until then gets the same one.
/// Two reasons, and the second is the one with teeth:
///
/// * A compile that needs two calls asks across several ticks. If the
///   first answer were released when it was first reported, it would be
///   gone by the tick the second arrived, and the compile could never
///   hold both.
/// * A released failure is a **retry**. The next tick asks again, and the
///   pool would start the work afresh — which is not a bounded repair
///   inside one call ([READINESS §8](../../docs/work/m4/READINESS.md))
///   but an unbounded loop spending a shared quota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Started, or already running. Ask again next tick.
    Running,
    /// Not started: the bound is full. Ask again next tick.
    Deferred,
    /// Finished, with what it produced.
    Done(Value),
    /// Finished badly, with the reason an item carries.
    Failed(&'static str),
    /// The request's deadline passed while it ran.
    TimedOut,
}

/// A settled answer, held until it is taken.
enum Settled {
    Done(Value),
    Failed(&'static str),
    TimedOut,
}

struct Slot {
    /// Dropped when the answer settles. A timed-out unit's thread is
    /// **detached** rather than waited for: Rust cannot kill a thread, and
    /// waiting for one nobody wants is the stall this module exists to
    /// remove.
    handle: Option<JoinHandle<Result<Value, &'static str>>>,
    settled: Option<Settled>,
}

impl Slot {
    /// In flight, and therefore counted against the bound.
    fn running(&self) -> bool {
        self.settled.is_none()
    }
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
        work: impl FnOnce() -> Result<Value, &'static str> + Send + 'static,
    ) -> Progress {
        let mut slots = self.slots.lock().expect("not poisoned");
        if let Some(slot) = slots.get_mut(key) {
            if slot.settled.is_none() {
                let finished = slot
                    .handle
                    .as_ref()
                    .is_some_and(std::thread::JoinHandle::is_finished);
                if finished {
                    // A unit that died is a unit that did not finish, and
                    // the caller is told so rather than left waiting.
                    slot.settled = Some(match slot.handle.take().expect("just seen").join() {
                        Ok(Ok(value)) => Settled::Done(value),
                        Ok(Err(reason)) => Settled::Failed(reason),
                        Err(_) => Settled::Failed("work_panicked"),
                    });
                } else if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    // **The deadline is the request's.** A unit that
                    // outlives one is not something anybody is still
                    // waiting for, and counting its slot would hold the
                    // bound against work that is.
                    slot.handle = None;
                    slot.settled = Some(Settled::TimedOut);
                }
            }
            return match &slot.settled {
                None => Progress::Running,
                Some(Settled::Done(value)) => Progress::Done(value.clone()),
                Some(Settled::Failed(reason)) => Progress::Failed(reason),
                Some(Settled::TimedOut) => Progress::TimedOut,
            };
        }
        // **The bound is on work in flight**, which is what is heavy. A
        // settled answer nobody has taken costs a map entry; holding the
        // bound with one would stall the next job for as long as it sat
        // there.
        if slots.values().filter(|slot| slot.running()).count() >= self.bound {
            return Progress::Deferred;
        }
        slots.insert(
            key.to_string(),
            Slot {
                handle: Some(std::thread::spawn(work)),
                settled: None,
            },
        );
        Progress::Running
    }

    /// Take `key`'s answer and free the key.
    ///
    /// The caller that reads a settled answer is the caller that releases
    /// it, once it has been used — the compile that put it in a packet, or
    /// the job that ended. Until then every ask gets the same answer.
    pub fn release(&self, key: &str) {
        self.slots.lock().expect("not poisoned").remove(key);
    }

    /// Take every answer whose key begins with `prefix`.
    ///
    /// **A job releases its own calls by naming itself.** It is the job,
    /// not an item, that knows when the answers have been used: a compile
    /// that produced its script has read every one it asked for, and a
    /// job that published or ended will not ask again. Releasing one at a
    /// time as it was read would free a key the next tick's compile would
    /// then ask afresh — a second call, and a second charge, for a
    /// question already answered.
    pub fn release_all(&self, prefix: &str) {
        self.slots
            .lock()
            .expect("not poisoned")
            .retain(|key, _| !key.starts_with(prefix));
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
        self.release(key);
    }

    /// How many units are **in flight**, which is what the bound counts.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn running(&self) -> usize {
        self.slots
            .lock()
            .expect("not poisoned")
            .values()
            .filter(|slot| slot.running())
            .count()
    }
}

#[cfg(test)]
mod tests;
