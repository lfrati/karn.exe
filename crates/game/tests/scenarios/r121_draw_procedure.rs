//! Draws are sequential retained operations, whether a frame or the turn called them.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::simulation::{Simulation, ResponseError};
use ir::Part;

const SHELL: usize = 0;
const ONE: usize = 1;
const DRAW: usize = 2;
const EACH: usize = 3;
const BLANK: usize = 4;
const ZERO: usize = 5;
const TWICE: usize = 6;
const DARKBLAST: usize = 7;

fn game(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Shambling Shell", "{1}{B}{G}", "Creature — Plant Zombie", "Sacrifice this creature: Put a +1/+1 counter on target creature.\nDredge 3", Some((3, 1))).unwrap(),
        CardDef::new("Small dredger", "{0}", "Creature", "Dredge 1", Some((1, 1))).unwrap(),
        CardDef::new("Draw sequence", "{0}", "Sorcery", "You gain 1 life. Draw two cards. You gain 2 life.", None).unwrap(),
        CardDef::new("Shared draws", "{0}", "Sorcery", "Each player draws two cards.", None).unwrap(),
        CardDef::new("Blank", "", "Land", "", None).unwrap(),
        CardDef::new("Zero dredger", "{0}", "Creature", "Dredge 0", Some((1, 1))).unwrap(),
        CardDef::new("Two instances", "{0}", "Creature", "Dredge 1, dredge 2", Some((1, 1))).unwrap(),
        CardDef::new("Darkblast", "{B}", "Instant", "Target creature gets -1/-1 until end of turn.\nDredge 3", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {}", def.name(), e.0)); }
    Game::new(defs, players, 20)
}
fn library(g: &mut Game, player: usize, count: usize) -> Vec<ObjId> {
    (0..count).map(|_| g.put(player, BLANK, Zone::Library)).collect()
}
fn cast(g: &mut Game, card: usize) {
    let player = g.active;
    let card = g.put(player, card, Zone::Hand);
    g.act(player, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn offer(g: &Game) -> (usize, Vec<ReplacementId>) {
    match &g.decision {
        Some(Decision::Choose { player, choice: Choice::Replacement { on: Target::Player(on), kind, count: 1, among, optional: true, .. } }) => {
            assert_eq!(on, player); assert_eq!(kind, "draw"); (*player, among.clone())
        },
        other => panic!("expected optional draw substitution: {other:?}"),
    }
}
fn choose(g: &mut Game, source: ObjId) {
    let (player, among) = offer(g);
    let effect = among.iter().find(|r| r.source == source).unwrap().clone();
    g.act(player, Move::Choose(Answer::Replacement(effect))).unwrap();
}
fn drew(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Drew { .. })).count() }
fn milled(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Milled { .. })).count() }

#[test]
fn r614_11a_a_suspended_draw_keeps_the_spell_and_does_not_repeat_earlier_instructions() {
    let mut g = game(2);
    let source = g.put(0, SHELL, Zone::Graveyard);
    let cards = library(&mut g, 0, 6);
    cast(&mut g, DRAW);
    assert_eq!(g.players[0].life, 21);
    let spell = g.resolving().unwrap().object;
    assert!(g.stack.contains(&spell));
    assert_eq!((drew(&g), milled(&g)), (0, 0));
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    crate::support::refused(g.act(1, Move::Choose(Answer::No)), "that decision is another player's");
    cite!(["616.1"]; refused("choose one of the replacement effects that apply (616.1)"): g.act(0, Move::Choose(Answer::Yes)));
    let mut other = g.clone();
    choose(&mut g, source);
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(cards[3..6].to_vec()))));
    cite!(["614.11a"]; assert_eq!((drew(&g), milled(&g)), (1, 3)));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 23));
    assert!(g.resolving().is_none());
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Resolved { spell: s } if *s == spell)).count(), 1);
    other.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["121.2"]; assert_eq!(drew(&other), 1));
    assert_eq!(other.players[0].life, 21);
    choose(&mut other, source);
    cite!(["404.3"]; ok: other.act(0, Move::Choose(Answer::Objects(cards[2..5].to_vec()))));
    assert_eq!((drew(&other), milled(&other)), (1, 3));
    assert_eq!(other.players[0].life, 23);
}

#[test]
fn r121_2_a_declined_replacement_is_offered_again_for_the_next_draw_with_a_new_request_id() {
    let mut g = game(2);
    g.put(0, ONE, Zone::Graveyard);
    library(&mut g, 0, 4);
    cast(&mut g, DRAW);
    let mut simulation = Simulation::new(g);
    let first = simulation.request().unwrap().unwrap().clone();
    simulation.respond(first.id, 0, Move::Choose(Answer::No)).unwrap();
    // The host records delivered events; the simulation retains only unfinished work
    // and its current response, not a replay archive.
    let first_draws = drew(simulation.game().unwrap());
    let delivered = simulation.game().unwrap().log.end();
    let second = simulation.request().unwrap().unwrap().clone();
    assert_ne!(first.id, second.id);
    assert!(matches!(simulation.respond(first.id, 0, Move::Choose(Answer::No)), Err(ResponseError::Stale { .. })));
    simulation.respond(second.id, 0, Move::Choose(Answer::No)).unwrap();
    let next_draws = simulation.game().unwrap().log.since(delivered).iter()
        .filter(|e| matches!(e, Event::Drew { .. })).count();
    cite!(["121.2"]; assert_eq!(first_draws + next_draws, 2));
    assert_eq!(simulation.game().unwrap().players[0].life, 23);
}

#[test]
fn r614_11a_a_card_milled_by_one_replacement_can_replace_the_next_draw() {
    let mut g = game(2);
    let source = g.put(0, ONE, Zone::Graveyard);
    let cards = library(&mut g, 0, 4);
    g.put(0, SHELL, Zone::Library);
    cast(&mut g, DRAW);
    choose(&mut g, source);
    assert_eq!((drew(&g), milled(&g)), (0, 1));
    let (_, options) = offer(&g);
    assert_eq!(options.len(), 1);
    let newly_milled = options[0].source;
    cite!(["702.52a"]; assert_eq!(g.obj(newly_milled).card, Some(SHELL)));
    assert_ne!(newly_milled, source);
    choose(&mut g, newly_milled);
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(cards[1..4].to_vec()))));
    cite!(["614.11a"]; assert_eq!((drew(&g), milled(&g)), (0, 4)));
    assert_eq!(g.players[0].life, 23);
    assert_eq!(g.players[0].hand.len(), 2);
}

#[test]
fn r702_52b_insufficient_library_never_offers_partial_dredging_and_is_rechecked_each_draw() {
    let mut g = game(2);
    g.put(0, SHELL, Zone::Graveyard);
    library(&mut g, 0, 3);
    cast(&mut g, DRAW);
    cite!(["702.52a"]; assert_eq!(offer(&g).1.len(), 1));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.52b"]; assert!(g.decision.is_none()));
    assert_eq!((drew(&g), milled(&g)), (2, 0));
    assert_eq!(g.players[0].library.len(), 1);
}

#[test]
fn r702_52a_dredge_functions_only_in_the_drawing_players_graveyard() {
    let mut g = game(2);
    g.put(1, ONE, Zone::Graveyard);
    g.put(0, ONE, Zone::Battlefield);
    g.put(0, ONE, Zone::Hand);
    library(&mut g, 0, 3);
    cast(&mut g, DRAW);
    cite!(["702.52a"]; assert!(g.decision.is_none()));
    assert_eq!((drew(&g), milled(&g)), (2, 0));
}

#[test]
fn r121_2c_multiple_players_finish_their_draws_in_apnap_order() {
    let mut g = game(3);
    g.active = 1; g.priority = 1;
    for player in 0..3 { g.put(player, ONE, Zone::Graveyard); library(&mut g, player, 4); }
    cast(&mut g, EACH);
    for player in [1, 1, 2, 2, 0, 0] {
        assert_eq!(offer(&g).0, player);
        g.act(player, Move::Choose(Answer::No)).unwrap();
    }
    let players: Vec<_> = g.log.iter().filter_map(|e| if let Event::Drew { player, .. } = e { Some(*player) } else { None }).collect();
    cite!(["121.2c"]; assert_eq!(players, [1, 1, 2, 2, 0, 0]));
    assert!(g.resolving().is_none());
}

#[test]
fn r504_1_the_draw_step_uses_the_same_suspending_operation_without_a_spell() {
    let mut g = game(2);
    g.turn = 2; g.part = Part::Upkeep;
    let source = g.put(0, SHELL, Zone::Graveyard);
    let cards = library(&mut g, 0, 4);
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::Draw);
    cite!(["504.1"]; assert!(g.resolving().is_none()));
    assert_eq!(offer(&g).0, 0);
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    choose(&mut g, source);
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(cards[1..4].to_vec()))));
    assert!(g.decision.is_none());
    cite!(["702.52a"]; assert_eq!((drew(&g), milled(&g)), (0, 3)));
    assert_eq!(g.part, Part::Draw);
    cite!(["504.2"]; assert_eq!(g.priority, 0));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Began { part: Part::Draw })).count(), 1);
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::PrecombatMain);
}

#[test]
fn r614_11_zero_dredge_can_replace_an_empty_library_draw_without_a_loss() {
    let mut g = game(2);
    let source = g.put(0, ZERO, Zone::Graveyard);
    g.draw(0, 1);
    assert!(!g.players[0].drew_from_empty);
    choose(&mut g, source);
    cite!(["121.6a"]; assert!(!g.players[0].drew_from_empty));
    assert!(!g.players[0].lost);
    assert_eq!((drew(&g), milled(&g)), (0, 0));
    assert_eq!(g.players[0].hand.len(), 1);
}

#[test]
fn r702_52_distinct_keyword_instances_in_one_row_keep_distinct_replacement_identities() {
    let mut g = game(2);
    g.put(0, TWICE, Zone::Graveyard);
    let cards = library(&mut g, 0, 4);
    g.draw(0, 1);
    let (_, options) = offer(&g);
    cite!(["113.2c"]; assert_eq!(options.len(), 2));
    assert_eq!(options[0].source, options[1].source);
    assert_eq!(options[0].ability().unwrap(), options[1].ability().unwrap());
    assert_ne!(options[0].part().unwrap(), options[1].part().unwrap());
    g.act(0, Move::Choose(Answer::Replacement(options[1].clone()))).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(cards[2..4].to_vec()))));
    cite!(["113.2c"]; assert_eq!(milled(&g), 2));
    assert_eq!(drew(&g), 0);
}

#[test]
fn r702_52a_an_instant_keeps_its_spell_instructions_and_its_graveyard_replacement_separate() {
    let mut g = game(2);
    let source = g.put(0, DARKBLAST, Zone::Graveyard);
    let cards = library(&mut g, 0, 3);
    g.draw(0, 1);
    choose(&mut g, source);
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(cards))));
    cite!(["702.52a"]; assert_eq!(milled(&g), 3));
    assert_eq!(g.players[0].hand.len(), 1);
    cite!(["702.52a"]; assert!(g.stack.is_empty()));
}

#[test]
fn r702_52a_printed_dredge_remains_a_characteristic_but_unimplemented_grants_are_refused() {
    let mut g = game(2);
    let card = g.put(0, SHELL, Zone::Battlefield);
    cite!(["109.3"]; assert!(g.has(card, "Dredge")));
    let grant = CardDef::new("Grant", "{0}", "Sorcery", "Target creature gains dredge 3 until end of turn.", None).unwrap();
    crate::support::refused(game::play::check(&grant).map_err(|e|e.0),"keyword: Dredge");
}

#[test]
fn r704_5b_an_empty_draw_is_committed_but_loss_waits_until_the_entire_instruction_finishes() {
    let mut g = game(3);
    // Player 0 cannot draw, player 1 still has questions, player 2 draws normally.
    g.put(1, ONE, Zone::Graveyard);
    library(&mut g, 1, 3); library(&mut g, 2, 3);
    cast(&mut g, EACH);
    assert!(g.players[0].drew_from_empty);
    cite!(["704.3"]; assert!(!g.players[0].lost));
    assert_eq!(offer(&g).0, 1);
    for _ in 0..2 { g.act(1, Move::Choose(Answer::No)).unwrap(); }
    cite!(["704.5b"]; assert!(g.players[0].lost));
    assert!(!g.players[1].lost);
    assert!(!g.players[2].lost);
    assert_eq!(drew(&g), 4);
}

#[test]
fn r614_11a_a_sacrificed_source_can_replace_the_draw_of_its_captured_ability() {
    let mut g = game(2);
    let def = CardDef::new("Grave-Shell Scarab", "{2}{B}{G}{G}", "Creature — Insect", "{1}, Sacrifice this creature: Draw a card.\nDredge 1", Some((4, 4))).unwrap();
    game::play::check(&def).unwrap();
    let i = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    let source = g.put(0, i, Zone::Battlefield);
    library(&mut g, 0, 2);
    g.players[0].pool.colorless = 1;
    g.act(0, paid("C", Move::Activate { source, ability: 0, targets: vec![], paying: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let (_, options) = offer(&g);
    cite!(["400.7"]; assert_ne!(options[0].source, source));
    choose(&mut g, options[0].source);
    assert_eq!(milled(&g), 1);
    assert_eq!(drew(&g), 0);
    cite!(["702.52a"]; assert!(g.players[0].hand.iter().any(|id| g.obj(*id).card == Some(i))));
}

fn reflection(g: &mut Game, player: usize) -> ObjId {
    let def = CardDef::new("Thought Reflection", "{4}{U}{U}{U}", "Enchantment", "If you would draw a card, draw two cards instead.", None).unwrap();
    game::play::check(&def).unwrap();
    let index = g.defs.len(); std::sync::Arc::make_mut(&mut g.defs).push(def);
    g.put(player, index, Zone::Battlefield)
}

fn select_replacement(g: &mut Game, source: ObjId, optional: bool) {
    let Some(Decision::Choose { player, choice: Choice::Replacement { among, optional: actual, .. } }) = &g.decision else { panic!("draw replacement expected") };
    assert_eq!(*actual, optional);
    let (player, chosen) = (*player, among.iter().find(|id| id.source == source).unwrap().clone());
    g.act(player, Move::Choose(Answer::Replacement(chosen))).unwrap();
}

#[test]
fn r614_5_a_replacement_cannot_apply_again_to_its_own_child_draws() {
    let mut g = game(2);
    reflection(&mut g, 0);
    library(&mut g, 0, 10);
    g.draw(0, 1);
    assert!(g.decision.is_none());
    cite!(["614.5"]; assert_eq!(g.players[0].hand.len(), 2));
    cite!(["614.5"]; assert_eq!(g.players[0].library.len(), 8));
    // It can apply anew to the next original draw; it never applies to the other player.
    g.draw(0, 2);
    cite!(["121.2"]; assert_eq!(g.players[0].hand.len(), 6));
    library(&mut g, 1, 3);
    g.draw(1, 1);
    assert_eq!(g.players[1].hand.len(), 1);
}

#[test]
fn r614_11a_a_nested_draw_pause_and_clone_keep_the_parent_and_earlier_actions_once() {
    let mut g = game(2);
    let reflection = reflection(&mut g, 0);
    let dredger = g.put(0, ONE, Zone::Graveyard);
    library(&mut g, 0, 10);
    cast(&mut g, DRAW);
    assert_eq!(g.players[0].life, 21);
    cite!(["616.1"]; refused("choose one of the replacement effects that apply (616.1)"): g.act(0, Move::Choose(Answer::No)));
    select_replacement(&mut g, reflection, false);
    // The original spell draw waits on the first of the replacement body's two draws.
    assert_eq!(offer(&g).1.len(), 1);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.players[0].hand.len(), 1);
    let mut clone = g.clone();
    select_replacement(&mut g, dredger, true);
    assert!(g.decision.is_none());
    assert!(g.resolving().is_none());
    cite!(["614.11a"]; assert_eq!(g.players[0].hand.len(), 4)); // one drawn, dredger returned, two more drawn
    assert_eq!(g.players[0].library.len(), 6);
    cite!(["614.11a"]; assert_eq!(g.players[0].life, 23));
    clone.act(0, Move::Choose(Answer::No)).unwrap();
    // The second original draw independently offers the reflection/dredge ordering.
    assert_eq!(clone.players[0].hand.len(), 2);
    assert_eq!(clone.players[0].life, 21);
    select_replacement(&mut clone, dredger, false);
    assert!(clone.decision.is_none());
    assert!(clone.resolving().is_none());
    cite!(["614.11a"]; assert_eq!(clone.players[0].hand.len(), 3));
    assert_eq!(clone.players[0].library.len(), 7);
    assert_eq!(clone.players[0].life, 23);
}

#[test]
fn r616_1_dredge_before_a_draw_multiplier_replaces_the_whole_original_draw() {
    let mut g = game(2);
    reflection(&mut g, 0);
    let dredger = g.put(0, ONE, Zone::Graveyard);
    library(&mut g, 0, 6);
    g.draw(0, 1);
    select_replacement(&mut g, dredger, false);
    cite!(["616.1f"]; assert!(g.decision.is_none()));
    cite!(["616.1"]; assert_eq!(g.players[0].hand.len(), 1));
    assert_eq!(g.players[0].library.len(), 5);
}

#[test]
fn r614_5_other_multipliers_apply_independently_to_each_sibling_draw() {
    let mut g = game(2);
    let first = reflection(&mut g, 0);
    reflection(&mut g, 0);
    library(&mut g, 0, 8);
    g.draw(0, 1);
    select_replacement(&mut g, first, false);
    assert!(g.decision.is_none());
    cite!(["614.5"]; assert_eq!(g.players[0].hand.len(), 4));
    assert_eq!(g.players[0].library.len(), 4);
}

#[test]
fn r504_1_the_draw_step_resumes_its_nested_replacement_body_without_a_spell() {
    let mut g = game(2);
    g.turn = 2; g.part = Part::Upkeep;
    let reflection = reflection(&mut g, 0);
    let dredger = g.put(0, ONE, Zone::Graveyard);
    library(&mut g, 0, 6);
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::Draw);
    select_replacement(&mut g, reflection, false);
    cite!(["504.1"]; assert!(g.resolving().is_none()));
    select_replacement(&mut g, dredger, true);
    assert!(g.decision.is_none());
    cite!(["614.11a"]; assert_eq!(g.players[0].hand.len(), 2));
    assert_eq!(g.players[0].library.len(), 4);
    assert_eq!(g.part, Part::Draw);
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::PrecombatMain);
    assert_eq!(g.players[0].hand.len(), 2);
}
