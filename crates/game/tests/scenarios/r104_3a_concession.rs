//! Concession (104.3a): "A player can concede the game at any time. A player who concedes
//! leaves the game immediately. That player loses the game." In a two-player game the other
//! player is then the only one left and wins (104.2a); the game ends with everything where
//! it is. In a game that goes on, the conceding player leaves it (800.4a).
use crate::support::cite;
use game::simulation::{ResponseError, Simulation};
use game::state::Decision;
use game::{CardDef, Event, Game, Move, Zone};

fn defs() -> Vec<CardDef> {
    let defs = vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    defs
}

fn game(players: usize) -> Game {
    let mut g = Game::new(defs(), players, 20);
    for p in 0..players { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    g.begin();
    g
}

fn conceded(g: &Game, player: usize) -> bool {
    let events: Vec<_> = g.log.iter().filter(|e| matches!(e, Event::Conceded { .. } | Event::Lost { .. })).collect();
    events == [&Event::Conceded { player }, &Event::Lost { player }]
}

#[test]
fn r104_3a_the_conceding_player_loses_and_the_other_wins_a_two_player_game() {
    let mut g = game(2);
    let bear = g.put(1, 0, Zone::Battlefield);
    assert_eq!(g.priority, 0);
    // player 1 concedes while player 0 has priority
    cite!(["104.3a"]; ok: g.act(1, Move::Concede));
    cite!(["104.3a"]; assert!(g.players[1].lost && conceded(&g, 1)));
    cite!(["104.2a"]; assert!(g.finished()));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
    cite!(["104.2a"]; assert!(g.asked().is_none(), "nothing more is asked in a game that is over"));
    // the game ends with everything where it is (104.2a): only a game that goes on is left (800.4a)
    cite!(["104.2a"]; assert_eq!(g.obj(bear).zone, Zone::Battlefield));
    cite!(["104.3a"]; refused("the game is over"): g.act(0, Move::Concede));
    cite!(["104.3a"]; refused("the game is over"): g.act(1, Move::Concede));
}

#[test]
fn r104_3a_a_concession_is_taken_while_the_other_players_decision_is_pending() {
    // the starting player is asked about a mulligan (103.5)
    let decks = vec![vec![1; 10], vec![1; 10]];
    let mut g = Game::start(defs(), &decks, 20, 0, 1);
    assert_eq!(g.decision, Some(Decision::Mulligan { player: 0 }));
    cite!(["104.3a"]; ok: g.act(1, Move::Concede));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
    // or attackers are being declared
    let mut g = game(2);
    g.put(0, 0, Zone::Battlefield);
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["104.3a"]; ok: g.act(1, Move::Concede));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
    // the player who is asked may concede too
    let mut g = Game::start(defs(), &decks, 20, 0, 1);
    cite!(["104.3a"]; ok: g.act(0, Move::Concede));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(1)));
}

#[test]
fn r104_3a_a_simulation_takes_a_concession_from_a_player_the_request_is_not_for() {
    let mut sim = Simulation::new(game(2));
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.player, 0);
    // a pass is not player 1's to make; a concession is (104.3a)
    assert!(matches!(sim.respond(request.id, 1, Move::Pass), Err(ResponseError::WrongPlayer { .. })));
    cite!(["104.3a"]; ok: sim.respond(request.id, 1, Move::Concede).map_err(|e| e.to_string()));
    cite!(["104.2a"]; assert!(sim.request().unwrap().is_none()));
    cite!(["104.2a"]; assert_eq!(sim.game().unwrap().winner(), Some(0)));
}

#[test]
fn r800_4a_in_a_game_that_goes_on_the_conceding_player_leaves_it() {
    let mut g = game(3);
    let bear = g.put(2, 0, Zone::Battlefield);
    assert_eq!(g.priority, 0);
    // what the player with priority would leave unanswered is not supported (refused, not approximated)
    crate::support::refused(g.act(0, Move::Concede), "a concession while that player's decision is pending is not supported in a game that goes on");
    assert!(!g.players[0].lost);
    cite!(["104.3a", "800.4a"]; ok: g.act(2, Move::Concede));
    cite!(["104.3a"]; assert!(g.players[2].lost && conceded(&g, 2)));
    cite!(["800.4a"]; assert!(!g.finished()));
    cite!(["800.4a"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    // the game goes on between the two left; one concedes, and the other wins (104.2a)
    cite!(["104.3a", "104.2a"]; ok: g.act(1, Move::Concede));
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(0)));
}
