//! Review of the upkeep triggers whose intervening "if" counts the cards in the hand of the
//! player whose upkeep it is (603.4), which 58716e6 made trigger: each at its threshold and one
//! card past it, only at an opponent's upkeep where it says so. Every card is read from its
//! Oracle text.
use crate::support::*;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Davriel, Rogue Shadowmage", "Lavaborn Muse", "Paupers' Cage", "Misers' Cage", "Ebony Owl Netsuke", "Shrieking Affliction",
        "Hollowborn Barghest", "Asylum Visitor", "Quest for the Nihil Stone", "Island"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Pass until it is this player's `part` with an empty stack, declaring no attackers or
/// blockers, ordering triggers as offered, answering "you may" yes and discarding to hand size.
fn run_to(t: &mut T, p: usize, part: Part) -> usize {
    let mut mays = 0;
    for _ in 0..2000 {
        if t.g.active == p && t.g.part == part && t.g.stack.is_empty() && matches!(t.g.asked(), Some((_, Asked::Priority))) { return mays; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); continue; }
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); continue; }
            Some(Decision::Order { player, abilities }) => { t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); continue; }
            _ => {}
        }
        match t.g.asked() {
            Some((q, Asked::Priority)) => { t.g.act(q, Move::Pass).unwrap(); }
            Some((q, Asked::May)) => { mays += 1; t.answer(q, Answer::Yes).unwrap(); }
            Some((q, Asked::Discard { among, count })) => { t.g.act(q, Move::Discard(among[..count].to_vec())).unwrap(); }
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("never reached player {p}'s {part:?}");
}

// Rules: 603.4
// Given: player 0's Davriel, Rogue Shadowmage ("... if that player has one or fewer cards in
// hand, Davriel deals 2 damage to them."), Lavaborn Muse (two or fewer, 3 damage), Paupers' Cage
// (two or fewer, 2), Misers' Cage (five or more, 2), Ebony Owl Netsuke (seven or more, 4) or
// Shrieking Affliction (one or fewer, loses 3); player 1 with as many cards in hand as it asks,
// or one past it.
// When: player 1's upkeep, then player 0's.
// Expect: at the threshold player 1 is hurt; past it, not; player 0 never, at their own upkeep.
#[test]
fn r603_4_each_opponents_upkeep_counts_the_cards_in_that_players_hand() {
    for (card, at, past, hurt) in [("Davriel, Rogue Shadowmage", 1, 2, 2), ("Lavaborn Muse", 2, 3, 3), ("Paupers' Cage", 2, 3, 2), ("Misers' Cage", 5, 4, 2),
        ("Ebony Owl Netsuke", 7, 6, 4), ("Shrieking Affliction", 1, 2, 3)] {
        for cards in [at, past] {
            let mut board = Board::new(2).fill(10).battlefield(0, &[card]).hand(1, &vec!["Island"; cards]);
            if card.starts_with("Davriel") { board = board.counters(card, "loyalty", 4); }
            let mut t = board.build();
            run_to(&mut t, 1, Part::Draw);
            let case = format!("{card}, {cards} cards");
            cite!(["603.4"]; case: case.clone(); assert_eq!(t.g.players[1].life, if cards == at { 20 - hurt } else { 20 }));
            run_to(&mut t, 0, Part::Draw);
            cite!(["603.4"]; case: case; assert_eq!(t.g.players[0].life, 20));
        }
    }
}

// Rules: 603.4
// Given: player 0's Hollowborn Barghest ("At the beginning of your upkeep, if you have no cards
// in hand, each opponent loses 2 life. At the beginning of each opponent's upkeep, if that
// player has no cards in hand, they lose 2 life."); player 1 with no cards or one.
// When: player 1's upkeep, then player 0's with an empty hand.
// Expect: with none, player 1 loses 2 at their upkeep; with one, none. At player 0's upkeep,
// player 1 loses 2 either way.
#[test]
fn r603_4_hollowborn_barghest_counts_that_players_hand_and_yours() {
    for cards in [0, 1] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Hollowborn Barghest"]).hand(1, &vec!["Island"; cards]).build();
        run_to(&mut t, 1, Part::Draw);
        cite!(["603.4"]; case: format!("{cards} cards"); assert_eq!(t.g.players[1].life, if cards == 0 { 18 } else { 20 }));
        // player 0 has drawn nothing since: their hand is still empty
        let before = t.g.players[1].life;
        run_to(&mut t, 0, Part::Draw);
        cite!(["603.4"]; case: format!("{cards} cards"); assert_eq!(t.g.players[1].life, before - 2));
    }
}

// Rules: 603.4
// Given: player 0's Asylum Visitor ("At the beginning of each player's upkeep, if that player has
// no cards in hand, you draw a card and you lose 1 life."); player 1 with no cards or one.
// When: player 1's upkeep.
// Expect: with none, player 0 draws a card and loses 1 life; with one, nothing.
#[test]
fn r603_4_asylum_visitor_counts_the_hand_of_the_player_whose_upkeep_it_is() {
    for cards in [0, 1] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Asylum Visitor"]).hand(0, &["Island", "Island"]).hand(1, &vec!["Island"; cards]).build();
        run_to(&mut t, 1, Part::Draw);
        let fired = cards == 0;
        cite!(["603.4"]; case: format!("{cards} cards"); assert_eq!((t.g.players[0].hand.len(), t.g.players[0].life), if fired { (3, 19) } else { (2, 20) }));
    }
}

// Rules: 603.4
// Given: player 0's Quest for the Nihil Stone with two quest counters or one ("At the beginning of
// each opponent's upkeep, if that player has no cards in hand and this enchantment has two or
// more quest counters on it, you may have that player lose 5 life."); player 1 with no cards.
// When: player 1's upkeep, player 0 saying yes.
// Expect: with two counters player 1 loses 5; with one, it doesn't trigger and nothing is asked.
#[test]
fn r603_4_quest_for_the_nihil_stone_needs_an_empty_hand_and_two_counters() {
    for quests in [2, 1] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Quest for the Nihil Stone"]).counters("Quest for the Nihil Stone", "quest", quests).build();
        let mays = run_to(&mut t, 1, Part::Draw);
        let fired = quests == 2;
        cite!(["603.4"]; case: format!("{quests} counters"); assert_eq!((mays, t.g.players[1].life), if fired { (1, 15) } else { (0, 20) }));
    }
}
