//! "As long as you have one or fewer cards in hand", "if an opponent has eight or more cards in
//! their graveyard", "if you have 200 or more cards in your library": how many cards are in a
//! player's own hand, graveyard or library, the cards they own there (400.3, 402.1, 404.1), and
//! "unless you have two or more opponents": how many opponents they have (102.3).
use crate::support;
use support::*;

/// Pass until attackers are to be declared.
fn to_attackers(t: &mut T) {
    for _ in 0..20 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    panic!("never reached a declare attackers step");
}

// Rules: 611.3a, 402.1
// Given: player 0 controls Thresher Lizard ("This creature gets +1/+2 as long as you have one or
// fewer cards in hand.", 3/2) and holds two Grizzly Bears; player 1 holds three cards.
// When: player 0 casts one of the Bears.
// Expect: the Lizard is 3/2 with two cards in hand and 4/4 with one; player 1's hand is not
// counted.
#[test]
fn r611_3a_thresher_lizard_counts_the_cards_in_your_hand() {
    let mut t = Board::new(2).battlefield(0, &["Thresher Lizard"]).hand(0, &["Grizzly Bears", "Grizzly Bears"])
        .hand(1, &["Grizzly Bears"; 3]).fill(10).build();
    t.expect().rules(["611.3a", "402.1"]).pt("Thresher Lizard", 3, 2).check();
    let bears = t.g.players[0].hand[0];
    t.mana(0, "{1}{G}");
    t.g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })).unwrap();
    t.expect().rules(["611.3a", "402.1"]).pt("Thresher Lizard", 4, 4).check();
}

// Rules: 603.4
// Given: player 0 holds Bloodhall Priest ("Whenever this creature enters or attacks, if you have
// no cards in hand, this creature deals 2 damage to any target."), alone or with Grizzly Bears.
// When: player 0 casts the Priest and it resolves.
// Expect: with no other card in hand its ability triggers and deals 2 damage to player 1; with
// the Bears still in hand it doesn't trigger.
#[test]
fn r603_4_bloodhall_priest_triggers_only_with_no_cards_in_hand() {
    for extra in [false, true] {
        let hand: &[&str] = if extra { &["Bloodhall Priest", "Grizzly Bears"] } else { &["Bloodhall Priest"] };
        let mut t = Board::new(2).hand(0, hand).fill(10).build();
        t.mana(0, "{2}{B}{R}").cast(0, "Bloodhall Priest", vec![], mana_pool("CCBR")).unwrap();
        let boundary = t.settle();
        if extra {
            cite!(["603.4"]; assert!(matches!(boundary, Boundary::Priority(_)) && t.g.stack.is_empty(), "{boundary:?}"));
            cite!(["603.4"]; assert_eq!(t.g.players[1].life, 20));
        } else {
            cite!(["603.4", "603.3d"]; assert!(matches!(t.g.decision, Some(Decision::Targets { player: 0, .. })), "{boundary:?}"));
            t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
            t.resolve();
            cite!(["603.4"]; assert_eq!(t.g.players[1].life, 18));
        }
    }
}

// Rules: 508.1c, 400.3
// Given: player 1 controls Relic Golem ("This creature can't attack or block unless an opponent
// has eight or more cards in their graveyard.") and Grizzly Bears.
// When: player 1 declares attackers with eight cards in player 0's graveyard, or with seven
// there and eight in player 1's own.
// Expect: the Golem may attack only in the first case: its own controller's graveyard is not
// an opponent's.
#[test]
fn r508_1c_relic_golem_attacks_only_when_an_opponent_has_eight_cards_in_their_graveyard() {
    for (theirs, own, able) in [(8, 0, true), (7, 8, false)] {
        let mut t = Board::new(2).battlefield(1, &["Relic Golem", "Grizzly Bears"])
            .graveyard(0, &vec!["Wastes"; theirs]).graveyard(1, &vec!["Wastes"; own]).fill(10).build();
        let golem = t.find_in_battlefield("Relic Golem");
        t.to_no_combat(1, Part::PrecombatMain);
        to_attackers(&mut t);
        let Some((_, Asked::Attackers(may))) = t.asked() else { panic!("{:?}", t.asked()) };
        cite!(["508.1c", "400.3"]; case: format!("{theirs} theirs, {own} own"); assert_eq!(may.iter().any(|a| a.creature == golem), able));
    }
}

// Rules: 603.4, 402.1
// Given: player 0 controls Hellfire Mongrel ("At the beginning of each opponent's upkeep, if that
// player has two or fewer cards in hand, this creature deals 2 damage to that player."); player
// 1 holds two or three cards.
// When: player 1's upkeep begins.
// Expect: with two cards player 1 is dealt 2 damage; with three the ability doesn't trigger.
#[test]
fn r603_4_hellfire_mongrel_counts_that_players_hand() {
    for (cards, life) in [(2, 18), (3, 20)] {
        let mut t = Board::new(2).battlefield(0, &["Hellfire Mongrel"]).hand(0, &["Grizzly Bears"; 5])
            .hand(1, &vec!["Grizzly Bears"; cards]).fill(10).build();
        t.to_no_combat(1, Part::Draw);
        cite!(["603.4", "402.1"]; case: format!("{cards} cards"); assert_eq!(t.g.players[1].life, life));
    }
}

// Rules: 104.2b, 603.4, 401.1
// Given: player 0 controls Battle of Wits ("At the beginning of your upkeep, if you have 200 or
// more cards in your library, you win the game.") with 200 or 199 cards in their library.
// When: player 0's next upkeep begins.
// Expect: with 200 cards player 0 wins the game; with 199 the game goes on.
#[test]
fn r104_2b_battle_of_wits_wins_with_two_hundred_cards_in_your_library() {
    for (cards, wins) in [(200, true), (199, false)] {
        let mut t = Board::new(2).battlefield(0, &["Battle of Wits"]).fill(cards).build();
        if wins { t.expect_losses(&[1]); }
        t.to_no_combat(1, Part::PrecombatMain);
        for _ in 0..200 {
            if t.g.finished() || t.g.active == 0 && t.g.part == Part::Draw { break; }
            match t.g.decision.clone() {
                Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![])).unwrap(),
                Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
                _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
            }
        }
        cite!(["104.2b", "603.4"]; case: format!("{cards} cards"); assert_eq!(t.g.winner(), wins.then_some(0)));
    }
}

// Rules: 614.1d, 102.3
// Given: player 0 holds Spire Garden ("This land enters tapped unless you have two or more
// opponents.").
// When: player 0 plays it, in a game of two players or of three.
// Expect: it enters tapped with one opponent and untapped with two.
#[test]
fn r614_1d_spire_garden_enters_untapped_with_two_or_more_opponents() {
    for (players, tapped) in [(2, true), (3, false)] {
        let mut t = Board::new(players).hand(0, &["Spire Garden"]).fill(10).build();
        let land = t.find_in(0, Zone::Hand, "Spire Garden");
        t.g.act(0, Move::PlayLand(land)).unwrap();
        cite!(["614.1d", "102.3"]; case: format!("{players} players"); assert_eq!(t.g.obj(t.find_in_battlefield("Spire Garden")).tapped, tapped));
    }
}

// Rules: 118.12a, 608.2c
// Given: player 0 casts Grizzly Bears; player 1 casts Anticognition on it ("Counter target
// creature or planeswalker spell unless its controller pays {2}. If an opponent has eight or
// more cards in their graveyard, instead counter that spell, then scry 2.").
// When: Anticognition resolves with seven or with eight cards in player 0's graveyard.
// Expect: with seven, player 0 is offered to pay {2}; with eight, the Bears is countered with no
// offer and player 1 scries 2.
#[test]
fn r608_2c_anticognition_counters_outright_when_an_opponent_has_eight_cards_in_their_graveyard() {
    for (cards, outright) in [(7, false), (8, true)] {
        let mut t = Board::new(2).hand(0, &["Grizzly Bears"]).hand(1, &["Anticognition"])
            .graveyard(0, &vec!["Wastes"; cards]).fill(10).build();
        t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
        let bears = *t.g.stack.last().unwrap();
        t.g.act(0, Move::Pass).unwrap();
        t.mana(1, "{1}{U}").cast(1, "Anticognition", vec![Tgt::Object(bears)], mana_pool("CU")).unwrap();
        t.pass_all();
        if outright {
            cite!(["608.2c"]; assert!(matches!(t.asked(), Some((1, Asked::Arrange { .. }))), "{:?}", t.asked()));
            let Some((_, Asked::Arrange { cards, .. })) = t.asked() else { unreachable!() };
            t.g.act(1, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap();
            t.expect().rules(["608.2c", "701.6a"]).zone(0, Zone::Battlefield, &[]).contains(0, Zone::Graveyard, "Grizzly Bears").check();
        } else {
            cite!(["118.12a"]; assert!(matches!(t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } })), "{:?}", t.g.decision));
        }
    }
}

// Rules: 603.4, 513.2
// Given: player 0 controls Keeper of the Accord ("At the beginning of each opponent's end step,
// if that player controls more creatures than you, create a 1/1 white Soldier creature
// token."); player 1 controls two Grizzly Bears or one.
// When: player 1's end step begins.
// Expect: "that player" is the player whose end step it is: with two Bears (more creatures
// than player 0's one) player 0 creates a Soldier; with one Bears, none.
#[test]
fn r603_4_keeper_of_the_accord_compares_with_the_player_whose_end_step_it_is() {
    for (bears, soldiers) in [(2, 1), (1, 0)] {
        let mut t = Board::new(2).battlefield(0, &["Keeper of the Accord"]).battlefield(1, &vec!["Grizzly Bears"; bears]).fill(10).build();
        t.to_no_combat(1, Part::End);
        let made = t.g.battlefield.iter().filter(|o| t.g.obj(**o).token() && t.g.controller(**o) == 0).count();
        cite!(["603.4", "513.2"]; case: format!("{bears} Bears"); assert_eq!(made, soldiers));
    }
}
