//! A search the text has each of several players make (701.23i, 101.4): Field of Ruin's "Each
//! player searches their library for a basic land card, puts it onto the battlefield, then
//! shuffles." Each player looks through their own library alone (701.23a), and they choose in
//! APNAP order; what each finds is seen by that player alone until it is put onto the
//! battlefield, under its owner's control (110.2a). One who finds nothing still shuffles; one
//! who has left the game does nothing (800.4a). Every card is read from its Oracle text.
use crate::support::cite;
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const FIELD: usize = 0;
const PLAINS: usize = 1;
const FOREST: usize = 2;
const BEARS: usize = 3;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Field of Ruin", "Plains", "Forest", "Grizzly Bears"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Player 0 activates a Field of Ruin at player 1's, and it resolves: the searches begin.
fn ruin(g: &mut Game) -> ObjId {
    let field = g.put(0, FIELD, Zone::Battlefield);
    let target = g.put(1, FIELD, Zone::Battlefield);
    for _ in 0..2 { g.add_mana(0, ManaSymbol::Color(Color::Green)); }
    g.act(0, crate::support::paid("GG", Move::Activate { source: field, ability: 1, targets: vec![vec![Target::Object(target)]], paying: vec![] })).unwrap();
    for _ in 0..g.players.len() * 2 {
        if !matches!(g.asked(), Some((_, Asked::Priority))) || g.stack.is_empty() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    target
}

fn shuffled(g: &Game) -> Vec<usize> {
    g.log.iter().filter_map(|e| match e { Event::Shuffled { player, .. } => Some(*player), _ => None }).collect()
}

fn lands(g: &Game, controller: usize, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name && g.controller(*o) == controller).collect()
}

#[test]
fn r701_23i_each_player_searches_their_own_library_in_apnap_order() {
    let mut g = game(2);
    let plains = g.put(0, PLAINS, Zone::Library);
    g.put(0, BEARS, Zone::Library);
    let forest = g.put(1, FOREST, Zone::Library);
    g.put(1, BEARS, Zone::Library);
    let target = ruin(&mut g);
    cite!(["701.21a"]; assert_eq!(g.obj(target).zone, Zone::Gone, "the target was destroyed first"));
    // the active player first (101.4), through their own library alone (701.23a)
    let Some((0, Asked::Search { cards, among, min, max })) = g.asked() else { panic!("player 0 searches: {:?}", g.asked()) };
    cite!(["701.23a"]; assert!(cards.len() == 2 && cards.iter().all(|c| g.obj(*c).owner == 0)));
    cite!(["701.23b"]; assert_eq!((among, min, max), (vec![plains], 0, 1)));
    cite!(["701.23a"]; assert!(!g.sees(1, plains) && g.sees(0, plains)));
    let before=format!("{g:?}");
    // Submit an otherwise valid selected object to isolate the wrong chooser.
    cite!(["608.2d"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(vec![plains]))));
    assert_eq!(format!("{g:?}"),before);
    g.act(0, Move::Choose(Answer::Objects(vec![plains]))).unwrap();
    // then the next player, through theirs (701.23i)
    let Some((1, Asked::Search { cards, among, .. })) = g.asked() else { panic!("player 1 searches: {:?}", g.asked()) };
    cite!(["701.23i"]; assert!(cards.len() == 2 && cards.iter().all(|c| g.obj(*c).owner == 1)));
    assert_eq!(among, vec![forest]);
    // what player 0 found is theirs to see alone (701.23e)
    cite!(["701.23e"]; assert!(g.sees(0, plains) && !g.sees(1, plains)));
    cite!(["701.23a"]; assert!(!g.sees(0, forest)));
    g.act(1, Move::Choose(Answer::Objects(vec![forest]))).unwrap();
    // each puts what they found onto the battlefield, under their own control (110.2a)
    cite!(["110.2a"]; assert_eq!((lands(&g, 0, "Plains").len(), lands(&g, 1, "Forest").len()), (1, 1)));
    cite!(["701.24a"]; assert_eq!(shuffled(&g), vec![0, 1]));
    assert!(g.stack.is_empty());
}

#[test]
fn r701_23b_a_player_who_finds_nothing_still_shuffles() {
    let mut g = game(2);
    let plains = g.put(0, PLAINS, Zone::Library);
    g.put(1, BEARS, Zone::Library);
    ruin(&mut g);
    let Some((0, Asked::Search { .. })) = g.asked() else { panic!("player 0 searches: {:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    // player 1 has no basic land: still asked, nothing to find (701.23b)
    if let Some((1, Asked::Search { among, .. })) = g.asked() {
        assert!(among.is_empty());
        g.act(1, Move::Choose(Answer::Objects(vec![]))).unwrap();
    }
    cite!(["701.23b"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name == "Field of Ruin")));
    assert_eq!(g.obj(plains).zone, Zone::Library);
    cite!(["701.24a"]; assert_eq!(shuffled(&g), vec![0, 1]));
}

#[test]
fn r800_4a_a_player_who_left_the_game_does_not_search() {
    let mut g = game(3);
    g.put(0, PLAINS, Zone::Library);
    g.put(1, FOREST, Zone::Library);
    let gone = g.put(2, FOREST, Zone::Library);
    g.players[2].lost = true;
    ruin(&mut g);
    for p in [0, 1] {
        let Some((asked, Asked::Search { among, .. })) = g.asked() else { panic!("player {p} searches: {:?}", g.asked()) };
        assert_eq!(asked, p);
        g.act(p, Move::Choose(Answer::Objects(among))).unwrap();
    }
    cite!(["800.4a"]; assert!(g.decision.is_none() && g.stack.is_empty()));
    cite!(["800.4a"]; assert_eq!(g.obj(gone).zone, Zone::Library));
    assert_eq!((lands(&g, 0, "Plains").len(), lands(&g, 1, "Forest").len()), (1, 1));
}
