//! Sylvan chooses the drawn hand incarnations before separate life payments.
use crate::support::{cite, oracle};
use game::{Event, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
use ir::Part;

const LIBRARY: usize = 0;
const FOREST: usize = 1;
const DARKBLAST: usize = 5;
const LOAM: usize = 6;
const SHELL: usize = 7;

fn game() -> (Game, ObjId) {
    let defs = ["Sylvan Library", "Forest", "Island", "Mountain", "Plains", "Darkblast", "Life from the Loam", "Shambling Shell"]
        .into_iter().map(oracle).collect::<Vec<_>>();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for index in 0..30 { g.put(player, FOREST + index % 4, Zone::Library); } }
    let library = g.put(0, LIBRARY, Zone::Battlefield);
    (g, library)
}
fn passes(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn next_upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected turn question: {other:?}"),
        }
    }
    panic!("no upkeep");
}
fn to_draw(g: &mut Game) {
    for _ in 0..10 {
        if g.part == Part::Draw { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no draw step");
}
fn extra(g: &mut Game) {
    passes(g);
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
}
fn select(g: &mut Game, cards: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(cards))).unwrap(); }
fn offered(g: &Game, count: usize) -> Vec<ObjId> {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: n, .. } }) if *n == count => among.clone(),
        other => panic!("expected {count}-card choice: {other:?}"),
    }
}
fn payment(g: &Game) {
    assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, life: 4, .. } }) if cost.0.is_empty()), "{:?}", g.decision);
}
fn keep(g: &mut Game) {
    payment(g);
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();
}

#[test]
fn r504_1_normal_draw_precedes_the_trigger_and_declining_additional_draws_selects_nothing() {
    let (mut g, _) = game();
    let old = g.put(0, FOREST, Zone::Hand);
    next_upkeep(&mut g); to_draw(&mut g);
    cite!(["504.1"]; assert_eq!(g.players[0].hand.len(), 2));
    assert!(g.players[0].hand.contains(&old));
    assert_eq!(g.stack.len(), 1);
    passes(&mut g);
    cite!(["118.12"]; ok: g.act(0, Move::Choose(Answer::No)));
    assert!(g.decision.is_none());
    cite!(["608.2c"]; assert_eq!((g.players[0].hand.len(), g.players[0].life), (2, 20)));
}

#[test]
fn r608_2c_each_selected_card_has_a_separate_payment_and_the_return_order_is_chosen() {
    let (mut g, _) = game();
    let old = g.put(0, FOREST, Zone::Hand);
    next_upkeep(&mut g); to_draw(&mut g); extra(&mut g);
    let drawn = offered(&g, 2);
    cite!(["400.7", "121.1"]; assert_eq!(drawn.len(), 3));
    assert!(!drawn.contains(&old));
    // Both cards are chosen before any life payment or card return.
    select(&mut g, vec![drawn[0], drawn[1]]);
    let order = offered(&g, 1);
    assert_eq!(order, [drawn[0], drawn[1]]);
    select(&mut g, vec![drawn[1]]);
    payment(&g);
    let mut both_back = g.clone();
    keep(&mut g);
    cite!(["119.4"]; assert_eq!(g.players[0].life, 16));
    payment(&g);
    cite!(["608.2c"]; ok: g.act(0, Move::Choose(Answer::No)));
    assert_eq!(g.players[0].life, 16);
    assert!(g.players[0].hand.contains(&drawn[1]));
    assert!(!g.players[0].hand.contains(&drawn[0]));
    assert_eq!(g.obj(*g.players[0].library.last().unwrap()).card, g.obj(drawn[0]).card);
    for _ in 0..2 { both_back.act(0, Move::Choose(Answer::No)).unwrap(); }
    cite!(["119.4"]; assert_eq!(both_back.players[0].life, 20));
    let top = both_back.top_cards(0, 2).into_iter().map(|o| both_back.obj(o).card).collect::<Vec<_>>();
    cite!(["401.4"]; assert_eq!(top, [both_back.obj(drawn[0]).card, both_back.obj(drawn[1]).card]));
    assert!(g.decision.is_none()); assert!(both_back.decision.is_none());
}

#[test]
fn r119_4_unaffordable_life_payment_returns_each_card_and_source_departure_does_not_end_the_ability() {
    let (mut g, library) = game();
    next_upkeep(&mut g); to_draw(&mut g);
    g.move_object(library, Zone::Graveyard);
    g.players[0].life = 3;
    extra(&mut g);
    let drawn = offered(&g, 2);
    select(&mut g, vec![drawn[0], drawn[1]]);
    select(&mut g, vec![drawn[0]]);
    cite!(["113.7a"]; assert!(g.stack.is_empty()));
    cite!(["119.4"]; assert!(g.decision.is_none()));
    assert_eq!(g.players[0].life, 3);
    assert_eq!(g.players[0].hand, [drawn[2]]);
    let top = g.top_cards(0, 2).into_iter().map(|o| g.obj(o).card).collect::<Vec<_>>();
    cite!(["401.4"]; assert_eq!(top, [g.obj(drawn[1]).card, g.obj(drawn[0]).card]));
}

#[test]
fn r400_7_prior_draws_are_eligible_but_a_discarded_and_returned_incarnation_is_not() {
    let (mut g, _) = game();
    next_upkeep(&mut g);
    g.draw(0, 2);
    let prior = g.players[0].hand.clone();
    g.move_object(prior[0], Zone::Graveyard);
    let grave = *g.players[0].graveyard.last().unwrap();
    g.move_object(grave, Zone::Hand);
    let returned = *g.players[0].hand.last().unwrap();
    to_draw(&mut g); extra(&mut g);
    let drawn = offered(&g, 2);
    cite!(["400.7"]; assert!(!drawn.contains(&returned)));
    cite!(["121.1"]; assert!(drawn.contains(&prior[1])));
    select(&mut g, vec![prior[1], *drawn.iter().find(|o| **o != prior[1]).unwrap()]);
    select(&mut g, vec![prior[1]]);
    keep(&mut g); keep(&mut g);
    assert!(g.players[0].hand.contains(&returned));
    assert!(g.players[0].hand.contains(&prior[1]));
}

fn dredge(g: &mut Game, source: ObjId) {
    let replacement = match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, .. } }) => among.iter().find(|r| r.source == source).unwrap().clone(),
        other => panic!("expected draw replacement: {other:?}"),
    };
    g.act(0, Move::Choose(Answer::Replacement(replacement))).unwrap();
    if let Some(Decision::Choose { player: 0, choice: Choice::OrderCards { cards, .. } }) = g.decision.clone() {
        g.act(0, Move::Choose(Answer::Objects(cards))).unwrap();
    }
}

#[test]
fn r118_12_accepting_replaced_draws_still_selects_only_the_cards_actually_drawn() {
    for actual in [0, 1] {
        let (mut g, _) = game();
        next_upkeep(&mut g);
        let first = g.put(0, DARKBLAST, Zone::Graveyard);
        let second = g.put(0, LOAM, Zone::Graveyard);
        let third = (actual == 0).then(|| g.put(0, SHELL, Zone::Graveyard));
        to_draw(&mut g); dredge(&mut g, first);
        extra(&mut g); dredge(&mut g, second);
        if let Some(third) = third { dredge(&mut g, third); }
        cite!(["614.6"]; case: actual; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Drew { player: 0, .. })).count(), actual));
        if actual == 0 {
            cite!(["118.12"]; assert!(g.decision.is_none()));
            assert_eq!(g.players[0].life, 20);
        } else {
            // With only one eligible card, the required two-card choice does as much as possible.
            payment(&g);
            cite!(["609.3"]; assert_eq!(g.players[0].life, 20));
            keep(&mut g);
            assert!(g.decision.is_none());
            assert_eq!(g.players[0].life, 16);
        }
        assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(DARKBLAST)));
        assert!(g.players[0].hand.iter().any(|o| g.obj(*o).card == Some(LOAM)));
    }
}
