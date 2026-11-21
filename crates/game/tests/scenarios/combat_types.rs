//! Removal from combat when characteristics change (CR 506.4). The example spells compose
//! existing concepts, read from text, to test the rule without depending on particular cards.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part, Status};

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn game(text: &str, blocked: bool) -> (Game, ObjId, Option<ObjId>) {
    let defs = vec![
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Change", "{0}", "Instant", text, None).unwrap(),
        CardDef::new("Battle cry", "{0}", "Enchantment", "Attacking creatures get +1/+1.", None).unwrap(),
    ];
    for d in &defs {
        check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    let attacker = g.put(0, 0, Zone::Battlefield);
    let blocker = blocked.then(|| g.put(1, 0, Zone::Battlefield));
    // A layer effect asks about combat status: evaluating types must not recurse through it.
    g.put(0, 2, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.part, Part::DeclareAttackers);
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::DeclareBlockers);
    if let Some(blocker) = blocker {
        g.act(1, Move::Block(vec![(blocker, attacker)])).unwrap();
    }
    assert_eq!(g.power(attacker), Some(3));
    (g, attacker, blocker)
}

fn change(g: &mut Game, target: ObjId) {
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
    all_pass(g);
    assert!(g.stack.is_empty());
}

#[test]
fn r506_4_an_attacker_that_stops_being_a_creature_leaves_combat_before_the_next_instruction() {
    let (mut g, attacker, _) = game(
        "Target creature becomes an artifact until end of turn. If you control an attacking creature, you gain 3 life.",
        false,
    );
    change(&mut g, attacker);
    assert!(g.is_a(attacker, CardType::Artifact));
    assert!(!g.is_a(attacker, CardType::Creature));
    assert_eq!(g.power(attacker), None);
    cite!(["506.4"]; assert!(!g.combat.attacking(attacker)));
    cite!(["506.4"]; assert_eq!(game::combat::status(&g, attacker, Status::Unblocked), Some(false)));
    cite!(["506.4"]; assert_eq!(g.players[0].life, 20, "506.4 applies during resolution, before the conditional instruction"));
    all_pass(&mut g);
    cite!(["508.8"]; assert_eq!(g.part, Part::CombatDamage, "having declared attackers, the step still happens (508.8)"));
    cite!(["506.4"]; assert_eq!(g.players[1].life, 20));
}

#[test]
fn r506_4_becoming_a_creature_again_does_not_restore_combat_status() {
    let (mut g, attacker, _) = game(
        "Target creature becomes an artifact until end of turn. It becomes a 2/2 creature until end of turn.",
        true,
    );
    change(&mut g, attacker);
    assert!(g.is_a(attacker, CardType::Creature));
    cite!(["506.4"]; assert_eq!(g.power(attacker), Some(2), "the effect on attacking creatures no longer applies"));
    cite!(["506.4"]; assert!(!g.combat.attacking(attacker)));
    cite!(["506.4"]; assert!(!g.combat.blocked.contains(&attacker)));
    all_pass(&mut g);
    cite!(["506.4"]; assert_eq!(g.obj(attacker).damage, 0));
    cite!(["506.4"]; assert_eq!(g.players[1].life, 20));
}

#[test]
fn r509_1h_a_blocker_that_stops_being_a_creature_leaves_its_attacker_blocked() {
    let (mut g, attacker, blocker) = game("Target creature becomes an artifact until end of turn.", true);
    let blocker = blocker.unwrap();
    change(&mut g, blocker);
    cite!(["506.4"]; assert!(!g.combat.blocking(blocker)));
    cite!(["506.4"]; assert!(g.combat.blockers_of(attacker).is_empty()));
    cite!(["509.1h"]; assert!(g.combat.blocked.contains(&attacker)));
    cite!(["506.4"]; assert_eq!(game::combat::status(&g, blocker, Status::Blocking), Some(false)));
    all_pass(&mut g);
    cite!(["510.1d"]; assert_eq!(g.obj(attacker).damage, 0));
    cite!(["510.1c"]; assert_eq!(g.players[1].life, 20, "a blocked attacker with no blockers assigns no damage (510.1c)"));
}

#[test]
fn r509_1g_a_blocker_remains_blocking_when_its_attacker_stops_being_a_creature() {
    let (mut g, attacker, blocker) = game("Target creature becomes an artifact until end of turn.", true);
    let blocker = blocker.unwrap();
    change(&mut g, attacker);
    cite!(["509.1g"]; assert!(g.combat.blocking(blocker)));
    cite!(["509.1g"]; assert!(g.combat.blockers.is_empty()));
    cite!(["509.1g"]; assert!(!g.combat.blocked.contains(&attacker)));
    all_pass(&mut g);
    cite!(["509.1g"]; assert_eq!(g.obj(blocker).damage, 0));
    cite!(["510.1d"]; assert_eq!(g.obj(attacker).damage, 0, "a blocking creature with no attackers assigns no damage (510.1d)"));
}

#[test]
fn r506_4_a_creature_that_becomes_a_battle_is_removed_even_if_it_is_still_a_creature() {
    for blocked in [false, true] {
        let (mut g, attacker, blocker) =
            game("Target creature becomes a battle in addition to its other types until end of turn.", blocked);
        let changed = blocker.unwrap_or(attacker);
        change(&mut g, changed);
        assert!(g.is_a(changed, CardType::Creature) && g.is_a(changed, CardType::Battle));
        cite!(["506.4"]; assert!(!g.combat.attacking(changed) && !g.combat.blocking(changed)));
        all_pass(&mut g);
        cite!(["506.4"]; assert_eq!(g.players[1].life, 20));
        cite!(["506.4"]; assert_eq!(g.obj(attacker).damage, 0));
    }
}

#[test]
fn r506_4_a_creature_that_gains_another_type_stays_in_combat() {
    let (mut g, attacker, _) =
        game("Target creature becomes an artifact in addition to its other types until end of turn.", false);
    change(&mut g, attacker);
    assert!(g.is_a(attacker, CardType::Artifact) && g.is_a(attacker, CardType::Creature));
    cite!(["506.4"]; assert!(g.combat.attacking(attacker)));
    cite!(["506.4"]; assert_eq!(g.power(attacker), Some(3)));
    all_pass(&mut g);
    cite!(["506.4"]; assert_eq!(g.players[1].life, 17));
}

#[test]
fn r506_4a_b_tapping_or_preventing_attack_does_not_remove_an_existing_attacker() {
    let (mut g, attacker, _) = game("Untap target creature. It can't attack this turn.", false);
    change(&mut g, attacker);
    assert!(!g.obj(attacker).tapped);
    cite!(["506.4a"]; assert!(g.combat.attacking(attacker)));
    all_pass(&mut g);
    cite!(["506.4a"]; assert_eq!(g.players[1].life, 17));
}
