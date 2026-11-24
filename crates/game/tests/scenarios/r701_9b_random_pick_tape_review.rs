//! Review of a random pick on a replay tape (701.9b): Hymn to Tourach's two cards discarded at
//! random are captured as one random order of the whole hand, and that receipt, replayed,
//! discards the same two cards. A receipt that names only the cards discarded is not that
//! order, and is refused as replay input, with the game as it was.
use crate::support;
use game::simulation::{ResponseError, Simulation};
use game::start::{CapturedRandom, RandomKind, RandomOutcome};
use support::*;

/// Hymn to Tourach cast at player 1, who holds four cards, and passed by its caster: player 1's
/// pass resolves it.
fn hymn() -> (Simulation, Vec<ObjId>) {
    let cards = ["Lightning Bolt", "Giant Growth", "Mountain", "Forest"];
    let mut t = Board::new(2).hand(0, &["Hymn to Tourach"]).hand(1, &cards).build();
    let hand = cards.map(|n| t.find_in(1, Zone::Hand, n)).to_vec();
    t.mana(0, "{B}{B}").cast(0, "Hymn to Tourach", vec![player(1)], mana_pool("BB")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    (Simulation::new(t.g), hand)
}
/// The physical cards of player 1's graveyard, once the order of the two discarded is given
/// (404.3), sorted.
fn discarded(sim: &mut Simulation) -> Vec<u64> {
    let request = sim.request().unwrap().unwrap().clone();
    if let Asked::OrderCards { cards, .. } = &request.question {
        sim.respond(request.id, request.player, Move::Choose(Answer::Objects(cards.clone()))).unwrap();
    }
    let g = sim.game().unwrap();
    let mut cards: Vec<_> = g.players[1].graveyard.iter().map(|o| g.physical_card_identity(*o).unwrap()).collect();
    cards.sort();
    cards
}

// Rules: 701.9b, 608.2c
// Given: Hymn to Tourach resolving at player 1, who holds four cards.
// When: a copy of the game resolves it capturing its random outcome; the outcome is replayed
// on the game.
// Expect: one order of all four cards is captured; replaying it consumes it and discards the
// same two cards, the first two of that order.
#[test]
fn r701_9b_a_captured_random_pick_replays_to_the_same_discards() {
    let (mut sim, hand) = hymn();
    let physical: Vec<_> = hand.iter().map(|o| sim.game().unwrap().physical_card_identity(*o).unwrap()).collect();
    let request = sim.request().unwrap().unwrap().clone();
    let mut recording = sim.clone();
    let captured = recording.respond_capturing_random(request.id, 1, Move::Pass).unwrap();
    let [CapturedRandom::Order { kind: RandomKind::RandomOrder, player: 1, position: None, order }] = captured.as_slice() else {
        panic!("one random order of player 1's hand: {captured:?}")
    };
    cite!(["701.9b"]; assert_eq!(order.len(), 4, "the whole hand"));
    let mut first: Vec<_> = order[..2].to_vec();
    first.sort();
    cite!(["701.9b", "608.2c"]; assert_eq!(discarded(&mut recording), first));
    // the receipt names physical cards; replay names the objects they are now
    let replayed = order.iter().map(|card| hand[physical.iter().position(|p| p == card).unwrap()]).collect();
    let receipt = RandomOutcome::RandomOrder { player: Some(1), order: replayed };
    let unused = sim.respond_with_random(request.id, 1, Move::Pass, vec![receipt], false).unwrap();
    cite!(["701.9b"]; assert!(unused.is_empty()));
    cite!(["701.9b", "608.2c"]; assert_eq!(discarded(&mut sim), first));
}

// Rules: 701.9b
// Given: Hymn to Tourach resolving at player 1, who holds four cards.
// When: it is replayed with a receipt naming only two of them, in an order.
// Expect: that is not an order of the cards picked from; it is refused as replay input, and
// player 1 still holds all four cards with Hymn on the stack.
#[test]
fn r701_9b_a_receipt_of_only_the_discarded_cards_is_refused() {
    let (mut sim, hand) = hymn();
    let request = sim.request().unwrap().unwrap().clone();
    let receipt = RandomOutcome::RandomOrder { player: Some(1), order: hand[..2].to_vec() };
    let refused = sim.respond_with_random(request.id, 1, Move::Pass, vec![receipt], false);
    cite!(["701.9b"]; assert!(matches!(refused, Err(ResponseError::ReplayInput(_))), "{:?}", refused.map(|_| ())));
    let g = sim.game().unwrap();
    cite!(["701.9b"]; assert_eq!((g.players[1].hand.len(), g.stack.len()), (4, 1)));
}
