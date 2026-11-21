//! Numbers counted in a named player's zones (400.1, 608.2h).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn game(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Measure", "{0}", "Sorcery", text, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    game::play::check(&defs[0]).unwrap();
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
}

#[test]
fn r608_2h_a_named_players_hand_is_counted_when_the_instruction_is_applied() {
    let mut g = game("Target player draws two cards. Measure deals damage to that player equal to the number of cards in that player's hand.");
    let private = g.put(1, 1, Zone::Hand);
    for _ in 0..3 { g.put(1, 1, Zone::Library); }
    resolve(&mut g, vec![vec![Target::Player(1)]]);
    cite!(["608.2h"]; assert_eq!(g.players[1].life, 17));
    assert_eq!(g.players[0].life, 20);
    cite!(["402.3"]; assert!(!g.sees(0, private)));
}

#[test]
fn r400_1_a_count_is_of_the_specified_players_specified_zone() {
    for (word, zone) in [("hand", Zone::Hand), ("library", Zone::Library), ("graveyard", Zone::Graveyard)] {
        let mut g = game(&format!("Measure deals damage to target player equal to the number of cards in that player's {word}."));
        for _ in 0..3 { g.put(1, 1, zone); }
        g.put(0, 1, zone);
        resolve(&mut g, vec![vec![Target::Player(1)]]);
        cite!(["400.1"]; case: word; assert_eq!(g.players[1].life, 17, "{word}"));
    }
}

#[test]
fn r608_2h_your_hand_is_counted_after_the_spell_leaves_it() {
    let mut g = game("You gain life equal to the number of cards in your hand.");
    for _ in 0..2 { g.put(0, 1, Zone::Hand); }
    g.put(0, 0, Zone::Hand);
    resolve(&mut g, vec![]);
    cite!(["608.2h", "405.1"]; assert_eq!(g.players[0].life, 23, "two Forests and the other Measure remain; the resolving Measure has left"));
}
