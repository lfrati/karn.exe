//! CR 113.6: a keyword row may contribute independently in several zones.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use game::play::check;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let def = CardDef::new(name, "{0}", types, text, pt).unwrap();
    check(&def).unwrap();
    def
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r113_6_mixed_cycling_flashback_row_keeps_hand_activation_and_graveyard_route_separate() {
    let def = card("Mixed", "Instant", "You gain 1 life.\nCycling {0}, flashback {0}", None);
    let mut g = Game::new(vec![def, card("Blank", "Creature", "", Some((2, 2)))], 2, 20);
    g.put(0, 1, Zone::Library);
    let hand = g.put(0, 0, Zone::Hand);
    let activation = 2; // Two printed rows precede the derived cycling ability.
    g.act(0, Move::Activate { source: hand, ability: activation, targets: vec![], paying: vec![] }).unwrap();
    let grave = g.players[0].graveyard[0];
    pass(&mut g);
    cite!(["702.29a"]; assert_eq!(g.players[0].hand.len(), 1, "cycling draws once"));
    let before=format!("{g:?}");
    cite!(["702.29a","113.6m"]; refused("that ability functions only in the zone it moves its card out of (113.6m)"): g.act(0, Move::Activate { source: grave, ability: activation, targets: vec![], paying: vec![] }));
    assert_eq!(format!("{g:?}"),before);
    g.act(0,Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]}))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
    assert!(g.players[0].graveyard.is_empty());
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 1));
}

#[test]
fn r113_6_mixed_flying_kicker_row_holds_only_the_battlefield_restriction() {
    let mut g = Game::new(vec![
        card("Mixed", "Creature", "Flying, kicker {0}\nWhen this creature enters, if it was kicked, you gain 2 life.", Some((2, 2))),
        card("Blank", "Creature", "", Some((2, 2))),
    ], 2, 20);
    let source = g.put(0, 0, Zone::Hand);
    g.act(0, Move::WithKicks(1, Box::new(Move::Cast { card: source, targets: vec![] }))).unwrap();
    pass(&mut g);
    pass(&mut g);
    cite!(["702.33d"]; assert_eq!(g.players[0].life, 22));
    let attacker = g.battlefield[0];
    let blocker = g.put(1, 1, Zone::Battlefield);
    assert!(g.has(attacker, "Flying"));
    // The flying contribution's battlefield restriction remains in force.
    cite!(["702.9b"]; assert!(g.cant_block(blocker, attacker, &[blocker])));
}

#[test]
fn r702_34a_flashback_destination_replaces_hand_library_and_graveyard_departures_after_cloning() {
    for destination in [Zone::Hand, Zone::Library, Zone::Graveyard] {
        let mut g = Game::new(vec![card("Echo", "Instant", "You gain 1 life.\nFlashback {0}", None)], 2, 20);
        let grave = g.put(0, 0, Zone::Graveyard);
        g.act(0,Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:grave,targets:vec![]}))).unwrap();
        let spell = g.stack[0];
        let copy = g.clone();
        for mut g in [g, copy] {
            g.move_object(spell, destination);
            cite!(["702.34a"]; case: format!("{destination:?}"); assert_eq!(g.exile.len(), 1, "every paid-flashback departure goes to exile"));
            assert!(g.players[0].hand.is_empty() && g.players[0].library.is_empty() && g.players[0].graveyard.is_empty());
        }
    }
}
