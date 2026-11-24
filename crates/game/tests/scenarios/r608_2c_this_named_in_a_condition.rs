//! A condition that names the ability's own object ("if this creature is a Warrior", "if Katara
//! is tapped") makes it the antecedent of a later "it" in that ability, as naming it in an act
//! does, even where the event or the cost could supply another object (608.2c).
use crate::support::*;
use game::legal::Asked;
use game::state::{Answer, Decision};
use game::{Move, ObjId, Zone};
use ir::Part;

/// Pass priority and answer the questions these abilities raise, until `p`'s turn reaches
/// `part` with an empty stack: simultaneous triggers are put on the stack in the order listed,
/// a "you may" is answered `may`, no one attacks.
fn until(t: &mut T, p: usize, part: Part, may: bool) {
    for _ in 0..2000 {
        if t.g.active == p && t.g.part == part && t.g.stack.is_empty() && matches!(t.g.asked(), Some((_, Asked::Priority))) { return; }
        match t.g.asked() {
            Some((q, Asked::Order(abilities))) => { t.g.act(q, Move::Order((0..abilities.len()).collect())).unwrap(); }
            Some((q, Asked::May)) => { t.answer(q, if may { Answer::Yes } else { Answer::No }).unwrap(); }
            _ => match t.g.decision.clone() {
                Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
                Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
                _ => { t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(); }
            },
        }
    }
    panic!("never reached player {p}'s {part:?}");
}

fn subtypes(t: &T, o: ObjId) -> Vec<&'static str> {
    let mut names: Vec<_> = t.g.seen(o, None).subtypes.iter().map(|s| s.name()).collect();
    names.sort();
    names
}

#[test]
fn r608_2c_answered_prayers_becomes_an_angel_itself_not_the_creature_that_entered() {
    let mut t = Board::new(2).battlefield(0, &["Answered Prayers"]).hand(0, &["Grizzly Bears"]).build();
    t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    t.resolve();
    let (prayers, bears) = (t.find_in(0, Zone::Battlefield, "Answered Prayers"), t.find_in(0, Zone::Battlefield, "Grizzly Bears"));
    cite!(["608.2c", "205.1b"]; assert_eq!(t.g.seen(prayers, None).pt, Some((3, 3))));
    cite!(["608.2c"]; assert!(t.g.has(prayers, "Flying") && subtypes(&t, prayers) == ["Angel"]));
    cite!(["608.2c"]; assert!(!t.g.has(bears, "Flying") && subtypes(&t, bears) == ["Bear"]));
    t.expect().life(0, 21).check();
}

#[test]
fn r608_2c_warden_of_the_first_tree_grows_only_through_its_types() {
    let mut t = Board::new(2).battlefield(0, &["Warden of the First Tree"]).build();
    let warden = t.find_in(0, Zone::Battlefield, "Warden of the First Tree");
    // not yet a Warrior: the second ability does nothing (and so does the third, not a Spirit)
    t.mana(0, "{2}{W}{W}").activate(0, "Warden of the First Tree", 1, vec![], mana_pool("CCWW")).unwrap().resolve();
    cite!(["608.2c", "608.2h"]; assert!(subtypes(&t, warden) == ["Human"] && !t.g.has(warden, "Trample")));
    t.mana(0, "{1}{W}").activate(0, "Warden of the First Tree", 0, vec![], mana_pool("CW")).unwrap().resolve();
    cite!(["205.1b"]; assert!(subtypes(&t, warden) == ["Human", "Warrior"] && t.g.seen(warden, None).pt == Some((3, 3))));
    t.mana(0, "{2}{W}{W}").activate(0, "Warden of the First Tree", 1, vec![], mana_pool("CCWW")).unwrap().resolve();
    cite!(["608.2c", "205.1b"]; assert!(subtypes(&t, warden) == ["Human", "Spirit", "Warrior"]));
    cite!(["608.2c"]; assert!(t.g.has(warden, "Trample") && t.g.has(warden, "Lifelink")));
    t.mana(0, "{3}{W}{W}{W}").activate(0, "Warden of the First Tree", 2, vec![], mana_pool("CCCWWW")).unwrap().resolve();
    t.expect().rules(["608.2c", "122.1a"]).counters("Warden of the First Tree", "+1/+1", 5).pt("Warden of the First Tree", 8, 8).check();
}

#[test]
fn r608_2c_evolved_sleeper_takes_its_counters_only_as_the_type_it_asks_for() {
    let mut t = Board::new(2).battlefield(0, &["Evolved Sleeper"]).library(0, &["Island"]).build();
    let sleeper = t.find_in(0, Zone::Battlefield, "Evolved Sleeper");
    t.mana(0, "{1}{B}").activate(0, "Evolved Sleeper", 1, vec![], mana_pool("CB")).unwrap().resolve();
    cite!(["608.2c", "608.2h"]; assert!(t.g.counters(sleeper).is_empty() && subtypes(&t, sleeper) == ["Human"]));
    t.mana(0, "{B}").activate(0, "Evolved Sleeper", 0, vec![], mana_pool("B")).unwrap().resolve();
    t.mana(0, "{1}{B}").activate(0, "Evolved Sleeper", 1, vec![], mana_pool("CB")).unwrap().resolve();
    cite!(["608.2c", "122.1b"]; assert_eq!(t.g.counters(sleeper).get("deathtouch"), Some(&1)));
    cite!(["608.2c", "205.1b"]; assert!(subtypes(&t, sleeper) == ["Cleric", "Human", "Phyrexian"] && t.g.has(sleeper, "Deathtouch")));
    t.mana(0, "{1}{B}{B}").activate(0, "Evolved Sleeper", 2, vec![], mana_pool("CBB")).unwrap().resolve();
    t.expect().rules(["608.2c"]).counters("Evolved Sleeper", "+1/+1", 1).pt("Evolved Sleeper", 4, 4)
        .zone_exact(0, Zone::Hand, &["Island"]).life(0, 19).check();
}

#[test]
fn r608_2c_katara_bending_prodigy_gets_her_counter_only_if_tapped() {
    for tapped in [true, false] {
        let board = Board::new(2);
        let board = if tapped { board.tapped(0, &["Katara, Bending Prodigy"]) } else { board.battlefield(0, &["Katara, Bending Prodigy"]) };
        let mut t = board.build();
        until(&mut t, 0, Part::End, false);
        let katara = t.find_in(0, Zone::Battlefield, "Katara, Bending Prodigy");
        cite!(["603.4", "608.2c"]; case: tapped; assert_eq!(t.g.counters(katara).get("+1/+1").copied(), tapped.then_some(1)));
    }
}

#[test]
fn r608_2c_mana_bloom_returns_to_its_owners_hand_once_it_has_no_charge_counters() {
    for charge in [0, 1] {
        let mut t = Board::new(2).active(1).at(Part::End).fill(3).battlefield(0, &["Mana Bloom"]).build();
        if charge > 0 { let bloom = t.find_in(0, Zone::Battlefield, "Mana Bloom"); t.g.set_initial_counters(bloom, "charge", charge); }
        until(&mut t, 0, Part::Draw, false);
        let zone = if charge == 0 { Zone::Hand } else { Zone::Battlefield };
        t.expect().rules(["603.4", "608.2c"]).contains(0, zone, "Mana Bloom").check();
    }
}

#[test]
fn r608_2c_ingenious_prodigy_trades_one_of_its_own_counters_for_a_card() {
    for remove in [true, false] {
        let mut t = Board::new(2).active(1).at(Part::End).fill(3).battlefield(0, &["Ingenious Prodigy"]).build();
        let prodigy = t.find_in(0, Zone::Battlefield, "Ingenious Prodigy");
        t.g.set_initial_counters(prodigy, "+1/+1", 2);
        let hand = t.g.players[0].hand.len();
        until(&mut t, 0, Part::Upkeep, remove);
        // removed from itself; the draw step has not begun
        cite!(["608.2c", "608.2d"]; case: remove; assert_eq!(t.g.counters(prodigy).get("+1/+1"), Some(if remove { &1 } else { &2 })));
        cite!(["608.2c", "608.2d"]; case: remove; assert_eq!(t.g.players[0].hand.len(), hand + usize::from(remove)));
    }
}

#[test]
fn r608_2c_avacynian_missionaries_transforms_at_the_end_step_only_while_equipped() {
    for equipped in [true, false] {
        let mut t = Board::new(2).battlefield(0, &["Avacynian Missionaries // Lunarch Inquisitors", "Bonesplitter"]).build();
        let missionaries = t.find_in(0, Zone::Battlefield, "Avacynian Missionaries");
        if equipped {
            let bonesplitter = t.find_in(0, Zone::Battlefield, "Bonesplitter");
            t.mana(0, "{1}");
            let ability = t.g.abilities(bonesplitter).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
            t.g.act(0, Move::Paying(mana_pool("C"), Box::new(Move::Activate { source: bonesplitter, ability, targets: vec![vec![game::Target::Object(missionaries)]], paying: vec![] }))).unwrap();
            t.resolve();
        }
        until(&mut t, 0, Part::End, false);
        let name = t.g.def(missionaries).unwrap().name().to_string();
        cite!(["603.4", "608.2c", "701.27a"]; case: equipped; assert_eq!(name, if equipped { "Lunarch Inquisitors" } else { "Avacynian Missionaries" }));
    }
}
