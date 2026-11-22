//! Keyword triggers reuse event participants, declaration batches, and temporary effects.
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn fixture(cards: Vec<CardDef>) -> Game {
    let mut g = Game::new(cards, 2, 20);
    g.set_ask_forced(true);
    g
}
fn attack(g: &mut Game, attackers: &[ObjId]) {
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() || matches!(g.decision, Some(Decision::Order { .. })) {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("unexpected question {d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn blockers(g: &mut Game, blocks: Vec<(ObjId, ObjId)>) {
    while g.part != Part::DeclareBlockers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Block(blocks)).unwrap();
    resolve(g);
}

#[test]
fn r702_83_exalted_pumps_the_lone_attacker_once_per_instance() {
    let mut g = fixture(vec![oracle("Akrasan Squire"), oracle("Grizzly Bears")]);
    g.put(0, 0, Zone::Battlefield);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    attack(&mut g, &[bear]);
    resolve(&mut g);
    cite!(["702.83a", "506.5"]; assert_eq!(g.power(bear), Some(4)));
    let mut g = fixture(vec![oracle("Akrasan Squire"), oracle("Grizzly Bears")]);
    let squire = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    attack(&mut g, &[squire, bear]);
    cite!(["702.83b"]; assert!(g.stack.is_empty()));
    cite!(["702.83a"]; assert_eq!(g.power(bear), Some(2)));
}
