//! A loss that ends the game leaves everything where it is (104.2a, 104.3a); only a player
//! who leaves a game that goes on takes their objects with them (800.4a).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn game(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Drain", "{0}", "Instant", "Target player loses 20 life.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, players, 20)
}

#[test]
fn r104_2a_the_loser_of_a_two_player_game_keeps_their_objects_where_they_are() {
    for players in [2, 3] {
        let mut g = game(players);
        let bear = g.put(1, 1, Zone::Battlefield);
        let dead = g.put(1, 1, Zone::Graveyard);
        let drain = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: drain, targets: vec![vec![Target::Player(1)]] }).unwrap();
        while !g.players[1].lost { g.act(g.priority, Move::Pass).unwrap(); }
        if players == 2 {
            cite!(["104.2a", "104.1"]; assert!(g.finished()));
            // the game ended at once (104.1); objects leave with their owner only in a multiplayer game (800.1, 800.4a)
            cite!(["104.1", "800.1"]; assert_eq!((g.obj(bear).zone, g.obj(dead).zone), (Zone::Battlefield, Zone::Graveyard)));
            cite!(["104.1", "800.1"]; assert!(g.battlefield.contains(&bear) && g.players[1].graveyard.contains(&dead)));
        } else {
            cite!(["800.4"]; assert!(!g.finished()));
            cite!(["800.4a"]; assert_eq!((g.obj(bear).zone, g.obj(dead).zone), (Zone::Gone, Zone::Gone), "800.4a"));
            cite!(["800.4a"]; assert!(!g.battlefield.contains(&bear)));
        }
    }
}
