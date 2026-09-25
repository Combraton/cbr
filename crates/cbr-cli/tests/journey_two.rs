//! **Journey 2: a large result, projected under bounded model context,
//! with what was left out declared.**
//!
//! [m5 READINESS §3](../../../docs/work/m5/READINESS.md) is the whole of
//! the promise. A large test log, build log or JSON document is sealed
//! whole first, as evidence, by the ordinary public client; a request
//! names it with an `evidence_included` item; and the packet carries a
//! **projection** of it — its run identity, the failures it names, the
//! excerpts that decide the task, and every byte it did not carry,
//! declared with a reason and an extent.
//!
//! Both of J2's negative controls are here, end to end, through `cbr`
//! against a provider whose model is the labelled `model.fake` control:
//!
//! 1. **Silent truncation.** Every test that reads a projection checks
//!    that its carried excerpts and its declared omissions **tile the
//!    artifact** — every byte, in order, exactly once — and that every
//!    declared omission is in the packet's own omission list. Remove the
//!    path that declares an omission and the tiling breaks, so the
//!    journey fails for the reason the control names.
//! 2. **Oversize input.** An input larger than the projection's own
//!    capacity ends as the typed `insufficient_capacity`, with no
//!    section, no omission and no call — never a partial summary.
//!
//! **A projected value is the source's bytes at its cited range**, and
//! that is asserted rather than trusted: every excerpt is compared with
//! the sealed artifact, fetched through `cbr fetch`, at the range the
//! projection names.
//!
//! The reader of the projection here is a parser written for this test
//! from the format the projection declares, and not the provider's own:
//! a test that read a projection with the code that wrote it would be
//! checking that the code agrees with itself.
//!
//! **No model is called.** The transport is the `model.fake` control.

use std::time::{Duration, Instant};

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, derivations, ledger, result};

/// The item every request here names.
const ITEM: &str = "log";

/// The prefix every part's question carries in its derivation record, so
/// a part's record can be told from anything else the store holds.
const PART_SELECTOR: &str = "project_large_result ";

// ---- inputs ---------------------------------------------------------------

/// `cargo test`'s own output, shaped as a real run prints it: a `Running`
/// line per test binary, `running N tests`, a status line per test, a
/// block per failure, the `failures:` summary and `test result:`.
///
/// `failing` names `(binary, test)` pairs that fail.
fn test_log(binaries: usize, tests: usize, failing: &[(usize, usize)]) -> String {
    let mut log = String::new();
    for binary in 0..binaries {
        let name = |test: usize| {
            format!("module_{binary}::tests::case_{test}_keeps_its_invariant_under_load")
        };
        log.push_str(&format!(
            "     Running unittests src/lib.rs (target/debug/deps/crate_{binary}-0123456789abcdef)\n\n"
        ));
        log.push_str(&format!("running {tests} tests\n"));
        let mut failed = Vec::new();
        for test in 0..tests {
            if failing.contains(&(binary, test)) {
                log.push_str(&format!("test {} ... FAILED\n", name(test)));
                failed.push(test);
            } else {
                log.push_str(&format!("test {} ... ok\n", name(test)));
            }
        }
        log.push('\n');
        if !failed.is_empty() {
            log.push_str("failures:\n\n");
            for test in &failed {
                log.push_str(&format!(
                    "---- {} stdout ----\n\nthread '{}' panicked at crates/crate_{binary}/src/lib.rs:{}:9:\n\
                     assertion `left == right` failed: the ledger held 5000 tokens\n  left: 5000\n right: 10000\n\
                     note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\n",
                    name(*test),
                    name(*test),
                    100 + test
                ));
            }
            log.push_str("\nfailures:\n");
            for test in &failed {
                log.push_str(&format!("    {}\n", name(*test)));
            }
            log.push('\n');
            log.push_str(&format!(
                "test result: FAILED. {} passed; {} failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s\n\n",
                tests - failed.len(),
                failed.len()
            ));
        } else {
            log.push_str(&format!(
                "test result: ok. {tests} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s\n\n"
            ));
        }
    }
    log
}

/// A log of a few kilobytes, which fits a projection whole.
fn small_log() -> String {
    test_log(1, 8, &[(0, 3)])
}

/// A log larger than one part and well inside the projection's capacity,
/// with three failing tests in two binaries.
fn large_log() -> String {
    test_log(8, 100, &[(1, 17), (5, 2), (5, 80)])
}

/// The three names `large_log` fails.
const LARGE_FAILURES: [&str; 3] = [
    "module_1::tests::case_17_keeps_its_invariant_under_load",
    "module_5::tests::case_2_keeps_its_invariant_under_load",
    "module_5::tests::case_80_keeps_its_invariant_under_load",
];

/// A log far larger than anything a projection may read.
fn oversize_log() -> String {
    test_log(40, 400, &[(3, 3)])
}

/// A conformance runner's `manifest.json`, pretty-printed as the runner
/// writes it: run identity at the top, a result per fixture, two of them
/// failing.
fn manifest_json(results: usize) -> String {
    let mut entries = Vec::new();
    for at in 0..results {
        let outcome = if at == 11 || at == 70 { "fail" } else { "pass" };
        entries.push(format!(
            "    {{\n      \"failed_step\": null,\n      \"fixture\": \"context.fixture-number-{at}-holds\",\n      \
             \"fixture_digest\": \"sha256:{at:064x}\",\n      \"outcome\": \"{outcome}\",\n      \
             \"polarity\": \"positive\",\n      \"reason\": null,\n      \"requirements\": [\n        \"CTX-{at}\"\n      ],\n      \
             \"transcript\": \"transcripts/context.fixture-number-{at}-holds.jsonl\",\n      \"version\": 1\n    }}"
        ));
    }
    format!(
        "{{\n  \"environment\": {{\n    \"arch\": \"aarch64\",\n    \"os\": \"macos\"\n  }},\n  \
         \"format\": \"combraton-conformance-result/1\",\n  \"results\": [\n{}\n  ],\n  \
         \"summary\": {{\n    \"fail\": 2,\n    \"pass\": {}\n  }}\n}}\n",
        entries.join(",\n"),
        results - 2
    )
}

// ---- driving the public client --------------------------------------------

/// Ingest `bytes` through `cbr ingest`, which seals them whole, and hand
/// back the artifact and its digest.
///
/// **Captured at the tree the requests name**, as the J2 harness ingests
/// CBR's own test output at the commit it was produced at: the capture
/// anchors are the run's identity, whatever of the log is carried.
fn ingest(fixture: &Fixture, name: &str, bytes: &[u8]) -> (String, String) {
    let file = fixture.directory.path().join(name);
    std::fs::write(&file, bytes).expect("writes the input");
    let ingested = fixture.cbr(&[
        "ingest",
        file.to_str().expect("utf-8"),
        "--repo",
        fixture.outside.to_str().expect("utf-8"),
    ]);
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

/// The sealed bytes of an artifact, through `cbr fetch`, which checks
/// them against the digest before it writes anything.
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

/// Submit a request naming the artifact, and poll until it settles.
fn asked(
    fixture: &Fixture,
    request: &str,
    artifact: &str,
    digest: &str,
    investigation: usize,
) -> Value {
    asked_as(fixture, None, request, artifact, digest, investigation)
}

/// The same, as the reader under `grant` when one is given.
fn asked_as(
    fixture: &Fixture,
    grant: Option<&str>,
    request: &str,
    artifact: &str,
    digest: &str,
    investigation: usize,
) -> Value {
    let want = format!("{ITEM}=evidence:{artifact}@{digest}");
    let investigation = investigation.to_string();
    // **The basis names a repository the provider never registered.** A
    // test log is about a tree, and the request says which; but nothing
    // here is about that tree's files, and a repository in the view would
    // give discovery something to search — and a model question of its own
    // to spend the same limit on. So the view is empty, discovery reads
    // nothing, and every call a request makes is a part of its projection.
    // The J2 harness asks the same way.
    let arguments = [
        "context",
        request,
        "--repo",
        fixture.outside.to_str().expect("utf-8"),
        "--repo-id",
        "j2",
        "--obligation",
        "required_before_start",
        "--task",
        "which tests failed, and what did they assert",
        "--capacity",
        "65536",
        "--investigation",
        &investigation,
        "--want",
        &want,
    ];
    let run = |arguments: &[&str]| match grant {
        Some(grant) => fixture.cbr_as_reader(grant, arguments),
        None => fixture.cbr(arguments),
    };
    let submitted = run(&arguments);
    assert!(
        submitted.status.success(),
        "submit {request}: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = run(&["request", request]);
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
            started.elapsed() < Duration::from_secs(120),
            "{request} never left preparing: {inspected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The sealed packet of a request, as the owner reads it.
fn sealed_packet(fixture: &Fixture, request: &str) -> Value {
    serving::sealed(&serving::packet(fixture, request))
}

/// The projection section the packet carries for the item, if any.
fn projection_section(packet: &Value) -> Option<Value> {
    packet
        .get("sections")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|section| section.get("item_id").and_then(Value::as_str) == Some(ITEM))
        .cloned()
}

/// Every omission the packet declares for the item, as `(section, reason)`.
fn item_omissions(packet: &Value) -> Vec<(String, String)> {
    packet
        .get("omissions")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter(|omission| omission.get("item_id").and_then(Value::as_str) == Some(ITEM))
        .map(|omission| {
            (
                omission
                    .get("section_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                omission
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

/// Every sealed record of a projection's part, as `(selector, answer,
/// offered, item)`, fetched through `cbr` as the owner.
fn part_records(fixture: &Fixture) -> Vec<(String, Value, Vec<Value>, String)> {
    let data = fixture.data();
    derivations(&data)
        .into_iter()
        .filter_map(|(id, record)| {
            let digest = record
                .get("descriptor")
                .and_then(|descriptor| descriptor.get("digest"))
                .and_then(Value::as_str)
                .expect("a digest")
                .to_string();
            let sealed = cbr_encoding::parse(&fetched(fixture, &id, &digest)).expect("a record");
            let question = sealed.get("question").cloned().unwrap_or(Value::Null);
            let selector = question
                .get("selector")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !selector.starts_with(PART_SELECTOR) {
                return None;
            }
            Some((
                selector,
                sealed.get("answer").cloned().unwrap_or(Value::Null),
                question
                    .get("offered")
                    .and_then(Value::as_array)
                    .unwrap_or_default()
                    .to_vec(),
                sealed
                    .get("item")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect()
}

// ---- reading a projection, from the format it declares ---------------------

#[derive(Debug, Clone, PartialEq)]
enum Extent {
    Carried {
        number: usize,
        start: usize,
        end: usize,
        kind: String,
        bytes: Vec<u8>,
    },
    Omitted {
        number: usize,
        start: usize,
        end: usize,
        reason: String,
    },
}

impl Extent {
    fn range(&self) -> (usize, usize) {
        match self {
            Extent::Carried { start, end, .. } | Extent::Omitted { start, end, .. } => {
                (*start, *end)
            }
        }
    }
}

#[derive(Debug)]
struct Read {
    /// The lines before the named failures: what was read, and how.
    header: Vec<String>,
    /// Each named failure: the name as shown, and its range.
    named: Vec<(String, usize, usize)>,
    /// How many failures the projection says it named, whether or not
    /// each is listed.
    named_count: usize,
    unresolved: Vec<String>,
    extents: Vec<Extent>,
}

impl Read {
    /// The header line that says what was read, and how.
    fn how(&self) -> &str {
        self.header
            .iter()
            .find(|line| line.starts_with("read as "))
            .unwrap_or_else(|| panic!("no `read as` line in {:?}", self.header))
    }

    /// How many parts the header says the input's excerpts fill.
    fn parts(&self) -> usize {
        let line = self.how();
        let before = line
            .split(" parts;")
            .next()
            .unwrap_or_else(|| panic!("no part count in {line:?}"));
        before
            .rsplit(' ')
            .next()
            .and_then(|number| number.parse().ok())
            .unwrap_or_else(|| panic!("no part count in {line:?}"))
    }
}

/// `s-e` as two numbers.
fn range_of(text: &str) -> (usize, usize) {
    let (start, end) = text
        .split_once('-')
        .unwrap_or_else(|| panic!("not a range: {text:?}"));
    (
        start.parse().expect("a start"),
        end.parse().expect("an end"),
    )
}

/// The line starting at `at`, without its newline, and `at` moved past it.
fn next_line(bytes: &[u8], at: &mut usize) -> Option<String> {
    if *at >= bytes.len() {
        return None;
    }
    let end = bytes[*at..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(bytes.len(), |offset| *at + offset);
    let text = String::from_utf8(bytes[*at..end].to_vec()).expect("utf-8 line");
    *at = (end + 1).min(bytes.len());
    Some(text)
}

/// Read a projection from its declared format, and nothing else.
///
/// An excerpt is framed by a line that says **how many bytes follow**,
/// so it is taken by count and never by searching for its end marker:
/// the bytes are the artifact's, and an artifact is free to contain
/// anything, including a line that looks like a marker.
fn read_projection(content: &str) -> Read {
    let bytes = content.as_bytes();
    let mut at = 0;
    let mut header = Vec::new();
    let count_line = loop {
        let line = next_line(bytes, &mut at).expect("a header, then a failures line");
        if line.starts_with("failures named: ") {
            break line;
        }
        header.push(line);
    };
    let named_count: usize = count_line
        .strip_prefix("failures named: ")
        .unwrap_or_else(|| panic!("not a failures line: {count_line:?}"))
        .parse()
        .expect("a count");
    let mut named = Vec::new();
    let mut unresolved = Vec::new();
    let mut extents = Vec::new();
    while let Some(text) = next_line(bytes, &mut at) {
        if let Some(rest) = text.strip_prefix("  and ") {
            assert!(rest.contains(" more"), "not a remainder line: {text:?}");
            continue;
        }
        if let Some(rest) = text.strip_prefix("  ") {
            let (name, range) = rest
                .rsplit_once(" at bytes ")
                .unwrap_or_else(|| panic!("not a named failure: {text:?}"));
            let (start, end) = range_of(range);
            named.push((name.to_string(), start, end));
            continue;
        }
        if let Some(rest) = text.strip_prefix("unresolved: ") {
            unresolved.push(rest.to_string());
            continue;
        }
        let (tag, rest) = text
            .split_once("] bytes ")
            .unwrap_or_else(|| panic!("not a ledger line: {text:?}"));
        let (range, rest) = rest
            .split_once(", lines ")
            .unwrap_or_else(|| panic!("no lines in {text:?}"));
        let (start, end) = range_of(range);
        let (_, what) = rest
            .split_once(", ")
            .unwrap_or_else(|| panic!("no kind in {text:?}"));
        if let Some(number) = tag.strip_prefix("[e") {
            let number: usize = number.parse().expect("an excerpt number");
            let length = end - start;
            let excerpt = bytes[at..at + length].to_vec();
            at += length;
            let closing = format!("\n[end e{number}]\n");
            assert_eq!(
                &bytes[at..(at + closing.len()).min(bytes.len())],
                closing.as_bytes(),
                "excerpt e{number} is not followed by its closing line"
            );
            at += closing.len();
            extents.push(Extent::Carried {
                number,
                start,
                end,
                kind: what.to_string(),
                bytes: excerpt,
            });
        } else if let Some(number) = tag.strip_prefix("[o") {
            let reason = what
                .strip_prefix("omitted: ")
                .unwrap_or_else(|| panic!("an omitted extent without a reason: {text:?}"));
            extents.push(Extent::Omitted {
                number: number.parse().expect("an omission number"),
                start,
                end,
                reason: reason.to_string(),
            });
        } else {
            panic!("not a ledger line: {text:?}");
        }
    }
    Read {
        header,
        named,
        named_count,
        unresolved,
        extents,
    }
}

/// **The silent-truncation control's detector**, and the property a
/// projection is worth nothing without.
///
/// Every byte of the artifact is either carried, as exactly the
/// artifact's bytes at the range the projection names, or declared
/// omitted with a reason; the extents come in order, touch end to end
/// and cover the whole artifact. Every omission is in the packet's own
/// omission list, under the reason the protocol's vocabulary gives it,
/// and nothing else is.
fn assert_every_byte_is_carried_or_declared(read: &Read, source: &[u8], packet: &Value) {
    let mut expected = 0;
    for extent in &read.extents {
        let (start, end) = extent.range();
        assert_eq!(
            start, expected,
            "a gap or an overlap before {extent:?}: the bytes {expected}-{start} are neither carried nor declared"
        );
        assert!(end > start, "an empty extent: {extent:?}");
        if let Extent::Carried { bytes, number, .. } = extent {
            assert_eq!(
                bytes.as_slice(),
                &source[start..end],
                "excerpt e{number} is not the artifact's bytes at {start}-{end}"
            );
        }
        expected = end;
    }
    assert_eq!(
        expected,
        source.len(),
        "the projection stops at byte {expected} of {}: the rest is neither carried nor declared",
        source.len()
    );

    let declared: Vec<(String, String)> = read
        .extents
        .iter()
        .filter_map(|extent| match extent {
            Extent::Omitted { number, reason, .. } => Some((
                format!("s-{ITEM}.o{number}"),
                match reason.as_str() {
                    "not_selected" => "applicability",
                    "over_projection" => "output_capacity",
                    "not_text" => "unavailable",
                    other => panic!("an omission reason nothing defines: {other}"),
                }
                .to_string(),
            )),
            Extent::Carried { .. } => None,
        })
        .collect();
    assert_eq!(
        item_omissions(packet),
        declared,
        "the packet's omissions are not the projection's"
    );
}

/// The projection a settled request carries, read, with the source it
/// projects checked against it.
fn projection_of(fixture: &Fixture, request: &str, source: &[u8]) -> Read {
    let packet = sealed_packet(fixture, request);
    let section = projection_section(&packet)
        .unwrap_or_else(|| panic!("no section for {ITEM} in {packet:?}"));
    let content = section
        .get("content")
        .and_then(Value::as_str)
        .expect("content")
        .to_string();
    let read = read_projection(&content);
    assert_every_byte_is_carried_or_declared(&read, source, &packet);
    read
}

// ---- the journey -----------------------------------------------------------

#[test]
fn a_large_test_log_is_projected_and_every_byte_is_carried_or_declared_omitted() {
    // **The positive half of the journey, and the silent-truncation
    // control's detector.** A log larger than one part, projected with a
    // model's help: every part is its own bounded call with its own
    // record, the parts are claimed together, and what comes back tiles
    // the artifact.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    assert_eq!(fetched(&fixture, &artifact, &digest), log.as_bytes());

    // The deterministic reading says how many parts the input fills; the
    // model-assisted request is given exactly that many questions, so
    // nothing else it could ask competes for them.
    let baseline = asked(&fixture, "baseline", &artifact, &digest, 0);
    assert_eq!(result(&baseline, ITEM).0, "satisfied", "{baseline:?}");
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    assert!(
        parts >= 2,
        "the fixture is meant to fill more than one part"
    );
    assert!(
        part_records(&fixture).is_empty(),
        "a request with no investigation asked a model"
    );

    let inspected = asked(&fixture, "assisted", &artifact, &digest, parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let read = projection_of(&fixture, "assisted", log.as_bytes());
    assert!(
        read.header[0].contains(&artifact) && read.header[0].contains(&digest),
        "the projection names the artifact it projects: {:?}",
        read.header
    );
    assert!(
        read.how().starts_with("read as libtest:"),
        "{:?}",
        read.header
    );
    assert!(
        read.how().contains("chosen by the model"),
        "{:?}",
        read.header
    );
    assert_eq!(read.parts(), parts);

    // **Named failures**, each the artifact's own bytes at its range.
    assert_eq!(read.named_count, LARGE_FAILURES.len());
    let names: Vec<&str> = read.named.iter().map(|(name, ..)| name.as_str()).collect();
    assert_eq!(names, LARGE_FAILURES);
    for (name, start, end) in &read.named {
        assert_eq!(&log.as_bytes()[*start..*end], name.as_bytes());
    }
    // **Run identity**: the capture anchors the log was sealed with name
    // the tree it came from, and no `test result:` line is ever left out
    // as though nothing had chosen it — it is carried, or declared chosen
    // and out of room.
    assert!(
        read.header
            .iter()
            .any(|line| line.starts_with("captured at ")
                && line.contains(&format!("git_tree {}", serving::tree_of(&fixture.outside)))),
        "the run's identity is not in the header: {:?}",
        read.header
    );
    let mut at = 0;
    while let Some(found) = log[at..].find("test result: ") {
        let byte = at + found;
        let holder = read
            .extents
            .iter()
            .find(|extent| {
                let (start, end) = extent.range();
                start <= byte && byte < end
            })
            .expect("every byte is in an extent");
        assert!(
            !matches!(holder, Extent::Omitted { reason, .. } if reason == "not_selected"),
            "a run's result at byte {byte} was left out as though nothing chose it: {holder:?}"
        );
        at = byte + 1;
    }
    // No excerpt group here is larger than a part, so partitioning cut
    // nothing a part had to leave unresolved.
    assert!(read.unresolved.is_empty(), "{:?}", read.unresolved);
    // And something was left out, or this is not a large result.
    assert!(
        read.extents
            .iter()
            .any(|extent| matches!(extent, Extent::Omitted { .. })),
        "nothing was omitted from a log larger than a part"
    );

    // **Every part is a bounded call with its own record**, and the
    // parts offered disjoint ranges of the artifact.
    let records = part_records(&fixture);
    assert_eq!(records.len(), parts, "one record per part: {records:?}");
    let mut offered: Vec<(usize, usize)> = Vec::new();
    for (number, (selector, answer, shown, item)) in records.iter().enumerate() {
        assert!(
            selector.contains(&artifact) && selector.contains(&digest),
            "{selector}"
        );
        // **A part is its item's question**, and its record says whose, as
        // a selection's does; only a discovery step belongs to no item.
        assert_eq!(item, ITEM, "part {number} names another item");
        assert!(
            answer.get("chose_ids").is_some(),
            "part {number} did not answer with ids: {answer:?}"
        );
        for one in shown {
            let path = one.get("path").and_then(Value::as_str).expect("a path");
            let (at, range) = path.rsplit_once('@').expect("artifact@range");
            assert_eq!(at, artifact, "a part offered another artifact");
            offered.push(range_of(range));
            assert!(
                one.get("digest").is_some(),
                "a record identifies what it showed by digest"
            );
        }
    }
    offered.sort_unstable();
    for pair in offered.windows(2) {
        assert!(
            pair[0].1 <= pair[1].0,
            "two parts offered overlapping ranges: {pair:?}"
        );
    }
    let mut selectors: Vec<&str> = records.iter().map(|(s, ..)| s.as_str()).collect();
    selectors.sort_unstable();
    for (number, selector) in selectors.iter().enumerate() {
        assert!(
            selector.ends_with(&format!("part {} of {parts}", number + 1)),
            "{selector}"
        );
    }
    // Charged once per part, and to this request.
    let charged: Vec<(String, String, i64)> = ledger(&fixture.data())
        .into_iter()
        .filter(|(request, ..)| request == "assisted")
        .collect();
    assert_eq!(charged.len(), parts, "{charged:?}");
    provider.stop();
}

#[test]
fn an_input_over_the_projections_capacity_is_insufficient_capacity_and_never_a_partial_summary() {
    // **J2's second negative control.** An input larger than the
    // summariser's own capacity ends as a typed outcome: the item unmet
    // with `insufficient_capacity`, no section that could be read as a
    // summary of part of it, nothing declared as if some of it had been
    // projected, and no call — with investigation and without.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = oversize_log();
    assert!(
        log.len() > 1 << 20,
        "the fixture is meant to be over a mebibyte"
    );
    let (artifact, digest) = ingest(&fixture, "oversize.log", log.as_bytes());

    for (request, investigation) in [("assisted", 8), ("deterministic", 0)] {
        let inspected = asked(&fixture, request, &artifact, &digest, investigation);
        assert_eq!(
            result(&inspected, ITEM),
            ("unmet".to_string(), "insufficient_capacity".to_string()),
            "{request}: {inspected:?}"
        );
        let packet = sealed_packet(&fixture, request);
        assert!(
            projection_section(&packet).is_none(),
            "{request}: a section was published for an input nothing could read whole: {packet:?}"
        );
        assert!(
            item_omissions(&packet).is_empty(),
            "{request}: omissions were declared as if part of it had been projected"
        );
    }
    assert!(part_records(&fixture).is_empty(), "a part was asked");
    assert!(ledger(&fixture.data()).is_empty(), "a call was charged");

    // **Too large is decided before a byte is read**, which bytes that are
    // not text are the way to see: nothing in them is offered to a model,
    // so no count of parts could say they were too large, and read anyway
    // they would be declared omitted whole as though they had been read.
    let blob: Vec<u8> = (0..(1_u32 << 20)).map(|n| (n % 251) as u8 | 0x80).collect();
    let (blob_artifact, blob_digest) = ingest(&fixture, "blob.bin", &blob);
    let inspected = asked(&fixture, "blob", &blob_artifact, &blob_digest, 0);
    assert_eq!(
        result(&inspected, ITEM),
        ("unmet".to_string(), "insufficient_capacity".to_string()),
        "{inspected:?}"
    );
    assert!(projection_section(&sealed_packet(&fixture, "blob")).is_none());
    provider.stop();
}

/// Plain text in `blocks` blocks of twenty 90-byte lines: each block is
/// one excerpt unit, and the escaped bytes of the units decide the parts.
fn blocks_of_lines(blocks: usize) -> String {
    format!("{}\n", "z".repeat(89)).repeat(20 * blocks)
}

/// A figure J2's harness declares, read from its source: `MAX_PARTS`,
/// which `projection::tests` holds to the provider's own constant.
fn declared_by_the_harness(name: &str) -> usize {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scripts")
        .join("j2_run.py");
    let source = std::fs::read_to_string(script).expect("the harness");
    let at = source
        .find(&format!("\n{name} = "))
        .unwrap_or_else(|| panic!("the harness no longer declares {name}"));
    source[at + name.len() + 4..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("a number")
}

#[test]
fn an_input_inside_the_size_bound_that_needs_more_parts_than_a_projection_has_is_insufficient_capacity()
 {
    // **J2's second control, where the other bound bites**, and at the
    // call site rather than only in `projection::next`: an input the size
    // check admits, whose units would need one part more than a projection
    // may ask. A partial summary here would be the rule's projection of
    // it, which is exactly what the control forbids — with a model and
    // without. The same input one block shorter fills the bound itself
    // and is projected, so it is the part count that refuses.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let max_parts = declared_by_the_harness("MAX_PARTS");

    let at_bound = blocks_of_lines(68);
    let (artifact, digest) = ingest(&fixture, "at-bound.log", at_bound.as_bytes());
    let inspected = asked(&fixture, "at-bound", &artifact, &digest, 0);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    assert_eq!(
        projection_of(&fixture, "at-bound", at_bound.as_bytes()).parts(),
        max_parts,
        "the shorter input is meant to fill exactly the bound"
    );

    let over = blocks_of_lines(69);
    assert!(
        over.len() <= 131_072,
        "the input is meant to pass the size check, INPUT_BYTES: {}",
        over.len()
    );
    let (artifact, digest) = ingest(&fixture, "over.log", over.as_bytes());
    for (request, investigation) in [("assisted", max_parts + 1), ("deterministic", 0)] {
        let inspected = asked(&fixture, request, &artifact, &digest, investigation);
        assert_eq!(
            result(&inspected, ITEM),
            ("unmet".to_string(), "insufficient_capacity".to_string()),
            "{request}: {inspected:?}"
        );
        let packet = sealed_packet(&fixture, request);
        assert!(
            projection_section(&packet).is_none(),
            "{request}: a section was published for an input no projection could hold"
        );
        assert!(
            item_omissions(&packet).is_empty(),
            "{request}: omissions were declared as if part of it had been projected"
        );
    }
    assert!(part_records(&fixture).is_empty(), "a part was asked");
    assert!(ledger(&fixture.data()).is_empty(), "a call was charged");
    provider.stop();
}

/// A conformance-shaped document whose run identity is most of a
/// projection: an `environment` of about fourteen kilobytes, then `small`
/// results of about a hundred bytes and `big` ones of about 1.9
/// kilobytes. The rule carries the identity and leaves room for a small
/// result and not for a big one, so **what a model is offered is fewer
/// parts than the document fills**.
fn identity_heavy(small: usize, big: usize) -> String {
    let environment: Vec<String> = (0..60)
        .map(|n| format!("    \"key_{n:02}\": \"{}\"", "v".repeat(220)))
        .collect();
    let mut results: Vec<String> = (0..small)
        .map(|n| format!("    {{\"fixture\": \"small.case-{n:03}\", \"outcome\": \"pass\", \"note\": \"quick\"}}"))
        .collect();
    results.extend((0..big).map(|n| {
        format!(
            "    {{\"fixture\": \"big.case-{n:03}\", \"outcome\": \"pass\", \"transcript\": \"{}\"}}",
            "t".repeat(1900)
        )
    }));
    format!(
        "{{\n  \"environment\": {{\n{}\n  }},\n  \"format\": \"combraton-conformance-result/1\",\n  \
         \"results\": [\n{}\n  ],\n  \"summary\": {{\n    \"pass\": {}\n  }}\n}}\n",
        environment.join(",\n"),
        results.join(",\n"),
        small + big
    )
}

#[test]
fn a_projection_the_investigation_limit_cannot_cover_is_not_started() {
    // **Its parts are one flow and are claimed together** (READINESS §6).
    // A projection with a part never read would name failures from part
    // of a log as if from all of it, so a limit one short of the parts
    // starts none of them. **The parts are the ones a model is offered**:
    // this document fills four, and only the two holding results small
    // enough to add are asked, so a limit of two is enough and a limit of
    // one is not.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let document = identity_heavy(128, 20);
    let big_results = 20 * 1900;
    assert!(
        big_results > 32 * 1024,
        "the big results alone are more than one part"
    );
    let (artifact, digest) = ingest(&fixture, "identity-heavy.json", document.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", document.as_bytes()).parts();
    assert_eq!(parts, 2, "the offer is meant to be two parts of the four");

    let inspected = asked(&fixture, "short", &artifact, &digest, parts - 1);
    assert_eq!(
        result(&inspected, ITEM),
        (
            "unmet".to_string(),
            "investigation_budget_exhausted".to_string()
        ),
        "{inspected:?}"
    );
    assert!(projection_section(&sealed_packet(&fixture, "short")).is_none());
    assert!(
        part_records(&fixture).is_empty(),
        "a part was asked though the flow could not finish"
    );

    let inspected = asked(&fixture, "enough", &artifact, &digest, parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    assert_eq!(part_records(&fixture).len(), parts);
    provider.stop();
}

#[test]
fn a_part_that_answers_outside_its_offer_leaves_the_item_unmet_with_that_reason() {
    // **Failure is the item's typed reason, never a fall back to the
    // deterministic rule**: that would report a model-assisted projection
    // no model made. Every call that was made is still recorded and
    // still charged.
    let fixture = Fixture::narrow(&["ids:u999"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();

    let inspected = asked(&fixture, "refused", &artifact, &digest, parts);
    assert_eq!(
        result(&inspected, ITEM),
        ("unmet".to_string(), "model_choice_not_offered".to_string()),
        "{inspected:?}"
    );
    assert!(projection_section(&sealed_packet(&fixture, "refused")).is_none());
    let records = part_records(&fixture);
    assert_eq!(records.len(), parts, "every call made is recorded");
    for (_, answer, ..) in &records {
        assert_eq!(
            answer.get("unmet").and_then(Value::as_str),
            Some("model_choice_not_offered"),
            "{answer:?}"
        );
    }
    provider.stop();
}

#[test]
fn with_no_investigation_the_projection_carries_run_identity_and_the_failures() {
    // **The deterministic rule**, which is the baseline a live run is
    // compared against: no model, and what a parser can read is carried
    // first — the run's identity, then every failure's own block.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    let inspected = asked(&fixture, "baseline", &artifact, &digest, 0);
    assert_eq!(result(&inspected, ITEM).0, "satisfied");
    let read = projection_of(&fixture, "baseline", log.as_bytes());
    assert!(
        read.how().contains("chosen by the deterministic rule"),
        "{:?}",
        read.header
    );
    for name in LARGE_FAILURES {
        let block = format!("---- {name} stdout ----");
        assert!(
            read.extents.iter().any(|extent| matches!(
                extent,
                Extent::Carried { bytes, kind, .. }
                    if kind == "failure" && String::from_utf8_lossy(bytes).contains(&block)
            )),
            "the failure block of {name} was not carried"
        );
    }
    assert!(part_records(&fixture).is_empty());
    assert!(ledger(&fixture.data()).is_empty());
    provider.stop();
}

/// The byte range `artifact@start-end` names.
fn range_of_path(path: &str) -> (usize, usize) {
    let (_, range) = path.rsplit_once('@').expect("artifact@range");
    range_of(range)
}

/// The extent holding byte `at`.
fn extent_at(read: &Read, at: usize) -> &Extent {
    read.extents
        .iter()
        .find(|extent| {
            let (start, end) = extent.range();
            start <= at && at < end
        })
        .expect("every byte is in an extent")
}

/// The excerpts the header says the model added, by number, or `None`
/// when it names no model.
fn added(read: &Read) -> Option<Vec<usize>> {
    let (_, list) = read.how().split_once(", which added ")?;
    if list == "nothing" {
        return Some(Vec::new());
    }
    Some(
        list.split(", ")
            .map(|id| {
                id.strip_prefix('e')
                    .and_then(|number| number.parse().ok())
                    .unwrap_or_else(|| panic!("not an excerpt id: {id:?}"))
            })
            .collect(),
    )
}

/// The ranges of a projection's carried excerpts, by their numbers.
fn excerpt_ranges(read: &Read, numbers: &[usize]) -> Vec<(usize, usize)> {
    read.extents
        .iter()
        .filter_map(|extent| match extent {
            Extent::Carried {
                number, start, end, ..
            } if numbers.contains(number) => Some((*start, *end)),
            _ => None,
        })
        .collect()
}

/// Every carried excerpt of `floor` is a carried excerpt of `read`, at the
/// same range: **the rule's projection is a floor**, and nothing a model
/// answers takes any of it away.
fn assert_carries_the_floor(read: &Read, floor: &Read) {
    for extent in &floor.extents {
        if let Extent::Carried { start, end, .. } = extent {
            assert!(
                read.extents.iter().any(|mine| matches!(
                    mine,
                    Extent::Carried { start: from, end: to, .. } if from == start && to == end
                )),
                "the rule's excerpt at {start}-{end} is not carried: {:?}",
                read.extents
            );
        }
    }
}

#[test]
fn what_the_model_added_is_carried_beside_everything_the_rule_carries() {
    // **A projection that says the model added excerpts carries them**, or
    // the label is a lie, **and carries everything the rule carries**, or
    // the model has undone what a parser found — J2 live's failure. The
    // fake names the second unit of every part it is offered, which in
    // this log is passing tests. So the rule's excerpts are all there, at
    // their ranges; the excerpts the header lists are exactly the units
    // the records say were chosen; and the projection is not the rule's.
    // An answer resolved against the wrong units, or not read at all,
    // fails one of the three.
    let fixture = Fixture::narrow(&["ids:u2"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let ruled = projection_of(&fixture, "baseline", log.as_bytes());
    let parts = ruled.parts();
    assert!(parts >= 2, "the fixture is meant to ask more than one part");

    let inspected = asked(&fixture, "assisted", &artifact, &digest, parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    let records = part_records(&fixture);
    assert_eq!(records.len(), parts);
    for (selector, answer, offered, _) in records {
        let ids: Vec<&str> = answer
            .get("chose_ids")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(ids, ["u2"], "{selector}");
        let second = offered
            .iter()
            .find(|candidate| candidate.get("id").and_then(Value::as_str) == Some("u2"))
            .unwrap_or_else(|| panic!("{selector} offered no u2"));
        chosen.push(range_of_path(
            second.get("path").and_then(Value::as_str).expect("a path"),
        ));
    }
    chosen.sort_unstable();
    // The fixture's premise: what the model chose is not a failure.
    for (start, end) in &chosen {
        let text = &log[*start..*end];
        assert!(
            !text.contains(" ... FAILED") && !text.contains("---- "),
            "u2 is meant to be passing tests, and is {text:?}"
        );
    }

    let read = projection_of(&fixture, "assisted", log.as_bytes());
    assert!(
        read.how()
            .contains("chosen by the model, one question per part"),
        "{:?}",
        read.header
    );
    let listed = added(&read).unwrap_or_else(|| panic!("no model named: {:?}", read.header));
    assert_eq!(
        excerpt_ranges(&read, &listed),
        chosen,
        "the excerpts the header lists are not what the model chose"
    );
    assert_carries_the_floor(&read, &ruled);
    for name in LARGE_FAILURES {
        let block = log
            .find(&format!("---- {name} stdout ----"))
            .expect("every failure has its block");
        assert!(
            matches!(extent_at(&read, block), Extent::Carried { .. }),
            "the failure {name} was not carried: {:?}",
            extent_at(&read, block)
        );
    }
    assert_ne!(
        read.extents, ruled.extents,
        "the model's projection is the rule's"
    );
    provider.stop();
}

#[test]
fn a_failure_the_model_did_not_choose_is_still_carried() {
    // **J2 live, as a control.** Both models left out failure blocks the
    // parser had found, and the projection declared them `not_selected`.
    // A model that chooses nothing now leaves the rule's projection as it
    // was: every excerpt, every omission and every reason, and the header
    // says the model added nothing.
    let fixture = Fixture::narrow(&["ids:"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let ruled = projection_of(&fixture, "baseline", log.as_bytes());
    let parts = ruled.parts();
    assert!(parts >= 1);

    let inspected = asked(&fixture, "assisted", &artifact, &digest, parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let records = part_records(&fixture);
    assert_eq!(records.len(), parts, "the model was asked every part");
    for (selector, answer, ..) in &records {
        assert_eq!(
            answer
                .get("chose_ids")
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(0),
            "{selector}: {answer:?}"
        );
    }
    let read = projection_of(&fixture, "assisted", log.as_bytes());
    assert!(
        read.how().ends_with(
            "excerpts chosen by the deterministic rule: failures, then run identity; \
             then chosen by the model, one question per part, which added nothing"
        ),
        "{:?}",
        read.header
    );
    let without_how = |read: &Read| {
        read.header
            .iter()
            .filter(|line| !line.starts_with("read as "))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(without_how(&read), without_how(&ruled));
    assert_eq!(read.named, ruled.named);
    assert_eq!(
        read.extents, ruled.extents,
        "a model that chose nothing changed the rule's ledger"
    );
    provider.stop();
}

#[test]
fn a_log_whose_floor_leaves_no_room_asks_no_model_and_spends_nothing() {
    // **A question no answer can change is not asked.** Forty failures,
    // each between passing tests, are forty excerpts where a projection
    // holds twenty-four: the rule's floor is full, nothing a model could
    // add would fit, and a request that authorises every part a projection
    // may ask gets the rule's projection, with no record and no charge.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let failing: Vec<(usize, usize)> = (0..40).map(|test| (0, test * 2 + 1)).collect();
    let log = test_log(1, 80, &failing);
    let (artifact, digest) = ingest(&fixture, "full.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let ruled = projection_of(&fixture, "baseline", log.as_bytes());
    assert_eq!(ruled.parts(), 0, "{:?}", ruled.header);

    let max_parts = declared_by_the_harness("MAX_PARTS");
    let inspected = asked(&fixture, "assisted", &artifact, &digest, max_parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let read = projection_of(&fixture, "assisted", log.as_bytes());
    assert!(
        read.how()
            .ends_with("excerpts chosen by the deterministic rule: failures, then run identity"),
        "{:?}",
        read.header
    );
    assert_eq!(read.header, ruled.header);
    assert_eq!(read.extents, ruled.extents);
    assert!(part_records(&fixture).is_empty(), "a part was asked");
    assert!(ledger(&fixture.data()).is_empty(), "a call was charged");
    provider.stop();
}

/// A conformance manifest of `results` results, the first of them failing
/// and the third `unsupported` with its reason: shaped as the core suite's
/// is, where an unsupported fixture is not a failure and the rule has no
/// reason to carry it. **The failure comes first in the part**, so the
/// offer and the plan number the unsupported result differently.
fn core_manifest(results: usize) -> String {
    let entries: Vec<String> = (0..results)
        .map(|at| {
            let (outcome, reason) = match at {
                0 => ("fail", "null"),
                2 => ("unsupported", "\"core.capabilities is not claimed\""),
                _ => ("pass", "null"),
            };
            format!(
                "    {{\n      \"failed_step\": null,\n      \"fixture\": \"core.fixture-number-{at}-holds\",\n      \
                 \"fixture_digest\": \"sha256:{at:064x}\",\n      \"outcome\": \"{outcome}\",\n      \
                 \"reason\": {reason},\n      \"requirements\": [\n        \"CORE-{at}\"\n      ],\n      \
                 \"transcript\": \"transcripts/core.fixture-number-{at}-holds.jsonl\",\n      \"version\": 1\n    }}"
            )
        })
        .collect();
    format!(
        "{{\n  \"format\": \"combraton-conformance-result/1\",\n  \"results\": [\n{}\n  ],\n  \
         \"summary\": {{\n    \"fail\": 1,\n    \"pass\": {},\n    \"unsupported\": 1\n  }}\n}}\n",
        entries.join(",\n"),
        results - 2
    )
}

#[test]
fn an_unsupported_record_the_model_adds_is_carried_and_attributed_to_it() {
    // **Where a model has a job the rule cannot do.** An unsupported
    // fixture is not a failure, so the rule does not carry it; a model
    // that names it adds it, the header says the model added it, and the
    // rule's own excerpts, the failing result among them, are all still
    // there. The failure is not offered, so the unsupported result is the
    // second id the model is shown, and the third unit its part holds.
    let fixture = Fixture::narrow(&["ids:u2"]);
    let provider = fixture.start();
    let json = core_manifest(50);
    let (artifact, digest) = ingest(&fixture, "core.manifest.json", json.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let ruled = projection_of(&fixture, "baseline", json.as_bytes());
    let parts = ruled.parts();
    assert_eq!(parts, 1, "{:?}", ruled.header);

    let inspected = asked(&fixture, "assisted", &artifact, &digest, parts);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let records = part_records(&fixture);
    assert_eq!(records.len(), 1);
    let (_, _, offered, _) = &records[0];
    let second = offered
        .iter()
        .find(|candidate| candidate.get("id").and_then(Value::as_str) == Some("u2"))
        .expect("u2 was offered");
    let range = range_of_path(second.get("path").and_then(Value::as_str).expect("a path"));
    assert!(
        json[range.0..range.1].contains("\"outcome\": \"unsupported\""),
        "the fixture's premise: u2 is the unsupported result, and is {:?}",
        &json[range.0..range.1]
    );
    let failing = json
        .find("\"fixture\": \"core.fixture-number-0-holds\"")
        .expect("the failing result");
    assert!(
        matches!(extent_at(&ruled, failing), Extent::Carried { .. }),
        "the rule carries the failing result"
    );
    assert!(
        !matches!(extent_at(&ruled, range.0), Extent::Carried { .. }),
        "the rule already carries the unsupported result"
    );

    let read = projection_of(&fixture, "assisted", json.as_bytes());
    let listed = added(&read).unwrap_or_else(|| panic!("no model named: {:?}", read.header));
    assert_eq!(excerpt_ranges(&read, &listed), [range]);
    assert_carries_the_floor(&read, &ruled);
    provider.stop();
}

#[test]
fn a_small_result_is_carried_whole_and_asks_nothing() {
    // **A call that cannot change the answer is not made.** Everything
    // fits, so everything is carried and nothing is omitted, whatever
    // the request authorised.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = small_log();
    let (artifact, digest) = ingest(&fixture, "small.log", log.as_bytes());
    let inspected = asked(&fixture, "small", &artifact, &digest, 2);
    assert_eq!(result(&inspected, ITEM).0, "satisfied");
    let read = projection_of(&fixture, "small", log.as_bytes());
    assert!(
        read.extents
            .iter()
            .all(|extent| matches!(extent, Extent::Carried { .. })),
        "something was omitted from a log that fits: {:?}",
        read.extents
    );
    assert!(read.how().contains("everything fits"), "{:?}", read.header);
    assert!(part_records(&fixture).is_empty(), "a call was made");
    assert!(ledger(&fixture.data()).is_empty());
    provider.stop();
}

#[test]
fn a_json_document_is_projected_by_its_structure_and_its_failures_are_named() {
    // **A JSON path, where the format allows it.** The runner's own
    // manifest: its identity members are carried, each failing result is
    // named by its fixture, and every excerpt is still the document's
    // bytes at a stated range — so a value and its unit arrive exactly as
    // they were written.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let json = manifest_json(160);
    let (artifact, digest) = ingest(&fixture, "manifest.json", json.as_bytes());
    let inspected = asked(&fixture, "json", &artifact, &digest, 0);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let read = projection_of(&fixture, "json", json.as_bytes());
    assert!(read.how().starts_with("read as json:"), "{:?}", read.header);
    let names: Vec<&str> = read.named.iter().map(|(name, ..)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "context.fixture-number-11-holds",
            "context.fixture-number-70-holds"
        ]
    );
    let carried: String = read
        .extents
        .iter()
        .filter_map(|extent| match extent {
            Extent::Carried { bytes, .. } => Some(String::from_utf8_lossy(bytes).to_string()),
            Extent::Omitted { .. } => None,
        })
        .collect();
    assert!(
        carried.contains("\"format\": \"combraton-conformance-result/1\""),
        "the run's identity was not carried"
    );
    assert!(
        carried.contains("\"fixture\": \"context.fixture-number-70-holds\""),
        "a failing result was not carried"
    );
    provider.stop();
}

#[test]
fn bytes_that_are_not_text_are_declared_omitted_whole() {
    // Nothing is guessed about bytes that are not text: the reference
    // stands, and every byte is declared, as one extent, with its reason.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let bytes: Vec<u8> = (0..20_000u32).map(|n| (n % 251) as u8 | 0x80).collect();
    let (artifact, digest) = ingest(&fixture, "blob.bin", &bytes);
    let inspected = asked(&fixture, "binary", &artifact, &digest, 2);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let read = projection_of(&fixture, "binary", &bytes);
    assert_eq!(
        read.extents,
        vec![Extent::Omitted {
            number: 1,
            start: 0,
            end: bytes.len(),
            reason: "not_text".to_string(),
        }]
    );
    assert!(part_records(&fixture).is_empty());
    provider.stop();
}

#[test]
fn a_log_with_crlf_line_endings_is_read_as_cargos_and_carried_as_its_own_bytes() {
    // **A projected value is the source's bytes**, and a carriage return is
    // one of them. A log written with `\r\n` is still cargo's output — its
    // failures are named — and every excerpt is checked against the sealed
    // bytes, so an excerpt with its line endings normalised is a different
    // length from the range it states and fails the reading.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log().replace('\n', "\r\n");
    let (artifact, digest) = ingest(&fixture, "crlf.log", log.as_bytes());
    let inspected = asked(&fixture, "crlf", &artifact, &digest, 0);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let read = projection_of(&fixture, "crlf", log.as_bytes());
    assert!(
        read.how().starts_with("read as libtest:"),
        "{:?}",
        read.header
    );
    let names: Vec<&str> = read.named.iter().map(|(name, ..)| name.as_str()).collect();
    assert_eq!(names, LARGE_FAILURES);
    let carried_returns = read
        .extents
        .iter()
        .filter(|extent| {
            matches!(extent, Extent::Carried { bytes, .. } if bytes.windows(2).any(|pair| pair == b"\r\n"))
        })
        .count();
    assert!(
        carried_returns > 0,
        "no excerpt carried a line ending, so nothing here reached the property"
    );
    provider.stop();
}

/// Seal `bytes` as the owner, over raw frames, **under capture anchors
/// given here**, and hand back the digest.
///
/// `evidence/1` lets a producer name any anchor of 1 to 256 characters.
/// `cbr ingest` names a checkout's tree and commit and nothing else, and a
/// verb added to the product so that a test could write any anchor would
/// be the wrong way round; so this speaks the protocol the way
/// `purge_as_owner` does.
fn seal_with_anchors(
    fixture: &Fixture,
    artifact: &str,
    bytes: &[u8],
    anchors: &[(&str, &str)],
) -> String {
    use std::io::{BufRead, BufReader, Write};
    let text = |value: &str| Value::String(value.to_string());
    let object = |members: Vec<(&str, Value)>| {
        Value::Object(
            members
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        )
    };
    let credential = std::fs::read_to_string(fixture.directory.path().join("credential"))
        .expect("the owner's credential");
    let stream = std::os::unix::net::UnixStream::connect(&fixture.socket).expect("connects");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    let mut id = 0;
    let mut call = |method: &str, params: Value| -> Value {
        id += 1;
        let frame = object(vec![
            ("jsonrpc", text("2.0")),
            ("id", Value::Int(id)),
            ("method", text(method)),
            ("params", params),
        ]);
        writer
            .write_all(&cbr_encoding::to_canonical(&frame))
            .expect("writes");
        writer.write_all(b"\n").expect("writes");
        let mut line = String::new();
        reader.read_line(&mut line).expect("reads");
        cbr_encoding::parse(line.trim().as_bytes())
            .expect("canonical JSON")
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("{method} refused: {line}"))
    };
    call(
        "core.authenticate",
        object(vec![
            ("operation", text("core.authenticate")),
            ("message_id", text("a")),
            (
                "payload",
                object(vec![("credential", text(credential.trim()))]),
            ),
        ]),
    );
    let profile = |name: &str, features: &[&str]| {
        object(vec![
            ("name", text(name)),
            ("majors", Value::Array(vec![Value::Int(1)])),
            ("required", Value::Bool(true)),
            (
                "required_features",
                Value::Array(features.iter().map(|feature| text(feature)).collect()),
            ),
            ("optional_features", Value::Array(Vec::new())),
        ])
    };
    let negotiated = call(
        "core.negotiate",
        object(vec![
            ("operation", text("core.negotiate")),
            ("message_id", text("n")),
            (
                "payload",
                object(vec![
                    (
                        "caller",
                        object(vec![("name", text("t")), ("version", text("1"))]),
                    ),
                    (
                        "receive_limits",
                        object(vec![("max_frame_bytes", Value::Int(1_048_576))]),
                    ),
                    (
                        "profiles",
                        Value::Array(vec![
                            profile("core", &["core.events", "core.grants"]),
                            profile("evidence", &[]),
                        ]),
                    ),
                ]),
            ),
        ]),
    );
    let generation = match negotiated
        .get("dedupe_window")
        .and_then(|window| window.get("current"))
    {
        Some(Value::Int(current)) => *current,
        _ => 1,
    };
    let subject = object(vec![
        ("kind", text("evidence.artifact")),
        ("id", text(artifact)),
    ]);
    let mut command = |operation: &str, name: &str, revision: i64, payload: Value| {
        let mut envelope = object(vec![
            ("operation", text(operation)),
            ("message_id", text(name)),
            ("command_id", text(&format!("{artifact}.{name}"))),
            ("dedupe_generation", Value::Int(generation)),
            ("subject", subject.clone()),
            (
                "preconditions",
                Value::Array(vec![object(vec![
                    ("subject", subject.clone()),
                    ("revision", Value::Int(revision)),
                ])]),
            ),
            ("requires", Value::Array(Vec::new())),
            ("payload", payload),
        ]);
        let digest = cbr_encoding::command_digest(&envelope).expect("a command digest");
        if let Value::Object(members) = &mut envelope {
            members.push(("command_digest".to_string(), text(&digest)));
        }
        call(operation, envelope)
    };
    let digest = cbr_encoding::digest_bytes(bytes);
    let anchors = anchors
        .iter()
        .map(|(kind, id)| object(vec![("kind", text(kind)), ("id", text(id))]))
        .collect();
    let prepared = command(
        "evidence.upload.prepare",
        "prepare",
        0,
        object(vec![
            ("digest", text(&digest)),
            ("size", Value::Int(bytes.len() as i64)),
            ("media_type", text("text/plain")),
            (
                "producer",
                object(vec![("producer_id", text("journey-two"))]),
            ),
            (
                "source",
                object(vec![("kind", text("file")), ("id", text("anchored.log"))]),
            ),
            ("scope", text("local")),
            (
                "capture",
                object(vec![
                    ("captured_at", text("2026-09-25T00:00:00Z")),
                    ("anchors", Value::Array(anchors)),
                ]),
            ),
            ("coverage", object(vec![("completeness", text("complete"))])),
            ("retention_class", text("standard")),
        ]),
    );
    let limit = match prepared
        .get("outcome")
        .and_then(|outcome| outcome.get("chunk_limit"))
    {
        Some(Value::Int(limit)) if *limit > 0 => *limit as usize,
        other => panic!("no chunk limit: {other:?}"),
    };
    let mut revision = 1;
    for (index, chunk) in bytes.chunks(limit).enumerate() {
        command(
            "evidence.upload.append",
            &format!("append{index}"),
            revision,
            object(vec![
                ("offset", Value::Int((index * limit) as i64)),
                ("data_base64", text(&cbr_encoding::encode_base64(chunk))),
            ]),
        );
        revision += 1;
    }
    let sealed = command("evidence.seal", "seal", revision, object(Vec::new()));
    assert_eq!(
        sealed
            .get("outcome")
            .and_then(|outcome| outcome.get("digest"))
            .and_then(Value::as_str),
        Some(digest.as_str()),
        "{sealed:?}"
    );
    digest
}

#[test]
fn a_capture_anchor_stays_on_one_line_whatever_it_holds() {
    // **The header is CBR's own account of what it read**, and a capture
    // anchor is the one thing in it CBR did not write. `evidence/1` lets a
    // producer name any anchor, and CBR does not check anchors at seal, so
    // one that held a newline could write header lines of its own — a
    // `failures named: 0` above the real count, which a reader takes
    // first. Each anchor is shown on one line, escaped, and the header
    // holds exactly one of each of CBR's lines.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = small_log();
    let forged = "abc\nfailures named: 0\nread as lines: 1 bytes, 1 lines, 1 units in 0 parts; \
                  excerpts chosen by nobody: everything fits";
    let digest = seal_with_anchors(&fixture, "anchored.1", log.as_bytes(), &[("note", forged)]);
    let inspected = asked(&fixture, "anchored", "anchored.1", &digest, 0);
    assert_eq!(result(&inspected, ITEM).0, "satisfied", "{inspected:?}");
    let packet = sealed_packet(&fixture, "anchored");
    let content = projection_section(&packet)
        .and_then(|section| {
            section
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .expect("a section");
    let starting = |prefix: &str| {
        content
            .split('\n')
            .filter(|line| line.starts_with(prefix))
            .count()
    };
    assert_eq!(starting("failures named: "), 1, "{content}");
    assert_eq!(starting("read as "), 1, "{content}");

    let read = projection_of(&fixture, "anchored", log.as_bytes());
    assert_eq!(read.named_count, 1, "the real count, not the anchor's");
    assert!(
        read.how().starts_with("read as libtest:"),
        "{:?}",
        read.header
    );
    assert!(
        read.header
            .contains(&format!("captured at note {}", forged.replace('\n', "\\n"))),
        "the anchor is not shown escaped on its own line: {:?}",
        read.header
    );
    provider.stop();
}

// ---- the readable set ------------------------------------------------------

/// A grant for the reader over the context subjects and both registered
/// repositories, with `evidence.read` over exactly `artifacts`.
fn grant_over(fixture: &Fixture, id: &str, artifacts: &[&str]) {
    let mut resources = vec![
        r#"{"kind":"context.request"}"#.to_string(),
        r#"{"kind":"context.job"}"#.to_string(),
        r#"{"kind":"context.packet"}"#.to_string(),
        r#"{"kind":"cbr.repository","id":"app"}"#.to_string(),
        r#"{"kind":"cbr.repository","id":"outside"}"#.to_string(),
    ];
    for artifact in artifacts {
        resources.push(format!(
            r#"{{"kind":"evidence.artifact","id":"{artifact}"}}"#
        ));
    }
    fixture.issue_grant(
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

/// A packet's body with its request's own id taken out, so two packets
/// for two requests can be compared for everything else.
fn without_request(packet: &Value) -> String {
    let mut packet = packet.clone();
    if let Value::Object(members) = &mut packet {
        members.retain(|(name, _)| name != "request");
    }
    String::from_utf8(cbr_encoding::to_canonical(&packet)).expect("utf-8")
}

#[test]
fn evidence_the_submitter_cannot_read_is_unavailable_exactly_as_evidence_that_does_not_exist() {
    // **A projection copies an artifact's bytes into a packet**, so the
    // packet is a way to read the artifact — and preparation runs on the
    // provider's own authority. The readable set is resolved at the
    // command, where the grant is, as the view and the claims are: an
    // artifact outside it is `evidence_unavailable`, and so is one that
    // was never sealed, with nothing to tell the two apart.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = small_log();
    let (artifact, digest) = ingest(&fixture, "small.log", log.as_bytes());
    grant_over(&fixture, "g-without", &[]);
    grant_over(&fixture, "g-with", &[&artifact]);

    let outside = asked_as(
        &fixture,
        Some("g-without"),
        "outside",
        &artifact,
        &digest,
        0,
    );
    assert_eq!(
        result(&outside, ITEM),
        ("unmet".to_string(), "evidence_unavailable".to_string())
    );
    let never = asked_as(
        &fixture,
        Some("g-without"),
        "never",
        "ingest.0000000000000000.1",
        &digest,
        0,
    );
    assert_eq!(
        result(&never, ITEM),
        ("unmet".to_string(), "evidence_unavailable".to_string())
    );
    let packet = |request: &str| {
        let printed =
            fixture.cbr_as_reader("g-without", &["packet", request, "--excerpt", "1000000"]);
        assert!(printed.status.success());
        serving::sealed(
            &cbr_encoding::parse(String::from_utf8_lossy(&printed.stdout).trim().as_bytes())
                .expect("canonical JSON"),
        )
    };
    assert_eq!(
        without_request(&packet("outside")),
        without_request(&packet("never")),
        "an artifact outside the grant is distinguishable from one that does not exist"
    );
    assert!(
        !without_request(&packet("outside")).contains("test result"),
        "and none of its bytes reached the packet"
    );

    // The other arm: the same reader, with the artifact in its grant.
    let within = asked_as(&fixture, Some("g-with"), "within", &artifact, &digest, 0);
    assert_eq!(result(&within, ITEM).0, "satisfied", "{within:?}");
    provider.stop();
}

#[test]
fn a_purged_artifact_is_unavailable_though_its_bytes_are_still_stored() {
    // **A purge is an order to destroy evidence**, and a projection copies
    // bytes into a packet, so projecting a purged artifact would publish
    // what somebody asked to be destroyed. A purge's row stays `sealed`,
    // and its object is kept for as long as another sealed artifact names
    // the same bytes — so neither the state nor a missing object says it
    // was purged. The same bytes are ingested twice, one is purged, and
    // the other, still available, shows the object is there.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = small_log();
    let (purged, digest) = ingest(&fixture, "small.log", log.as_bytes());
    let (kept, kept_digest) = ingest(&fixture, "small.log", log.as_bytes());
    assert_ne!(purged, kept);
    assert_eq!(digest, kept_digest, "one object, named twice");
    fixture.purge_as_owner(
        &purged,
        serving::artifact_revision(&fixture.data(), &purged),
    );

    let inspected = asked(&fixture, "purged", &purged, &digest, 0);
    assert_eq!(
        result(&inspected, ITEM),
        ("unmet".to_string(), "evidence_unavailable".to_string()),
        "a purged artifact was projected: {inspected:?}"
    );
    let packet = sealed_packet(&fixture, "purged");
    assert!(projection_section(&packet).is_none(), "{packet:?}");
    assert!(item_omissions(&packet).is_empty());

    let inspected = asked(&fixture, "kept", &kept, &kept_digest, 0);
    assert_eq!(
        result(&inspected, ITEM).0,
        "satisfied",
        "the bytes are meant to be still stored: {inspected:?}"
    );
    provider.stop();
}

#[test]
fn stored_bytes_that_no_longer_match_their_digest_are_unavailable_and_never_projected() {
    // **A projected value is the source's bytes**, and the source is what
    // was sealed, not what the store holds now. The object is changed on
    // disk with the provider stopped, one byte and the same length, as
    // `fetch` is tested against altered bytes behind honest metadata: the
    // projection must not carry them as the artifact's.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = small_log();
    let (artifact, digest) = ingest(&fixture, "small.log", log.as_bytes());
    provider.stop();

    let path = object_path(&fixture.data(), &digest);
    let mut altered = std::fs::read(&path).expect("the stored object");
    assert_eq!(altered, log.as_bytes());
    let at = log.find("FAILED").expect("a failure");
    altered[at] = b'P';
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).expect("writable");
    std::fs::write(&path, &altered).expect("alters the object");

    let provider = fixture.start();
    let inspected = asked(&fixture, "altered", &artifact, &digest, 0);
    assert_eq!(
        result(&inspected, ITEM),
        ("unmet".to_string(), "evidence_unavailable".to_string()),
        "bytes that are not the sealed digest were projected: {inspected:?}"
    );
    let packet = sealed_packet(&fixture, "altered");
    assert!(projection_section(&packet).is_none(), "{packet:?}");
    assert!(item_omissions(&packet).is_empty());
    provider.stop();
}

#[test]
fn a_parts_record_is_readable_only_by_a_reader_who_can_read_the_evidence_it_was_made_from() {
    // **A part's record identifies what it showed by range and digest**,
    // and a digest of a short excerpt is something a reader can confirm
    // by guessing. So the record is sealed under the evidence the job
    // could read, as it is under the job's view and claims, and all
    // three doors check it: `inspect`, a listing, and `fetch`.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    asked(&fixture, "assisted", &artifact, &digest, parts);

    let records: Vec<(String, String)> = derivations(&fixture.data())
        .into_iter()
        .map(|(id, record)| {
            let digest = record
                .get("descriptor")
                .and_then(|descriptor| descriptor.get("digest"))
                .and_then(Value::as_str)
                .expect("a digest")
                .to_string();
            (id, digest)
        })
        .collect();
    assert_eq!(records.len(), parts, "{records:?}");
    let ids: Vec<&str> = records.iter().map(|(id, _)| id.as_str()).collect();
    // Both grants cover the records themselves and both repositories;
    // the only difference between them is the log.
    grant_over(&fixture, "g-records", &ids);
    let mut with_log = ids.clone();
    with_log.push(&artifact);
    grant_over(&fixture, "g-records-and-log", &with_log);

    let (record, record_digest) = &records[0];
    let inspect = |grant: &str| {
        fixture.query_as_reader(
            grant,
            "evidence.inspect",
            &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{record}"}}}}"#),
        )
    };
    let listed = |grant: &str| -> Vec<String> {
        fixture
            .query_as_reader(grant, "evidence.query", "{}")
            .get("result")
            .and_then(|result| result.get("items"))
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|item| item.get("artifact"))
            .filter_map(|subject| subject.get("id"))
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    };
    let fetch = |grant: &str, name: &str| {
        let out = fixture.directory.path().join(name);
        let fetched = fixture.cbr_as_reader(
            grant,
            &[
                "fetch",
                record,
                "--digest",
                record_digest,
                "--out",
                out.to_str().expect("utf-8"),
            ],
        );
        (fetched.status.success(), out.exists())
    };

    let refused = inspect("g-records");
    assert_eq!(
        refused
            .get("error")
            .and_then(|error| error.get("data"))
            .and_then(|data| data.get("code"))
            .and_then(Value::as_str),
        Some("permission_denied"),
        "inspect served a reader who cannot read the log: {refused:?}"
    );
    assert!(
        !listed("g-records").contains(record),
        "the listing named it to a reader who cannot read the log"
    );
    assert_eq!(
        fetch("g-records", "refused.json"),
        (false, false),
        "fetch served it to a reader who cannot read the log"
    );

    assert!(
        inspect("g-records-and-log").get("result").is_some(),
        "inspect refused a reader who can read the log"
    );
    assert!(
        listed("g-records-and-log").contains(record),
        "the listing hid it from a reader who can read the log"
    );
    assert_eq!(
        fetch("g-records-and-log", "served.json"),
        (true, true),
        "fetch refused a reader who can read the log"
    );
    provider.stop();
}

/// Submit a request naming **two** artifacts, as the owner.
fn asked_for_both(
    fixture: &Fixture,
    request: &str,
    first: (&str, &str),
    second: (&str, &str),
    investigation: usize,
) -> Value {
    asked_for_both_as(fixture, None, request, first, second, investigation)
}

/// The same, as the reader under `grant` when one is given.
fn asked_for_both_as(
    fixture: &Fixture,
    grant: Option<&str>,
    request: &str,
    first: (&str, &str),
    second: (&str, &str),
    investigation: usize,
) -> Value {
    let run = |arguments: &[&str]| match grant {
        Some(grant) => fixture.cbr_as_reader(grant, arguments),
        None => fixture.cbr(arguments),
    };
    let investigation = investigation.to_string();
    let wants = [
        format!("{ITEM}=evidence:{}@{}", first.0, first.1),
        format!("other=evidence:{}@{}", second.0, second.1),
    ];
    let submitted = run(&[
        "context",
        request,
        "--repo",
        fixture.outside.to_str().expect("utf-8"),
        "--repo-id",
        "j2",
        "--obligation",
        "required_before_start",
        "--task",
        "which tests failed, and what did they assert",
        "--capacity",
        "65536",
        "--investigation",
        &investigation,
        "--want",
        &wants[0],
        "--want",
        &wants[1],
    ]);
    assert!(
        submitted.status.success(),
        "{}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = Instant::now();
    loop {
        let polled = run(&["request", request]);
        let inspected =
            cbr_encoding::parse(String::from_utf8_lossy(&polled.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if inspected.get("state").and_then(Value::as_str) != Some("preparing") {
            return inspected;
        }
        assert!(started.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_parts_record_is_sealed_under_the_artifact_it_showed_and_no_other() {
    // **A record is no wider than what its question showed.** A request
    // naming two artifacts projects each on its own; a part of the log's
    // projection shows the log and nothing of the other, so its record is
    // sealed under the log alone — and a reader who may read the log, and
    // not the other, is served it at the door and answered from it by a
    // rebuild.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    let (other, other_digest) = ingest(&fixture, "small.log", small_log().as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    let both = asked_for_both(
        &fixture,
        "both",
        (&artifact, &digest),
        (&other, &other_digest),
        parts,
    );
    assert_eq!(result(&both, ITEM).0, "satisfied", "{both:?}");
    let records = derivations(&fixture.data());
    assert_eq!(records.len(), parts, "{records:?}");
    for (id, record) in &records {
        let sealed_under: Vec<&str> = record
            .get("readable_under")
            .and_then(|under| under.get("readable_evidence"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{id} names no evidence it was sealed under"))
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(sealed_under, [artifact.as_str()], "{id}");
    }
    let (record, record_digest) = {
        let (id, record) = &records[0];
        let digest = record
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("digest"))
            .and_then(Value::as_str)
            .expect("a digest")
            .to_string();
        (id.clone(), digest)
    };
    let mut readable: Vec<&str> = records.iter().map(|(id, _)| id.as_str()).collect();
    readable.push(&artifact);
    grant_over(&fixture, "g-log", &readable);
    let out = fixture.directory.path().join("served.json");
    let fetched = fixture.cbr_as_reader(
        "g-log",
        &[
            "fetch",
            &record,
            "--digest",
            &record_digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(
        fetched.status.success(),
        "a reader of the log was refused a record that shows only the log: {}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    provider.stop();

    let rebuilding = fixture.start_replaying();
    let rebuilt = asked_as(
        &fixture,
        Some("g-log"),
        "rebuilt",
        &artifact,
        &digest,
        parts,
    );
    assert_eq!(
        result(&rebuilt, ITEM).0,
        "satisfied",
        "a rebuild refused a reader of the log the records of its projection: {rebuilt:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_projection_rebuilt_offline_reproduces_its_section() {
    // **Replay, by m4d's rule, for every part.** The same store, no
    // transport, no credential: each part's question is answered from
    // its retained record, and the projection is the same bytes.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    asked(&fixture, "live", &artifact, &digest, parts);
    let live = projection_section(&sealed_packet(&fixture, "live")).expect("a section");
    provider.stop();

    let rebuilding = fixture.start_replaying();
    let rebuilt = asked(&fixture, "rebuilt", &artifact, &digest, parts);
    assert_eq!(result(&rebuilt, ITEM).0, "satisfied", "{rebuilt:?}");
    let again = projection_section(&sealed_packet(&fixture, "rebuilt")).expect("a section");
    assert_eq!(
        live.get("content"),
        again.get("content"),
        "the rebuilt projection differs from the live one"
    );
    rebuilding.stop();
}

// ---- a record that answers outside its own question ------------------------

/// Where a store keeps the object sealed under `digest`.
fn object_path(data: &std::path::Path, digest: &str) -> std::path::PathBuf {
    let hex = digest
        .split_once(':')
        .map(|(_, hex)| hex)
        .expect("a digest");
    data.join("objects")
        .join("sha256")
        .join(&hex[0..2])
        .join(&hex[2..4])
        .join(&hex[4..])
}

/// A member of an object, replaced.
fn with(value: &Value, member: &str, replacement: Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, found)| {
                    if name == member {
                        (name.clone(), replacement.clone())
                    } else {
                        (name.clone(), found.clone())
                    }
                })
                .collect(),
        ),
        other => panic!("not an object: {other:?}"),
    }
}

#[test]
fn a_rebuild_refuses_a_record_that_chose_outside_its_own_part() {
    // **The closed set is checked again on the way out**, and this is the
    // only way to reach the second check: a retained record whose answer
    // names an id its own question never offered. A store that holds one
    // is corrupted or forged, and a rebuild from it must not quietly drop
    // the stranger and project from the rest. The record is rewritten at
    // its own digest, with the provider stopped, as m4h's test of a
    // previous format does.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    let live = asked(&fixture, "live", &artifact, &digest, parts);
    assert_eq!(result(&live, ITEM).0, "satisfied");
    provider.stop();

    let data = fixture.data();
    let (id, row) = derivations(&data)
        .into_iter()
        .next()
        .expect("a part record");
    let sealed = row
        .get("descriptor")
        .and_then(|descriptor| descriptor.get("digest"))
        .and_then(Value::as_str)
        .expect("a digest")
        .to_string();
    let record = cbr_encoding::parse(&std::fs::read(object_path(&data, &sealed)).expect("object"))
        .expect("a record");
    let forged = cbr_encoding::to_canonical(&with(
        &record,
        "answer",
        Value::Object(vec![(
            "chose_ids".to_string(),
            Value::Array(vec![
                Value::String("u1".to_string()),
                Value::String("u999".to_string()),
            ]),
        )]),
    ));
    let forged_digest = cbr_encoding::digest_bytes(&forged);
    let path = object_path(&data, &forged_digest);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
    std::fs::write(&path, &forged).expect("the forged object");
    let descriptor = with(
        &with(
            row.get("descriptor").expect("a descriptor"),
            "digest",
            Value::String(forged_digest),
        ),
        "size",
        Value::Int(forged.len() as i64),
    );
    let renamed = String::from_utf8(cbr_encoding::to_canonical(&with(
        &row,
        "descriptor",
        descriptor,
    )))
    .expect("utf-8");
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let updated = connection
        .execute(
            "UPDATE subjects SET value = ?1 WHERE kind = 'evidence.artifact' AND id = ?2",
            rusqlite::params![renamed, id],
        )
        .expect("updates the row");
    assert_eq!(updated, 1);
    drop(connection);

    let rebuilding = fixture.start_replaying();
    let rebuilt = asked(&fixture, "rebuilt", &artifact, &digest, parts);
    assert_eq!(
        result(&rebuilt, ITEM),
        ("unmet".to_string(), "model_choice_not_offered".to_string()),
        "a rebuild projected from a record that chose outside its part: {rebuilt:?}"
    );
    rebuilding.stop();
}

#[test]
fn a_rebuild_honours_every_artifact_a_record_is_sealed_under() {
    // **The rebuild is a door like fetch**, and it asks about every
    // artifact the record's row says it was sealed under — against the
    // evidence the rebuilding request may read, which is resolved at its
    // command from the artifacts it names. A part is sealed under exactly
    // the artifact it showed, so no ordinary store puts a second one there;
    // the row is widened here, with the provider stopped, to show the door
    // reads the row rather than assuming it.
    let fixture = Fixture::narrow(&["ids:u1"]);
    let provider = fixture.start();
    let log = large_log();
    let (artifact, digest) = ingest(&fixture, "large.log", log.as_bytes());
    let (other, _) = ingest(&fixture, "small.log", small_log().as_bytes());
    asked(&fixture, "baseline", &artifact, &digest, 0);
    let parts = projection_of(&fixture, "baseline", log.as_bytes()).parts();
    asked(&fixture, "live", &artifact, &digest, parts);
    grant_over(&fixture, "g-log", &[&artifact]);
    grant_over(&fixture, "g-both", &[&artifact, &other]);
    provider.stop();

    let data = fixture.data();
    let (id, row) = derivations(&data)
        .into_iter()
        .next()
        .expect("a part record");
    let under = row.get("readable_under").expect("a readable set").clone();
    let widened = with(
        &row,
        "readable_under",
        with(
            &under,
            "readable_evidence",
            Value::Array(vec![
                Value::String(artifact.clone()),
                Value::String(other.clone()),
            ]),
        ),
    );
    let connection = rusqlite::Connection::open(data.join("cbr.sqlite")).expect("opens the store");
    let updated = connection
        .execute(
            "UPDATE subjects SET value = ?1 WHERE kind = 'evidence.artifact' AND id = ?2",
            rusqlite::params![
                String::from_utf8(cbr_encoding::to_canonical(&widened)).expect("utf-8"),
                id
            ],
        )
        .expect("updates the row");
    assert_eq!(updated, 1);
    drop(connection);

    let rebuilding = fixture.start_replaying();
    let narrow = asked_as(&fixture, Some("g-log"), "narrow", &artifact, &digest, parts);
    assert_eq!(
        result(&narrow, ITEM),
        ("unmet".to_string(), "model_answer_not_retained".to_string()),
        "a rebuild answered a reader from a record sealed under an artifact they cannot read"
    );
    // A request that names both, by a reader who may read both, is
    // answered: the rule is about what the reader may read, not a refusal
    // of everybody.
    let small_digest = cbr_encoding::digest_bytes(small_log().as_bytes());
    let wide = asked_for_both_as(
        &fixture,
        Some("g-both"),
        "wide",
        (&artifact, &digest),
        (&other, &small_digest),
        parts,
    );
    assert_eq!(result(&wide, ITEM).0, "satisfied", "{wide:?}");
    rebuilding.stop();
}
