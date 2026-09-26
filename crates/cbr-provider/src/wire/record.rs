//! The recording boundary: between the transport and the store.
//!
//! [`Recording`] wraps a transport. Every exchange passes through it on its
//! way back, is **redacted**, and is written before the answer reaches the
//! caller. Nothing else writes a model exchange, and [`record`] takes
//! [`Redacted`] — for which [`super::redact::redact`] is the only
//! constructor — so "redaction happens before the write" is a fact about
//! the types rather than about whoever wrote the call site.
//!
//! What is stored here is the exchange, not a sealed artifact: m4d turns
//! these rows into evidence with an ancestry and a readable set. What m4b
//! owes is that **no unredacted byte ever reaches disk**, because a later
//! sweep cannot un-write one.

#![allow(dead_code)]

use rusqlite::{Connection, params};

use super::Dialect;
use super::redact::{Redacted, redact};
use super::response;
use crate::budget::{Ledger, LedgerError, Reservation, Settlement};
use crate::model::{Answer, Call, Exchange, Transport};

pub fn migrate(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS model_calls (
             id          INTEGER PRIMARY KEY,
             job         TEXT NOT NULL,
             request     TEXT NOT NULL,
             call        TEXT NOT NULL,
             dialect     TEXT NOT NULL,
             model       TEXT NOT NULL,
             sent        BLOB NOT NULL,
             received    BLOB NOT NULL,
             recorded_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS model_calls_job ON model_calls (job);",
    )?;
    // **The ledger row the call was sent under** (m5-settle), so a start
    // can reconcile a reservation a killed process left against what came
    // back. Added to a store written before it; its old rows name none,
    // and nor does an answer read as the provider's quota being gone
    // (`Recording::send_for`).
    let named: i64 = connection.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('model_calls') WHERE name = 'reservation'",
        [],
        |row| row.get(0),
    )?;
    if named == 0 {
        connection.execute_batch("ALTER TABLE model_calls ADD COLUMN reservation INTEGER")?;
    }
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS model_calls_reservation ON model_calls (reservation);",
    )
}

/// A transport that records every exchange, redacted, on its way past.
pub struct Recording<'a> {
    pub inner: &'a dyn Transport,
    pub store: &'a Connection,
    pub now: &'a str,
    pub job: &'a str,
    pub request: &'a str,
    pub model: &'a str,
    pub dialect: Dialect,
    /// The held credential, in the one form the boundary may have it: a
    /// thing that can replace occurrences and do nothing else.
    pub scrubber: Option<&'a crate::keychain::Scrubber>,
}

impl Recording<'_> {
    fn write(&self, reservation: Option<i64>, call: Call, body: &[u8], exchange: &Exchange) {
        // A failed write must not lose the answer: the ledger has already
        // reserved for this call and settling it is what keeps the spend
        // counted. The record is the thing that is missing, and m4d's
        // sealing is where an unrecorded call becomes an error.
        let _ = record(
            self.store,
            self.now,
            self.job,
            self.request,
            call,
            self.dialect,
            self.model,
            reservation,
            &redact(body, self.scrubber),
            &redact(&exchange.raw, self.scrubber),
        );
    }
}

impl Transport for Recording<'_> {
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        let exchange = self.inner.send(call, body);
        self.write(None, call, body, &exchange);
        exchange
    }

    /// The call path's send: the record names the reservation it was sent
    /// under, **before the answer reaches the call path**, so what came
    /// back is in the store before anything is settled on it.
    ///
    /// **Except an answer the transport read as the provider's quota being
    /// gone** (verification round 2): the call path settles that to
    /// nothing whatever usage its body reports, and the record cannot say
    /// so, since it keeps the body and not a 429's status. Naming no
    /// reservation, it is nothing a start reconciles, and a killed call's
    /// reservation stands at its estimate.
    fn send_for(&self, reservation: &Reservation, call: Call, body: &[u8]) -> Exchange {
        let exchange = self.inner.send_for(reservation, call, body);
        let named =
            (!matches!(exchange.answer, Answer::ProviderExhausted)).then_some(reservation.id);
        self.write(named, call, body, &exchange);
        exchange
    }
}

/// Write one exchange. **Takes [`Redacted`] and nothing else.**
#[allow(clippy::too_many_arguments)]
pub fn record(
    connection: &Connection,
    now: &str,
    job: &str,
    request: &str,
    call: Call,
    dialect: Dialect,
    model: &str,
    reservation: Option<i64>,
    sent: &Redacted,
    received: &Redacted,
) -> Result<(), rusqlite::Error> {
    let call = match call {
        Call::Count => "count",
        Call::Completion => "completion",
    };
    connection.execute(
        "INSERT INTO model_calls
             (job, request, call, dialect, model, sent, received, recorded_at, reservation)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            job,
            request,
            call,
            dialect.name(),
            model,
            sent.bytes(),
            received.bytes(),
            now,
            reservation
        ],
    )?;
    Ok(())
}

/// **What a start makes of a reservation a killed process left**
/// (m5-settle, verification round 1). Called before anything is admitted;
/// returns how many it settled.
///
/// The recording boundary writes what came back before the call path
/// sees it, beside the reservation it was sent under. A process killed
/// after that and before its settlement landed leaves the reservation at
/// its estimate and no `overrun`. So each reservation still standing is
/// read against its recorded exchange, and when the usage the response
/// reports — the transport's own reading, [`response::accounting`] — is
/// above the estimate, it is settled to that usage, which writes its
/// `overrun`. A usage within the reservation is left at its estimate,
/// which over-counts, as the crash matrix has it. An answer the transport
/// read as the provider's quota being gone names no reservation, so it is
/// not read here at all ([`Recording`]'s `send_for`).
///
/// **Nothing is reconciled for a call killed before its answer was
/// written**, on the wire or before this boundary's own write landed: its
/// reservation stands at its estimate, and a bill above it is not seen.
pub fn reconcile(connection: &Connection) -> Result<usize, LedgerError> {
    let standing: Vec<(i64, i64, String, Vec<u8>, String)> = {
        let mut statement = connection.prepare(
            "SELECT l.id, l.estimate, c.dialect, c.received, l.recorded_at
             FROM model_ledger l JOIN model_calls c ON c.reservation = l.id
             WHERE l.kind = 'reservation' ORDER BY l.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let ledger = Ledger::new(connection);
    let mut settled = 0;
    for (id, estimate, dialect, received, at) in standing {
        let Some(dialect) = Dialect::parse(&dialect) else {
            continue;
        };
        let (usage, _) = response::accounting(dialect, &received);
        let estimate = estimate.max(0) as u64;
        let Some(bill) = usage.filter(|usage| *usage > estimate) else {
            continue;
        };
        ledger.settle(&at, &Reservation { id, estimate }, Settlement::Usage(bill))?;
        settled += 1;
    }
    Ok(settled)
}

/// One recorded exchange: which call it was, what was sent, what came back.
pub type Row = (String, Vec<u8>, Vec<u8>);

/// Every recorded exchange, for a test or a report.
pub fn rows(connection: &Connection) -> Result<Vec<Row>, rusqlite::Error> {
    let mut statement =
        connection.prepare("SELECT call, sent, received FROM model_calls ORDER BY id")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    rows.collect()
}
