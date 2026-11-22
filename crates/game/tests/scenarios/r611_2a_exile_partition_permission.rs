//! A play permission binds the actual card exiled from a looked-at partition.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Zone};
use game::state::Answer;

#[test]
fn r611_2a_expressive_iteration_permits_only_its_exiled_card() {
    let mut g = Game::new(vec![oracle("Expressive Iteration"), oracle("Island"), oracle("Forest"), oracle("Plains")], 2, 20);
    let white = g.put(0, 3, Zone::Library);
    let green = g.put(0, 2, Zone::Library);
    let blue = g.put(0, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("UR");
    g.act(0, paid("UR", Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Objects(vec![blue]))).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![green]))).unwrap();
    // Exiling the one remaining card needs no additional choice.
    cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    cite!(["400.7"]; assert_eq!(g.obj(white).zone, Zone::Gone));
    let exiled = g.exile[0];
    cite!(["611.2a"]; ok: g.act(0, Move::PlayLand(exiled)));
    cite!(["611.2a"]; assert_eq!(g.obj(g.battlefield[0]).chars.name, "Plains"));
    cite!(["608.2c"]; assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Island"));
    cite!(["608.2c"]; assert_eq!(g.obj(g.players[0].library[0]).chars.name, "Forest"));
}
