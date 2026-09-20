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
    /// Canonical form, so that the count call and the completion send the
    /// same body and the local estimate is an estimate of what was sent.
    pub fn serialize(&self, dialect: Dialect) -> Vec<u8> {
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
            (_, None) => {}
        }
        for held in &self.messages {
            messages.push(message(held.role.name(), &held.text));
        }
        body.push(("messages".into(), Value::Array(messages)));
        match &self.want {
            Want::Text => {}
            Want::Structure { schema } => {
                if dialect == Dialect::OpenAi {
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
                Dialect::OpenAi => {
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
