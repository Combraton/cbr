//! The transport: the one thing in CBR that opens a socket.
//!
//! **It cannot be built without a [`Permit`]**, and a permit exists only
//! when the launch was given `--permit-model-network`. That is why no test
//! in this suite reaches a network: not because none of them tries, but
//! because none of them can.
//!
//! # Decomposed so that everything but the glue is testable without one
//!
//! Which URL a call goes to, how the agent is configured, what a status and
//! a body mean, and what an error means are each a function here with its
//! own test. [`Http::send`] is what is left over, and it makes no decisions.
//!
//! # `NotSent` is a claim, not a default
//!
//! The ledger settles a reservation differently for each: `NotSent` means
//! **nothing was charged**, so it is claimed only where the request body
//! provably never reached the provider — a name that did not resolve, a
//! connection that was refused, a timeout in the resolver or the connect.
//! Everything else that goes wrong is a [`Answer::Failed`] with no usage,
//! which settles at the reservation's estimate: a request that was written
//! and then went wrong is one the provider may well have charged for, and
//! under-counting a quota shared with the owner's own tools is the one
//! direction that cannot be corrected later.

#![allow(dead_code)]

use std::time::Duration;

use super::Dialect;
use super::endpoint;
use super::net::{self, Permit};
use super::response;
use crate::keychain::Secret;
use crate::model::{Answer, Call, Exchange, Transport};

/// The most a response may be before it is refused unread.
pub const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

pub struct Http<'a> {
    /// Held, not read: its existence is the point.
    permit: Permit,
    pub dialect: Dialect,
    pub credential: &'a Secret,
    pub timeout: Duration,
}

impl<'a> Http<'a> {
    /// **There is no endpoint parameter.** The host is pinned and the path
    /// is the dialect's, so there is nothing here for a configuration, an
    /// operator's mistake or a planted file to point elsewhere.
    pub fn new(
        permit: Permit,
        dialect: Dialect,
        credential: &'a Secret,
        timeout: Duration,
    ) -> Self {
        Http {
            permit,
            dialect,
            credential,
            timeout,
        }
    }
}

impl Transport for Http<'_> {
    fn send(&self, call: Call, body: &[u8]) -> Exchange {
        // **Pinned at the last moment**, not only where configuration was
        // read. Nothing goes past this with the credential attached.
        let url = match endpoint::url_to_send(self.dialect, call) {
            Ok(url) => url,
            Err(refusal) => {
                return Exchange {
                    // Nothing left the process: no socket was opened.
                    answer: Answer::NotSent(refusal.reason().into()),
                    raw: Vec::new(),
                };
            }
        };
        // The credential's one destination, in a value that zeroes itself.
        let authorization = self.credential.authorization();
        net::note_request();
        let sent = agent(self.timeout)
            .post(url.as_str())
            .header("Authorization", authorization.value())
            .header("Content-Type", "application/json")
            .send(body);
        let mut response = match sent {
            Ok(response) => response,
            Err(error) => {
                return Exchange {
                    answer: answer_for_error(&error),
                    raw: Vec::new(),
                };
            }
        };
        let status = response.status().as_u16();
        let raw = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .unwrap_or_default();
        Exchange {
            answer: answer_for(call, self.dialect, status, &raw),
            raw,
        }
    }
}

pub fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // **The environment does not get to redirect a request carrying the
        // owner's credential.** ureq reads `HTTP_PROXY` and friends by
        // default, which would let anything able to set a variable in this
        // process receive the `Authorization` header.
        .proxy(None)
        // This provider reports its failures in the body, and a 429's body
        // holds its own account of the exhaustion. Turning the status into
        // an error throws that away before it can be read.
        .http_status_as_error(false)
        // A redirect on a POST carrying repository text and a credential is
        // not something to follow without being told to.
        .max_redirects(0)
        .build()
        .new_agent()
}

/// What a status and a body mean.
pub fn answer_for(call: Call, dialect: Dialect, status: u16, raw: &[u8]) -> Answer {
    // The status the provider uses for a quota that is gone, before the
    // body is consulted at all.
    if status == 429 {
        return Answer::ProviderExhausted;
    }
    let (usage, failure) = response::accounting(dialect, raw);
    if failure == Some(response::Unusable::ProviderExhausted) {
        return Answer::ProviderExhausted;
    }
    let failed = |reason: &str| Answer::Failed {
        // Typed, and never the provider's own words: its message is a
        // stranger's text and would reach a log and an item's reason.
        reason: reason.to_string(),
        usage,
    };
    if !(200..300).contains(&status) {
        return failed("provider_status");
    }
    if failure.is_some() {
        return failed("provider_error");
    }
    match call {
        Call::Count => match response::read_count(raw) {
            Some(counted) => Answer::Counted(counted),
            // Unreadable, so the local estimate stands. It is a failure
            // **after the send**, which settles at the estimate rather
            // than at nothing.
            None => Answer::Failed {
                reason: "count_unreadable".into(),
                usage: None,
            },
        },
        Call::Completion => Answer::Completed {
            body: raw.to_vec(),
            usage,
        },
    }
}

/// What a transport error means for the ledger.
pub fn answer_for_error(error: &ureq::Error) -> Answer {
    if never_left_the_process(error) {
        return Answer::NotSent(reason_for(error).into());
    }
    Answer::Failed {
        reason: reason_for(error).into(),
        usage: None,
    }
}

/// Whether the request body provably never reached the provider.
fn never_left_the_process(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::HostNotFound | ureq::Error::BadUri(_) | ureq::Error::InvalidProxyUrl => true,
        // The resolver and the connect both run before a byte of the
        // request is written. Everything later may have been received.
        ureq::Error::Timeout(ureq::Timeout::Resolve | ureq::Timeout::Connect) => true,
        ureq::Error::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::AddrNotAvailable
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::HostUnreachable
        ),
        _ => false,
    }
}

/// A **typed** reason. The error's own text is a stranger's and is dropped.
fn reason_for(error: &ureq::Error) -> &'static str {
    match error {
        ureq::Error::HostNotFound => "host_not_found",
        ureq::Error::BadUri(_) => "endpoint_unusable",
        ureq::Error::InvalidProxyUrl => "proxy_unusable",
        ureq::Error::Timeout(_) => "timed_out",
        ureq::Error::Io(_) => "connection_failed",
        ureq::Error::StatusCode(_) => "provider_status",
        ureq::Error::Protocol(_) => "protocol_error",
        ureq::Error::RedirectFailed => "redirect_refused",
        _ => "transport_error",
    }
}
