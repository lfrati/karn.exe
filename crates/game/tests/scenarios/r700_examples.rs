//! The worked examples of rule 700 (general keyword-action rules), with real cards.
use crate::support;
use support::*;

/// 700.7: "Target creature gets +2/+2 until end of turn. Destroy that creature at the
/// beginning of the next end step" destroys the same object even if it isn't a creature then.
/// Mogg Cannon ("{T}: Target creature you control gets +1/+0 and gains flying until end of
/// turn. Destroy that creature at the beginning of the next end step") targets a Forest that
/// Living Lands makes a creature; Naturalize then destroys Living Lands, so at the end step
/// the Forest is only a land, and is still destroyed.
#[test]
fn r700_7_example_the_delayed_destruction_follows_the_object_not_the_creature() {
    let mut t = Board::new(2).battlefield(0, &["Living Lands", "Forest", "Mogg Cannon"]).hand(0, &["Naturalize"]).build();
    t.activate(0, "Mogg Cannon", 0, vec!["Forest".into()], crate::support::mana_pool("")).unwrap();
    t.resolve();
    t.expect().rules(["613.4b", "613.4c"]).pt("Forest", 2, 1).check();
    t.mana(0, "{1}{G}").cast(0, "Naturalize", vec!["Living Lands".into()], crate::support::mana_pool("CG")).unwrap();
    t.resolve();
    let forest = t.find_in_battlefield("Forest");
    assert!(!t.g.seen(forest, None).types.contains(&ir::CardType::Creature));
    t.to_no_combat(0, Part::End);
    t.resolve();
    t.expect().rules(["700.7"]).zone(0, Zone::Battlefield, &["Mogg Cannon"]).contains(0, Zone::Graveyard, "Forest").check();
}
