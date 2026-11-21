//! "It's your first, second, or third turn of the game" (Starting Town): of the turns that
//! player has taken, extra turns included (500.7), counted per player, not by the game's turn
//! number.
use game::state::Decision;
use game::{CardDef, Game, Move, Zone};

const TOWN: usize = 0;
const ISLAND: usize = 1;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let town = CardDef::from_face(faces.iter().find(|f| f.name == "Starting Town").unwrap()).unwrap();
    game::play::check(&town).unwrap();
    let defs = vec![town, CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    g
}
/// Pass until it is `player`'s precombat main phase.
fn to_main(g: &mut Game, player: usize) {
    loop {
        if g.part == ir::Part::PrecombatMain && g.active == player && g.stack.is_empty() && g.decision.is_none() { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Discard { player, count }) => { let cards = g.players[player].hand[..count].to_vec(); g.act(player, Move::Discard(cards)).unwrap() }
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn play_town(g: &mut Game, player: usize) -> bool {
    let town = g.put(player, TOWN, Zone::Hand);
    g.act(player, Move::PlayLand(town)).unwrap();
    let on = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(on).chars.name, "Starting Town");
    g.obj(on).tapped
}

#[test]
fn starting_town_enters_untapped_on_each_players_first_three_turns_only() {
    let mut g = game();
    // player 0's turns are game turns 1, 3, 5, 7; player 1's are 2, 4, 6
    let mut seen = Vec::new();
    for _ in 0..4 {
        to_main(&mut g, 0);
        seen.push((0, g.turn, play_town(&mut g, 0)));
        if g.turn >= 7 { break; }
        to_main(&mut g, 1);
        seen.push((1, g.turn, play_town(&mut g, 1)));
    }
    // Uncited (card text): "This land enters tapped unless it's your first, second, or third turn of the game.": counted per player; no rule defines "your Nth turn"
    assert_eq!(seen, vec![(0, 1, false), (1, 2, false), (0, 3, false), (1, 4, false), (0, 5, false), (1, 6, false), (0, 7, true)]);
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (4, 3));
}
