//! Resolving one multi-object action and paying one plural cost are simultaneous events.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};

fn game(leader: &str, action: &str) -> Game {
    let defs = vec![
        CardDef::new("Leader", "{0}", "Creature", leader, Some((2, 2))).unwrap(),
        CardDef::new("Follower", "{0}", "Creature", "", Some((1, 1))).unwrap(),
        CardDef::new("Sweep", "{0}", "Sorcery", action, None).unwrap(),
        CardDef::new("Altar", "{0}", "Artifact", "Sacrifice two creatures: You gain 2 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, units: &str) {
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, paid(units, Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
}
#[test]
fn r701_8a_a_mass_destruction_checks_indestructibility_before_any_permanent_leaves() {
    for reversed in [false, true] {
        let mut g = game("Other creatures you control have indestructible.", "Destroy all creatures.");
        let (leader, follower) = if reversed {
            let follower = g.put(0, 1, Zone::Battlefield);
            (g.put(0, 0, Zone::Battlefield), follower)
        } else {
            (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield))
        };
        cast(&mut g, "");
        assert_eq!(g.obj(leader).zone, Zone::Gone);
        cite!(["702.12b", "701.8a"]; case: reversed; assert_eq!(g.obj(follower).zone, Zone::Battlefield, "order {reversed}"));
        assert!(!g.has(follower, "Indestructible"));
    }
}
#[test]
fn r608_2h_simultaneous_zone_changes_remember_characteristics_before_any_departure() {
    for text in ["Destroy all creatures.", "Exile all creatures.", "Return all creatures to their owners' hands."] {
        let mut g = game("Other creatures you control get +1/+1.", text);
        g.put(0, 0, Zone::Battlefield);
        let follower = g.put(0, 1, Zone::Battlefield);
        cast(&mut g, "");
        cite!(["608.2h"]; case: text; assert_eq!(g.power(follower), Some(2), "{text}"));
    }
}
#[test]
fn r601_2h_a_plural_sacrifice_cost_is_one_simultaneous_event() {
    let mut g = game("Whenever another creature dies, you gain 1 life.", "You gain 1 life.");
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    let altar = g.put(0, 3, Zone::Battlefield);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![leader, follower]] })
        .unwrap();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![leader, follower] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![follower, leader]))).unwrap();
    cite!(["601.2h", "603.10a"]; assert_eq!(g.stack.len(), 2, "the sacrificed leader sees the simultaneous follower death"));
    resolve(&mut g);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r603_2_separate_cost_parts_capture_triggers_before_later_costs_remove_the_source() {
    for leader_first in [false, true] {
        let mut g = game("Whenever another creature dies, you gain 1 life.", "You gain 1 life.");
        let altar_def = CardDef::new(
            "Separate Altar",
            "{0}",
            "Artifact",
            "Sacrifice a creature, Sacrifice a creature: You gain 2 life.",
            None,
        )
        .unwrap();
        game::play::check(&altar_def).unwrap();
        std::sync::Arc::make_mut(&mut g.defs).push(altar_def);
        let leader = g.put(0, 0, Zone::Battlefield);
        let follower = g.put(0, 1, Zone::Battlefield);
        let altar = g.put(0, 4, Zone::Battlefield);
        let paying = if leader_first { vec![vec![leader], vec![follower]] } else { vec![vec![follower], vec![leader]] };
        g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying }).unwrap();
        cite!(["603.10a"]; case: leader_first; assert_eq!(g.stack.len(), if leader_first { 1 } else { 2 }));
        while !g.stack.is_empty() {
            resolve(&mut g);
        }
        assert_eq!(g.players[0].life, if leader_first { 22 } else { 23 });
    }
}
#[test]
fn r608_2c_separate_destructions_recheck_the_state_after_the_first_instruction() {
    let mut g =
        game("Other creatures you control have indestructible.", "Destroy target creature. Destroy target creature.");
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 2, Zone::Hand);
    g.act(
        0,
        Move::Cast { card, targets: vec![vec![game::Target::Object(leader)], vec![game::Target::Object(follower)]] },
    )
    .unwrap();
    resolve(&mut g);
    cite!(["608.2c", "702.12b"]; assert!(g.battlefield.is_empty(), "the second instruction no longer sees the indestructibility grant"));
}
#[test]
fn r608_2e_multiplayer_sacrifices_share_one_pre_event_state() {
    let mut g = game(
        "Other creatures get +1/+1.\nWhenever another creature dies, you gain 1 life.",
        "Each player sacrifices a creature.",
    );
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, "");
    cite!(["608.2e"]; assert_eq!((g.obj(leader).zone, g.obj(follower).zone), (Zone::Gone, Zone::Gone)));
    cite!(["608.2e", "608.2h"]; assert_eq!(g.power(follower), Some(2)));
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r601_2h_plural_additional_costs_preserve_last_known_characteristics_and_triggers() {
    let mut g = game(
        "Other creatures you control get +1/+1.\nWhenever another creature dies, you gain 1 life.",
        "As an additional cost to cast this spell, sacrifice two creatures.\nYou gain 2 life.",
    );
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, Move::WithCosts(vec![vec![leader, follower]], Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![leader, follower] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![follower, leader]))).unwrap();
    cite!(["601.2h"]; assert_eq!(g.power(follower), Some(2)));
    cite!(["601.2h", "603.10a"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
}
#[test]
fn r701_19a_mass_destruction_applies_regeneration_without_consuming_an_indestructible_objects_shield() {
    let mut g = game("Other creatures you control have indestructible.", "Destroy all creatures.");
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    g.regenerate(leader);
    g.regenerate(follower);
    cast(&mut g, "");
    cite!(["701.19a"]; assert_eq!(g.battlefield, vec![leader, follower]));
    cite!(["701.19a"]; assert!(g.obj(leader).tapped));
    cite!(["702.12b", "701.19a"]; assert!(!g.obj(follower).tapped));
    g.exile(leader);
    cite!(["701.19a"]; assert!(!g.destroy(follower), "the follower's shield was not consumed by the sweep"));
    assert!(g.obj(follower).tapped);
    assert_eq!(g.obj(follower).zone, Zone::Battlefield);
}
#[test]
fn r701_8a_planar_cleansing_preserves_a_creature_shielded_by_a_departing_aura() {
    let defs = vec![
        CardDef::new("Shielded by Faith", "{1}{W}{W}", "Enchantment — Aura", "Enchant creature\nEnchanted creature has indestructible.\nWhenever a creature enters, you may attach this Aura to that creature.", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Planar Cleansing", "{3}{W}{W}{W}", "Sorcery", "Destroy all nonland permanents.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    // Put the Aura first so the implementation cannot rely on battlefield insertion order.
    let aura = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    assert!(g.attach(aura, game::Target::Object(bear)));
    for _ in 0..6 {
        g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::White));
    }
    cast(&mut g, "WWWWWW");
    assert_eq!(g.obj(aura).zone, Zone::Gone);
    cite!(["701.8a"]; assert_eq!(g.battlefield, vec![bear]));
    assert!(!g.has(bear, "Indestructible"));
}
