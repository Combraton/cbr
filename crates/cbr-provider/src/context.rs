//! The context profile's records and pure rules (`context/1`).
//!
//! A request, its job and its published packets are Core subjects, so
//! revisions, preconditions and events apply to them unchanged. Everything in
//! this module is a pure function of the records it is given; the provider
//! owns the command path, authorization, the store, the clock and the peers.
//!
//! What the rules protect:
//!
//! - **Satisfaction is decided by the item's check**, never by a section being
//!   present. A section for the wrong path, a citation of other bytes or
//!   content at a superseded authority revision leaves the item unmet.
//! - **Mandatory content is never dropped.** Required items that cannot fit
//!   the output capacity refuse the request with the size they need; only
//!   advisory content is omitted, and each omission says why.
//! - **The three limits stay separate.** An exhausted investigation budget, an
//!   omission for capacity and a passed deadline are three reasons, and none
//!   is ever reported as another.
//! - **A claim is carried as exactly the revision read**, with its digest
//!   recomputed; a label never promotes it, and only `invalid_for_target`
//!   makes its section stale (CONTEXT section 14).
//! - **Published facts never change.** Later changes are read-time facts
//!   beside them: supersession, invalidation and unverified items.

use cbr_encoding::Value;

use crate::envelope::is_dotted_name;
use crate::errors::ProtocolError;
use crate::knowledge::{
    Checked, array_of, check_claim_reference, closed, digest_of, identifier_of, instant_of,
    integer_of, one_of, text_of,
};
use crate::store::SubjectKey;

pub const REQUEST: &str = "context.request";
pub const JOB: &str = "context.job";
pub const PACKET: &str = "context.packet";
pub const PACKET_MEDIA_TYPE: &str = "application/vnd.combraton.context-packet+json";
/// Packet bytes for a request submitted without `context.claims`.
pub const FORMAT: &str = "combraton-context-packet/1";
/// Packet bytes for a request submitted under `context.claims`.
pub const CLAIMS_FORMAT: &str = "combraton-context-packet/2";
/// The provenance `compiler` of a packet prepared by the `context.script`
/// test control: it names what actually compiled the packet, which here is a
/// script and not CBR's retrieval.
pub const SCRIPT_COMPILER: &str = "cbr-context-script";
/// The `producer_id` of every packet artifact this provider seals.
pub const PRODUCER_ID: &str = "cbr-context";

pub const FEATURES: [&str; 7] = [
    "context.advisory",
    "context.required_before_start",
    "context.required_before_transition",
    "context.shared_jobs",
    "context.updates",
    "context.expand",
    "context.claims",
];

const OBLIGATIONS: [&str; 3] = [
    "advisory",
    "required_before_start",
    "required_before_transition",
];
const RELIANCES: [&str; 4] = ["binding", "evidence", "hypothesis", "reference"];

pub fn key(kind: &str, id: &str) -> SubjectKey {
    SubjectKey {
        kind: kind.into(),
        id: id.into(),
    }
}

// ---- values ----------------------------------------------------------------

static NULL: Value = Value::Null;

pub fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

pub fn string(text: &str) -> Value {
    Value::String(text.into())
}

pub fn subject(kind: &str, id: &str) -> Value {
    object(vec![("kind", string(kind)), ("id", string(id))])
}

pub fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("canonical form is UTF-8")
}

/// Equal as canonical JSON: member order never makes two values differ.
pub fn same(a: &Value, b: &Value) -> bool {
    cbr_encoding::to_canonical(a) == cbr_encoding::to_canonical(b)
}

/// The value at a path of member names, or `null`.
pub fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut current = value;
    for name in path {
        match current.get(name) {
            Some(next) => current = next,
            None => return &NULL,
        }
    }
    current
}

pub fn is_null(value: &Value) -> bool {
    matches!(value, Value::Null)
}

pub fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    at(value, path).as_str().unwrap_or_default()
}

pub fn int(value: &Value, path: &[&str]) -> i64 {
    match at(value, path) {
        Value::Int(number) => *number,
        _ => 0,
    }
}

pub fn list<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    at(value, path).as_array().unwrap_or_default()
}

pub fn set(target: &mut Value, name: &str, member: Value) {
    if let Value::Object(members) = target {
        match members.iter_mut().find(|(n, _)| n == name) {
            Some((_, slot)) => *slot = member,
            None => members.push((name.to_string(), member)),
        }
    }
}

pub fn remove(target: &mut Value, name: &str) {
    if let Value::Object(members) = target {
        members.retain(|(n, _)| n != name);
    }
}

fn member_mut<'a>(target: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    match target {
        Value::Object(members) => members
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, value)| value),
        _ => None,
    }
}

/// Append to an array member, creating it when absent.
pub fn push(target: &mut Value, name: &str, item: Value) {
    if member_mut(target, name).is_none() {
        set(target, name, Value::Array(Vec::new()));
    }
    if let Some(Value::Array(items)) = member_mut(target, name) {
        items.push(item);
    }
}

/// Set a member of an object member, creating the object when absent.
pub fn set_in(target: &mut Value, name: &str, key: &str, member: Value) {
    if !at(target, &[name]).is_object() {
        set(target, name, Value::Object(Vec::new()));
    }
    if let Some(inner) = member_mut(target, name) {
        set(inner, key, member);
    }
}

// ---- step 2: shapes and the rules a schema cannot state --------------------

fn invalid(path: &str, reason: &str) -> ProtocolError {
    ProtocolError::invalid_envelope(path, reason)
}

/// An evidence reference: `provider` is optional in a context request.
fn check_evidence_reference(value: Option<&Value>, path: &str) -> Checked<()> {
    let reference = closed(
        value,
        path,
        &["provider", "artifact", "digest"],
        &["artifact", "digest"],
    )?;
    if let Some(provider) = reference.get("provider") {
        identifier_of(Some(provider), &format!("{path}/provider"))?;
    }
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

fn check_item(value: &Value, path: &str) -> Checked<()> {
    let item = closed(
        Some(value),
        path,
        &[
            "item_id",
            "selector",
            "obligation",
            "transition",
            "reliance",
            "selected_by",
            "check",
        ],
        &[
            "item_id",
            "selector",
            "obligation",
            "reliance",
            "selected_by",
            "check",
        ],
    )?;
    identifier_of(item.get("item_id"), &format!("{path}/item_id"))?;
    let selector = closed(
        item.get("selector"),
        &format!("{path}/selector"),
        &["kind", "value"],
        &["kind", "value"],
    )?;
    text_of(
        selector.get("kind"),
        &format!("{path}/selector/kind"),
        1,
        64,
    )?;
    text_of(
        selector.get("value"),
        &format!("{path}/selector/value"),
        1,
        512,
    )?;
    let obligation = one_of(
        item.get("obligation"),
        &format!("{path}/obligation"),
        &OBLIGATIONS,
    )?;
    // A transition is named exactly for an obligation that has one.
    match (item.get("transition"), obligation) {
        (Some(transition), "required_before_transition") => {
            text_of(Some(transition), &format!("{path}/transition"), 1, 128)?;
        }
        (None, "required_before_transition") => {
            return Err(invalid(
                &format!("{path}/transition"),
                "required_before_transition names its transition",
            ));
        }
        (Some(_), _) => {
            return Err(invalid(
                &format!("{path}/transition"),
                "only required_before_transition names a transition",
            ));
        }
        (None, _) => {}
    }
    one_of(
        item.get("reliance"),
        &format!("{path}/reliance"),
        &RELIANCES,
    )?;
    identifier_of(item.get("selected_by"), &format!("{path}/selected_by"))?;
    let check_path = format!("{path}/check");
    match at(value, &["check", "kind"]).as_str() {
        Some("source_included") => {
            let check = closed(
                item.get("check"),
                &check_path,
                &["kind", "repository", "path"],
                &["kind", "repository", "path"],
            )?;
            text_of(
                check.get("repository"),
                &format!("{check_path}/repository"),
                1,
                128,
            )?;
            text_of(check.get("path"), &format!("{check_path}/path"), 1, 1024)?;
        }
        Some("evidence_included") => {
            let check = closed(
                item.get("check"),
                &check_path,
                &["kind", "evidence"],
                &["kind", "evidence"],
            )?;
            check_evidence_reference(check.get("evidence"), &format!("{check_path}/evidence"))?;
        }
        Some("authority_content_included") => {
            closed(item.get("check"), &check_path, &["kind"], &["kind"])?;
        }
        Some("claim_included") => {
            let check = closed(
                item.get("check"),
                &check_path,
                &["kind", "claim"],
                &["kind", "claim"],
            )?;
            check_claim_reference(check.get("claim"), &format!("{check_path}/claim"))?;
        }
        // "Understand the repository" is not a checkable item.
        _ => {
            return Err(invalid(
                &format!("{check_path}/kind"),
                "not a check kind: satisfaction must be decidable",
            ));
        }
    }
    Ok(())
}

fn check_basis(value: Option<&Value>, path: &str) -> Checked<()> {
    let basis = closed(
        value,
        path,
        &[
            "repositories",
            "environment",
            "configuration",
            "build",
            "completeness",
        ],
        &["repositories", "completeness"],
    )?;
    let repositories = array_of(
        basis.get("repositories"),
        &format!("{path}/repositories"),
        64,
    )?;
    if repositories.is_empty() {
        return Err(invalid(
            &format!("{path}/repositories"),
            "names at least one repository",
        ));
    }
    for (index, repository) in repositories.iter().enumerate() {
        let at_path = format!("{path}/repositories/{index}");
        let repository = closed(
            Some(repository),
            &at_path,
            &["id", "tree", "workspace", "dirty"],
            &["id", "tree", "workspace", "dirty"],
        )?;
        text_of(repository.get("id"), &format!("{at_path}/id"), 1, 128)?;
        text_of(repository.get("tree"), &format!("{at_path}/tree"), 1, 128)?;
        one_of(
            repository.get("workspace"),
            &format!("{at_path}/workspace"),
            &["clean", "dirty", "unknown"],
        )?;
        match repository.get("dirty") {
            Some(Value::Null) => {}
            dirty => {
                let dirty = closed(
                    dirty,
                    &format!("{at_path}/dirty"),
                    &["snapshot_digest"],
                    &["snapshot_digest"],
                )?;
                digest_of(
                    dirty.get("snapshot_digest"),
                    &format!("{at_path}/dirty/snapshot_digest"),
                )?;
            }
        }
    }
    for member in ["environment", "configuration", "build"] {
        if let Some(value) = basis.get(member) {
            text_of(Some(value), &format!("{path}/{member}"), 1, 256)?;
        }
    }
    one_of(
        basis.get("completeness"),
        &format!("{path}/completeness"),
        &["complete", "partial"],
    )?;
    Ok(())
}

fn check_limits(value: Option<&Value>, path: &str) -> Checked<()> {
    let limits = closed(
        value,
        path,
        &["deadline", "investigation", "output_capacity"],
        &["deadline", "investigation", "output_capacity"],
    )?;
    instant_of(limits.get("deadline"), &format!("{path}/deadline"))?;
    let investigation = closed(
        limits.get("investigation"),
        &format!("{path}/investigation"),
        &["units", "amount"],
        &["units", "amount"],
    )?;
    text_of(
        investigation.get("units"),
        &format!("{path}/investigation/units"),
        1,
        64,
    )?;
    integer_of(
        investigation.get("amount"),
        &format!("{path}/investigation/amount"),
        0,
    )?;
    let capacity = closed(
        limits.get("output_capacity"),
        &format!("{path}/output_capacity"),
        &["units", "amount"],
        &["units", "amount"],
    )?;
    one_of(
        capacity.get("units"),
        &format!("{path}/output_capacity/units"),
        &["bytes"],
    )?;
    integer_of(
        capacity.get("amount"),
        &format!("{path}/output_capacity/amount"),
        0,
    )?;
    Ok(())
}

/// `context.request.submit` (CONTEXT section 3), step 2: the schema, then the
/// rules it cannot state.
pub fn check_submit_payload(payload: &Value) -> Checked<()> {
    let payload = closed(
        Some(payload),
        "/payload",
        &[
            "consumer",
            "basis",
            "items",
            "fallback",
            "limits",
            "authority_content",
            "origin",
        ],
        &["consumer", "basis", "items", "limits"],
    )?;
    let consumer = closed(
        payload.get("consumer"),
        "/payload/consumer",
        &["task", "principal", "executor"],
        &["task", "principal"],
    )?;
    text_of(consumer.get("task"), "/payload/consumer/task", 1, 256)?;
    identifier_of(consumer.get("principal"), "/payload/consumer/principal")?;
    if let Some(executor) = consumer.get("executor") {
        identifier_of(Some(executor), "/payload/consumer/executor")?;
    }
    check_basis(payload.get("basis"), "/payload/basis")?;
    let items = array_of(payload.get("items"), "/payload/items", 256)?;
    if items.is_empty() {
        return Err(invalid("/payload/items", "names at least one item"));
    }
    for (index, item) in items.iter().enumerate() {
        check_item(item, &format!("/payload/items/{index}"))?;
    }
    if let Some(fallback) = payload.get("fallback") {
        one_of(
            Some(fallback),
            "/payload/fallback",
            &["proceed_with_gap", "wait_until_deadline"],
        )?;
    }
    check_limits(payload.get("limits"), "/payload/limits")?;
    let supplied = match payload.get("authority_content") {
        Some(value) => array_of(Some(value), "/payload/authority_content", 256)?,
        None => &[],
    };
    for (index, content) in supplied.iter().enumerate() {
        let path = format!("/payload/authority_content/{index}");
        let content = closed(
            Some(content),
            &path,
            &["item_id", "evidence", "authority_revision"],
            &["item_id", "evidence", "authority_revision"],
        )?;
        identifier_of(content.get("item_id"), &format!("{path}/item_id"))?;
        check_evidence_reference(content.get("evidence"), &format!("{path}/evidence"))?;
        integer_of(
            content.get("authority_revision"),
            &format!("{path}/authority_revision"),
            0,
        )?;
    }
    if let Some(origin) = payload.get("origin") {
        let origin = closed(
            Some(origin),
            "/payload/origin",
            &["initiator", "depth", "call_budget"],
            &["initiator", "depth", "call_budget"],
        )?;
        let initiator = closed(
            origin.get("initiator"),
            "/payload/origin/initiator",
            &["kind", "id"],
            &["kind", "id"],
        )?;
        if !initiator
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(is_dotted_name)
        {
            return Err(invalid(
                "/payload/origin/initiator/kind",
                "not a subject kind",
            ));
        }
        identifier_of(initiator.get("id"), "/payload/origin/initiator/id")?;
        integer_of(origin.get("depth"), "/payload/origin/depth", 0)?;
        integer_of(origin.get("call_budget"), "/payload/origin/call_budget", 0)?;
    }

    // A workspace that is not clean, with no dirty snapshot, is a commit-only
    // basis: it cannot describe the working tree completely.
    let commit_only = list(payload, &["basis", "repositories"])
        .iter()
        .any(|r| text(r, &["workspace"]) != "clean" && at(r, &["dirty"]) == &Value::Null);
    if commit_only && text(payload, &["basis", "completeness"]) == "complete" {
        return Err(invalid(
            "/payload/basis/completeness",
            "a commit-only basis for a workspace that is not clean is partial",
        ));
    }
    for (index, content) in supplied.iter().enumerate() {
        let item = text(content, &["item_id"]);
        if !items.iter().any(|i| text(i, &["item_id"]) == item) {
            return Err(invalid(
                &format!("/payload/authority_content/{index}/item_id"),
                "authority content names no item of this request",
            ));
        }
    }
    Ok(())
}

pub fn check_cancel_payload(payload: &Value) -> Checked<()> {
    closed(Some(payload), "/payload", &[], &[])?;
    Ok(())
}

pub fn check_request_inspect_payload(payload: &Value) -> Checked<String> {
    let payload = closed(Some(payload), "/payload", &["request"], &["request"])?;
    Ok(identifier_of(payload.get("request"), "/payload/request")?.to_string())
}

/// `context.packet.inspect`, or with `expand` `context.expand`: the packet,
/// the revision and, for expand, the citation.
pub fn check_packet_payload(payload: &Value, expand: bool) -> Checked<(String, i64, String)> {
    let (allowed, required): (&[&str], &[&str]) = if expand {
        (
            &["packet", "revision", "citation", "offset", "max_bytes"],
            &["packet", "revision", "citation"],
        )
    } else {
        (
            &["packet", "revision", "offset", "max_bytes"],
            &["packet", "revision"],
        )
    };
    let payload = closed(Some(payload), "/payload", allowed, required)?;
    let packet = identifier_of(payload.get("packet"), "/payload/packet")?.to_string();
    let revision = integer_of(payload.get("revision"), "/payload/revision", 1)?;
    let citation = if expand {
        identifier_of(payload.get("citation"), "/payload/citation")?.to_string()
    } else {
        String::new()
    };
    if let Some(offset) = payload.get("offset") {
        integer_of(Some(offset), "/payload/offset", 0)?;
    }
    if let Some(max) = payload.get("max_bytes") {
        match max {
            Value::Int(n) if (1..=16_777_216).contains(n) => {}
            _ => {
                return Err(invalid(
                    "/payload/max_bytes",
                    "not an integer from 1 to 16777216",
                ));
            }
        }
    }
    Ok((packet, revision, citation))
}

/// Step 3: the features a submit needs. Every obligation it uses is timing
/// semantics negotiated explicitly, and a claim check needs `context.claims`
/// (CONTEXT sections 1 and 14). The order is not significant.
pub fn needed_features(payload: &Value) -> Vec<String> {
    let items = list(payload, &["items"]);
    let mut features: Vec<String> = Vec::new();
    if items
        .iter()
        .any(|item| text(item, &["check", "kind"]) == "claim_included")
    {
        features.push("context.claims".into());
    }
    for item in items {
        let feature = format!("context.{}", text(item, &["obligation"]));
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    features
}

// ---- requests and jobs -----------------------------------------------------

pub fn is_required(item: &Value) -> bool {
    text(item, &["obligation"]) != "advisory"
}

/// A script step's name and argument: each step is a one-member object.
pub fn step(value: &Value) -> (&str, &Value) {
    match value {
        Value::Object(members) if members.len() == 1 => (members[0].0.as_str(), &members[0].1),
        _ => ("", &NULL),
    }
}

/// The content bytes the scripted sections for required items need: what a
/// request's mandatory items take from its output capacity (CONTEXT section
/// 12, "Mandatory size").
pub fn mandatory_size(script: &[Value], items: &[Value]) -> i64 {
    script
        .iter()
        .filter_map(|s| match step(s) {
            ("section", section) => Some(section),
            _ => None,
        })
        .filter(|section| {
            items.iter().any(|item| {
                is_required(item) && text(item, &["item_id"]) == text(section, &["item_id"])
            })
        })
        .map(|section| text(section, &["content"]).len() as i64)
        .sum()
}

/// A new request record. `updates` and `claims` are the submitting session's
/// features, fixed with the request: a later session cannot change what a
/// request was promised.
pub fn new_request(payload: &Value, submitted_by: &str, updates: bool, claims: bool) -> Value {
    let mut record = object(vec![
        ("consumer", at(payload, &["consumer"]).clone()),
        ("basis", at(payload, &["basis"]).clone()),
        ("items", at(payload, &["items"]).clone()),
        ("limits", at(payload, &["limits"]).clone()),
        (
            "fallback",
            payload
                .get("fallback")
                .cloned()
                .unwrap_or_else(|| string("proceed_with_gap")),
        ),
        (
            "authority_content",
            payload
                .get("authority_content")
                .cloned()
                .unwrap_or(Value::Array(Vec::new())),
        ),
        ("submitted_by", string(submitted_by)),
        ("updates", Value::Bool(updates)),
        ("claims", Value::Bool(claims)),
        ("packets", Value::Array(Vec::new())),
    ]);
    if let Some(origin) = payload.get("origin") {
        set(&mut record, "origin", origin.clone());
    }
    record
}

/// A new job for a request, following `script`.
pub fn new_job(principal: &str, payload: &Value, script: &[Value], request: &str) -> Value {
    object(vec![
        ("state", string("running")),
        ("principal", string(principal)),
        ("requests", Value::Array(vec![string(request)])),
        ("basis", at(payload, &["basis"]).clone()),
        ("items", at(payload, &["items"]).clone()),
        ("limits", at(payload, &["limits"]).clone()),
        ("script", Value::Array(script.to_vec())),
        ("cursor", Value::Int(0)),
        ("spent", Value::Int(0)),
        ("sections", Value::Array(Vec::new())),
        ("coverage", Value::Array(Vec::new())),
        ("unmet", Value::Object(Vec::new())),
        ("omissions", Value::Array(Vec::new())),
        ("corrections", Value::Object(Vec::new())),
        ("conditions", Value::Array(Vec::new())),
        ("published", Value::Bool(false)),
    ])
}

/// Whether a request may subscribe to an existing job (CONTEXT section 4): the
/// job is still running and has not published, and the request has the same
/// basis, the same items and the same access scope. Access scope is both the
/// submitting principal *and* the view its grant gave it: another principal
/// never joins a job, and neither does the same principal under a narrower
/// grant.
pub fn may_join(
    job: &Value,
    principal: &str,
    payload: &Value,
    view: &[String],
    claims: &[String],
    evidence: &[String],
) -> bool {
    let same_view = list(job, &["view"])
        .iter()
        .filter_map(Value::as_str)
        .eq(view.iter().map(String::as_str));
    let same_claims = list(job, &["readable_claims"])
        .iter()
        .filter_map(Value::as_str)
        .eq(claims.iter().map(String::as_str));
    let same_evidence = list(job, &["readable_evidence"])
        .iter()
        .filter_map(Value::as_str)
        .eq(evidence.iter().map(String::as_str));
    text(job, &["state"]) == "running"
        && at(job, &["published"]) != &Value::Bool(true)
        && text(job, &["principal"]) == principal
        && same(at(job, &["basis"]), at(payload, &["basis"]))
        && same(at(job, &["items"]), at(payload, &["items"]))
        // The same principal can hold two grants of different width. A job
        // compiled under the wider one has already read repositories, and
        // claims, the narrower request may not, so joining it would hand
        // them over.
        && same_view
        && same_claims
        // And evidence, since m5a: a job whose projection copied an
        // artifact's bytes into its packet has read what a narrower grant
        // may not.
        && same_evidence
}

// ---- preparation -----------------------------------------------------------

fn rank(reliance: &str) -> u8 {
    match reliance {
        "binding" => 4,
        "evidence" => 3,
        "hypothesis" => 2,
        "reference" => 1,
        _ => 0,
    }
}

/// A context basis as a Knowledge target (CONTEXT section 14): each
/// repository's id and tree, its dirty snapshot only when the basis gives
/// one, environment and build when present, and completeness. Workspace and
/// configuration have no Knowledge counterpart.
pub fn to_target(basis: &Value) -> Value {
    let repositories = list(basis, &["repositories"])
        .iter()
        .map(|r| {
            let mut out = object(vec![
                ("id", at(r, &["id"]).clone()),
                ("tree", at(r, &["tree"]).clone()),
            ]);
            if at(r, &["dirty"]).is_object() {
                set(&mut out, "dirty", at(r, &["dirty"]).clone());
            }
            out
        })
        .collect();
    let mut target = object(vec![
        ("repositories", Value::Array(repositories)),
        ("completeness", at(basis, &["completeness"]).clone()),
    ]);
    for member in ["environment", "build"] {
        if let Some(value) = basis.get(member) {
            set(&mut target, member, value.clone());
        }
    }
    target
}

/// Why a claim could not be carried.
pub const KNOWLEDGE_UNAVAILABLE: &str = "knowledge_unavailable";
pub const DIGEST_MISMATCH: &str = "claim_digest_mismatch";

/// The section snapshot of a claim from a `knowledge.claim.inspect` result,
/// after recomputing the record's digest (CONTEXT section 14). The second
/// value is the lineage's current revision, which the snapshot does not carry
/// and the read-time facts compare against.
///
/// The read must be of exactly the referenced revision: a result naming a
/// different revision or digest, or whose record does not digest to the
/// reference, is never carried.
pub fn claim_snapshot(
    read: Result<&Value, &'static str>,
    reference: &Value,
    basis: &Value,
) -> Result<(Value, i64), &'static str> {
    let found = read?;
    let recomputed = crate::knowledge::record_digest(at(found, &["record"]));
    if recomputed != text(reference, &["digest"])
        || !crate::knowledge::same_reference(at(found, &["reference"]), reference)
    {
        return Err(DIGEST_MISMATCH);
    }
    let target = to_target(basis);
    let applicability = list(found, &["applicability"])
        .iter()
        .find(|a| same(at(a, &["target"]), &target))
        .map_or_else(
            || object(vec![("result", string("unknown"))]),
            |a| {
                object(vec![
                    ("result", at(a, &["result"]).clone()),
                    ("evaluation", at(a, &["evaluation"]).clone()),
                ])
            },
        );
    let conflicts = list(found, &["conflicts"])
        .iter()
        .filter(|c| text(c, &["state"]) == "open")
        .map(|c| {
            object(vec![
                ("conflict", at(c, &["conflict"]).clone()),
                ("kind", at(c, &["kind"]).clone()),
                ("status", at(c, &["status"]).clone()),
            ])
        })
        .collect();
    let snapshot = object(vec![
        ("reference", at(found, &["reference"]).clone()),
        ("plane", at(found, &["record", "plane"]).clone()),
        ("reliance", at(found, &["reliance"]).clone()),
        ("applicability", applicability),
        ("conflicts", Value::Array(conflicts)),
        (
            "support",
            object(vec![("class", at(found, &["support", "class"]).clone())]),
        ),
    ]);
    Ok((snapshot, int(found, &["current_revision"])))
}

/// Why a claim snapshot does not satisfy an item's reliance, if it does not
/// (the table in CONTEXT section 14).
pub fn claim_shortfall(item: &Value, snapshot: &Value) -> Option<&'static str> {
    let state = text(snapshot, &["reliance", "state"]);
    let result = text(snapshot, &["applicability", "result"]);
    match text(item, &["reliance"]) {
        wanted @ ("binding" | "evidence") => {
            if state != "accepted_for_use"
                || rank(text(snapshot, &["reliance", "permitted_use"])) < rank(wanted)
            {
                Some("not_accepted")
            } else if result == "invalid_for_target" {
                Some("invalid_for_target")
            } else if result != "applicable" {
                Some("applicability_not_established")
            } else {
                None
            }
        }
        _ if state == "rejected" => Some("not_accepted"),
        _ => None,
    }
}

/// Apply what was read for a scripted section's claim to that section: its
/// snapshot, or the reason it cannot be carried. A section may keep the label
/// `binding` only for a claim accepted for binding use; only a claim invalid
/// for the target makes the section historical and stale.
pub fn attach_claim(section: &mut Value, read: Result<(Value, i64), &'static str>) {
    match read {
        Ok((snapshot, _)) => {
            let binding = text(&snapshot, &["reliance", "state"]) == "accepted_for_use"
                && text(&snapshot, &["reliance", "permitted_use"]) == "binding";
            if text(section, &["label"]) == "binding" && !binding {
                set(section, "label", string("hypothesis"));
            }
            if text(&snapshot, &["applicability", "result"]) == "invalid_for_target" {
                set(section, "historical", Value::Bool(true));
                set(section, "label", string("stale"));
            }
            set(section, "claim", snapshot);
        }
        Err(reason) => set(section, "claim_error", string(reason)),
    }
}

/// Which sections fit the output capacity, and every omission with its
/// reason (CONTEXT sections 3 and 5). A section whose claim could not be read
/// or verified is never carried. Capacity is reserved for the sections of
/// required items first, so they are always included and advisory or
/// unattached content fills only what remains, whatever order the sections
/// were prepared in; the refusal at submit is what keeps the reservation
/// within the capacity.
pub fn inclusion(record: &Value, job: &Value) -> (Vec<String>, Vec<Value>) {
    let items = list(record, &["items"]);
    let sections = list(job, &["sections"]);
    let carried = |section: &Value| section.get("claim_error").is_none();
    let size = |section: &Value| text(section, &["content"]).len() as i64;
    let mandatory = |section: &Value| {
        items.iter().any(|item| {
            is_required(item)
                && section.get("item_id").and_then(Value::as_str) == Some(text(item, &["item_id"]))
        })
    };
    let reserved: i64 = sections
        .iter()
        .filter(|s| carried(s) && mandatory(s))
        .map(&size)
        .sum();
    let mut remaining = int(record, &["limits", "output_capacity", "amount"]) - reserved;
    let mut included = Vec::new();
    let mut omissions: Vec<Value> = list(job, &["omissions"]).to_vec();
    for section in sections {
        let section_id = text(section, &["section_id"]);
        let omit = |reason: &str| {
            let mut omission = object(vec![
                ("section_id", string(section_id)),
                ("reason", string(reason)),
            ]);
            if let Some(item) = section.get("item_id") {
                set(&mut omission, "item_id", item.clone());
            }
            omission
        };
        if !carried(section) {
            omissions.push(omit("unavailable"));
        } else if mandatory(section) {
            included.push(section_id.to_string());
        } else if size(section) <= remaining {
            remaining -= size(section);
            included.push(section_id.to_string());
        } else {
            omissions.push(omit("output_capacity"));
        }
    }
    (included, omissions)
}

/// Whether a section meets its item's check (CONTEXT sections 3 and 12).
pub fn check_passes(item: &Value, section: &Value, record: &Value, job: &Value) -> bool {
    let check = at(item, &["check"]);
    match text(check, &["kind"]) {
        "source_included" => {
            // Either the path the bytes are at, or the symbolic link the
            // item named to reach them. Both are sealed in the section, so
            // this stays a string comparison over the packet and needs no
            // repository at read time (CONTEXT section 3: satisfaction has
            // to be decidable).
            let wanted = text(check, &["path"]);
            text(section, &["source", "repository"]) == text(check, &["repository"])
                && section.get("source").is_some()
                && (text(section, &["source", "path"]) == wanted
                    || text(section, &["source", "via"]) == wanted)
        }
        "evidence_included" => list(section, &["citations"]).iter().any(|citation| {
            same(
                at(citation, &["evidence", "artifact"]),
                at(check, &["evidence", "artifact"]),
            ) && text(citation, &["evidence", "digest"]) == text(check, &["evidence", "digest"])
        }),
        "claim_included" => {
            section.get("claim").is_some()
                && same(at(section, &["claim", "reference"]), at(check, &["claim"]))
                && claim_shortfall(item, at(section, &["claim"])).is_none()
        }
        "authority_content_included" => {
            let item_id = text(item, &["item_id"]);
            let Some(supplied) = list(record, &["authority_content"])
                .iter()
                .find(|c| text(c, &["item_id"]) == item_id)
            else {
                return false;
            };
            let current = match at(job, &["corrections", item_id]) {
                Value::Int(corrected) => *corrected,
                _ => int(supplied, &["authority_revision"]),
            };
            matches!(section.get("authority_revision"), Some(Value::Int(r)) if *r == current)
        }
        _ => false,
    }
}

/// The unmet reason a claim check gives an item that has a claim section.
fn claim_reason(item: &Value, job: &Value) -> Option<&'static str> {
    if text(item, &["check", "kind"]) != "claim_included" {
        return None;
    }
    let item_id = text(item, &["item_id"]);
    let section = list(job, &["sections"]).iter().find(|s| {
        s.get("item_id").and_then(Value::as_str) == Some(item_id)
            && (s.get("claim").is_some() || s.get("claim_error").is_some())
    })?;
    if let Some(error) = section.get("claim_error").and_then(Value::as_str) {
        return Some(if error == DIGEST_MISMATCH {
            DIGEST_MISMATCH
        } else {
            KNOWLEDGE_UNAVAILABLE
        });
    }
    if !same(
        at(section, &["claim", "reference"]),
        at(item, &["check", "claim"]),
    ) {
        return Some("unavailable");
    }
    claim_shortfall(item, at(section, &["claim"]))
        .or((at(section, &["historical"]) == &Value::Bool(true)).then_some("invalid_for_target"))
}

/// Item results for a request from its job's state (CONTEXT section 3).
///
/// While preparing (`finishing` false) an item not yet satisfied is
/// `pending`. At publication it is `unmet` if required, or `degraded` if
/// advisory, with the first reason that applies: a scripted unmet reason, the
/// claim check's reason, an omission for capacity, a correction during
/// preparation, the reason preparation ended, or `unavailable`. A scripted
/// reason never makes a satisfied item unmet.
pub fn item_results(
    record: &Value,
    job: &Value,
    finishing: bool,
    reason: Option<&str>,
) -> Vec<Value> {
    let (included, _) = inclusion(record, job);
    let sections = list(job, &["sections"]);
    list(record, &["items"])
        .iter()
        .map(|item| {
            let item_id = text(item, &["item_id"]);
            let for_item = |s: &&Value| s.get("item_id").and_then(Value::as_str) == Some(item_id);
            let current = |s: &&Value| at(s, &["historical"]) != &Value::Bool(true);
            let is_included = |s: &&Value| included.iter().any(|i| i == text(s, &["section_id"]));
            let satisfied = sections
                .iter()
                .filter(for_item)
                .filter(current)
                .filter(is_included)
                .any(|s| check_passes(item, s, record, job));
            let omitted = sections
                .iter()
                .filter(for_item)
                .filter(current)
                .any(|s| !is_included(&s));
            let mut result = object(vec![
                ("item_id", string(item_id)),
                ("obligation", at(item, &["obligation"]).clone()),
            ]);
            let scripted = at(job, &["unmet", item_id]);
            if !satisfied && !is_null(scripted) {
                set(&mut result, "result", string("unmet"));
                set(&mut result, "reason", scripted.clone());
            } else if satisfied {
                set(&mut result, "result", string("satisfied"));
            } else if !finishing {
                set(&mut result, "result", string("pending"));
            } else {
                let cause = if let Some(claim) = claim_reason(item, job) {
                    claim
                } else if omitted {
                    "output_capacity"
                } else if !is_null(at(job, &["corrections", item_id])) {
                    "corrected_during_preparation"
                } else {
                    reason.unwrap_or("unavailable")
                };
                let outcome = if is_required(item) {
                    "unmet"
                } else {
                    "degraded"
                };
                set(&mut result, "result", string(outcome));
                set(&mut result, "reason", string(cause));
            }
            result
        })
        .collect()
}

/// A request's state from its item results (CONTEXT section 3).
pub fn state_of(items: &[Value]) -> &'static str {
    if items.iter().any(|i| text(i, &["result"]) == "unmet") {
        "unmet"
    } else if items.iter().any(|i| text(i, &["result"]) == "degraded") {
        "partial"
    } else {
        "ready"
    }
}

/// Everything one packet revision publishes.
pub struct Packet {
    /// The canonical bytes to seal.
    pub content: Vec<u8>,
    pub digest: String,
    pub revision: i64,
    pub artifact: String,
    pub items: Vec<Value>,
    pub state: &'static str,
    /// The result facts, without the reference, which names where the bytes
    /// were sealed and so is added once they are.
    pub facts: Value,
}

/// Compile the next packet revision of `request` (CONTEXT section 5): the
/// bytes, and the facts published beside them.
pub fn compile_packet(
    request: &str,
    record: &Value,
    job: &Value,
    reason: Option<&str>,
    compiler: &str,
) -> Packet {
    let published = list(record, &["packets"]).len() as i64;
    let revision = published + 1;
    let items = item_results(record, job, true, reason);
    let state = state_of(&items);
    let (included, omissions) = inclusion(record, job);
    let claims = at(record, &["claims"]) == &Value::Bool(true);
    let is_included = |s: &&Value| included.iter().any(|i| i == text(s, &["section_id"]));
    let sections: Vec<&Value> = list(job, &["sections"])
        .iter()
        .filter(is_included)
        .collect();

    let body_sections = sections
        .iter()
        .map(|s| {
            let mut out = object(vec![
                ("section_id", at(s, &["section_id"]).clone()),
                ("label", at(s, &["label"]).clone()),
                ("historical", at(s, &["historical"]).clone()),
                ("content", at(s, &["content"]).clone()),
                ("citations", Value::Array(list(s, &["citations"]).to_vec())),
            ]);
            if let Some(item) = s.get("item_id") {
                set(&mut out, "item_id", item.clone());
            }
            if claims && at(s, &["claim"]).is_object() {
                set(&mut out, "claim", at(s, &["claim"]).clone());
            }
            out
        })
        .collect();
    let coverage = Value::Array(list(job, &["coverage"]).to_vec());
    let corrections = at(job, &["corrections"]);
    let authority: Vec<Value> = list(record, &["authority_content"])
        .iter()
        .map(|content| {
            let item = text(content, &["item_id"]);
            let revision = match at(corrections, &[item]) {
                Value::Int(corrected) => Value::Int(*corrected),
                _ => at(content, &["authority_revision"]).clone(),
            };
            object(vec![
                ("item_id", string(item)),
                ("authority_revision", revision),
                ("evidence", at(content, &["evidence"]).clone()),
                ("coverage", coverage.clone()),
            ])
        })
        .collect();
    let applicability = object(vec![
        ("basis", at(record, &["basis"]).clone()),
        (
            "conditions",
            Value::Array(list(job, &["conditions"]).to_vec()),
        ),
    ]);
    let mut body = object(vec![
        (
            "format",
            string(if claims { CLAIMS_FORMAT } else { FORMAT }),
        ),
        ("request", string(request)),
        ("revision", Value::Int(revision)),
        ("sections", Value::Array(body_sections)),
        ("items", Value::Array(items.clone())),
        ("coverage", coverage.clone()),
        ("omissions", Value::Array(omissions.clone())),
        ("applicability", applicability.clone()),
        ("authority", Value::Array(authority.clone())),
    ]);
    if revision > 1 {
        set(
            &mut body,
            "supersedes",
            object(vec![("revision", Value::Int(revision - 1))]),
        );
    }
    let content = cbr_encoding::to_canonical(&body);
    let digest = cbr_encoding::digest_bytes(&content);

    let citations: Vec<Value> = sections
        .iter()
        .flat_map(|s| list(s, &["citations"]).iter().cloned())
        .collect();
    let selected: Vec<Value> = sections
        .iter()
        .filter_map(|s| s.get("source").cloned())
        .chain(
            citations
                .iter()
                .map(|c| object(vec![("evidence", at(c, &["evidence"]).clone())])),
        )
        .collect();
    let job_subject = subject(JOB, text(record, &["job"]));
    let fact_sections = sections
        .iter()
        .map(|s| {
            let mut out = object(vec![
                ("section_id", at(s, &["section_id"]).clone()),
                ("label", at(s, &["label"]).clone()),
                ("historical", at(s, &["historical"]).clone()),
                ("length", Value::Int(text(s, &["content"]).len() as i64)),
                (
                    "citations",
                    Value::Array(
                        list(s, &["citations"])
                            .iter()
                            .map(|c| at(c, &["citation_id"]).clone())
                            .collect(),
                    ),
                ),
            ]);
            if let Some(item) = s.get("item_id") {
                set(&mut out, "item_id", item.clone());
            }
            if claims && at(s, &["claim"]).is_object() {
                set(&mut out, "claim", at(s, &["claim"]).clone());
            }
            out
        })
        .collect();
    let mut facts = object(vec![
        ("request", subject(REQUEST, request)),
        ("job", job_subject.clone()),
        ("selected", Value::Array(selected)),
        (
            "provenance",
            object(vec![
                ("compiler", string(compiler)),
                ("job", job_subject),
                ("basis", at(record, &["basis"]).clone()),
            ]),
        ),
        ("sections", Value::Array(fact_sections)),
        (
            "inclusions",
            Value::Array(included.iter().map(|i| string(i)).collect()),
        ),
        ("omissions", Value::Array(omissions)),
        ("citations", Value::Array(citations)),
        ("coverage", coverage),
        ("items", Value::Array(items.clone())),
        ("applicability", applicability),
        ("authority", Value::Array(authority)),
        ("body", body.clone()),
    ]);
    if let Some(supersedes) = body.get("supersedes") {
        set(&mut facts, "supersedes", supersedes.clone());
    }
    Packet {
        content,
        digest,
        revision,
        artifact: crate::ids::packet_artifact(request, revision),
        items,
        state,
        facts,
    }
}

/// Every identifier-typed member of a packet's published facts, as a JSON
/// pointer with `*` for each element of an array.
///
/// **These are the `context.packet.inspect` result's**, read out of the
/// vendored schema and its references: every path the schema types as
/// `core/1`'s `identifier` and that is present when a packet is published.
/// A test walks the schema and fails if a path it finds is in neither this
/// list nor [`IDENTIFIERS_NOT_AT_PUBLISH`]. The sealed body's sections and
/// the citations they carry are checked beside them, because the body is
/// what a consumer reads and the facts are only a projection of it.
pub const IDENTIFIER_PATHS: [&str; 30] = [
    "/request/id",
    "/job/id",
    "/provenance/job/id",
    "/selected/*/evidence/provider",
    "/selected/*/evidence/artifact/id",
    "/sections/*/section_id",
    "/sections/*/item_id",
    "/sections/*/citations/*",
    "/sections/*/claim/reference/claim",
    "/sections/*/claim/reference/provider",
    "/sections/*/claim/reliance/decision",
    "/sections/*/claim/applicability/evaluation",
    "/sections/*/claim/conflicts/*/conflict",
    "/inclusions/*",
    "/omissions/*/section_id",
    "/omissions/*/item_id",
    "/citations/*/citation_id",
    "/citations/*/evidence/provider",
    "/citations/*/evidence/artifact/id",
    "/items/*/item_id",
    "/applicability/conditions/*/condition_id",
    "/applicability/conditions/*/item_id",
    "/authority/*/item_id",
    "/authority/*/evidence/provider",
    "/authority/*/evidence/artifact/id",
    "/body/sections/*/section_id",
    "/body/sections/*/item_id",
    "/body/sections/*/citations/*/citation_id",
    "/body/sections/*/citations/*/evidence/provider",
    "/body/sections/*/citations/*/evidence/artifact/id",
];

/// The identifier-typed paths of the `context.packet.inspect` result that
/// are not in the facts a packet is published with, each for a stated
/// reason, so the schema walk can tell a path left out on purpose from one
/// forgotten.
///
/// - `/reference/…` is added once the packet is sealed: its artifact id is
///   the packet artifact, checked as the second argument of
///   [`ids_outside_grammar`]; its packet id is the request id, checked as
///   `/request/id`; and its provider is this provider's own id.
/// - `/claim_changes`, `/invalidated_items` and `/unverified_items` are
///   read-time facts (CONTEXT sections 8 and 14), computed at each read from
///   section ids, item ids and claim references the published facts hold.
#[cfg(test)]
pub const IDENTIFIERS_NOT_AT_PUBLISH: [&str; 12] = [
    "/reference/artifact/artifact/id",
    "/reference/artifact/provider",
    "/reference/packet/id",
    "/claim_changes/*/section_id",
    "/claim_changes/*/claim/claim",
    "/claim_changes/*/claim/provider",
    "/invalidated_items/*/item_id",
    "/invalidated_items/*/claim/claim",
    "/invalidated_items/*/claim/provider",
    "/unverified_items/*/item_id",
    "/unverified_items/*/claim/claim",
    "/unverified_items/*/claim/provider",
];

/// Where a packet about to be published names an id outside the protocol's
/// identifier grammar, or names one section id or one citation id twice, as
/// JSON pointers. Empty for a packet that may be published.
///
/// **The last door before a packet leaves the tick.** The compiler builds
/// its ids inside the grammar ([`crate::ids`]); a script can still name any
/// string at all, and nothing else stands between a malformed id and a
/// sealed, published packet that every consumer's schema would reject.
///
/// Pointers only, never the values: an id is often a repository path, and
/// what this returns is logged.
///
/// It does not check an omission's section id against the sections a
/// packet holds: the omission `s-x.o1` of item `x` and the section of an
/// item named `x.o1` share an id, and that is a recorded follow-up rather
/// than a refusal.
pub fn ids_outside_grammar(facts: &Value, artifact: &str) -> Vec<String> {
    fn walk(value: &Value, segments: &[&str], pointer: String, found: &mut Vec<String>) {
        match segments.split_first() {
            None => match value {
                Value::Null => {}
                Value::String(id) if crate::envelope::is_identifier(id) => {}
                _ => found.push(pointer),
            },
            Some((&"*", rest)) => {
                for (index, element) in value.as_array().unwrap_or_default().iter().enumerate() {
                    walk(element, rest, format!("{pointer}/{index}"), found);
                }
            }
            Some((name, rest)) => {
                if let Some(member) = value.get(name) {
                    walk(member, rest, format!("{pointer}/{name}"), found);
                }
            }
        }
    }
    let mut found = Vec::new();
    if !crate::envelope::is_identifier(artifact) {
        found.push("/reference/artifact/artifact/id".to_string());
    }
    for path in IDENTIFIER_PATHS {
        let segments: Vec<&str> = path.split('/').skip(1).collect();
        walk(facts, &segments, String::new(), &mut found);
    }
    // A reader addresses a section and a citation by its id, so one id
    // naming two of either is a packet whose reader cannot tell which.
    for (listed, member) in [("sections", "section_id"), ("citations", "citation_id")] {
        let mut seen = std::collections::BTreeSet::new();
        for (index, entry) in list(facts, &[listed]).iter().enumerate() {
            if let Some(id) = entry.get(member).and_then(Value::as_str)
                && !seen.insert(id)
            {
                found.push(format!("/{listed}/{index}/{member}"));
            }
        }
    }
    found
}

/// A packet reference (CONTEXT section 2).
pub fn packet_reference(
    request: &str,
    revision: i64,
    provider: &str,
    artifact: &str,
    digest: &str,
) -> Value {
    object(vec![
        ("packet", subject(PACKET, request)),
        ("revision", Value::Int(revision)),
        (
            "artifact",
            object(vec![
                ("provider", string(provider)),
                ("artifact", subject(crate::evidence::ARTIFACT, artifact)),
                ("digest", string(digest)),
            ]),
        ),
    ])
}

/// The descriptor of a packet artifact, as its producer submits it.
pub fn packet_descriptor(request: &str, digest: &str, size: usize, captured_at: &str) -> Value {
    object(vec![
        ("digest", string(digest)),
        ("size", Value::Int(size as i64)),
        ("media_type", string(PACKET_MEDIA_TYPE)),
        (
            "producer",
            object(vec![("producer_id", string(PRODUCER_ID))]),
        ),
        ("source", subject(PACKET, request)),
        ("scope", string("context")),
        (
            "capture",
            object(vec![
                ("captured_at", string(captured_at)),
                ("anchors", Value::Array(Vec::new())),
            ]),
        ),
        (
            "coverage",
            object(vec![("completeness", string("complete"))]),
        ),
        ("retention_class", string("context-packet")),
    ])
}

// ---- reads -----------------------------------------------------------------

/// Authority corrections recorded after a revision was prepared (CONTEXT
/// section 8).
pub fn invalidated_by_corrections(facts: &Value, job: &Value) -> Vec<Value> {
    list(facts, &["authority"])
        .iter()
        .filter_map(|entry| {
            let item = text(entry, &["item_id"]);
            match at(job, &["corrections", item]) {
                Value::Int(corrected) if *corrected > int(entry, &["authority_revision"]) => {
                    Some(object(vec![
                        ("item_id", string(item)),
                        ("authority_revision", Value::Int(*corrected)),
                    ]))
                }
                _ => None,
            }
        })
        .collect()
}

/// Read-time claim facts for a published revision (CONTEXT section 14):
/// `claim_changes`, and the claim invalidations and unverified items that sit
/// beside the authority corrections already in `invalidated`. `reread` reads a
/// carried claim again exactly as preparation did.
pub fn read_time_claims(
    record: &Value,
    facts: &Value,
    invalidated: &mut Vec<Value>,
    mut reread: impl FnMut(&Value) -> Result<(Value, i64), &'static str>,
) -> (Vec<Value>, Vec<Value>) {
    let mut changes = Vec::new();
    let mut unverified = Vec::new();
    for section in list(facts, &["sections"])
        .iter()
        .filter(|s| at(s, &["claim"]).is_object())
    {
        let snapshot = at(section, &["claim"]);
        let reference = at(snapshot, &["reference"]);
        let item = list(record, &["items"]).iter().find(|i| {
            section.get("item_id").and_then(Value::as_str) == Some(text(i, &["item_id"]))
        });
        let satisfied_required = item.is_some_and(|item| {
            is_required(item)
                && text(item, &["check", "kind"]) == "claim_included"
                && list(facts, &["items"]).iter().any(|r| {
                    text(r, &["item_id"]) == text(item, &["item_id"])
                        && text(r, &["result"]) == "satisfied"
                })
        });
        let change = |kind: &str| {
            object(vec![
                ("section_id", at(section, &["section_id"]).clone()),
                ("claim", reference.clone()),
                ("change", string(kind)),
            ])
        };
        let entry = |reason: &str| {
            object(vec![
                ("item_id", at(section, &["item_id"]).clone()),
                ("claim", reference.clone()),
                ("reason", string(reason)),
            ])
        };
        match reread(reference) {
            Err(reason) => {
                changes.push(change("unavailable"));
                // Unavailable knowledge is unknown, never valid.
                if satisfied_required {
                    unverified.push(entry(reason));
                }
            }
            Ok((now, current_revision)) => {
                if text(&now, &["reliance", "state"]) != text(snapshot, &["reliance", "state"])
                    || at(&now, &["reliance", "permitted_use"])
                        != at(snapshot, &["reliance", "permitted_use"])
                {
                    changes.push(change("reliance_changed"));
                }
                if text(&now, &["applicability", "result"])
                    != text(snapshot, &["applicability", "result"])
                {
                    changes.push(change("applicability_changed"));
                }
                let opened = list(&now, &["conflicts"]).iter().any(|c| {
                    !list(snapshot, &["conflicts"])
                        .iter()
                        .any(|o| same(at(o, &["conflict"]), at(c, &["conflict"])))
                });
                if opened {
                    changes.push(change("conflict_opened"));
                }
                if current_revision > int(reference, &["revision"]) {
                    changes.push(change("lineage_revised"));
                }
                if !satisfied_required {
                    continue;
                }
                let item = item.expect("a satisfied item exists");
                // A lost permitted use comes first; for every reliance, a claim
                // now invalid for the target would make its section historical.
                let shortfall =
                    claim_shortfall(item, &now).or((text(&now, &["applicability", "result"])
                        == "invalid_for_target")
                        .then_some("invalid_for_target"));
                match shortfall {
                    Some("not_accepted") => invalidated.push(entry("permitted_use_lost")),
                    Some("invalid_for_target") => invalidated.push(entry("invalid_for_target")),
                    Some("applicability_not_established") => {
                        unverified.push(entry("applicability_not_established"))
                    }
                    _ => {}
                }
            }
        }
    }
    (changes, unverified)
}

/// An exact byte range: at most `max_bytes` (default 4096) from `offset`,
/// halved until its encoding fits the caller's receive budget. It never
/// carries a digest, even when it covers every byte (CONTEXT section 6).
pub fn excerpt(data: &[u8], payload: &Value, frame_budget: usize) -> Value {
    let offset = (int(payload, &["offset"]).max(0) as usize).min(data.len());
    let wanted = match payload.get("max_bytes") {
        Some(Value::Int(n)) => *n as usize,
        _ => 4096,
    };
    let mut take = wanted.min(data.len() - offset);
    while take > 1 && take.div_ceil(3) * 4 + 16_384 > frame_budget {
        take /= 2;
    }
    object(vec![
        ("offset", Value::Int(offset as i64)),
        ("length", Value::Int(take as i64)),
        ("size", Value::Int(data.len() as i64)),
        (
            "data_base64",
            string(&cbr_encoding::encode_base64(&data[offset..offset + take])),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Value {
        cbr_encoding::parse(text.as_bytes()).expect("test JSON parses")
    }

    fn record(items: &str, capacity: i64) -> Value {
        parse(&format!(
            r#"{{"items":{items},"limits":{{"deadline":"2030-01-01T01:00:00Z",
            "investigation":{{"units":"q","amount":10}},
            "output_capacity":{{"units":"bytes","amount":{capacity}}}}},
            "authority_content":[],"basis":{{"repositories":[{{"id":"r","tree":"t","workspace":"clean","dirty":null}}],"completeness":"complete"}},
            "claims":false,"packets":[],"job":"j"}}"#
        ))
    }

    fn source_item(id: &str, obligation: &str, path: &str) -> String {
        format!(
            r#"{{"item_id":"{id}","obligation":"{obligation}","reliance":"binding",
            "check":{{"kind":"source_included","repository":"r","path":"{path}"}}}}"#
        )
    }

    fn job(sections: &str) -> Value {
        parse(&format!(
            r#"{{"sections":{sections},"omissions":[],"coverage":[],"unmet":{{}},"corrections":{{}},"conditions":[]}}"#
        ))
    }

    #[test]
    fn content_for_the_wrong_path_leaves_a_required_item_unmet() {
        let record = record(
            &format!("[{}]", source_item("i", "required_before_start", "a.rs")),
            100,
        );
        let wrong = job(
            r#"[{"section_id":"s","item_id":"i","label":"source_inspected","historical":false,"content":"x","source":{"repository":"r","path":"b.rs"}}]"#,
        );
        let right = job(
            r#"[{"section_id":"s","item_id":"i","label":"source_inspected","historical":false,"content":"x","source":{"repository":"r","path":"a.rs"}}]"#,
        );
        assert_eq!(
            text(&item_results(&record, &wrong, true, None)[0], &["result"]),
            "unmet"
        );
        assert_eq!(
            text(&item_results(&record, &right, true, None)[0], &["result"]),
            "satisfied"
        );
    }

    #[test]
    fn mandatory_content_is_included_past_capacity_and_advisory_content_is_omitted() {
        let items = format!(
            "[{},{}]",
            source_item("req", "required_before_start", "a"),
            source_item("adv", "advisory", "b")
        );
        let record = record(&items, 5);
        let job = job(
            r#"[{"section_id":"s-req","item_id":"req","label":"binding","historical":false,"content":"0123456789","source":{"repository":"r","path":"a"}},
                          {"section_id":"s-adv","item_id":"adv","label":"observation","historical":false,"content":"012","source":{"repository":"r","path":"b"}}]"#,
        );
        let (included, omissions) = inclusion(&record, &job);
        assert_eq!(included, vec!["s-req".to_string()]);
        assert_eq!(text(&omissions[0], &["reason"]), "output_capacity");
        let results = item_results(&record, &job, true, None);
        assert_eq!(text(&results[0], &["result"]), "satisfied");
        assert_eq!(text(&results[1], &["result"]), "degraded");
        assert_eq!(text(&results[1], &["reason"]), "output_capacity");
    }

    #[test]
    fn capacity_is_reserved_for_required_content_whatever_order_it_was_prepared_in() {
        let items = format!(
            "[{},{}]",
            source_item("req", "required_before_start", "a"),
            source_item("adv", "advisory", "b")
        );
        let record = record(&items, 20);
        // Advisory content prepared first would fit on its own, and would
        // leave too little for the required section after it.
        let job = job(
            r#"[{"section_id":"s-adv","item_id":"adv","label":"observation","historical":false,"content":"012345678901234","source":{"repository":"r","path":"b"}},
                {"section_id":"s-req","item_id":"req","label":"binding","historical":false,"content":"0123456789","source":{"repository":"r","path":"a"}}]"#,
        );
        let (included, omissions) = inclusion(&record, &job);
        assert_eq!(included, vec!["s-req".to_string()]);
        assert_eq!(omissions.len(), 1);
        assert_eq!(text(&omissions[0], &["section_id"]), "s-adv");
        assert_eq!(text(&omissions[0], &["reason"]), "output_capacity");
    }

    #[test]
    fn a_missing_required_item_is_unmet_never_satisfied_or_downgraded() {
        let items = format!(
            "[{},{}]",
            source_item("req", "required_before_start", "a"),
            source_item("adv", "advisory", "b")
        );
        let record = record(&items, 100);
        let results = item_results(&record, &job("[]"), true, Some("deadline_passed"));
        assert_eq!(text(&results[0], &["obligation"]), "required_before_start");
        assert_eq!(text(&results[0], &["result"]), "unmet");
        assert_eq!(text(&results[0], &["reason"]), "deadline_passed");
        assert_eq!(text(&results[1], &["result"]), "degraded");
        let pending = item_results(&record, &job("[]"), false, None);
        assert_eq!(text(&pending[0], &["result"]), "pending");
    }

    #[test]
    fn a_scripted_unmet_reason_never_overrides_a_satisfied_item() {
        let record = record(
            &format!("[{}]", source_item("i", "required_before_start", "a")),
            100,
        );
        let mut job = job(
            r#"[{"section_id":"s","item_id":"i","label":"binding","historical":false,"content":"x","source":{"repository":"r","path":"a"}}]"#,
        );
        set_in(&mut job, "unmet", "i", string("scripted_gap"));
        assert_eq!(
            text(&item_results(&record, &job, true, None)[0], &["result"]),
            "satisfied"
        );
        let mut empty = self::job("[]");
        set_in(&mut empty, "unmet", "i", string("scripted_gap"));
        let results = item_results(&record, &empty, true, Some("deadline_passed"));
        assert_eq!(text(&results[0], &["reason"]), "scripted_gap");
    }

    #[test]
    fn a_label_never_promotes_an_unaccepted_claim_and_only_invalid_makes_it_stale() {
        let snapshot = |state: &str, used: &str, result: &str| {
            parse(&format!(
                r#"{{"reliance":{{"state":"{state}","permitted_use":"{used}"}},"applicability":{{"result":"{result}"}}}}"#
            ))
        };
        let mut proposed = parse(r#"{"label":"binding","historical":false}"#);
        attach_claim(
            &mut proposed,
            Ok((snapshot("proposed", "binding", "applicable"), 1)),
        );
        assert_eq!(text(&proposed, &["label"]), "hypothesis");
        let mut needs_check = parse(r#"{"label":"observation","historical":false}"#);
        attach_claim(
            &mut needs_check,
            Ok((snapshot("accepted_for_use", "evidence", "needs_check"), 1)),
        );
        assert_eq!(text(&needs_check, &["label"]), "observation");
        assert_eq!(at(&needs_check, &["historical"]), &Value::Bool(false));
        let mut invalid = parse(r#"{"label":"binding","historical":false}"#);
        attach_claim(
            &mut invalid,
            Ok((
                snapshot("accepted_for_use", "binding", "invalid_for_target"),
                1,
            )),
        );
        assert_eq!(text(&invalid, &["label"]), "stale");
        assert_eq!(at(&invalid, &["historical"]), &Value::Bool(true));
    }

    #[test]
    fn claim_shortfall_follows_the_reliance_table() {
        let item = |reliance: &str| parse(&format!(r#"{{"reliance":"{reliance}"}}"#));
        let snapshot = |state: &str, used: &str, result: &str| {
            parse(&format!(
                r#"{{"reliance":{{"state":"{state}","permitted_use":"{used}"}},"applicability":{{"result":"{result}"}}}}"#
            ))
        };
        assert_eq!(
            claim_shortfall(
                &item("binding"),
                &snapshot("accepted_for_use", "evidence", "applicable")
            ),
            Some("not_accepted")
        );
        assert_eq!(
            claim_shortfall(
                &item("evidence"),
                &snapshot("accepted_for_use", "binding", "applicable")
            ),
            None
        );
        assert_eq!(
            claim_shortfall(
                &item("binding"),
                &snapshot("accepted_for_use", "binding", "needs_check")
            ),
            Some("applicability_not_established")
        );
        assert_eq!(
            claim_shortfall(
                &item("binding"),
                &snapshot("accepted_for_use", "binding", "invalid_for_target")
            ),
            Some("invalid_for_target")
        );
        assert_eq!(
            claim_shortfall(&item("hypothesis"), &snapshot("proposed", "", "unknown")),
            None
        );
        assert_eq!(
            claim_shortfall(&item("reference"), &snapshot("rejected", "", "applicable")),
            Some("not_accepted")
        );
    }

    #[test]
    fn a_claim_whose_record_does_not_digest_to_its_reference_is_never_carried() {
        let record = parse(r#"{"plane":"normative","statement":"x"}"#);
        let digest = crate::knowledge::record_digest(&record);
        let reference = parse(&format!(
            r#"{{"provider":"p","claim":"c","revision":1,"digest":"{digest}"}}"#
        ));
        let found = object(vec![
            ("reference", reference.clone()),
            ("record", record),
            ("reliance", parse(r#"{"state":"proposed"}"#)),
            ("applicability", Value::Array(vec![])),
            ("conflicts", Value::Array(vec![])),
            ("support", parse(r#"{"class":"unsupported"}"#)),
            ("current_revision", Value::Int(2)),
        ]);
        let basis = parse(
            r#"{"repositories":[{"id":"r","tree":"t","workspace":"clean","dirty":null}],"completeness":"complete"}"#,
        );
        let (snapshot, current) = claim_snapshot(Ok(&found), &reference, &basis).expect("verifies");
        assert_eq!(text(&snapshot, &["applicability", "result"]), "unknown");
        assert_eq!(current, 2);
        let mut altered = found.clone();
        set(
            &mut altered,
            "record",
            parse(r#"{"plane":"normative","statement":"y"}"#),
        );
        assert_eq!(
            claim_snapshot(Ok(&altered), &reference, &basis).err(),
            Some(DIGEST_MISMATCH)
        );
        // A read of another revision is not the referenced one, whatever it digests to.
        let mut later = found;
        set(
            &mut later,
            "reference",
            parse(&format!(
                r#"{{"provider":"p","claim":"c","revision":2,"digest":"{digest}"}}"#
            )),
        );
        assert_eq!(
            claim_snapshot(Ok(&later), &reference, &basis).err(),
            Some(DIGEST_MISMATCH)
        );
    }

    #[test]
    fn a_commit_only_basis_declared_complete_is_refused_at_completeness() {
        let payload = parse(
            r#"{"consumer":{"task":"t","principal":"a"},
            "basis":{"repositories":[{"id":"r","tree":"t","workspace":"dirty","dirty":null}],"completeness":"complete"},
            "items":[{"item_id":"i","selector":{"kind":"path","value":"p"},"obligation":"advisory","reliance":"reference","selected_by":"o","check":{"kind":"source_included","repository":"r","path":"p"}}],
            "limits":{"deadline":"2030-01-01T00:00:00Z","investigation":{"units":"q","amount":1},"output_capacity":{"units":"bytes","amount":1}}}"#,
        );
        let error = check_submit_payload(&payload).expect_err("refused");
        assert_eq!(error.code, "invalid_envelope");
        let mut partial = payload;
        if let Some(basis) = member_mut(&mut partial, "basis") {
            set(basis, "completeness", string("partial"));
        }
        assert!(check_submit_payload(&partial).is_ok());
    }

    #[test]
    fn the_excerpt_is_an_exact_range_without_a_digest() {
        let data = b"0123456789";
        let excerpt = excerpt(data, &parse(r#"{"offset":2,"max_bytes":3}"#), 1 << 20);
        assert_eq!(int(&excerpt, &["length"]), 3);
        assert_eq!(
            text(&excerpt, &["data_base64"]),
            cbr_encoding::encode_base64(b"234")
        );
        assert!(excerpt.get("digest").is_none());
    }

    #[test]
    fn a_job_is_joined_only_by_a_request_with_the_same_view() {
        // Two grants of different width belong to the same principal often
        // enough: one for a review, one for everything. A job compiled under
        // the wider one has already read repositories the narrower request
        // may not, so joining it would hand them over -- and `may_join` is
        // the only place that can refuse, because preparation runs on the
        // provider's own authority and never re-checks.
        let payload = parse(
            r#"{"consumer":{"task":"t","principal":"p"},
                "basis":{"repositories":[{"id":"a","tree":"t1","workspace":"clean","dirty":null}],"completeness":"complete"},
                "items":[],
                "limits":{"deadline":"2030-01-01T00:00:00Z","investigation":{"units":"q","amount":1},"output_capacity":{"units":"bytes","amount":4096}}}"#,
        );
        let wide = vec!["a".to_string(), "b".to_string()];
        let narrow = vec!["a".to_string()];
        let claims: Vec<String> = vec!["c-1".into()];
        let job = new_job("owner", &payload, &[], "r-1");
        let mut wide_job = job.clone();
        set(
            &mut wide_job,
            "view",
            Value::Array(wide.iter().map(|id| string(id)).collect()),
        );
        let mut narrow_job = job;
        set(
            &mut narrow_job,
            "view",
            Value::Array(narrow.iter().map(|id| string(id)).collect()),
        );
        for job in [&mut wide_job, &mut narrow_job] {
            set(
                job,
                "readable_claims",
                Value::Array(claims.iter().map(|id| string(id)).collect()),
            );
        }

        assert!(may_join(&wide_job, "owner", &payload, &wide, &claims, &[]));
        assert!(
            !may_join(&wide_job, "owner", &payload, &narrow, &claims, &[]),
            "a narrower request never joins a wider job"
        );
        assert!(
            !may_join(&narrow_job, "owner", &payload, &wide, &claims, &[]),
            "and a wider one never joins a narrower job either: the packet \
             would be short of what it was entitled to without saying so"
        );
        assert!(may_join(
            &narrow_job,
            "owner",
            &payload,
            &narrow,
            &claims,
            &[]
        ));
        assert!(
            !may_join(&narrow_job, "someone-else", &payload, &narrow, &claims, &[]),
            "another principal never joins a job"
        );
        // And the claims a grant could read are part of the scope too: a
        // job compiled while another claim was readable has already read it.
        assert!(
            !may_join(
                &narrow_job,
                "owner",
                &payload,
                &narrow,
                &["c-1".into(), "c-2".into()],
                &[]
            ),
            "a request that may read more claims never joins a narrower job"
        );
        assert!(
            !may_join(&narrow_job, "owner", &payload, &narrow, &[], &[]),
            "nor one that may read fewer"
        );
        // And the evidence, since m5a: a job whose projection read an
        // artifact has put its bytes in a packet.
        let mut reading = narrow_job.clone();
        set(
            &mut reading,
            "readable_evidence",
            Value::Array(vec![string("log")]),
        );
        assert!(may_join(
            &reading,
            "owner",
            &payload,
            &narrow,
            &claims,
            &["log".into()]
        ));
        assert!(
            !may_join(&reading, "owner", &payload, &narrow, &claims, &[]),
            "a request that may not read the artifact never joins a job that read it"
        );
        assert!(
            !may_join(
                &narrow_job,
                "owner",
                &payload,
                &narrow,
                &claims,
                &["log".into()]
            ),
            "nor one that may read it a job that could not"
        );
    }

    // ---- the identifier guard ---------------------------------------------

    /// Put `leaf` at `path` in `target`, creating what is missing: an
    /// object for a name, and element 0 of an array for `*`.
    fn insert(target: &mut Value, segments: &[&str], leaf: Value) {
        let Some((first, rest)) = segments.split_first() else {
            *target = leaf;
            return;
        };
        if *first == "*" {
            if !matches!(target, Value::Array(_)) {
                *target = Value::Array(Vec::new());
            }
            let Value::Array(elements) = target else {
                unreachable!()
            };
            if elements.is_empty() {
                elements.push(Value::Object(Vec::new()));
            }
            insert(&mut elements[0], rest, leaf);
        } else {
            if !matches!(target, Value::Object(_)) {
                *target = Value::Object(Vec::new());
            }
            if target.get(first).is_none() {
                set(target, first, Value::Null);
            }
            insert(member_mut(target, first).expect("just set"), rest, leaf);
        }
    }

    /// Facts with a valid identifier at every path the guard checks.
    fn full_facts() -> Value {
        let mut facts = Value::Object(Vec::new());
        for path in IDENTIFIER_PATHS {
            let segments: Vec<&str> = path.split('/').skip(1).collect();
            insert(&mut facts, &segments, string("ok-id.1:~_"));
        }
        facts
    }

    #[test]
    fn the_guard_names_every_id_outside_the_grammar_by_where_it_is() {
        let facts = full_facts();
        assert_eq!(
            ids_outside_grammar(&facts, "packet.r-1.1"),
            Vec::<String>::new()
        );
        // **One bad value per path**, each reported at exactly its own
        // pointer, and a value that is not a string at all is as bad.
        for path in IDENTIFIER_PATHS {
            let segments: Vec<&str> = path.split('/').skip(1).collect();
            let pointer = path.replace('*', "0");
            for bad in [
                string("s/x"),
                string(&"a".repeat(129)),
                string("-lead"),
                Value::Int(7),
            ] {
                let mut broken = facts.clone();
                insert(&mut broken, &segments, bad.clone());
                assert_eq!(
                    ids_outside_grammar(&broken, "packet.r-1.1"),
                    vec![pointer.clone()],
                    "{path} holding {bad:?}"
                );
            }
        }
        assert_eq!(
            ids_outside_grammar(&facts, &format!("packet.{}.1", "r".repeat(128))),
            vec!["/reference/artifact/artifact/id".to_string()],
            "the packet's own artifact id"
        );
    }

    #[test]
    fn the_guard_refuses_one_section_id_or_one_citation_id_named_twice() {
        let facts = cbr_encoding::parse(
            br#"{"sections":[{"section_id":"s-a"},{"section_id":"s-b"},{"section_id":"s-a"}],
                 "citations":[{"citation_id":"c-a"},{"citation_id":"c-a"}]}"#,
        )
        .expect("json");
        assert_eq!(
            ids_outside_grammar(&facts, "packet.r-1.1"),
            vec![
                "/sections/2/section_id".to_string(),
                "/citations/1/citation_id".to_string()
            ]
        );
    }

    /// Take the member at `segments` out of `target`, following element 0
    /// of an array for `*`.
    fn remove(target: &mut Value, segments: &[&str]) {
        match segments {
            [] => {}
            [last] => {
                if let Value::Object(members) = target {
                    members.retain(|(name, _)| name != last);
                }
            }
            [first, rest @ ..] => {
                let next = if *first == "*" {
                    match target {
                        Value::Array(elements) => elements.first_mut(),
                        _ => None,
                    }
                } else {
                    member_mut(target, first)
                };
                if let Some(next) = next {
                    remove(next, rest);
                }
            }
        }
    }

    #[test]
    fn the_guard_names_a_required_id_that_is_absent_or_null() {
        // **An id that is not there is not an id inside the grammar.** The
        // inspect schema requires a section's `section_id` and a
        // citation's `citation_id`, and a script can leave either out: the
        // section's id is copied into the facts as `null` and the
        // citation's is not copied at all. Each is reported where it is
        // missing, as a bad value there would be.
        let facts = full_facts();
        for (path, pointer) in [
            ("/request/id", "/request/id"),
            ("/sections/*/section_id", "/sections/0/section_id"),
            ("/citations/*/citation_id", "/citations/0/citation_id"),
            (
                "/citations/*/evidence/artifact/id",
                "/citations/0/evidence/artifact/id",
            ),
            ("/items/*/item_id", "/items/0/item_id"),
            ("/body/sections/*/section_id", "/body/sections/0/section_id"),
            (
                "/body/sections/*/citations/*/citation_id",
                "/body/sections/0/citations/0/citation_id",
            ),
        ] {
            let segments: Vec<&str> = path.split('/').skip(1).collect();
            let mut absent = facts.clone();
            remove(&mut absent, &segments);
            assert_eq!(
                ids_outside_grammar(&absent, "packet.r-1.1"),
                vec![pointer.to_string()],
                "{path} absent"
            );
            let mut null = facts.clone();
            insert(&mut null, &segments, Value::Null);
            assert_eq!(
                ids_outside_grammar(&null, "packet.r-1.1"),
                vec![pointer.to_string()],
                "{path} null"
            );
        }
        // An element of a list of ids is required by being in the list.
        for (path, pointer) in [
            ("/sections/*/citations/*", "/sections/0/citations/0"),
            ("/inclusions/*", "/inclusions/0"),
        ] {
            let segments: Vec<&str> = path.split('/').skip(1).collect();
            let mut null = facts.clone();
            insert(&mut null, &segments, Value::Null);
            assert_eq!(
                ids_outside_grammar(&null, "packet.r-1.1"),
                vec![pointer.to_string()],
                "{path} null"
            );
        }
        // **An optional id may be absent**: a section of no item, an
        // omission of a whole item, evidence that does not name its
        // provider. So may the object that would hold a required one: a
        // section that is not a claim has no claim reference to name.
        for path in [
            "/sections/*/item_id",
            "/omissions/*/section_id",
            "/omissions/*/item_id",
            "/selected/*/evidence/provider",
            "/citations/*/evidence/provider",
            "/body/sections/*/item_id",
            "/sections/*/claim",
        ] {
            let segments: Vec<&str> = path.split('/').skip(1).collect();
            let mut absent = facts.clone();
            remove(&mut absent, &segments);
            assert_eq!(
                ids_outside_grammar(&absent, "packet.r-1.1"),
                Vec::<String>::new(),
                "{path} absent"
            );
        }
    }

    #[test]
    fn the_guard_reads_the_sealed_body_where_it_differs_from_the_facts() {
        // **The body is what a consumer reads.** Today its sections and
        // their citations are built from the same values as the facts, so
        // a bad id in one is a bad id in the other; these pointers are
        // written out rather than read from the guard's list, so a body
        // path dropped from it fails here, whether or not the two could
        // ever differ.
        let facts = full_facts();
        for pointer in [
            "/body/sections/0/section_id",
            "/body/sections/0/item_id",
            "/body/sections/0/citations/0/citation_id",
            "/body/sections/0/citations/0/evidence/provider",
            "/body/sections/0/citations/0/evidence/artifact/id",
        ] {
            let segments: Vec<&str> = pointer
                .split('/')
                .skip(1)
                .map(|segment| if segment == "0" { "*" } else { segment })
                .collect();
            let mut broken = facts.clone();
            insert(&mut broken, &segments, string("s/x"));
            assert_eq!(
                ids_outside_grammar(&broken, "packet.r-1.1"),
                vec![pointer.to_string()],
                "the body alone holding a bad id at {pointer}"
            );
        }
    }

    /// Every identifier-typed path in a vendored schema, following `$ref`
    /// across files, with `*` for an array's elements.
    fn identifier_paths(file: &std::path::Path) -> std::collections::BTreeSet<String> {
        fn load(file: &std::path::Path) -> Value {
            let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            cbr_encoding::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e:?}", file.display()))
        }
        fn walk(
            file: &std::path::Path,
            node: &Value,
            pointer: &str,
            depth: usize,
            found: &mut std::collections::BTreeSet<String>,
        ) {
            assert!(depth < 64, "a reference cycle at {pointer}");
            if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
                let (target, fragment) = reference.split_once('#').unwrap_or((reference, ""));
                let target = if target.is_empty() {
                    file.to_path_buf()
                } else {
                    file.parent().expect("a directory").join(target)
                };
                if target.ends_with("core/1/common.schema.json") && fragment == "/$defs/identifier"
                {
                    found.insert(pointer.to_string());
                    return;
                }
                let mut resolved = load(&target);
                for part in fragment.split('/').filter(|part| !part.is_empty()) {
                    resolved = resolved.get(part).cloned().expect("the reference resolves");
                }
                walk(&target, &resolved, pointer, depth + 1, found);
            }
            for combinator in ["oneOf", "anyOf", "allOf"] {
                for option in list(node, &[combinator]) {
                    walk(file, option, pointer, depth + 1, found);
                }
            }
            if let Some(Value::Object(members)) = node.get("properties") {
                for (name, member) in members {
                    walk(file, member, &format!("{pointer}/{name}"), depth + 1, found);
                }
            }
            if let Some(items) = node.get("items") {
                walk(file, items, &format!("{pointer}/*"), depth + 1, found);
            }
        }
        let mut found = std::collections::BTreeSet::new();
        walk(file, &load(file), "", 0, &mut found);
        found
    }

    #[test]
    fn the_guard_covers_every_identifier_the_inspect_schema_types() {
        // **The list is checked against the schema, not against memory.**
        // Every path the vendored `context.packet.inspect` result types as
        // an identifier is either checked when a packet is published or
        // named, with its reason, as not present then.
        let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../vendor/protocol/v0.1.0/schemas/context/1/context.packet.inspect.result.schema.json",
        );
        let typed = identifier_paths(&schema);
        assert!(
            typed.len() > 30,
            "the walk reached the references: {typed:?}"
        );
        let accounted: std::collections::BTreeSet<String> = IDENTIFIER_PATHS
            .iter()
            .chain(IDENTIFIERS_NOT_AT_PUBLISH.iter())
            .map(|path| path.to_string())
            .collect();
        let missed: Vec<&String> = typed.difference(&accounted).collect();
        assert!(
            missed.is_empty(),
            "identifier paths the guard does not check: {missed:?}"
        );
        // And the other way: nothing listed is a path the schema does not
        // have, except the sealed body, which the inspect result does not
        // carry.
        let stale: Vec<&String> = accounted
            .iter()
            .filter(|path| !path.starts_with("/body/"))
            .filter(|path| !typed.contains(*path))
            .collect();
        assert!(
            stale.is_empty(),
            "listed paths the schema does not type: {stale:?}"
        );
    }
}
