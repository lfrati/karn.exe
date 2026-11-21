//! Mana activation inside casting: targets precede costs; priority, SBA and trigger placement follow (601.2, 605.3a).
use crate::support::cite;
use game::legal::Step;
use game::state::{Decision, Pool};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn game(source: &str, source_type: &str, spell: &str, cost: &str, watcher: Option<&str>) -> Game {
    let mut defs = vec![
        CardDef::new("Source", "{0}", source_type, source, source_type.contains("Creature").then_some((2, 2))).unwrap(),
        CardDef::new("Trial", cost, "Sorcery", spell, None).unwrap(),
        CardDef::new("Recovery", "{0}", "Artifact", "{T}: Add {C}. You gain 1 life.", None).unwrap(),
    ];
    if let Some(text) = watcher { defs.push(CardDef::new("Watcher", "{0}", "Enchantment", text, None).unwrap()); }
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}
fn step(source: ObjId) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] } }
fn mana(green: u32, colorless: u32) -> Pool { Pool { colors: [0,0,0,0,green], colorless, ..Pool::default() } }
fn cast(card: ObjId, targets: Vec<Vec<Target>>, plan: Vec<Step>, with: Pool) -> Move {
    Move::Paying(with, Box::new(Move::WithMana(plan, Box::new(Move::Cast { card, targets }))))
}
fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r601_2c_g_a_spell_can_target_the_creature_sacrificed_for_its_mana() {
    let mut g = game("Sacrifice this creature: Add {G}.", "Creature — Spirit", "Target creature gets +2/+2 until end of turn.", "{G}", None);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, cast(card, vec![vec![Target::Object(source)]], vec![step(source)], mana(1,0))).unwrap();
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.obj(source).zone, Zone::Gone);
    assert_eq!(g.players[0].pool.total(), 0);
    let announced = g.log.iter().position(|e| matches!(e, Event::ZoneChanged { was, to: Zone::Stack, .. } if *was == card)).unwrap();
    let sacrificed = g.log.iter().position(|e| matches!(e, Event::Sacrificed { object, .. } if *object == source)).unwrap();
    let cast = g.log.iter().position(|e| matches!(e, Event::Cast { .. })).unwrap();
    cite!(["601.2c", "601.2g"]; assert!(announced < sacrificed && sacrificed < cast));
    resolve(&mut g);
    cite!(["608.2b"]; assert!(g.stack.is_empty(), "the spell has no legal target on resolution"));
    assert_eq!(g.players[0].graveyard.len(), 2);
}

#[test]
fn r601_2i_a_mana_payment_trigger_is_put_above_the_completed_spell() {
    let mut g = game("{T}: Add {G}.", "Land", "You gain 3 life.", "{G}",
        Some("Whenever a player taps a land for mana, this enchantment deals 1 damage to that player."));
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(1, 3, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, cast(card, vec![], vec![step(source)], mana(1,0))).unwrap();
    assert_eq!(g.stack.len(), 2);
    cite!(["603.3"]; assert_eq!(g.obj(g.stack[0]).chars.name, "Trial"));
    assert_eq!(g.players[0].life, 20);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 19);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r117_5_payment_trigger_targets_wait_until_the_spell_is_cast_and_paid() {
    let mut g = game("{T}: Add {G}.", "Land", "You gain 3 life.", "{G}",
        Some("Whenever a land is tapped for mana, target player loses 1 life."));
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(1, 3, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, cast(card, vec![], vec![step(source)], mana(1,0))).unwrap();
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.obj(g.stack[0]).chars.name, "Trial");
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 1, .. }))));
    g.act(1, Move::ChooseTargets(vec![vec![Target::Player(0)]])).unwrap();
    cite!(["603.3"]; assert_eq!(g.stack.len(), 2));
}

#[test]
fn r704_4_state_based_actions_do_not_interrupt_mana_activations_during_casting() {
    let mut g = game("{T}, Pay 20 life: Add {C}.", "Artifact", "You gain 3 life.", "{2}", None);
    let source = g.put(0, 0, Zone::Battlefield);
    let recovery = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, cast(card, vec![], vec![step(source), step(recovery)], mana(0,2))).unwrap();
    assert_eq!(g.players[0].life, 1);
    cite!(["704.3"]; assert!(!g.players[0].lost));
    assert_eq!(g.stack.len(), 1);
    cite!(["704.3"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Lost { .. }))));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 4);
}

#[test]
fn r733_1_an_invalid_complete_cast_rolls_back_all_proposed_mana_activations() {
    for duplicate in [false, true] {
        let mut g = game("{T}: Add {G}.", "Land", "You gain 1 life.", "{G}{G}", None);
        let source = g.put(0, 0, Zone::Battlefield);
        let card = g.put(0, 1, Zone::Hand);
        let before = format!("{g:?}");
        let plan = if duplicate { vec![step(source), step(source)] } else { vec![step(source)] };
        let why = if duplicate { "Source can't pay {T} as it is (107.5, 118.3)" } else { "that mana is not in your mana pool (118.3a)" };
        cite!(["733.1"]; case: why; refused(why): g.act(0, cast(card, vec![], plan, mana(2,0))));
        assert_eq!(format!("{g:?}"), before);
    }
}

#[test]
fn r605_3a_only_mana_abilities_can_be_activated_during_casting() {
    let mut g = game("{T}: You gain 1 life.", "Artifact", "You gain 3 life.", "{0}", None);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["605.3a"]; refused("only mana abilities can be activated during casting (605.3a)"): g.act(0, cast(card, vec![], vec![step(source)], mana(0,0))));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r601_2g_targets_are_chosen_before_a_supplied_mana_action_sacrifices_them() {
    let mut g = game("Sacrifice this creature: Add {G}.", "Creature — Spirit", "Target creature gets +2/+2 until end of turn.", "{G}", None);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    let plan = vec![step(source)];
    g.act(0, cast(card, vec![vec![Target::Object(source)]], plan, mana(1,0))).unwrap();
    resolve(&mut g);
    assert!(g.stack.is_empty());
}

#[test]
fn r702_34a_casting_mana_preserves_flashback_and_its_exile_replacement() {
    let mut g = game("{T}: Add {G}.", "Land", "You gain 3 life.\nFlashback {G}", "{2}{G}", None);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(cast(card, vec![], vec![step(source)], mana(1,0))))).unwrap();
    assert_eq!(g.stack.len(), 1);
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
    assert!(g.players[0].graveyard.is_empty());
    cite!(["702.34a"]; assert!(g.exile.iter().any(|id| g.obj(*id).chars.name == "Trial")));
}

#[test]
fn r603_2_payment_triggers_survive_a_later_mana_cost_removing_the_watcher() {
    let mut g = game("{T}: Add {G}.", "Land", "You gain 3 life.", "{G}{G}",
        Some("Whenever a land is tapped for mana, this enchantment deals 1 damage to that player.\nSacrifice this enchantment: Add {G}."));
    let source = g.put(0, 0, Zone::Battlefield);
    let watcher = g.put(0, 3, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, cast(card, vec![], vec![step(source), Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  ability: 1, ..step(watcher) }], mana(2,0))).unwrap();
    assert_eq!(g.obj(watcher).zone, Zone::Gone);
    cite!(["113.7a"]; assert_eq!(g.stack.len(), 2, "the earlier trigger remembers its departed source"));
    resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[0].life, 19));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22);
}
