//! "Whenever a creature dealt damage by this creature this turn dies": a leaves-the-battlefield
//! trigger asking what the creature was dealt damage by this turn, as it last existed (603.10a,
//! 120.1). Sengir Vampire; Bold Defense for an "instead" that leads the sentence (608.2c).
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;
use crate::support::{cite, paid};

const SENGIR: usize = 0;
const BEAR: usize = 1;
const GIANT: usize = 2;
const BITE: usize = 3;
const MURDER: usize = 4;
const DEFENSE: usize = 5;
const ISLAND: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Sengir Vampire"),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Giant", "Creature — Giant", "", Some((7, 7))),
        c("Bite", "Instant", "Target creature you control deals damage equal to its power to target creature you don't control.", None),
        c("Murder", "Instant", "Destroy target creature.", None),
        real("Bold Defense"),
        c("Island", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, Move::Cast { card: spell, targets }).unwrap();
    resolve(g);
}
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.values().sum() }

#[test]
fn r603_10a_what_this_creature_dealt_damage_to_dies() {
    let mut g = game();
    let sengir = g.put(0, SENGIR, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, BITE, vec![vec![Target::Object(sengir)], vec![Target::Object(bear)]]);
    cite!(["603.10a"]; assert_eq!(counters(&g, sengir), 1));
}

#[test]
fn r603_10a_only_what_it_dealt_damage_to_and_only_this_turn() {
    let mut g = game();
    let sengir = g.put(0, SENGIR, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let other = g.put(1, GIANT, Zone::Battlefield);
    // the Giant survives the damage, then dies later this turn
    cast(&mut g, BITE, vec![vec![Target::Object(sengir)], vec![Target::Object(giant)]]);
    assert_eq!(counters(&g, sengir), 0);
    cast(&mut g, MURDER, vec![vec![Target::Object(giant)]]);
    cite!(["603.10a"]; assert_eq!(counters(&g, sengir), 1));
    // a creature it did not deal damage to
    cast(&mut g, MURDER, vec![vec![Target::Object(bear)]]);
    assert_eq!(counters(&g, sengir), 1);
    // damage dealt last turn is not dealt this turn
    cast(&mut g, BITE, vec![vec![Target::Object(sengir)], vec![Target::Object(other)]]);
    let mut t = crate::support::T::new(g);
    t.to_no_combat(1, Part::PrecombatMain);
    let mut g = t.g;
    let murder = g.put(1, MURDER, Zone::Hand);
    g.players[1].pool.colors = [9; 5];
    g.act(1, Move::Cast { card: murder, targets: vec![vec![Target::Object(other)]] }).unwrap();
    resolve(&mut g);
    assert!(g.players[1].graveyard.iter().filter(|o| g.obj(**o).card == Some(GIANT)).count() == 2);
    cite!(["603.10a"]; assert_eq!(counters(&g, sengir), 1));
}

#[test]
fn r608_2c_bold_defense_kicked_gives_the_second_sentence_instead() {
    for kicked in [false, true] {
        let mut g = game();
        let bear = g.put(0, BEAR, Zone::Battlefield);
        let spell = g.put(0, DEFENSE, Zone::Hand);
        g.players[0].pool.colors = [9; 5];
        let cast = Move::Cast { card: spell, targets: vec![] };
        g.act(0, paid(if kicked { "WWWWWWW" } else { "WWW" }, if kicked { Move::WithKicks(1, Box::new(cast)) } else { cast })).unwrap();
        resolve(&mut g);
        let seen = g.seen(bear, None);
        cite!(["608.2c"]; case: format!("kicked: {kicked}"); assert_eq!(seen.pt, Some(if kicked { (4, 4) } else { (3, 3) }), "kicked: {kicked}"));
        cite!(["608.2c"]; case: format!("kicked: {kicked}"); assert_eq!(g.has(bear, "First Strike"), kicked));
    }
}
