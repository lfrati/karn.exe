//! Undying and persist mean ordinary triggered return instructions (702.93a, 702.79a).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn geist() -> CardDef {
    let d = CardDef::new("Strangleroot Geist", "{G}{G}", "Creature — Spirit", "Haste\nUndying", Some((2, 1))).unwrap();
    game::play::check(&d).unwrap(); d
}
fn creature(text: &str) -> CardDef {
    let d = CardDef::new("Witness", "{0}", "Creature — Spirit", text, Some((3, 3))).unwrap();
    game::play::check(&d).unwrap(); d
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn die(g: &mut Game, source: ObjId) { g.destroy(source); g.act(g.priority, Move::Pass).unwrap(); }
fn counters(g: &Game, object: ObjId, kind: &str) -> u32 { g.obj(object).counters.get(kind).copied().unwrap_or(0) }

#[test]
fn r702_93_full_strangleroot_geist_returns_once_with_a_counter_and_keeps_haste() {
    let mut g = Game::new(vec![geist()], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    g.act(0, crate::support::paid("GG",Move::Cast { card, targets: vec![] })).unwrap(); pass(&mut g);
    let source = g.battlefield[0];
    assert!(g.has(source, "Haste") && g.has(source, "Undying"));
    die(&mut g, source); cite!(["702.93a"]; assert_eq!(g.stack.len(), 1)); pass(&mut g);
    let returned = g.battlefield[0]; cite!(["400.7"]; assert_ne!(returned, source));
    cite!(["702.93a"]; assert_eq!(counters(&g, returned, "+1/+1"), 1));
    assert_eq!((g.power(returned), g.toughness(returned)), (Some(3), Some(2)));
    assert!(!g.summoning_sick(returned));
    die(&mut g, returned);
    cite!(["702.93a"]; assert!(g.stack.is_empty() && g.battlefield.is_empty()));
    assert_eq!(g.players[0].graveyard.len(), 1);
}

#[test]
fn r702_79_persist_uses_the_same_return_with_opposite_counter_kind() {
    let mut g = Game::new(vec![creature("Persist")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield); assert!(g.has(source, "Persist"));
    die(&mut g, source); pass(&mut g); let returned = g.battlefield[0];
    cite!(["702.79a"]; assert_eq!(counters(&g, returned, "-1/-1"), 1));
    assert_eq!((g.power(returned), g.toughness(returned)), (Some(2), Some(2)));
    die(&mut g, returned); cite!(["702.79a"]; assert!(g.stack.is_empty() && g.battlefield.is_empty()));
}

#[test]
fn r603_4_undying_reads_departed_counter_facts_not_the_empty_graveyard_object() {
    for kind in ["+1/+1", "charge"] {
        let mut g = Game::new(vec![geist()], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
        g.put_counters(source, kind, 1); die(&mut g, source);
        let grave = g.players[0].graveyard[0]; assert!(g.obj(grave).counters.is_empty());
        cite!(["603.10a", "702.93a"]; case: kind; assert_eq!(g.stack.len(), usize::from(kind != "+1/+1")));
        if kind != "+1/+1" { pass(&mut g); cite!(["702.93a"]; case: kind; assert_eq!(counters(&g, g.battlefield[0], "+1/+1"), 1)); }
    }
    // 603.10a: simultaneous lethal toughness and opposing counters use the pre-SBA facts.
    let mut g = Game::new(vec![geist()], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "+1/+1", 1); g.put_counters(source, "-1/-1", 2);
    g.act(0, Move::Pass).unwrap();
    cite!(["603.10a"]; assert!(g.battlefield.is_empty() && g.stack.is_empty(), "it had a +1/+1 counter immediately before dying"));
}

#[test]
fn r400_7_undying_retains_only_the_exact_graveyard_incarnation_across_clones() {
    let mut g = Game::new(vec![geist()], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
    die(&mut g, source); let grave = g.players[0].graveyard[0];
    g.retire_events(g.log.end()); g.collect_departed();
    assert!(g.get_object(source).is_some(), "the pending trigger owns its original LKI");
    let mut moved = g.clone();
    moved.exile(grave); let exile = moved.exile[0]; moved.move_object(exile, Zone::Graveyard);
    let changed = moved.players[0].graveyard[0]; assert_ne!(changed, grave);
    pass(&mut moved); cite!(["400.7"]; assert!(moved.battlefield.is_empty())); assert_eq!(moved.players[0].graveyard, vec![changed]);
    pass(&mut g); cite!(["702.93a"]; assert_eq!(g.battlefield.len(), 1)); assert!(g.players[0].graveyard.is_empty());
}

#[test]
fn r603_3_multiple_undying_instances_each_trigger_but_cannot_return_the_next_incarnation() {
    let mut g = Game::new(vec![creature("Undying, undying")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield); die(&mut g, source);
    assert!(matches!(g.decision, Some(Decision::Order { player: 0, .. })));
    g.act(0, Move::Order(vec![1, 0])).unwrap(); cite!(["113.2c", "603.3b"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g); let returned = g.battlefield[0];
    // It dies again, now with a +1/+1 counter, while the other original trigger waits.
    g.destroy(returned); pass(&mut g);
    cite!(["400.7"]; assert!(g.battlefield.is_empty())); assert_eq!(g.players[0].graveyard.len(), 1);
    cite!(["702.93a"]; assert!(g.stack.is_empty()));
}

#[test]
fn r616_1_undying_entry_counter_replacements_pause_and_clone_without_replaying_return() {
    let scale = CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap();
    let double = CardDef::new("Branching Evolution", "{1}{G}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap();
    let mut g = Game::new(vec![geist(), scale, double], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield); let scale = g.put(0, 1, Zone::Battlefield); let double = g.put(0, 2, Zone::Battlefield);
    die(&mut g, source); pass(&mut g);
    let among = match &g.decision { Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, count: 1, .. } }) => among.clone(), other => panic!("{other:?}") };
    let grave = g.players[0].graveyard[0]; assert_eq!(g.battlefield.len(), 2);
    cite!(["616.1"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))));
    let clone = g.clone();
    for (mut branch, first, count) in [(g, scale, 4), (clone, double, 3)] {
        let chosen = among.iter().find(|r| r.source == first).unwrap().clone();
        branch.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        assert!(branch.stack.is_empty() && branch.decision.is_none());
        let returned = *branch.battlefield.iter().find(|o| branch.obj(**o).card == Some(0)).unwrap();
        cite!(["616.1"]; case: format!("count {count}"); assert_eq!(counters(&branch, returned, "+1/+1"), count));
        assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::ZoneChanged { was, to: Zone::Battlefield, .. } if *was == grave)).count(), 1);
    }
}

#[test]
fn r614_regeneration_prevents_death_and_exile_never_triggers_undying() {
    let mut g = Game::new(vec![geist()], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
    g.regenerate(source); g.destroy(source); g.act(0, Move::Pass).unwrap();
    cite!(["701.19a"]; assert_eq!(g.battlefield, vec![source])); assert!(g.stack.is_empty()); cite!(["701.19a"]; assert!(g.obj(source).tapped));
    g.exile(source); g.act(g.priority, Move::Pass).unwrap();
    cite!(["702.93a"]; assert!(g.stack.is_empty() && g.battlefield.is_empty()));
    assert_eq!(g.exile.len(), 1);
}

#[test]
fn r113_10_removing_abilities_before_death_suppresses_the_keyword_trigger() {
    let strip = CardDef::new("Strip", "{0}", "Instant", "Target creature loses all abilities until end of turn.", None).unwrap();
    game::play::check(&strip).unwrap();
    let mut g = Game::new(vec![geist(), strip], 2, 20); let source = g.put(0, 0, Zone::Battlefield); let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(source)]] }).unwrap(); pass(&mut g);
    assert!(!g.has(source, "Undying")); die(&mut g, source); cite!(["613.1f"]; assert!(g.stack.is_empty()));
}

#[test]
fn r702_93_unsupported_grants_and_death_replacements_remain_explicit_refusals() {

    for text in ["Target creature gains undying until end of turn.", "Target creature gains persist until end of turn."] {
        let def = CardDef::new("Unsupported", "{0}", "Instant", text, None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),if text.contains("undying") {"keyword: Undying"} else {"keyword: Persist"});
    }
    let def = CardDef::new("Unsupported", "{0}", "Creature", "Undying\nIf this creature would die, exile it instead.", Some((2, 2))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"zone-destination replacement: its own death or exile (the library form is DeathToLibrary)");
    let def = CardDef::new("Unsupported", "{0}", "Sorcery", "If this permanent had no +1/+1 counters on it, draw a card.", None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"past counter condition requires a source departure trigger");
}

#[test]
fn r111_8_copied_tokens_have_undying_but_cannot_return_after_leaving() {
    let def = creature("Undying\n{0}: Create a token that's a copy of this creature.");
    let mut g = Game::new(vec![def], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap(); pass(&mut g);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    die(&mut g, token); cite!(["707.2"]; assert_eq!(g.stack.len(), 1)); pass(&mut g);
    cite!(["111.8"]; assert_eq!(g.battlefield, vec![source])); assert!(g.players[0].graveyard.is_empty());
}

#[test]
fn r603_4_plain_rule_text_uses_the_same_lki_predicate_and_entry_counter_operation() {
    let def = creature("When this permanent is put into a graveyard from the battlefield, if it had no +1/+1 counters on it, return it to the battlefield under its owner's control with two +1/+1 counters on it.");
    let mut g = Game::new(vec![def], 2, 20); let source = g.put(0, 0, Zone::Battlefield);
    die(&mut g, source); pass(&mut g);
    cite!(["603.10a", "603.4"]; assert_eq!(counters(&g, g.battlefield[0], "+1/+1"), 2));
}
