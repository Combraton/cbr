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
//! decides first**, and alone can refuse; the provider count runs only for a
//! request the local step has already admitted, and is itself a call with a
//! view, a record and a cost. Only the local half exists at m4a, and it is a
//! complete admission path: CI exercises it with no network at all.
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
}

impl Refusal {
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
            Refusal::PerRequest => false,
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
    /// The provider's own accounting of what was spent.
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

    /// Rows that count as spend. A refusal is recorded and is not one.
    /// Rows that count as spend. A refusal, an anomaly, a divergence and a
    /// mismatch are all recorded and none of them is one.
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
    pub fn settle(
        &self,
        now: &str,
        reservation: &Reservation,
        settlement: Settlement,
    ) -> Result<(), LedgerError> {
        let seconds = epoch_seconds(now).ok_or_else(|| LedgerError::Instant(now.to_string()))?;
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
        match tokens {
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
        // Spending more than was reserved is a fact about the estimate, and
        // READINESS section 3 requires it to be kept rather than absorbed.
        if let Settlement::Usage(tokens) = settlement
            && tokens > reservation.estimate
        {
            self.note(now, "", "", "divergence", tokens, reservation.estimate)?;
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
        let found: Option<i64> = self
            .connection
            .prepare("SELECT 1 FROM model_ledger WHERE kind = 'bound_unsound' LIMIT 1")?
            .query_row([], |row| row.get(0))
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
