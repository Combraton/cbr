//! Durable provider state: one SQLite database plus a content-addressed object
//! store, per `docs/work/readiness/STACK.md` section 3.
//!
//! Two properties here are load-bearing and are asserted rather than assumed.
//!
//! **Durability settings are read back from the open connection**, not inferred
//! from the code that set them. `synchronous=NORMAL` in WAL mode is documented
//! to lose durability on power loss, which would let a committed row vanish
//! while the object it names survived -- a hole in a contiguous event stream,
//! and the reverse of the failure this store exists to prevent.
//!
//! **An object is published before the row that names it.** A crash may leave
//! an unreferenced object, which is collectable; it must never leave a row
//! claiming bytes that are not there. On Apple targets Rust's `File::sync_all`
//! is `F_FULLFSYNC` while SQLite's default commit sync is a plain `fsync`, so
//! the object ends up *more* durable than its row, which is the direction that
//! keeps the invariant.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use cbr_encoding::Value;

/// The subject kind a registered repository takes. CBR-owned: the protocol
/// defines no repository subject, and a grant narrows this kind by id exactly
/// as it narrows any other.
pub const REPOSITORY: &str = "cbr.repository";

/// A failure inside the derived memory, as a store error. It is corruption
/// of this file either way: the projections live in it.
fn memory_failed(error: cbr_memory::index::IndexError) -> StoreError {
    eprintln!("cbr-provider: derived memory: {error}");
    StoreError::Corrupt("the derived memory could not be migrated")
}
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// A provider-owned object, named by kind and id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectKey {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Default)]
pub struct SubjectState {
    /// Provider-owned integer. `0` means the subject does not exist, so a
    /// subject with no row is at revision 0 by definition.
    pub revision: i64,
    pub value: String,
    /// How many commands actually applied to this subject. A replay does not
    /// increment it, which is how a fixture tells an idempotent replay apart
    /// from a re-execution.
    pub applied_count: i64,
}

/// The durable record of an accepted command, which is what binds its identity.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub digest: String,
    /// The exact result to return on replay, with `replay` set to true.
    pub result: Value,
}

/// Hold a data directory for one serving process, for that process's
/// lifetime. A second provider over the same directory refuses to start.
///
/// Two would be unsafe, not just untidy: the start-time collection pass
/// deletes every object no committed row names, and an object another process
/// has published but not yet named is exactly such an object. The lock is an
/// advisory `flock` on `provider.lock`, which the kernel releases when the
/// process dies however it dies, so a `SIGKILL` never leaves the directory
/// held. Credential administration does not take it: it touches rows, never
/// objects, and runs beside a serving provider by design.
pub fn lock_data_dir(data_dir: &Path) -> Result<fs::File, String> {
    use std::os::unix::io::AsRawFd;
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("data directory {}: {error}", data_dir.display()))?;
    let path = data_dir.join("provider.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    // SAFETY: flock takes a valid open descriptor and two flag bits.
    let status = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if status == 0 {
        Ok(file)
    } else {
        Err(format!(
            "another provider is serving the data directory {}",
            data_dir.display()
        ))
    }
}

fn claim_revision_row(
    (revision, digest, record, recorded_at): (i64, String, String, String),
) -> Result<ClaimRevisionRow, StoreError> {
    let record = cbr_encoding::parse(record.as_bytes())
        .map_err(|_| StoreError::Corrupt("a claim revision record does not parse"))?;
    Ok(ClaimRevisionRow {
        revision,
        digest,
        record,
        recorded_at,
    })
}

/// What the object directory holds, as the collection pass sees it.
#[derive(Debug, Default)]
pub struct Inventory {
    pub objects: Vec<String>,
    pub staging: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    /// A durability setting did not read back as it was set.
    Pragma {
        name: &'static str,
        expected: String,
        found: String,
    },
    /// Bytes written did not match the digest they were published under.
    DigestMismatch {
        expected: String,
        computed: String,
    },
    /// The store's own state contradicts itself. Never a caller's mistake.
    Corrupt(&'static str),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "sqlite: {e}"),
            StoreError::Io(e) => write!(f, "io: {e}"),
            StoreError::Pragma {
                name,
                expected,
                found,
            } => {
                write!(f, "PRAGMA {name} read back as {found}, expected {expected}")
            }
            StoreError::DigestMismatch { expected, computed } => {
                write!(f, "object digest {computed} does not match {expected}")
            }
            StoreError::Corrupt(what) => write!(f, "inconsistent store: {what}"),
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Sqlite(error)
    }
}

impl From<StoreError> for crate::errors::ProtocolError {
    /// A store failure means nothing was bound, so the caller may retransmit
    /// the identical command. The detail stays on standard error rather than
    /// on the wire: a storage message is not a caller's business.
    fn from(error: StoreError) -> Self {
        eprintln!("cbr-provider: store: {error}");
        crate::errors::ProtocolError::unavailable()
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        StoreError::Io(error)
    }
}

/// One event a command appends. Committed in the same transaction as the
/// state change that caused it (CORE section 16.3): if that transaction fails,
/// neither exists.
pub struct NewEvent {
    pub event_type: String,
    /// Built from the subject's new revision, which is only known inside the
    /// transaction. The authority subject's event payload is its epoch, and
    /// its epoch is its revision, so the two cannot disagree.
    pub payload: Box<dyn FnOnce(i64) -> Value>,
    /// Copied unchanged from the command envelope (CORE section 16.2).
    pub caused_by: Vec<String>,
}

/// A position in the stream. Contiguous within an epoch, and meaningful only
/// for this provider's stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub epoch: i64,
    pub sequence: i64,
}

impl Position {
    pub fn to_value(self) -> Value {
        Value::Object(vec![
            ("epoch".into(), Value::Int(self.epoch)),
            ("sequence".into(), Value::Int(self.sequence)),
        ])
    }
}

/// The outcome of one read.
pub struct ReadEvents {
    pub stream_epoch: i64,
    pub items: Vec<Value>,
    pub next_cursor: Position,
    pub filtered: bool,
    /// The first item alone exceeds the caller's receive limit, so there is no
    /// honest partial answer: the caller is told rather than handed a view that
    /// silently omits it.
    pub first_item_too_large: bool,
}

fn epoch_change_item(from_epoch: i64, vouched_through: i64) -> Value {
    Value::Object(vec![(
        "epoch_change".into(),
        Value::Object(vec![
            ("from_epoch".into(), Value::Int(from_epoch)),
            ("to_epoch".into(), Value::Int(from_epoch + 1)),
            ("vouched_through".into(), Value::Int(vouched_through)),
        ]),
    )])
}

fn event_to_value(stream: &str, event: &EventRecord) -> Value {
    let mut members = vec![
        ("stream".into(), Value::String(stream.to_string())),
        ("epoch".into(), Value::Int(event.position.epoch)),
        ("sequence".into(), Value::Int(event.position.sequence)),
        ("type".into(), Value::String(event.event_type.clone())),
        (
            "subject".into(),
            Value::Object(vec![
                ("kind".into(), Value::String(event.subject.kind.clone())),
                ("id".into(), Value::String(event.subject.id.clone())),
            ]),
        ),
        ("revision".into(), Value::Int(event.revision)),
        ("origin".into(), Value::String(event.origin.clone())),
    ];
    // `operation_ref` and `command_id` are present exactly when the event was
    // caused by an accepted command (CORE section 16.2).
    if let Some(reference) = &event.operation_ref {
        members.push(("operation_ref".into(), Value::String(reference.clone())));
    }
    if let Some(command_id) = &event.command_id {
        members.push(("command_id".into(), Value::String(command_id.clone())));
    }
    members.push(("caused_by".into(), event.caused_by.clone()));
    members.push((
        "recorded_at".into(),
        Value::String(event.recorded_at.clone()),
    ));
    members.push(("payload".into(), event.payload.clone()));
    Value::Object(members)
}

/// A recorded event, as `core.events.read` returns it.
#[derive(Debug, Clone)]
pub struct EventRecord {
    pub position: Position,
    pub event_type: String,
    pub subject: SubjectKey,
    pub revision: i64,
    pub origin: String,
    pub operation_ref: Option<String>,
    pub command_id: Option<String>,
    pub caused_by: Value,
    pub recorded_at: String,
    pub payload: Value,
}

/// What one accepted command changes and binds.
pub struct Commit<'a> {
    pub key: &'a SubjectKey,
    pub value: &'a str,
    pub principal: &'a str,
    pub command_id: &'a str,
    pub digest: &'a str,
    pub generation: i64,
    /// The event this command appends, if any.
    pub event: Option<NewEvent>,
    /// Further subjects the same command changes, each with its own event, in
    /// the order they are appended. CORE section 16.3 requires the primary
    /// subject first, which is why it is not simply one more entry here.
    pub also: Vec<Change>,
    /// Effects the command authorizes, recorded in its transaction before any
    /// external action (CORE section 19.1). Their ids derive from the
    /// command's operation reference, so `make_result` can name them.
    pub effects: Vec<crate::effects::NewEffect>,
    /// The grant the command acted under, recorded in each effect's
    /// authorization.
    pub grant: Option<&'a str>,
    /// Further events on the primary subject, after `event`, at the same
    /// revision: one change a command makes can have more than one fact to
    /// record, as a purge confirmed at once records both its request and its
    /// confirmation.
    pub more_events: Vec<NewEvent>,
    /// Staged evidence bytes this command appends or discards, in the same
    /// transaction as the state change that accounts for them.
    pub chunks: Chunks<'a>,
    /// The provider clock's instant for this command. Supplied rather than
    /// read here, so `recorded_at` follows a controlled clock like every other
    /// protocol-visible time, and so one command records one instant.
    pub recorded_at: &'a str,
    /// A knowledge claim revision this command records, appended beside the
    /// claim subject it numbers.
    pub claim_revision: Option<ClaimRevision<'a>>,
}

/// A new claim revision: its number, digest and canonical record. The number
/// must be the claim subject's new revision, which the store checks inside the
/// transaction, so a revision row can never disagree with its subject.
pub struct ClaimRevision<'a> {
    pub claim: &'a str,
    pub revision: i64,
    pub digest: &'a str,
    pub record: &'a str,
}

/// A stored claim revision.
#[derive(Debug, Clone)]
pub struct ClaimRevisionRow {
    pub revision: i64,
    pub digest: String,
    pub record: Value,
    pub recorded_at: String,
}

/// Staged evidence bytes a command changes.
#[derive(Default)]
pub enum Chunks<'a> {
    #[default]
    None,
    Append {
        artifact: &'a str,
        offset: i64,
        bytes: &'a [u8],
    },
    Discard {
        artifact: &'a str,
    },
}

/// A further subject one command changes. A change need not be an event: a
/// context job gaining a subscriber changes the job's record, and what a
/// reader observes is the request's own event (CONTEXT section 11).
pub struct Change {
    pub key: SubjectKey,
    pub value: String,
    pub event: Option<NewEvent>,
}

/// One subject a provider-origin batch writes: the revision it had when the
/// batch was computed, and the revision and value it ends at. A subject that
/// changed in between refuses the whole batch, so a batch never overwrites a
/// change it did not see.
pub struct ProviderWrite {
    pub key: SubjectKey,
    pub base: i64,
    pub revision: i64,
    pub value: String,
}

/// One provider-origin event of a batch, at a revision the batch wrote.
pub struct ProviderEvent {
    pub key: SubjectKey,
    pub revision: i64,
    pub event_type: String,
    pub payload: Value,
}

pub struct Store {
    connection: Connection,
    objects: PathBuf,
}

impl Store {
    /// Open or create the store under `data_dir`, applying and then verifying
    /// the durability settings.
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(data_dir)?;
        let objects = data_dir.join("objects");
        fs::create_dir_all(&objects)?;
        let connection = Connection::open(data_dir.join("cbr.sqlite"))?;

        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;

        let store = Self {
            connection,
            objects,
        };
        store.verify_durability()?;
        store.migrate()?;
        Ok(store)
    }

    /// Read the durability settings back from the open connection.
    ///
    /// Setting a PRAGMA and assuming it took is how a store ends up running in
    /// a weaker mode than its documentation claims: `journal_mode` silently
    /// stays as it was if the database cannot change it, and `synchronous` is
    /// per-connection, so a connection opened elsewhere would not inherit it.
    fn verify_durability(&self) -> Result<(), StoreError> {
        let journal: String = self
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        if !journal.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Pragma {
                name: "journal_mode",
                expected: "wal".into(),
                found: journal,
            });
        }
        // 2 is FULL. In WAL mode EXTRA is identical to FULL, so there is
        // nothing above it to ask for.
        let synchronous: i64 = self
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        if synchronous != 2 {
            return Err(StoreError::Pragma {
                name: "synchronous",
                expected: "2 (FULL)".into(),
                found: synchronous.to_string(),
            });
        }
        let foreign_keys: i64 = self
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        if foreign_keys != 1 {
            return Err(StoreError::Pragma {
                name: "foreign_keys",
                expected: "1".into(),
                found: foreign_keys.to_string(),
            });
        }
        Ok(())
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS meta_text (
                 key  TEXT PRIMARY KEY,
                 text TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS subjects (
                 kind          TEXT    NOT NULL,
                 id            TEXT    NOT NULL,
                 revision      INTEGER NOT NULL,
                 value         TEXT    NOT NULL,
                 applied_count INTEGER NOT NULL,
                 PRIMARY KEY (kind, id)
             );
             CREATE TABLE IF NOT EXISTS events (
                 epoch         INTEGER NOT NULL,
                 sequence      INTEGER NOT NULL,
                 type          TEXT    NOT NULL,
                 subject_kind  TEXT    NOT NULL,
                 subject_id    TEXT    NOT NULL,
                 revision      INTEGER NOT NULL,
                 origin        TEXT    NOT NULL,
                 operation_ref TEXT,
                 command_id    TEXT,
                 caused_by     TEXT    NOT NULL,
                 recorded_at   TEXT    NOT NULL,
                 payload       TEXT    NOT NULL,
                 PRIMARY KEY (epoch, sequence)
             );
             -- One row per epoch the stream has had. `vouched_through` is the
             -- last sequence the provider still vouches for in a closed epoch;
             -- nothing after it is ever delivered again (CORE section 16.1).
             CREATE TABLE IF NOT EXISTS epochs (
                 epoch           INTEGER PRIMARY KEY,
                 vouched_through INTEGER NOT NULL,
                 closed          INTEGER NOT NULL
             );
             -- The capability snapshot (CORE section 17). One row: the
             -- current predicates and the revision that numbers them.
             CREATE TABLE IF NOT EXISTS capabilities (
                 id          INTEGER PRIMARY KEY CHECK (id = 1),
                 provider_id TEXT    NOT NULL,
                 revision    INTEGER NOT NULL,
                 predicates  TEXT    NOT NULL
             );
             -- Bytes of a staged evidence upload, committed with the append
             -- that brought them. Discarded when the upload seals or ends;
             -- sealed bytes live in the object store.
             CREATE TABLE IF NOT EXISTS evidence_chunks (
                 artifact TEXT    NOT NULL,
                 offset   INTEGER NOT NULL,
                 bytes    BLOB    NOT NULL,
                 PRIMARY KEY (artifact, offset)
             );
             -- Principal credentials for a shared transport (CORE section
             -- 18.1): the digest of each whole credential string, never the
             -- credential, with its principal and revocation state.
             CREATE TABLE IF NOT EXISTS credentials (
                 digest    TEXT    PRIMARY KEY,
                 principal TEXT    NOT NULL,
                 revoked   INTEGER NOT NULL
             );
             -- Knowledge claim revisions (KNOWLEDGE section 3). Insert-only: a
             -- revision's record never changes once proposed, and the two
             -- triggers make that a property of the database rather than of
             -- the code that happens to write it.
             CREATE TABLE IF NOT EXISTS knowledge_revisions (
                 claim       TEXT    NOT NULL,
                 revision    INTEGER NOT NULL,
                 digest      TEXT    NOT NULL,
                 record      TEXT    NOT NULL,
                 recorded_at TEXT    NOT NULL,
                 PRIMARY KEY (claim, revision)
             );
             CREATE TRIGGER IF NOT EXISTS knowledge_revisions_never_updated
                 BEFORE UPDATE ON knowledge_revisions
                 BEGIN SELECT RAISE(ABORT, 'claim revisions are immutable'); END;
             CREATE TRIGGER IF NOT EXISTS knowledge_revisions_never_deleted
                 BEFORE DELETE ON knowledge_revisions
                 BEGIN SELECT RAISE(ABORT, 'claim revisions are immutable'); END;
             CREATE TABLE IF NOT EXISTS commands (
                 principal  TEXT    NOT NULL,
                 command_id TEXT    NOT NULL,
                 digest     TEXT    NOT NULL,
                 generation INTEGER NOT NULL,
                 result     TEXT    NOT NULL,
                 PRIMARY KEY (principal, command_id)
             );
             -- Where a registered repository is checked out on this machine.
             -- Deliberately not a subject and never a fact: a path is local
             -- configuration, it identifies the operator's filesystem, and
             -- nothing that leaves this process needs it.
             CREATE TABLE IF NOT EXISTS repository_checkout (
                 repository TEXT PRIMARY KEY,
                 path       TEXT NOT NULL
             );",
        )?;
        // The derived memory lives in the same file, so it migrates with the
        // journal: an index that is not there is a projection that is
        // unavailable, and the compiler would have to say so on every read.
        cbr_memory::index::migrate(&self.connection).map_err(memory_failed)?;
        cbr_memory::retrieval::migrate(&self.connection).map_err(memory_failed)?;
        Ok(())
    }

    fn meta(&self, key: &str) -> Result<Option<i64>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The stream's stable identity, minted once when the store is created.
    ///
    /// Cursors carry it, so a well-formed cursor from another store is
    /// `invalid_cursor` rather than silently readable against this one.
    pub fn stream_id(&self) -> Result<String, StoreError> {
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT text FROM meta_text WHERE key = 'stream_id'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        // Minted from the creation instant and the process id. It need not be
        // unpredictable, only distinct between stores.
        let seed = format!(
            "{:?}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default(),
            std::process::id()
        );
        let id = cbr_encoding::sha256_hex(seed.as_bytes())[..16].to_string();
        self.connection.execute(
            "INSERT INTO meta_text (key, text) VALUES ('stream_id', ?1)",
            params![id],
        )?;
        Ok(id)
    }

    pub fn current_epoch(&self) -> Result<i64, StoreError> {
        Ok(self.meta("current_epoch")?.unwrap_or(1))
    }

    /// The last sequence recorded in an epoch, or 0 when it has none.
    pub fn last_sequence(&self, epoch: i64) -> Result<i64, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
                params![epoch],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Start a new stream epoch, closing the current one.
    ///
    /// A provider does this when it can no longer vouch for continuity.
    /// `unvouched_last` is how many trailing events of the closing epoch it can
    /// no longer vouch for; they stay in the table but are never delivered,
    /// because a consumer holding them must be told so explicitly rather than
    /// have them quietly reappear.
    pub fn start_new_epoch(&mut self, unvouched_last: i64) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let epoch: i64 = transaction
            .query_row(
                "SELECT value FROM meta WHERE key = 'current_epoch'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(1);
        let last: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
            params![epoch],
            |row| row.get(0),
        )?;
        let vouched = (last - unvouched_last).max(0);
        transaction.execute(
            "INSERT INTO epochs (epoch, vouched_through, closed) VALUES (?1, ?2, 1)
             ON CONFLICT(epoch) DO UPDATE SET vouched_through = ?2, closed = 1",
            params![epoch, vouched],
        )?;
        transaction.execute(
            "INSERT INTO meta (key, value) VALUES ('current_epoch', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![epoch + 1],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Discard all but the last `retain` events, across epochs.
    ///
    /// Retention is what makes a gap real: a reader whose position precedes the
    /// earliest retained event is told so with a snapshot, never handed the
    /// next surviving event as though nothing were missing.
    pub fn retain_last_events(&mut self, retain: i64) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // The highest position about to be discarded becomes the watermark. A
        // read starting at or before it gets a gap, which is what stops the
        // next surviving event from being served as though nothing were
        // missing.
        let doomed: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT epoch, sequence FROM events
                 WHERE (epoch, sequence) NOT IN (
                     SELECT epoch, sequence FROM events ORDER BY epoch DESC, sequence DESC LIMIT ?1
                 )
                 ORDER BY epoch DESC, sequence DESC LIMIT 1",
                params![retain],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((epoch, sequence)) = doomed {
            transaction.execute(
                "DELETE FROM events WHERE (epoch, sequence) NOT IN (
                     SELECT epoch, sequence FROM events ORDER BY epoch DESC, sequence DESC LIMIT ?1
                 )",
                params![retain],
            )?;
            for (key, value) in [("discarded_epoch", epoch), ("discarded_sequence", sequence)] {
                transaction.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = ?2",
                    params![key, value],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Advance the deduplication generation for a new process, and discard the
    /// records that fall out of the retained window.
    ///
    /// A provider "may raise `oldest_retained` only by discarding records of
    /// generations below the new value" (CORE section 6.3), so the discard is
    /// what entitles the window to move; leaving the rows and pretending they
    /// were gone would make `dedupe_history_unavailable` a lie.
    pub fn start_generation(
        &mut self,
        advance_on_start: i64,
        retain_generations: i64,
    ) -> Result<(i64, i64), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored: Option<i64> = transaction
            .query_row(
                "SELECT value FROM meta WHERE key = 'dedupe_current'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let current = stored.unwrap_or(1) + advance_on_start;
        transaction.execute(
            "INSERT INTO meta (key, value) VALUES ('dedupe_current', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![current],
        )?;
        let oldest = (current - retain_generations + 1).max(0);
        transaction.execute(
            "DELETE FROM commands WHERE generation < ?1",
            params![oldest],
        )?;
        transaction.commit()?;
        Ok((current, oldest))
    }

    /// The deduplication window as the process that started this store left
    /// it, for a further connection to the same store. Unlike
    /// `start_generation`, this advances and discards nothing: a connection is
    /// not a process start.
    pub fn current_generation(&self, retain_generations: i64) -> Result<(i64, i64), StoreError> {
        let current = self.meta("dedupe_current")?.unwrap_or(1);
        Ok((current, (current - retain_generations + 1).max(0)))
    }

    pub fn revision(&self, key: &SubjectKey) -> Result<i64, StoreError> {
        Ok(self.subject(key)?.map_or(0, |state| state.revision))
    }

    pub fn subject(&self, key: &SubjectKey) -> Result<Option<SubjectState>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT revision, value, applied_count FROM subjects WHERE kind = ?1 AND id = ?2",
                params![key.kind, key.id],
                |row| {
                    Ok(SubjectState {
                        revision: row.get(0)?,
                        value: row.get(1)?,
                        applied_count: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Bring the stored capability snapshot in line with what the provider
    /// observes at launch, and return its revision.
    ///
    /// A new store's first snapshot is revision 1 and appends nothing. After
    /// that, any difference raises the revision and appends one
    /// provider-origin `core.capabilities.changed` event, in the same
    /// transaction, so the snapshot and its history cannot disagree
    /// (CORE section 17.3). An identical snapshot changes nothing, which is
    /// what keeps the revision stable across a restart where nothing changed.
    ///
    /// The comparison is over canonical form, which is sound only because the
    /// predicates carry no `observed_at`; a predicate that gains one must be
    /// compared without it, or every restart would look like a change.
    pub fn reconcile_capabilities(
        &mut self,
        provider_id: &str,
        predicates: &Value,
        recorded_at: &str,
    ) -> Result<i64, StoreError> {
        let canonical = String::from_utf8(cbr_encoding::to_canonical(predicates))
            .expect("canonical form is UTF-8");
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored: Option<(i64, String)> = transaction
            .query_row(
                "SELECT revision, predicates FROM capabilities WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let revision = match stored {
            None => {
                transaction.execute(
                    "INSERT INTO capabilities (id, provider_id, revision, predicates)
                     VALUES (1, ?1, 1, ?2)",
                    params![provider_id, canonical],
                )?;
                1
            }
            Some((revision, previous)) if previous == canonical => revision,
            Some((revision, _)) => {
                let revision = revision + 1;
                transaction.execute(
                    "UPDATE capabilities SET provider_id = ?1, revision = ?2, predicates = ?3
                     WHERE id = 1",
                    params![provider_id, revision, canonical],
                )?;
                let epoch: i64 = transaction
                    .query_row(
                        "SELECT value FROM meta WHERE key = 'current_epoch'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(1);
                let last: i64 = transaction.query_row(
                    "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
                    params![epoch],
                    |row| row.get(0),
                )?;
                let payload = Value::Object(vec![("predicates".into(), predicates.clone())]);
                // Provider origin: no operation caused it, so no operation_ref
                // or command_id, and an empty caused_by (CORE section 16.2).
                transaction.execute(
                    "INSERT INTO events (epoch, sequence, type, subject_kind, subject_id, revision,
                                         origin, operation_ref, command_id, caused_by, recorded_at, payload)
                     VALUES (?1, ?2, 'core.capabilities.changed', 'core.capabilities', ?3, ?4,
                             'provider', NULL, NULL, '[]', ?5, ?6)",
                    params![
                        epoch,
                        last + 1,
                        provider_id,
                        revision,
                        recorded_at,
                        String::from_utf8(cbr_encoding::to_canonical(&payload))
                            .expect("canonical form is UTF-8")
                    ],
                )?;
                revision
            }
        };
        transaction.commit()?;
        Ok(revision)
    }

    /// A change the provider makes itself rather than a command — an effect
    /// observation, an attempt, an obligation falling overdue. The subject's
    /// revision rises; its applied count does not, because no command applied
    /// it. Its events are provider-origin and commit in the same transaction
    /// as the change.
    pub fn commit_provider_change(
        &mut self,
        key: &SubjectKey,
        value: &str,
        events: Vec<NewEvent>,
        recorded_at: &str,
        discard_chunks: bool,
    ) -> Result<i64, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision: i64 = transaction
            .query_row(
                "SELECT revision FROM subjects WHERE kind = ?1 AND id = ?2",
                params![key.kind, key.id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or(StoreError::Corrupt(
                "a provider change to a subject that does not exist",
            ))?
            + 1;
        transaction.execute(
            "UPDATE subjects SET revision = ?3, value = ?4 WHERE kind = ?1 AND id = ?2",
            params![key.kind, key.id, revision, value],
        )?;
        if discard_chunks {
            transaction.execute(
                "DELETE FROM evidence_chunks WHERE artifact = ?1",
                params![key.id],
            )?;
        }
        // Several events for one change share its revision: two obligations
        // falling overdue together are one change to the effect.
        for event in events {
            let epoch: i64 = transaction
                .query_row(
                    "SELECT value FROM meta WHERE key = 'current_epoch'",
                    [],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(1);
            let last: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
                params![epoch],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO events (epoch, sequence, type, subject_kind, subject_id, revision,
                                     origin, operation_ref, command_id, caused_by, recorded_at, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'provider', NULL, NULL, '[]', ?7, ?8)",
                params![
                    epoch,
                    last + 1,
                    event.event_type,
                    key.kind,
                    key.id,
                    revision,
                    recorded_at,
                    String::from_utf8(cbr_encoding::to_canonical(&(event.payload)(revision)))
                        .expect("canonical form is UTF-8")
                ],
            )?;
        }
        transaction.commit()?;
        Ok(revision)
    }

    /// Commit a provider-origin batch in one owner transaction: every write,
    /// then every event in the order given (CORE section 16.3). The batch is
    /// the provider's own work -- a context job advancing against the clock --
    /// so its events have no command, operation or cause.
    ///
    /// Each write is checked against the revision the batch was computed
    /// from, and each event must name a revision its subject's write passes
    /// through; either failing is corruption of the caller's bookkeeping and
    /// nothing is committed.
    pub fn commit_provider_batch(
        &mut self,
        writes: &[ProviderWrite],
        events: &[ProviderEvent],
        recorded_at: &str,
    ) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for write in writes {
            let stored: i64 = transaction
                .query_row(
                    "SELECT revision FROM subjects WHERE kind = ?1 AND id = ?2",
                    params![write.key.kind, write.key.id],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            if stored != write.base || write.revision <= write.base {
                return Err(StoreError::Corrupt(
                    "a provider batch computed from a revision that is no longer current",
                ));
            }
            transaction.execute(
                "INSERT INTO subjects (kind, id, revision, value, applied_count)
                 VALUES (?1, ?2, ?3, ?4, 0)
                 ON CONFLICT(kind, id) DO UPDATE SET revision = ?3, value = ?4",
                params![write.key.kind, write.key.id, write.revision, write.value],
            )?;
        }
        let epoch: i64 = transaction
            .query_row(
                "SELECT value FROM meta WHERE key = 'current_epoch'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(1);
        let mut sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
            params![epoch],
            |row| row.get(0),
        )?;
        for event in events {
            let written = writes.iter().any(|w| {
                w.key == event.key && event.revision > w.base && event.revision <= w.revision
            });
            if !written {
                return Err(StoreError::Corrupt(
                    "a provider event at a revision its batch did not write",
                ));
            }
            sequence += 1;
            transaction.execute(
                "INSERT INTO events (epoch, sequence, type, subject_kind, subject_id, revision,
                                     origin, operation_ref, command_id, caused_by, recorded_at, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'provider', NULL, NULL, '[]', ?7, ?8)",
                params![
                    epoch,
                    sequence,
                    event.event_type,
                    event.key.kind,
                    event.key.id,
                    event.revision,
                    recorded_at,
                    String::from_utf8(cbr_encoding::to_canonical(&event.payload))
                        .expect("canonical form is UTF-8")
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Every stored credential digest with its principal and revocation.
    /// Register a repository, or confirm one already registered: a
    /// `cbr.repository` subject holding the id, and a local row holding the
    /// checkout path. The subject never carries the path.
    ///
    /// Registering the same id at the same path again is not a change, so it
    /// produces no revision and no event; a different path is a change.
    pub fn register_repository(
        &mut self,
        id: &str,
        path: &Path,
        recorded_at: &str,
    ) -> Result<i64, StoreError> {
        let key = SubjectKey {
            kind: REPOSITORY.to_string(),
            id: id.to_string(),
        };
        let facts = Value::Object(vec![
            ("repository".into(), Value::String(id.to_string())),
            ("registered_at".into(), Value::String(recorded_at.into())),
        ]);
        let canonical =
            String::from_utf8(cbr_encoding::to_canonical(&facts)).expect("canonical form is UTF-8");
        let text = path.to_string_lossy().into_owned();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let known: Option<String> = transaction
            .query_row(
                "SELECT path FROM repository_checkout WHERE repository = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        transaction.execute(
            "INSERT INTO repository_checkout (repository, path) VALUES (?1, ?2)
             ON CONFLICT (repository) DO UPDATE SET path = excluded.path",
            params![id, text],
        )?;
        let existing: Option<i64> = transaction
            .query_row(
                "SELECT revision FROM subjects WHERE kind = ?1 AND id = ?2",
                params![key.kind, key.id],
                |row| row.get(0),
            )
            .optional()?;
        let revision = match existing {
            Some(revision) if known.as_deref() == Some(text.as_str()) => revision,
            Some(revision) => {
                transaction.execute(
                    "UPDATE subjects SET revision = ?3 WHERE kind = ?1 AND id = ?2",
                    params![key.kind, key.id, revision + 1],
                )?;
                revision + 1
            }
            None => {
                transaction.execute(
                    "INSERT INTO subjects (kind, id, revision, value, applied_count)
                     VALUES (?1, ?2, 1, ?3, 0)",
                    params![key.kind, key.id, canonical],
                )?;
                1
            }
        };
        transaction.commit()?;
        Ok(revision)
    }

    /// Where a registered repository is checked out, if it is registered on
    /// this machine.
    pub fn repository_checkout(&self, id: &str) -> Result<Option<PathBuf>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT path FROM repository_checkout WHERE repository = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(PathBuf::from))
    }

    /// The connection, for the derived memory that lives in the same file.
    ///
    /// Index rows are written outside the caller's command transaction on
    /// purpose: an index is derived and idempotent, so a build that survives
    /// a failed tick costs a rebuild at worst and never makes a record wrong.
    pub fn connection(&self) -> &rusqlite::Connection {
        &self.connection
    }

    pub fn credentials(&self) -> Result<Vec<crate::config::Credential>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT digest, principal, revoked FROM credentials ORDER BY digest")?;
        let rows = statement.query_map([], |row| {
            Ok(crate::config::Credential {
                digest: row.get(0)?,
                principal: row.get(1)?,
                revoked: row.get::<_, i64>(2)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Record a new credential's digest for a principal, revoking every earlier
    /// one for that principal in the same transaction: rotation leaves exactly
    /// one live credential, and never zero or two.
    pub fn rotate_credential(&mut self, principal: &str, digest: &str) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE credentials SET revoked = 1 WHERE principal = ?1",
            params![principal],
        )?;
        transaction.execute(
            "INSERT INTO credentials (digest, principal, revoked) VALUES (?1, ?2, 0)",
            params![digest, principal],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Revoke every credential of a principal. Future authentications fail;
    /// sessions already authenticated are not ended by this (CORE 18.3).
    pub fn revoke_credentials(&mut self, principal: &str) -> Result<usize, StoreError> {
        Ok(self.connection.execute(
            "UPDATE credentials SET revoked = 1 WHERE principal = ?1 AND revoked = 0",
            params![principal],
        )?)
    }

    /// Bind a command whose outcome changes nothing: no revision, no event,
    /// only the command record, so a retransmission replays it. An already
    /// sealed artifact resealed, or a purge repeated, is exactly that.
    pub fn bind_command_only(
        &mut self,
        principal: &str,
        command_id: &str,
        digest: &str,
        generation: i64,
        result: &Value,
    ) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO commands (principal, command_id, digest, generation, result)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                principal,
                command_id,
                digest,
                generation,
                String::from_utf8(cbr_encoding::to_canonical(result))
                    .expect("canonical form is UTF-8")
            ],
        )?;
        Ok(())
    }

    /// The staged bytes of an upload, in offset order.
    pub fn staged_bytes(&self, artifact: &str) -> Result<Vec<u8>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT bytes FROM evidence_chunks WHERE artifact = ?1 ORDER BY offset")?;
        let rows = statement.query_map(params![artifact], |row| row.get::<_, Vec<u8>>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.extend(row?);
        }
        Ok(out)
    }

    fn object_path(&self, digest: &str) -> Option<PathBuf> {
        let hex = digest.split_once(':').map(|(_, hex)| hex)?;
        if hex.len() < 5 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(
            self.objects
                .join("sha256")
                .join(&hex[0..2])
                .join(&hex[2..4])
                .join(&hex[4..]),
        )
    }

    /// The bytes published under a digest, as they are on disk now. `None` if
    /// there is no such object. The caller verifies them: an object that no
    /// longer matches its name is an integrity failure, and is never served.
    pub fn read_object(&self, digest: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(path) = self.object_path(digest) else {
            return Ok(None);
        };
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Physically delete a published object. Called only after the row that
    /// records the purge has committed — the reverse of publication's order —
    /// so a crash between them leaves an object no available row names, never
    /// an available row naming a missing object.
    pub fn delete_object(&self, digest: &str) -> Result<(), StoreError> {
        let Some(path) = self.object_path(digest) else {
            return Ok(());
        };
        match fs::remove_file(&path) {
            Ok(()) => {
                if let Some(parent) = path.parent() {
                    fs::File::open(parent)?.sync_all()?;
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Every published object's digest, and every staging file a publication
    /// interrupted by a crash left behind, for the start-time collection pass.
    /// Anything else under `objects/` is not this store's and is left alone.
    pub fn object_inventory(&self) -> Result<Inventory, StoreError> {
        let mut inventory = Inventory::default();
        let root = self.objects.join("sha256");
        let hex = |name: &str, length: usize| {
            name.len() == length && name.bytes().all(|b| b.is_ascii_hexdigit())
        };
        let Ok(first) = fs::read_dir(&root) else {
            return Ok(inventory);
        };
        for a in first {
            let a = a?;
            let a_name = a.file_name().to_string_lossy().into_owned();
            if !hex(&a_name, 2) || !a.file_type()?.is_dir() {
                continue;
            }
            for b in fs::read_dir(a.path())? {
                let b = b?;
                let b_name = b.file_name().to_string_lossy().into_owned();
                if !hex(&b_name, 2) || !b.file_type()?.is_dir() {
                    continue;
                }
                for object in fs::read_dir(b.path())? {
                    let object = object?;
                    let name = object.file_name().to_string_lossy().into_owned();
                    if !object.file_type()?.is_file() {
                        continue;
                    }
                    if name.starts_with(".staging-") {
                        inventory.staging.push(object.path());
                    } else if hex(&name, 60) {
                        inventory
                            .objects
                            .push(format!("sha256:{a_name}{b_name}{name}"));
                    }
                }
            }
        }
        inventory.objects.sort();
        inventory.staging.sort();
        Ok(inventory)
    }

    /// Remove a staging file left by an interrupted publication.
    pub fn discard_staging(&self, path: &Path) -> Result<(), StoreError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Damage a published object in place. Test control only (the
    /// `evidence.store` control's `corrupt`), so the integrity check that
    /// refuses to serve it is exercised against real bytes on disk.
    pub fn corrupt_object(&self, digest: &str) -> Result<(), StoreError> {
        let Some(path) = self.object_path(digest) else {
            return Ok(());
        };
        let mut bytes = fs::read(&path)?;
        if let Some(first) = bytes.first_mut() {
            *first ^= 0xff;
        } else {
            bytes.push(0);
        }
        let mut permissions = fs::metadata(&path)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions)?;
        fs::write(&path, bytes)?;
        Ok(())
    }

    /// The current capability snapshot: its revision and predicates, or `None`
    /// before the first reconcile.
    pub fn capabilities(&self) -> Result<Option<(i64, Value)>, StoreError> {
        let row: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT revision, predicates FROM capabilities WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(revision, predicates)| {
            (
                revision,
                cbr_encoding::parse(predicates.as_bytes()).unwrap_or(Value::Array(vec![])),
            )
        }))
    }

    /// Every subject of one kind, ordered by id, with its stored value.
    ///
    /// The store deliberately knows nothing about what a value means, so a
    /// relationship between subjects — a grant's parent — is walked by the
    /// caller over this rather than indexed here. A scan is proportionate to
    /// v0.1's grant counts; an index belongs here only when a measurement says
    /// so.
    /// One claim revision, if this provider holds it.
    pub fn claim_revision(
        &self,
        claim: &str,
        revision: i64,
    ) -> Result<Option<ClaimRevisionRow>, StoreError> {
        let row: Option<(i64, String, String, String)> = self
            .connection
            .query_row(
                "SELECT revision, digest, record, recorded_at FROM knowledge_revisions
                 WHERE claim = ?1 AND revision = ?2",
                params![claim, revision],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        row.map(claim_revision_row).transpose()
    }

    /// Every revision of a claim lineage, oldest first.
    pub fn claim_revisions(&self, claim: &str) -> Result<Vec<ClaimRevisionRow>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT revision, digest, record, recorded_at FROM knowledge_revisions
             WHERE claim = ?1 ORDER BY revision",
        )?;
        let rows = statement.query_map(params![claim], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(claim_revision_row(row?)?);
        }
        Ok(out)
    }

    /// Subjects of one kind in the order they were first recorded, with their
    /// revisions and values. A subject row keeps its SQLite row id when it is
    /// updated, so row id order is creation order.
    pub fn subjects_in_recorded_order(
        &self,
        kind: &str,
    ) -> Result<Vec<(String, i64, String)>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, revision, value FROM subjects WHERE kind = ?1 ORDER BY rowid")?;
        let rows = statement.query_map(params![kind], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The position of the event of this type that recorded this revision of
    /// this subject, or `None` when that event is no longer retained.
    pub fn event_position(
        &self,
        event_type: &str,
        key: &SubjectKey,
        revision: i64,
    ) -> Result<Option<Position>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT epoch, sequence FROM events
                 WHERE type = ?1 AND subject_kind = ?2 AND subject_id = ?3 AND revision = ?4
                 ORDER BY epoch, sequence LIMIT 1",
                params![event_type, key.kind, key.id, revision],
                |row| {
                    Ok(Position {
                        epoch: row.get(0)?,
                        sequence: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn subjects_of_kind(&self, kind: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, value FROM subjects WHERE kind = ?1 ORDER BY id")?;
        let rows = statement.query_map(params![kind], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn applied_count(&self, key: &SubjectKey) -> Result<i64, StoreError> {
        Ok(self.subject(key)?.map_or(0, |state| state.applied_count))
    }

    /// The authority subject for a scope. Its revision **is** the scope's
    /// current epoch: each claim raises both together, so they cannot diverge
    /// and a precondition on the subject is a precondition on the epoch.
    pub fn authority_key(scope: &str) -> SubjectKey {
        SubjectKey {
            kind: "core-test.authority".to_string(),
            id: scope.to_string(),
        }
    }

    pub fn epoch(&self, scope: &str) -> Result<i64, StoreError> {
        self.revision(&Self::authority_key(scope))
    }

    pub fn command(
        &self,
        principal: &str,
        command_id: &str,
    ) -> Result<Option<CommandRecord>, StoreError> {
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT digest, result FROM commands WHERE principal = ?1 AND command_id = ?2",
                params![principal, command_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(digest, result)| CommandRecord {
            digest,
            result: cbr_encoding::parse(result.as_bytes())
                .expect("a stored result was written as canonical bytes"),
        }))
    }

    /// Apply a command and bind its identity in **one** transaction.
    ///
    /// `make_result` is handed the subject's new revision so the acknowledgment
    /// it builds is the one that is stored: a result computed outside the
    /// transaction could name a revision the transaction did not produce.
    pub fn commit_command(
        &mut self,
        commit: Commit<'_>,
        make_result: impl FnOnce(i64, &str) -> Value,
    ) -> Result<Value, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT revision, applied_count FROM subjects WHERE kind = ?1 AND id = ?2",
                params![commit.key.kind, commit.key.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (revision, applied) = existing.map_or((1, 1), |(r, a)| (r + 1, a + 1));
        transaction.execute(
            "INSERT INTO subjects (kind, id, revision, value, applied_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(kind, id) DO UPDATE SET revision = ?3, value = ?4, applied_count = ?5",
            params![
                commit.key.kind,
                commit.key.id,
                revision,
                commit.value,
                applied
            ],
        )?;
        // The operation reference is minted from a durable counter, so it is
        // unique across restarts and an event can name the operation that
        // produced it without two processes colliding.
        let operation_number: i64 = transaction
            .query_row(
                "SELECT value FROM meta WHERE key = 'operation_counter'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0)
            + 1;
        transaction.execute(
            "INSERT INTO meta (key, value) VALUES ('operation_counter', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![operation_number],
        )?;
        let operation_ref = format!("op-{operation_number}");

        // The events commit here, in the same transaction as the state changes
        // and the command record. A crash between them is not representable,
        // and a command that changes several subjects cannot leave some
        // changed and others not.
        let append = |transaction: &rusqlite::Transaction<'_>,
                      key: &SubjectKey,
                      revision: i64,
                      event: NewEvent|
         -> Result<(), StoreError> {
            let epoch: i64 = transaction
                .query_row(
                    "SELECT value FROM meta WHERE key = 'current_epoch'",
                    [],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(1);
            // Read inside the transaction, so each append sees the previous
            // one and a multi-event command stays contiguous.
            let last: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch = ?1",
                params![epoch],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO events (epoch, sequence, type, subject_kind, subject_id, revision,
                                     origin, operation_ref, command_id, caused_by, recorded_at, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'command', ?7, ?8, ?9, ?10, ?11)",
                params![
                    epoch,
                    last + 1,
                    event.event_type,
                    key.kind,
                    key.id,
                    revision,
                    operation_ref,
                    commit.command_id,
                    String::from_utf8(cbr_encoding::to_canonical(&Value::Array(
                        event.caused_by.iter().map(|c| Value::String(c.clone())).collect(),
                    )))
                    .expect("canonical form is UTF-8"),
                    commit.recorded_at,
                    String::from_utf8(cbr_encoding::to_canonical(&(event.payload)(revision)))
                        .expect("canonical form is UTF-8")
                ],
            )?;
            Ok(())
        };
        if let Some(event) = commit.event {
            append(&transaction, commit.key, revision, event)?;
        }
        for event in commit.more_events {
            append(&transaction, commit.key, revision, event)?;
        }
        if let Some(row) = &commit.claim_revision {
            if row.revision != revision || row.claim != commit.key.id {
                return Err(StoreError::Corrupt(
                    "a claim revision out of step with its claim subject",
                ));
            }
            transaction.execute(
                "INSERT INTO knowledge_revisions (claim, revision, digest, record, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.claim,
                    row.revision,
                    row.digest,
                    row.record,
                    commit.recorded_at
                ],
            )?;
        }
        match commit.chunks {
            Chunks::None => {}
            Chunks::Append {
                artifact,
                offset,
                bytes,
            } => {
                transaction.execute(
                    "INSERT INTO evidence_chunks (artifact, offset, bytes) VALUES (?1, ?2, ?3)",
                    params![artifact, offset, bytes],
                )?;
            }
            Chunks::Discard { artifact } => {
                transaction.execute(
                    "DELETE FROM evidence_chunks WHERE artifact = ?1",
                    params![artifact],
                )?;
            }
        }
        for change in commit.also {
            let existing: Option<(i64, i64)> = transaction
                .query_row(
                    "SELECT revision, applied_count FROM subjects WHERE kind = ?1 AND id = ?2",
                    params![change.key.kind, change.key.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (changed_revision, changed_applied) =
                existing.map_or((1, 1), |(r, a)| (r + 1, a + 1));
            transaction.execute(
                "INSERT INTO subjects (kind, id, revision, value, applied_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(kind, id) DO UPDATE SET revision = ?3, value = ?4, applied_count = ?5",
                params![
                    change.key.kind,
                    change.key.id,
                    changed_revision,
                    change.value,
                    changed_applied
                ],
            )?;
            if let Some(event) = change.event {
                append(&transaction, &change.key, changed_revision, event)?;
            }
        }

        for (index, effect) in commit.effects.iter().enumerate() {
            let id = crate::effects::effect_id(&operation_ref, index);
            let record = crate::effects::new_record(
                effect,
                &id,
                &operation_ref,
                commit.principal,
                commit.grant,
                commit.recorded_at,
            );
            transaction.execute(
                "INSERT INTO subjects (kind, id, revision, value, applied_count)
                 VALUES (?1, ?2, 1, ?3, 1)",
                params![
                    crate::effects::KIND,
                    id,
                    String::from_utf8(cbr_encoding::to_canonical(&record))
                        .expect("canonical form is UTF-8")
                ],
            )?;
        }

        let result = make_result(revision, &operation_ref);
        transaction.execute(
            "INSERT INTO commands (principal, command_id, digest, generation, result)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                commit.principal,
                commit.command_id,
                commit.digest,
                commit.generation,
                String::from_utf8(cbr_encoding::to_canonical(&result))
                    .expect("canonical form is UTF-8")
            ],
        )?;
        transaction.commit()?;
        Ok(result)
    }

    /// Record how far retention has discarded, so a later read can tell a
    /// reader that events are missing instead of handing them the next
    /// surviving event as though nothing were gone.
    fn discarded_watermark(&self) -> Result<Option<Position>, StoreError> {
        let epoch = self.meta("discarded_epoch")?;
        let sequence = self.meta("discarded_sequence")?;
        Ok(match (epoch, sequence) {
            (Some(epoch), Some(sequence)) => Some(Position { epoch, sequence }),
            _ => None,
        })
    }

    /// The head of the stream: the last position recorded anywhere.
    fn head(&self) -> Result<Position, StoreError> {
        let epoch = self.current_epoch()?;
        Ok(Position {
            epoch,
            sequence: self.last_sequence(epoch)?,
        })
    }

    fn epoch_vouched_through(&self, epoch: i64) -> Result<Option<i64>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT vouched_through FROM epochs WHERE epoch = ?1 AND closed = 1",
                params![epoch],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn events_in(
        &self,
        epoch: i64,
        from_sequence: i64,
        through: Option<i64>,
    ) -> Result<Vec<EventRecord>, StoreError> {
        let ceiling = through.unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT epoch, sequence, type, subject_kind, subject_id, revision, origin,
                    operation_ref, command_id, caused_by, recorded_at, payload
             FROM events WHERE epoch = ?1 AND sequence >= ?2 AND sequence <= ?3
             ORDER BY sequence",
        )?;
        let rows = statement.query_map(params![epoch, from_sequence, ceiling], |row| {
            let caused_by: String = row.get(9)?;
            let payload: String = row.get(11)?;
            Ok(EventRecord {
                position: Position {
                    epoch: row.get(0)?,
                    sequence: row.get(1)?,
                },
                event_type: row.get(2)?,
                subject: SubjectKey {
                    kind: row.get(3)?,
                    id: row.get(4)?,
                },
                revision: row.get(5)?,
                origin: row.get(6)?,
                operation_ref: row.get(7)?,
                command_id: row.get(8)?,
                caused_by: cbr_encoding::parse(caused_by.as_bytes())
                    .expect("stored caused_by was written as canonical bytes"),
                recorded_at: row.get(10)?,
                payload: cbr_encoding::parse(payload.as_bytes())
                    .expect("a stored payload was written as canonical bytes"),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Every visible subject and its current state, for a retention snapshot.
    /// The visible subjects for a retention snapshot, and whether any were
    /// hidden.
    fn snapshot_subjects(
        &self,
        visible: &dyn Fn(&SubjectKey) -> bool,
    ) -> Result<(Vec<Value>, bool), StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, id, revision, value FROM subjects ORDER BY kind, id")?;
        let rows = statement.query_map([], |row| {
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            let revision: i64 = row.get(2)?;
            let value: String = row.get(3)?;
            Ok((kind, id, revision, value))
        })?;
        let mut out = Vec::new();
        let mut hidden = false;
        for row in rows {
            let (kind, id, revision, value) = row?;
            let key = SubjectKey {
                kind: kind.clone(),
                id: id.clone(),
            };
            if !visible(&key) {
                hidden = true;
                continue;
            }
            // Each subject kind declares its own snapshot state (CORE section
            // 16.4). The authority subject's state is its epoch, which is its
            // revision, so the two cannot disagree here either.
            let state = match kind.as_str() {
                "core-test.authority" => {
                    Value::Object(vec![("epoch".into(), Value::Int(revision))])
                }
                "core.grant" => Value::Object(vec![(
                    "grant".into(),
                    cbr_encoding::parse(value.as_bytes()).unwrap_or(Value::Null),
                )]),
                _ => Value::Object(vec![("value".into(), Value::String(value))]),
            };
            out.push(Value::Object(vec![
                (
                    "subject".into(),
                    Value::Object(vec![
                        ("kind".into(), Value::String(kind)),
                        ("id".into(), Value::String(id)),
                    ]),
                ),
                ("revision".into(), Value::Int(revision)),
                ("state".into(), state),
            ]));
        }

        // The capability snapshot is not a row in `subjects`, but once any
        // change event has been recorded it is a subject "changed by an event
        // at or before as_of", which a snapshot MUST list (CORE section 16.4).
        // Revision 1 is the initial snapshot, which appended no event, so it
        // is left out rather than listed on the section's permission alone.
        let capabilities: Option<(String, i64, String)> = self
            .connection
            .query_row(
                "SELECT provider_id, revision, predicates FROM capabilities WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((provider_id, revision, predicates)) = capabilities
            && revision > 1
        {
            let key = SubjectKey {
                kind: "core.capabilities".into(),
                id: provider_id.clone(),
            };
            if visible(&key) {
                out.push(Value::Object(vec![
                    (
                        "subject".into(),
                        Value::Object(vec![
                            ("kind".into(), Value::String(key.kind)),
                            ("id".into(), Value::String(provider_id)),
                        ]),
                    ),
                    ("revision".into(), Value::Int(revision)),
                    (
                        "state".into(),
                        Value::Object(vec![(
                            "predicates".into(),
                            cbr_encoding::parse(predicates.as_bytes()).unwrap_or(Value::Null),
                        )]),
                    ),
                ]));
            } else {
                hidden = true;
            }
        }
        Ok((out, hidden))
    }

    /// The earliest position a reader may ask for.
    pub fn stream_start(&self) -> Result<Position, StoreError> {
        let earliest: Option<i64> = self
            .connection
            .query_row("SELECT MIN(epoch) FROM epochs", [], |row| row.get(0))
            .optional()?
            .flatten();
        Ok(Position {
            epoch: earliest.unwrap_or(1).min(self.current_epoch()?),
            sequence: 1,
        })
    }

    /// Read the stream from `start`, in order, without silent gaps.
    ///
    /// Returns the items, the position to resume from, and whether anything in
    /// the range covered was hidden. `budget` is the caller's receive limit:
    /// items are added while they fit, and if not even the first one fits the
    /// caller is told so rather than handed a truncated view.
    pub fn read_events(
        &self,
        start: Position,
        limit: i64,
        kinds: &[String],
        budget: usize,
        visible: &dyn Fn(&SubjectKey) -> bool,
    ) -> Result<ReadEvents, StoreError> {
        let stream = self.stream_id()?;
        let current = self.current_epoch()?;
        let head = self.head()?;
        let mut items: Vec<Value> = Vec::new();
        let mut filtered = false;
        let mut at = start;
        let mut used: usize = 0;
        let mut too_large_alone = false;

        let watermark = self.discarded_watermark()?;

        'outer: while at.epoch <= current && (items.len() as i64) < limit {
            let vouched = self.epoch_vouched_through(at.epoch)?;

            // A closed epoch whose vouched end the reader is already past:
            // announce the change before anything else. A consumer holding
            // events beyond it must be told they will never be delivered
            // again, rather than have them quietly disappear into a gap.
            if let Some(vouched_through) = vouched
                && at.sequence > vouched_through
                && at.epoch < current
            {
                let item = epoch_change_item(at.epoch, vouched_through);
                let size = cbr_encoding::to_canonical(&item).len();
                if used + size > budget {
                    break 'outer;
                }
                used += size;
                items.push(item);
                at = Position {
                    epoch: at.epoch + 1,
                    sequence: 1,
                };
                continue;
            }

            // Events missing from this epoch become a typed gap with a
            // snapshot. A gap may span epochs, and epoch changes inside it are
            // not reported separately because the snapshot supersedes them
            // (CORE section 16.4).
            let earliest: Option<i64> = self
                .connection
                .query_row(
                    "SELECT MIN(sequence) FROM events WHERE epoch = ?1",
                    params![at.epoch],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            let missing_here = earliest.is_none_or(|first| at.sequence < first);
            if let Some(discarded) = watermark
                && missing_here
                && (at.epoch, at.sequence) <= (discarded.epoch, discarded.sequence)
            {
                let (subjects, hidden) = self.snapshot_subjects(visible)?;
                // A snapshot subject hidden by authorization makes the result
                // filtered exactly as a hidden event does (CORE section 16.4).
                filtered |= hidden;
                let item = Value::Object(vec![(
                    "gap".into(),
                    Value::Object(vec![
                        ("kind".into(), Value::String("retention".into())),
                        ("from".into(), at.to_value()),
                        ("to".into(), head.to_value()),
                        (
                            "snapshot".into(),
                            Value::Object(vec![
                                ("as_of".into(), head.to_value()),
                                ("subjects".into(), Value::Array(subjects)),
                            ]),
                        ),
                    ]),
                )]);
                let size = cbr_encoding::to_canonical(&item).len();
                if used + size > budget {
                    break 'outer;
                }
                used += size;
                items.push(item);
                at = Position {
                    epoch: head.epoch,
                    sequence: head.sequence + 1,
                };
                continue;
            }

            for event in self.events_in(at.epoch, at.sequence, vouched)? {
                if (items.len() as i64) >= limit {
                    break 'outer;
                }
                at = Position {
                    epoch: event.position.epoch,
                    sequence: event.position.sequence + 1,
                };
                // CORE section 16.6: an event is included only if the
                // reader could read its subject directly, so `core.events.read`
                // alone reveals nothing a principal could not already read.
                // `kinds` is the caller's own narrowing; both count as
                // filtering, and neither leaves a gap.
                if !visible(&event.subject)
                    || (!kinds.is_empty() && !kinds.contains(&event.subject.kind))
                {
                    filtered = true;
                    continue;
                }
                let item = Value::Object(vec![("event".into(), event_to_value(&stream, &event))]);
                let size = cbr_encoding::to_canonical(&item).len();
                if used + size > budget {
                    if items.is_empty() {
                        too_large_alone = true;
                    }
                    // Stop before the item, and leave `at` pointing at it so a
                    // resume delivers it rather than skipping it.
                    at = event.position;
                    break 'outer;
                }
                used += size;
                items.push(item);
            }
            match vouched {
                // A closed epoch: announce the change and continue in the next.
                Some(vouched_through) if at.epoch < current => {
                    if (items.len() as i64) >= limit {
                        break 'outer;
                    }
                    let item = epoch_change_item(at.epoch, vouched_through);
                    let size = cbr_encoding::to_canonical(&item).len();
                    if used + size > budget {
                        break 'outer;
                    }
                    used += size;
                    items.push(item);
                    at = Position {
                        epoch: at.epoch + 1,
                        sequence: 1,
                    };
                }
                _ => break 'outer,
            }
        }

        Ok(ReadEvents {
            stream_epoch: current,
            items,
            next_cursor: at,
            filtered,
            first_item_too_large: too_large_alone,
        })
    }

    /// Publish immutable bytes, then return the path they landed at.
    ///
    /// **Deliberately ahead of its caller.** No `core` fixture carries a
    /// payload, so nothing in this stage calls it. It lands here anyway
    /// because durability is the thing that must not be retrofitted: adding
    /// the object store alongside Evidence would mean designing the crash
    /// ordering while also designing the profile, and the ordering below is
    /// the part that is easy to get subtly wrong. It is exercised by its own
    /// test rather than by a fixture, and that is stated rather than implied.
    #[allow(dead_code)]
    ///
    /// The ordering is the whole point and every step is load-bearing: stage in
    /// the destination directory so the rename cannot cross a filesystem;
    /// verify the digest from what was actually written; `sync_all` the file;
    /// rename; then **fsync the parent directory**, without which the directory
    /// entry may not survive a crash even though `rename` itself is atomic.
    /// Only after this returns may a caller commit a row naming the object.
    pub fn publish_object(&self, digest: &str, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        // A cheap pre-check on the caller's own bytes. It is not the
        // verification that matters -- that one reads back from disk below --
        // but it avoids writing bytes that were already wrong.
        let computed = cbr_encoding::digest_bytes(bytes);
        if computed != digest {
            return Err(StoreError::DigestMismatch {
                expected: digest.to_string(),
                computed,
            });
        }
        let hex = digest.split_once(':').map_or(digest, |(_, hex)| hex);
        let directory = self
            .objects
            .join("sha256")
            .join(&hex[0..2])
            .join(&hex[2..4]);
        fs::create_dir_all(&directory)?;
        let final_path = directory.join(&hex[4..]);
        if final_path.exists() {
            // Content addressing means equal bytes may already be published —
            // by another artifact, or by a seal that crashed before its row
            // committed. Reuse it only if it still verifies from disk; an
            // object that no longer matches its name is replaced, not trusted.
            if fs::read(&final_path)
                .is_ok_and(|existing| cbr_encoding::digest_bytes(&existing) == digest)
            {
                return Ok(final_path);
            }
            let mut permissions = fs::metadata(&final_path)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(&final_path, permissions)?;
            fs::remove_file(&final_path)?;
        }

        let staged = directory.join(format!(".staging-{hex}"));
        {
            let mut file = fs::File::create(&staged)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }

        // STACK section 3 step 2: verify the digest **from what was actually
        // written**, after the sync, not from the buffer that was handed in.
        // Checking the input argument proves only that the caller computed its
        // own digest correctly; it says nothing about what reached the disk, so
        // a short write or a corrupting filesystem would still publish and the
        // row naming it would be a lie.
        let written = fs::read(&staged)?;
        let on_disk = cbr_encoding::digest_bytes(&written);
        if on_disk != digest {
            let _ = fs::remove_file(&staged);
            return Err(StoreError::DigestMismatch {
                expected: digest.to_string(),
                computed: on_disk,
            });
        }

        fs::rename(&staged, &final_path)?;
        fs::File::open(&directory)?.sync_all()?;

        // Published objects are read-only, so immutability is enforced rather
        // than promised.
        let mut permissions = fs::metadata(&final_path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&final_path, permissions)?;
        Ok(final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("temp dir");
        let store = Store::open(directory.path()).expect("store opens");
        (directory, store)
    }

    #[test]
    fn a_provider_batch_commits_whole_or_not_at_all() {
        let (_directory, mut store) = store();
        let job = SubjectKey {
            kind: "context.job".into(),
            id: "j".into(),
        };
        let request = SubjectKey {
            kind: "context.request".into(),
            id: "r".into(),
        };
        let write = |key: &SubjectKey, base: i64, revision: i64, value: &str| ProviderWrite {
            key: key.clone(),
            base,
            revision,
            value: value.into(),
        };
        let event = |key: &SubjectKey, revision: i64| ProviderEvent {
            key: key.clone(),
            revision,
            event_type: "context.request.changed".into(),
            payload: Value::Object(vec![]),
        };
        let last = |store: &Store| {
            let epoch = store.current_epoch().expect("epoch");
            store.last_sequence(epoch).expect("sequence")
        };
        store
            .commit_provider_batch(
                &[write(&job, 0, 1, "{}"), write(&request, 0, 2, "{}")],
                &[event(&request, 1), event(&request, 2)],
                "2030-01-01T00:00:00Z",
            )
            .expect("a batch from the current revisions commits");
        assert_eq!(store.revision(&request).expect("revision"), 2);
        assert_eq!(last(&store), 2);

        // Computed from a revision that is no longer current: nothing lands,
        // not even the write that was still current.
        let stale = store.commit_provider_batch(
            &[
                write(&job, 1, 2, "{\"changed\":1}"),
                write(&request, 1, 3, "{}"),
            ],
            &[],
            "2030-01-01T00:00:01Z",
        );
        assert!(matches!(stale, Err(StoreError::Corrupt(_))), "{stale:?}");
        assert_eq!(store.revision(&job).expect("revision"), 1);
        assert_eq!(store.subject(&job).expect("read").expect("job").value, "{}");

        // An event at a revision the batch did not write is refused whole.
        let unwritten = store.commit_provider_batch(
            &[write(&job, 1, 2, "{}")],
            &[event(&request, 3)],
            "2030-01-01T00:00:02Z",
        );
        assert!(
            matches!(unwritten, Err(StoreError::Corrupt(_))),
            "{unwritten:?}"
        );
        assert_eq!(store.revision(&job).expect("revision"), 1);
        assert_eq!(last(&store), 2, "no event was appended");
    }

    #[test]
    fn durability_settings_read_back_from_the_open_connection() {
        let (_directory, store) = store();
        // Not "we called pragma_update": what the connection reports now.
        let journal: String = store
            .connection
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        let synchronous: i64 = store
            .connection
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();
        let foreign_keys: i64 = store
            .connection
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_ascii_lowercase(), "wal");
        assert_eq!(
            synchronous, 2,
            "2 is FULL; NORMAL loses durability on power loss"
        );
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn a_claim_revision_can_be_neither_updated_nor_deleted() {
        let (_directory, store) = store();
        store
            .connection
            .execute(
                "INSERT INTO knowledge_revisions (claim, revision, digest, record, recorded_at)
                 VALUES ('c', 1, 'sha256:00', '{}', '2030-01-01T00:00:00Z')",
                [],
            )
            .expect("a revision is inserted");
        let updated = store.connection.execute(
            "UPDATE knowledge_revisions SET record = '{\"edited\":true}' WHERE claim = 'c'",
            [],
        );
        assert!(updated.is_err(), "an update is refused by the database");
        let deleted = store
            .connection
            .execute("DELETE FROM knowledge_revisions WHERE claim = 'c'", []);
        assert!(deleted.is_err(), "a delete is refused by the database");
        let row = store
            .claim_revision("c", 1)
            .expect("reads")
            .expect("still there");
        assert_eq!(row.record, Value::Object(vec![]), "and unchanged");
    }

    #[test]
    fn state_and_command_records_survive_reopening() {
        let directory = tempfile::tempdir().expect("temp dir");
        let key = SubjectKey {
            kind: "core-test.subject".into(),
            id: "s-1".into(),
        };
        {
            let mut store = Store::open(directory.path()).unwrap();
            store.start_generation(0, 1).unwrap();
            store
                .commit_command(
                    Commit {
                        key: &key,
                        value: "v",
                        principal: "alice",
                        command_id: "cmd-1",
                        digest: "sha256:aa",
                        generation: 1,
                        event: None,
                        also: Vec::new(),
                        effects: Vec::new(),
                        grant: None,
                        more_events: Vec::new(),
                        chunks: Chunks::None,
                        recorded_at: "2030-01-01T00:00:00Z",
                        claim_revision: None,
                    },
                    |revision, _| Value::Object(vec![("revision".into(), Value::Int(revision))]),
                )
                .unwrap();
        }
        let store = Store::open(directory.path()).unwrap();
        assert_eq!(store.revision(&key).unwrap(), 1);
        assert_eq!(store.applied_count(&key).unwrap(), 1);
        assert!(store.command("alice", "cmd-1").unwrap().is_some());
        // Deduplication scope is the principal, so another principal's use of
        // the same command id is a different command.
        assert!(store.command("bob", "cmd-1").unwrap().is_none());
    }

    #[test]
    fn a_retention_snapshot_lists_the_capability_subject_once_it_has_changed() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut store = Store::open(directory.path()).unwrap();
        let everything = |_: &SubjectKey| true;
        let predicates = |status: &str| {
            Value::Array(vec![Value::Object(vec![
                ("name".into(), Value::String("core-test.writes".into())),
                ("status".into(), Value::String(status.into())),
            ])])
        };
        let listed = |store: &Store| {
            store
                .snapshot_subjects(&everything)
                .unwrap()
                .0
                .iter()
                .any(|entry| {
                    entry.get("subject").and_then(|s| s.get("kind"))
                        == Some(&Value::String("core.capabilities".into()))
                })
        };

        // The initial snapshot appended no event, so nothing obliges listing it.
        assert_eq!(
            store
                .reconcile_capabilities("p", &predicates("supported"), "2030-01-01T00:00:00Z")
                .unwrap(),
            1
        );
        assert!(!listed(&store));

        // A change appends an event, and from then on the subject that event
        // changed must appear in any snapshot covering it, even after
        // retention has discarded the event itself.
        assert_eq!(
            store
                .reconcile_capabilities("p", &predicates("unknown"), "2030-01-01T00:00:00Z")
                .unwrap(),
            2
        );
        assert!(listed(&store));

        // And an unchanged snapshot is not a change.
        assert_eq!(
            store
                .reconcile_capabilities("p", &predicates("unknown"), "2030-01-01T00:00:00Z")
                .unwrap(),
            2
        );
    }

    #[test]
    fn advancing_the_generation_discards_records_below_the_window() {
        let directory = tempfile::tempdir().expect("temp dir");
        let key = SubjectKey {
            kind: "core-test.subject".into(),
            id: "s-1".into(),
        };
        {
            let mut store = Store::open(directory.path()).unwrap();
            let (current, _) = store.start_generation(0, 1).unwrap();
            assert_eq!(current, 1);
            store
                .commit_command(
                    Commit {
                        key: &key,
                        value: "v",
                        principal: "alice",
                        command_id: "cmd-1",
                        digest: "sha256:aa",
                        generation: current,
                        event: None,
                        also: Vec::new(),
                        effects: Vec::new(),
                        grant: None,
                        more_events: Vec::new(),
                        chunks: Chunks::None,
                        recorded_at: "2030-01-01T00:00:00Z",
                        claim_revision: None,
                    },
                    |_, _| Value::Object(vec![]),
                )
                .unwrap();
        }
        {
            // Retained: the window still covers generation 1.
            let mut store = Store::open(directory.path()).unwrap();
            let (current, oldest) = store.start_generation(1, 5).unwrap();
            assert_eq!((current, oldest), (2, 0));
            assert!(store.command("alice", "cmd-1").unwrap().is_some());
        }
        {
            // Forgotten: the window has moved past generation 1, and the record
            // is actually gone rather than merely reported gone.
            let mut store = Store::open(directory.path()).unwrap();
            let (current, oldest) = store.start_generation(2, 1).unwrap();
            assert_eq!((current, oldest), (4, 4));
            assert!(store.command("alice", "cmd-1").unwrap().is_none());
        }
    }

    #[test]
    fn publishing_an_object_verifies_its_digest_and_makes_it_read_only() {
        let (_directory, store) = store();
        let bytes = b"some evidence";
        let digest = cbr_encoding::digest_bytes(bytes);
        let path = store.publish_object(&digest, bytes).expect("publishes");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());

        // Publishing bytes under the wrong digest must fail rather than store
        // them, or a row could name content that is not what it claims.
        let wrong = store.publish_object(&digest, b"different bytes");
        assert!(matches!(wrong, Err(StoreError::DigestMismatch { .. })));

        // Nothing is left behind by a refused publication.
        let staged: Vec<_> = walk(&store.objects)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(".staging-"))
            })
            .collect();
        assert!(
            staged.is_empty(),
            "a refused publication left staging files: {staged:?}"
        );
    }

    fn walk(root: &std::path::Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(root) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn the_published_digest_is_verified_from_the_bytes_on_disk() {
        let (_directory, store) = store();
        let bytes = b"evidence that must land intact";
        let digest = cbr_encoding::digest_bytes(bytes);
        let path = store.publish_object(&digest, bytes).expect("publishes");

        // The check that matters is the one against what the filesystem holds,
        // so re-reading the published object must reproduce the digest it was
        // published under.
        assert_eq!(
            cbr_encoding::digest_bytes(&fs::read(&path).unwrap()),
            digest
        );

        // Republishing identical bytes is a no-op, not an error.
        assert_eq!(store.publish_object(&digest, bytes).unwrap(), path);
    }
}
