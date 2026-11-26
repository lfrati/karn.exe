//! Review of "put into a graveyard this way" (35746bd): only a card the effect's instruction put
//! there, as the new object it is there (400.7, 608.2c), never a card already there, nor one a
//! replacement put elsewhere; and "if you don't" after an optional move asks whether that move
//! was done (608.2c). Every card is read from its Oracle text.
use crate::support::*;
use game::legal::Asked;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Discerning Taste", "Coerced Confession", "Necromantic Selection", "Ainok Wayfarer", "Rest in Peace", "Hill Giant",
        "Grizzly Bears", "Serra Angel", "Colossal Dreadmaw", "Island", "Forest"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn names(t: &T, objs: &[ObjId]) -> Vec<String> {
    let mut n: Vec<String> = objs.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    n.sort();
    n
}

// Rules: 400.7, 608.2c
// Given: player 0's library of Island, Hill Giant (power 3), Grizzly Bears and Serra Angel on top;
// Colossal Dreadmaw (power 6) already in their graveyard; Rest in Peace or not.
// When: player 0 casts Discerning Taste ("Look at the top four cards of your library. Put one of
// them into your hand and the rest into your graveyard. You gain life equal to the greatest power
// among creature cards put into your graveyard this way.") and keeps the Angel.
// Expect: 3 life, the Giant's power; the Dreadmaw was not put there this way. Under Rest in
// Peace the rest are exiled instead, and none was put into a graveyard: no life.
#[test]
fn r400_7_discerning_taste_counts_only_the_cards_it_put_there() {
    for rest in [false, true] {
        let mine: &[&str] = if rest { &["Rest in Peace"] } else { &[] };
        let mut t = Board::new(2).fill(5).hand(0, &["Discerning Taste"]).battlefield(0, mine).library(0, &["Island", "Hill Giant", "Grizzly Bears", "Serra Angel"])
            .graveyard(0, &["Colossal Dreadmaw"]).build();
        t.mana(0, "{2}{B}").cast(0, "Discerning Taste", vec![], mana_pool("CCB")).unwrap();
        let mut asked = Vec::new();
        loop {
            match t.settle() {
                Boundary::Question(0, Asked::Objects { among, .. } | Asked::Select { among, .. } | Asked::Search { among, .. }) => {
                    asked = names(&t, &among);
                    let angel = *among.iter().find(|o| t.g.obj(**o).chars.name == "Serra Angel").unwrap();
                    t.answer(0, Answer::Objects(vec![angel])).unwrap();
                }
                Boundary::Question(p, Asked::OrderCards { cards, .. }) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
                Boundary::Priority(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        let case = format!("rest in peace {rest}");
        cite!(["608.2d"]; case: case.clone(); assert_eq!(asked, ["Grizzly Bears", "Hill Giant", "Island", "Serra Angel"]));
        cite!(["400.7", "608.2c"]; case: case; assert_eq!(t.g.players[0].life, if rest { 20 } else { 23 }));
    }
}

// Rules: 400.7, 608.2c
// Given: player 1's library of Grizzly Bears, Island, Hill Giant and Island on top; Serra Angel
// already in their graveyard.
// When: player 0 casts Coerced Confession ("Target player mills four cards. You draw a card for
// each creature card put into their graveyard this way.") at player 1.
// Expect: two creature cards were milled: player 0 draws two, not three.
#[test]
fn r400_7_coerced_confession_draws_for_the_creature_cards_milled() {
    let mut t = Board::new(2).fill(5).hand(0, &["Coerced Confession"]).library(1, &["Grizzly Bears", "Island", "Hill Giant", "Island"]).graveyard(1, &["Serra Angel"]).build();
    t.mana(0, "{4}{U}").cast(0, "Coerced Confession", vec![Tgt::Player(1)], mana_pool("CCCCU")).unwrap();
    loop {
        match t.settle() {
            Boundary::Question(p, Asked::OrderCards { cards, .. }) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            Boundary::Priority(_) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    cite!(["400.7", "608.2c"]; assert_eq!(t.g.players[0].hand.len(), 2));
}

// Rules: 400.7, 608.2c
// Given: player 0's Grizzly Bears, player 1's Hill Giant; Serra Angel already in player 1's
// graveyard.
// When: player 0 casts Necromantic Selection ("Destroy all creatures, then return a creature card
// put into a graveyard this way to the battlefield under your control.").
// Expect: the Bears and the Giant are offered, not the Angel; the Giant returns under player 0's
// control.
#[test]
fn r400_7_necromantic_selection_returns_a_card_it_destroyed() {
    let mut t = Board::new(2).fill(5).hand(0, &["Necromantic Selection"]).battlefield(0, &["Grizzly Bears"]).battlefield(1, &["Hill Giant"]).graveyard(1, &["Serra Angel"]).build();
    t.mana(0, "{4}{B}{B}{B}").cast(0, "Necromantic Selection", vec![], mana_pool("CCCCBBB")).unwrap();
    let mut asked = Vec::new();
    loop {
        match t.settle() {
            Boundary::Question(0, Asked::Objects { among, .. } | Asked::Select { among, .. }) => {
                asked = names(&t, &among);
                let giant = *among.iter().find(|o| t.g.obj(**o).chars.name == "Hill Giant").unwrap();
                t.answer(0, Answer::Objects(vec![giant])).unwrap();
            }
            Boundary::Priority(_) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    cite!(["400.7", "608.2c"]; assert_eq!(asked, ["Grizzly Bears", "Hill Giant"]));
    let giant = t.find_in_battlefield("Hill Giant");
    cite!(["608.2c"]; assert_eq!((t.g.controller(giant), t.g.obj(giant).owner), (0, 1)));
    t.expect().rules(["400.7"]).zone(1, Zone::Graveyard, &["Serra Angel"]).zone(0, Zone::Graveyard, &["Grizzly Bears"]).check();
}

// Rules: 608.2c, 118.12
// Given: player 0's library of Forest, Grizzly Bears and Island on top, or of three creature
// cards.
// When: player 0 casts Ainok Wayfarer ("When this creature enters, mill three cards. You may put a
// land card from among them into your hand. If you don't, put a +1/+1 counter on this
// creature.") and takes the Forest, or declines.
// Expect: taking it, no counter; declining, or with no land card milled, a counter.
#[test]
fn r608_2c_ainok_wayfarer_gets_a_counter_unless_it_took_a_land() {
    for (lands, take, counter) in [(true, true, 0), (true, false, 1), (false, true, 1)] {
        let library: &[&str] = if lands { &["Forest", "Grizzly Bears", "Island"] } else { &["Grizzly Bears", "Hill Giant", "Serra Angel"] };
        let mut t = Board::new(2).fill(5).hand(0, &["Ainok Wayfarer"]).library(0, library).build();
        t.mana(0, "{1}{G}").cast(0, "Ainok Wayfarer", vec![], mana_pool("CG")).unwrap();
        let mut offered = Vec::new();
        loop {
            match t.settle() {
                Boundary::Question(0, Asked::OptionalObjects { among, .. }) => {
                    offered = names(&t, &among);
                    let forest = among.iter().copied().find(|o| t.g.obj(*o).chars.name == "Forest");
                    t.answer(0, Answer::OptionalObjects(take.then(|| vec![forest.unwrap()]))).unwrap();
                }
                Boundary::Question(0, Asked::May) => { t.answer(0, if take { Answer::Yes } else { Answer::No }).unwrap(); }
                Boundary::Question(0, Asked::Objects { among, .. } | Asked::Select { among, .. }) => {
                    offered = names(&t, &among);
                    let forest = among.iter().copied().filter(|o| t.g.obj(*o).chars.name == "Forest").collect();
                    t.answer(0, Answer::Objects(forest)).unwrap();
                }
                Boundary::Question(p, Asked::OrderCards { cards, .. }) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
                Boundary::Priority(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        let case = format!("lands {lands}, take {take}");
        if lands && take { cite!(["608.2d"]; case: case.clone(); assert_eq!(offered, ["Forest", "Island"])); }
        let wayfarer = t.find_in_battlefield("Ainok Wayfarer");
        cite!(["608.2c", "118.12"]; case: case; assert_eq!(t.g.counters(wayfarer).get("+1/+1").copied().unwrap_or(0), counter));
    }
}
