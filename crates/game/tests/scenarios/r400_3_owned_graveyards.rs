//! Zone ownership reuses the same named-player relation as ordinary ownership predicates.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r400_3_crime_targets_an_opponents_graveyard_and_enters_under_its_casters_control() {
    let mut g = Game::new(vec![oracle("Crime // Punishment"), oracle("Grizzly Bears")], 3, 20);
    let mine = g.put(0, 1, Zone::Graveyard);
    let theirs = g.put(2, 1, Zone::Graveyard);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CCCWB");
    let cast = |card| paid("CCCWB", Move::CastAs(game::card::CastAs::Half(0), Box::new(Move::Cast { card: spell, targets: vec![vec![Target::Object(card)]] })));
    cite!(["115.1", "400.3"]; refused("an opponent owns"): g.act(0, cast(mine)));
    g.act(0, cast(theirs)).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["400.7"]; assert_eq!(g.obj(theirs).zone, Zone::Gone));
    let bear = g.battlefield[0];
    cite!(["110.2"]; assert_eq!((g.obj(bear).owner, g.controller(bear)), (2, 0)));
    cite!(["400.3"]; assert_eq!(g.players[0].graveyard.len(), 2));
}

#[test]
fn r400_3_relic_lets_the_target_choose_only_from_their_own_graveyard() {
    let mut g = Game::new(vec![oracle("Relic of Progenitus"), oracle("Island"), oracle("Forest")], 3, 20);
    let relic = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Graveyard);
    let a = g.put(1, 1, Zone::Graveyard);
    let b = g.put(1, 2, Zone::Graveyard);
    let other = g.put(2, 1, Zone::Graveyard);
    g.put(0, 1, Zone::Library);
    g.act(0, Move::Activate { source: relic, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 1, choice: Choice::Objects { verb: ir::Verb::Exile, count: 1, among, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
    cite!(["400.3", "608.2d"]; assert!(among.len() == 2 && among.contains(&a) && among.contains(&b)));
    cite!(["608.2d"]; refused("not among the offered objects"): g.act(1, Move::Choose(Answer::Objects(vec![mine]))));
    g.act(1, Move::Choose(Answer::Objects(vec![a]))).unwrap();
    cite!(["701.13a"]; assert_eq!(g.players[1].graveyard, vec![b]));
    cite!(["400.3"]; assert_eq!(g.players[2].graveyard, vec![other]));
    g.players[0].pool = mana_pool("C");
    g.act(0, paid("C", Move::Activate { source: relic, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.13a"]; assert!(g.players.iter().all(|p| p.graveyard.is_empty())));
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 1));
    cite!(["701.13a"]; assert_eq!(g.exile.len(), 5));
}
