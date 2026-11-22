//! Dash's haste and delayed return belong to its alternative casting announcement.
use crate::support::{cite, mana_pool, oracle, paid};
use game::announcement::CastCost;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

fn fixture(dash: bool) -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Mardu Scout"), oracle("Mountain"), oracle("Torpor Orb")], 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, 1, Zone::Library); } }
    g.put(0, 2, Zone::Battlefield);
    let scout = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("RR");
    let cast = Move::Cast { card: scout, targets: vec![] };
    let cast = if dash { Move::CastFor(CastCost::Alternative(0), Box::new(cast)) } else { cast };
    g.act(0, paid("RR", cast)).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let scout = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    (g, scout)
}
fn end_step(g: &mut Game) {
    for _ in 0..100 {
        if g.part == Part::End { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no end step");
}
#[test]
fn r702_109a_dash_haste_allows_attacking_and_return_is_not_an_enters_trigger() {
    let (mut g, scout) = fixture(true);
    cite!(["702.109a"]; assert!(g.has(scout, "Haste")));
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.109a", "302.6"]; ok: g.act(0, Move::Attack(vec![(scout, Target::Player(1))])));
    end_step(&mut g);
    cite!(["702.109a", "603.7"]; assert_eq!(g.stack.len(), 1));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.109a"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(0))));
}
#[test]
fn r702_109a_normal_cast_has_no_dash_haste_or_return() {
    let (mut g, scout) = fixture(false);
    cite!(["702.109a"]; assert!(!g.has(scout, "Haste")));
    end_step(&mut g);
    cite!(["702.109a"]; assert!(g.stack.is_empty()));
    cite!(["702.109a"]; assert!(g.battlefield.contains(&scout)));
}
#[test]
fn r400_7_dash_return_does_not_follow_a_departed_permanent() {
    let (mut g, scout) = fixture(true);
    g.move_object(scout, Zone::Exile);
    let other = g.put(0, 0, Zone::Battlefield);
    end_step(&mut g);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["400.7", "702.109a"]; assert!(g.battlefield.contains(&other)));
    cite!(["400.7"]; assert!(g.players[0].hand.is_empty()));
}
