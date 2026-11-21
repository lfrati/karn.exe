//! A distributive "each … deals damage equal to its power"
//! reads each source's own power (608.2c), never the recipient's. Alpha Brawl and Band
//! Together are tested separately in `other_actor_probe` and `other_target_actor`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e {
        Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
        _ => None,
    }).collect()
}

/// "its" is each dealing creature, not the 1-power target: bound to the target, each would deal 1.
#[test]
fn each_creature_you_control_deals_its_own_power() {
    let spell = CardDef::new("Probe", "{0}", "Sorcery", "Each creature you control deals damage equal to its power to target creature.", None).unwrap();
    game::play::check(&spell).expect("admitted");
    let big = CardDef::new("Big", "{0}", "Creature — Beast", "", Some((3, 5))).unwrap();
    let small = CardDef::new("Small", "{0}", "Creature — Beast", "", Some((2, 2))).unwrap();
    for power in [1, 0] {
        let wall = CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((power, 20))).unwrap();
        let mut g = Game::new(vec![spell.clone(), big.clone(), small.clone(), wall], 2, 20);
        let a = g.put(0, 1, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        let w = g.put(1, 3, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(w)]] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let mut got = damage(&g);
        got.sort_by_key(|d| d.0);
        // each dealing creature is its damage's source (120.1) and its quantity is read as the effect applies (608.2h); "its/their" naming each dealer is the card text's English
        cite!(["120.1", "608.2h"]; assert_eq!(got, vec![(a, Target::Object(w), 3), (b, Target::Object(w), 2)]));
    }
}

/// Current boundary: the per-source object actor combined with a per-recipient player actor
/// is unreachable, so no two-overlay composition is needed.
#[test]
fn each_creature_to_each_opponent_stays_refused() {

    for recipient in ["each opponent", "target player", "any target"] {
        let text = format!("Each creature you control deals damage equal to its power to {recipient}.");
        let spell = CardDef::new("Probe", "{0}", "Sorcery", &text, None).unwrap();
        crate::support::refused(game::play::check(&spell).map_err(|e|e.0),"per-source object quantity in damage a player may be dealt");
    }
}

fn creature(name: &str, power: i32, toughness: i32) -> CardDef {
    CardDef::new(name, "{0}", "Creature — Beast", "", Some((power, toughness))).unwrap()
}
fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn resolve(g: &mut Game) {
    for _ in 0..8 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not finish");
}

/// 608.2b: legal Band sources act independently; an illegal recipient never receives damage.
#[test]
fn band_together_legality_and_zero_or_one_source() {
    for (count, invalid_a, invalid_b, invalid_recipient) in [
        (0, false, false, false), (1, false, false, false),
        (2, true, false, false), (2, false, true, false),
        (2, true, true, false), (2, false, false, true),
    ] {
        let mut g = Game::new(vec![pinned("Band Together"), creature("Big", 3, 5),
            creature("Small", 2, 2), creature("Wall", 1, 20)], 2, 20);
        let a = g.put(0, 1, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        let w = g.put(1, 3, Zone::Battlefield);
        let sources = [a, b][..count].iter().copied().map(Target::Object).collect();
        g.players[0].pool.colors = [10; 5];
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid("WWG", Move::Cast { card: c, targets: vec![sources, vec![Target::Object(w)]] })).unwrap();
        if invalid_a { g.move_object(a, Zone::Graveyard); }
        if invalid_b { g.move_object(b, Zone::Graveyard); }
        if invalid_recipient { g.move_object(w, Zone::Graveyard); }
        resolve(&mut g);
        let mut expected = Vec::new();
        if !invalid_recipient {
            if count > 0 && !invalid_a { expected.push((a, Target::Object(w), 3)); }
            if count > 1 && !invalid_b { expected.push((b, Target::Object(w), 2)); }
        }
        cite!(["608.2b"]; assert_eq!(damage(&g), expected, "{count} {invalid_a} {invalid_b} {invalid_recipient}"));
    }
}

/// 109.5 / 608.2h: distributive source possessors do not consume explicit target/group reads.
#[test]
fn explicit_target_and_group_total_remain_independent() {
    for subject in ["Each creature you control deals", "Creatures you control deal", "All creatures you control deal"] {
    for (amount, expected) in [("the power of that creature", 1),
        ("the total power of creatures you control", 5)] {
        let text = format!("{subject} damage equal to {amount} to target creature.");
        let spell = CardDef::new("Probe", "{0}", "Sorcery", &text, None).unwrap();
        game::play::check(&spell).unwrap();
        let mut g = Game::new(vec![spell, creature("Big", 3, 5), creature("Small", 2, 2),
            creature("Wall", 1, 20)], 2, 20);
        let a = g.put(0, 1, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        let w = g.put(1, 3, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(w)]] }).unwrap();
        resolve(&mut g);
        cite!(["120.1", "608.2h"]; assert_eq!(damage(&g), vec![(a, Target::Object(w), expected), (b, Target::Object(w), expected)]));
    }
    }
}

/// No mid-resolution SBA: Alpha's lethally marked source still deals its own damage.
#[test]
fn alpha_brawl_group_and_lethal_sources_survive_until_resolution_ends() {
    let mut g = Game::new(vec![pinned("Alpha Brawl"), creature("Boss", 4, 9),
        creature("Bear", 2, 2), creature("Wall", 1, 5)], 2, 20);
    let boss = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    let wall = g.put(1, 3, Zone::Battlefield);
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("WWWWWWRR", Move::Cast { card: c, targets: vec![vec![Target::Object(boss)]] })).unwrap();
    let mut fork = g.clone();
    for branch in [&mut g, &mut fork] {
        resolve(branch);
        cite!(["704.4"]; assert_eq!(damage(branch), vec![(boss, Target::Object(bear), 4),
            (boss, Target::Object(wall), 4), (bear, Target::Object(boss), 2),
            (wall, Target::Object(boss), 1)]));
        assert_eq!(branch.obj(boss).damage, 3);
        cite!(["120.3e"]; assert_eq!(branch.obj(wall).damage, 4));
        cite!(["704.5g"]; assert!(!branch.battlefield.contains(&bear), "lethal source dies after it deals damage"));
        assert!(branch.battlefield.contains(&boss));
    }
}
