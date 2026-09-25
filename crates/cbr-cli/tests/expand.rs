//! **`cbr expand`: a packet's citation, read back as the artifact's own
//! bytes.**
//!
//! RELEASE-SCOPE §2 lists "expand a citation" among the thin client's
//! operations, and the provider has served `context.expand` since m3a; the
//! verb that reaches it did not exist. A packet section that cites an
//! artifact says *where the rest is*, and this is how a consumer follows
//! that: the request, the citation id the packet printed, and optionally
//! a range. Every test here runs the real `cbr` binary against the real
//! provider over its socket.
//!
//! What is asserted is **the bytes**, never a summary of them: the whole
//! artifact against the file that was ingested and against `cbr fetch`, a
//! range against the source sliced at that range, and a read larger than
//! one frame against the source as well, because the provider halves a
//! read that would not fit the caller's frame and the client has to put
//! the pieces back together.
//!
//! **Refusals are one refusal.** An unknown citation, a citation whose
//! artifact the grant cannot read and a citation in a request the grant
//! cannot read are all `permission_denied out_of_scope` at the provider,
//! and the client must not tell them apart either: no output file, and
//! the same words on standard error.
//!
//! **No model is called.** Every request here asks with an investigation
//! of zero, or is scripted.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, Running};

/// The item every request here names, so its section's citation is
/// `c-log`.
const ITEM: &str = "log";

/// **The launch names itself `cbr`**, which is what a production launch
/// is called, so the scripted sections below can cite `cbr` and the
/// grants can be addressed to it. A compiled section's citation names
/// whatever the launch is called — the last test here launches under
/// another id to hold that.
const PROVIDER: &str = r#""provider_id":"cbr""#;

/// A small cargo test log, a few kilobytes: text, so it is projected, and
/// long enough that a range in its middle is a range of something.
fn log() -> Vec<u8> {
    let mut log = String::from("running 60 tests\n");
    for test in 0..60 {
        let status = if test == 17 { "FAILED" } else { "ok" };
        log.push_str(&format!(
            "test module::tests::case_{test}_holds_its_invariant ... {status}\n"
        ));
    }
    log.push_str(
        "\ntest result: FAILED. 59 passed; 1 failed; 0 ignored; 0 measured; \
         0 filtered out; finished in 0.42s\n",
    );
    log.into_bytes()
}

/// Bytes that are not text and do not repeat within a frame, so a piece
/// put back in the wrong place cannot match by accident.
fn noise(size: usize) -> Vec<u8> {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn start(fixture: &Fixture) -> Running {
    fixture.start_configured(&format!(r#"{PROVIDER},"context":{{"compile":true}}"#))
}

/// Ingest `bytes` through `cbr ingest` and hand back the artifact and its
/// sealed digest.
fn ingest(fixture: &Fixture, name: &str, bytes: &[u8]) -> (String, String) {
    let file = fixture.directory.path().join(name);
    std::fs::write(&file, bytes).expect("writes the input");
    let ingested = fixture.cbr(&["ingest", file.to_str().expect("utf-8")]);
    assert!(
        ingested.status.success(),
        "ingest: {}",
        String::from_utf8_lossy(&ingested.stderr)
    );
    let stdout = String::from_utf8_lossy(&ingested.stdout).to_string();
    let field = |name: &str| {
        stdout
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} ")))
            .unwrap_or_else(|| panic!("no `{name}` in {stdout}"))
            .to_string()
    };
    (field("artifact"), field("digest"))
}

/// Submit a request naming the artifact as the owner, and poll until it
/// settles. The basis names a repository the provider never registered,
/// as `journey_two` does, so nothing but the item is prepared.
fn asked(fixture: &Fixture, request: &str, artifact: &str, digest: &str) -> Value {
    let want = format!("{ITEM}=evidence:{artifact}@{digest}");
    let submitted = fixture.cbr(&[
        "context",
        request,
        "--repo",
        fixture.outside.to_str().expect("utf-8"),
        "--repo-id",
        "j2",
        "--obligation",
        "required_before_start",
        "--investigation",
        "0",
        "--want",
        &want,
    ]);
    assert!(
        submitted.status.success(),
        "submit {request}: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = fixture.cbr(&["request", request]);
        assert!(
            polled.status.success(),
            "inspect: {}",
            String::from_utf8_lossy(&polled.stderr)
        );
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            return inspected;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{request} never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The sealed bytes of an artifact through `cbr fetch`: the other public
/// way to read them, for comparison.
fn fetched(fixture: &Fixture, artifact: &str, digest: &str) -> Vec<u8> {
    let out = fixture.directory.path().join(format!("{artifact}.fetched"));
    let fetched = fixture.cbr(&[
        "fetch",
        artifact,
        "--digest",
        digest,
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    assert!(
        fetched.status.success(),
        "fetch: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    std::fs::read(&out).expect("the fetched bytes")
}

fn out_file(fixture: &Fixture, name: &str) -> PathBuf {
    fixture.directory.path().join(name)
}

/// Nothing was written for `out`: neither the file nor the staged file a
/// write goes through first.
fn assert_nothing_written(out: &Path) {
    assert!(!out.exists(), "{} was written", out.display());
    assert!(
        !out.with_extension("cbr-partial").exists(),
        "a staged file was left for {}",
        out.display()
    );
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn succeeded(output: &Output) -> &Output {
    assert!(
        output.status.success(),
        "cbr expand failed: {}",
        stderr(output)
    );
    output
}

/// The line `--out` prints.
fn wrote(
    length: usize,
    offset: usize,
    size: usize,
    citation: &str,
    evidence: &(String, String),
) -> String {
    format!(
        "wrote {length} bytes at offset {offset} of {size}, citation {citation}, evidence {} at {}\n",
        evidence.0, evidence.1
    )
}

// ---- the whole artifact ----------------------------------------------------

#[test]
fn the_sections_citation_expands_to_exactly_the_ingested_bytes() {
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = start(&fixture);
    let source = log();
    let evidence = ingest(&fixture, "run.log", &source);
    let inspected = asked(&fixture, "whole", &evidence.0, &evidence.1);
    assert_eq!(
        serving::result(&inspected, ITEM).0,
        "satisfied",
        "{inspected:?}"
    );

    let out = out_file(&fixture, "whole.out");
    let expanded = fixture.cbr(&[
        "expand",
        "whole",
        "c-log",
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    succeeded(&expanded);
    let bytes = std::fs::read(&out).expect("the expanded bytes");
    assert_eq!(
        bytes, source,
        "the expanded bytes are not the ingested file"
    );
    assert_eq!(
        bytes,
        fetched(&fixture, &evidence.0, &evidence.1),
        "expand and fetch disagree about the artifact"
    );
    assert_eq!(
        String::from_utf8_lossy(&expanded.stdout),
        wrote(source.len(), 0, source.len(), "c-log", &evidence),
    );
    assert!(
        !out.with_extension("cbr-partial").exists(),
        "the staged file was left behind"
    );

    // **Without `--out` the bytes are the whole of standard output**, so a
    // caller can pipe them: nothing is printed before or after them.
    let printed = fixture.cbr(&["expand", "whole", "c-log"]);
    assert_eq!(succeeded(&printed).stdout, source);
    provider.stop();
}

// ---- a range ---------------------------------------------------------------

#[test]
fn a_range_is_exactly_the_artifacts_bytes_at_that_range() {
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = start(&fixture);
    let source = log();
    let size = source.len();
    let evidence = ingest(&fixture, "run.log", &source);
    asked(&fixture, "range", &evidence.0, &evidence.1);

    // `(offset, length, what it should come back as)`. The last two run
    // to the end: one by stating a length that ends there, one by
    // stating none, and one asks past the end and gets what there is.
    let cases: [(usize, Option<usize>, std::ops::Range<usize>); 5] = [
        (0, Some(1), 0..1),
        (100, Some(250), 100..350),
        (size - 37, Some(37), size - 37..size),
        (size - 10, None, size - 10..size),
        (size - 5, Some(500), size - 5..size),
    ];
    for (at, (offset, length, range)) in cases.into_iter().enumerate() {
        let offset_text = offset.to_string();
        let length_text = length.map(|length| length.to_string());
        let mut arguments = vec!["expand", "range", "c-log", "--offset", &offset_text];
        if let Some(length) = &length_text {
            arguments.extend(["--length", length.as_str()]);
        }
        let printed = fixture.cbr(&arguments);
        assert_eq!(
            succeeded(&printed).stdout,
            &source[range.clone()],
            "case {at}: standard output is not the source at {range:?}"
        );

        let out = out_file(&fixture, &format!("range-{at}.out"));
        arguments.extend(["--out", out.to_str().expect("utf-8")]);
        let written = fixture.cbr(&arguments);
        succeeded(&written);
        assert_eq!(
            std::fs::read(&out).expect("the expanded range"),
            &source[range.clone()],
            "case {at}: the file is not the source at {range:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&written.stdout),
            wrote(range.len(), offset, size, "c-log", &evidence),
            "case {at}"
        );
    }
    provider.stop();
}

#[test]
fn an_offset_past_the_end_is_an_error_and_writes_nothing() {
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = start(&fixture);
    let source = log();
    let size = source.len();
    let evidence = ingest(&fixture, "run.log", &source);
    asked(&fixture, "past", &evidence.0, &evidence.1);

    // The provider clamps an offset to the artifact's size and answers
    // with nothing, which a client that trusted it would print as an
    // empty success.
    let out = out_file(&fixture, "past.out");
    let past = (size + 1).to_string();
    let refused = fixture.cbr(&[
        "expand",
        "past",
        "c-log",
        "--offset",
        &past,
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    assert!(
        !refused.status.success(),
        "an offset past the end succeeded"
    );
    assert!(
        stderr(&refused).contains("past the end"),
        "{}",
        stderr(&refused)
    );
    assert_nothing_written(&out);
    let printed = fixture.cbr(&["expand", "past", "c-log", "--offset", &past]);
    assert!(!printed.status.success());
    assert!(printed.stdout.is_empty(), "bytes were printed");

    // **An empty range is not a question**, so it is refused before
    // anything is asked, in the client's own words. The provider would
    // refuse a `max_bytes` of zero as well, so a nonzero exit alone
    // cannot tell the two apart; the words can, and so can asking with
    // no provider there at all — a client that got as far as connecting
    // would say so instead.
    let empty = fixture.cbr(&["expand", "past", "c-log", "--length", "0"]);
    assert!(!empty.status.success(), "a zero length succeeded");
    assert_eq!(
        stderr(&empty),
        "cbr: --length is a positive number of bytes\n"
    );
    let unconnected = Command::new(env!("CARGO_BIN_EXE_cbr"))
        .args(["expand", "past", "c-log", "--length", "0", "--socket"])
        .arg(fixture.directory.path().join("nothing-listens.sock"))
        .arg("--credential-file")
        .arg(fixture.directory.path().join("credential"))
        .output()
        .expect("cbr runs");
    assert_eq!(
        stderr(&unconnected),
        stderr(&empty),
        "a zero length reached the socket"
    );
    provider.stop();
}

// ---- refusals --------------------------------------------------------------

/// A grant for the reader over `requests` (every request when empty) with
/// `evidence.read` over exactly `artifacts`.
fn grant(fixture: &Fixture, id: &str, requests: &[&str], artifacts: &[&str]) {
    let mut resources = Vec::new();
    if requests.is_empty() {
        resources.push(r#"{"kind":"context.request"}"#.to_string());
        resources.push(r#"{"kind":"context.packet"}"#.to_string());
    }
    for request in requests {
        resources.push(format!(r#"{{"kind":"context.request","id":"{request}"}}"#));
        resources.push(format!(r#"{{"kind":"context.packet","id":"{request}"}}"#));
    }
    resources.push(r#"{"kind":"context.job"}"#.to_string());
    resources.push(r#"{"kind":"cbr.repository","id":"app"}"#.to_string());
    resources.push(r#"{"kind":"cbr.repository","id":"outside"}"#.to_string());
    for artifact in artifacts {
        resources.push(format!(
            r#"{{"kind":"evidence.artifact","id":"{artifact}"}}"#
        ));
    }
    fixture.issue_grant_to(
        "cbr",
        id,
        &[
            "evidence.read",
            "context.request",
            "context.read",
            "context.packet.read",
        ],
        &resources.join(","),
    );
}

#[test]
fn an_unknown_citation_and_one_the_grant_cannot_read_are_one_refusal() {
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = start(&fixture);
    let source = log();
    let evidence = ingest(&fixture, "run.log", &source);
    asked(&fixture, "mine", &evidence.0, &evidence.1);
    grant(&fixture, "g-all", &[], &[&evidence.0]);
    grant(&fixture, "g-no-evidence", &[], &[]);
    grant(&fixture, "g-elsewhere", &["elsewhere"], &[&evidence.0]);

    // **The control arm first**: the reader, under a grant that covers
    // the request and the artifact, reads the citation. Without it every
    // refusal below could be a grant that reads nothing at all.
    let within = out_file(&fixture, "within.out");
    succeeded(&fixture.cbr_as_reader(
        "g-all",
        &[
            "expand",
            "mine",
            "c-log",
            "--out",
            within.to_str().expect("utf-8"),
        ],
    ));
    assert_eq!(std::fs::read(&within).expect("the expanded bytes"), source);

    let refused = |name: &str, output: Output| -> String {
        let out = out_file(&fixture, name);
        assert!(!output.status.success(), "{name} succeeded");
        assert!(
            output.stdout.is_empty(),
            "{name} printed on standard output"
        );
        assert_nothing_written(&out);
        stderr(&output)
    };
    let with_out = |name: &str, arguments: &[&str]| -> Vec<String> {
        let mut arguments: Vec<String> = arguments.iter().map(|a| a.to_string()).collect();
        arguments.push("--out".into());
        arguments.push(out_file(&fixture, name).to_str().expect("utf-8").into());
        arguments
    };
    let run_reader = |grant: &str, name: &str, arguments: &[&str]| {
        let arguments = with_out(name, arguments);
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        refused(name, fixture.cbr_as_reader(grant, &arguments))
    };
    let run_owner = |name: &str, arguments: &[&str]| {
        let arguments = with_out(name, arguments);
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        refused(name, fixture.cbr(&arguments))
    };

    let unknown = run_reader("g-all", "unknown.out", &["expand", "mine", "c-nope"]);
    let unreadable_artifact = run_reader(
        "g-no-evidence",
        "artifact.out",
        &["expand", "mine", "c-log"],
    );
    // `--revision` is given, so the one question asked is the expand
    // itself; without it, the lookup of the last revision is refused
    // first, which is the next paragraph.
    let unreadable_request = run_reader(
        "g-elsewhere",
        "request.out",
        &["expand", "mine", "c-log", "--revision", "1"],
    );
    let unknown_to_owner = run_owner("owner.out", &["expand", "mine", "c-nope"]);
    assert!(
        unknown.contains("permission_denied") && unknown.contains("out_of_scope"),
        "{unknown}"
    );
    assert_eq!(
        unreadable_artifact, unknown,
        "an unreadable artifact is told apart"
    );
    assert_eq!(
        unreadable_request, unknown,
        "an unreadable request is told apart"
    );
    assert_eq!(
        unknown_to_owner, unknown,
        "the owner's unknown citation is told apart"
    );

    // **At the client's own door**: with no `--revision`, `cbr` asks for
    // the request's last revision first. A request the grant cannot read
    // and a request that was never submitted must fail there alike.
    let not_readable = run_reader("g-elsewhere", "door-1.out", &["expand", "mine", "c-log"]);
    let never = run_reader("g-elsewhere", "door-2.out", &["expand", "never", "c-log"]);
    assert!(not_readable.contains("permission_denied"), "{not_readable}");
    assert_eq!(
        not_readable, never,
        "a request the grant cannot read is distinguishable from one that does not exist"
    );
    provider.stop();
}

// ---- revisions -------------------------------------------------------------

/// A scripted section for item `log`, citing `evidence` as `citation`.
fn section(id: &str, citation: &str, evidence: &(String, String)) -> String {
    format!(
        r#"{{"section":{{"section_id":"{id}","item_id":"{ITEM}","label":"observation","content":"{id}","citations":[{{"citation_id":"{citation}","evidence":{{"provider":"cbr","artifact":{{"kind":"evidence.artifact","id":"{}"}},"digest":"{}"}}}}]}}}}"#,
        evidence.0, evidence.1
    )
}

/// Submit `request`, which a script answers, wanting `evidence`, and poll
/// until `packets` revisions of it are published.
fn scripted(fixture: &Fixture, request: &str, evidence: &(String, String), packets: usize) {
    let want = format!("{ITEM}=evidence:{}@{}", evidence.0, evidence.1);
    let submitted = fixture.cbr(&[
        "context",
        request,
        "--repo",
        fixture.outside.to_str().expect("utf-8"),
        "--repo-id",
        "j2",
        "--want",
        &want,
    ]);
    assert!(
        submitted.status.success(),
        "submit {request}: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = fixture.cbr(&["request", request]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        let published = inspected
            .get("packets")
            .and_then(Value::as_array)
            .map_or(0, <[Value]>::len);
        if published == packets {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{request} never published {packets} revisions: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// **A request with two packet revisions**, which only a script makes.
///
/// A compiled request publishes once and ends; a second publication needs
/// a job that publishes again under `context.updates`, which `cbr`
/// negotiates. The script cites artifacts that exist only once they are
/// ingested, and a script is configuration, so the provider is started
/// once to ingest, stopped, and started again with the script naming what
/// it sealed. Revision 1 cites `c-first`; revision 2 keeps it and adds
/// `c-second`.
fn two_revisions(
    fixture: &Fixture,
    first: &[u8],
    second: &[u8],
) -> (Running, (String, String), (String, String)) {
    let ingesting = start(fixture);
    let first = ingest(fixture, "first.bin", first);
    let second = ingest(fixture, "second.bin", second);
    ingesting.stop();
    let script = format!(
        r#"[{},{{"publish":{{}}}},{},{{"publish":{{}}}}]"#,
        section("s-first", "c-first", &first),
        section("s-second", "c-second", &second),
    );
    let provider = fixture.start_configured(&format!(
        r#"{PROVIDER},"context":{{"compile":true,"scripts":{{"two":{script}}}}}"#
    ));
    scripted(fixture, "two", &first, 2);
    (provider, first, second)
}

#[test]
fn revision_picks_that_revisions_citations_and_defaults_to_the_last() {
    let fixture = Fixture::narrow(&["ids:u1"]);
    let (first_bytes, second_bytes) = (b"the first artifact\n".to_vec(), noise(3000));
    let (provider, _, _) = two_revisions(&fixture, &first_bytes, &second_bytes);

    // No `--revision` is the last one, which cites both.
    let latest = fixture.cbr(&["expand", "two", "c-second"]);
    assert_eq!(succeeded(&latest).stdout, second_bytes);
    let explicit = fixture.cbr(&["expand", "two", "c-second", "--revision", "2"]);
    assert_eq!(succeeded(&explicit).stdout, second_bytes);

    // Revision 1 has `c-first` and never had `c-second`.
    let earlier = fixture.cbr(&["expand", "two", "c-first", "--revision", "1"]);
    assert_eq!(succeeded(&earlier).stdout, first_bytes);
    let out = out_file(&fixture, "not-yet.out");
    let not_yet = fixture.cbr(&[
        "expand",
        "two",
        "c-second",
        "--revision",
        "1",
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    assert!(
        !not_yet.status.success(),
        "revision 1 expanded a citation only revision 2 has"
    );
    assert!(
        stderr(&not_yet).contains("out_of_scope"),
        "{}",
        stderr(&not_yet)
    );
    assert_nothing_written(&out);

    // And a revision never published is refused the same way.
    let never = fixture.cbr(&["expand", "two", "c-first", "--revision", "3"]);
    assert!(!never.status.success(), "revision 3 was expanded");
    assert_eq!(stderr(&never), stderr(&not_yet));
    provider.stop();
}

// ---- reads larger than a frame ---------------------------------------------

#[test]
fn a_read_larger_than_one_frame_is_put_back_together_from_short_reads() {
    // **`cbr` negotiates a receive limit of 1 MiB**, and the provider
    // halves a read until its base64 fits the caller's frame, so an
    // artifact of 1.2 MB cannot come back in one answer: at least two
    // come back shorter than asked, and the client has to keep asking
    // from where the last one ended. A projection reads at most 128 KiB,
    // so a compiled section never cites anything this large; a scripted
    // one does.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let large = noise(1_200_000);
    let (provider, _, second) = two_revisions(&fixture, b"small\n", &large);

    let out = out_file(&fixture, "large.out");
    let written = fixture.cbr(&[
        "expand",
        "two",
        "c-second",
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    succeeded(&written);
    assert_eq!(std::fs::read(&out).expect("the expanded bytes"), large);
    assert_eq!(
        String::from_utf8_lossy(&written.stdout),
        wrote(large.len(), 0, large.len(), "c-second", &second),
    );

    // A range across the same boundary, printed rather than written.
    let printed = fixture.cbr(&[
        "expand", "two", "c-second", "--offset", "1000", "--length", "1100000",
    ]);
    assert_eq!(succeeded(&printed).stdout, &large[1000..1_101_000]);
    provider.stop();
}

// ---- another provider's citation -------------------------------------------

#[test]
fn a_citation_naming_another_provider_is_refused_as_an_unknown_one_is() {
    // **A citation is a reference, and a reference names where the
    // evidence is** (EVIDENCE section 2). One naming another provider is
    // not this provider's to read, even when this provider happens to
    // hold an artifact with that id and digest: CONTEXT section 6 makes it
    // the refusal an unknown citation gets. A compiled packet only ever
    // cites the provider that compiled it, so only a script reaches this;
    // the section cites one sealed artifact three ways, naming this
    // provider, naming none (the provider being asked) and naming
    // `elsewhere`.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let ingesting = start(&fixture);
    let source = log();
    let evidence = ingest(&fixture, "run.log", &source);
    ingesting.stop();
    let reference = format!(
        r#""artifact":{{"kind":"evidence.artifact","id":"{}"}},"digest":"{}""#,
        evidence.0, evidence.1
    );
    let script = format!(
        r#"[{{"section":{{"section_id":"s-log","item_id":"{ITEM}","label":"observation","content":"cited three ways","citations":[{{"citation_id":"c-here","evidence":{{"provider":"cbr",{reference}}}}},{{"citation_id":"c-bare","evidence":{{{reference}}}}},{{"citation_id":"c-other","evidence":{{"provider":"elsewhere",{reference}}}}}]}}}},{{"publish":{{}}}}]"#
    );
    let provider = fixture.start_configured(&format!(
        r#"{PROVIDER},"context":{{"compile":true,"scripts":{{"three":{script}}}}}"#
    ));
    scripted(&fixture, "three", &evidence, 1);

    // **The control arms**: the same artifact, cited as this provider's
    // or as nobody's, is the ingested bytes. Without them the refusal
    // below could be a script whose citations reach nothing.
    let here = fixture.cbr(&["expand", "three", "c-here"]);
    assert_eq!(succeeded(&here).stdout, source);
    let bare = fixture.cbr(&["expand", "three", "c-bare"]);
    assert_eq!(succeeded(&bare).stdout, source);

    let refused = |name: &str, citation: &str| -> String {
        let out = out_file(&fixture, name);
        let output = fixture.cbr(&[
            "expand",
            "three",
            citation,
            "--out",
            out.to_str().expect("utf-8"),
        ]);
        assert!(!output.status.success(), "{citation} was expanded");
        assert!(
            output.stdout.is_empty(),
            "{citation} printed on standard output"
        );
        assert_nothing_written(&out);
        stderr(&output)
    };
    let unknown = refused("unknown.out", "c-nope");
    let other = refused("other.out", "c-other");
    assert!(
        unknown.contains("permission_denied") && unknown.contains("out_of_scope"),
        "{unknown}"
    );
    assert_eq!(
        other, unknown,
        "another provider's citation is told apart from an unknown one"
    );
    provider.stop();
}

// ---- a provider under another name -----------------------------------------

#[test]
fn a_compiled_citation_names_the_provider_that_compiled_it_and_expands_there() {
    // **Whatever the provider is called.** Every test above launches as
    // `cbr`, as production does. `cbr context` used to write an evidence
    // item as a reference to provider `cbr` literally, and compiling
    // copies an item's reference into the section's citation, so under
    // any other `provider_id` — every conformance launch, or a production
    // one configured with its own — the packet cited `cbr`, and the
    // provider's own `context.expand` refused that citation as another
    // provider's. Now the item names no provider, which is the provider
    // being asked (EVIDENCE section 2), and the citation the packet
    // carries names that provider by its own id.
    const ELSEWHERE: &str = "cbr-elsewhere";
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start_configured(&format!(
        r#""provider_id":"{ELSEWHERE}","context":{{"compile":true}}"#
    ));
    let source = log();
    let evidence = ingest(&fixture, "run.log", &source);
    let inspected = asked(&fixture, "elsewhere", &evidence.0, &evidence.1);
    assert_eq!(
        serving::result(&inspected, ITEM).0,
        "satisfied",
        "{inspected:?}"
    );

    let packet = serving::sealed(&serving::packet(&fixture, "elsewhere"));
    let cited: Vec<String> = packet
        .get("sections")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .flat_map(|section| {
            section
                .get("citations")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .to_vec()
        })
        .filter(|citation| citation.get("citation_id").and_then(Value::as_str) == Some("c-log"))
        .map(|citation| {
            String::from_utf8(cbr_encoding::to_canonical(
                citation.get("evidence").unwrap_or(&Value::Null),
            ))
            .expect("utf-8")
        })
        .collect();
    assert_eq!(
        cited,
        vec![format!(
            r#"{{"artifact":{{"id":"{}","kind":"evidence.artifact"}},"digest":"{}","provider":"{ELSEWHERE}"}}"#,
            evidence.0, evidence.1
        )],
        "the citation does not name the provider that compiled it"
    );

    let out = out_file(&fixture, "elsewhere.out");
    let expanded = fixture.cbr(&[
        "expand",
        "elsewhere",
        "c-log",
        "--out",
        out.to_str().expect("utf-8"),
    ]);
    succeeded(&expanded);
    assert_eq!(std::fs::read(&out).expect("the expanded bytes"), source);
    provider.stop();
}
