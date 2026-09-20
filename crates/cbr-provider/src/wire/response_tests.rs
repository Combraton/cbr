//! The gate for reading a provider's answer.
//!
//! **Every fixture below is hand-written and unverified against the live
//! service** — see `fixtures/README.md`. A passing test here says CBR reads
//! what CBR believes the provider sends. The calibration is what will say
//! whether that belief is true.

use cbr_encoding::Value;

use super::Dialect;
use super::request::Want;
use super::response::*;

macro_rules! unverified_fixture {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/", $name, ".unverified-fixture.json"))
    };
}

const UNVERIFIED_OPENAI_TEXT: &[u8] = unverified_fixture!("openai-text");
const UNVERIFIED_OPENAI_TOOL_CALL: &[u8] = unverified_fixture!("openai-tool-call");
const UNVERIFIED_OPENAI_THINK_LEAK: &[u8] = unverified_fixture!("openai-think-leak");
const UNVERIFIED_OPENAI_TRUNCATED: &[u8] = unverified_fixture!("openai-truncated");
const UNVERIFIED_OPENAI_RATE_LIMITED: &[u8] = unverified_fixture!("openai-rate-limited");
const UNVERIFIED_OPENAI_NO_BALANCE: &[u8] = unverified_fixture!("openai-insufficient-balance");
const UNVERIFIED_ANTHROPIC_TEXT: &[u8] = unverified_fixture!("anthropic-text");
const UNVERIFIED_ANTHROPIC_TOOL_USE: &[u8] = unverified_fixture!("anthropic-tool-use");
const UNVERIFIED_ANTHROPIC_THINK_LEAK: &[u8] = unverified_fixture!("anthropic-think-leak");
const UNVERIFIED_ANTHROPIC_ERROR: &[u8] = unverified_fixture!("anthropic-error");
const UNVERIFIED_COUNT: &[u8] = unverified_fixture!("count");

fn schema() -> Value {
    Value::Object(vec![("type".into(), Value::String("object".into()))])
}

fn tool() -> Want {
    Want::Tool {
        name: "choose_spans".into(),
        schema: schema(),
    }
}

// --- the ordinary answer -------------------------------------------------

#[test]
fn both_dialects_read_text_and_the_cost_the_provider_reported() {
    for (dialect, fixture) in [
        (Dialect::OpenAi, UNVERIFIED_OPENAI_TEXT),
        (Dialect::Anthropic, UNVERIFIED_ANTHROPIC_TEXT),
    ] {
        let read = read_completion(dialect, &Want::Text, fixture);
        assert_eq!(
            read.reply,
            Ok(Reply::Text(
                "Units are dropped where np.concatenate rebuilds the array.".into()
            )),
            "{}",
            dialect.name()
        );
        // The OpenAI wire reports a total; the Anthropic one reports the two
        // halves and CBR adds them. Both come to the same figure here, on
        // purpose, so that a parser reading the wrong member is visible.
        assert_eq!(read.usage, Some(1222), "{}", dialect.name());
    }
}

#[test]
fn both_dialects_read_a_tool_call_out_of_their_own_shape() {
    // The difference that matters: the OpenAI wire carries the arguments as
    // a **JSON string** that has to be read a second time, and the
    // Anthropic wire carries an object. A parser that forgets which is
    // which gets a `Reply::Tool` whose input is the text of a JSON
    // document rather than the document.
    let wanted = Value::Object(vec![(
        "ids".into(),
        Value::Array(vec![Value::String("s1".into()), Value::String("s4".into())]),
    )]);
    for (dialect, fixture) in [
        (Dialect::OpenAi, UNVERIFIED_OPENAI_TOOL_CALL),
        (Dialect::Anthropic, UNVERIFIED_ANTHROPIC_TOOL_USE),
    ] {
        let read = read_completion(dialect, &tool(), fixture);
        assert_eq!(
            read.reply,
            Ok(Reply::Tool {
                name: "choose_spans".into(),
                input: wanted.clone(),
            }),
            "{}",
            dialect.name()
        );
        assert_eq!(read.usage, Some(1204), "{}", dialect.name());
    }
}

// --- the think marker ----------------------------------------------------

#[test]
fn a_response_still_carrying_a_think_marker_is_refused_in_both_dialects() {
    // `reasoning_split` is set on every request, so this should not happen.
    // When it does, the reasoning is **not** stripped and the answer kept:
    // that would put CBR in the business of deciding which half of a
    // response was the answer. It is refused, and the refusal is recorded.
    for (dialect, fixture) in [
        (Dialect::OpenAi, UNVERIFIED_OPENAI_THINK_LEAK),
        (Dialect::Anthropic, UNVERIFIED_ANTHROPIC_THINK_LEAK),
    ] {
        let read = read_completion(dialect, &Want::Text, fixture);
        assert_eq!(
            read.reply,
            Err(Unusable::ReasoningLeaked),
            "{}",
            dialect.name()
        );
        // And the call still cost what it cost.
        assert_eq!(read.usage, Some(1276), "{}", dialect.name());
    }
}

#[test]
fn a_think_marker_is_refused_wherever_in_the_content_it_appears() {
    // Not only as a prefix: a marker in the middle is the same problem.
    let body = br#"{"type":"message","content":[{"type":"text","text":"answer <think>second thoughts</think>"}],"usage":{"input_tokens":1,"output_tokens":1}}"#;
    assert_eq!(
        read_completion(Dialect::Anthropic, &Want::Text, body).reply,
        Err(Unusable::ReasoningLeaked)
    );
    let closing_only = br#"{"type":"message","content":[{"type":"text","text":"tail</think> answer"}],"usage":{"input_tokens":1,"output_tokens":1}}"#;
    assert_eq!(
        read_completion(Dialect::Anthropic, &Want::Text, closing_only).reply,
        Err(Unusable::ReasoningLeaked)
    );
}

// --- the two ordinary outcomes the provider's own behaviour produces ------

#[test]
fn prose_where_a_structure_was_asked_for_is_an_outcome_and_is_repairable() {
    // `response_format` is silently ignored by this provider, so this is
    // not a transport error and not a surprise: it is the documented
    // behaviour, and the answer is a bounded repair rather than a retry.
    let read = read_completion(
        Dialect::OpenAi,
        &Want::Structure { schema: schema() },
        UNVERIFIED_OPENAI_TEXT,
    );
    assert_eq!(read.reply, Err(Unusable::NotStructured));
    assert!(Unusable::NotStructured.repairable());
    assert_eq!(
        Unusable::NotStructured.reason(),
        "model_output_unstructured"
    );
    assert_eq!(read.usage, Some(1222), "and it cost what it cost");
}

#[test]
fn text_where_a_tool_call_was_demanded_is_an_outcome_and_is_repairable() {
    // `tool_choice: "required"` is silently ignored while `"none"` is
    // honoured, so a loop that assumes a forced call misbehaves with no
    // error at all. This is what that looks like when it is noticed.
    for (dialect, fixture) in [
        (Dialect::OpenAi, UNVERIFIED_OPENAI_TEXT),
        (Dialect::Anthropic, UNVERIFIED_ANTHROPIC_TEXT),
    ] {
        let read = read_completion(dialect, &tool(), fixture);
        assert_eq!(read.reply, Err(Unusable::NoToolCall), "{}", dialect.name());
    }
    assert!(Unusable::NoToolCall.repairable());
    assert_eq!(Unusable::NoToolCall.reason(), "model_tool_call_missing");
}

#[test]
fn a_structured_answer_that_is_json_is_read_as_one() {
    let body = br#"{"choices":[{"message":{"role":"assistant","content":"{\"ids\":[\"s1\"]}"},"finish_reason":"stop"}],"usage":{"total_tokens":10}}"#;
    let read = read_completion(Dialect::OpenAi, &Want::Structure { schema: schema() }, body);
    assert_eq!(
        read.reply,
        Ok(Reply::Structure(Value::Object(vec![(
            "ids".into(),
            Value::Array(vec![Value::String("s1".into())])
        )])))
    );
}

#[test]
fn a_structured_answer_outside_the_protocols_domain_is_not_structured() {
    // It parses as JSON and is still not something CBR can seal, so it is
    // the same outcome as prose rather than a second kind of failure.
    let body = br#"{"choices":[{"message":{"content":"{\"score\":0.75}"},"finish_reason":"stop"}],"usage":{"total_tokens":10}}"#;
    let read = read_completion(Dialect::OpenAi, &Want::Structure { schema: schema() }, body);
    assert_eq!(read.reply, Err(Unusable::NotStructured));
}

// --- what is not repairable ----------------------------------------------

#[test]
fn a_truncated_answer_is_reported_rather_than_repaired() {
    // The generation limit was reached. Asking again under the same limit
    // produces the same truncation, so a repair would spend twice for one
    // outcome. It is reported with its own reason instead.
    let read = read_completion(
        Dialect::OpenAi,
        &Want::Structure { schema: schema() },
        UNVERIFIED_OPENAI_TRUNCATED,
    );
    assert_eq!(read.reply, Err(Unusable::Truncated));
    assert!(!Unusable::Truncated.repairable());
    assert_eq!(read.usage, Some(1196));
}

#[test]
fn a_provider_error_carried_in_a_200_is_still_an_error() {
    // This provider reports failures in the body with an HTTP 200, so a
    // transport that reads only the status code sees a successful call
    // that returned no content and calls it malformed.
    let read = read_completion(Dialect::OpenAi, &Want::Text, UNVERIFIED_OPENAI_RATE_LIMITED);
    assert_eq!(read.reply, Err(Unusable::ProviderExhausted));
    assert_eq!(
        read_completion(Dialect::OpenAi, &Want::Text, UNVERIFIED_OPENAI_NO_BALANCE).reply,
        Err(Unusable::ProviderExhausted)
    );
    // Any other non-zero code is an error rather than exhaustion: the
    // mapping above is a guess about two codes, and guessing wrong must
    // degrade to "something failed", never to "it worked".
    let other = br#"{"base_resp":{"status_code":1004,"status_msg":"authentication failed"}}"#;
    assert_eq!(
        read_completion(Dialect::OpenAi, &Want::Text, other).reply,
        Err(Unusable::ProviderError)
    );
}

#[test]
fn the_anthropic_dialects_error_shape_is_read_too() {
    let read = read_completion(Dialect::Anthropic, &Want::Text, UNVERIFIED_ANTHROPIC_ERROR);
    assert_eq!(read.reply, Err(Unusable::ProviderExhausted));
}

#[test]
fn a_provider_error_never_carries_the_providers_own_words() {
    // Its message is a stranger's text and would reach a log, an event and
    // an item's reason. The typed outcome carries none of it.
    let body = br#"{"base_resp":{"status_code":1004,"status_msg":"PROVIDER-CANARY"}}"#;
    let read = read_completion(Dialect::OpenAi, &Want::Text, body);
    let rendered = format!("{:?} {}", read, Unusable::ProviderError.reason());
    assert!(!rendered.contains("CANARY"), "{rendered}");
}

#[test]
fn bytes_that_are_not_this_dialects_shape_are_malformed() {
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        assert_eq!(
            read_completion(dialect, &Want::Text, b"not json").reply,
            Err(Unusable::Malformed),
            "{}",
            dialect.name()
        );
        assert_eq!(
            read_completion(dialect, &Want::Text, b"{}").reply,
            Err(Unusable::Malformed),
            "{}",
            dialect.name()
        );
    }
    assert!(!Unusable::Malformed.repairable());
    assert!(!Unusable::ProviderExhausted.repairable());
    assert!(!Unusable::ProviderError.repairable());
    assert!(!Unusable::ReasoningLeaked.repairable());
}

// --- the count -----------------------------------------------------------

#[test]
fn the_count_is_read_from_the_fixture_shape() {
    assert_eq!(read_count(UNVERIFIED_COUNT), Some(1180));
}

#[test]
fn a_count_body_naming_none_of_the_members_is_refused_rather_than_guessed() {
    // **The least verified thing in this module.** If the real shape is not
    // one of the names below, this returns nothing, the local estimate
    // stands, and the calibration is what finds out — which is the
    // conservative direction and the whole reason the local bound exists.
    assert_eq!(read_count(br#"{"n_tokens":1180}"#), None);
    assert_eq!(read_count(b"not json"), None);
    assert_eq!(read_count(br#"{"total_tokens":-3}"#), None);
}

#[test]
fn a_count_body_whose_members_disagree_is_refused() {
    // Two different figures under two accepted names is a body this parser
    // does not understand, and picking one of them is guessing.
    assert_eq!(
        read_count(br#"{"total_tokens":1180,"input_tokens":99}"#),
        None
    );
    assert_eq!(
        read_count(br#"{"total_tokens":1180,"input_tokens":1180}"#),
        Some(1180)
    );
}
