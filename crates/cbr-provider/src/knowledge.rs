//! The knowledge profile's records and pure rules (`knowledge/1`).
//!
//! Claims, decisions, evaluations, conflicts and authority bindings are
//! subjects, so Core revisions, preconditions and events apply to them
//! unchanged. A claim revision's record also lives in an insert-only table
//! beside its subject, because a revision is immutable once proposed and the
//! lineage keeps every one of them.
//!
//! Every rule here compares **structure, not meaning**: exact equality of
//! identifiers, qualifier values, trees and canonical JSON. Where structure
//! cannot settle a question the answer is uncertain, never guessed
//! (KNOWLEDGE, introduction). Everything in this module is a pure function of
//! its inputs; the provider owns the command path, authorization and the store.

use cbr_encoding::Value;

use crate::envelope::{is_dotted_name, is_identifier};
use crate::errors::ProtocolError;
use crate::store::SubjectKey;

pub const CLAIM: &str = "knowledge.claim";
pub const DECISION: &str = "knowledge.decision";
pub const EVALUATION: &str = "knowledge.evaluation";
pub const CONFLICT: &str = "knowledge.conflict";
pub const AUTHORITY: &str = "knowledge.authority";
pub const CLAIM_FORMAT: &str = "combraton-knowledge-claim/1";
pub const CONDITION_KINDS: [&str; 3] = ["repository_tree", "dirty_snapshot", "environment_digest"];

/// The largest canonical statement value (KNOWLEDGE section 3).
const MAX_VALUE_BYTES: usize = 4096;
const MAX_SAFE: i64 = cbr_encoding::MAX_SAFE_INTEGER;

pub fn key(kind: &str, id: &str) -> SubjectKey {
    SubjectKey {
        kind: kind.into(),
        id: id.into(),
    }
}

pub fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("canonical form is UTF-8")
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn string(text: &str) -> Value {
    Value::String(text.into())
}

// ---- step 2: shapes and the rules a schema cannot state --------------------

pub(crate) type Checked<T> = Result<T, ProtocolError>;

fn invalid(path: &str, reason: &str) -> ProtocolError {
    ProtocolError::invalid_envelope(path, reason)
}

/// A closed object: no member outside `allowed`, every member of `required`.
pub(crate) fn closed<'a>(
    value: Option<&'a Value>,
    path: &str,
    allowed: &[&str],
    required: &[&str],
) -> Checked<&'a Value> {
    let Some(value @ Value::Object(members)) = value else {
        return Err(invalid(path, "not an object"));
    };
    for (name, _) in members {
        if !allowed.contains(&name.as_str()) {
            return Err(invalid(&format!("{path}/{name}"), "unknown field"));
        }
    }
    for name in required {
        if value.get(name).is_none() {
            return Err(invalid(&format!("{path}/{name}"), "required field absent"));
        }
    }
    Ok(value)
}

/// A string of `min..=max` code points.
pub(crate) fn text_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    min: usize,
    max: usize,
) -> Checked<&'a str> {
    match value {
        Some(Value::String(text)) if (min..=max).contains(&text.chars().count()) => Ok(text),
        _ => Err(invalid(
            path,
            &format!("not a string of {min} to {max} characters"),
        )),
    }
}

pub(crate) fn identifier_of<'a>(value: Option<&'a Value>, path: &str) -> Checked<&'a str> {
    match value {
        Some(Value::String(text)) if is_identifier(text) => Ok(text),
        _ => Err(invalid(path, "not an identifier")),
    }
}

/// An algorithm-qualified lowercase-hex digest, with the known algorithms'
/// lengths checked (CORE section 10 step 2).
pub(crate) fn digest_of<'a>(value: Option<&'a Value>, path: &str) -> Checked<&'a str> {
    let Some(Value::String(text)) = value else {
        return Err(invalid(path, "not a digest"));
    };
    let shaped = text.len() <= 256
        && text.split_once(':').is_some_and(|(algorithm, hex)| {
            !algorithm.is_empty()
                && algorithm
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+._-".contains(&b))
                && !hex.is_empty()
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
    if !shaped {
        return Err(invalid(path, "not a digest"));
    }
    match cbr_encoding::parse_digest(text) {
        Ok(_) | Err(cbr_encoding::DigestError::UnsupportedAlgorithm(_)) => Ok(text),
        Err(_) => Err(invalid(path, "wrong length for its algorithm")),
    }
}

pub(crate) fn instant_of<'a>(value: Option<&'a Value>, path: &str) -> Checked<&'a str> {
    match value {
        Some(Value::String(text)) if crate::grants::is_instant(text) => Ok(text),
        _ => Err(invalid(path, "not a UTC instant")),
    }
}

pub(crate) fn one_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    allowed: &[&str],
) -> Checked<&'a str> {
    match value {
        Some(Value::String(text)) if allowed.contains(&text.as_str()) => Ok(text),
        _ => Err(invalid(path, &format!("not one of {}", allowed.join(", ")))),
    }
}

pub(crate) fn integer_of(value: Option<&Value>, path: &str, min: i64) -> Checked<i64> {
    match value {
        Some(Value::Int(n)) if (min..=MAX_SAFE).contains(n) => Ok(*n),
        _ => Err(invalid(path, &format!("not an integer of at least {min}"))),
    }
}

pub(crate) fn array_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    max: usize,
) -> Checked<&'a [Value]> {
    match value {
        Some(Value::Array(items)) if items.len() <= max => Ok(items),
        _ => Err(invalid(
            path,
            &format!("not an array of at most {max} items"),
        )),
    }
}

/// `{ provider, claim, revision, digest }`.
pub fn check_claim_reference(value: Option<&Value>, path: &str) -> Checked<()> {
    let reference = closed(
        value,
        path,
        &["provider", "claim", "revision", "digest"],
        &["provider", "claim", "revision", "digest"],
    )?;
    identifier_of(reference.get("provider"), &format!("{path}/provider"))?;
    identifier_of(reference.get("claim"), &format!("{path}/claim"))?;
    integer_of(reference.get("revision"), &format!("{path}/revision"), 1)?;
    digest_of(reference.get("digest"), &format!("{path}/digest"))?;
    Ok(())
}

/// An evidence reference that names its provider (KNOWLEDGE section 2).
fn check_evidence_reference(value: Option<&Value>, path: &str) -> Checked<()> {
    let reference = closed(
        value,
        path,
        &["provider", "artifact", "digest"],
        &["provider", "artifact", "digest"],
    )?;
    identifier_of(reference.get("provider"), &format!("{path}/provider"))?;
    let artifact = closed(
        reference.get("artifact"),
        &format!("{path}/artifact"),
        &["kind", "id"],
        &["kind", "id"],
    )?;
    one_of(
        artifact.get("kind"),
        &format!("{path}/artifact/kind"),
        &[crate::evidence::ARTIFACT],
    )?;
    identifier_of(artifact.get("id"), &format!("{path}/artifact/id"))?;
    digest_of(reference.get("digest"), &format!("{path}/digest"))?;
    Ok(())
}

fn check_root(value: Option<&Value>, path: &str) -> Checked<()> {
    match value.and_then(|v| v.get("kind")).and_then(Value::as_str) {
        Some("evidence") => {
            let root = closed(
                value,
                path,
                &["kind", "provider", "artifact", "digest"],
                &["kind", "provider", "artifact", "digest"],
            )?;
            let as_reference = object(vec![
                (
                    "provider",
                    root.get("provider").cloned().unwrap_or(Value::Null),
                ),
                (
                    "artifact",
                    root.get("artifact").cloned().unwrap_or(Value::Null),
                ),
                ("digest", root.get("digest").cloned().unwrap_or(Value::Null)),
            ]);
            check_evidence_reference(Some(&as_reference), path)
        }
        Some("claim") => {
            let root = closed(
                value,
                path,
                &["kind", "provider", "claim", "revision", "digest"],
                &["kind", "provider", "claim", "revision", "digest"],
            )?;
            identifier_of(root.get("provider"), &format!("{path}/provider"))?;
            identifier_of(root.get("claim"), &format!("{path}/claim"))?;
            integer_of(root.get("revision"), &format!("{path}/revision"), 1)?;
            digest_of(root.get("digest"), &format!("{path}/digest"))?;
            Ok(())
        }
        _ => Err(invalid(&format!("{path}/kind"), "not evidence or claim")),
    }
}

/// A claim `basis` or an evaluation `target`.
pub fn check_basis(value: Option<&Value>, path: &str) -> Checked<()> {
    let basis = closed(
        value,
        path,
        &["repositories", "environment", "build", "completeness"],
        &["repositories", "completeness"],
    )?;
    let repositories = array_of(
        basis.get("repositories"),
        &format!("{path}/repositories"),
        64,
    )?;
    for (index, repository) in repositories.iter().enumerate() {
        let at = format!("{path}/repositories/{index}");
        let repository = closed(
            Some(repository),
            &at,
            &["id", "tree", "dirty"],
            &["id", "tree"],
        )?;
        text_of(repository.get("id"), &format!("{at}/id"), 1, 128)?;
        text_of(repository.get("tree"), &format!("{at}/tree"), 1, 128)?;
        if let Some(dirty) = repository.get("dirty") {
            let dirty = closed(
                Some(dirty),
                &format!("{at}/dirty"),
                &["snapshot_digest"],
                &["snapshot_digest"],
            )?;
            digest_of(
                dirty.get("snapshot_digest"),
                &format!("{at}/dirty/snapshot_digest"),
            )?;
        }
    }
    if let Some(environment) = basis.get("environment") {
        text_of(Some(environment), &format!("{path}/environment"), 1, 256)?;
    }
    if let Some(build) = basis.get("build") {
        text_of(Some(build), &format!("{path}/build"), 1, 256)?;
    }
    one_of(
        basis.get("completeness"),
        &format!("{path}/completeness"),
        &["complete", "partial"],
    )?;
    Ok(())
}

fn check_condition(value: Option<&Value>, path: &str) -> Checked<()> {
    match value.and_then(|v| v.get("kind")).and_then(Value::as_str) {
        Some("repository_tree" | "dirty_snapshot") => {
            let condition = closed(
                value,
                path,
                &["condition_id", "kind", "repository", "expected"],
                &["condition_id", "kind", "repository", "expected"],
            )?;
            identifier_of(
                condition.get("condition_id"),
                &format!("{path}/condition_id"),
            )?;
            text_of(
                condition.get("repository"),
                &format!("{path}/repository"),
                1,
                128,
            )?;
            text_of(
                condition.get("expected"),
                &format!("{path}/expected"),
                1,
                256,
            )?;
            Ok(())
        }
        Some("environment_digest") => {
            let condition = closed(
                value,
                path,
                &["condition_id", "kind", "expected"],
                &["condition_id", "kind", "expected"],
            )?;
            identifier_of(
                condition.get("condition_id"),
                &format!("{path}/condition_id"),
            )?;
            text_of(
                condition.get("expected"),
                &format!("{path}/expected"),
                1,
                256,
            )?;
            Ok(())
        }
        _ => Err(invalid(&format!("{path}/kind"), "not a condition kind")),
    }
}

/// The content members `propose` and `revise` share, with `supersedes` for
/// `revise` (KNOWLEDGE section 3).
pub fn check_claim_payload(payload: &Value, revise: bool) -> Checked<()> {
    let mut allowed = vec![
        "plane",
        "statement",
        "scope",
        "validity",
        "basis",
        "support",
        "derivation",
        "dependencies",
        "conditions",
        "health",
    ];
    let mut required = vec!["plane", "statement", "scope", "support", "derivation"];
    if revise {
        allowed.push("supersedes");
        required.push("supersedes");
    }
    let payload = closed(Some(payload), "/payload", &allowed, &required)?;

    let plane = one_of(
        payload.get("plane"),
        "/payload/plane",
        &["normative", "observed", "interpretive"],
    )?;

    let statement = closed(
        payload.get("statement"),
        "/payload/statement",
        &["subject", "predicate", "value", "cardinality"],
        &["subject", "predicate", "value", "cardinality"],
    )?;
    let subject = closed(
        statement.get("subject"),
        "/payload/statement/subject",
        &["kind", "id"],
        &["kind", "id"],
    )?;
    match subject.get("kind") {
        Some(Value::String(kind)) if is_dotted_name(kind) => {}
        _ => {
            return Err(invalid(
                "/payload/statement/subject/kind",
                "not a subject kind",
            ));
        }
    }
    identifier_of(subject.get("id"), "/payload/statement/subject/id")?;
    text_of(
        statement.get("predicate"),
        "/payload/statement/predicate",
        1,
        128,
    )?;
    if let Some(value) = statement.get("value")
        && cbr_encoding::to_canonical(value).len() > MAX_VALUE_BYTES
    {
        return Err(invalid(
            "/payload/statement/value",
            "a statement value is at most 4096 bytes of canonical JSON",
        ));
    }
    one_of(
        statement.get("cardinality"),
        "/payload/statement/cardinality",
        &["single", "multiple", "unknown"],
    )?;

    let scope = closed(
        payload.get("scope"),
        "/payload/scope",
        &["id", "qualifiers"],
        &["id", "qualifiers"],
    )?;
    identifier_of(scope.get("id"), "/payload/scope/id")?;
    let Some(Value::Object(qualifiers)) = scope.get("qualifiers") else {
        return Err(invalid("/payload/scope/qualifiers", "not an object"));
    };
    if qualifiers.len() > 32 {
        return Err(invalid(
            "/payload/scope/qualifiers",
            "more than 32 qualifiers",
        ));
    }
    for (name, value) in qualifiers {
        let at = format!("/payload/scope/qualifiers/{name}");
        if !(1..=64).contains(&name.chars().count()) {
            return Err(invalid(&at, "a qualifier name is 1 to 64 characters"));
        }
        text_of(Some(value), &at, 1, 256)?;
    }

    if let Some(validity) = payload.get("validity") {
        let validity = closed(Some(validity), "/payload/validity", &["from", "until"], &[])?;
        let from = validity
            .get("from")
            .map(|v| instant_of(Some(v), "/payload/validity/from"))
            .transpose()?;
        let until = validity
            .get("until")
            .map(|v| instant_of(Some(v), "/payload/validity/until"))
            .transpose()?;
        // Instants of this fixed shape order as strings.
        if let (Some(from), Some(until)) = (from, until)
            && from >= until
        {
            return Err(invalid(
                "/payload/validity",
                "from must be before until: the interval is empty or inverted",
            ));
        }
    }

    if let Some(basis) = payload.get("basis") {
        check_basis(Some(basis), "/payload/basis")?;
    }

    // Each entry is validated first, so an invalid entry is reported at its
    // own path before any duplicate (KNOWLEDGE section 3).
    let support = array_of(payload.get("support"), "/payload/support", 64)?;
    for (index, entry) in support.iter().enumerate() {
        let at = format!("/payload/support/{index}");
        let entry = closed(
            Some(entry),
            &at,
            &["support_id", "evidence", "ancestry"],
            &["support_id", "evidence", "ancestry"],
        )?;
        identifier_of(entry.get("support_id"), &format!("{at}/support_id"))?;
        check_evidence_reference(entry.get("evidence"), &format!("{at}/evidence"))?;
        let ancestry = closed(
            entry.get("ancestry"),
            &format!("{at}/ancestry"),
            &["completeness", "roots"],
            &["completeness", "roots"],
        )?;
        let completeness = one_of(
            ancestry.get("completeness"),
            &format!("{at}/ancestry/completeness"),
            &["complete", "partial", "unknown"],
        )?;
        let roots = array_of(ancestry.get("roots"), &format!("{at}/ancestry/roots"), 32)?;
        for (r, root) in roots.iter().enumerate() {
            check_root(Some(root), &format!("{at}/ancestry/roots/{r}"))?;
        }
        if (completeness == "unknown") != roots.is_empty() {
            return Err(invalid(
                &format!("{at}/ancestry/roots"),
                "unknown ancestry declares no roots; complete or partial ancestry declares at least one",
            ));
        }
    }
    let mut ids: Vec<&str> = Vec::new();
    for entry in support {
        let id = entry
            .get("support_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if ids.contains(&id) {
            return Err(invalid("/payload/support", "duplicate support_id"));
        }
        ids.push(id);
    }

    let derivation = closed(
        payload.get("derivation"),
        "/payload/derivation",
        &["kind", "record", "inputs"],
        &["kind", "inputs"],
    )?;
    one_of(
        derivation.get("kind"),
        "/payload/derivation/kind",
        &["deterministic", "model_assisted", "human"],
    )?;
    if let Some(record) = derivation.get("record") {
        check_evidence_reference(Some(record), "/payload/derivation/record")?;
    }
    for (index, input) in array_of(derivation.get("inputs"), "/payload/derivation/inputs", 32)?
        .iter()
        .enumerate()
    {
        check_evidence_reference(Some(input), &format!("/payload/derivation/inputs/{index}"))?;
    }

    if let Some(dependencies) = payload.get("dependencies") {
        for (index, dependency) in array_of(Some(dependencies), "/payload/dependencies", 32)?
            .iter()
            .enumerate()
        {
            check_claim_reference(Some(dependency), &format!("/payload/dependencies/{index}"))?;
        }
    }

    if let Some(conditions) = payload.get("conditions") {
        let conditions = array_of(Some(conditions), "/payload/conditions", 32)?;
        for (index, condition) in conditions.iter().enumerate() {
            check_condition(Some(condition), &format!("/payload/conditions/{index}"))?;
        }
        let mut ids: Vec<&str> = Vec::new();
        for condition in conditions {
            let id = condition
                .get("condition_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if ids.contains(&id) {
                return Err(invalid("/payload/conditions", "duplicate condition_id"));
            }
            ids.push(id);
        }
    }

    if let Some(health) = payload.get("health") {
        one_of(
            Some(health),
            "/payload/health",
            &["healthy", "degraded", "failing", "unknown"],
        )?;
        if plane != "observed" {
            return Err(invalid(
                "/payload/health",
                "health is reported only by observed claims",
            ));
        }
    }

    if revise {
        let supersedes = closed(
            payload.get("supersedes"),
            "/payload/supersedes",
            &["revision", "digest"],
            &["revision", "digest"],
        )?;
        integer_of(
            supersedes.get("revision"),
            "/payload/supersedes/revision",
            1,
        )?;
        digest_of(supersedes.get("digest"), "/payload/supersedes/digest")?;
    }
    Ok(())
}

pub fn check_authority_payload(payload: &Value) -> Checked<()> {
    let payload = closed(Some(payload), "/payload", &["authority"], &["authority"])?;
    identifier_of(payload.get("authority"), "/payload/authority")?;
    Ok(())
}

pub fn check_decision_payload(payload: &Value) -> Checked<()> {
    let payload = closed(
        Some(payload),
        "/payload",
        &[
            "claim",
            "decision",
            "permitted_use",
            "supersedes_decision",
            "validation_basis",
            "rationale",
        ],
        &["claim", "decision", "validation_basis", "rationale"],
    )?;
    check_claim_reference(payload.get("claim"), "/payload/claim")?;
    let decision = one_of(
        payload.get("decision"),
        "/payload/decision",
        &["accepted_for_use", "rejected", "superseded"],
    )?;
    match (decision, payload.get("permitted_use")) {
        ("accepted_for_use", Some(used)) => {
            one_of(
                Some(used),
                "/payload/permitted_use",
                &["binding", "evidence", "hypothesis", "reference"],
            )?;
        }
        ("accepted_for_use", None) => {
            return Err(invalid(
                "/payload/permitted_use",
                "required for accepted_for_use",
            ));
        }
        (_, Some(_)) => {
            return Err(invalid(
                "/payload/permitted_use",
                "present only for accepted_for_use",
            ));
        }
        (_, None) => {}
    }
    if let Some(supersedes) = payload.get("supersedes_decision") {
        identifier_of(Some(supersedes), "/payload/supersedes_decision")?;
    }
    let basis = closed(
        payload.get("validation_basis"),
        "/payload/validation_basis",
        &["evidence", "receipts", "target"],
        &["evidence", "receipts"],
    )?;
    for (index, evidence) in array_of(
        basis.get("evidence"),
        "/payload/validation_basis/evidence",
        32,
    )?
    .iter()
    .enumerate()
    {
        check_evidence_reference(
            Some(evidence),
            &format!("/payload/validation_basis/evidence/{index}"),
        )?;
    }
    for (index, receipt) in array_of(
        basis.get("receipts"),
        "/payload/validation_basis/receipts",
        32,
    )?
    .iter()
    .enumerate()
    {
        let at = format!("/payload/validation_basis/receipts/{index}");
        let receipt = closed(
            Some(receipt),
            &at,
            &["provider", "receipt", "artifact"],
            &["provider", "receipt", "artifact"],
        )?;
        identifier_of(receipt.get("provider"), &format!("{at}/provider"))?;
        identifier_of(receipt.get("receipt"), &format!("{at}/receipt"))?;
        check_evidence_reference(receipt.get("artifact"), &format!("{at}/artifact"))?;
    }
    if let Some(target) = basis.get("target") {
        check_basis(Some(target), "/payload/validation_basis/target")?;
    }
    text_of(payload.get("rationale"), "/payload/rationale", 1, 4096)?;
    Ok(())
}

pub fn check_conflict_open_payload(payload: &Value) -> Checked<()> {
    let payload = closed(
        Some(payload),
        "/payload",
        &["revisions", "note"],
        &["revisions"],
    )?;
    let revisions = array_of(payload.get("revisions"), "/payload/revisions", 2)?;
    if revisions.len() != 2 {
        return Err(invalid("/payload/revisions", "exactly two revisions"));
    }
    for (index, reference) in revisions.iter().enumerate() {
        check_claim_reference(Some(reference), &format!("/payload/revisions/{index}"))?;
    }
    if canonical(&revisions[0]) == canonical(&revisions[1]) {
        return Err(invalid(
            "/payload/revisions",
            "a conflict names two different revisions",
        ));
    }
    if let Some(note) = payload.get("note") {
        text_of(Some(note), "/payload/note", 1, 4096)?;
    }
    Ok(())
}

pub fn check_conflict_resolve_payload(payload: &Value) -> Checked<()> {
    let payload = closed(
        Some(payload),
        "/payload",
        &["resolution", "selected", "rationale"],
        &["resolution", "rationale"],
    )?;
    one_of(
        payload.get("resolution"),
        "/payload/resolution",
        &[
            "select",
            "narrow_scope",
            "reject_support",
            "supersede",
            "request_observation",
            "not_a_conflict",
        ],
    )?;
    if let Some(selected) = payload.get("selected") {
        check_claim_reference(Some(selected), "/payload/selected")?;
    }
    text_of(payload.get("rationale"), "/payload/rationale", 1, 4096)?;
    Ok(())
}

pub fn check_evaluate_payload(payload: &Value) -> Checked<()> {
    let payload = closed(
        Some(payload),
        "/payload",
        &["claim", "target"],
        &["claim", "target"],
    )?;
    check_claim_reference(payload.get("claim"), "/payload/claim")?;
    check_basis(payload.get("target"), "/payload/target")?;
    Ok(())
}

pub fn check_inspect_payload(payload: &Value) -> Checked<(String, Option<i64>)> {
    let payload = closed(
        Some(payload),
        "/payload",
        &["claim", "revision"],
        &["claim"],
    )?;
    let claim = identifier_of(payload.get("claim"), "/payload/claim")?.to_string();
    let revision = payload
        .get("revision")
        .map(|r| integer_of(Some(r), "/payload/revision", 1))
        .transpose()?;
    Ok((claim, revision))
}

pub fn check_history_payload(payload: &Value) -> Checked<String> {
    let payload = closed(Some(payload), "/payload", &["claim"], &["claim"])?;
    Ok(identifier_of(payload.get("claim"), "/payload/claim")?.to_string())
}

pub fn check_authority_get_payload(payload: &Value) -> Checked<String> {
    let payload = closed(Some(payload), "/payload", &["scope"], &["scope"])?;
    Ok(identifier_of(payload.get("scope"), "/payload/scope")?.to_string())
}

// ---- revisions and references ---------------------------------------------

/// The revision record (KNOWLEDGE section 3). Absent `validity`, `basis` and
/// `supersedes` are recorded as `null`, absent `dependencies` and `conditions`
/// as `[]`, absent `health` as `not_applicable`; nothing is filled from the
/// recorded time. The producer is always the session principal.
pub fn build_record(
    provider: &str,
    claim: &str,
    revision: i64,
    producer: &str,
    payload: &Value,
) -> Value {
    let or = |name: &str, absent: Value| payload.get(name).cloned().unwrap_or(absent);
    object(vec![
        ("format", string(CLAIM_FORMAT)),
        ("provider", string(provider)),
        ("claim", string(claim)),
        ("revision", Value::Int(revision)),
        ("producer", string(producer)),
        ("plane", or("plane", Value::Null)),
        ("statement", or("statement", Value::Null)),
        ("scope", or("scope", Value::Null)),
        ("validity", or("validity", Value::Null)),
        ("basis", or("basis", Value::Null)),
        ("support", or("support", Value::Array(vec![]))),
        ("derivation", or("derivation", Value::Null)),
        ("dependencies", or("dependencies", Value::Array(vec![]))),
        ("conditions", or("conditions", Value::Array(vec![]))),
        ("health", or("health", string("not_applicable"))),
        ("supersedes", or("supersedes", Value::Null)),
    ])
}

/// The canonical `sha256` digest of a revision record, which any reader can
/// recompute from what `knowledge.claim.inspect` returns.
pub fn record_digest(record: &Value) -> String {
    cbr_encoding::digest_canonical(record)
}

pub fn reference(provider: &str, claim: &str, revision: i64, digest: &str) -> Value {
    object(vec![
        ("provider", string(provider)),
        ("claim", string(claim)),
        ("revision", Value::Int(revision)),
        ("digest", string(digest)),
    ])
}

/// A validated reference's four members.
pub struct Reference<'a> {
    pub provider: &'a str,
    pub claim: &'a str,
    pub revision: i64,
    pub digest: &'a str,
}

pub fn reference_parts(value: &Value) -> Reference<'_> {
    Reference {
        provider: value
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        claim: value
            .get("claim")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        revision: match value.get("revision") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        },
        digest: value
            .get("digest")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    }
}

/// Exact reference identity: provider, claim, revision and digest.
pub fn same_reference(a: &Value, b: &Value) -> bool {
    let (a, b) = (reference_parts(a), reference_parts(b));
    a.provider == b.provider
        && a.claim == b.claim
        && a.revision == b.revision
        && a.digest == b.digest
}

// ---- support (section 5) ---------------------------------------------------

/// `(class, unknown_ancestry)` for a revision's support entries.
///
/// Roots are compared as whole references in canonical form, so member order
/// in the input cannot make one origin look like two. Two roots with different
/// references and an equal digest are **not known to differ**: equal bytes may
/// be one origin copied.
pub fn support_class(support: &[Value]) -> (&'static str, Vec<String>) {
    struct Entry {
        roots: Vec<String>,
        digests: Vec<String>,
        completeness: String,
    }
    let unknown_ancestry: Vec<String> = support
        .iter()
        .filter(|entry| completeness_of(entry) == "unknown")
        .filter_map(|entry| entry.get("support_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    if support.is_empty() {
        return ("unsupported", unknown_ancestry);
    }
    let entries: Vec<Entry> = support
        .iter()
        .map(|entry| {
            // A wrapper is not an origin: only declared roots count.
            let roots = entry
                .get("ancestry")
                .and_then(|a| a.get("roots"))
                .and_then(Value::as_array)
                .unwrap_or_default();
            Entry {
                roots: roots.iter().map(canonical).collect(),
                digests: roots
                    .iter()
                    .filter_map(|r| r.get("digest").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect(),
                completeness: completeness_of(entry).to_string(),
            }
        })
        .collect();

    #[derive(PartialEq)]
    enum Pair {
        Shared,
        Disjoint,
        Undetermined,
    }
    let relation = |a: &Entry, b: &Entry| {
        if a.roots.iter().any(|root| b.roots.contains(root)) {
            return Pair::Shared;
        }
        let equal_digest = a.digests.iter().any(|d| b.digests.contains(d));
        if a.completeness == "complete" && b.completeness == "complete" && !equal_digest {
            Pair::Disjoint
        } else {
            Pair::Undetermined
        }
    };
    let mut any_disjoint = false;
    let mut all_shared = true;
    for (i, a) in entries.iter().enumerate() {
        for b in &entries[i + 1..] {
            match relation(a, b) {
                Pair::Disjoint => {
                    any_disjoint = true;
                    all_shared = false;
                }
                Pair::Undetermined => all_shared = false,
                Pair::Shared => {}
            }
        }
    }
    let any_unknown = entries.iter().any(|e| e.completeness == "unknown");
    // The rules in order; the first that matches applies.
    if any_unknown && !any_disjoint {
        return ("undetermined", unknown_ancestry);
    }
    let common_root = entries[0]
        .roots
        .iter()
        .any(|root| entries.iter().all(|e| e.roots.contains(root)));
    if common_root {
        return ("single_lineage", unknown_ancestry);
    }
    if any_disjoint {
        return ("multiple_lineages", unknown_ancestry);
    }
    if entries.len() >= 2 && all_shared {
        return ("overlapping_lineages", unknown_ancestry);
    }
    ("undetermined", unknown_ancestry)
}

fn completeness_of(entry: &Value) -> &str {
    entry
        .get("ancestry")
        .and_then(|a| a.get("completeness"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

/// What the provider observes of one support artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    Available,
    Unavailable,
    Purged,
    Unknown,
}

/// The availability facet from per-artifact observations (section 5).
pub fn availability(observed: &[Observed]) -> Value {
    let count = |which: Observed| observed.iter().filter(|o| **o == which).count() as i64;
    let (available, unavailable, purged, unknown) = (
        count(Observed::Available),
        count(Observed::Unavailable),
        count(Observed::Purged),
        count(Observed::Unknown),
    );
    let total = observed.len() as i64;
    let state = if total == 0 {
        "unknown"
    } else if available == total {
        "complete"
    } else if available > 0 {
        "partial"
    } else if unknown > 0 {
        "unknown"
    } else if purged == total {
        "purged"
    } else {
        "unavailable"
    };
    object(vec![
        ("state", string(state)),
        (
            "counts",
            object(vec![
                ("available", Value::Int(available)),
                ("unavailable", Value::Int(unavailable)),
                ("purged", Value::Int(purged)),
                ("unknown", Value::Int(unknown)),
            ]),
        ),
    ])
}

// ---- conflicts (section 7) -------------------------------------------------

/// A structural comparison that did not separate the two revisions.
#[derive(Debug, PartialEq, Eq)]
pub struct Comparison {
    pub kind: &'static str,
    pub status: &'static str,
    pub uncertain: Vec<&'static str>,
}

/// Compare two revision records in the order of section 7. `Err` is the
/// refusal reason of the first check that separates them.
pub fn compare(a: &Value, b: &Value) -> Result<Comparison, &'static str> {
    let member = |record: &Value, path: &[&str]| -> Value {
        let mut current = record.clone();
        for name in path {
            current = current.get(name).cloned().unwrap_or(Value::Null);
        }
        current
    };
    let same = |x: &Value, y: &Value| canonical(x) == canonical(y);

    if !same(
        &member(a, &["statement", "subject"]),
        &member(b, &["statement", "subject"]),
    ) {
        return Err("subject_differs");
    }
    if !same(
        &member(a, &["statement", "predicate"]),
        &member(b, &["statement", "predicate"]),
    ) {
        return Err("predicate_differs");
    }
    if !same(&member(a, &["scope", "id"]), &member(b, &["scope", "id"])) {
        return Err("scope_differs");
    }
    let mut uncertain = Vec::new();

    // 4. Qualifiers present in both have equal values.
    let (qa, qb) = (
        member(a, &["scope", "qualifiers"]),
        member(b, &["scope", "qualifiers"]),
    );
    if let (Value::Object(ma), Value::Object(_)) = (&qa, &qb) {
        for (name, value) in ma {
            if let Some(other) = qb.get(name)
                && !same(value, other)
            {
                return Err("qualifiers_disjoint");
            }
        }
    }
    if !same(&qa, &qb) {
        uncertain.push("qualifiers");
    }

    // 5. Repositories present in both have equal trees; environment and build,
    // where both are present, are equal.
    let (ba, bb) = (member(a, &["basis"]), member(b, &["basis"]));
    let trees = |basis: &Value| -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = basis
            .get("repositories")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .map(|r| {
                (
                    r.get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    r.get("tree")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                )
            })
            .collect();
        out.sort();
        out
    };
    let basis_certain = if ba.is_object() && bb.is_object() {
        let (ta, tb) = (trees(&ba), trees(&bb));
        for (id, tree) in &ta {
            if tb.iter().any(|(other, t)| other == id && t != tree) {
                return Err("basis_disjoint");
            }
        }
        for name in ["environment", "build"] {
            if let (Some(x), Some(y)) = (ba.get(name), bb.get(name))
                && !same(x, y)
            {
                return Err("basis_disjoint");
            }
        }
        ta == tb
            && same(
                &member(&ba, &["environment"]),
                &member(&bb, &["environment"]),
            )
            && same(&member(&ba, &["build"]), &member(&bb, &["build"]))
            && member(&ba, &["completeness"]).as_str() == Some("complete")
            && member(&bb, &["completeness"]).as_str() == Some("complete")
    } else {
        false
    };
    if !basis_certain {
        uncertain.push("basis");
    }

    // 6. Validity intervals, where both are fully known, overlap.
    let known = |record: &Value| -> Option<(String, String)> {
        let validity = member(record, &["validity"]);
        Some((
            validity.get("from")?.as_str()?.to_string(),
            validity.get("until")?.as_str()?.to_string(),
        ))
    };
    match (known(a), known(b)) {
        (Some((from_a, until_a)), Some((from_b, until_b))) => {
            if !(from_a < until_b && from_b < until_a) {
                return Err("validity_disjoint");
            }
        }
        _ => uncertain.push("validity"),
    }

    // 7. Neither cardinality is `multiple`.
    let (ca, cb) = (
        member(a, &["statement", "cardinality"]),
        member(b, &["statement", "cardinality"]),
    );
    if ca.as_str() == Some("multiple") || cb.as_str() == Some("multiple") {
        return Err("multiple_values_permitted");
    }
    if !(ca.as_str() == Some("single") && cb.as_str() == Some("single")) {
        uncertain.push("cardinality");
    }

    // 8. Values differ.
    if same(
        &member(a, &["statement", "value"]),
        &member(b, &["statement", "value"]),
    ) {
        return Err("values_equal");
    }

    let planes = (
        member(a, &["plane"])
            .as_str()
            .unwrap_or_default()
            .to_string(),
        member(b, &["plane"])
            .as_str()
            .unwrap_or_default()
            .to_string(),
    );
    let drift = matches!(
        (planes.0.as_str(), planes.1.as_str()),
        ("normative", "observed") | ("observed", "normative")
    );
    if !drift && planes.0 != planes.1 {
        uncertain.push("plane");
    }
    Ok(Comparison {
        kind: if drift { "drift" } else { "conflict" },
        status: if uncertain.is_empty() {
            "demonstrated"
        } else {
            "potential"
        },
        uncertain,
    })
}

// ---- applicability (section 8) ---------------------------------------------

/// The finding for each condition of a record against a target, with only
/// `kinds` implemented by the evaluator.
pub fn condition_findings(record: &Value, target: &Value, kinds: &[String]) -> Vec<Value> {
    let repository = |name: &Value| -> Option<Value> {
        target
            .get("repositories")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .find(|r| r.get("id") == Some(name))
            .cloned()
    };
    let mut findings = Vec::new();
    for condition in record
        .get("conditions")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        let kind = condition
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let finding = if !kinds.iter().any(|k| k == kind) {
            "unsupported"
        } else {
            let name = condition.get("repository").cloned().unwrap_or(Value::Null);
            let observed: Option<Value> = match kind {
                "repository_tree" => repository(&name).and_then(|r| r.get("tree").cloned()),
                "dirty_snapshot" => repository(&name).and_then(|r| {
                    r.get("dirty")
                        .and_then(|d| d.get("snapshot_digest"))
                        .cloned()
                }),
                "environment_digest" => target.get("environment").cloned(),
                _ => None,
            };
            match observed {
                None => "missing_anchor",
                Some(value) if Some(&value) == condition.get("expected") => "match",
                Some(_) => "mismatch",
            }
        };
        findings.push(object(vec![
            (
                "condition_id",
                condition
                    .get("condition_id")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            ("finding", string(finding)),
        ]));
    }
    findings
}

/// The result of an evaluation from its findings: the first rule that applies
/// wins (section 8).
pub fn result_of(findings: &[Value]) -> &'static str {
    let any = |names: &[&str]| {
        findings.iter().any(|f| {
            f.get("finding")
                .and_then(Value::as_str)
                .is_some_and(|finding| names.contains(&finding))
        })
    };
    if any(&["mismatch"]) {
        "invalid_for_target"
    } else if any(&["missing_anchor", "unsupported"]) {
        "unknown"
    } else if any(&["unchecked"]) {
        "needs_check"
    } else if findings.is_empty() {
        "unknown"
    } else {
        "applicable"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Value {
        cbr_encoding::parse(text.as_bytes()).expect("test JSON")
    }

    fn evidence_root(id: &str, digest_byte: char) -> String {
        format!(
            r#"{{"kind":"evidence","provider":"cbr","artifact":{{"kind":"evidence.artifact","id":"{id}"}},"digest":"sha256:{}"}}"#,
            digest_byte.to_string().repeat(64)
        )
    }

    fn entry(support_id: &str, wrapper: char, completeness: &str, roots: &[String]) -> String {
        format!(
            r#"{{"support_id":"{support_id}","evidence":{{"provider":"cbr","artifact":{{"kind":"evidence.artifact","id":"w-{support_id}"}},"digest":"sha256:{}"}},"ancestry":{{"completeness":"{completeness}","roots":[{}]}}}}"#,
            wrapper.to_string().repeat(64),
            roots.join(",")
        )
    }

    fn class(entries: &[String]) -> &'static str {
        let support = parse(&format!("[{}]", entries.join(",")));
        support_class(support.as_array().unwrap()).0
    }

    /// PROTOCOL-PIN section 3's control. One captured artifact `C`, two
    /// derivations over it producing `D1` and `D2`, a claim supported by both,
    /// each declaring `C` as its complete root: one lineage, because two model
    /// passes over the same log are not two sources.
    #[test]
    fn two_derivations_over_one_captured_root_are_a_single_lineage() {
        let c = evidence_root("captured-log", 'c');
        let honest = [
            entry("d1", '1', "complete", std::slice::from_ref(&c)),
            entry("d2", '2', "complete", std::slice::from_ref(&c)),
        ];
        assert_eq!(class(&honest), "single_lineage");

        // The mistake the control exists to catch: each derivation listing
        // itself as its own root. Two complete, disjoint roots satisfy rule 4
        // and the claim falsely reads as independently corroborated.
        let self_rooted = [
            entry("d1", '1', "complete", &[evidence_root("w-d1", '1')]),
            entry("d2", '2', "complete", &[evidence_root("w-d2", '2')]),
        ];
        assert_eq!(class(&self_rooted), "multiple_lineages");
    }

    #[test]
    fn support_classes_follow_the_rules_in_order() {
        let (a, b, c) = (
            evidence_root("a", 'a'),
            evidence_root("b", 'b'),
            evidence_root("c", 'c'),
        );
        assert_eq!(class(&[]), "unsupported");
        assert_eq!(
            class(&[
                entry("e1", '1', "complete", &[a.clone(), b.clone()]),
                entry("e2", '2', "complete", &[b.clone(), c.clone()]),
                entry("e3", '3', "complete", &[a.clone(), c.clone()]),
            ]),
            "overlapping_lineages"
        );
        assert_eq!(
            class(&[
                entry("e1", '1', "complete", std::slice::from_ref(&a)),
                entry("e2", '2', "partial", std::slice::from_ref(&b)),
            ]),
            "undetermined"
        );
        assert_eq!(
            class(&[
                entry("e1", '1', "complete", std::slice::from_ref(&a)),
                entry("e2", '2', "unknown", &[]),
            ]),
            "undetermined"
        );
        // Equal digests under different references are not known to differ.
        assert_eq!(
            class(&[
                entry("e1", '1', "complete", &[evidence_root("copy-1", 'f')]),
                entry("e2", '2', "complete", &[evidence_root("copy-2", 'f')]),
            ]),
            "undetermined"
        );
        // A root's member order does not make it a different root.
        let reordered = r#"{"digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","artifact":{"id":"a","kind":"evidence.artifact"},"provider":"cbr","kind":"evidence"}"#;
        assert_eq!(
            class(&[
                entry("e1", '1', "complete", std::slice::from_ref(&a)),
                entry("e2", '2', "complete", &[reordered.to_string()]),
            ]),
            "single_lineage"
        );
    }

    #[test]
    fn a_digest_any_reader_recomputes_names_the_record_exactly() {
        let payload = parse(
            r#"{"plane":"observed","statement":{"subject":{"kind":"app.service","id":"billing"},"predicate":"queue","value":"v1","cardinality":"single"},"scope":{"id":"svc","qualifiers":{}},"support":[],"derivation":{"kind":"deterministic","inputs":[]}}"#,
        );
        let record = build_record("cbr", "c1", 1, "owner", &payload);
        assert_eq!(
            record.get("validity"),
            Some(&Value::Null),
            "unknown stays unknown"
        );
        assert_eq!(record.get("health"), Some(&string("not_applicable")));
        let digest = record_digest(&record);
        let reparsed = cbr_encoding::parse(&cbr_encoding::to_canonical(&record)).unwrap();
        assert_eq!(record_digest(&reparsed), digest);
        let changed = build_record("cbr", "c1", 1, "someone-else", &payload);
        assert_ne!(
            record_digest(&changed),
            digest,
            "the producer is part of the record"
        );
    }

    #[test]
    fn precedence_puts_an_observed_mismatch_above_everything_incomplete() {
        let finding = |f: &str| object(vec![("finding", string(f))]);
        assert_eq!(
            result_of(&[
                finding("mismatch"),
                finding("unsupported"),
                finding("unchecked")
            ]),
            "invalid_for_target"
        );
        assert_eq!(
            result_of(&[finding("match"), finding("missing_anchor")]),
            "unknown"
        );
        assert_eq!(
            result_of(&[finding("match"), finding("unchecked")]),
            "needs_check"
        );
        assert_eq!(result_of(&[]), "unknown");
        assert_eq!(result_of(&[finding("match")]), "applicable");
    }
}
