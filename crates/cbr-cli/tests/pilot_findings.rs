//! **What the M3d pilots found, as four tests written before the fixes.**
//!
//! Each test here is the gate for one change, and each one describes a
//! defect a pilot ran into on a repository nobody wrote for CBR. They are
//! stated as properties of the compiler, not as properties of those two
//! repositories, because a fix fitted to a pilot question is worth nothing:
//!
//! 1. **A dirty working tree's modified tracked files are named.** brian2
//!    had none, so nothing showed; Knowscroll-v2 had twenty-one, which were
//!    searched at their committed bytes with no gap saying so. An unstated
//!    gap is the false-absence rule's own case (INTERNALS section 5).
//! 2. **A symbolic link never supplies an item's content.** Knowscroll's
//!    `AGENTS.md` is a link to `CLAUDE.md`; the indexer skips links on
//!    purpose and `select_source` read the blob anyway, citing the link
//!    target's *name*, nine bytes, as the file's content.
//! 3. **Claim relevance discriminates.** In a one-repository store the rule
//!    "a condition names a repository of the basis" selects every claim, so
//!    twenty-two of twenty-two went into a packet about one of them.
//! 4. **A span reaches its neighbours.** In both pilots something the
//!    question wanted sat a few lines past a cited span's edge, on the far
//!    side of a fixed twenty-line chunk boundary.
//!
//! No model runs. Every repository here is written by the test.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

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

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args([
            "-c",
            "user.name=cbr-test",
            "-c",
            "user.email=cbr-test@invalid",
            "-c",
            "commit.gpgsign=false",
        ])
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

/// A provider over a checkout the test builds, with the `cbr` client.
struct Fixture {
    directory: tempfile::TempDir,
    socket: PathBuf,
    checkout: PathBuf,
}

impl Fixture {
    /// `build` writes the checkout's committed files; it is committed
    /// afterwards, so anything the test writes later is a working-tree
    /// change rather than part of the tree.
    fn new(build: impl FnOnce(&Path)) -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let sockets = directory.path().join("s");
        std::fs::create_dir(&sockets).expect("socket dir");
        std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
        std::fs::write(
            directory.path().join("cbr.json"),
            r#"{"format":"cbr-config/1","principal":"owner"}"#,
        )
        .expect("config");

        let checkout = directory.path().join("app");
        std::fs::create_dir_all(&checkout).expect("checkout");
        build(&checkout);
        git(&checkout, &["init", "-q", "-b", "main"]);
        git(&checkout, &["add", "-A"]);
        git(&checkout, &["commit", "-q", "-m", "the tree"]);

        Self {
            socket: sockets.join("cbr.sock"),
            checkout,
            directory,
        }
    }

    fn data(&self) -> PathBuf {
        self.directory.path().join("data")
    }

    fn start(&self) -> Child {
        let child = Command::new(provider_binary())
            .arg("--data-dir")
            .arg(self.data())
            .arg("--config")
            .arg(self.directory.path().join("cbr.json"))
            .arg("--socket")
            .arg(&self.socket)
            .arg("--register-repository")
            .arg(format!("app={}", self.checkout.display()))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("provider starts");
        let started = Instant::now();
        while UnixStream::connect(&self.socket).is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
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

    fn write(&self, name: &str, text: &str) -> String {
        let path = self.directory.path().join(name);
        std::fs::write(&path, text).expect("writes");
        path.to_str().expect("utf-8").to_string()
    }

    /// Submit one request over this checkout and read the packet it
    /// published, with the whole of its own text.
    fn packet(&self, request: &str, selector: &str, task: &str, wants: &[&str]) -> Value {
        let mut arguments: Vec<String> = vec![
            "context".into(),
            request.into(),
            "--repo".into(),
            self.checkout.to_str().expect("utf-8").into(),
            "--repo-id".into(),
            "app".into(),
            "--selector".into(),
            selector.into(),
            "--task".into(),
            task.into(),
            "--capacity".into(),
            "65536".into(),
        ];
        for want in wants {
            arguments.push("--want".into());
            arguments.push((*want).into());
        }
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let submitted = self.cbr(&borrowed);
        assert!(
            submitted.status.success(),
            "submit: {}",
            String::from_utf8_lossy(&submitted.stderr)
        );
        let _ = self.cbr(&["request", request]);
        ok(&self.cbr(&["packet", request, "--excerpt", "1000000"]))
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

fn list(value: &Value, path: &[&str]) -> Vec<Value> {
    at(value, path).as_array().unwrap_or_default().to_vec()
}

fn field(output: &Output, name: &str) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")).map(str::to_string))
        .unwrap_or_else(|| panic!("no {name} line in {output:?}"))
}

/// Every gap line the packet's coverage carries, across repositories.
fn gaps(packet: &Value) -> Vec<String> {
    list(packet, &["coverage"])
        .iter()
        .flat_map(|entry| list(entry, &["gaps"]))
        .filter_map(|gap| gap.as_str().map(str::to_string))
        .collect()
}

fn item(packet: &Value, item_id: &str) -> Value {
    list(packet, &["items"])
        .into_iter()
        .find(|entry| text(entry, &["item_id"]) == item_id)
        .unwrap_or_else(|| panic!("no item {item_id} in {packet:?}"))
}

/// The section whose id is `section_id`, with the content the packet gives.
fn section_content(packet: &Value, section_id: &str) -> String {
    let sealed = sealed_packet(packet);
    list(&sealed, &["sections"])
        .into_iter()
        .find(|entry| text(entry, &["section_id"]) == section_id)
        .map(|entry| text(&entry, &["content"]))
        .unwrap_or_else(|| panic!("no section {section_id}"))
}

/// The packet's own bytes, parsed. `context.packet.inspect` returns the
/// section metadata in its result and the sealed document in `excerpt`;
/// the content lives in the sealed document.
fn sealed_packet(packet: &Value) -> Value {
    use std::io::Read as _;
    let data = text(packet, &["excerpt", "data_base64"]);
    let mut bytes = Vec::new();
    base64_decode(&data)
        .as_slice()
        .read_to_end(&mut bytes)
        .expect("reads");
    cbr_encoding::parse(&bytes).expect("the sealed packet is canonical JSON")
}

fn base64_decode(text: &str) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte == b'=' || byte == b'\n' || byte == b'\r' {
            continue;
        }
        let Some(index) = TABLE.iter().position(|candidate| *candidate == byte) else {
            continue;
        };
        buffer = (buffer << 6) | index as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 1. A dirty working tree's modified tracked files are named in the coverage
// ---------------------------------------------------------------------------

#[test]
fn a_modified_tracked_file_is_named_in_the_coverage_beside_the_untracked_ones() {
    // The index is built at the committed tree, which is the owner's
    // decision and is right: the working tree is not a basis anyone else
    // can resolve. What was wrong is that the packet did not say so. A
    // caller could not tell "searched and not found" from "searched a
    // version of this file that is no longer on disk".
    let fixture = Fixture::new(|checkout| {
        std::fs::write(checkout.join("guide.md"), "# Guide\n\nThe queue drains.\n")
            .expect("writes");
        std::fs::write(checkout.join("keep.md"), "# Keep\n\nUnchanged.\n").expect("writes");
    });
    // One tracked file modified after the commit, and one untracked file.
    std::fs::write(
        fixture.checkout.join("guide.md"),
        "# Guide\n\nThe queue drains on a deadline now.\n",
    )
    .expect("writes");
    std::fs::write(fixture.checkout.join("scratch.txt"), "not committed\n").expect("writes");

    let mut provider = fixture.start();
    let packet = fixture.packet(
        "dirty",
        "queue",
        "what happens when the queue drains",
        &["guide=source:guide.md"],
    );
    let gaps = gaps(&packet);

    assert!(
        gaps.iter().any(|gap| gap.contains("modified")),
        "the coverage names the modified tracked files: {gaps:?}"
    );
    assert!(
        gaps.iter().any(|gap| gap.starts_with("1 tracked file")),
        "counted, and there is exactly one: {gaps:?}"
    );
    assert!(
        gaps.iter()
            .any(|gap| gap.trim_start().starts_with("of those, 1 are .md")),
        "and named by kind, as the untracked gap is: {gaps:?}"
    );
    // The untracked gap is still there: this adds a gap, it does not
    // replace one.
    assert!(
        gaps.iter().any(|gap| gap.contains("untracked")),
        "the untracked gap is unchanged: {gaps:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

// ---------------------------------------------------------------------------
// 2. A symbolic link never supplies an item's content
// ---------------------------------------------------------------------------

#[test]
fn a_link_inside_the_tree_is_followed_to_its_target_and_one_pointing_out_is_unmet() {
    let fixture = Fixture::new(|checkout| {
        std::fs::create_dir_all(checkout.join("docs")).expect("docs");
        std::fs::write(
            checkout.join("docs/real.md"),
            "# Real\n\nThe adapter rewrites the queue's response field.\n",
        )
        .expect("writes");
        // A link that resolves inside the tree, as Knowscroll's AGENTS.md
        // does, and one that points outside it, which nothing in the tree
        // can supply.
        std::os::unix::fs::symlink("docs/real.md", checkout.join("inside.md")).expect("symlink");
        std::os::unix::fs::symlink("../elsewhere.md", checkout.join("outside.md"))
            .expect("symlink");
    });

    let mut provider = fixture.start();
    let packet = fixture.packet(
        "links",
        "adapter",
        "what does the adapter do to the queue",
        &["in=source:inside.md", "out=source:outside.md"],
    );

    assert_eq!(
        text(&item(&packet, "in"), &["result"]),
        "satisfied",
        "a link inside the tree is followed: {packet:?}"
    );
    let content = section_content(&packet, "s-in");
    assert!(
        content.contains("docs/real.md"),
        "and cited at the target's path, not the link's: {content}"
    );
    assert!(
        content.contains("rewrites the queue"),
        "with the target's bytes as the content: {content}"
    );
    assert!(
        !content.lines().any(|line| line.trim() == "docs/real.md"),
        "never the link's own bytes, which are just the target's name: {content}"
    );

    let outside = item(&packet, "out");
    assert_eq!(
        text(&outside, &["result"]),
        "unmet",
        "a link out of the tree cannot be satisfied from it: {packet:?}"
    );
    assert!(
        text(&outside, &["reason"]).contains("link"),
        "and the reason says it is a link: {outside:?}"
    );
    let gaps = gaps(&packet);
    assert!(
        gaps.iter()
            .any(|gap| gap.contains("outside.md") && gap.contains("elsewhere.md")),
        "the coverage says where it points, which a 64-character reason cannot: {gaps:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

// ---------------------------------------------------------------------------
// 3. Claim relevance discriminates among eligible claims
// ---------------------------------------------------------------------------

#[test]
fn among_many_eligible_claims_the_packet_carries_the_ones_the_question_is_about() {
    let fixture = Fixture::new(|checkout| {
        std::fs::write(
            checkout.join("DECISIONS.md"),
            "# Decisions\n\nEvery decision of this repository lives in this one file.\n",
        )
        .expect("writes");
        std::fs::write(checkout.join("src.rs"), "pub fn thing() {}\n").expect("writes");
    });
    let mut provider = fixture.start();

    // `cbr basis` prints the target and then a human line, so only the
    // first line is canonical JSON.
    let printed = fixture.cbr(&[
        "basis",
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
        "--repo-id",
        "app",
    ]);
    let first = String::from_utf8_lossy(&printed.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let basis = cbr_encoding::parse(first.as_bytes()).expect("canonical JSON");
    let tree = list(&basis, &["repositories"])
        .first()
        .map(|entry| text(entry, &["tree"]))
        .expect("a tree");

    let ingested = fixture.cbr(&[
        "ingest",
        fixture
            .checkout
            .join("DECISIONS.md")
            .to_str()
            .expect("utf-8"),
        "--media-type",
        "text/markdown",
        "--source-kind",
        "human_decision_record",
        "--repo",
        fixture.checkout.to_str().expect("utf-8"),
    ]);
    let artifact = field(&ingested, "artifact");
    let digest = field(&ingested, "digest");
    ok(&fixture.cbr(&["authority", "bind", "app", "--authority", "owner"]));

    // Eight claims, every one of them eligible: each carries a condition
    // naming the repository the basis names, which is exactly the case
    // Knowscroll produced. Only one of them is about the question.
    let subjects = [
        ("d-retention", "how long a recording is retained"),
        ("d-naming", "how a module is named"),
        ("d-formatting", "how source is formatted"),
        ("d-licence", "which licence the repository uses"),
        (
            "d-credentials",
            "how credentials in a recording are redacted",
        ),
        ("d-testing", "how a test names what it proves"),
        ("d-review", "who reviews a change before it lands"),
        ("d-release", "when a release is tagged"),
    ];
    for (claim, statement) in subjects {
        let content = fixture.write(
            &format!("{claim}.json"),
            &format!(
                r#"{{"plane":"normative",
                     "statement":{{"subject":{{"kind":"app.decision","id":"{claim}"}},
                                   "predicate":"decides","value":"{statement}",
                                   "cardinality":"single"}},
                     "scope":{{"id":"app","qualifiers":{{}}}},
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
                                     "repository":"app","expected":"{tree}"}}]}}"#
            ),
        );
        ok(&fixture.cbr(&["propose", claim, "--content", &content]));
        ok(&fixture.cbr(&[
            "decide",
            &format!("dec-{claim}"),
            "--claim",
            claim,
            "--revision",
            "1",
            "--decision",
            "accepted_for_use",
            "--use",
            "binding",
            "--rationale",
            "the owner decided it",
        ]));
    }

    let packet = fixture.packet(
        "claims",
        "credentials redacted",
        "how must credentials in a recording be redacted",
        &["src=source:src.rs"],
    );

    let carried: Vec<String> = list(&packet, &["sections"])
        .iter()
        .map(|section| text(section, &["section_id"]))
        .filter(|id| id.starts_with("d-claim-"))
        .collect();
    assert!(
        carried.contains(&"d-claim-d-credentials".to_string()),
        "the one claim the question is about is carried: {carried:?}"
    );
    assert!(
        carried.len() < subjects.len(),
        "eligibility is not selection: {carried:?}"
    );
    let omitted: Vec<String> = list(&packet, &["omissions"])
        .iter()
        .filter(|omission| text(omission, &["reason"]) == "applicability")
        .map(|omission| text(omission, &["section_id"]))
        .collect();
    assert_eq!(
        carried.len() + omitted.len(),
        subjects.len(),
        "every eligible claim is either carried or counted: {carried:?} {omitted:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}

// ---------------------------------------------------------------------------
// 4. A span reaches its neighbours
// ---------------------------------------------------------------------------

/// Twenty lines that hold the query's word, then the sentence that answers
/// it two lines into the next chunk. A twenty-line boundary falls between
/// them, which is the whole point: the boundary is an artefact of indexing
/// and a reader should not lose an answer to it.
fn straddling_file() -> String {
    let mut text = String::new();
    for line in 0..19 {
        text.push_str(&format!(
            "Line {line}: the retention policy is discussed here.\n"
        ));
    }
    text.push_str("Line 19: the retention policy, continued.\n");
    text.push_str("Line 20: a sentence of no interest.\n");
    text.push_str("Line 21: recordings are deleted after thirty days, always.\n");
    for line in 22..40 {
        text.push_str(&format!(
            "Line {line}: unrelated prose about other things.\n"
        ));
    }
    text
}

#[test]
fn a_cited_span_reaches_the_neighbouring_chunk_when_the_excerpt_has_room() {
    let fixture = Fixture::new(|checkout| {
        std::fs::write(checkout.join("policy.md"), straddling_file()).expect("writes");
        // A second file whose two chunks both rank, so discovery returns
        // two adjacent hits in one file. Each widens towards the other and
        // they land on the same bytes: the case that published one section
        // twice on brian2.
        let mut adjacent = String::new();
        for line in 0..40 {
            // Short lines on purpose: a chunk has to be small enough that
            // its neighbour fits inside `EXCERPT_BYTES` beside it, or
            // neither widens and the collision never happens.
            adjacent.push_str(&format!("L{line}: retention policy, recordings.\n"));
        }
        std::fs::write(checkout.join("notes.md"), adjacent).expect("writes");
    });
    let mut provider = fixture.start();
    let packet = fixture.packet(
        "neighbours",
        "retention policy",
        "what is the retention policy for recordings",
        &["policy=source:policy.md"],
    );

    let content = section_content(&packet, "s-policy");
    assert!(
        content.contains("deleted after thirty days"),
        "the answer two lines past the chunk edge is inside the excerpt: {content}"
    );
    // Still one contiguous range of the artifact, and the locator still
    // says which: extending a span must not turn it into a stitched-up
    // window a reader cannot check by fetching the citation.
    let locator = content.lines().next().unwrap_or_default().to_string();
    assert!(
        locator.contains("excerpt is bytes"),
        "the locator states the range: {locator}"
    );
    let range: Vec<i64> = locator
        .rsplit("excerpt is bytes ")
        .next()
        .unwrap_or_default()
        .split(" of the artifact")
        .next()
        .unwrap_or_default()
        .split('-')
        .filter_map(|part| part.parse().ok())
        .collect();
    assert_eq!(range.len(), 2, "two byte offsets: {locator}");
    let body = content.split_once('\n').map(|(_, rest)| rest).unwrap_or("");
    assert_eq!(
        (range[1] - range[0]) as usize,
        body.len(),
        "the stated range is exactly the excerpt's length: {locator}"
    );

    // **Two adjacent hits in one file widen towards each other**, and the
    // first form of this rule published the same span twice: brian2's
    // rerun came back with one section repeated. Overlap is decided after
    // widening, so a section id appears once.
    let sealed = sealed_packet(&packet);
    let ids: Vec<String> = list(&sealed, &["sections"])
        .iter()
        .map(|section| text(section, &["section_id"]))
        .collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        ids.len(),
        unique.len(),
        "no section is published twice: {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id.contains("notes.md")),
        "and the file whose two chunks collide is in the packet: {ids:?}"
    );

    provider.kill().expect("kills");
    provider.wait().expect("reaps");
}
