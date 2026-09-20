//! Where a call may go, checked by parsing rather than by looking.
//!
//! # "MiniMax only" is a label until something checks the host
//!
//! The owner's decision names one provider. Before this module, a launch
//! could name provider `minimax` and an endpoint at `collector.example`,
//! or at `api.minimax.io.collector.example`, or at
//! `api.minimax.io@collector.example` — and each would have sent the
//! owner's credential and repository text to that host under the MiniMax
//! name. **Configuration no longer supplies an endpoint at all**: the host
//! is a constant, and the path is the dialect's.
//!
//! # And checked again at the last moment
//!
//! [`url_to_send`] both builds the URL and pins it, so the check is where
//! the send is rather than only where the configuration was read. Today
//! the two are the same place and the second check is nearly tautological;
//! that is the point. A future code path that builds a URL some other way
//! reaches the same assertion, and the assertion is what the credential
//! goes past rather than what the configuration went past.
//!
//! # Why the host is compared after a narrow normalisation
//!
//! DNS is case-insensitive and a single trailing dot is the root label
//! written out, so `API.MINIMAX.IO` and `api.minimax.io.` **are** the
//! pinned host. Refusing them would be a false refusal. The normalisation
//! is exactly those two things — ASCII case, and at most one trailing dot
//! — and everything else is compared byte for byte, so nothing that merely
//! *resembles* the host can normalise into it.

#![allow(dead_code)]

use super::Dialect;
use crate::model::Call;

/// The only host CBR ever addresses.
pub const HOST: &str = "api.minimax.io";

/// The path admission counts go to, for both dialects.
pub const COUNT_PATH: &str = "/v1/responses/input_tokens";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPinned {
    /// Not `https`.
    Scheme,
    /// A username or password before the host, which is how a URL is made
    /// to read like one host and address another.
    Userinfo,
    /// Some other host.
    Host,
    /// A port that is not the default.
    Port,
    /// A fragment, which no request carries and which hides text.
    Fragment,
    /// Not a URL this code is willing to take apart.
    Malformed,
}

impl NotPinned {
    /// Typed, and **naming no host**: a refused endpoint is an operator's
    /// mistake or an attacker's URL, and this reason reaches a log.
    pub fn reason(self) -> &'static str {
        match self {
            NotPinned::Scheme => "endpoint_not_https",
            NotPinned::Userinfo => "endpoint_carries_userinfo",
            NotPinned::Host => "endpoint_host_not_pinned",
            NotPinned::Port => "endpoint_port_not_default",
            NotPinned::Fragment => "endpoint_carries_fragment",
            NotPinned::Malformed => "endpoint_malformed",
        }
    }
}

/// Whether `url` addresses `host` over `https` and nothing else.
pub fn pinned(url: &str, host: &str) -> Result<(), NotPinned> {
    let rest = url.strip_prefix("https://").ok_or(NotPinned::Scheme)?;
    // Anything a parser might disagree about is refused rather than
    // interpreted: one component's idea of where the authority ends is how
    // it comes to differ from another's.
    // A backslash is the clearest example: WHATWG's parser treats one in
    // the authority as a separator, so `api.minimax.io\@host/` is the
    // pinned host with an odd path; a parser that does not treat it that
    // way reads `@` as userinfo and addresses `host`. Two components, two
    // hosts, one string. Refused rather than interpreted.
    if url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err(NotPinned::Malformed);
    }
    if url.contains('#') {
        return Err(NotPinned::Fragment);
    }
    let authority = rest.split(['/', '?']).next().ok_or(NotPinned::Malformed)?;
    if authority.contains('@') {
        return Err(NotPinned::Userinfo);
    }
    let named = match authority.split_once(':') {
        // The default port, written out, is the same endpoint.
        Some((named, "443")) => named,
        Some(_) => return Err(NotPinned::Port),
        None => authority,
    };
    if named.is_empty() {
        return Err(NotPinned::Malformed);
    }
    // ASCII case and at most one trailing dot, and nothing else.
    let normalised = named.to_ascii_lowercase();
    let normalised = normalised.strip_suffix('.').unwrap_or(&normalised);
    if normalised != host {
        return Err(NotPinned::Host);
    }
    Ok(())
}

/// A URL that has been through [`pinned`].
///
/// **Its only constructor is [`url_to_send`]**, and its field is private to
/// this module, so the transport cannot hand the request builder a URL that
/// was not checked — the same shape as
/// [`Redacted`](super::redact::Redacted), for the same reason.
///
/// What this does **not** do is make the check killable by a test. The URL
/// is built from constants, so removing the check changes nothing a test
/// can observe; that is reported as a surviving mutant rather than claimed
/// as a kill. What the type buys is that bypassing it is a visible rewrite
/// of the call site rather than a deleted line.
pub struct PinnedUrl(String);

impl PinnedUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for PinnedUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PinnedUrl({})", self.0)
    }
}

/// The URL for a call, built from constants and **checked before it exists
/// as something sendable**.
pub fn url_to_send(dialect: Dialect, call: Call) -> Result<PinnedUrl, NotPinned> {
    let url = match call {
        Call::Count => format!("https://{HOST}{COUNT_PATH}"),
        Call::Completion => format!("https://{HOST}{}{}", dialect.base(), dialect.path()),
    };
    pinned(&url, HOST)?;
    Ok(PinnedUrl(url))
}
