//! Reveal/choose/discard composes through one retained program (701.9, 701.20, 608.2).
use crate::support::cite;
use game::{card::CardDef, play::check, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

const DURESS: &str = "Target opponent reveals their hand. You choose a noncreature, nonland card from it. That player discards that card.";
const THOUGHTSEIZE: &str = "Target player reveals their hand. You choose a nonland card from it. That player discards that card. You lose 2 life.";

fn game(text: &str, seats: usize) -> Game {
    let defs = vec![
        CardDef::new("Witness", "{B}", "Sorcery", text, None).unwrap(),
        CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
        CardDef::new("Divination", "{2}{U}", "Sorcery", "Draw two cards.", None).unwrap(),
    ];
    for def in &defs { check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
    Game::new(defs, seats, 20)
}
fn cast(g: &mut Game, target: usize) -> ObjId {
    let land = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("B", Move::Cast { card: spell, targets: vec![vec![Target::Player(target)]] })).unwrap();
    let source = *g.stack.last().unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
    source
}
fn choice(g: &Game) -> (usize, Vec<ObjId>) {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Objects { among, count: 1, .. } }) => (*player, among.clone()),
        other => panic!("expected a single-card decision: {other:?}"),
    }
}
fn choose(g: &mut Game, card: ObjId) { g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap(); }

#[test]
fn r701_9b_caster_selects_from_revealed_opponents_hand_and_opponent_discards() {
    let mut g = game(DURESS, 3);
    let land = g.put(1, 1, Zone::Hand);
    let bear = g.put(1, 2, Zone::Hand);
    let shock = g.put(1, 3, Zone::Hand);
    let divination = g.put(1, 4, Zone::Hand);
    let other = g.put(2, 3, Zone::Hand);
    let source = cast(&mut g, 1);
    cite!(["701.9b"]; assert_eq!(choice(&g), (0, vec![shock, divination])));
    for seat in 0..3 {
        cite!(["701.20a"]; case: seat; assert!(g.sees(seat, land) && g.sees(seat, bear) && g.sees(seat, shock)));
        assert_eq!(g.observations(seat).len(), 1);
        assert_eq!(g.observations(seat)[0].cards.iter().map(|c| c.object).collect::<Vec<_>>(), vec![land,bear,shock,divination]);
    }
    assert!(!g.sees(0, other));
    let before = g.log.len();
    for (player, answer, why, cited) in [
        (1, Move::Choose(Answer::Objects(vec![shock])), "that decision is another player's", false),
        (0, Move::Choose(Answer::Objects(vec![land])), "a chosen object is not among the offered objects to discard (608.2d)", true),
        (0, Move::Choose(Answer::Objects(vec![shock,shock])), "choose each object to discard once (608.2d)", true),
        (0, Move::Pass, "a decision is pending: a choice of the effect being applied (608.2d)", false),
    ] {
        if cited { cite!(["608.2d"]; case: why; refused(why): g.act(player, answer)); }
        else { crate::support::refused(g.act(player, answer), why); }
        assert_eq!(g.log.len(), before);
    }
    choose(&mut g, divination);
    cite!(["701.9a"]; assert_eq!(g.players[1].hand, vec![land, bear, shock]));
    assert_eq!(g.players[2].hand, vec![other]);
    assert_eq!(g.players[1].graveyard.len(), 1);
    cite!(["701.9a"]; assert_eq!(g.obj(g.players[1].graveyard[0]).card, Some(4)));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Discarded { player: 1, .. })).count(), 1);
    assert!(g.log.iter().any(|e| matches!(e, Event::Discarded { player: 1, cause: Some(cause), .. } if cause.source == source && cause.controller == 0)));
    cite!(["701.20a"]; assert!(!g.sees(0, shock) && !g.sees(2, shock)));
    assert!(g.stack.is_empty() && g.resolving().is_none());
}

#[test]
fn r608_2c_thoughtseize_loses_life_after_a_creature_is_chosen() {
    let mut g = game(THOUGHTSEIZE, 2);
    let bear = g.put(1, 2, Zone::Hand);
    let shock = g.put(1, 3, Zone::Hand);
    let land = g.put(1, 1, Zone::Hand);
    cast(&mut g, 1);
    cite!(["608.2c"]; assert_eq!(choice(&g), (0, vec![bear, shock])));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 20));
    choose(&mut g, bear);
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 18));
    cite!(["701.9a"]; assert_eq!(g.players[1].hand, vec![shock, land]));
}

#[test]
fn r609_3_no_eligible_card_does_not_prevent_later_life_loss() {
    for hand in [vec![], vec![1]] {
        let mut g = game(THOUGHTSEIZE, 2);
        let hand: Vec<_> = hand.into_iter().map(|c| g.put(1, c, Zone::Hand)).collect();
        cast(&mut g, 1);
        cite!(["609.3"]; case: hand.len(); assert!(g.decision.is_none()));
        assert_eq!(g.players[1].hand, hand);
        cite!(["609.3"]; case: hand.len(); assert_eq!(g.players[0].life, 18));
        assert!(g.players[1].graveyard.is_empty());
    }
    let mut g = game(DURESS, 2);
    let bear = g.put(1, 2, Zone::Hand);
    cast(&mut g, 1);
    cite!(["609.3"]; assert!(g.decision.is_none()));
    cite!(["609.3"]; assert_eq!(g.players[1].hand, vec![bear]));
}

#[test]
fn r608_2d_a_single_eligible_card_needs_no_spurious_question() {
    let mut g = game(DURESS, 2);
    g.put(1, 3, Zone::Hand);
    cast(&mut g, 1);
    cite!(["608.2d"]; assert!(g.decision.is_none() && g.players[1].hand.is_empty()));
    cite!(["701.20a"]; assert_eq!(g.observations(0).len(), 1));
}

#[test]
fn r608_2c_cloned_selection_resumes_without_repeating_the_reveal() {
    let mut g = game(THOUGHTSEIZE, 2);
    let shock = g.put(1, 3, Zone::Hand);
    let divination = g.put(1, 4, Zone::Hand);
    cast(&mut g, 1);
    let mut copy = g.clone();
    choose(&mut g, shock);
    choose(&mut copy, divination);
    cite!(["608.2d"]; assert_eq!(g.players[1].hand, vec![divination]));
    cite!(["608.2d"]; assert_eq!(copy.players[1].hand, vec![shock]));
    cite!(["608.2c"]; assert_eq!((g.players[0].life, copy.players[0].life), (18, 18)));
    assert_eq!((g.observations(0).len(), copy.observations(0).len()), (1,1));
}

#[test]
fn r115_1a_thoughtseize_can_target_its_caster_but_duress_cannot() {
    let mut g = game(THOUGHTSEIZE, 2);
    let shock = g.put(0, 3, Zone::Hand);
    let divination = g.put(0, 4, Zone::Hand);
    cast(&mut g, 0);
    cite!(["115.1"]; assert_eq!(choice(&g), (0, vec![shock,divination])));
    choose(&mut g, shock);
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 18));
    let mut g = game(DURESS, 2);
    let land = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let spell = g.put(0, 0, Zone::Hand);
    cite!(["115.1"]; refused("an illegal target, you: you aren't an opponent (115.1)"): g.act(0, crate::support::paid("B", Move::Cast { card: spell, targets: vec![vec![Target::Player(0)]] })));
    assert_eq!(g.players[0].hand, vec![spell]);
    assert!(g.stack.is_empty());
}

#[test]
fn r608_2h_missing_and_ambiguous_group_references_are_refused() {
    for text in [
        "You choose a nonland card from it. You discard that card.",
        "Reveal your hand. Reveal your graveyard. You choose a nonland card from it. You discard that card.",
    ] {
        let def = CardDef::new("Unbound", "{B}", "Sorcery", text, None).unwrap();
        let reason = if text.starts_with("You choose") { "reference Anaphor ([]): no earlier instruction produces what it names" } else { "reference Anaphor ([]): more than one earlier group could be meant" };
        crate::support::refused(check(&def).map_err(|e|e.0), reason);
    }
}

#[test]
fn r701_9a_rule_discard_has_no_resolving_effect_cause() {
    let mut g = game(DURESS, 2);
    let card = g.put(1, 3, Zone::Hand);
    g.discard(1, card);
    cite!(["701.9a"]; assert!(g.log.iter().any(|e| matches!(e, Event::Discarded { player: 1, cause: None, .. }))));
}

#[test]
fn r608_2b_an_illegal_only_player_target_prevents_reveal_and_life_loss() {
    let mut g = game(THOUGHTSEIZE, 3);
    g.put(1, 3, Zone::Hand);
    let land = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("B", Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] })).unwrap();
    // A departed player cannot remain a legal target (800.4a).
    g.players[1].lost = true;
    g.act(0, Move::Pass).unwrap();
    g.act(2, Move::Pass).unwrap();
    cite!(["608.2b"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    cite!(["608.2b"]; assert_eq!(g.players[0].life, 20));
    cite!(["608.2b"]; assert!(g.observations(0).is_empty()));
    assert!(g.log.iter().any(|e| matches!(e, Event::Fizzled { .. })));
}

#[test]
fn r608_2d_selecting_zero_never_asks_and_does_not_bind_an_unselected_card() {
    let mut g = game("Target opponent reveals their hand. You choose zero cards from it. That player discards those cards.", 2);
    let shock = g.put(1, 3, Zone::Hand);
    cast(&mut g, 1);
    cite!(["608.2d"]; assert!(g.decision.is_none()));
    cite!(["608.2c"]; assert_eq!(g.players[1].hand, vec![shock]));
}

#[test]
fn r608_2c_chosen_hand_result_can_feed_exile_in_the_same_clause() {
    let mut g = game("Target opponent reveals their hand. You choose a nonland card from it and exile that card.", 2);
    let shock = g.put(1, 3, Zone::Hand);
    let bear = g.put(1, 2, Zone::Hand);
    cast(&mut g, 1);
    choose(&mut g, bear);
    cite!(["608.2c"]; assert_eq!(g.players[1].hand, vec![shock]));
    cite!(["608.2c"]; assert_eq!(g.exile.len(), 1));
    assert_eq!(g.obj(g.exile[0]).card, Some(2));
    assert!(g.players[1].graveyard.is_empty());
}

#[test]
fn r701_20a_hand_reveal_does_not_leak_a_following_private_scry() {
    let mut g = game("Target opponent reveals their hand. You choose a creature card from it. That player discards that card. Scry 1.", 3);
    let bear = g.put(1, 2, Zone::Hand);
    let bear2 = g.put(1, 2, Zone::Hand);
    let top = g.put(0, 4, Zone::Library);
    cast(&mut g, 1);
    choose(&mut g, bear);
    cite!(["701.22a"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { cards, .. } }) if cards == &vec![top])));
    cite!(["701.22a"]; assert!(g.sees(0, top)));
    cite!(["701.22a"]; assert!(!g.sees(1, top) && !g.sees(2, top)));
    assert_eq!((g.observations(0).len(),g.observations(1).len(),g.observations(2).len()), (2,1,1));
    cite!(["701.9a"]; assert_eq!(g.players[1].hand, vec![bear2]));
}
