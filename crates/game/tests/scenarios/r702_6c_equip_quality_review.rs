//! Review of equip [quality] and granted equip (702.6c, 613.1f) on cards that newly run:
//! Puresteel Paladin's metalcraft equip {0}, Gemcutter Buccaneer's Treasures, Team Pennant's
//! "Equip creature token" and Unstable Molecule Suit's "Equip commander". Every card is read
//! from its Oracle text.
use crate::support;
use support::*;
use ir::Ability;

/// Its activated abilities as it has them now, in order (a Treasure's mana ability first).
fn activated(t: &T, o: ObjId) -> Vec<usize> {
    t.g.abilities(o).iter().enumerate().filter(|(_, a)| matches!(&***a, Ability::Activated { .. })).map(|(i, _)| i).collect()
}
fn equip(t: &mut T, p: usize, source: ObjId, nth: usize, to: ObjId, mana: &str) -> Result<(), String> {
    let ability = activated(t, source)[nth];
    t.g.act(p, paid(mana, Move::Activate { source, ability, targets: vec![vec![Target::Object(to)]], paying: vec![] }))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Puresteel Paladin", "Gemcutter Buccaneer", "Team Pennant", "Unstable Molecule Suit"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

// Rules: 613.1f, 702.6a, 702.6d, 602.5d
// Given: Puresteel Paladin, Bonesplitter, Ornithopter and Grizzly Bears; a second Ornithopter
// in hand; Shatter in player 1's hand.
// When: the second Ornithopter is cast; the granted equip {0} attaches Bonesplitter to the
// Bears; in player 1's turn, Shatter is cast on an Ornithopter.
// Expect: with two artifacts Bonesplitter has its own equip alone, with three it has equip {0}
// too. That ability is activated only as a sorcery. Once an artifact is gone, Bonesplitter
// loses it again and stays attached.
#[test]
fn r613_1f_puresteel_paladins_equip_0_lasts_while_metalcraft_holds() {
    // Puresteel Paladin: "Metalcraft — Equipment you control have equip {0} as long as you
    // control three or more artifacts."
    let mut t = Board::new(2).fill(5).named(0, Zone::Battlefield, "splitter", "Bonesplitter")
        .named(0, Zone::Battlefield, "thopter", "Ornithopter").battlefield(0, &["Puresteel Paladin", "Grizzly Bears"])
        .hand(0, &["Ornithopter"]).hand(1, &["Shatter"]).build();
    let (splitter, thopter) = (t.id("splitter"), t.id("thopter"));
    let bears = t.find_in_battlefield("Grizzly Bears");
    // two artifacts: Bonesplitter and an Ornithopter
    cite!(["702.6a", "702.6d"]; assert_eq!(activated(&t, splitter).len(), 1, "its own equip {{1}} alone"));
    t.cast(0, "Ornithopter", vec![], mana_pool("")).unwrap();
    t.resolve();
    cite!(["613.1f", "702.6d"]; assert_eq!(activated(&t, splitter).len(), 2, "equip {{1}} and equip {{0}}"));
    cite!(["613.1f", "702.6a"]; ok: equip(&mut t, 0, splitter, 1, bears, ""));
    t.resolve();
    t.expect().rules(["702.6a", "301.5"]).pt("Grizzly Bears", 4, 2).check();
    // in player 1's turn, with Shatter on the stack, equip is not activated (602.5d, 702.6a)
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{1}{R}").cast(1, "Shatter", vec![Tgt::Object(thopter)], mana_pool("CR")).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    let paladin = t.find_in_battlefield("Puresteel Paladin");
    cite!(["602.5d", "702.6a"]; refused("sorcery"): equip(&mut t, 0, splitter, 1, paladin, ""));
    t.resolve();
    t.expect().rules(["701.8a"]).zone(0, Zone::Graveyard, &["Ornithopter"]).check();
    cite!(["613.1f", "611.3a"]; assert_eq!(activated(&t, splitter).len(), 1, "two artifacts: no equip {{0}}"));
    // losing the ability doesn't unattach it (301.5c)
    t.expect().rules(["301.5c"]).pt("Grizzly Bears", 4, 2).check();
}

// Rules: 613.1f, 702.6c, 205.3m
// Given: Gemcutter Buccaneer cast; Grizzly Bears on the battlefield.
// When: its trigger makes a tapped Treasure; the Treasure's equip Pirate {1} targets the Bears,
// then the Buccaneer; then its equip {3} targets the Bears.
// Expect: the Treasure is an Equipment with its mana ability, equip Pirate {1} and equip {3};
// the Bears are no Pirate; the equipped creature gets +2/+0, tapped Treasure or not.
#[test]
fn r702_6c_a_treasure_has_equip_pirate_and_equip() {
    // Gemcutter Buccaneer: "Whenever this creature or another Pirate you control enters, create
    // a tapped Treasure token. Treasures you control are Equipment in addition to their other
    // types and have "Equipped creature gets +2/+0," equip Pirate {1}, and equip {3}."
    let mut t = Board::new(2).hand(0, &["Gemcutter Buccaneer"]).battlefield(0, &["Grizzly Bears"]).build();
    t.mana(0, "{3}{R}").cast(0, "Gemcutter Buccaneer", vec![], mana_pool("CCCR")).unwrap();
    t.resolve();
    t.resolve();
    let treasure = *t.g.battlefield.iter().find(|o| t.g.obj(**o).token()).expect("a Treasure");
    let (bears, buccaneer) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Gemcutter Buccaneer"));
    cite!(["111.10a"]; assert!(t.g.obj(treasure).tapped, "a tapped Treasure"));
    cite!(["613.1f", "702.6d"]; assert_eq!(activated(&t, treasure).len(), 3, "its mana ability, equip Pirate {{1}} and equip {{3}}"));
    t.mana(0, "{1}");
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, 0, treasure, 1, bears, "C"));
    cite!(["702.6c"]; ok: equip(&mut t, 0, treasure, 1, buccaneer, "C"));
    t.resolve();
    t.expect().rules(["702.6c", "613.4c"]).pt("Gemcutter Buccaneer", 3, 3).check();
    t.mana(0, "{3}");
    cite!(["702.6a"]; ok: equip(&mut t, 0, treasure, 2, bears, "CCC"));
    t.resolve();
    t.expect().rules(["702.6a", "301.5c"]).pt("Grizzly Bears", 4, 2).pt("Gemcutter Buccaneer", 1, 3).check();
}

// Rules: 702.6c, 702.163a
// Given: Team Pennant and Grizzly Bears; Vulshok Splitter cast, making its Rebel token.
// When: Team Pennant's equip creature token {1} targets the Bears, then the Rebel.
// Expect: the Bears are no token; the Rebel gets +1/+1, vigilance and trample.
#[test]
fn r702_6c_equip_creature_token_targets_only_a_token() {
    // Team Pennant: "Equipped creature gets +1/+1 and has vigilance and trample. Equip creature
    // token {1}. Equip {3}"
    let mut t = Board::new(2).named(0, Zone::Battlefield, "pennant", "Team Pennant").battlefield(0, &["Grizzly Bears"])
        .hand(0, &["Vulshok Splitter"]).build();
    t.mana(0, "{3}{R}").cast(0, "Vulshok Splitter", vec![], mana_pool("CCCR")).unwrap();
    t.resolve();
    let (pennant, bears) = (t.id("pennant"), t.find_in_battlefield("Grizzly Bears"));
    let rebel = *t.g.battlefield.iter().find(|o| t.g.obj(**o).token()).expect("a Rebel");
    t.mana(0, "{1}");
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, 0, pennant, 0, bears, "C"));
    cite!(["702.6c"]; ok: equip(&mut t, 0, pennant, 0, rebel, "C"));
    t.resolve();
    // the Rebel is 2/2, with Vulshok Splitter's +2/+0 and the Pennant's +1/+1
    cite!(["702.6c", "613.4c"]; assert_eq!((t.g.power(rebel), t.g.toughness(rebel)), (Some(5), Some(3))));
    cite!(["702.6c", "613.1f"]; assert!(t.g.has(rebel, "Vigilance") && t.g.has(rebel, "Trample")));
}

// Rules: 702.6c, 903.3
// Given: Unstable Molecule Suit, Grizzly Bears, and Isamaru, Hound of Konda, the commander of
// player 0, on the battlefield.
// When: the Suit's equip commander {2} targets the Bears, then Isamaru.
// Expect: the Bears are no commander; Isamaru gets +2/+2 and indestructible.
#[test]
fn r702_6c_equip_commander_targets_only_a_commander() {
    // Unstable Molecule Suit: "Equipped creature gets +2/+2 and has indestructible. Equip
    // commander {2}. Equip {4}"
    let mut t = Board::new(2).named(0, Zone::Battlefield, "suit", "Unstable Molecule Suit").battlefield(0, &["Grizzly Bears"])
        .named(0, Zone::Command, "dog", "Isamaru, Hound of Konda").build();
    let (suit, dog) = (t.id("suit"), t.id("dog"));
    t.g.designate_commander(dog).unwrap();
    t.g.move_object(dog, Zone::Battlefield);
    let (bears, dog) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Isamaru, Hound of Konda"));
    t.mana(0, "{2}");
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, 0, suit, 0, bears, "CC"));
    cite!(["702.6c", "903.3"]; ok: equip(&mut t, 0, suit, 0, dog, "CC"));
    t.resolve();
    t.expect().rules(["702.6c", "613.4c"]).pt("Isamaru, Hound of Konda", 4, 4).check();
    cite!(["702.12b", "613.1f"]; assert!(t.g.has(dog, "Indestructible")));
}
