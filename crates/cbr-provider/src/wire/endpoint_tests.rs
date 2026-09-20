//! The gate for the host pin.
//!
//! **"MiniMax only" is a label until something checks the host.** Every
//! case below sends the owner's credential and repository text somewhere,
//! and each one passed configuration before this existed.

use super::Dialect;
use super::endpoint::*;
use crate::model::Call;

#[test]
fn the_pinned_host_is_the_one_the_owner_named() {
    assert_eq!(HOST, "api.minimax.io");
    assert_eq!(
        pinned("https://api.minimax.io/v1/chat/completions", HOST),
        Ok(())
    );
}

#[test]
fn another_host_entirely_is_refused() {
    assert_eq!(
        pinned("https://collector.example/v1", HOST),
        Err(NotPinned::Host)
    );
}

#[test]
fn a_lookalike_host_is_refused() {
    // `api.minimax.io.collector.example` starts with the pinned host and
    // is a different host. A prefix check accepts it; that is the mutant.
    assert_eq!(
        pinned("https://api.minimax.io.collector.example/v1", HOST),
        Err(NotPinned::Host)
    );
    assert_eq!(
        pinned("https://not-api.minimax.io/v1", HOST),
        Err(NotPinned::Host)
    );
}

#[test]
fn the_userinfo_form_is_refused() {
    // `https://api.minimax.io@collector.example/v1` addresses
    // **collector.example**, with `api.minimax.io` as a username. It reads
    // like the pinned host to everything that does not parse it.
    assert_eq!(
        pinned("https://api.minimax.io@collector.example/v1", HOST),
        Err(NotPinned::Userinfo)
    );
    assert_eq!(
        pinned("https://user:pass@api.minimax.io/v1", HOST),
        Err(NotPinned::Userinfo)
    );
}

#[test]
fn a_port_other_than_the_default_is_refused() {
    assert_eq!(
        pinned("https://api.minimax.io:8443/v1", HOST),
        Err(NotPinned::Port)
    );
    // The default, written out, is the same endpoint.
    assert_eq!(pinned("https://api.minimax.io:443/v1", HOST), Ok(()));
}

#[test]
fn a_fragment_is_refused() {
    assert_eq!(
        pinned("https://api.minimax.io/v1#collector.example", HOST),
        Err(NotPinned::Fragment)
    );
}

#[test]
fn anything_but_https_is_refused() {
    for url in [
        "http://api.minimax.io/v1",
        "ftp://api.minimax.io/v1",
        "//api.minimax.io/v1",
        "api.minimax.io/v1",
    ] {
        assert_eq!(pinned(url, HOST), Err(NotPinned::Scheme), "{url}");
    }
}

#[test]
fn a_host_that_differs_only_in_case_or_a_trailing_dot_is_the_same_host() {
    // DNS is case-insensitive and a single trailing dot is the root label
    // written out, so these **are** the pinned host rather than lookalikes
    // of it. Refusing them would be a false refusal; accepting anything
    // that merely normalises *towards* it would not be.
    assert_eq!(pinned("https://API.MINIMAX.IO/v1", HOST), Ok(()));
    assert_eq!(pinned("https://api.minimax.io./v1", HOST), Ok(()));
    // And the normalisation does not reach further than that.
    assert_eq!(
        pinned("https://api.minimax.io../v1", HOST),
        Err(NotPinned::Host)
    );
    assert_eq!(
        pinned("https://api\u{2024}minimax.io/v1", HOST),
        Err(NotPinned::Host),
        "a one-dot-leader is not a dot"
    );
}

#[test]
fn a_backslash_or_a_control_character_in_the_authority_is_refused() {
    // Some parsers treat a backslash as a separator and some do not, which
    // is how one component's idea of the host differs from another's.
    for url in [
        "https://api.minimax.io\\@collector.example/v1",
        "https://api.minimax.io\u{0000}.collector.example/v1",
        "https://api.minimax.io\t/v1",
    ] {
        assert!(pinned(url, HOST).is_err(), "{url:?}");
    }
}

#[test]
fn every_url_the_transport_builds_is_pinned() {
    // The two halves joined: what the code constructs passes what the code
    // checks, for both dialects and both calls.
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        for call in [Call::Count, Call::Completion] {
            let url = url_to_send(dialect, call).expect("its own URL is pinned");
            assert_eq!(pinned(url.as_str(), HOST), Ok(()), "{url:?}");
            assert!(
                url.as_str().starts_with("https://api.minimax.io/"),
                "{url:?}"
            );
        }
    }
    // And the path is the dialect's, not configuration's.
    assert_eq!(
        url_to_send(Dialect::OpenAi, Call::Completion)
            .unwrap()
            .as_str(),
        "https://api.minimax.io/v1/chat/completions"
    );
    assert_eq!(
        url_to_send(Dialect::Anthropic, Call::Completion)
            .unwrap()
            .as_str(),
        "https://api.minimax.io/anthropic/v1/messages"
    );
    assert_eq!(
        url_to_send(Dialect::OpenAi, Call::Count).unwrap().as_str(),
        "https://api.minimax.io/v1/responses/input_tokens"
    );
}

#[test]
fn every_refusal_has_a_reason_and_none_of_them_names_the_host_it_refused() {
    // The reason reaches an item and a log. A refused endpoint is an
    // operator's mistake or an attacker's URL, and neither belongs there.
    for refusal in [
        NotPinned::Scheme,
        NotPinned::Userinfo,
        NotPinned::Host,
        NotPinned::Port,
        NotPinned::Fragment,
        NotPinned::Malformed,
    ] {
        assert!(!refusal.reason().is_empty());
        assert!(!refusal.reason().contains('.'), "{}", refusal.reason());
    }
}
