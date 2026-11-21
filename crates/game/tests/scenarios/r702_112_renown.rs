//! Renown (702.112a): "When this creature deals combat damage to a player, if it isn't
//! renowned, put N +1/+1 counters on it and it becomes renowned." Renowned is a designation
//! kept until it leaves the battlefield (702.112b); each instance triggers separately and only
//! the first to resolve does anything (702.112c).
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BLADE: usize = 0;
const TWICE: usize = 1;
const ISLAND: usize = 2;
const WALL: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        real("Topan Freeblade"),
        CardDef::new("Twice", "{0}", "Creature — Soldier", "Renown 1\nRenown 2", Some((1, 1))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "Defender", Some((0, 8))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..9 { g.put(p, ISLAND, Zone::Library); } }
    g
}
/// Attack player 1 with this creature on player 0's next turn, blocked by `blocker` if any,
/// and play out the turn.
fn combat(g: &mut Game, attacker: ObjId, blocker: Option<ObjId>) {
    while g.decision != Some(Decision::Attackers { player: 0 }) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(Decision::Discard { .. }) => panic!("discard"),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    let turn = g.turn;
    while g.turn == turn {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(blocker.map(|b| vec![(b, attacker)]).unwrap_or_default())).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r702_112a_combat_damage_to_a_player_makes_it_renowned_once() {
    let mut g = game();
    let blade = g.put(0, BLADE, Zone::Battlefield);
    let wall = g.put(1, WALL, Zone::Battlefield);
    // blocked: no damage to a player, no renown
    combat(&mut g, blade, Some(wall));
    cite!(["702.112a"]; assert!(!g.obj(blade).renowned));
    combat(&mut g, blade, None);
    cite!(["702.112a"]; assert!(g.obj(blade).renowned));
    cite!(["702.112a"]; assert_eq!((plus(&g, blade), g.players[1].life), (1, 18)));
    combat(&mut g, blade, None);
    cite!(["702.112a", "603.4"]; assert_eq!((plus(&g, blade), g.players[1].life), (1, 15), "already renowned: no more counters"));
}

#[test]
fn r702_112c_of_two_instances_only_the_first_to_resolve_does_anything() {
    let mut g = game();
    let twice = g.put(0, TWICE, Zone::Battlefield);
    combat(&mut g, twice, None);
    cite!(["702.112c"]; assert!(g.obj(twice).renowned));
    let n = plus(&g, twice);
    cite!(["702.112c"]; assert!(n == 1 || n == 2, "one instance's counters, not both: {n}"));
}
