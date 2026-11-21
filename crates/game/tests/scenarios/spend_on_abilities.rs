//! Mana that may be spent only to activate abilities named by what they are ("equip
//! abilities", 106.6, 702): the grammar reads it; the engine restricts mana by ability
//! source, but does not yet restrict it by keyword, so that form is refused.
use game::CardDef;

#[test]
fn r106_6_mana_only_for_keyword_abilities_is_refused() {
    let text = "{T}: Add {R}. Spend this mana only to activate equip abilities.";
    let d = CardDef::new("Procedure", "{1}", "Artifact", text, None).unwrap();
    let refused = game::play::check(&d).map_err(|u| u.0).expect_err(text);
    assert_eq!(refused, "activation selector: unsupported ability family or source relation");
    let plain = CardDef::new("Procedure", "{1}", "Artifact", "{T}: Add {R}.", None).unwrap();
    assert!(game::play::check(&plain).is_ok());
}
