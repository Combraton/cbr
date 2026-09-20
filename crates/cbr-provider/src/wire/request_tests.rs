//! The gate for the request side of both dialects.
//!
//! **The fixtures these tests assert against are hand-written from the
//! provider's public documentation and are unverified against the live
//! service** until the calibration ([READINESS §10]). A test here proves
//! CBR frames what it meant to frame; it does not prove the provider agrees.
//!
//! [READINESS §10]: ../../../docs/work/m4/READINESS.md

use cbr_encoding::Value;

use super::Dialect;
use super::request::*;

const BOTH: [Dialect; 2] = [Dialect::OpenAi, Dialect::Anthropic];

fn schema() -> Value {
    Value::Object(vec![
        ("type".into(), Value::String("object".into())),
        (
            "properties".into(),
            Value::Object(vec![(
                "ids".into(),
                Value::Object(vec![("type".into(), Value::String("array".into()))]),
            )]),
        ),
    ])
}

fn request(want: Want) -> Request {
    Request {
        model: "MiniMax-M2.7".into(),
        system: Some("Choose only among the ids offered.".into()),
        messages: vec![Message {
            role: Role::User,
            text: "which spans".into(),
        }],
        generation: 16,
        want,
    }
}

fn parsed(dialect: Dialect, want: Want) -> Value {
    let bytes = request(want).serialize(dialect);
    cbr_encoding::parse(&bytes)
        .expect("CBR's own request body is inside the protocol's JSON domain")
}

// --- the generation limit ----------------------------------------------

#[test]
fn the_generation_limit_is_bound_to_the_field_each_dialect_reads() {
    // **This replaces m4a's placeholder.** That check asked whether the
    // number appeared anywhere in the body, which a body that mentions 16
    // in a message and caps generation at 4096 satisfies while spending
    // 4096. The limit has to be *bound to the field the provider reads*,
    // and the only way to know that is to parse the body and look.
    for dialect in BOTH {
        let body = parsed(dialect, Want::Text);
        assert_eq!(
            body.get(dialect.generation_field()),
            Some(&Value::Int(16)),
            "{}: the limit is not bound to {}",
            dialect.name(),
            dialect.generation_field()
        );
    }
}

#[test]
fn a_limit_that_is_merely_mentioned_in_the_body_is_not_declared() {
    // The exact defect the textual check could not see: the field says
    // 4096 and the number 16 appears in prose. m4a's check passed this.
    for dialect in BOTH {
        let lying = br#"{"max_tokens":4096,"messages":[{"content":"about 16 spans"}]}"#;
        assert!(
            !declares_generation(dialect, lying, 16),
            "{}: a body capped at 4096 declared a limit of 16",
            dialect.name()
        );
        assert!(
            declares_generation(dialect, lying, 4096),
            "{}: the limit it does declare is 4096",
            dialect.name()
        );
    }
}

#[test]
fn a_body_that_is_not_json_declares_nothing() {
    for dialect in BOTH {
        assert!(!declares_generation(dialect, b"max_tokens=16", 16));
        assert!(!declares_generation(dialect, b"", 16));
        assert!(!declares_generation(dialect, b"{}", 16));
    }
}

#[test]
fn what_each_dialect_serializes_declares_its_own_limit() {
    // The two halves joined: the serializer's output satisfies the check
    // that guards the send. If this ever fails, one of them has moved.
    for dialect in BOTH {
        let body = request(Want::Text).serialize(dialect);
        assert!(
            declares_generation(dialect, &body, 16),
            "{}: its own body does not pass its own check",
            dialect.name()
        );
    }
}

#[test]
fn both_dialects_name_the_same_field_today_and_are_asked_separately_anyway() {
    // Stated rather than hidden: MiniMax's OpenAI-compatible surface and
    // its Anthropic-compatible one both read `max_tokens`. The check asks
    // the dialect regardless, so that the day one of them moves — to
    // `max_completion_tokens`, say — one constant changes and the guard
    // follows it.
    assert_eq!(Dialect::OpenAi.generation_field(), "max_tokens");
    assert_eq!(Dialect::Anthropic.generation_field(), "max_tokens");
}

// --- the four provider behaviours, in the request -----------------------

#[test]
fn the_reasoning_split_is_set_in_both_dialects() {
    // MiniMax-M3 embeds `<think>...</think>` in the content unless this is
    // set. Reasoning text in a sealed derivation record, presented as
    // output, is a correctness problem rather than a cosmetic one.
    for dialect in BOTH {
        assert_eq!(
            parsed(dialect, Want::Text).get("reasoning_split"),
            Some(&Value::Bool(true)),
            "{}",
            dialect.name()
        );
    }
}

#[test]
fn neither_dialect_streams() {
    // There is no `data: [DONE]` sentinel, so a streaming reader has to
    // infer the end of a message. M4 asks for whole responses instead.
    for dialect in BOTH {
        assert_eq!(
            parsed(dialect, Want::Text).get("stream"),
            Some(&Value::Bool(false)),
            "{}",
            dialect.name()
        );
    }
}

// --- where each dialect keeps things ------------------------------------

#[test]
fn the_system_instruction_goes_where_each_dialect_reads_it() {
    // The one structural difference that silently degrades rather than
    // erroring: an Anthropic-shaped body with a system *message* in the
    // array is accepted and the instruction is treated as conversation.
    let openai = parsed(Dialect::OpenAi, Want::Text);
    let messages = openai
        .get("messages")
        .and_then(Value::as_array)
        .expect("messages");
    assert_eq!(
        messages[0].get("role").and_then(Value::as_str),
        Some("system"),
        "the OpenAI dialect carries it as the first message"
    );
    assert!(openai.get("system").is_none());

    let anthropic = parsed(Dialect::Anthropic, Want::Text);
    assert_eq!(
        anthropic.get("system").and_then(Value::as_str),
        Some("Choose only among the ids offered."),
        "the Anthropic dialect carries it at the top level"
    );
    let messages = anthropic
        .get("messages")
        .and_then(Value::as_array)
        .expect("messages");
    for message in messages {
        assert_ne!(
            message.get("role").and_then(Value::as_str),
            Some("system"),
            "and never as a message"
        );
    }
}

#[test]
fn a_demanded_tool_is_named_in_each_dialects_own_shape() {
    let want = || Want::Tool {
        name: "choose_spans".into(),
        schema: schema(),
    };
    let openai = parsed(Dialect::OpenAi, want());
    let tools = openai
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools");
    assert_eq!(
        tools[0].get("type").and_then(Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0]
            .get("function")
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str),
        Some("choose_spans")
    );
    assert!(
        tools[0]
            .get("function")
            .and_then(|f| f.get("parameters"))
            .is_some(),
        "the OpenAI dialect calls the schema `parameters`"
    );

    let anthropic = parsed(Dialect::Anthropic, want());
    let tools = anthropic
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools");
    assert_eq!(
        tools[0].get("name").and_then(Value::as_str),
        Some("choose_spans")
    );
    assert!(
        tools[0].get("input_schema").is_some(),
        "the Anthropic dialect calls it `input_schema`"
    );
    assert_eq!(
        anthropic
            .get("tool_choice")
            .and_then(|c| c.get("name"))
            .and_then(Value::as_str),
        Some("choose_spans")
    );
}

#[test]
fn a_structure_is_asked_for_only_where_the_dialect_has_somewhere_to_ask() {
    // `response_format` is silently ignored by this provider, so asking is
    // never the thing that makes the answer valid — CBR validates. The
    // Anthropic dialect has no such field at all, and inventing one would
    // be a body the provider ignores for a different reason.
    let openai = parsed(Dialect::OpenAi, Want::Structure { schema: schema() });
    assert_eq!(
        openai
            .get("response_format")
            .and_then(|f| f.get("type"))
            .and_then(Value::as_str),
        Some("json_schema")
    );
    let anthropic = parsed(Dialect::Anthropic, Want::Structure { schema: schema() });
    assert!(
        anthropic.get("response_format").is_none(),
        "the Anthropic dialect has no response_format"
    );
}

#[test]
fn the_model_id_is_the_configured_one_in_both_dialects() {
    for dialect in BOTH {
        assert_eq!(
            parsed(dialect, Want::Text)
                .get("model")
                .and_then(Value::as_str),
            Some("MiniMax-M2.7")
        );
    }
}

// --- the bytes themselves -----------------------------------------------

#[test]
fn the_same_request_serializes_to_the_same_bytes() {
    // The count call and the completion send **the same body**, and the
    // local estimate is taken over those bytes. A serializer whose output
    // varies run to run would make the count a count of something else.
    for dialect in BOTH {
        let once = request(Want::Text).serialize(dialect);
        let twice = request(Want::Text).serialize(dialect);
        assert_eq!(once, twice, "{}", dialect.name());
    }
}

#[test]
fn the_framed_message_count_includes_the_system_instruction() {
    // What the estimate's per-message overhead is multiplied by. The
    // system instruction is framed too, wherever the dialect puts it, so
    // leaving it out under-counts by one message in every request.
    for dialect in BOTH {
        assert_eq!(
            request(Want::Text).framed_messages(dialect),
            2,
            "{}: one user message and one system instruction",
            dialect.name()
        );
    }
}

#[test]
fn a_conversation_keeps_its_turns_in_order_and_in_role() {
    // A model-assisted selection is not always one question: a repair adds
    // a turn, and m4c's loop will add more. The roles have to survive, and
    // an assistant turn is the one a serializer that only ever wrote `user`
    // would get wrong without any test noticing.
    let mut asked = request(Want::Text);
    asked.messages.push(Message {
        role: Role::Assistant,
        text: "which ones are eligible?".into(),
    });
    asked.messages.push(Message {
        role: Role::User,
        text: "s1, s4".into(),
    });
    for dialect in BOTH {
        let body = cbr_encoding::parse(&asked.serialize(dialect)).expect("its own body");
        let messages = body
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages");
        let roles: Vec<_> = messages
            .iter()
            .filter_map(|message| message.get("role").and_then(Value::as_str))
            .collect();
        let expected: Vec<&str> = match dialect {
            Dialect::OpenAi => vec!["system", "user", "assistant", "user"],
            Dialect::Anthropic => vec!["user", "assistant", "user"],
        };
        assert_eq!(roles, expected, "{}", dialect.name());
    }
}
