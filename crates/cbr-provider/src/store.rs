//! Provider state.
//!
//! **This store is in memory and is not durable.** No fixture in the `stream`
//! suite requires durability, so making it durable here would be work done
//! before anything could check it. The SQLite store specified in
//! `docs/work/readiness/STACK.md` section 3 — WAL, `synchronous=FULL`,
//! object-before-row publication — arrives with the Core suite, which does
//! exercise restart. Until then, nothing in this repository may claim the
//! store survives a restart.
//!
//! The shapes here are the durable ones, so replacing the backing is a change
//! of storage rather than a change of model.

use std::collections::HashMap;

use cbr_encoding::Value;

/// A provider-owned object, named by kind and id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectKey {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Default)]
pub struct SubjectState {
    /// Provider-owned integer. `0` means the subject does not exist, so a
    /// subject that is absent from the map is at revision 0 by definition.
    pub revision: i64,
    /// The value the last applied command set. Read back by `core-test`
    /// operations in the Core suite.
    pub value: String,
    /// How many commands actually applied to this subject. A replay does not
    /// increment it, which is how a fixture tells a genuine idempotent replay
    /// apart from a re-execution.
    pub applied_count: i64,
}

/// The durable record of an accepted command, which is what binds its identity.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub digest: String,
    /// The exact result to return on replay, byte-for-byte under canonical
    /// encoding, with `replay` set to true.
    pub result: Value,
}

#[derive(Debug, Default)]
pub struct Store {
    subjects: HashMap<SubjectKey, SubjectState>,
    /// Keyed by the deduplication scope — the principal — and the command id.
    commands: HashMap<(String, String), CommandRecord>,
    /// Current authority epoch per scope. A scope with no entry is at 0.
    epochs: HashMap<String, i64>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subject(&self, key: &SubjectKey) -> Option<&SubjectState> {
        self.subjects.get(key)
    }

    /// Claim the next authority epoch for a scope, returning it.
    pub fn claim_epoch(&mut self, scope: &str) -> i64 {
        let epoch = self.epochs.entry(scope.to_string()).or_insert(0);
        *epoch += 1;
        *epoch
    }

    pub fn revision(&self, key: &SubjectKey) -> i64 {
        self.subjects.get(key).map_or(0, |state| state.revision)
    }

    /// Apply a `core-test.subject.put`, returning the new revision.
    pub fn put(&mut self, key: SubjectKey, value: String) -> i64 {
        let state = self.subjects.entry(key).or_default();
        state.revision += 1;
        state.applied_count += 1;
        state.value = value;
        state.revision
    }

    pub fn applied_count(&self, key: &SubjectKey) -> i64 {
        self.subjects
            .get(key)
            .map_or(0, |state| state.applied_count)
    }

    pub fn command(&self, principal: &str, command_id: &str) -> Option<&CommandRecord> {
        self.commands
            .get(&(principal.to_string(), command_id.to_string()))
    }

    pub fn bind_command(&mut self, principal: &str, command_id: &str, record: CommandRecord) {
        self.commands
            .insert((principal.to_string(), command_id.to_string()), record);
    }

    pub fn epoch(&self, scope: &str) -> i64 {
        self.epochs.get(scope).copied().unwrap_or(0)
    }
}
