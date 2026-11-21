//! The worked examples of rule 613 (interaction of continuous effects), with real cards.
//! Where the example names an unsupported card or a generic effect, a real card with the same
//! effect stands in, and the comment says so.
use crate::support;
use support::*;

/// 613.5, first example: Honor of the Pure; a 2/2 black creature turned white gets +1/+1
/// (layers 5 then 7c), and back to 2/2 once it's turned red.
/// Cloudchaser Kestrel's "{W}: Target permanent becomes white until end of turn" and Incite's
/// "Target creature becomes red until end of turn" are the color effects; Walking Corpse is
/// the 2/2 black creature.
#[test]
fn r613_5_example_honor_of_the_pure_follows_the_creatures_current_color() {
    let mut t = Board::new(2).battlefield(0, &["Honor of the Pure", "Walking Corpse", "Cloudchaser Kestrel"]).hand(0, &["Incite"]).build();
    t.expect().pt("Walking Corpse", 2, 2).check();
    t.mana(0, "{W}").activate(0, "Cloudchaser Kestrel", 2, vec!["Walking Corpse".into()], crate::support::mana_pool("W")).unwrap();
    t.resolve();
    t.expect().rules(["613.5"]).pt("Walking Corpse", 3, 3).check();
    t.mana(0, "{R}").cast(0, "Incite", vec!["Walking Corpse".into()], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    t.expect().rules(["613.5"]).pt("Walking Corpse", 2, 2).check();
}

/// 613.5, second example: Gray Ogre with a +1/+1 counter (3/3), "+4/+4 until end of turn"
/// (7/7), "+0/+2" for creatures you control (7/9), then "becomes 0/1" (layer 7b): 5/8.
/// Titanic Growth is the +4/+4 spell; Shield Wall ("Creatures you control get +0/+2 until end
/// of turn") stands in for the enchantment, also layer 7c; Flatline ("Creatures your opponents
/// control have base power and toughness 0/1 until end of turn") is the 0/1 effect.
#[test]
fn r613_5_example_gray_ogre_sets_base_power_below_its_modifications() {
    let mut t = Board::new(2).battlefield(0, &["Gray Ogre"]).counters("Gray Ogre", "+1/+1", 1)
        .hand(0, &["Titanic Growth", "Shield Wall"]).hand(1, &["Flatline"]).build();
    t.expect().pt("Gray Ogre", 3, 3).check();
    t.mana(0, "{1}{G}").cast(0, "Titanic Growth", vec!["Gray Ogre".into()], crate::support::mana_pool("CG")).unwrap();
    t.resolve();
    t.expect().pt("Gray Ogre", 7, 7).check();
    t.mana(0, "{1}{W}").cast(0, "Shield Wall", vec![], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    t.expect().rules(["613.4c"]).pt("Gray Ogre", 7, 9).check();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{2}{U}").cast(1, "Flatline", vec![], crate::support::mana_pool("CCU")).unwrap();
    t.resolve();
    t.expect().rules(["613.5", "613.4b"]).pt("Gray Ogre", 5, 8).check();
}
