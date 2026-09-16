//! Pending output for one connection, written by its own thread.
//!
//! The shape is the one `docs/work/readiness/STACK.md` section 2 argues for: a
//! `Mutex` and a `Condvar` around a bounded queue, with a dedicated writer.
//! The point is that **the session thread never blocks on a consumer that
//! stopped reading**. A provider that writes responses inline stalls its own
//! command path the moment a subscriber stops draining, which turns one slow
//! reader into a stuck store.
//!
//! Queueing also gives the ordering CORE section 16.5 requires for free: a
//! notification carrying events caused by a command on the same connection is
//! pushed after that command's response, and the queue preserves that.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct State {
    queue: VecDeque<Vec<u8>>,
    /// Bytes produced but not yet written. This is the quantity a bound is
    /// placed on; it is visible to the session thread without blocking.
    pending: usize,
    closed: bool,
    failed: bool,
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
                    match state.queue.pop_front() {
                        Some(frame) => {
                            state.pending -= frame.len();
                            frame
                        }
                        // Closed and drained.
                        None => return,
                    }
                };
                if out.write_all(&frame).and_then(|()| out.flush()).is_err() {
                    // The peer is gone. Pending frames are undelivered, and the
                    // provider must not die of it.
                    let mut state = writer.lock();
                    state.failed = true;
                    writer.changed.notify_all();
                    return;
                }
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

    /// Queue a frame. Returns false once writing has failed.
    pub fn push(&self, frame: Vec<u8>) -> bool {
        let mut state = self.lock();
        if state.failed {
            return false;
        }
        state.pending += frame.len();
        state.queue.push_back(frame);
        self.changed.notify_all();
        true
    }

    /// Bytes produced but not yet written.
    pub fn pending(&self) -> usize {
        self.lock().pending
    }

    /// Stop accepting frames and wait for the queue to drain, so a response
    /// already produced is not lost to a tidy shutdown.
    pub fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.changed.notify_all();
        while !state.queue.is_empty() && !state.failed {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A writer that blocks until released, standing in for a consumer that
    /// has stopped reading.
    struct Blocking {
        release: mpsc::Receiver<()>,
        wrote: mpsc::Sender<usize>,
    }

    impl Write for Blocking {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = self.release.recv();
            let _ = self.wrote.send(buf.len());
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
        for _ in 0..16 {
            assert!(outbox.push(b"frame".to_vec()));
        }
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
}
