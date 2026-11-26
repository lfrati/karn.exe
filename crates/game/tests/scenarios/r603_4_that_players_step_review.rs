//! Review of the cards batch 17 made run or changed: an upkeep or end step trigger whose
//! intervening "if" asks about "that player", the player whose step it is (603.4), checked as
//! it triggers and again as it resolves (their rulings); Sheoldred, Whispering One; the
//! hellbent "instead" spells (if you have no cards in hand, as they resolve); and Library of
//! Alexandria's "exactly seven cards in hand". Every card is read from its Oracle text.
use crate::support::*;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Greener Pastures", "Rivalry", "Spiritual Sanctuary", "Razor Pendulum", "Urborg Stalker", "Sheoldred, Whispering One",
        "Cackling Flames", "Twinstrike", "Infernal Tutor", "Library of Alexandria", "Lightning Bolt", "Revitalize",
        "Grizzly Bears", "Hill Giant", "Sol Ring", "Mountain", "Plains", "Island", "Swamp"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn tokens(t: &T, p: usize) -> usize { t.g.battlefield.iter().filter(|o| t.g.obj(**o).token() && t.g.controller(**o) == p).count() }

/// Pass until it is this player's `part` with an empty stack, declaring no attackers or
/// blockers and handing every other question to `answer`.
fn run_to(t: &mut T, p: usize, part: Part, answer: &mut dyn FnMut(&mut T, usize, Asked)) {
    for _ in 0..2000 {
        if t.g.active == p && t.g.part == part && t.g.stack.is_empty() && matches!(t.g.asked(), Some((_, Asked::Priority))) { return; }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); continue; }
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); continue; }
            Some(Decision::Order { player, abilities }) => { t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); continue; }
            _ => {}
        }
        match t.g.asked() {
            Some((q, Asked::Priority)) => { t.g.act(q, Move::Pass).unwrap(); }
            Some((q, asked)) => answer(t, q, asked),
            None => panic!("the game ended"),
        }
    }
    panic!("never reached player {p}'s {part:?}");
}

fn no_questions(_: &mut T, p: usize, asked: Asked) { panic!("unexpected question to player {p}: {asked:?}") }

// Rules: 603.4
// Given: three players; player 0's Rivalry ("At the beginning of each player's upkeep, if that
// player controls more lands than each other player, this enchantment deals 2 damage to them.")
// and Greener Pastures ("... the player creates a 1/1 green Saproling creature token."); two
// Mountains of player 0's, three or two of player 1's, one of player 2's.
// When: player 1's upkeep, then player 2's and player 0's.
// Expect: "each other player" is each player but that one: with three, player 1 is dealt 2
// damage and creates a Saproling; with two, as many as player 0, neither. Players 2 and 0
// never control the most.
#[test]
fn r603_4_rivalry_and_greener_pastures_ask_of_the_player_whose_upkeep_it_is() {
    for (lands, hurt) in [(3, true), (2, false)] {
        let mut t = Board::new(3).fill(10).battlefield(0, &["Rivalry", "Greener Pastures", "Mountain", "Mountain"]).battlefield(1, &vec!["Mountain"; lands])
            .battlefield(2, &["Mountain"]).build();
        run_to(&mut t, 1, Part::Draw, &mut no_questions);
        let case = format!("{lands} lands");
        cite!(["603.4"]; case: case.clone(); assert_eq!((t.g.players[1].life, tokens(&t, 1)), if hurt { (18, 1) } else { (20, 0) }));
        run_to(&mut t, 0, Part::Draw, &mut no_questions);
        cite!(["603.4"]; case: case; assert_eq!((t.g.players[2].life, tokens(&t, 2), t.g.players[0].life, tokens(&t, 0)), (20, 0, 20, 0)));
    }
}

// Rules: 603.4
// Given: player 0's Spiritual Sanctuary ("At the beginning of each player's upkeep, if that
// player controls a Plains, they gain 1 life."); a Plains of player 1's, none of player 0's.
// When: player 1's upkeep, then player 0's.
// Expect: player 1 gains 1 life; player 0 gains none.
#[test]
fn r603_4_spiritual_sanctuary_gains_life_for_the_player_with_a_plains() {
    let mut t = Board::new(2).fill(10).battlefield(0, &["Spiritual Sanctuary", "Island"]).battlefield(1, &["Plains"]).build();
    run_to(&mut t, 1, Part::Draw, &mut no_questions);
    cite!(["603.4"]; assert_eq!(t.g.players[1].life, 21));
    run_to(&mut t, 0, Part::Draw, &mut no_questions);
    cite!(["603.4"]; assert_eq!((t.g.players[0].life, t.g.players[1].life), (20, 21)));
}

// Rules: 603.4
// Given: player 0's Razor Pendulum ("At the beginning of each player's end step, if that player
// has 5 or less life, this artifact deals 2 damage to that player."); player 1 at 5 or 6 life,
// player 0 at 5.
// When: player 1's end step, or player 1 gains 3 life with Revitalize in response to it.
// Expect: at 5 life player 1 is dealt 2; at 6, or at 8 as it resolves (its condition is asked
// again, 603.4), none. Player 0's life doesn't matter on player 1's turn.
#[test]
fn r603_4_razor_pendulum_hits_the_player_whose_end_step_it_is_if_still_at_5_or_less() {
    for (life, gains, expected) in [(5, false, 3), (6, false, 6), (5, true, 8)] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Razor Pendulum"]).hand(1, &["Revitalize"]).build();
        t.g.players[1].life = life;
        run_to(&mut t, 1, Part::PostcombatMain, &mut no_questions);
        t.g.players[0].life = 5;
        t.g.act(1, Move::Pass).unwrap();
        t.g.act(0, Move::Pass).unwrap();
        cite!(["603.4"]; case: format!("{life} life"); assert_eq!(t.g.stack.len(), usize::from(life <= 5)));
        if gains {
            t.mana(1, "{1}{W}").cast(1, "Revitalize", vec![], mana_pool("CW")).unwrap();
        }
        run_to(&mut t, 1, Part::End, &mut no_questions);
        cite!(["603.4"]; case: format!("{life} life, gains {gains}"); assert_eq!((t.g.players[1].life, t.g.players[0].life), (expected, 5)));
    }
}

// Rules: 603.4
// Given: player 0's Urborg Stalker ("At the beginning of each player's upkeep, if that player
// controls a nonblack, nonland permanent, this creature deals 1 damage to that player.");
// player 1's Grizzly Bears; Lightning Bolt in player 0's hand.
// When: player 1's upkeep, with the Bears Bolted in response or not; then player 0's upkeep.
// Expect: player 1 is dealt 1, or none once the Bears is gone as it resolves (its ruling);
// player 0, whose only nonland permanent is the black Stalker, is dealt none.
#[test]
fn r603_4_urborg_stalker_asks_again_as_it_resolves() {
    for bolt in [false, true] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Urborg Stalker", "Swamp"]).battlefield(1, &["Grizzly Bears"]).hand(0, &["Lightning Bolt"]).build();
        let bears = t.find_in_battlefield("Grizzly Bears");
        run_to(&mut t, 0, Part::End, &mut no_questions);
        while !(t.g.active == 1 && t.g.part == Part::Upkeep) { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
        cite!(["603.4"]; case: format!("bolt {bolt}"); assert_eq!(t.g.stack.len(), 1));
        if bolt {
            t.g.act(1, Move::Pass).unwrap();
            t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
        }
        run_to(&mut t, 1, Part::Draw, &mut no_questions);
        cite!(["603.4"]; case: format!("bolt {bolt}"); assert_eq!(t.g.players[1].life, if bolt { 20 } else { 19 }));
        run_to(&mut t, 0, Part::Draw, &mut no_questions);
        cite!(["603.4"]; case: format!("bolt {bolt}"); assert_eq!(t.g.players[0].life, 20));
    }
}

// Rules: 603.3d, 701.21a
// Given: player 0's Sheoldred, Whispering One ("At the beginning of your upkeep, return target
// creature card from your graveyard to the battlefield. At the beginning of each opponent's
// upkeep, that player sacrifices a creature of their choice."); player 1's Grizzly Bears and
// Hill Giant; Hill Giant in player 0's graveyard.
// When: player 1's upkeep, then player 0's.
// Expect: player 1 chooses and sacrifices the Bears (its ruling); at player 0's upkeep the
// Giant returns to player 0's battlefield.
#[test]
fn r701_21a_sheoldred_makes_the_opponent_whose_upkeep_it_is_sacrifice_their_choice() {
    let mut t = Board::new(2).fill(10).battlefield(0, &["Sheoldred, Whispering One"]).battlefield(1, &["Grizzly Bears", "Hill Giant"]).graveyard(0, &["Hill Giant"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let mut asked = Vec::new();
    run_to(&mut t, 1, Part::Draw, &mut |t, p, a| match a {
        Asked::Objects { among, .. } | Asked::Select { among, .. } => { asked.push((p, among.len())); t.answer(p, Answer::Objects(vec![bears])).unwrap(); }
        other => panic!("unexpected {other:?}"),
    });
    cite!(["701.21a"]; assert_eq!(asked, vec![(1, 2)]));
    t.expect().rules(["701.21a"]).zone(1, Zone::Graveyard, &["Grizzly Bears"]).zone(1, Zone::Battlefield, &["Hill Giant"]).check();
    let giant = t.find_in(0, Zone::Graveyard, "Hill Giant");
    run_to(&mut t, 0, Part::Draw, &mut |t, p, a| match a {
        Asked::Targets { .. } => { t.g.act(p, Move::ChooseTargets(vec![vec![Target::Object(giant)]])).unwrap(); }
        other => panic!("unexpected {other:?}"),
    });
    t.expect().rules(["603.3d"]).zone(0, Zone::Battlefield, &["Hill Giant", "Sheoldred, Whispering One"]).zone(0, Zone::Graveyard, &[]).check();
}

// Rules: 608.2c, 207.2c
// Given: Hill Giant (3/3) of player 1's; Cackling Flames ("deals 3 damage to any target.
// Hellbent — ... 5 damage instead if you have no cards in hand.") in player 0's hand, with an
// Island or alone.
// When: player 0 casts it at player 1.
// Expect: with only the Flames, its hand is empty as it resolves: 5 damage; with the Island, 3.
#[test]
fn r608_2c_cackling_flames_deals_5_with_no_cards_in_hand() {
    for (island, life) in [(false, 15), (true, 17)] {
        let hand: &[&str] = if island { &["Cackling Flames", "Island"] } else { &["Cackling Flames"] };
        let mut t = Board::new(2).fill(5).hand(0, hand).build();
        t.mana(0, "{3}{R}").cast(0, "Cackling Flames", vec![Tgt::Player(1)], mana_pool("CCCR")).unwrap();
        t.resolve();
        cite!(["608.2c"]; case: format!("island {island}"); assert_eq!(t.g.players[1].life, life));
    }
}

// Rules: 608.2c, 614.1a
// Given: player 1's two Hill Giants; Twinstrike ("deals 2 damage to each of two target creatures.
// Hellbent — Destroy those creatures instead if you have no cards in hand.") in player 0's hand,
// with an Island or alone.
// When: player 0 casts it at both Giants.
// Expect: with no other card, both are destroyed and dealt no damage; with the Island, each is
// dealt 2 and survives.
#[test]
fn r608_2c_twinstrike_destroys_instead_with_no_cards_in_hand() {
    for island in [false, true] {
        let hand: &[&str] = if island { &["Twinstrike", "Island"] } else { &["Twinstrike"] };
        let mut t = Board::new(2).fill(5).hand(0, hand).battlefield(1, &["Hill Giant", "Hill Giant"]).build();
        let giants: Vec<ObjId> = t.g.battlefield.clone();
        let card = t.find_in(0, Zone::Hand, "Twinstrike");
        t.mana(0, "{3}{B}{R}");
        t.g.act(0, paid("CCCBR", Move::Cast { card, targets: vec![vec![Target::Object(giants[0]), Target::Object(giants[1])]] })).unwrap();
        loop {
            match t.settle() {
                Boundary::Question(p, Asked::OrderCards { cards, .. }) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
                Boundary::Priority(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        let case = format!("island {island}");
        if island {
            cite!(["608.2c"]; case: case; assert_eq!(giants.iter().map(|g| t.g.obj(*g).damage).collect::<Vec<_>>(), vec![2, 2]));
        } else {
            t.expect().rules(["608.2c", "614.1a"]).zone(1, Zone::Graveyard, &["Hill Giant", "Hill Giant"]).check();
        }
    }
}

// Rules: 608.2c, 701.23a
// Given: player 0's library of Sol Ring, Grizzly Bears and Islands; Infernal Tutor ("Reveal a card
// from your hand. Search your library for a card with the same name as that card ... Hellbent —
// If you have no cards in hand, instead search your library for a card ...") in hand, with
// Grizzly Bears or alone.
// When: player 0 casts it.
// Expect: alone, the search offers every card of the library and the Sol Ring is taken; with the
// Bears revealed, only the Grizzly Bears of the library is offered.
#[test]
fn r608_2c_infernal_tutor_searches_for_any_card_with_no_cards_in_hand() {
    for bears in [false, true] {
        let hand: &[&str] = if bears { &["Infernal Tutor", "Grizzly Bears"] } else { &["Infernal Tutor"] };
        let mut t = Board::new(2).hand(0, hand).library(0, &["Sol Ring", "Grizzly Bears", "Island", "Island", "Island"]).fill(5).build();
        let in_hand = bears.then(|| t.find_in(0, Zone::Hand, "Grizzly Bears"));
        t.mana(0, "{1}{B}").cast(0, "Infernal Tutor", vec![], mana_pool("CB")).unwrap();
        let mut offered = Vec::new();
        loop {
            match t.settle() {
                Boundary::Question(0, Asked::Search { among, .. }) => {
                    let names: Vec<String> = among.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
                    offered = names.clone();
                    let take = among[names.iter().position(|n| n == if bears { "Grizzly Bears" } else { "Sol Ring" }).unwrap()];
                    t.answer(0, Answer::Objects(vec![take])).unwrap();
                }
                Boundary::Question(0, Asked::Objects { .. } | Asked::Select { .. }) => { t.answer(0, Answer::Objects(vec![in_hand.unwrap()])).unwrap(); }
                Boundary::Priority(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        let case = format!("bears {bears}");
        if bears {
            cite!(["701.23a"]; case: case; assert_eq!(offered, ["Grizzly Bears"]));
            t.expect().rules(["608.2c"]).zone(0, Zone::Hand, &["Grizzly Bears", "Grizzly Bears"]).check();
        } else {
            cite!(["608.2c"]; case: case; assert!(offered.len() >= 5 && offered.contains(&"Sol Ring".to_string()), "{offered:?}"));
            t.expect().rules(["608.2c"]).zone(0, Zone::Hand, &["Sol Ring"]).check();
        }
    }
}

// Rules: 602.5
// Given: player 0's Library of Alexandria ("{T}: Draw a card. Activate only if you have exactly
// seven cards in hand."), with seven or six Islands in hand.
// When: player 0 activates its second ability.
// Expect: with seven it draws a card; with six it can't be activated.
#[test]
fn r602_5_library_of_alexandria_draws_only_with_exactly_seven_cards_in_hand() {
    for cards in [7, 6] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Library of Alexandria"]).hand(0, &vec!["Island"; cards]).build();
        let result = t.activate(0, "Library of Alexandria", 1, vec![], mana_pool("")).map(|_| ());
        if cards == 7 {
            cite!(["602.5"]; ok: result);
            t.resolve();
            cite!(["602.5"]; assert_eq!(t.g.players[0].hand.len(), 8));
        } else {
            cite!(["602.5"]; refused("activation condition does not hold"): result);
        }
    }
}
