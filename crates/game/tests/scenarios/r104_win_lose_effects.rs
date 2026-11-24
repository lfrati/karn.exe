//! An effect may state that a player wins the game (104.2b) or loses it (104.3e). A game ends
//! immediately when a player wins (104.1), every other player then out of it; a player who
//! loses a game that goes on leaves it (800.4a), and in a two-player game the other player is
//! the only one left and wins (104.2a).
use crate::support::{cite, mana_pool, oracle};
use game::state::Decision;
use game::{Event, Game, Move, Target, Zone};
use ir::Part;

fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
}

/// A game of `players` at `life`, each with a few cards to draw, and this card on player 1's
/// battlefield; then it goes on to player 1's first upkeep, the trigger on the stack.
fn upkeep(name: &str, players: usize, life: i32) -> (Game, game::ObjId) {
    let mut g = Game::new(vec![oracle(name), oracle("Grizzly Bears")], players, life);
    for p in 0..players { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
    let card = g.put(1, 0, Zone::Battlefield);
    while !(g.active == 1 && g.part == Part::Upkeep) { step(&mut g); }
    (g, card)
}

fn lost(g: &Game) -> Vec<usize> {
    g.log.iter().filter_map(|e| if let Event::Lost { player } = e { Some(*player) } else { None }).collect()
}

#[test]
fn r104_2b_the_controller_of_felidar_sovereign_wins_at_forty_life() {
    // "At the beginning of your upkeep, if you have 40 or more life, you win the game."
    let (mut g, felidar) = upkeep("Felidar Sovereign", 2, 40);
    cite!(["603.4"]; assert_eq!(g.stack.len(), 1));
    while !g.finished() { step(&mut g); }
    cite!(["104.2b"]; assert_eq!(g.winner(), Some(1)));
    cite!(["104.2b", "104.1"]; assert!(g.players[0].lost && !g.players[1].lost && lost(&g) == vec![0]));
    // the game ends with everything where it is (104.1)
    cite!(["104.1"]; assert!(g.asked().is_none() && g.obj(felidar).zone == Zone::Battlefield));
    cite!(["104.1"]; refused("the game is over"): g.act(1, Move::Pass));
}

#[test]
fn r603_4_short_of_forty_life_it_does_not_trigger() {
    let (mut g, _) = upkeep("Felidar Sovereign", 2, 39);
    cite!(["603.4"]; assert!(g.stack.is_empty()));
    while g.part == Part::Upkeep { step(&mut g); }
    cite!(["603.4"]; assert!(!g.finished() && g.players.iter().all(|p| !p.lost)));
}

#[test]
fn r104_1_a_win_ends_a_multiplayer_game_and_every_other_player_loses_it() {
    let (mut g, _) = upkeep("Felidar Sovereign", 3, 40);
    while !g.finished() { step(&mut g); }
    cite!(["104.1", "104.2b"]; assert_eq!(g.winner(), Some(1)));
    cite!(["104.1"]; assert!(g.players[0].lost && g.players[2].lost && lost(&g) == vec![0, 2]));
}

#[test]
fn r104_3e_its_controller_loses_when_the_last_oil_counter_is_removed() {
    // Archfiend of the Dross: "At the beginning of your upkeep, remove an oil counter from this
    // creature. Then if it has no oil counters on it, you lose the game."
    let mut g = Game::new(vec![oracle("Archfiend of the Dross"), oracle("Grizzly Bears")], 2, 20);
    for p in 0..2 { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
    let archfiend = g.put(1, 0, Zone::Battlefield);
    g.set_initial_counters(archfiend, "oil", 2);
    while !(g.active == 1 && g.part == Part::Upkeep && !g.stack.is_empty()) { step(&mut g); }
    while !g.stack.is_empty() { step(&mut g); }
    // one is left: the game goes on
    cite!(["608.2c"]; assert!(!g.finished() && g.obj(archfiend).counters.get("oil") == Some(&1)));
    while !(g.active == 1 && g.part == Part::Upkeep && !g.stack.is_empty()) { step(&mut g); }
    while !g.finished() { step(&mut g); }
    cite!(["104.3e"]; assert!(g.players[1].lost && lost(&g) == vec![1]));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
}

/// Door to Nothingness: "{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}, {T}, Sacrifice this artifact: Target
/// player loses the game." Put onto the battlefield untapped and activated by player 0 in its
/// first main phase at that player.
fn door(players: usize, at: usize) -> (Game, game::ObjId) {
    let mut g = Game::new(vec![oracle("Door to Nothingness"), oracle("Grizzly Bears")], players, 20);
    for p in 0..players { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
    g.put(0, 0, Zone::Battlefield);
    let bears = g.put(at, 1, Zone::Battlefield);
    assert_eq!(g.part, Part::PrecombatMain);
    let source = g.battlefield[0];
    let ability = g.abilities(source).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    let mana = "WWUUBBRRGG";
    g.players[0].pool = mana_pool(mana);
    let activate = Move::Activate { source, ability, targets: vec![vec![Target::Player(at)]], paying: vec![] };
    g.act(0, Move::Paying(mana_pool(mana), Box::new(activate))).unwrap();
    cite!(["602.2"]; assert_eq!(g.stack.len(), 1));
    (g, bears)
}

#[test]
fn r104_3e_target_player_loses_the_game_and_the_other_wins() {
    let (mut g, bears) = door(2, 1);
    while !g.finished() { step(&mut g); }
    cite!(["104.3e"]; assert!(g.players[1].lost && lost(&g) == vec![1]));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
    cite!(["104.2a"]; assert_eq!(g.obj(bears).zone, Zone::Battlefield));
}

#[test]
fn r800_4a_in_a_game_that_goes_on_the_player_who_loses_leaves_it() {
    let (mut g, bears) = door(3, 2);
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["104.3e"]; assert!(g.players[2].lost && lost(&g) == vec![2]));
    cite!(["800.4a"]; assert!(!g.finished() && g.winner().is_none()));
    // what they owned has left the game with them (800.4a)
    cite!(["800.4a"]; assert!(!g.battlefield.contains(&bears) && g.players[2].library.is_empty()));
    // the game goes on between the others
    cite!(["800.4a"]; assert!(matches!(g.asked(), Some((0, game::legal::Asked::Priority)))));
    while g.turn == 1 { step(&mut g); }
    cite!(["800.4a"]; assert_eq!(g.active, 1));
    while g.turn == 2 { step(&mut g); }
    cite!(["800.4a"]; assert_eq!(g.active, 0));
}

#[test]
fn r800_4a_the_controller_of_what_resolves_losing_a_game_that_goes_on_is_refused() {
    // Archfiend of the Dross in a three-player game: its controller would leave the game while
    // its own ability resolves (800.4a), which is not done yet
    let mut g = Game::new(vec![oracle("Archfiend of the Dross"), oracle("Grizzly Bears")], 3, 20);
    for p in 0..3 { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
    let archfiend = g.put(1, 0, Zone::Battlefield);
    g.set_initial_counters(archfiend, "oil", 1);
    while !(g.active == 1 && g.part == Part::Upkeep && !g.stack.is_empty()) { step(&mut g); }
    g.act(1, Move::Pass).unwrap();
    g.act(2, Move::Pass).unwrap();
    cite!(["800.4a"]; refused("a player leaving a game that goes on while what they control resolves"): g.act(0, Move::Pass));
}
