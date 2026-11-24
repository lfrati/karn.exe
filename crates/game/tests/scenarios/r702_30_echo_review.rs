//! Review of an echo cost that is not mana (702.30a): Skizzik Surger's "Echo—Sacrifice two
//! lands." is paid with two lands, all of it or none (118.3).
use crate::support;
use support::*;

/// Player 0's next upkeep, its echo trigger resolved up to the first question it asks.
fn upkeep(t: &mut T) -> Boundary {
    let turn = t.g.turn;
    for _ in 0..2000 {
        if t.g.turn > turn && t.g.active == 0 && t.g.part == Part::Upkeep && !t.g.stack.is_empty() { return t.settle(); }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
        }
    }
    panic!("no upkeep of player 0 with its echo trigger on the stack");
}
/// Skizzik Surger cast by player 0, who controls these Mountains.
fn surger(lands: &[&str]) -> T {
    let mut t = Board::new(2).fill(10).hand(0, &["Skizzik Surger"]).battlefield(0, lands).build();
    t.mana(0, "{4}{R}{R}").cast(0, "Skizzik Surger", vec![], mana_pool("CCCCRR")).unwrap();
    t.resolve();
    t
}

#[test]
fn the_cards_of_these_tests_run() {
    // Uncited (admission): the engine runs each card of these tests.
    assert_eq!(game::play::check(&oracle("Skizzik Surger")).err().map(|u| u.0), None);
}

// Rules: 702.30a, 118.12, 701.21a
// Given: Skizzik Surger and two Mountains on player 0's battlefield.
// When: its echo trigger resolves at player 0's next upkeep and they pay.
// Expect: the two lands are sacrificed, and the Surger stays.
#[test]
fn r702_30a_skizzik_surgers_echo_sacrifices_two_lands() {
    // Skizzik Surger: "Haste. Echo—Sacrifice two lands."
    let mut t = surger(&["Mountain", "Mountain"]);
    let asked = upkeep(&mut t);
    cite!(["702.30a", "118.12"]; assert!(matches!(asked, Boundary::Question(0, Asked::May)), "pay or not: {asked:?}"));
    t.answer(0, Answer::Yes).unwrap();
    if let Boundary::Question(0, Asked::OrderCards { cards, .. }) = t.settle() { t.answer(0, Answer::Objects(cards)).unwrap(); }
    t.expect().rules(["702.30a", "701.21a"]).zone_exact(0, Zone::Battlefield, &["Skizzik Surger"])
        .zone_exact(0, Zone::Graveyard, &["Mountain", "Mountain"]).check();
}

// Rules: 118.3, 702.30a
// Given: Skizzik Surger and a single Mountain.
// When: its echo trigger resolves.
// Expect: one land is not the cost: nothing is asked, no land is sacrificed, and the Surger is.
#[test]
fn r118_3_with_one_land_the_echo_cost_cannot_be_paid() {
    let mut t = surger(&["Mountain"]);
    let asked = upkeep(&mut t);
    cite!(["118.3", "702.30a"]; assert!(matches!(asked, Boundary::Priority(_)), "nothing to pay with: {asked:?}"));
    t.expect().rules(["702.30a", "118.3"]).zone_exact(0, Zone::Battlefield, &["Mountain"]).zone_exact(0, Zone::Graveyard, &["Skizzik Surger"]).check();
}
