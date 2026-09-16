//! Command intent and its digest, CORE section 6.1.
//!
//! The intent is what makes a retransmission the same command and a changed
//! request a different one. Fields excluded here are exactly the ones that may
//! legitimately differ between transmissions of one command: `message_id`,
//! `dedupe_generation`, `authority_epoch`, `correlation`, `caused_by`, and any
//! extension the command did not declare in `requires`.

use crate::digest::digest_canonical;
use crate::value::Value;

/// Members of the intent, in the order CORE section 6.1 lists them. Canonical
/// form sorts them anyway, so this order is documentation, not semantics.
const INTENT_MEMBERS: [&str; 5] = [
    "operation",
    "subject",
    "preconditions",
    "requires",
    "payload",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentError {
    /// The envelope is not an object.
    NotAnObject,
    /// A member the intent needs is absent.
    MissingMember(&'static str),
    /// `requires` is present but is not an array of strings.
    MalformedRequires,
    /// `extensions` is present but is not an object.
    MalformedExtensions,
    /// `requires` names an extension key the envelope does not carry. CORE
    /// section 10 step 2 rejects this as an envelope-semantics failure, so it
    /// must not reach the digest as a silently empty extension.
    RequiredExtensionMissing(String),
    /// The same `requires` entry twice. Section 5.1 states this rule alongside
    /// the one above, in the same step, with the same outcome.
    DuplicateRequires(String),
}

/// CORE section 5.1: a feature name is `<profile>.<feature>` and never contains
/// a slash; an extension key always contains one. That is the whole test for
/// telling a `requires` entry apart, and it is decidable from the entry alone.
fn is_extension_key(entry: &str) -> bool {
    entry.contains('/')
}

/// Build the command intent object from a full command envelope.
///
/// `extensions` is always present in the result. It contains only the
/// extensions whose keys are listed in `requires`, and is an empty object when
/// there are none, so an optional extension cannot change a command's identity.
///
/// Fails when the envelope violates either `requires` rule of section 5.1: a
/// duplicate entry, or an entry containing a slash with no matching member in
/// `extensions`. Pinned fixtures `core.envelope.requires-unique` and
/// `core.envelope.required-extension-must-be-present` require `invalid_envelope`
/// for these, with no state change.
pub fn command_intent(envelope: &Value) -> Result<Value, IntentError> {
    if !envelope.is_object() {
        return Err(IntentError::NotAnObject);
    }
    let mut intent: Vec<(String, Value)> = Vec::with_capacity(INTENT_MEMBERS.len() + 1);
    for name in INTENT_MEMBERS {
        let member = envelope.get(name).ok_or(IntentError::MissingMember(name))?;
        intent.push((name.to_string(), member.clone()));
    }

    let required: Vec<&str> = match envelope.get("requires") {
        Some(Value::Array(items)) => {
            let mut keys = Vec::with_capacity(items.len());
            for item in items {
                keys.push(item.as_str().ok_or(IntentError::MalformedRequires)?);
            }
            keys
        }
        _ => return Err(IntentError::MalformedRequires),
    };

    let declared = match envelope.get("extensions") {
        None => &[][..],
        Some(Value::Object(members)) => members.as_slice(),
        Some(_) => return Err(IntentError::MalformedExtensions),
    };

    // Both rules below are envelope semantics from section 5.1, checked in the
    // same step. They are enforced here rather than left to a caller because
    // an envelope that violates either has no well-defined intent: the digest
    // it would produce belongs to a command that must never be accepted.
    let mut seen: Vec<&str> = Vec::with_capacity(required.len());
    let mut included: Vec<(String, Value)> = Vec::new();
    for key in required {
        if seen.contains(&key) {
            return Err(IntentError::DuplicateRequires(key.to_string()));
        }
        seen.push(key);
        if !is_extension_key(key) {
            // A feature name. It constrains what must be negotiated, and
            // contributes nothing to the intent.
            continue;
        }
        match declared.iter().find(|(name, _)| name == key) {
            Some((name, value)) => included.push((name.clone(), value.clone())),
            None => return Err(IntentError::RequiredExtensionMissing(key.to_string())),
        }
    }
    intent.push(("extensions".to_string(), Value::Object(included)));
    Ok(Value::Object(intent))
}

/// The `command_digest` a provider recomputes and compares.
///
/// Recomputed from parsed values, never from the received bytes, so whitespace
/// and member order on the wire do not change a command's identity.
pub fn command_digest(envelope: &Value) -> Result<String, IntentError> {
    Ok(digest_canonical(&command_intent(envelope)?))
}
