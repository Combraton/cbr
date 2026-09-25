//! One session over one connection: frames in, responses and notifications out.
//!
//! Output goes through a per-connection outbox written by its own thread, so
//! the session never performs I/O on the consumer's output. What remains is
//! flow control, and CORE section 16.5 makes it bounded in both space and time:
//!
//! - **Space.** Output produced but not yet written stays within
//!   `max_pending_notification_bytes`. A notification that would exceed it is
//!   withheld — its items stay undelivered and are produced later, never
//!   skipped — and a response that would exceed it waits for room.
//! - **Time.** A stall starts when either would exceed the bound. The consumer
//!   has `backpressure_notice_ms` from that instant to drain everything
//!   pending; partial progress does not extend it. A consumer that misses it is
//!   too slow, and its connection is closed.
//!
//! So the session thread does wait on a stalled consumer, but only for the room
//! deadline, and never inside a write. On one connection that wait **is** the
//! backpressure: a consumer that has stopped reading is not sent more, and its
//! further requests are not read, until it makes room or loses the connection.
//! A command's commit never waits on output at all, because output is produced
//! only after the store transaction has committed.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cbr_encoding::Value;

use crate::errors::FrameFailure;
use crate::frames::{self, Frame, FrameReader};
use crate::jsonrpc;
use crate::outbox::Outbox;
use crate::provider::Provider;

/// How long a tidy end waits for output already produced to be written. A
/// consumer that stopped reading cannot keep the process alive past it.
const DRAIN_ON_EXIT: Duration = Duration::from_secs(30);

/// How the session ended.
#[derive(Debug)]
pub enum Ended {
    /// End of input, or a frame-level failure that closes the connection.
    Normally,
    /// The consumer did not make room within the deadline.
    TooSlow(TooSlow),
}

/// What was observed when a consumer was declared too slow. Recorded rather
/// than inferred, so the bound is measured and not assumed to have held.
#[derive(Debug)]
pub struct TooSlow {
    pub bound: usize,
    /// Bytes produced and not yet written when the consumer was declared too
    /// slow.
    pub pending: usize,
    /// The most bytes ever pending on this connection.
    pub max_pending: usize,
    /// The most bytes pending before any ending notice was queued. Everything
    /// the bound governs was produced by then.
    pub max_pending_before_notices: usize,
    pub stall_started: Instant,
    pub declared: Instant,
    pub closed: Instant,
    /// Ending notices produced. Zero when the session did not negotiate
    /// `core.events.backpressure`, which is then owed none.
    pub notices: usize,
    /// Whether everything, notices included, was written within the budget.
    pub notices_written: bool,
}

impl TooSlow {
    /// One line for standard error, which is where the ending is recorded.
    pub fn record(&self) -> String {
        format!(
            "consumer too slow: {} bytes pending against a bound of {}; at most {} pending before \
             any ending notice, {} including them; declared {} ms after the stall began, closed \
             {} ms after that; {} ending notices, {}",
            self.pending,
            self.bound,
            self.max_pending_before_notices,
            self.max_pending,
            self.declared.duration_since(self.stall_started).as_millis(),
            self.closed.duration_since(self.declared).as_millis(),
            self.notices,
            if self.notices == 0 {
                "none owed"
            } else if self.notices_written {
                "written"
            } else {
                "not written"
            }
        )
    }
}

/// Serve one connection until its input ends or it is closed.
///
/// `closer`, for a socket, is shut down when the connection must close while
/// its writer may still be blocked on a consumer that stopped reading: the
/// shutdown is what unblocks it. On stdio there is nothing to shut down;
/// closing is the process ending.
pub fn serve<R: Read, W: Write + Send + 'static>(
    input: R,
    out: W,
    provider: &mut Provider,
    closer: Option<&std::os::unix::net::UnixStream>,
) -> Result<Ended, String> {
    let outbox = Outbox::start(out);
    let ended = serve_frames(input, &outbox, provider);
    if let (Ok(Ended::TooSlow(_)), Some(stream)) = (&ended, closer) {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
    ended
}

fn serve_frames<R: Read>(
    input: R,
    outbox: &Arc<Outbox>,
    provider: &mut Provider,
) -> Result<Ended, String> {
    let outbox = outbox.clone();
    let mut reader = FrameReader::new(input);

    loop {
        let frame = match reader.next_frame(provider.frame_limit()) {
            Ok(frame) => frame,
            Err(error) => return Err(format!("reading input: {error}")),
        };
        match frame {
            // End of input: the provider exits after draining what it produced.
            Frame::Eof => {
                outbox.close(DRAIN_ON_EXIT);
                return Ok(Ended::Normally);
            }
            Frame::Blank => continue,
            // A socket session polls. With no request in hand it still owes
            // re-checks: a grant can expire or be revoked from another
            // connection, and events committed by another session are
            // delivered, without waiting for this consumer to say anything
            // (CORE section 16.5).
            Frame::Idle => {
                if let Err(ended) = deliver_idle(&outbox, provider) {
                    return Ok(ended);
                }
            }
            Frame::TooLarge => return frame_failure(&outbox, provider, FrameFailure::TooLarge),
            Frame::Bytes(bytes) => {
                let value = match frames::parse_frame(&bytes) {
                    Ok(value) => value,
                    Err(failure) => return frame_failure(&outbox, provider, failure),
                };
                match jsonrpc::classify(&value) {
                    // A notification is neither processed nor answered.
                    jsonrpc::Incoming::Notification => continue,
                    jsonrpc::Incoming::Invalid { id } => {
                        let error = jsonrpc::error_response(id, jsonrpc::invalid_request_error());
                        if let Err(ended) = respond(&outbox, provider, &error) {
                            return Ok(ended);
                        }
                    }
                    jsonrpc::Incoming::Request { id, method, params } => {
                        // The command commits inside `handle`, before any
                        // output for it exists.
                        let response = match provider.handle(&method, &params) {
                            Ok(result) => jsonrpc::response(id, result),
                            Err(error) => {
                                jsonrpc::error_response(id, error.to_error_object(error.code))
                            }
                        };
                        if let Err(ended) = respond(&outbox, provider, &response) {
                            return Ok(ended);
                        }
                        // Notifications follow the response of the command that
                        // caused them, which queueing in this order guarantees
                        // (CORE section 16.5).
                        if let Err(ended) = deliver(&outbox, provider) {
                            return Ok(ended);
                        }
                    }
                }
            }
        }
    }
}

/// A frame-level failure: answer once with a null id, flush, and close without
/// reading further input (STREAM section 2).
fn frame_failure(
    outbox: &Arc<Outbox>,
    provider: &mut Provider,
    failure: FrameFailure,
) -> Result<Ended, String> {
    let error = Value::Object(vec![
        ("code".into(), Value::Int(failure.jsonrpc_code())),
        ("message".into(), Value::String(failure.code().into())),
        (
            "data".into(),
            Value::Object(vec![
                ("code".into(), Value::String(failure.code().into())),
                ("retry".into(), Value::String("no".into())),
                ("details".into(), Value::Object(vec![])),
            ]),
        ),
    ]);
    if let Err(ended) = respond(
        outbox,
        provider,
        &jsonrpc::error_response(Value::Null, error),
    ) {
        return Ok(ended);
    }
    outbox.close(DRAIN_ON_EXIT);
    Ok(Ended::Normally)
}

/// Queue a response, waiting for room first if it would exceed the bound.
fn respond(outbox: &Arc<Outbox>, provider: &mut Provider, value: &Value) -> Result<(), Ended> {
    let frame = frames::encode(value);
    let bound = provider.pending_output_bound();
    let pending = outbox.pending();
    // With nothing pending the frame is queued whatever its size: the frame
    // limit already bounds it, and waiting for room that already exists would
    // never end.
    if pending > 0 && pending + frame.len() > bound {
        make_room(outbox, provider)?;
    }
    outbox.push(frame);
    Ok(())
}

/// Produce and queue owed notifications without exceeding the bound.
fn deliver(outbox: &Arc<Outbox>, provider: &mut Provider) -> Result<(), Ended> {
    deliver_with(outbox, provider, false)
}

/// An idle poll reserves FIFO processing progress if the lock is occupied,
/// then returns so the session can read a command on the next loop iteration.
fn deliver_idle(outbox: &Arc<Outbox>, provider: &mut Provider) -> Result<(), Ended> {
    deliver_with(outbox, provider, true)
}

fn deliver_with(outbox: &Arc<Outbox>, provider: &mut Provider, idle: bool) -> Result<(), Ended> {
    loop {
        let bound = provider.pending_output_bound();
        let pending = outbox.pending();
        let drained = if idle {
            provider.poll_idle(bound.saturating_sub(pending), pending == 0)
        } else {
            Some(provider.drain_subscriptions(bound.saturating_sub(pending), pending == 0))
        };
        let Some((notifications, withheld)) = drained else {
            return Ok(());
        };
        for notification in notifications {
            outbox.push(frames::encode(&notification));
        }
        if !withheld {
            return Ok(());
        }
        make_room(outbox, provider)?;
    }
}

/// A stall has started. Wait for the consumer to drain everything pending
/// within the room deadline, or close the connection.
fn make_room(outbox: &Arc<Outbox>, provider: &mut Provider) -> Result<(), Ended> {
    let stall_started = Instant::now();
    if outbox.wait_drained(provider.backpressure_notice()) {
        return Ok(());
    }
    Err(Ended::TooSlow(close_too_slow(
        outbox,
        provider,
        stall_started,
    )))
}

/// Close a too-slow consumer's connection (CORE section 16.5).
///
/// A session that negotiated `core.events.backpressure` is sent one final
/// notification per subscription with reason `consumer_too_slow`, all sharing
/// **one** budget of `backpressure_notice_ms` from the moment the consumer is
/// declared too slow. A session that did not is sent none. Either way every
/// subscription ends and nothing more is written once the budget ends,
/// whether or not the notices were.
fn close_too_slow(
    outbox: &Arc<Outbox>,
    provider: &mut Provider,
    stall_started: Instant,
) -> TooSlow {
    let declared = Instant::now();
    let pending = outbox.pending();
    // Measured before any notice is queued: the notices are the one thing
    // allowed past the bound, so this is the figure the bound governs.
    let max_pending_before_notices = outbox.max_pending();
    let ending = provider.end_subscriptions("consumer_too_slow");
    let (notices, notices_written) = if provider.backpressure_negotiated() {
        let count = ending.len();
        for notice in ending {
            outbox.push(frames::encode(&notice));
        }
        (count, outbox.wait_drained(provider.backpressure_notice()))
    } else {
        (0, false)
    };
    let max_pending = outbox.max_pending();
    outbox.abandon();
    TooSlow {
        bound: provider.pending_output_bound(),
        pending,
        max_pending,
        max_pending_before_notices,
        stall_started,
        declared,
        closed: Instant::now(),
        notices,
        notices_written,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mode};
    use crate::outbox::tests::Blocking;
    use std::sync::{Mutex, mpsc};

    const BOUND: i64 = 2048;

    fn provider(directory: &std::path::Path, notice_ms: i64) -> Provider {
        let config = Config {
            mode: Mode::Conformance,
            principal: "owner".into(),
            authority_principals: vec!["owner".into()],
            events_max_pending_notification_bytes: BOUND,
            events_backpressure_notice_ms: notice_ms,
            ..Config::default()
        };
        let clock = crate::clock::Clock::open(crate::clock::Source::System).expect("clock");
        Provider::open(config, clock, directory).expect("opens")
    }

    /// A pipelined client that never waits for a response: negotiation,
    /// `subscriptions` subscriptions, then `puts` writes, all already sent.
    /// Each value is 300 bytes, so with three subscriptions one write owes more
    /// notification bytes than the bound holds.
    fn pipelined(backpressure: bool, subscriptions: usize, puts: usize) -> Vec<u8> {
        let features = if backpressure {
            r#""core.events","core.events.backpressure""#
        } else {
            r#""core.events""#
        };
        let mut input = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"core.negotiate","params":{{"operation":"core.negotiate","message_id":"m-n","payload":{{"caller":{{"name":"t","version":"1"}},"receive_limits":{{"max_frame_bytes":1048576}},"profiles":[{{"name":"core","majors":[1],"required":true,"required_features":[{features}],"optional_features":[]}},{{"name":"core-test","majors":[1],"required":true,"required_features":[],"optional_features":[]}}]}}}}}}"#
        );
        input.push('\n');
        for n in 0..subscriptions {
            input.push_str(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"core.events.subscribe","params":{{"operation":"core.events.subscribe","message_id":"m-s{n}","payload":{{"from":"now"}}}}}}"#
            ));
            input.push('\n');
        }
        for n in 0..puts {
            let envelope = format!(
                r#"{{"operation":"core-test.subject.put","message_id":"m-{n}","command_id":"cmd-{n}","dedupe_generation":1,"subject":{{"kind":"core-test.subject","id":"s-{n}"}},"preconditions":[{{"subject":{{"kind":"core-test.subject","id":"s-{n}"}},"revision":0}}],"authority_epoch":0,"requires":[],"payload":{{"value":"{n:0>300}"}}}}"#
            );
            let value = cbr_encoding::parse(envelope.as_bytes()).expect("parses");
            let digest = cbr_encoding::command_digest(&value).expect("intent");
            input.push_str(&format!(
                r#"{{"jsonrpc":"2.0","id":{n},"method":"core-test.subject.put","params":{}}}"#,
                envelope.replace(
                    r#""payload""#,
                    &format!(r#""command_digest":"{digest}","payload""#)
                )
            ));
            input.push('\n');
        }
        input.into_bytes()
    }

    /// Run a session on its own thread, with a watchdog: a session that waits
    /// on its consumer without a deadline fails the test instead of hanging it.
    fn serve_with_watchdog<W: Write + Send + 'static>(
        input: Vec<u8>,
        out: W,
        mut provider: Provider,
        watchdog: Duration,
    ) -> (Ended, Duration) {
        let (done, finished) = mpsc::channel();
        std::thread::spawn(move || {
            let started = Instant::now();
            let ended = serve(&input[..], out, &mut provider, None).expect("serves");
            let _ = done.send((ended, started.elapsed()));
        });
        finished
            .recv_timeout(watchdog)
            .expect("the session ended within its watchdog; it must not wait on a consumer forever")
    }

    /// A writer that holds everything until `until`, then passes it through.
    struct Delayed {
        until: Instant,
        sink: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for Delayed {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let now = Instant::now();
            if now < self.until {
                std::thread::sleep(self.until - now);
            }
            self.sink.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn frames(bytes: &[u8]) -> Vec<Value> {
        bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| cbr_encoding::parse(line).expect("frame parses"))
            .collect()
    }

    #[test]
    fn a_consumer_that_never_reads_is_closed_within_twice_the_notice_budget() {
        let directory = tempfile::tempdir().expect("temp dir");
        let notice = Duration::from_millis(300);
        let (release, blocked) = mpsc::channel::<()>();
        let (wrote, written) = mpsc::channel();

        // Three subscriptions, so a notice budget restarted per subscription
        // would take four budgets to close, not two.
        let (ended, elapsed) = serve_with_watchdog(
            pipelined(true, 3, 200),
            Blocking {
                release: blocked,
                wrote,
            },
            provider(directory.path(), notice.as_millis() as i64),
            Duration::from_secs(10),
        );
        let Ended::TooSlow(record) = ended else {
            panic!("a consumer that never reads is too slow: {ended:?}");
        };
        // The measured ending, for the test log.
        eprintln!("{}", record.record());

        // Time, in the two phases CORE section 11 actually bounds: declared
        // at the room deadline, then closed within the notice budget of
        // *being declared*. Each phase gets its own scheduling slack.
        //
        // This was one bound on the sum with a single slack, and it failed
        // on a loaded CI runner at 861ms against 850: the product had done
        // exactly the right thing -- declared at 420ms, closed 441ms later
        // -- and two independent scheduling delays had added up while the
        // one slack did not. Per phase, the same two bounds hold and the
        // sum is their honest composition.
        let slack = Duration::from_millis(250);
        let to_declared = record.declared.duration_since(record.stall_started);
        let declared_to_closed = record.closed.duration_since(record.declared);
        let to_closed = record.closed.duration_since(record.stall_started);
        assert!(
            to_declared >= notice && to_declared < notice + slack,
            "declared too slow at the room deadline: {to_declared:?}"
        );
        assert!(
            declared_to_closed < notice + slack,
            "closed within the notice budget of being declared: \
             {declared_to_closed:?}; {}",
            record.record()
        );
        assert!(
            to_closed < 2 * (notice + slack),
            "closed within 2x the notice budget of the stall: {to_closed:?}; {}",
            record.record()
        );
        assert!(
            elapsed < 2 * notice + Duration::from_secs(2),
            "the whole session, which never wrote a byte, ended promptly: {elapsed:?}"
        );

        // Space. Everything the bound governs stayed within it; the notices
        // are the only frames queued past it.
        assert!(
            record.max_pending_before_notices <= record.bound,
            "output produced before closure stayed within the bound: {}",
            record.record()
        );
        assert_eq!(record.notices, 3, "one notice per subscription");
        assert!(!record.notices_written, "the consumer never read them");

        // The consumer never received a byte, and never will: closure writes
        // nothing more.
        assert!(written.try_recv().is_err());
        drop(release);
    }

    #[test]
    fn an_older_consumer_is_closed_without_a_notice() {
        let directory = tempfile::tempdir().expect("temp dir");
        let notice = Duration::from_millis(300);
        let (release, blocked) = mpsc::channel::<()>();
        let (wrote, _written) = mpsc::channel();

        let (ended, _) = serve_with_watchdog(
            pipelined(false, 1, 200),
            Blocking {
                release: blocked,
                wrote,
            },
            provider(directory.path(), notice.as_millis() as i64),
            Duration::from_secs(10),
        );
        let Ended::TooSlow(record) = ended else {
            panic!("bounded and closed without the feature too: {ended:?}");
        };
        assert_eq!(
            record.notices, 0,
            "a session that did not negotiate the feature is never sent consumer_too_slow"
        );
        let to_closed = record.closed.duration_since(record.stall_started);
        assert!(
            to_closed < notice + Duration::from_millis(250),
            "without the feature, closed within one notice budget: {to_closed:?}"
        );
        drop(release);
    }

    #[test]
    fn a_consumer_returning_within_the_notice_budget_receives_the_ending_notice() {
        let directory = tempfile::tempdir().expect("temp dir");
        // Wide, so the release lands between the declaration (one budget after
        // the stall) and closure (two budgets after it) on a slow machine too.
        let notice = Duration::from_millis(1000);
        let sink = Arc::new(Mutex::new(Vec::new()));
        let (ended, _) = serve_with_watchdog(
            pipelined(true, 1, 200),
            Delayed {
                until: Instant::now() + notice + notice / 2,
                sink: sink.clone(),
            },
            provider(directory.path(), notice.as_millis() as i64),
            Duration::from_secs(15),
        );
        let Ended::TooSlow(record) = ended else {
            panic!("still too slow: it missed the room deadline: {ended:?}");
        };
        assert!(record.notices_written, "{}", record.record());

        let received = frames(&sink.lock().unwrap());
        let last = received.last().expect("something was written");
        assert_eq!(
            last.get("params")
                .and_then(|p| p.get("ended"))
                .and_then(|e| e.get("reason"))
                .and_then(Value::as_str),
            Some("consumer_too_slow"),
            "the last frame written is the ending notice: {last:?}"
        );
    }

    #[test]
    fn withheld_notifications_are_delivered_later_and_never_skipped() {
        let directory = tempfile::tempdir().expect("temp dir");
        let sink = Arc::new(Mutex::new(Vec::new()));
        let puts = 120;
        // Three subscriptions of 300-byte values, so each write owes more
        // than 2 KiB of notifications against a 2 KiB bound: some are withheld on every
        // write, whether or not the consumer is keeping up. With one
        // subscription a response and its notification always fit together,
        // nothing is ever withheld, and this test would pass against a
        // provider that skips what it withholds — which is how its first
        // version did.
        let (ended, _) = serve_with_watchdog(
            pipelined(true, 3, puts),
            Delayed {
                until: Instant::now() + Duration::from_millis(150),
                sink: sink.clone(),
            },
            provider(directory.path(), 5000),
            Duration::from_secs(30),
        );
        assert!(
            matches!(ended, Ended::Normally),
            "a consumer that keeps up is not closed: {ended:?}"
        );

        let mut sequences: std::collections::BTreeMap<String, Vec<i64>> = Default::default();
        for frame in frames(&sink.lock().unwrap()) {
            if frame.get("method").and_then(Value::as_str) != Some("core.events.notify") {
                continue;
            }
            let params = frame.get("params").expect("params");
            let subscription = params
                .get("subscription")
                .and_then(Value::as_str)
                .expect("subscription")
                .to_string();
            for item in params
                .get("items")
                .and_then(Value::as_array)
                .unwrap_or_default()
            {
                if let Some(Value::Int(sequence)) =
                    item.get("event").and_then(|e| e.get("sequence"))
                {
                    sequences
                        .entry(subscription.clone())
                        .or_default()
                        .push(*sequence);
                }
            }
        }
        let expected: Vec<i64> = (1..=puts as i64).collect();
        assert_eq!(
            sequences.len(),
            3,
            "every subscription delivered: {sequences:?}"
        );
        for (subscription, delivered) in &sequences {
            assert_eq!(
                delivered, &expected,
                "{subscription}: every event delivered once, in order, with none skipped"
            );
        }
    }
}
