//! "Equip [quality]" (702.6c): an equip ability that may target only a creature you control
//! with that quality; and equip granted to other permanents (613.1f). Every card is read
//! from its Oracle text.
use crate::support;
use support::*;
use ir::Ability;

/// Its equip abilities as it has them now, in order: the activated abilities whose effect
/// attaches it.
fn equips(t: &T, o: ObjId) -> Vec<usize> {
    t.g.abilities(o).iter().enumerate().filter(|(_, a)| matches!(&***a, Ability::Activated { .. })).map(|(i, _)| i).collect()
}
fn equip(t: &mut T, source: ObjId, nth: usize, to: ObjId, mana: &str) -> Result<(), String> {
    let ability = equips(t, source)[nth];
    t.g.act(0, paid(mana, Move::Activate { source, ability, targets: vec![vec![Target::Object(to)]], paying: vec![] }))
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Steelclaw Lance", "Blackblade Reforged", "Robe of the Archmagi", "Syr Gwyn, Hero of Ashvale", "Armed with Proof"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_6c_equip_knight_targets_only_a_knight_you_control() {
    // Steelclaw Lance: "Equipped creature gets +2/+2. Equip Knight {1}. Equip {3}"
    let mut t = Board::new(2).named(0, Zone::Battlefield, "lance", "Steelclaw Lance")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "knight", "Knight of Meadowgrain")
        .named(1, Zone::Battlefield, "theirs", "Knight of the Reliquary").build();
    let (lance, bears, knight, theirs) = (t.id("lance"), t.id("bears"), t.id("knight"), t.id("theirs"));
    cite!(["702.6c", "702.6d"]; assert_eq!(equips(&t, lance).len(), 2));
    let before = t.state();
    t.mana(0, "{1}");
    // the Bears are not a Knight
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, lance, 0, bears, "C"));
    // a Knight another player controls
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, lance, 0, theirs, "C"));
    // Uncited (implementation contract): a refused activation changes nothing.
    assert_eq!(t.state(), before);
    cite!(["702.6c"]; ok: equip(&mut t, lance, 0, knight, "C"));
    t.resolve();
    t.expect().rules(["702.6c", "301.5"]).pt("Knight of Meadowgrain", 4, 4).check();
    // "Equip {3}" targets any creature you control (702.6a, 702.6d)
    t.mana(0, "{3}");
    cite!(["702.6a", "702.6d"]; ok: equip(&mut t, lance, 1, bears, "CCC"));
    t.resolve();
    t.expect().rules(["702.6a", "301.5c"]).pt("Grizzly Bears", 4, 4).pt("Knight of Meadowgrain", 2, 2).check();
}

#[test]
fn r702_6c_equip_legendary_creature() {
    // Blackblade Reforged: "Equipped creature gets +1/+1 for each land you control. Equip
    // legendary creature {3}. Equip {7}"
    let mut t = Board::new(2).named(0, Zone::Battlefield, "blade", "Blackblade Reforged")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "dog", "Isamaru, Hound of Konda")
        .battlefield(0, &["Plains", "Plains"]).build();
    let (blade, bears, dog) = (t.id("blade"), t.id("bears"), t.id("dog"));
    t.mana(0, "{3}");
    // the Bears are not legendary
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, blade, 0, bears, "CCC"));
    cite!(["702.6c"]; ok: equip(&mut t, blade, 0, dog, "CCC"));
    t.resolve();
    t.expect().rules(["702.6c", "301.5"]).pt("Isamaru, Hound of Konda", 4, 4).check();
}

#[test]
fn r702_6c_equip_one_of_several_qualities() {
    // Robe of the Archmagi: "Equip {4}. Equip Shaman, Warlock, or Wizard {1}"
    let mut t = Board::new(2).named(0, Zone::Battlefield, "robe", "Robe of the Archmagi")
        .named(0, Zone::Battlefield, "bears", "Grizzly Bears").named(0, Zone::Battlefield, "wizard", "Prodigal Sorcerer").build();
    let (robe, bears, wizard) = (t.id("robe"), t.id("bears"), t.id("wizard"));
    t.mana(0, "{1}");
    // the Bears are none of them
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, robe, 1, bears, "C"));
    cite!(["702.6c"]; ok: equip(&mut t, robe, 1, wizard, "C"));
    t.resolve();
    cite!(["702.6c", "701.3a"]; assert_eq!(t.g.attached_to(robe), Some(Target::Object(wizard))));
}

#[test]
fn r613_1f_granted_equip_knight_is_one_more_equip_ability() {
    // Syr Gwyn, Hero of Ashvale: "Equipment you control have equip Knight {0}." Ruling:
    // "Equipment you control still have their other equip abilities in addition to equip
    // Knight {0}."
    let mut t = Board::new(2).named(0, Zone::Battlefield, "gwyn", "Syr Gwyn, Hero of Ashvale")
        .named(0, Zone::Battlefield, "splitter", "Bonesplitter").named(0, Zone::Battlefield, "bears", "Grizzly Bears").build();
    let (gwyn, splitter, bears) = (t.id("gwyn"), t.id("splitter"), t.id("bears"));
    cite!(["613.1f", "702.6d"]; assert_eq!(equips(&t, splitter).len(), 2, "Equip {{1}} and equip Knight {{0}}"));
    // the granted one is after the printed one (613.1f)
    // the Bears are not a Knight
    cite!(["702.6c", "115.1"]; refused("an illegal target"): equip(&mut t, splitter, 1, bears, ""));
    cite!(["702.6c", "613.1f"]; ok: equip(&mut t, splitter, 1, gwyn, ""));
    t.resolve();
    t.expect().rules(["702.6c", "301.5"]).pt("Syr Gwyn, Hero of Ashvale", 7, 5).check();
}

#[test]
fn r613_1f_a_clue_that_is_equipment_has_equip() {
    // Armed with Proof: "When this enchantment enters, investigate twice. Clues you control are
    // Equipment in addition to their other types and have "Equipped creature gets +2/+0" and
    // equip {2}."
    let mut t = Board::new(2).hand(0, &["Armed with Proof"]).battlefield(0, &["Grizzly Bears"]).build();
    t.mana(0, "{2}{W}").cast(0, "Armed with Proof", vec![], mana_pool("CCW")).unwrap();
    t.resolve();
    let clues: Vec<ObjId> = t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).token()).collect();
    cite!(["701.16a"]; assert_eq!(clues.len(), 2));
    let bears = t.find_in_battlefield("Grizzly Bears");
    // a Clue's own ability, then the equip it was given
    cite!(["613.1d", "613.1f"]; assert_eq!(equips(&t, clues[0]).len(), 2));
    t.mana(0, "{2}");
    let ability = equips(&t, clues[0])[1];
    cite!(["702.6a", "613.1f"]; ok: t.g.act(0, paid("CC", Move::Activate { source: clues[0], ability, targets: vec![vec![Target::Object(bears)]], paying: vec![] })));
    t.resolve();
    t.expect().rules(["702.6a", "301.5", "613.1f"]).pt("Grizzly Bears", 4, 2).check();
}

#[test]
fn r702_6e_equip_planeswalker_remains_refused() {
    // Uncited (admission): "Equip planeswalker" attaches as though a planeswalker were a
    // creature, which is not modeled.
    let d = CardDef::new("Harness", "{0}", "Artifact — Equipment", "Equip planeswalker {1}", None).unwrap();
    assert_eq!(game::play::check(&d).expect_err("equip planeswalker remains unsupported").0, "keyword: Equip");
}
