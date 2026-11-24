//! A static ability whose subject is the object it is on reads "it" in its condition as that
//! object: "This creature gets +0/+3 as long as it's untapped." It is not locked in, and
//! applies at any moment to what its text says then (611.3a). Every card is read from its
//! Oracle text.
use crate::support;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Giant Tortoise", "Kor Scythemaster", "Manor Gargoyle"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r611_3a_it_is_the_creature_and_the_condition_is_read_each_moment() {
    // Giant Tortoise, 1/1: "This creature gets +0/+3 as long as it's untapped."
    let mut t = Board::new(2).battlefield(0, &["Giant Tortoise"]).build();
    let tortoise = t.find_in_battlefield("Giant Tortoise");
    t.expect().rules(["611.3a", "613.4c"]).pt("Giant Tortoise", 1, 4).check();
    t.g.tap(tortoise);
    t.expect().rules(["611.3a"]).pt("Giant Tortoise", 1, 1).check();
}

#[test]
fn r611_3a_it_has_first_strike_while_it_is_attacking() {
    // Kor Scythemaster: "This creature has first strike as long as it's attacking."
    let mut t = Board::new(2).fill(5).battlefield(0, &["Kor Scythemaster"]).build();
    let scythemaster = t.find_in_battlefield("Kor Scythemaster");
    cite!(["611.3a"]; assert!(!t.g.has(scythemaster, "First strike")));
    while t.g.decision != Some(Decision::Attackers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Attack(vec![(scythemaster, Target::Player(1))])).unwrap();
    cite!(["611.3a", "508.1k"]; assert!(t.g.has(scythemaster, "First strike")));
}

#[test]
fn r611_3a_it_is_indestructible_while_it_has_defender() {
    // Manor Gargoyle: "Defender / This creature has indestructible as long as it has
    // defender. / {1}: Until end of turn, this creature loses defender and gains flying."
    let mut t = Board::new(2).battlefield(0, &["Manor Gargoyle"]).build();
    let gargoyle = t.find_in_battlefield("Manor Gargoyle");
    cite!(["611.3a"]; assert!(t.g.has(gargoyle, "Indestructible")));
    let ability = t.g.abilities(gargoyle).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("its activated ability");
    t.mana(0, "{1}");
    t.g.act(0, paid("C", Move::Activate { source: gargoyle, ability, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    cite!(["611.3a", "613.1f"]; assert_eq!((t.g.has(gargoyle, "Defender"), t.g.has(gargoyle, "Flying"), t.g.has(gargoyle, "Indestructible")), (false, true, false)));
}
