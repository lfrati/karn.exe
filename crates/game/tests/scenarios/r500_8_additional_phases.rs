//! Additional phases (500.8): "After this main phase, there is an additional combat phase
//! followed by an additional main phase." The phases are added directly after the phase named,
//! when it is the one in progress, as one block; the block added most recently comes first.
//! They are phases of the turn in progress, the active player's, whoever's effect added them
//! (ruling 6879dae3c4b0c251). Every main phase after the first is a postcombat main phase
//! (505.1a). Each combat phase is a whole one: its own attackers, its own first-strike damage
//! step (510.4), its own end of combat (511), and whatever happens "this turn" spans them all.

use crate::support::{cite, mana_pool, paid};
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::turn::Phase;
use ir::Part;

const LAND: usize = 0;
const SEIZE: usize = 1;
const QUICK_SEIZE: usize = 2;
const BEARS: usize = 3;
const STRIKER: usize = 4;
const GROWTH: usize = 5;
const LATER: usize = 6;
const MOMENT: usize = 7;
const CHRONICLE: usize = 8;
const ASSAULT: usize = 9;
const CHARGER: usize = 10;
const SORCERY: usize = 11;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        crate::support::oracle("Wastes"),
        crate::support::oracle("Seize the Day"),
        card(
            "Quick Seize",
            "{R}",
            "Instant",
            "Untap target creature. After this main phase, there is an additional combat phase followed by an additional main phase.",
            None,
        ),
        crate::support::oracle("Grizzly Bears"),
        card("Striker", "{1}{W}", "Creature — Soldier", "First strike", Some((2, 2))),
        card("Growth", "{G}", "Instant", "Target creature gets +3/+3 until end of turn.", None),
        card("Later", "{W}", "Instant", "You gain 3 life at end of combat.", None),
        crate::support::oracle("Moment of Silence"),
        card("Chronicle", "{0}", "Enchantment — Saga", "I — You gain 1 life.\nII — You gain 2 life.\nIII — You gain 3 life.", None),
        crate::support::oracle("Aggravated Assault"),
        crate::support::oracle("Hellkite Charger"),
        card("Study", "{U}", "Sorcery", "You gain 1 life.", None),
    ]
}

/// Two players at player 0's first precombat main phase, each with cards to draw.
fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 {
        for _ in 0..12 {
            g.put(p, LAND, Zone::Library);
        }
    }
    g
}

fn mana(g: &mut Game, player: usize) {
    g.players[player].pool.colors = [9; 5];
    g.players[player].pool.colorless = 9;
}

/// `player` gets priority (others passing), casts `card` with enough mana, and it resolves.
fn cast_card(g: &mut Game, player: usize, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) {
    mana(g, player);
    for _ in 0..4 {
        if g.priority == player {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(player, paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
}

/// A new copy of this card in `player`'s hand is cast, and it resolves.
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(player, def, Zone::Hand);
    cast_card(g, player, card, targets, spending);
    card
}

/// The top of the stack resolves.
fn resolve(g: &mut Game) {
    let depth = g.stack.len();
    while depth > 0 && g.stack.len() >= depth {
        g.act(g.priority, Move::Pass).unwrap();
    }
    for p in 0..g.players.len() {
        g.players[p].pool = Default::default();
    }
}

/// The least a player can do: pass, or declare no attackers or blockers.
fn go(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(other) => panic!("unexpected decision {other:?}"),
        None => g.act(g.priority, Move::Pass).unwrap(),
    }
}

/// Go on until `done`, at most a few hundred moves.
fn until(g: &mut Game, done: impl Fn(&Game) -> bool) {
    for _ in 0..500 {
        if done(g) {
            return;
        }
        go(g);
    }
    panic!("never reached");
}

/// The active player has priority in this part of this turn, with the stack empty.
fn at(turn: u32, part: Part) -> impl Fn(&Game) -> bool {
    move |g: &Game| g.turn == turn && g.part == part && g.decision.is_none() && g.stack.is_empty() && g.priority == g.active
}

/// The parts begun in this turn, in order.
fn began_in(g: &Game, turn: u32) -> Vec<Part> {
    let mut current = 1;
    let mut parts = Vec::new();
    for e in g.log.iter() {
        match e {
            Event::TurnBegan { .. } => current += 1,
            Event::Began { part } if current == turn => parts.push(*part),
            _ => {}
        }
    }
    parts
}

fn count(parts: &[Part], part: Part) -> usize {
    parts.iter().filter(|p| **p == part).count()
}

/// The phases still to come this turn: which, and whether an effect added it.
fn upcoming(g: &Game) -> Vec<(Phase, bool)> {
    g.turn_plan().upcoming().map(|run| (run.phase, run.added)).collect()
}

use Part::{BeginningOfCombat as BOC, Cleanup, DeclareAttackers as DA, End, EndOfCombat as EOC, PostcombatMain as M2};

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        game::play::check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r500_8_from_the_precombat_main_phase_the_added_combat_comes_before_the_regular_one() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    cite!(["500.8"]; assert_eq!(
        upcoming(&g),
        [(Phase::Combat, true), (Phase::PostcombatMain, true), (Phase::Combat, false), (Phase::PostcombatMain, false), (Phase::Ending, false)],
        "directly after this main phase (500.8)"
    ));
    until(&mut g, at(1, BOC));
    let plan = g.turn_plan();
    let first = plan.phase().expect("a phase in progress");
    assert!(first.added && first.phase == Phase::Combat, "the first combat phase is the added one");
    assert_eq!(plan.combats(), 1);
    drop(plan);
    until(&mut g, at(1, M2));
    cite!(["505.1a"]; assert!(g.turn_plan().phase().unwrap().added, "the added main phase is a postcombat main phase (505.1a)"));
    until(&mut g, |g| g.turn == 2);
    // the blockers and damage steps are left out without attackers (508.8)
    cite!(["500.8", "508.8"]; assert_eq!(began_in(&g, 1), [BOC, DA, EOC, M2, BOC, DA, EOC, M2, End, Cleanup]));
}

#[test]
fn r500_8_from_the_postcombat_main_phase_the_added_phases_come_before_the_ending_phase() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, at(1, M2));
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    cite!(["500.8"]; assert_eq!(upcoming(&g), [(Phase::Combat, true), (Phase::PostcombatMain, true), (Phase::Ending, false)]));
    until(&mut g, |g| g.turn == 2);
    assert_eq!(began_in(&g, 1), [BOC, DA, EOC, M2, BOC, DA, EOC, M2, End, Cleanup]);
}

#[test]
fn r500_8_the_block_added_most_recently_comes_first_and_its_phases_stay_together() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, at(1, M2));
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    let first: Vec<_> = g.turn_plan().upcoming().map(|run| run.id).collect();
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    let plan = g.turn_plan();
    let runs: Vec<_> = plan.upcoming().map(|run| (run.phase, run.id)).collect();
    assert_eq!(runs.len(), 5);
    assert_eq!(runs.iter().map(|r| r.0).collect::<Vec<_>>(), [Phase::Combat, Phase::PostcombatMain, Phase::Combat, Phase::PostcombatMain, Phase::Ending]);
    cite!(["500.8"]; assert_eq!(runs[2..].iter().map(|r| r.1).collect::<Vec<_>>(), first, "the earlier block follows the newer one, whole"));
    cite!(["500.8"]; assert!(runs[0].1 > first[0] && runs[1].1 > first[0], "the newer block is first, its main phase directly after its combat"));
}

#[test]
fn r500_8_flashback_in_the_added_main_phase_adds_a_third_combat_phase() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, at(1, M2));
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    let &[seize] = g.players[0].graveyard.as_slice() else { panic!("Seize the Day in the graveyard") };
    until(&mut g, |g| at(1, M2)(g) && g.turn_plan().phase().is_some_and(|run| run.added));
    // sorcery timing holds in an added main phase (505.1a): flashback casts it from the graveyard
    mana(&mut g,0);
    g.act(0,paid("CCR", Move::CastVia(game::announcement::CastRoute::Flashback,Box::new(Move::Cast{card:seize,targets:vec![vec![Target::Object(bears)]]})))).unwrap();
    resolve(&mut g);
    cite!(["702.34a"]; assert!(g.players[0].graveyard.is_empty() && g.exile.len() == 1, "flashback exiles it"));
    cite!(["500.8"]; assert_eq!(upcoming(&g), [(Phase::Combat, true), (Phase::PostcombatMain, true), (Phase::Ending, false)]));
    until(&mut g, |g| g.turn == 2);
    cite!(["500.8"]; assert_eq!(count(&began_in(&g, 1), BOC), 3));
    assert_eq!(count(&began_in(&g, 1), M2), 3);
}

#[test]
fn r500_8_outside_a_main_phase_only_the_creature_untaps() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, at(1, BOC));
    g.set_tapped(bears, true);
    let before = upcoming(&g);
    cast(&mut g, 0, QUICK_SEIZE, vec![vec![Target::Object(bears)]], "R");
    assert!(!g.obj(bears).tapped, "the creature untaps");
    // Uncited (ruling): 6879dae3c4b0c251, cast not during a main phase "all that happens is you untap the target creature"
    assert_eq!(upcoming(&g), before, "no phase is added: this is not a main phase (ruling 6879dae3c4b0c251)");
    until(&mut g, at(1, End));
    cast(&mut g, 0, QUICK_SEIZE, vec![vec![Target::Object(bears)]], "R");
    until(&mut g, |g| g.turn == 2);
    assert_eq!(count(&began_in(&g, 1), BOC), 1);
}

#[test]
fn r500_8_cast_in_an_opponents_main_phase_that_opponent_gets_the_combat() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    // player 1 casts it in player 0's precombat main phase
    cast(&mut g, 1, QUICK_SEIZE, vec![vec![Target::Object(theirs)]], "R");
    cite!(["500.8"]; assert_eq!(upcoming(&g)[..2], [(Phase::Combat, true), (Phase::PostcombatMain, true)]));
    until(&mut g, |g| g.decision.is_some());
    cite!(["506.2"]; assert_eq!(g.decision, Some(Decision::Attackers { player: 0 }), "it is still player 0's turn: player 0 attacks"));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    until(&mut g, |g| g.turn == 2);
    assert_eq!(g.players[1].life, 18);
    assert_eq!(count(&began_in(&g, 1), BOC), 2);
}

#[test]
fn r500_8_what_lasts_the_turn_spans_every_phase_and_the_turn_ends_once() {
    let mut g = game();
    let saga = g.put(0, CHRONICLE, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    until(&mut g, at(3, Part::PrecombatMain));
    assert_eq!(g.obj(saga).counters.get("lore").copied(), Some(1));
    let land = g.put(0, LAND, Zone::Hand);
    let other = g.put(0, LAND, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    cast(&mut g, 0, GROWTH, vec![vec![Target::Object(bears)]], "G");
    cast(&mut g, 0, BEARS, vec![], "CG");
    let fresh = g.battlefield.iter().copied().find(|id| *id != bears && g.obj(*id).chars.name == "Grizzly Bears").expect("the new Bears");
    // the regular combat: Bears attack
    until(&mut g, |g| g.decision.is_some());
    let Some((_, game::legal::Asked::Attackers(able))) = g.asked() else { panic!("{:?}", g.asked()) };
    cite!(["302.6"]; assert!(!able.iter().any(|a| a.creature == fresh), "summoning sick (302.6)"));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    until(&mut g, at(3, M2));
    assert_eq!(g.players[1].life, 15, "5/5 until end of turn");
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    assert!(!g.obj(bears).tapped);
    // the added combat: the untapped Bears attack again, still 5/5; the new one is still sick
    until(&mut g, |g| g.decision.is_some());
    let Some((_, game::legal::Asked::Attackers(able))) = g.asked() else { panic!("{:?}", g.asked()) };
    assert!(able.iter().any(|a| a.creature == bears));
    cite!(["302.6"]; assert!(!able.iter().any(|a| a.creature == fresh), "still summoning sick: it is the same turn"));
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    until(&mut g, |g| at(3, M2)(g) && g.turn_plan().phase().is_some_and(|run| run.added));
    cite!(["611.2a"]; assert_eq!(g.players[1].life, 10));
    cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(other)));
    cast(&mut g, 0, SORCERY, vec![], "U");
    until(&mut g, |g| g.turn == 4);
    let parts = began_in(&g, 3);
    cite!(["505.1a"]; assert_eq!(count(&parts, Part::PrecombatMain), 1, "only the first main phase is precombat (505.1a)"));
    assert_eq!(count(&parts, Cleanup), 1);
    assert_eq!(count(&parts, End), 1);
    cite!(["714.3c"]; assert_eq!(g.obj(saga).counters.get("lore").copied().unwrap_or(0), 1, "one lore counter: one precombat main phase (714.3c)"));
    cite!(["514.2"]; assert_eq!(g.power(bears), Some(2), "until end of turn ended at the cleanup step (514.2)"));
}

#[test]
fn r510_4_each_combat_phase_starts_empty_and_has_its_own_first_strike_damage_step() {
    let mut g = game();
    until(&mut g, at(3, Part::PrecombatMain));
    let striker = g.put(0, STRIKER, Zone::Battlefield);
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(striker)]], "CCCR");
    for _ in 0..2 {
        until(&mut g, |g| g.decision.is_some());
        cite!(["511.3"]; assert!(g.combat.attackers.is_empty(), "no attackers carried over from another combat phase"));
        g.act(0, Move::Attack(vec![(striker, Target::Player(1))])).unwrap();
        until(&mut g, |g| g.part == EOC);
        // the creature attacked: untap it for the next combat
        g.set_tapped(striker, false);
    }
    until(&mut g, |g| g.turn == 4);
    let parts = began_in(&g, 3);
    cite!(["510.4"]; assert_eq!(count(&parts, Part::CombatDamage), 4, "a first-strike and a regular damage step in each combat (510.4)"));
    assert_eq!(count(&parts, EOC), 2);
    assert_eq!(g.players[1].life, 16);
}

#[test]
fn r603_7b_an_end_of_combat_delayed_trigger_fires_at_the_next_end_of_combat_step_to_begin() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    // created during the added combat's beginning: that same combat's end step
    until(&mut g, at(1, BOC));
    cast(&mut g, 0, LATER, vec![], "W");
    assert_eq!(g.players[0].life, 20);
    until(&mut g, at(1, EOC));
    cite!(["603.7b"]; assert_eq!(g.players[0].life, 23, "this combat's end, though its phase began before the trigger existed"));
    // created after that end step began: the regular combat's end step
    cast(&mut g, 0, LATER, vec![], "W");
    until(&mut g, at(1, M2));
    assert_eq!(g.players[0].life, 23);
    until(&mut g, |g| at(1, EOC)(g) && !g.turn_plan().phase().unwrap().added);
    cite!(["603.7b"]; assert_eq!(g.players[0].life, 26));
}

#[test]
fn r614_10_moment_of_silence_skips_the_added_combat_when_it_comes_first() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, 0, SEIZE, vec![vec![Target::Object(bears)]], "CCCR");
    cast(&mut g, 0, MOMENT, vec![vec![Target::Player(0)]], "W");
    until(&mut g, |g| g.turn == 2);
    let skipped: Vec<_> = g.log.iter().filter(|e| matches!(e, Event::Skipped { .. })).collect();
    cite!(["614.10"]; assert_eq!(skipped.len(), 1));
    // the added combat was skipped whole; its main phase and the regular combat happened
    cite!(["614.10", "500.8"]; assert_eq!(began_in(&g, 1), [M2, BOC, DA, EOC, M2, End, Cleanup]));
    assert!(g.skips.is_empty());
}

#[test]
fn r602_5d_aggravated_assault_adds_phases_from_a_main_phase_and_only_as_a_sorcery() {
    let mut g = game();
    let assault = g.put(0, ASSAULT, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.set_tapped(bears, true);
    mana(&mut g, 0);
    g.act(0, paid("CCCRR", Move::Activate { source: assault, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    resolve(&mut g);
    assert!(!g.obj(bears).tapped);
    cite!(["500.8"]; assert_eq!(upcoming(&g)[..2], [(Phase::Combat, true), (Phase::PostcombatMain, true)]));
    until(&mut g, at(1, BOC));
    mana(&mut g, 0);
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): g.act(0, paid("CCCRR", Move::Activate { source: assault, ability: 0, targets: vec![], paying: vec![] })));
}

#[test]
fn r500_8_hellkite_charger_adds_a_combat_directly_after_this_combat() {
    let mut g = game();
    until(&mut g, at(3, Part::PrecombatMain));
    let charger = g.put(0, CHARGER, Zone::Battlefield);
    until(&mut g, |g| g.decision.is_some());
    g.act(0, Move::Attack(vec![(charger, Target::Player(1))])).unwrap();
    // its trigger: pay {5}{R}{R}
    until(&mut g, |g| !g.stack.is_empty() || g.decision.is_some());
    mana(&mut g, 0);
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Choose { player, .. }) => {
                g.act(player, Move::Choose(game::state::Answer::Pay{plan:vec![],with:Some(mana_pool("CCCCCRR")),tapped:vec![]})).unwrap();
            }
            Some(other) => panic!("{other:?}"),
            None if !g.stack.is_empty() => g.act(g.priority, Move::Pass).unwrap(),
            None => break,
        }
    }
    assert!(!g.obj(charger).tapped, "attacking creatures untapped");
    // Hellkite Charger "after this phase, there is an additional combat phase": added directly after it, with no main phase between (500.8; ruling 484e5ccb62bc7e40)
    cite!(["500.8"]; assert_eq!(upcoming(&g), [(Phase::Combat, true), (Phase::PostcombatMain, false), (Phase::Ending, false)], "no main phase between (ruling 484e5ccb62bc7e40)"));
    until(&mut g, |g| g.turn == 4);
    cite!(["500.8"]; assert_eq!(&began_in(&g, 3)[4..], [BOC, DA, Part::DeclareBlockers, Part::CombatDamage, EOC, BOC, DA, EOC, M2, End, Cleanup]));
}
