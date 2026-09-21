//! Reading a provider's answer.
//!
//! # Nothing downstream trusts a shape the model was only asked for
//!
//! Three of the four behaviours the owner recorded in
//! [STACK §8.1](../../../docs/work/readiness/STACK.md) land here.
//! `response_format` is ignored, so prose arrives where an object was
//! asked for. `tool_choice: "required"` is ignored, so text arrives where
//! a call was demanded. Neither is a transport error: both are **ordinary
//! outcomes** with a bounded repair, because that is what the provider
//! does rather than what has gone wrong. The third is the `<think>` marker,
//! and that one is refused outright.
//!
//! # Why refusing rather than stripping the reasoning
//!
//! Stripping would put CBR in the business of deciding which half of a
//! response was the answer, on a response that has already shown it is not
//! shaped the way the request asked. A refusal is recorded, costs one call,
//! and cannot silently seal a model's private reasoning as its output.
//!
//! # None of a provider's own words survive
//!
//! An error message from a provider is a stranger's text that would reach a
//! log, an event and an item's unmet reason. [`Unusable`] is typed and
//! carries none of it.

#![allow(dead_code)]

use cbr_encoding::Value;

use super::Dialect;
use super::json::{self, Json};
use super::request::Want;

/// The markers MiniMax-M3 embeds in `message.content` when
/// `reasoning_split` is not honoured.
pub const THINK_OPEN: &str = "<think>";
pub const THINK_CLOSE: &str = "</think>";

/// The members a token count could arrive under.
///
/// **The least verified thing in this module.** A body naming none of them
/// is refused rather than guessed at, which leaves the local estimate
/// standing — the conservative direction, and the reason the local bound
/// exists at all. The calibration replaces this list with the answer.
const COUNT_MEMBERS: [&str; 4] = ["total_tokens", "input_tokens", "tokens", "token_num"];

/// Provider status codes that mean its quota is gone rather than that
/// something went wrong. **A guess about two codes**: anything else
/// non-zero degrades to [`Unusable::ProviderError`], never to success.
const EXHAUSTED_CODES: [u64; 2] = [1002, 1008];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Text(String),
    Structure(Value),
    Tool { name: String, input: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    /// Not this dialect's response shape at all.
    Malformed,
    /// The provider reported a failure in the body, with an HTTP 200.
    ProviderError,
    /// The provider says its quota is gone while CBR's ledger has room.
    ProviderExhausted,
    /// Reasoning markers survived into the content.
    ReasoningLeaked,
    /// The generation limit was reached.
    Truncated,
    /// Prose where a structure was asked for. **Repairable.**
    NotStructured,
    /// Text where a tool call was demanded. **Repairable.**
    NoToolCall,
}

impl Unusable {
    pub fn reason(self) -> &'static str {
        match self {
            Unusable::Malformed => "model_answer_malformed",
            Unusable::ProviderError => "model_call_failed",
            Unusable::ProviderExhausted => "provider_quota_exhausted",
            Unusable::ReasoningLeaked => "model_reasoning_leaked",
            Unusable::Truncated => "model_answer_truncated",
            Unusable::NotStructured => "model_output_unstructured",
            Unusable::NoToolCall => "model_tool_call_missing",
        }
    }

    /// Whether asking again, once, could plausibly produce something
    /// different.
    ///
    /// Two of these are what the provider's documented behaviour produces:
    /// prose where a structure was asked for, and text where a tool call
    /// was demanded. **The third is truncation**, which m4b had as *not*
    /// repairable on the reasoning that asking again under the same limit
    /// gives the same answer. That premise was the mistake: the repair
    /// asks again with a **larger** limit. Run 2 spent a whole call on
    /// sixteen tokens of reasoning and returned nothing, and under the old
    /// rule that call was simply lost.
    ///
    /// Everything else would spend twice for one answer.
    pub fn repairable(self) -> bool {
        matches!(
            self,
            Unusable::NotStructured | Unusable::NoToolCall | Unusable::Truncated
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    pub reply: Result<Reply, Unusable>,
    /// What the provider said it cost, when it said anything — **read even
    /// when the reply is unusable**, because an unusable answer is still a
    /// charge against a shared quota.
    pub usage: Option<u64>,
    /// What the provider said the **input** cost, separately.
    ///
    /// The calibration compares it against the counting endpoint's
    /// prediction for the same request, which is the measurement that says
    /// whether making the count is worth anything.
    pub input_usage: Option<u64>,
}

/// One text part and one optional tool call, however the dialect carries
/// them.
struct Content {
    text: Option<String>,
    tool: Option<(String, Json)>,
    truncated: bool,
}

pub fn read_completion(dialect: Dialect, want: &Want, body: &[u8]) -> Read {
    let Some(read) = json::read(body) else {
        return Read {
            reply: Err(Unusable::Malformed),
            usage: None,
            input_usage: None,
        };
    };
    let usage = usage(dialect, &read);
    let input_usage = input_usage(&read);
    if let Some(failure) = provider_failure(&read) {
        return Read {
            reply: Err(failure),
            usage,
            input_usage,
        };
    }
    let Some(content) = content(dialect, &read) else {
        return Read {
            reply: Err(Unusable::Malformed),
            usage,
            input_usage,
        };
    };
    Read {
        reply: interpret(want, content),
        usage,
        input_usage,
    }
}

fn interpret(want: &Want, content: Content) -> Result<Reply, Unusable> {
    // Before anything else is decided about it: reasoning that reached the
    // content is refused whole, wherever in it the marker appears.
    if let Some(text) = &content.text
        && (text.contains(THINK_OPEN) || text.contains(THINK_CLOSE))
    {
        return Err(Unusable::ReasoningLeaked);
    }
    if content.truncated {
        return Err(Unusable::Truncated);
    }
    match want {
        Want::Text => content.text.map(Reply::Text).ok_or(Unusable::Malformed),
        Want::Structure { .. } => {
            let text = content.text.ok_or(Unusable::NotStructured)?;
            let parsed = json::read(text.as_bytes()).ok_or(Unusable::NotStructured)?;
            // It parses and is still not something CBR can seal, which is
            // the same outcome as prose rather than a second kind of
            // failure: both are repaired by asking again.
            let recordable = parsed.recordable().ok_or(Unusable::NotStructured)?;
            if !recordable.is_object() {
                return Err(Unusable::NotStructured);
            }
            Ok(Reply::Structure(recordable))
        }
        Want::Tool { name, .. } => {
            let (called, input) = content.tool.ok_or(Unusable::NoToolCall)?;
            if &called != name {
                // A call to something else is not the call that was
                // demanded, and the closed-set rule says a name CBR did
                // not offer is never resolved.
                return Err(Unusable::NoToolCall);
            }
            let input = input.recordable().ok_or(Unusable::NotStructured)?;
            Ok(Reply::Tool {
                name: called,
                input,
            })
        }
    }
}

/// A failure the provider reported **inside a successful HTTP response**,
/// which is how this provider reports most of them.
fn provider_failure(read: &Json) -> Option<Unusable> {
    if let Some(status) = read
        .get("base_resp")
        .and_then(|base| base.get("status_code"))
        .and_then(Json::as_u64)
        && status != 0
    {
        return Some(if EXHAUSTED_CODES.contains(&status) {
            Unusable::ProviderExhausted
        } else {
            Unusable::ProviderError
        });
    }
    // **A body that carries an error member is a failure, whatever the
    // status says, in every dialect.**
    //
    // The first live call proved the need for it: the failure arrived as
    // `{"error":{"message","code"}}` with a *string* code — neither the
    // `base_resp` shape above nor Anthropic's `{"type":"error"}` — and a
    // parser that let the HTTP status decide read it as a successful
    // response with no content. That is the worst available reading,
    // because everything downstream then treats emptiness as the answer.
    //
    // The shapes differ per dialect and will keep differing. The rule does
    // not, so it is written once and over the member rather than over any
    // one vendor's spelling of it.
    // The Responses API reports a failure in `status` as well as in
    // `error`, and the two do not always both appear.
    if read.get("status").and_then(Json::as_str) == Some("failed") {
        return Some(Unusable::ProviderError);
    }
    match read.get("error") {
        // `"error": null` is what a successful Responses API answer
        // carries. Reading that as a failure turns every good answer into
        // one, which is the opposite mistake and no better.
        None | Some(Json::Null) => {}
        Some(error) => {
            return Some(if names_an_exhausted_quota(error) {
                Unusable::ProviderExhausted
            } else {
                Unusable::ProviderError
            });
        }
    }
    None
}

/// Whether an error says the quota is gone rather than that the request was
/// wrong. Read across the members vendors put it in — `type`, `code`,
/// `message` — because which one carries it is the part that varies.
fn names_an_exhausted_quota(error: &Json) -> bool {
    let mut said = String::new();
    for member in ["type", "code", "message"] {
        if let Some(text) = error.get(member).and_then(Json::as_str) {
            said.push_str(text);
            said.push(' ');
        }
    }
    if let Some(text) = error.as_str() {
        said.push_str(text);
    }
    let said = said.to_ascii_lowercase();
    ["rate_limit", "rate limit", "quota", "insufficient balance"]
        .iter()
        .any(|marker| said.contains(marker))
}

fn content(dialect: Dialect, read: &Json) -> Option<Content> {
    match dialect {
        Dialect::Responses => {
            let output = read.get("output")?.as_array()?;
            // `incomplete` is an ordinary outcome with a cost, not a
            // fault: reasoning tokens are output tokens and cannot be
            // disabled on the M2.x models, so a small limit can be spent
            // entirely on reasoning and end with no text at all.
            let truncated = read.get("status").and_then(Json::as_str) == Some("incomplete");
            let mut text = read
                .get("output_text")
                .and_then(Json::as_str)
                .map(str::to_string);
            let mut tool = None;
            for item in output {
                match item.get("type").and_then(Json::as_str) {
                    // Reasoning is its own item here, which is what
                    // `reasoning_split` asks the chat dialects for. It is
                    // **never** read as the answer.
                    Some("reasoning") => {}
                    Some("message") if text.is_none() => {
                        text = item
                            .get("content")
                            .and_then(Json::as_array)
                            .and_then(|parts| {
                                parts
                                    .iter()
                                    .find(|part| {
                                        part.get("type").and_then(Json::as_str)
                                            == Some("output_text")
                                    })
                                    .and_then(|part| part.get("text"))
                                    .and_then(Json::as_str)
                            })
                            .map(str::to_string);
                    }
                    // As on the chat dialect, the arguments are a JSON
                    // **string** and are read a second time.
                    Some("function_call") => {
                        if let (Some(name), Some(written)) = (
                            item.get("name").and_then(Json::as_str),
                            item.get("arguments").and_then(Json::as_str),
                        ) && let Some(input) = json::read(written.as_bytes())
                        {
                            tool = Some((name.to_string(), input));
                        }
                    }
                    _ => {}
                }
            }
            Some(Content {
                text,
                tool,
                truncated,
            })
        }
        Dialect::OpenAi => {
            let choice = read.get("choices")?.as_array()?.first()?;
            let message = choice.get("message")?;
            let truncated = choice.get("finish_reason").and_then(Json::as_str) == Some("length");
            let text = message
                .get("content")
                .and_then(Json::as_str)
                .map(str::to_string);
            // The arguments arrive as a **JSON string**, so they are read a
            // second time. A parser that forgets gets the text of a
            // document where the document was meant.
            let tool = message
                .get("tool_calls")
                .and_then(Json::as_array)
                .and_then(<[Json]>::first)
                .and_then(|call| call.get("function"))
                .and_then(|function| {
                    let name = function.get("name")?.as_str()?.to_string();
                    let written = function.get("arguments")?.as_str()?;
                    Some((name, json::read(written.as_bytes())?))
                });
            Some(Content {
                text,
                tool,
                truncated,
            })
        }
        Dialect::Anthropic => {
            let parts = read.get("content")?.as_array()?;
            let truncated = read.get("stop_reason").and_then(Json::as_str) == Some("max_tokens");
            let mut text = None;
            let mut tool = None;
            for part in parts {
                match part.get("type").and_then(Json::as_str) {
                    Some("text") => {
                        text = part.get("text").and_then(Json::as_str).map(str::to_string);
                    }
                    // Here the input is an object already, which is the
                    // difference from the other dialect.
                    Some("tool_use") => {
                        if let (Some(name), Some(input)) =
                            (part.get("name").and_then(Json::as_str), part.get("input"))
                        {
                            tool = Some((name.to_string(), input.clone()));
                        }
                    }
                    _ => {}
                }
            }
            Some(Content {
                text,
                tool,
                truncated,
            })
        }
    }
}

/// The input half of the usage, which every dialect spells the same way.
fn input_usage(read: &Json) -> Option<u64> {
    let usage = read.get("usage")?;
    usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Json::as_u64)
}

fn usage(dialect: Dialect, read: &Json) -> Option<u64> {
    let usage = read.get("usage")?;
    match dialect {
        // `total_tokens` is the whole charge, reasoning included, so
        // reading it needs no arithmetic and cannot forget a part.
        Dialect::Responses | Dialect::OpenAi => usage.get("total_tokens").and_then(Json::as_u64),
        // Reported as two halves, which CBR adds: a parser reading one of
        // them charges a shared quota for less than it spent.
        Dialect::Anthropic => {
            let input = usage.get("input_tokens").and_then(Json::as_u64)?;
            let output = usage.get("output_tokens").and_then(Json::as_u64)?;
            Some(input.saturating_add(output))
        }
    }
}

/// What the transport needs to settle a reservation truthfully: what the
/// provider said this cost, and whether the body itself reports a failure.
///
/// Separate from [`read_completion`] because the transport settles the
/// ledger before anything has decided what the answer *means*, and the
/// ledger's question is only ever "what was spent, and was the quota gone".
pub fn accounting(dialect: Dialect, raw: &[u8]) -> (Option<u64>, Option<Unusable>) {
    let Some(read) = json::read(raw) else {
        return (None, Some(Unusable::Malformed));
    };
    (usage(dialect, &read), provider_failure(&read))
}

/// What the provider said the **input** of a completion cost, for the
/// tripwire that compares it against the bound that admitted the call.
pub fn input_usage_of(dialect: Dialect, raw: &[u8]) -> Option<u64> {
    let _ = dialect;
    input_usage(&json::read(raw)?)
}

/// The provider's own token count, from `POST /v1/responses/input_tokens`.
pub fn read_count(body: &[u8]) -> Option<u64> {
    let read = json::read(body)?;
    if provider_failure(&read).is_some() {
        return None;
    }
    let mut found: Option<u64> = None;
    for member in COUNT_MEMBERS {
        let Some(count) = read.get(member).and_then(Json::as_u64) else {
            continue;
        };
        match found {
            // Two accepted names disagreeing is a body this parser does not
            // understand, and picking one of them is guessing.
            Some(held) if held != count => return None,
            _ => found = Some(count),
        }
    }
    found
}
