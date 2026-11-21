//! Independent zone ownership, announcement, and shuffle edge cases.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, Target, Zone};
use ir::Part;

fn real(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap()
}
fn game(extra: CardDef) -> Game {
    let defs = vec![extra, CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() || g.decision.is_some() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not settle");
}
#[test]
fn r701_24d_feldons_cane_shuffles_even_an_empty_graveyard() {
    // Rules: 701.24d
    let mut g = game(real("Feldon's Cane"));
    let cane = g.put(0, 0, Zone::Battlefield);
    let from = g.log.len();
    g.act(0, Move::Activate { source: cane, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    cite!(["701.24d"]; assert!(g.log[from..].iter().any(|e| matches!(e, Event::Shuffled { player: 0, .. })), "empty set still shuffles the named library"));
}
#[test]
fn r701_24d_repopulate_shuffles_the_named_players_library_without_creatures() {
    // Rules: 701.24d
    let mut g = game(real("Repopulate"));
    let land = g.put(1, 2, Zone::Graveyard);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [5; 5];
    let from = g.log.len();
    g.act(0, paid("GG", Move::Cast { card, targets: vec![vec![Target::Player(1)]] })).unwrap();
    settle(&mut g);
    // which cards move and whose library is named is the card's text, not a rule: uncited
    assert!(g.players[1].graveyard.contains(&land));
    cite!(["701.24d"]; assert!(g.log[from..].iter().any(|e| matches!(e, Event::Shuffled { player: 1, .. })), "filtered empty set still shuffles the named library"));
    assert!(!g.log[from..].iter().any(|e| matches!(e, Event::Shuffled { player: 0, .. })));
}
#[test]
fn r400_3_a_card_returned_to_that_players_hand_still_goes_to_its_owners_hand() {
    // Rules: 400.3
    let d = CardDef::new("Zone review", "{0}", "Instant", "Target opponent loses 1 life. Return target creature to that player's hand.", None).unwrap();
    let mut g = game(d);
    let bear = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Object(bear)]] }).unwrap();
    settle(&mut g);
    cite!(["400.3"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(1))));
    cite!(["400.3"]; assert!(g.players[1].hand.is_empty()));
}
#[test]
fn r400_3_unbound_hand_destination_players_are_refused() {
    // Grammar admission guard; the legacy name does not establish 400.3 evidence.
    let d = CardDef::new("Unbound", "{0}", "Instant", "Return target creature to that player's hand.", None).unwrap();
    // Admission validation for an unbound pronoun, not 400.3 owner redirection.
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "input reference: nothing before it supplies the required participant/cardinality");
}
#[test]
fn r400_3_unbound_library_destination_players_are_refused() {
    // Grammar admission guard; the legacy name does not establish 400.3 evidence.
    let d = CardDef::new("Unbound", "{0}", "Instant", "Put target creature on top of that player's library.", None).unwrap();
    // Admission validation for an unbound pronoun, not 400.3 owner redirection.
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "input reference: nothing before it supplies the required participant/cardinality");
}

#[test]
fn r601_2c_a_player_named_only_by_the_destination_is_still_a_target() {
    // Rules: 601.2c, 400.3
    let d = CardDef::new("Destination review", "{0}", "Instant", "Return target creature to target player's hand.", None).unwrap();
    let mut g = game(d);
    let bear = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    // Both target mentions are announced even though 400.3 redirects the card.
    cite!(["601.2c"]; ok: g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bear)], vec![Target::Player(1)]] }));
    settle(&mut g);
    cite!(["400.3"]; assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(1))));
    cite!(["400.3"]; assert!(g.players[1].hand.is_empty()));
}

#[test]
fn r608_2b_an_unselected_optional_player_does_not_run_a_hand_placement() {
    // Rules: 601.2c
    // No player was targeted (not 608.2b's illegal targets); that the instruction then does nothing is
    // the card's text with no actor, which no rule states: uncited.
    let d = CardDef::new("Optional recipient", "{0}", "Instant", "Up to one target opponent puts a card from their hand on top of their library.", None).unwrap();
    let mut g = game(d);
    g.put(0, 1, Zone::Hand);
    g.put(1, 1, Zone::Hand);
    let hands = [g.players[0].hand.clone(), g.players[1].hand.clone()];
    let card = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; ok: g.act(0, Move::Cast { card, targets: vec![vec![]] }));
    settle(&mut g);
    assert!(g.decision.is_none());
    assert_eq!([g.players[0].hand.clone(), g.players[1].hand.clone()], hands);
}

#[test]
fn r400_3_bare_their_destinations_need_a_player_referent_too() {
    // Grammar admission guard; the legacy name does not establish 400.3 evidence.
    let mut admitted = Vec::new();
    for text in [
        "Return target creature to their hand.",
        "Put target creature into their graveyard.",
        "Put target creature on top of their library.",
        "Shuffle target creature into their library.",
    ] {
        let d = CardDef::new("Possessive review", "{0}", "Instant", text, None).unwrap();
        if game::play::check(&d).is_ok() { admitted.push(text); }
    }
    // Admission validation for unbound pronouns, not 400.3 owner redirection.
    assert!(admitted.is_empty(), "no player has supplied the possessive referent: {admitted:?}");
}

#[test]
fn r701_24_a_controller_instructed_to_shuffle_their_library_names_the_previous_opponent() {
    // These checks identify the player "their" names: Oracle-text reading, which no rule states
    // (608.2c orders instructions; 701.24a is randomization). Uncited.
    for text in [
        "Target opponent draws a card. Shuffle their graveyard into their library.",
        "Target opponent draws a card. You shuffle their graveyard into their library.",
    ] {
        let d = CardDef::new("Pronoun witness", "{0}", "Instant", text, None).unwrap();
        let mut g = game(d);
        let mine = g.put(0, 1, Zone::Graveyard);
        g.put(1, 1, Zone::Graveyard);
        let card = g.put(0, 0, Zone::Hand);
        let from = g.log.len();
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap();
        settle(&mut g);
        assert!(g.players[0].graveyard.contains(&mine), "{text}");
        assert!(g.players[1].graveyard.is_empty(), "{text}");
        let shuffled: Vec<_> = g.log[from..].iter().filter_map(|e| if let Event::Shuffled { player, .. } = e { Some(*player) } else { None }).collect();
        assert_eq!(shuffled, vec![1], "the explicit You actor does not turn third-person Their into Your: {text}");
    }
}
