//! Multiplayer rules, exercised on the engine directly.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn game(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Flame", "{0}", "Instant", "Flame deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Spark", "{0}", "Instant", "Spark deals 1 damage to target opponent or planeswalker.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Inferno", "{0}", "Instant", "Inferno deals 3 damage to each creature and each player.", None).unwrap(),
        CardDef::new("Watcher", "{0}", "Creature — Giant", "Whenever this creature is dealt damage, you gain 1 life.", Some((4, 5))).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, players, 20);
    for player in 0..players {
        for _ in 0..5 { g.put(player, 2, Zone::Library); }
    }
    g
}

fn pass_all(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn eliminate(g: &mut Game, player: usize) {
    g.players[player].life = 3;
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(player)]] }).unwrap();
    pass_all(g);
    assert!(g.players[player].lost);
}

#[test]
fn r800_4j_the_departed_active_players_turn_continues_with_living_players_receiving_priority() {
    for players in [3, 4] {
        let mut g = game(players);
        eliminate(&mut g, 0);
        cite!(["800.4j"]; case: players; assert_eq!(g.active, 0, "the current turn does not end early"));
        cite!(["800.4j"]; case: players; assert_eq!(g.priority, 1, "the next living player receives priority"));
        cite!(["800.4a"]; case: players; refused("that player is not in the game (800.4a)"): g.act(0, Move::Pass));
        let mut reached_end = false;
        for _ in 0..100 {
            assert!(!g.players[g.priority].lost);
            if g.turn == 2 { break; }
            reached_end |= g.part == Part::End;
            assert!(g.decision.is_none(), "no departed player should be asked a turn-based choice");
            g.act(g.priority, Move::Pass).unwrap();
        }
        cite!(["800.4j"]; case: players; assert!(reached_end, "the rest of the turn still happens"));
        assert_eq!((g.turn, g.active), (2, 1));
    }
}

#[test]
fn r800_4a_opponent_or_planeswalker_does_not_allow_a_departed_player_as_a_target() {
    let mut g = game(4);
    eliminate(&mut g, 2);
    let card = g.put(0, 1, Zone::Hand);
    let before = (g.stack.clone(), g.players[0].hand.clone(), g.log.len());
    cite!(["115.1", "800.4a"]; refused("an illegal target, that player: that player has left the game (115.1)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(2)]] }));
    assert_eq!((g.stack.clone(), g.players[0].hand.clone(), g.log.len()), before);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(3)]] }).unwrap();
    pass_all(&mut g);
    assert_eq!(g.players[3].life, 19);
}

#[test]
fn r800_4k_departed_players_are_skipped_in_priority_and_future_turns() {
    let mut g = game(4);
    eliminate(&mut g, 1);
    assert_eq!(g.priority, 0);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.priority, 2);
    g.act(2, Move::Pass).unwrap();
    assert_eq!(g.priority, 3);
    g.act(3, Move::Pass).unwrap();
    assert_eq!(g.part, Part::BeginningOfCombat);
    for _ in 0..100 {
        if g.turn == 2 { break; }
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            ref other => panic!("unexpected decision {other:?}"),
        }
    }
    cite!(["800.4k"]; assert_eq!((g.turn, g.active), (2, 2)));
}

#[test]
fn r800_4d_triggers_waiting_for_a_departed_player_are_not_put_on_the_stack() {
    let mut g = game(4);
    g.players[0].life = 3;
    g.put(0, 4, Zone::Battlefield);
    g.put(1, 4, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass_all(&mut g);
    assert!(g.players[0].lost);
    cite!(["800.4d"]; assert_eq!(g.stack.len(), 1, "only the surviving player's damage trigger is put on the stack"));
    cite!(["800.4d"]; assert_eq!(g.obj(g.stack[0]).controller, 1));
    assert!(g.decision.is_none());
    pass_all(&mut g);
    assert_eq!(g.players[1].life, 18);
    assert_eq!(g.players[0].life, 0);
}

#[test]
fn r104_4a_simultaneous_loss_of_all_remaining_players_is_a_draw() {
    for players in [2, 3, 4] {
        let mut g = game(players);
        for player in &mut g.players { player.life = 3; }
        let card = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        pass_all(&mut g);
        assert!(g.players.iter().all(|p| p.lost));
        cite!(["104.4a"]; case: players; assert!(g.finished()));
        cite!(["104.4a"]; case: players; assert_eq!(g.winner(), None));
        assert_eq!(g.asked(), None);
        for player in 0..players {
            crate::support::refused(g.act(player, Move::Pass), "the game is over");
        }
    }
}

#[test]
fn r104_4a_simultaneous_losses_leave_a_winner_when_one_player_survives() {
    let mut g = game(4);
    for player in &mut g.players[..3] { player.life = 3; }
    let card = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass_all(&mut g);
    assert!(g.finished());
    cite!(["104.2a"]; assert_eq!(g.winner(), Some(3)));
    assert_eq!(g.asked(), None);
}
