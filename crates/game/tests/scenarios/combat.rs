//! Combat (CR 506–511), rule by rule: who can attack and block and what can be attacked, the
//! declarations, combat damage, and the abilities that trigger on them.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const GIANT: usize = 2;
const ONES: usize = 3;
const HATCHLING: usize = 4;
const BOLT: usize = 5;
const CONQUISTADOR: usize = 6;
const TANTIV: usize = 7;
const THIEF: usize = 8;
const CAUSE: usize = 9;
const FOREST: usize = 10;
const HUNTERS: usize = 11;
const COLOSSAPEDE: usize = 12;
const SPHERE: usize = 13;
const CIRCLE: usize = 14;
const CROW: usize = 15;
const WAR_TORCH: usize = 16;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Wandering Ones", "{U}", "Creature — Spirit", "", Some((1, 1))),
        card("Kraken Hatchling", "{U}", "Creature — Kraken", "", Some((0, 4))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card(
            "Vicious Conquistador",
            "{B}",
            "Creature — Vampire Soldier",
            "Whenever this creature attacks, each opponent loses 1 life.",
            Some((1, 2)),
        ),
        card("Deepwood Tantiv", "{4}{G}", "Creature — Beast", "Whenever this creature becomes blocked, you gain 2 life.", Some((2, 4))),
        card(
            "Scroll Thief",
            "{2}{U}",
            "Creature — Merfolk Rogue",
            "Whenever this creature deals combat damage to a player, draw a card.",
            Some((1, 3)),
        ),
        card("Righteous Cause", "{3}{W}{W}", "Enchantment", "Whenever a creature attacks, you gain 1 life.", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card(
            "Sun-Crowned Hunters",
            "{4}{R}{R}",
            "Creature — Dinosaur",
            "Enrage — Whenever this creature is dealt damage, it deals 3 damage to target opponent or planeswalker.",
            Some((5, 4)),
        ),
        card("Colossapede", "{4}{G}", "Creature — Insect", "", Some((5, 5))),
        card("Shield Sphere", "{0}", "Artifact Creature — Wall", "Defender\nWhenever this creature blocks, put a -0/-1 counter on it.", Some((0, 6))),
        card(
            "Circle of Flame",
            "{1}{R}",
            "Enchantment",
            "Whenever a creature without flying attacks you or a planeswalker you control, this enchantment deals 1 damage to that creature.",
            None,
        ),
        card("Storm Crow", "{1}{U}", "Creature — Bird", "Flying", Some((1, 2))),
        card(
            "War-Torch Goblin",
            "{R}",
            "Creature — Goblin Warrior",
            "{R}, Sacrifice this creature: It deals 2 damage to target blocking creature.",
            Some((1, 1)),
        ),
    ]
}

/// A two-player game set up in player 0's first precombat main phase.
fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn object(o: ObjId) -> Target {
    Target::Object(o)
}

/// All players pass in succession once.
fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Pass from the precombat main phase to the declaration of attackers.
fn to_attackers(g: &mut Game) {
    all_pass(g);
    assert_eq!(g.part, Part::BeginningOfCombat);
    all_pass(g);
    assert_eq!(g.part, Part::DeclareAttackers);
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r508_1a_untapped_creatures_the_active_player_controls_attack() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.tap(tapped);
    to_attackers(&mut g);
    cite!(["508.1a"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 })));
    // attackers are declared before anyone has priority
    cite!(["508.1"]; refused("a decision is pending: declare attackers (508.1)"): g.act(0, Move::Pass));
    let attack = |g: &mut Game, o: ObjId| g.act(0, Move::Attack(vec![(o, Target::Player(1))]));
    cite!(["508.1a"]; refused("a tapped creature can't be declared as an attacker (508.1a)"): attack(&mut g, tapped));
    cite!(["508.1a"]; refused("only creatures the active player controls can attack (508.1a)"): attack(&mut g, theirs));
    cite!(["506.3"]; refused("only a creature can attack (506.3)"): attack(&mut g, land));
    // the active player declares
    cite!(["508.1"]; refused("that decision is another player's"): g.act(1, Move::Attack(vec![])));
    // an illegal declaration changes nothing (508.1)
    cite!(["508.1"]; assert!(g.combat.attackers.is_empty() && !g.obj(bears).tapped));
    attack(&mut g, bears).unwrap();
    cite!(["508.1f"]; assert!(g.obj(bears).tapped, "attacking creatures become tapped (508.1f)"));
    cite!(["508.1a"]; assert_eq!(g.combat.attackers, vec![(bears, Target::Player(1))]));
    cite!(["508.2"]; assert_eq!(g.priority, 0, "then the active player gets priority (508.2)"));
    cite!(["508.1a"]; assert!(g.log.contains(&Event::Attacked { attacker: bears, target: Target::Player(1) })));
}

#[test]
fn r506_3_only_a_player_or_a_planeswalker_or_battle_is_attacked() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    cite!(["506.3"]; refused("only a player, a planeswalker or a battle can be attacked (506.3)"): g.act(0, Move::Attack(vec![(bears, object(theirs))])));
    cite!(["506.2"]; refused("a creature attacks a defending player or their planeswalker (506.2)"): g.act(0, Move::Attack(vec![(bears, Target::Player(0))])));
    cite!(["506.2"]; ok: g.act(0, Move::Attack(vec![(bears, Target::Player(1))])));
}

#[test]
fn r302_6_a_creature_attacks_only_if_controlled_since_the_turn_began() {
    let mut g = game();
    let (forest, mountain) = (g.put(0, FOREST, Zone::Battlefield), g.put(0, FOREST, Zone::Battlefield));
    let card = g.put(0, BEARS, Zone::Hand);
    for _ in 0..12 {
        g.put(0, MOUNTAIN, Zone::Library);
        g.put(1, MOUNTAIN, Zone::Library);
    }
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("GG", Move::Cast { card, targets: vec![] })).unwrap();
    all_pass(&mut g);
    let bears = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(bears).chars.name, "Grizzly Bears");
    cite!(["302.6"]; refused("that creature has not been under your control since your turn began (302.6)"): g.can_attack(bears));
    // with no creature able to attack, nobody is asked: the declaration is empty
    to_attackers(&mut g);
    cite!(["302.6"]; assert_eq!(g.decision, None));
    cite!(["302.6"]; assert!(g.combat.attackers.is_empty()));
    // in that player's next turn it has been theirs since the turn began
    while !(g.turn == 3 && g.part == Part::DeclareAttackers) {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["302.6"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 })));
    cite!(["302.6"]; ok: g.act(0, Move::Attack(vec![(bears, Target::Player(1))])));
}

#[test]
fn r508_8_with_no_attackers_the_declare_blockers_and_combat_damage_steps_are_skipped() {
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![])).unwrap();
    cite!(["508.2"]; assert_eq!(g.part, Part::DeclareAttackers, "the step still happens and gives priority (508.2)"));
    all_pass(&mut g);
    cite!(["508.8"]; assert_eq!(g.part, Part::EndOfCombat));
}

#[test]
fn r510_1b_an_unblocked_creature_deals_combat_damage_to_the_player_it_attacks() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    // the defending player has no creature that could block: nobody is asked (509.1)
    cite!(["509.1"]; assert_eq!((g.part, g.decision.clone()), (Part::DeclareBlockers, None)));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["510.1b"]; assert_eq!(g.players[1].life, 18));
    cite!(["510.1b"]; assert!(g.log.contains(&Event::DamageDealt { source: bears, to: Target::Player(1), amount: 2, combat: true })));
    cite!(["510.3"]; assert_eq!(g.priority, 0, "then the active player gets priority (510.3)"));
}

#[test]
fn r509_1a_the_defending_player_blocks_with_untapped_creatures_they_control() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let idle = g.put(0, GIANT, Zone::Battlefield);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    let tapped = g.put(1, BEARS, Zone::Battlefield);
    g.tap(tapped);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    cite!(["509.1a"]; assert_eq!(g.decision, Some(Decision::Blockers { player: 1 })));
    // the defending player declares
    cite!(["509.1"]; refused("that decision is another player's"): g.act(0, Move::Block(vec![])));
    cite!(["509.1a"]; refused("a tapped creature can't be declared as a blocker (509.1a)"): g.act(1, Move::Block(vec![(tapped, attacker)])));
    cite!(["509.1a"]; refused("a player declares only creatures they control as blockers (509.1a)"): g.act(1, Move::Block(vec![(idle, attacker)])));
    // it blocks an attacking creature
    cite!(["509.1a"]; refused("a creature blocks a creature that is attacking its controller or their planeswalker (509.1a)"): g.act(1, Move::Block(vec![(blocker, idle)])));
    g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
    cite!(["509.1a"]; assert_eq!(g.combat.blockers, vec![(blocker, attacker)]));
    cite!(["509.1h"]; assert_eq!(g.combat.blocked, vec![attacker], "an attacker with a blocker declared for it is blocked (509.1h)"));
    cite!(["509.2"]; assert_eq!(g.priority, 0, "then the active player gets priority (509.2)"));
}

#[test]
fn r510_2_combat_damage_is_dealt_simultaneously() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    // each dealt the other 2: both have lethal damage and are destroyed together (704.5g)
    cite!(["510.1c", "704.5g"]; assert_eq!(g.players[1].life, 20, "a blocked creature deals no damage to the player (510.1c)"));
    cite!(["510.2"]; assert!(g.battlefield.is_empty()));
    cite!(["510.2"]; assert_eq!((g.players[0].graveyard.len(), g.players[1].graveyard.len()), (1, 1)));
}

#[test]
fn r510_1a_a_creature_with_no_power_assigns_no_combat_damage() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, HATCHLING, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
    all_pass(&mut g);
    cite!(["510.1a"]; assert_eq!(g.obj(attacker).damage, 0));
    cite!(["510.1a"]; assert_eq!(g.obj(blocker).damage, 2));
    cite!(["510.1a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 1));
}

#[test]
fn r510_1c_a_creature_blocked_by_several_divides_its_damage_as_its_controller_chooses() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let (a, b) = (g.put(1, ONES, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(giant, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(a, giant), (b, giant)])).unwrap();
    all_pass(&mut g);
    let (to_a, to_b) = (object(a), object(b));
    cite!(["510.1c"]; assert_eq!(g.decision, Some(Decision::Divide { player: 0, attacker: giant, among: vec![to_a, to_b], amount: 3 })));
    // all of it is assigned
    cite!(["510.1a"]; refused("it assigns combat damage equal to its power, 3 (510.1a)"): g.act(0, Move::Divide(vec![(to_a, 1), (to_b, 1)])));
    // to the creatures blocking it
    cite!(["510.1c"]; refused("that damage is divided only among its combat recipients (510.1c–d)"): g.act(0, Move::Divide(vec![(object(giant), 3)])));
    // not to the player it attacks: it has no trample (702.19b)
    cite!(["510.1c"]; refused("that damage is divided only among its combat recipients (510.1c–d)"): g.act(0, Move::Divide(vec![(to_a, 1), (Target::Player(1), 2)])));
    g.act(0, Move::Divide(vec![(to_a, 1), (to_b, 2)])).unwrap();
    // Both blockers enter player 1's graveyard together; their owner orders the block.
    cite!(["404.3"]; ok: g.act(1, Move::Choose(game::state::Answer::Objects(vec![a, b]))));
    // the giant took 1 and 2; both blockers took lethal damage
    cite!(["510.1c"]; assert_eq!(g.battlefield, vec![], "three damage on a 3/3, and each blocker destroyed"));
}

#[test]
fn r506_4_a_creature_that_leaves_the_battlefield_is_removed_from_combat() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
    // the blocker is destroyed before damage: the attacker stays blocked (509.1h) and, with no
    // creature blocking it, assigns no combat damage (510.1c)
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![object(blocker)]] })).unwrap();
    all_pass(&mut g);
    cite!(["506.4"]; assert!(g.combat.blockers.is_empty()));
    cite!(["509.1h"]; assert_eq!(g.combat.blocked, vec![attacker]));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["510.1c"]; assert_eq!(g.players[1].life, 20));
    cite!(["506.4"]; assert_eq!(g.obj(attacker).damage, 0));
}

#[test]
fn r509_1g_a_blocking_creature_remains_one_until_it_is_removed_from_combat() {
    // "{R}, Sacrifice this creature: It deals 2 damage to target blocking creature.": the
    // creature blocking the Goblin is still a blocking creature when the Goblin, sacrificed
    // to pay, has left combat, so it is a legal target as the ability resolves (608.2b)
    let mut g = game();
    let goblin = g.put(0, WAR_TORCH, Zone::Battlefield);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(goblin, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(blocker, goblin)])).unwrap();
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Activate { source: goblin, ability: 0, targets: vec![vec![object(blocker)]], paying: vec![] })).unwrap();
    cite!(["509.1g"]; assert!(g.combat.blockers.is_empty(), "it blocks nothing now"));
    cite!(["509.1g"]; assert!(g.combat.blocking(blocker), "and is still a blocking creature"));
    all_pass(&mut g);
    cite!(["509.1g"]; assert_eq!(g.obj(blocker).damage, 2));
}

#[test]
fn r511_3_creatures_are_removed_from_combat_as_the_end_of_combat_step_ends() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    while g.part != Part::EndOfCombat {
        all_pass(&mut g);
    }
    cite!(["511.3"]; assert!(g.combat.attacking(bears), "still attacking in the end of combat step"));
    all_pass(&mut g);
    assert_eq!(g.part, Part::PostcombatMain);
    cite!(["511.3"]; assert!(!g.combat.attacking(bears)));
    assert!(g.obj(bears).tapped, "and still tapped: it untaps in its controller's untap step (502.3)");
}

#[test]
fn r508_3a_an_ability_triggers_when_its_creature_is_declared_as_an_attacker() {
    let mut g = game();
    let conquistador = g.put(0, CONQUISTADOR, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // "whenever a creature attacks": once for each creature declared, whoever controls this
    let cause = g.put(1, CAUSE, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(conquistador, Target::Player(1)), (bears, Target::Player(1))])).unwrap();
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (1, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 1 && trigger.source == Some(cause)));
    g.act(1, Move::Order(vec![0, 1])).unwrap();
    cite!(["508.2"]; assert_eq!(g.stack.len(), 3, "on the stack before the active player gets priority (508.2)"));
    for _ in 0..3 {
        all_pass(&mut g);
    }
    cite!(["508.3a"]; assert_eq!(g.players[1].life, 20 - 1 + 2));
    assert_eq!(g.part, Part::DeclareAttackers);
}

#[test]
fn r509_3c_becomes_blocked_triggers_once_however_many_creatures_block() {
    let mut g = game();
    let tantiv = g.put(0, TANTIV, Zone::Battlefield);
    let (a, b) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, ONES, Zone::Battlefield));
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(tantiv, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(a, tantiv), (b, tantiv)])).unwrap();
    cite!(["509.3c"]; assert_eq!(g.stack.len(), 1));
    all_pass(&mut g);
    cite!(["509.3c"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn r508_3a_whenever_a_creature_attacks_a_player_or_a_planeswalker_they_control() {
    // "Whenever a creature without flying attacks you or a planeswalker you control, this
    // enchantment deals 1 damage to that creature.": either of the two, for each such creature
    let mut g = game();
    g.put(1, CIRCLE, Zone::Battlefield);
    g.put(0, CIRCLE, Zone::Battlefield);
    let (ones, crow) = (g.put(0, ONES, Zone::Battlefield), g.put(0, CROW, Zone::Battlefield));
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(ones, Target::Player(1)), (crow, Target::Player(1))])).unwrap();
    cite!(["508.3a"]; assert_eq!(g.stack.len(), 1, "the defending player's Circle, for the creature without flying"));
    cite!(["508.3a"]; assert_eq!(g.obj(g.stack[0]).controller, 1));
    all_pass(&mut g);
    cite!(["508.3a"]; assert_eq!((g.obj(ones).zone, g.obj(crow).zone), (Zone::Gone, Zone::Battlefield), "1 damage to the 1/1"));
}

#[test]
fn r113_7a_an_ability_that_triggered_exists_whatever_becomes_of_its_source() {
    // "Whenever this creature is dealt damage, it deals 3 damage to target opponent": dealt
    // lethal combat damage, it has triggered (603.2) before the state-based action destroys
    // it, and its ability goes on the stack and resolves without it
    let mut g = game();
    let hunters = g.put(0, HUNTERS, Zone::Battlefield);
    let colossapede = g.put(1, COLOSSAPEDE, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hunters, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(colossapede, hunters)])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["113.7a"]; assert_eq!((g.obj(hunters).zone, g.obj(colossapede).zone), (Zone::Gone, Zone::Gone), "5 damage each: both destroyed"));
    // its controller chooses the target as it goes on the stack (603.3d)
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    all_pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[1].life, 17));
}

#[test]
fn r122_1a_a_counter_that_subtracts_from_toughness_alone() {
    // "Whenever this creature blocks, put a -0/-1 counter on it."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let sphere = g.put(1, SPHERE, Zone::Battlefield);
    assert_eq!((g.power(sphere), g.toughness(sphere)), (Some(0), Some(6)));
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(sphere, bears)])).unwrap();
    all_pass(&mut g);
    cite!(["122.1a"]; assert_eq!(g.obj(sphere).counters.get("-0/-1"), Some(&1)));
    cite!(["122.1a"]; assert_eq!((g.power(sphere), g.toughness(sphere)), (Some(0), Some(5)), "X from power, Y from toughness"));
}

#[test]
fn r510_3a_an_ability_triggers_on_combat_damage_and_not_on_other_damage() {
    let mut g = game();
    let thief = g.put(0, THIEF, Zone::Battlefield);
    g.put(0, MOUNTAIN, Zone::Library);
    g.put(0, MOUNTAIN, Zone::Library);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(thief, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["510.3a"]; assert_eq!(g.players[1].life, 19));
    cite!(["510.3a"]; assert_eq!(g.stack.len(), 1, "it triggered on the damage being dealt (510.3a)"));
    all_pass(&mut g);
    cite!(["510.3a"]; assert_eq!(g.players[0].hand.len(), 1));
    // damage that is not combat damage, from the same creature, is not what its text names
    g.damage(thief, Target::Player(1), 1);
    all_pass(&mut g);
    cite!(["510.3a"]; assert!(g.stack.is_empty(), "noncombat damage must not leave a draw trigger pending"));
    cite!(["510.3a"]; assert_eq!(g.players[0].hand.len(), 1));
    cite!(["510.3a"]; assert_eq!(g.players[0].library.len(), 1));
}
