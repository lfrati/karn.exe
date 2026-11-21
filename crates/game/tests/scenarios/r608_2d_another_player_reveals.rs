//! An instruction said of another player than the controller is that player's to follow
//! (608.2d): Goblin Guide's "Whenever this creature attacks, defending player reveals the top
//! card of their library. If it's a land card, that player puts it into their hand." The
//! defending player (506.2) reveals the top card of their own library, and puts it into their
//! own hand if it's a land card. Every card is read from its Oracle text.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const GUIDE: usize = 0;
const FOREST: usize = 1;
const BEARS: usize = 2;
const BOLT: usize = 3;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Goblin Guide", "Forest", "Grizzly Bears", "Lightning Bolt"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, players, 20);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

/// Player 0's Goblin Guide attacks this player, and its trigger resolves.
fn attack(g: &mut Game, defender: usize) -> ObjId {
    let guide = g.put(0, GUIDE, Zone::Battlefield);
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(guide, Target::Player(defender))])).unwrap();
    assert_eq!(g.stack.len(), 1, "the attack trigger");
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    guide
}

fn names<'a>(g: &'a Game, cards: &[ObjId]) -> Vec<&'a str> { cards.iter().map(|c| g.obj(*c).chars.name.as_str()).collect() }

fn revealed(g: &Game) -> Vec<(usize, Vec<ObjId>)> {
    g.log.iter().filter_map(|e| match e { Event::Revealed { player, cards } => Some((*player, cards.clone())), _ => None }).collect()
}

#[test]
fn r608_2d_the_defending_player_reveals_a_land_and_puts_it_into_their_hand() {
    let mut g = game(2);
    g.put(1, BEARS, Zone::Library);
    let forest = g.put(1, FOREST, Zone::Library);
    g.put(0, FOREST, Zone::Library);
    attack(&mut g, 1);
    cite!(["608.2d"]; assert_eq!(revealed(&g), vec![(1, vec![forest])], "the defending player reveals their top card"));
    cite!(["608.2d"]; assert_eq!(names(&g, &g.players[1].hand), ["Forest"], "into that player's hand"));
    cite!(["400.7"]; assert_eq!(g.obj(forest).zone, Zone::Gone));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[1].library.len(), 1);
}

#[test]
fn r608_2d_a_nonland_card_stays_on_top_revealed() {
    let mut g = game(2);
    let bears = g.put(1, BEARS, Zone::Library);
    attack(&mut g, 1);
    cite!(["701.20a"]; assert_eq!(revealed(&g), vec![(1, vec![bears])]));
    cite!(["608.2c"]; assert_eq!(g.players[1].library.last(), Some(&bears)));
    assert!(g.players[1].hand.is_empty());
}

#[test]
fn r506_2_of_three_players_the_one_attacked_reveals() {
    let mut g = game(3);
    let one = g.put(1, FOREST, Zone::Library);
    let two = g.put(2, FOREST, Zone::Library);
    attack(&mut g, 2);
    cite!(["506.2"]; assert_eq!(revealed(&g), vec![(2, vec![two])]));
    assert_eq!((names(&g, &g.players[2].hand), g.obj(one).zone), (vec!["Forest"], Zone::Library));
}

#[test]
fn r608_2d_an_empty_library_reveals_nothing() {
    let mut g = game(2);
    attack(&mut g, 1);
    cite!(["608.2d"]; assert!(revealed(&g).iter().all(|(_, cards)| cards.is_empty())));
    assert!(g.players[1].hand.is_empty() && g.stack.is_empty());
}

#[test]
fn r508_5_a_creature_removed_from_combat_still_names_the_player_it_was_attacking() {
    // the Guide attacks player 2 of three, and is destroyed in response to its trigger
    let mut g = game(3);
    g.put(1, FOREST, Zone::Library);
    let two = g.put(2, FOREST, Zone::Library);
    let guide = g.put(0, GUIDE, Zone::Battlefield);
    while g.part != Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(guide, Target::Player(2))])).unwrap();
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(guide)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(guide).zone, Zone::Gone, "the Guide died before its trigger resolved");
    cite!(["508.5"]; assert_eq!(revealed(&g), vec![(2, vec![two])]));
    assert_eq!(names(&g, &g.players[2].hand), ["Forest"]);
}
