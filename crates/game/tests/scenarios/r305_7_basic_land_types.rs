//! Setting a basic land type removes rules-text abilities, not the replacement mana ability.
use crate::support::cite;
use crate::support;
use support::Board;
use game::{Move,Zone};
use ir::{CardType,Subtype};

#[test]
fn r305_7_blood_moon_removes_entry_and_printed_abilities_but_grants_red_mana() {
    let mut t=Board::new(2).battlefield(1,&["Blood Moon"]).hand(0,&["Temple Garden"]).build();
    let land=t.find_in(0,Zone::Hand,"Temple Garden");
    t.g.act(0,Move::PlayLand(land)).unwrap();cite!(["614.12", "305.7"]; assert!(t.g.decision.is_none(),"the life choice was removed before entry"));
    let land=t.find_in_battlefield("Temple Garden");let seen=t.g.seen(land,None);
    cite!(["305.7"]; assert!(!seen.base_abilities));assert!(seen.intrinsic_abilities);
    cite!(["305.7"]; assert_eq!(seen.subtypes,vec![Subtype::from_name("Mountain").unwrap()]));
    assert!(!t.g.obj(land).tapped);
    t.g.act(0,Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();cite!(["305.6"]; assert_eq!(t.g.players[0].pool.colors,[0,0,0,1,0]));
    let moon=t.find_in_battlefield("Blood Moon");t.g.move_object(moon,Zone::Graveyard);
    let restored=t.g.seen(land,None);assert!(restored.base_abilities);assert!(restored.intrinsic_abilities);
    assert_eq!(restored.subtypes.len(),2);
}

#[test]
fn r305_7_adding_a_basic_land_type_keeps_printed_abilities_and_other_subtypes() {
    let t=Board::new(2).battlefield(0,&["Urborg, Tomb of Yawgmoth","Temple Garden"]).build();
    let land=t.find_in_battlefield("Temple Garden");let seen=t.g.seen(land,None);
    cite!(["305.7"]; assert!(seen.base_abilities));assert!(seen.intrinsic_abilities);
    cite!(["305.7"]; assert_eq!(seen.subtypes.len(),3));
    assert!(seen.subtypes.contains(&Subtype::from_name("Swamp").unwrap()));
}

#[test]
fn r305_7_type_setting_preserves_creature_type_color_and_separately_granted_abilities() {
    let mut t=Board::new(2).battlefield(0,&["Dryad Arbor"]).battlefield(1,&["Blood Moon"])
        .hand(0,&["Abundant Growth"]).library(0,&["Island"]).build();
    let land=t.find_in_battlefield("Dryad Arbor");let seen=t.g.seen(land,None);
    cite!(["305.7"]; assert!(seen.types.contains(&CardType::Creature)));assert_eq!(seen.pt,Some((1,1)));
    assert_eq!(seen.colors,vec![ir::Color::Green]);
    assert!(seen.subtypes.contains(&Subtype::from_name("Dryad").unwrap()));
    t.mana(0,"{G}").cast(0,"Abundant Growth",vec!["Dryad Arbor".into()], crate::support::mana_pool("G")).unwrap().resolve();
    t.expect().zone(0,Zone::Hand,&["Island"]).check(); // the entry trigger draws this card
    let seen=t.g.seen(land,None);
    assert!(!seen.base_abilities);assert!(seen.intrinsic_abilities);cite!(["305.7"]; assert_eq!(seen.given.len(),1));
    cite!(["305.7"]; assert_eq!(t.g.abilities(land).len(),2,"new basic mana and granted mana both survive"));
}
