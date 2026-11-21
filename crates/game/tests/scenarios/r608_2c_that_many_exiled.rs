//! "That many" after an instruction that acts on a group of objects is the number of them it
//! acted on (608.2c), and the player the text names next makes the optional search (608.2d):
//! Settle the Wreckage's "Exile all attacking creatures target player controls. That player
//! may search their library for that many basic land cards, put those cards onto the
//! battlefield tapped, then shuffle." Every card is read from its Oracle text.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const SETTLE: usize = 0;
const BEARS: usize = 1;
const PLAINS: usize = 2;
const FOREST: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Settle the Wreckage", "Grizzly Bears", "Plains", "Forest"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Player 0 attacks player 1 with these; player 1 casts Settle the Wreckage at player 0 and
/// it resolves until a choice is asked.
fn settle(g: &mut Game, attackers: &[ObjId]) {
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    g.act(0, Move::Pass).unwrap();
    let settle = g.put(1, SETTLE, Zone::Hand);
    g.players[1].pool.colors = [4, 0, 0, 0, 0];
    g.act(1, paid("WWWW", Move::Cast { card: settle, targets: vec![vec![Target::Player(0)]] })).unwrap();
    for _ in 0..2 { if g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
}

fn lands(g: &Game, player: usize) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.controller(*o) == player && g.obj(*o).chars.name == "Plains").collect()
}

#[test]
fn r608_2c_that_player_may_search_for_as_many_basic_lands_as_creatures_exiled() {
    let mut g = game();
    let attackers: Vec<_> = (0..2).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    let home = g.put(0, BEARS, Zone::Battlefield);
    for _ in 0..3 { g.put(0, PLAINS, Zone::Library); }
    g.put(0, BEARS, Zone::Library);
    settle(&mut g, &attackers);
    cite!(["608.2c"]; assert!(attackers.iter().all(|a| g.obj(*a).zone == Zone::Gone) && g.battlefield.contains(&home), "the attacking creatures only"));
    let Some((0, Asked::May)) = g.asked() else { panic!("that player is asked: {:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let Some((0, Asked::Search { cards, among, min, max })) = g.asked() else { panic!("that player searches: {:?}", g.asked()) };
    cite!(["701.23a"]; assert_eq!((cards.len(), among.len()), (4, 3)));
    cite!(["608.2c"]; assert_eq!((min, max), (0, 2), "that many: the two creatures exiled"));
    g.act(0, Move::Choose(Answer::Objects(among[..2].to_vec()))).unwrap();
    let found = lands(&g, 0);
    cite!(["608.2c"]; assert!(found.len() == 2 && found.iter().all(|o| g.obj(*o).tapped), "tapped, under that player's control"));
    assert!(g.log.iter().any(|e| matches!(e, Event::Shuffled { player: 0, .. })));
    assert!(lands(&g, 1).is_empty());
}

#[test]
fn r608_2d_declining_searches_nothing() {
    let mut g = game();
    let attackers = vec![g.put(0, BEARS, Zone::Battlefield)];
    g.put(0, PLAINS, Zone::Library);
    settle(&mut g, &attackers);
    let Some((0, Asked::May)) = g.asked() else { panic!("asked: {:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2d"]; assert!(lands(&g, 0).is_empty() && !g.log.iter().any(|e| matches!(e, Event::Shuffled { .. }))));
}

#[test]
fn r608_2c_one_creature_exiled_finds_at_most_one() {
    let mut g = game();
    let attackers = vec![g.put(0, BEARS, Zone::Battlefield)];
    for _ in 0..2 { g.put(0, PLAINS, Zone::Library); }
    g.put(0, FOREST, Zone::Library);
    settle(&mut g, &attackers);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let Some((0, Asked::Search { among, max, .. })) = g.asked() else { panic!("searches: {:?}", g.asked()) };
    cite!(["608.2c"]; assert_eq!((among.len(), max), (3, 1)));
}
