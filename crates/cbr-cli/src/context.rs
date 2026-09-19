//! `cbr` verbs over `context/1`: submit a request, and read the packet it
//! produced.
//!
//! The point of these two verbs is that a journey has a public entry point.
//! Everything they do is ordinary protocol: `context.request.submit`,
//! `context.request.inspect` and `context.packet.inspect`, over the same
//! socket and under the same credential as every other verb. The basis is
//! resolved here, from the checkout the caller already has, exactly as
//! `cbr basis` does — the provider is never sent a path.
//!
//! An item is written `<item-id>=<kind>:<value>`, where the kind is one of
//! `source`, `claim` or `evidence`. That is the whole of what a check can be
//! (CONTEXT section 3): satisfaction has to be decidable, so "understand the
//! repository" is not something this client can ask for.

use std::path::Path;

use cbr_encoding::Value;

use crate::Options;
use crate::session::{Session, at, canonical, object, string, subject};

const REQUEST: &str = "context.request";

fn open(options: &Options) -> Result<Session, String> {
    Session::open(
        &options.socket,
        &options.credential_file,
        &["context"],
        options.grant.clone(),
    )
}

fn print(value: &Value) {
    println!("{}", canonical(value));
}

/// One `--want` argument: `<item-id>=<kind>:<value>`.
fn item(
    specification: &str,
    obligation: &str,
    repository: &str,
    selector: Option<&str>,
) -> Result<Value, String> {
    let (item_id, rest) = specification
        .split_once('=')
        .ok_or_else(|| format!("--want takes <item-id>=<kind>:<value>, not {specification:?}"))?;
    let (kind, value) = rest
        .split_once(':')
        .ok_or_else(|| format!("--want takes <item-id>=<kind>:<value>, not {specification:?}"))?;
    let (check, selector_value) = match kind {
        "source" => (
            object(vec![
                ("kind", string("source_included")),
                ("repository", string(repository)),
                ("path", string(value)),
            ]),
            value.to_string(),
        ),
        "claim" => (
            object(vec![
                ("kind", string("claim_included")),
                (
                    "claim",
                    object(vec![("provider", string("cbr")), ("claim", string(value))]),
                ),
            ]),
            value.to_string(),
        ),
        "evidence" => {
            let (artifact, digest) = value
                .split_once('@')
                .ok_or("an evidence item is <item-id>=evidence:<artifact>@<digest>")?;
            (
                object(vec![
                    ("kind", string("evidence_included")),
                    (
                        "evidence",
                        object(vec![
                            ("provider", string("cbr")),
                            ("artifact", subject("evidence.artifact", artifact)),
                            ("digest", string(digest)),
                        ]),
                    ),
                ]),
                artifact.to_string(),
            )
        }
        other => return Err(format!("{other:?} is not an item kind")),
    };
    Ok(object(vec![
        ("item_id", string(item_id)),
        (
            "selector",
            object(vec![
                ("kind", string(kind)),
                ("value", string(selector.unwrap_or(selector_value.as_str()))),
            ]),
        ),
        ("obligation", string(obligation)),
        ("reliance", string("evidence")),
        ("selected_by", string("cbr-cli")),
        ("check", check),
    ]))
}

/// `cbr context <request> --repo PATH --repo-id ID --want …`
#[allow(clippy::too_many_arguments)]
pub fn submit(
    options: &Options,
    request: &str,
    repository: &Path,
    repository_id: &str,
    commit: &str,
    wants: &[String],
    also: &[String],
    obligation: &str,
    selector: Option<&str>,
    task: &str,
    capacity: i64,
    deadline: &str,
    investigation: i64,
) -> Result<(), String> {
    let (target, _) = crate::knowledge::repository_basis(repository, repository_id, commit, None)?;
    // A task can span repositories, and a basis names every one it is about
    // (CONTEXT section 3). `--also <id>=<path>` adds one, resolved the same
    // way and at its own `HEAD`: the provider is sent trees, never paths.
    let mut extra = Vec::new();
    for specification in also {
        let (id, path) = specification
            .split_once('=')
            .ok_or("--also takes <id>=<path>")?;
        let (other, _) = crate::knowledge::repository_basis(Path::new(path), id, "HEAD", None)?;
        extra.extend(
            at(&other, &["repositories"])
                .as_array()
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
    }
    // A context basis says what the working tree is, which a knowledge target
    // does not have to: `dirty` carries the snapshot when there is one, and
    // `workspace` says whether there was anything to snapshot. A dirty tree
    // with a snapshot is still a complete basis; without one it is partial,
    // which the provider checks and this client does not pretend otherwise.
    let repositories: Vec<Value> = at(&target, &["repositories"])
        .as_array()
        .unwrap_or_default()
        .iter()
        .chain(extra.iter())
        .map(|entry| {
            let mut entry = entry.clone();
            let dirty = entry.get("dirty").is_some_and(|value| value.is_object());
            crate::session::set(
                &mut entry,
                "workspace",
                string(if dirty { "dirty" } else { "clean" }),
            );
            if !dirty {
                crate::session::set(&mut entry, "dirty", Value::Null);
            }
            entry
        })
        .collect();
    let complete = repositories.iter().all(|entry| {
        entry.get("dirty").is_some_and(Value::is_object)
            || entry.get("workspace").and_then(Value::as_str) == Some("clean")
    });
    let basis = object(vec![
        ("repositories", Value::Array(repositories)),
        (
            "completeness",
            string(if complete { "complete" } else { "partial" }),
        ),
    ]);

    let mut items = Vec::new();
    for want in wants {
        items.push(item(want, obligation, repository_id, selector)?);
    }
    if items.is_empty() {
        return Err("a request names at least one item".into());
    }

    let mut session = open(options)?;
    let outcome = session.command(
        "context.request.submit",
        &format!("{request}.submit"),
        subject(REQUEST, request),
        0,
        None,
        object(vec![
            (
                "consumer",
                object(vec![
                    ("task", string(task)),
                    ("principal", string("cbr-cli")),
                ]),
            ),
            ("basis", basis),
            ("items", Value::Array(items)),
            ("fallback", string("proceed_with_gap")),
            (
                "limits",
                object(vec![
                    ("deadline", string(deadline)),
                    (
                        "investigation",
                        object(vec![
                            ("units", string("calls")),
                            ("amount", Value::Int(investigation)),
                        ]),
                    ),
                    (
                        "output_capacity",
                        object(vec![
                            ("units", string("bytes")),
                            ("amount", Value::Int(capacity)),
                        ]),
                    ),
                ]),
            ),
        ]),
    )?;
    print(&outcome);
    Ok(())
}

/// `cbr request <request>`: what the request looks like now.
pub fn inspect(options: &Options, request: &str) -> Result<(), String> {
    let mut session = open(options)?;
    let result = session.query(
        "context.request.inspect",
        object(vec![("request", string(request))]),
    )?;
    print(&result);
    Ok(())
}

/// `cbr packet <request> [--revision N] [--excerpt BYTES]`: the packet a
/// request published, as any consumer would read it. `--excerpt` asks for
/// that many bytes of the packet's own text.
pub fn packet(
    options: &Options,
    request: &str,
    revision: Option<i64>,
    excerpt: Option<&str>,
) -> Result<(), String> {
    let mut session = open(options)?;
    let revision = match revision {
        Some(revision) => revision,
        None => {
            let inspected = session.query(
                "context.request.inspect",
                object(vec![("request", string(request))]),
            )?;
            let published = at(&inspected, &["packets"]);
            let last = published
                .as_array()
                .and_then(<[Value]>::last)
                .map(|entry| at(entry, &["reference", "revision"]));
            match last {
                Some(Value::Int(revision)) => revision,
                _ => return Err(format!("{request} has published no packet")),
            }
        }
    };
    let mut payload = vec![
        ("packet", string(request)),
        ("revision", Value::Int(revision)),
    ];
    if let Some(max_bytes) = excerpt {
        let max_bytes: i64 = max_bytes
            .parse()
            .map_err(|_| "--excerpt is a number of bytes")?;
        payload.push(("max_bytes", Value::Int(max_bytes)));
    }
    let result = session.query("context.packet.inspect", object(payload))?;
    print(&result);
    Ok(())
}
