//! Identity and lifetime boundaries of composed instructions (400.7, 608.2h, 611.2).
use super::*;
use crate::{CardDef, Move};

fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", kind, text, pt).unwrap()
}
fn setup(text: &str) -> Game {
    let defs = vec![
        card("Witness", "Instant", text, None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
    ];
    crate::play::check(&defs[0]).unwrap();
    Game::new(defs, 3, 20)
}
fn cast(g: &mut Game, targets: Vec<Vec<Target>>) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r608_2h_controller_projection_uses_departed_incarnation_not_graveyard_owner() {
    for indestructible in [false, true] {
        let mut g = setup("Destroy target creature. Its controller creates a 3/3 green Ape creature token.");
        Arc::make_mut(&mut g.defs).push(card("Survivor", "Creature — Bear", "Indestructible", Some((2, 2))));
        let bear = g.put(1, if indestructible { 2 } else { 1 }, Zone::Battlefield);
        g.obj_mut(bear).controller = 2;
        cast(&mut g, vec![vec![Target::Object(bear)]]);
        assert_eq!(g.obj(bear).zone, if indestructible { Zone::Battlefield } else { Zone::Gone });
        let tokens: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).token()).collect();
        assert_eq!(tokens.len(), 1);
        assert_eq!(g.obj(*tokens[0]).controller, 2);
        assert_eq!(g.players[1].graveyard.len(), usize::from(!indestructible));
    }
}

#[test]
fn r400_7_retained_reference_does_not_follow_zone_changes_across_a_pause_or_clone() {
    let mut g = setup("Tap target creature. You may gain 1 life. Untap it.");
    let bear = g.put(1, 1, Zone::Battlefield);
    let source = g.put(0, 0, Zone::Stack);
    let ctx = Ctx::new(0, source).with_ability(0);
    let mut frame = Frame::new(required(&g, &ctx).unwrap(), ctx);
    frame.set_targets(&[vec![Target::Object(bear)]]).unwrap();
    assert!(run(&mut g, &mut frame).unwrap().is_some());
    assert!(g.obj(bear).tapped);
    let (mut other, mut other_frame) = (g.clone(), frame.clone());
    frame.answer(Answer::Yes);
    assert!(run(&mut g, &mut frame).unwrap().is_none());
    assert!(!g.obj(bear).tapped);
    assert_eq!(g.players[0].life, 21);
    other.move_object(bear, Zone::Hand);
    let hand = *other.players[1].hand.last().unwrap();
    other.move_object(hand, Zone::Battlefield);
    let new = *other.battlefield.last().unwrap();
    other.tap(new);
    other_frame.answer(Answer::No);
    assert!(run(&mut other, &mut other_frame).unwrap().is_none());
    assert!(other.obj(new).tapped, "the referent is the old incarnation");
    assert_eq!(other.players[0].life, 20);
}

#[test]
fn r601_2c_coordinated_changes_have_one_announced_target() {
    let mut g = setup("Target creature gets +3/+3 and gains trample until end of turn.");
    let bear = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(bear)]]);
    assert_eq!(g.power(bear), Some(5));
    assert!(g.has(bear, "Trample"));
    assert!(g.continuous.iter().all(|c| c.affected == vec![bear] && c.lifetime == crate::layers::Lifetime::Turn));
}

#[test]
fn r611_2c_resolved_effect_keeps_its_set_when_new_creatures_arrive() {
    let mut g = setup("Creatures you control get +3/+3 and gain trample until end of turn.");
    let bear = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, vec![]);
    let late = g.put(0, 1, Zone::Battlefield);
    assert_eq!(g.power(bear), Some(5));
    assert_eq!(g.power(late), Some(2));
    assert!(!g.has(late, "Trample"));
}

#[test]
fn r608_2h_unbound_ambiguous_and_wrong_sort_references_are_refused() {
    for (text, reason) in [
        ("Untap it.", "input reference: nothing before it supplies the required participant/cardinality"),
        ("Tap target creature. Tap target creature. Untap it.", "reference: ambiguous earlier subject producers"),
        ("Target player gains 1 life. Untap that creature.", "input reference: nothing before it supplies the required participant/cardinality"),
    ] {
        let def = card("Witness", "Instant", text, None);
        let error = crate::play::check(&def).unwrap_err().0;
        assert_eq!(error, reason, "{text}");
    }
}

#[test]
fn r205_1b_duration_rider_keeps_the_prior_card_types() {
    let def = card("Mutavault", "Land", "{1}: Until end of turn, this land becomes a 2/2 creature with all creature types. It's still a land.", None);
    crate::play::check(&def).unwrap();
}

#[test]
fn r611_2d_r514_2_numbers_are_fixed_at_resolution_and_expire_in_cleanup() {
    let mut g = setup("Target creature gets +X/+X until end of turn, where X is the number of creatures you control.");
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g, vec![vec![Target::Object(bear)]]);
    assert_eq!(g.power(bear), Some(4));
    g.put(0, 1, Zone::Battlefield);
    assert_eq!(g.power(bear), Some(4), "the count was locked when the effect began");
    g.part = ir::Part::End;
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.power(bear), Some(2));
    assert!(g.continuous.is_empty());
}

#[test]
fn r611_2c_coordinated_changes_do_not_reselect_after_changing_types() {
    let mut g = setup("Creatures you control become artifacts and gain flying until end of turn.");
    let bear = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, vec![]);
    assert!(!g.seen(bear, None).types.contains(&ir::CardType::Creature));
    assert!(g.has(bear, "Flying"), "the second change still affects the original set");
}

#[test]
fn r611_player_subjects_are_not_silently_dropped_from_continuous_effects() {
    let def = card("Witness", "Instant", "You and permanents you control gain hexproof until end of turn.", None);
    assert_eq!(crate::play::check(&def).unwrap_err().0, "continuous changes to players are not lowered");
}


#[test]
fn r608_2k_r400_7j_cost_references_keep_lki_and_the_permitted_public_zone_successor() {
    let defs = vec![
        card("Cannon", "Artifact", "Sacrifice a creature: It deals 2 damage to target player. Return it to its owner's hand.", None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
    ];
    crate::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(2)]], paying: vec![vec![bear]] }).unwrap();
    assert_eq!(g.obj(bear).zone, Zone::Gone);
    let graveyard = g.players[0].graveyard[0];
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[2].life, 18);
    assert!(g.log.iter().any(|e| matches!(e, crate::Event::DamageDealt { source, .. } if *source == bear)));
    assert!(g.players[0].graveyard.is_empty());
    assert_eq!(g.players[0].hand.len(), 1);
    assert_ne!(g.players[0].hand[0], graveyard);
}

#[test]
fn r608_2k_cost_reference_does_not_choose_between_two_object_payments() {
    let def = card("Ambiguous", "Artifact", "Sacrifice a creature, Sacrifice a creature: It deals 2 damage to target player.", None);
    let error = crate::play::check(&def).unwrap_err().0;
    assert_eq!(error, "input reference: ambiguous occurrence participants");
}

#[test]
fn r611_2c_registered_rule_keeps_a_chosen_value_after_its_frame_finishes() {
    let mut g = setup("Choose a color. Creatures of the chosen color can't block this turn.");
    let white = card("White", "Creature — Bear", "", Some((2, 2)));
    Arc::make_mut(&mut g.defs).push(white);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Value(Value::Color(ir::Color::White)))).unwrap();
    let attacker = g.put(0, 1, Zone::Battlefield);
    let late = g.put(1, 2, Zone::Battlefield);
    g.obj_mut(late).chars.colors = vec![ir::Color::White];
    assert!(g.cant_block(late, attacker, &[late]));
    g.obj_mut(late).chars.colors.clear();
    assert!(!g.cant_block(late, attacker, &[late]));
}

#[test]
fn r113_7a_an_activated_occurrence_keeps_its_program_after_the_sources_definition_changes() {
    let mut g = Game::new(vec![
        card("Original", "Artifact", "{0}: You gain 2 life.", None),
        card("Changed", "Artifact", "{0}: You gain 9 life.", None),
    ], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = *g.stack.last().unwrap();
    let captured = g.obj(ability).invocations.as_ref().unwrap().as_ref().unwrap()[0].program.id;
    g.obj_mut(source).card = Some(1);
    assert_ne!(captured, required(&g, &Ctx::new(0, source).with_ability(0)).unwrap().id);
    let mut cloned = g.clone();
    for game in [&mut g, &mut cloned] {
        for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); }
        assert_eq!(game.players[0].life, 22);
        assert!(game.stack.is_empty());
    }
}

#[test]
fn r603_3_a_trigger_keeps_targets_and_intervening_condition_before_stack_placement() {
    let mut g = Game::new(vec![
        card("Original", "Creature", "Whenever this creature deals damage to a player, if you have 20 or less life, this creature deals 2 damage to target creature.", Some((2, 2))),
        card("Changed", "Creature", "Whenever this creature deals damage to a player, if you have 0 or less life, you gain 9 life.", Some((2, 2))),
        card("Survivor", "Creature", "", Some((3, 3))),
    ], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let target = g.put(1, 2, Zone::Battlefield);
    g.damage(source, Target::Player(1), 1);
    g.check_triggers();
    assert_eq!(g.triggered.len(), 1);
    g.obj_mut(source).card = Some(1);
    g.settle().unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, groups: 1, .. })));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(target).damage, 2);
    assert_eq!(g.players[0].life, 20);
    assert!(g.stack.is_empty());
}

#[test]
fn r603_4_the_captured_intervening_condition_is_rechecked_against_current_state() {
    let mut g = Game::new(vec![
        card("Watcher", "Creature", "Whenever this creature deals damage to a player, if you have 20 or less life, you draw a card.", Some((2, 2))),
        card("Bear", "Creature", "", Some((2, 2))),
    ], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Library);
    g.damage(source, Target::Player(1), 1);
    g.settle().unwrap();
    assert_eq!(g.stack.len(), 1);
    g.players[0].life = 21;
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].library.len(), 1);
    assert!(g.stack.is_empty());
}

#[test]
fn r603_3c_modal_trigger_keeps_its_original_modes_before_the_choice_and_across_clone() {
    let mut g = Game::new(vec![
        card("Original", "Creature", "When this creature enters, choose one —\n• You gain 2 life.\n• You draw a card.", Some((2, 2))),
        card("Changed", "Creature", "When this creature enters, choose one —\n• You gain 9 life.\n• You lose 9 life.", Some((2, 2))),
    ], 2, 20);
    let source = g.put(0, 0, Zone::Hand);
    g.move_object(source, Zone::Battlefield);
    g.check_triggers();
    let permanent = g.battlefield[0];
    g.obj_mut(permanent).card = Some(1);
    g.settle().unwrap();
    assert!(matches!(g.decision, Some(Decision::Modes { .. })));
    let mut clone = g.clone();
    for game in [&mut g, &mut clone] {
        game.act(0, Move::Modes(vec![0], Box::new(Move::ChooseTargets(vec![])))).unwrap();
        for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); }
        assert_eq!(game.players[0].life, 22);
    }
}

#[test]
fn r603_7_a_delayed_program_retains_its_local_choice_after_the_parent_frame_ends() {
    let defs = vec![
        card("Witness", "Instant", "Choose a color. This spell deals 2 damage to target creature. When that creature dies this turn, you may destroy all creatures of the chosen color.", None),
        card("Bear", "Creature", "", Some((2, 2))),
        card("Survivor", "Creature", "", Some((3, 3))),
    ];
    crate::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(1, 1, Zone::Battlefield);
    let survivor = g.put(1, 2, Zone::Battlefield);
    g.obj_mut(survivor).chars.colors = vec![ir::Color::White];
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Value(Value::Color(ir::Color::White)))).unwrap();
    assert_eq!(g.obj(bear).zone, Zone::Gone);
    assert_eq!(g.stack.len(), 1);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    let mut declined = g.clone();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.obj(survivor).zone, Zone::Gone);
    assert_eq!(declined.obj(survivor).zone, Zone::Battlefield);
}
