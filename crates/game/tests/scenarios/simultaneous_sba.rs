//! State-based actions inspect one state, act together, then inspect again (704.3).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};

fn game(lord_text: &str) -> Game {
    let defs = vec![
        CardDef::new("Leader", "{0}", "Creature", lord_text, Some((2, 2))).unwrap(),
        CardDef::new("Follower", "{0}", "Creature", "", Some((1, 1))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 3, 20)
}
#[test]
fn r704_3_losing_a_toughness_bonus_is_a_later_state_based_action_batch() {
    for reversed in [false, true] {
        let mut g = game("Other creatures you control get +1/+1.\nWhenever another creature dies, you gain 1 life.");
        let (leader, follower) = if reversed {
            let follower = g.put(0, 1, Zone::Battlefield);
            (g.put(0, 0, Zone::Battlefield), follower)
        } else {
            (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield))
        };
        g.damage(leader, Target::Object(leader), 2);
        g.damage(leader, Target::Object(follower), 1);
        assert!(g.state_based_actions());
        assert_eq!((g.obj(leader).zone, g.obj(follower).zone), (Zone::Gone, Zone::Gone));
        g.act(0, Move::Pass).unwrap();
        cite!(["704.3", "603.10a"]; case: reversed; assert!(g.stack.is_empty(), "the leader is gone before the follower dies; insertion order {reversed}"));
    }
}
#[test]
fn r603_10a_simultaneous_deaths_keep_characteristics_from_before_any_removal() {
    for reversed in [false, true] {
        let mut g = game("Other creatures you control get +1/+1.");
        let (leader, follower) = if reversed {
            let follower = g.put(0, 1, Zone::Battlefield);
            (g.put(0, 0, Zone::Battlefield), follower)
        } else {
            (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield))
        };
        g.damage(leader, Target::Object(leader), 2);
        g.damage(leader, Target::Object(follower), 2);
        g.state_based_actions();
        cite!(["704.3", "603.10"]; case: reversed; assert_eq!(
            g.power(follower),
            Some(2),
            "last known power includes the leader's bonus; insertion order {reversed}"
        ));
    }
}
#[test]
fn r704_3_losing_indestructible_does_not_merge_two_death_batches() {
    let mut g =
        game("Other creatures you control have indestructible.\nWhenever another creature dies, you gain 1 life.");
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    g.damage(leader, Target::Object(leader), 2);
    g.damage(leader, Target::Object(follower), 1);
    g.state_based_actions();
    assert_eq!((g.obj(leader).zone, g.obj(follower).zone), (Zone::Gone, Zone::Gone));
    g.act(0, Move::Pass).unwrap();
    cite!(["704.3", "603.10a"]; assert!(g.stack.is_empty(), "the leader cannot see the later loss of the formerly indestructible follower"));
}

#[test]
fn r603_10a_a_leader_sees_a_follower_that_really_dies_in_the_same_batch() {
    for reversed in [false, true] {
        let mut g = game("Other creatures you control get +1/+1.\nWhenever another creature dies, you gain 1 life.");
        let (leader, follower) = if reversed {
            let follower = g.put(0, 1, Zone::Battlefield);
            (g.put(0, 0, Zone::Battlefield), follower)
        } else {
            (g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield))
        };
        g.damage(leader, Target::Object(leader), 2);
        g.damage(leader, Target::Object(follower), 2);
        g.state_based_actions();
        cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![leader,follower]))));
        g.act(0, Move::Pass).unwrap();
        cite!(["603.10a"]; case: reversed; assert_eq!(g.stack.len(), 1));
        for _ in 0..3 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        assert_eq!(g.players[0].life, 21);
    }
}
#[test]
fn r603_10a_a_power_qualified_death_trigger_uses_the_simultaneous_pre_event_power() {
    let mut g = game("Other creatures you control get +2/+2.");
    let watcher =
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a creature with power 3 or greater dies, you gain 1 life.", None).unwrap();
    game::play::check(&watcher).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(watcher);
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    g.damage(leader, Target::Object(leader), 2);
    g.damage(leader, Target::Object(follower), 3);
    g.state_based_actions();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![leader,follower]))));
    g.act(0, Move::Pass).unwrap();
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "the follower had power 3 when it died"));
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r704_3_regeneration_counts_as_a_performed_state_based_action() {
    let mut g = game("Other creatures you control get +1/+1.");
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 1, Zone::Battlefield);
    g.regenerate(leader);
    g.damage(leader, Target::Object(leader), 2);
    g.damage(leader, Target::Object(follower), 1);
    cite!(["704.3"]; assert!(g.state_based_actions()));
    cite!(["701.19a"]; assert_eq!(g.battlefield, vec![leader, follower]));
    assert!(g.obj(leader).tapped);
    cite!(["701.19a"]; assert_eq!(g.obj(leader).damage, 0));
    assert_eq!(g.toughness(follower), Some(2));
    cite!(["704.3"]; assert!(!g.state_based_actions(), "a stable board does not repeat a replaced destruction"));
}
#[test]
fn r704_5h_indestructibility_loss_does_not_reuse_deathtouch_from_an_earlier_check() {
    let mut g = game("Other creatures you control have indestructible.");
    let large = CardDef::new("Large", "{0}", "Creature", "", Some((3, 3))).unwrap();
    let snake = CardDef::new("Snake", "{0}", "Creature", "Deathtouch", Some((1, 1))).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).extend([large, snake]);
    let leader = g.put(0, 0, Zone::Battlefield);
    let follower = g.put(0, 2, Zone::Battlefield);
    let source = g.put(1, 3, Zone::Battlefield);
    g.damage(leader, Target::Object(leader), 2);
    g.damage(source, Target::Object(follower), 1);
    g.state_based_actions();
    assert_eq!(g.obj(leader).zone, Zone::Gone);
    cite!(["704.5h"]; assert_eq!(g.obj(follower).zone, Zone::Battlefield));
    assert!(!g.has(follower, "Indestructible"));
    cite!(["704.5h"]; assert_eq!(g.obj(follower).damage, 1));
}
#[test]
fn r704_3_diregraf_captain_does_not_see_a_zombie_die_after_its_bonus_ends() {
    let defs = vec![
        CardDef::new("Diregraf Captain", "{1}{U}{B}", "Creature — Zombie Soldier", "Deathtouch\nOther Zombie creatures you control get +1/+1.\nWhenever another Zombie you control dies, target opponent loses 1 life.", Some((2, 2))).unwrap(),
        CardDef::new("Walking Corpse", "{1}{B}", "Creature — Zombie", "", Some((2, 2))).unwrap(),
        CardDef::new("Pyroclasm", "{1}{R}", "Sorcery", "Pyroclasm deals 2 damage to each creature.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 3, 20);
    g.put(1, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    let card = g.put(0, 2, Zone::Hand);
    g.act(0, paid("RR", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert!(g.battlefield.is_empty());
    assert!(g.decision.is_none());
    cite!(["704.3", "603.10a"]; assert!(g.stack.is_empty()));
    cite!(["704.3"]; assert!(g.players.iter().all(|p| p.life == 20)));
}
