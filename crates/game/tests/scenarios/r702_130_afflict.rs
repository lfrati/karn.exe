//! Afflict triggers once when blocked and keeps the attacked permanent's defending player.
use crate::support::{cite, oracle};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::Decision;
fn resolve(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn block(g: &mut Game, source: ObjId, defender: Target, blockers: Vec<ObjId>) {
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(source,defender)])).unwrap();
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Blockers { player: 1 })) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(1, Move::Block(blockers.into_iter().map(|b|(b,source)).collect())).unwrap();
}
#[test]
fn r702_130a_multiple_blockers_trigger_once_and_source_departure_does_not_stop_life_loss() {
    let mut g = Game::new(vec![oracle("Spellweaver Eternal"),oracle("Grizzly Bears")],3,20);
    let source=g.put(0,0,Zone::Battlefield);
    let blockers=(0..2).map(|_|g.put(1,1,Zone::Battlefield)).collect();
    block(&mut g,source,Target::Player(1),blockers);
    cite!(["702.130a","509.3c"]; assert_eq!(g.stack.len(),1));
    g.destroy(source);
    resolve(&mut g);
    cite!(["702.130a","113.7a"]; assert_eq!(g.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![20,18,20]));
}
#[test]
fn r702_130a_a_planeswalkers_controller_is_the_defending_player() {
    let mut g=Game::new(vec![oracle("Khenra Eternal"),oracle("Grizzly Bears"),oracle("Jace Beleren")],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    let blocker=g.put(1,1,Zone::Battlefield);
    let walker=g.put(1,2,Zone::Battlefield); g.put_counters(walker,"loyalty",3);
    block(&mut g,source,Target::Object(walker),vec![blocker]);
    resolve(&mut g);
    cite!(["702.130a","508.5"]; assert_eq!(g.players[1].life,19));
    assert!(g.battlefield.contains(&walker));
}
