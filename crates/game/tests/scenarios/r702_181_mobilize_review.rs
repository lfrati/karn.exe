//! Mobilize (702.181).
//!
//! 702.181a: "Mobilize N" means "Whenever this creature attacks, create N 1/1 red Warrior
//! creature tokens. Those tokens enter tapped and attacking. Sacrifice them at the beginning of
//! the next end step." 701.21a: a player can't sacrifice something that isn't a permanent they
//! control. Ruling (2025-04-04): the tokens were never declared as attackers, so abilities that
//! trigger whenever a creature attacks don't trigger for them.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const PLAINS: usize = 0;
const BRIGADE: usize = 1;
const PACKBEASTS: usize = 2;
const STEAL: usize = 3;
const MURDER: usize = 4;
const HORN: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Plains", "Shock Brigade", "Dalkovan Packbeasts"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    let card = |name, types, text| CardDef::new(name, "{0}", types, text, None).unwrap();
    defs.push(card("Steal", "Instant", "Gain control of target creature until end of turn."));
    defs.push(card("Murder", "Instant", "Destroy target creature."));
    defs.push(card("Horn", "Artifact", "Whenever a creature you control attacks, you gain 1 life."));
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, PLAINS, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn declare(g: &mut Game, attackers: &[ObjId]) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() || matches!(g.decision, Some(Decision::Order { .. })) {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn to(g: &mut Game, part: Part) {
    while g.part != part {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn warriors(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).token() && g.obj(*o).chars.name.contains("Warrior")).collect()
}

// Rules: 702.181a, 701.21a, 603.7e, 514.2
// Given: Dalkovan Packbeasts attack and mobilize 3 makes three Warriors.
// When: player 1 gains control of one Warrior until end of turn after combat.
// Expect: at the beginning of the end step, player 0 sacrifices the two Warriors they control;
// the stolen one stays (and returns to player 0 at cleanup).
// Why: the delayed "sacrifice them" is player 0's (603.7e), and a player can't sacrifice a
// permanent they don't control (701.21a).
#[test]
fn r702_181a_a_warrior_another_player_controls_at_the_end_step_is_not_sacrificed() {
    let mut g = game();
    let beasts = g.put(0, PACKBEASTS, Zone::Battlefield);
    let steal = g.put(1, STEAL, Zone::Hand);
    declare(&mut g, &[beasts]);
    settle(&mut g);
    let tokens = warriors(&g);
    assert_eq!(tokens.len(), 3);
    to(&mut g, Part::PostcombatMain);
    assert_eq!((g.priority, g.stack.is_empty()), (0, true));
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(tokens[0])]] }).unwrap();
    settle(&mut g);
    assert_eq!(g.controller(tokens[0]), 1, "setup: stolen");
    to(&mut g, Part::End);
    settle(&mut g);
    cite!(["702.181a"]; assert!(tokens[1..].iter().all(|t| g.obj(*t).zone == Zone::Gone), "player 0's two are sacrificed"));
    cite!(["701.21a", "702.181a"]; assert_eq!((g.obj(tokens[0]).zone, g.controller(tokens[0])), (Zone::Battlefield, 1)));
}

// Rules: 702.181a, 508.3a, 508.4
// Given: "Whenever a creature you control attacks, you gain 1 life." and Shock Brigade.
// When: the Brigade attacks and mobilize 1 makes a Warrior attacking.
// Expect: one life gained, for the Brigade; none for the Warrior.
// Why: tokens put onto the battlefield attacking were never declared as attackers (508.4),
// so "whenever ... attacks" does not trigger for them (ruling of Shock Brigade).
#[test]
fn r702_181a_tokens_entering_attacking_do_not_trigger_attack_abilities() {
    let mut g = game();
    let brigade = g.put(0, BRIGADE, Zone::Battlefield);
    g.put(0, HORN, Zone::Battlefield);
    declare(&mut g, &[brigade]);
    settle(&mut g);
    settle(&mut g);
    assert_eq!(warriors(&g).len(), 1);
    cite!(["508.3a", "508.4"]; assert_eq!(g.players[0].life, 21, "the Brigade's trigger only"));
}

// Rules: 702.181a, 603.7c, 113.7a
// Given: Shock Brigade attacks; with its mobilize trigger on the stack, player 1 destroys it.
// When: the trigger resolves and the turn reaches its end step.
// Expect: the Warrior is still created attacking, and still sacrificed at the end step.
// Why: a triggered ability exists independently of its source (113.7a); the delayed trigger
// it creates does not depend on the mobilize creature either.
#[test]
fn r702_181a_the_mobilize_creature_gone_before_its_trigger_resolves() {
    let mut g = game();
    let brigade = g.put(0, BRIGADE, Zone::Battlefield);
    let murder = g.put(1, MURDER, Zone::Hand);
    declare(&mut g, &[brigade]);
    assert_eq!((g.priority, g.stack.len()), (0, 1), "setup: the mobilize trigger");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: murder, targets: vec![vec![Target::Object(brigade)]] }).unwrap();
    settle(&mut g);
    assert_ne!(g.obj(brigade).zone, Zone::Battlefield);
    let tokens = warriors(&g);
    cite!(["702.181a", "113.7a"]; assert_eq!(tokens.len(), 1));
    cite!(["702.181a"]; assert!(g.obj(tokens[0]).tapped && g.combat.attackers.contains(&(tokens[0], Target::Player(1)))));
    to(&mut g, Part::End);
    settle(&mut g);
    cite!(["702.181a"]; assert_eq!(g.obj(tokens[0]).zone, Zone::Gone));
}
