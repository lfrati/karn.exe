//! "An effect may refer to an object 'with [keyword ability]' or 'that has [keyword ability]'"
//! (702.1d): an object has every keyword ability it prints, in every zone, whether that keyword
//! is a static ability held in force (flying) or only the triggered, activated or spell
//! ability it also is (cascade, level up, kicker, madness).
use crate::support::*;
use game::legal::Asked;
use game::state::Answer;
use game::{Move, Target, Zone};

/// Resolve the stack, putting simultaneous triggers on it in the order they are listed and
/// declining an offered cast.
fn settle(t: &mut T) {
    loop {
        match t.settle() {
            Boundary::Question(p, Asked::Order(abilities)) => { t.g.act(p, Move::Order((0..abilities.len()).collect())).unwrap(); }
            Boundary::Question(p, Asked::Cast { .. }) => { t.answer(p, Answer::No).unwrap(); }
            Boundary::Priority(_) => return,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn r702_1d_aurora_phoenix_returns_when_its_controller_casts_a_spell_with_cascade() {
    let mut t = Board::new(2).graveyard(0, &["Aurora Phoenix"]).hand(0, &["Bloodbraid Elf", "Grizzly Bears"]).build();
    // A spell without cascade does not trigger it.
    t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    cite!(["702.1d", "603.2"]; assert_eq!(t.g.stack.len(), 1));
    t.resolve();
    t.mana(0, "{2}{R}{G}").cast(0, "Bloodbraid Elf", vec![], mana_pool("CCRG")).unwrap();
    // Bloodbraid Elf's cascade and Aurora Phoenix's return both trigger on the cast.
    let Some((0, Asked::Order(triggered))) = t.asked() else { panic!("{:?}", t.asked()) };
    cite!(["702.1d", "702.85a", "603.2", "603.3b"]; assert_eq!(triggered.len(), 2));
    settle(&mut t);
    t.expect().rules(["702.1d", "603.2"])
        .zone_exact(0, Zone::Hand, &["Aurora Phoenix"])
        .zone_exact(0, Zone::Graveyard, &[])
        .check();
}

#[test]
fn r702_1d_anje_falkenrath_untaps_for_a_discarded_card_with_madness_only() {
    for (discarded, untaps) in [("Fiery Temper", true), ("Lightning Bolt", false)] {
        let mut t = Board::new(2).battlefield(0, &["Anje Falkenrath"]).hand(0, &[discarded]).library(0, &["Island"]).build();
        let anje = t.find_in(0, Zone::Battlefield, "Anje Falkenrath");
        let card = t.find_in(0, Zone::Hand, discarded);
        t.g.act(0, Move::Activate { source: anje, ability: 1, targets: vec![], paying: vec![vec![card]] }).unwrap();
        cite!(["702.1d"]; assert!(t.g.obj(anje).tapped));
        // Madness's trigger offers the cast; declined, the card goes to the graveyard (702.35a).
        settle(&mut t);
        cite!(["702.1d", "702.35a", "603.4"]; case: discarded; assert_eq!(!t.g.obj(anje).tapped, untaps));
        t.expect().zone_exact(0, Zone::Hand, &["Island"]).zone_exact(0, Zone::Graveyard, &[discarded]).check();
    }
}

#[test]
fn r702_1d_murasa_sproutling_returns_a_card_with_a_kicker_or_multikicker_ability() {
    for (card, legal) in [("Burst Lightning", true), ("Everflowing Chalice", true), ("Lightning Bolt", false)] {
        let mut t = Board::new(2).hand(0, &["Murasa Sproutling"]).graveyard(0, &[card]).build();
        let sproutling = t.find_in(0, Zone::Hand, "Murasa Sproutling");
        t.mana(0, "{3}{G}{G}");
        t.g.act(0, paid("CCCGG", Move::WithKicks(1, Box::new(Move::Cast { card: sproutling, targets: vec![] })))).unwrap();
        t.pass_all();
        let returned = t.find_in(0, Zone::Graveyard, card);
        let chosen = t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(returned)]]));
        cite!(["702.1d", "702.33c", "115.1"]; case: card; assert_eq!(chosen.is_ok(), legal, "{chosen:?}"));
        if legal {
            t.resolve();
            t.expect().zone_exact(0, Zone::Hand, &[card]).check();
        }
    }
}

#[test]
fn r702_1d_venerated_teacher_puts_level_counters_on_each_creature_with_level_up() {
    let mut t = Board::new(2).hand(0, &["Venerated Teacher"]).battlefield(0, &["Student of Warfare", "Grizzly Bears"]).build();
    t.mana(0, "{2}{U}").cast(0, "Venerated Teacher", vec![], mana_pool("CCU")).unwrap();
    t.resolve();
    let (student, bears) = (t.find_in(0, Zone::Battlefield, "Student of Warfare"), t.find_in(0, Zone::Battlefield, "Grizzly Bears"));
    cite!(["702.1d", "702.87a"]; assert_eq!(t.g.obj(student).counters.get("level").copied(), Some(2)));
    cite!(["702.1d"]; assert_eq!(t.g.obj(bears).counters.get("level").copied(), None));
}
