//! For Mirrodin! (702.163a): "When this Equipment enters, create a 2/2 red Rebel creature
//! token, then attach this Equipment to it." Every card is read from its Oracle text.
use crate::support;
use support::*;
use ir::Color;

fn rebel(t: &T) -> ObjId {
    *t.g.battlefield.iter().find(|o| t.g.obj(**o).token()).expect("a Rebel token")
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Vulshok Splitter", "Kemba's Banner", "Mirran Bardiche"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_163a_it_creates_a_rebel_and_attaches_to_it() {
    // Vulshok Splitter: "For Mirrodin! Equipped creature gets +2/+0. Equip {2}{R}"
    let mut t = Board::new(2).hand(0, &["Vulshok Splitter"]).build();
    t.mana(0, "{3}{R}").cast(0, "Vulshok Splitter", vec![], mana_pool("CCCR")).unwrap();
    t.pass_all();
    // it entered, and its ability triggered (603.2)
    t.expect().rules(["702.163a", "603.2"]).stack(&["Vulshok Splitter"]).zone_exact(0, Zone::Battlefield, &["Vulshok Splitter"]).check();
    t.resolve();
    let (splitter, rebel) = (t.find_in_battlefield("Vulshok Splitter"), rebel(&t));
    cite!(["702.163a", "701.3a"]; assert_eq!(t.g.attached_to(splitter), Some(Target::Object(rebel))));
    cite!(["702.163a", "111.4"]; assert!(t.g.obj(rebel).chars.colors == vec![Color::Red]
        && t.g.obj(rebel).chars.subtypes.iter().any(|s| s.name() == "Rebel")));
    cite!(["702.163a", "301.5"]; assert_eq!((t.g.power(rebel), t.g.toughness(rebel)), (Some(4), Some(2))));
}

#[test]
fn r702_163a_the_rebel_enters_before_it_is_equipped() {
    // Kemba's Banner: "Equipped creature gets +1/+1 for each creature you control."
    // Ruling: "The Rebel enters the battlefield as a 2/2 creature, then the Equipment becomes
    // attached to it." With two Bears it is 2/2 + 3/3.
    let mut t = Board::new(2).hand(0, &["Kemba's Banner"]).battlefield(0, &["Grizzly Bears", "Grizzly Bears"]).build();
    t.mana(0, "{3}{W}").cast(0, "Kemba's Banner", vec![], mana_pool("CCCW")).unwrap();
    t.resolve();
    let rebel = rebel(&t);
    cite!(["702.163a", "613.4c"]; assert_eq!((t.g.power(rebel), t.g.toughness(rebel)), (Some(5), Some(5))));
}

#[test]
fn r702_163a_the_equipment_stays_when_the_rebel_dies() {
    // Mirran Bardiche: "Equipped creature gets +2/+1 and has vigilance." Ruling: "If the
    // Rebel is destroyed, the Equipment stays on the battlefield."
    let mut t = Board::new(2).fill(5).hand(0, &["Mirran Bardiche"]).hand(1, &["Lightning Bolt"]).build();
    t.mana(0, "{4}{W}").cast(0, "Mirran Bardiche", vec![], mana_pool("CCCCW")).unwrap();
    t.resolve();
    let rebel = rebel(&t);
    cite!(["702.163a", "702.20a"]; assert!(t.g.has(rebel, "Vigilance")));
    t.to_no_combat(1, Part::PrecombatMain);
    let rebel = t.g.battlefield.iter().copied().find(|o| t.g.obj(*o).token()).unwrap();
    t.mana(1, "{R}").cast(1, "Lightning Bolt", vec![Tgt::Object(rebel)], mana_pool("R")).unwrap();
    t.resolve();
    let bardiche = t.find_in_battlefield("Mirran Bardiche");
    t.expect().rules(["704.5g", "301.5c"]).zone_exact(0, Zone::Battlefield, &["Mirran Bardiche"]).check();
    cite!(["301.5c", "704.5n"]; assert_eq!(t.g.attached_to(bardiche), None));
}
