//! Setting basic land types (305.7), its layer-4
//! dependencies (613.8) and timestamps (613.7), and entry reveals (614.12). Real cards only.
use crate::support::{cite, paid};
use crate::support;
use ir::{CardType, Subtype};
use support::*;

fn subtypes(t: &T, name: &str) -> Vec<String> {
    let mut s: Vec<String> = t.g.seen(t.find_in_battlefield(name), None).subtypes.iter().map(|s| s.name().to_string()).collect();
    s.sort();
    s
}

/// The colors of mana the land's abilities can add, by tapping each in a separate branch.
fn taps_for(t: &T, name: &str) -> usize { t.g.abilities(t.find_in_battlefield(name)).len() }

#[test]
fn r613_8_blood_moon_turns_off_urborg_whatever_the_timestamps() {
    // Urborg's effect depends on Blood Moon's (613.8a): Blood Moon removes Urborg's ability,
    // so it applies first and Urborg's effect no longer exists.
    for moon_first in [true, false] {
        let b = Board::new(2);
        let b = if moon_first { b.battlefield(1, &["Blood Moon"]).battlefield(0, &["Urborg, Tomb of Yawgmoth", "Forest"]) }
            else { b.battlefield(0, &["Urborg, Tomb of Yawgmoth", "Forest"]).battlefield(1, &["Blood Moon"]) };
        let t = b.build();
        cite!(["613.8a"]; case: format!("moon first: {moon_first}"); assert_eq!(subtypes(&t, "Urborg, Tomb of Yawgmoth"), vec!["Mountain"], "moon first: {moon_first}"));
        cite!(["613.8a"]; case: format!("moon first: {moon_first}"); assert_eq!(subtypes(&t, "Forest"), vec!["Forest"], "the basic Forest is no Swamp (moon first: {moon_first})"));
        assert_eq!(taps_for(&t, "Forest"), 1);
    }
}

#[test]
fn r613_8_magus_of_the_moon_turns_off_yavimaya_too() {
    let t = Board::new(2).battlefield(0, &["Yavimaya, Cradle of Growth", "Island"]).battlefield(1, &["Magus of the Moon"]).build();
    cite!(["305.7", "613.8a"]; assert_eq!(subtypes(&t, "Yavimaya, Cradle of Growth"), vec!["Mountain"]));
    cite!(["613.8a"]; assert_eq!(subtypes(&t, "Island"), vec!["Island"]));
}

#[test]
fn r613_7_spreading_seas_and_urborg_apply_in_timestamp_order() {
    // Neither depends on the other: the later one wins on the Forest.
    let mut t = Board::new(2).battlefield(0, &["Urborg, Tomb of Yawgmoth", "Forest"]).hand(0, &["Spreading Seas"]).library(0, &["Island"]).build();
    t.mana(0, "{1}{U}").cast(0, "Spreading Seas", vec!["Forest".into()], crate::support::mana_pool("CU")).unwrap();
    t.resolve();
    t.expect().zone(0, Zone::Hand, &["Island"]).check();
    cite!(["613.7", "305.7"]; assert_eq!(subtypes(&t, "Forest"), vec!["Island"], "Seas is later: the Forest is only an Island"));
    assert_eq!(taps_for(&t, "Forest"), 1);

    let mut t = Board::new(2).battlefield(0, &["Forest"]).hand(0, &["Spreading Seas", "Urborg, Tomb of Yawgmoth"]).library(0, &["Island"]).build();
    t.mana(0, "{1}{U}").cast(0, "Spreading Seas", vec!["Forest".into()], crate::support::mana_pool("CU")).unwrap();
    t.resolve();
    t.expect().zone(0, Zone::Hand, &["Island", "Urborg, Tomb of Yawgmoth"]).check();
    let urborg = t.find_in(0, Zone::Hand, "Urborg, Tomb of Yawgmoth");
    t.g.act(0, Move::PlayLand(urborg)).unwrap();
    cite!(["613.7"]; assert_eq!(subtypes(&t, "Forest"), vec!["Island", "Swamp"], "Urborg is later: Island and Swamp"));
    cite!(["305.6"]; assert_eq!(taps_for(&t, "Forest"), 2));
}

#[test]
fn r305_7_spreading_seas_on_a_land_creature_keeps_its_creature_type_and_color() {
    let mut t = Board::new(2).battlefield(0, &["Dryad Arbor"]).hand(0, &["Spreading Seas"]).library(0, &["Island"]).build();
    t.mana(0, "{1}{U}").cast(0, "Spreading Seas", vec!["Dryad Arbor".into()], crate::support::mana_pool("CU")).unwrap();
    t.resolve();
    t.expect().zone(0, Zone::Hand, &["Island"]).check();
    let arbor = t.find_in_battlefield("Dryad Arbor");
    let seen = t.g.seen(arbor, None);
    cite!(["305.7"]; assert!(seen.types.contains(&CardType::Creature) && seen.types.contains(&CardType::Land)));
    cite!(["305.7"]; assert_eq!(subtypes(&t, "Dryad Arbor"), vec!["Dryad", "Island"]));
    assert_eq!(seen.colors, vec![ir::Color::Green]);
    cite!(["305.7"]; assert_eq!(taps_for(&t, "Dryad Arbor"), 1, "only {{T}}: Add {{U}}"));
}

#[test]
fn r305_7_evil_presence_removes_a_nonbasic_lands_activated_ability() {
    let mut t = Board::new(2).battlefield(0, &["Gavony Township"]).hand(0, &["Evil Presence"]).build();
    assert_eq!(taps_for(&t, "Gavony Township"), 2);
    t.mana(0, "{B}").cast(0, "Evil Presence", vec!["Gavony Township".into()], crate::support::mana_pool("B")).unwrap();
    t.resolve();
    cite!(["305.7"]; assert_eq!(subtypes(&t, "Gavony Township"), vec!["Swamp"]));
    cite!(["305.7"]; assert_eq!(taps_for(&t, "Gavony Township"), 1, "only {{T}}: Add {{B}}"));
}

#[test]
fn r305_7_an_animated_land_stays_a_creature_when_blood_moon_arrives() {
    // The resolved ability's effect isn't an ability of the land: it keeps applying (611.2).
    let mut t = Board::new(2).battlefield(0, &["Mutavault"]).hand(0, &["Blood Moon"]).build();
    let vault = t.find_in_battlefield("Mutavault");
    t.mana(0, "{1}");
    t.g.act(0, paid("C", Move::Activate { source: vault, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    t.resolve();
    assert_eq!(t.g.seen(vault, None).pt, Some((2, 2)));
    t.mana(0, "{2}{R}").cast(0, "Blood Moon", vec![], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    let seen = t.g.seen(vault, None);
    cite!(["611.2a", "305.7"]; assert!(seen.types.contains(&CardType::Creature), "still a 2/2 creature until end of turn"));
    assert_eq!(seen.pt, Some((2, 2)));
    assert!(seen.subtypes.contains(&Subtype::from_name("Mountain").unwrap()));
    cite!(["205.1a"]; assert!(seen.subtypes.contains(&Subtype::from_name("Bear").unwrap()), "all creature types remain"));
    cite!(["305.7"]; assert_eq!(taps_for(&t, "Mutavault"), 1, "only {{T}}: Add {{R}}"));
}

#[test]
fn r614_12a_a_nonbasic_card_with_the_type_qualifies_for_the_reveal() {
    let mut t = Board::new(2).hand(0, &["Choked Estuary", "Underground Sea"]).build();
    let estuary = t.find_in(0, Zone::Hand, "Choked Estuary");
    t.g.act(0, Move::PlayLand(estuary)).unwrap();
    let sea = t.find_in(0, Zone::Hand, "Underground Sea");
    cite!(["614.12a"]; assert!(matches!(&t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::Select { among, .. } }) if among == &vec![sea]), "{:?}", t.g.decision));
    t.choose(0, &["Underground Sea"]).unwrap();
    t.expect().tapped("Choked Estuary", false).contains(0, Zone::Hand, "Underground Sea").check();
}
