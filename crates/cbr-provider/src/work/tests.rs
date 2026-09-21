//! The gate for work that has left the preparation tick.
//!
//! **Nothing here sleeps through a wait.** A unit of work is held open by
//! a channel the test closes when it wants the work to finish, so the
//! timings are the test's rather than the scheduler's.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cbr_encoding::Value;

use super::*;

fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

/// Work that blocks until the test lets it finish.
/// The work a factory builds, boxed so `held`'s return type can be
/// written down.
type Work = Box<dyn FnOnce() -> Result<Value, &'static str> + Send>;

fn held() -> (mpsc::Sender<()>, impl FnOnce() -> Work) {
    let (release, wait) = mpsc::channel::<()>();
    (release, move || {
        Box::new(move || {
            let _ = wait.recv();
            Ok(Value::Null)
        }) as Work
    })
}

/// Work that produces nothing, as a factory — which is what `progress`
/// takes, so that a tick which is only asking builds no work at all.
fn nothing() -> impl FnOnce() -> Result<Value, &'static str> + Send + 'static {
    || Ok(Value::Null)
}

/// `Progress::Done` with nothing in it.
fn done() -> Progress {
    Progress::Done(Value::Null)
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
fn work_that_finishes_hands_back_what_it_produced() {
    let pool = Pool::new(CONCURRENCY);
    assert_eq!(
        pool.progress("a", None, || || Ok(string("chosen"))),
        Progress::Running
    );
    until(|| pool.progress("a", None, nothing) != Progress::Running);
    assert_eq!(
        pool.progress("a", None, nothing),
        Progress::Done(string("chosen")),
        "the caller gets the value the work produced"
    );
}

#[test]
fn a_result_is_kept_until_it_is_taken() {
    // **A compile that needs two calls asks across several ticks.** If
    // `Done` released the slot, the first answer would be gone by the tick
    // the second arrived, and the compile could never see both. So a
    // finished slot holds its result until the caller takes it, and until
    // then every ask gets the same answer.
    let pool = Pool::new(CONCURRENCY);
    let started = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&started);
    pool.progress("a", None, move || {
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(string("chosen"))
        }
    });
    until(|| pool.progress("a", None, nothing) != Progress::Running);
    for _ in 0..5 {
        assert_eq!(
            pool.progress("a", None, nothing),
            Progress::Done(string("chosen"))
        );
    }
    assert_eq!(started.load(Ordering::SeqCst), 1, "and it ran once");

    pool.release("a");
    assert_eq!(
        pool.progress("a", None, || || Ok(string("again"))),
        Progress::Running,
        "taken, the key is free for new work"
    );
}

#[test]
fn work_that_fails_reports_its_typed_reason_and_is_not_retried() {
    // **Every failure ends as a typed reason**, which is what an item
    // carries. A panic in the work is the same: a unit that died is a unit
    // that did not finish, and the caller is told so rather than left.
    //
    // A failure is kept like an answer, because the next tick asks again
    // and a released failure would be a retry nobody asked for. READINESS
    // §8 allows a bounded repair inside one call and no retry beyond it.
    let pool = Pool::new(CONCURRENCY);
    let started = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&started);
    pool.progress("a", None, move || {
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Err("index_unavailable")
        }
    });
    until(|| pool.progress("a", None, nothing) != Progress::Running);
    for _ in 0..5 {
        assert_eq!(
            pool.progress("a", None, nothing),
            Progress::Failed("index_unavailable")
        );
    }
    assert_eq!(
        started.load(Ordering::SeqCst),
        1,
        "it ran once, and was not retried"
    );

    let pool = Pool::new(CONCURRENCY);
    pool.progress("b", None, || || panic!("the work died"));
    until(|| pool.progress("b", None, nothing) == Progress::Failed("work_panicked"));
}

#[test]
fn a_result_waiting_to_be_taken_does_not_hold_the_bound() {
    // The bound is on work **in flight**, because that is what is heavy.
    // A finished unit costs a map entry, and holding the bound with one
    // would stall the next job for as long as nobody took the answer.
    let pool = Pool::new(2);
    let (first, work) = held();
    pool.progress("a", None, work);
    let (second, work) = held();
    pool.progress("b", None, work);
    assert_eq!(pool.progress("c", None, nothing), Progress::Deferred);

    drop(first);
    until(|| pool.progress("a", None, nothing) != Progress::Running);
    assert_eq!(pool.running(), 1, "only the unfinished one is in flight");
    assert_eq!(
        pool.progress("c", None, nothing),
        Progress::Running,
        "and the deferred work starts, though `a` has not been taken"
    );
    drop(second);
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
        let work = work();
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
            work()
        }
    });
    for _ in 0..5 {
        let counter = Arc::clone(&started);
        assert_eq!(
            pool.progress("a", None, move || {
                move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(Value::Null)
                }
            }),
            Progress::Running
        );
    }
    drop(release);
    until(|| pool.progress("a", None, nothing) == done());
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
        pool.progress("c", None, nothing),
        Progress::Deferred,
        "the third is deferred"
    );
    assert_eq!(pool.running(), 2);
    drop(first);
    until(|| pool.progress("a", None, nothing) == done());
    // With room again, the deferred one starts.
    assert_eq!(pool.progress("c", None, nothing), Progress::Running);
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
        pool.progress("a", Some(deadline), nothing),
        Progress::TimedOut
    );
    // The bound is not held by work nobody wants.
    assert_eq!(pool.running(), 0);
    // And it is not started again: a request whose deadline has passed is
    // not a request to try once more.
    for _ in 0..5 {
        assert_eq!(
            pool.progress("a", Some(deadline), nothing),
            Progress::TimedOut
        );
    }
    drop(release);
}

#[test]
fn a_deadline_in_the_future_does_not_end_it() {
    let pool = Pool::new(CONCURRENCY);
    let (release, work) = held();
    let deadline = Instant::now() + Duration::from_secs(30);
    pool.progress("a", Some(deadline), work);
    assert_eq!(
        pool.progress("a", Some(deadline), nothing),
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
        pool.progress("a", None, || || Err("would have failed")),
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

#[test]
fn a_job_releases_every_answer_it_asked_for_by_naming_itself() {
    // The compile that read them is the compile that frees them, and it
    // frees them together: an answer released the moment it was read
    // would be asked again by the next tick's compile, which is a second
    // call and a second charge for a question already answered.
    let pool = Pool::new(4);
    for key in ["model:j1:r1:a.rs", "model:j1:r1:b.rs", "model:j2:r1:a.rs"] {
        pool.progress(key, None, || || Ok(string("c1")));
        until(|| pool.progress(key, None, nothing) != Progress::Running);
    }
    pool.release_all("model:j1:");
    assert_eq!(
        pool.progress("model:j2:r1:a.rs", None, nothing),
        Progress::Done(string("c1")),
        "another job's answer is untouched"
    );
    for key in ["model:j1:r1:a.rs", "model:j1:r1:b.rs"] {
        assert_eq!(
            pool.progress(key, None, || || Ok(string("fresh"))),
            Progress::Running,
            "{key} was taken"
        );
    }
}

#[test]
fn work_is_built_only_when_it_is_actually_started() {
    // **The tick that is only asking builds nothing.** `progress` takes a
    // factory rather than the work, because a caller whose work opens a
    // connection to the store used to open one on every tick and throw it
    // away — thousands over one call, paid on the preparation tick this
    // module exists to keep short.
    let built = Arc::new(AtomicUsize::new(0));
    let pool = Pool::new(1);
    let (release, work) = held();
    let counter = Arc::clone(&built);
    pool.progress("a", None, move || {
        counter.fetch_add(1, Ordering::SeqCst);
        work()
    });
    for _ in 0..5 {
        let counter = Arc::clone(&built);
        // Asked again while it runs, and again while the bound is full.
        pool.progress("a", None, move || {
            counter.fetch_add(1, Ordering::SeqCst);
            nothing()
        });
        let counter = Arc::clone(&built);
        assert_eq!(
            pool.progress("b", None, move || {
                counter.fetch_add(1, Ordering::SeqCst);
                nothing()
            }),
            Progress::Deferred
        );
    }
    assert_eq!(
        built.load(Ordering::SeqCst),
        1,
        "built once, for the one start"
    );
    drop(release);
    until(|| pool.progress("a", None, nothing) != Progress::Running);

    // And once it is settled, asking again still builds nothing: the
    // answer is there to be taken.
    let counter = Arc::clone(&built);
    pool.progress("a", None, move || {
        counter.fetch_add(1, Ordering::SeqCst);
        nothing()
    });
    assert_eq!(built.load(Ordering::SeqCst), 1, "still once");
}

#[test]
fn a_finished_unit_nobody_asked_about_again_does_not_hold_the_bound() {
    // **The bound is on threads, not on map entries.** An answer settles
    // only when somebody asks for it, so a caller that stopped asking —
    // because it got what it wanted another way — would hold the bound
    // for ever with a thread that ended long ago.
    //
    // Not hypothetical: the index build asks once, is told `Running`, and
    // by the time it has built its caller reads the manifest and never
    // asks again. One repository cost half the bound permanently, and two
    // would have meant no model call could ever start.
    let pool = Pool::new(2);
    let (release, work) = held();
    pool.progress("a", None, work);
    let (second, work) = held();
    pool.progress("b", None, work);
    assert_eq!(pool.progress("c", None, nothing), Progress::Deferred);

    // `a` finishes, and **nothing asks about it again**.
    drop(release);
    until(|| pool.running() == 1);
    assert_eq!(
        pool.progress("c", None, nothing),
        Progress::Running,
        "a thread that has ended is not in flight, asked about or not"
    );
    drop(second);
}
