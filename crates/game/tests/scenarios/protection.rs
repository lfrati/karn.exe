//! One protection quality governs targeting, damage, blocking and attachment (702.16).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, Target, Zone};
fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn game() -> Game {
    Game::new(
        vec![
            def("Guard", "{W}", "Creature — Human", "Protection from red", Some((2, 2))),
            def("Red", "{R}", "Creature — Spirit", "Lifelink\nDeathtouch", Some((3, 3))),
            def("Blue", "{U}", "Creature — Spirit", "", Some((3, 3))),
            def("Bolt", "{R}", "Instant", "Bolt deals 2 damage to target creature.", None),
            def("Sweep", "{R}", "Sorcery", "Destroy all creatures.", None),
            def("Shield", "{W}", "Instant", "Target creature gains protection from red until end of turn.", None),
            def("Aura", "{R}", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +1/+1.", None),
            def("Equipment", "{R}", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None),
        ],
        2,
        20,
    )
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn cast(g: &mut Game, d: usize, target: Option<game::ObjId>, mana: &str) {
    g.players[0].pool.colors = [10; 5];
    let card = g.put(0, d, Zone::Hand);
    g.act(0, paid(mana, Move::Cast { card, targets: target.map(|o| vec![vec![Target::Object(o)]]).unwrap_or_default() })).unwrap();
    resolve(g);
}
#[test]
fn r702_16b_protection_refuses_matching_spell_targets_for_either_controller() {
    for owner in 0..2 {
        let mut g = game();
        let guard = g.put(owner, 0, Zone::Battlefield);
        let bolt = g.put(0, 3, Zone::Hand);
        g.players[0].pool.colors[3] = 1;
        let before = format!("{g:?}");
        cite!(["702.16b"]; refused("an illegal target, Guard: it has protection from that (702.16b)"): g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(guard)]] })));
        assert_eq!(format!("{g:?}"), before);
    }
}
#[test]
fn r702_16e_prevented_damage_has_no_damage_event_lifelink_or_deathtouch_result() {
    let mut g = game();
    let guard = g.put(1, 0, Zone::Battlefield);
    let red = g.put(0, 1, Zone::Battlefield);
    let before = g.log.len();
    g.damage(red, Target::Object(guard), 3);
    cite!(["702.16e"]; assert_eq!(g.obj(guard).damage, 0));
    cite!(["702.16e"]; assert_eq!(g.players[0].life, 20));
    assert!(!g.log[before..].iter().any(|e| matches!(e, Event::DamageDealt { .. })));
    let blue = g.put(0, 2, Zone::Battlefield);
    g.damage(blue, Target::Object(guard), 1);
    cite!(["702.16e"]; assert_eq!(g.obj(guard).damage, 1));
}
#[test]
fn r702_16f_protection_restricts_blockers_not_what_the_protected_creature_can_block() {
    let mut g = game();
    let guard = g.put(0, 0, Zone::Battlefield);
    let red = g.put(1, 1, Zone::Battlefield);
    let blue = g.put(1, 2, Zone::Battlefield);
    cite!(["702.16f"]; assert!(g.cant_block(red, guard, &[red])));
    assert!(!g.cant_block(blue, guard, &[blue]));
    cite!(["702.16f"]; assert!(!g.cant_block(guard, red, &[guard])));
}
#[test]
fn r702_16_protection_does_not_stop_nontargeting_destruction() {
    let mut g = game();
    let guard = g.put(1, 0, Zone::Battlefield);
    cast(&mut g, 4, None, "R");
    cite!(["115.10a"]; assert_eq!(g.obj(guard).zone, Zone::Gone));
}
#[test]
fn r702_16b_gaining_protection_rechecks_targets_on_resolution() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let bolt = g.put(0, 3, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bear)]] })).unwrap();
    let spell = *g.stack.last().unwrap();
    cast(&mut g, 5, Some(bear), "W");
    resolve(&mut g);
    assert_eq!(g.obj(bear).damage, 0);
    assert_eq!(g.obj(bear).zone, Zone::Battlefield);
    cite!(["702.16b", "608.2b"]; assert!(g.log.contains(&Event::Fizzled { spell }), "illegal target, not merely prevented damage"));
}
#[test]
fn r702_16c_d_protection_removes_matching_attachments_but_only_the_aura_dies() {
    let mut g = game();
    let bear = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 6, Some(bear), "R");
    let aura = *g.battlefield.last().unwrap();
    let equipment = g.put(0, 7, Zone::Battlefield);
    g.act(
        0,
        Move::Activate { source: equipment, ability: 2, targets: vec![vec![Target::Object(bear)]], paying: vec![] },
    )
    .unwrap();
    resolve(&mut g);
    assert_eq!(g.attached_to(aura), Some(Target::Object(bear)));
    assert_eq!(g.attached_to(equipment), Some(Target::Object(bear)));
    cast(&mut g, 5, Some(bear), "W");
    cite!(["702.16c"]; assert_eq!(g.obj(aura).zone, Zone::Gone));
    cite!(["702.16d"]; assert_eq!(g.obj(equipment).zone, Zone::Battlefield));
    cite!(["702.16d"]; assert_eq!(g.attached_to(equipment), None));
    cite!(["702.16d"]; assert!(!g.attach(equipment, Target::Object(bear)), "nontargeting attachment is prohibited too"));
}

#[test]
fn r702_16e_unpreventable_damage_bypasses_prevention_but_not_targeting() {
    let mut g = game();
    let guard = g.put(1, 0, Zone::Battlefield);
    let red = g.put(0, 1, Zone::Battlefield);
    std::sync::Arc::make_mut(&mut g.defs).push(def(
        "Unstoppable",
        "{0}",
        "Instant",
        "Damage can't be prevented this turn.",
        None,
    ));
    cast(&mut g, 8, None, "");
    g.damage(red, Target::Object(guard), 1);
    cite!(["615.12"]; assert_eq!(g.obj(guard).damage, 1));
    assert_eq!(g.players[0].life, 21);
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(0, red), Target::Object(guard))));
}

#[test]
fn r615_12_unpreventable_composes_with_protection_given_later_and_ends_with_the_turn() {
    let mut g = game();
    for player in 0..2 {
        for _ in 0..4 { g.put(player, 2, Zone::Library); }
    }
    let guard = g.put(1, 0, Zone::Battlefield);
    let blue = g.put(1, 2, Zone::Battlefield);
    let red = g.put(0, 1, Zone::Battlefield);
    std::sync::Arc::make_mut(&mut g.defs).push(def("Unstoppable", "{0}", "Instant", "Damage can't be prevented this turn.", None));
    // prevented before the rule is in force
    g.damage(red, Target::Object(guard), 1);
    assert_eq!(g.obj(guard).damage, 0);
    // said twice, it is one question; and protection given after it is in force prevents nothing
    cast(&mut g, 8, None, "");
    cast(&mut g, 8, None, "");
    cast(&mut g, 5, Some(blue), "W");
    assert!(g.unpreventable());
    g.damage(red, Target::Object(blue), 1);
    g.damage(red, Target::Object(guard), 1);
    cite!(["615.12"]; assert_eq!((g.obj(blue).damage, g.obj(guard).damage), (1, 1)));
    g.state_based_actions();
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![guard, blue] } }));
    g.act(1, Move::Choose(game::state::Answer::Objects(vec![blue, guard]))).unwrap();
    // the turn ends (514.2): both rules with it, and protection prevents again
    while g.turn == 1 {
        match g.decision.clone() {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["514.2"]; assert!(!g.unpreventable()));
    // the one dealt deathtouch damage has died
    let guard = g.put(1, 0, Zone::Battlefield);
    let life = g.players[0].life;
    g.damage(red, Target::Object(guard), 1);
    cite!(["702.16e"]; assert_eq!((g.obj(guard).damage, g.players[0].life), (0, life), "prevented: no damage, no lifelink"));
}

#[test]
fn r702_16a_source_color_changes_and_last_known_colors_change_what_is_protected() {
    for departed in [false, true] {
        let mut g = game();
        let guard = g.put(1, 0, Zone::Battlefield);
        let red = g.put(0, 1, Zone::Battlefield);
        std::sync::Arc::make_mut(&mut g.defs).push(def(
            "Dye",
            "{U}",
            "Instant",
            "Target creature becomes blue until end of turn.",
            None,
        ));
        cast(&mut g, 8, Some(red), "U");
        if departed {
            g.exile(red);
        }
        cite!(["702.16b"]; case: format!("departed={departed}"); assert!(g.can_target(&game::select::Ctx::new(0, red), Target::Object(guard))));
        g.damage(red, Target::Object(guard), 1);
        cite!(["702.16e"]; case: format!("departed={departed}"); assert_eq!(g.obj(guard).damage, 1, "current and last-known source colors are blue"));
    }
}

#[test]
fn r702_16b_abilities_use_the_sources_quality_even_after_the_source_is_sacrificed() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    std::sync::Arc::make_mut(&mut g.defs).push(def(
        "Thrower",
        "{R}",
        "Creature — Spirit",
        "Sacrifice this creature: This creature deals 2 damage to target creature.",
        Some((2, 2)),
    ));
    let source = g.put(0, 8, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(bear)]], paying: vec![] }).unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    cast(&mut g, 5, Some(bear), "W");
    resolve(&mut g);
    cite!(["702.16b", "608.2b"]; assert_eq!(g.obj(bear).damage, 0));
}

#[test]
fn r514_2_temporary_protection_ends_at_cleanup() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    let red = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 5, Some(bear), "W");
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(0, red), Target::Object(bear))));
    for _ in 0..100 {
        if g.turn > 1 {
            break;
        }
        match g.decision {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert_eq!(g.turn, 2);
    cite!(["514.2"]; assert!(g.can_target(&game::select::Ctx::new(0, red), Target::Object(bear))));
}

#[test]
fn r702_16g_multiple_fixed_colors_and_redundant_instances_use_the_same_quality_test() {
    let mut g = game();
    std::sync::Arc::make_mut(&mut g.defs).extend([
        def(
            "Crusader",
            "{W}",
            "Creature — Human",
            "Protection from black and from green\nProtection from black",
            Some((2, 2)),
        ),
        def("Hybrid", "{B/G}", "Creature — Spirit", "", Some((2, 2))),
        def("Blank", "{0}", "Artifact Creature — Golem", "", Some((2, 2))),
        def("Black", "{B}", "Creature — Spirit", "", Some((2, 2))),
        def("Green", "{G}", "Creature — Spirit", "", Some((2, 2))),
    ]);
    let guard = g.put(0, 8, Zone::Battlefield);
    let hybrid = g.put(1, 9, Zone::Battlefield);
    let blank = g.put(1, 10, Zone::Battlefield);
    cite!(["702.16g", "702.16f"]; assert!(g.cant_block(hybrid, guard, &[hybrid])));
    for def in [11, 12] {
        let mono = g.put(1, def, Zone::Battlefield);
        cite!(["702.16g", "702.16f"]; case: format!("def {def}"); assert!(g.cant_block(mono, guard, &[mono])));
        g.damage(mono, Target::Object(guard), 1);
        cite!(["702.16g", "702.16e"]; case: format!("def {def}"); assert_eq!(g.obj(guard).damage, 0));
    }
    cite!(["702.16f"]; assert!(!g.cant_block(blank, guard, &[blank])));
    g.damage(hybrid, Target::Object(guard), 1);
    assert_eq!(g.obj(guard).damage, 0);
}

#[test]
fn r702_16a_unimplemented_protection_qualities_stay_refused() {
    for text in [
        "Protection from the chosen color",
        "Protection from the chosen player",
        // "or" is a choice made as an effect resolves, which the engine does not run
        "Protection from artifacts or from the color of your choice",
    ] {
        let card = CardDef::new("Unsupported", "{W}", "Creature — Spirit", text, Some((2, 2))).unwrap();
        let reason=if text=="Protection from the chosen color" {"linked choice: no ability identity"}else{"keyword: Protection"};
        crate::support::refused(game::play::check(&card).map_err(|e|e.0),reason);
    }
}

#[test]
fn r702_19b_trample_assigns_lethal_before_prevention_is_applied() {
    let mut g = game();
    std::sync::Arc::make_mut(&mut g.defs).push(def("Trampler", "{R}", "Creature — Beast", "Trample", Some((6, 6))));
    let attacker = g.put(0, 8, Zone::Battlefield);
    let guard = g.put(1, 0, Zone::Battlefield);
    resolve(&mut g);
    resolve(&mut g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    resolve(&mut g);
    g.act(1, Move::Block(vec![(guard, attacker)])).unwrap();
    resolve(&mut g);
    cite!(["702.19b"]; refused("Trampler assigns damage to what it attacks once Guard is assigned lethal damage (702.19b); the assignment begins again (510.1e)"): g.act(0, Move::Divide(vec![(Target::Object(guard), 1), (Target::Player(1), 5)])));
    cite!(["702.19b"]; ok: g.act(0, Move::Divide(vec![(Target::Object(guard), 2), (Target::Player(1), 4)])));
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 16));
    assert_eq!(g.obj(guard).zone, Zone::Battlefield);
    cite!(["702.16e"]; assert_eq!(g.obj(guard).damage, 0));
    assert_eq!(g.obj(attacker).damage, 2);
}

#[test]
fn r113_6_protection_does_not_protect_creature_spells_or_graveyard_cards() {
    let mut g = game();
    std::sync::Arc::make_mut(&mut g.defs).extend([
        def("Denial", "{R}", "Instant", "Counter target spell.", None),
        def(
            "Restore",
            "{R}",
            "Sorcery",
            "Put target creature card from a graveyard onto the battlefield under your control.",
            None,
        ),
    ]);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors = [10; 5];
    g.act(0, paid("W", Move::Cast { card, targets: vec![] })).unwrap();
    let spell = g.stack[0];
    cast(&mut g, 8, Some(spell), "R");
    let dead = g.players[0].graveyard.iter().copied().find(|o| g.obj(*o).chars.name == "Guard").unwrap();
    cast(&mut g, 9, Some(dead), "R");
    cite!(["113.6"]; assert_eq!(g.battlefield.len(), 1));
    let red = g.put(1, 1, Zone::Battlefield);
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(1, red), Target::Object(g.battlefield[0]))));
}

#[test]
fn r702_16d_equipment_can_grant_parameterized_protection_until_it_is_unattached() {
    let mut g = game();
    std::sync::Arc::make_mut(&mut g.defs).push(def(
        "Ward",
        "{U}",
        "Artifact — Equipment",
        "Equipped creature has protection from red.\nEquip {0}",
        None,
    ));
    let bear = g.put(0, 2, Zone::Battlefield);
    let ward = g.put(0, 8, Zone::Battlefield);
    let red = g.put(1, 1, Zone::Battlefield);
    assert!(g.attach(ward, Target::Object(bear)));
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(1, red), Target::Object(bear))));
    g.exile(ward);
    cite!(["611.3b"]; assert!(g.can_target(&game::select::Ctx::new(1, red), Target::Object(bear))));
}

#[test]
fn r702_16h_protection_from_each_color_does_not_include_colorless() {
    let mut g = game();
    std::sync::Arc::make_mut(&mut g.defs).push(def(
        "Ward",
        "{W}",
        "Creature — Spirit",
        "Protection from each color",
        Some((2, 2)),
    ));
    let guard = g.put(0, 8, Zone::Battlefield);
    for (i, cost) in ["{W}", "{U}", "{B}", "{R}", "{G}", "{0}"].iter().enumerate() {
        let index = g.defs.len();
        std::sync::Arc::make_mut(&mut g.defs).push(def("Source", cost, "Creature — Spirit", "", Some((1, 1))));
        let source = g.put(1, index, Zone::Battlefield);
        cite!(["702.16h"]; case: cost; assert_eq!(g.can_target(&game::select::Ctx::new(1, source), Target::Object(guard)), i == 5));
    }
}
