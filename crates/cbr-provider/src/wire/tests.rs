//! The gate for the wire.

use super::*;

#[test]
fn each_dialect_round_trips_its_name() {
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        assert_eq!(Dialect::parse(dialect.name()), Some(dialect));
    }
    assert_eq!(Dialect::parse("grpc"), None);
}

#[test]
fn both_endpoints_are_the_owners_and_both_are_https() {
    // The two wires of one provider, recorded in ADR 001 question 3.
    assert_eq!(
        Dialect::OpenAi.default_endpoint(),
        "https://api.minimax.io/v1"
    );
    assert_eq!(
        Dialect::Anthropic.default_endpoint(),
        "https://api.minimax.io/anthropic"
    );
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        assert!(dialect.default_endpoint().starts_with("https://"));
    }
}

#[test]
fn the_three_models_are_the_ones_the_owner_named() {
    assert_eq!(
        MODELS,
        ["MiniMax-M2.7-highspeed", "MiniMax-M2.7", "MiniMax-M3"]
    );
    assert_eq!(PROVIDER_ID, "minimax");
}
