//! Battle cry (702.91a): "Whenever this creature attacks, each other attacking creature gets
//! +1/+0 until end of turn." Each instance triggers separately (702.91b).
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;
use crate::support::cite;

const HERO: usize = 0;
const BEAR: usize = 1;
const CRIER: usize = 2;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let hero = CardDef::from_face(faces.iter().find(|f| f.name == "Hero of Bladehold").unwrap()).unwrap();
    let defs = vec![
        hero,
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Crier", "{0}", "Creature — Goblin", "Battle cry\nBattle cry", Some((1, 1))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 3, Zone::Library); } }
    g
}
/// Attack player 1 with these; resolve the attack triggers (in listed order) and stop at
/// declare blockers.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while g.part != Part::DeclareBlockers {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

#[test]
fn r702_91a_hero_pumps_each_other_attacker_including_its_tokens() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let home = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[hero, bear]);
    // two Soldier tokens enter tapped and attacking; which trigger resolved first decides
    // whether they got +1/+0, so only the bear and the hero are asserted exactly
    cite!(["702.91a"]; assert_eq!(g.power(bear), Some(3)));
    cite!(["702.91a"]; assert_eq!(g.power(hero), Some(3), "not itself"));
    cite!(["702.91a"]; assert_eq!(g.power(home), Some(2), "not a creature that isn't attacking"));
    assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token() && g.combat.attacking(**o)).count(), 2);
}

#[test]
fn r702_91b_each_instance_triggers_separately() {
    let mut g = game();
    let crier = g.put(0, CRIER, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[crier, bear]);
    cite!(["702.91b"]; assert_eq!(g.power(bear), Some(4)));
    cite!(["702.91a"]; assert_eq!(g.power(crier), Some(1)));
}

#[test]
fn r702_91a_the_bonus_ends_with_the_turn() {
    let mut g = game();
    let crier = g.put(0, CRIER, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[crier, bear]);
    let turn = g.turn;
    while g.turn == turn {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["514.2"]; assert_eq!(g.power(bear), Some(2)));
    cite!(["702.91b"]; assert_eq!(g.players[1].life, 20 - 4 - 1));
}
