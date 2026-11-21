//! Starting Town and Kellan, Planar Trailblazer: an extra turn is one of the
//! player's turns (500.7); during an opponent's turn it is none of yours, so Starting Town
//! put onto the battlefield then enters tapped; a Clone of Kellan gives the trigger to itself,
//! about itself (201.5a, 707.2: what Kellan gave itself isn't copied).
use crate::support::{cite, paid};
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const TOWN: usize = 0;
const WALK: usize = 1;
const DROP: usize = 2;
const KELLAN: usize = 3;
const CLONE: usize = 4;
const ISLAND: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(&catalog[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Starting Town"),
        c("Walk", "Sorcery", "Take an extra turn after this one.", None),
        c("Drop", "Instant", "Put a land card from your hand onto the battlefield.", None),
        real("Kellan, Planar Trailblazer"), real("Clone"),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    g
}
fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(Decision::Discard { player, count }) => { let cards = g.players[player].hand[..count].to_vec(); g.act(player, Move::Discard(cards)).unwrap() }
        Some(d) => panic!("{d:?}"),
        None => g.act(g.priority, Move::Pass).unwrap(),
    }
}
/// On to the next precombat main phase of `player`'s, stack empty.
fn to_main(g: &mut Game, player: usize) {
    let turn = g.turn;
    while g.turn == turn || !(g.part == ir::Part::PrecombatMain && g.active == player && g.stack.is_empty() && g.decision.is_none()) { step(g); }
}
fn play_town(g: &mut Game, player: usize) -> bool {
    let town = g.put(player, TOWN, Zone::Hand);
    g.act(player, Move::PlayLand(town)).unwrap();
    let on = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(on).chars.name, "Starting Town");
    g.obj(on).tapped
}

/// Player 0 takes an extra turn as their 2nd: game turn 4 is their 3rd turn (untapped), game
/// turn 5 their 4th (tapped).
#[test]
fn an_extra_turn_counts_among_the_players_turns() {
    let mut g = game();
    while g.part != ir::Part::PrecombatMain { step(&mut g); }
    // Uncited (card text): "enters tapped unless it's your first, second, or third turn of the game"
    assert!(!play_town(&mut g, 0), "1st turn");
    let walk = g.put(0, WALK, Zone::Hand);
    g.act(0, Move::Cast { card: walk, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { step(&mut g); }
    to_main(&mut g, 0);
    cite!(["500.7"]; assert_eq!((g.turn, g.players[0].turns_taken), (2, 2), "the extra turn"));
    to_main(&mut g, 1);
    assert_eq!((g.turn, g.players[1].turns_taken), (3, 1));
    to_main(&mut g, 0);
    cite!(["500.7"]; assert_eq!((g.turn, g.players[0].turns_taken), (4, 3)));
    // Uncited (card text): "your … third turn of the game", the extra turn among them (500.7 cited above)
    assert!(!play_town(&mut g, 0), "3rd turn");
    to_main(&mut g, 1);
    to_main(&mut g, 0);
    assert_eq!(g.players[0].turns_taken, 4);
    // Uncited (card text): a fourth turn is none of "your first, second, or third turn of the game"
    assert!(play_town(&mut g, 0), "4th turn");
}

/// Player 1 puts Starting Town onto the battlefield during player 0's first turn: it is none of
/// player 1's turns, so it enters tapped.
#[test]
fn during_an_opponents_turn_it_enters_tapped() {
    let mut g = game();
    while g.part != ir::Part::PrecombatMain { step(&mut g); }
    g.act(0, Move::Pass).unwrap();
    g.put(1, TOWN, Zone::Hand);
    let drop = g.put(1, DROP, Zone::Hand);
    g.act(1, Move::Cast { card: drop, targets: vec![] }).unwrap();
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, .. }) => {
                let town = *g.players[1].hand.iter().find(|o| g.obj(**o).card == Some(TOWN)).unwrap();
                g.act(player, Move::Choose(Answer::Objects(vec![town]))).unwrap();
            }
            None if g.stack.is_empty() => break,
            _ => step(&mut g),
        }
    }
    let town = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Starting Town").expect("put onto the battlefield");
    cite!(["102.1"]; assert!(g.obj(town).tapped));
}

fn activate(g: &mut Game, source: ObjId, ability: usize, mana: &str) {
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    g.act(0, paid(mana,Move::Activate { source, ability, targets: vec![], paying: vec![] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

/// Kellan has made itself a Detective with the trigger; a Clone of it is a Scout with neither,
/// and the legend rule keeps the Clone. Attacking, the Clone triggers nothing; once it
/// activates its own first ability, its own combat damage triggers it, once.
#[test]
fn a_clone_of_kellan_gives_the_trigger_to_itself() {
    let mut g = game();
    while g.part != ir::Part::PrecombatMain { step(&mut g); }
    let kellan = g.put(0, KELLAN, Zone::Battlefield);
    activate(&mut g, kellan, 0,"CR");
    let clone = g.put(0, CLONE, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.act(0, paid("CCCU",Move::Cast { card: clone, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Objects(vec![kellan]))).unwrap();
    let copy = *g.battlefield.iter().find(|o| **o != kellan && g.obj(**o).chars.name == "Kellan, Planar Trailblazer").expect("the copy");
    // the legend rule: keep the copy
    while let Some(Decision::Choose { player, .. }) = g.decision.clone() { g.act(player, Move::Choose(Answer::Objects(vec![copy]))).unwrap(); }
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["704.5j"]; assert!(!g.battlefield.contains(&kellan)));
    cite!(["707.2"]; assert!(g.seen(copy, None).subtypes.iter().any(|s| s.name() == "Scout"), "copiable values only"));
    let attack = |g: &mut Game| {
        to_main(g, 1);
        while g.decision != Some(Decision::Attackers { player: 0 }) { step(g); }
        g.act(0, Move::Attack(vec![(copy, Target::Player(1))])).unwrap();
        while g.part != ir::Part::PostcombatMain { step(g); }
    };
    let exiled = g.exile.len();
    attack(&mut g);
    cite!(["707.2"]; assert_eq!((g.players[1].life, g.exile.len()), (18, exiled), "no trigger without its own activation"));
    activate(&mut g, copy, 0,"CR");
    assert!(g.seen(copy, None).subtypes.iter().any(|s| s.name() == "Detective"));
    attack(&mut g);
    cite!(["201.5a"]; assert_eq!((g.players[1].life, g.exile.len()), (16, exiled + 1), "its own damage triggers it, once"));
}
