//! The envelope: what CBR is allowed to spend, decided before anything is
//! sent, and a ledger that survives the process.
//!
//! **This module exists before any transport does.** M4's first stage is the
//! admission path, because the quota it guards is the owner's own, shared
//! with their other tools ([`docs/work/m4/READINESS.md`] §3): an overspend
//! degrades the environment they work in rather than merely costing money.
//! Nothing here opens a socket, reads a credential or knows what a provider
//! looks like.
//!
//! # Admission is two steps, because the count call is itself a send
//!
//! MiniMax's `POST /v1/responses/input_tokens` carries the **fully serialized
//! request** to the provider. Counting there first has already sent the body
//! it was deciding whether to send. So a **local, conservative estimate
//! decides first**, and alone can refuse. When it admits, no count is made.
//! The provider count runs only for a request the local step has
//! **refused**, and only on a counter a tighter figure could satisfy — the
//! window, the month, the job or the run, never the per-request ceiling
//! ([`Refusal::a_tighter_figure_could_admit`]). The count is itself a call
//! with a view, a record and a cost, admitted and settled like any other,
//! and the completion is then admitted on it. A caller whose purpose is
//! the count, which is the calibration, counts first instead. Only the
//! local half exists at m4a, and it is a complete admission path: CI
//! exercises it with no network at all.
//!
//! # A bill above its reservation stops the store (m5-settle)
//!
//! A settlement is charged whole, whatever it says: the counters hold what
//! the shared quota was charged. When that is more than the reservation,
//! the same transaction writes an `overrun` row naming the call's job and
//! request, and from then on [`Ledger::admit`] refuses everything, before
//! any ceiling, as it does once the local bound is recorded unsound. The
//! stop is a row, so it holds for every caller and across restarts; an
//! operator clears it only with a new data directory. It catches a
//! provider billing above what it was asked; it cannot prevent one, so a
//! store can pass a ceiling once, by what calls already admitted bill
//! above their reservations.
//!
//! **A settlement the store does not take** — another writer holding the
//! lock past the busy timeout, an I/O error, a full disk — writes none of
//! it, the `overrun` row included. The process keeps an overrun it could
//! not write, refuses every later admission on that store, and writes it
//! at the first admission the store takes ([`UNWRITTEN`]). A process
//! killed before its settlement landed is reconciled at the next start,
//! from what the recording boundary wrote (`wire::record::reconcile`).
//!
//! # The estimate is one-sided
//!
//! It may over-estimate. It must never under-estimate, because an
//! under-estimate admits a request the provider then charges for anyway.
//! [`estimate`] is therefore built on a property rather than a measurement:
//! **a byte-level BPE emits at most one token per byte of its input**, since
//! every token decodes to at least one byte and the tokens tile the input. So
//! the UTF-8 byte length of the serialized request is an upper bound on its
//! token count, whatever the vocabulary, and for any text in any script.
//!
//! It is a loose bound — three to four times the real count for English
//! prose — and it is deliberately the loose-and-sound choice rather than the
//! tight-and-unverifiable one. The alternative was MiniMax's published
//! `tokenizer.json`, vendored with its digest and licence; that requires
//! fetching it, which READINESS rules out at build and run time and which
//! this session has no authorisation to do. **No count here has been compared
//! with the provider's own**, and **byte-level is an assumption about the
//! provider's tokenizer rather than a fact CBR has checked**. The
//! calibration checks it — its own step after m4b merges, on the owner's
//! word, with one count above its local estimate stopping M4 outright.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension as _, params};

/// The owner's envelope ([STACK §8.1]). Two counters with different reset
/// semantics: the month can be almost untouched while the window is spent.
pub const WINDOW_TOKENS: u64 = 20_000_000;
pub const MONTH_TOKENS: u64 = 200_000_000;
/// The rolling window, in seconds. CBR's own five hours over its own ledger:
/// it cannot see the provider's counter and does not pretend to.
pub const WINDOW_SECONDS: i64 = 5 * 60 * 60;

/// Ceilings under the envelope, so one runaway job cannot consume a window
/// even when the window has room.
pub const PER_REQUEST_TOKENS: u64 = 250_000;
pub const PER_JOB_TOKENS: u64 = 1_000_000;

/// Per-message framing the provider counts and the serialized body does not
/// obviously show. Added on top of the byte bound, never instead of it.
pub const MESSAGE_OVERHEAD_TOKENS: u64 = 8;
/// The safety margin every completion reserves on top of its input and the
/// generation limit it asks for, per [MODEL-RUNTIME §2].
///
/// **There is no longer a *default* generation reserve.** m4a's `estimate`
/// added a fixed 4,096 because admission happened before the request's own
/// limit was known; it is known at every call site now, so the reservation
/// carries the real figure. The default was also what made the calibration
/// table's ratio column measure the reservation rather than the bound.
pub const SAFETY_MARGIN_TOKENS: u64 = 1_024;

/// The smallest generation budget worth asking for.
///
/// **Reasoning cannot be disabled on the M2.x models**, and the service
/// reports no breakdown, so `max_output_tokens` has to cover reasoning
/// *plus* the answer and CBR cannot learn the split by measuring. Below
/// this a model cannot finish a thought, and calibration run 2 proved what
/// that costs: sixteen tokens, all of them reasoning, no answer, and the
/// whole call wasted.
// Read by `wire::request::generation_for`, which m4c's selection call site
// calls. The calibration deliberately does not: READINESS §10 fixes its
// completion at sixteen tokens so the path is exercised end to end and
// cannot cost much even if everything else is wrong.
#[cfg_attr(not(test), allow(dead_code))]
pub const MIN_OUTPUT_TOKENS: u64 = 512;

/// The floor a **discovery** step's generation limit gets, whatever its
/// answer is worth.
///
/// **Measured, on the live run of 2026-09-22**, which is what this
/// milestone existed to measure. `MiniMax-M3` used 13 to 32 output
/// tokens for every step of six flows. `MiniMax-M2.7-highspeed` used
/// 277 to 512 tokens of reasoning *before* the answer, truncated two of
/// its three flows at the 512 of [`MIN_OUTPUT_TOKENS`], and truncated
/// one of them again at the 1,024 its repair asked for.
///
/// So reasoning is not proportional to the answer and cannot be sized
/// from it: [`REASONING_HEADROOM`] multiplies a figure that is already
/// small, and four times a short list of ids is still less room than
/// this model needs to think. The floor is set above every figure
/// measured, on the asymmetry that governs the whole rule — an unused
/// limit costs nothing, and a limit one token short costs the call.
///
/// **Selection is deliberately not changed.** No selection call
/// truncated in that run, and raising a bound that nothing has been
/// measured against would be the estimating this constant exists to
/// stop. [READINESS §3](../../docs/work/m4/READINESS.md) records it as
/// open.
pub const DISCOVERY_MIN_OUTPUT_TOKENS: u64 = 2_048;

/// How much room reasoning is given relative to the answer itself.
///
/// **The asymmetry decides this, not an estimate of how much a model
/// thinks.** Billing is by tokens produced, so a limit that is too large
/// costs nothing that is not used; a limit that is too small costs the
/// whole call and returns nothing. Generosity is therefore the cheap
/// error and parsimony the expensive one, and the multiplier is set
/// accordingly rather than tuned. m4e measures what is actually used.
#[cfg_attr(not(test), allow(dead_code))]
pub const REASONING_HEADROOM: u64 = 4;

/// How far below the local byte bound a provider's own count may fall before
/// it is disbelieved. The bound runs **three to four times** the real count
/// for prose, measured on this crate's own sources, so a figure below an
/// eighth of it is not a tighter count — it is a number to record as an
/// anomaly and not to act on. The local figure stands in that case, which is
/// the conservative direction.
pub const IMPLAUSIBLE_RATIO: u64 = 8;

/// Why a call was refused before it was sent. Each is a typed outcome that
/// reaches a context item as its unmet reason; none is retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// CBR's own rolling five hours would be exceeded.
    WindowExhausted,
    /// CBR's own calendar month would be exceeded.
    MonthExhausted,
    /// One request may not be this large whatever the envelope has left.
    PerRequest,
    /// This job has spent its allowance.
    PerJob,
    /// A launch-configured ceiling for this whole run.
    RunCeiling,
    /// This store once settled a call above its reservation. Durable: the
    /// store admits nothing again. Also this process's refusal on a store
    /// whose overrun it saw and could not write ([`UNWRITTEN`]).
    Overrun,
    /// This store once recorded the local bound being wrong. Durable, as
    /// above.
    BoundUnsound,
}

impl Refusal {
    /// Every refusal, for the tests that must see each one.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [Refusal; 7] = [
        Refusal::WindowExhausted,
        Refusal::MonthExhausted,
        Refusal::PerRequest,
        Refusal::PerJob,
        Refusal::RunCeiling,
        Refusal::Overrun,
        Refusal::BoundUnsound,
    ];

    /// Whether a **tighter figure for the same request** could turn this
    /// refusal into an admission.
    ///
    /// The three quota counters and the run ceiling, yes: they are about
    /// how much is left, and the local bound over-states what this request
    /// will use by three to four times. **The per-request ceiling, no** —
    /// it is a policy limit on how large one request may be, and the local
    /// bound is deliberately the conservative measure of that. Letting the
    /// provider's figure talk CBR into sending a larger request inverts
    /// the direction the bound exists to protect.
    pub fn a_tighter_figure_could_admit(self) -> bool {
        match self {
            Refusal::WindowExhausted
            | Refusal::MonthExhausted
            | Refusal::PerJob
            | Refusal::RunCeiling => true,
            Refusal::PerRequest | Refusal::Overrun | Refusal::BoundUnsound => false,
        }
    }

    /// The reason a consumer reads. `budget_exhausted` for the envelope's own
    /// two counters, and the ceilings named separately so a caller can tell a
    /// policy limit from an exhausted quota.
    pub fn reason(self) -> &'static str {
        match self {
            Refusal::WindowExhausted | Refusal::MonthExhausted => "budget_exhausted",
            Refusal::PerRequest => "request_over_ceiling",
            Refusal::PerJob => "job_over_ceiling",
            Refusal::RunCeiling => "run_over_ceiling",
            Refusal::Overrun => "reservation_overrun",
            Refusal::BoundUnsound => "local_bound_unsound",
        }
    }
}

/// How a reserved call ended.
///
/// **Split by what is known, not by whether the call succeeded.** A failure
/// after the body went out is a call the provider may well have charged for,
/// and settling it to zero under-counts a shared quota. A failure before
/// anything left the process spent nothing and settling it to anything else
/// over-counts. The transport says which, because only the transport knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    /// The provider's own accounting of what was spent, **charged whole**
    /// even above the reservation. Above it, the settlement also records an
    /// `overrun` naming the call, and the store admits nothing again.
    Usage(u64),
    /// **Sent, and no usage reported.** The reservation's estimate stands:
    /// the provider may have charged and nobody said how much, so the
    /// conservative figure is the one already written.
    UsageUnknown,
    /// **Nothing left the process.** Nothing was spent.
    NothingSpent,
    /// **The provider says its quota is exhausted while CBR's ledger has
    /// room.** The quota is shared with the owner's other tools and CBR sees
    /// only its own spending, so this will happen and it is not
    /// `budget_exhausted`: reporting it as such would name the wrong limit.
    /// Never retried.
    ProviderExhausted,
}

impl Settlement {
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Settlement::Usage(_) => None,
            Settlement::UsageUnknown => Some("model_call_failed"),
            Settlement::NothingSpent => Some("model_not_sent"),
            Settlement::ProviderExhausted => Some("provider_quota_exhausted"),
        }
    }
}

/// A reservation written before a send. Holding one is the only way to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub id: i64,
    pub estimate: u64,
}

/// What the ledger holds right now, for a caller that wants to report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spend {
    pub window: u64,
    pub month: u64,
    pub job: u64,
}

#[derive(Debug)]
pub enum LedgerError {
    Storage(rusqlite::Error),
    /// An instant the clock produced that this module cannot read. Refusing
    /// is the conservative answer: a ledger that cannot place a row in time
    /// cannot enforce a window.
    Instant(String),
}

impl From<rusqlite::Error> for LedgerError {
    fn from(error: rusqlite::Error) -> Self {
        LedgerError::Storage(error)
    }
}

/// A settlement the store did not take, and whether this process kept it
/// ([`UNWRITTEN`]). A caller charges a kept one its bill, which is what the
/// ledger will hold, and any other its reservation's estimate. Read from
/// the settlement itself rather than from [`UNWRITTEN`] afterwards, where
/// another worker's admission may already have written it and removed it.
#[derive(Debug)]
pub struct Unsettled {
    pub error: LedgerError,
    pub kept: bool,
}

impl From<Unsettled> for LedgerError {
    fn from(unsettled: Unsettled) -> Self {
        unsettled.error
    }
}

impl std::fmt::Display for Unsettled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::Storage(error) => write!(formatter, "storage: {error}"),
            LedgerError::Instant(instant) => write!(formatter, "unreadable instant: {instant}"),
        }
    }
}

/// The input alone: the byte bound plus the provider's per-message
/// framing, **without** the reserved generation and margin.
///
/// [`estimate`] adds a default generation reserve on top, which is right
/// for admitting a call whose generation limit is not yet known. A
/// completion's reservation knows the limit the request actually asks for,
/// so it adds that instead — and adding both would reserve the default
/// twice.
pub fn input_bound(serialized: &[u8], messages: usize) -> u64 {
    worst_case_tokens(serialized) + MESSAGE_OVERHEAD_TOKENS * messages as u64
}

/// **What a completion reserves on the local bound**: the input bound of
/// the body as serialized and of the messages the provider frames, plus
/// the generation the body binds and the margin.
///
/// One function, because two places need the figure and must agree:
/// admission reserves it, and the arithmetic that says what a question
/// can hold prices it (`model::send_worst`). A count reserves its input
/// alone, since it generates nothing, and a completion admitted on a count
/// reserves the count in place of the input bound.
pub fn reservation(serialized: &[u8], messages: usize, generation: u64) -> u64 {
    input_bound(serialized, messages)
        .saturating_add(generation)
        .saturating_add(SAFETY_MARGIN_TOKENS)
}

/// The largest number of tokens any byte-level BPE could emit for `text`:
/// one per byte. Used by the corpus test as the reference the estimate must
/// never fall below.
pub fn worst_case_tokens(text: &[u8]) -> u64 {
    text.len() as u64
}

/// Seconds since the epoch for an RFC 3339 instant in UTC.
pub fn epoch_seconds(instant: &str) -> Option<i64> {
    // RFC 3339 in UTC, which is the only shape the clock produces.
    let bytes = instant.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' || !instant.ends_with('Z') {
        return None;
    }
    let number = |from: usize, to: usize| instant.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // Howard Hinnant's days-from-civil: exact, branch-free and with no
    // dependency, which matters in the credential and budget path.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// **Overruns this process saw and the store did not take**, by the
/// store's database file (m5-settle, verification round 1).
///
/// A settlement is one transaction, so a store that refuses it takes none
/// of it and its `overrun` row is not written. A store named here admits
/// nothing again in this process, whatever its rows say, and each
/// admission first writes what is kept, if the store takes it now. Keyed
/// by the file, so every connection this process opens to the store
/// reads it; a store with no file, which only a test opens, has its rows
/// alone.
static UNWRITTEN: Mutex<BTreeMap<String, Vec<Unwritten>>> = Mutex::new(BTreeMap::new());

/// One settlement above its reservation that the store did not take.
#[derive(Debug, Clone)]
struct Unwritten {
    now: String,
    reservation: Reservation,
    tokens: u64,
}

fn unwritten() -> MutexGuard<'static, BTreeMap<String, Vec<Unwritten>>> {
    UNWRITTEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The database file a connection has open, when it has one.
fn store_file(connection: &Connection) -> Option<String> {
    connection
        .path()
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

/// The ledger: durable, conservative, and the only thing that admits a call.
pub struct Ledger<'a> {
    connection: &'a Connection,
    run_ceiling: Option<u64>,
}

impl<'a> Ledger<'a> {
    pub fn new(connection: &'a Connection) -> Self {
        Ledger {
            connection,
            run_ceiling: None,
        }
    }

    /// A ceiling for this whole run, from launch configuration. It can only
    /// lower the owner's envelope.
    pub fn with_run_ceiling(mut self, ceiling: Option<u64>) -> Self {
        self.run_ceiling = ceiling;
        self
    }

    /// Everything this ledger has ever spent, which is what a run ceiling
    /// bounds. The window rolls and the month resets; a run does neither.
    fn run_spend(&self) -> Result<u64, LedgerError> {
        let value: Option<i64> = self
            .connection
            .prepare(&format!(
                "SELECT SUM(tokens) FROM model_ledger WHERE {}",
                Self::SPENT
            ))?
            .query_row([], |row| row.get(0))
            .optional()?
            .flatten();
        Ok(value.unwrap_or(0).max(0) as u64)
    }

    pub fn migrate(connection: &Connection) -> Result<(), LedgerError> {
        // The same store, under the same WAL, `synchronous=FULL` and
        // `BEGIN IMMEDIATE` discipline as everything else: a ledger with
        // weaker durability than the records it guards would be the weakest
        // link in the thing it exists to enforce.
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_ledger (
                 id          INTEGER PRIMARY KEY,
                 job         TEXT    NOT NULL,
                 request     TEXT    NOT NULL,
                 kind        TEXT    NOT NULL,
                 tokens      INTEGER NOT NULL,
                 estimate    INTEGER NOT NULL,
                 recorded_at TEXT    NOT NULL,
                 seconds     INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS model_ledger_time ON model_ledger (seconds);
             CREATE INDEX IF NOT EXISTS model_ledger_job ON model_ledger (job);",
        )?;
        Ok(())
    }

    /// Rows that count as spend. A refusal, an anomaly, a divergence, an
    /// overrun and a mismatch are all recorded and none of them is one: an
    /// overrun's tokens are the bill its settled row already counts.
    const SPENT: &'static str =
        "kind IN ('reservation', 'usage', 'unknown', 'provider_exhausted', 'not_sent')";

    /// The local half of admission. Refuses without writing a reservation;
    /// admits by writing one, **before anything is sent**, so that a crash
    /// between the reservation and the send leaves the spend counted rather
    /// than forgotten.
    pub fn admit(
        &self,
        now: &str,
        job: &str,
        request: &str,
        estimate: u64,
    ) -> Result<Result<Reservation, Refusal>, LedgerError> {
        // **An overrun this process saw and the store did not take** is
        // written first, if the store takes it now, and is a stop either
        // way. The stop is read inside the admission's transaction, with
        // the store's own; when the store cannot be asked, or cannot say,
        // it is this process's and needs neither.
        self.write_unwritten();
        let admitted = self.admit_once(now, job, request, estimate);
        if admitted.is_err() && self.holds_a_stop() {
            return Ok(Err(Refusal::Overrun));
        }
        admitted
    }

    /// Whether this process kept an overrun on this store that the store
    /// did not take. Once it has, the store admits nothing again here,
    /// whether or not the overrun has since been written.
    fn holds_a_stop(&self) -> bool {
        store_file(self.connection).is_some_and(|file| unwritten().contains_key(&file))
    }

    /// Write what this process kept for this store, where the store takes
    /// it.
    fn write_unwritten(&self) {
        let Some(file) = store_file(self.connection) else {
            return;
        };
        let Some(kept) = unwritten().get(&file).cloned() else {
            return;
        };
        let written: Vec<i64> = kept
            .iter()
            .filter(|kept| {
                self.settle_once(&kept.now, &kept.reservation, Settlement::Usage(kept.tokens))
                    .is_ok()
            })
            .map(|kept| kept.reservation.id)
            .collect();
        if let Some(kept) = unwritten().get_mut(&file) {
            kept.retain(|kept| !written.contains(&kept.reservation.id));
        }
    }

    fn admit_once(
        &self,
        now: &str,
        job: &str,
        request: &str,
        estimate: u64,
    ) -> Result<Result<Reservation, Refusal>, LedgerError> {
        // **One write transaction across the check and the insert.** m4c
        // adds concurrency, and a check-then-write race is an overspend:
        // two admissions could each read a spend the other was about to
        // write and both be admitted. `BEGIN IMMEDIATE` takes the write lock
        // before the read, so the second waits or fails rather than racing.
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        let admitted = self.admit_locked(now, job, request, estimate);
        let ended = self.connection.execute_batch("COMMIT");
        match (admitted, ended) {
            (Ok(admitted), Ok(())) => Ok(admitted),
            (admitted, ended) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                ended?;
                admitted
            }
        }
    }

    fn admit_locked(
        &self,
        now: &str,
        job: &str,
        request: &str,
        estimate: u64,
    ) -> Result<Result<Reservation, Refusal>, LedgerError> {
        // **The store's stops, before every ceiling**, and inside the same
        // transaction as the insert: every caller admits here, so no path
        // (the `Counting::Always` one included) and no restart skips them,
        // and a stop written while a call's count was out refuses its
        // completion. The process's own stop comes after the store's, so
        // an unsound bound is still reported first, and is read here too,
        // under the lock (m5-settle, verification round 2): read before
        // it, an admission already waiting on the lock while a settlement
        // was refused would be granted once the lock came free.
        let stop = match self.stop()? {
            Some(stop) => Some(stop),
            None => self.holds_a_stop().then_some(Refusal::Overrun),
        };
        if let Some(stop) = stop {
            self.write(now, job, request, stop.reason(), 0, estimate)?;
            return Ok(Err(stop));
        }
        let spend = self.spend(now, job)?;
        let run = self.run_spend()?;
        // Ceilings first, so a caller learns it asked for something no
        // envelope would ever allow rather than that the quota is low.
        let refusal = if estimate > PER_REQUEST_TOKENS {
            Some(Refusal::PerRequest)
        } else if spend.job + estimate > PER_JOB_TOKENS {
            Some(Refusal::PerJob)
        } else if spend.window + estimate > WINDOW_TOKENS {
            Some(Refusal::WindowExhausted)
        } else if spend.month + estimate > MONTH_TOKENS {
            Some(Refusal::MonthExhausted)
        } else if self
            .run_ceiling
            .is_some_and(|ceiling| run + estimate > ceiling)
        {
            // Checked **after** the envelope, so a ceiling can only lower it.
            Some(Refusal::RunCeiling)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            // A refusal is an event, so that a report can say how often the
            // envelope bound and which counter did it.
            self.write(now, job, request, refusal.reason(), 0, estimate)?;
            return Ok(Err(refusal));
        }
        let id = self.write(now, job, request, "reservation", estimate, estimate)?;
        Ok(Ok(Reservation { id, estimate }))
    }

    /// One row, in one transaction, with the instant read once.
    fn write(
        &self,
        now: &str,
        job: &str,
        request: &str,
        kind: &str,
        tokens: u64,
        estimate: u64,
    ) -> Result<i64, LedgerError> {
        let seconds = epoch_seconds(now).ok_or_else(|| LedgerError::Instant(now.to_string()))?;
        let kind = if kind == "budget_exhausted"
            || kind == "request_over_ceiling"
            || kind == "job_over_ceiling"
            || kind == "reservation_overrun"
            || kind == "local_bound_unsound"
        {
            "refusal"
        } else {
            kind
        };
        self.connection.execute(
            "INSERT INTO model_ledger (job, request, kind, tokens, estimate, recorded_at, seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                job,
                request,
                kind,
                tokens as i64,
                estimate as i64,
                now,
                seconds
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Replace a reservation's estimate with what was actually spent. A
    /// reservation that is never settled stays counted at its estimate,
    /// which over-counts rather than loses a spend.
    ///
    /// **One transaction**: the settled row and, when the bill is above
    /// the reservation, the `overrun` row that stops the store. Apart, a
    /// failure between them would count the bill and record no stop.
    ///
    /// A bill above the reservation that the store does not take is kept
    /// by this process ([`UNWRITTEN`]): the store admits nothing again here,
    /// and the next admission it takes writes it. The error says whether
    /// it was kept ([`Unsettled`]).
    pub fn settle(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
    ) -> Result<(), Unsettled> {
        self.settle_once(now, reservation, settlement)
    }

    /// Keep a settlement the store did not take, when it is a bill above
    /// its reservation on a store with a file, once per reservation.
    /// Returns whether it is kept.
    fn keep(&self, now: &str, reservation: &Reservation, settlement: Settlement) -> bool {
        let Settlement::Usage(tokens) = settlement else {
            return false;
        };
        if tokens <= reservation.estimate {
            return false;
        }
        let Some(file) = store_file(self.connection) else {
            return false;
        };
        let mut kept = unwritten();
        let kept = kept.entry(file).or_default();
        if !kept
            .iter()
            .any(|kept| kept.reservation.id == reservation.id)
        {
            kept.push(Unwritten {
                now: now.to_string(),
                reservation: reservation.clone(),
                tokens,
            });
        }
        true
    }

    /// Whether this process kept a settlement of `reservation` that the
    /// store did not take, to write at the next admission it takes.
    #[cfg(test)]
    pub fn keeps(&self, reservation: &Reservation) -> bool {
        store_file(self.connection).is_some_and(|file| {
            unwritten().get(&file).is_some_and(|kept| {
                kept.iter()
                    .any(|kept| kept.reservation.id == reservation.id)
            })
        })
    }

    fn settle_once(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
    ) -> Result<(), Unsettled> {
        let unsettled = |error: LedgerError| Unsettled {
            kept: self.keep(now, reservation, settlement),
            error,
        };
        let seconds =
            epoch_seconds(now).ok_or_else(|| unsettled(LedgerError::Instant(now.to_string())))?;
        self.connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| unsettled(error.into()))?;
        self.settle_locked(now, seconds, reservation, settlement)
            .and_then(|()| Ok(self.connection.execute_batch("COMMIT")?))
            .map_err(|error| {
                // **Kept before the ROLLBACK**, while this transaction still
                // holds the write lock (m5-settle, verification round 3):
                // kept after it, an admission already waiting on the lock
                // would be granted it while the process held no stop.
                let unsettled = unsettled(error);
                // Nothing of a settlement lands unless all of it does; the
                // reservation then stands at its estimate.
                let _ = self.connection.execute_batch("ROLLBACK");
                unsettled
            })
    }

    fn settle_locked(
        &self,
        now: &str,
        seconds: i64,
        reservation: &Reservation,
        settlement: Settlement,
    ) -> Result<(), LedgerError> {
        let (kind, tokens) = match settlement {
            Settlement::Usage(tokens) => ("usage", Some(tokens)),
            // Sent, and nobody said what it cost: the estimate stands, which
            // over-counts rather than losing a spend that may have happened.
            Settlement::UsageUnknown => ("unknown", None),
            // Nothing left the process, so nothing was spent.
            Settlement::NothingSpent => ("not_sent", Some(0)),
            // A call the provider refused spent nothing; the reservation is
            // reconciled to that rather than left holding an estimate, which
            // is what "it does not corrupt the ledger" means.
            Settlement::ProviderExhausted => ("provider_exhausted", Some(0)),
        };
        // The reservation row **becomes** the settlement, so there is never a
        // moment when both are counted and never one when neither is.
        let changed = match tokens {
            Some(tokens) => self.connection.execute(
                "UPDATE model_ledger
                 SET kind = ?1, tokens = ?2, recorded_at = ?3, seconds = ?4
                 WHERE id = ?5 AND kind = 'reservation'",
                params![kind, tokens as i64, now, seconds, reservation.id],
            )?,
            None => self.connection.execute(
                "UPDATE model_ledger
                 SET kind = ?1, recorded_at = ?2, seconds = ?3
                 WHERE id = ?4 AND kind = 'reservation'",
                params![kind, now, seconds, reservation.id],
            )?,
        };
        // **A settlement lands once, and its overrun with it.** A
        // reservation already settled — by this process writing one it
        // kept, or by a start reconciling one a killed process left —
        // takes nothing more.
        if changed == 0 {
            return Ok(());
        }
        // Spending more than was reserved is kept rather than absorbed
        // (READINESS section 3), **as a stop naming the call**: the row
        // copies the reservation's job and request. Stores written before
        // m5-settle hold unattributed `divergence` rows, which stop nothing.
        if let Settlement::Usage(tokens) = settlement
            && tokens > reservation.estimate
        {
            self.connection.execute(
                "INSERT INTO model_ledger (job, request, kind, tokens, estimate, recorded_at, seconds)
                 SELECT job, request, 'overrun', ?1, ?2, ?3, ?4 FROM model_ledger WHERE id = ?5",
                params![
                    tokens as i64,
                    reservation.estimate as i64,
                    now,
                    seconds,
                    reservation.id
                ],
            )?;
        }
        Ok(())
    }

    /// What this ledger says has been spent, in the rolling window, in the
    /// calendar month, and by one job.
    pub fn spend(&self, now: &str, job: &str) -> Result<Spend, LedgerError> {
        let seconds = epoch_seconds(now).ok_or_else(|| LedgerError::Instant(now.to_string()))?;
        let month = now.get(0..7).unwrap_or_default();
        let total =
            |sql: String, parameters: &[&dyn rusqlite::ToSql]| -> Result<u64, LedgerError> {
                let value: Option<i64> = self
                    .connection
                    .prepare(&sql)?
                    .query_row(parameters, |row| row.get(0))
                    .optional()?
                    .flatten();
                Ok(value.unwrap_or(0).max(0) as u64)
            };
        Ok(Spend {
            window: total(
                format!(
                    "SELECT SUM(tokens) FROM model_ledger WHERE {} AND seconds > ?1",
                    Self::SPENT
                ),
                &[&(seconds - WINDOW_SECONDS)],
            )?,
            month: total(
                format!(
                    "SELECT SUM(tokens) FROM model_ledger WHERE {} AND substr(recorded_at, 1, 7) = ?1",
                    Self::SPENT
                ),
                &[&month],
            )?,
            job: total(
                format!(
                    "SELECT SUM(tokens) FROM model_ledger WHERE {} AND job = ?1",
                    Self::SPENT
                ),
                &[&job],
            )?,
        })
    }

    /// Record something that is not a spend: an anomaly, a divergence, a
    /// mismatched answer. Kept out of [`Self::SPENT`] on purpose — these are
    /// facts about a call, not charges for one.
    pub fn note(
        &self,
        now: &str,
        job: &str,
        request: &str,
        kind: &str,
        tokens: u64,
        estimate: u64,
    ) -> Result<(), LedgerError> {
        self.write(now, job, request, kind, tokens, estimate)?;
        Ok(())
    }

    /// Whether this store has ever recorded the local bound being wrong.
    ///
    /// **Durable, which is stronger than "for this process".** The bound is
    /// a property of the code, so a restart with the same code has the same
    /// bound; forgetting at restart would be forgetting the one observation
    /// that invalidates every admission the store has ever made.
    pub fn bound_is_unsound(&self) -> Result<bool, LedgerError> {
        self.recorded("bound_unsound")
    }

    /// Whether a completion admitted on a count was ever billed input above
    /// the count and the margin. Durable, like the bound's stop; it closes
    /// the serving count path and nothing else.
    pub fn count_is_unsound(&self) -> Result<bool, LedgerError> {
        self.recorded("count_unsound")
    }

    /// **The stop this store has recorded**, when it has one: an unsound
    /// bound before an overrun, the order a call ends in. Old `divergence`
    /// rows are not one.
    fn stop(&self) -> Result<Option<Refusal>, LedgerError> {
        let kind: Option<String> = self
            .connection
            .prepare(
                "SELECT kind FROM model_ledger WHERE kind IN ('bound_unsound', 'overrun')
                 ORDER BY kind = 'overrun' LIMIT 1",
            )?
            .query_row([], |row| row.get(0))
            .optional()?;
        Ok(kind.map(|kind| {
            if kind == "bound_unsound" {
                Refusal::BoundUnsound
            } else {
                Refusal::Overrun
            }
        }))
    }

    fn recorded(&self, kind: &str) -> Result<bool, LedgerError> {
        let found: Option<i64> = self
            .connection
            .prepare("SELECT 1 FROM model_ledger WHERE kind = ?1 LIMIT 1")?
            .query_row([kind], |row| row.get(0))
            .optional()?;
        Ok(found.is_some())
    }

    /// Every row, for a test or a report. Refusals are recorded and are not
    /// spend.
    // Read by the tests and by the crash matrix; the reporter that will
    // use it in production is m4c's, and this exists now because the rows
    // it returns are what the crash boundaries are checked against.
    #[allow(dead_code)]
    pub fn rows(&self) -> Result<Vec<(String, String, u64)>, LedgerError> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, estimate, tokens FROM model_ledger ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.to_string(),
                row.get::<_, i64>(2)?.max(0) as u64,
            ))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests;
