//! Scavenge (702.97a): "[Cost], Exile this card from your graveyard: Put a number of +1/+1
//! counters equal to the power of the card you exiled on target creature. Activate only as a
//! sorcery." It functions only while the card is in a graveyard. Every card is read from its
//! Oracle text.
use crate::support;
use support::*;

/// Activate the scavenge ability of the card at `source`, its first ability that is activated.
fn scavenge(t: &mut T, source: ObjId, to: ObjId, mana: &str) -> Result<(), String> {
    let ability = t.g.abilities(source).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("scavenge");
    t.g.act(0, paid(mana, Move::Activate { source, ability, targets: vec![vec![Target::Object(to)]], paying: vec![] }))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Deadbridge Goliath", "Slitherhead", "Boneyard Mycodrax"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_97a_it_exiles_the_card_and_puts_counters_equal_to_its_power() {
    // Deadbridge Goliath: 5/5, "Scavenge {4}{G}{G}"
    let mut t = Board::new(2).named(0, Zone::Graveyard, "goliath", "Deadbridge Goliath").battlefield(0, &["Grizzly Bears"]).build();
    let (goliath, bears) = (t.id("goliath"), t.find_in_battlefield("Grizzly Bears"));
    t.mana(0, "{4}{G}{G}");
    cite!(["702.97a", "602.2"]; ok: scavenge(&mut t, goliath, bears, "CCCCGG"));
    // exiling it is part of the cost: it is gone before the ability resolves (601.2h, 602.2b)
    t.expect().rules(["702.97a", "602.2b"]).zone_exact(0, Zone::Exile, &["Deadbridge Goliath"]).zone_exact(0, Zone::Graveyard, &[]).check();
    t.resolve();
    t.expect().rules(["702.97a", "122.1a"]).counters("Grizzly Bears", "+1/+1", 5).pt("Grizzly Bears", 7, 7).check();
}

#[test]
fn r702_97a_it_is_activated_only_as_a_sorcery() {
    let mut t = Board::new(2).fill(5).named(0, Zone::Graveyard, "goliath", "Deadbridge Goliath").battlefield(0, &["Grizzly Bears"]).build();
    let (goliath, bears) = (t.id("goliath"), t.find_in_battlefield("Grizzly Bears"));
    // in another player's main phase, with that player passing to it and the stack empty
    t.to_no_combat(1, Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{4}{G}{G}");
    let before = t.state();
    cite!(["702.97a", "602.5d"]; refused("only as a sorcery"): scavenge(&mut t, goliath, bears, "CCCCGG"));
    // Uncited (implementation contract): a refused activation changes nothing.
    assert_eq!(t.state(), before);
}

#[test]
fn r702_97a_it_functions_only_in_a_graveyard() {
    // Slitherhead: "Scavenge {0}"; on the battlefield or in a hand the ability does nothing
    let mut t = Board::new(2).named(0, Zone::Hand, "held", "Slitherhead").named(0, Zone::Battlefield, "out", "Slitherhead").build();
    let (held, out) = (t.id("held"), t.id("out"));
    cite!(["702.97a", "113.6m"]; refused("functions only in the zone"): scavenge(&mut t, held, out, ""));
    cite!(["702.97a", "113.6m"]; refused("functions only in the zone"): scavenge(&mut t, out, out, ""));
}

#[test]
fn r702_97a_the_power_is_the_cards_as_it_last_existed_in_the_graveyard() {
    // Boneyard Mycodrax: "Boneyard Mycodrax's power and toughness are each equal to the number
    // of other creature cards in your graveyard." Ruling: "The number of counters given by
    // Boneyard Mycodrax's scavenge ability is its power as it last existed in your graveyard
    // before exiling it." With two other creature cards there it gives two.
    let mut t = Board::new(2).named(0, Zone::Graveyard, "drax", "Boneyard Mycodrax")
        .graveyard(0, &["Llanowar Elves", "Savannah Lions", "Forest"]).battlefield(0, &["Grizzly Bears"]).build();
    let (drax, bears) = (t.id("drax"), t.find_in_battlefield("Grizzly Bears"));
    // its characteristic-defining ability functions in the graveyard (604.3)
    cite!(["604.3"]; assert_eq!(t.g.power(drax), Some(2)));
    t.mana(0, "{4}{B}");
    cite!(["702.97a", "604.3"]; ok: scavenge(&mut t, drax, bears, "CCCCB"));
    t.resolve();
    t.expect().rules(["702.97a", "608.2h"]).counters("Grizzly Bears", "+1/+1", 2).check();
}
