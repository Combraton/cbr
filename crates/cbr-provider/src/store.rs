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

/// What one accepted command changes and binds.
pub struct Commit<'a> {
    pub key: &'a SubjectKey,
    pub value: &'a str,
    pub principal: &'a str,
    pub command_id: &'a str,
    pub digest: &'a str,
    pub generation: i64,
}

pub struct Store {
    connection: Connection,
    // Read only by `publish_object`, which lands ahead of its consumer.
    #[allow(dead_code)]
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
             CREATE TABLE IF NOT EXISTS subjects (
                 kind          TEXT    NOT NULL,
                 id            TEXT    NOT NULL,
                 revision      INTEGER NOT NULL,
                 value         TEXT    NOT NULL,
                 applied_count INTEGER NOT NULL,
                 PRIMARY KEY (kind, id)
             );
             CREATE TABLE IF NOT EXISTS commands (
                 principal  TEXT    NOT NULL,
                 command_id TEXT    NOT NULL,
                 digest     TEXT    NOT NULL,
                 generation INTEGER NOT NULL,
                 result     TEXT    NOT NULL,
                 PRIMARY KEY (principal, command_id)
             );",
        )?;
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
        make_result: impl FnOnce(i64) -> Value,
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
        let result = make_result(revision);
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
            // Content addressing means equal bytes are already published.
            return Ok(final_path);
        }

        let staged = directory.join(format!(".staging-{hex}"));
        {
            let mut file = fs::File::create(&staged)?;
            file.write_all(bytes)?;
            file.sync_all()?;
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
                    },
                    |revision| Value::Object(vec![("revision".into(), Value::Int(revision))]),
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
                    },
                    |_| Value::Object(vec![]),
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

        // Republishing identical bytes is a no-op, not an error.
        assert_eq!(store.publish_object(&digest, bytes).unwrap(), path);
    }
}
