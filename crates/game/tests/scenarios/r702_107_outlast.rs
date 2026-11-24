//! Outlast (702.107a): "Outlast [cost]" means "[Cost], {T}: Put a +1/+1 counter on this
//! creature. Activate only as a sorcery." Every card is read from its Oracle text.
use crate::support;
use support::*;
use ir::{Ability, CostPart};

/// The index of its outlast ability among its abilities as it is now: the activated one
/// whose cost has {T}.
fn outlast(t: &T, label: &str) -> usize {
    let o = t.id(label);
    t.g.abilities(o).iter().position(|a| matches!(&**a, Ability::Activated { cost, .. } if cost.0.contains(&CostPart::Tap)))
        .expect("an outlast ability")
}
fn activate(t: &mut T, label: &str, mana: &str) -> Result<(), String> {
    let (source, ability) = (t.id(label), outlast(t, label));
    t.g.act(0, paid(mana, Move::Activate { source, ability, targets: vec![], paying: vec![] }))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Abzan Falconer", "Ainok Bond-Kin", "Salt Road Patrol"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_107a_outlast_taps_and_puts_a_counter_on_it() {
    // Abzan Falconer: "Outlast {W}. Each creature you control with a +1/+1 counter on it has
    // flying."
    let mut t = Board::new(2).named(0, Zone::Battlefield, "falconer", "Abzan Falconer").build();
    let falconer = t.id("falconer");
    cite!(["702.107a"]; assert!(!t.g.has(falconer, "Flying")));
    t.mana(0, "{W}");
    cite!(["702.107a", "602.2"]; ok: activate(&mut t, "falconer", "W"));
    t.expect().rules(["702.107a", "602.2b"]).tapped("Abzan Falconer", true).counters("Abzan Falconer", "+1/+1", 0).stack(&["Abzan Falconer"]).check();
    t.resolve();
    t.expect().rules(["702.107a"]).counters("Abzan Falconer", "+1/+1", 1).pt("Abzan Falconer", 3, 4).check();
    cite!(["702.107a", "613.1f"]; assert!(t.g.has(falconer, "Flying"), "a creature with a +1/+1 counter has flying"));
}

#[test]
fn r702_107a_outlast_costs_its_mana_and_untapped_creature() {
    let mut t = Board::new(2).named(0, Zone::Battlefield, "patrol", "Salt Road Patrol").build();
    // "Outlast {1}{W}": {W} alone does not pay it
    t.mana(0, "{W}");
    cite!(["702.107a", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): activate(&mut t, "patrol", "W"));
    t.mana(0, "{1}");
    let patrol = t.id("patrol");
    t.g.set_tapped(patrol, true);
    cite!(["702.107a", "107.5"]; refused("Salt Road Patrol can't pay {T} as it is (107.5, 118.3)"): activate(&mut t, "patrol", "CW"));
    t.g.set_tapped(patrol, false);
    cite!(["702.107a"]; ok: activate(&mut t, "patrol", "CW"));
    t.resolve();
    t.expect().rules(["702.107a"]).counters("Salt Road Patrol", "+1/+1", 1).pt("Salt Road Patrol", 3, 6).check();
}

#[test]
fn r702_107a_outlast_only_as_a_sorcery() {
    let mut t = Board::new(2).at(Part::BeginningOfCombat).named(0, Zone::Battlefield, "kin", "Ainok Bond-Kin").build();
    t.mana(0, "{1}{W}");
    cite!(["702.107a", "602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"):
        activate(&mut t, "kin", "CW"));
}

#[test]
fn r302_6_a_creature_that_just_arrived_cannot_outlast() {
    let mut t = Board::new(2).named(0, Zone::Hand, "kin", "Ainok Bond-Kin").build();
    t.mana(0, "{1}{W}").cast(0, "kin", vec![], mana_pool("CW")).unwrap();
    t.resolve();
    let kin = t.find_in_battlefield("Ainok Bond-Kin");
    t.bind("arrived", kin);
    t.mana(0, "{1}{W}");
    cite!(["302.6", "602.5a"]; refused("Ainok Bond-Kin has not been under your control since your turn began (602.5a)"): activate(&mut t, "arrived", "CW"));
}

#[test]
fn r702_107a_the_counter_gives_another_creature_first_strike() {
    // Ainok Bond-Kin: "Outlast {1}{W}. Each creature you control with a +1/+1 counter on it
    // has first strike."
    let t = Board::new(2).named(0, Zone::Battlefield, "kin", "Ainok Bond-Kin")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "plain", "Llanowar Elves")
        .counters("bears", "+1/+1", 1).build();
    cite!(["613.1f"]; assert!(t.g.has(t.id("bears"), "First Strike") && !t.g.has(t.id("plain"), "First Strike") && !t.g.has(t.id("kin"), "First Strike")));
}
