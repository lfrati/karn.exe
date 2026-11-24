//! Reinforce (702.77a): "Reinforce N—[cost]" means "[Cost], Discard this card: Put N +1/+1
//! counters on target creature." It functions only while the card is in a hand. Every card is
//! read from its Oracle text.
use crate::support;
use support::*;

/// Activate the reinforce ability of the card at `source`: its first activated ability.
fn reinforce(t: &mut T, p: usize, source: ObjId, to: ObjId, mana: &str, x: Option<u32>) -> Result<(), String> {
    let ability = t.g.abilities(source).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("reinforce");
    let activate = Move::Activate { source, ability, targets: vec![vec![Target::Object(to)]], paying: vec![] };
    t.g.act(p, paid(mana, match x { Some(x) => Move::WithX(x, Box::new(activate)), None => activate }))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Burrenton Bombardier", "Swell of Courage", "Mosquito Guard"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_77a_it_discards_the_card_and_puts_n_counters_at_instant_speed() {
    // Burrenton Bombardier: "Flying. Reinforce 2—{2}{W}"; reinforce is not limited to sorcery
    // timing, so it is activated in another player's turn
    let mut t = Board::new(2).fill(5).named(0, Zone::Hand, "bomber", "Burrenton Bombardier").battlefield(0, &["Grizzly Bears"]).build();
    let (bomber, bears) = (t.id("bomber"), t.find_in_battlefield("Grizzly Bears"));
    t.to_no_combat(1, Part::PrecombatMain);
    // the active player passes: the other may act with the stack empty
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{2}{W}");
    cite!(["702.77a", "602.2"]; ok: reinforce(&mut t, 0, bomber, bears, "CCW", None));
    // discarding it is part of the cost (602.2b)
    t.expect().rules(["702.77a", "701.9a"]).zone_exact(0, Zone::Graveyard, &["Burrenton Bombardier"]).zone_exact(0, Zone::Hand, &[]).check();
    t.resolve();
    t.expect().rules(["702.77a", "122.1a"]).counters("Grizzly Bears", "+1/+1", 2).pt("Grizzly Bears", 4, 4).check();
}

#[test]
fn r702_77a_reinforce_x_puts_x_counters() {
    // Swell of Courage: "Reinforce X—{X}{W}{W}"
    let mut t = Board::new(2).named(0, Zone::Hand, "swell", "Swell of Courage").battlefield(0, &["Grizzly Bears"]).build();
    let (swell, bears) = (t.id("swell"), t.find_in_battlefield("Grizzly Bears"));
    t.mana(0, "{3}{W}{W}");
    cite!(["702.77a", "107.3a"]; ok: reinforce(&mut t, 0, swell, bears, "CCCWW", Some(3)));
    t.resolve();
    t.expect().rules(["702.77a", "107.3a"]).counters("Grizzly Bears", "+1/+1", 3).check();
}

#[test]
fn r702_77a_it_functions_only_in_a_hand() {
    // Mosquito Guard: "First strike. Reinforce 1—{1}{W}"; on the battlefield it has no use
    let mut t = Board::new(2).named(0, Zone::Battlefield, "guard", "Mosquito Guard").build();
    let guard = t.id("guard");
    t.mana(0, "{1}{W}");
    cite!(["702.77a", "113.6m"]; refused("functions only in the zone"): reinforce(&mut t, 0, guard, guard, "CW", None));
}
