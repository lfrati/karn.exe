//! 111.10s A Map token is a colorless Map artifact token with "{1}, {T}, Sacrifice this token:
//! Target creature you control explores. Activate only as a sorcery." Created by Fanatical
//! Offering (sacrifice an artifact or creature as an additional cost; draw two cards and
//! create a Map token).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const OFFERING: usize = 0;
const BEAR: usize = 1;
const SPELL: usize = 2;
const ISLAND: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let offering = CardDef::from_face(faces.iter().find(|f| f.name == "Fanatical Offering").unwrap()).unwrap();
    let defs = vec![
        offering,
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Spell", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }) => g.act(0, Move::Choose(Answer::No)).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not resolve");
}
/// Cast Fanatical Offering sacrificing `fodder`; the Map it creates.
fn offering(g: &mut Game, fodder: ObjId) -> ObjId {
    let card = g.put(0, OFFERING, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, Move::WithCosts(vec![vec![fodder]], Box::new(paid("CB", Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g);
    g.log.iter().find_map(|e| if let Event::Created { token } = e { Some(*token) } else { None }).expect("a Map")
}
fn explore(map: ObjId, creature: ObjId) -> Move {
    paid("C", Move::Activate { source: map, ability: 0, targets: vec![vec![Target::Object(creature)]], paying: vec![] })
}
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r111_10s_fanatical_offering_draws_two_and_makes_a_map() {
    let mut g = game();
    let fodder = g.put(0, BEAR, Zone::Battlefield);
    let hand = g.players[0].hand.len();
    let map = offering(&mut g, fodder);
    cite!(["601.2h"]; assert_eq!(g.obj(fodder).zone, Zone::Gone, "sacrificed as it was cast"));
    assert_eq!(g.players[0].hand.len(), hand + 2);
    cite!(["111.10s"]; assert_eq!(g.obj(map).chars.predefined, Some(ir::Predefined::Map)));
    cite!(["111.10s"]; assert!(g.is_a(map, CardType::Artifact) && g.obj(map).chars.colors.is_empty()));
}

#[test]
fn r111_10s_a_map_makes_a_creature_you_control_explore() {
    let mut g = game();
    let fodder = g.put(0, BEAR, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    let map = offering(&mut g, fodder);
    let spell = g.put(0, SPELL, Zone::Library);
    cite!(["118.3"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0, explore(map, bear)));
    g.players[0].pool.colorless = 1;
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one you control, and another player controls it (115.1)"): g.act(0, explore(map, theirs)));
    cite!(["111.10s"]; ok: g.act(0, explore(map, bear)));
    cite!(["602.2b", "601.2h"]; assert_eq!(g.obj(map).zone, Zone::Gone, "sacrificed as a cost"));
    resolve(&mut g);
    // the spell revealed is not a land: a counter, and it stays on top (No)
    cite!(["701.44a"]; assert_eq!(plus(&g, bear), 1));
    cite!(["701.44a"]; assert_eq!(g.players[0].library.last(), Some(&spell)));
}

#[test]
fn r111_10s_a_map_is_activated_only_as_a_sorcery() {
    let mut g = game();
    let fodder = g.put(0, BEAR, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let map = offering(&mut g, fodder);
    // with a spell on the stack
    let spell = g.put(0, SPELL, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.players[0].pool.colorless = 1;
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): g.act(0, explore(map, bear)));
    resolve(&mut g);
    // in combat
    while g.part != Part::BeginningOfCombat { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): g.act(0, explore(map, bear)));
}
