//! The steps of combat (CR 506–511), rule by rule: the declarations of attackers and blockers
//! and the assignment of combat damage as turn-based actions, who gets priority after each,
//! what attacking and blocking make of a creature, and the abilities that trigger on them.
//! Where an ability's trigger condition is one the engine does not watch for yet, the test is
//! that a real card that has it is refused rather than run wrongly.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::combat::status;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Part, Status};

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const GIANT: usize = 2;
const ONES: usize = 3;
const BOLT: usize = 4;
const CONQUISTADOR: usize = 5;
const HORDECALLER: usize = 6;
const KILL_SHOT: usize = 7;
const PLAINS: usize = 8;
const TROOPER: usize = 9;
const TIGER: usize = 10;
const ACE: usize = 11;
const WOLVES: usize = 12;
const DREADMAW: usize = 13;
const JACE: usize = 14;
const TEMPLE: usize = 15;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Wandering Ones", "{U}", "Creature — Spirit", "", Some((1, 1))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card(
            "Vicious Conquistador",
            "{B}",
            "Creature — Vampire Soldier",
            "Whenever this creature attacks, each opponent loses 1 life.",
            Some((1, 2)),
        ),
        card(
            "Razorkin Hordecaller",
            "{4}{R}",
            "Creature — Human Clown Berserker",
            "Haste\nWhenever you attack, create a 1/1 red Gremlin creature token.",
            Some((4, 4)),
        ),
        card("Kill Shot", "{2}{W}", "Instant", "Destroy target attacking creature.", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Royal Trooper", "{2}{W}", "Creature — Human Soldier", "Whenever this creature blocks, it gets +2/+2 until end of turn.", Some((2, 2))),
        card(
            "Cave Tiger",
            "{2}{G}",
            "Creature — Cat",
            "Whenever this creature becomes blocked by a creature, this creature gets +1/+1 until end of turn.",
            Some((2, 2)),
        ),
        card("Fencing Ace", "{1}{W}", "Creature — Human Soldier", "Double strike", Some((1, 1))),
        card("Tundra Wolves", "{W}", "Creature — Wolf", "First strike", Some((1, 1))),
        card("Colossal Dreadmaw", "{4}{G}{G}", "Creature — Dinosaur", "Trample", Some((6, 6))),
        card(
            "Jace Beleren",
            "{1}{U}{U}",
            "Legendary Planeswalker — Jace",
            "+2: Each player draws a card.\n−1: Target player draws a card.\n−10: Target player mills twenty cards.",
            None,
        ),
        card(
            "Temple of Triumph",
            "",
            "Land",
            "This land enters tapped.\nWhen this land enters, scry 1. (Look at the top card of your library. You may put that card on the bottom.)\n{T}: Add {R} or {W}.",
            None,
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

/// Jace Beleren on the battlefield under this player's control, with the three loyalty
/// counters a planeswalker enters with (306.5b).
fn jace(g: &mut Game, player: usize) -> ObjId {
    let jace = g.put(player, JACE, Zone::Battlefield);
    g.put_counters(jace, "loyalty", 3);
    jace
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

fn destroyed(g: &Game, o: ObjId) -> bool {
    g.log.iter().any(|e| matches!(e, Event::Destroyed { object } if *object == o))
}

/// The combat damage dealt so far: source, what it was dealt to, how much.
fn combat_damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { source, to, amount, combat: true } => Some((*source, *to, *amount)),
            _ => None,
        })
        .collect()
}

fn is(g: &Game, o: ObjId, s: Status) -> bool {
    status(g, o, s) == Some(true)
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        // loyalty abilities are not activated yet; the planeswalker is put onto the
        // battlefield here, to be attacked
        if d.name() != "Jace Beleren" {
            check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
        }
    }
}

// ------------------------------------------------------------------------------ 506 and 703

#[test]
fn r506_2_the_active_player_attacks_the_defending_player_and_the_planeswalkers_they_control() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = jace(&mut g, 1);
    let mine = jace(&mut g, 0);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    to_attackers(&mut g);
    // the active player is the attacking player
    cite!(["506.2"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 })));
    cite!(["508.1"]; refused("that decision is another player's"): g.act(1, Move::Attack(vec![])));
    // the nonactive player, and planeswalkers they control, may be attacked; nothing else
    cite!(["506.2"]; refused("a creature attacks a defending player or their planeswalker (506.2)"): g.act(0, Move::Attack(vec![(bears, object(mine))])));
    cite!(["506.2"]; refused("a creature attacks a defending player or their planeswalker (506.2)"): g.act(0, Move::Attack(vec![(bears, Target::Player(0))])));
    g.act(0, Move::Attack(vec![(bears, object(theirs))])).unwrap();
    all_pass(&mut g);
    // the creature attacking the planeswalker is attacking the defending player's: they block
    cite!(["506.2"]; assert_eq!(g.decision, Some(Decision::Blockers { player: 1 })));
    g.act(1, Move::Block(vec![(blocker, bears)])).unwrap();
    cite!(["506.2"]; assert_eq!(g.combat.blockers, vec![(blocker, bears)]));
}

#[test]
fn r703_4i_immediately_after_the_declare_attackers_step_begins_the_active_player_declares_attackers() {
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    cite!(["703.4i"]; assert_eq!(g.log.last(), Some(&Event::Began { part: Part::DeclareAttackers }), "nothing else has happened in the step"));
    cite!(["703.4i"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 })));
    // before anyone gets priority in it
    cite!(["508.1"]; refused("a decision is pending: declare attackers (508.1)"): g.act(0, Move::Pass));
    cite!(["508.1"]; refused("that decision is another player's"): g.act(1, Move::Pass));
}

// ------------------------------------------------------------------------- declare attackers

#[test]
fn r508_1_an_illegal_declaration_of_attackers_returns_the_game_to_the_moment_before() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    g.tap(tapped);
    to_attackers(&mut g);
    let log = g.log.len();
    // one creature able to comply and one not: the declaration is illegal, all of it
    cite!(["508.1a"]; refused("a tapped creature can't be declared as an attacker (508.1a)"): g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (tapped, Target::Player(1))])));
    cite!(["508.1"]; assert!(!g.obj(bears).tapped));
    cite!(["508.1"]; assert!(g.combat.attackers.is_empty()));
    cite!(["508.1"]; assert_eq!(g.log.len(), log, "nothing happened"));
    cite!(["508.1"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 }), "the declaration is still to be made"));
    // the declaration is a turn-based action: it doesn't use the stack
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    cite!(["508.1"]; assert!(g.stack.is_empty()));
    cite!(["508.1"]; assert!(g.combat.attacking(bears)));
}

#[test]
fn r508_1b_the_active_player_announces_what_each_attacking_creature_attacks() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let jace = jace(&mut g, 1);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1)), (giant, object(jace))])).unwrap();
    cite!(["508.1b"]; assert_eq!(g.combat.attackers, vec![(bears, Target::Player(1)), (giant, object(jace))]));
    cite!(["508.1b"]; assert!(g.log.contains(&Event::Attacked { attacker: giant, target: object(jace) })));
    // nothing could block: both pass through the declare blockers step to combat damage
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["508.1b"]; assert_eq!(combat_damage(&g), vec![(bears, Target::Player(1), 2), (giant, object(jace), 3)]));
    cite!(["508.1b"]; assert_eq!(g.players[1].life, 18, "only the bears' damage to the player"));
    // 3 damage removes its 3 loyalty counters (120.3c): it is put into the graveyard (704.5i)
    cite!(["120.3c", "704.5i"]; assert_eq!(g.obj(jace).zone, Zone::Gone));
}

#[test]
fn r508_1f_attacking_causes_the_chosen_creatures_to_become_tapped() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    cite!(["508.1f"]; assert!(g.obj(bears).tapped));
    cite!(["508.1f"]; assert!(!g.obj(giant).tapped, "only the chosen creatures"));
    // not a cost paid: the creature becomes tapped, an event like any other tapping
    cite!(["508.1f"]; assert!(g.log.contains(&Event::Tapped { object: bears })));
    cite!(["508.1f"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Activated { .. }))));
}

#[test]
fn r508_1k_each_chosen_creature_becomes_an_attacking_creature() {
    // "Destroy target attacking creature."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let plains: Vec<ObjId> = (0..6).map(|_| g.put(1, PLAINS, Zone::Battlefield)).collect();
    let kill_shot = g.put(1, KILL_SHOT, Zone::Hand);
    let cast = |g: &mut Game, o: ObjId| g.act(1, paid("WWW", Move::Cast { card: kill_shot, targets: vec![vec![object(o)]] }));
    // in the beginning of combat step no creature is attacking yet
    all_pass(&mut g);
    assert_eq!(g.part, Part::BeginningOfCombat);
    g.act(0, Move::Pass).unwrap();
    for p in &plains[..3] {
        g.act(1, Move::Activate { source: *p, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    cite!(["508.1k"]; refused("an illegal target, Grizzly Bears: it isn't attacking (115.1)"): cast(&mut g, bears));
    // the mana ability was an action between the passes: the step ends when both have passed
    // again in succession (117.4)
    g.act(1, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.part, Part::BeginningOfCombat));
    g.act(0, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.part, Part::DeclareAttackers));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    g.act(0, Move::Pass).unwrap();
    for p in &plains[3..] {
        g.act(1, Move::Activate { source: *p, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    // a creature that was not chosen is not attacking
    cite!(["508.1k"]; refused("an illegal target, Hill Giant: it isn't attacking (115.1)"): cast(&mut g, giant));
    cite!(["508.1k"]; ok: cast(&mut g, bears));
    all_pass(&mut g);
    assert!(destroyed(&g, bears));
    cite!(["506.4"]; assert!(g.combat.attackers.is_empty(), "and it has left combat (506.4)"));
}

#[test]
fn r508_1m_abilities_that_trigger_on_attackers_being_declared_trigger() {
    // "Whenever this creature attacks, …" and "Whenever you attack, …"
    let mut g = game();
    let conquistador = g.put(0, CONQUISTADOR, Zone::Battlefield);
    g.put(0, HORDECALLER, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(conquistador, Target::Player(1))])).unwrap();
    // both triggered on the declaration; their controller puts them on the stack in the
    // order they choose (603.3b), and they are there as the active player gets priority
    // (508.2)
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["508.1m"]; assert_eq!((g.stack.len(), g.priority), (2, 0)));
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1m"]; assert_eq!(g.players[1].life, 19));
    cite!(["508.1m"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).token()).count(), 1));
}

#[test]
fn r508_2_after_attackers_are_declared_the_active_player_gets_priority() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    cite!(["508.2"]; assert_eq!((g.decision.clone(), g.priority), (None, 0)));
    // not the defending player
    cite!(["508.2"]; refused("that player does not have priority"): g.act(1, Move::Pass));
    g.act(0, Move::Pass).unwrap();
    cite!(["508.2"]; assert_eq!((g.part, g.priority), (Part::DeclareAttackers, 1)));
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::DeclareBlockers);
}

#[test]
fn r508_3d_whenever_you_attack_triggers_once_however_many_creatures_attack() {
    // "Whenever you attack, create a 1/1 red Gremlin creature token."
    let tokens = |g: &Game| g.battlefield.iter().filter(|o| g.obj(**o).token()).count();
    let mut g = game();
    let hordecaller = g.put(0, HORDECALLER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hordecaller, Target::Player(1)), (bears, Target::Player(1))])).unwrap();
    cite!(["508.3d"]; assert_eq!(g.stack.len(), 1));
    all_pass(&mut g);
    cite!(["508.3d"]; assert_eq!(tokens(&g), 1));

    // with no creature declared as an attacker, the player has not attacked
    let mut g = game();
    g.put(0, HORDECALLER, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![])).unwrap();
    cite!(["508.3d"]; assert!(g.stack.is_empty()));
    cite!(["508.3d"]; assert_eq!(tokens(&g), 0));
}

#[test]
fn r508_5_defending_player_in_an_attacking_creatures_ability_is_the_one_it_attacks() {
    // "Whenever this creature attacks, defending player loses 1 life and you gain 1 life.":
    // of three players, the one whose planeswalker it attacks (508.5, 508.5a)
    let mut defs = defs();
    let assassin = CardDef::new(
        "Agate-Blade Assassin",
        "{1}{B}",
        "Creature — Lizard Assassin",
        "Whenever this creature attacks, defending player loses 1 life and you gain 1 life.",
        Some((1, 3)),
    )
    .expect("the grammar reads it");
    check(&assassin).unwrap();
    defs.push(assassin);
    let index = defs.len() - 1;
    let mut g = Game::new(defs, 3, 20);
    let assassin = g.put(0, index, Zone::Battlefield);
    let theirs = jace(&mut g, 2);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(assassin, object(theirs))])).unwrap();
    all_pass(&mut g);
    cite!(["508.5"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![21, 20, 19]));
}

// -------------------------------------------------------------------------- declare blockers

#[test]
fn r509_1_an_illegal_declaration_of_blockers_returns_the_game_to_the_moment_before() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let tapped = g.put(1, ONES, Zone::Battlefield);
    g.tap(tapped);
    attack(&mut g, &[bears]);
    cite!(["509.1"]; assert_eq!(g.decision, Some(Decision::Blockers { player: 1 })));
    let log = g.log.len();
    cite!(["509.1a"]; refused("a tapped creature can't be declared as a blocker (509.1a)"): g.act(1, Move::Block(vec![(giant, bears), (tapped, bears)])));
    cite!(["509.1"]; assert!(g.combat.blockers.is_empty() && !g.combat.blocking(giant)));
    cite!(["509.1"]; assert_eq!(g.log.len(), log, "nothing happened"));
    cite!(["509.1"]; assert_eq!(g.decision, Some(Decision::Blockers { player: 1 }), "the declaration is still to be made"));
    // the declaration is a turn-based action: it doesn't use the stack
    g.act(1, Move::Block(vec![(giant, bears)])).unwrap();
    cite!(["509.1"]; assert!(g.stack.is_empty()));
    cite!(["509.1"]; assert_eq!(g.combat.blockers, vec![(giant, bears)]));
}

#[test]
fn r509_1c_a_requirement_to_block_is_not_in_the_engine_and_the_card_is_refused() {
    // "This creature blocks each combat if able.": requirements to block are not obeyed by the
    // engine yet (combat.rs), so the card is refused rather than run as one that may not block
    let screen = CardDef::new(
        "Razorgrass Screen",
        "{1}",
        "Artifact Creature — Wall",
        "Defender (This creature can't attack.)\nThis creature blocks each combat if able.",
        Some((2, 1)),
    )
    .expect("the grammar reads it");
    assert!(check(&screen).unwrap_err().0.contains("requirement: to block"));
}

#[test]
fn r509_1h_an_attacker_with_a_blocker_declared_is_blocked_and_one_without_is_unblocked() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let blocker = g.put(1, ONES, Zone::Battlefield);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    attack(&mut g, &[bears, giant]);
    // before blockers are declared it is neither
    for s in [Status::Blocked, Status::Unblocked] {
        cite!(["509.1h"]; assert!(!is(&g, bears, s) && !is(&g, giant, s)));
    }
    g.act(1, Move::Block(vec![(blocker, bears)])).unwrap();
    cite!(["509.1h"]; assert!(is(&g, bears, Status::Blocked) && !is(&g, bears, Status::Unblocked)));
    cite!(["509.1h"]; assert!(is(&g, giant, Status::Unblocked) && !is(&g, giant, Status::Blocked)));
    // it remains blocked even if all the creatures blocking it are removed from combat
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![object(blocker)]] })).unwrap();
    all_pass(&mut g);
    assert!(destroyed(&g, blocker));
    cite!(["509.1h"]; assert!(is(&g, bears, Status::Blocked) && !is(&g, bears, Status::Unblocked)));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["510.1c"]; assert_eq!(combat_damage(&g), vec![(giant, Target::Player(1), 3)], "the blocked bears deal none (510.1c)"));
}

#[test]
fn r509_1i_abilities_that_trigger_on_blockers_being_declared_trigger() {
    // "Whenever this creature becomes blocked by a creature, …" and "Whenever this creature
    // blocks, …": each player's, on the one declaration
    let mut g = game();
    let tiger = g.put(0, TIGER, Zone::Battlefield);
    let trooper = g.put(1, TROOPER, Zone::Battlefield);
    attack(&mut g, &[tiger]);
    g.act(1, Move::Block(vec![(trooper, tiger)])).unwrap();
    cite!(["509.2"]; assert_eq!((g.stack.len(), g.priority), (2, 0), "on the stack as the active player gets priority (509.2)"));
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["509.1i"]; assert_eq!((g.power(tiger), g.toughness(tiger)), (Some(3), Some(3))));
    cite!(["509.1i"]; assert_eq!((g.power(trooper), g.toughness(trooper)), (Some(4), Some(4))));
}

#[test]
fn r509_2_after_blockers_are_declared_the_active_player_gets_priority() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    attack(&mut g, &[bears]);
    g.act(1, Move::Block(vec![(giant, bears)])).unwrap();
    cite!(["509.2"]; assert_eq!((g.decision.clone(), g.priority), (None, 0)));
    // not the defending player who declared
    cite!(["509.2"]; refused("that player does not have priority"): g.act(1, Move::Pass));
    g.act(0, Move::Pass).unwrap();
    cite!(["509.2"]; assert_eq!((g.part, g.priority), (Part::DeclareBlockers, 1)));
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::CombatDamage);
}

#[test]
fn r509_3a_whenever_a_creature_blocks_triggers_if_it_is_declared_as_a_blocker() {
    // "Whenever this creature blocks, it gets +2/+2 until end of turn."
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let trooper = g.put(1, TROOPER, Zone::Battlefield);
    let idle = g.put(1, TROOPER, Zone::Battlefield);
    attack(&mut g, &[giant]);
    g.act(1, Move::Block(vec![(trooper, giant)])).unwrap();
    cite!(["509.3a"]; assert_eq!(g.stack.len(), 1, "the one declared as a blocker, and not the other"));
    all_pass(&mut g);
    cite!(["509.3a"]; assert_eq!((g.power(trooper), g.toughness(trooper)), (Some(4), Some(4))));
    cite!(["509.3a"]; assert_eq!((g.power(idle), g.toughness(idle)), (Some(2), Some(2))));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    assert!(destroyed(&g, giant));
    cite!(["509.3a"]; assert_eq!(g.obj(trooper).damage, 3, "3 damage on a 4/4"));
    cite!(["509.3a"]; assert!(g.battlefield.contains(&trooper)));
}

#[test]
fn r509_3d_becomes_blocked_by_a_creature_triggers_once_for_each_creature_that_blocks() {
    // "Whenever this creature becomes blocked by a creature, this creature gets +1/+1 until
    // end of turn."
    let mut g = game();
    let tiger = g.put(0, TIGER, Zone::Battlefield);
    let (a, b) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, ONES, Zone::Battlefield));
    attack(&mut g, &[tiger]);
    g.act(1, Move::Block(vec![(a, tiger), (b, tiger)])).unwrap();
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(tiger)));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["509.3d"]; assert_eq!(g.stack.len(), 2));
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["509.3d"]; assert_eq!((g.power(tiger), g.toughness(tiger)), (Some(4), Some(4))));
}

#[test]
fn r509_3g_attacks_and_isnt_blocked_is_not_watched_for_and_the_card_is_refused() {
    // "Whenever this creature attacks and isn't blocked": not watched for yet (trigger.rs)
    let dwellers = CardDef::new(
        "Murk Dwellers",
        "{3}{B}",
        "Creature — Zombie",
        "Whenever this creature attacks and isn't blocked, it gets +2/+0 until end of combat.",
        Some((2, 2)),
    )
    .expect("the grammar reads it");
    assert!(check(&dwellers).unwrap_err().0.contains("AttacksUnblocked"));
}

// ----------------------------------------------------------------------------- combat damage

#[test]
fn r120_2a_each_attacking_and_blocking_creature_deals_combat_damage_equal_to_its_power() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let trooper = g.put(1, TROOPER, Zone::Battlefield);
    attack(&mut g, &[giant]);
    g.act(1, Move::Block(vec![(trooper, giant)])).unwrap();
    // the Trooper's trigger makes it a 4/4 before damage
    all_pass(&mut g);
    cite!(["120.2a"]; assert!(combat_damage(&g).is_empty(), "none before the combat damage step"));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["120.2a"]; assert_eq!(combat_damage(&g), vec![(giant, object(trooper), 3), (trooper, object(giant), 4)], "its power as damage is dealt"));
}

#[test]
fn r510_1_the_active_player_assigns_the_attacking_creatures_damage_and_it_doesnt_use_the_stack() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let (ones, bears) = (g.put(1, ONES, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    fight(&mut g, &[giant], &[(ones, giant), (bears, giant)]);
    cite!(["510.1"]; assert!(matches!(g.decision, Some(Decision::Divide { player: 0, attacker, .. }) if attacker == giant), "{:?}", g.decision));
    // nobody has priority while it is assigned
    cite!(["510.1"]; refused("that decision is another player's"): g.act(1, Move::Pass));
    cite!(["510.1"]; assert!(combat_damage(&g).is_empty()));
    g.act(0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])).unwrap();
    cite!(["404.3"]; ok: g.act(1, Move::Choose(game::state::Answer::Objects(vec![ones, bears]))));
    // all of it is dealt at once, with nothing on the stack
    cite!(["510.1"]; assert!(g.stack.is_empty()));
    cite!(["510.1"]; assert_eq!(combat_damage(&g).len(), 4));
    assert!(destroyed(&g, giant) && destroyed(&g, ones) && destroyed(&g, bears));
}

#[test]
fn r510_1d_a_blocking_creature_assigns_its_combat_damage_to_the_creature_it_blocks() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let ones = g.put(0, ONES, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    let mountain = g.put(1, MOUNTAIN, Zone::Battlefield);
    let bolt = g.put(1, BOLT, Zone::Hand);
    attack(&mut g, &[bears, ones]);
    g.act(1, Move::Block(vec![(giant, bears), (blocker, ones)])).unwrap();
    // the defending player destroys the creature their bears block: those block nothing now
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![object(ones)]] })).unwrap();
    all_pass(&mut g);
    assert!(destroyed(&g, ones));
    cite!(["510.1d"]; assert!(g.combat.blocking(blocker) && g.combat.blockers_of(ones).is_empty()));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    // blocking exactly one creature: all its damage to that one; blocking none: no damage
    cite!(["510.1d"]; assert_eq!(combat_damage(&g), vec![(bears, object(giant), 2), (giant, object(bears), 3)]));
    cite!(["510.1d"]; assert_eq!(g.players[0].life, 20));
}

#[test]
fn r510_1e_an_illegal_total_assignment_begins_again_from_the_first_creature() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let (ones, bears) = (g.put(1, ONES, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    let other = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[giant, dreadmaw], &[(ones, giant), (bears, giant), (other, dreadmaw)]);
    let first = g.decision.clone();
    assert!(matches!(first, Some(Decision::Divide { attacker, .. }) if attacker == giant), "{first:?}");
    let began = format!("{g:?}");
    g.act(0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])).unwrap();
    cite!(["510.1e"]; assert!(matches!(g.decision, Some(Decision::Divide { attacker, .. }) if attacker == dreadmaw)));
    // once every creature's damage is assigned, the whole assignment is checked: trample
    // assigns to the player only once the blocker is assigned lethal damage (702.19b)
    cite!(["510.1e"]; refused("Colossal Dreadmaw assigns damage to what it attacks once Grizzly Bears is assigned lethal damage (702.19b); the assignment begins again (510.1e)"): g.act(0, Move::Divide(vec![(object(other), 1), (Target::Player(1), 5)])));
    cite!(["510.1e"]; assert_eq!(g.decision, first, "back to the moment before the player began to assign, the giant's division too"));
    cite!(["510.1e"]; assert_eq!(format!("{g:?}"), began, "the whole game as it was before the first assignment"));
    cite!(["510.1e"]; assert!(combat_damage(&g).is_empty()));
    cite!(["510.1e"]; assert_eq!(g.players[1].life, 20));
    g.act(0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])).unwrap();
    g.act(0, Move::Divide(vec![(object(other), 2), (Target::Player(1), 4)])).unwrap();
    cite!(["510.1e"]; assert_eq!(g.players[1].life, 16));
}

/// The restart is a new question to the seat, not the refused one asked again: its request
/// is the first creature's division, and an answer to it is accepted.
#[test]
fn r510_1e_a_restarted_assignment_is_asked_as_a_new_request() {
    use game::legal::Asked;
    use game::simulation::{ResponseError, Simulation};
    use crate::support::refused;
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let (ones, bears) = (g.put(1, ONES, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    let other = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[giant, dreadmaw], &[(ones, giant), (bears, giant), (other, dreadmaw)]);
    let mut sim = Simulation::new(g);
    let asked = |sim: &mut Simulation| { let r = sim.request().unwrap().unwrap().clone(); let Asked::Divide { attacker, .. } = r.question else { panic!("{:?}", r.question) }; (r.id, attacker) };
    let (id, attacker) = asked(&mut sim);
    assert_eq!(attacker, giant);
    sim.respond(id, 0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])).unwrap();
    let (id, attacker) = asked(&mut sim);
    assert_eq!(attacker, dreadmaw);
    // trample: the player is assigned damage only once the blocker has lethal (702.19b)
    refused(sim.respond(id, 0, Move::Divide(vec![(object(other), 1), (Target::Player(1), 5)])), "once Grizzly Bears is assigned lethal damage (702.19b)");
    let (again, attacker) = asked(&mut sim);
    cite!(["510.1e"]; assert_eq!(attacker, giant, "the assignment begins again from the first creature (510.1e)"));
    assert_ne!(again, id, "a new question is a new request");
    assert_eq!(sim.respond(id, 0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])), Err(ResponseError::Stale { expected: again, received: id }), "the old request is stale");
    sim.respond(again, 0, Move::Divide(vec![(object(ones), 1), (object(bears), 2)])).unwrap();
    let (id, _) = asked(&mut sim);
    sim.respond(id, 0, Move::Divide(vec![(object(other), 2), (Target::Player(1), 4)])).unwrap();
    cite!(["510.1e"]; assert_eq!(sim.game().unwrap().players[1].life, 16));
}

/// Two Colossal Dreadmaws (6/6, trample) attack player 1, each blocked by one Grizzly Bears
/// (2/2), after `first` was done in player 0's precombat main phase: the simulation asks
/// player 0 to divide the first Dreadmaw's damage.
fn trampling_pair(players: usize, first: impl FnOnce(&mut Game)) -> (game::simulation::Simulation, [ObjId; 4]) {
    let mut g = Game::new(defs(), players, 20);
    first(&mut g);
    let (one, two) = (g.put(0, DREADMAW, Zone::Battlefield), g.put(0, DREADMAW, Zone::Battlefield));
    let (a, b) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    fight(&mut g, &[one, two], &[(a, one), (b, two)]);
    (game::simulation::Simulation::new(g), [one, two, a, b])
}

/// The division asked for, and the request it is asked under.
fn division_asked(sim: &mut game::simulation::Simulation) -> (game::simulation::RequestId, ObjId) {
    let r = sim.request().unwrap().unwrap().clone();
    let game::legal::Asked::Divide { attacker, .. } = r.question else { panic!("{:?}", r.question) };
    (r.id, attacker)
}

/// This much of a Dreadmaw's 6 damage to the Grizzly Bears blocking it, and the rest
/// trampling over to player 1.
fn trample(bears: ObjId, to_bears: u32) -> Move {
    Move::Divide(vec![(object(bears), to_bears), (Target::Player(1), 6 - to_bears)])
}

/// Each blocked Dreadmaw assigned the Bears' lethal 2 and 4 to the player is legal (702.19b),
/// and is the assignment once the restart is answered: player 1 is dealt 8.
fn assign_legally(sim: &mut game::simulation::Simulation, [_, _, a, b]: [ObjId; 4], case: &str) {
    for bears in [a, b] {
        let (id, _) = division_asked(sim);
        sim.respond(id, 0, trample(bears, 2)).unwrap();
    }
    cite!(["510.1e", "702.19b"]; assert_eq!(sim.game().unwrap().players[1].life, 12, "{case}"));
}

/// The ways a response can be given: plainly, replaying random outcomes or shuffles, or
/// capturing them. Each scopes its randomness to that one response.
#[derive(Clone, Copy, Debug)]
enum Via { Respond, Random, Shuffles, Capture }

fn respond_via(sim: &mut game::simulation::Simulation, via: Via, id: game::simulation::RequestId, mv: Move) -> Result<(), game::simulation::ResponseError> {
    match via {
        Via::Respond => sim.respond(id, 0, mv),
        Via::Random => sim.respond_with_random(id, 0, mv, vec![], false).map(|unused| assert!(unused.is_empty())),
        Via::Shuffles => sim.respond_with_shuffles(id, 0, mv, vec![]),
        Via::Capture => sim.respond_capturing_random(id, 0, mv).map(|captured| assert!(captured.is_empty())),
    }
}

/// However each division is given, an illegal total returns the game to the moment before
/// the player began to assign (510.1e): the first Dreadmaw's division, asked as a new
/// request. Dividing needs no randomness, so a replay or capture scope changes nothing.
#[test]
fn r510_1e_an_illegal_total_begins_again_through_every_response_entry_point() {
    const VIAS: [Via; 4] = [Via::Respond, Via::Random, Via::Shuffles, Via::Capture];
    for (first, second) in VIAS.into_iter().flat_map(|first| VIAS.map(|second| (first, second))) {
        let case = format!("first division {first:?}, second {second:?}");
        let (mut sim, creatures @ [one, two, a, b]) = trampling_pair(2, |_| {});
        let (id, attacker) = division_asked(&mut sim);
        assert_eq!(attacker, one);
        respond_via(&mut sim, first, id, trample(a, 2)).unwrap();
        let (id, attacker) = division_asked(&mut sim);
        assert_eq!(attacker, two);
        // 1 of the Bears' lethal 2, and the rest to the player (702.19b)
        cite!(["510.1e", "702.19b"]; refused("Colossal Dreadmaw assigns damage to what it attacks once Grizzly Bears is assigned lethal damage (702.19b); the assignment begins again (510.1e)"): respond_via(&mut sim, second, id, trample(b, 1)));
        let (again, attacker) = division_asked(&mut sim);
        cite!(["510.1e"]; assert_eq!(attacker, one, "{case}: the moment before the player began to assign"));
        cite!(["510.1e"]; assert_ne!(again, id, "{case}: a new question is a new request"));
        cite!(["510.1e"]; assert_eq!(sim.game().unwrap().players[1].life, 20, "{case}: no damage is dealt"));
        assign_legally(&mut sim, creatures, &case);
    }
}

/// What a seat was shown is its own once taken (the look of a scry, 701.22a), not part of the
/// game an illegal total returns to (510.1e): a seat that takes it between its divisions still
/// begins its assignment again.
#[test]
fn r510_1e_an_illegal_total_begins_again_after_the_seat_takes_what_it_was_shown() {
    use game::state::{Answer, Choice};
    let (mut sim, creatures @ [one, _, a, b]) = trampling_pair(2, |g| {
        g.put(0, BEARS, Zone::Library);
        let temple = g.put(0, TEMPLE, Zone::Hand);
        g.act(0, Move::PlayLand(temple)).unwrap();
        // "When this land enters, scry 1."
        all_pass(g);
        let Some(Decision::Choose { choice: Choice::Arrange { cards, .. }, .. }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        g.act(0, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap();
    });
    let (id, _) = division_asked(&mut sim);
    sim.respond(id, 0, trample(a, 2)).unwrap();
    let shown = sim.take_observations(0).unwrap();
    cite!(["701.22a"]; assert!(matches!(&shown[..], [look] if look.kind == game::observation::Kind::Looked && look.cards[0].name == "Grizzly Bears"), "{shown:?}"));
    let (id, _) = division_asked(&mut sim);
    cite!(["510.1e", "702.19b"]; refused("once Grizzly Bears is assigned lethal damage (702.19b)"): sim.respond(id, 0, trample(b, 1)));
    let (again, attacker) = division_asked(&mut sim);
    cite!(["510.1e"]; assert_eq!(attacker, one, "the moment before the player began to assign"));
    assert_ne!(again, id);
    cite!(["510.1e"]; assert!(sim.take_observations(0).unwrap().is_empty(), "what was taken is not shown again"));
    assign_legally(&mut sim, creatures, "after a take");
}

/// A player can concede at any time (104.3a), while another assigns combat damage too, and
/// leaves the game (800.4a). An illegal total then returns to the moment before the assigning
/// player began (510.1e) except for that concession, which nothing undoes.
#[test]
fn r510_1e_an_illegal_total_begins_again_after_another_player_concedes() {
    let (mut sim, creatures @ [one, two, a, b]) = trampling_pair(3, |_| {});
    let (id, _) = division_asked(&mut sim);
    sim.respond(id, 0, trample(a, 2)).unwrap();
    let (id, _) = division_asked(&mut sim);
    cite!(["104.3a"]; ok: sim.respond(id, 2, Move::Concede).map_err(|e| e.to_string()));
    let (id, attacker) = division_asked(&mut sim);
    cite!(["104.3a"]; assert_eq!(attacker, two, "the assignment goes on"));
    cite!(["510.1e", "702.19b"]; refused("once Grizzly Bears is assigned lethal damage (702.19b)"): sim.respond(id, 0, trample(b, 1)));
    let (again, attacker) = division_asked(&mut sim);
    cite!(["510.1e"]; assert_eq!(attacker, one, "the moment before the player began to assign"));
    assert_ne!(again, id);
    cite!(["104.3a", "800.4a"]; assert!(sim.game().unwrap().players[2].lost, "the concession stands"));
    cite!(["510.1e"]; assert_eq!(sim.game().unwrap().players[1].life, 20));
    assign_legally(&mut sim, creatures, "after a concession");
}

#[test]
fn r510_3_after_combat_damage_the_active_player_gets_priority() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    fight(&mut g, &[bears], &[]);
    assert_eq!(g.players[1].life, 18);
    cite!(["510.3"]; assert_eq!((g.decision.clone(), g.priority), (None, 0)));
    cite!(["510.3"]; refused("that player does not have priority"): g.act(1, Move::Pass));
    g.act(0, Move::Pass).unwrap();
    cite!(["510.3"]; assert_eq!((g.part, g.priority), (Part::CombatDamage, 1)));
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::EndOfCombat);
}

// -------------------------------------------------------------------------- keywords of combat

#[test]
fn r702_4_double_strike_deals_both_first_strike_and_regular_combat_damage() {
    let mut g = game();
    let ace = g.put(0, ACE, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    fight(&mut g, &[ace], &[(bears, ace)]);
    // in the first step only the ace: 1 to the bears
    cite!(["702.4b"]; assert_eq!(combat_damage(&g), vec![(ace, object(bears), 1)]));
    cite!(["702.4b"]; assert_eq!((g.obj(bears).damage, g.obj(ace).damage), (1, 0)));
    // in the second, the ace again and the bears with it
    all_pass(&mut g);
    cite!(["702.4"]; assert_eq!(g.part, Part::CombatDamage));
    cite!(["702.4b"]; assert_eq!(combat_damage(&g)[1..], [(ace, object(bears), 1), (bears, object(ace), 2)]));
    assert!(destroyed(&g, ace) && destroyed(&g, bears));
}

#[test]
fn r702_7_a_blocking_creature_with_first_strike_deals_its_damage_first() {
    let mut g = game();
    let ones = g.put(0, ONES, Zone::Battlefield);
    let wolves = g.put(1, WOLVES, Zone::Battlefield);
    fight(&mut g, &[ones], &[(wolves, ones)]);
    cite!(["702.7b"]; assert_eq!(combat_damage(&g), vec![(wolves, object(ones), 1)]));
    cite!(["702.7b"]; assert!(destroyed(&g, ones)));
    // the attacker is not there to deal its damage in the second step
    all_pass(&mut g);
    cite!(["702.7"]; assert_eq!(g.part, Part::CombatDamage));
    cite!(["702.7b"]; assert_eq!(combat_damage(&g).len(), 1));
    cite!(["702.7b"]; assert_eq!(g.obj(wolves).damage, 0));
    cite!(["702.7"]; assert!(g.battlefield.contains(&wolves)));
}

#[test]
fn r702_19f_a_creature_attacking_a_planeswalker_assigns_none_of_its_damage_to_the_defending_player() {
    // with trample, blocked: what is more than lethal to the blocker goes to the planeswalker,
    // even beyond its loyalty, and never to its controller
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let jace = jace(&mut g, 1);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(dreadmaw, object(jace))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(bears, dreadmaw)])).unwrap();
    all_pass(&mut g);
    let among = vec![object(bears), object(jace)];
    cite!(["702.19f"]; assert_eq!(g.decision, Some(Decision::Divide { player: 0, attacker: dreadmaw, among, amount: 6 })));
    cite!(["702.19f"]; refused("that damage is divided only among its combat recipients (510.1c–d)"): g.act(0, Move::Divide(vec![(object(bears), 2), (Target::Player(1), 4)])));
    g.act(0, Move::Divide(vec![(object(bears), 2), (object(jace), 4)])).unwrap();
    cite!(["404.3"]; ok: g.act(1, Move::Choose(game::state::Answer::Objects(vec![bears, jace]))));
    cite!(["702.19f"]; assert_eq!(g.players[1].life, 20));
    cite!(["702.19f"]; assert_eq!(g.obj(jace).zone, Zone::Gone));

    // the planeswalker removed from combat before damage: all of it to the blocker
    let mut g = game();
    let dreadmaw = g.put(0, DREADMAW, Zone::Battlefield);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let jace = self::jace(&mut g, 1);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(dreadmaw, object(jace))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(bears, dreadmaw)])).unwrap();
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![object(jace)]] })).unwrap();
    all_pass(&mut g);
    cite!(["702.19f"]; assert_eq!(g.obj(jace).zone, Zone::Gone));
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["702.19f"]; assert_eq!(g.decision, None));
    cite!(["702.19f"]; assert_eq!(combat_damage(&g), vec![(dreadmaw, object(bears), 6), (bears, object(dreadmaw), 2)]));
    cite!(["702.19f"]; assert_eq!(g.players[1].life, 20));

    // without trample and unblocked, the same: all to the planeswalker
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let jace = self::jace(&mut g, 1);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(giant, object(jace))])).unwrap();
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["702.19f"]; assert_eq!(combat_damage(&g), vec![(giant, object(jace), 3)]));
    cite!(["702.19f"]; assert_eq!(g.players[1].life, 20));
}
