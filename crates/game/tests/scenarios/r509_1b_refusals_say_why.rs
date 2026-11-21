//! A refused block says which restriction it disobeys and its rule (509.1b): flying (702.9b),
//! menace (702.111b), protection (702.16f), an effect. A refused
//! block of a flyer must not say only "can't block Sunderflock that way (509.1b)".
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }

/// Player 0 attacks with `attacker`; returns at player 1's declaration of blockers. Each game
/// gives player 1 a creature that can block, so the declaration is asked for (509.1).
fn at_blocks(cards: Vec<CardDef>, attacker: usize, blockers: &[usize]) -> (Game, ObjId, Vec<ObjId>) {
    for d in &cards { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(cards, 2, 20);
    let a = g.put(0, attacker, Zone::Battlefield);
    let bs: Vec<ObjId> = blockers.iter().map(|b| g.put(1, *b, Zone::Battlefield)).collect();
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![(a, Target::Player(1))])).unwrap(),
            Some(Decision::Blockers { .. }) => return (g, a, bs),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no declaration of blockers")
}

#[test]
fn a_flyer_blocked_by_a_ground_creature_names_flying() {
    let (mut g, a, bs) = at_blocks(vec![
        c("Sunderflock", "{0}", "Creature — Elemental", "Flying", Some((5, 5))),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Spider", "{0}", "Creature — Spider", "Reach", Some((1, 3))),
    ], 0, &[1, 2]);
    cite!(["509.1b", "702.9b"]; assert_eq!(g.act(1, Move::Block(vec![(bs[0], a)])), Err("Bear can't block Sunderflock: Sunderflock has flying (702.9b, 509.1b)".into())));
}

#[test]
fn a_menace_creature_blocked_by_one_names_menace() {
    let (mut g, a, bs) = at_blocks(vec![
        c("Brute", "{0}", "Creature — Ogre", "Menace", Some((3, 3))),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ], 0, &[1, 1]);
    cite!(["509.1b", "702.111b"]; assert_eq!(g.act(1, Move::Block(vec![(bs[0], a)])), Err("Bear can't block Brute: Brute has menace (702.111b, 509.1b)".into())));
    cite!(["702.111b"]; ok: g.act(1, Move::Block(vec![(bs[0], a), (bs[1], a)])));
}

#[test]
fn protection_names_its_rule() {
    let (mut g, a, bs) = at_blocks(vec![
        c("Knight", "{0}", "Creature — Knight", "Protection from green", Some((2, 2))),
        c("Bear", "{G}", "Creature — Bear", "", Some((2, 2))),
        c("Golem", "{0}", "Artifact Creature — Golem", "", Some((2, 2))),
    ], 0, &[1, 2]);
    cite!(["509.1b", "702.16f"]; assert_eq!(g.act(1, Move::Block(vec![(bs[0], a)])), Err("Bear can't block Knight: Knight has protection from a quality Bear has (702.16f, 509.1b)".into())));
}

#[test]
fn an_effect_names_its_source() {
    let (mut g, a, bs) = at_blocks(vec![
        c("Giant", "{0}", "Creature — Giant", "", Some((3, 3))),
        c("Goblin", "{0}", "Creature — Goblin", "This creature can't block.", Some((2, 2))),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
    ], 0, &[1, 2]);
    assert_eq!(g.act(1, Move::Block(vec![(bs[0], a)])), Err("Goblin can't block Giant: Goblin says it can't, or not that way (509.1b)".into()));
}
