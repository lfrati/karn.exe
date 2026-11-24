//! "During target opponent's next turn, creatures that player controls attack Gideon Jura if
//! able.": a requirement to attack a particular player or planeswalker (508.1d). A creature it
//! is said of attacks that one if it can, and may attack anything else, or nothing, when it
//! can't. The most requirements that can be obeyed without paying a cost are (508.1d). Said of
//! "creatures that player controls", it is of whatever creatures they control as attackers are
//! declared, not only those they controlled as it resolved (ruling 2010-06-15). Said of "that
//! player's next turn", it applies in that turn only.
use crate::support;
use game::legal::Asked;
use game::state::Decision;
use support::*;

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

/// Pass until attackers are to be declared.
fn to_attackers(t: &mut T) {
    for _ in 0..20 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        let p = t.g.decision_maker(t.g.priority);
        t.g.act(p, Move::Pass).unwrap();
    }
    panic!("never reached a declare attackers step");
}

/// What this creature may attack as attackers are declared, and whether it must.
fn may_attack(t: &mut T, creature: ObjId) -> (Vec<Target>, bool) {
    let Some((_, Asked::Attackers(able))) = t.asked() else { panic!("{:?}", t.asked()) };
    let a = able.iter().find(|a| a.creature == creature).expect("able to attack");
    (a.attacks.clone(), a.must)
}

/// Player 0's Gideon Jura (6 loyalty) after its +2 has resolved targeting player 1, who controls
/// Grizzly Bears.
fn gideon(hand: &[&str]) -> (T, ObjId, ObjId) {
    let mut t = Board::new(2).battlefield(0, &["Gideon Jura"]).counters("Gideon Jura", "loyalty", 6)
        .battlefield(1, &["Grizzly Bears"]).hand(1, hand).fill(10).build();
    let gideon = t.find_in_battlefield("Gideon Jura");
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.activate(0, "Gideon Jura", 0, vec![Tgt::Player(1)], mana_pool("")).unwrap();
    resolve(&mut t);
    (t, gideon, bears)
}

// Rules: 508.1d, 506.2
// Given: player 0's Gideon Jura has used "+2: During target opponent's next turn, creatures that
// player controls attack Gideon Jura if able." on player 1, who controls Grizzly Bears.
// When: player 1 declares attackers on their next turn.
// Expect: the Bears must attack and may attack Gideon only; attacking player 0 or not attacking
// is refused, and attacking Gideon is legal. Gideon's loyalty is 8 after the ability.
#[test]
fn r508_1d_gideon_juras_creatures_attack_him_during_that_players_next_turn() {
    let (mut t, gideon, bears) = gideon(&[]);
    cite!(["606.4"]; assert_eq!(t.g.obj(gideon).counters.get("loyalty"), Some(&8)));
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Object(gideon)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks what an effect says it attacks if able (508.1d)"): t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])));
    cite!(["508.1d"]; refused("Grizzly Bears attacks if able (508.1d)"): t.g.act(1, Move::Attack(vec![])));
    cite!(["508.1d", "506.2"]; ok: t.g.act(1, Move::Attack(vec![(bears, Target::Object(gideon))])));
}

// Rules: 508.1d, 611.2c
// Given: Gideon Jura's +2 has resolved targeting player 1.
// When: on their next turn player 1 casts Raging Goblin (haste) before combat.
// Expect: the Goblin, which player 1 didn't control as the ability resolved, must attack Gideon
// too (ruling 2010-06-15: the ability doesn't lock in what it applies to).
#[test]
fn r508_1d_gideon_juras_requirement_is_of_creatures_that_player_controls_later_too() {
    let (mut t, gideon, _) = gideon(&["Raging Goblin"]);
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{R}").cast(1, "Raging Goblin", vec![], mana_pool("R")).unwrap();
    resolve(&mut t);
    let goblin = t.find_in_battlefield("Raging Goblin");
    to_attackers(&mut t);
    cite!(["508.1d", "611.2c"]; assert_eq!(may_attack(&mut t, goblin), (vec![Target::Object(gideon)], true)));
}

// Rules: 508.1d, 611.2a
// Given: Gideon Jura's +2 has resolved targeting player 1.
// When: player 1 attacks Gideon with the Bears on their next turn, and declares attackers again
// on the turn after.
// Expect: on that later turn the Bears may attack player 0 or not at all: the requirement lasted
// through player 1's next turn only.
#[test]
fn r508_1d_gideon_juras_requirement_ends_with_that_turn() {
    let (mut t, gideon, bears) = gideon(&[]);
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    t.g.act(1, Move::Attack(vec![(bears, Target::Object(gideon))])).unwrap();
    t.to_no_combat(0, Part::PrecombatMain);
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    let (attacks, must) = may_attack(&mut t, bears);
    cite!(["508.1d", "611.2a"]; assert!(!must && attacks.contains(&Target::Player(0))));
    cite!(["508.1d"]; ok: t.g.act(1, Move::Attack(vec![])));
}

// Rules: 508.1d
// Given: Gideon Jura's +2 has resolved targeting player 1, who holds Hero's Downfall.
// When: on their next turn player 1 destroys Gideon before combat.
// Expect: the Bears can't attack Gideon, so they may attack player 0 or nothing (ruling
// 2010-06-15).
#[test]
fn r508_1d_a_creature_that_cant_attack_gideon_attacks_anything_or_nothing() {
    let (mut t, gideon, bears) = gideon(&["Hero's Downfall"]);
    t.to_no_combat(1, Part::PrecombatMain);
    t.mana(1, "{1}{B}{B}").cast(1, "Hero's Downfall", vec![Tgt::Object(gideon)], mana_pool("CBB")).unwrap();
    resolve(&mut t);
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0)], false)));
    cite!(["508.1d"]; ok: t.g.act(1, Move::Attack(vec![])));
}

// Rules: 508.1d
// Given: three players; player 0 casts Taunt ("During target player's next turn, creatures that
// player controls attack you if able.") targeting player 1, who controls Grizzly Bears.
// When: player 1 declares attackers on their next turn.
// Expect: the Bears must attack player 0; attacking player 2 is refused.
#[test]
fn r508_1d_taunt_makes_that_players_creatures_attack_you() {
    let mut t = Board::new(3).hand(0, &["Taunt"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{U}").cast(0, "Taunt", vec![Tgt::Player(1)], mana_pool("U")).unwrap();
    resolve(&mut t);
    t.to_no_combat(1, Part::PrecombatMain);
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks what an effect says it attacks if able (508.1d)"): t.g.act(1, Move::Attack(vec![(bears, Target::Player(2))])));
    cite!(["508.1d"]; ok: t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])));
}

// Rules: 508.1d, 611.2a
// Given: three players; player 0 controls Alluring Siren ("{T}: Target creature an opponent
// controls attacks you this turn if able."); player 1 controls Grizzly Bears.
// When: in player 1's precombat main phase, player 0 activates the Siren targeting the Bears.
// Expect: the Bears must attack player 0 that turn; attacking player 2 is refused.
#[test]
fn r508_1d_alluring_siren_makes_a_creature_attack_you_this_turn() {
    let mut t = Board::new(3).battlefield(0, &["Alluring Siren"]).battlefield(1, &["Grizzly Bears"]).fill(10).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.to_no_combat(1, Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    t.g.act(2, Move::Pass).unwrap();
    t.activate(0, "Alluring Siren", 0, vec![Tgt::Object(bears)], mana_pool("")).unwrap();
    resolve(&mut t);
    to_attackers(&mut t);
    cite!(["508.1d"]; assert_eq!(may_attack(&mut t, bears), (vec![Target::Player(0)], true)));
    cite!(["508.1d"]; refused("Grizzly Bears attacks what an effect says it attacks if able (508.1d)"): t.g.act(1, Move::Attack(vec![(bears, Target::Player(2))])));
    cite!(["508.1d"]; ok: t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])));
}
