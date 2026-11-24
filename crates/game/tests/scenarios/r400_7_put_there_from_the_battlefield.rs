//! "All permanent cards in your graveyard that were put there from the battlefield this turn":
//! the card in the graveyard is a new object (400.7), and the permanent it was is what the event
//! is said of, as it last was on the battlefield (603.10a). The card is still the one that was
//! put there, so it is found. Every card is read from its Oracle text.
use crate::support;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Faith's Reward", "Grim Return", "Shock", "Murder", "Grizzly Bears", "Hill Giant"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// The names of the cards in that player's zone.
fn names(t: &T, p: usize, zone: Zone) -> Vec<String> {
    let cards = match zone {
        Zone::Graveyard => t.g.players[p].graveyard.clone(),
        Zone::Battlefield => t.g.battlefield.iter().copied().filter(|o| t.g.controller(*o) == p).collect(),
        _ => unreachable!(),
    };
    let mut names: Vec<String> = cards.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

// Rules: 400.7, 603.10a, 608.2c
// Given: player 0's Grizzly Bears on the battlefield, and Hill Giant already in their graveyard.
// When: player 0 casts Shock or Murder at the Bears, then casts Faith's Reward.
// Expect: the Bears was put into the graveyard from the battlefield this turn and returns; the
// Hill Giant was not and stays.
#[test]
fn r400_7_faiths_reward_returns_the_card_put_there_from_the_battlefield() {
    // Faith's Reward: "Return to the battlefield all permanent cards in your graveyard that were
    // put there from the battlefield this turn."
    for (removal, cost, pool) in [("Shock", "{R}", "R"), ("Murder", "{1}{B}{B}", "CBB")] {
        let mut t = Board::new(2).fill(8).hand(0, &[removal, "Faith's Reward"]).battlefield(0, &["Grizzly Bears"]).graveyard(0, &["Hill Giant"]).build();
        let bears = t.find_in_battlefield("Grizzly Bears");
        t.mana(0, cost).cast(0, removal, vec![Tgt::Object(bears)], mana_pool(pool)).unwrap();
        t.resolve();
        assert_eq!(names(&t, 0, Zone::Graveyard), ["Grizzly Bears", "Hill Giant", removal]);
        t.mana(0, "{3}{W}").cast(0, "Faith's Reward", vec![], mana_pool("CCCW")).unwrap();
        t.resolve();
        cite!(["400.7", "603.10a", "608.2c"]; case: format!("{removal}"); assert_eq!(names(&t, 0, Zone::Battlefield), ["Grizzly Bears"]));
        cite!(["400.7"]; case: format!("{removal}"); assert_eq!(names(&t, 0, Zone::Graveyard), ["Faith's Reward", "Hill Giant", removal]));
    }
}

// Rules: 400.7, 603.10a, 115.1
// Given: player 1's Grizzly Bears on the battlefield, and Hill Giant already in their graveyard.
// When: player 0 Shocks the Bears, then casts Grim Return at the Hill Giant, or at the Bears.
// Expect: the Hill Giant can't be its target; the Bears, put into a graveyard from the
// battlefield this turn, can, and enters under player 0's control.
#[test]
fn r400_7_grim_return_can_target_only_the_card_put_there_from_the_battlefield() {
    // Grim Return: "Choose target creature card in a graveyard that was put there from the
    // battlefield this turn. Put that card onto the battlefield under your control."
    let mut t = Board::new(2).fill(8).hand(0, &["Shock", "Grim Return"]).battlefield(1, &["Grizzly Bears"]).graveyard(1, &["Hill Giant"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    t.resolve();
    let (giant, bears) = (t.find_in(1, Zone::Graveyard, "Hill Giant"), t.find_in(1, Zone::Graveyard, "Grizzly Bears"));
    let grim = t.find_in(0, Zone::Hand, "Grim Return");
    t.mana(0, "{2}{B}");
    let cast = |t: &mut T, card: ObjId| t.g.act(0, paid("CCB", Move::Cast { card: grim, targets: vec![vec![Target::Object(card)]] }));
    // the Hill Giant was not put there this turn
    cite!(["400.7", "115.1"]; refused("an illegal target"): cast(&mut t.clone(), giant));
    cite!(["400.7", "603.10a"]; assert!(cast(&mut t, bears).is_ok()));
    t.resolve();
    cite!(["400.7", "603.10a"]; assert_eq!(names(&t, 0, Zone::Battlefield), ["Grizzly Bears"]));
    cite!(["400.7"]; assert_eq!(names(&t, 1, Zone::Graveyard), ["Hill Giant"]));
}
