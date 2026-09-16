//! `cbr` verbs over `knowledge/1`: propose, revise, decide, evaluate, inspect,
//! history, and binding a scope's authority.
//!
//! Each verb is one or more ordinary protocol requests. Where a command needs
//! something the caller should not have to type — the current revision and its
//! digest for `revise`, the binding epoch and the latest decision for
//! `decide` — the verb reads it through the protocol first, as any client
//! would. The provider still checks all of it; nothing here decides anything.

use std::path::Path;

use cbr_encoding::Value;

use crate::Options;
use crate::session::{Session, at, canonical, object, string, subject};

fn open(options: &Options) -> Result<Session, String> {
    Session::open(
        &options.socket,
        &options.credential_file,
        &["knowledge"],
        options.grant.clone(),
    )
}

fn read_object(path: &Path) -> Result<Value, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    let value = cbr_encoding::parse(&bytes)
        .map_err(|error| format!("parsing {}: {error}", path.display()))?;
    if !value.is_object() {
        return Err(format!("{} is not a JSON object", path.display()));
    }
    Ok(value)
}

fn print(value: &Value) {
    println!("{}", canonical(value));
}

fn int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}

/// The reference of a claim at a revision, or at its current revision.
fn reference(session: &mut Session, claim: &str, revision: Option<i64>) -> Result<Value, String> {
    let mut payload = vec![("claim", string(claim))];
    if let Some(revision) = revision {
        payload.push(("revision", Value::Int(revision)));
    }
    let inspected = session.query("knowledge.claim.inspect", object(payload))?;
    Ok(object(vec![
        ("reference", at(&inspected, &["reference"])),
        ("scope", at(&inspected, &["record", "scope", "id"])),
    ]))
}

pub fn propose(options: &Options, claim: &str, content: &Path) -> Result<(), String> {
    let payload = read_object(content)?;
    let mut session = open(options)?;
    let proposed = session.command(
        "knowledge.claim.propose",
        &format!("claim.{claim}.propose"),
        subject("knowledge.claim", claim),
        0,
        None,
        payload,
    )?;
    print(&at(&proposed, &["outcome"]));
    Ok(())
}

pub fn revise(options: &Options, claim: &str, content: &Path) -> Result<(), String> {
    let mut payload = read_object(content)?;
    let mut session = open(options)?;
    let current = at(&reference(&mut session, claim, None)?, &["reference"]);
    let revision = int(&at(&current, &["revision"])).ok_or("no current revision")?;
    if let Value::Object(members) = &mut payload {
        members.retain(|(name, _)| name != "supersedes");
        members.push((
            "supersedes".into(),
            object(vec![
                ("revision", Value::Int(revision)),
                ("digest", at(&current, &["digest"])),
            ]),
        ));
    }
    let revised = session.command(
        "knowledge.claim.revise",
        &format!("claim.{claim}.revise.{}", revision + 1),
        subject("knowledge.claim", claim),
        revision,
        None,
        payload,
    )?;
    print(&at(&revised, &["outcome"]));
    Ok(())
}

pub struct Decision<'a> {
    pub id: &'a str,
    pub claim: &'a str,
    pub revision: Option<i64>,
    pub value: &'a str,
    pub permitted_use: Option<&'a str>,
    pub rationale: &'a str,
}

pub fn decide(options: &Options, decision: &Decision<'_>) -> Result<(), String> {
    let mut session = open(options)?;
    let named = reference(&mut session, decision.claim, decision.revision)?;
    let claim_reference = at(&named, &["reference"]);
    let scope = at(&named, &["scope"]);
    // The binding's epoch, if the scope is bound. If it is not, the provider
    // refuses the decision with not_authority; the epoch sent does not matter.
    let epoch = session
        .query(
            "knowledge.authority.get",
            object(vec![("scope", scope.clone())]),
        )
        .ok()
        .and_then(|binding| int(&at(&binding, &["epoch"])))
        .unwrap_or(1);
    // The latest decision about exactly this revision, which a new decision
    // must name as the one it replaces.
    let history = session.query(
        "knowledge.claim.history",
        object(vec![("claim", string(decision.claim))]),
    )?;
    let latest = at(&history, &["decisions"])
        .as_array()
        .unwrap_or_default()
        .iter()
        .rfind(|d| canonical(&at(d, &["claim"])) == canonical(&claim_reference))
        .map(|d| at(d, &["decision"]));

    let mut payload = vec![
        ("claim", claim_reference),
        ("decision", string(decision.value)),
    ];
    if let Some(used) = decision.permitted_use {
        payload.push(("permitted_use", string(used)));
    }
    if let Some(latest) = latest {
        payload.push(("supersedes_decision", latest));
    }
    payload.push((
        "validation_basis",
        object(vec![
            ("evidence", Value::Array(vec![])),
            ("receipts", Value::Array(vec![])),
        ]),
    ));
    payload.push(("rationale", string(decision.rationale)));
    let recorded = session.command(
        "knowledge.decision.record",
        &format!("decision.{}", decision.id),
        subject("knowledge.decision", decision.id),
        0,
        Some(epoch),
        object(payload),
    )?;
    print(&at(&recorded, &["outcome"]));
    Ok(())
}

/// Where an evaluation's target comes from: a file, or a repository whose
/// identity is computed here (PROTOCOL-PIN section 5).
pub enum Target<'a> {
    File(&'a Path),
    Repository {
        path: &'a Path,
        id: &'a str,
        commit: &'a str,
        environment: Option<&'a Path>,
    },
}

/// The target basis for a repository: its root tree, its dirty snapshot when
/// the working tree differs from the commit, and the environment digest when
/// a fact set is declared.
pub fn repository_basis(
    path: &Path,
    id: &str,
    commit: &str,
    environment: Option<&Path>,
) -> Result<(Value, Value), String> {
    let basis = cbr_identity::git_basis(path, commit).map_err(|e| e.to_string())?;
    let mut repository = vec![("id", string(id)), ("tree", string(&basis.tree))];
    // The working tree describes the target only when the commit named is the
    // one checked out; an earlier commit has no uncommitted changes.
    let head = cbr_identity::git_basis(path, "HEAD").map_err(|e| e.to_string())?;
    let snapshot = if head.commit == basis.commit {
        cbr_identity::dirty_snapshot(path).map_err(|e| e.to_string())?
    } else {
        None
    };
    if let Some(snapshot) = &snapshot {
        repository.push((
            "dirty",
            object(vec![("snapshot_digest", string(&snapshot.digest))]),
        ));
    }
    let mut target = vec![("repositories", Value::Array(vec![object(repository)]))];
    let mut records = vec![
        ("commit", string(&basis.commit)),
        ("snapshot", snapshot.map_or(Value::Null, |s| s.document)),
    ];
    if let Some(facts) = environment {
        let bytes = std::fs::read(facts).map_err(|e| e.to_string())?;
        let facts = cbr_identity::parse_facts(&bytes).map_err(|e| e.to_string())?;
        let (digest, record) = cbr_identity::environment(&facts).map_err(|e| e.to_string())?;
        target.push(("environment", string(&digest)));
        records.push(("environment", record));
    }
    target.push(("completeness", string("complete")));
    Ok((object(target), object(records)))
}

pub fn evaluate(
    options: &Options,
    evaluation: &str,
    claim: &str,
    revision: Option<i64>,
    target: &Target<'_>,
) -> Result<(), String> {
    let target = match target {
        Target::File(path) => read_object(path)?,
        Target::Repository {
            path,
            id,
            commit,
            environment,
        } => repository_basis(path, id, commit, *environment)?.0,
    };
    let mut session = open(options)?;
    let claim_reference = at(&reference(&mut session, claim, revision)?, &["reference"]);
    let evaluated = session.command(
        "knowledge.applicability.evaluate",
        &format!("evaluation.{evaluation}"),
        subject("knowledge.evaluation", evaluation),
        0,
        None,
        object(vec![("claim", claim_reference), ("target", target)]),
    )?;
    print(&at(&evaluated, &["outcome"]));
    Ok(())
}

pub fn inspect(options: &Options, claim: &str, revision: Option<i64>) -> Result<(), String> {
    let mut session = open(options)?;
    let mut payload = vec![("claim", string(claim))];
    if let Some(revision) = revision {
        payload.push(("revision", Value::Int(revision)));
    }
    print(&session.query("knowledge.claim.inspect", object(payload))?);
    Ok(())
}

pub fn history(options: &Options, claim: &str) -> Result<(), String> {
    let mut session = open(options)?;
    print(&session.query(
        "knowledge.claim.history",
        object(vec![("claim", string(claim))]),
    )?);
    Ok(())
}

pub fn bind(options: &Options, scope: &str, authority: &str) -> Result<(), String> {
    let mut session = open(options)?;
    let bound = session.command(
        "knowledge.authority.bind",
        &format!("authority.{scope}.bind"),
        subject("knowledge.authority", scope),
        0,
        None,
        object(vec![("authority", string(authority))]),
    )?;
    print(&at(&bound, &["outcome"]));
    Ok(())
}
