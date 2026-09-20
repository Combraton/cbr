//! The gate for the reader that meets a provider's bytes.

use super::json::*;

#[test]
fn a_non_integer_number_is_read_and_kept_exactly_as_written() {
    // **Why this reader exists at all.** A provider's response is not a
    // protocol value, and a `0.7` in one is ordinary. Refusing it would
    // turn a perfectly good response into an invalid-output outcome.
    let read = read(br#"{"temperature":0.7,"exp":1e3,"n":-0}"#).expect("reads");
    assert_eq!(read.get("temperature"), Some(&Json::Number("0.7".into())));
    assert_eq!(read.get("exp"), Some(&Json::Number("1e3".into())));
    assert_eq!(read.get("n"), Some(&Json::Number("-0".into())));
}

#[test]
fn the_protocols_own_reader_refuses_exactly_what_this_one_accepts() {
    // The two readers are not redundant, and this is the demonstration.
    // `cbr_encoding::parse` implements the protocol's value domain, which
    // has no non-integer numbers because two participants have to agree on
    // canonical bytes. It is right to refuse these, and wrong to be the
    // thing that meets a provider.
    let body = br#"{"temperature":0.7}"#;
    assert!(cbr_encoding::parse(body).is_err());
    assert!(read(body).is_some());
}

#[test]
fn what_is_recorded_goes_back_through_the_protocols_domain() {
    // A model's answer that is going to be sealed has to be a protocol
    // value, so the conversion is where a float stops being acceptable —
    // not at the wire, where it is merely a number CBR does not use.
    let answer = read(br#"{"ids":["a","b"],"n":3,"ok":true}"#).expect("reads");
    let recordable = answer.recordable().expect("inside the domain");
    assert_eq!(
        cbr_encoding::to_canonical(&recordable),
        br#"{"ids":["a","b"],"n":3,"ok":true}"#.to_vec()
    );
    let float = read(br#"{"score":0.75}"#).expect("reads");
    assert!(
        float.recordable().is_none(),
        "a non-integer number is not something CBR can seal"
    );
}

#[test]
fn nesting_beyond_the_bound_is_refused() {
    // Untrusted input, so the bound is the reader's rather than the
    // stack's. A recursive reader without one is a crash a stranger picks.
    let deep = format!(
        "{}1{}",
        "[".repeat(MAX_DEPTH + 2),
        "]".repeat(MAX_DEPTH + 2)
    );
    assert!(read(deep.as_bytes()).is_none());
    let shallow = format!("{}1{}", "[".repeat(8), "]".repeat(8));
    assert!(read(shallow.as_bytes()).is_some());
}

#[test]
fn an_oversized_body_is_refused_before_it_is_read() {
    let big = format!("\"{}\"", "x".repeat(MAX_BYTES));
    assert!(read(big.as_bytes()).is_none());
}

#[test]
fn trailing_bytes_are_refused() {
    // Two documents in one response is not a response. Reading the first
    // and ignoring the rest is how a parser is talked into disagreeing
    // with whatever else reads the same bytes.
    assert!(read(br#"{"a":1} {"b":2}"#).is_none());
    assert!(read(br#"{"a":1}x"#).is_none());
    assert!(
        read(br#"{"a":1}  "#).is_some(),
        "trailing space is not a byte that matters"
    );
}

#[test]
fn escapes_and_astral_characters_survive_the_reader() {
    let read = read(b"{\"t\":\"a\\\"b\\\\c\\nd\\u00e9\\ud83d\\ude00\"}").expect("reads");
    assert_eq!(
        read.get("t").and_then(Json::as_str),
        Some("a\"b\\c\nd\u{e9}\u{1f600}")
    );
}

#[test]
fn a_lone_surrogate_is_refused() {
    // It cannot become a Rust string, and silently substituting U+FFFD
    // would change bytes CBR may later have to seal.
    assert!(read(br#"{"t":"\ud83d"}"#).is_none());
}

#[test]
fn a_duplicate_member_is_refused() {
    // Which of the two a reader keeps is a choice, and any choice makes
    // this reader disagree with some other reader of the same bytes.
    assert!(read(br#"{"a":1,"a":2}"#).is_none());
}

#[test]
fn the_shapes_a_response_is_read_through_all_work() {
    let read =
        read(br#"{"choices":[{"index":0}],"n":42,"s":"t","b":false,"z":null}"#).expect("reads");
    assert_eq!(
        read.get("choices")
            .and_then(Json::as_array)
            .map(<[Json]>::len),
        Some(1)
    );
    assert_eq!(read.get("n").and_then(Json::as_u64), Some(42));
    assert_eq!(read.get("s").and_then(Json::as_str), Some("t"));
    assert_eq!(read.get("b"), Some(&Json::Bool(false)));
    assert_eq!(read.get("z"), Some(&Json::Null));
    assert_eq!(read.get("missing"), None);
    // A negative or fractional count is not a token count.
    assert_eq!(read_value("-1").as_u64(), None);
    assert_eq!(read_value("1.5").as_u64(), None);
}

fn read_value(text: &str) -> Json {
    read(format!("{{\"v\":{text}}}").as_bytes())
        .expect("reads")
        .get("v")
        .expect("v")
        .clone()
}
