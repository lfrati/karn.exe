//! Owners arrange simultaneous arrivals (401.4, 404.3) before their retained caller resumes.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision, Placement};

fn game(text: &str, players: usize) -> Game {
    let card = |name, kind, text, pt| CardDef::new(name, "{0}", kind, text, pt).unwrap();
    let defs = vec![card("Effect", "Instant", text, None),
        card("First", "Creature — Bear", "", Some((2, 2))),
        card("Second", "Creature — Wolf", "", Some((2, 2))),
        card("Old", "Sorcery", "You gain 1 life.", None),
        card("Forest", "Basic Land — Forest", "", None),
        card("Island", "Basic Land — Island", "", None),
        card("Doomed", "Creature — Bear", "", Some((1, 0))),
        card("Control", "Instant", "Gain control of target creature.", None)];
    for def in &defs { game::play::check(def).unwrap(); }
    Game::new(defs, players, 20)
}
fn cast(game: &mut Game) {
    let actor = game.priority;
    let spell = game.put(actor, 0, Zone::Hand);
    game.act(actor, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..game.players.len() { game.act(game.priority, Move::Pass).unwrap(); }
}
fn order(game: &Game, owner: usize, expected: &[ObjId]) {
    cite!(["404.3"]; assert!(matches!(&game.decision, Some(Decision::Choose { player, choice: Choice::OrderCards {
        to: Placement::Zone(Zone::Graveyard), cards,
    } }) if *player == owner && cards == expected)));
}
fn names(game: &Game, owner: usize) -> Vec<String> {
    game.players[owner].graveyard.iter().map(|card| game.obj(*card).chars.name.clone()).collect()
}

#[test]
fn r404_3_discard_hand_orders_only_the_new_block_then_draws_once_in_pending_clones() {
    // Stoke Genius's instruction: discard the hand, then draw two. The old pile stays put;
    // no discard or draw commits until the owner answers the incoming block's order.
    let mut g = game("Discard your hand, then draw two cards.", 2);
    let old = [g.put(0, 3, Zone::Graveyard), g.put(0, 4, Zone::Graveyard)];
    let incoming = [g.put(0, 1, Zone::Hand), g.put(0, 2, Zone::Hand)];
    g.put(0, 4, Zone::Library); g.put(0, 5, Zone::Library);
    cast(&mut g);
    order(&g, 0, &incoming);
    cite!(["101.4a", "402.3"]; assert!(g.public_choices().next().is_none()));
    cite!(["404.2", "404.3"]; assert_eq!(g.players[0].graveyard, old));
    cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 2));
    let mut reversed = g.clone();
    for (state, permutation, expected) in [(&mut g, incoming.to_vec(), vec!["Old", "Forest", "Second", "First", "Effect"]),
        (&mut reversed, incoming.iter().rev().copied().collect(), vec!["Old", "Forest", "First", "Second", "Effect"])] {
        cite!(["404.3"]; ok: state.act(0, Move::Choose(Answer::Objects(permutation))));
        cite!(["404.2", "404.3", "608.2n"]; assert_eq!(names(state, 0), expected));
        cite!(["608.2c", "121.2"]; assert_eq!(state.players[0].hand.len(), 2));
        cite!(["608.2c"]; assert!(state.players[0].library.is_empty()));
        cite!(["701.9a"]; assert_eq!(state.log.iter().filter(|event| matches!(event, Event::Discarded { .. })).count(), 2));
    }
}

#[test]
fn r404_3_wrong_seat_and_invalid_permutations_preserve_every_pending_arrival() {
    let mut g = game("Destroy all creatures.", 2);
    let cards = [g.put(0, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield)];
    let outside = g.put(0, 4, Zone::Hand);
    cast(&mut g); order(&g, 0, &cards);
    let before = format!("{g:?}");
    cite!(["404.3"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(cards.to_vec()))));
    cite!(["404.3"]; assert_eq!(format!("{g:?}"), before));
    for invalid in [vec![cards[0]], vec![cards[0]; 2], vec![cards[0], outside]] {
        cite!(["404.3"]; refused("order every offered card exactly once (401.4, 404.3)"): g.act(0, Move::Choose(Answer::Objects(invalid))));
        cite!(["404.3"]; assert_eq!(format!("{g:?}"), before));
    }
}

#[test]
fn r101_4_owners_choose_in_apnap_order_before_any_simultaneous_death_commits() {
    let mut g = game("Destroy all creatures.", 3);
    g.active = 1; g.priority = 1;
    let groups: Vec<_> = (0..3).map(|owner| [g.put(owner, 1, Zone::Battlefield), g.put(owner, 2, Zone::Battlefield)]).collect();
    let control = g.put(2, 7, Zone::Hand);
    g.act(1, Move::Pass).unwrap();
    g.act(2, Move::Cast { card: control, targets: vec![vec![game::Target::Object(groups[0][0])]] }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["611.2a"]; assert_eq!(g.controller(groups[0][0]), 2));
    let old: Vec<_> = (0..3).map(|owner| {
        g.put(owner, 3, Zone::Graveyard); g.players[owner].graveyard.clone()
    }).collect();
    cast(&mut g);
    for owner in [1, 2, 0] {
        order(&g, owner, &groups[owner]);
        cite!(["101.4", "404.3"]; assert_eq!(g.battlefield.len(), 6));
        cite!(["404.2"]; assert_eq!(g.players[owner].graveyard, old[owner]));
        cite!(["101.4", "404.3"]; ok: g.act(owner, Move::Choose(Answer::Objects(groups[owner].to_vec()))));
        if owner != 0 {
            cite!(["101.4b", "404.3"]; assert!(g.public_choices().any(|choice| matches!(choice,
                game::state::PublicChoice::OrderCards { player, to: Placement::Zone(Zone::Graveyard), cards }
                    if *player == owner && cards == &groups[owner]))));
        }
    }
    cite!(["701.8a"]; assert!(g.battlefield.is_empty()));
    for owner in 0..3 {
        let expected = if owner == 1 { vec!["Old", "Second", "First", "Effect"] }
            else if owner == 2 { vec!["Control", "Old", "Second", "First"] }
            else { vec!["Old", "Second", "First"] };
        cite!(["400.3", "404.3"]; case: owner; assert_eq!(names(&g, owner), expected));
    }
}

#[test]
fn r404_3_redirected_cards_are_not_offered_and_singleton_arrivals_need_no_choice() {
    for normal in [1, 2] {
        let mut g = game("Destroy all creatures.", 2);
        let exile = g.put(0, 1, Zone::Battlefield);
        g.set_initial_counters(exile, "finality", 1);
        let cards: Vec<_> = (0..normal).map(|i| g.put(0, 1 + i, Zone::Battlefield)).collect();
        cast(&mut g);
        if normal == 2 {
            order(&g, 0, &cards);
            cite!(["404.3", "614.6"]; ok: g.act(0, Move::Choose(Answer::Objects(cards.clone()))));
        } else {
            cite!(["404.3"]; assert!(g.decision.is_none()));
        }
        cite!(["122.1h", "614.6"]; assert_eq!(g.exile.len(), 1));
        cite!(["404.3"]; assert_eq!(g.players[0].graveyard.len(), normal + 1));
    }
}

#[test]
fn r704_3_simultaneous_zero_toughness_deaths_retain_the_owner_order_without_repeating() {
    let mut g = game("You gain 1 life.", 2);
    let cards = [g.put(0, 6, Zone::Battlefield), g.put(0, 6, Zone::Battlefield)];
    cite!(["704.3", "704.5f"]; assert!(g.state_based_actions()));
    order(&g, 0, &cards);
    cite!(["704.3", "404.3"]; assert_eq!(g.battlefield, cards));
    let mut cloned = g.clone();
    for state in [&mut g, &mut cloned] {
        cite!(["404.3"]; ok: state.act(0, Move::Choose(Answer::Objects(cards.to_vec()))));
        cite!(["704.5f"]; assert!(state.battlefield.is_empty()));
        cite!(["704.3"]; assert_eq!(state.players[0].graveyard.len(), 2));
        cite!(["704.3"]; assert_eq!(state.log.iter().filter(|event| matches!(event, Event::ZoneChanged { to: Zone::Graveyard, .. })).count(), 2));
    }
}

#[test]
fn r701_17_milling_orders_the_incoming_block_before_the_next_instruction() {
    let mut g = game("Mill two cards. You gain 1 life.", 2);
    let bottom = g.put(0, 4, Zone::Library);
    let first = g.put(0, 1, Zone::Library);
    let second = g.put(0, 2, Zone::Library);
    cast(&mut g); order(&g, 0, &[second, first]);
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 20));
    cite!(["701.17a", "404.3"]; assert_eq!(g.players[0].library, vec![bottom, first, second]));
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(vec![first, second]))));
    cite!(["404.3"]; assert_eq!(names(&g, 0), ["Second", "First", "Effect"]));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 21));
    cite!(["701.17a"]; assert_eq!(g.log.iter().filter(|event| matches!(event, Event::Milled { .. })).count(), 2));
}

#[test]
fn r101_4a_simultaneous_hand_discard_orders_do_not_publish_hidden_cards_early() {
    let mut g = game("Each player discards two cards.", 2);
    let mine = [g.put(0, 1, Zone::Hand), g.put(0, 2, Zone::Hand)];
    let theirs = [g.put(1, 1, Zone::Hand), g.put(1, 2, Zone::Hand)];
    cast(&mut g); order(&g, 0, &mine);
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(mine.to_vec()))));
    order(&g, 1, &theirs);
    cite!(["101.4a", "402.3"]; assert!(g.public_choices().next().is_none()));
    cite!(["101.4", "701.9a"]; assert_eq!(g.players[0].hand, mine));
    cite!(["404.3"]; ok: g.act(1, Move::Choose(Answer::Objects(theirs.to_vec()))));
    cite!(["701.9a"]; assert!(g.players.iter().all(|player| player.hand.is_empty())));
}
