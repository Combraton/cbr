//! The pinned encoding vectors, run against this implementation.
//!
//! These are Protocol's own vectors, vendored at the pin. Every case is
//! exercised: a rejected case must be rejected *for the reason the vector
//! names*, not merely rejected, because a parser that refuses everything would
//! otherwise pass.

use std::path::PathBuf;

use cbr_encoding::{
    ErrorKind, Value, command_digest, command_intent, digest_bytes, parse, to_canonical,
};

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/protocol/v0.1.0/conformance/vectors/encoding.json")
}

fn load() -> Value {
    let bytes = std::fs::read(vectors_path()).expect("vendored encoding vectors are readable");
    parse(&bytes).expect("the vectors file is itself inside the domain")
}

fn unhex(text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2),
        "hex string has an even length"
    );
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}

fn field<'a>(case: &'a Value, name: &str) -> &'a str {
    case.get(name)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("vector case has a string `{name}`"))
}

#[test]
fn vectors_file_is_the_pinned_one() {
    let vectors = load();
    assert_eq!(field(&vectors, "format"), "combraton-encoding-vectors/1");
    assert_eq!(field(&vectors, "spec"), "docs/spec/bindings/ENCODING.md");
}

#[test]
fn canonical_cases_round_trip_to_exact_bytes_and_digests() {
    let vectors = load();
    let cases = vectors
        .get("canonical")
        .and_then(Value::as_array)
        .expect("canonical cases");
    assert_eq!(
        cases.len(),
        12,
        "the pinned vector count changed; re-verify the pin"
    );

    for case in cases {
        let id = field(case, "id");
        let input = unhex(field(case, "input_hex"));
        let expected = unhex(field(case, "canonical_hex"));

        let value = parse(&input).unwrap_or_else(|e| panic!("{id}: should parse, got {e}"));
        let produced = to_canonical(&value);
        assert_eq!(
            produced,
            expected,
            "{id}: canonical bytes differ\n produced {}\n expected {}",
            String::from_utf8_lossy(&produced),
            String::from_utf8_lossy(&expected)
        );
        assert_eq!(
            digest_bytes(&produced),
            field(case, "digest"),
            "{id}: digest differs"
        );

        // Canonical form is a fixed point: re-reading it must produce the same
        // bytes again, or the encoder and the parser disagree about the domain.
        let reparsed = parse(&produced)
            .unwrap_or_else(|e| panic!("{id}: canonical form should parse, got {e}"));
        assert_eq!(
            to_canonical(&reparsed),
            expected,
            "{id}: canonical form is not a fixed point"
        );
    }
}

/// The vector's stated reason, mapped to the reasons this implementation may
/// give for it. Kept explicit so a case that starts failing for a different
/// reason is a test failure rather than a silent pass.
fn acceptable_reasons(vector_reason: &str) -> &'static [ErrorKind] {
    match vector_reason {
        "duplicate member name" => &[ErrorKind::DuplicateMember],
        "non-integer number" => &[ErrorKind::NonIntegerNumber],
        "negative zero" => &[ErrorKind::NegativeZero],
        "integer outside safe range" => &[ErrorKind::IntegerOutOfRange],
        "unpaired surrogate" => &[ErrorKind::UnpairedSurrogate],
        "noncharacter" => &[ErrorKind::Noncharacter],
        "invalid UTF-8" => &[ErrorKind::InvalidUtf8],
        "byte-order mark" => &[ErrorKind::ByteOrderMark],
        // The vectors use one label for two structurally different failures:
        // an unknown token, and input continuing past the top-level value.
        "not JSON" => &[ErrorKind::NotJson, ErrorKind::TrailingGarbage],
        other => panic!("unmapped vector reason `{other}`; add it deliberately"),
    }
}

#[test]
fn rejected_cases_are_refused_for_the_stated_reason() {
    let vectors = load();
    let cases = vectors
        .get("rejected")
        .and_then(Value::as_array)
        .expect("rejected cases");
    assert_eq!(
        cases.len(),
        18,
        "the pinned vector count changed; re-verify the pin"
    );

    for case in cases {
        let id = field(case, "id");
        let input = unhex(field(case, "input_hex"));
        let vector_reason = field(case, "reason");

        let error = match parse(&input) {
            Ok(value) => {
                panic!("{id}: should have been rejected ({vector_reason}), parsed as {value:?}")
            }
            Err(error) => error,
        };
        let allowed = acceptable_reasons(vector_reason);
        assert!(
            allowed.contains(&error.kind),
            "{id}: rejected for the wrong reason. vector says `{vector_reason}`, got `{}`",
            error.kind.reason()
        );
    }
}

#[test]
fn command_intent_matches_the_pinned_vector() {
    let vectors = load();
    let cases = vectors
        .get("intent")
        .and_then(Value::as_array)
        .expect("intent cases");
    assert_eq!(
        cases.len(),
        1,
        "the pinned vector count changed; re-verify the pin"
    );

    for case in cases {
        let id = field(case, "id");
        let envelope = case.get("envelope").expect("intent case has an envelope");
        let intent = command_intent(envelope).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let produced = to_canonical(&intent);
        let expected = unhex(field(case, "intent_canonical_hex"));
        assert_eq!(
            produced,
            expected,
            "{id}: intent canonical bytes differ\n produced {}\n expected {}",
            String::from_utf8_lossy(&produced),
            String::from_utf8_lossy(&expected)
        );
        assert_eq!(
            command_digest(envelope).unwrap(),
            field(case, "command_digest"),
            "{id}: command digest differs"
        );

        // The vector's envelope carries an extension that `requires` does not
        // name. If it leaked into the intent the bytes above would differ, but
        // assert the property directly so the reason a future failure appears
        // is visible rather than inferred from a hex mismatch.
        assert!(
            intent
                .get("extensions")
                .unwrap()
                .get("example.org/note")
                .is_none(),
            "{id}: an undeclared extension must not enter the intent"
        );
        assert!(
            intent
                .get("extensions")
                .unwrap()
                .get("example.org/audit")
                .is_some(),
            "{id}: a declared extension must enter the intent"
        );
    }
}
