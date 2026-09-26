//! The gate for the wire.

use super::*;

#[test]
fn each_dialect_round_trips_its_name() {
    for dialect in Dialect::ALL {
        assert_eq!(Dialect::parse(dialect.name()), Some(dialect));
    }
    assert_eq!(Dialect::parse("grpc"), None);
}

#[test]
fn every_dialect_a_launch_can_name_is_one_the_published_figures_price() {
    // **A published worst case is the most over `Dialect::ALL`**, and a
    // launch configures its dialect by name, so every name a configuration
    // may use has to be in that list, once: a dialect a launch could serve
    // and the arithmetic never priced is a bound that does not bound it.
    // The three names are the ones the owner's configurations use.
    for name in ["responses", "openai", "anthropic"] {
        let dialect = Dialect::parse(name).unwrap_or_else(|| panic!("`{name}` is refused"));
        assert_eq!(
            Dialect::ALL
                .iter()
                .filter(|listed| **listed == dialect)
                .count(),
            1,
            "`{name}` is configurable and priced other than once"
        );
    }
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
