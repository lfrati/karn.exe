//! Monstrosity (701.37a): "If this permanent isn't monstrous, put N +1/+1 counters on it and it
//! becomes monstrous." Monstrous is a designation the permanent keeps until it leaves the
//! battlefield (701.37b); "when this creature becomes monstrous" triggers as it does.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Zone};

const LION: usize = 0;
const DRAGON: usize = 1;
const ISLAND: usize = 2;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        real("Fleecemane Lion"),
        real("Stormbreath Dragon"),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn monstrosity(g: &mut Game, source: ObjId, ability: usize, spending: &str) {
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    g.act(0, crate::support::paid(spending, Move::Activate { source, ability, targets: vec![], paying: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r701_37a_monstrosity_happens_once_and_its_designation_is_read() {
    let mut g = game();
    let lion = g.put(0, LION, Zone::Battlefield);
    assert!(!g.has(lion, "Indestructible"));
    monstrosity(&mut g, lion, 0, "CCCGW");
    cite!(["701.37a"]; assert_eq!(plus(&g, lion), 1));
    cite!(["701.37a"]; assert!(g.obj(lion).monstrous));
    // "As long as this creature is monstrous, it has hexproof and indestructible."
    cite!(["701.37b"]; assert!(g.has(lion, "Indestructible") && g.has(lion, "Hexproof")));
    // already monstrous: no counters
    monstrosity(&mut g, lion, 0, "CCCGW");
    cite!(["701.37a"]; assert_eq!(plus(&g, lion), 1));
}

#[test]
fn r701_37b_a_permanent_that_leaves_and_returns_is_a_new_object_that_isnt_monstrous() {
    let mut g = game();
    let lion = g.put(0, LION, Zone::Battlefield);
    monstrosity(&mut g, lion, 0, "CCCGW");
    g.move_object(lion, Zone::Hand);
    let card = *g.players[0].hand.last().unwrap();
    g.move_object(card, Zone::Battlefield);
    let back = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(back).chars.name, "Fleecemane Lion");
    cite!(["701.37b", "400.7"]; assert!(!g.obj(back).monstrous && !g.has(back, "Indestructible")));
    monstrosity(&mut g, back, 0, "CCCGW");
    cite!(["701.37a"]; assert_eq!(plus(&g, back), 1));
}

#[test]
fn r603_2e_becoming_monstrous_triggers_once() {
    let mut g = game();
    let dragon = g.put(0, DRAGON, Zone::Battlefield);
    for _ in 0..3 { g.put(1, ISLAND, Zone::Hand); }
    let hand = g.players[1].hand.len() as i32;
    monstrosity(&mut g, dragon, 1, "CCCCCRR");
    cite!(["701.37a"]; assert_eq!(plus(&g, dragon), 3));
    cite!(["603.2"]; assert_eq!(g.players[1].life, 20 - hand));
    // it doesn't become monstrous again, so nothing triggers
    monstrosity(&mut g, dragon, 1, "CCCCCRR");
    cite!(["701.37a", "603.2e"]; assert_eq!((plus(&g, dragon), g.players[1].life), (3, 20 - hand)));
}
