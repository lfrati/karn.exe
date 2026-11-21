//! Independent event-subject and counted-group checks.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::state::Decision;
use ir::Part;

fn resolve(g: &mut Game) {
    for _ in 0..30 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("stack did not empty");
}

fn declare(g: &mut Game, attackers: &[game::ObjId]) {
    for _ in 0..30 {
        if g.part == Part::DeclareAttackers && g.decision.is_some() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}

#[test]
fn r508_3a_other_attackers_are_other_than_the_trigger_subject() {
    // Rules: 508.1m, 508.3a, 603.2c
    // The watcher is a creature and is one of the three attackers. For each subject,
    // "two other creatures" excludes that subject, not the watcher that owns the text.
    let watcher = CardDef::new("Watcher", "{0}", "Creature — Human",
        "Whenever a creature you control and at least two other creatures attack, you gain 1 life.", Some((1, 4))).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![watcher, bear], 2, 20);
    let w = g.put(0, 0, Zone::Battlefield);
    let a = g.put(0, 1, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    declare(&mut g, &[w, a, b]);
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 3)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(w)));
    g.act(0, Move::Order(vec![0, 1, 2])).unwrap();
    resolve(&mut g);
    cite!(["508.3a", "603.2c"]; assert_eq!(g.players[0].life, 23, "each of the three declared creatures attacked with two others"));
}

#[test]
fn r508_3d_player_attack_count_other_still_excludes_the_ability_source() {
    // Rules: 508.3d, 603.2c
    let watcher = CardDef::new("Watcher", "{0}", "Creature — Human",
        "Whenever you attack with two or more other creatures, you gain 1 life.", Some((1, 4))).unwrap();
    game::play::check(&watcher).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for count in [1, 2] {
        let mut g = Game::new(vec![watcher.clone(), bear.clone()], 2, 20);
        let w = g.put(0, 0, Zone::Battlefield);
        let mut attackers = vec![w];
        attackers.extend((0..count).map(|_| g.put(0, 1, Zone::Battlefield)));
        declare(&mut g, &attackers);
        resolve(&mut g);
        cite!(["508.3d"]; case: format!("{count} others"); assert_eq!(g.players[0].life, if count == 2 { 21 } else { 20 }));
    }
}

#[test]
fn r608_2k_a_counted_plural_group_does_not_supply_a_singular_it() {
    // Rules: 508.3d, 608.2k
    let d = CardDef::new("Unbound singular", "{0}", "Enchantment",
        "Whenever you attack with two or more creatures, it gets +1/+1 until end of turn.", None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");
}

#[test]
fn r506_5_an_attacker_removed_from_combat_before_capture_does_not_make_an_alone_attack() {
    // Rules: 506.4, 506.5, 508.1k-m
    let vanisher = CardDef::new("Vanisher", "{0}", "Creature — Human",
        "As long as this creature is attacking, it's an enchantment.", Some((1, 1))).unwrap();
    game::play::check(&vanisher).unwrap();
    let kavu = CardDef::new("Kavu", "{0}", "Creature — Kavu",
        "Whenever this creature attacks alone, it gets +2/+0 until end of turn.", Some((1, 1))).unwrap();
    game::play::check(&kavu).unwrap();
    let mut g = Game::new(vec![vanisher, kavu], 2, 20);
    let v = g.put(0, 0, Zone::Battlefield);
    let k = g.put(0, 1, Zone::Battlefield);
    declare(&mut g, &[v, k]);
    resolve(&mut g);
    cite!(["506.5"]; assert_eq!(g.power(k), Some(1), "two creatures were declared, even if one stops being a creature"));
}

#[test]
fn r508_3d_a_declared_creature_removed_from_combat_still_counts_for_the_declaration() {
    // Rules: 506.4, 508.1k-m, 508.3c-d
    let vanisher = CardDef::new("Vanisher", "{0}", "Creature — Human",
        "As long as this creature is attacking, it's an enchantment.", Some((1, 1))).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let watcher = CardDef::new("Watcher", "{0}", "Enchantment",
        "Whenever you attack with two or more creatures, you gain 1 life.", None).unwrap();
    for d in [&vanisher, &bear, &watcher] { game::play::check(d).unwrap(); }
    let mut g = Game::new(vec![vanisher, bear, watcher], 2, 20);
    let v = g.put(0, 0, Zone::Battlefield);
    let b = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    declare(&mut g, &[v, b]);
    resolve(&mut g);
    cite!(["508.3d"]; assert_eq!(g.players[0].life, 21, "two creatures were declared as attackers"));
}
