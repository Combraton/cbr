//! Knowledge behaviour no single conformance fixture shows end to end, driven
//! against the real provider binary over stdio.
//!
//! - **J9** (JOURNEYS): an authority transfer invalidates the stale decision
//!   path without editing any history, and a transfer does not reset reliance.
//! - **The derived-artifact ancestry control** (PROTOCOL-PIN section 3): two
//!   derivations over one captured artifact, each declaring that artifact as
//!   its root, are one lineage.
//!
//! Each start of the provider acts as one principal, as the launch
//! configuration assigns it on stdio; the data directory persists across
//! starts, so a sequence of starts is a sequence of principals acting on one
//! store.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

use cbr_encoding::Value;

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cbr-provider")
}

fn parse(text: &str) -> Value {
    cbr_encoding::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{e:?}: {text}"))
}

fn canonical(value: &Value) -> String {
    String::from_utf8(cbr_encoding::to_canonical(value)).expect("utf-8")
}

struct Provider {
    child: Child,
    reader: BufReader<ChildStdout>,
    next: i64,
}

impl Provider {
    /// Start as `principal` over the store in `directory`, and negotiate core
    /// with events and grants, evidence with retention control, and knowledge.
    fn start(directory: &Path, principal: &str) -> Self {
        let config = directory.join("config.json");
        std::fs::write(
            &config,
            format!(
                r#"{{"format":"combraton-conformance-config/1","principal":"{principal}","authority_principals":["owner"]}}"#
            ),
        )
        .expect("config");
        let mut child = Command::new(binary())
            .arg("--data-dir")
            .arg(directory.join("data"))
            .arg("--config")
            .arg(&config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("provider starts");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        let mut provider = Self {
            child,
            reader,
            next: 1,
        };
        let negotiated = provider.query(
            "core.negotiate",
            r#"{"caller":{"name":"knowledge-tests","version":"1"},"receive_limits":{"max_frame_bytes":1048576},"profiles":[{"name":"core","majors":[1],"required":true,"required_features":["core.events","core.grants"],"optional_features":[]},{"name":"evidence","majors":[1],"required":true,"required_features":[],"optional_features":[]},{"name":"knowledge","majors":[1],"required":true,"required_features":[],"optional_features":[]}]}"#,
            None,
        );
        assert!(negotiated.get("result").is_some(), "{negotiated:?}");
        provider
    }

    fn send(&mut self, method: &str, params: String) -> Value {
        let id = self.next;
        self.next += 1;
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
        )
        .expect("writes");
        stdin.flush().expect("flushes");
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("reads");
        parse(line.trim_end())
    }

    fn query(&mut self, operation: &str, payload: &str, grant: Option<&str>) -> Value {
        let grant = grant.map_or(String::new(), |g| format!(r#","grant":"{g}""#));
        self.send(
            operation,
            format!(
                r#"{{"operation":"{operation}","message_id":"q-{}"{grant},"payload":{payload}}}"#,
                self.next
            ),
        )
    }

    /// A command on `subject` at precondition `revision`, with the digest the
    /// provider recomputes, optionally under a grant and an authority epoch.
    #[allow(clippy::too_many_arguments)]
    fn command(
        &mut self,
        operation: &str,
        command_id: &str,
        subject: (&str, &str),
        revision: i64,
        payload: &str,
        grant: Option<&str>,
        epoch: Option<i64>,
    ) -> Value {
        let subject = format!(r#"{{"kind":"{}","id":"{}"}}"#, subject.0, subject.1);
        let mut extra = String::new();
        if let Some(g) = grant {
            extra.push_str(&format!(r#","grant":"{g}""#));
        }
        if let Some(e) = epoch {
            extra.push_str(&format!(r#","authority_epoch":{e}"#));
        }
        let envelope = format!(
            r#"{{"operation":"{operation}","message_id":"m-{command_id}-{}","command_id":"{command_id}","dedupe_generation":1,"subject":{subject},"preconditions":[{{"subject":{subject},"revision":{revision}}}],"requires":[]{extra},"payload":{payload}}}"#,
            self.next
        );
        let digest = cbr_encoding::command_digest(&parse(&envelope)).expect("intent");
        let envelope = envelope.replacen(
            r#""payload""#,
            &format!(r#""command_digest":"{digest}","payload""#),
            1,
        );
        self.send(operation, envelope)
    }

    fn stop(mut self) {
        drop(self.child.stdin.take());
        self.child.wait().expect("exits");
    }
}

fn result(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected a result: {response:?}"))
}

fn error_code(response: &Value) -> String {
    response
        .get("error")
        .and_then(|e| e.get("data"))
        .and_then(|d| d.get("code"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected an error: {response:?}"))
        .to_string()
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut current = value;
    for name in path {
        current = current
            .get(name)
            .unwrap_or_else(|| panic!("no {name} in {value:?}"));
    }
    current
}

const GRANT_RESOURCES: &str = r#"[{"kind":"knowledge.claim"},{"kind":"knowledge.decision"},{"kind":"knowledge.conflict"},{"kind":"knowledge.evaluation"},{"kind":"knowledge.authority"}]"#;

fn issue_grant(owner: &mut Provider, id: &str, holder: &str) {
    let issued = owner.command(
        "core.grant.issue",
        &format!("grant-{id}"),
        ("core.grant", id),
        0,
        &format!(
            r#"{{"holder":"{holder}","audience":"conformance-provider","rights":["knowledge.propose","knowledge.read","knowledge.decide"],"resources":{GRANT_RESOURCES},"delegation":{{"allowed":false,"max_depth":0}}}}"#
        ),
        None,
        None,
    );
    result(&issued);
}

const CLAIM_CONTENT: &str = r#""plane":"normative","statement":{"subject":{"kind":"app.service","id":"billing"},"predicate":"queue","value":"v2","cardinality":"single"},"scope":{"id":"svc","qualifiers":{}},"support":[],"derivation":{"kind":"human","inputs":[]}"#;

fn decision_payload(reference: &Value, decision: &str, supersedes: Option<&str>) -> String {
    let used = if decision == "accepted_for_use" {
        r#","permitted_use":"binding""#
    } else {
        ""
    };
    let supersedes = supersedes.map_or(String::new(), |s| {
        format!(r#","supersedes_decision":"{s}""#)
    });
    format!(
        r#"{{"claim":{},"decision":"{decision}"{used}{supersedes},"validation_basis":{{"evidence":[],"receipts":[]}},"rationale":"j9"}}"#,
        canonical(reference)
    )
}

/// J9. An authority transfer or epoch change invalidates the stale decision
/// path, without editing any history; a transfer does not reset reliance.
#[test]
fn j9_a_transfer_invalidates_the_stale_decision_path_without_editing_history() {
    let directory = tempfile::tempdir().expect("temp dir");
    let dir = directory.path();

    let mut owner = Provider::start(dir, "owner");
    issue_grant(&mut owner, "g-a", "authority-a");
    issue_grant(&mut owner, "g-b", "authority-b");
    let bound = owner.command(
        "knowledge.authority.bind",
        "bind-svc",
        ("knowledge.authority", "svc"),
        0,
        r#"{"authority":"authority-a"}"#,
        None,
        None,
    );
    assert_eq!(at(result(&bound), &["outcome", "epoch"]), &Value::Int(1));
    owner.stop();

    // Authority A proposes and accepts, under epoch 1.
    let mut a = Provider::start(dir, "authority-a");
    let proposed = a.command(
        "knowledge.claim.propose",
        "propose-c",
        ("knowledge.claim", "c"),
        0,
        &format!("{{{CLAIM_CONTENT}}}"),
        Some("g-a"),
        None,
    );
    let reference = at(result(&proposed), &["outcome", "reference"]).clone();
    let accepted = a.command(
        "knowledge.decision.record",
        "decide-d1",
        ("knowledge.decision", "d1"),
        0,
        &decision_payload(&reference, "accepted_for_use", None),
        Some("g-a"),
        Some(1),
    );
    assert_eq!(at(result(&accepted), &["outcome", "epoch"]), &Value::Int(1));
    let before =
        result(&a.query("knowledge.claim.history", r#"{"claim":"c"}"#, Some("g-a"))).clone();
    a.stop();

    // The owner transfers the scope to authority B: epoch 2.
    let mut owner = Provider::start(dir, "owner");
    let transferred = owner.command(
        "knowledge.authority.transfer",
        "transfer-svc",
        ("knowledge.authority", "svc"),
        1,
        r#"{"authority":"authority-b"}"#,
        None,
        None,
    );
    assert_eq!(
        at(result(&transferred), &["outcome", "epoch"]),
        &Value::Int(2)
    );
    owner.stop();

    // The stale path: A under the old epoch is stale; A under the new epoch
    // is no longer the authority.
    let mut a = Provider::start(dir, "authority-a");
    let stale = a.command(
        "knowledge.decision.record",
        "decide-a-stale",
        ("knowledge.decision", "a-stale"),
        0,
        &decision_payload(&reference, "rejected", Some("d1")),
        Some("g-a"),
        Some(1),
    );
    assert_eq!(error_code(&stale), "stale_authority_epoch");
    let late = a.command(
        "knowledge.decision.record",
        "decide-a-late",
        ("knowledge.decision", "a-late"),
        0,
        &decision_payload(&reference, "rejected", Some("d1")),
        Some("g-a"),
        Some(2),
    );
    assert_eq!(error_code(&late), "permission_denied");
    assert_eq!(
        at(&late, &["error", "data", "details", "reason"]).as_str(),
        Some("not_authority")
    );
    a.stop();

    let mut b = Provider::start(dir, "authority-b");
    // A transfer does not reset reliance: A's decision stays in effect.
    let inspected = b.query("knowledge.claim.inspect", r#"{"claim":"c"}"#, Some("g-b"));
    assert_eq!(
        at(result(&inspected), &["reliance", "state"]).as_str(),
        Some("accepted_for_use")
    );
    assert_eq!(
        at(result(&inspected), &["reliance", "decision"]).as_str(),
        Some("d1")
    );
    // B under the old epoch is stale too.
    let b_stale = b.command(
        "knowledge.decision.record",
        "decide-b-stale",
        ("knowledge.decision", "b-stale"),
        0,
        &decision_payload(&reference, "rejected", Some("d1")),
        Some("g-b"),
        Some(1),
    );
    assert_eq!(error_code(&b_stale), "stale_authority_epoch");
    // No record was edited and none was added by any refused command.
    let unchanged =
        result(&b.query("knowledge.claim.history", r#"{"claim":"c"}"#, Some("g-b"))).clone();
    assert_eq!(
        canonical(&unchanged),
        canonical(&before),
        "history is exactly what it was before the transfer and the refusals"
    );

    // The current authority records a later decision; supersession is a new
    // record linked to the old one, never an edit of it.
    let rejected = b.command(
        "knowledge.decision.record",
        "decide-d2",
        ("knowledge.decision", "d2"),
        0,
        &decision_payload(&reference, "rejected", Some("d1")),
        Some("g-b"),
        Some(2),
    );
    let outcome = at(result(&rejected), &["outcome"]);
    assert_eq!(at(outcome, &["epoch"]), &Value::Int(2));
    assert_eq!(at(outcome, &["author_is_decider"]), &Value::Bool(false));
    let after =
        result(&b.query("knowledge.claim.history", r#"{"claim":"c"}"#, Some("g-b"))).clone();
    let decisions = at(&after, &["decisions"]).as_array().expect("decisions");
    assert_eq!(decisions.len(), 2);
    assert_eq!(
        canonical(&decisions[0]),
        canonical(&at(&before, &["decisions"]).as_array().unwrap()[0]),
        "the first decision is unchanged, position included"
    );
    assert_eq!(
        at(&decisions[1], &["supersedes_decision"]).as_str(),
        Some("d1")
    );
    assert_eq!(at(&decisions[1], &["epoch"]), &Value::Int(2));
    let now = b.query("knowledge.claim.inspect", r#"{"claim":"c"}"#, Some("g-b"));
    assert_eq!(
        at(result(&now), &["reliance", "state"]).as_str(),
        Some("rejected")
    );
    b.stop();
}

fn seal(owner: &mut Provider, id: &str, source_kind: &str, bytes: &[u8]) -> String {
    let digest = cbr_encoding::digest_bytes(bytes);
    result(&owner.command(
        "evidence.upload.prepare",
        &format!("prepare-{id}"),
        ("evidence.artifact", id),
        0,
        &format!(
            r#"{{"digest":"{digest}","size":{},"media_type":"text/plain","producer":{{"producer_id":"knowledge-tests"}},"source":{{"kind":"{source_kind}","id":"{id}"}},"scope":"local","capture":{{"captured_at":"2030-01-01T00:00:00Z","anchors":[]}},"coverage":{{"completeness":"complete"}},"retention_class":"standard"}}"#,
            bytes.len()
        ),
        None,
        None,
    ));
    result(&owner.command(
        "evidence.upload.append",
        &format!("append-{id}"),
        ("evidence.artifact", id),
        1,
        &format!(
            r#"{{"offset":0,"data_base64":"{}"}}"#,
            cbr_encoding::encode_base64(bytes)
        ),
        None,
        None,
    ));
    result(&owner.command(
        "evidence.seal",
        &format!("seal-{id}"),
        ("evidence.artifact", id),
        2,
        "{}",
        None,
        None,
    ));
    digest
}

fn evidence_reference(id: &str, digest: &str) -> String {
    format!(
        r#"{{"provider":"conformance-provider","artifact":{{"kind":"evidence.artifact","id":"{id}"}},"digest":"{digest}"}}"#
    )
}

fn root(id: &str, digest: &str) -> String {
    format!(
        r#"{{"kind":"evidence","provider":"conformance-provider","artifact":{{"kind":"evidence.artifact","id":"{id}"}},"digest":"{digest}"}}"#
    )
}

/// PROTOCOL-PIN section 3. One captured log `C`; two derivations over it,
/// sealed as `D1` and `D2` under a CBR derivation source kind; a claim
/// supported by both, each declaring `C` as its complete root. Two model passes
/// over one log are not two sources: the class is `single_lineage`.
#[test]
fn two_derivations_over_one_captured_log_are_one_lineage() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut owner = Provider::start(directory.path(), "owner");
    let captured = seal(
        &mut owner,
        "captured-log",
        "test_report",
        b"41 passed, 1 failed\n",
    );
    let first = seal(
        &mut owner,
        "derivation-1",
        "cbr.derivation.transcript",
        b"the parser drops a BOM\n",
    );
    let second = seal(
        &mut owner,
        "derivation-2",
        "cbr.derivation.transcript",
        b"one test fails on a BOM input\n",
    );

    let support = |roots: [String; 2]| {
        format!(
            r#"[{{"support_id":"d1","evidence":{},"ancestry":{{"completeness":"complete","roots":[{}]}}}},{{"support_id":"d2","evidence":{},"ancestry":{{"completeness":"complete","roots":[{}]}}}}]"#,
            evidence_reference("derivation-1", &first),
            roots[0],
            evidence_reference("derivation-2", &second),
            roots[1]
        )
    };
    let propose = |owner: &mut Provider, id: &str, support: String| {
        result(&owner.command(
            "knowledge.claim.propose",
            &format!("propose-{id}"),
            ("knowledge.claim", id),
            0,
            &format!(
                r#"{{"plane":"interpretive","statement":{{"subject":{{"kind":"app.component","id":"parser"}},"predicate":"bom","value":"dropped","cardinality":"single"}},"scope":{{"id":"svc","qualifiers":{{}}}},"support":{support},"derivation":{{"kind":"model_assisted","inputs":[{}]}}}}"#,
                evidence_reference("captured-log", &captured)
            ),
            None,
            None,
        ));
    };

    // Honest ancestry: each derivation names the log it was drawn from.
    propose(
        &mut owner,
        "honest",
        support([
            root("captured-log", &captured),
            root("captured-log", &captured),
        ]),
    );
    let honest = owner.query("knowledge.claim.inspect", r#"{"claim":"honest"}"#, None);
    assert_eq!(
        at(result(&honest), &["support", "class"]).as_str(),
        Some("single_lineage"),
        "two derivations over one captured root are one lineage"
    );
    assert_eq!(
        at(result(&honest), &["availability", "state"]).as_str(),
        Some("complete")
    );

    // The mistake the control exists to catch, made by the producer: each
    // derivation declares itself as its own root, and the claim falsely reads
    // as independently corroborated.
    propose(
        &mut owner,
        "self-rooted",
        support([root("derivation-1", &first), root("derivation-2", &second)]),
    );
    let self_rooted = owner.query(
        "knowledge.claim.inspect",
        r#"{"claim":"self-rooted"}"#,
        None,
    );
    assert_eq!(
        at(result(&self_rooted), &["support", "class"]).as_str(),
        Some("multiple_lineages")
    );
    owner.stop();
}
