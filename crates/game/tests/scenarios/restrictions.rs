//! Restrictions and requirements on attacking and blocking (CR 101.2, 508.1c–d, 509.1b), rule
//! by rule: what the text says can't happen or must, the declarations of combat obey.
//!
//! Every card here is read from its printed text; nothing is written for any of them. What
//! their sentences forbid or require is an event of the lexicon, and the declaration is checked
//! against it.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const BEARS: usize = 0;
const GIANT: usize = 1;
const ONES: usize = 2;
const SWIFTCLAW: usize = 3;
const MYR: usize = 4;
const HULKING: usize = 5;
const TALAS: usize = 6;
const STALKER: usize = 7;
const WARLOCK: usize = 8;
const RIDERS: usize = 9;
const BRIGAND: usize = 10;
const ORCS: usize = 11;
const CHAOS: usize = 12;
const INFILTRATE: usize = 13;
const MOUNTAIN: usize = 14;
const ISLAND: usize = 15;
const CERATOPS: usize = 16;
const BLACK: usize = 17;
const CASTAWAYS: usize = 18;
const COHORT: usize = 19;
const BEHEMOTH: usize = 20;
const MURDER: usize = 21;
const RATS: usize = 22;
const CINDERMAW: usize = 23;
const DUSK: usize = 24;
const NECTAR: usize = 25;
const FLOURISH: usize = 26;
const SHOCK: usize = 27;
const SWAMP: usize = 28;
const PLAINS: usize = 29;
const FOREST: usize = 30;
const GOLEM: usize = 31;
const CHOKE: usize = 32;
const CHILL: usize = 33;
const LAST_STAND: usize = 34;
const SKULLCRACK: usize = 35;
const MERCY: usize = 36;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Wandering Ones", "{U}", "Creature — Spirit", "", Some((1, 1))),
        card("Oreskos Swiftclaw", "{1}{W}", "Creature — Cat Warrior", "", Some((3, 1))),
        card("Alpha Myr", "{2}", "Artifact Creature — Myr", "", Some((2, 1))),
        card("Hulking Goblin", "{1}{R}", "Creature — Goblin", "This creature can't block.", Some((2, 2))),
        card("Talas Warrior", "{1}{U}{U}", "Creature — Human Pirate Warrior", "This creature can't be blocked.", Some((2, 2))),
        card(
            "Giltgrove Stalker",
            "{1}{G}",
            "Creature — Merfolk Warrior",
            "This creature can't be blocked by creatures with power 2 or less.",
            Some((2, 1)),
        ),
        card(
            "Dread Warlock",
            "{1}{B}{B}",
            "Creature — Human Wizard Warlock",
            "This creature can't be blocked except by black creatures.",
            Some((2, 2)),
        ),
        card("Norwood Riders", "{3}{G}", "Creature — Elf", "This creature can't be blocked by more than one creature.", Some((3, 3))),
        card("Goblin Brigand", "{1}{R}", "Creature — Goblin Warrior", "This creature attacks each combat if able.", Some((2, 2))),
        card(
            "Ironclaw Orcs",
            "{1}{R}",
            "Creature — Orc",
            "This creature can't block creatures with power 2 or greater.",
            Some((2, 2)),
        ),
        card("Chaos", "{2}{R}", "Instant", "Creatures can't block this turn.", None),
        card("Infiltrate", "{U}", "Instant", "Target creature can't be blocked this turn.", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card(
            "Rampaging Ceratops",
            "{4}{R}",
            "Creature — Dinosaur",
            "This creature can't be blocked except by three or more creatures.",
            Some((5, 4)),
        ),
        card("Dakmor Scorpion", "{1}{B}", "Creature — Scorpion", "", Some((2, 1))),
        card(
            "Desperate Castaways",
            "{1}{B}",
            "Creature — Human Pirate",
            "This creature can't attack unless you control an artifact.",
            Some((2, 3)),
        ),
        card(
            "Reckless Cohort",
            "{1}{R}",
            "Creature — Human Warrior Ally",
            "This creature attacks each combat if able unless you control another Ally.",
            Some((2, 2)),
        ),
        card(
            "Spearbreaker Behemoth",
            "{5}{G}{G}",
            "Creature — Beast",
            "Indestructible\n{1}: Target creature with power 5 or greater gains indestructible until end of turn.",
            Some((5, 5)),
        ),
        card("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        card(
            "Typhoid Rats",
            "{B}",
            "Creature — Rat",
            "Deathtouch (Any amount of damage this deals to a creature is enough to destroy it.)",
            Some((1, 1)),
        ),
        card(
            "Giant Cindermaw",
            "{2}{R}",
            "Creature — Dinosaur Beast",
            "Trample (This creature can deal excess combat damage to the player or planeswalker it's attacking.)\nPlayers can't gain life.",
            Some((4, 3)),
        ),
        card(
            "Knight of Dusk's Shadow",
            "{1}{B}",
            "Creature — Human Knight",
            "Menace (This creature can't be blocked except by two or more creatures.)\nYour opponents can't gain life.\n{1}{B}: This creature gets +1/+1 until end of turn.",
            Some((2, 2)),
        ),
        card("Sacred Nectar", "{1}{W}", "Sorcery", "You gain 4 life.", None),
        card(
            "Masterful Flourish",
            "{B}",
            "Instant",
            "Target creature you control gets +1/+0 and gains indestructible until end of turn. (Damage and effects that say \"destroy\" don't destroy it.)",
            None,
        ),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card(
            "Battered Golem",
            "{3}",
            "Artifact Creature — Golem",
            "This creature doesn't untap during your untap step.\nWhenever an artifact enters, you may untap this creature.",
            Some((3, 2)),
        ),
        card("Choke", "{2}{G}", "Enchantment", "Islands don't untap during their controllers' untap steps.", None),
        card(
            "Crippling Chill",
            "{2}{U}",
            "Instant",
            "Tap target creature. It doesn't untap during its controller's next untap step.\nDraw a card.",
            None,
        ),
        card(
            "Rhonas's Last Stand",
            "{G}{G}",
            "Sorcery",
            "Create a 5/4 green Snake creature token. Lands you control don't untap during your next untap step.",
            None,
        ),
        card(
            "Skullcrack",
            "{1}{R}",
            "Instant",
            "Players can't gain life this turn. Damage can't be prevented this turn. Skullcrack deals 3 damage to target player or planeswalker.",
            None,
        ),
        card("Angel's Mercy", "{2}{W}{W}", "Instant", "You gain 7 life.", None),
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

/// Player 0 attacks player 1 with these, from the precombat main phase; then both pass, so
/// that the game is at the declaration of blockers.
fn attack(g: &mut Game, attackers: &[ObjId]) {
    all_pass(g);
    all_pass(g);
    assert_eq!(g.part, Part::DeclareAttackers);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    all_pass(g);
    assert_eq!(g.part, Part::DeclareBlockers);
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r509_1b_a_creature_that_cant_block_is_not_declared_as_a_blocker() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let goblin = g.put(1, HULKING, Zone::Battlefield);
    attack(&mut g, &[attacker]);
    // the only creature the defending player has can't block: nobody is asked
    cite!(["509.1b"]; assert_eq!(g.decision, None));
    // with another that can, the declaration is theirs, and the Goblin still can't
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let goblin2 = g.put(1, HULKING, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[attacker]);
    assert_eq!(g.decision, Some(Decision::Blockers { player: 1 }));
    cite!(["509.1b"]; refused("Hulking Goblin can't block Grizzly Bears: Hulking Goblin says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(goblin2, attacker)])));
    g.act(1, Move::Block(vec![(bears, attacker)])).unwrap();
    let _ = goblin;
}

#[test]
fn r509_1b_a_creature_that_cant_be_blocked_is_not_blocked() {
    let mut g = game();
    let talas = g.put(0, TALAS, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    attack(&mut g, &[talas, bears]);
    cite!(["509.1b"]; refused("Hill Giant can't block Talas Warrior: Talas Warrior says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(blocker, talas)])));
    g.act(1, Move::Block(vec![(blocker, bears)])).unwrap();
    while g.part != Part::EndOfCombat {
        all_pass(&mut g);
    }
    cite!(["510.1b"]; assert_eq!(g.players[1].life, 18, "the unblocked Warrior dealt its 2 (510.1b)"));
}

#[test]
fn r509_1b_cant_be_blocked_by_is_said_of_the_blocker() {
    let mut g = game();
    let stalker = g.put(0, STALKER, Zone::Battlefield);
    let (small, big) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    attack(&mut g, &[stalker]);
    // "can't be blocked by creatures with power 2 or less"
    cite!(["509.1b"]; refused("Grizzly Bears can't block Giltgrove Stalker: Giltgrove Stalker says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(small, stalker)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(big, stalker)])));
}

#[test]
fn r509_1b_cant_be_blocked_except_by_is_said_of_everything_else() {
    let mut g = game();
    let warlock = g.put(0, WARLOCK, Zone::Battlefield);
    let (green, black) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, BLACK, Zone::Battlefield));
    attack(&mut g, &[warlock]);
    // "except by black creatures"
    cite!(["509.1b"]; refused("Grizzly Bears can't block Dread Warlock: Dread Warlock says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(green, warlock)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(black, warlock)])));
}

#[test]
fn r509_1b_except_by_three_or_more_creatures_counts_the_blockers() {
    let mut g = game();
    let ceratops = g.put(0, CERATOPS, Zone::Battlefield);
    let (a, b, c) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, SWIFTCLAW, Zone::Battlefield), g.put(1, MYR, Zone::Battlefield));
    attack(&mut g, &[ceratops]);
    cite!(["509.1b"]; refused("Grizzly Bears can't block Rampaging Ceratops: Rampaging Ceratops says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(a, ceratops)])));
    cite!(["509.1b"]; refused("Grizzly Bears can't block Rampaging Ceratops: Rampaging Ceratops says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(a, ceratops), (b, ceratops)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(a, ceratops), (b, ceratops), (c, ceratops)])));
}

#[test]
fn r509_1b_by_more_than_one_creature_counts_the_blockers() {
    let mut g = game();
    let riders = g.put(0, RIDERS, Zone::Battlefield);
    let (a, b) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    attack(&mut g, &[riders]);
    cite!(["509.1b"]; refused("Grizzly Bears can't block Norwood Riders: Norwood Riders says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(a, riders), (b, riders)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(b, riders)])));
}

#[test]
fn r509_1b_cant_block_what_is_said_of_the_attacker() {
    let mut g = game();
    let (small, big) = (g.put(0, ONES, Zone::Battlefield), g.put(0, GIANT, Zone::Battlefield));
    let orcs = g.put(1, ORCS, Zone::Battlefield);
    attack(&mut g, &[small, big]);
    // "can't block creatures with power 2 or greater"
    cite!(["509.1b"]; refused("Ironclaw Orcs can't block Hill Giant: Ironclaw Orcs says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(orcs, big)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(orcs, small)])));
}

#[test]
fn r508_1d_a_creature_that_attacks_if_able_attacks() {
    let mut g = game();
    let brigand = g.put(0, BRIGAND, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.decision, Some(Decision::Attackers { player: 0 }));
    cite!(["508.1d"]; refused("Goblin Brigand attacks if able (508.1d)"): g.act(0, Move::Attack(vec![])));
    cite!(["508.1d"]; refused("Goblin Brigand attacks if able (508.1d)"): g.act(0, Move::Attack(vec![(bears, Target::Player(1))])));
    cite!(["508.1d"]; ok: g.act(0, Move::Attack(vec![(brigand, Target::Player(1))])));
    // tapped, it is not able: the requirement asks nothing (508.1d, 508.1a)
    let mut g = game();
    let brigand = g.put(0, BRIGAND, Zone::Battlefield);
    g.tap(brigand);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1d", "508.1a"]; assert_eq!((g.part, g.decision.clone()), (Part::DeclareAttackers, None)));
}

#[test]
fn r508_1c_a_restriction_with_a_condition_is_obeyed_while_the_condition_makes_it_so() {
    // "This creature can't attack unless you control an artifact."
    let mut g = game();
    let castaways = g.put(0, CASTAWAYS, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1c"]; assert_eq!((g.part, g.decision.clone()), (Part::DeclareAttackers, None), "it can't attack: nobody is asked"));
    // an artifact an opponent controls is not one "you control"
    let mut g = game();
    let castaways2 = g.put(0, CASTAWAYS, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.put(1, MYR, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1c"]; refused("Desperate Castaways can't attack that (508.1c)"): g.act(0, Move::Attack(vec![(castaways2, Target::Player(1))])));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    // with an artifact of its controller's the restriction is not in force
    let mut g = game();
    let castaways3 = g.put(0, CASTAWAYS, Zone::Battlefield);
    g.put(0, MYR, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1c"]; ok: g.act(0, Move::Attack(vec![(castaways3, Target::Player(1))])));
    let _ = castaways;
}

#[test]
fn r508_1d_a_requirement_with_a_condition_asks_only_while_the_condition_makes_it_so() {
    // "This creature attacks each combat if able unless you control another Ally."
    let mut g = game();
    let cohort = g.put(0, COHORT, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1d"]; refused("Reckless Cohort attacks if able (508.1d)"): g.act(0, Move::Attack(vec![])));
    cite!(["508.1d"]; ok: g.act(0, Move::Attack(vec![(cohort, Target::Player(1))])));
    // another Ally: itself is not "another", a second Cohort is, for each of them
    let mut g = game();
    g.put(0, COHORT, Zone::Battlefield);
    g.put(0, COHORT, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["508.1d"]; ok: g.act(0, Move::Attack(vec![])));
}

#[test]
fn r611_2c_a_rule_a_spell_puts_in_force_applies_to_whatever_fits_it_until_the_turn_ends() {
    let mut g = game();
    let attacker = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    let lands: Vec<ObjId> = (0..3).map(|_| g.put(0, MOUNTAIN, Zone::Battlefield)).collect();
    let chaos = g.put(0, CHAOS, Zone::Hand);
    for _ in 0..12 {
        g.put(0, MOUNTAIN, Zone::Library);
        g.put(1, MOUNTAIN, Zone::Library);
    }
    for l in lands {
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    // "Creatures can't block this turn."
    g.act(0, paid("RRR", Move::Cast { card: chaos, targets: vec![] })).unwrap();
    all_pass(&mut g);
    attack(&mut g, &[attacker]);
    cite!(["509.1b"]; assert_eq!(g.decision, None, "no creature can block: nobody is asked"));
    while g.part != Part::EndOfCombat {
        all_pass(&mut g);
    }
    assert_eq!(g.players[1].life, 18);
    // it ended in the cleanup step (514.2): in player 0's next turn the Giant blocks
    while g.turn < 3 {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    while g.part != Part::PrecombatMain {
        all_pass(&mut g);
    }
    attack(&mut g, &[attacker]);
    cite!(["514.2"]; ok: g.act(1, Move::Block(vec![(blocker, attacker)])));
}

#[test]
fn r611_2a_a_rule_said_of_a_target_holds_for_that_creature_this_turn() {
    let mut g = game();
    let (a, b) = (g.put(0, BEARS, Zone::Battlefield), g.put(0, GIANT, Zone::Battlefield));
    let blocker = g.put(1, GIANT, Zone::Battlefield);
    let island = g.put(0, ISLAND, Zone::Battlefield);
    let infiltrate = g.put(0, INFILTRATE, Zone::Hand);
    g.act(0, Move::Activate { source: island, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    // "Target creature can't be blocked this turn."
    g.act(0, paid("U", Move::Cast { card: infiltrate, targets: vec![vec![Target::Object(a)]] })).unwrap();
    all_pass(&mut g);
    attack(&mut g, &[a, b]);
    cite!(["509.1b"]; refused("Hill Giant can't block Grizzly Bears: Infiltrate says it can't, or not that way (509.1b)"): g.act(1, Move::Block(vec![(blocker, a)])));
    cite!(["509.1b"]; ok: g.act(1, Move::Block(vec![(blocker, b)])));
}

/// Player 0 casts this card with the mana of these lands at these targets, and it resolves.
fn cast(g: &mut Game, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    all_pass(g);
}

#[test]
fn r101_2_an_effect_that_says_something_cant_happen_takes_precedence() {
    // "Players can't gain life." against "You gain 4 life.": nothing is gained (119.7)
    let mut g = game();
    g.put(1, CINDERMAW, Zone::Battlefield);
    cast(&mut g, NECTAR, &[PLAINS, PLAINS], vec![], "WW");
    cite!(["101.2"]; assert_eq!(g.players[0].life, 20));
    cite!(["101.2"]; assert!(!g.log.iter().any(|e| matches!(e, Event::LifeGained { .. })), "the event did not happen"));
    // "Your opponents can't gain life." is of the opponents of its controller
    for (controller, life) in [(0, 24), (1, 20)] {
        let mut g = game();
        g.put(controller, DUSK, Zone::Battlefield);
        cast(&mut g, NECTAR, &[PLAINS, PLAINS], vec![], "WW");
        cite!(["101.2", "109.5"]; case: format!("controller {controller}"); assert_eq!(g.players[0].life, life, "the Knight is player {controller}'s"));
    }
}

#[test]
fn r119_7_players_who_cant_gain_life_this_turn_gain_none_until_the_turn_ends() {
    // "Players can't gain life this turn. Damage can't be prevented this turn. Skullcrack
    // deals 3 damage to target player or planeswalker."
    let mut g = long_game();
    cast(&mut g, SKULLCRACK, &[MOUNTAIN, MOUNTAIN], vec![vec![Target::Player(1)]], "RR");
    assert_eq!(g.players[1].life, 17);
    cast(&mut g, MERCY, &[PLAINS, PLAINS, PLAINS, PLAINS], vec![], "WWWW");
    cite!(["101.2"]; assert_eq!(g.players[0].life, 20, "\"You gain 7 life.\" in that turn"));
    // "this turn" ends in the cleanup step (514.2)
    to_upkeep(&mut g, 2);
    g.act(1, Move::Pass).unwrap();
    cast(&mut g, MERCY, &[PLAINS, PLAINS, PLAINS, PLAINS], vec![], "WWWW");
    cite!(["514.2"]; assert_eq!(g.players[0].life, 27));
}

#[test]
fn r615_12_an_effect_says_that_damage_cant_be_prevented_this_turn() {
    // a rule in force like any other the spell states: what prevents damage asks it
    let mut g = long_game();
    assert!(!g.unpreventable());
    cast(&mut g, SKULLCRACK, &[MOUNTAIN, MOUNTAIN], vec![vec![Target::Player(1)]], "RR");
    cite!(["615.12"]; assert!(g.unpreventable()));
    assert_eq!(g.players[1].life, 17);
    to_upkeep(&mut g, 2);
    cite!(["514.2"]; assert!(!g.unpreventable(), "until the turn ended (514.2)"));
}

#[test]
fn r615_12_damage_that_cant_be_prevented_is_refused_where_it_is_of_some_damage_or_lasts_longer() {
    let refused = |text: &str| game::play::check(&CardDef::new("Witness", "{0}", "Instant", text, None).unwrap()).unwrap_err().0;
    // no window: nothing ends it, and a resolved spell keeps a rule only for a turn
    assert_eq!(refused("Damage can't be prevented."), "a rule that lasts other than this turn");
    assert_eq!(refused("Damage that would be dealt by target creature can't be prevented this turn."), "unpreventable: of some damage only");
    assert_eq!(refused("Damage can't be prevented or dealt instead to another permanent or player this turn."), "unpreventable: damage that can't be redirected");
}

#[test]
fn r608_2c_damage_cant_be_prevented_only_where_its_condition_held_as_it_resolved() {
    // Wild Slash's ferocious sentence: the rule is put in force by the branch taken
    let text = "If you control a creature with power 4 or greater, damage can't be prevented this turn.";
    for (big, in_force) in [(true, true), (false, false)] {
        let mut g = game();
        let at = g.defs.len();
        let defs = std::sync::Arc::make_mut(&mut g.defs);
        defs.push(CardDef::new("Slash", "{0}", "Instant", text, None).unwrap());
        defs.push(CardDef::new("Giant", "{0}", "Creature — Giant", "", Some((4, 4))).unwrap());
        game::play::check(&g.defs[at]).unwrap();
        if big { g.put(0, at + 1, Zone::Battlefield); }
        let card = g.put(0, at, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        all_pass(&mut g);
        assert_eq!(g.unpreventable(), in_force);
    }
}

#[test]
fn r702_12b_a_permanent_with_indestructible_cant_be_destroyed() {
    let mut g = game();
    let behemoth = g.put(1, BEHEMOTH, Zone::Battlefield);
    // by an effect that says "destroy"
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(behemoth)]], "BBB");
    cite!(["702.12b"]; assert_eq!(g.obj(behemoth).zone, Zone::Battlefield));
    // nor by lethal damage: it ignores the state-based action that checks for it (704.5g),
    // and the damage stays marked
    for _ in 0..3 {
        cast(&mut g, SHOCK, &[MOUNTAIN], vec![vec![Target::Object(behemoth)]], "R");
    }
    cite!(["702.12b", "704.5g"]; assert_eq!((g.obj(behemoth).damage, g.obj(behemoth).zone), (6, Zone::Battlefield)));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Destroyed { .. })));
    // nor by damage from a source with deathtouch (704.5h)
    let mut g = game();
    let rats = g.put(0, RATS, Zone::Battlefield);
    let behemoth = g.put(1, BEHEMOTH, Zone::Battlefield);
    attack(&mut g, &[rats]);
    g.act(1, Move::Block(vec![(behemoth, rats)])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["702.12b", "704.5h"]; assert_eq!((g.obj(behemoth).damage, g.obj(behemoth).zone), (1, Zone::Battlefield)));
    assert_eq!(g.obj(rats).zone, Zone::Gone, "the Rats were dealt 5");
}

#[test]
fn r702_12b_a_creature_that_gains_indestructible_has_what_its_rule_says() {
    // "Target creature you control gets +1/+0 and gains indestructible until end of turn."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, FLOURISH, &[SWAMP], vec![vec![Target::Object(bears)]], "B");
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(bears)]], "BBB");
    cite!(["702.12b"]; assert_eq!(g.obj(bears).zone, Zone::Battlefield));
    // without it the same spell destroys the same creature
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Object(bears)]], "BBB");
    assert_eq!(g.obj(bears).zone, Zone::Gone);
}

/// A game with cards to draw, so that turns can go by.
fn long_game() -> Game {
    let mut g = game();
    for player in [0, 1] {
        for _ in 0..6 {
            g.put(player, MOUNTAIN, Zone::Library);
        }
    }
    g
}

/// Pass, attacking with nothing, until the upkeep of this turn: its untap step has happened.
fn to_upkeep(g: &mut Game, turn: u32) {
    while !(g.turn == turn && g.part == Part::Upkeep) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Discard { player, count }) => {
                let cards = g.players[player].hand[..count].to_vec();
                g.act(player, Move::Discard(cards)).unwrap()
            }
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

#[test]
fn r502_3_an_effect_can_keep_a_permanent_from_untapping_in_its_controllers_untap_step() {
    // "This creature doesn't untap during your untap step.": the rest of its controller's
    // permanents untap
    let mut g = long_game();
    let golem = g.put(0, GOLEM, Zone::Battlefield);
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.tap(golem);
    g.tap(land);
    to_upkeep(&mut g, 3);
    cite!(["502.3"]; assert!(g.obj(golem).tapped));
    cite!(["502.3"]; assert!(!g.obj(land).tapped));
    // "Islands don't untap during their controllers' untap steps.": each player's Islands, in
    // that player's untap step, whoever controls the enchantment
    let mut g = long_game();
    g.put(1, CHOKE, Zone::Battlefield);
    let (mine, theirs) = (g.put(0, ISLAND, Zone::Battlefield), g.put(1, ISLAND, Zone::Battlefield));
    let mountain = g.put(1, MOUNTAIN, Zone::Battlefield);
    for land in [mine, theirs, mountain] {
        g.tap(land);
    }
    to_upkeep(&mut g, 3);
    cite!(["502.3"]; assert_eq!((g.obj(mine).tapped, g.obj(theirs).tapped, g.obj(mountain).tapped), (true, true, false)));
}

#[test]
fn r611_2a_an_effect_that_lasts_through_a_next_untap_step_ends_with_that_step() {
    // "Tap target creature. It doesn't untap during its controller's next untap step.": the
    // opponent's creature stays tapped through their next untap step, and untaps in the one
    // after
    let mut g = long_game();
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, CHILL, &[ISLAND, ISLAND, ISLAND], vec![vec![Target::Object(theirs)]], "UUU");
    assert!(g.obj(theirs).tapped);
    to_upkeep(&mut g, 2);
    cite!(["611.2a", "502.3"]; assert!(g.obj(theirs).tapped, "its controller's next untap step"));
    to_upkeep(&mut g, 4);
    cite!(["611.2a"]; assert!(!g.obj(theirs).tapped, "the one after"));
    // said of the caster's own creature, the next untap step of its controller is two turns on
    let mut g = long_game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, CHILL, &[ISLAND, ISLAND, ISLAND], vec![vec![Target::Object(mine)]], "UUU");
    to_upkeep(&mut g, 3);
    cite!(["611.2a"]; assert!(g.obj(mine).tapped));
    to_upkeep(&mut g, 5);
    cite!(["611.2a"]; assert!(!g.obj(mine).tapped));
    // "Lands you control don't untap during your next untap step.": the lands its player
    // controls in that step (611.2c), and that step only
    let mut g = long_game();
    cast(&mut g, LAST_STAND, &[FOREST, FOREST], vec![], "GG");
    let forests: Vec<ObjId> = g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Forest").collect();
    let theirs = g.put(1, FOREST, Zone::Battlefield);
    g.tap(theirs);
    to_upkeep(&mut g, 2);
    assert!(!g.obj(theirs).tapped, "not the opponent's");
    to_upkeep(&mut g, 3);
    cite!(["611.2a", "502.3"]; assert!(forests.iter().all(|f| g.obj(*f).tapped)));
    to_upkeep(&mut g, 5);
    cite!(["611.2a"]; assert!(forests.iter().all(|f| !g.obj(*f).tapped)));
}
