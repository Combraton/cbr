//! Effect records and obligations (CORE section 19, feature `core.effects`).
//!
//! An effect is something CBR does, or attempts, **outside its own store**,
//! whose outcome can be uncertain even though CBR's records are durable. In
//! CBR the first real ones arrive with the model runtime: submitting a prompt
//! to a provider is exactly an effect with an uncertain outcome and a retry
//! class. Nothing in M1 produces one, so no operation here returns a non-empty
//! `effect_refs`, and **no fixture CBR can run exercises this feature**. What
//! exists is the record, the query, the obligation lifecycle and the producer
//! API, and they rest on CBR's own tests.
//!
//! The record is stored as the value of a subject of kind `core.effect`, the
//! same way a grant is: revisions, durability and the command transaction are
//! the store's, and `core.effects.abort_obligation`'s precondition is an
//! ordinary precondition on that subject.

use cbr_encoding::Value;

use crate::store::SubjectKey;

pub const KIND: &str = "core.effect";

/// The right `core.effects.abort_obligation` needs on the effect's target.
pub const ABORT_RIGHT: &str = "core.effects.abort_obligation";

/// Retry classes (CORE section 19.3). Recorded with the effect; a producer
/// must not retry outside its class, which is the producer's obligation and
/// is enforced where effects are produced, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))] // Produced from M4; exercised by tests now.
pub enum RetryClass {
    Pure,
    Read,
    IdempotentKey,
    Compensatable,
    NonRepeatable,
}

impl RetryClass {
    fn name(self) -> &'static str {
        match self {
            RetryClass::Pure => "pure",
            RetryClass::Read => "read",
            RetryClass::IdempotentKey => "idempotent_key",
            RetryClass::Compensatable => "compensatable",
            RetryClass::NonRepeatable => "non_repeatable",
        }
    }
}

/// An effect a command authorizes, recorded in that command's transaction
/// **before** any external I/O (CORE section 19.1).
#[cfg_attr(not(test), allow(dead_code))] // Produced from M4; exercised by tests now.
pub struct NewEffect {
    pub kind: String,
    pub target: SubjectKey,
    pub payload_digest: String,
    pub retry_class: RetryClass,
    pub idempotency_key: Option<String>,
    /// `(id, expects, deadline)`: what observation each obligation waits for,
    /// and by when on the provider clock.
    pub obligations: Vec<(String, String, Option<String>)>,
}

/// The effect id for the `index`th effect a command records. Derived from the
/// command's operation reference, which is minted in the same transaction, so
/// a network retry of the command replays the same ids and a new command can
/// never collide with an old one.
pub fn effect_id(operation_ref: &str, index: usize) -> String {
    format!("{operation_ref}.e{}", index + 1)
}

/// The initial record: descriptor, one `pending` observation, no attempts.
pub fn new_record(
    effect: &NewEffect,
    id: &str,
    operation_ref: &str,
    principal: &str,
    grant: Option<&str>,
    recorded_at: &str,
) -> Value {
    let mut descriptor = vec![
        ("id".into(), Value::String(id.into())),
        ("kind".into(), Value::String(effect.kind.clone())),
        ("target".into(), subject_value(&effect.target)),
        (
            "payload_digest".into(),
            Value::String(effect.payload_digest.clone()),
        ),
        ("authorization".into(), {
            let mut authorization = vec![("principal".into(), Value::String(principal.into()))];
            if let Some(grant) = grant {
                authorization.push(("grant".into(), Value::String(grant.into())));
            }
            Value::Object(authorization)
        }),
        (
            "retry_class".into(),
            Value::String(effect.retry_class.name().into()),
        ),
        ("operation_ref".into(), Value::String(operation_ref.into())),
    ];
    if let Some(key) = &effect.idempotency_key {
        descriptor.push(("idempotency_key".into(), Value::String(key.clone())));
    }
    Value::Object(vec![
        ("descriptor".into(), Value::Object(descriptor)),
        (
            "observations".into(),
            Value::Array(vec![observation(
                "pending",
                "provider",
                "recorded before any external action",
                recorded_at,
            )]),
        ),
        ("attempts".into(), Value::Array(vec![])),
        (
            "obligations".into(),
            Value::Array(
                effect
                    .obligations
                    .iter()
                    .map(|(id, expects, deadline)| {
                        Value::Object(vec![
                            ("id".into(), Value::String(id.clone())),
                            ("expects".into(), Value::String(expects.clone())),
                            (
                                "deadline".into(),
                                deadline.clone().map_or(Value::Null, Value::String),
                            ),
                            ("state".into(), Value::String("open".into())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn subject_value(key: &SubjectKey) -> Value {
    Value::Object(vec![
        ("kind".into(), Value::String(key.kind.clone())),
        ("id".into(), Value::String(key.id.clone())),
    ])
}

fn observation(status: &str, class: &str, source: &str, recorded_at: &str) -> Value {
    Value::Object(vec![
        ("status".into(), Value::String(status.into())),
        (
            "evidence".into(),
            Value::Object(vec![
                ("class".into(), Value::String(class.into())),
                ("source".into(), Value::String(source.into())),
            ]),
        ),
        ("recorded_at".into(), Value::String(recorded_at.into())),
    ])
}

/// The effect's target subject.
pub fn target(record: &Value) -> Option<SubjectKey> {
    let target = record.get("descriptor")?.get("target")?;
    Some(SubjectKey {
        kind: target.get("kind")?.as_str()?.into(),
        id: target.get("id")?.as_str()?.into(),
    })
}

/// The latest observation's status (CORE section 19.2).
pub fn status(record: &Value) -> String {
    record
        .get("observations")
        .and_then(Value::as_array)
        .and_then(|observations| observations.last())
        .and_then(|last| last.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

/// `core.effects.get`'s result for a stored record at `revision`.
pub fn get_result(record: &Value, revision: i64) -> Value {
    let member = |name: &str| record.get(name).cloned().unwrap_or(Value::Array(vec![]));
    Value::Object(vec![
        (
            "effect".into(),
            record.get("descriptor").cloned().unwrap_or(Value::Null),
        ),
        ("revision".into(), Value::Int(revision)),
        ("status".into(), Value::String(status(record))),
        ("observations".into(), member("observations")),
        ("attempts".into(), member("attempts")),
        ("obligations".into(), member("obligations")),
    ])
}

fn obligations_mut(record: &mut Value) -> Vec<&mut Vec<(String, Value)>> {
    let Value::Object(members) = record else {
        return Vec::new();
    };
    let Some((_, Value::Array(obligations))) =
        members.iter_mut().find(|(name, _)| name == "obligations")
    else {
        return Vec::new();
    };
    obligations
        .iter_mut()
        .filter_map(|obligation| match obligation {
            Value::Object(fields) => Some(fields),
            _ => None,
        })
        .collect()
}

fn field<'a>(fields: &'a [(String, Value)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .and_then(|(_, value)| value.as_str())
}

fn set_state(fields: &mut [(String, Value)], state: &str) {
    if let Some((_, value)) = fields.iter_mut().find(|(field, _)| field == "state") {
        *value = Value::String(state.into());
    }
}

/// Whether an obligation with this id is still waiting: `open` or `overdue`.
/// Anything else is `not_found` to `abort_obligation` (CORE section 19.4).
pub fn obligation_waiting(record: &Value, obligation: &str) -> bool {
    record
        .get("obligations")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .any(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(obligation)
                && matches!(
                    entry.get("state").and_then(Value::as_str),
                    Some("open" | "overdue")
                )
        })
}

/// Abort a waiting obligation. **The effect's status is not touched**: an
/// aborted wait for an `unknown` effect leaves it `unknown` (EFF-4). Closing a
/// wait is not an observation.
pub fn abort(record: &mut Value, obligation: &str) {
    for fields in obligations_mut(record) {
        if field(fields, "id") == Some(obligation) {
            set_state(fields, "aborted");
        }
    }
}

/// Mark every `open` obligation whose deadline has passed as `overdue`, and
/// return their ids. A deadline equal to now has passed. An ended wait is not
/// an observation, so this never satisfies anything (CORE section 19.4).
pub fn mark_overdue(record: &mut Value, now: &str) -> Vec<String> {
    let mut marked = Vec::new();
    for fields in obligations_mut(record) {
        let passed = field(fields, "deadline").is_some_and(|deadline| deadline <= now);
        if field(fields, "state") == Some("open") && passed {
            marked.push(field(fields, "id").unwrap_or_default().to_string());
            set_state(fields, "overdue");
        }
    }
    marked
}

/// Append an observation. A settled outcome — `succeeded` or `failed` —
/// satisfies obligations that expect an outcome, whether they are still open
/// or already overdue. `pending` and `unknown` satisfy nothing: an outcome
/// that could not be established leaves the wait open, which is exactly what
/// CORE section 19.2 requires of a provider that cannot establish it.
#[cfg_attr(not(test), allow(dead_code))] // Produced from M4; exercised by tests now.
pub fn observe(record: &mut Value, status: &str, class: &str, source: &str, recorded_at: &str) {
    if let Value::Object(members) = record
        && let Some((_, Value::Array(observations))) =
            members.iter_mut().find(|(name, _)| name == "observations")
    {
        observations.push(observation(status, class, source, recorded_at));
    }
    if matches!(status, "succeeded" | "failed") {
        for fields in obligations_mut(record) {
            if field(fields, "expects") == Some("outcome")
                && matches!(field(fields, "state"), Some("open" | "overdue"))
            {
                set_state(fields, "satisfied");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(deadline: Option<&str>) -> Value {
        new_record(
            &NewEffect {
                kind: "cbr.model_call".into(),
                target: SubjectKey {
                    kind: "core-test.subject".into(),
                    id: "s-1".into(),
                },
                payload_digest: format!("sha256:{}", "a".repeat(64)),
                retry_class: RetryClass::NonRepeatable,
                idempotency_key: None,
                obligations: vec![("o-1".into(), "outcome".into(), deadline.map(str::to_string))],
            },
            "op-1.e1",
            "op-1",
            "owner",
            None,
            "2030-01-01T00:00:00Z",
        )
    }

    #[test]
    fn retry_classes_carry_the_schemas_names() {
        // The schema's `retry_class` enum, in order. A misspelled class would
        // be recorded silently and refused by every reader that validates.
        let names: Vec<&str> = [
            RetryClass::Pure,
            RetryClass::Read,
            RetryClass::IdempotentKey,
            RetryClass::Compensatable,
            RetryClass::NonRepeatable,
        ]
        .iter()
        .map(|class| class.name())
        .collect();
        assert_eq!(
            names,
            [
                "pure",
                "read",
                "idempotent_key",
                "compensatable",
                "non_repeatable"
            ]
        );
    }

    #[test]
    fn aborting_a_wait_never_changes_the_effects_status() {
        let mut effect = record(None);
        observe(
            &mut effect,
            "unknown",
            "provider",
            "response lost",
            "2030-01-01T00:00:01Z",
        );
        assert_eq!(status(&effect), "unknown");
        assert!(
            obligation_waiting(&effect, "o-1"),
            "unknown satisfies nothing"
        );

        abort(&mut effect, "o-1");
        assert_eq!(
            status(&effect),
            "unknown",
            "EFF-4: still unknown after the abort"
        );
        assert!(!obligation_waiting(&effect, "o-1"));
    }

    #[test]
    fn a_passed_deadline_makes_an_obligation_overdue_not_satisfied() {
        let mut effect = record(Some("2030-01-01T00:01:00Z"));
        assert!(mark_overdue(&mut effect, "2030-01-01T00:00:59Z").is_empty());
        assert_eq!(
            mark_overdue(&mut effect, "2030-01-01T00:01:00Z"),
            vec!["o-1"]
        );
        assert_eq!(
            status(&effect),
            "pending",
            "an ended wait is not an observation"
        );
        assert!(
            obligation_waiting(&effect, "o-1"),
            "overdue is still waiting"
        );
        // Marked once, not again on every later tick.
        assert!(mark_overdue(&mut effect, "2030-01-01T00:02:00Z").is_empty());

        // The expected observation, arriving late, still satisfies it.
        observe(
            &mut effect,
            "succeeded",
            "provider",
            "response",
            "2030-01-01T00:03:00Z",
        );
        assert!(!obligation_waiting(&effect, "o-1"));
    }
}
