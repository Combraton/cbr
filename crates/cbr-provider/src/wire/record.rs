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
use crate::model::{Call, Exchange, Transport};

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
}

impl Transport for Recording<'_> {
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        let exchange = self.inner.send(call, body);
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
            &redact(body),
            &redact(&exchange.raw),
        );
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
    sent: &Redacted,
    received: &Redacted,
) -> Result<(), rusqlite::Error> {
    let call = match call {
        Call::Count => "count",
        Call::Completion => "completion",
    };
    connection.execute(
        "INSERT INTO model_calls
             (job, request, call, dialect, model, sent, received, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            job,
            request,
            call,
            dialect.name(),
            model,
            sent.bytes(),
            received.bytes(),
            now
        ],
    )?;
    Ok(())
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
