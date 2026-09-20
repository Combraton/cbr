//! The wire: who CBR may talk to, in which dialect, and over what.
//!
//! **No live call is made anywhere in this milestone.** Everything here is
//! exercised against hand-written fixtures and a transport that is shut by
//! default; the first real call is the calibration, which is its own step
//! after this work has been reviewed and merged
//! ([READINESS §10](../../docs/work/m4/READINESS.md)).
//!
//! # Two dialects, one provider, one transport
//!
//! The owner's decision ([ADR 001 question 3]) is MiniMax addressed through
//! both of its wires: an OpenAI-compatible one and an Anthropic-compatible
//! one. They differ in how a request is framed and how a response is read,
//! and in nothing else — so they are two serializers and two parsers over
//! one transport rather than two clients.
//!
//! [ADR 001 question 3]: ../../docs/decisions/001-standalone-v0.1-scope-and-stack.md

/// The only provider id CBR admits. A launch naming any other is refused
/// before anything is opened, read or sent.
pub const PROVIDER_ID: &str = "minimax";

/// The three models the owner named, chosen per task:
/// `MiniMax-M2.7-highspeed` for extraction and large-result projection,
/// `MiniMax-M2.7` for ordinary derivation, `MiniMax-M3` for synthesis,
/// request-time investigation and image input.
pub const MODELS: [&str; 3] = ["MiniMax-M2.7-highspeed", "MiniMax-M2.7", "MiniMax-M3"];

/// Where admission counts go, for **both** dialects.
///
/// `POST /v1/responses/input_tokens` is MiniMax's own and belongs to
/// neither compatibility surface, so it is not derived from the dialect's
/// endpoint ([STACK §8.1](../../docs/work/readiness/STACK.md)).
pub const COUNT_ENDPOINT: &str = "https://api.minimax.io/v1/responses/input_tokens";

/// Which of the provider's two wires a configured model speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `https://api.minimax.io/v1`, OpenAI-compatible.
    OpenAi,
    /// `https://api.minimax.io/anthropic`, Anthropic-compatible.
    Anthropic,
}

impl Dialect {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "openai" => Some(Dialect::OpenAi),
            "anthropic" => Some(Dialect::Anthropic),
            _ => None,
        }
    }

    // Recorded in every derivation record, which is m4d's; read by the
    // tests now so that the two names cannot drift from `parse`.
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Dialect::OpenAi => "openai",
            Dialect::Anthropic => "anthropic",
        }
    }

    /// The member this dialect's provider reads the generation limit from.
    ///
    /// **Both are `max_tokens` today**, and they are asked for separately
    /// anyway: the guard that refuses to send a body whose limit is not
    /// bound to this field asks the dialect, so the day one surface moves
    /// to `max_completion_tokens` one constant changes and the guard
    /// follows it.
    pub fn generation_field(self) -> &'static str {
        match self {
            Dialect::OpenAi => "max_tokens",
            Dialect::Anthropic => "max_tokens",
        }
    }

    /// The path this dialect's request goes to, under the configured
    /// endpoint.
    // Read by the transport, later in this same pull request.
    #[allow(dead_code)]
    pub fn path(self) -> &'static str {
        match self {
            Dialect::OpenAi => "/chat/completions",
            Dialect::Anthropic => "/v1/messages",
        }
    }

    /// The endpoint the owner recorded for this dialect. It is a **default
    /// for configuration**, not a literal the call path reaches for: the
    /// endpoint that is used is the one in the launch configuration.
    pub fn default_endpoint(self) -> &'static str {
        match self {
            Dialect::OpenAi => "https://api.minimax.io/v1",
            Dialect::Anthropic => "https://api.minimax.io/anthropic",
        }
    }
}

pub mod http;
pub mod json;
pub mod net;
pub mod record;
pub mod redact;
pub mod request;
pub mod response;

#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod json_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod redact_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod response_tests;
#[cfg(test)]
mod tests;
