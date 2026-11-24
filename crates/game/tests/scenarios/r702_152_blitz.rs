//! Blitz (702.152a): "You may cast this card by paying [cost] rather than its mana cost," "If
//! this spell's blitz cost was paid, sacrifice the permanent this spell becomes at the
//! beginning of the next end step," and "As long as this permanent's blitz cost was paid, it
//! has haste and 'When this permanent is put into a graveyard from the battlefield, draw a
//! card.'" Every card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::announcement::CastCost;
use game::state::Decision;
use game::{Game, Move, ObjId, Zone};
use ir::Part;

/// Riveteers Requisitioner, {1}{R} 3/1: "When this creature dies, create a Treasure token. /
/// Blitz {2}{R}", cast by player 0, for its blitz cost or its mana cost, and resolved.
fn fixture(blitz: bool) -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Riveteers Requisitioner"), oracle("Mountain")], 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, 1, Zone::Library); } }
    let card = g.put(0, 0, Zone::Hand);
    let (mana, spend) = if blitz { ("RRR", "RRR") } else { ("RR", "RR") };
    g.players[0].pool = mana_pool(mana);
    let cast = Move::Cast { card, targets: vec![] };
    // the alternative is named by its row: blitz is the card's second ability
    let cast = if blitz { Move::CastFor(CastCost::Alternative(1), Box::new(cast)) } else { cast };
    g.act(0, paid(spend, cast)).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.152a"]; assert!(g.stack.is_empty(), "the sacrifice is delayed, not an entry trigger"));
    let permanent = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    (g, permanent)
}
fn to_end_step(g: &mut Game) {
    for _ in 0..100 {
        if g.part == Part::End { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no end step");
}
/// Its controller puts the abilities that triggered together on the stack in the order it
/// chooses (603.3b), and the stack resolves.
fn resolve_all(g: &mut Game) {
    for _ in 0..20 {
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}
fn treasures(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).chars.name.starts_with("Treasure")).count() }

#[test]
fn the_cards_of_these_tests_run() {
    // Uncited (admission): the engine runs each card of these tests.
    assert_eq!(game::play::check(&oracle("Riveteers Requisitioner")).err().map(|u| u.0), None);
}

#[test]
fn r702_152a_blitzed_it_has_haste_is_sacrificed_at_the_next_end_step_and_draws_a_card() {
    let (mut g, requisitioner) = fixture(true);
    cite!(["702.152a"]; assert!(g.has(requisitioner, "Haste")));
    let hand = g.players[0].hand.len();
    to_end_step(&mut g);
    cite!(["702.152a", "603.7"]; assert_eq!(g.stack.len(), 1, "the delayed sacrifice triggers"));
    resolve_all(&mut g);
    cite!(["702.152a", "701.21a"]; assert!(!g.battlefield.contains(&requisitioner) && g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(0))));
    // both its own dies trigger and the one blitz gives it trigger (603.10a)
    cite!(["702.152a", "603.10a"]; assert_eq!((g.players[0].hand.len(), treasures(&g)), (hand + 1, 1)));
}

#[test]
fn r702_152a_cast_for_its_mana_cost_it_has_no_haste_and_stays() {
    let (mut g, requisitioner) = fixture(false);
    cite!(["702.152a"]; assert!(!g.has(requisitioner, "Haste")));
    to_end_step(&mut g);
    cite!(["702.152a"]; assert!(g.stack.is_empty()));
    cite!(["702.152a"]; assert!(g.battlefield.contains(&requisitioner)));
}

#[test]
fn r702_152a_it_draws_a_card_when_it_dies_any_way() {
    // Ruling: "The triggered ability that lets its controller draw a card triggers when it dies
    // for any reason, not just when you sacrifice it during the end step."
    let (mut g, requisitioner) = fixture(true);
    let hand = g.players[0].hand.len();
    g.destroy(requisitioner);
    // its dies triggers are put on the stack as the next player would receive priority
    g.act(g.priority, Move::Pass).unwrap();
    resolve_all(&mut g);
    cite!(["702.152a"]; assert_eq!((g.players[0].hand.len(), treasures(&g)), (hand + 1, 1)));
    // Ruling: "that permanent will be sacrificed only if it's still on the battlefield when
    // that triggered ability resolves. If it dies or goes to another zone before then, it will
    // stay where it is."
    let other = g.put(0, 0, Zone::Battlefield);
    to_end_step(&mut g);
    resolve_all(&mut g);
    cite!(["702.152a", "400.7"]; assert!(g.battlefield.contains(&other)));
}
