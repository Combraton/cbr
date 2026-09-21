//! **A derivation record is readable only under the job's own view and
//! readable claims.**
//!
//! A record names the paths and line ranges of repositories the job could
//! read. Handing one to a reader who could not read those is the M3 leak
//! arriving through a new door — that leak was discovery calling an
//! operation whose authorization belonged to a command that had not run,
//! and a new kind of artifact is exactly where the same mistake fits
//! next.
//!
//! Two statements, and the second is the one [READINESS §6] names as the
//! gate:
//!
//! * a reader authorised to read artifacts, but not the repositories a
//!   derivation is about, is refused it — and a reader who *can* read
//!   them is not, so the rule is about the view rather than about
//!   refusing everybody;
//! * **the m3c byte-identity assertion, applied to derivations**: the
//!   packet a reader outside the job's view receives is byte-identical
//!   to the one they would have received in a store where the model
//!   call never happened.
//!
//! The second is a gate rather than a bug report. Nothing today puts a
//! derivation into a packet, and the assertion is what will notice if
//! something later does.
//!
//! **No model is called.** The transport is the `model.fake` control.
//!
//! [READINESS §6]: ../../../docs/work/m4/READINESS.md

use cbr_encoding::Value;

mod serving;

use serving::{Fixture, derivations, prepared};

/// The artifact id and digest of the one derivation the store holds.
fn only_derivation(fixture: &Fixture) -> (String, String) {
    let found = derivations(&fixture.data());
    assert_eq!(found.len(), 1, "one derivation: {found:?}");
    let digest = found[0]
        .1
        .get("descriptor")
        .and_then(|descriptor| descriptor.get("digest"))
        .and_then(Value::as_str)
        .expect("a digest")
        .to_string();
    (found[0].0.clone(), digest)
}

/// Everything the reader needs to try to read artifacts: the right
/// itself, over every artifact, plus the repositories named.
fn grant_reading(fixture: &Fixture, id: &str, repositories: &[&str]) {
    let mut resources = vec![
        r#"{"kind":"evidence.artifact"}"#.to_string(),
        r#"{"kind":"context.request"}"#.to_string(),
        r#"{"kind":"context.job"}"#.to_string(),
        r#"{"kind":"context.packet"}"#.to_string(),
    ];
    for repository in repositories {
        resources.push(format!(
            r#"{{"kind":"cbr.repository","id":"{repository}"}}"#
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

#[test]
fn a_reader_who_cannot_read_the_repository_cannot_read_the_derivation_about_it() {
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    prepared(&fixture, "gated", "1");
    let (artifact, digest) = only_derivation(&fixture);

    // The reader holds `evidence.read` over every artifact, so step 6
    // lets this through and the readable set is the only thing left
    // standing between them and it. Their grant covers `outside`, which
    // the derivation is not about.
    grant_reading(&fixture, "g-narrow", &["outside"]);
    let out = fixture.directory.path().join("narrow.json");
    let refused = fixture.cbr_as_reader(
        "g-narrow",
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(!refused.status.success(), "it was served");
    let said = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(said.contains("permission_denied"), "refused with: {said}");
    assert!(!out.exists(), "and wrote nothing");
    provider.stop();
}

#[test]
fn a_reader_outside_the_view_cannot_inspect_it_either() {
    // **`fetch` is not the only door.** `inspect` returns the
    // descriptor, which says which job made the derivation, for which
    // request and of which model, so a gate on one and not the other
    // would be a gate with a second entrance.
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    prepared(&fixture, "inspected", "1");
    let (artifact, _) = only_derivation(&fixture);

    grant_reading(&fixture, "g-narrow", &["outside"]);
    let answered = fixture.query_as_reader(
        "g-narrow",
        "evidence.inspect",
        &format!(r#"{{"artifact":{{"kind":"evidence.artifact","id":"{artifact}"}}}}"#),
    );
    let code = answered
        .get("error")
        .and_then(|error| error.get("data"))
        .and_then(|data| data.get("code"))
        .and_then(Value::as_str);
    assert_eq!(
        code,
        Some("permission_denied"),
        "inspect answered: {answered:?}"
    );
    provider.stop();
}

#[test]
fn a_reader_outside_the_view_does_not_see_it_in_a_listing() {
    // The third door. A listing hands back the same descriptor
    // `inspect` would, so a derivation the reader may not inspect is
    // one they may not be shown.
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    prepared(&fixture, "listed", "1");
    let (artifact, _) = only_derivation(&fixture);

    grant_reading(&fixture, "g-narrow", &["outside"]);
    let answered = fixture.query_as_reader("g-narrow", "evidence.query", "{}");
    let listed: Vec<String> = answered
        .get("result")
        .and_then(|result| result.get("items"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no items in {answered:?}"))
        .iter()
        .filter_map(|item| item.get("artifact"))
        .filter_map(|subject| subject.get("id"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    assert!(
        !listed.contains(&artifact),
        "the listing named {artifact}: {listed:?}"
    );
    provider.stop();
}

#[test]
fn a_reader_who_can_read_the_repository_reads_the_derivation() {
    // The other arm, and the one that stops the rule above from being
    // "refuse everybody, which is trivially safe".
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    prepared(&fixture, "allowed", "1");
    let (artifact, digest) = only_derivation(&fixture);

    grant_reading(&fixture, "g-wide", &["app", "outside"]);
    let out = fixture.directory.path().join("wide.json");
    let served = fixture.cbr_as_reader(
        "g-wide",
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    assert!(
        served.status.success(),
        "refused a reader who could read it: {}",
        String::from_utf8_lossy(&served.stderr)
    );
    let bytes = std::fs::read(&out).expect("the fetched file");
    assert_eq!(cbr_encoding::digest_bytes(&bytes), digest);
    provider.stop();
}

/// The packet a reader outside the job's view gets, with and without a
/// model-assisted request having been prepared in the same store first.
///
/// Everything else is held equal: the same checkout, the same tree, the
/// same request, the same grant. The only difference between the two
/// runs is whether a derivation exists.
fn packet_for_reader(with_a_derivation: bool) -> Vec<u8> {
    let fixture = Fixture::answering(&["choose:c2"]);
    let provider = fixture.start();
    if with_a_derivation {
        prepared(&fixture, "theirs", "1");
        assert_eq!(derivations(&fixture.data()).len(), 1, "a derivation exists");
    } else {
        assert!(derivations(&fixture.data()).is_empty());
    }

    // The reader's view is `outside` and their request is about
    // `outside`. A derivation about `app` is no part of their answer,
    // and the assertion is that it is no part of their *bytes* either.
    grant_reading(&fixture, "g-reader", &["outside"]);
    let submitted = fixture.cbr_as_reader(
        "g-reader",
        &[
            "context",
            "mine",
            "--repo",
            fixture.outside.to_str().expect("utf-8"),
            "--repo-id",
            "outside",
            "--selector",
            "elsewhere queue",
            "--task",
            "what does the elsewhere repository say",
            "--capacity",
            "65536",
            "--want",
            "q=source:elsewhere.md",
        ],
    );
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let started = std::time::Instant::now();
    loop {
        let inspected = fixture.cbr_as_reader("g-reader", &["request", "mine"]);
        let parsed =
            cbr_encoding::parse(String::from_utf8_lossy(&inspected.stdout).trim().as_bytes())
                .expect("canonical JSON");
        if parsed.get("state").and_then(Value::as_str) != Some("preparing") {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "never left preparing: {parsed:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let printed = fixture.cbr_as_reader("g-reader", &["packet", "mine", "--excerpt", "1000000"]);
    assert!(
        printed.status.success(),
        "packet: {}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let packet = cbr_encoding::parse(String::from_utf8_lossy(&printed.stdout).trim().as_bytes())
        .expect("canonical JSON");
    let data = packet
        .get("excerpt")
        .and_then(|excerpt| excerpt.get("data_base64"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no excerpt in {packet:?}"));
    let bytes = cbr_encoding::decode_base64(data).expect("base64");
    provider.stop();
    bytes
}

#[test]
fn a_derivation_is_absent_from_another_readers_packet_byte_for_byte() {
    // The m3c assertion, applied to derivation records. Not that the
    // packet labels them carefully: that the packet is the packet they
    // would have received in a store where no model was ever called.
    let with = packet_for_reader(true);
    let without = packet_for_reader(false);
    assert_eq!(
        String::from_utf8_lossy(&with),
        String::from_utf8_lossy(&without),
        "a derivation outside the reader's view is a derivation that never happened"
    );
    let text = String::from_utf8_lossy(&with);
    assert!(!text.contains("der."), "and no derivation is named");
    assert!(
        !text.contains("MiniMax"),
        "and the model this store called is not"
    );
}
