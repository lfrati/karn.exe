//! The worked examples of rule 608 (resolving spells and abilities), with real cards.
use crate::support;
use support::*;

/// 608.2j: "Destroy all black creatures" destroys a white-and-black creature (Cleanse; Tithe
/// Drinker is white and black). The example's other half, "Destroy all nonblack creatures",
/// uses Hellfire, including its operation-local death count.
#[test]
fn r608_2j_example_destroy_all_black_creatures_destroys_a_white_and_black_creature() {
    let mut t = Board::new(2).hand(0, &["Cleanse"]).battlefield(1, &["Tithe Drinker", "Walking Corpse", "White Knight"]).build();
    t.mana(0, "{2}{W}{W}").cast(0, "Cleanse", vec![], crate::support::mana_pool("CCWW")).unwrap();
    let a=t.find_in_battlefield("Tithe Drinker");let b=t.find_in_battlefield("Walking Corpse");
    while t.g.decision.is_none() && !t.g.stack.is_empty(){t.g.act(t.g.priority,Move::Pass).unwrap();}
    cite!(["404.3"]; ok: t.g.act(1,Move::Choose(Answer::Objects(vec![a,b]))));
    t.resolve();
    t.expect().rules(["608.2j"])
        .zone(1, Zone::Battlefield, &["White Knight"])
        .zone(1, Zone::Graveyard, &["Tithe Drinker", "Walking Corpse"])
        .check();
}

#[test]
fn r608_2j_example_nonblack_does_not_include_white_and_black() {
    let mut t = Board::new(2).hand(0, &["Hellfire"]).battlefield(1, &["Tithe Drinker", "Walking Corpse", "White Knight"]).build();
    t.mana(0, "{2}{B}{B}{B}").cast(0, "Hellfire", vec![], crate::support::mana_pool("CCBBB")).unwrap();
    t.resolve();
    t.expect().holds(|g| g.players[0].life == 16, "one actual death adds to the three mandatory damage")
        .rules(["608.2j"]).zone(1, Zone::Battlefield, &["Tithe Drinker", "Walking Corpse"])
        .zone(1, Zone::Graveyard, &["White Knight"]).check();
}
