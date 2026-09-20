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
fn each_dialect_lives_under_its_own_base_on_the_one_pinned_host() {
    // The two wires of one provider, recorded in ADR 001 question 3. They
    // are paths under a constant host rather than endpoints, so there is
    // nothing here for a configuration to point elsewhere.
    assert_eq!(Dialect::OpenAi.base(), "/v1");
    assert_eq!(Dialect::Anthropic.base(), "/anthropic");
    for dialect in [Dialect::OpenAi, Dialect::Anthropic] {
        assert!(dialect.base().starts_with('/'));
        assert!(!dialect.base().contains("//"), "not a host in disguise");
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
