//! Attack requirements in a turn with more than one combat phase (500.8, 508.1d). A requirement
//! of "this combat" ends with that combat phase (500.5a). One an effect puts in force "this
//! turn" applies in each declare attackers step of the turn (508.1d), and a static "attacks
//! each combat if able" in every one. A static "attacks each turn if able" is obeyed by
//! attacking once that turn (ruling 0e5bb944856b8e46), which needs the creature's attack
//! history: it is refused, not read as "each combat".

use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const LAND: usize = 0;
const SEIZE: usize = 1;
const BEARS: usize = 2;
const THIS_COMBAT: usize = 3;
const THIS_TURN: usize = 4;
const BRIGAND: usize = 5;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        crate::support::oracle("Wastes"),
        crate::support::oracle("Seize the Day"),
        crate::support::oracle("Grizzly Bears"),
        card("Goad Now", "{R}", "Instant", "Target creature attacks this combat if able.", None),
        card("Goad Today", "{R}", "Instant", "Target creature attacks this turn if able.", None),
        card("Brigand", "{1}{R}", "Creature — Goblin Warrior", "This creature attacks each combat if able.", Some((2, 2))),
    ]
}

/// Player 0's third turn, precombat main phase, with Bears that can attack.
fn game() -> (Game, ObjId) {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 {
        for _ in 0..12 {
            g.put(p, LAND, Zone::Library);
        }
    }
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, |g| g.turn == 3 && g.part == Part::PrecombatMain && g.decision.is_none() && g.stack.is_empty());
    (g, bears)
}

fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, units: &str) {
    let card = g.put(player, def, Zone::Hand);
    g.players[player].pool.colors = [9; 5];
    g.players[player].pool.colorless = 9;
    g.act(player, paid(units, Move::Cast { card, targets })).unwrap();
    while !g.stack.is_empty() {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.players[player].pool = Default::default();
}

fn until(g: &mut Game, done: impl Fn(&Game) -> bool) {
    for _ in 0..500 {
        if done(g) {
            return;
        }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(other) => panic!("unexpected decision {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("never reached");
}

/// The next declare attackers decision: whether this creature must attack in it.
fn must_attack(g: &mut Game, creature: ObjId) -> bool {
    until(g, |g| matches!(g.decision, Some(Decision::Attackers { .. })));
    let Some((_, Asked::Attackers(able))) = g.asked() else { panic!("{:?}", g.asked()) };
    able.iter().find(|a| a.creature == creature).expect("able to attack").must
}

/// Declare this creature the only attacker, then go on to the main phase that follows.
fn attack(g: &mut Game, creature: ObjId) {
    g.act(0, Move::Attack(vec![(creature, Target::Player(1))])).unwrap();
    until(g, |g| g.part == Part::PostcombatMain && g.decision.is_none() && g.stack.is_empty());
}

fn decline(g: &mut Game) {
    g.act(0, Move::Attack(vec![])).unwrap();
    until(g, |g| g.part == Part::PostcombatMain && g.decision.is_none() && g.stack.is_empty());
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        game::play::check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r500_5a_attacks_this_combat_if_able_binds_that_combat_only() {
    let (mut g, bears) = game();
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    // put in force before any combat: the next one, the added combat
    cast(&mut g, 0, THIS_COMBAT, vec![vec![Target::Object(bears)]], "R");
    cite!(["500.5a"]; assert!(must_attack(&mut g, bears), "the added combat, the first this turn"));
    attack(&mut g, bears);
    g.set_tapped(bears, false);
    cite!(["500.5a"]; assert!(!must_attack(&mut g, bears), "ended with its combat phase (500.5a)"));
    decline(&mut g);
    // put in force during a combat: that combat
    let (mut g, bears) = game();
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    until(&mut g, |g| g.part == Part::BeginningOfCombat && g.stack.is_empty() && g.decision.is_none());
    cast(&mut g, 0, THIS_COMBAT, vec![vec![Target::Object(bears)]], "R");
    cite!(["500.5a"]; assert!(must_attack(&mut g, bears)));
    attack(&mut g, bears);
    g.set_tapped(bears, false);
    cite!(["500.5a"]; assert!(!must_attack(&mut g, bears)));
}

#[test]
fn r508_1d_attacks_this_turn_if_able_applies_in_each_combat_of_the_turn() {
    let (mut g, bears) = game();
    cast(&mut g, 0, THIS_TURN, vec![vec![Target::Object(bears)]], "R");
    cite!(["508.1d"]; assert!(must_attack(&mut g, bears)));
    attack(&mut g, bears);
    // Seize the Day untaps it: it must attack again in the added combat
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    cite!(["508.1d"]; assert!(must_attack(&mut g, bears), "a requirement for the turn applies in each declare attackers step (508.1d)"));
    attack(&mut g, bears);
    cite!(["508.1d"]; assert_eq!(g.players[1].life, 16));
    until(&mut g, |g| g.turn == 5 && g.part == Part::PrecombatMain);
    cite!(["508.1d"]; assert!(!must_attack(&mut g, bears), "and ended with the turn"));
}

#[test]
fn r508_1d_attacks_each_combat_if_able_applies_in_every_combat() {
    let (mut g, _) = game();
    let brigand = g.put(0, BRIGAND, Zone::Battlefield);
    cite!(["508.1d"]; assert!(must_attack(&mut g, brigand)));
    attack(&mut g, brigand);
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(brigand)]], "CCCR");
    cite!(["508.1d"]; assert!(must_attack(&mut g, brigand)));
}

#[test]
fn r508_1d_a_static_attacks_each_turn_if_able_is_refused_not_read_as_each_combat() {
    let d = CardDef::new("Eager", "{1}{R}", "Creature — Goblin", "This creature attacks each turn if able.", Some((2, 2))).expect("read");
    let why = game::play::check(&d).expect_err("refused").0;
    assert!(why.contains("once each turn (needs attack history)"), "{why}");
}
