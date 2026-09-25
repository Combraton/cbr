//! **J1, on a real registered repository, with no model.**
//!
//! The repository is this one: CBR's own checkout, registered at launch and
//! read at the tree `HEAD` names. Its decision records and its source are the
//! real thing, which is the point — a journey run against a fixture written
//! to be found proves that the fixture was found.
//!
//! # The oracle, declared before the run
//!
//! The question asked is **"why does CBR's source identity use `gix` rather
//! than the installed `git` binary, and where is that decided?"**, and the
//! request **names no path that answers it**: a task, a one-word selector,
//! and one required item that is deliberately not the answer. Everything
//! that answers the question must be found by the compiler.
//!
//! A correct packet must contain:
//!
//! 1. **the decision**, as an accepted claim: the ADR's question 11 was
//!    ingested as evidence, proposed as a claim and accepted by the owner
//!    for `binding` use, and the packet must carry it labelled `binding`;
//! 2. **the decision record itself**, cited to a span whose bytes contain
//!    the question 11 row;
//! 3. **the code that decision is about**, cited to a span of
//!    `crates/cbr-identity/src/lib.rs` whose bytes contain `gix`;
//! 4. **its own coverage**: the frontier it searched, named as a tree.
//!
//! And two traps it must not fall into:
//!
//! 5. **nothing read from source may be labelled `binding`.** A file in a
//!    repository is evidence of what the repository says; it is not an
//!    accepted decision, and a packet that promoted it would be asserting
//!    that CBR has decided something it has only read.
//! 6. **a rejected claim must never appear as current.** One is planted in
//!    the store. It may appear — INTERNALS section 5 step 3 wants rejected
//!    alternatives distinguishable rather than absent — but only as
//!    historical, and never as `binding`.
//!
//! Every citation must additionally resolve to an exact span at the named
//! tree, and every section's excerpt must be a prefix of the span it cites:
//! the test fetches each cited artifact and compares.
//!
//! A packet that is correct and cited but omits 1, 2 or 3 is a **failed
//! J1**, not a partial one. That is what scoring against a predeclared list
//! means.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

/// The identifier grammar and the locator reader the serving fixture
/// shares. Only those: J1 has its own fixture, over this checkout.
mod serving;

/// The decision record the question is about.
const DECISION_PATH: &str = "docs/decisions/001-standalone-v0.1-scope-and-stack.md";
/// The code that decision is about.
const CODE_PATH: &str = "crates/cbr-identity/src/lib.rs";

fn provider_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let binary = path.join("cbr-provider");
    assert!(
        binary.exists(),
        "{} is not built; run the tests with `cargo test --workspace`",
        binary.display()
    );
    binary
}

/// This repository's root: the real checkout J1 runs against.
fn checkout() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    assert!(
        path.join(".git").exists(),
        "{} is not a checkout",
        path.display()
    );
    path
}

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A blob's bytes, as the repository holds them.
fn git_blob(repository: &Path, blob: &str) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["cat-file", "blob", blob])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "blob {blob} is in the repository");
    output.stdout
}

struct Fixture {
    directory: tempfile::TempDir,
    socket: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let sockets = directory.path().join("s");
        std::fs::create_dir(&sockets).expect("socket dir");
        std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
        std::fs::write(
            directory.path().join("cbr.json"),
            r#"{"format":"cbr-config/1","principal":"owner"}"#,
        )
        .expect("config");
        Self {
            socket: sockets.join("cbr.sock"),
            directory,
        }
    }

    fn data(&self) -> PathBuf {
        self.directory.path().join("data")
    }

    /// Where the provider's standard error goes. A file rather than a pipe:
    /// a pipe nobody reads until the end holds what went wrong out of sight
    /// while it matters, and a file can be read at the moment a wait fails.
    fn stderr_file(&self) -> PathBuf {
        self.directory.path().join("provider.stderr")
    }

    /// Everything the provider has written to standard error so far.
    fn logged(&self) -> String {
        std::fs::read_to_string(self.stderr_file()).unwrap_or_default()
    }

    fn start(&self, registrations: &[String]) -> Child {
        let mut command = Command::new(provider_binary());
        command
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(self.directory.path().join("cbr.json"))
            .arg("--socket")
            .arg(&self.socket);
        for registration in registrations {
            command.arg("--register-repository").arg(registration);
        }
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                std::fs::File::create(self.stderr_file()).expect("stderr file"),
            ))
            .spawn()
            .expect("provider starts");
        let started = Instant::now();
        while UnixStream::connect(&self.socket).is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    fn cbr(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cbr"))
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(self.data().join("credentials").join("owner"))
            .output()
            .expect("cbr runs")
    }
}

fn ok(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "cbr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("canonical JSON on stdout")
}

fn at(value: &Value, path: &[&str]) -> Value {
    let mut current = value.clone();
    for name in path {
        current = current.get(name).cloned().unwrap_or(Value::Null);
    }
    current
}

fn text(value: &Value, path: &[&str]) -> String {
    at(value, path).as_str().unwrap_or_default().to_string()
}

/// Every byte under a directory, for the cost a run leaves on disk.
fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => directory_bytes(&entry.path()),
            _ => entry.metadata().map(|data| data.len()).unwrap_or(0),
        })
        .sum()
}

/// One `name value` line of a verb that prints lines rather than JSON.
fn field(output: &Output, name: &str) -> String {
    assert!(
        output.status.success(),
        "cbr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")).map(str::to_string))
        .unwrap_or_else(|| panic!("no {name} line"))
}

fn array(value: &Value, path: &[&str]) -> Vec<Value> {
    at(value, path)
        .as_array()
        .map(<[Value]>::to_vec)
        .unwrap_or_default()
}

/// How long a request may take to publish before the test calls it stuck.
///
/// It is a bound on a stall, not a performance assertion. Each test's first
/// wait is one index build of this whole repository, with the binary's other
/// tests building beside it: 27.5-27.9 s for all eight at once on an idle
/// development machine. In CI the binary's median is 60-70 s for two waves
/// of four, so about 30 s a wait. Every CI failure at the old 120 s was on a
/// runner that ran `evaluator_properties` -- a property test with its own
/// store and no provider, index or socket -- at 5 to 12 times its median,
/// and on such runners this binary still passed at up to 428 s. Twelve times
/// 30 s is 360 s; 600 s covers the slowest runner measured with room left.
const PACKET_WAIT: Duration = Duration::from_secs(600);

/// Drive the provider's clock-driven preparation by asking it something, and
/// return the packet once one is published.
fn wait_for_packet(fixture: &Fixture, request: &str) -> (Value, Duration) {
    let started = Instant::now();
    loop {
        let inspected = ok(&fixture.cbr(&["request", request]));
        if !array(&inspected, &["packets"]).is_empty() {
            let elapsed = started.elapsed();
            return (ok(&fixture.cbr(&["packet", request])), elapsed);
        }
        assert!(
            started.elapsed() < PACKET_WAIT,
            "no packet after {} s: {inspected:?}\nprovider stderr:\n{}",
            PACKET_WAIT.as_secs(),
            fixture.logged()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn j1_a_question_finds_its_own_answer_with_a_cited_packet() {
    let repository = checkout();
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    // The decision, ingested and accepted, exactly as a user would: the ADR
    // becomes evidence, a claim cites it, and the owner accepts that claim
    // for binding use.
    let adr = repository.join(DECISION_PATH);
    let ingested = fixture.cbr(&[
        "ingest",
        adr.to_str().expect("utf-8"),
        "--media-type",
        "text/markdown",
        "--source-kind",
        "human_decision_record",
        "--repo",
        repository.to_str().expect("utf-8"),
    ]);
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    ok(&fixture.cbr(&["authority", "bind", "stack", "--authority", "owner"]));
    let accepted = claim_file(
        &fixture,
        "accepted.json",
        &artifact,
        &digest,
        &tree,
        "uses_gix_for_source_identity",
        true,
    );
    ok(&fixture.cbr(&["propose", "gix-decision", "--content", &accepted]));
    ok(&fixture.cbr(&[
        "decide",
        "d-gix",
        "--claim",
        "gix-decision",
        "--revision",
        "1",
        "--decision",
        "accepted_for_use",
        "--use",
        "binding",
        "--rationale",
        "ADR 001 question 11, recorded and fired",
    ]));
    // The trap: a rejected alternative, in the store, at the same basis.
    let rejected = claim_file(
        &fixture,
        "rejected.json",
        &artifact,
        &digest,
        &tree,
        "uses_the_git_binary_for_source_identity",
        false,
    );
    ok(&fixture.cbr(&["propose", "git-binary", "--content", &rejected]));
    ok(&fixture.cbr(&[
        "decide",
        "d-git",
        "--claim",
        "git-binary",
        "--revision",
        "1",
        "--decision",
        "rejected",
        "--rationale",
        "the trigger fired and the move was made",
    ]));

    // The question. No path that answers it is named: one required item
    // that is deliberately not the answer, a task, and one word.
    let submitted = Instant::now();
    let outcome = ok(&fixture.cbr(&[
        "context",
        "j1",
        "--repo",
        repository.to_str().expect("utf-8"),
        "--repo-id",
        "cbr",
        "--want",
        "readme=source:README.md",
        "--obligation",
        "required_before_start",
        "--selector",
        "gix",
        "--task",
        "why does source identity use gix rather than the git binary",
        "--capacity",
        "65536",
    ]));
    assert_eq!(
        text(&outcome, &["outcome", "state"]),
        "preparing",
        "{outcome:?}"
    );

    let (packet, waited) = wait_for_packet(&fixture, "j1");
    let to_first_packet = submitted.elapsed();
    let body = packet_body(&fixture, "j1");

    let sections = array(&body, &["sections"]);
    let citations = array(&packet, &["citations"]);
    let coverage = array(&packet, &["coverage"]);

    assert_eq!(
        text(&packet, &["provenance", "compiler"]),
        "cbr-context-compiler/4",
        "a compiled packet says which compiler made it: {packet:?}"
    );

    // 1. The decision, as an accepted claim, labelled by what the authority
    //    permitted it for.
    let claim_section = sections
        .iter()
        .find(|section| text(section, &["content"]).contains("claim gix-decision"))
        .unwrap_or_else(|| panic!("required fact 1: the accepted claim is absent: {sections:?}"));
    assert_eq!(
        text(claim_section, &["label"]),
        "binding",
        "an accepted claim is binding: {claim_section:?}"
    );
    assert_ne!(at(claim_section, &["historical"]), Value::Bool(true));

    // 2 and 3. The record and the code, found by the compiler rather than
    //    named by the request, and cited to spans that hold the answer.
    let spans = cited_spans(&fixture, &repository, &body, &citations);
    let holds = |path: &str, needle: &str| {
        spans.iter().any(|(section_path, bytes, _)| {
            section_path == path && String::from_utf8_lossy(bytes).contains(needle)
        })
    };
    assert!(
        holds(DECISION_PATH, "How source identity reaches git"),
        "required fact 2: no cited span of the decision record holds question 11; spans: {:?}",
        spans.iter().map(|(path, ..)| path).collect::<Vec<_>>()
    );
    assert!(
        holds(CODE_PATH, "gix"),
        "required fact 3: no cited span of the identity crate holds `gix`; spans: {:?}",
        spans.iter().map(|(path, ..)| path).collect::<Vec<_>>()
    );

    // And the excerpt a reader actually sees holds it too. The decision
    // record's chunk is twenty table rows and far larger than the excerpt
    // cap, so a window taken from the span's start stopped several rows
    // before the answer: the citation resolved and the packet still did not
    // show it.
    let shown = |path: &str, needle: &str| {
        sections.iter().any(|section| {
            let content = text(section, &["content"]);
            content.contains(path) && content.contains(needle)
        })
    };
    assert!(
        shown(DECISION_PATH, "How source identity reaches git"),
        "the excerpt of the decision record shows question 11, not just the span around it"
    );
    assert!(
        shown(CODE_PATH, "gix"),
        "and the excerpt of the code shows `gix`"
    );

    // 4. The packet says what it searched.
    assert_eq!(coverage.len(), 1, "{coverage:?}");
    assert_eq!(text(&coverage[0], &["frontier"]), tree, "{coverage:?}");
    assert!(
        text(&coverage[0], &["producer"]).contains("cbr-context-compiler/4"),
        "{coverage:?}"
    );

    // 5. The first trap: source is evidence, never a decision.
    for section in &sections {
        if section.get("claim").is_none() && !text(section, &["content"]).starts_with("claim ") {
            assert_ne!(
                text(section, &["label"]),
                "binding",
                "source read from a repository is not a binding decision: {section:?}"
            );
        }
    }

    // 6. The second trap: a rejected claim is present and never current.
    let rejected_section = sections
        .iter()
        .find(|section| text(section, &["content"]).contains("claim git-binary"))
        .unwrap_or_else(|| panic!("the rejected alternative is not even listed: {sections:?}"));
    assert_eq!(
        at(rejected_section, &["historical"]),
        Value::Bool(true),
        "a rejected claim is never current: {rejected_section:?}"
    );
    assert_eq!(
        text(rejected_section, &["label"]),
        "stale",
        "{rejected_section:?}"
    );
    // The label vocabulary of CONTEXT section 14 has no "rejected", and
    // "stale" alone says "was once valid", which this never was. The content
    // says what happened and names the decision that did it.
    let standing = text(rejected_section, &["content"]);
    assert!(
        standing.contains("rejected by decision d-git"),
        "a rejected claim names the decision that rejected it: {standing}"
    );

    // Every excerpt is exactly the byte range of the cited artifact that its
    // own locator names. Not "contains", not "starts at the line": those
    // bytes.
    for (path, bytes, section) in &spans {
        let content = text(section, &["content"]);
        let (locator, excerpt) = content
            .split_once('\n')
            .unwrap_or_else(|| panic!("a section carries a locator and then bytes: {content}"));
        let (from, to) = locator
            .split_once("excerpt is bytes ")
            .and_then(|(_, rest)| rest.split_once(" of the artifact"))
            .and_then(|(range, _)| range.split_once('-'))
            .and_then(|(from, to)| Some((from.parse::<usize>().ok()?, to.parse::<usize>().ok()?)))
            .unwrap_or_else(|| panic!("no byte range in {locator}"));
        assert!(
            to <= bytes.len() && from <= to,
            "the range {from}-{to} of {path} is not inside the artifact"
        );
        assert_eq!(
            &bytes[from..to],
            excerpt.as_bytes(),
            "the excerpt of {path} is not bytes {from}-{to} of the artifact it cites"
        );
    }

    // ---- every id an identifier, and every citation expanded ---------
    //
    // **A citation is only a citation if a reader can follow it.** The
    // protocol's identifier grammar (`core/1`'s `identifier`) is what the
    // `context` schemas require of a section id and a citation id, and
    // `context.expand` refuses a citation id outside it. The ids of a
    // discovered span used to carry the repository path, slashes and all,
    // so a published packet held schema-invalid ids and only a citation of
    // a file at the repository's root could be expanded. So J1 expands
    // **every** citation it was given through `cbr expand`, and a packet in
    // which one of them cannot be read back is a failed J1.
    serving::assert_packet_ids(&packet, &body);
    let mut nested = false;
    let mut ingested = Vec::new();
    for (expanded, citation) in citations.iter().enumerate() {
        let citation_id = text(citation, &["citation_id"]);
        let artifact = text(citation, &["evidence", "artifact", "id"]);
        let digest = text(citation, &["evidence", "digest"]);
        let out = fixture
            .directory
            .path()
            .join(format!("expanded-{expanded}.bytes"));
        let whole = fixture.cbr(&[
            "expand",
            "j1",
            &citation_id,
            "--out",
            out.to_str().expect("utf-8"),
        ]);
        assert!(
            whole.status.success(),
            "citation {citation_id} does not expand: {}",
            String::from_utf8_lossy(&whole.stderr)
        );
        let bytes = std::fs::read(&out).expect("the expanded bytes");
        if let Some(blob) = artifact.strip_prefix("src.") {
            // A repository blob: the whole of it is the blob at the tree,
            // and the range its section's locator names is the excerpt the
            // section shows.
            assert_eq!(
                bytes,
                git_blob(&repository, blob),
                "citation {citation_id} expands to something other than blob {blob}"
            );
            let section = array(&body, &["sections"])
                .into_iter()
                .find(|section| {
                    array(section, &["citations"])
                        .iter()
                        .any(|carried| text(carried, &["citation_id"]) == citation_id)
                })
                .unwrap_or_else(|| panic!("citation {citation_id} belongs to no section"));
            let content = text(&section, &["content"]);
            let (_, path) = serving::locator_path(&content)
                .unwrap_or_else(|| panic!("no path in the locator of {citation_id}: {content}"));
            nested |= path.contains('/');
            let (from, to) = serving::locator_range(&content)
                .unwrap_or_else(|| panic!("no byte range in the locator of {citation_id}"));
            if to > from {
                let ranged = fixture.cbr(&[
                    "expand",
                    "j1",
                    &citation_id,
                    "--offset",
                    &from.to_string(),
                    "--length",
                    &(to - from).to_string(),
                ]);
                assert!(
                    ranged.status.success(),
                    "citation {citation_id} at {from}-{to} does not expand: {}",
                    String::from_utf8_lossy(&ranged.stderr)
                );
                assert_eq!(
                    ranged.stdout,
                    &bytes[from..to],
                    "citation {citation_id} at {from}-{to} is not those bytes of {path}"
                );
                let excerpt = content.split_once('\n').map_or("", |(_, excerpt)| excerpt);
                assert_eq!(
                    String::from_utf8_lossy(&ranged.stdout),
                    excerpt,
                    "citation {citation_id} at {from}-{to} is not the excerpt its section shows"
                );
            }
        } else if artifact.starts_with("ingest.") {
            // An ingested artifact, which a claim section cites: the bytes
            // are what `cbr fetch` returns for the same reference.
            let fetched_to = fixture
                .directory
                .path()
                .join(format!("fetched-{expanded}.bytes"));
            let fetched = fixture.cbr(&[
                "fetch",
                &artifact,
                "--digest",
                &digest,
                "--out",
                fetched_to.to_str().expect("utf-8"),
            ]);
            assert!(fetched.status.success(), "{artifact} fetches");
            assert_eq!(
                bytes,
                std::fs::read(&fetched_to).expect("the fetched bytes"),
                "citation {citation_id} and `cbr fetch` disagree about {artifact}"
            );
            ingested.push(citation_id.clone());
        } else {
            // No citation J1 is given is unexpandable by design, so there
            // is no list of exceptions: a new kind has to be named here
            // before it can pass.
            panic!("citation {citation_id} cites {artifact}, a kind J1 does not know");
        }
    }
    // **Both claims were followed to their records**, not only the source:
    // the accepted decision and the rejected one each cite the artifact the
    // claim was proposed from, which is the other kind of citation a J1
    // reader follows. A claim section that cited nothing, or cited
    // something `cbr fetch` does not return, fails here.
    for (claim, section) in [
        ("gix-decision", claim_section),
        ("git-binary", rejected_section),
    ] {
        let cited: Vec<String> = array(section, &["citations"])
            .iter()
            .map(|carried| text(carried, &["citation_id"]))
            .collect();
        assert!(
            !cited.is_empty() && cited.iter().all(|id| ingested.contains(id)),
            "claim {claim} cites {cited:?}; the ingested artifacts followed were {ingested:?}"
        );
    }
    assert!(
        nested,
        "no expanded citation is of a file below the repository's root, which is the case \
         the old ids broke"
    );

    // ---- cost, recorded even with no model ----------------------------
    let blobs = git(&repository, &["ls-tree", "-r", "--name-only", &tree])
        .lines()
        .count();
    let store = directory_bytes(&fixture.data());
    eprintln!(
        "J1 cost: model none, no tokens. Time to first packet {to_first_packet:?}, of which \
         index build and compile {waited:?}. Tree {tree}, {blobs} blobs in it. Store after the \
         run {store} bytes. Sections {}, citations {}, coverage gaps {}.",
        sections.len(),
        citations.len(),
        array(&coverage[0], &["gaps"]).len()
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

/// A claim about the identity crate, citing the ingested decision record and
/// conditioned on the tree it was read at.
fn claim_file(
    fixture: &Fixture,
    name: &str,
    artifact: &str,
    digest: &str,
    tree: &str,
    predicate: &str,
    value: bool,
) -> String {
    let path = fixture.directory.path().join(name);
    std::fs::write(
        &path,
        format!(
            r#"{{"plane":"normative",
                 "statement":{{"subject":{{"kind":"cbr.crate","id":"cbr-identity"}},
                               "predicate":"{predicate}","value":{value},
                               "cardinality":"single"}},
                 "scope":{{"id":"stack","qualifiers":{{}}}},
                 "support":[{{"support_id":"s1",
                              "evidence":{{"provider":"cbr",
                                           "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                           "digest":"{digest}"}},
                              "ancestry":{{"completeness":"complete",
                                           "roots":[{{"kind":"evidence","provider":"cbr",
                                                      "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                                      "digest":"{digest}"}}]}}}}],
                 "derivation":{{"kind":"human","inputs":[]}},
                 "conditions":[{{"condition_id":"at-tree","kind":"repository_tree",
                                 "repository":"cbr","expected":"{tree}"}}]}}"#
        ),
    )
    .expect("writes");
    path.to_str().expect("utf-8").to_string()
}

/// The packet's own bytes, as a consumer reads them.
fn packet_body(fixture: &Fixture, request: &str) -> Value {
    let printed = ok(&fixture.cbr(&["packet", request, "--excerpt", "1000000"]));
    let data = text(&printed, &["excerpt", "data_base64"]);
    let bytes = cbr_encoding::decode_base64(&data).expect("base64");
    cbr_encoding::parse(&bytes).expect("canonical JSON")
}

/// Every cited span, as `(path, the artifact's bytes over that span)`, with
/// the artifact fetched and checked against the blob in the tree.
fn cited_spans(
    fixture: &Fixture,
    repository: &Path,
    body: &Value,
    citations: &[Value],
) -> Vec<(String, Vec<u8>, Value)> {
    let mut spans = Vec::new();
    for citation in citations {
        let artifact = text(citation, &["evidence", "artifact", "id"]);
        let digest = text(citation, &["evidence", "digest"]);
        // A claim section cites the evidence its claim rests on, which is
        // an ingested artifact rather than a repository blob. Those are
        // checked by the claim assertions; this walks the source spans.
        let Some(blob) = artifact.strip_prefix("src.") else {
            continue;
        };
        let out = fixture.directory.path().join(format!("{blob}.bytes"));
        let fetched = fixture.cbr(&[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ]);
        assert!(
            fetched.status.success(),
            "a citation must fetch: {}",
            String::from_utf8_lossy(&fetched.stderr)
        );
        let from_packet = std::fs::read(&out).expect("reads");
        let from_tree = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["cat-file", "blob", blob])
            .output()
            .expect("git runs");
        assert!(from_tree.status.success(), "the blob is in the tree");
        assert_eq!(
            from_packet, from_tree.stdout,
            "the cited artifact is the blob at the named tree, exactly"
        );

        // The span itself: the section that carries this citation says which
        // lines, and the locator says which path.
        // The inspect result lists citation ids inside a section; the
        // sealed body carries the citation objects themselves. Accept both,
        // because this walks one and reads the other.
        let citation_id = text(citation, &["citation_id"]);
        let section = array(body, &["sections"]).into_iter().find(|section| {
            array(section, &["citations"]).iter().any(|carried| {
                carried.as_str() == Some(citation_id.as_str())
                    || text(carried, &["citation_id"]) == citation_id
            })
        });
        let section = section.unwrap_or_else(|| {
            panic!("citation {citation_id} belongs to no section of the sealed packet")
        });
        let locator = text(&section, &["content"]);
        let path = locator
            .split_once(':')
            .and_then(|(_, rest)| rest.split_once(' '))
            .map(|(path, _)| path.to_string())
            .unwrap_or_default();
        // Lines are in the locator; the bytes are the whole blob, and the
        // span is what the excerpt was taken from, so the check that matters
        // is that the blob holds the fact.
        spans.push((path, from_packet, section));
    }
    spans
}

#[test]
fn j1_negative_control_a_moved_tree_is_never_silently_answered_from() {
    // **The control.** A repository moves to a new tree and nothing
    // re-ingests it. A packet already published must still name the tree it
    // was prepared at, and a request that names the old tree must still be
    // answered from the old tree's bytes — not from whatever is checked out
    // now. The control fails if a citation resolves to the new tree's blob.
    //
    // Purpose-built rather than run against this checkout: the control has
    // to move a repository, and moving this one would make the test depend
    // on its own branch's history.
    let fixture = Fixture::new();
    let repository = fixture.directory.path().join("moving");
    std::fs::create_dir_all(repository.join("src")).expect("src");
    std::fs::write(
        repository.join("src/queue.rs"),
        "pub fn drainQueue() -> &'static str {\n    \"the old behaviour\"\n}\n",
    )
    .expect("writes");
    for arguments in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    ] {
        git(&repository, &arguments);
    }
    let old_commit = git(&repository, &["rev-parse", "HEAD"]);
    let old_tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let old_blob = git(&repository, &["rev-parse", "HEAD:src/queue.rs"]);

    let mut provider = fixture.start(&[format!("moving={}", repository.display())]);
    let submit = |request: &str, commit: &str| {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "moving",
            "--commit",
            commit,
            "--want",
            "code=source:src/queue.rs",
            "--selector",
            "drain queue",
            "--capacity",
            "65536",
        ]))
    };
    submit("before", &old_commit);
    let (before, _) = wait_for_packet(&fixture, "before");
    let cited_before = citation_blobs(&before);
    assert_eq!(cited_before, vec![old_blob.clone()], "{before:?}");

    // The repository moves. Nothing re-ingests it.
    std::fs::write(
        repository.join("src/queue.rs"),
        "pub fn drainQueue() -> &'static str {\n    \"the new behaviour\"\n}\n",
    )
    .expect("writes");
    git(&repository, &["add", "."]);
    git(
        &repository,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "two",
        ],
    );
    let new_tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let new_blob = git(&repository, &["rev-parse", "HEAD:src/queue.rs"]);
    assert_ne!(old_tree, new_tree);
    assert_ne!(old_blob, new_blob);

    // 1. The published packet still names the tree it was prepared at.
    let (again, _) = wait_for_packet(&fixture, "before");
    let basis = array(&again, &["applicability", "basis", "repositories"]);
    assert_eq!(text(&basis[0], &["tree"]), old_tree, "{again:?}");
    assert_ne!(
        text(&basis[0], &["tree"]),
        new_tree,
        "a packet never claims applicability to a tree nobody prepared it at"
    );
    assert_eq!(citation_blobs(&again), vec![old_blob.clone()], "{again:?}");

    // 2. A new request at the old tree is still answered from the old tree,
    //    even though the index has since been rebuilt for the new one.
    submit("after-at-new", "HEAD");
    let (at_new, _) = wait_for_packet(&fixture, "after-at-new");
    assert_eq!(
        citation_blobs(&at_new),
        vec![new_blob.clone()],
        "a request at the new tree gets the new bytes: {at_new:?}"
    );
    submit("after-at-old", &old_commit);
    let (at_old, _) = wait_for_packet(&fixture, "after-at-old");
    assert_eq!(
        citation_blobs(&at_old),
        vec![old_blob],
        "and one at the old tree still gets the old bytes: {at_old:?}"
    );
    assert_ne!(citation_blobs(&at_old), citation_blobs(&at_new));

    // Discovery reads the basis too, not `HEAD` and not an index built at
    // another tree. Every blob any section cites — asked for or discovered —
    // must be in the tree the packet names.
    for (packet, tree) in [
        (&again, &old_tree),
        (&at_old, &old_tree),
        (&at_new, &new_tree),
    ] {
        let listed = git(
            &repository,
            &["ls-tree", "-r", "--format=%(objectname)", tree],
        );
        for blob in citation_blobs(packet) {
            assert!(
                listed.lines().any(|line| line == blob),
                "a citation of a tree that is not the basis: {blob} not in {tree}"
            );
        }
    }

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

/// The blobs a packet's citations name, in order.
fn citation_blobs(packet: &Value) -> Vec<String> {
    array(packet, &["citations"])
        .iter()
        .map(|citation| {
            text(citation, &["evidence", "artifact", "id"])
                .strip_prefix("src.")
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

#[test]
fn a_sealed_packet_rebuilds_to_the_same_digest() {
    // Reproducibility from retained records (TALK section 3): the same
    // request over the same tree compiles to the same bytes. Two requests
    // differ only in their id, which a packet's bytes do not carry.
    let repository = checkout();
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let mut digests = Vec::new();
    for request in ["r-one", "r-two"] {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "cbr",
            "--want",
            &format!("code=source:{CODE_PATH}"),
            "--selector",
            "dirty snapshot tree entries",
            "--capacity",
            "65536",
        ]));
        let (packet, _) = wait_for_packet(&fixture, request);
        let sections: Vec<(String, String)> = array(&packet, &["sections"])
            .iter()
            .map(|section| (text(section, &["section_id"]), text(section, &["label"])))
            .collect();
        let citations: Vec<String> = array(&packet, &["citations"])
            .iter()
            .map(|citation| text(citation, &["evidence", "digest"]))
            .collect();
        digests.push((sections, citations));
    }
    assert_eq!(
        digests[0], digests[1],
        "the same request over the same tree selects the same content"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
    let stderr = fixture.logged();
    assert!(
        !stderr.contains(repository.to_str().expect("utf-8")),
        "the checkout path is never logged: {stderr}"
    );
}

#[test]
fn an_excerpt_is_what_the_output_capacity_counts() {
    // A section that carried only a locator would make the budget measure
    // pointers. With excerpts, a small capacity buys the required item and
    // nothing else, and everything dropped says why.
    let repository = checkout();
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let submit = |request: &str, capacity: &str| {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "cbr",
            "--want",
            "readme=source:README.md",
            "--obligation",
            "required_before_start",
            "--selector",
            "gix",
            "--task",
            "why does source identity use gix rather than the git binary",
            "--capacity",
            capacity,
        ]))
    };

    // Just over the required item's own excerpt, and under the smallest
    // discovered section that would follow it.
    submit("tight", "2400");
    let (tight, _) = wait_for_packet(&fixture, "tight");
    let kept = array(&tight, &["sections"]);
    let dropped = array(&tight, &["omissions"]);
    assert_eq!(kept.len(), 1, "only the required item fits: {kept:?}");
    assert_eq!(text(&kept[0], &["item_id"]), "readme", "{kept:?}");
    assert!(!dropped.is_empty(), "and the rest is dropped: {tight:?}");
    for omission in &dropped {
        assert_eq!(
            text(omission, &["reason"]),
            "output_capacity",
            "every drop says why: {omission:?}"
        );
    }
    // The required item is satisfied even though everything else went.
    let items = array(&tight, &["items"]);
    assert_eq!(text(&items[0], &["result"]), "satisfied", "{items:?}");

    // The same request with room keeps what the tight one dropped.
    submit("roomy", "65536");
    let (roomy, _) = wait_for_packet(&fixture, "roomy");
    assert!(
        array(&roomy, &["sections"]).len() > kept.len(),
        "capacity is what decided it: {roomy:?}"
    );
    assert!(array(&roomy, &["omissions"]).is_empty(), "{roomy:?}");

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

#[test]
fn discovery_covers_more_than_one_file_however_loud_a_single_file_is() {
    // Ranking by score alone lets one file take the whole budget: eight
    // spans from one file have covered less ground than eight spans from
    // eight, and the file that holds the answer is the one that loses.
    let fixture = Fixture::new();
    let repository = fixture.directory.path().join("loud");
    std::fs::create_dir_all(repository.join("docs")).expect("docs");
    std::fs::write(repository.join("README.md"), "a repository\n").expect("writes");
    // Twelve chunks of the same strong match, all in one file.
    std::fs::write(
        repository.join("docs/loud.md"),
        "the queue drains through the compatibility adapter\n".repeat(20 * 12),
    )
    .expect("writes");
    // One chunk, in another file, that also matches.
    std::fs::write(
        repository.join("docs/answer.md"),
        "the compatibility adapter drains the queue exactly once\n",
    )
    .expect("writes");
    for arguments in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    ] {
        git(&repository, &arguments);
    }

    let mut provider = fixture.start(&[format!("loud={}", repository.display())]);
    ok(&fixture.cbr(&[
        "context",
        "spread",
        "--repo",
        repository.to_str().expect("utf-8"),
        "--repo-id",
        "loud",
        "--want",
        "readme=source:README.md",
        "--selector",
        "repository",
        "--task",
        "where does the compatibility adapter drain the queue",
        "--capacity",
        "65536",
    ]));
    wait_for_packet(&fixture, "spread");

    // The sealed body, because that is where a section's locator is.
    let mut per_file: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for section in array(&packet_body(&fixture, "spread"), &["sections"]) {
        if section.get("item_id").is_some() {
            continue;
        }
        if !text(&section, &["section_id"]).starts_with("d-span-") {
            continue;
        }
        // The path is the locator's, not a piece of the id: an id escapes
        // the path, and past 128 bytes keeps only a digest and a tail.
        let content = text(&section, &["content"]);
        let Some((_, path)) = serving::locator_path(&content) else {
            continue;
        };
        *per_file.entry(path).or_default() += 1;
    }
    assert!(
        per_file.contains_key("docs/answer.md"),
        "the quiet file that holds the answer is in the packet: {per_file:?}"
    );
    for (path, count) in &per_file {
        assert!(
            *count <= 2,
            "no file takes more than its share: {path} has {count}"
        );
    }

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

#[test]
fn a_discovered_claim_is_a_claim_section_and_a_later_rejection_shows_at_the_read() {
    // A binding statement a reader cannot check against a revision and a
    // digest is an assertion, not a citation. A discovered claim carries the
    // same `claim` reference an asked-for one carries, so CONTEXT section
    // 14's read-time facts see it: reject it after publication and the read
    // says so, without the packet's own bytes changing.
    let repository = checkout();
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let adr = repository.join(DECISION_PATH);
    let ingested = fixture.cbr(&[
        "ingest",
        adr.to_str().expect("utf-8"),
        "--media-type",
        "text/markdown",
        "--source-kind",
        "human_decision_record",
        "--repo",
        repository.to_str().expect("utf-8"),
    ]);
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    ok(&fixture.cbr(&["authority", "bind", "stack", "--authority", "owner"]));
    let accepted = claim_file(
        &fixture,
        "later.json",
        &artifact,
        &digest,
        &tree,
        "uses_gix_for_source_identity",
        true,
    );
    ok(&fixture.cbr(&["propose", "gix-decision", "--content", &accepted]));
    ok(&fixture.cbr(&[
        "decide",
        "d-gix",
        "--claim",
        "gix-decision",
        "--revision",
        "1",
        "--decision",
        "accepted_for_use",
        "--use",
        "binding",
        "--rationale",
        "ADR 001 question 11",
    ]));

    ok(&fixture.cbr(&[
        "context",
        "later",
        "--repo",
        repository.to_str().expect("utf-8"),
        "--repo-id",
        "cbr",
        "--want",
        "readme=source:README.md",
        "--selector",
        "gix",
        "--task",
        "why does source identity use gix rather than the git binary",
        "--capacity",
        "65536",
    ]));
    let (before, _) = wait_for_packet(&fixture, "later");
    let carried = array(&before, &["sections"])
        .into_iter()
        .find(|section| text(section, &["section_id"]) == "d-claim-gix-decision")
        .expect("the accepted claim is carried");
    assert_eq!(text(&carried, &["label"]), "binding", "{carried:?}");
    assert_eq!(
        text(&carried, &["claim", "reference", "claim"]),
        "gix-decision",
        "a discovered claim carries its reference, so a reader can check it: {carried:?}"
    );
    assert!(
        !text(&carried, &["claim", "reference", "digest"]).is_empty(),
        "and the digest of the revision it snapshotted: {carried:?}"
    );
    // And the evidence the claim rests on, where the reader may read it: a
    // binding statement a reader cannot check against evidence is an
    // assertion, not a citation.
    let cited = array(&carried, &["citations"]);
    assert_eq!(
        cited.len(),
        1,
        "a claim section cites its support: {carried:?}"
    );
    let supporting = array(&before, &["citations"])
        .into_iter()
        .find(|citation| Some(text(citation, &["citation_id"]).as_str()) == cited[0].as_str())
        .unwrap_or_else(|| panic!("the citation resolves: {carried:?}"));
    assert_eq!(
        text(&supporting, &["evidence", "digest"]),
        digest,
        "and it is the evidence the claim was proposed with: {supporting:?}"
    );

    // The authority changes its mind after the packet was sealed.
    ok(&fixture.cbr(&[
        "decide",
        "d-reversal",
        "--claim",
        "gix-decision",
        "--revision",
        "1",
        "--decision",
        "rejected",
        "--rationale",
        "reconsidered",
    ]));

    let after = ok(&fixture.cbr(&["packet", "later"]));
    let changes = array(&after, &["claim_changes"]);
    assert!(
        changes
            .iter()
            .any(|change| text(change, &["claim", "claim"]) == "gix-decision"),
        "the read reports that the carried claim changed: {after:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

#[test]
fn under_pressure_a_packet_loses_what_it_can_most_afford_to() {
    // INTERNALS section 5 step 5 orders a packet: mandatory first, then task
    // evidence, then optional context. Sorting discovered sections by id
    // dropped by path name instead, so an anchor outlived a binding
    // decision. At a capacity where about half the advisory content fits,
    // the decision and both answers must outlive the rest.
    let repository = checkout();
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let adr = repository.join(DECISION_PATH);
    let ingested = fixture.cbr(&[
        "ingest",
        adr.to_str().expect("utf-8"),
        "--media-type",
        "text/markdown",
        "--source-kind",
        "human_decision_record",
        "--repo",
        repository.to_str().expect("utf-8"),
    ]);
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    ok(&fixture.cbr(&["authority", "bind", "stack", "--authority", "owner"]));
    let accepted = claim_file(
        &fixture,
        "pressure.json",
        &artifact,
        &digest,
        &tree,
        "uses_gix_for_source_identity",
        true,
    );
    ok(&fixture.cbr(&["propose", "gix-decision", "--content", &accepted]));
    ok(&fixture.cbr(&[
        "decide",
        "d-gix",
        "--claim",
        "gix-decision",
        "--revision",
        "1",
        "--decision",
        "accepted_for_use",
        "--use",
        "binding",
        "--rationale",
        "ADR 001 question 11",
    ]));

    let submit = |request: &str, capacity: &str| {
        ok(&fixture.cbr(&[
            "context",
            request,
            "--repo",
            repository.to_str().expect("utf-8"),
            "--repo-id",
            "cbr",
            "--want",
            "readme=source:README.md",
            "--selector",
            "dirty_snapshot",
            "--task",
            "where does dirty_snapshot read the working tree, and was that decided",
            "--capacity",
            capacity,
        ]))
    };

    submit("roomy", "65536");
    let (roomy, _) = wait_for_packet(&fixture, "roomy");
    let full = array(&roomy, &["sections"]).len();

    // `dirty_snapshot` is identifier-shaped, so this request produces anchor
    // sections as well as spans and claims. Without them the rank order
    // could not be told from sorting by section id: `d-anchor-` sorts before
    // `d-claim-`, and with no anchors present that inversion never showed.
    let roomy_ids: Vec<String> = array(&roomy, &["sections"])
        .iter()
        .map(|section| text(section, &["section_id"]))
        .collect();
    assert!(
        roomy_ids.iter().any(|id| id.starts_with("d-anchor-")),
        "the roomy packet has anchors to lose: {roomy_ids:?}"
    );

    // About half the advisory content: the required item's own section plus
    // the first few of what followed it, leaving the anchor and the
    // historical claim outside.
    // 8000, raised from 5000 at m3e. A ranked span now reaches the chunks
    // either side of it, so the same capacity buys fewer and larger
    // sections: at 5000 only one span fitted and the test could no longer
    // tell an ordering from a truncation. The assertions below are
    // unchanged; the budget is scaled to the sections the compiler now
    // produces, which is the trade the widening rule makes and is recorded
    // in STATE rather than hidden here.
    submit("half", "8000");
    let (half, _) = wait_for_packet(&fixture, "half");
    let kept: Vec<String> = array(&half, &["sections"])
        .iter()
        .map(|section| text(section, &["section_id"]))
        .collect();
    assert!(
        kept.len() < full && kept.len() > 1,
        "about half fits: {} of {full}: {kept:?}",
        kept.len()
    );
    assert!(
        kept.contains(&"d-claim-gix-decision".to_string()),
        "the binding decision outlives everything advisory: {kept:?}"
    );
    let spans: Vec<&String> = kept.iter().filter(|id| id.contains("d-span-")).collect();
    // Deliberately *not* "and the identity crate's span survives". That is a
    // statement about ranking, and this test is about order: it failed the
    // moment this repository's own content changed under it, which is the
    // wrong reason for an order test to fail. J1 is where relevance is
    // scored.
    assert!(!spans.is_empty(), "task evidence survives at all: {kept:?}");
    // `context::inclusion` packs rather than truncates: it walks sections in
    // order and keeps each one that still fits, so a small section low in
    // the order can occupy leftover room a larger one could not use. What
    // the order decides is therefore priority, not exclusion, and the
    // decisive statement is which section loses when two compete.
    //
    // Historical material is last under this rule and near the front under
    // the old one, where `d-claim-` sorted before `d-span-`. So: the
    // rejected alternative goes while task evidence stays.
    assert!(
        !kept.contains(&"d-claim-git-binary".to_string()),
        "historical material is the first thing to go: {kept:?}"
    );
    assert!(
        spans.len() >= 2,
        "while more task evidence stays than anything ranked below it: {kept:?}"
    );
    for omission in array(&half, &["omissions"]) {
        assert!(
            !text(&omission, &["reason"]).is_empty(),
            "every drop says why: {omission:?}"
        );
    }

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

#[test]
fn a_readable_claim_that_does_not_bear_on_the_request_is_omitted_with_its_reason() {
    // Every claim in the store is not context for every request. A pilot
    // repository with twenty-two decisions must not put all twenty-two into
    // a packet about one of them — and what it leaves out must be counted,
    // not silently dropped.
    let repository = checkout();
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    let fixture = Fixture::new();
    let mut provider = fixture.start(&[format!("cbr={}", repository.display())]);

    let adr = repository.join(DECISION_PATH);
    let ingested = fixture.cbr(&[
        "ingest",
        adr.to_str().expect("utf-8"),
        "--media-type",
        "text/markdown",
        "--source-kind",
        "human_decision_record",
        "--repo",
        repository.to_str().expect("utf-8"),
    ]);
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    ok(&fixture.cbr(&["authority", "bind", "stack", "--authority", "owner"]));

    // One claim that bears on the request: its condition names `cbr`.
    let about = claim_file(
        &fixture,
        "about.json",
        &artifact,
        &digest,
        &tree,
        "uses_gix_for_source_identity",
        true,
    );
    ok(&fixture.cbr(&["propose", "gix-decision", "--content", &about]));
    ok(&fixture.cbr(&[
        "decide",
        "d-gix",
        "--claim",
        "gix-decision",
        "--revision",
        "1",
        "--decision",
        "accepted_for_use",
        "--use",
        "binding",
        "--rationale",
        "ADR 001 question 11",
    ]));

    // One that does not: another scope, another repository, and no word of
    // it in the question.
    let elsewhere = {
        let path = fixture.directory.path().join("elsewhere.json");
        std::fs::write(
            &path,
            format!(
            r#"{{"plane":"normative",
                 "statement":{{"subject":{{"kind":"kitchen.appliance","id":"toaster"}},
                               "predicate":"browns_bread","value":true,
                               "cardinality":"single"}},
                 "scope":{{"id":"breakfast","qualifiers":{{}}}},
                 "support":[{{"support_id":"s1",
                              "evidence":{{"provider":"cbr",
                                           "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                           "digest":"{digest}"}},
                              "ancestry":{{"completeness":"complete",
                                           "roots":[{{"kind":"evidence","provider":"cbr",
                                                      "artifact":{{"kind":"evidence.artifact","id":"{artifact}"}},
                                                      "digest":"{digest}"}}]}}}}],
                 "derivation":{{"kind":"human","inputs":[]}},
                 "conditions":[{{"condition_id":"at-tree","kind":"repository_tree",
                                 "repository":"kitchen","expected":"{tree}"}}]}}"#
            ),
        )
        .expect("writes");
        path.to_str().expect("utf-8").to_string()
    };
    ok(&fixture.cbr(&["authority", "bind", "breakfast", "--authority", "owner"]));
    ok(&fixture.cbr(&["propose", "toaster", "--content", &elsewhere]));
    ok(&fixture.cbr(&[
        "decide",
        "d-toaster",
        "--claim",
        "toaster",
        "--revision",
        "1",
        "--decision",
        "accepted_for_use",
        "--use",
        "binding",
        "--rationale",
        "it does brown bread",
    ]));

    ok(&fixture.cbr(&[
        "context",
        "relevance",
        "--repo",
        repository.to_str().expect("utf-8"),
        "--repo-id",
        "cbr",
        "--want",
        "readme=source:README.md",
        "--selector",
        "gix",
        "--task",
        "why does source identity use gix rather than the git binary",
        "--capacity",
        "65536",
    ]));
    let (packet, _) = wait_for_packet(&fixture, "relevance");

    let sections: Vec<String> = array(&packet, &["sections"])
        .iter()
        .map(|section| text(section, &["section_id"]))
        .collect();
    assert!(
        sections.contains(&"d-claim-gix-decision".to_string()),
        "the claim that bears on the request is carried: {sections:?}"
    );
    assert!(
        !sections.contains(&"d-claim-toaster".to_string()),
        "the one that does not is not: {sections:?}"
    );
    // And it is counted, with the reason the protocol has for it.
    let omitted = array(&packet, &["omissions"]);
    let toaster = omitted
        .iter()
        .find(|omission| text(omission, &["section_id"]) == "d-claim-toaster")
        .unwrap_or_else(|| panic!("an irrelevant claim is omitted, not dropped: {omitted:?}"));
    assert_eq!(text(toaster, &["reason"]), "applicability", "{toaster:?}");

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}
