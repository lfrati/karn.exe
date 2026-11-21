//! Annihilator (702.86): "Annihilator N" means "Whenever this creature attacks, defending
//! player sacrifices N permanents." The defending player, the one it attacks (508.5), chooses
//! which of the permanents they control (701.21a, 608.2d). Ulamog's Crusher is read from its
//! Oracle text.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const CRUSHER: usize = 0;
const BEARS: usize = 1;
const FOREST: usize = 2;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Ulamog's Crusher", "Grizzly Bears", "Forest"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Player 0's Crusher attacks this player; its trigger resolves until a choice is asked.
fn attack(g: &mut Game, defender: usize) {
    let crusher = g.put(0, CRUSHER, Zone::Battlefield);
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(crusher, Target::Player(defender))])).unwrap();
    cite!(["702.86a"]; assert_eq!(g.stack.len(), 1, "the attack trigger"));
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r702_86a_the_defending_player_sacrifices_two_permanents_of_their_choice() {
    let mut g = game(2);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let forest = g.put(1, FOREST, Zone::Battlefield);
    let other = g.put(1, FOREST, Zone::Battlefield);
    attack(&mut g, 1);
    let Some(Decision::Choose { player, choice: Choice::Objects { among, count, .. } }) = g.decision.clone() else { panic!("asked: {:?}", g.decision) };
    cite!(["701.21a"]; assert_eq!((player, count), (1, 2), "the defending player chooses two"));
    cite!(["701.21a"]; assert_eq!(among, vec![bears, forest, other], "among the permanents they control"));
    g.act(1, Move::Choose(Answer::Objects(vec![bears, forest]))).unwrap();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![bears, forest] } }));
    g.act(1, Move::Choose(game::state::Answer::Objects(vec![forest, bears]))).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.86a"]; assert!(g.battlefield.contains(&other) && g.battlefield.contains(&mine)));
    assert!(!g.battlefield.contains(&bears) && !g.battlefield.contains(&forest));
    assert_eq!(g.players[1].graveyard.len(), 2);
}

#[test]
fn r702_86a_with_two_or_fewer_permanents_all_are_sacrificed() {
    let mut g = game(2);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, 1);
    cite!(["701.21a"]; assert!(g.decision.is_none() && g.stack.is_empty()));
    assert!(!g.battlefield.contains(&bears));
}

#[test]
fn r508_5_of_three_players_the_attacked_one_sacrifices() {
    let mut g = game(3);
    let one: Vec<ObjId> = (0..2).map(|_| g.put(1, FOREST, Zone::Battlefield)).collect();
    let two: Vec<ObjId> = (0..2).map(|_| g.put(2, FOREST, Zone::Battlefield)).collect();
    attack(&mut g, 2);
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 2, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: two.clone() } }));
    g.act(2, Move::Choose(game::state::Answer::Objects(vec![two[1], two[0]]))).unwrap();
    cite!(["508.5"]; assert!(one.iter().all(|o| g.battlefield.contains(o)) && two.iter().all(|o| !g.battlefield.contains(o))));
}
