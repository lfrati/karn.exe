//! Shuffling is an action even for an empty library (701.24a, e–f).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Mix", "{0}", "Instant", "Shuffle.", None).unwrap(),
        CardDef::new("Double", "{0}", "Instant", "Shuffle. Shuffle.", None).unwrap(),
        CardDef::new("Watcher", "{2}", "Artifact", "Whenever a player shuffles their library, Watcher deals 2 damage to that player.", None).unwrap(),
        CardDef::new("Witness", "{0}", "Artifact", "Whenever a player shuffles your library, you gain 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0)); }
    Game::new(defs, 2, 20)
}

fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

fn cast(g: &mut Game, player: usize, def: usize) {
    g.priority = player;
    let card = g.put(player, def, Zone::Hand);
    g.act(player, Move::Cast { card, targets: vec![] }).unwrap();
    pass(g);
}

#[test]
fn r701_24a_shuffle_preserves_cards_and_identities_hides_order_and_replays_from_a_clone() {
    let mut g = game();
    for i in 0..20 { g.put(0, 4 + i % 2, Zone::Library); }
    g.put(1, 4, Zone::Library);
    let before = g.players[0].library.clone();
    let other = g.players[1].library.clone();
    let mut replay = g.clone();
    cast(&mut g, 0, 0);
    cast(&mut replay, 0, 0);
    assert_eq!(g.players[0].library, replay.players[0].library);
    assert_ne!(g.players[0].library, before, "this fixed seed changes the order");
    let mut after = g.players[0].library.clone();
    after.sort_by_key(|id| id.0);
    cite!(["701.24a"]; assert_eq!(after, before));
    assert_eq!(g.players[1].library, other);
    for id in after {
        assert_eq!(g.obj(id).zone, Zone::Library);
        for viewer in 0..2 { cite!(["701.24a"]; assert!(!g.sees(viewer, id))); }
    }
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Shuffled { player: 0, .. })).count(), 1);
}

#[test]
fn r701_24e_empty_and_single_card_libraries_still_trigger_shuffle_abilities() {
    for count in 0..=1 {
        let mut g = game();
        g.put(0, 2, Zone::Battlefield);
        for _ in 0..count { g.put(1, 4, Zone::Library); }
        cast(&mut g, 1, 0);
        assert_eq!(g.players[1].library.len(), count);
        assert_eq!(g.stack.len(), 1);
        pass(&mut g);
        cite!(["701.24e"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 18)));
    }
}

#[test]
fn r603_2c_two_shuffles_are_two_occurrences_even_with_no_change_in_order() {
    let mut g = game();
    g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, 1);
    // The two identical triggers still need ordering before either resolves.
    if g.decision.is_some() { g.act(0, Move::Order(vec![0, 1])).unwrap(); }
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g);
    pass(&mut g);
    cite!(["603.2c"]; assert_eq!(g.players[0].life, 16));
}

#[test]
fn r701_24a_a_shuffle_pattern_matches_the_owner_of_the_library() {
    let mut g = game();
    g.put(0, 3, Zone::Battlefield);
    cast(&mut g, 1, 0);
    cite!(["603.2"]; assert!(g.stack.is_empty()));
    cast(&mut g, 0, 0);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r701_24a_a_creature_cannot_perform_a_shuffle() {

    assert_eq!(CardDef::new("Invalid", "{0}", "Instant", "Target creature shuffles.", None).expect_err("only a player shuffles a library"),"text the grammar does not read");
}
