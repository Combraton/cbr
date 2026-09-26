//! What CBR asks a model for, and how each dialect frames it.
//!
//! # The generation limit is bound, not mentioned
//!
//! m4a guarded the send with a textual check: it asked whether the number
//! appeared anywhere in the serialized body. That admits a body whose limit
//! is 4,096 and whose prose happens to say 16, which spends 4,096 against a
//! reservation made for 16. [`declares_generation`] replaces it by parsing
//! the body and reading the member the dialect's provider actually reads.
//!
//! # What is asked for and what is checked are different things
//!
//! `response_format` and `tool_choice: "required"` are **silently ignored**
//! by this provider ([STACK §8.1](../../../docs/work/readiness/STACK.md)).
//! They are still sent — a provider that starts honouring them costs
//! nothing — but nothing downstream treats having asked as having received.
//! The parser is where a shape becomes true.

use cbr_encoding::Value;

use super::Dialect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    // A conversation has one as soon as anything is repaired or asked
    // again; m4c's loop builds them. Exercised by the serializer's tests
    // now, so the roles cannot drift before there is a caller.
    #[cfg_attr(not(test), allow(dead_code))]
    Assistant,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

/// What shape the answer must take. **The provider guarantees none of
/// these**, which is why each is something CBR checks rather than trusts.
#[derive(Debug, Clone)]
pub enum Want {
    /// Free text, and prose is a valid answer.
    Text,
    // The calibration asks only for text. These two are what m4c's
    // selection asks for — a closed set of ids, or a call to a tool that
    // takes them — and both halves of the wire for them are exercised by
    // the serializer's and the parser's tests now.
    /// A JSON object. Asked for where the dialect has a field for it, and
    /// validated on the way back either way.
    #[cfg_attr(not(test), allow(dead_code))]
    Structure { schema: Value },
    /// A named tool call. `required` is ignored by this provider, so a text
    /// answer here is an ordinary outcome to repair rather than an error.
    #[cfg_attr(not(test), allow(dead_code))]
    Tool { name: String, schema: Value },
}

#[derive(Debug, Clone)]
pub struct Request {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    /// The limit this request is reserved for, and the one the body binds.
    pub generation: u64,
    pub want: Want,
}

impl Request {
    /// The exact bytes that go on the wire.
    ///
    /// Canonical form, so that the count call and the completion are over
    /// bytes that can be compared, and so the local estimate is an
    /// estimate of what was sent.
    pub fn serialize(&self, dialect: Dialect) -> Vec<u8> {
        match dialect {
            Dialect::Responses => self.serialize_responses(false),
            Dialect::OpenAi | Dialect::Anthropic => self.serialize_chat(dialect),
        }
    }

    /// The body the **counting endpoint** is sent, or `None` for a dialect
    /// it does not describe.
    ///
    /// It is the completion's body minus the three members the endpoint
    /// does not document — `max_output_tokens`, `service_tier`, `stream` —
    /// and identical in everything else. That is the whole point: a count
    /// predicts a completion's cost only if it counts the same input, the
    /// same instructions and the same tools.
    pub fn serialize_count(&self, dialect: Dialect) -> Option<Vec<u8>> {
        dialect.counted().then(|| self.serialize_responses(true))
    }

    /// `POST /v1/responses`, and its counting endpoint.
    fn serialize_responses(&self, counting: bool) -> Vec<u8> {
        let mut body: Vec<(String, Value)> =
            vec![("model".into(), Value::String(self.model.clone()))];
        if !counting {
            body.push((
                Dialect::Responses.generation_field().into(),
                Value::Int(self.generation.min(i64::MAX as u64) as i64),
            ));
            // **Explicitly `standard`, and `priority` never.** A default is
            // a thing that changes; and it is the owner's quota, so
            // nothing here spends it faster on its own say-so.
            body.push(("service_tier".into(), Value::String("standard".into())));
            // No `[DONE]` sentinel to read a stream to the end by.
            body.push(("stream".into(), Value::Bool(false)));
        }
        if let Some(system) = &self.system {
            // Its own member here, not an item of `input`. Put in `input`
            // it is accepted and treated as conversation.
            body.push(("instructions".into(), Value::String(system.clone())));
        }
        body.push((
            "input".into(),
            Value::Array(
                self.messages
                    .iter()
                    .map(|held| {
                        Value::Object(vec![
                            ("type".into(), Value::String("message".into())),
                            ("role".into(), Value::String(held.role.name().into())),
                            ("content".into(), Value::String(held.text.clone())),
                        ])
                    })
                    .collect(),
            ),
        ));
        match &self.want {
            // `text.format.type` documents one value, `text`, so there is
            // no structured-output member to ask in. CBR validates instead,
            // which it does on every dialect anyway.
            Want::Text | Want::Structure { .. } => {}
            Want::Tool { name, schema } => {
                body.push((
                    "tools".into(),
                    Value::Array(vec![Value::Object(vec![
                        ("type".into(), Value::String("function".into())),
                        ("name".into(), Value::String(name.clone())),
                        ("parameters".into(), schema.clone()),
                    ])]),
                ));
                // `tool_choice` documents `none` and `auto` and not
                // `required`, so a call cannot be demanded here at all —
                // which is why a text answer where one was wanted is an
                // ordinary outcome rather than a surprise.
                body.push(("tool_choice".into(), Value::String("auto".into())));
            }
        }
        cbr_encoding::to_canonical(&Value::Object(body))
    }

    /// The two chat dialects, unchanged from m4b.
    fn serialize_chat(&self, dialect: Dialect) -> Vec<u8> {
        let mut body: Vec<(String, Value)> = vec![
            ("model".into(), Value::String(self.model.clone())),
            (
                dialect.generation_field().into(),
                Value::Int(self.generation.min(i64::MAX as u64) as i64),
            ),
            // Whole responses only: there is no `data: [DONE]` sentinel to
            // read a stream to the end by.
            ("stream".into(), Value::Bool(false)),
            // Without it, MiniMax-M3 embeds `<think>...</think>` in the
            // content and reasoning reaches a record as if it were output.
            ("reasoning_split".into(), Value::Bool(true)),
        ];
        let mut messages: Vec<Value> = Vec::new();
        match (dialect, &self.system) {
            // The OpenAI dialect has no top-level instruction: it is the
            // first message, and it is framed like one.
            (Dialect::OpenAi, Some(system)) => messages.push(message("system", system)),
            (Dialect::Anthropic, Some(system)) => {
                body.push(("system".into(), Value::String(system.clone())));
            }
            // Reached only through `serialize`, which routes the Responses
            // dialect elsewhere.
            (Dialect::Responses, _) | (_, None) => {}
        }
        for held in &self.messages {
            messages.push(message(held.role.name(), &held.text));
        }
        body.push(("messages".into(), Value::Array(messages)));
        match &self.want {
            Want::Text => {}
            Want::Structure { schema } => {
                if dialect != Dialect::Anthropic {
                    body.push((
                        "response_format".into(),
                        Value::Object(vec![
                            ("type".into(), Value::String("json_schema".into())),
                            (
                                "json_schema".into(),
                                Value::Object(vec![
                                    ("name".into(), Value::String("answer".into())),
                                    ("schema".into(), schema.clone()),
                                ]),
                            ),
                        ]),
                    ));
                }
                // The Anthropic dialect has no such member. Inventing one
                // would be a body ignored for a second, worse reason.
            }
            Want::Tool { name, schema } => match dialect {
                Dialect::Responses | Dialect::OpenAi => {
                    body.push((
                        "tools".into(),
                        Value::Array(vec![Value::Object(vec![
                            ("type".into(), Value::String("function".into())),
                            (
                                "function".into(),
                                Value::Object(vec![
                                    ("name".into(), Value::String(name.clone())),
                                    ("parameters".into(), schema.clone()),
                                ]),
                            ),
                        ])]),
                    ));
                    body.push((
                        "tool_choice".into(),
                        Value::Object(vec![
                            ("type".into(), Value::String("function".into())),
                            (
                                "function".into(),
                                Value::Object(vec![("name".into(), Value::String(name.clone()))]),
                            ),
                        ]),
                    ));
                }
                Dialect::Anthropic => {
                    body.push((
                        "tools".into(),
                        Value::Array(vec![Value::Object(vec![
                            ("name".into(), Value::String(name.clone())),
                            ("input_schema".into(), schema.clone()),
                        ])]),
                    ));
                    body.push((
                        "tool_choice".into(),
                        Value::Object(vec![
                            ("type".into(), Value::String("tool".into())),
                            ("name".into(), Value::String(name.clone())),
                        ]),
                    ));
                }
            },
        }
        cbr_encoding::to_canonical(&Value::Object(body))
    }

    /// How many messages the provider frames, which is what the estimate's
    /// per-message overhead multiplies. The system instruction is framed
    /// too, wherever the dialect keeps it.
    pub fn framed_messages(&self, _dialect: Dialect) -> usize {
        self.messages.len() + usize::from(self.system.is_some())
    }
}

/// The generation budget for an answer that itself needs `answer` tokens.
///
/// See [`crate::budget::REASONING_HEADROOM`] for why this is generous: an
/// under-sized limit costs the whole call and returns nothing, while an
/// over-sized one costs only what is produced.
#[cfg_attr(not(test), allow(dead_code))]
pub fn generation_for(answer: u64) -> u64 {
    answer
        .saturating_mul(crate::budget::REASONING_HEADROOM)
        .max(crate::budget::MIN_OUTPUT_TOKENS)
}

/// The budget a repair asks for after a truncation: **more room, not the
/// same room again**. Doubling once, and the repair bound stops it there.
pub fn widened(generation: u64) -> u64 {
    generation.saturating_mul(2)
}

/// What CBR says when it asks again. **Its own words**, naming the shape
/// rather than quoting the answer that did not have it.
pub fn repair_instruction(want: &Want) -> &'static str {
    match want {
        Want::Text => "Answer in plain text.",
        Want::Structure { .. } => {
            "That answer was not usable. Reply with a single JSON object and nothing else: \
             no explanation, no code fence, no text before or after it."
        }
        Want::Tool { .. } => {
            "That answer was not usable. Reply by calling the tool you were given, with its \
             arguments, and send no other content."
        }
    }
}

/// How many bytes `text` takes inside a request body, which is JSON: the
/// escapes are counted, because they are sent. A quotation mark is two,
/// U+0001 is six, and `é` is its two bytes of UTF-8.
///
/// Measured by the serializer that writes the body, so it cannot disagree
/// with what is sent.
pub fn carried_bytes(text: &str) -> usize {
    cbr_encoding::to_canonical(&Value::String(text.to_string())).len() - 2
}

/// The longest prefix of `text`, on a character boundary, that a request
/// body carries in at most `bytes` bytes.
///
/// **Cut between characters, never inside one**, and so never inside an
/// escape: JSON escapes one character at a time, so what a prefix carries
/// is the sum of what its characters carry, and the first character that
/// would pass the bound is where the prefix ends.
pub fn carried_within(text: &str, bytes: usize) -> &str {
    let mut used = 0usize;
    for (at, character) in text.char_indices() {
        used += carried_bytes(character.encode_utf8(&mut [0u8; 4]));
        if used > bytes {
            return &text[..at];
        }
    }
    text
}

fn message(role: &str, text: &str) -> Value {
    Value::Object(vec![
        ("role".into(), Value::String(role.into())),
        ("content".into(), Value::String(text.into())),
    ])
}

/// Whether `body` **binds** the generation limit to the field this dialect's
/// provider reads.
///
/// Parsed, not searched. A body that cannot be parsed declares nothing,
/// which is the conservative answer: an unreadable body is one whose limit
/// CBR cannot vouch for, and it does not leave the process.
pub fn declares_generation(dialect: Dialect, body: &[u8], generation: u64) -> bool {
    let Ok(parsed) = cbr_encoding::parse(body) else {
        return false;
    };
    parsed.get(dialect.generation_field()) == Some(&Value::Int(generation as i64))
}
