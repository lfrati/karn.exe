//! Skipped steps and phases (500.11, 614.10): "Target player skips their next draw step." is a
//! replacement effect that waits for that player's next such step and replaces it with
//! nothing. The step never begins, so nothing triggers at its beginning and nothing waiting
//! for "your next draw step" happens (614.10a). The rules' own omissions (103.8a, 508.8) use
//! no skip effect up. Which of several skips that last differently applies is the affected
//! player's choice (616.1).

use crate::support::{cite, paid};
use crate::support::oracle;
use game::legal::Asked;
use game::snapshot::Snapshot;
use game::state::{Answer, Decision};
use game::turn::{Occurrence, SkipLasts};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::turn::Phase;
use ir::Part;

const LAND: usize = 0;
const FATIGUE: usize = 1;
const QUICK_FATIGUE: usize = 2;
const MOMENT: usize = 3;
const NEXT_COMBAT: usize = 4;
const UNTAP_SKIP: usize = 5;
const NEXT_DRAW_LIFE: usize = 6;
const RAM: usize = 7;
const TIME_WALK: usize = 8;
const FREEZE: usize = 9;
const BEARS: usize = 10;
const STONEHORN: usize = 11;
const UPKEEP_SKIP: usize = 12;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        oracle("Wastes"),
        oracle("Fatigue"),
        card("Quick Fatigue", "{U}", "Instant", "Target player skips their next draw step.", None),
        oracle("Moment of Silence"),
        card("Lasting Peace", "{W}", "Instant", "Target player skips their next combat phase.", None),
        card("Frozen Morning", "{U}", "Instant", "Target player skips their next untap step.", None),
        card("Morning Gift", "{W}", "Instant", "You gain 3 life at the beginning of your next draw step.", None),
        oracle("Nyx-Fleece Ram"),
        card("Quick Walk", "{U}", "Instant", "Take an extra turn after this one.", None),
        card("Frost Grip", "{U}", "Instant", "Tap target creature. It doesn't untap during its controller's next untap step.", None),
        oracle("Grizzly Bears"),
        oracle("Stonehorn Dignitary"),
        card("Restless Night", "{B}", "Instant", "Target player skips their next upkeep step.", None),
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

/// `player` casts a new copy of this card, with enough mana, targeting `targets`; it resolves.
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let card = g.put(player, def, Zone::Hand);
    g.players[player].pool.colors = [5; 5];
    g.players[player].pool.colorless = 5;
    // whoever has priority passes until it is this player's
    for _ in 0..4 {
        if g.priority == player { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(player, paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
    card
}

/// The top of the stack resolves.
fn resolve(g: &mut Game) {
    let depth = g.stack.len();
    while g.stack.len() >= depth && depth > 0 {
        let p = g.priority;
        g.act(p, Move::Pass).unwrap();
    }
    for p in 0..g.players.len() { g.players[p].pool = Default::default(); }
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


/// Go on until this part of this turn has begun and its active player has priority.
fn to(g: &mut Game, turn: u32, part: Part) {
    for _ in 0..500 {
        if g.turn == turn && g.part == part && g.decision.is_none() && g.stack.is_empty() { return; }
        assert!(g.turn <= turn, "turn {turn} {part:?} never came");
        go(g);
    }
    panic!("turn {turn} {part:?} never came");
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

/// The skips used in this turn: whose, and what.
fn skipped_in(g: &Game, turn: u32) -> Vec<(usize, Occurrence)> {
    let mut current = 1;
    let mut out = Vec::new();
    for e in g.log.iter() {
        match e {
            Event::TurnBegan { .. } => current += 1,
            Event::Skipped { player, what, .. } if current == turn => out.push((*player, *what)),
            _ => {}
        }
    }
    out
}

const DRAW: Occurrence = Occurrence::Part(Part::Draw);
const COMBAT: Occurrence = Occurrence::Phase(Phase::Combat);

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        game::play::check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r614_10_fatigue_skips_the_opponents_next_draw_step_and_only_that_one() {
    let mut g = game();
    let ram = g.put(1, RAM, Zone::Battlefield);
    let _ = ram;
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    assert_eq!(g.skips.len(), 1);
    to(&mut g, 2, Part::PrecombatMain);
    assert_eq!(g.active, 1);
    cite!(["500.11", "614.10"]; assert_eq!(began_in(&g, 2)[..3], [Part::Untap, Part::Upkeep, Part::PrecombatMain], "the draw step never began"));
    assert_eq!(skipped_in(&g, 2), [(1, DRAW)]);
    assert!(g.players[1].hand.is_empty(), "nothing was drawn");
    cite!(["614.10"]; assert_eq!(g.players[1].life, 21, "the upkeep trigger still triggered (614.10 replaces only the draw step)"));
    assert!(g.skips.is_empty(), "the skip is used up");
    to(&mut g, 4, Part::PrecombatMain);
    cite!(["614.10"]; assert!(began_in(&g, 4).contains(&Part::Draw), "the following draw step is not skipped"));
    assert_eq!(g.players[1].hand.len(), 1);
    assert!(skipped_in(&g, 4).is_empty());
}

#[test]
fn r614_10a_two_fatigues_skip_two_draw_steps_one_each_and_ask_nothing() {
    let mut g = game();
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    assert_eq!(g.skips.len(), 2);
    to(&mut g, 2, Part::PrecombatMain);
    assert_eq!(skipped_in(&g, 2), [(1, DRAW)]);
    cite!(["614.10a"]; assert_eq!(g.skips.len(), 1, "one skip effect per draw step (614.10a)"));
    to(&mut g, 4, Part::PrecombatMain);
    cite!(["614.10a"]; assert_eq!(skipped_in(&g, 4), [(1, DRAW)]));
    to(&mut g, 6, Part::PrecombatMain);
    assert!(skipped_in(&g, 6).is_empty());
    cite!(["614.10a"]; assert_eq!(g.players[1].hand.len(), 1, "only the third draw step drew"));
}

#[test]
fn r614_10_a_skip_made_during_your_draw_step_skips_the_next_one() {
    let mut g = game();
    to(&mut g, 3, Part::Draw);
    assert_eq!(g.players[0].hand.len(), 1);
    cast(&mut g, 0, QUICK_FATIGUE, vec![vec![Target::Player(0)]], "U");
    cite!(["614.10"]; assert_eq!(g.part, Part::Draw, "the step in progress is not skipped: it already began"));
    to(&mut g, 5, Part::PrecombatMain);
    cite!(["614.10"]; assert_eq!(skipped_in(&g, 5), [(0, DRAW)]));
    assert_eq!(g.players[0].hand.len(), 1, "turn 3's draw only: turn 5's was skipped");
}

#[test]
fn r614_10a_what_waits_for_the_next_draw_step_waits_past_a_skipped_one() {
    let mut g = game();
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    cast(&mut g, 1, NEXT_DRAW_LIFE, vec![], "W");
    to(&mut g, 2, Part::PrecombatMain);
    cite!(["614.10a"]; assert_eq!(g.players[1].life, 20, "the skipped draw step never began: nothing triggered"));
    to(&mut g, 4, Part::PrecombatMain);
    cite!(["614.10a"]; assert_eq!(g.players[1].life, 23, "its next draw step that begins is turn 4's"));
}

#[test]
fn r614_10_a_skip_is_kept_by_a_clone_and_used_once_in_each() {
    let mut g = game();
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        to(g, 4, Part::PrecombatMain);
        cite!(["614.10"]; assert_eq!(skipped_in(g, 2), [(1, DRAW)]));
        cite!(["614.10a"]; assert!(skipped_in(g, 4).is_empty()));
        assert_eq!(g.players[1].hand.len(), 1);
    }
}

#[test]
fn r614_10_a_skip_waits_through_another_players_extra_turn() {
    let mut g = game();
    cast(&mut g, 0, FATIGUE, vec![vec![Target::Player(1)]], "CU");
    cast(&mut g, 0, TIME_WALK, vec![], "U");
    to(&mut g, 2, Part::PrecombatMain);
    cite!(["500.7"]; assert_eq!(g.active, 0, "the extra turn (500.7)"));
    cite!(["614.10a"]; assert!(skipped_in(&g, 2).is_empty(), "player 0's draw step is not player 1's"));
    assert_eq!(g.players[0].hand.len(), 1);
    to(&mut g, 3, Part::PrecombatMain);
    assert_eq!(g.active, 1);
    cite!(["614.10a"]; assert_eq!(skipped_in(&g, 3), [(1, DRAW)]));
    assert!(g.players[1].hand.is_empty());
}

#[test]
fn r103_8a_the_first_draw_omission_uses_no_skip_effect() {
    let mut g = game();
    g.begin();
    assert_eq!(g.part, Part::Upkeep);
    cast(&mut g, 0, QUICK_FATIGUE, vec![vec![Target::Player(0)]], "U");
    to(&mut g, 1, Part::PrecombatMain);
    cite!(["103.8a"]; assert!(skipped_in(&g, 1).is_empty(), "the rules left the draw step out (103.8a): no skip was used"));
    assert_eq!(g.skips.len(), 1);
    to(&mut g, 3, Part::PrecombatMain);
    cite!(["614.10"]; assert_eq!(skipped_in(&g, 3), [(0, DRAW)]));
    assert!(g.players[0].hand.is_empty(), "no draw on turn 1 (103.8a) nor turn 3 (skipped)");
}

#[test]
fn r502_an_untap_step_skipped_at_a_turn_start_untaps_nothing_and_the_turn_starts_with_its_upkeep() {
    let mut g = game();
    let land = g.put(1, LAND, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.set_tapped(land, true);
    cast(&mut g, 0, UNTAP_SKIP, vec![vec![Target::Player(1)]], "U");
    // and a creature that doesn't untap during its controller's next untap step (502.3)
    cast(&mut g, 0, FREEZE, vec![vec![Target::Object(bears)]], "U");
    to(&mut g, 2, Part::Upkeep);
    cite!(["500.11", "614.10"]; assert_eq!(began_in(&g, 2).first(), Some(&Part::Upkeep), "the untap step never began"));
    assert_eq!(skipped_in(&g, 2), [(1, Occurrence::Part(Part::Untap))]);
    cite!(["614.10a"]; assert!(g.obj(land).tapped, "nothing untapped"));
    assert!(g.obj(bears).tapped);
    to(&mut g, 4, Part::Upkeep);
    assert!(!g.obj(land).tapped, "turn 4's untap step happened");
    cite!(["502.3"]; assert!(g.obj(bears).tapped, "its controller's next untap step that happened was turn 4's (502.3)"));
    to(&mut g, 6, Part::Upkeep);
    assert!(!g.obj(bears).tapped);
}

#[test]
fn r614_10_an_upkeep_skip_keeps_upkeep_triggers_from_triggering() {
    let mut g = game();
    g.put(1, RAM, Zone::Battlefield);
    cast(&mut g, 0, UPKEEP_SKIP, vec![vec![Target::Player(1)]], "B");
    to(&mut g, 2, Part::PrecombatMain);
    cite!(["614.10"]; assert_eq!(skipped_in(&g, 2), [(1, Occurrence::Part(Part::Upkeep))]));
    cite!(["614.10a"]; assert_eq!(g.players[1].life, 20, "the upkeep never began: Nyx-Fleece Ram did not trigger"));
    assert_eq!(g.players[1].hand.len(), 1, "the draw step still happened");
}

/// Player 0's game in their precombat main phase, with an older skip of their next combat
/// phase and a newer one of their next combat phase this turn.
fn competing() -> Game {
    let mut g = game();
    cast(&mut g, 0, NEXT_COMBAT, vec![vec![Target::Player(0)]], "W");
    cast(&mut g, 0, MOMENT, vec![vec![Target::Player(0)]], "W");
    assert_eq!(g.skips.iter().map(|s| s.lasts).collect::<Vec<_>>(), [SkipLasts::Next, SkipLasts::NextThisTurn { turn: 1 }]);
    // the precombat main phase ends
    assert_eq!(g.part, Part::PrecombatMain);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    g
}

#[test]
fn r616_1_competing_skips_of_a_combat_phase_are_asked_of_its_player_before_it_begins() {
    let mut g = competing();
    let Some((player, Asked::Skip { what, among })) = g.asked() else { panic!("expected a skip choice, got {:?}", g.asked()) };
    cite!(["616.1"]; assert_eq!((player, what), (0, COMBAT)));
    cite!(["616.1e"]; assert_eq!(among.len(), 2));
    cite!(["614.10"]; assert!(!began_in(&g, 1).contains(&Part::BeginningOfCombat), "nothing of the combat phase began"));
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, .. })), "no player has priority");
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    cite!(["616.1"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Skip(among[0].id))));
    cite!(["616.1"]; refused("choose one of the skip effects offered (616.1)"): g.act(0, Move::Choose(Answer::Skip(game::turn::SkipId(999)))));
    let seen = Snapshot::of(&g, Some(1));
    assert_eq!(seen.skipping, Some(COMBAT));
    assert_eq!(seen.skips.len(), 2);
}

#[test]
fn r616_1_answered_with_this_turns_skip_the_older_one_waits_for_the_next_turns_combat() {
    let mut g = competing();
    let Some((_, Asked::Skip { among, .. })) = g.asked() else { panic!() };
    let mut other = g.clone();
    let moment = among.iter().find(|s| s.lasts != SkipLasts::Next).unwrap().id;
    let older = among.iter().find(|s| s.lasts == SkipLasts::Next).unwrap().id;

    g.act(0, Move::Choose(Answer::Skip(moment))).unwrap();
    assert_eq!(g.part, Part::PostcombatMain, "the combat phase was skipped whole");
    cite!(["616.1f"]; assert_eq!(skipped_in(&g, 1), [(0, COMBAT)]));
    assert!(!began_in(&g, 1).contains(&Part::BeginningOfCombat));
    to(&mut g, 3, Part::PostcombatMain);
    cite!(["614.10a"]; assert_eq!(skipped_in(&g, 3), [(0, COMBAT)], "the older skip waited for turn 3's combat"));
    assert!(!began_in(&g, 3).contains(&Part::BeginningOfCombat));

    // the clone made at the question answers the other way, on its own
    other.act(0, Move::Choose(Answer::Skip(older))).unwrap();
    assert_eq!(other.part, Part::PostcombatMain);
    // two skip effects: one is satisfied by the first skipped combat, the other remains (614.10a)
    cite!(["614.10a"]; assert_eq!(other.skips.len(), 1, "this turn's skip waits for a combat this turn"));
    to(&mut other, 2, Part::Upkeep);
    cite!(["611.2a"]; assert!(other.skips.is_empty(), "and ends with the turn (614.10a)"));
    to(&mut other, 3, Part::PostcombatMain);
    assert!(skipped_in(&other, 3).is_empty());
    assert!(began_in(&other, 3).contains(&Part::BeginningOfCombat), "turn 3's combat happens");
    // and the first game was not changed by the clone's answer
    assert!(!began_in(&g, 3).contains(&Part::BeginningOfCombat));
}

#[test]
fn r614_10a_equivalent_skips_of_a_combat_phase_ask_nothing() {
    let mut g = game();
    cast(&mut g, 0, NEXT_COMBAT, vec![vec![Target::Player(0)]], "W");
    cast(&mut g, 0, NEXT_COMBAT, vec![vec![Target::Player(0)]], "W");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::PostcombatMain, "no question: either applies alike");
    assert_eq!(skipped_in(&g, 1), [(0, COMBAT)]);
    cite!(["614.10a"]; assert_eq!(g.skips.len(), 1));
}

#[test]
fn r614_10a_moment_of_silence_on_the_player_whose_turn_it_is_not_does_nothing_and_ends_with_the_turn() {
    let mut g = game();
    cast(&mut g, 0, MOMENT, vec![vec![Target::Player(1)]], "W");
    to(&mut g, 1, Part::PostcombatMain);
    // Uncited (ruling a61cd3f10cefb679): "If cast on a player when it is not their turn, it has no effect."
    assert!(began_in(&g, 1).contains(&Part::BeginningOfCombat), "player 0's combat is not player 1's");
    assert_eq!(g.skips.len(), 1);
    to(&mut g, 2, Part::PostcombatMain);
    assert!(g.skips.is_empty());
    cite!(["611.2a"]; assert!(began_in(&g, 2).contains(&Part::BeginningOfCombat), "not next turn's"));
}

#[test]
fn r614_10a_moment_of_silence_after_the_last_combat_is_unused_and_dropped() {
    let mut g = game();
    to(&mut g, 1, Part::PostcombatMain);
    cast(&mut g, 0, MOMENT, vec![vec![Target::Player(0)]], "W");
    to(&mut g, 3, Part::PostcombatMain);
    assert!(g.skips.is_empty());
    cite!(["611.2a"]; assert!(began_in(&g, 3).contains(&Part::BeginningOfCombat)));
    assert!(skipped_in(&g, 3).is_empty());
}

#[test]
fn r614_10_stonehorn_dignitary_skips_the_opponents_next_combat_phase() {
    let mut g = game();
    cast(&mut g, 0, STONEHORN, vec![], "CCCW");
    // its enters trigger targets the opponent
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    resolve(&mut g);
    assert_eq!(g.skips.len(), 1);
    to(&mut g, 1, Part::PostcombatMain);
    assert!(began_in(&g, 1).contains(&Part::BeginningOfCombat), "only the opponent's");
    to(&mut g, 2, Part::PostcombatMain);
    cite!(["614.10"]; assert_eq!(skipped_in(&g, 2), [(1, COMBAT)]));
    cite!(["500.11"]; assert!(!began_in(&g, 2).contains(&Part::BeginningOfCombat)));
}

#[test]
fn r614_10_ivory_gargoyle_skips_the_next_draw_step_as_its_dies_trigger_resolves_not_at_the_end_step() {
    let mut g = Game::new(vec![oracle("Wastes"), oracle("Ivory Gargoyle"), CardDef::new("Doom", "{B}", "Instant", "Destroy target creature.", None).unwrap()], 2, 20);
    for p in 0..2 {
        for _ in 0..12 {
            g.put(p, 0, Zone::Library);
        }
    }
    let gargoyle = g.put(0, 1, Zone::Battlefield);
    // it dies in the opponent's end step, after that step began: the "next end step" that
    // returns it is player 0's turn 3 one, after turn 3's draw step
    to(&mut g, 2, Part::End);
    let doom = g.put(1, 2, Zone::Hand);
    g.players[1].pool.colors = [5; 5];
    g.act(1, paid("B", Move::Cast { card: doom, targets: vec![vec![Target::Object(gargoyle)]] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.skips.len(), 1, "the skip exists as the trigger resolved (ruling cd58a76a55974fb3)");
    to(&mut g, 3, Part::PrecombatMain);
    cite!(["614.10"]; assert_eq!(skipped_in(&g, 3), [(0, DRAW)]));
    assert!(g.players[0].hand.is_empty());
    assert!(!g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Ivory Gargoyle"), "not back before this end step");
    to(&mut g, 3, Part::End);
    resolve(&mut g);
    assert!(g.battlefield.iter().any(|id| g.obj(*id).chars.name == "Ivory Gargoyle"), "back at the beginning of the next end step");
}
