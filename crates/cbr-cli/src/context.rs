//! `cbr` verbs over `context/1`: submit a request, read the packet it
//! produced, and follow one of that packet's citations back to the bytes it
//! cites.
//!
//! The point of these verbs is that a journey has a public entry point.
//! Everything they do is ordinary protocol: `context.request.submit`,
//! `context.request.inspect`, `context.packet.inspect` and `context.expand`,
//! over the same socket and under the same credential as every other verb. The basis is
//! resolved here, from the checkout the caller already has, exactly as
//! `cbr basis` does — the provider is never sent a path.
//!
//! An item is written `<item-id>=<kind>:<value>`, where the kind is one of
//! `source`, `claim` or `evidence`. That is the whole of what a check can be
//! (CONTEXT section 3): satisfaction has to be decidable, so "understand the
//! repository" is not something this client can ask for.

use std::io::Write;
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
            // **No provider**, which is the provider being asked (EVIDENCE
            // section 2): the artifact is one `cbr ingest` sealed over this
            // same socket, and on a fresh store no answer the protocol gives
            // an authority principal says yet what that provider calls
            // itself: a claim reference, a packet citation or a grant's
            // audience would, once one exists. It used to
            // say `cbr` here, and a provider called anything else compiled
            // a citation its own `context.expand` refused as another
            // provider's. The packet's citation names the provider by its
            // own id.
            (
                object(vec![
                    ("kind", string("evidence_included")),
                    (
                        "evidence",
                        object(vec![
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

/// `cbr cancel <request>`: stop preparing it.
///
/// **The payload is empty** and the request is the command's subject, so
/// there is nothing here to get wrong. What it does on the provider is
/// the interesting half: a job nobody is waiting for any more lets go of
/// everything it asked a model, and a call still in flight stops holding
/// the concurrency bound — its answer is never read, so nothing it chose
/// reaches a packet.
pub fn cancel(options: &Options, request: &str) -> Result<(), String> {
    let mut session = open(options)?;
    // **The revision this client last saw.** A command carries the
    // revision it expects, so a cancel racing a publication is refused
    // rather than applied to a request that has moved on. Read here
    // rather than asked for on the command line: the caller is
    // cancelling *this* request, not a particular version of it, and a
    // number they had to look up first would be a number they could get
    // wrong.
    let seen = session.query(
        "context.request.inspect",
        object(vec![("request", string(request))]),
    )?;
    let revision = match at(&seen, &["revision"]) {
        Value::Int(revision) => revision,
        other => return Err(format!("{request} has no revision: {other:?}")),
    };
    let outcome = session.command(
        "context.request.cancel",
        &format!("{request}.cancel"),
        subject(REQUEST, request),
        revision,
        None,
        Value::Object(Vec::new()),
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
        None => last_published(&mut session, request)?,
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

/// The last packet revision `request` published, which is what `packet` and
/// `expand` read when no `--revision` is given.
fn last_published(session: &mut Session, request: &str) -> Result<i64, String> {
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
        Some(Value::Int(revision)) => Ok(revision),
        _ => Err(format!("{request} has published no packet")),
    }
}

/// The protocol's ceiling on one `context.expand` read: `max_bytes` is an
/// integer from 1 to this (CONTEXT section 10).
pub const MAX_BYTES: u64 = 16_777_216;

/// One answer of `context.expand`: the offset it starts at, the bytes it
/// carries, and the size of the whole artifact.
#[derive(Debug)]
pub struct Piece {
    pub offset: u64,
    pub data: Vec<u8>,
    pub size: u64,
}

/// **Put a range of an artifact back together from reads that may each be
/// short.**
///
/// `read(offset, max_bytes)` performs one call. The provider may answer
/// with fewer bytes than asked — it halves a read until it fits the
/// caller's frame — so this asks again from wherever the last answer
/// ended, for what is still missing, until `length` bytes from `offset`
/// are here or the artifact ends. It returns the bytes and the artifact's
/// size.
///
/// Everything an answer could get wrong is an error rather than a guess,
/// because the bytes are going to be presented as the artifact's own:
///
/// - **an offset past the end.** The provider clamps it and answers with
///   nothing, which would otherwise print as an empty success. An offset
///   exactly at the end is the empty range there, as a slice is;
/// - **no bytes before the end**, as `fetch` refuses it: a loop that
///   accepted it would never finish or would finish short;
/// - **more bytes than were asked for, or than the range holds.** Trimming
///   them would hide a provider that does not do what the protocol says;
/// - **an answer from somewhere else**, or an artifact whose size changed
///   between two reads.
pub fn assemble(
    offset: u64,
    length: Option<u64>,
    mut read: impl FnMut(u64, u64) -> Result<Piece, String>,
) -> Result<(Vec<u8>, u64), String> {
    let mut assembled: Vec<u8> = Vec::new();
    // Where the range ends and how large the artifact is, both known only
    // once the first answer has said the size.
    let mut known: Option<(u64, u64)> = None;
    loop {
        let position = offset + assembled.len() as u64;
        let wanted = match known {
            Some((end, _)) => end - position,
            None => length.unwrap_or(MAX_BYTES),
        };
        let asked = wanted.min(MAX_BYTES);
        let piece = read(position, asked)?;
        let (end, size) = match known {
            Some((end, size)) => {
                if piece.size != size {
                    return Err(format!(
                        "the artifact was {size} bytes and is now {}",
                        piece.size
                    ));
                }
                (end, size)
            }
            None => {
                if offset > piece.size {
                    return Err(format!(
                        "offset {offset} is past the end of the artifact, which is {} bytes",
                        piece.size
                    ));
                }
                let end = length.map_or(piece.size, |length| {
                    offset.saturating_add(length).min(piece.size)
                });
                known = Some((end, piece.size));
                (end, piece.size)
            }
        };
        if piece.offset != position {
            return Err(format!(
                "asked for bytes at {position} and was answered from {}",
                piece.offset
            ));
        }
        let got = piece.data.len() as u64;
        if got > asked.min(end - position) {
            return Err(format!(
                "the provider returned {got} bytes at {position}, more than the {} asked for",
                asked.min(end - position)
            ));
        }
        if got == 0 && position < end {
            return Err("the provider returned no bytes before the end".into());
        }
        assembled.extend(piece.data);
        if position + got >= end {
            return Ok((assembled, size));
        }
    }
}

/// **The whole artifact is checked against the digest its citation names**
/// before anything is written or printed, as `fetch` checks against the
/// digest it asked for. A range cannot be: a digest is of the whole.
pub fn check_whole(bytes: &[u8], offset: u64, size: u64, digest: &str) -> Result<(), String> {
    if offset != 0 || bytes.len() as u64 != size {
        return Ok(());
    }
    let computed = cbr_encoding::digest_bytes(bytes);
    if computed != digest {
        return Err(format!(
            "assembled bytes have digest {computed}, not the cited {digest}; nothing written"
        ));
    }
    Ok(())
}

/// **Check, then hand over.** The bytes are checked first, by
/// [`check_whole`], and only then written to `out` through a staged file
/// — with `said` as the one line `stdout` gets — or, with no `out`, given
/// to `stdout` as they are. A check that fails leaves no file, no staged
/// file and nothing on `stdout`.
pub fn deliver(
    bytes: &[u8],
    offset: u64,
    size: u64,
    digest: &str,
    out: Option<&Path>,
    said: &str,
    stdout: &mut impl Write,
) -> Result<(), String> {
    check_whole(bytes, offset, size, digest)?;
    match out {
        Some(out) => {
            let staged = out.with_extension("cbr-partial");
            std::fs::write(&staged, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&staged, out).map_err(|e| e.to_string())?;
            writeln!(stdout, "{said}").map_err(|e| e.to_string())?;
        }
        None => stdout.write_all(bytes).map_err(|e| e.to_string())?,
    }
    stdout.flush().map_err(|e| e.to_string())
}

/// **One `context.expand` answer, as a [`Piece`]**, or why it is not one.
///
/// `cited` is the evidence the first answer named, kept here: every later
/// answer must name the same, or the pieces are of different things. The
/// excerpt's `length` must be the length of the bytes it carries, and its
/// `offset` and `size` must be there, since [`assemble`] decides with
/// them.
pub fn answer(result: &Value, cited: &mut Option<Value>) -> Result<Piece, String> {
    let evidence = at(result, &["evidence"]);
    match cited {
        None => *cited = Some(evidence),
        Some(seen) if *seen != evidence => {
            return Err("the citation named different evidence between two reads".into());
        }
        Some(_) => {}
    }
    let excerpt = at(result, &["excerpt"]);
    let number = |name: &str| match excerpt.get(name) {
        Some(Value::Int(n)) if *n >= 0 => Ok(*n as u64),
        _ => Err(format!("context.expand returned no {name}")),
    };
    let data = excerpt
        .get("data_base64")
        .and_then(Value::as_str)
        .and_then(cbr_encoding::decode_base64)
        .ok_or("the provider sent no valid base64")?;
    if data.len() as u64 != number("length")? {
        return Err("the excerpt's length is not the length of its bytes".into());
    }
    Ok(Piece {
        offset: number("offset")?,
        data,
        size: number("size")?,
    })
}

/// `cbr expand <request> <citation> [--revision N] [--offset O] [--length L]
/// [--out FILE]`: the bytes a packet's citation names.
///
/// The revision is the request's last published one unless given, found as
/// `packet` finds it. With `--out` the bytes are written through a staged
/// file and one line says what they are; without it they are the whole of
/// standard output, so they can be piped. **A refusal is whatever the
/// provider said**, and the provider says the same thing for a citation
/// that does not exist, one the grant cannot read and one in a request it
/// cannot read, so this adds nothing that would tell them apart.
pub fn expand(
    options: &Options,
    request: &str,
    citation: &str,
    revision: Option<i64>,
    offset: u64,
    length: Option<u64>,
    out: Option<&Path>,
) -> Result<(), String> {
    let mut session = open(options)?;
    let revision = match revision {
        Some(revision) => revision,
        None => last_published(&mut session, request)?,
    };
    let mut cited: Option<Value> = None;
    let (bytes, size) = assemble(offset, length, |from, max_bytes| {
        let result = session.query(
            "context.expand",
            object(vec![
                ("packet", string(request)),
                ("revision", Value::Int(revision)),
                ("citation", string(citation)),
                (
                    "offset",
                    Value::Int(i64::try_from(from).map_err(|_| "--offset is too large")?),
                ),
                // At most `MAX_BYTES`, which `assemble` never exceeds.
                ("max_bytes", Value::Int(max_bytes as i64)),
            ]),
        )?;
        answer(&result, &mut cited)
    })?;
    let evidence = cited.unwrap_or(Value::Null);
    let text = |path: &[&str]| {
        at(&evidence, path)
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("the citation names no {}", path.join(".")))
    };
    let (artifact, digest) = (text(&["artifact", "id"])?, text(&["digest"])?);
    let said = format!(
        "wrote {} bytes at offset {offset} of {size}, citation {citation}, \
         evidence {artifact} at {digest}",
        bytes.len()
    );
    deliver(
        &bytes,
        offset,
        size,
        &digest,
        out,
        &said,
        &mut std::io::stdout().lock(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider that answers as `context.expand` does — the offset
    /// clamped to the size, at most `max_bytes` — and never more than
    /// `frame` bytes at once, as the halving to the caller's frame does.
    /// Every `(offset, max_bytes)` asked is kept.
    fn provider<'a>(
        source: &'a [u8],
        frame: usize,
        asked: &'a mut Vec<(u64, u64)>,
    ) -> impl FnMut(u64, u64) -> Result<Piece, String> + 'a {
        move |offset, max_bytes| {
            asked.push((offset, max_bytes));
            let from = (offset as usize).min(source.len());
            let take = (max_bytes as usize).min(frame).min(source.len() - from);
            Ok(Piece {
                offset: from as u64,
                data: source[from..from + take].to_vec(),
                size: source.len() as u64,
            })
        }
    }

    fn source() -> Vec<u8> {
        (0..100u8).collect()
    }

    #[test]
    fn short_reads_are_put_back_together_asking_for_what_is_missing() {
        let source = source();
        let mut asked = Vec::new();
        let (bytes, size) = assemble(0, None, provider(&source, 7, &mut asked)).expect("assembles");
        assert_eq!((bytes, size), (source.clone(), 100));
        // The first read cannot know the size, so it asks for the ceiling;
        // every later one asks for exactly what is still missing.
        assert_eq!(asked[0], (0, MAX_BYTES));
        assert_eq!(asked[1], (7, 93));
        assert_eq!(asked.last(), Some(&(98, 2)));
        assert_eq!(asked.len(), 15);
    }

    #[test]
    fn a_range_is_its_bytes_whatever_the_reads_are_cut_into() {
        let source = source();
        for frame in [1, 3, 64, 1000] {
            for (offset, length, range) in [
                (10, Some(25), 10..35),
                (90, Some(10), 90..100),
                (95, Some(50), 95..100),
                (40, None, 40..100),
                (100, None, 100..100),
            ] {
                let mut asked = Vec::new();
                let (bytes, size) = assemble(offset, length, provider(&source, frame, &mut asked))
                    .expect("assembles");
                assert_eq!(bytes, &source[range.clone()], "frame {frame}, {range:?}");
                assert_eq!(size, 100);
                assert!(
                    asked.iter().all(|&(_, max)| (1..=MAX_BYTES).contains(&max)),
                    "{asked:?}"
                );
            }
        }
    }

    #[test]
    fn no_read_asks_for_more_than_the_protocols_ceiling() {
        let mut asked = Vec::new();
        let mut read = |offset: u64, max_bytes: u64| {
            asked.push(max_bytes);
            // An artifact three ceilings long, served one ceiling at a time
            // without holding it: only the length matters.
            Ok(Piece {
                offset,
                data: vec![0; max_bytes.min(3 * MAX_BYTES - offset) as usize],
                size: 3 * MAX_BYTES,
            })
        };
        let (bytes, _) = assemble(0, Some(2 * MAX_BYTES + 5), &mut read).expect("assembles");
        assert_eq!(bytes.len() as u64, 2 * MAX_BYTES + 5);
        assert_eq!(asked, vec![MAX_BYTES, MAX_BYTES, 5]);
    }

    #[test]
    fn no_bytes_before_the_end_is_an_error() {
        let source = source();
        let mut calls = 0;
        let refused = assemble(0, None, |offset, max_bytes| {
            calls += 1;
            // One honest answer, then nothing, from before the end.
            let take = if calls == 1 { 30 } else { 0 };
            Ok(Piece {
                offset,
                data: source[offset as usize..][..take.min(max_bytes as usize)].to_vec(),
                size: 100,
            })
        });
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("no bytes before the end")),
            "{refused:?}"
        );
        assert_eq!(calls, 2, "it asked again after the empty answer");
    }

    #[test]
    fn more_bytes_than_asked_for_is_an_error_and_is_not_trimmed() {
        let refused = assemble(0, Some(10), |offset, max_bytes| {
            Ok(Piece {
                offset,
                data: vec![7; max_bytes as usize + 1],
                size: 100,
            })
        });
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("more than")),
            "{refused:?}"
        );
        // More than the range holds is the same violation, even when the
        // read asked for more: here the artifact ends first.
        let refused = assemble(95, None, |offset, _| {
            Ok(Piece {
                offset,
                data: vec![7; 6],
                size: 100,
            })
        });
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("more than")),
            "{refused:?}"
        );
    }

    #[test]
    fn an_offset_past_the_end_is_an_error_and_one_at_the_end_is_empty() {
        let source = source();
        let mut asked = Vec::new();
        let refused = assemble(101, Some(5), provider(&source, 64, &mut asked));
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("past the end")),
            "{refused:?}"
        );
        let mut asked = Vec::new();
        let at_end = assemble(100, Some(5), provider(&source, 64, &mut asked));
        assert_eq!(at_end, Ok((Vec::new(), 100)));
    }

    #[test]
    fn an_answer_from_elsewhere_or_a_size_that_moves_is_an_error() {
        let elsewhere = assemble(10, Some(5), |_, _| {
            Ok(Piece {
                offset: 0,
                data: vec![1; 5],
                size: 100,
            })
        });
        assert!(
            elsewhere
                .as_ref()
                .is_err_and(|error| error.contains("answered from 0")),
            "{elsewhere:?}"
        );
        let mut size = 100;
        let moved = assemble(0, None, |offset, _| {
            size += 1;
            Ok(Piece {
                offset,
                data: vec![1; 10],
                size: size - 1,
            })
        });
        assert!(
            moved
                .as_ref()
                .is_err_and(|error| error.contains("is now 101")),
            "{moved:?}"
        );
    }

    #[test]
    fn only_the_whole_artifact_is_checked_and_a_mismatch_is_refused() {
        let source = source();
        let digest = cbr_encoding::digest_bytes(&source);
        assert_eq!(check_whole(&source, 0, 100, &digest), Ok(()));
        let mut altered = source.clone();
        altered[50] ^= 1;
        let refused = check_whole(&altered, 0, 100, &digest);
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("nothing written")),
            "{refused:?}"
        );
        // A range has no digest of its own to be checked against.
        assert_eq!(check_whole(&altered[10..], 10, 100, &digest), Ok(()));
        assert_eq!(check_whole(&altered[..90], 0, 100, &digest), Ok(()));
    }

    /// **Nothing is handed over when the check fails**, whichever way it
    /// would have gone: no file at `--out`, no staged file beside it, and
    /// nothing on standard output — neither the line nor the bytes. A
    /// check that ran after the write or the print would leave one of
    /// them behind.
    #[test]
    fn a_mismatch_hands_over_nothing_and_leaves_no_staged_file() {
        let source = source();
        let digest = cbr_encoding::digest_bytes(&source);
        let mut altered = source.clone();
        altered[50] ^= 1;
        let directory = tempfile::tempdir().expect("temp dir");
        let out = directory.path().join("expanded.bin");

        let mut stdout = Vec::new();
        let refused = deliver(&altered, 0, 100, &digest, Some(&out), "said", &mut stdout);
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("nothing written")),
            "{refused:?}"
        );
        assert!(!out.exists(), "the file was written");
        assert!(
            !out.with_extension("cbr-partial").exists(),
            "the staged file was left"
        );
        assert!(stdout.is_empty(), "{stdout:?}");

        let mut stdout = Vec::new();
        let refused = deliver(&altered, 0, 100, &digest, None, "said", &mut stdout);
        assert!(refused.is_err(), "{refused:?}");
        assert!(stdout.is_empty(), "the bytes were printed");
    }

    /// The other arm: bytes that match are written through the staged
    /// file, which is gone afterwards, with the one line; or printed as
    /// they are, with nothing else.
    #[test]
    fn a_match_is_written_through_a_staged_file_or_printed_whole() {
        let source = source();
        let digest = cbr_encoding::digest_bytes(&source);
        let directory = tempfile::tempdir().expect("temp dir");
        let out = directory.path().join("expanded.bin");

        let mut stdout = Vec::new();
        deliver(&source, 0, 100, &digest, Some(&out), "said", &mut stdout).expect("delivers");
        assert_eq!(std::fs::read(&out).expect("the file"), source);
        assert!(!out.with_extension("cbr-partial").exists());
        assert_eq!(stdout, b"said\n");

        let mut stdout = Vec::new();
        deliver(&source, 0, 100, &digest, None, "said", &mut stdout).expect("delivers");
        assert_eq!(stdout, source);
    }

    /// A `context.expand` answer over `data`, naming `artifact`, with
    /// `length` as its excerpt says it.
    fn answered(artifact: &str, offset: i64, data: &[u8], length: i64) -> Value {
        let text = format!(
            r#"{{"citation":"c-log","evidence":{{"artifact":{{"id":"{artifact}","kind":"evidence.artifact"}},"digest":"sha256:{}","provider":"cbr"}},"excerpt":{{"data_base64":"{}","length":{length},"offset":{offset},"size":100}}}}"#,
            "0".repeat(64),
            cbr_encoding::encode_base64(data),
        );
        cbr_encoding::parse(text.as_bytes()).expect("canonical JSON")
    }

    #[test]
    fn an_answer_is_its_excerpts_bytes_where_it_says_they_are() {
        let mut cited = None;
        let piece = answer(&answered("a-1", 10, b"abc", 3), &mut cited).expect("an answer");
        assert_eq!(
            (piece.offset, piece.data, piece.size),
            (10, b"abc".to_vec(), 100)
        );
        assert_eq!(
            cited
                .as_ref()
                .map(|evidence| at(evidence, &["artifact", "id"])),
            Some(string("a-1"))
        );
        // A second answer naming the same evidence is another piece of it.
        answer(&answered("a-1", 13, b"de", 2), &mut cited).expect("the same evidence");
    }

    #[test]
    fn evidence_that_changes_between_two_reads_is_refused() {
        let mut cited = None;
        answer(&answered("a-1", 0, b"abc", 3), &mut cited).expect("the first answer");
        let refused = answer(&answered("a-2", 3, b"def", 3), &mut cited);
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("different evidence between two reads")),
            "{:?}",
            refused.map(|piece| piece.data)
        );
    }

    #[test]
    fn an_excerpt_whose_length_is_not_its_bytes_is_refused() {
        for length in [2, 4] {
            let refused = answer(&answered("a-1", 0, b"abc", length), &mut None);
            assert!(
                refused
                    .as_ref()
                    .is_err_and(|error| error.contains("length is not the length of its bytes")),
                "length {length}: {:?}",
                refused.map(|piece| piece.data)
            );
        }
    }
}
