//! The gate for the redaction boundary.

use super::redact::*;

fn redacted(text: &str) -> String {
    String::from_utf8(redact(text.as_bytes(), None).bytes().to_vec()).expect("utf-8 survives")
}

/// A scrubber for a known credential, built the way the recording boundary
/// builds one: from the secret itself, exposing only "replace occurrences".
fn scrubber_for(credential: &str) -> crate::keychain::Scrubber {
    crate::keychain::Scrubber::over(credential)
}

#[test]
fn a_presigned_url_loses_its_credential_and_keeps_its_shape() {
    // **The case this exists for, and it is not hypothetical.** A provider
    // response can carry a third party's *live* credential: one of the
    // pilot repositories has already seen a speech response return a
    // presigned object-store URL with the key and signature in its query
    // string. Recorded unredacted, that is a working credential sitting in
    // CBR's store, belonging to somebody else.
    let response = "{\"audio\":\"https://oss-cn.example.com/o.mp3\
?OSSAccessKeyId=LTAI5tPLANTEDKEYCANARY&Expires=1789000000&Signature=PLANTEDSIGCANARY%3D\"}";
    let seen = redacted(response);
    assert!(!seen.contains("PLANTEDKEYCANARY"), "{seen}");
    assert!(!seen.contains("PLANTEDSIGCANARY"), "{seen}");
    // The URL is still recognisably a URL, and the part that says which
    // object it was survives: a record that redacts everything records
    // nothing.
    assert!(seen.contains("https://oss-cn.example.com/o.mp3"), "{seen}");
    assert!(seen.contains("Expires=1789000000"), "{seen}");
    assert!(seen.contains(REDACTED), "{seen}");
}

#[test]
fn a_query_parameter_is_judged_by_its_name_not_by_what_it_holds() {
    // A credential is not recognisable by looking at it — that is what
    // makes it a credential. The parameter's *name* is the signal, and the
    // set is deliberately generous, because over-redacting a query string
    // costs a record some detail and under-redacting it leaks a key.
    for name in [
        "token",
        "access_token",
        "api_key",
        "apikey",
        "sig",
        "signature",
        "password",
        "secret",
        "credential",
        "X-Amz-Signature",
        "OSSAccessKeyId",
    ] {
        let seen = redacted(&format!("https://h/p?a=1&{name}=PLANTEDCANARY&b=2"));
        assert!(!seen.contains("PLANTEDCANARY"), "{name}: {seen}");
        assert!(
            seen.contains("a=1") && seen.contains("b=2"),
            "{name}: {seen}"
        );
    }
}

#[test]
fn a_bearer_token_anywhere_in_the_bytes_is_redacted() {
    let seen = redacted("error: Authorization: Bearer sk-PLANTEDBEARERCANARY9999 rejected");
    assert!(!seen.contains("PLANTEDBEARERCANARY"), "{seen}");
    assert!(seen.contains("Bearer"), "the shape is kept: {seen}");
}

#[test]
fn a_member_that_names_a_secret_loses_its_value() {
    let seen = redacted(r#"{"model":"MiniMax-M2.7","api_key":"PLANTEDMEMBERCANARY","n":1}"#);
    assert!(!seen.contains("PLANTEDMEMBERCANARY"), "{seen}");
    assert!(seen.contains("\"model\":\"MiniMax-M2.7\""), "{seen}");
    assert!(seen.contains("\"n\":1"), "{seen}");
}

#[test]
fn well_known_credential_prefixes_are_redacted_wherever_they_appear() {
    for planted in [
        "sk-PLANTEDPREFIXCANARY0123456789",
        "ghp_PLANTEDPREFIXCANARY0123456789",
        "xoxb-PLANTEDPREFIXCANARY0123456789",
        "AKIAPLANTEDPREFIXCANARY0",
        "eyJhbGciOiJIUzI1NiJ9.PLANTEDPREFIXCANARY.aaaaaaaaaaaaaaaa",
    ] {
        let seen = redacted(&format!("the response said {planted} and stopped"));
        assert!(!seen.contains("PLANTEDPREFIXCANARY"), "{planted}: {seen}");
    }
}

#[test]
fn ordinary_text_comes_through_untouched() {
    // The negative control. A redactor that redacts everything passes every
    // test above and destroys every record, so this is the one that says
    // the rules are rules rather than a blanket.
    for ordinary in [
        r#"{"choices":[{"message":{"content":"Units are dropped in np.concatenate."}}]}"#,
        "https://github.com/Combraton/cbr/blob/main/README.md#gates",
        "see docs/work/m4/READINESS.md section 10 for the calibration",
        r#"{"usage":{"prompt_tokens":1180,"total_tokens":1222}}"#,
        "a key insight about the token bucket, keyed by path",
    ] {
        assert_eq!(redacted(ordinary), ordinary, "over-redacted");
    }
}

#[test]
fn bytes_that_are_not_text_survive_redaction() {
    let bytes = [0u8, 159, 146, 150, b'o', b'k'];
    assert_eq!(
        redact(&bytes, None).bytes(),
        bytes,
        "a record is not only text"
    );
}

// --- the four ways it leaked ---------------------------------------------

#[test]
fn a_json_escaped_url_does_not_hide_a_credential() {
    // **(a)** The URL arrives escaped, because it is inside a JSON string:
    // the slashes are `\/` and the ampersand is `\u0026`. Scanning the raw
    // bytes, neither the "is this a URL" test nor the parameter boundary
    // finds anything, and both values go to the store in the clear.
    //
    // The fix is structural: when the bytes are JSON, redaction runs over
    // the **decoded** string values and the document is re-serialized, so
    // an escape cannot hide anything from it.
    let body = "{\"audio\":\"https:\\/\\/oss.example\\/a.mp3\
?OSSAccessKeyId=ESCAPEDKEYCANARY\\u0026Signature=ESCAPEDSIGCANARY\"}";
    let seen = redacted(body);
    assert!(!seen.contains("ESCAPEDKEYCANARY"), "{seen}");
    assert!(!seen.contains("ESCAPEDSIGCANARY"), "{seen}");
    assert!(seen.contains("oss.example"), "still a record: {seen}");
    // And it is still JSON, with the member that carried it.
    let read = super::json::read(seen.as_bytes()).expect("still JSON");
    assert!(read.get("audio").is_some(), "{seen}");
}

#[test]
fn one_invalid_byte_does_not_turn_redaction_off_for_the_whole_record() {
    // **(b)** A single byte that is not UTF-8 made the old redactor return
    // the input untouched — so anything holding one leaked everything.
    //
    // A lossy decoding cannot be written back without corrupting the
    // record, so when a credential-shaped match is found in bytes that
    // cannot be mapped back safely, the record **fails closed**: the whole
    // thing is replaced by a marker and its length.
    let mut body = b"{\"url\":\"https://h/p?token=INVALIDBYTECANARY\",\"blob\":\"".to_vec();
    body.extend_from_slice(&[0xff, 0xfe, 0x00]);
    body.extend_from_slice(b"\"}");
    let out = redact(&body, None);
    let seen = String::from_utf8_lossy(out.bytes()).to_string();
    assert!(!seen.contains("INVALIDBYTECANARY"), "{seen}");
    assert!(seen.contains("redacted record"), "it failed closed: {seen}");
    assert!(
        seen.contains(&body.len().to_string()),
        "with its length: {seen}"
    );
}

#[test]
fn bytes_that_are_not_text_and_hold_no_credential_are_still_kept() {
    // The other half of failing closed: a record that is merely binary is
    // not destroyed. Only one that cannot be redacted safely is.
    let bytes = [0u8, 159, 146, 150, b'o', b'k'];
    assert_eq!(redact(&bytes, None).bytes(), bytes);
}

#[test]
fn the_exact_credential_is_scrubbed_even_in_a_shape_nothing_recognises() {
    // **(c)** A provider error that echoes the key back, in no shape any
    // prefix or parameter name matches. Nothing structural can catch this,
    // because the only thing that makes those bytes a credential is that
    // they are *this process's* credential.
    let held = "Zm9vYmFy-not-a-known-prefix-0123456789";
    let scrubber = scrubber_for(held);
    let body = format!("{{\"error\":\"unauthorised: {held} was rejected\"}}");
    let out = redact(body.as_bytes(), Some(&scrubber));
    let seen = String::from_utf8_lossy(out.bytes()).to_string();
    assert!(!seen.contains(held), "{seen}");
    assert!(seen.contains("unauthorised"), "still a record: {seen}");
}

#[test]
fn a_percent_encoded_nested_url_does_not_hide_a_credential() {
    // **(d)** The credential is in a URL that is itself a parameter of
    // another URL, percent-encoded. The outer parameter is called `next`,
    // which names no secret, so nothing looked inside it.
    let body = "{\"redirect\":\"https://h/go\
?next=https%3A%2F%2Foss.example%2Fa.mp3%3FSignature%3DNESTEDCANARY%26x%3D1\"}";
    let seen = redacted(body);
    assert!(!seen.contains("NESTEDCANARY"), "{seen}");
}

#[test]
fn the_scrubber_catches_the_credential_in_each_encoding_it_travels_in() {
    // Raw, base64 and percent-encoded: the three shapes a credential is
    // repeated in by something that logged, framed or forwarded it.
    let held = "sk-live-AAAA/BBBB+CCCC=DDDD";
    let scrubber = scrubber_for(held);
    let base64 = cbr_encoding::encode_base64(held.as_bytes());
    let percent = "sk-live-AAAA%2FBBBB%2BCCCC%3DDDDD";
    for shape in [held, base64.as_str(), percent] {
        let body = format!("{{\"note\":\"saw {shape} here\"}}");
        let out = redact(body.as_bytes(), Some(&scrubber));
        let seen = String::from_utf8_lossy(out.bytes()).to_string();
        assert!(!seen.contains(shape), "{shape} survived: {seen}");
    }
}

#[test]
fn a_planted_secret_never_survives_wherever_it_is_put() {
    // The property, over positions and encodings rather than over the four
    // cases above: a secret inserted anywhere in a record, in any of the
    // shapes it travels in, is not in the output.
    let held = "sk-PROPERTY-SECRET-0123456789abcdef";
    let scrubber = scrubber_for(held);
    let base64 = cbr_encoding::encode_base64(held.as_bytes());
    let percent = held.replace('-', "%2D");
    let shapes = [held, base64.as_str(), percent.as_str()];
    // A fixed generator, so a failure is reproducible rather than a story
    // about a run that once happened.
    let mut seed = 0x5eed_1234_u64;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) as usize
    };
    let carriers: [&str; 4] = [
        "{\"choices\":[{\"message\":{\"content\":\"PLACE\"}}],\"usage\":{\"total_tokens\":7}}",
        "{\"url\":\"https://oss.example/a?x=PLACE&y=2\"}",
        "not json at all, just a line with PLACE in it",
        "{\"nested\":{\"deep\":[\"a\",\"PLACE\",\"b\"]}}",
    ];
    for carrier in carriers {
        for shape in shapes {
            for _ in 0..16 {
                let filler: String = std::iter::repeat_n('x', next() % 7).collect();
                let at = next() % (shape.len() + 1);
                let planted = format!(
                    "{}{filler}{}",
                    &shape[..char_boundary(shape, at)],
                    &shape[char_boundary(shape, at)..]
                );
                let body = carrier.replace("PLACE", &format!("{planted}{shape}"));
                let out = redact(body.as_bytes(), Some(&scrubber));
                let seen = String::from_utf8_lossy(out.bytes()).to_string();
                assert!(
                    !seen.contains(shape),
                    "{shape} survived in {carrier}: {seen}"
                );
            }
        }
    }
}

fn char_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
