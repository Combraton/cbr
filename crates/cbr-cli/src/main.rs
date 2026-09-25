//! `cbr`: a command-line client of CBR's public protocol.
//!
//! It has **no internal shortcut**. It opens the provider's Unix socket,
//! authenticates with a handed-off credential, negotiates, and uses the same
//! `evidence/1` and `knowledge/1` operations any other client would. Nothing
//! here opens the store or links the provider's code, so what `cbr` can do is
//! exactly what the protocol lets a client do.
//!
//! ```text
//! cbr ingest <file> [--media-type T] [--scope S] [--source-kind K] [--repo PATH [--commit REV]]
//! cbr fetch <artifact> --digest D --out FILE
//! cbr propose <claim> --content FILE
//! cbr revise <claim> --content FILE
//! cbr decide <decision> --claim C [--revision N] --decision V [--use U] --rationale TEXT
//! cbr evaluate <evaluation> --claim C [--revision N]
//!     (--target FILE | --repo PATH --repo-id ID [--commit REV] [--environment FACTS])
//! cbr inspect <claim> [--revision N]
//! cbr history <claim>
//! cbr authority bind <scope> --authority PRINCIPAL
//! cbr basis --repo PATH --repo-id ID [--commit REV] [--environment FACTS]
//! cbr context <request> --repo PATH --repo-id ID [--commit REV] [--also ID=PATH]…
//!     --want <item>=source:<path>|claim:<claim>|evidence:<artifact>@<digest> …
//!     [--obligation O] [--selector TEXT] [--task TEXT]
//!     [--capacity BYTES] [--deadline INSTANT] [--investigation N]
//! cbr request <request>
//! cbr cancel <request>
//! cbr packet <request> [--revision N] [--excerpt SECTION]
//! cbr expand <request> <citation> [--revision N] [--offset O] [--length L] [--out FILE]
//! ```
//!
//! Every verb except `basis` takes `--socket PATH --credential-file PATH` and
//! an optional `--grant ID`. `basis` runs locally and prints the target basis
//! and the records behind it, for writing claim content by hand.
//!
//! `ingest` prints `artifact`, `digest` and `size` lines. `fetch` writes the
//! bytes only after checking that what it assembled has the digest it asked
//! for; a mismatch writes nothing and exits nonzero. `expand` reads the bytes
//! a packet's citation names, the whole artifact or a range of it, and checks
//! a whole one against the cited digest the same way; with `--out` it prints
//! one line saying what it wrote, and without it the bytes are the whole of
//! standard output. The knowledge verbs print the outcome or result as
//! canonical JSON.

mod context;
mod evidence;
mod knowledge;
mod session;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Options {
    pub socket: PathBuf,
    pub credential_file: PathBuf,
    pub grant: Option<String>,
    pub media_type: String,
    pub scope: String,
    pub source_kind: String,
    pub repo: Option<PathBuf>,
    pub commit: String,
}

const USAGE: &str = "usage: cbr ingest|fetch|propose|revise|decide|evaluate|inspect|history|\
                     authority bind|basis|context|request|cancel|packet|expand …";

/// Options that take a value; everything else starting `--` is refused.
const VALUED: [&str; 29] = [
    "--socket",
    "--authority",
    "--credential-file",
    "--grant",
    "--digest",
    "--out",
    "--media-type",
    "--scope",
    "--source-kind",
    "--repo",
    "--repo-id",
    "--commit",
    "--environment",
    "--content",
    "--claim",
    "--revision",
    "--decision",
    "--use",
    "--rationale",
    "--want",
    "--obligation",
    "--selector",
    "--task",
    "--capacity",
    "--deadline",
    "--investigation",
    "--excerpt",
    "--offset",
    "--length",
];

fn run() -> Result<(), String> {
    let mut argv = std::env::args().skip(1);
    let verb = argv.next().ok_or(USAGE)?;
    let mut positional = Vec::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut wants: Vec<String> = Vec::new();
    let mut also: Vec<String> = Vec::new();
    let mut target = None;
    while let Some(argument) = argv.next() {
        if argument == "--want" {
            wants.push(argv.next().ok_or("--want needs a value")?);
        } else if argument == "--also" {
            also.push(argv.next().ok_or("--also needs <id>=<path>")?);
        } else if argument == "--target" {
            target = Some(argv.next().ok_or("--target needs a value")?);
        } else if VALUED.contains(&argument.as_str()) {
            let value = argv.next().ok_or(format!("{argument} needs a value"))?;
            values.insert(argument, value);
        } else if argument.starts_with("--") {
            return Err(format!("unknown option {argument}"));
        } else {
            positional.push(argument);
        }
    }
    let value = |name: &str| values.get(name).cloned();
    let required = |name: &str| value(name).ok_or(format!("{name} is required"));
    let revision = value("--revision")
        .map(|r| r.parse::<i64>().map_err(|_| "--revision is a number"))
        .transpose()?;

    if verb == "basis" {
        let repo = PathBuf::from(required("--repo")?);
        let (target, records) = knowledge::repository_basis(
            &repo,
            &required("--repo-id")?,
            &value("--commit").unwrap_or_else(|| "HEAD".into()),
            value("--environment").as_deref().map(Path::new),
        )?;
        println!("{}", session::canonical(&target));
        println!("{}", session::canonical(&records));
        return Ok(());
    }

    let options = Options {
        socket: PathBuf::from(required("--socket")?),
        credential_file: PathBuf::from(required("--credential-file")?),
        grant: value("--grant"),
        media_type: value("--media-type").unwrap_or_else(|| "application/octet-stream".into()),
        scope: value("--scope").unwrap_or_else(|| "local".into()),
        source_kind: value("--source-kind").unwrap_or_else(|| "file".into()),
        repo: value("--repo").map(PathBuf::from),
        commit: value("--commit").unwrap_or_else(|| "HEAD".into()),
    };
    let positional: Vec<&str> = positional.iter().map(String::as_str).collect();
    match (verb.as_str(), positional.as_slice()) {
        ("ingest", [file]) => evidence::ingest(&options, Path::new(file)),
        ("context", [request]) => context::submit(
            &options,
            request,
            &PathBuf::from(required("--repo")?),
            &required("--repo-id")?,
            &options.commit,
            &wants,
            &also,
            &value("--obligation").unwrap_or_else(|| "advisory".into()),
            value("--selector").as_deref(),
            &value("--task").unwrap_or_else(|| "context".into()),
            value("--capacity")
                .as_deref()
                .map_or(Ok(65536), str::parse)
                .map_err(|_| "--capacity is a number of bytes")?,
            &value("--deadline").unwrap_or_else(|| "2099-01-01T00:00:00Z".into()),
            value("--investigation")
                .as_deref()
                .map_or(Ok(0), str::parse)
                .map_err(|_| "--investigation is a number")?,
        ),
        ("cancel", [request]) => context::cancel(&options, request),
        ("request", [request]) => context::inspect(&options, request),
        ("packet", [request]) => context::packet(
            &options,
            request,
            value("--revision")
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(|_| "--revision is a number")?,
            value("--excerpt").as_deref(),
        ),
        ("expand", [request, citation]) => context::expand(
            &options,
            request,
            citation,
            revision,
            value("--offset")
                .as_deref()
                .map_or(Ok(0), str::parse)
                .map_err(|_| "--offset is a number of bytes")?,
            // An empty range is not a question, so zero is refused here
            // rather than asked.
            match value("--length").map(|length| length.parse::<u64>()) {
                None => None,
                Some(Ok(length)) if length > 0 => Some(length),
                Some(_) => return Err("--length is a positive number of bytes".into()),
            },
            value("--out").as_deref().map(Path::new),
        ),
        ("fetch", [artifact]) => evidence::fetch(
            &options,
            artifact,
            &required("--digest")?,
            Path::new(&required("--out")?),
        ),
        ("propose", [claim]) => {
            knowledge::propose(&options, claim, Path::new(&required("--content")?))
        }
        ("revise", [claim]) => {
            knowledge::revise(&options, claim, Path::new(&required("--content")?))
        }
        ("decide", [decision]) => knowledge::decide(
            &options,
            &knowledge::Decision {
                id: decision,
                claim: &required("--claim")?,
                revision,
                value: &required("--decision")?,
                permitted_use: value("--use").as_deref(),
                rationale: &required("--rationale")?,
            },
        ),
        ("evaluate", [evaluation]) => {
            let claim = required("--claim")?;
            match &target {
                Some(file) => knowledge::evaluate(
                    &options,
                    evaluation,
                    &claim,
                    revision,
                    &knowledge::Target::File(Path::new(file)),
                ),
                None => {
                    let repo = PathBuf::from(required("--repo")?);
                    let environment = value("--environment");
                    knowledge::evaluate(
                        &options,
                        evaluation,
                        &claim,
                        revision,
                        &knowledge::Target::Repository {
                            path: &repo,
                            id: &required("--repo-id")?,
                            commit: &options.commit,
                            environment: environment.as_deref().map(Path::new),
                        },
                    )
                }
            }
        }
        ("inspect", [claim]) => knowledge::inspect(&options, claim, revision),
        ("history", [claim]) => knowledge::history(&options, claim),
        ("authority", ["bind", scope]) => {
            knowledge::bind(&options, scope, &required("--authority")?)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("cbr: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}
