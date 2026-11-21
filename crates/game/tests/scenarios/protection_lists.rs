//! Protection from several listed qualities (702.16g): "protection from [quality A] and from
//! [quality B]" behaves as separate protection abilities, so a list of three stops each of the
//! three and nothing else. Real card text, read by the grammar and run by the engine.
//!
//! These fixtures exercise fixed-color lists. Other protection qualities have their own
//! tests; disjunctive lists remain explicit admission refusals. Existing attachment
//! state-based actions are tested in protection.rs. This is scoped evidence.
use crate::support::cite;
use game::select::Ctx;
use game::{CardDef, Game, Target, Zone};

const LISTED: &str = "Protection from blue, from black, and from red";
/// Blue, black, red (listed), then green (not listed).
const COLORS: [&str; 4] = ["{U}", "{B}", "{R}", "{G}"];

fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}

/// The protected creature, then for each color a creature, an Equipment and an Aura of it.
fn game() -> Game {
    let mut defs = vec![def("Oversoul", "{G}{W}", "Creature — Spirit Avatar", LISTED, Some((5, 5)))];
    for cost in COLORS {
        defs.push(def("Source", cost, "Creature — Spirit", "", Some((1, 1))));
        defs.push(def("Gear", cost, "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None));
        defs.push(def("Charm", cost, "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None));
    }
    Game::new(defs, 2, 20)
}

fn stopped(color: usize) -> bool {
    color < 3
}

#[test]
fn r702_16g_a_list_of_three_is_read_as_three_qualities() {
    let text = LISTED;
    let card = CardDef::new("Oversoul", "{G}{W}", "Creature — Spirit Avatar", text, Some((5, 5))).unwrap();
    assert!(game::play::check(&card).is_ok(), "the engine runs protection from fixed colors in any list");
    for or in [
        "Protection from blue, from black, or from red",
        "Protection from artifacts or from the color of your choice",
    ] {
        let card = CardDef::new("Choice", "{W}", "Creature — Spirit", or, Some((1, 1))).unwrap();
        crate::support::refused(game::play::check(&card).map_err(|e|e.0),"keyword: Protection");
    }
}

#[test]
fn r702_16b_three_listed_colors_cannot_target_and_a_fourth_can() {
    for color in 0..4 {
        let mut g = game();
        let guard = g.put(0, 0, Zone::Battlefield);
        let source = g.put(1, 1 + 3 * color, Zone::Battlefield);
        cite!(["702.16g", "702.16b"]; case: color; assert_eq!(!g.can_target(&Ctx::new(1, source), Target::Object(guard)), stopped(color), "color {color}"));
    }
}

#[test]
fn r702_16c_three_listed_colors_cannot_enchant_and_a_fourth_can() {
    for color in 0..4 {
        let mut g = game();
        let guard = g.put(1, 0, Zone::Battlefield);
        let aura = g.put(0, 3 + 3 * color, Zone::Battlefield);
        cite!(["702.16g", "702.16c"]; case: color; assert_eq!(!g.attach(aura, Target::Object(guard)), stopped(color), "color {color}"));
    }
}

#[test]
fn r702_16d_three_listed_colors_cannot_equip_and_a_fourth_can() {
    for color in 0..4 {
        let mut g = game();
        let guard = g.put(1, 0, Zone::Battlefield);
        let gear = g.put(0, 2 + 3 * color, Zone::Battlefield);
        cite!(["702.16g", "702.16d"]; case: color; assert_eq!(!g.attach(gear, Target::Object(guard)), stopped(color), "color {color}"));
    }
}

#[test]
fn r702_16e_damage_from_three_listed_colors_is_prevented_and_a_fourth_is_not() {
    for color in 0..4 {
        let mut g = game();
        let guard = g.put(1, 0, Zone::Battlefield);
        let source = g.put(0, 1 + 3 * color, Zone::Battlefield);
        g.damage(source, Target::Object(guard), 1);
        cite!(["702.16g", "702.16e"]; case: color; assert_eq!(g.obj(guard).damage == 0, stopped(color), "color {color}"));
    }
}

#[test]
fn r702_16f_three_listed_colors_cannot_block_it_and_a_fourth_can() {
    for color in 0..4 {
        let mut g = game();
        let guard = g.put(0, 0, Zone::Battlefield);
        let blocker = g.put(1, 1 + 3 * color, Zone::Battlefield);
        cite!(["702.16g", "702.16f"]; case: color; assert_eq!(g.cant_block(blocker, guard, &[blocker]), stopped(color), "color {color}"));
    }
}
