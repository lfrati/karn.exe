//! Sengir Vampire that left the battlefield and came back is a new object
//! (400.7): it dealt no damage this turn, so what the old one damaged dying gives it nothing.
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;
use crate::support::cite;

const SENGIR: usize = 0;
const GIANT: usize = 1;
const BITE: usize = 2;
const MURDER: usize = 3;
const FLICKER: usize = 4;
const ISLAND: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.name == "Sengir Vampire").unwrap()).unwrap(),
        c("Giant", "Creature — Giant", "", Some((7, 7))),
        c("Bite", "Instant", "Target creature you control deals damage equal to its power to target creature you don't control.", None),
        c("Murder", "Instant", "Destroy target creature.", None),
        c("Flicker", "Instant", "Exile target creature, then return it to the battlefield under its owner's control.", None),
        c("Island", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, Move::Cast { card: spell, targets }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.values().sum() }

#[test]
fn a_flickered_sengir_did_not_deal_the_old_ones_damage() {
    let mut g = game();
    let sengir = g.put(0, SENGIR, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, BITE, vec![vec![Target::Object(sengir)], vec![Target::Object(giant)]]);
    cast(&mut g, FLICKER, vec![vec![Target::Object(sengir)]]);
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(SENGIR)).unwrap();
    assert_ne!(back, sengir);
    cast(&mut g, MURDER, vec![vec![Target::Object(giant)]]);
    assert_eq!(g.obj(giant).zone, Zone::Gone);
    cite!(["400.7"]; assert_eq!(counters(&g, back), 0));
}
