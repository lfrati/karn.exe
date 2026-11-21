//! "[Do something] unless [a player does something else]" (118.12a): the grammar reads what the
//! player may do instead, one act or alternatives joined by "or". One player's single payment
//! is offered (`r118_12_object_payments`); alternatives, and a payment each of several players
//! makes for themselves, are not yet, so the card is refused, naming the modifier.
use game::CardDef;

fn checked(text: &str) -> Result<(), String> {
    let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&d).map_err(|u| u.0)
}

#[test]
fn r118_12a_an_offer_to_do_one_of_several_acts_instead_is_refused() {
    for text in [
        "Each opponent loses 3 life unless they discard a card.",
        "Each opponent loses 3 life unless they discard a card or sacrifice a creature.",
        "You lose 3 life unless you discard a card or sacrifice a creature.",
        "Counter target spell unless its controller pays {B} or {3}.",
    ] {
        let refused = checked(text).expect_err(text);
        assert!(refused.to_lowercase().contains("unless"), "{text}: {refused}");
    }
    assert!(checked("Each opponent loses 3 life.").is_ok());
}
