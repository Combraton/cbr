//! The gate for work that has left the preparation tick.
//!
//! **Nothing here sleeps through a wait.** A unit of work is held open by
//! a channel the test closes when it wants the work to finish, so the
//! timings are the test's rather than the scheduler's.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;

/// Work that blocks until the test lets it finish.
fn held() -> (
    mpsc::Sender<()>,
    impl FnOnce() -> Result<(), &'static str> + Send + 'static,
) {
    let (release, wait) = mpsc::channel::<()>();
    (release, move || {
        let _ = wait.recv();
        Ok(())
    })
}

/// Waits for `check` to hold, so no test depends on a sleep being long
/// enough.
fn until(check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check() {
        assert!(Instant::now() < deadline, "the condition never held");
        std::thread::yield_now();
    }
}

#[test]
fn work_that_finishes_is_reported_done_once() {
    let pool = Pool::new(CONCURRENCY);
    assert_eq!(pool.progress("a", None, || Ok(())), Progress::Running);
    until(|| pool.progress("a", None, || Ok(())) == Progress::Done);
    // And the slot is gone: a second ask starts new work rather than
    // reporting the old result for ever.
    assert_eq!(pool.progress("a", None, || Ok(())), Progress::Running);
}

#[test]
fn work_that_fails_reports_its_typed_reason() {
    // **Every failure ends as a typed reason**, which is what an item
    // carries. A panic in the work is the same: a unit that died is a unit
    // that did not finish, and the caller is told so rather than left.
    let pool = Pool::new(CONCURRENCY);
    pool.progress("a", None, || Err("index_unavailable"));
    until(|| matches!(pool.progress("a", None, || Ok(())), Progress::Failed(_)));

    let pool = Pool::new(CONCURRENCY);
    pool.progress("b", None, || panic!("the work died"));
    until(|| pool.progress("b", None, || Ok(())) == Progress::Failed("work_panicked"));
}

#[test]
fn the_same_key_is_not_started_twice() {
    // The tick asks every time it runs. Starting the work again each time
    // would be the stall it was moved out of the tick to avoid, multiplied.
    let started = Arc::new(AtomicUsize::new(0));
    let pool = Pool::new(CONCURRENCY);
    let (release, work) = held();
    let counter = Arc::clone(&started);
    pool.progress("a", None, move || {
        counter.fetch_add(1, Ordering::SeqCst);
        work()
    });
    for _ in 0..5 {
        let counter = Arc::clone(&started);
        assert_eq!(
            pool.progress("a", None, move || {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            Progress::Running
        );
    }
    drop(release);
    until(|| pool.progress("a", None, || Ok(())) == Progress::Done);
    assert_eq!(started.load(Ordering::SeqCst), 1, "started once");
}

#[test]
fn the_bound_defers_work_beyond_it_rather_than_queueing_it() {
    // **A shared quota plus unbounded concurrency is an overspend, and a
    // shared machine plus unbounded concurrency is a stall.** Work beyond
    // the bound is deferred, not queued: the tick asks again, and by then
    // the request may have been cancelled or its deadline passed, so
    // holding a queue would be holding work nobody wants.
    let pool = Pool::new(2);
    let (first, work) = held();
    assert_eq!(pool.progress("a", None, work), Progress::Running);
    let (second, work) = held();
    assert_eq!(pool.progress("b", None, work), Progress::Running);
    assert_eq!(
        pool.progress("c", None, || Ok(())),
        Progress::Deferred,
        "the third is deferred"
    );
    assert_eq!(pool.running(), 2);
    drop(first);
    until(|| pool.progress("a", None, || Ok(())) == Progress::Done);
    // With room again, the deferred one starts.
    assert_eq!(pool.progress("c", None, || Ok(())), Progress::Running);
    drop(second);
}

#[test]
fn a_deadline_that_passes_ends_the_work_as_timed_out() {
    // The deadline is the request's, and a unit that outlives one is not
    // something anybody is still waiting for.
    let pool = Pool::new(CONCURRENCY);
    let (release, work) = held();
    let deadline = Instant::now() - Duration::from_millis(1);
    assert_eq!(pool.progress("a", Some(deadline), work), Progress::Running);
    assert_eq!(
        pool.progress("a", Some(deadline), || Ok(())),
        Progress::TimedOut
    );
    // The slot is released, so the bound is not held by work nobody wants.
    assert_eq!(pool.running(), 0);
    drop(release);
}

#[test]
fn a_deadline_in_the_future_does_not_end_it() {
    let pool = Pool::new(CONCURRENCY);
    let (release, work) = held();
    let deadline = Instant::now() + Duration::from_secs(30);
    pool.progress("a", Some(deadline), work);
    assert_eq!(
        pool.progress("a", Some(deadline), || Ok(())),
        Progress::Running
    );
    drop(release);
}

#[test]
fn cancelling_discards_the_result_and_frees_the_slot() {
    // **A cancelled unit leaves nothing behind.** Its result is not read,
    // so nothing it produced is recorded — which is what "a cancelled call
    // leaves no partial derivation record" means at this level.
    let pool = Pool::new(CONCURRENCY);
    let (release, work) = held();
    pool.progress("a", None, work);
    assert_eq!(pool.running(), 1);
    pool.cancel("a");
    assert_eq!(pool.running(), 0, "the slot is free at once");
    assert_eq!(
        pool.progress("a", None, || Err("would have failed")),
        Progress::Running,
        "and the next ask is new work, not the cancelled one's answer"
    );
    // There is no `Cancelled` to report, and deliberately so: a caller
    // that cancelled is a caller that stopped asking. A variant nothing
    // ever produces is decoration.
    drop(release);
}

#[test]
fn cancelling_work_that_was_never_started_is_harmless() {
    let pool = Pool::new(CONCURRENCY);
    pool.cancel("never");
    assert_eq!(pool.running(), 0);
}

#[test]
fn the_default_bound_is_a_stated_number() {
    assert_eq!(CONCURRENCY, 2);
}
