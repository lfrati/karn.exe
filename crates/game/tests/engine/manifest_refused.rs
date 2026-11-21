//! Manifest and cloak (701.40, 701.58, 701.62): the grammar reads a card from any zone; the
//! engine manifests or cloaks the top cards of a library, a card from the player's own hand, or
//! cards just looked at, so a card from elsewhere is refused, naming the action. Manifesting
//! dread a number of times only known as the spell resolves is refused too.
use game::CardDef;

#[test]
fn r701_40a_manifesting_or_cloaking_a_card_from_another_zone_is_refused() {
    for text in ["Manifest a card from your graveyard.", "Cloak a card from your graveyard.", "Manifest dread X times."] {
        let d = CardDef::new("Procedure", "{X}", "Sorcery", text, None).unwrap();
        let refused = game::play::check(&d).map_err(|u| u.0).expect_err(text);
        let named = refused.to_lowercase();
        assert!(named.contains("manifest") || named.contains("cloak") || named.contains("times"), "{text}: {refused}");
    }
    for text in ["Manifest the top card of your library.", "Manifest a card from your hand.", "Cloak a card from your hand.", "Cloak the top card of your library.", "Manifest dread."] {
        let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    }
}
