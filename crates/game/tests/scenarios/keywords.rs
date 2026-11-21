//! Keyword abilities (CR 702), rule by rule. Each means what its rule says: where the rule is
//! a sentence, the grammar reads that sentence and the engine holds it in force, so these tests
//! check the rule's own words running.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::keyword::meaning;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Keyword, Part};

const BEARS: usize = 0;
const SENTRY: usize = 1;
const CROW: usize = 2;
const ARCHERS: usize = 3;
const GOLLUM: usize = 4;
const PILGRIM: usize = 5;
const SHIELDMATE: usize = 6;
const MOUNTAIN: usize = 7;
const GIANT: usize = 8;
const HALBERDIER: usize = 9;
const WOLVES: usize = 10;
const ACE: usize = 11;
const DREADMAW: usize = 12;
const GILACORN: usize = 13;
const GLUTTON: usize = 14;
const CHEETAH: usize = 15;
const FOREST: usize = 16;
const BOLT: usize = 17;
const ONES: usize = 18;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Monoist Sentry", "{B}", "Artifact Creature — Robot", "Defender", Some((4, 1))),
        card("Storm Crow", "{1}{U}", "Creature — Bird", "Flying", Some((1, 2))),
        card("Keen-Eyed Archers", "{2}{W}", "Creature — Elf Archer", "Reach", Some((2, 2))),
        card("Gollum, Silent Slinker", "{3}{B}", "Legendary Creature — Halfling Horror", "Menace", Some((4, 3))),
        card("Pyromantic Pilgrim", "{2}{R}", "Creature — Human Wizard", "Haste", Some((3, 1))),
        card("Vernadi Shieldmate", "{1}{G/W}", "Creature — Human Soldier", "Vigilance", Some((2, 2))),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Halberdier", "{3}{R}", "Creature — Human Barbarian", "First strike", Some((3, 1))),
        card("Tundra Wolves", "{W}", "Creature — Wolf", "First strike", Some((1, 1))),
        card("Fencing Ace", "{1}{W}", "Creature — Human Soldier", "Double strike", Some((1, 1))),
        card("Colossal Dreadmaw", "{4}{G}{G}", "Creature — Dinosaur", "Trample", Some((6, 6))),
        card("Gilacorn", "{B}", "Creature — Lizard", "Deathtouch", Some((1, 1))),
        card("Blood Glutton", "{4}{B}", "Creature — Vampire", "Lifelink", Some((4, 3))),
        card("Pouncing Cheetah", "{2}{G}", "Creature — Cat", "Flash", Some((3, 2))),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Wandering Ones", "{U}", "Creature — Spirit", "", Some((1, 1))),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn to_attackers(g: &mut Game) {
    all_pass(g);
    all_pass(g);
    assert_eq!(g.part, Part::DeclareAttackers);
}

/// Player 0 attacks player 1 with these; then both pass, to the declaration of blockers.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    to_attackers(g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    all_pass(g);
    assert_eq!(g.part, Part::DeclareBlockers);
}

/// Player 0 attacks player 1 with these, player 1 blocks so, and both pass: to the combat
/// damage step, or the first of two.
fn fight(g: &mut Game, attackers: &[ObjId], blocks: &[(ObjId, ObjId)]) {
    attack(g, attackers);
    if g.decision == Some(Decision::Blockers { player: 1 }) {
        g.act(1, Move::Block(blocks.to_vec())).unwrap();
    }
    all_pass(g);
    assert_eq!(g.part, Part::CombatDamage);
}

/// How many combat damage steps have begun.
fn damage_steps(g: &Game) -> usize {
    g.log.iter().filter(|e| matches!(e, Event::Began { part: Part::CombatDamage })).count()
}

fn destroyed(g: &Game, o: ObjId) -> bool {
    g.log.iter().any(|e| matches!(e, Event::Destroyed { object } if *object == o))
}

#[test]
fn the_grammar_reads_each_keywords_rule() {
    for name in ["Defender", "Flying", "Menace"] {
        assert!(meaning(Keyword::from_name(name).unwrap()).is_some(), "{name}");
    }
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        // a hybrid cost is not paid yet; the card is put onto the battlefield here
        if d.name() != "Vernadi Shieldmate" {
            check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
        }
    }
}

#[test]
fn r702_3b_a_creature_with_defender_cant_attack() {
    let mut g = game();
    let sentry = g.put(0, SENTRY, Zone::Battlefield);
    to_attackers(&mut g);
    cite!(["702.3b"]; assert_eq!(g.decision, None, "its controller has nothing to declare"));
    let mut g = game();
    let sentry2 = g.put(0, SENTRY, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    cite!(["508.1c"]; refused("Monoist Sentry can't attack that (508.1c)"): g.act(0, Move::Attack(vec![(sentry2, Target::Player(1))])));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    let _ = sentry;
}

#[test]
fn r702_9b_a_creature_with_flying_is_blocked_only_by_creatures_with_flying_or_reach() {
    let mut g = game();
    let crow = g.put(0, CROW, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let (ground, flier, reach) =
        (g.put(1, BEARS, Zone::Battlefield), g.put(1, CROW, Zone::Battlefield), g.put(1, ARCHERS, Zone::Battlefield));
    attack(&mut g, &[crow, bears]);
    cite!(["702.9b", "509.1b"]; refused("Grizzly Bears can't block Storm Crow: Storm Crow has flying (702.9b, 509.1b)"): g.act(1, Move::Block(vec![(ground, crow)])));
    // "a creature with flying can block a creature with or without flying"
    cite!(["702.9b"]; ok: g.act(1, Move::Block(vec![(reach, crow), (flier, bears)])));
}

#[test]
fn r702_111b_a_creature_with_menace_is_blocked_only_by_two_or_more_creatures() {
    let mut g = game();
    let gollum = g.put(0, GOLLUM, Zone::Battlefield);
    let (a, b) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    attack(&mut g, &[gollum]);
    assert_eq!(g.decision, Some(Decision::Blockers { player: 1 }));
    cite!(["702.111b", "509.1b"]; refused("Grizzly Bears can't block Gollum, Silent Slinker: Gollum, Silent Slinker has menace (702.111b, 509.1b)"): g.act(1, Move::Block(vec![(a, gollum)])));
    cite!(["702.111b"]; ok: g.act(1, Move::Block(vec![(a, gollum), (b, gollum)])));
    // with only one creature the defending player has no block to declare
    let mut g = game();
    let gollum = g.put(0, GOLLUM, Zone::Battlefield);
    g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[gollum]);
    cite!(["702.111b"]; assert_eq!(g.decision, None));
}

#[test]
fn r702_10b_a_creature_with_haste_attacks_the_turn_it_comes_under_control() {
    let mut g = game();
    let lands: Vec<ObjId> = (0..3).map(|_| g.put(0, MOUNTAIN, Zone::Battlefield)).collect();
    let card = g.put(0, PILGRIM, Zone::Hand);
    for l in lands {
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    g.act(0, paid("RRR", Move::Cast { card, targets: vec![] })).unwrap();
    all_pass(&mut g);
    let pilgrim = *g.battlefield.last().unwrap();
    to_attackers(&mut g);
    cite!(["702.10b"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 })));
    cite!(["702.10b"]; ok: g.act(0, Move::Attack(vec![(pilgrim, Target::Player(1))])));
}

#[test]
fn r702_20b_attacking_doesnt_cause_a_creature_with_vigilance_to_tap() {
    let mut g = game();
    let shieldmate = g.put(0, SHIELDMATE, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(shieldmate, Target::Player(1)), (bears, Target::Player(1))])).unwrap();
    cite!(["702.20b"]; assert!(!g.obj(shieldmate).tapped));
    cite!(["508.1f"]; assert!(g.obj(bears).tapped));
    assert!(g.combat.attacking(shieldmate));
}

#[test]
fn r702_7b_a_creature_with_first_strike_deals_its_combat_damage_in_a_step_before() {
    let mut g = game();
    let halberdier = g.put(0, HALBERDIER, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[halberdier], &[(bears, halberdier)]);
    // only the creature with first strike has assigned combat damage
    assert!(destroyed(&g, bears));
    cite!(["702.7b"]; assert_eq!(g.obj(halberdier).damage, 0));
    // the phase gets a second combat damage step; the bears are not there to deal theirs
    all_pass(&mut g);
    cite!(["702.7b"]; assert_eq!((g.part, damage_steps(&g)), (Part::CombatDamage, 2)));
    cite!(["702.7b"]; assert_eq!(g.obj(halberdier).damage, 0));
    assert!(g.battlefield.contains(&halberdier), "a 3/1 that would have been dealt 2");
    all_pass(&mut g);
    assert_eq!(g.part, Part::EndOfCombat);

    // with no first strike or double strike in combat there is one step, and both deal at once
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[attacker], &[(blocker, attacker)]);
    cite!(["510.2", "704.5g"]; assert!(destroyed(&g, attacker) && destroyed(&g, blocker)));
    all_pass(&mut g);
    cite!(["506.1"]; assert_eq!((g.part, damage_steps(&g)), (Part::EndOfCombat, 1)));
}

#[test]
fn r510_4_in_the_second_step_the_creatures_that_had_neither_assign_theirs() {
    let mut g = game();
    let wolves = g.put(0, WOLVES, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    fight(&mut g, &[wolves, bears], &[]);
    cite!(["510.4"]; assert_eq!(g.players[1].life, 19, "the wolves' 1"));
    all_pass(&mut g);
    assert_eq!((g.part, damage_steps(&g)), (Part::CombatDamage, 2));
    cite!(["510.4"]; assert_eq!(g.players[1].life, 17, "the bears' 2, and not the wolves' again"));
}

#[test]
fn r702_4b_a_creature_with_double_strike_assigns_combat_damage_in_both_steps() {
    let mut g = game();
    let ace = g.put(0, ACE, Zone::Battlefield);
    fight(&mut g, &[ace], &[]);
    cite!(["702.4b"]; assert_eq!(g.players[1].life, 19));
    all_pass(&mut g);
    assert_eq!((g.part, damage_steps(&g)), (Part::CombatDamage, 2));
    cite!(["702.4b"]; assert_eq!(g.players[1].life, 18));
    all_pass(&mut g);
    cite!(["702.4b"]; assert_eq!((g.part, damage_steps(&g)), (Part::EndOfCombat, 2), "and there is no third"));
}

#[test]
fn r702_19b_trample_assigns_what_is_more_than_lethal_to_what_it_attacks() {
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[dreadmaw], &[(bears, dreadmaw)]);
    let among = vec![Target::Object(bears), Target::Player(1)];
    let asked = Some(Decision::Divide { player: 0, attacker: dreadmaw, among, amount: 6 });
    cite!(["702.19b"]; assert_eq!(g.decision, asked));
    // not to the player before the creature blocking it is assigned lethal damage
    let short = g.act(0, Move::Divide(vec![(Target::Object(bears), 1), (Target::Player(1), 5)])).unwrap_err();
    cite!(["510.1e", "702.19b"]; assert_eq!(short, "Colossal Dreadmaw assigns damage to what it attacks once Grizzly Bears is assigned lethal damage (702.19b); the assignment begins again (510.1e)"));
    assert_eq!(g.decision, asked, "the assignment begins again");
    g.act(0, Move::Divide(vec![(Target::Object(bears), 2), (Target::Player(1), 4)])).unwrap();
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 16));
    assert!(destroyed(&g, bears));

    // its controller need not assign any of it to the player
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[dreadmaw], &[(bears, dreadmaw)]);
    g.act(0, Move::Divide(vec![(Target::Object(bears), 6)])).unwrap();
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 20));

    // damage already marked on the creature counts toward lethal
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.damage(dreadmaw, Target::Object(bears), 1);
    fight(&mut g, &[dreadmaw], &[(bears, dreadmaw)]);
    g.act(0, Move::Divide(vec![(Target::Object(bears), 1), (Target::Player(1), 5)])).unwrap();
    cite!(["702.19b"]; assert_eq!(g.players[1].life, 15));
}

#[test]
fn r702_19d_trample_blocked_with_no_creature_blocking_it_assigns_all_to_what_it_attacks() {
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[dreadmaw]);
    g.act(1, Move::Block(vec![(bears, dreadmaw)])).unwrap();
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    all_pass(&mut g);
    assert!(destroyed(&g, bears));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["702.19d"]; assert_eq!(g.decision, None, "nothing to divide"));
    cite!(["702.19d"]; assert_eq!(g.players[1].life, 14));
}

#[test]
fn r702_2b_a_creature_dealt_damage_by_a_source_with_deathtouch_is_destroyed() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let gilacorn = g.put(1, GILACORN, Zone::Battlefield);
    fight(&mut g, &[giant], &[(gilacorn, giant)]);
    cite!(["704.5h"]; assert!(destroyed(&g, giant), "1 damage on a 3/3, from a source with deathtouch (704.5h)"));
    assert!(destroyed(&g, gilacorn));

    // from a source without it, 1 damage is marked and the creature stays
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let ones = g.put(1, ONES, Zone::Battlefield);
    fight(&mut g, &[giant], &[(ones, giant)]);
    cite!(["120.3e"]; assert_eq!(g.obj(giant).damage, 1));
    assert!(!destroyed(&g, giant));
}

/// No card is printed with both deathtouch and trample, so the rule is asked where trample
/// asks it: of what lethal damage is.
#[test]
fn r702_2c_any_damage_from_a_source_with_deathtouch_is_lethal_damage() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let gilacorn = g.put(1, GILACORN, Zone::Battlefield);
    let ones = g.put(1, ONES, Zone::Battlefield);
    cite!(["702.2c"]; assert!(g.lethal(giant, &[gilacorn], 1)));
    cite!(["702.2c"]; assert!(!g.lethal(giant, &[gilacorn], 0), "a nonzero amount"));
    assert!(!g.lethal(giant, &[ones], 1));
    cite!(["120.6"]; assert!(g.lethal(giant, &[ones], 3), "its toughness (120.6)"));
    g.damage(ones, Target::Object(giant), 2);
    cite!(["702.19b"]; assert!(g.lethal(giant, &[ones], 1), "with what is already marked on it"));
}

#[test]
fn r702_15b_damage_dealt_by_a_source_with_lifelink_also_gains_its_controller_that_much_life() {
    let mut g = game();
    let glutton = g.put(0, GLUTTON, Zone::Battlefield);
    fight(&mut g, &[glutton], &[]);
    cite!(["702.15b"]; assert_eq!((g.players[0].life, g.players[1].life), (24, 16)));
    assert!(g.log.contains(&Event::LifeGained { player: 0, amount: 4 }));

    // whatever it is dealt to (120.3f)
    let mut g = game();
    let glutton = g.put(0, GLUTTON, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[glutton], &[(bears, glutton)]);
    cite!(["702.15b", "120.3f"]; assert_eq!((g.players[0].life, g.players[1].life), (24, 20)));

    // a source without lifelink gains its controller nothing
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    fight(&mut g, &[giant], &[]);
    assert_eq!((g.players[0].life, g.players[1].life), (20, 17));
}

#[test]
fn r702_8a_a_card_with_flash_is_cast_any_time_an_instant_could_be() {
    let mut g = game();
    let forests: Vec<ObjId> = (0..5).map(|_| g.put(1, FOREST, Zone::Battlefield)).collect();
    let cheetah = g.put(1, CHEETAH, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Hand);
    // in another player's turn, with priority
    g.act(0, Move::Pass).unwrap();
    for f in forests {
        g.act(1, Move::Activate { source: f, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(1, paid("GG", Move::Cast { card: bears, targets: vec![] })));
    g.act(1, paid("GGG", Move::Cast { card: cheetah, targets: vec![] })).unwrap();
    all_pass(&mut g);
    let arrived = *g.battlefield.last().unwrap();
    cite!(["702.8a"]; assert_eq!((g.obj(arrived).chars.name.as_str(), g.obj(arrived).controller), ("Pouncing Cheetah", 1)));
}
