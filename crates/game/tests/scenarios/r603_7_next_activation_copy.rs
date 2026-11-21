//! Next-event consumption and expiry are independent (603.7b); copying isn't activation
//! (707.10). Full pinned Pit/Expert text exercises the shared delayed stack-copy owner.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::legal::Asked;
use std::sync::OnceLock;

fn definitions() -> Vec<CardDef> {
    static DEFS: OnceLock<Vec<CardDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs: Vec<_> = ["Pit Automaton", "Afterburner Expert"]
            .iter().map(|name| CardDef::from_faces(catalog.get(name).unwrap()).unwrap()).collect();
        defs.extend([
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            CardDef::new("Watcher", "{0}", "Artifact", "{0}: When you next cast a creature spell this turn, you gain 1 life.", None).unwrap(),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        ]);
        for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}",def.name())); }
        defs
    }).clone()
}
fn game() -> Game {
    let mut g = Game::new(definitions(), 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 2, Zone::Library); } }
    g
}
fn activate(source: ObjId, ability: usize, spending: &str) -> Move {
    paid(spending, Move::Activate { source, ability, targets: vec![], paying: vec![] })
}
fn resolve(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
fn settle(g: &mut Game) {
    for _ in 0..24 {
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        if g.stack.is_empty() { return; }
        resolve(g);
    }
    panic!("unsettled stack");
}

#[test]
fn r603_7b_explicit_next_consumes_once_despite_this_turn_duration() {
    let mut g = game();
    let watcher = g.put(0, 3, Zone::Battlefield);
    g.act(0, activate(watcher, 0, "")).unwrap(); settle(&mut g);
    for expected in [21, 21] {
        let bear = g.put(0, 4, Zone::Hand);
        g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap(); settle(&mut g);
        cite!(["603.7b"]; assert_eq!(g.players[0].life, expected));
    }
}

#[test]
fn r707_10_two_pit_registrations_copy_one_exhaust_without_reactivating() {
    let mut g = game();
    g.players[0].pool.colorless = 6;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    for _ in 0..2 {
        let pit = g.put(0, 0, Zone::Battlefield);
        g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    }
    let expert = g.put(0, 1, Zone::Battlefield);
    g.act(0, activate(expert, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7a", "707.10b"]; assert_eq!(g.obj(expert).counters.get("+1/+1").copied().unwrap_or(0), 6));
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    cite!(["702.177a", "707.10"]; refused("only once"): g.act(0, activate(expert, 0, "CCGG")));
}

fn add(g: &mut Game, name: &str, types: &str, text: &str) -> usize {
    let def = CardDef::new(name, "{0}", types, text, None).unwrap();
    game::play::check(&def).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def); index
}

#[test]
fn r603_7_the_new_event_input_displaces_captured_registration_antecedents() {
    let mut g = game();
    let index = add(&mut g, "Registration", "Sorcery", "Tap target creature. When you next activate an exhaust ability that isn't a mana ability this turn, copy it. You may choose new targets for the copy. You gain 1 life.");
    let bear = g.put(0, 4, Zone::Battlefield);
    let card = g.put(0, index, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }).unwrap(); settle(&mut g);
    cite!(["603.7a"]; assert_eq!(g.players[0].life, 21));
    assert!(g.obj(bear).tapped);
    let expert = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colorless = 2; g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(expert, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7a", "707.10b"]; assert_eq!(g.obj(expert).counters.get("+1/+1"), Some(&4)));
    assert_eq!(g.players[0].life, 21, "unrelated next sentence belongs to registration");
}

#[test]
fn r603_7_new_delayed_body_antecedent_is_not_reinterpreted_as_the_event() {
    let def = CardDef::new("Ambiguous", "{0}", "Sorcery", "When you next activate an exhaust ability that isn't a mana ability this turn, tap target creature, then copy it.", None).unwrap();
    let why = game::play::check(&def).expect_err("permanent copy instruction is not modeled");
    assert!(why.0.contains("copy:"), "{why:?}");
}

#[test]
fn r608_2h_countered_original_is_copied_but_countered_delayed_trigger_is_consumed() {
    let mut g = game();
    let pit = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 4; g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    let expert = g.put(0, 1, Zone::Battlefield);
    g.act(0, activate(expert, 0, "CCGG")).unwrap();
    assert_eq!(g.stack.len(), 2);
    let mut delayed_countered = g.clone();
    let original = g.stack[0];
    cite!(["608.2h"]; assert!(g.counter(original)));
    settle(&mut g);
    cite!(["608.2h", "707.10b"]; assert_eq!(g.obj(expert).counters.get("+1/+1"), Some(&2)));
    let delayed = *delayed_countered.stack.last().unwrap();
    cite!(["603.7b"]; assert!(delayed_countered.counter(delayed)));
    settle(&mut delayed_countered);
    assert_eq!(delayed_countered.obj(expert).counters.get("+1/+1"), Some(&2));
    let other = delayed_countered.put(0, 1, Zone::Battlefield);
    delayed_countered.players[0].pool.colorless = 2;
    delayed_countered.players[0].pool.colors[ir::Color::Green as usize] = 2;
    delayed_countered.act(0, activate(other, 0, "CCGG")).unwrap(); settle(&mut delayed_countered);
    cite!(["603.7b"]; assert_eq!(delayed_countered.obj(other).counters.get("+1/+1"), Some(&2)));
}

#[test]
fn r707_10b_copied_ability_keeps_its_departed_source_and_sacrificed_payment_reference() {
    let mut g = game();
    let def = CardDef::new("Offering", "{0}", "Creature — Bear", "Exhaust — Sacrifice a creature: You gain life equal to the sacrificed creature's power.", Some((3, 2))).unwrap();
    game::play::check(&def).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    let pit = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    let source = g.put(0, index, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![vec![source]] }).unwrap();
    assert!(!g.battlefield.contains(&source));
    let original = g.stack[0];
    resolve(&mut g); // Delayed trigger puts its copy above the original.
    let copy = *g.stack.last().unwrap();
    cite!(["707.10b"]; assert_eq!(g.obj(copy).ability, g.obj(original).ability));
    assert_eq!(g.obj(copy).ability.unwrap().0, Some(source));
    settle(&mut g);
    cite!(["707.10"]; assert_eq!(g.players[0].life, 26));
    let activations = g.log.iter().filter(|event| matches!(event, game::Event::Activated { source: s, .. } if *s == source)).count();
    let copies = g.log.iter().filter(|event| matches!(event, game::Event::AbilityCopied { original: o, .. } if *o == original)).count();
    cite!(["707.10"]; assert_eq!((activations, copies), (1, 1)));
}

#[test]
fn r603_7a_activation_in_response_does_not_consume_a_future_registration() {
    let mut g = game();
    g.players[0].pool.colorless = 6; g.players[0].pool.colors[ir::Color::Green as usize] = 4;
    let pit = g.put(0, 0, Zone::Battlefield);
    g.act(0, activate(pit, 2, "CC")).unwrap();
    let before = g.put(0, 1, Zone::Battlefield);
    g.act(0, activate(before, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7a"]; assert_eq!(g.obj(before).counters.get("+1/+1"), Some(&2)));
    let after = g.put(0, 1, Zone::Battlefield);
    g.act(0, activate(after, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7a"]; assert_eq!(g.obj(after).counters.get("+1/+1"), Some(&4)));
}

#[test]
fn r603_7e_registration_keeps_controller_after_source_control_change_or_departure() {
    let mut g = game();
    let pit = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    let steal = add(&mut g, "Steal", "Instant", "You gain control of target creature.");
    for departing in [false, true] {
        let mut branch = g.clone();
        if departing { branch.move_object(pit, Zone::Graveyard); }
        else {
            branch.act(0, Move::Pass).unwrap();
            let card = branch.put(1, steal, Zone::Hand);
            branch.act(1, Move::Cast { card, targets: vec![vec![Target::Object(pit)]] }).unwrap();
            settle(&mut branch);
            assert_eq!(branch.controller(pit), 1);
            while branch.priority != 0 { branch.act(branch.priority, Move::Pass).unwrap(); }
        }
        let source = branch.put(0, 1, Zone::Battlefield);
        branch.players[0].pool.colorless = 2;
        branch.players[0].pool.colors[ir::Color::Green as usize] = 2;
        branch.act(0, activate(source, 0, "CCGG")).unwrap(); settle(&mut branch);
        cite!(["603.7e"]; assert_eq!(branch.obj(source).counters.get("+1/+1"), Some(&4)));
    }
}

#[test]
fn r603_7b_unused_next_registration_expires_at_cleanup() {
    let mut g = game();
    let pit = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    let turn = g.turn;
    for _ in 0..100 {
        if g.turn != turn { break; }
        let (player, asked) = g.asked().unwrap();
        g.act(player, match asked { Asked::Priority => Move::Pass, Asked::Attackers(_) => Move::Attack(vec![]), other => panic!("{other:?}") }).unwrap();
    }
    assert_ne!(g.turn, turn);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let expert = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colorless = 2; g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(expert, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7b", "514.2"]; assert_eq!(g.obj(expert).counters.get("+1/+1"), Some(&2)));
}

#[test]
fn r603_7_only_successful_controller_nonmana_exhaust_consumes_registration() {
    let mut g = game();
    let ordinary = add(&mut g, "Ordinary", "Artifact", "{0}: You gain 1 life.");
    let mana = add(&mut g, "Mana", "Artifact", "Exhaust — {0}: Add {G}.");
    let pit = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.act(0, activate(pit, 2, "CC")).unwrap(); settle(&mut g);
    let ordinary = g.put(0, ordinary, Zone::Battlefield);
    g.act(0, activate(ordinary, 0, "")).unwrap(); settle(&mut g);
    let mana = g.put(0, mana, Zone::Battlefield);
    g.act(0, activate(mana, 0, "")).unwrap();
    assert!(g.stack.is_empty(), "mana Exhaust doesn't create a stack occurrence");
    g.act(0, Move::Pass).unwrap();
    let opponent = g.put(1, 1, Zone::Battlefield);
    g.players[1].pool.colorless = 2; g.players[1].pool.colors[ir::Color::Green as usize] = 2;
    g.act(1, activate(opponent, 0, "CCGG")).unwrap(); settle(&mut g);
    assert_eq!(g.obj(opponent).counters.get("+1/+1"), Some(&2));
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let expert = g.put(0, 1, Zone::Battlefield);
    cite!(["733.1"]; refused("mana pool"): g.act(0, activate(expert, 0, "CCGG")));
    g.players[0].pool.colorless = 2; g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(expert, 0, "CCGG")).unwrap(); settle(&mut g);
    cite!(["603.7b", "605.1a", "733.1"]; assert_eq!(g.obj(expert).counters.get("+1/+1"), Some(&4)));
}

#[test]
fn r603_7_compound_retarget_sentence_is_not_absorbed_or_replayed_twice() {
    let def = CardDef::new("Compound", "{0}", "Sorcery", "When you next activate an exhaust ability that isn't a mana ability this turn, copy it. You may choose new targets for the copy, and you gain 1 life.", None).unwrap();
    let why = game::play::check(&def).expect_err("compound retarget scope is not modeled");
    assert!(why.0.contains("preceding stack copies"), "{why:?}");
}

#[test]
fn r707_10_copy_event_is_not_falsely_admitted_as_an_activation_predicate() {
    let def = CardDef::new("Copy Watcher", "{0}", "Artifact", "Whenever you copy an activated ability, you gain 1 life.", None).unwrap();
    let why = game::play::check(&def).expect_err("copy-event ability predicates are not yet modeled");
    assert!(why.0.contains("activation predicate requires an activation event"), "{why:?}");
}

#[test]
fn r707_10c_ability_retarget_preserves_mode_x_and_original_source_exclusion() {
    let mut g = game();
    let def = CardDef::new("Modal", "{0}", "Creature — Wizard", "{X}: Choose one —\n• Put X +1/+1 counters on another target creature.\n• You gain X life.", Some((1, 1))).unwrap();
    game::play::check(&def).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    let source = g.put(0, index, Zone::Battlefield);
    let first = g.put(0, 4, Zone::Battlefield);
    let second = g.put(0, 4, Zone::Battlefield);
    let registration = add(&mut g, "Ability Registration", "Artifact", "{2}: When you next activate an ability that isn't a mana ability this turn, copy it. You may choose new targets for the copy.");
    let registration = g.put(0, registration, Zone::Battlefield);
    g.players[0].pool.colorless = 5;
    g.act(0, activate(registration, 0, "CC")).unwrap(); settle(&mut g);
    g.act(0, paid("CCC", Move::WithX(3, Box::new(Move::Modes(vec![0], Box::new(Move::Activate {
        source, ability: 0, targets: vec![vec![Target::Object(first)]], paying: vec![],
    })))))).unwrap();
    let original = g.stack[0];
    let mut unchanged_illegal = g.clone();
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Retarget { copy, slots, .. } }) = g.decision.clone() else { panic!("retarget copy"); };
    cite!(["707.10", "700.2g"]; assert_eq!((g.obj(copy).x, g.obj(copy).modes.clone()), (Some(3), vec![0])));
    cite!(["707.10b"]; assert_eq!(g.obj(copy).ability, g.obj(original).ability));
    assert!(!slots[0].among.contains(&Target::Object(source)), "copy retains source-relative other exclusion");
    assert!(slots[0].among.contains(&Target::Object(second)));
    let before = format!("{g:?}");
    cite!(["707.10c"]; refused("a changed target must be legal without invalidating an unchanged target (115.7d)"): g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Object(source)]]))));
    assert_eq!(format!("{g:?}"), before);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Retarget { copy: same, .. }, .. }) if same == copy));
    g.act(0, Move::Choose(Answer::Targets(vec![vec![Target::Object(second)]]))).unwrap(); settle(&mut g);
    cite!(["707.10c"]; assert_eq!((g.obj(first).counters.get("+1/+1"), g.obj(second).counters.get("+1/+1")), (Some(&3), Some(&3))));
    assert_eq!(g.players[0].life, 20, "copied mode does not become the life-gain mode");

    unchanged_illegal.move_object(first, Zone::Graveyard);
    resolve(&mut unchanged_illegal);
    let Some(Decision::Choose { choice: Choice::Retarget { copy, .. }, .. }) = unchanged_illegal.decision.clone() else { panic!("keep invalid original target"); };
    cite!(["707.10c"]; ok: unchanged_illegal.act(0, Move::Choose(Answer::No)));
    settle(&mut unchanged_illegal);
    cite!(["608.2b"]; assert!(unchanged_illegal.log.iter().any(|event| matches!(event, game::Event::Fizzled { spell } if *spell == copy))));
    assert!(unchanged_illegal.obj(second).counters.is_empty());
}

#[test]
fn r722_707_full_codie_copies_prepared_spell_marker_without_casting_or_activation() {
    let mut g = game();
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let codie = g.defs.len();
    for name in ["Codie, Ravenous Codex", "Goblin Glasswright // Craft with Pride"] {
        let def = CardDef::from_faces(catalog.get(name).unwrap()).unwrap(); game::play::check(&def).unwrap();
        std::sync::Arc::make_mut(&mut g.defs).push(def);
    }
    let watcher = add(&mut g, "Prepared Watcher", "Artifact", "Whenever you cast a prepared spell, you gain 1 life.\nWhenever you copy a prepared spell, you gain 2 life.");
    g.put(0, codie, Zone::Battlefield); g.put(0, watcher, Zone::Battlefield);
    let source = g.put(0, codie + 1, Zone::Battlefield); g.as_entered(source);
    let prepared = g.exile[0];
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    g.act(0, paid("R", Move::Cast { card: prepared, targets: vec![] })).unwrap(); settle(&mut g);
    cite!(["722.3c", "707.10"]; assert!(!g.obj(source).prepared));
    cite!(["707.10", "722.3d"]; assert_eq!(g.players[0].life, 23, "one cast and one copy preserve prepared-spell classification"));
    let treasure = ir::Subtype::from_name("Treasure").unwrap();
    assert_eq!(g.battlefield.iter().filter(|object| g.obj(**object).chars.subtypes.contains(&treasure)).count(), 2);
    let casts = g.log.iter().filter(|event| matches!(event, game::Event::Cast { .. })).count();
    let copies = g.log.iter().filter(|event| matches!(event, game::Event::SpellCopied { .. })).count();
    let activations = g.log.iter().filter(|event| matches!(event, game::Event::Activated { .. })).count();
    cite!(["707.10"]; assert_eq!((casts, copies, activations), (1, 1, 0)));
}
