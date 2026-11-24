//! "No counters on it" is a count of zero (122.1): a permanent with counters of that kind does
//! not have none, so a condition or description asking for none is false of it.
use crate::support::{Board, cite, mana_pool};
use game::{Move, Zone};

#[test]
fn r122_1_damning_verdict_spares_a_creature_with_a_counter() {
    // "Destroy all creatures with no counters on them."
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Hill Giant"]).counters("Hill Giant", "+1/+1", 1)
        .hand(0, &["Damning Verdict"]).build();
    t.mana(0, "{3}{W}{W}");
    cite!(["601.2"]; ok: t.cast(0, "Damning Verdict", vec![], mana_pool("{3}{W}{W}")));
    t.resolve();
    t.expect().rules(["122.1", "701.8a"])
        .zone_exact(0, Zone::Battlefield, &["Hill Giant"])
        .zone_exact(0, Zone::Graveyard, &["Grizzly Bears", "Damning Verdict"]).check();
}

#[test]
fn r122_1_dark_depths_with_ice_counters_does_not_trigger() {
    // "When Dark Depths has no ice counters on it, sacrifice it. If you do, create Marit Lage."
    let mut t = Board::new(2).battlefield(0, &["Dark Depths"]).counters("Dark Depths", "ice", 2).build();
    t.g.act(0, Move::Pass).unwrap();
    cite!(["603.8", "122.1"]; assert!(t.g.stack.is_empty(), "two ice counters are not none"));
    t.g.act(1, Move::Pass).unwrap();
    t.expect().rules(["603.8"]).zone_exact(0, Zone::Battlefield, &["Dark Depths"]).counters("Dark Depths", "ice", 2).check();
}

#[test]
fn r603_8_dark_depths_triggers_once_its_last_ice_counter_is_removed() {
    let mut t = Board::new(2).battlefield(0, &["Dark Depths"]).counters("Dark Depths", "ice", 1).build();
    t.mana(0, "{3}");
    cite!(["602.2"]; ok: t.activate(0, "Dark Depths", 1, vec![], mana_pool("{3}")));
    t.resolve();
    t.expect().rules(["603.8", "122.1"])
        .zone_exact(0, Zone::Graveyard, &["Dark Depths"])
        .zone_exact(0, Zone::Battlefield, &["Marit Lage"]).check();
}
