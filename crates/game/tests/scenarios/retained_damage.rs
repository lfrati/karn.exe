//! Retained damage results (120.4), Wither/LKI (702.80), and trigger timing (120.4b).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};

fn creature(name: &str, text: &str, pt: (i32, i32)) -> CardDef {
    CardDef::new(name, "{0}", "Creature", text, Some(pt)).unwrap()
}
fn make(defs: Vec<CardDef>) -> Game {
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..40 {
        if g.stack.is_empty() { return; }
        assert!(g.decision.is_none(), "{:?}", g.decision);
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not finish")
}

#[test]
fn r702_80_full_ram_gang_deals_counters_to_creatures_and_life_loss_to_players() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let ram = CardDef::from_faces(catalog.get("Boggart Ram-Gang").unwrap()).unwrap();
    let mut g = make(vec![ram, creature("Wall", "", (0, 10))]);
    let ram = g.put(0, 0, Zone::Battlefield);
    let wall = g.put(1, 1, Zone::Battlefield);
    g.damage(ram, wall.into(), 3);
    assert_eq!(g.obj(wall).damage, 0);
    cite!(["702.80a"]; assert_eq!(g.counters(wall).get("-1/-1"), Some(&3)));
    g.damage(ram, Target::Player(1), 3);
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 17));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 2);
}

#[test]
fn r702_80_sacrificed_source_uses_wither_and_lifelink_lki() {
    let mut g = make(vec![
        creature("Source", "Wither\nLifelink\nSacrifice this creature: This creature deals 2 damage to target creature.", (2, 2)),
        creature("Wall", "", (0, 10)),
    ]);
    let source = g.put(0, 0, Zone::Battlefield);
    let wall = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 2, targets: vec![vec![wall.into()]], paying: vec![] }).unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    resolve(&mut g);
    cite!(["702.15c"]; assert_eq!(g.players[0].life, 22));
    cite!(["702.80b"]; assert_eq!((g.obj(wall).damage, g.counters(wall).get("-1/-1")), (0, Some(&2))));
}

#[test]
fn r120_4b_damage_trigger_reads_power_before_wither_results() {
    for wither in [false, true] {
        let mut g = make(vec![
            creature("Source", if wither { "Wither" } else { "" }, (2, 2)),
            creature("Target", "", (2, 8)),
            CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a creature with power 2 or greater is dealt damage, you gain 1 life.", None).unwrap(),
        ]);
        let source = g.put(0, 0, Zone::Battlefield);
        let target = g.put(1, 1, Zone::Battlefield);
        g.put(0, 2, Zone::Battlefield);
        g.damage(source, target.into(), 2);
        g.state_based_actions();
        // The captured damage subscription survives power becoming zero.
        g.act(0, Move::Pass).unwrap();
        resolve(&mut g);
        cite!(["120.4b"]; case: format!("wither={wither}"); assert_eq!(g.players[0].life, 21, "wither={wither}"));
    }
}

#[test]
fn r120_3d_wither_still_has_deathtouch_and_both_creature_planeswalker_results() {
    let mut g = make(vec![creature("Source", "Wither\nDeathtouch", (1, 1)), CardDef::planeswalker("Walker", "{0}", "Planeswalker", "This planeswalker is a 5/10 creature in addition to its other types.", 6).unwrap()]);
    let source = g.put(0, 0, Zone::Battlefield);
    let walker = g.put(1, 1, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 6);
    g.damage(source, walker.into(), 1);
    cite!(["120.3c"]; assert_eq!(g.counters(walker).get("loyalty"), Some(&5)));
    cite!(["120.3d"]; assert_eq!(g.counters(walker).get("-1/-1"), Some(&1)));
    assert_eq!(g.obj(walker).damage, 0);
    assert!(g.obj(walker).deathtouched);
    g.state_based_actions();
    cite!(["702.2b"]; assert_eq!(g.obj(walker).zone, Zone::Gone));
}

#[test]
fn r120_4c_life_loss_subscription_reads_the_post_result_total() {
    let mut g = make(vec![creature("Source", "", (2, 2)), CardDef::new("Watcher", "{0}", "Enchantment", "Whenever you lose life, you gain life equal to your life total.", None).unwrap()]);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    g.damage(source, Target::Player(1), 2);
    g.act(0, Move::Pass).unwrap();
    resolve(&mut g);
    cite!(["120.4c", "608.2h"]; assert_eq!(g.players[1].life, 36));
}
