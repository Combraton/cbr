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
//! Cycles are refused rather than recovered: CBR's derivations are a DAG, and
//! a cycle is a bug in the caller's functions, not a state to serve from.

use std::collections::BTreeMap;
use std::sync::Arc;

use rusqlite::Connection;

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

/// One registered function. It reads its dependencies through the session, in
/// the order it needs them, and may declare an untracked read.
pub type Computation = Arc<dyn Fn(&mut Session<'_>, &str) -> Result<Value, EvalError> + Send + Sync>;

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
        computation: impl Fn(&mut Session<'_>, &str) -> Result<Value, EvalError>
        + Send
        + Sync
        + 'static,
    ) {
        self.registered
            .insert(name.to_string(), Arc::new(computation));
    }
}

/// One evaluation against one connection. Every read a function performs goes
/// through here, which is what makes the dependency record exact.
pub struct Session<'a> {
    #[allow(dead_code)]
    connection: &'a Connection,
    #[allow(dead_code)]
    functions: &'a Functions,
    #[allow(dead_code)]
    stack: Vec<String>,
    #[allow(dead_code)]
    frames: Vec<Frame>,
}

#[derive(Default)]
#[allow(dead_code)]
struct Frame {
    dependencies: Vec<String>,
    untracked: bool,
}

impl Session<'_> {
    /// Read a dependency, recording the edge.
    pub fn read(&mut self, _key: &str) -> Result<Value, EvalError> {
        unimplemented!("m3b: the evaluator is not implemented yet")
    }

    /// Declare that this computation read something the graph does not
    /// record, so it can never be validated without re-executing.
    pub fn report_untracked(&mut self) {
        unimplemented!("m3b: the evaluator is not implemented yet")
    }
}

/// Create the evaluator's tables. Safe to call on every open.
pub fn migrate(_connection: &Connection) -> Result<(), EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}

/// Set an input's value and durability. Returns whether anything changed;
/// setting the value it already has advances no revision, so nothing that
/// read it is invalidated.
pub fn set_input(
    _connection: &Connection,
    _key: &str,
    _value: &[u8],
    _durability: Durability,
) -> Result<bool, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}

pub fn input(_connection: &Connection, _key: &str) -> Result<Option<Value>, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}

/// The current revision.
pub fn revision(_connection: &Connection) -> Result<i64, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}

/// Compute or reuse a derived value.
pub fn evaluate(
    _connection: &Connection,
    _functions: &Functions,
    _key: &str,
) -> Result<Value, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
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

pub fn memo(_connection: &Connection, _key: &str) -> Result<Option<Memo>, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}

/// Every memo that recorded a direct dependency on `key`, through the reverse
/// index. This is how a change is turned into the set of derived results that
/// must be re-evaluated or reported stale.
pub fn dependents(_connection: &Connection, _key: &str) -> Result<Vec<String>, EvalError> {
    unimplemented!("m3b: the evaluator is not implemented yet")
}
