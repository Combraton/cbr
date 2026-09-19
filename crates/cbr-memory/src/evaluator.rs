//! A persistent, demand-driven dependency evaluator (STACK section 7).
//!
//! Memory is versioned like a codebase, so what is derived from it — an
//! applicability result, a projection, a packet's selection — must be
//! invalidated by the inputs it actually read, not by a global counter. This
//! is the machinery that does it: every derived value records the inputs it
//! read, in order, and is reused only when none of them has changed.
//!
//! The algorithm is salsa's, kept deliberately small and written against
//! SQLite so a crash mid-recompute leaves the graph exactly as it was: all of
//! it runs inside the caller's transaction. What is copied is the part that is
//! easy to get wrong:
//!
//! - **Two revision stamps per memo.** `changed_at` is when the value last
//!   differed; `verified_at` is when it was last known current. A dependent
//!   asks "did you change after my last verification", not "were you touched".
//! - **Early cutoff by backdating.** A recomputed value equal to the old one
//!   keeps the old `changed_at`, so dependents stop rather than cascade.
//! - **Durability.** An input declares how often it changes. A memo whose
//!   inputs are all at least as durable as itself can be verified without
//!   walking anything, because no input that durable has changed.
//! - **The guards.** Backdating is refused when a recomputation's durability
//!   *decreased*, and a memo that read something untracked is never validated
//!   without re-executing. Both are unsoundness, not inefficiency: without
//!   them a dependent can be told "unchanged" about a value that changed.
//!
//! The durability guard is the subtle one, and the harness generates the shape
//! that needs it: a memo that used to read only durable inputs, and now reads
//! a volatile one, would keep a `changed_at` old enough for a dependent to
//! shallow-verify against a durability the dependent no longer has.
//!
//! Cycles are refused rather than recovered: CBR's derivations are a DAG, and
//! a cycle is a bug in the caller's functions, not a state to serve from.

use std::collections::BTreeMap;
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

/// A derived or input value. The comparator is byte equality of these bytes:
/// callers that want a looser one serialize accordingly.
pub type Value = Vec<u8>;

/// How often an input changes. A memo takes the least durable of its inputs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Durability {
    /// Changes freely: a working tree, a clock reading.
    Low = 0,
    /// Changes occasionally: a project's decisions.
    Medium = 1,
    /// Changes rarely: a sealed artifact's bytes, a pinned toolchain.
    High = 2,
}

impl Durability {
    fn level(self) -> i64 {
        self as i64
    }

    fn from_level(level: i64) -> Self {
        match level {
            2 => Durability::High,
            1 => Durability::Medium,
            _ => Durability::Low,
        }
    }
}

#[derive(Debug)]
pub enum EvalError {
    /// The functions asked for a key that is being computed already.
    Cycle(Vec<String>),
    /// No function is registered for this key's name.
    UnknownFunction(String),
    /// A function failed. The text is the caller's, and is not interpreted.
    Function(String),
    Storage(rusqlite::Error),
}

impl From<rusqlite::Error> for EvalError {
    fn from(error: rusqlite::Error) -> Self {
        EvalError::Storage(error)
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Cycle(stack) => write!(formatter, "cycle: {}", stack.join(" -> ")),
            EvalError::UnknownFunction(name) => write!(formatter, "unknown function {name}"),
            EvalError::Function(reason) => write!(formatter, "function failed: {reason}"),
            EvalError::Storage(error) => write!(formatter, "storage: {error}"),
        }
    }
}

/// A derived key: `name:argument`. The name selects a registered function and
/// the argument is passed to it.
pub fn key(name: &str, argument: &str) -> String {
    format!("{name}:{argument}")
}

fn split(key: &str) -> (&str, &str) {
    key.split_once(':').unwrap_or((key, ""))
}

/// One registered function. It reads its dependencies through the session, in
/// the order it needs them, and may declare an untracked read.
pub type Computation =
    Arc<dyn Fn(&mut Session<'_>, &str) -> Result<Value, EvalError> + Send + Sync>;

/// The functions a store's derived values are computed by. Registering is
/// code, not data: reopening a database means registering the same functions.
#[derive(Default, Clone)]
pub struct Functions {
    registered: BTreeMap<String, Computation>,
}

impl Functions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: &str,
        computation: impl Fn(&mut Session<'_>, &str) -> Result<Value, EvalError> + Send + Sync + 'static,
    ) {
        self.registered
            .insert(name.to_string(), Arc::new(computation));
    }

    fn get(&self, name: &str) -> Option<Computation> {
        self.registered.get(name).cloned()
    }
}

/// One evaluation against one connection. Every read a function performs goes
/// through here, which is what makes the dependency record exact.
pub struct Session<'a> {
    connection: &'a Connection,
    functions: &'a Functions,
    /// The keys being computed, innermost last: a repeat is a cycle.
    stack: Vec<String>,
    /// One frame per key on the stack, recording what it read.
    frames: Vec<Frame>,
    /// The revision this whole evaluation is against. Inputs do not change
    /// under it, because setting an input is a separate call.
    revision: i64,
    /// Keys already verified or recomputed in this evaluation.
    settled: Vec<String>,
}

#[derive(Default)]
struct Frame {
    dependencies: Vec<String>,
    untracked: bool,
}

impl Session<'_> {
    /// Read a dependency, recording the edge.
    pub fn read(&mut self, key: &str) -> Result<Value, EvalError> {
        if let Some(frame) = self.frames.last_mut()
            && !frame.dependencies.iter().any(|recorded| recorded == key)
        {
            frame.dependencies.push(key.to_string());
        }
        self.fetch(key)
    }

    /// Declare that this computation read something the graph does not
    /// record, so it can never be validated without re-executing.
    pub fn report_untracked(&mut self) {
        if let Some(frame) = self.frames.last_mut() {
            frame.untracked = true;
        }
    }

    /// The value of a key, computing or validating it as needed.
    fn fetch(&mut self, key: &str) -> Result<Value, EvalError> {
        if let Some(input) = read_input(self.connection, key)? {
            return Ok(input.value);
        }
        if self.stack.iter().any(|open| open == key) {
            let mut cycle = self.stack.clone();
            cycle.push(key.to_string());
            return Err(EvalError::Cycle(cycle));
        }
        if let Some(existing) = read_memo(self.connection, key)?
            && self.validate(key, &existing)?
        {
            return Ok(existing.value);
        }
        self.execute(key)
    }

    /// Whether a memo is still current, without recomputing it.
    fn validate(&mut self, key: &str, memo: &Row) -> Result<bool, EvalError> {
        if memo.verified_at == self.revision || self.settled.iter().any(|done| done == key) {
            return Ok(true);
        }
        // A memo that read something untracked is worth nothing without
        // re-executing: the thing it read is not in the graph to compare.
        if memo.untracked {
            return Ok(false);
        }
        // Shallow: no input at least as durable as this memo has changed since
        // it was verified, and every input it read is at least that durable.
        if changed_at_or_above(self.connection, memo.durability)? <= memo.verified_at {
            mark_verified(self.connection, key, self.revision)?;
            self.settled.push(key.to_string());
            return Ok(true);
        }
        // Deep: walk the recorded edges, in the order they were read.
        for dependency in &memo.dependencies {
            if self.changed_after(dependency, memo.verified_at)? {
                return Ok(false);
            }
        }
        mark_verified(self.connection, key, self.revision)?;
        self.settled.push(key.to_string());
        Ok(true)
    }

    /// Whether a dependency changed after `since`. A derived dependency is
    /// brought up to date first, which is what makes this demand-driven.
    fn changed_after(&mut self, key: &str, since: i64) -> Result<bool, EvalError> {
        if let Some(input) = read_input(self.connection, key)? {
            return Ok(input.changed_at > since);
        }
        self.fetch(key)?;
        let memo = read_memo(self.connection, key)?
            .ok_or_else(|| EvalError::Function(format!("no memo for {key} after fetching it")))?;
        Ok(memo.changed_at > since)
    }

    fn execute(&mut self, key: &str) -> Result<Value, EvalError> {
        let (name, argument) = split(key);
        let computation = self
            .functions
            .get(name)
            .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
        let previous = read_memo(self.connection, key)?;

        self.stack.push(key.to_string());
        self.frames.push(Frame::default());
        let computed = computation(self, argument);
        let frame = self.frames.pop().unwrap_or_default();
        self.stack.pop();
        let value = computed?;

        // A memo is as durable as its least durable input, and one that read
        // something untracked is not durable at all.
        let mut durability = Durability::High;
        for dependency in &frame.dependencies {
            durability = durability.min(self.durability_of(dependency)?);
        }
        if frame.untracked {
            durability = Durability::Low;
        }
        // Early cutoff, with the guard: the same value keeps its old
        // `changed_at` so dependents stop here, but never when this
        // recomputation is less durable than the memo it replaces.
        let backdate = previous
            .as_ref()
            .is_some_and(|old| old.value == value && durability >= old.durability);
        let changed_at = match (&previous, backdate) {
            (Some(old), true) => old.changed_at,
            _ => self.revision,
        };
        write_memo(
            self.connection,
            key,
            &Row {
                value: value.clone(),
                changed_at,
                verified_at: self.revision,
                durability,
                untracked: frame.untracked,
                dependencies: frame.dependencies,
            },
        )?;
        self.settled.push(key.to_string());
        Ok(value)
    }

    fn durability_of(&mut self, key: &str) -> Result<Durability, EvalError> {
        if let Some(input) = read_input(self.connection, key)? {
            return Ok(input.durability);
        }
        match read_memo(self.connection, key)? {
            Some(memo) => Ok(memo.durability),
            // A dependency read through `read` always has a memo by now.
            None => Ok(Durability::Low),
        }
    }
}

// ---- storage ---------------------------------------------------------------

/// Create the evaluator's tables. Safe to call on every open.
pub fn migrate(connection: &Connection) -> Result<(), EvalError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS memo_meta (
             key   TEXT PRIMARY KEY,
             value INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS memo_input (
             key        TEXT PRIMARY KEY,
             value      BLOB NOT NULL,
             changed_at INTEGER NOT NULL,
             durability INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS memo_node (
             key         TEXT PRIMARY KEY,
             value       BLOB NOT NULL,
             changed_at  INTEGER NOT NULL,
             verified_at INTEGER NOT NULL,
             durability  INTEGER NOT NULL,
             untracked   INTEGER NOT NULL
         );
         -- Ordered, because a dependency read first must be checked first: a
         -- later read can depend on the value of an earlier one.
         CREATE TABLE IF NOT EXISTS memo_edge (
             node       TEXT NOT NULL,
             position   INTEGER NOT NULL,
             dependency TEXT NOT NULL,
             PRIMARY KEY (node, position)
         );
         CREATE INDEX IF NOT EXISTS memo_edge_dependency ON memo_edge (dependency);",
    )?;
    Ok(())
}

struct Input {
    value: Value,
    changed_at: i64,
    durability: Durability,
}

struct Row {
    value: Value,
    changed_at: i64,
    verified_at: i64,
    durability: Durability,
    untracked: bool,
    dependencies: Vec<String>,
}

fn meta(connection: &Connection, name: &str) -> Result<i64, EvalError> {
    Ok(connection
        .query_row(
            "SELECT value FROM memo_meta WHERE key = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn set_meta(connection: &Connection, name: &str, value: i64) -> Result<(), EvalError> {
    connection.execute(
        "INSERT INTO memo_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![name, value],
    )?;
    Ok(())
}

/// The last revision in which an input of at least this durability changed.
/// A memo is exactly as shallow-verifiable as this number allows.
fn changed_at_or_above(connection: &Connection, durability: Durability) -> Result<i64, EvalError> {
    meta(
        connection,
        &format!("changed_at_or_above_{}", durability.level()),
    )
}

fn read_input(connection: &Connection, key: &str) -> Result<Option<Input>, EvalError> {
    Ok(connection
        .query_row(
            "SELECT value, changed_at, durability FROM memo_input WHERE key = ?1",
            params![key],
            |row| {
                Ok(Input {
                    value: row.get(0)?,
                    changed_at: row.get(1)?,
                    durability: Durability::from_level(row.get(2)?),
                })
            },
        )
        .optional()?)
}

fn read_memo(connection: &Connection, key: &str) -> Result<Option<Row>, EvalError> {
    let row = connection
        .query_row(
            "SELECT value, changed_at, verified_at, durability, untracked
             FROM memo_node WHERE key = ?1",
            params![key],
            |row| {
                Ok(Row {
                    value: row.get(0)?,
                    changed_at: row.get(1)?,
                    verified_at: row.get(2)?,
                    durability: Durability::from_level(row.get(3)?),
                    untracked: row.get::<_, i64>(4)? != 0,
                    dependencies: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut row) = row else {
        return Ok(None);
    };
    let mut statement =
        connection.prepare("SELECT dependency FROM memo_edge WHERE node = ?1 ORDER BY position")?;
    let dependencies = statement.query_map(params![key], |row| row.get::<_, String>(0))?;
    for dependency in dependencies {
        row.dependencies.push(dependency?);
    }
    Ok(Some(row))
}

fn write_memo(connection: &Connection, key: &str, row: &Row) -> Result<(), EvalError> {
    connection.execute(
        "INSERT INTO memo_node (key, value, changed_at, verified_at, durability, untracked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(key) DO UPDATE SET value = ?2, changed_at = ?3, verified_at = ?4,
                                        durability = ?5, untracked = ?6",
        params![
            key,
            row.value,
            row.changed_at,
            row.verified_at,
            row.durability.level(),
            i64::from(row.untracked)
        ],
    )?;
    connection.execute("DELETE FROM memo_edge WHERE node = ?1", params![key])?;
    for (position, dependency) in row.dependencies.iter().enumerate() {
        connection.execute(
            "INSERT INTO memo_edge (node, position, dependency) VALUES (?1, ?2, ?3)",
            params![key, position as i64, dependency],
        )?;
    }
    Ok(())
}

fn mark_verified(connection: &Connection, key: &str, revision: i64) -> Result<(), EvalError> {
    connection.execute(
        "UPDATE memo_node SET verified_at = ?2 WHERE key = ?1",
        params![key, revision],
    )?;
    Ok(())
}

// ---- the caller's interface ------------------------------------------------

/// Set an input's value and durability. Returns whether anything changed;
/// setting the value it already has advances no revision, so nothing that
/// read it is invalidated.
pub fn set_input(
    connection: &Connection,
    key: &str,
    value: &[u8],
    durability: Durability,
) -> Result<bool, EvalError> {
    let existing = read_input(connection, key)?;
    if let Some(existing) = &existing
        && existing.value == value
        && existing.durability == durability
    {
        return Ok(false);
    }
    let revision = meta(connection, "revision")? + 1;
    set_meta(connection, "revision", revision)?;
    connection.execute(
        "INSERT INTO memo_input (key, value, changed_at, durability) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = ?2, changed_at = ?3, durability = ?4",
        params![key, value, revision, durability.level()],
    )?;
    // Every level at or below this input's durability has now seen a change
    // -- and, when the durability *decreased*, every level at or below the
    // durability it used to have. Memos that read this input recorded the old
    // durability, and a memo shallow-verifies against the level it recorded:
    // leaving the old level alone would let one of them skip verification and
    // report unchanged for an input that changed. The property harness found
    // exactly this, at an input that went from high durability to medium.
    let announced = durability
        .level()
        .max(existing.map_or(0, |existing| existing.durability.level()));
    for level in 0..=announced {
        set_meta(
            connection,
            &format!("changed_at_or_above_{level}"),
            revision,
        )?;
    }
    Ok(true)
}

pub fn input(connection: &Connection, key: &str) -> Result<Option<Value>, EvalError> {
    Ok(read_input(connection, key)?.map(|input| input.value))
}

/// The current revision.
pub fn revision(connection: &Connection) -> Result<i64, EvalError> {
    meta(connection, "revision")
}

/// Compute or reuse a derived value.
pub fn evaluate(
    connection: &Connection,
    functions: &Functions,
    key: &str,
) -> Result<Value, EvalError> {
    let mut session = Session {
        connection,
        functions,
        stack: Vec::new(),
        frames: Vec::new(),
        revision: revision(connection)?,
        settled: Vec::new(),
    };
    session.fetch(key)
}

/// What a memo recorded, for tests and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memo {
    pub value: Value,
    pub changed_at: i64,
    pub verified_at: i64,
    pub durability: Durability,
    pub untracked: bool,
    pub dependencies: Vec<String>,
}

pub fn memo(connection: &Connection, key: &str) -> Result<Option<Memo>, EvalError> {
    Ok(read_memo(connection, key)?.map(|row| Memo {
        value: row.value,
        changed_at: row.changed_at,
        verified_at: row.verified_at,
        durability: row.durability,
        untracked: row.untracked,
        dependencies: row.dependencies,
    }))
}

/// Every memo that recorded a direct dependency on `key`, through the reverse
/// index. This is how a change is turned into the set of derived results that
/// must be re-evaluated or reported stale.
pub fn dependents(connection: &Connection, key: &str) -> Result<Vec<String>, EvalError> {
    let mut statement = connection
        .prepare("SELECT DISTINCT node FROM memo_edge WHERE dependency = ?1 ORDER BY node")?;
    let rows = statement.query_map(params![key], |row| row.get::<_, String>(0))?;
    let mut dependents = Vec::new();
    for row in rows {
        dependents.push(row?);
    }
    Ok(dependents)
}
