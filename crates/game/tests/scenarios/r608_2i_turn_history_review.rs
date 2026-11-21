//! The turn's history (608.2i, 400.7, 508.1, 603.2, 119.9). What
//! happened earlier in the turn is asked of the objects as they were then: they need not
//! "currently meet the criteria described in the action, as long as they did so at the
//! specified time" (608.2i). Whether a game notes what happened must not depend on which of
//! its cards' texts the engine recognizes as looking back. Wingmate Roc, Aurelia, Vanguard
//! Seraph, Deepway Navigator, Turn to Frog, Bristlebane Outrider, Animate Land, Kinbinding and
//! Kutzil's Flanker are Oracle cards.

use crate::scenarios::r708_8_turned_face_up::{game, put};
use crate::support::{cite, paid};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

type T = crate::support::T;

fn modal(modes: &[usize], then: Move) -> Move { Move::Modes(modes.to_vec(), Box::new(then)) }

/// Both players pass until the stack is empty; a pending question is a failure.
fn settle(t: &mut T) {
    for _ in 0..50 {
        if t.g.stack.is_empty() && t.g.decision.is_none() { return; }
        assert!(t.g.decision.is_none(), "unexpected decision {:?}", t.g.decision);
        t.g.act(t.g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}

fn to_main(t: &mut T) {
    while t.g.part != Part::PrecombatMain { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}

/// This player casts this card from their hand with these targets and mana; it resolves.
fn cast(t: &mut T, p: usize, name: &str, targets: Vec<Vec<Target>>, mana: &str) {
    while t.g.priority != p { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let card = put(t, p, name, Zone::Hand);
    t.mana(p, mana);
    t.g.act(p, paid(mana, Move::Cast { card, targets })).unwrap();
    settle(t);
}

fn on_battlefield(t: &T, name: &str) -> ObjId {
    *t.g.battlefield.iter().rev().find(|o| t.g.obj(**o).chars.name == name).unwrap_or_else(|| panic!("no {name}"))
}

/// To this player's next declaration of attackers.
fn to_attackers(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.decision == Some(Decision::Attackers { player }) { return; }
        match g.decision {
            Some(Decision::Attackers { player: p }) => { g.act(p, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Blockers { player: p }) => { g.act(p, Move::Block(vec![])).unwrap(); }
            _ => { g.act(g.priority, Move::Pass).unwrap(); }
        }
    }
    panic!("no declaration of attackers: {:?} {:?}", g.part, g.decision);
}

/// To this player's postcombat main phase, with priority and an empty stack; no blocks.
fn to_main_after_combat(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.part == Part::PostcombatMain && g.active == player && g.priority == player && g.stack.is_empty() { return; }
        match g.decision {
            Some(Decision::Attackers { player: p }) => { g.act(p, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Blockers { player: p }) => { g.act(p, Move::Block(vec![])).unwrap(); }
            _ => { g.act(g.priority, Move::Pass).unwrap(); }
        }
    }
    panic!("not there: {:?}", g.part);
}

const BEAR: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Bear", "{0}", "Creature — Bear", "", Some((2, 2)));
const MERFOLK: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Merfolk", "{0}", "Creature — Merfolk", "", Some((1, 1)));
const BOUNCE: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Bounce", "{0}", "Instant", "Return target creature to its owner's hand.", None);
const TRINKET: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Trinket", "{3}", "Artifact", "", None);
const ANIMATE: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Animate", "{0}", "Instant", "Target artifact becomes an artifact creature with base power and toughness 3/3 until end of turn.", None);
const MERFOLKIZE: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Merfolkize", "{0}", "Instant", "Target creature becomes a Merfolk in addition to its other types until end of turn.", None);
const DISENCHANT: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Disenchant", "{0}", "Instant", "Destroy target enchantment.", None);
const KILL: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Kill", "{0}", "Instant", "Destroy target creature.", None);
const WALK: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Walk", "{0}", "Sorcery", "Take an extra turn after this one.", None);
const NO_GAIN: (&str, &str, &str, &str, Option<(i32, i32)>) = ("No Gain", "{0}", "Enchantment", "Players can't gain life.", None);
const HEAL: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Heal", "{0}", "Instant", "You gain 1 life.", None);
const TWICE: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Twice", "{0}", "Instant", "You gain 1 life. Then you gain 1 life.", None);
const NOTHING: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Nothing", "{0}", "Instant", "You gain life equal to the number of cards in your hand.", None);
const FLICKER: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Flicker", "{0}", "Instant", "Exile target creature you control, then return that card to the battlefield under its owner's control.", None);

#[test]
fn every_test_card_is_one_the_engine_runs() {
    for (name, cost, types, text, pt) in [BEAR, MERFOLK, BOUNCE, TRINKET, ANIMATE, MERFOLKIZE, DISENCHANT, NO_GAIN, KILL, WALK, HEAL, TWICE, NOTHING, FLICKER] {
        let def = game::CardDef::new(name, cost, types, text, pt).unwrap();
        assert_eq!(game::play::check(&def).err().map(|u| u.0), None, "{name}");
    }
}

/// Kutzil's Flanker: "Put a +1/+1 counter on this creature for each creature that left the
/// battlefield under your control this turn." In a game whose only card that looks back is
/// the Flanker, a creature of yours returned to hand earlier in the turn is counted (608.2i);
/// whether the game noted it must not depend on recognizing the look-back in a mode.
fn flanker_counts(names: &[&str]) -> i32 {
    let mut t = game(names, &[BEAR, BOUNCE]);
    to_main(&mut t);
    let bear = put(&mut t, 0, "Bear", Zone::Battlefield);
    cast(&mut t, 0, "Bounce", vec![vec![Target::Object(bear)]], "");
    let flanker = put(&mut t, 0, "Kutzil's Flanker", Zone::Hand);
    t.mana(0, "{2}{W}");
    t.g.act(0, paid("CCW", Move::Cast { card: flanker, targets: vec![] })).unwrap();
    for _ in 0..10 { if matches!(t.g.decision, Some(Decision::Modes { .. })) { break; } t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, modal(&[0], Move::ChooseTargets(vec![]))).unwrap();
    settle(&mut t);
    t.g.power(on_battlefield(&t, "Kutzil's Flanker")).unwrap()
}

#[test]
fn kutzils_flanker_counts_a_creature_that_left_when_another_card_looks_back() {
    cite!(["608.2i"]; assert_eq!(flanker_counts(&["Kutzil's Flanker", "Storm Fleet Spy"]), 4, "one creature left: one counter"));
}

#[test]
fn kutzils_flanker_alone_counts_a_creature_that_left() {
    cite!(["608.2i"]; assert_eq!(flanker_counts(&["Kutzil's Flanker"]), 4, "one creature left: one counter"));
}

/// Raid on a creature cast after combat: the attack happened before the card looking back
/// was cast, and it is still seen (608.2i, 508.1).
#[test]
fn wingmate_roc_cast_after_combat_sees_the_attack() {
    let mut t = game(&["Wingmate Roc"], &[BEAR]);
    to_main(&mut t);
    let bear = put(&mut t, 0, "Bear", Zone::Battlefield);
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(vec![(bear, Target::Player(1))])).unwrap();
    to_main_after_combat(&mut t.g, 0);
    cast(&mut t, 0, "Wingmate Roc", vec![], "{3}{W}{W}");
    let birds = t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name.contains("Bird")).count();
    cite!(["608.2i", "508.1"]; assert_eq!(birds, 1, "you attacked this turn: a Bird token"));
}

/// "As long as another creature entered the battlefield under your control this turn": a land
/// played this turn and animated since did not enter as a creature (608.2i: the objects need
/// not "currently meet the criteria described in the action, as long as they did so at the
/// specified time"; here they meet them now and did not then).
#[test]
fn a_land_animated_after_it_entered_is_not_a_creature_that_entered() {
    let mut t = game(&["Bristlebane Outrider", "Animate Land"], &[]);
    to_main(&mut t);
    let outrider = put(&mut t, 0, "Bristlebane Outrider", Zone::Battlefield);
    let wastes = put(&mut t, 0, "Wastes", Zone::Hand);
    t.g.act(0, Move::PlayLand(wastes)).unwrap();
    let land = on_battlefield(&t, "Wastes");
    assert_eq!(t.g.power(outrider), Some(3), "a land entered, not a creature");
    cast(&mut t, 0, "Animate Land", vec![vec![Target::Object(land)]], "{G}");
    assert_eq!(t.g.power(land), Some(3), "Animate Land made it a 3/3 creature");
    cite!(["608.2i"]; assert_eq!(t.g.power(outrider), Some(3), "it entered as a noncreature land"));
}

/// Kinbinding: "where X is the number of creatures that entered the battlefield under your
/// control this turn": the same, counted (608.2i).
#[test]
fn kinbinding_does_not_count_an_artifact_animated_after_it_entered() {
    let mut t = game(&["Kinbinding"], &[TRINKET, BEAR, ANIMATE]);
    to_main(&mut t);
    let bear = put(&mut t, 0, "Bear", Zone::Battlefield);
    cast(&mut t, 0, "Kinbinding", vec![], "{3}{W}{W}");
    cast(&mut t, 0, "Trinket", vec![], "{3}");
    let trinket = on_battlefield(&t, "Trinket");
    assert_eq!(t.g.power(bear), Some(2), "no creature entered");
    cast(&mut t, 0, "Animate", vec![vec![Target::Object(trinket)]], "");
    assert!(t.g.toughness(trinket).is_some(), "an artifact creature now");
    cite!(["608.2i"]; assert_eq!(t.g.power(bear), Some(2), "it entered as a noncreature artifact"));
}

/// Deepway Navigator: "As long as you attacked with three or more Merfolk this turn": a
/// Merfolk that attacked and is a Frog now still attacked as a Merfolk (608.2i, 508.1).
#[test]
fn a_merfolk_that_attacked_and_is_a_frog_now_attacked_as_a_merfolk() {
    let mut t = game(&["Deepway Navigator", "Turn to Frog"], &[MERFOLK]);
    to_main(&mut t);
    let navigator = put(&mut t, 0, "Deepway Navigator", Zone::Battlefield);
    let merfolk: Vec<ObjId> = (0..3).map(|_| put(&mut t, 0, "Merfolk", Zone::Battlefield)).collect();
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(merfolk.iter().map(|m| (*m, Target::Player(1))).collect())).unwrap();
    to_main_after_combat(&mut t.g, 0);
    assert_eq!(t.g.power(navigator), Some(3));
    cast(&mut t, 0, "Turn to Frog", vec![vec![Target::Object(merfolk[0])]], "{1}{U}");
    cite!(["608.2i", "508.1"]; assert_eq!(t.g.power(navigator), Some(3), "three Merfolk attacked"));
}

/// The converse: a Bear that attacked and is a Merfolk since did not attack as a Merfolk;
/// two Merfolk and it are not three Merfolk you attacked with (608.2i, 508.1).
#[test]
fn a_bear_that_attacked_and_is_a_merfolk_now_did_not_attack_as_a_merfolk() {
    let mut t = game(&["Deepway Navigator"], &[MERFOLK, BEAR, MERFOLKIZE]);
    to_main(&mut t);
    let navigator = put(&mut t, 0, "Deepway Navigator", Zone::Battlefield);
    let mut attackers: Vec<ObjId> = (0..2).map(|_| put(&mut t, 0, "Merfolk", Zone::Battlefield)).collect();
    let bear = put(&mut t, 0, "Bear", Zone::Battlefield);
    attackers.push(bear);
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(attackers.iter().map(|m| (*m, Target::Player(1))).collect())).unwrap();
    to_main_after_combat(&mut t.g, 0);
    assert_eq!(t.g.power(navigator), Some(2));
    cast(&mut t, 0, "Merfolkize", vec![vec![Target::Object(bear)]], "");
    cite!(["608.2i", "508.1"]; assert_eq!(t.g.power(navigator), Some(2), "two Merfolk and a Bear attacked"));
}

/// The control case: a Merfolk turned into a Frog before it attacked is not one of the
/// Merfolk you attacked with (508.1).
#[test]
fn two_merfolk_and_a_frog_are_not_three_merfolk() {
    let mut t = game(&["Deepway Navigator", "Turn to Frog"], &[MERFOLK]);
    to_main(&mut t);
    let navigator = put(&mut t, 0, "Deepway Navigator", Zone::Battlefield);
    let merfolk: Vec<ObjId> = (0..3).map(|_| put(&mut t, 0, "Merfolk", Zone::Battlefield)).collect();
    cast(&mut t, 0, "Turn to Frog", vec![vec![Target::Object(merfolk[0])]], "{1}{U}");
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(merfolk.iter().map(|m| (*m, Target::Player(1))).collect())).unwrap();
    to_main_after_combat(&mut t.g, 0);
    cite!(["608.2i", "508.1"]; assert_eq!(t.g.power(navigator), Some(2), "a Frog and two Merfolk attacked"));
}

/// "Attacked with three or more Merfolk this turn" counts the Merfolk, over every combat of
/// the turn: two in the first combat and another in Aurelia's additional combat are three;
/// the same two attacking again are still two (508.1, 506.1).
#[test]
fn merfolk_attacking_in_two_combats_are_counted_once_each() {
    for third in [false, true] {
        let mut t = game(&["Deepway Navigator", "Aurelia, the Warleader"], &[MERFOLK]);
        to_main(&mut t);
        let navigator = put(&mut t, 0, "Deepway Navigator", Zone::Battlefield);
        let aurelia = put(&mut t, 0, "Aurelia, the Warleader", Zone::Battlefield);
        let merfolk: Vec<ObjId> = (0..3).map(|_| put(&mut t, 0, "Merfolk", Zone::Battlefield)).collect();
        to_attackers(&mut t.g, 0);
        t.g.act(0, Move::Attack(vec![(aurelia, Target::Player(1)), (merfolk[0], Target::Player(1)), (merfolk[1], Target::Player(1))])).unwrap();
        assert_eq!(t.g.power(navigator), Some(2), "two Merfolk so far");
        to_attackers(&mut t.g, 0);
        assert_eq!(t.g.part, Part::DeclareAttackers, "Aurelia's additional combat");
        let again = if third { merfolk[2] } else { merfolk[0] };
        t.g.act(0, Move::Attack(vec![(again, Target::Player(1)), (merfolk[1], Target::Player(1))])).unwrap();
        to_main_after_combat(&mut t.g, 0);
        cite!(["508.1", "608.2i"]; case: third; assert_eq!(t.g.power(navigator), Some(2 + i32::from(third)), "three different Merfolk attacked: {third}"));
    }
}

/// Aurelia "attacks for the first time each turn": returned to the battlefield between the
/// combats, it is a new object, which hasn't attacked this turn (400.7), and the ability
/// triggers again.
#[test]
fn aurelia_flickered_between_combats_attacks_for_the_first_time_again() {
    let mut t = game(&["Aurelia, the Warleader"], &[FLICKER]);
    to_main(&mut t);
    let aurelia = put(&mut t, 0, "Aurelia, the Warleader", Zone::Battlefield);
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(vec![(aurelia, Target::Player(1))])).unwrap();
    assert_eq!(t.g.stack.len(), 1, "the first time");
    settle(&mut t);
    let card = put(&mut t, 0, "Flicker", Zone::Hand);
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(aurelia)]] }).unwrap();
    settle(&mut t);
    let aurelia = on_battlefield(&t, "Aurelia, the Warleader");
    to_attackers(&mut t.g, 0);
    t.g.act(0, Move::Attack(vec![(aurelia, Target::Player(1))])).unwrap();
    cite!(["400.7", "603.2"]; assert_eq!(t.g.stack.len(), 1, "a new object's first attack"));
}

/// Vanguard Seraph's ability on several gains of the turn (603.2, 119.9, rulings 2018-07-13):
/// two gains in one resolution trigger it once; a gain before it was on the battlefield is
/// the first; a gain of 0 life is no event, and the next gain is the first.
fn seraph_triggers(before: &[(&str, bool)], spell: &str) -> usize {
    let mut t = game(&["Vanguard Seraph"], &[HEAL, TWICE, NOTHING]);
    to_main(&mut t);
    let gain = |t: &mut T, name: &str| {
        let card = put(t, 0, name, Zone::Hand);
        while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        for _ in 0..2 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        let triggered = t.g.stack.len();
        while !t.g.stack.is_empty() || t.g.decision.is_some() {
            if let Some(Decision::Choose { player, choice: game::state::Choice::Arrange { cards, .. } }) = t.g.decision.clone() {
                t.g.act(player, Move::Choose(game::state::Answer::Arrange { top: cards, away: vec![] })).unwrap();
            } else { t.g.act(t.g.priority, Move::Pass).unwrap(); }
        }
        triggered
    };
    let mut seraph = false;
    for (name, with_seraph) in before {
        if *with_seraph && !seraph { put(&mut t, 0, "Vanguard Seraph", Zone::Battlefield); seraph = true; }
        gain(&mut t, name);
    }
    if !seraph { put(&mut t, 0, "Vanguard Seraph", Zone::Battlefield); }
    gain(&mut t, spell)
}

#[test]
fn vanguard_seraph_two_gains_in_one_resolution_trigger_once() {
    cite!(["603.2c", "603.2"]; assert_eq!(seraph_triggers(&[], "Twice"), 1));
}

#[test]
fn vanguard_seraph_a_gain_before_it_was_there_was_the_first() {
    cite!(["603.2"]; assert_eq!(seraph_triggers(&[("Heal", false)], "Heal"), 0, "ruling: it won't trigger that turn"));
}

#[test]
fn vanguard_seraph_a_gain_of_zero_life_is_not_the_first() {
    cite!(["119.9", "603.2"]; assert_eq!(seraph_triggers(&[("Nothing", true)], "Heal"), 1, "gaining 0 life is no life gain event"));
}

/// A life gain that didn't happen, because players can't gain life (119.7), is no event, and
/// the next gain is the first of the turn (603.2g).
#[test]
fn vanguard_seraph_a_gain_that_could_not_happen_is_not_the_first() {
    let mut t = game(&["Vanguard Seraph"], &[HEAL, DISENCHANT, NO_GAIN]);
    to_main(&mut t);
    put(&mut t, 0, "Vanguard Seraph", Zone::Battlefield);
    let vortex = put(&mut t, 1, "No Gain", Zone::Battlefield);
    cast(&mut t, 0, "Heal", vec![], "");
    assert_eq!(t.g.players[0].life, 20, "players can't gain life");
    cast(&mut t, 0, "Disenchant", vec![vec![Target::Object(vortex)]], "");
    let card = put(&mut t, 0, "Heal", Zone::Hand);
    t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..2 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    cite!(["119.7", "603.2g", "603.2"]; assert_eq!(t.g.stack.len(), 1, "the first life gain of the turn"));
}

/// A creature destroyed while its going to the graveyard is replaced by exile (614.6) didn't
/// die: Deadly Embrace counts only what died (608.2i, 700.4).
#[test]
fn a_death_replaced_by_exile_is_not_counted() {
    let mut t = game(&["Deadly Embrace", "Rest in Peace"], &[BEAR, KILL]);
    to_main(&mut t);
    let mine = put(&mut t, 0, "Bear", Zone::Battlefield);
    let theirs = put(&mut t, 1, "Bear", Zone::Battlefield);
    put(&mut t, 1, "Rest in Peace", Zone::Battlefield);
    cast(&mut t, 0, "Kill", vec![vec![Target::Object(mine)]], "");
    assert!(t.g.exile.iter().any(|o| t.g.obj(*o).chars.name == "Bear"), "exiled instead");
    let hand = t.g.players[0].hand.len();
    cast(&mut t, 0, "Deadly Embrace", vec![vec![Target::Object(theirs)]], "{3}{B}{B}");
    cite!(["614.6", "700.4", "608.2i"]; assert_eq!(t.g.players[0].hand.len(), hand, "nothing died"));
}

/// "Another creature entered the battlefield under your control this turn" is of this turn:
/// an extra turn begins with nothing having entered (500.7, 500.1); a creature entering in it
/// is counted again.
#[test]
fn an_extra_turn_begins_with_nothing_having_entered() {
    let mut t = game(&["Bristlebane Outrider"], &[BEAR, WALK]);
    to_main(&mut t);
    let outrider = put(&mut t, 0, "Bristlebane Outrider", Zone::Battlefield);
    cast(&mut t, 0, "Bear", vec![], "");
    assert_eq!(t.g.power(outrider), Some(5));
    cast(&mut t, 0, "Walk", vec![], "");
    let turn = t.g.turn;
    t.to_no_combat(0, Part::Upkeep);
    assert!(t.g.turn > turn, "the extra turn");
    cite!(["500.7", "500.1", "608.2i"]; assert_eq!(t.g.power(outrider), Some(3)));
    to_main(&mut t);
    cast(&mut t, 0, "Bear", vec![], "");
    cite!(["608.2i"]; assert_eq!(t.g.power(outrider), Some(5)));
}
