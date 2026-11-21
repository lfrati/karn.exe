//! 208.5: "If a creature somehow has no value for its power, its power is 0. The same is
//! true for toughness." A noncreature permanent with no printed power and toughness has none
//! (208.3); when an effect makes it a creature without setting them, it is a 0/0, and what
//! modifies power and toughness, counters included, modifies that 0 (613.4c). With toughness
//! 0 it is put into its owner's graveyard (704.5f). Each card is a test text read by the
//! compiler; the rule, not a printed card, is what is under test.
use crate::support::cite;
use crate::support;
use support::*;

type Text = (&'static str, &'static str, &'static str, &'static str, Option<(i32, i32)>);
const HUSK: Text = ("Husk", "{0}", "Artifact",
    "{0}: This artifact becomes an artifact creature until end of turn.\n{0}: Put a +1/+1 counter on this artifact.", None);
const ANTHEM: Text = ("Anthem", "{0}", "Enchantment", "Creatures you control get +1/+1.", None);

/// Each library holds ten Anthems, so that nobody draws from an empty one.
fn game() -> Game {
    let defs: Vec<CardDef> = [HUSK, ANTHEM].iter().map(|(name, cost, types, text, pt)| {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        d
    }).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 1, Zone::Library); } }
    g
}
fn activate(g: &mut Game, husk: ObjId, ability: usize) {
    g.act(0, Move::Activate { source: husk, ability, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}
fn creature(g: &Game, o: ObjId) -> bool { g.is_a(o, ir::CardType::Creature) }

#[test]
fn r208_5_a_creature_with_no_value_for_power_and_toughness_is_a_0_0_and_dies_of_it() {
    let mut g = game();
    let husk = g.put(0, 0, Zone::Battlefield);
    // a noncreature permanent has no power or toughness (208.3)
    cite!(["208.3"]; assert_eq!((g.power(husk), g.toughness(husk)), (None, None)));
    activate(&mut g, husk, 0);
    // it became a creature with no power or toughness set: each is 0 (208.5), and with
    // toughness 0 it is put into its owner's graveyard (704.5f)
    cite!(["208.5", "704.5f"]; assert_eq!(g.obj(husk).zone, Zone::Gone));
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r208_5_what_modifies_power_and_toughness_modifies_the_0() {
    let mut g = game();
    let husk = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    // the anthem's +1/+1 does nothing to a noncreature (208.3a)
    cite!(["208.3a"]; assert_eq!(g.power(husk), None));
    activate(&mut g, husk, 0);
    // 0/0 (208.5), then +1/+1 in layer 7c (613.4c): a 1/1 that survives
    cite!(["208.5", "613.4c"]; assert!(creature(&g, husk)));
    cite!(["208.5", "613.4c"]; assert_eq!((g.power(husk), g.toughness(husk)), (Some(1), Some(1))));
    assert_eq!(g.obj(husk).zone, Zone::Battlefield);
}

#[test]
fn r208_5_counters_put_on_a_noncreature_modify_the_0_once_it_is_a_creature() {
    let mut g = game();
    let husk = g.put(0, 0, Zone::Battlefield);
    activate(&mut g, husk, 1);
    activate(&mut g, husk, 1);
    // a +1/+1 counter on a noncreature permanent does nothing to it (208.3, 122.1a)
    cite!(["208.3"]; assert_eq!(g.power(husk), None));
    activate(&mut g, husk, 0);
    // 0/0 (208.5), +2/+2 from two +1/+1 counters (613.4c, 122.1a)
    cite!(["208.5", "613.4c"]; assert_eq!((g.power(husk), g.toughness(husk)), (Some(2), Some(2))));
    // until end of turn: a noncreature again, with no power or toughness (208.3, 514.2)
    let mut t = T::new(g);
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["208.3", "514.2"]; assert_eq!((creature(&t.g, husk), t.g.power(husk)), (false, None)));
}
