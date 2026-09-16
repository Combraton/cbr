//! Black-box conformance runner for Combraton Protocol (draft 0.1).
//!
//! Subcommands:
//!   self-test       check this runner against conformance/vectors/encoding.json
//!   check-fixtures  validate fixtures: schema, unique ids, known requirement ids, negative kills
//!   run             run fixtures against a participant and write a result manifest
//!   check-mutants   prove every negative fixture fails its declared mutants

mod exec;
mod matcher;
mod participant;
mod schemas;
mod strict;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

use exec::{Context, Outcome, run_fixture};
use participant::Descriptor;
use schemas::Schemas;

const USAGE: &str = "usage: combraton-conformance <self-test|check-fixtures|run|check-mutants|version> [--repo DIR] [--participant FILE] [--out DIR] [--filter SUBSTRING] [--mutant NAME] [--client-mutant IMPLEMENTATION=NAME] [--fixtures DIR]";

struct Options {
    command: String,
    repo: PathBuf,
    participant: Option<PathBuf>,
    out: Option<PathBuf>,
    filter: Option<String>,
    mutant: Option<String>,
    /// Another fixture tree, for example one pinned at an accepted release, to run unmodified.
    fixtures: Option<PathBuf>,
    /// A mutant of one client implementation, `implementation=mutant`.
    client_mutant: Option<(String, String)>,
}

fn parse_options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    let mut options = Options {
        command,
        repo: PathBuf::from("."),
        participant: None,
        out: None,
        filter: None,
        mutant: None,
        fixtures: None,
        client_mutant: None,
    };
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--repo" => options.repo = PathBuf::from(value()?),
            "--participant" => options.participant = Some(PathBuf::from(value()?)),
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--filter" => options.filter = Some(value()?),
            "--mutant" => options.mutant = Some(value()?),
            "--fixtures" => options.fixtures = Some(PathBuf::from(value()?)),
            "--client-mutant" => {
                let raw = value()?;
                let (implementation, mutant) = raw
                    .split_once('=')
                    .ok_or("--client-mutant needs IMPLEMENTATION=NAME")?;
                options.client_mutant = Some((implementation.to_string(), mutant.to_string()));
            }
            other => return Err(format!("unexpected argument {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

struct Fixture {
    path: PathBuf,
    value: Value,
    digest: String,
}

impl Fixture {
    fn id(&self) -> &str {
        self.value["id"].as_str().unwrap_or_default()
    }
}

fn load_fixtures(root: &Path, filter: Option<&str>) -> Result<Vec<Fixture>, String> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let mut fixtures = Vec::new();
    for path in paths {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let value = strict::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let digest = strict::sha256(&strict::canonical(&value));
        let fixture = Fixture {
            path,
            value,
            digest,
        };
        if filter.is_none_or(|f| fixture.id().contains(f)) {
            fixtures.push(fixture);
        }
    }
    Ok(fixtures)
}

fn matrix_ids(repo: &Path) -> Result<BTreeSet<String>, String> {
    let text = std::fs::read_to_string(repo.join("docs/work/release-0.1/MATRIX.md"))
        .map_err(|e| format!("MATRIX.md: {e}"))?;
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("| "))
        .filter_map(|rest| rest.split(" |").next())
        .filter(|id| {
            id.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && id.contains('-')
                && !id.contains(' ')
        })
        .map(String::from)
        .collect())
}

/// The path of the first pattern object that has a `$` directive and other members.
fn mixed_directive(pattern: &Value, path: String) -> Option<String> {
    match pattern {
        Value::Object(map) => {
            if map.len() > 1 && map.keys().any(|k| k.starts_with('$')) {
                return Some(if path.is_empty() { "/".into() } else { path });
            }
            map.iter()
                .find_map(|(key, value)| mixed_directive(value, format!("{path}/{key}")))
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, value)| mixed_directive(value, format!("{path}/{i}"))),
        _ => None,
    }
}

fn check_fixtures(options: &Options, schemas: &Schemas) -> Result<bool, String> {
    let fixtures = load_fixtures(&options.repo.join("conformance/fixtures"), None)?;
    let known = matrix_ids(&options.repo)?;
    let mut known_mutants = BTreeSet::new();
    let mut known_barriers = BTreeSet::new();
    for entry in std::fs::read_dir(options.repo.join("conformance/participants"))
        .map_err(|e| e.to_string())?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let descriptor = Descriptor::load(&path)?;
            known_barriers.extend(
                descriptor.raw["claims"]["test_barriers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(String::from),
            );
            known_mutants.extend(descriptor.mutants);
        }
    }
    let mut ok = true;
    let mut ids = BTreeSet::new();
    for fixture in &fixtures {
        let label = fixture.path.display();
        for error in schemas.fixture.iter_errors(&fixture.value) {
            println!("{label}: schema: {} at {}", error, error.instance_path());
            ok = false;
        }
        if !ids.insert(fixture.id().to_string()) {
            println!("{label}: duplicate fixture id {}", fixture.id());
            ok = false;
        }
        for requirement in fixture.value["requirements"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if !known.contains(requirement.as_str().unwrap_or_default()) {
                println!("{label}: requirement {requirement} is not in MATRIX.md");
                ok = false;
            }
        }
        for mutant in fixture.value["kills"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !known_mutants.contains(mutant) {
                println!("{label}: declares mutant {mutant} that no participant descriptor lists");
                ok = false;
            }
        }
        for barrier in fixture.value["requires_barriers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !known_barriers.contains(barrier) {
                println!(
                    "{label}: requires barrier {barrier} that no participant descriptor declares"
                );
                ok = false;
            }
        }
        if let Some(expectations) = fixture.value["kill_expectations"].as_object() {
            for mutant in expectations.keys() {
                let declared = fixture.value["kills"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|m| m == mutant.as_str()));
                if !declared {
                    println!(
                        "{label}: kill_expectations names {mutant}, which kills does not declare"
                    );
                    ok = false;
                }
            }
        }
        // A pattern object with a `$` directive is that directive only; other members would be
        // silently ignored, so they are refused.
        for (index, step) in fixture.value["steps"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(path) = mixed_directive(&step["expect"], String::new()) {
                println!(
                    "{label}: step {index}: pattern at {path} mixes a $ directive with other members"
                );
                ok = false;
            }
        }
        for kill in fixture.value["client_kills"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let implementation = kill["implementation"].as_str().unwrap_or_default();
            let path = options
                .repo
                .join("conformance/thirdparty")
                .join(implementation)
                .join("participant.json");
            match Descriptor::load_client(&path) {
                Ok(client) if client.mutants.iter().any(|m| kill["mutant"] == m.as_str()) => {}
                Ok(_) => {
                    println!(
                        "{label}: client_kills names mutant {} that client {implementation} does not list",
                        kill["mutant"]
                    );
                    ok = false;
                }
                Err(error) => {
                    println!("{label}: client_kills: {error}");
                    ok = false;
                }
            }
        }
        let kills = fixture.value["kills"].as_array().map_or(0, Vec::len)
            + fixture.value["client_kills"].as_array().map_or(0, Vec::len);
        if fixture.value["polarity"] == "negative" && kills == 0 {
            println!("{label}: negative fixture declares no mutant it must fail");
            ok = false;
        }
    }
    println!(
        "check-fixtures: {} fixtures, {} matrix requirement ids, {}",
        fixtures.len(),
        known.len(),
        if ok { "ok" } else { "ERRORS" }
    );
    Ok(ok)
}

/// The protocol commit: `git rev-parse HEAD` in a checkout, or the `commit` recorded in
/// `RELEASE-SOURCE.json` of an extracted release bundle.
fn git_head(repo: &Path) -> Value {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| json!(String::from_utf8_lossy(&out.stdout).trim()))
        .or_else(|| {
            std::fs::read(repo.join("RELEASE-SOURCE.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .map(|source| source["commit"].clone())
        })
        .unwrap_or(Value::Null)
}

#[allow(clippy::too_many_arguments)]
fn run_suite(
    options: &Options,
    schemas: &Schemas,
    descriptor: &Descriptor,
    mutant: Option<&str>,
    client_mutant: Option<(&str, &str)>,
    fixtures: &[&Fixture],
    out: &Path,
    quiet: bool,
) -> Result<Vec<CaseSummary>, String> {
    std::fs::create_dir_all(out.join("transcripts")).map_err(|e| e.to_string())?;
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    let mut entries = Vec::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        let ctx = Context {
            descriptor,
            schemas,
            repo: &options.repo,
            mutant,
            client_mutant,
            work_dir: work.path().join(format!("case-{index}")),
        };
        let result = run_fixture(&fixture.value, &ctx);
        let transcript_path = out
            .join("transcripts")
            .join(format!("{}.jsonl", fixture.id()));
        let lines: Vec<String> = result.transcript.iter().map(Value::to_string).collect();
        std::fs::write(&transcript_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
        let stderr_log = ctx.work_dir.join("participant-stderr.log");
        if let Ok(stderr) = std::fs::read(&stderr_log)
            && !stderr.is_empty()
        {
            std::fs::write(
                out.join("transcripts")
                    .join(format!("{}.stderr.log", fixture.id())),
                stderr,
            )
            .map_err(|e| e.to_string())?;
        }
        if let Ok(clients) = std::fs::read_dir(ctx.work_dir.join("clients")) {
            for client in clients.flatten() {
                if let Ok(stderr) = std::fs::read(client.path().join("stderr.log"))
                    && !stderr.is_empty()
                {
                    std::fs::write(
                        out.join("transcripts").join(format!(
                            "{}.client-{}.stderr.log",
                            fixture.id(),
                            client.file_name().to_string_lossy()
                        )),
                        stderr,
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
        if !quiet {
            let detail = result
                .reason
                .as_deref()
                .map(|r| format!(" — step {}: {r}", result.step.unwrap_or_default()))
                .unwrap_or_default();
            println!("{:<15} {}{}", result.outcome.as_str(), fixture.id(), detail);
        }
        entries.push(json!({
            "fixture": fixture.id(),
            "version": fixture.value["version"],
            "fixture_digest": fixture.digest,
            "polarity": fixture.value["polarity"],
            "requirements": fixture.value["requirements"],
            "outcome": result.outcome.as_str(),
            "failed_step": result.step,
            "reason": result.reason,
            "transcript": format!("transcripts/{}.jsonl", fixture.id()),
        }));
        results.push(CaseSummary {
            fixture: fixture.id().to_string(),
            outcome: result.outcome,
            step: result.step,
            reason: result.reason.clone(),
        });
    }
    let mut summary: BTreeMap<&str, usize> = BTreeMap::new();
    for case in &results {
        *summary.entry(case.outcome.as_str()).or_default() += 1;
    }
    let coverage_limits: Vec<Value> = results
        .iter()
        .filter(|case| {
            case.outcome == Outcome::Unsupported
                && case
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.starts_with("coverage limit"))
        })
        .map(|case| json!({"fixture": case.fixture, "reason": case.reason}))
        .collect();
    let suite_digest = strict::sha256(&strict::canonical(&json!(
        fixtures
            .iter()
            .map(|f| json!([f.id(), f.value["version"], f.digest]))
            .collect::<Vec<_>>()
    )));
    let manifest = json!({
        "format": "combraton-conformance-result/1",
        "status_note": "Protocol 0.1 conformance suite. A result is a conformance claim only for the fixtures it lists as passing, run from the v0.1.0 release inventory; skipped and unsupported outcomes are coverage limits, not passes.",
        "suite": {"fixtures": fixtures.len(), "fixtures_digest": suite_digest, "protocol_commit": git_head(&options.repo)},
        "runner": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
        "participant": {"name": descriptor.name, "version": descriptor.version, "descriptor_digest": descriptor.digest, "claimed_profiles": descriptor.raw["claims"]["profiles"], "mutant": mutant, "client_mutant": client_mutant.map(|(i, m)| json!({"implementation": i, "mutant": m}))},
        "environment": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "summary": summary,
        "coverage_limits": coverage_limits,
        "results": entries,
    });
    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(results)
}

struct CaseSummary {
    fixture: String,
    outcome: Outcome,
    step: Option<usize>,
    reason: Option<String>,
}

fn passing(outcome: Outcome) -> bool {
    matches!(
        outcome,
        Outcome::Pass | Outcome::Unsupported | Outcome::Skipped
    )
}

fn main() -> ExitCode {
    match real_main() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("combraton-conformance: {error}");
            ExitCode::from(2)
        }
    }
}

fn real_main() -> Result<bool, String> {
    let options = parse_options()?;
    if matches!(options.command.as_str(), "version" | "--version") {
        println!(
            "{} {} (Protocol 0.1 conformance runner)",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        return Ok(true);
    }
    if options.command == "self-test" {
        let path = options.repo.join("conformance/vectors/encoding.json");
        let vectors =
            strict::parse(&std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?)?;
        let checked = strict::self_test(&vectors)?;
        println!("self-test: {checked} encoding vectors ok");
        return Ok(true);
    }
    let schemas = Schemas::load(&options.repo)?;
    if options.command == "check-fixtures" {
        return check_fixtures(&options, &schemas);
    }
    let participant = options
        .participant
        .as_ref()
        .ok_or("--participant is required")?;
    let descriptor = Descriptor::load(participant)?;
    let fixture_root = options
        .fixtures
        .clone()
        .unwrap_or_else(|| options.repo.join("conformance/fixtures"));
    let fixtures = load_fixtures(&fixture_root, options.filter.as_deref())?;
    let out = options.out.clone().unwrap_or_else(|| {
        options
            .repo
            .join("conformance/results")
            .join(&descriptor.name)
    });
    match options.command.as_str() {
        "run" => {
            let selected: Vec<&Fixture> = fixtures.iter().collect();
            let results = run_suite(
                &options,
                &schemas,
                &descriptor,
                options.mutant.as_deref(),
                options
                    .client_mutant
                    .as_ref()
                    .map(|(i, m)| (i.as_str(), m.as_str())),
                &selected,
                &out,
                false,
            )?;
            let failed = results.iter().filter(|case| !passing(case.outcome)).count();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for case in &results {
                *counts.entry(case.outcome.as_str()).or_default() += 1;
            }
            let breakdown: Vec<String> = counts
                .iter()
                .map(|(status, count)| format!("{count} {status}"))
                .collect();
            println!(
                "run: {} fixtures ({}), {failed} not passing; manifest {}",
                results.len(),
                breakdown.join(", "),
                out.join("manifest.json").display()
            );
            Ok(failed == 0)
        }
        "check-mutants" => {
            let mut ok = true;
            // Declared kills are checked against all participant descriptors by check-fixtures;
            // here only fixtures applicable to this participant are run.
            let fixtures: Vec<&Fixture> = fixtures
                .iter()
                .filter(|f| exec::applicable(&f.value, &descriptor).is_ok())
                .collect();
            let mut record = Vec::new();
            for mutant in &descriptor.mutants {
                let targets: Vec<&Fixture> = fixtures
                    .iter()
                    .copied()
                    .filter(|f| {
                        f.value["kills"]
                            .as_array()
                            .is_some_and(|k| k.iter().any(|m| m == mutant))
                    })
                    .collect();
                if targets.is_empty() {
                    println!("MUTANT NOT COVERED  {mutant}: no fixture declares it");
                    record.push(json!({"mutant": mutant, "fixture": null, "outcome": null, "killed": false}));
                    ok = false;
                    continue;
                }
                let results = run_suite(
                    &options,
                    &schemas,
                    &descriptor,
                    Some(mutant),
                    None,
                    &targets,
                    &out.join("mutants").join(mutant),
                    true,
                )?;
                for case in &results {
                    let killed = matches!(case.outcome, Outcome::Fail | Outcome::Timeout);
                    let expectation = targets
                        .iter()
                        .find(|f| f.id() == case.fixture)
                        .map(|f| f.value["kill_expectations"][mutant.as_str()].clone())
                        .filter(|e| !e.is_null());
                    let as_intended = match &expectation {
                        None => true,
                        Some(e) => {
                            case.step.map(|s| s as u64) == e["step"].as_u64()
                                && case.reason.as_deref().is_some_and(|r| {
                                    r.contains(e["reason_contains"].as_str().unwrap_or_default())
                                })
                        }
                    };
                    let label = match (killed, as_intended) {
                        (true, true) => "killed",
                        (true, false) => "WRONG-REASON",
                        (false, _) => "SURVIVED",
                    };
                    println!(
                        "{label:<10} {mutant:<28} {} ({})",
                        case.fixture,
                        case.outcome.as_str()
                    );
                    record.push(json!({
                        "mutant": mutant,
                        "fixture": case.fixture,
                        "outcome": case.outcome.as_str(),
                        "failed_step": case.step,
                        "reason": case.reason,
                        "expected": expectation,
                        "killed": killed,
                        "as_intended": killed && as_intended,
                    }));
                    ok &= killed && as_intended;
                }
            }
            // Client mutants (M6-Q2): a deliberately broken client implementation must fail the
            // composition fixtures that declare it.
            for fixture in &fixtures {
                for kill in fixture.value["client_kills"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let implementation = kill["implementation"].as_str().unwrap_or_default();
                    let mutant = kill["mutant"].as_str().unwrap_or_default();
                    let label_dir = format!("client-{implementation}-{mutant}");
                    let results = run_suite(
                        &options,
                        &schemas,
                        &descriptor,
                        None,
                        Some((implementation, mutant)),
                        &[*fixture],
                        &out.join("mutants").join(label_dir),
                        true,
                    )?;
                    for case in &results {
                        let killed = matches!(case.outcome, Outcome::Fail | Outcome::Timeout);
                        let as_intended = kill.get("step").is_none_or(|step| {
                            case.step.map(|s| s as u64) == step.as_u64()
                                && case.reason.as_deref().is_some_and(|r| {
                                    r.contains(kill["reason_contains"].as_str().unwrap_or_default())
                                })
                        });
                        let label = match (killed, as_intended) {
                            (true, true) => "killed",
                            (true, false) => "WRONG-REASON",
                            (false, _) => "SURVIVED",
                        };
                        println!(
                            "{label:<10} {implementation}={mutant:<28} {} ({})",
                            case.fixture,
                            case.outcome.as_str()
                        );
                        record.push(json!({
                            "client": implementation,
                            "mutant": mutant,
                            "fixture": case.fixture,
                            "outcome": case.outcome.as_str(),
                            "failed_step": case.step,
                            "reason": case.reason,
                            "killed": killed,
                            "as_intended": killed && as_intended,
                        }));
                        ok &= killed && as_intended;
                    }
                }
            }
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            std::fs::write(
                out.join("mutants.json"),
                serde_json::to_vec_pretty(&json!({
                    "format": "combraton-conformance-mutants/1",
                    "participant": descriptor.name,
                    "protocol_commit": git_head(&options.repo),
                    "all_killed": ok,
                    "results": record,
                }))
                .unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!(
                "check-mutants: {}",
                if ok {
                    "every declared mutant was killed by every fixture that declares it"
                } else {
                    "FAILURES"
                }
            );
            Ok(ok)
        }
        other => Err(format!("unknown command {other}\n{USAGE}")),
    }
}
