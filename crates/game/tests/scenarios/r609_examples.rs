//! The worked examples of rule 609 (effects), with real cards.
use crate::support::cite;
use crate::support;
use support::*;

/// 609.2, first half: an effect that makes all lands creatures (Living Lands: "All Forests are
/// 1/1 creatures that are still lands") doesn't change Forest cards in a graveyard, so
/// "Return target creature card from your graveyard" (Raise Dead) can't target one.
#[test]
fn r609_2_example_living_lands_leaves_graveyard_forests_noncreatures() {
    let mut t = Board::new(2).battlefield(0, &["Living Lands", "Forest"]).graveyard(0, &["Forest"]).hand(0, &["Raise Dead"]).build();
    t.expect().rules(["609.2"]).pt("Forest", 1, 1).check();
    let in_graveyard = t.find_in(0, Zone::Graveyard, "Forest");
    let spell = t.find_in(0, Zone::Hand, "Raise Dead");
    t.mana(0, "{B}");
    let tried = t.g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(in_graveyard)]] });
    cite!(["609.2"]; refused("an illegal target, Forest: it isn't a creature (115.1)"): tried);
}

/// 609.2, second half: "spells cost more" (Sphere of Resistance: "Spells cost {1} more to
/// cast") applies to the spell being cast, which is on the stack.
#[test]
fn r609_2_example_a_cost_increase_applies_to_the_spell_being_cast() {
    let mut t = Board::new(2).battlefield(1, &["Sphere of Resistance"]).hand(0, &["Grizzly Bears"]).build();
    t.mana(0, "{1}{G}");
    cite!(["609.2", "601.2f"]; refused("that mana is not in your mana pool (118.3a)"): t.cast(0, "Grizzly Bears", vec![], crate::support::mana_pool("CCG")));
    cite!(["609.2", "601.2f"]; ok: t.mana(0, "{1}").cast(0, "Grizzly Bears", vec![], crate::support::mana_pool("CCG")));
    t.resolve();
    t.expect().contains(0, Zone::Battlefield, "Grizzly Bears").check();
}
