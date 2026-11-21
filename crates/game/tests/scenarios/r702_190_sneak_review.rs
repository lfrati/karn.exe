//! Sneak (702.190).
//!
//! 702.190a: "Sneak [cost]" means "Any time you could cast an instant during your declare
//! blockers step, you may cast this spell by paying [cost] and returning an unblocked creature
//! you control to its owner's hand rather than paying this spell's mana cost."
//! 702.190b: "A permanent spell whose sneak cost was paid enters the battlefield tapped and
//! attacking (see rule 506.3a). It will be attacking the same player, planeswalker, or battle
//! as the creature that was returned to its owner's hand to pay the sneak cost of the spell
//! that became that permanent."
//! 509.1h: an attacking creature with no blocking creatures is unblocked; one that was blocked
//! remains blocked even if all its blockers are removed from combat.
//! Ruling (2026-01-27): creatures entering attacking were never declared as attackers, so
//! "whenever ... attacks" abilities don't trigger (508.3a, 508.4).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const WASTES: usize = 0;
const BEARS: usize = 1;
const NINJA: usize = 2;
const WALKER: usize = 3;
const BANNER: usize = 4;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Wastes", "", "Basic Land", "", None),
        c("Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        c("Ninja", "{3}{B}", "Creature — Ninja", "Sneak {B}\nWhenever this creature attacks, you gain 2 life.", Some((3, 1))),
        CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Jace", "+1: You gain 1 life.", 5).unwrap(),
        c("Banner", "{0}", "Enchantment", "Whenever a creature you control attacks, you gain 1 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, WASTES, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); } }
fn attack(g: &mut Game, attacks: Vec<(ObjId, Target)>) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attacks)).unwrap();
    // any attack triggers resolve
    while !g.stack.is_empty() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
}
fn block(g: &mut Game, blocks: Vec<(ObjId, ObjId)>) {
    pass(g);
    while let Some(Decision::Blockers { player }) = g.decision {
        let mine = blocks.iter().filter(|(b, _)| g.obj(*b).controller == player).cloned().collect();
        g.act(player, Move::Block(mine)).unwrap();
    }
    assert_eq!((g.part, g.priority), (Part::DeclareBlockers, 0));
}
fn sneak(g: &mut Game, card: ObjId, returned: ObjId) -> Result<(), String> {
    // the Ninja's first row is its sneak keyword
    g.act(0, crate::support::paid("B",Move::CastFor(CastCost::Alternative(0), Box::new(Move::WithCosts(vec![vec![returned]], Box::new(Move::Cast { card, targets: vec![] })))))).map_err(|e| e.to_string())
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); } }
fn ninjas(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).card == Some(NINJA)).collect() }

// Rules: 702.190b, 506.3a, 508.1b
// Given: one Bears attacks player 1, another attacks player 1's planeswalker; neither blocked.
// Expect: the Ninja sneaked by returning the Bears that attacked the planeswalker enters tapped
// and attacking that planeswalker, not player 1; its combat damage removes loyalty.
#[test]
fn r702_190b_it_attacks_the_planeswalker_the_returned_creature_attacked() {
    let mut g = game();
    let walker = g.put(1, WALKER, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 5);
    let at_player = g.put(0, BEARS, Zone::Battlefield);
    let at_walker = g.put(0, BEARS, Zone::Battlefield);
    let ninja = g.put(0, NINJA, Zone::Hand);
    attack(&mut g, vec![(at_player, Target::Player(1)), (at_walker, Target::Object(walker))]);
    block(&mut g, vec![]);
    g.players[0].pool.colors[2] = 1;
    sneak(&mut g, ninja, at_walker).unwrap();
    resolve(&mut g);
    let [now] = ninjas(&g)[..] else { panic!("{:?}", ninjas(&g)) };
    cite!(["702.190b"]; assert!(g.obj(now).tapped));
    cite!(["702.190b", "506.3a"]; assert!(g.combat.attackers.contains(&(now, Target::Object(walker))), "{:?}", g.combat.attackers));
    cite!(["702.190b"]; assert!(!g.combat.attackers.contains(&(now, Target::Player(1)))));
    while g.part != Part::EndOfCombat { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    cite!(["510.1c", "120.3c"]; assert_eq!((g.players[1].life, g.obj(walker).counters.get("loyalty").copied()), (18, Some(2))));
}

// Rules: 702.190a, 509.1h, 506.4
// Given: the declare blockers step of player 0's turn: one Bears attacked unblocked, one was
// blocked and its blocker then left the battlefield, one did not attack.
// Expect: only the unblocked attacker pays the sneak cost: a creature that did not attack is
// not an unblocked creature, and a blocked creature remains blocked once its blocker is gone.
#[test]
fn r702_190a_only_an_unblocked_attacker_is_returned() {
    let mut g = game();
    let unblocked = g.put(0, BEARS, Zone::Battlefield);
    let blocked = g.put(0, BEARS, Zone::Battlefield);
    let home = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    let ninja = g.put(0, NINJA, Zone::Hand);
    attack(&mut g, vec![(unblocked, Target::Player(1)), (blocked, Target::Player(1))]);
    block(&mut g, vec![(blocker, blocked)]);
    assert!(g.destroy(blocker));
    g.players[0].pool.colors[2] = 1;
    cite!(["702.190a"]; refused("Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sneak(g, ninja, home)));
    cite!(["702.190a", "509.1h"]; refused("Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sneak(g, ninja, blocked)));
    assert_eq!(g.players[0].pool.colors[2], 1, "nothing paid");
    cite!(["702.190a"]; ok: sneak(&mut g, ninja, unblocked).map_err(|e| e.to_string()));
}

// Rules: 702.190b, 508.3a, 508.4
// Given: Banner ("Whenever a creature you control attacks, you gain 1 life") on the
// battlefield; the Ninja, "Whenever this creature attacks, you gain 2 life", is sneaked.
// Expect: the declared Bears' attack gives 1 life; the Ninja entering attacking triggers
// neither its own ability nor Banner's: it was never declared as an attacker.
#[test]
fn r508_4_a_sneaked_creature_was_not_declared_as_an_attacker() {
    let mut g = game();
    let banner = g.put(0, BANNER, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let other = g.put(0, BEARS, Zone::Battlefield);
    let ninja = g.put(0, NINJA, Zone::Hand);
    attack(&mut g, vec![(bears, Target::Player(1)), (other, Target::Player(1))]);
    // Banner triggers separately for each declared attacker. Explicitly order
    // and resolve those occurrences before testing the sneaked entrant's absence.
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("expected Banner trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    assert!(abilities.iter().all(|ability| ability.source == Some(banner) && ability.controller == 0));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22, "two declared attackers");
    block(&mut g, vec![]);
    g.players[0].pool.colors[2] = 1;
    sneak(&mut g, ninja, bears).unwrap();
    resolve(&mut g);
    assert_eq!(ninjas(&g).len(), 1);
    cite!(["508.4", "508.3a", "702.190b"]; assert!(g.stack.is_empty() && g.players[0].life == 22, "life {}", g.players[0].life));
}

// Rules: 702.190a, 702.190b
// Given: a Ninja sneaked by returning a Bears that attacked player 1's planeswalker; then, in
// the same declare blockers step, a second Ninja sneaked by returning the first.
// Expect: the first Ninja is an unblocked attacking creature, so it pays the second sneak cost;
// the second enters attacking the planeswalker the first was attacking.
#[test]
fn r702_190b_a_sneaked_creature_pays_another_sneak_cost() {
    let mut g = game();
    let walker = g.put(1, WALKER, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 5);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let first = g.put(0, NINJA, Zone::Hand);
    let second = g.put(0, NINJA, Zone::Hand);
    attack(&mut g, vec![(bears, Target::Object(walker))]);
    block(&mut g, vec![]);
    g.players[0].pool.colors[2] = 1;
    sneak(&mut g, first, bears).unwrap();
    resolve(&mut g);
    let [entered] = ninjas(&g)[..] else { panic!() };
    assert_eq!((g.part, g.priority), (Part::DeclareBlockers, 0));
    g.players[0].pool.colors[2] = 1;
    cite!(["702.190a"]; ok: sneak(&mut g, second, entered).map_err(|e| e.to_string()));
    resolve(&mut g);
    let [now] = ninjas(&g)[..] else { panic!("{:?}", ninjas(&g)) };
    assert_ne!(now, entered);
    cite!(["702.190b"]; assert!(g.combat.attackers.contains(&(now, Target::Object(walker))), "{:?}", g.combat.attackers));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
