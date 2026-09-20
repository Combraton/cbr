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

// --- an error member is an error, whatever the status says ---------------

const OBSERVED_COUNT_ERROR: &[u8] =
    include_bytes!("fixtures/count-invalid-request.observed-2026-09-20.json");

#[test]
fn the_error_shape_the_live_service_actually_returned_is_read_as_a_failure() {
    // **Observed on 2026-09-20**, in the calibration's one call. Neither
    // `base_resp.status_code` nor Anthropic's `{"type":"error"}` appeared;
    // it is OpenAI's shape, and `code` is a string.
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        let read = read_completion(dialect, &Want::Text, OBSERVED_COUNT_ERROR);
        assert_eq!(
            read.reply,
            Err(Unusable::ProviderError),
            "{}",
            dialect.name()
        );
    }
    assert_eq!(
        read_count(OBSERVED_COUNT_ERROR),
        None,
        "and it is not a count"
    );
}

#[test]
fn a_body_that_carries_an_error_member_is_a_failure_whatever_the_status_says() {
    // **The general rule.** This provider reported a failure inside an HTTP
    // 200 once already, and a parser that lets the status decide reads that
    // as a successful response with no content — the worst possible reading,
    // because everything downstream then treats emptiness as the answer.
    //
    // In **every** dialect: the shapes differ, the rule does not.
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        for body in [
            br#"{"error":{"message":"bad request","code":"invalid_prompt"}}"#.as_slice(),
            br#"{"error":{"message":"bad request","code":1004}}"#.as_slice(),
            br#"{"error":"flat string error"}"#.as_slice(),
        ] {
            let read = read_completion(dialect, &Want::Text, body);
            assert!(
                matches!(read.reply, Err(Unusable::ProviderError)),
                "{}: {} was read as {:?}",
                dialect.name(),
                String::from_utf8_lossy(body),
                read.reply
            );
        }
    }
}

#[test]
fn an_error_member_naming_an_exhausted_quota_is_still_exhaustion() {
    // The distinction the ledger depends on survives the new rule: a quota
    // that is gone is not the same outcome as a request that was wrong.
    for body in [
        br#"{"error":{"message":"x","code":"rate_limit_exceeded"}}"#.as_slice(),
        br#"{"error":{"message":"insufficient quota","code":"x"}}"#.as_slice(),
    ] {
        assert_eq!(
            read_completion(Dialect::OpenAi, &Want::Text, body).reply,
            Err(Unusable::ProviderExhausted),
            "{}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn an_error_members_own_words_still_never_reach_the_outcome() {
    let body = br#"{"error":{"message":"ERRORMEMBER-CANARY","code":"ERRCODE-CANARY"}}"#;
    let read = read_completion(Dialect::OpenAi, &Want::Text, body);
    assert!(!format!("{read:?}").contains("CANARY"), "{read:?}");
}

#[test]
fn a_null_error_member_is_not_an_error() {
    // The Responses API carries `"error": null` on every successful
    // response. Reading that as a failure would turn every good answer
    // into one, which is the opposite mistake and just as bad.
    let body = br#"{"error":null,"choices":[{"message":{"content":"hi"},
"finish_reason":"stop"}],"usage":{"total_tokens":3}}"#;
    let read = read_completion(Dialect::OpenAi, &Want::Text, body);
    assert_eq!(read.reply, Ok(Reply::Text("hi".into())), "{read:?}");
}

// --- the responses dialect -----------------------------------------------

macro_rules! documented_fixture {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/", $name, ".documented-2026-09-21.json"))
    };
}

macro_rules! observed_fixture {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/", $name, ".observed-2026-09-21.json"))
    };
}

/// Written from the published reference and **not yet seen** from the
/// service: calibration run 2 obtained neither a completed text answer nor
/// a tool call, because its one completion was spent on reasoning.
const DOCUMENTED_RESPONSES_TEXT: &[u8] = documented_fixture!("responses-text");
const DOCUMENTED_RESPONSES_TOOL_CALL: &[u8] = documented_fixture!("responses-tool-call");

/// **Seen from the live service on 2026-09-21**, in calibration run 2.
const OBSERVED_RESPONSES_INCOMPLETE: &[u8] =
    observed_fixture!("responses-incomplete-reasoning-only");
const OBSERVED_RESPONSES_COUNT: &[u8] = observed_fixture!("responses-input-tokens");

#[test]
fn the_responses_dialect_reads_text_and_its_usage() {
    let read = read_completion(Dialect::Responses, &Want::Text, DOCUMENTED_RESPONSES_TEXT);
    assert_eq!(
        read.reply,
        Ok(Reply::Text(
            "Units are dropped where np.concatenate rebuilds the array.".into()
        ))
    );
    // `total_tokens` is the whole charge, reasoning included.
    assert_eq!(read.usage, Some(1222));
}

#[test]
fn an_incomplete_answer_with_no_text_is_an_outcome_with_usage_not_a_failure() {
    // **Reasoning tokens are output tokens and cannot be disabled on the
    // M2.x models.** So a sixteen-token limit can be spent entirely on
    // reasoning and end with no text at all. That is the provider doing
    // what it documents, not a fault: it has a cost, it has a reason, and
    // reporting it as a transport failure would lose both.
    let read = read_completion(
        Dialect::Responses,
        &Want::Text,
        OBSERVED_RESPONSES_INCOMPLETE,
    );
    assert_eq!(read.reply, Err(Unusable::Truncated), "{read:?}");
    // The figures are the ones run 2 actually came back with: sixteen
    // output tokens, all of them reasoning, and no answer.
    assert_eq!(read.usage, Some(44), "and it cost what it cost");
    assert_eq!(read.input_usage, Some(28), "of which the input was 28");
    assert!(
        !Unusable::Truncated.repairable(),
        "asking again under the same limit gets the same answer"
    );
    assert_eq!(Unusable::Truncated.reason(), "model_answer_truncated");
}

#[test]
fn reasoning_is_a_separate_output_item_and_is_never_the_answer() {
    // It arrives as its own item rather than inside the content, which is
    // what `reasoning_split` was asking for on the chat dialects. Here it
    // is the documented shape, and the parser must not read it as text —
    // sealing a model's private reasoning as its output is a correctness
    // problem, not a cosmetic one.
    //
    // A **completed** response carrying both items, because the
    // incomplete fixture ends as `Truncated` whatever the parser does with
    // its reasoning, so it cannot tell the two apart. A mutant proved
    // that by surviving the first version of this test.
    let both = br#"{"object":"response","status":"completed","error":null,"output":[
{"type":"reasoning","content":[{"type":"reasoning_text","text":"REASONING-NOT-THE-ANSWER"}],
"summary":[]},
{"type":"message","role":"assistant","content":[{"type":"output_text","text":"the answer"}]}],
"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}"#;
    let read = read_completion(Dialect::Responses, &Want::Text, both);
    assert_eq!(read.reply, Ok(Reply::Text("the answer".into())), "{read:?}");
    assert!(
        !format!("{read:?}").contains("REASONING-NOT-THE-ANSWER"),
        "{read:?}"
    );

    // And when reasoning is the *only* output, there is no answer at all —
    // not the reasoning standing in for one.
    let only = br#"{"object":"response","status":"completed","error":null,"output":[
{"type":"reasoning","content":[{"type":"reasoning_text","text":"REASONING-NOT-THE-ANSWER"}],
"summary":[]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}"#;
    let read = read_completion(Dialect::Responses, &Want::Text, only);
    assert_eq!(read.reply, Err(Unusable::Malformed), "{read:?}");
    assert!(
        !format!("{read:?}").contains("REASONING-NOT-THE-ANSWER"),
        "{read:?}"
    );
}

#[test]
fn the_responses_dialect_reads_a_tool_call_from_its_own_output_item() {
    let read = read_completion(
        Dialect::Responses,
        &Want::Tool {
            name: "choose_spans".into(),
            schema: schema(),
        },
        DOCUMENTED_RESPONSES_TOOL_CALL,
    );
    assert_eq!(
        read.reply,
        Ok(Reply::Tool {
            name: "choose_spans".into(),
            input: Value::Object(vec![(
                "ids".into(),
                Value::Array(vec![Value::String("s1".into()), Value::String("s4".into())])
            )]),
        })
    );
    assert_eq!(read.usage, Some(1204));
}

#[test]
fn the_observed_count_response_is_read() {
    // `{"object": "response.input_tokens", "input_tokens": N}`, seen seven
    // times in calibration run 2 and carrying **no usage member**, which is
    // why what a count costs is still the provider's silence rather than a
    // figure. The member name was already in the set the parser accepts,
    // which is the one guess m4b made that turned out right.
    assert_eq!(read_count(OBSERVED_RESPONSES_COUNT), Some(1180));
}

#[test]
fn the_observed_completion_reports_no_reasoning_breakdown() {
    // **A documented member that the service does not send.** The
    // reference names `usage.output_tokens_details.reasoning_tokens`; run 2
    // received no `output_tokens_details` at all. So reasoning tokens are
    // inside `output_tokens` and CBR cannot tell how much of a completion
    // was reasoning — which matters, because on the M2.x models reasoning
    // cannot be turned off and can consume the whole limit.
    let read = super::json::read(OBSERVED_RESPONSES_INCOMPLETE).expect("reads");
    let usage = read.get("usage").expect("usage");
    assert!(usage.get("output_tokens").is_some());
    assert!(
        usage.get("output_tokens_details").is_none(),
        "the service sent a breakdown after all; the fixture is stale"
    );
    // What it does send: the total, and the cached-input breakdown.
    assert!(usage.get("total_tokens").is_some());
    assert!(usage.get("input_tokens_details").is_some());
}

#[test]
fn the_observed_response_echoes_the_request_back() {
    // Thirty-seven members where the reference describes twelve, most of
    // them the request returned. Nothing downstream may assume a response
    // holds only what was documented — a parser that walked every member
    // would be walking CBR's own request.
    let read = super::json::read(OBSERVED_RESPONSES_INCOMPLETE).expect("reads");
    for echoed in [
        "instructions",
        "max_output_tokens",
        "temperature",
        "truncation",
    ] {
        assert!(read.get(echoed).is_some(), "{echoed} was not echoed");
    }
    // And `service_tier` comes back null though `standard` was sent, so
    // whether it was honoured is not observable from the response.
    assert_eq!(read.get("service_tier"), Some(&super::json::Json::Null));
    // `store` is false, which is what the owner's decision requires and
    // what CBR cannot ask for: there is no request parameter.
    assert_eq!(read.get("store"), Some(&super::json::Json::Bool(false)));
}

#[test]
fn a_failed_status_is_a_failure_even_with_no_error_member() {
    let body = br#"{"object":"response","status":"failed","output":[],
"usage":{"input_tokens":5,"output_tokens":0,"total_tokens":5}}"#;
    let read = read_completion(Dialect::Responses, &Want::Text, body);
    assert_eq!(read.reply, Err(Unusable::ProviderError), "{read:?}");
    assert_eq!(read.usage, Some(5), "and it still cost something");
}
