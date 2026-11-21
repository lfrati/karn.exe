//! Ordered library placement (401.4, 401.7) composes with ownership, searching and replacement.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn game(text: &str, players: usize) -> Game {
    let defs = vec![
        CardDef::new("Placement", "{0}", "Instant", text, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Wolf", "{0}", "Creature — Wolf", "", Some((2, 2))).unwrap(),
        CardDef::new("Restore", "{0}", "Instant", "Put target creature card from a graveyard onto the battlefield under your control.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, players, 20)
}

fn cast(g: &mut Game, def: usize, target: Option<ObjId>, zone: Zone) {
    let card = g.put(0, def, zone);
    g.act(0, Move::Cast { card, targets: target.map(|o| vec![vec![Target::Object(o)]]).unwrap_or_default() }).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn names(g: &Game, player: usize) -> Vec<String> {
    g.players[player].library.iter().rev().map(|o| g.obj(*o).chars.name.clone()).collect()
}

#[test]
fn r401_7_placement_counts_from_the_top_and_short_libraries_use_the_bottom() {
    for (text, place) in [
        ("Put target creature on top of its owner's library.", 0),
        ("Put target creature on the bottom of its owner's library.", usize::MAX),
        ("Put target creature into its owner's library third from the top.", 2),
    ] {
        for count in [0, 1, 2, 4] {
            let mut g = game(text, 2);
            for _ in 0..count { g.put(1, 1, Zone::Library); }
            let bear = g.put(1, 2, Zone::Battlefield);
            cast(&mut g, 0, Some(bear), Zone::Hand);
            let mut expected = vec!["Forest".to_string(); count];
            expected.insert(place.min(count), "Bear".to_string());
            cite!(["401.7"]; case: format!("{text} {count}"); assert_eq!(names(&g, 1), expected));
            assert_eq!(g.obj(bear).zone, Zone::Gone);
            assert!(g.players[0].library.is_empty());
        }
    }
}

#[test]
fn r400_3_a_controlled_opponents_card_goes_to_its_owners_library_even_when_yours_is_named() {
    let mut g = game("Put target creature on top of your library.", 2);
    let bear = g.put(1, 2, Zone::Graveyard);
    cast(&mut g, 4, Some(bear), Zone::Hand);
    let bear = g.battlefield[0];
    assert_eq!((g.obj(bear).owner, g.obj(bear).controller), (1, 0));
    cast(&mut g, 0, Some(bear), Zone::Hand);
    assert!(g.players[0].library.is_empty());
    cite!(["400.3"]; assert_eq!(names(&g, 1), ["Bear"]));
}

#[test]
fn r401_4_owners_choose_order_privately_in_apnap_order_before_any_cards_move() {
    let mut g = game("Put all creatures on top of their owners' libraries.", 3);
    let groups: Vec<_> = (0..3).map(|p| vec![g.put(p, 2, Zone::Battlefield), g.put(p, 3, Zone::Battlefield)]).collect();
    cast(&mut g, 0, None, Zone::Hand);
    let mut other_order = g.clone();
    for player in 0..3 {
        cite!(["401.4"]; case: player; assert!(matches!(&g.decision, Some(Decision::Choose { player: p, choice: Choice::OrderCards { cards, .. } }) if *p == player && cards.len() == 2)));
        assert_eq!(g.battlefield.len(), 6, "all choices precede movement");
        let before = format!("{g:?}");
        cite!(["401.4"]; case: player; refused("order every offered card exactly once (401.4, 404.3)"): g.act(player, Move::Choose(Answer::Objects(vec![groups[player][0]; 2]))));
        assert_eq!(format!("{g:?}"), before);
        g.act(player, Move::Choose(Answer::Objects(groups[player].clone()))).unwrap();
        other_order.act(player, Move::Choose(Answer::Objects(groups[player].iter().rev().copied().collect()))).unwrap();
    }
    assert!(g.battlefield.is_empty());
    for player in 0..3 {
        assert_eq!(names(&g, player), ["Bear", "Wolf"]);
        cite!(["401.4"]; case: player; assert_eq!(names(&other_order, player), ["Wolf", "Bear"]));
    }
    cite!(["401.4"]; assert_eq!(g.log, other_order.log, "public events must not disclose private ordering"));
}

#[test]
fn r401_4_random_order_is_replayable_and_requires_no_player_ordering_choice() {
    let mut g = game("Put all creatures on the bottom of their owners' libraries in a random order.", 2);
    for i in 0..6 { g.put(0, 2 + i % 2, Zone::Battlefield); }
    g.put(0, 1, Zone::Library);
    let mut cloned = g.clone();
    cast(&mut g, 0, None, Zone::Hand);
    cast(&mut cloned, 0, None, Zone::Hand);
    assert!(g.decision.is_none());
    assert_eq!(g.players[0].library, cloned.players[0].library);
    assert_eq!(names(&g, 0)[0], "Forest");
    assert_eq!(g.players[0].library.len(), 7);
}

#[test]
fn r701_24b_search_shuffle_and_put_on_top_keeps_the_found_card_out_of_the_shuffle() {
    let mut g = game("Search your library for a creature card, reveal it, then shuffle and put that card on top.", 2);
    for _ in 0..20 { g.put(0, 1, Zone::Library); }
    let found = g.put(0, 2, Zone::Library);
    cast(&mut g, 0, None, Zone::Hand);
    g.act(0, Move::Choose(Answer::Objects(vec![found]))).unwrap();
    cite!(["701.24b"]; assert_eq!(names(&g, 0)[0], "Bear"));
    assert_eq!(g.players[0].library.len(), 21);
    cite!(["701.24b"]; assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == found))));
    g.draw(0, 1);
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Bear");
}

#[test]
fn r701_24b_a_found_card_is_not_kept_aside_after_placement_or_a_declined_previous_move() {
    for (text, declined) in [
        ("Search your library for a card, put it on top of your library, then shuffle.", false),
        ("Search your library for a card. You may put that card into your hand. Shuffle.", true),
    ] {
        let mut g = game(text, 2);
        for _ in 0..20 { g.put(0, 1, Zone::Library); }
        let found = g.put(0, 2, Zone::Library);
        cast(&mut g, 0, None, Zone::Hand);
        g.act(0, Move::Choose(Answer::Objects(vec![found]))).unwrap();
        if declined { g.act(0, Move::Choose(Answer::No)).unwrap(); }
        assert_eq!(g.players[0].library.len(), 21);
        assert!(g.players[0].hand.is_empty());
        cite!(["701.24a"]; case: text; assert_ne!(names(&g, 0)[0], "Bear", "this fixed seed shuffles the found card away from the top"));
    }
}

#[test]
fn r702_34a_flashback_replaces_an_instructed_library_destination_with_exile() {
    let mut g = game("Put this spell on the bottom of its owner's library.\nFlashback {0}", 2);
    let card=g.put(0,0,Zone::Graveyard);
    g.act(0,Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    assert!(g.players[0].library.is_empty());
    cite!(["702.34a"]; assert_eq!(g.exile.len(), 1));
    assert_eq!(g.obj(g.exile[0]).chars.name, "Placement");
}

#[test]
fn r701_24b_search_shuffle_intermediate_library_operations_are_rejected_until_modeled() {
    let card = CardDef::new("Unsupported", "{0}", "Instant", "Search your library for a card. Shuffle. Draw a card. Put that card into your hand.", None).unwrap();
    assert!(game::play::check(&card).unwrap_err().0.contains("immediate destination"));
}

#[test]
fn r506_4_azorius_charm_puts_an_attacker_on_top_and_removes_it_from_combat() {
    let defs = vec![
        CardDef::new("Azorius Charm", "{W}{U}", "Instant", "Choose one —\n• Creatures you control gain lifelink until end of turn.\n• Draw a card.\n• Put target attacking or blocking creature on top of its owner's library.", None).unwrap(),
        CardDef::new("Raging Goblin", "{R}", "Creature — Goblin Berserker", "Haste", Some((1, 1))).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let goblin = g.put(0, 1, Zone::Battlefield);
    let plains = g.put(1, 2, Zone::Battlefield);
    let island = g.put(1, 3, Zone::Battlefield);
    while !matches!(g.decision, Some(Decision::Attackers { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(goblin, Target::Player(1))])).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source: plains, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(1, Move::Activate { source: island, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let charm = g.put(1, 0, Zone::Hand);
    g.act(1, paid("WU", Move::Modes(vec![2], Box::new(Move::Cast { card: charm, targets: vec![vec![Target::Object(goblin)]] })))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(names(&g, 0), ["Raging Goblin"]);
    cite!(["506.4"]; assert!(g.combat.attackers.is_empty()));
    assert_eq!(g.players[1].life, 20);
    assert_eq!(g.obj(goblin).zone, Zone::Gone);
}
