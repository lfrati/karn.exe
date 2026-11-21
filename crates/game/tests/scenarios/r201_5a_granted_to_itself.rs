//! An ability an object gives itself names it by its name (201.5a): Kellan, Planar
//! Trailblazer's "Whenever Kellan deals combat damage to a player" is about that Kellan.
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const KELLAN: usize = 0;
const ISLAND: usize = 1;
const BEAR: usize = 2;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let kellan = CardDef::from_face(faces.iter().find(|f| f.name == "Kellan, Planar Trailblazer").unwrap()).unwrap();
    game::play::check(&kellan).unwrap();
    let defs = vec![
        kellan,
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..9 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn activate(g: &mut Game, source: ObjId, ability: usize, mana: &str) {
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    g.act(0, paid(mana,Move::Activate { source, ability, targets: vec![], paying: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// Attack player 1 with these, unblocked, through combat damage and its triggers.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while g.part != ir::Part::PostcombatMain {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

#[test]
fn r201_5a_kellan_triggers_on_its_own_combat_damage_once_it_has_given_itself_the_ability() {
    let mut g = game();
    let kellan = g.put(0, KELLAN, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    activate(&mut g, kellan, 0,"CR");
    assert!(g.seen(kellan, None).subtypes.iter().any(|s| s.name() == "Detective"));
    // the Bear's damage isn't Kellan's
    let library = g.players[0].library.len();
    attack(&mut g, &[kellan, bear]);
    cite!(["201.5a"]; assert_eq!(g.players[0].library.len(), library - 1, "one card exiled, for Kellan alone"));
    let exiled = *g.exile.last().unwrap();
    assert_eq!(g.obj(exiled).chars.name, "Island");
    // "You may play that card this turn."
    g.act(0, Move::PlayLand(exiled)).unwrap();
    assert_eq!(g.obj(*g.battlefield.last().unwrap()).chars.name, "Island");
}

#[test]
fn r201_5a_without_the_ability_or_a_scout_nothing_triggers_and_the_second_step_needs_a_detective() {
    let mut g = game();
    let kellan = g.put(0, KELLAN, Zone::Battlefield);
    // not a Detective yet: the second ability does nothing
    activate(&mut g, kellan, 1,"CCR");
    assert_eq!(g.power(kellan), Some(2));
    let library = g.players[0].library.len();
    attack(&mut g, &[kellan]);
    assert_eq!(g.players[0].library.len(), library);
    assert_eq!(g.players[1].life, 18);
}

#[test]
fn r201_5a_the_rogue_step_keeps_the_given_trigger_and_strikes_twice() {
    let mut g = game();
    let kellan = g.put(0, KELLAN, Zone::Battlefield);
    activate(&mut g, kellan, 0,"CR");
    activate(&mut g, kellan, 1,"CCR");
    assert_eq!((g.power(kellan), g.toughness(kellan)), (Some(3), Some(2)));
    assert!(g.has(kellan, "Double strike"));
    // the first ability again: no longer a Scout, nothing happens
    activate(&mut g, kellan, 0,"CR");
    assert!(g.seen(kellan, None).subtypes.iter().any(|s| s.name() == "Rogue"));
    let library = g.players[0].library.len();
    attack(&mut g, &[kellan]);
    cite!(["702.4b"]; assert_eq!(g.players[1].life, 14));
    cite!(["201.5a", "702.4b"]; assert_eq!(g.players[0].library.len(), library - 2, "each combat damage step's damage triggers it"));
}
