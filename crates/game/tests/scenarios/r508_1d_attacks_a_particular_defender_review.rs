//! Review of the requirements to attack a particular player or planeswalker (508.1d): Alluring
//! Siren's "you" is the player, not their planeswalker (its ruling); "this turn" on its
//! controller's own turn asks nothing later; Gideon Jura's requirement holds in each combat of
//! that turn (its ruling); two requirements that can't both be obeyed are obeyed once either
//! way; and a requirement with a cost to obey needn't be (508.1d, the rulings). Every card is read
//! from its Oracle text.
use crate::support::*;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Alluring Siren", "Jace Beleren", "Gideon Jura", "Relentless Assault", "Taunt", "Ghostly Prison", "Grizzly Bears"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Pass until attackers are to be declared.
fn to_attackers(t: &mut T) {
    for _ in 0..30 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    panic!("never reached a declare attackers step");
}

/// Pass until this player has priority.
fn priority_to(t: &mut T, p: usize) {
    while t.g.decision_maker(t.g.priority) != p {
        let q = t.g.decision_maker(t.g.priority);
        t.g.act(q, Move::Pass).unwrap();
    }
}

/// What this creature may attack as attackers are declared, and whether it must.
fn may_attack(t: &mut T, creature: ObjId) -> (Vec<Target>, bool) {
    let Some((_, Asked::Attackers(able))) = t.asked() else { panic!("{:?}", t.asked()) };
    let a = able.iter().find(|a| a.creature == creature).expect("able to attack");
    (a.attacks.clone(), a.must)
}

/// Declare these attackers, on a copy of the game.
fn attack(t: &T, with: &[(ObjId, Target)]) -> Result<(), String> { t.g.clone().act(1, Move::Attack(with.to_vec())) }

// Rules: 508.1d
// Given: player 0's Alluring Siren and Jace Beleren; player 1's Grizzly Bears.
// When: in player 1's precombat main phase, player 0 activates the Siren ("{T}: Target creature an
// opponent controls attacks you this turn if able.") at the Bears, and attackers are declared.
// Expect: the Bears must attack player 0, not Jace (its ruling): attacking Jace or nothing is
// refused.
#[test]
fn r508_1d_alluring_sirens_creature_attacks_you_and_not_your_planeswalker() {
    let mut t = Board::new(2).battlefield(0, &["Alluring Siren", "Jace Beleren"]).counters("Jace Beleren", "loyalty", 3).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let (bears, jace) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Jace Beleren"));
    t.to_no_combat(1, Part::PrecombatMain);
    priority_to(&mut t, 0);
    t.activate(0, "Alluring Siren", 0, vec![Tgt::Object(bears)], mana_pool("")).unwrap();
    t.resolve();
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks what an effect says it attacks if able (508.1d)"): attack(&t, &[(bears, Target::Object(jace))]));
    cite!(["508.1d"]; refused("Grizzly Bears attacks if able (508.1d)"): attack(&t, &[]));
    cite!(["508.1d"]; ok: attack(&t, &[(bears, Target::Player(0))]));
}

// Rules: 508.1d, 611.2a
// Given: player 0's Alluring Siren; player 1's Grizzly Bears.
// When: player 0 activates the Siren at the Bears in their own turn, and attackers are declared on
// player 1's next turn.
// Expect: "this turn" was player 0's (its ruling): the Bears needn't attack.
#[test]
fn r611_2a_alluring_siren_on_its_controllers_turn_asks_nothing_later() {
    let mut t = Board::new(2).battlefield(0, &["Alluring Siren"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.activate(0, "Alluring Siren", 0, vec![Tgt::Object(bears)], mana_pool("")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    cite!(["611.2a", "508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0)], false)));
    cite!(["508.1d"]; ok: attack(&t, &[]));
}

// Rules: 508.1d, 500.8
// Given: Gideon Jura's +2 ("During target opponent's next turn, creatures that player controls
// attack Gideon Jura if able.") has resolved targeting player 1, who controls Grizzly Bears and
// holds Relentless Assault ("Untap all creatures that attacked this turn. After this main phase,
// there is an additional combat phase followed by an additional main phase.").
// When: on their next turn the Bears attack Gideon, player 1 casts Relentless Assault, and
// attackers are declared again.
// Expect: the requirement holds in each combat of that turn (its ruling): the Bears must attack
// Gideon again.
#[test]
fn r508_1d_gideon_juras_requirement_holds_in_each_combat_of_that_turn() {
    let mut t = Board::new(2).battlefield(0, &["Gideon Jura"]).counters("Gideon Jura", "loyalty", 6).battlefield(1, &["Grizzly Bears"]).hand(1, &["Relentless Assault"]).fill(10).build();
    let (gideon, bears) = (t.find_in_battlefield("Gideon Jura"), t.find_in_battlefield("Grizzly Bears"));
    t.activate(0, "Gideon Jura", 0, vec![Tgt::Player(1)], mana_pool("")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    t.g.act(1, Move::Attack(vec![(bears, Target::Object(gideon))])).unwrap();
    while !(t.g.part == Part::PostcombatMain && t.g.stack.is_empty()) {
        match t.g.decision.clone() {
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    cite!(["508.1d", "306.8"]; assert_eq!(t.g.counters(gideon).get("loyalty").copied(), Some(6)));
    t.mana(1, "{2}{R}{R}").cast(1, "Relentless Assault", vec![], mana_pool("CCRR")).unwrap();
    t.resolve();
    to_attackers(&mut t);
    cite!(["508.1d", "500.8"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Object(gideon)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks if able (508.1d)"): attack(&t, &[]));
}

// Rules: 508.1d
// Given: player 0's Gideon Jura has used its +2 on player 1, and player 0 has cast Taunt ("During
// target player's next turn, creatures that player controls attack you if able.") at player 1,
// who controls Grizzly Bears.
// When: player 1 declares attackers on their next turn.
// Expect: the Bears can obey only one of the two requirements, either one (508.1d): attacking
// player 0 or Gideon is legal, attacking with nothing is refused.
#[test]
fn r508_1d_of_two_requirements_to_attack_different_defenders_either_is_obeyed() {
    let mut t = Board::new(2).battlefield(0, &["Gideon Jura"]).counters("Gideon Jura", "loyalty", 6).hand(0, &["Taunt"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let (gideon, bears) = (t.find_in_battlefield("Gideon Jura"), t.find_in_battlefield("Grizzly Bears"));
    t.activate(0, "Gideon Jura", 0, vec![Tgt::Player(1)], mana_pool("")).unwrap();
    t.resolve();
    t.mana(0, "{U}").cast(0, "Taunt", vec![Tgt::Player(1)], mana_pool("U")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    let (defenders, must) = may_attack(&mut t, bears);
    let both = [Target::Player(0), Target::Object(gideon)];
    cite!(["508.1d"]; assert!(must && defenders.len() == 2 && both.iter().all(|d| defenders.contains(d)), "{defenders:?}"));
    cite!(["508.1d"]; refused("Grizzly Bears attacks if able (508.1d)"): attack(&t, &[]));
    cite!(["508.1d"]; ok: attack(&t, &[(bears, Target::Player(0))]));
    cite!(["508.1d"]; ok: attack(&t, &[(bears, Target::Object(gideon))]));
}

// Rules: 508.1d
// Given: three players; player 0 has cast Taunt at player 1, who controls Grizzly Bears; player 2
// controls Alluring Siren.
// When: in player 1's precombat main phase, player 2 activates the Siren at the Bears.
// Expect: attacking player 0 or player 2 obeys one requirement, as many as can be: either is
// legal, attacking with nothing is refused.
#[test]
fn r508_1d_taunt_and_alluring_siren_of_two_players_are_obeyed_once_either_way() {
    let mut t = Board::new(3).hand(0, &["Taunt"]).battlefield(2, &["Alluring Siren"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{U}").cast(0, "Taunt", vec![Tgt::Player(1)], mana_pool("U")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::PrecombatMain);
    priority_to(&mut t, 2);
    t.activate(2, "Alluring Siren", 0, vec![Tgt::Object(bears)], mana_pool("")).unwrap();
    t.resolve();
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0), Target::Player(2)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks if able (508.1d)"): attack(&t, &[]));
    cite!(["508.1d"]; ok: attack(&t, &[(bears, Target::Player(0))]));
    cite!(["508.1d"]; ok: attack(&t, &[(bears, Target::Player(2))]));
}

// Rules: 508.1d, 508.1h
// Given: player 0's Ghostly Prison ("Creatures can't attack you unless their controller pays {2}
// for each creature they control that's attacking you."); player 0 has cast Taunt at player 1,
// who controls Grizzly Bears.
// When: player 1 declares attackers on their next turn.
// Expect: obeying the requirement costs {2}, which needn't be paid (508.1d): the Bears may attack
// player 0 for {2}, and need not attack.
#[test]
fn r508_1d_a_requirement_whose_attack_has_a_cost_need_not_be_obeyed() {
    let mut t = Board::new(2).hand(0, &["Taunt"]).battlefield(0, &["Ghostly Prison"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{U}").cast(0, "Taunt", vec![Tgt::Player(1)], mana_pool("U")).unwrap();
    t.resolve();
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    let Some((_, Asked::Attackers(able))) = t.asked() else { panic!("{:?}", t.asked()) };
    let a = able.iter().find(|a| a.creature == bears).expect("able to attack");
    cite!(["508.1d", "508.1h"]; assert_eq!((a.attacks.clone(), a.must), (vec![Target::Player(0)], false)));
    cite!(["508.1h"]; assert_eq!(a.costs.len(), 1));
    cite!(["508.1d"]; ok: attack(&t, &[]));
}
