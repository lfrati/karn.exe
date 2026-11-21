//! Earthbend (701.66a): "Target land you control becomes a 0/0 land creature with haste in
//! addition to its other types. Put N +1/+1 counters on it. When that land dies or is put into
//! exile, return it to the battlefield tapped under your control."
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::CardType;

const BENDER: usize = 0;
const FOREST: usize = 1;
const EXILED: usize = 2;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Bender", "{0}", "Sorcery", "Earthbend 2.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Exiled", "{0}", "Creature — Spirit", "When this creature is put into exile, you gain 3 life.", Some((1, 1))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, FOREST, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// After an event made outside a spell: the triggers it caused are put on the stack the next
/// time a player would receive priority (603.3), and resolve.
fn after(g: &mut Game) {
    g.act(g.priority, Move::Pass).unwrap();
    settle(g);
}
fn earthbend(g: &mut Game, land: ObjId) {
    let spell = g.put(0, BENDER, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(land)]] }).unwrap();
    settle(g);
}
fn forests(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Forest").collect() }

#[test]
fn r701_66a_the_land_becomes_a_hasty_creature_with_counters() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    earthbend(&mut g, land);
    cite!(["701.66a"]; assert!(g.is_a(land, CardType::Creature) && g.is_a(land, CardType::Land)));
    cite!(["701.66a"]; assert_eq!((g.power(land), g.toughness(land)), (Some(2), Some(2))));
    cite!(["701.66a"]; assert!(g.has(land, "Haste")));
    // only a land you control is a legal target
    let theirs = g.put(1, FOREST, Zone::Battlefield);
    let spell = g.put(0, BENDER, Zone::Hand);
    cite!(["115.1"]; refused("an illegal target, Forest: the target is one you control, and another player controls it (115.1)"): g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(theirs)]] }));
}

#[test]
fn r701_66a_when_it_dies_it_returns_tapped_as_a_new_land() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    earthbend(&mut g, land);
    g.destroy(land);
    after(&mut g);
    let back = forests(&g);
    cite!(["701.66a"]; assert_eq!(back.len(), 1));
    let back = back[0];
    cite!(["400.7"]; assert_ne!(back, land));
    cite!(["701.66a", "400.7"]; assert!(g.obj(back).tapped && !g.is_a(back, CardType::Creature) && g.obj(back).counters.is_empty()));
    cite!(["701.66a"]; assert_eq!(g.controller(back), 0));
}

#[test]
fn r701_66a_when_it_is_exiled_it_returns_under_your_control() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    earthbend(&mut g, land);
    g.move_object(land, Zone::Exile);
    after(&mut g);
    let back = forests(&g);
    cite!(["701.66a"]; assert_eq!(back.len(), 1));
    cite!(["701.66a"]; assert!(g.obj(back[0]).tapped));
    // returned once: leaving again does nothing
    g.destroy(back[0]);
    after(&mut g);
    cite!(["603.7c"]; assert!(forests(&g).is_empty()));
}

#[test]
fn r701_66a_returning_to_hand_is_neither_dying_nor_exile() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    earthbend(&mut g, land);
    g.move_object(land, Zone::Hand);
    after(&mut g);
    cite!(["701.66a", "700.4"]; assert!(forests(&g).is_empty()));
}

/// "is put into exile", of a permanent: from the battlefield, not from another zone.
#[test]
fn r603_6c_put_into_exile_from_the_battlefield_triggers_and_from_the_graveyard_does_not() {
    let mut g = game();
    let spirit = g.put(0, EXILED, Zone::Battlefield);
    g.move_object(spirit, Zone::Exile);
    after(&mut g);
    cite!(["603.6c"]; assert_eq!(g.players[0].life, 23));
    let dead = g.put(0, EXILED, Zone::Graveyard);
    g.move_object(dead, Zone::Exile);
    after(&mut g);
    cite!(["603.6c"]; assert_eq!(g.players[0].life, 23));
}
