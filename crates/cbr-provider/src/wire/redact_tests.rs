//! The gate for the redaction boundary.

use super::redact::*;

fn redacted(text: &str) -> String {
    String::from_utf8(redact(text.as_bytes()).bytes().to_vec()).expect("utf-8 survives")
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
    assert_eq!(redact(&bytes).bytes(), bytes, "a record is not only text");
}
