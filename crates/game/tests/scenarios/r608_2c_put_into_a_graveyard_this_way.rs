//! "A creature card put into a graveyard this way", "each creature card put into their
//! graveyard this way": a card in that graveyard that an instruction of this effect put there
//! (608.2c), as the card it is there (400.7). A card that was there already is not one, nor a
//! card a replacement put elsewhere. "You may put a card from among the cards milled this way
//! into your hand. If you don't, …" asks whether that was done, not whether the mill was.
use crate::support::{Board, Boundary, T, cite, mana_pool};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Move, ObjId, Zone};
use ir::Part;

/// Player 0 casts this card from their hand, paying `mana`, until a question or priority.
fn cast(t: &mut T, name: &str, mana: &str) -> Boundary {
    t.mana(0, mana).cast(0, name, vec![], mana_pool(mana)).unwrap();
    t.settle()
}

/// Each owner puts the cards going to their graveyard together in the order they choose
/// (404.3), until another question or priority.
fn ordered(t: &mut T, mut boundary: Boundary) -> Boundary {
    while let Boundary::Question(p, Asked::OrderCards { cards, .. }) = boundary {
        t.g.act(p, Move::Choose(Answer::Objects(cards))).unwrap();
        boundary = t.settle();
    }
    boundary
}

/// What player 0 is asked to choose one of.
fn among(boundary: Boundary) -> Vec<ObjId> {
    match boundary {
        Boundary::Question(0, Asked::Objects { among, count: 1, .. }) => among,
        other => panic!("expected a choice of one card: {other:?}"),
    }
}

#[test]
fn r608_2c_a_creature_card_put_into_a_graveyard_this_way_is_one_the_destruction_put_there() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Necromantic Selection"]).battlefield(0, &["Grizzly Bears"])
        .battlefield(1, &["Serra Angel", "Hill Giant"]).graveyard(1, &["Shivan Dragon"]).build();
    let dragon = t.find_in(1, Zone::Graveyard, "Shivan Dragon");
    let boundary = cast(&mut t, "Necromantic Selection", "{4}{B}{B}{B}");
    let mut offered = among(ordered(&mut t, boundary));
    let angel = t.find_in(1, Zone::Graveyard, "Serra Angel");
    let mut destroyed = vec![t.find_in(0, Zone::Graveyard, "Grizzly Bears"), angel, t.find_in(1, Zone::Graveyard, "Hill Giant")];
    offered.sort();
    destroyed.sort();
    // of any graveyard, the cards it put there, not the one already there
    cite!(["608.2c", "400.7"]; assert_eq!(offered, destroyed));
    cite!(["608.2c"]; assert!(!offered.contains(&dragon)));
    t.answer(0, Answer::Objects(vec![angel])).unwrap().resolve();
    t.expect().rules(["608.2c", "110.2"]).zone(0, Zone::Battlefield, &["Serra Angel"]).zone(1, Zone::Graveyard, &["Shivan Dragon", "Hill Giant"])
        .zone(0, Zone::Exile, &["Necromantic Selection"]).check();
    let returned = t.find_in_battlefield("Serra Angel");
    cite!(["110.2"]; assert_eq!(t.g.controller(returned), 0));
    let seen = t.g.seen(returned, None);
    cite!(["613.1d", "613.1e"]; assert!(seen.subtypes.iter().any(|s| s.name() == "Zombie") && seen.subtypes.iter().any(|s| s.name() == "Angel")));
    cite!(["105.3"]; assert!(seen.colors.contains(&ir::Color::Black) && seen.colors.contains(&ir::Color::White)));
}

#[test]
fn r608_2c_a_card_put_into_your_graveyard_this_way_is_one_you_own() {
    for gift in [false, true] {
        let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Starfall Invocation"]).battlefield(0, &["Grizzly Bears"])
            .battlefield(1, &["Serra Angel"]).build();
        let card = t.find_in(0, Zone::Hand, "Starfall Invocation");
        t.mana(0, "{3}{W}{W}");
        let cast = Move::Cast { card, targets: vec![] };
        t.g.act(0, Move::Paying(mana_pool("{3}{W}{W}"), Box::new(if gift { Move::GiftTo(1, Box::new(cast)) } else { cast }))).unwrap();
        let boundary = t.settle();
        match boundary {
            // the angel is not in your graveyard: the bears are the one card it may be
            Boundary::Question(0, Asked::Objects { among, .. }) if gift => {
                cite!(["608.2c"]; case: gift; assert_eq!(among, vec![t.find_in(0, Zone::Graveyard, "Grizzly Bears")]));
                t.answer(0, Answer::Objects(among)).unwrap().resolve();
            }
            Boundary::Priority(_) => {}
            other => panic!("unexpected {other:?}"),
        }
        let bears = if gift { &["Grizzly Bears"][..] } else { &[][..] };
        t.expect().rules(["608.2c", "702.174j"]).zone(0, Zone::Battlefield, bears).zone(1, Zone::Graveyard, &["Serra Angel"]).check();
    }
}

#[test]
fn r608_2c_a_count_of_cards_put_into_their_graveyard_this_way_is_of_those_milled() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Coerced Confession"]).graveyard(1, &["Hill Giant"])
        .library(1, &["Grizzly Bears", "Mountain", "Serra Angel", "Island"]).build();
    let card = t.find_in(0, Zone::Hand, "Coerced Confession");
    t.mana(0, "{4}{U}");
    t.g.act(0, Move::Paying(mana_pool("{4}{U}"), Box::new(Move::Cast { card, targets: vec![vec![game::Target::Player(1)]] }))).unwrap();
    let boundary = t.settle();
    let boundary = ordered(&mut t, boundary);
    assert!(matches!(boundary, Boundary::Priority(_)), "{boundary:?}");
    // the two creature cards it milled, not the giant already there
    t.expect().rules(["608.2c", "701.17a"]).zone(1, Zone::Graveyard, &["Hill Giant", "Grizzly Bears", "Mountain", "Serra Angel", "Island"]).check();
    cite!(["608.2c"]; assert_eq!(t.g.players[0].hand.len(), 2));
}

#[test]
fn r608_2c_if_you_dont_put_a_card_from_among_the_milled_cards_into_your_hand() {
    // declined, or with no creature card among them, it gets the counter (Ravenous Gigamole
    // ruling); a creature card put into the hand, it does not
    for (library, take) in [(["Grizzly Bears", "Mountain", "Island"], true), (["Grizzly Bears", "Mountain", "Island"], false), (["Plains", "Mountain", "Island"], false)] {
        let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Ravenous Gigamole"]).library(0, &library).build();
        let boundary = cast(&mut t, "Ravenous Gigamole", "{3}{B}");
        let mut boundary = ordered(&mut t, boundary);
        let fits = library[0] == "Grizzly Bears";
        if fits {
            let Boundary::Question(0, Asked::May) = boundary else { panic!("expected whether to put a card: {boundary:?}") };
            let bears = t.find_in(0, Zone::Graveyard, "Grizzly Bears");
            t.answer(0, if take { Answer::Yes } else { Answer::No }).unwrap();
            boundary = t.settle();
            if take {
                if let Boundary::Question(0, Asked::Objects { among, .. }) = boundary {
                    cite!(["608.2d"]; assert_eq!(among, vec![bears]));
                    t.answer(0, Answer::Objects(vec![bears])).unwrap();
                    boundary = t.settle();
                }
            }
        }
        cite!(["608.2c"]; case: take; assert!(matches!(boundary, Boundary::Priority(_)), "{boundary:?}"));
        let counters = if take { 0 } else { 1 };
        t.expect().rules(["608.2c", "701.17a"]).counters("Ravenous Gigamole", "+1/+1", counters).check();
        if take { t.expect().rules(["608.2c"]).zone(0, Zone::Hand, &["Grizzly Bears"]).check(); }
    }
}

#[test]
fn r608_2c_cards_put_into_a_graveyard_by_an_instruction_that_keeps_no_result_are_refused() {
    // a sacrifice keeps no record of what it put where: "this way" would miss its cards
    for text in [
        "Each player sacrifices a creature. Return a creature card put into a graveyard this way to the battlefield under your control.",
        "Surveil 2. You gain 1 life for each card put into your graveyard this way.",
    ] {
        let def = CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap();
        cite!(["608.2c"]; refused("result group: an instruction put cards into a graveyard and kept no result"): game::play::check(&def).map_err(|e| e.0));
    }
    // read later by a delayed body, it would read only the cards still there
    let def = CardDef::new("Probe", "{0}", "Sorcery", "Mill four cards. Whenever a creature attacks this turn, it gets +1/+0 until end of turn for each creature card put into your graveyard this way.", None).unwrap();
    cite!(["608.2c", "603.7"]; refused("result group: cards put into a graveyard this way, read by a delayed body"): game::play::check(&def).map_err(|e| e.0));
}

#[test]
fn r608_2c_the_cards_put_into_your_graveyard_this_way_are_the_rest_of_those_looked_at() {
    let mut t = Board::new(2).fill(5).at(Part::PrecombatMain).hand(0, &["Discerning Taste"]).graveyard(0, &["Shivan Dragon"])
        .library(0, &["Serra Angel", "Grizzly Bears", "Hill Giant", "Mountain"]).build();
    let boundary = cast(&mut t, "Discerning Taste", "{2}{B}");
    let Boundary::Question(0, Asked::Objects { among, .. }) = boundary else { panic!("expected the card to put into the hand: {boundary:?}") };
    let mountain = *among.iter().find(|o| t.g.obj(**o).chars.name == "Mountain").expect("the mountain is offered");
    t.answer(0, Answer::Objects(vec![mountain])).unwrap();
    let boundary = t.settle();
    let boundary = ordered(&mut t, boundary);
    assert!(matches!(boundary, Boundary::Priority(_)), "{boundary:?}");
    // the angel's power, of the three put there; not the dragon's, which was there already
    t.expect().rules(["608.2c", "208.1"]).zone(0, Zone::Hand, &["Mountain"]).life(0, 24).check();
}
