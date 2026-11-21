use super::*;
use crate::{CardDef, Move};

fn card(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let card = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    crate::play::check(&card).unwrap();
    card
}
fn bear() -> CardDef { card("Bear", "Creature — Bear", "", Some((2, 2))) }
/// Collect as in a later turn: what died in a turn is also kept while that turn lasts, and
/// what was cast in it through the turn after (608.2i), which its own test below is about.
fn collect(game: &mut Game) -> usize {
    game.forget_turn();
    game.forget_turn();
    game.check_triggers();
    if game.returning() { game.drive_procedures().expect("bounded fixture return completes"); }
    game.retire_events(game.log.end());
    game.collect_departed()
}
fn resolve(game: &mut Game) {
    for _ in 0..game.players.len() { game.act(game.priority, Move::Pass).unwrap(); }
}

#[test]
fn r400_7_repeated_zone_changes_reuse_slots_without_reusing_incarnations() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let original = game.put(0, 0, Zone::Battlefield);
    let mut current = original;
    for index in 0..1000 {
        current = game.move_to(current, if index % 2 == 0 { Zone::Hand } else { Zone::Battlefield }, 0).unwrap();
        assert_eq!(collect(&mut game), 1);
        assert_eq!(game.object_count(), 1);
        assert_eq!(game.object_slots(), 2);
    }
    assert_ne!(original, current);
    assert_eq!(original.slot(), current.slot(), "the original slot was reused");
    assert!(game.get_object(original).is_none());
    let before = (game.object_count(), game.log.end(), game.battlefield.clone());
    assert_eq!(game.act(0, Move::Activate { source: original, ability: 0, targets: vec![], paying: vec![] }), Err("an object handle is stale or unknown".into()));
    assert_eq!(before, (game.object_count(), game.log.end(), game.battlefield.clone()));
    assert_eq!(game.obj(current).zone, Zone::Battlefield);
    let mut cloned = game.clone();
    cloned.move_to(current, Zone::Exile, 0);
    collect(&mut cloned);
    assert!(cloned.get_object(current).is_none());
    assert_eq!(game.obj(current).zone, Zone::Battlefield);
}

#[test]
fn r603_10_pending_events_keep_departures_until_their_payloads_are_consumed() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let was = game.put(0, 0, Zone::Battlefield);
    let now = game.move_to(was, Zone::Graveyard, 0).unwrap();
    assert_eq!(game.collect_departed(), 0);
    game.check_triggers();
    assert_eq!(game.collect_departed(), 0, "the host can still read event participants");
    assert_eq!(collect(&mut game), 1);
    assert!(game.get_object(was).is_none());
    assert_eq!(game.obj(now).zone, Zone::Graveyard);
}

#[test]
fn r608_2k_cost_lki_and_public_successor_remain_owned_by_the_stack_occurrence() {
    let cannon = card("Cannon", "Artifact", "Sacrifice a creature: It deals 2 damage to target player. Return it to its owner's hand.", None);
    let mut game = Game::new(vec![cannon, bear()], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let sacrificed = game.put(0, 1, Zone::Battlefield);
    game.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![vec![sacrificed]] }).unwrap();
    collect(&mut game);
    assert_eq!(game.obj(sacrificed).zone, Zone::Gone);
    for mut branch in [game.clone(), game] {
        resolve(&mut branch);
        assert_eq!(branch.players[1].life, 18);
        assert!(branch.players[0].graveyard.is_empty());
        assert_eq!(branch.players[0].hand.len(), 1);
        assert!(branch.log.iter().any(|event| matches!(event, Event::DamageDealt { source, .. } if *source == sacrificed)));
        collect(&mut branch);
        assert!(branch.get_object(sacrificed).is_none());
    }
}

#[test]
fn r603_7_delayed_source_lki_survives_turns_and_clones_then_expires() {
    let spell = card("Witness", "Instant", "This spell deals 1 damage to target creature. When that creature dies, this spell deals 3 damage to the creature's controller.", None);
    let mut game = Game::new(vec![spell, bear()], 2, 20);
    let watched = game.put(1, 1, Zone::Battlefield);
    let card = game.put(0, 0, Zone::Hand);
    game.act(0, Move::Cast { card, targets: vec![vec![Target::Object(watched)]] }).unwrap();
    let source = game.stack[0];
    resolve(&mut game);
    assert_eq!(game.delayed.len(), 1);
    let successor = game.players[0].graveyard[0];
    let exiled = game.move_to(successor, Zone::Exile, 0).unwrap();
    game.move_to(exiled, Zone::Hand, 0);
    collect(&mut game);
    assert!(game.get_object(successor).is_none(), "retained sources do not own successor chains");
    assert!(game.get_object(exiled).is_none());
    for _ in 0..3 {
        game.begin_part(Part::Cleanup).unwrap();
        collect(&mut game);
        assert_eq!(game.obj(source).zone, Zone::Gone);
    }
    assert_eq!(game.turn, 4);
    for mut branch in [game.clone(), game] {
        branch.destroy(watched);
        branch.settle().unwrap();
        collect(&mut branch);
        assert_eq!(branch.stack.len(), 1);
        assert_eq!(branch.obj(watched).zone, Zone::Gone);
        resolve(&mut branch);
        assert_eq!(branch.players[1].life, 17);
        assert!(branch.delayed.is_empty());
        collect(&mut branch);
        assert!(branch.get_object(source).is_none());
        assert!(branch.get_object(watched).is_none());
    }
}

#[test]
fn r607_2a_linked_exile_survives_source_departure_and_captured_trigger_clones() {
    let ring = card("Ring", "Enchantment", "When this enchantment enters, exile another target nonland permanent.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control.", None);
    let mut game = Game::new(vec![ring, bear()], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(1, 1, Zone::Battlefield);
    let exiled = game.exile_with(bear, source, Some(crate::binding::AbilityKey::printed(0)), None).unwrap();
    collect(&mut game);
    game.destroy(source);
    game.settle().unwrap();
    collect(&mut game);
    assert_eq!(game.obj(source).zone, Zone::Gone);
    assert_eq!(game.exiled_with.len(), 1);
    for mut branch in [game.clone(), game] {
        resolve(&mut branch);
        assert!(branch.exile.is_empty());
        assert_eq!(branch.obj(branch.battlefield[0]).owner, 1);
        collect(&mut branch);
        assert!(branch.get_object(source).is_none());
        assert!(branch.get_object(exiled).is_none());
        assert!(branch.exiled_with.is_empty());
    }
}

#[test]
fn r607_2a_orphaned_exile_links_and_continuous_membership_do_not_retain_payloads() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let bear = game.put(1, 0, Zone::Battlefield);
    game.begin_effect_for(vec![crate::layers::Change::Pt(3, 3)], vec![source], crate::layers::Lifetime::Forever);
    let exiled = game.exile_with(bear, source, None, None).unwrap();
    game.move_to(source, Zone::Hand, 0);
    collect(&mut game);
    assert!(game.get_object(source).is_none());
    assert!(game.exiled_with.is_empty());
    assert!(game.continuous.is_empty());
    assert_eq!(game.obj(exiled).zone, Zone::Exile);
}

#[test]
fn r608_2_suspended_procedures_keep_all_payloads_until_they_resume() {
    let spell = card("Witness", "Instant", "Tap target creature. You may gain 1 life. Untap it.", None);
    let mut game = Game::new(vec![spell, bear()], 2, 20);
    let creature = game.put(1, 1, Zone::Battlefield);
    let unrelated = game.put(1, 1, Zone::Hand);
    game.move_to(unrelated, Zone::Graveyard, 1);
    let spell = game.put(0, 0, Zone::Hand);
    game.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(creature)]] }).unwrap();
    resolve(&mut game);
    assert!(matches!(game.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    assert!(!game.procedures.is_empty());
    let count = game.object_count();
    assert_eq!(collect(&mut game), 0);
    assert_eq!(game.object_count(), count);
    assert!(game.get_object(unrelated).is_some());
    for mut branch in [game.clone(), game] {
        branch.act(0, Move::Choose(Answer::Yes)).unwrap();
        assert!(!branch.obj(creature).tapped);
        assert_eq!(branch.players[0].life, 21);
        assert!(branch.procedures.is_empty());
        assert!(collect(&mut branch) > 0);
        assert!(branch.get_object(unrelated).is_none());
    }
}

#[test]
fn r106_6_restricted_mana_retains_its_departed_source_until_the_pool_empties() {
    let land = card("Land", "Land", "{T}: Add {G}. Spend this mana only to cast a creature spell.", None);
    let mut game = Game::new(vec![land, bear()], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    game.act(0, Move::Activate { source: source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    game.move_to(source, Zone::Hand, 0);
    collect(&mut game);
    assert_eq!(game.obj(source).zone, Zone::Gone);
    let creature = game.put(0, 1, Zone::Hand);
    assert!(game.may_spend(&game.players[0].pool.restricted[0], Spent::Spell(creature, crate::card::CastAs::Whole)));
    game.end_part().unwrap();
    collect(&mut game);
    assert!(game.get_object(source).is_none());
}

#[test]
fn r113_7a_grant_provenance_does_not_pin_the_granting_permanent() {
    let lord = card("Lord", "Creature", "Other creatures you control have \"{T}: This creature deals 1 damage to any target.\"", Some((2, 2)));
    let mut game = Game::new(vec![lord, bear()], 2, 20);
    let lord = game.put(0, 0, Zone::Battlefield);
    let source = game.put(0, 1, Zone::Battlefield);
    game.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    game.destroy(lord);
    collect(&mut game);
    assert!(game.get_object(lord).is_none());
    resolve(&mut game);
    assert_eq!(game.players[1].life, 19, "the captured template needs its source, not its grantor");
}

#[test]
fn r610_3_until_links_survive_collection_and_release_after_returning() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let creature = game.put(1, 0, Zone::Battlefield);
    let exiled = game.exile_until(&[creature], source, None, None)[0].1;
    collect(&mut game);
    assert!(game.get_object(creature).is_none());
    for mut branch in [game.clone(), game] {
        branch.destroy(source);
        collect(&mut branch);
        assert!(branch.until.is_empty());
        assert!(branch.exile.is_empty());
        assert!(branch.get_object(source).is_none());
        assert!(branch.get_object(exiled).is_none());
        assert_eq!(branch.obj(branch.battlefield[0]).owner, 1);
    }
}

#[test]
fn r608_2i_what_died_this_turn_is_kept_until_the_turn_is_over() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let was = game.put(0, 0, Zone::Battlefield);
    game.destroy(was);
    game.check_triggers();
    if game.returning() { game.drive_procedures().expect("bounded fixture return completes"); }
    game.retire_events(game.log.end());
    // nothing else owns it: only that a condition may ask what died this turn
    assert_eq!(game.collect_departed(), 0);
    assert_eq!(game.clone().obj(was).zone, Zone::Gone);
    game.forget_turn();
    assert_eq!(game.collect_departed(), 1);
    assert!(game.get_object(was).is_none());
}

#[test]
fn r611_2a_lasting_rule_retains_source_and_locals_until_cleanup() {
    let rule = card("Rule", "Instant", "Choose a color. Creatures of the chosen color can't block this turn.", None);
    let mut game = Game::new(vec![rule, bear()], 2, 20);
    let attacker = game.put(0, 1, Zone::Battlefield);
    let blocker = game.put(1, 1, Zone::Battlefield);
    game.obj_mut(blocker).chars.colors = vec![Color::White];
    let card = game.put(0, 0, Zone::Hand);
    game.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let source = game.stack[0];
    resolve(&mut game);
    game.act(0, Move::Choose(Answer::Value(crate::binding::Value::Color(Color::White)))).unwrap();
    collect(&mut game);
    assert_eq!(game.obj(source).zone, Zone::Gone);
    assert!(game.cant_block(blocker, attacker, &[blocker]));
    game.begin_part(Part::Cleanup).unwrap();
    collect(&mut game);
    assert!(game.get_object(source).is_none());
    assert!(!game.cant_block(blocker, attacker, &[blocker]));
}

#[test]
fn r603_10_predeparture_worlds_end_at_trigger_delivery() {
    let mut game = Game::new(vec![bear()], 2, 20);
    let was = game.put(0, 0, Zone::Battlefield);
    game.destroy(was);
    assert_eq!(game.lookbacks.len(), 1);
    for mut branch in [game.clone(), game] {
        assert_eq!(branch.collect_departed(), 0);
        branch.check_triggers();
        assert!(branch.lookbacks.is_empty(), "occurrences never retain event query worlds");
        collect(&mut branch);
        assert!(branch.get_object(was).is_none());
    }
}
