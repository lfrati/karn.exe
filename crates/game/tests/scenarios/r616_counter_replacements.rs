//! Several replacement effects on one placement of counters (616.1, 614.5): the affected
//! permanent's controller chooses which applies next, each effect applies once, and the
//! counters of one instruction are put together once every order is chosen.
//!
//! Both replacement cards and the spells exercising them pass admission. Entry placements
//! use the same replacement ordering and are covered in r614_counter_entry.rs.
use crate::support::{cite, paid};
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const SCALES: usize = 0;
const EVOLUTION: usize = 1;
const BATTLEGROWTH: usize = 2;
const BOON: usize = 3;
const BEAR: usize = 4;
const RAIN: usize = 5;

const SCALES_TEXT: &str = "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.";
const EVOLUTION_TEXT: &str = "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.";

fn game(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Hardened Scales", "{G}", "Enchantment", SCALES_TEXT, None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", EVOLUTION_TEXT, None).unwrap(),
        CardDef::new("Battlegrowth", "{G}", "Instant", "Put a +1/+1 counter on target creature.", None).unwrap(),
        CardDef::new("Titania's Boon", "{3}{G}", "Sorcery", "Put a +1/+1 counter on each creature you control.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Rain", "{G}", "Sorcery", "Put a +1/+1 counter on each creature.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    let mut g = Game::new(defs, players, 20);
    g.players[0].pool.colors = [10; 5];
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    resolve(g);
}

fn counters(g: &Game, o: ObjId) -> u32 {
    g.obj(o).counters.get("+1/+1").copied().unwrap_or(0)
}

fn placed(g: &Game) -> Vec<(ObjId, u32)> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::CountersPut { on: Target::Object(o), count, .. } => Some((*o, *count)),
            _ => None,
        })
        .collect()
}

fn effect(source: ObjId) -> ReplacementId {
    ReplacementId::from_ability(source, game::binding::AbilityKey::printed(0), 0)
}

fn choose(g: &mut Game, player: usize, source: ObjId) {
    g.act(player, Move::Choose(Answer::Replacement(effect(source)))).unwrap();
}

/// What is asked: of whom, about which permanent, how many counters as they now stand, and
/// which effects.
fn question(g: &Game) -> (usize, ObjId, u32, Vec<ReplacementId>) {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { on: Target::Object(on), kind, count, among, optional: false, .. } }) => {
            assert_eq!(kind, "+1/+1");
            (*player, *on, *count, among.clone())
        }
        other => panic!("no replacement question: {other:?}"),
    }
}

#[test]
fn r122_6_admission_accepts_plus_one_replacements_but_refuses_other_counter_kinds() {
    game(2);
    let text = SCALES_TEXT.replace("+1/+1", "charge");
    let def = CardDef::new("Charge replacement", "{G}", "Enchantment", &text, None).unwrap();
    let refused = game::play::check(&def).unwrap_err();
    assert!(refused.0.contains("named permanent-counter replacements currently require +1/+1"), "{}", refused.0);
}

#[test]
fn r616_1_the_plus_one_applied_before_the_doubling_puts_four() {
    let mut g = game(2);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    // the spell on the stack is a new object (400.7)
    let spell = g.resolving().expect("the spell waits for the order").object;
    cite!(["616.1"]; assert_eq!(question(&g), (0, bear, 1, vec![effect(scales), effect(evolution)])));
    assert_eq!(counters(&g, bear), 0, "nothing is put while the order is chosen");
    choose(&mut g, 0, scales);
    // the doubling alone is left and applies without a question (616.1e, 614.5)
    assert_eq!(g.decision, None);
    cite!(["616.1f"]; assert_eq!(counters(&g, bear), 4));
    cite!(["614.6"]; assert_eq!(placed(&g), vec![(bear, 4)], "only the replaced event happens (614.6)"));
    assert!(g.log.contains(&Event::Resolved { spell }));
    assert!(g.resolving().is_none());
    assert_eq!(g.priority, g.active);
}

#[test]
fn r616_1_the_doubling_applied_before_the_plus_one_puts_three() {
    let mut g = game(2);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    cite!(["616.1"]; assert_eq!(question(&g).3, vec![effect(scales), effect(evolution)]));
    choose(&mut g, 0, evolution);
    assert_eq!(g.decision, None);
    assert_eq!(counters(&g, bear), 3);
    cite!(["616.1f"]; assert_eq!(placed(&g), vec![(bear, 3)]));
}

#[test]
fn r616_1_the_affected_permanents_controller_chooses_not_the_spells() {
    let mut g = game(2);
    let scales = g.put(1, SCALES, Zone::Battlefield);
    let evolution = g.put(1, EVOLUTION, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    cite!(["616.1"]; assert_eq!(question(&g).0, 1));
    let before = (g.decision.clone(), g.log.len());
    cite!(["616.1"]; refused("that decision is another player's"): g.act(0, Move::Choose(Answer::Replacement(effect(scales)))));
    assert_eq!((g.decision.clone(), g.log.len()), before);
    choose(&mut g, 1, evolution);
    assert_eq!(counters(&g, bear), 3);
}

#[test]
fn r614_1a_an_effect_about_your_creatures_leaves_an_opponents_alone() {
    let mut g = game(2);
    g.put(0, SCALES, Zone::Battlefield);
    g.put(0, EVOLUTION, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(theirs)]], "G");
    assert_eq!(g.decision, None);
    cite!(["109.5"]; assert_eq!(placed(&g), vec![(theirs, 1)]));
}

#[test]
fn r616_1_an_answer_that_is_not_an_offered_effect_changes_nothing() {
    let mut g = game(2);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    g.put(0, EVOLUTION, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    let before = (g.decision.clone(), g.log.clone(), g.stack.clone());
    let unoffered = "choose one of the replacement effects that apply (616.1)";
    for (wrong, why) in [
        (Move::Choose(Answer::Replacement(effect(bear))), unoffered),
        (Move::Choose(Answer::Replacement(ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(1), 0))), unoffered),
        (Move::Choose(Answer::Yes), unoffered),
        (Move::Pass, "a decision is pending: a choice of the effect being applied (608.2d)"),
    ] {
        if why == unoffered { cite!(["616.1"]; case: format!("{wrong:?}"); refused(why): g.act(0, wrong)); }
        else { crate::support::refused(g.act(0, wrong), why); }
        assert_eq!((g.decision.clone(), g.log.clone(), g.stack.clone()), before);
        assert_eq!(counters(&g, bear), 0);
    }
    // the question is still answerable, and is answered once
    choose(&mut g, 0, scales);
    assert_eq!(placed(&g), vec![(bear, 4)]);
    crate::support::refused(g.act(0, Move::Choose(Answer::Replacement(effect(scales)))), "no decision is pending");
    assert_eq!(placed(&g), vec![(bear, 4)]);
}

#[test]
fn r614_5_two_instances_of_one_text_each_apply_once_in_either_order() {
    for first in [0, 1] {
        let mut g = game(2);
        let both = [g.put(0, SCALES, Zone::Battlefield), g.put(0, SCALES, Zone::Battlefield)];
        let bear = g.put(0, BEAR, Zone::Battlefield);
        cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
        // two effects apply, so the choice is the player's, whatever the arithmetic (616.1)
        cite!(["616.1"]; assert_eq!(question(&g), (0, bear, 1, vec![effect(both[0]), effect(both[1])])));
        choose(&mut g, 0, both[first]);
        assert_eq!(g.decision, None);
        cite!(["614.5"]; case: format!("first={first}"); assert_eq!(placed(&g), vec![(bear, 3)], "each instance once, none twice"));
    }
}

#[test]
fn r616_1_a_sole_effect_applies_without_a_question() {
    let mut g = game(2);
    g.put(0, SCALES, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    cite!(["616.1"]; assert_eq!(g.decision, None));
    cite!(["614.1a"]; assert_eq!(placed(&g), vec![(bear, 2)]));
}

#[test]
fn r122_1_counters_are_put_as_before_where_no_replacement_applies() {
    let mut g = game(2);
    let bears = [g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield)];
    let other = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, BOON, vec![], "WWWG");
    assert_eq!(g.decision, None);
    assert_eq!(placed(&g), vec![(bears[0], 1), (bears[1], 1)]);
    assert_eq!(counters(&g, other), 0);
    cite!(["122.1a"]; assert_eq!(g.power(bears[0]), Some(3)));
}

#[test]
fn r616_1_each_creature_of_a_batch_has_its_own_order_and_none_is_put_early() {
    let mut g = game(2);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let bears = [g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield)];
    cast(&mut g, BOON, vec![], "WWWG");
    let spell = g.resolving().expect("the spell waits for the order").object;
    assert_eq!(question(&g), (0, bears[0], 1, vec![effect(scales), effect(evolution)]));
    choose(&mut g, 0, scales);
    // the first creature's order is settled, the second's is asked: nothing is put yet
    cite!(["616.1"]; assert_eq!(question(&g), (0, bears[1], 1, vec![effect(scales), effect(evolution)])));
    assert_eq!(placed(&g), vec![]);
    assert_eq!((counters(&g, bears[0]), counters(&g, bears[1])), (0, 0));
    assert!(g.resolving().is_some());
    assert!(!g.log.contains(&Event::Resolved { spell }));
    choose(&mut g, 0, evolution);
    assert_eq!(g.decision, None);
    cite!(["616.1f"]; assert_eq!(placed(&g), vec![(bears[0], 4), (bears[1], 3)]));
    assert!(g.log.contains(&Event::Resolved { spell }));
}

#[test]
fn r616_1_a_copied_game_goes_on_from_the_question_on_its_own() {
    let mut g = game(2);
    let scales = g.put(0, SCALES, Zone::Battlefield);
    let evolution = g.put(0, EVOLUTION, Zone::Battlefield);
    let bears = [g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield)];
    cast(&mut g, BOON, vec![], "WWWG");
    choose(&mut g, 0, scales);
    let mut copy = g.clone();
    choose(&mut g, 0, scales);
    assert_eq!(placed(&g), vec![(bears[0], 4), (bears[1], 4)]);
    // the copy kept the first answer and still waits for the second
    assert_eq!(question(&copy).1, bears[1]);
    assert_eq!(placed(&copy), vec![]);
    choose(&mut copy, 0, evolution);
    assert_eq!(placed(&copy), vec![(bears[0], 4), (bears[1], 3)]);
    assert_eq!(placed(&g), vec![(bears[0], 4), (bears[1], 4)]);
}

#[test]
fn r101_4_the_active_player_orders_first_then_the_others_in_turn_order() {
    let mut g = game(3);
    // the last player's creature is the first on the battlefield, the active player's last
    let mut bears = [ObjId(0); 3];
    let mut sources = [[ObjId(0); 2]; 3];
    for player in [2, 1, 0] {
        bears[player] = g.put(player, BEAR, Zone::Battlefield);
        sources[player] = [g.put(player, SCALES, Zone::Battlefield), g.put(player, EVOLUTION, Zone::Battlefield)];
    }
    assert_eq!(g.active, 0);
    cast(&mut g, RAIN, vec![], "G");
    for player in [0, 1, 2] {
        let [scales, evolution] = sources[player];
        // each is asked about their own creature and offered their own effects only
        cite!(["101.4", "616.1"]; case: format!("player {player}"); assert_eq!(question(&g), (player, bears[player], 1, vec![effect(scales), effect(evolution)])));
        cite!(["101.4"]; case: format!("player {player}"); assert_eq!(placed(&g), vec![]));
        // those who chose before are known to those who choose after (101.4b)
        let known: Vec<_> = g.public_choices().cloned().collect();
        cite!(["101.4b"]; case: format!("player {player}"); assert_eq!(known.len(), player));
        choose(&mut g, player, if player == 1 { evolution } else { scales });
    }
    assert_eq!(g.decision, None);
    assert_eq!(placed(&g), vec![(bears[2], 4), (bears[1], 3), (bears[0], 4)]);
    assert_eq!(g.public_choices().count(), 0, "the operation is gone with its choices");
}


#[test]
fn r614_5_three_effects_resume_the_same_event_without_reapplying_an_instance() {
    let mut g = game(2);
    let first = g.put(0, SCALES, Zone::Battlefield);
    let twice = g.put(0, EVOLUTION, Zone::Battlefield);
    let second = g.put(0, SCALES, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BATTLEGROWTH, vec![vec![Target::Object(bear)]], "G");
    choose(&mut g, 0, first);
    cite!(["614.5", "616.1f"]; assert_eq!(question(&g), (0, bear, 2, vec![effect(twice), effect(second)])));
    assert!(placed(&g).is_empty());
    let decision = g.decision.clone();
    let log = g.log.clone();
    cite!(["616.1"]; refused("choose one of the replacement effects that apply (616.1)"): g.act(0, Move::Choose(Answer::Replacement(effect(first)))));
    assert_eq!(g.decision, decision);
    assert_eq!(g.log, log);
    let mut alternate = g.clone();
    choose(&mut g, 0, twice);
    choose(&mut alternate, 0, second);
    cite!(["616.1f"]; assert_eq!(placed(&g), vec![(bear, 5)]));
    cite!(["616.1f"]; assert_eq!(placed(&alternate), vec![(bear, 6)]));
    assert!(g.resolving().is_none());
    assert!(alternate.resolving().is_none());
}

#[test]
fn r608_2_counter_replacement_resumes_between_optional_instructions_without_replay() {
    let scales = CardDef::new("Hardened Scales", "{G}", "Enchantment", SCALES_TEXT, None).unwrap();
    let evolution = CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", EVOLUTION_TEXT, None).unwrap();
    let spell = CardDef::new("Sequence fixture", "{0}", "Instant",
        "Draw a card. You may put a +1/+1 counter on target creature. You may draw a card.", None).unwrap();
    game::play::check(&spell).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![scales, evolution, spell, bear], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let twice = g.put(0, 1, Zone::Battlefield);
    let bear = g.put(0, 3, Zone::Battlefield);
    g.put(0, 3, Zone::Library);
    g.put(0, 3, Zone::Library);
    cast(&mut g, 2, vec![vec![Target::Object(bear)]], "");
    assert_eq!(g.players[0].hand.len(), 1);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(question(&g).2, 1);
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 1));
    assert!(placed(&g).is_empty());
    choose(&mut g, 0, twice);
    assert_eq!(placed(&g), vec![(bear, 3)]);
    assert_eq!(g.players[0].hand.len(), 1);
    assert!(g.resolving().is_some(), "the following optional draw still belongs to resolution");
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 2));
    assert_eq!(placed(&g), vec![(bear, 3)]);
    assert!(g.resolving().is_none());
    assert!(g.decision.is_none());
}
