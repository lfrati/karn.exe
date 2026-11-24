//! Spectacle (702.137a): "You may pay [cost] rather than pay this spell's mana cost if an
//! opponent lost life this turn," an alternative cost (601.2b, 601.2f-h). Every card is read
//! from its Oracle text.
use crate::support;
use support::*;
use game::announcement::CastCost;

const SKEWER: &str = "Skewer the Critics";

/// Cast Skewer the Critics for its spectacle cost {R}, at player 1.
fn spectacle(t: &mut T) -> Result<(), String> {
    let card = t.find(SKEWER);
    t.mana(0, "{R}");
    t.g.act(0, paid("R", Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]] }))))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in [SKEWER, "Hackrobat", "Light Up the Stage"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_137a_without_an_opponent_losing_life_it_is_cast_for_its_mana_cost() {
    // Skewer the Critics: "Spectacle {R}. Skewer the Critics deals 3 damage to any target."
    let mut t = Board::new(2).hand(0, &[SKEWER]).build();
    let before = t.state();
    // no opponent lost life this turn
    cite!(["702.137a", "601.2b"]; refused("the condition for that alternative cost is not true"): spectacle(&mut t));
    // Uncited (implementation contract): a refused cast changes nothing.
    assert_eq!(t.state(), before);
    t.mana(0, "{2}{R}").cast(0, SKEWER, vec![Tgt::Player(1)], mana_pool("CCR")).unwrap();
    t.resolve();
    t.expect().rules(["601.2f"]).life(1, 17).check();
}

#[test]
fn r702_137a_after_an_opponent_lost_life_it_may_be_cast_for_its_spectacle_cost() {
    let mut t = Board::new(2).hand(0, &[SKEWER, "Shock"]).build();
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    t.expect().rules(["120.3a"]).life(1, 18).check();
    cite!(["702.137a", "601.2b", "601.2f"]; ok: spectacle(&mut t));
    t.resolve();
    t.expect().rules(["702.137a", "120.3a"]).life(1, 15).zone_exact(0, Zone::Graveyard, &["Shock", SKEWER]).check();
}

#[test]
fn r702_137a_an_opponent_who_gained_back_the_life_still_lost_it() {
    // Ruling: "Spectacle cares only that an opponent lost life during the turn, not that the
    // opponent's life total is currently lower than it was."
    let mut t = Board::new(2).hand(0, &[SKEWER, "Shock", "Healing Salve"]).build();
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    // Healing Salve: "Choose one — • Target player gains 3 life. • ..."
    let salve = t.find("Healing Salve");
    t.mana(0, "{W}");
    t.g.act(0, paid("W", Move::Modes(vec![0], Box::new(Move::Cast { card: salve, targets: vec![vec![Target::Player(1)]] })))).unwrap();
    t.resolve();
    t.expect().rules(["119.3"]).life(1, 21).check();
    cite!(["702.137a"]; ok: spectacle(&mut t));
}

#[test]
fn r702_137a_only_an_opponent_losing_life_counts() {
    // you losing life is not an opponent losing life
    let mut t = Board::new(2).hand(0, &[SKEWER, "Shock"]).build();
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(0)], mana_pool("R")).unwrap();
    t.resolve();
    t.expect().life(0, 18).check();
    // only you lost life
    cite!(["702.137a"]; refused("the condition for that alternative cost is not true"): spectacle(&mut t));
}

#[test]
fn r702_137a_a_permanent_knows_its_spectacle_cost_was_paid() {
    // Rix Maadi Reveler: "Spectacle {2}{B}{R}. When this creature enters, discard a card, then
    // draw a card. If this creature's spectacle cost was paid, instead discard your hand, then
    // draw three cards."
    let mut t = Board::new(2).fill(10).hand(0, &["Rix Maadi Reveler", "Shock", "Island", "Forest"]).build();
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Player(1)], mana_pool("R")).unwrap();
    t.resolve();
    let card = t.find("Rix Maadi Reveler");
    t.mana(0, "{2}{B}{R}");
    cite!(["702.137a", "601.2f"]; ok: t.g.act(0, paid("CCBR", Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: vec![] })))));
    // its hand is discarded: both cards go to the graveyard at once, in the order it chooses
    let order = t.settle();
    let Boundary::Question(0, Asked::OrderCards { cards, .. }) = order else { panic!("discards its hand: {order:?}") };
    cite!(["702.137a", "701.9a"]; ok: t.answer(0, Answer::Objects(cards)).map(|_| ()));
    t.resolve();
    t.expect().rules(["702.137a", "603.4"]).zone_exact(0, Zone::Graveyard, &["Forest", "Island", "Shock"]).check();
    cite!(["702.137a", "121.1"]; assert_eq!(t.g.players[0].hand.len(), 3));
}

#[test]
fn r702_137a_cast_for_its_mana_cost_the_spectacle_cost_was_not_paid() {
    let mut t = Board::new(2).fill(10).hand(0, &["Rix Maadi Reveler", "Island"]).build();
    t.mana(0, "{1}{R}").cast(0, "Rix Maadi Reveler", vec![], mana_pool("CR")).unwrap();
    t.resolve();
    t.resolve();
    t.expect().rules(["702.137a", "603.4"]).zone_exact(0, Zone::Graveyard, &["Island"]).check();
    cite!(["702.137a", "121.1"]; assert_eq!(t.g.players[0].hand.len(), 1));
}
