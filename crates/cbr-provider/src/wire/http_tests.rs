//! The gate for the transport.
//!
//! **Nothing here opens a socket.** The transport is decomposed so that
//! every decision it makes — which URL, which agent settings, what a status
//! and a body mean, what an error means — is a function that can be
//! answered without one. What is left is the glue, and the glue cannot run
//! at all without a permit that no test has.

use std::time::Duration;

use super::Dialect;
use super::http::*;
use crate::model::{Answer, Call};

const OPENAI: &str = "https://api.minimax.io/v1";
const ANTHROPIC: &str = "https://api.minimax.io/anthropic";
const COUNT: &str = "https://api.minimax.io/v1/responses/input_tokens";

#[test]
fn a_transport_cannot_be_built_without_a_permit() {
    // Stated rather than asserted, because what is being claimed is the
    // absence of an API: `Http::new` takes a `net::Permit`, whose only
    // source is `net::permit()`, which answers `None` until the launch
    // opens the gate. This test is the reminder that the line below is
    // what enforces it, and `wire::net::tests` holds the assertions.
    assert!(
        crate::wire::net::permit().is_none(),
        "a test could build a transport"
    );
}

#[test]
fn each_call_goes_to_the_endpoint_it_belongs_to() {
    // The completion goes to the dialect's own path under the configured
    // endpoint. **The count does not**: admission for both dialects goes
    // to MiniMax's own counting endpoint, which is not part of either
    // compatibility surface.
    assert_eq!(
        url_for(OPENAI, COUNT, Dialect::OpenAi, Call::Completion),
        "https://api.minimax.io/v1/chat/completions"
    );
    assert_eq!(
        url_for(ANTHROPIC, COUNT, Dialect::Anthropic, Call::Completion),
        "https://api.minimax.io/anthropic/v1/messages"
    );
    // Both endpoints, because the OpenAI one is a prefix of the counting
    // endpoint: checking only that pair lets a transport that derives the
    // count URL from the dialect's endpoint pass, which a mutant proved.
    for (endpoint, dialect) in [(OPENAI, Dialect::OpenAi), (ANTHROPIC, Dialect::Anthropic)] {
        assert_eq!(
            url_for(endpoint, COUNT, dialect, Call::Count),
            COUNT,
            "{}: the count is not derived from the dialect's endpoint",
            dialect.name()
        );
    }
}

#[test]
fn a_trailing_slash_on_the_endpoint_does_not_double_the_separator() {
    assert_eq!(
        url_for(
            "https://api.minimax.io/v1/",
            COUNT,
            Dialect::OpenAi,
            Call::Completion
        ),
        "https://api.minimax.io/v1/chat/completions"
    );
}

#[test]
fn the_agent_ignores_the_environments_proxy() {
    // **The environment is not allowed to redirect a request carrying the
    // owner's credential.** ureq reads `HTTP_PROXY` and friends by default,
    // which would let anything that can set a variable in this process's
    // environment receive the `Authorization` header.
    //
    // The variable has to be *set* for this to test anything. Without it
    // the default configuration has no proxy either, so the assertion held
    // whether or not the code asked for one — which a mutant proved by
    // surviving the first version of this test.
    //
    // SAFETY: single-threaded setup around one agent construction. No
    // other test in this suite reads these variables.
    unsafe { std::env::set_var("HTTPS_PROXY", "http://proxy.invalid:8080") };
    unsafe { std::env::set_var("HTTP_PROXY", "http://proxy.invalid:8080") };
    let built = agent(Duration::from_secs(30));
    let from_environment = ureq::Proxy::try_from_env();
    unsafe { std::env::remove_var("HTTPS_PROXY") };
    unsafe { std::env::remove_var("HTTP_PROXY") };
    assert!(
        from_environment.is_some(),
        "the variable was readable, so the assertion below means something"
    );
    assert!(
        built.config().proxy().is_none(),
        "the agent took a proxy from the environment"
    );
}

#[test]
fn the_agent_does_not_turn_an_error_status_into_an_error() {
    // This provider reports failures in the body, and a 429's body is
    // where its own account of the exhaustion is. Letting the client turn
    // the status into an error throws that away before it is read.
    let agent = agent(Duration::from_secs(30));
    assert!(!agent.config().http_status_as_error());
}

#[test]
fn the_agent_carries_the_timeout_it_was_given() {
    let agent = agent(Duration::from_secs(7));
    assert_eq!(
        agent.config().timeouts().global,
        Some(Duration::from_secs(7))
    );
}

// --- what a response means ----------------------------------------------

const TEXT: &[u8] = br#"{"choices":[{"message":{"role":"assistant","content":"hello"},
"finish_reason":"stop"}],"usage":{"total_tokens":1222}}"#;

#[test]
fn a_completion_carries_the_body_and_the_cost_the_provider_reported() {
    let answer = answer_for(Call::Completion, Dialect::OpenAi, 200, TEXT);
    assert_eq!(
        answer,
        Answer::Completed {
            body: TEXT.to_vec(),
            usage: Some(1222),
        }
    );
}

#[test]
fn a_completion_the_provider_did_not_price_says_so_rather_than_saying_zero() {
    // The ledger settles an unpriced completion at the reservation's
    // estimate. A transport that reports zero here turns that into a spend
    // of nothing against a shared quota.
    let unpriced = br#"{"choices":[{"message":{"content":"hi"},"finish_reason":"stop"}]}"#;
    let answer = answer_for(Call::Completion, Dialect::OpenAi, 200, unpriced);
    assert!(
        matches!(answer, Answer::Completed { usage: None, .. }),
        "{answer:?}"
    );
}

#[test]
fn a_count_carries_the_number_and_a_count_that_cannot_be_read_does_not() {
    assert_eq!(
        answer_for(
            Call::Count,
            Dialect::OpenAi,
            200,
            br#"{"total_tokens":1180}"#
        ),
        Answer::Counted(1180)
    );
    // Unreadable, so the local estimate has to stand: reported as a failure
    // after the send, which settles at the estimate rather than at nothing.
    assert!(
        matches!(
            answer_for(Call::Count, Dialect::OpenAi, 200, br#"{"n_tokens":1180}"#),
            Answer::Failed { usage: None, .. }
        ),
        "an unreadable count is not a count"
    );
}

#[test]
fn an_exhausted_quota_is_reported_as_exhaustion_from_the_status_or_the_body() {
    // Both routes, because this provider uses both: HTTP 429, and an HTTP
    // 200 whose body carries the code.
    assert_eq!(
        answer_for(Call::Completion, Dialect::OpenAi, 429, b"{}"),
        Answer::ProviderExhausted
    );
    assert_eq!(
        answer_for(
            Call::Completion,
            Dialect::OpenAi,
            200,
            br#"{"base_resp":{"status_code":1008,"status_msg":"insufficient balance"}}"#
        ),
        Answer::ProviderExhausted
    );
    assert_eq!(
        answer_for(
            Call::Completion,
            Dialect::Anthropic,
            200,
            br#"{"type":"error","error":{"type":"rate_limit_error","message":"x"}}"#
        ),
        Answer::ProviderExhausted
    );
}

#[test]
fn an_error_status_is_a_failure_after_the_send_and_keeps_any_usage_it_reported() {
    // **After the send**, so the reservation settles at what the provider
    // said it charged — or at the estimate when it said nothing. Settling
    // to zero here is the defect the m4a review found.
    let answer = answer_for(
        Call::Completion,
        Dialect::OpenAi,
        500,
        br#"{"usage":{"total_tokens":17}}"#,
    );
    assert!(
        matches!(
            answer,
            Answer::Failed {
                usage: Some(17),
                ..
            }
        ),
        "{answer:?}"
    );
    let silent = answer_for(Call::Completion, Dialect::OpenAi, 500, b"nonsense");
    assert!(
        matches!(silent, Answer::Failed { usage: None, .. }),
        "{silent:?}"
    );
}

#[test]
fn a_failure_never_carries_the_providers_own_words() {
    let answer = answer_for(
        Call::Completion,
        Dialect::OpenAi,
        500,
        br#"{"error":{"message":"TRANSPORT-CANARY"}}"#,
    );
    assert!(!format!("{answer:?}").contains("CANARY"), "{answer:?}");
}

// --- what an error means -------------------------------------------------

#[test]
fn an_error_before_the_body_went_out_is_reported_as_not_sent() {
    // `NotSent` means **nothing was charged**, so it is claimed only where
    // the request body provably never reached the provider.
    for error in [
        ureq::Error::HostNotFound,
        ureq::Error::BadUri("no scheme".into()),
        ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        ureq::Error::Timeout(ureq::Timeout::Resolve),
        ureq::Error::Timeout(ureq::Timeout::Connect),
    ] {
        assert!(
            matches!(answer_for_error(&error), Answer::NotSent(_)),
            "{error:?} should be a call that never happened"
        );
    }
}

#[test]
fn an_error_after_the_body_went_out_is_reported_as_a_failure_that_may_have_cost() {
    // The conservative direction. A request that was written and then went
    // wrong is one the provider may well have charged for, and settling it
    // to nothing under-counts a quota shared with the owner's own tools.
    for error in [
        ureq::Error::Timeout(ureq::Timeout::SendBody),
        ureq::Error::Timeout(ureq::Timeout::RecvResponse),
        ureq::Error::Timeout(ureq::Timeout::Global),
        ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)),
        ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
    ] {
        assert!(
            matches!(answer_for_error(&error), Answer::Failed { usage: None, .. }),
            "{error:?} should be a call that may have cost something"
        );
    }
}

#[test]
fn an_errors_own_text_never_reaches_the_answer() {
    let error = ureq::Error::BadUri("ERROR-TEXT-CANARY".into());
    let answer = answer_for_error(&error);
    assert!(!format!("{answer:?}").contains("CANARY"), "{answer:?}");
}
