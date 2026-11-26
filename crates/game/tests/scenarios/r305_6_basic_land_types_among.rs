//! Domain: "the number of basic land types among lands you control" counts each of the five
//! basic land types (305.6) once, whichever land has it and however many have it; a land with
//! two types counts for both. "The number of different mana values among nonland permanents
//! you control" counts each value once (202.3).
use crate::support;
use support::*;

// Rules: 305.6, 107.3a
// Given: player 0 controls Volcanic Island (Island Mountain), two Mountains and a Forest;
// player 1 controls a Plains. Player 0 holds Tribal Flames ("Domain — Tribal Flames deals X
// damage to any target, where X is the number of basic land types among lands you control.").
// When: player 0 casts Tribal Flames at player 1.
// Expect: X is 3 (Island, Mountain, Forest): the second Mountain and player 1's Plains count
// nothing.
#[test]
fn r305_6_tribal_flames_counts_each_basic_land_type_once() {
    let mut t = Board::new(2).battlefield(0, &["Volcanic Island", "Mountain", "Mountain", "Forest"]).battlefield(1, &["Plains"])
        .hand(0, &["Tribal Flames"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{1}{R}").cast(0, "Tribal Flames", vec![Tgt::Player(1)], mana_pool("CR")).unwrap();
    t.resolve();
    cite!(["305.6"]; assert_eq!(t.g.players[1].life, 17));
}

// Rules: 305.6, 611.2c
// Given: player 0 controls a Swamp and a Plains and holds Drag to the Bottom ("Domain — Each
// creature gets -X/-X until end of turn, where X is 1 plus the number of basic land types
// among lands you control."); player 1 controls Hill Giant (3/3) and Craw Wurm (6/4).
// When: player 0 casts it.
// Expect: X is 3: the Giant dies and the Wurm is 3/1.
#[test]
fn r305_6_drag_to_the_bottom_is_one_plus_the_basic_land_types() {
    let mut t = Board::new(2).battlefield(0, &["Swamp", "Plains"]).battlefield(1, &["Hill Giant", "Craw Wurm"])
        .hand(0, &["Drag to the Bottom"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.mana(0, "{2}{B}{B}").cast(0, "Drag to the Bottom", vec![], mana_pool("CCBB")).unwrap();
    t.resolve();
    t.expect().rules(["305.6", "704.5f"]).contains(1, Zone::Graveyard, "Hill Giant").pt("Craw Wurm", 3, 1).check();
}

// Rules: 202.3, 121.1
// Given: player 0 controls Grizzly Bears and Coral Merfolk (mana value 2 each), Hill Giant
// (mana value 4) and an Island, and holds Lunar Insight ("Draw a card for each different mana
// value among nonland permanents you control.").
// When: player 0 casts it.
// Expect: two different mana values, so two cards drawn; the land's 0 is not counted.
#[test]
fn r202_3_lunar_insight_counts_each_mana_value_once() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Coral Merfolk", "Hill Giant", "Island"])
        .hand(0, &["Lunar Insight"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    let before = t.g.players[0].hand.len();
    t.mana(0, "{2}{U}").cast(0, "Lunar Insight", vec![], mana_pool("CCU")).unwrap();
    t.resolve();
    cite!(["202.3", "121.1"]; assert_eq!(t.g.players[0].hand.len(), before - 1 + 2));
}

// Rules: 305.6, 604.3, 613.4a
// Given: player 0 controls Territorial Kavu ("power and toughness are each equal to the number
// of basic land types among lands you control"), a Forest and a Mountain, and holds an Island.
// When: they play the Island.
// Expect: the Kavu is 2/2, then 3/3.
#[test]
fn r604_3_territorial_kavu_grows_with_each_new_basic_land_type() {
    let mut t = Board::new(2).battlefield(0, &["Territorial Kavu", "Forest", "Mountain"]).hand(0, &["Island"]).fill(10).build();
    t.to_no_combat(0, Part::PrecombatMain);
    t.expect().rules(["305.6", "604.3"]).pt("Territorial Kavu", 2, 2).check();
    let island = t.find_in(0, Zone::Hand, "Island");
    t.g.act(0, Move::PlayLand(island)).unwrap();
    t.expect().rules(["305.6", "613.4a"]).pt("Territorial Kavu", 3, 3).check();
}
