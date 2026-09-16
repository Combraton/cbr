//! Pending output for one connection, written by its own thread.
//!
//! The shape is the one `docs/work/readiness/STACK.md` section 2 argues for: a
//! `Mutex` and a `Condvar` around a queue, with a dedicated writer. The session
//! thread never performs I/O on the consumer's output, so a consumer that stops
//! reading can stall only its own delivery, and only for as long as the session
//! chooses to wait for it — which is bounded (CORE section 16.5, and
//! `crate::session`).
//!
//! Queueing also gives the ordering CORE section 16.5 requires for free: a
//! notification carrying events caused by a command on the same connection is
//! pushed after that command's response, and the queue preserves that.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct State {
    queue: VecDeque<Vec<u8>>,
    /// Bytes produced but not yet written. This is the quantity the
    /// backpressure bound is placed on; it is visible to the session thread
    /// without blocking.
    pending: usize,
    /// The most bytes ever pending at once, so a test can measure the bound
    /// rather than assume it held.
    max_pending: usize,
    queued: u64,
    written: u64,
    closed: bool,
    failed: bool,
    /// Closure after a too-slow consumer: nothing more is written, whether or
    /// not it was queued.
    abandoned: bool,
}

pub struct Outbox {
    state: Mutex<State>,
    changed: Condvar,
}

impl Outbox {
    /// Start the writer thread for `out`.
    pub fn start<W: Write + Send + 'static>(mut out: W) -> Arc<Self> {
        let outbox = Arc::new(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let writer = outbox.clone();
        std::thread::spawn(move || {
            loop {
                let frame = {
                    let mut state = writer.lock();
                    while state.queue.is_empty() && !state.closed {
                        state = writer
                            .changed
                            .wait(state)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                    if state.abandoned {
                        return;
                    }
                    match state.queue.front() {
                        // Left in the queue until written, so `pending` counts
                        // a frame the consumer has not yet taken.
                        Some(frame) => frame.clone(),
                        // Closed and drained.
                        None => return,
                    }
                };
                let ok = out.write_all(&frame).and_then(|()| out.flush()).is_ok();
                let mut state = writer.lock();
                if state.abandoned {
                    return;
                }
                if !ok {
                    // The peer is gone. Pending frames are undelivered, and the
                    // provider must not die of it.
                    state.failed = true;
                    writer.changed.notify_all();
                    return;
                }
                state.queue.pop_front();
                state.pending -= frame.len();
                state.written += 1;
                writer.changed.notify_all();
            }
        });
        outbox
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Queue a frame. Never waits. Returns false once writing has failed or
    /// the connection was abandoned.
    pub fn push(&self, frame: Vec<u8>) -> bool {
        let mut state = self.lock();
        if state.failed || state.abandoned {
            return false;
        }
        state.pending += frame.len();
        state.max_pending = state.max_pending.max(state.pending);
        state.queued += 1;
        state.queue.push_back(frame);
        self.changed.notify_all();
        true
    }

    /// Bytes produced but not yet written.
    pub fn pending(&self) -> usize {
        self.lock().pending
    }

    /// The most bytes ever pending at once on this connection.
    pub fn max_pending(&self) -> usize {
        self.lock().max_pending
    }

    /// Wait, for at most `timeout`, until everything queued **now** has been
    /// written. Returns false if writing failed or the time ran out first.
    ///
    /// The deadline is fixed when the wait starts: partial progress does not
    /// extend it, which is exactly CORE section 16.5's room deadline.
    pub fn wait_drained(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        let target = state.queued;
        loop {
            if state.written >= target {
                return true;
            }
            if state.failed || state.abandoned {
                return false;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Discard everything not yet written. The writer stops after any write
    /// already in progress and writes nothing more.
    pub fn abandon(&self) {
        let mut state = self.lock();
        state.abandoned = true;
        state.closed = true;
        state.queue.clear();
        state.pending = 0;
        self.changed.notify_all();
    }

    /// Stop the writer once the queue is empty, after waiting at most `timeout`
    /// for it to empty. A tidy end should not lose a response already
    /// produced, and a consumer that stopped reading must not keep the process
    /// alive forever either.
    pub fn close(&self, timeout: Duration) {
        self.wait_drained(timeout);
        self.lock().closed = true;
        self.changed.notify_all();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::mpsc;

    /// A writer that blocks until released, standing in for a consumer that
    /// has stopped reading.
    pub(crate) struct Blocking {
        pub release: mpsc::Receiver<()>,
        pub wrote: mpsc::Sender<Vec<u8>>,
    }

    impl Write for Blocking {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.release.recv().is_err() {
                return Err(std::io::Error::other("consumer gone"));
            }
            let _ = self.wrote.send(buf.to_vec());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_session_thread_never_blocks_on_a_stalled_consumer() {
        let (release, blocked) = mpsc::channel();
        let (wrote, written) = mpsc::channel();
        let outbox = Outbox::start(Blocking {
            release: blocked,
            wrote,
        });

        // The consumer is not reading. Queueing must still return promptly,
        // and the pending count must reflect what is waiting.
        let started = Instant::now();
        for _ in 0..16 {
            assert!(outbox.push(b"frame".to_vec()));
        }
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "queueing does not wait on the consumer"
        );
        assert!(
            outbox.pending() > 0,
            "queued bytes are visible without blocking"
        );

        // Releasing the consumer drains them.
        for _ in 0..16 {
            let _ = release.send(());
        }
        let mut seen = 0;
        while seen < 16 {
            match written.recv_timeout(Duration::from_secs(5)) {
                Ok(_) => seen += 1,
                Err(_) => panic!("writer stalled after {seen} frames"),
            }
        }
    }

    #[test]
    fn waiting_for_room_ends_at_its_deadline_not_when_the_consumer_returns() {
        let (release, blocked) = mpsc::channel::<()>();
        let (wrote, _written) = mpsc::channel();
        let outbox = Outbox::start(Blocking {
            release: blocked,
            wrote,
        });
        assert!(outbox.push(vec![b'x'; 64]));

        let started = Instant::now();
        assert!(
            !outbox.wait_drained(Duration::from_millis(200)),
            "a consumer that never reads is not drained"
        );
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(200) && waited < Duration::from_millis(1200),
            "the wait is bounded by its deadline: {waited:?}"
        );

        // Abandoning discards what is pending, and nothing queued afterwards
        // is accepted: closure writes nothing more.
        outbox.abandon();
        assert_eq!(outbox.pending(), 0);
        assert!(!outbox.push(b"late".to_vec()));
        drop(release);
    }
}
