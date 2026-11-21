//! Revolt reuses the announced target and remembers control at battlefield departure.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::simulation::Simulation;
use std::sync::OnceLock;

fn game() -> Game {
    static DEFS: OnceLock<Vec<CardDef>> = OnceLock::new();
    let defs = DEFS.get_or_init(|| {
        let mut ds = vec![CardDef::new("Fatal Push", "{B}", "Instant", "Destroy target creature if it has mana value 2 or less.\nRevolt — Destroy that creature if it has mana value 4 or less instead if a permanent left the battlefield under your control this turn.", None).unwrap()];
        for n in 1..=5 { ds.push(CardDef::new(&format!("Creature {n}"), &format!("{{{n}}}"), "Creature — Bear", "", Some((2, 2))).unwrap()); }
        ds.push(CardDef::new("Departure", "{0}", "Instant", "Exile target permanent.", None).unwrap());
        ds.push(CardDef::new("Relic", "{0}", "Artifact", "", None).unwrap());
        ds.push(CardDef::new("Steal", "{0}", "Instant", "Gain control of target creature.", None).unwrap());
        for d in &ds { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
        ds
    });
    let mut g = Game::new(defs.clone(), 3, 20);
    for p in 0..3 { for _ in 0..4 { g.put(p, 7, Zone::Library); } }
    g
}
fn cast(g: &mut Game, by: usize, index: usize, target: ObjId, spending: &str) {
    while g.priority != by { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[by].pool.colors[2] = 1;
    let spell = g.put(by, index, Zone::Hand);
    g.act(by, paid(spending, Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
}
fn resolve(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn depart(g: &mut Game, whose: usize) {
    let relic = g.put(whose, 7, Zone::Battlefield);
    cast(g, 0, 6, relic, ""); resolve(g);
}
#[test]
fn r608_2c_fatal_push_reuses_one_target_and_both_mana_value_guards() {
    for revolt in [false, true] { for value in 1..=5 {
        let mut g = game();
        if revolt { depart(&mut g, 0); }
        let creature = g.put(1, value, Zone::Battlefield);
        cast(&mut g, 0, 0, creature, "B"); resolve(&mut g);
        cite!(["608.2c"]; case: format!("revolt={revolt}, value={value}"); assert_eq!(!g.battlefield.contains(&creature), value <= if revolt { 4 } else { 2 }, "revolt={revolt}, value={value}"));
    } }
}
#[test]
fn r608_2h_revolt_uses_departure_controller_not_owner_or_another_players_departure() {
    let mut g = game();
    depart(&mut g, 2);
    let creature = g.put(1, 4, Zone::Battlefield);
    cast(&mut g, 0, 0, creature, "B"); resolve(&mut g);
    cite!(["608.2i"]; assert!(g.battlefield.contains(&creature)));
    let borrowed = g.put(2, 1, Zone::Battlefield);
    cast(&mut g, 0, 8, borrowed, ""); resolve(&mut g);
    assert_eq!(g.controller(borrowed), 0);
    cast(&mut g, 0, 6, borrowed, ""); resolve(&mut g);
    g.collect_departed();
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        cast(g, 0, 0, creature, "B"); resolve(g);
        cite!(["608.2i"]; assert!(!g.battlefield.contains(&creature)));
    }
}
#[test]
fn r608_2c_revolt_is_checked_at_resolution_including_departures_after_casting() {
    let mut g = game();
    let creature = g.put(1, 4, Zone::Battlefield);
    cast(&mut g, 0, 0, creature, "B");
    depart(&mut g, 0); // resolves above Fatal Push
    resolve(&mut g);
    cite!(["608.2i"]; assert!(!g.battlefield.contains(&creature)));
}
#[test]
fn r500_7_revolt_departures_expire_at_the_next_turn() {
    let mut g = game(); depart(&mut g, 0);
    let turn = g.turn;
    while g.turn == turn { g.act(g.priority, Move::Pass).unwrap(); }
    let creature = g.put(1, 4, Zone::Battlefield);
    cast(&mut g, 0, 0, creature, "B"); resolve(&mut g);
    cite!(["608.2i"]; assert!(g.battlefield.contains(&creature)));
}
#[test]
fn r608_2b_a_legal_high_mana_target_resolves_without_destruction_through_simulation() {
    let mut g = game(); let creature = g.put(1, 5, Zone::Battlefield);
    cast(&mut g, 0, 0, creature, "B");
    let mut sim = Simulation::new(g);
    for _ in 0..3 {
        let r = sim.request().unwrap().unwrap().clone();
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    cite!(["608.2b", "608.2c"]; assert!(sim.game().unwrap().battlefield.contains(&creature)));
}

#[test]
fn r608_2i_revolt_remembers_the_departure_and_releases_the_object_when_the_turn_ends() {
    // what left is remembered as it last was while "this turn" can ask about it (608.2i);
    // once the turn has ended nothing keeps it
    let mut g=game();
    let relic=g.put(0,7,Zone::Battlefield);
    g.move_object(relic,Zone::Exile);
    g.act(0,Move::Pass).unwrap();
    g.retire_events(g.log.end());
    g.collect_departed();
    let creature=g.put(1,4,Zone::Battlefield);
    cast(&mut g, 0, 0, creature, "B");resolve(&mut g);
    cite!(["608.2i"]; assert!(!g.battlefield.contains(&creature)));
    let turn=g.turn;
    while g.turn==turn { g.act(g.priority,Move::Pass).unwrap(); }
    g.retire_events(g.log.end());
    g.collect_departed();
    assert!(g.get_object(relic).is_none(),"the turn's history no longer roots its former object");
}
