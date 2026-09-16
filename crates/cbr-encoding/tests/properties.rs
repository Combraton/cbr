//! Properties the pinned vectors do not pin, and traps they do not name.
//!
//! The vectors cover the cases Protocol chose to freeze. These cover the ways
//! this particular implementation could be wrong while still passing them.

use cbr_encoding::{
    Algorithm, DigestError, ErrorKind, IntentError, MAX_SAFE_INTEGER, Value, command_intent,
    digest_bytes, digest_canonical, parse, parse_digest, parse_with_depth, to_canonical,
};

/// RFC 8785 orders member names by UTF-16 code units, which is *not* code-point
/// order. A supplementary character encodes as a surrogate pair starting at
/// U+D800, so it sorts below U+E000..U+FFFF even though its code point is
/// larger. Sorting Rust `str`s directly gets this pair backwards, and nothing
/// else in the suite would notice.
#[test]
fn member_order_is_utf16_not_code_point() {
    // U+FF3A is the single unit FF3A. U+10000 is the pair D800 DC00.
    // UTF-16 order: D800 < FF3A, so U+10000 comes first.
    // Code-point order: 0xFF3A < 0x10000, so U+FF3A would come first.
    //
    // The escapes are built rather than written literally, so the source cannot
    // be flattened into raw characters by a tool that rewrites escapes. Both
    // spellings are tested, because a parser could handle one and not the other.
    let escaped = format!("{{\"{E}ff3a\":1,\"{E}d800{E}dc00\":2}}", E = "\\u");
    let raw = "{\"\u{FF3A}\":1,\"\u{10000}\":2}";

    for input in [escaped.as_str(), raw] {
        let value = parse(input.as_bytes()).expect("both keys are inside the domain");
        let canonical = String::from_utf8(to_canonical(&value)).unwrap();
        let astral_at = canonical.find('\u{10000}').expect("astral key present");
        let bmp_at = canonical.find('\u{FF3A}').expect("bmp key present");
        assert!(
            astral_at < bmp_at,
            "U+10000 must sort before U+FF3A under UTF-16 ordering, got {canonical}"
        );
    }
}

#[test]
fn integer_bounds_are_inclusive_on_both_sides() {
    for value in [MAX_SAFE_INTEGER, -MAX_SAFE_INTEGER] {
        let text = format!(r#"{{"n":{value}}}"#);
        assert!(
            parse(text.as_bytes()).is_ok(),
            "{value} is inside the domain"
        );
    }
    for value in [
        MAX_SAFE_INTEGER as i128 + 1,
        -(MAX_SAFE_INTEGER as i128) - 1,
    ] {
        let text = format!(r#"{{"n":{value}}}"#);
        let error = parse(text.as_bytes()).expect_err("just outside the domain");
        assert_eq!(error.kind, ErrorKind::IntegerOutOfRange, "for {value}");
    }
}

#[test]
fn plain_zero_is_fine_but_negative_zero_is_not() {
    assert!(parse(br#"{"n":0}"#).is_ok());
    assert_eq!(
        parse(br#"{"n":-0}"#).unwrap_err().kind,
        ErrorKind::NegativeZero
    );
}

#[test]
fn leading_zeros_are_refused_as_such() {
    assert_eq!(
        parse(br#"{"n":01}"#).unwrap_err().kind,
        ErrorKind::LeadingZero
    );
}

#[test]
fn unescaped_control_characters_in_strings_are_refused() {
    // A raw line feed inside a string is invalid JSON, and is also the byte the
    // stream binding uses as its frame terminator.
    let raw = [
        b'{', b'"', b's', b'"', b':', b'"', b'a', 0x0A, b'b', b'"', b'}',
    ];
    let error = parse(&raw).unwrap_err();
    assert_eq!(error.kind, ErrorKind::ControlCharacterInString);
}

#[test]
fn delete_is_emitted_raw_but_other_controls_are_escaped() {
    // JSON requires escaping below U+0020 only, so U+007F is emitted raw while
    // U+0001 becomes a lowercase hex escape. Escapes are built rather than
    // written literally, for the reason given above.
    let input = format!("{{\"s\":\"{E}007f{E}0001\"}}", E = "\\u");
    let value = parse(input.as_bytes()).expect("both are inside the domain");
    let canonical = to_canonical(&value);

    let mut expected = Vec::new();
    expected.extend_from_slice(b"{\"s\":\"");
    expected.push(0x7F);
    expected.extend_from_slice(b"\\u0001\"}");
    assert_eq!(canonical, expected);
}

#[test]
fn no_unicode_normalisation_is_applied() {
    // Precomposed and decomposed forms are different values, and must stay so.
    let composed = parse("{\"s\":\"\u{00e9}\"}".as_bytes()).unwrap();
    let decomposed = parse("{\"s\":\"e\u{0301}\"}".as_bytes()).unwrap();
    assert_ne!(to_canonical(&composed), to_canonical(&decomposed));
    assert_ne!(digest_canonical(&composed), digest_canonical(&decomposed));
}

#[test]
fn content_digests_are_over_bytes_not_over_meaning() {
    // The same value serialised differently is different content. This is the
    // distinction between a content digest and a record digest; getting it
    // wrong would let a re-serialised packet pass as the sealed bytes.
    let spaced = br#"{ "a" : 1 }"#;
    let tight = br#"{"a":1}"#;
    assert_ne!(digest_bytes(spaced), digest_bytes(tight));

    let from_spaced = parse(spaced).unwrap();
    let from_tight = parse(tight).unwrap();
    assert_eq!(
        digest_canonical(&from_spaced),
        digest_canonical(&from_tight)
    );
}

#[test]
fn nesting_beyond_the_limit_is_refused() {
    let deep = format!("{}{}", "[".repeat(40), "]".repeat(40));
    assert!(parse_with_depth(deep.as_bytes(), 64).is_ok());
    let error = parse_with_depth(deep.as_bytes(), 8).unwrap_err();
    assert_eq!(error.kind, ErrorKind::DepthExceeded);
}

#[test]
fn digest_strings_are_validated_against_the_grammar() {
    let sha256 = format!("sha256:{}", "a".repeat(64));
    assert_eq!(parse_digest(&sha256).unwrap().0, Algorithm::Sha256);

    let sha512 = format!("sha512:{}", "0".repeat(128));
    assert_eq!(parse_digest(&sha512).unwrap().0, Algorithm::Sha512);

    // Right algorithm, wrong encoded length.
    assert_eq!(
        parse_digest(&format!("sha256:{}", "a".repeat(63))).unwrap_err(),
        DigestError::WrongLength
    );
    // The grammar is lowercase hex only.
    assert_eq!(
        parse_digest(&format!("sha256:{}", "A".repeat(64))).unwrap_err(),
        DigestError::Malformed
    );
    // Weak algorithms are never supported, and must be reported as unsupported
    // rather than quietly lumped in with malformed input.
    assert_eq!(
        parse_digest(&format!("md5:{}", "a".repeat(32))).unwrap_err(),
        DigestError::UnsupportedAlgorithm("md5".into())
    );
    assert_eq!(
        parse_digest(&format!("sha1:{}", "a".repeat(40))).unwrap_err(),
        DigestError::UnsupportedAlgorithm("sha1".into())
    );
    assert_eq!(parse_digest("sha256").unwrap_err(), DigestError::Malformed);
    assert_eq!(parse_digest(":abc").unwrap_err(), DigestError::Malformed);
}

#[test]
fn a_requires_entry_that_is_a_feature_contributes_no_extension() {
    // `requires` carries feature names as well as extension keys. A feature
    // name must not become an empty extension member.
    let envelope = parse(
        br#"{"operation":"core.grant.issue","message_id":"m","command_id":"c",
             "dedupe_generation":1,"subject":{"kind":"core.grant","id":"g"},
             "preconditions":[],"requires":["core.grants"],"payload":{}}"#,
    )
    .unwrap();
    let intent = command_intent(&envelope).unwrap();
    assert_eq!(intent.get("extensions"), Some(&Value::Object(vec![])));
}

/// CORE section 5.1: "An extension key always contains exactly one slash",
/// a feature name "never contains a slash", and "That is how a `requires`
/// entry is told apart." An entry containing a slash MUST also be present in
/// `extensions`; a violation is `invalid_envelope`.
///
/// Pinned fixture `core.envelope.required-extension-must-be-present` sends
/// exactly this envelope and requires `invalid_envelope` with no state change,
/// so tolerating it here would produce a digest for a command that must never
/// have been accepted.
#[test]
fn a_requires_entry_with_a_slash_must_be_present_in_extensions() {
    // The fixture's envelope: `requires` names an extension, `extensions` is absent.
    let absent = parse(
        br#"{"operation":"core-test.subject.put","message_id":"m","command_id":"cmd-1",
             "dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},
             "preconditions":[],"requires":["example.org/audit"],"payload":{"value":"v"}}"#,
    )
    .unwrap();
    assert_eq!(
        command_intent(&absent).unwrap_err(),
        IntentError::RequiredExtensionMissing("example.org/audit".into())
    );

    // Present but under a different key is still absent.
    let wrong_key = parse(
        br#"{"operation":"core-test.subject.put","message_id":"m","command_id":"cmd-1",
             "dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},
             "preconditions":[],"requires":["example.org/audit"],
             "extensions":{"example.org/other":{"level":2}},"payload":{"value":"v"}}"#,
    )
    .unwrap();
    assert_eq!(
        command_intent(&wrong_key).unwrap_err(),
        IntentError::RequiredExtensionMissing("example.org/audit".into())
    );

    // A feature name contains no slash and needs no extension member.
    let feature = parse(
        br#"{"operation":"core-test.subject.put","message_id":"m","command_id":"cmd-1",
             "dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},
             "preconditions":[],"requires":["core.digest-sha512"],"payload":{"value":"v"}}"#,
    )
    .unwrap();
    assert!(
        command_intent(&feature).is_ok(),
        "a feature name is not an extension key"
    );
}

/// CORE section 5.1 puts uniqueness in the same envelope-semantics step, and
/// pinned fixture `core.envelope.requires-unique` requires `invalid_envelope`.
#[test]
fn duplicate_requires_entries_are_refused() {
    let duplicated = parse(
        br#"{"operation":"core-test.subject.put","message_id":"m","command_id":"cmd-1",
             "dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},
             "preconditions":[],"requires":["example.org/x","example.org/x"],
             "extensions":{"example.org/x":{"n":1}},"payload":{"value":"v"}}"#,
    )
    .unwrap();
    assert_eq!(
        command_intent(&duplicated).unwrap_err(),
        IntentError::DuplicateRequires("example.org/x".into())
    );

    // Duplicate feature names are equally invalid; the rule is about the list,
    // not about what the entries name.
    let features = parse(
        br#"{"operation":"core-test.subject.put","message_id":"m","command_id":"cmd-1",
             "dedupe_generation":1,"subject":{"kind":"core-test.subject","id":"s-1"},
             "preconditions":[],"requires":["core.grants","core.grants"],"payload":{"value":"v"}}"#,
    )
    .unwrap();
    assert_eq!(
        command_intent(&features).unwrap_err(),
        IntentError::DuplicateRequires("core.grants".into())
    );
}

#[test]
fn intent_excludes_the_members_that_may_differ_between_transmissions() {
    let base = |message: &str, generation: i64, epoch: i64| {
        format!(
            r#"{{"operation":"core-test.subject.put","message_id":"{message}","command_id":"c-1",
                 "dedupe_generation":{generation},"subject":{{"kind":"core-test.subject","id":"s"}},
                 "preconditions":[],"authority_epoch":{epoch},"correlation":{{"trace":"{message}"}},
                 "requires":[],"payload":{{"value":1}}}}"#
        )
    };
    let first = parse(base("m-1", 1, 0).as_bytes()).unwrap();
    let retransmission = parse(base("m-2", 7, 3).as_bytes()).unwrap();

    // A retransmission under a new message id, generation and authority epoch
    // is the same command. If any of those entered the intent, a caller that
    // reconnected under a new epoch could not safely retransmit.
    assert_eq!(
        to_canonical(&command_intent(&first).unwrap()),
        to_canonical(&command_intent(&retransmission).unwrap())
    );
}

#[test]
fn a_changed_payload_is_a_different_command() {
    let envelope = |value: i64| {
        parse(
            format!(
                r#"{{"operation":"core-test.subject.put","message_id":"m","command_id":"c-1",
                     "dedupe_generation":1,"subject":{{"kind":"core-test.subject","id":"s"}},
                     "preconditions":[],"requires":[],"payload":{{"value":{value}}}}}"#
            )
            .as_bytes(),
        )
        .unwrap()
    };
    assert_ne!(
        to_canonical(&command_intent(&envelope(1)).unwrap()),
        to_canonical(&command_intent(&envelope(2)).unwrap())
    );
}
