//! Searching a hidden library, choosing cards, then following ordinary instructions (701.23).
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};

fn game() -> Game {
    let card = |name, text| CardDef::new(name, "{0}", "Sorcery", text, None).unwrap();
    let defs = vec![
        card("Growth", "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle."),
        card("Tutor", "Search your library for a card, put that card into your hand, then shuffle."),
        card("Several", "Search your library for three cards, put them into your hand, then shuffle."),
        card("Gift", "Search your library for a basic land card, reveal it, put it into your hand, then shuffle."),
        card("Later", "Search your library for a card, then shuffle. You may put that card into your hand."),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        card("Forget", "Reveal the top card of your library. Shuffle. You may put that card into your hand."),
        card("Empty", "Reveal the top card of your library. Search your library for a basic land card, put it into your hand, then shuffle."),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0)); }
    Game::new(defs, 2, 20)
}

fn cast(g: &mut Game, def: usize) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

fn search(g: &Game) -> (Vec<ObjId>, Vec<ObjId>, usize, usize) {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Search { cards, among, min, max } }) =>
            (cards.clone(), among.clone(), *min, *max),
        other => panic!("expected private library search: {other:?}"),
    }
}

fn choose(g: &mut Game, found: Vec<ObjId>) {
    g.act(0, Move::Choose(Answer::Objects(found))).unwrap();
}

#[test]
fn r701_23a_search_shows_the_whole_library_only_to_the_searcher_and_finds_matching_cards() {
    let mut g = game();
    let land = g.put(0, 5, Zone::Library);
    let bear = g.put(0, 6, Zone::Library);
    cast(&mut g, 0);
    cite!(["701.23a"]; assert_eq!(search(&g), (vec![land, bear], vec![land], 0, 1)));
    for card in [land, bear] { cite!(["701.23a"]; assert!(g.sees(0, card))); assert!(!g.sees(1, card)); }
    let before = format!("{g:?}");
    for (player, cards) in [(1, vec![land]), (0, vec![bear]), (0, vec![land, land])] {
        // another player's search; a card that isn't a land; the same card twice
        let result = g.act(player, Move::Choose(Answer::Objects(cards.clone())));
        if player == 1 { crate::support::refused(result, "that decision is another player's"); }
        else if cards.len() == 1 { cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a)"): result); }
        else { cite!(["608.2d"]; refused("each object is chosen once (608.2d)"): result); }
        assert_eq!(format!("{g:?}"), before, "an illegal answer must not mutate state");
    }
    choose(&mut g, vec![land]);
    assert_eq!(g.battlefield.len(), 1);
    let arrived = g.battlefield[0];
    assert_eq!(g.obj(arrived).chars.name, "Forest");
    assert!(g.obj(arrived).tapped);
    cite!(["400.7"]; assert_ne!(land, arrived));
    assert_eq!(g.players[0].library, vec![bear]);
    assert!(!g.sees(0, bear));
    assert!(g.sees(1, arrived));
}

#[test]
fn r701_23b_a_qualified_hidden_search_may_fail_even_when_a_matching_card_exists() {
    let mut g = game();
    let land = g.put(0, 5, Zone::Library);
    cast(&mut g, 0);
    choose(&mut g, vec![]);
    cite!(["701.23b"]; assert_eq!(g.players[0].library, vec![land]));
    cite!(["701.23b"]; assert!(g.battlefield.is_empty()));
    assert!(g.log.iter().any(|e| matches!(e, Event::Shuffled { player: 0, .. })));
}

#[test]
fn r701_23d_an_unqualified_search_must_find_as_many_as_possible() {
    for count in 0..=4 {
        let mut g = game();
        let cards: Vec<_> = (0..count).map(|_| g.put(0, 5, Zone::Library)).collect();
        cast(&mut g, 2);
        let (_, among, min, max) = search(&g);
        assert_eq!(among, cards);
        cite!(["701.23d"]; case: count; assert_eq!((min, max), (count.min(3), count.min(3))));
        if count > 0 { let n = count.min(3); cite!(["701.23d"]; case: count; refused(&format!("0 chosen (608.2d): choose between {n} and {n} distinct matching objects")): g.act(0, Move::Choose(Answer::Objects(vec![])))); }
        choose(&mut g, cards[..count.min(3)].to_vec());
        assert_eq!(g.players[0].hand.len(), count.min(3));
        assert_eq!(g.players[0].library.len(), count.saturating_sub(3));
    }
}

#[test]
fn r701_23e_found_cards_are_not_revealed_unless_the_instructions_say_so() {
    for (def, reveal) in [(1, false), (3, true)] {
        let mut g = game();
        let card = g.put(0, 5, Zone::Library);
        cast(&mut g, def);
        choose(&mut g, vec![card]);
        let shown: Vec<_> = g.log.iter().filter_map(|e| match e { Event::Revealed { cards, .. } => Some(cards.clone()), _ => None }).collect();
        cite!(["701.23e"]; case: def; assert_eq!(shown, if reveal { vec![vec![card]] } else { vec![] }));
        let hand = g.players[0].hand[0];
        assert!(g.sees(0, hand));
        assert!(!g.sees(1, hand));
    }
}

#[test]
fn r701_24b_found_cards_remain_set_aside_while_the_rest_of_the_library_is_shuffled() {
    let mut g = game();
    let cards: Vec<_> = (0..20).map(|_| g.put(0, 5, Zone::Library)).collect();
    cast(&mut g, 4);
    let mut clone = g.clone();
    choose(&mut g, vec![cards[7]]);
    choose(&mut clone, vec![cards[7]]);
    cite!(["701.24b"]; assert_eq!(g.players[0].library[7], cards[7]));
    assert_ne!(g.players[0].library, cards);
    assert_eq!(g.players[0].library, clone.players[0].library);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May { possible: true }, .. })));
    assert!(g.sees(0, cards[7]));
    assert!(!g.sees(1, cards[7]));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    clone.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(clone.players[0].library.len(), 20);
}

#[test]
fn r701_20d_a_shuffled_revealed_card_is_a_new_object_that_cannot_be_tracked() {
    let mut g = game();
    let old = g.put(0, 5, Zone::Library);
    cast(&mut g, 7);
    cite!(["701.20d"]; assert_eq!(g.obj(old).zone, Zone::Gone));
    let now = g.players[0].library[0];
    cite!(["701.20d"]; assert_ne!(old, now));
    cite!(["701.20d"]; assert!(!g.sees(0, now) && !g.sees(1, now)));
    assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == old)));
    // The old revealed identity no longer exists; moving it is impossible. It must not
    // be offered as an option (608.2d), and an unsolicited answer changes nothing.
    assert!(g.decision.is_none());
    crate::support::refused(g.act(0, Move::Choose(Answer::Yes)), "no decision is pending");
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].library, vec![now]);
}

#[test]
fn r701_23b_a_failed_search_does_not_move_a_previously_named_card() {
    let mut g = game();
    g.put(0, 6, Zone::Library);
    cast(&mut g, 8);
    choose(&mut g, vec![]);
    // Uncited (card text): "Search your library for a basic land card, put it into your hand": "it" is the card found, none was, so the revealed Bear stays (608.2k is of cost/trigger references)
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].library.len(), 1);
    assert_eq!(g.obj(g.players[0].library[0]).chars.name, "Bear");
}

#[test]
fn r701_23a_search_cannot_find_a_player_and_unsupported_zones_are_rejected_before_play() {

    assert_eq!(CardDef::new("Invalid", "{0}", "Instant", "Search your library for a player.", None).expect_err("search finds cards, not players (701.23a)"),"text the grammar does not read");
    let def = CardDef::new("Other", "{0}", "Instant", "Search your graveyard for a card.", None).unwrap();
    assert!(game::play::check(&def).unwrap_err().0.contains("only the controller searching their own library"));
    let def = CardDef::new("Combined", "{0}", "Instant", "Search your library for a card. Search your library for a card. Shuffle.", None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"search: combined searches before one shuffle (701.23h)");
}
