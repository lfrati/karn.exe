//! Tapping for mana means a mana ability with a tap-symbol cost, not any tap (106.12).
use crate::support::{cite,paid};
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

const EARTH: &str = "Whenever a player taps a nonbasic land for mana, this enchantment deals 1 damage to that player.";

fn game(source: &str, watcher: &str) -> Game {
    let defs = vec![
        CardDef::new("Burning Earth", "{3}{R}", "Enchantment", watcher, None).unwrap(),
        CardDef::new("Source", "", "Land", source, None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    let mut g = Game::new(defs, 3, 20);
    g.put(0, 0, Zone::Battlefield);
    g
}

fn activate(g: &mut Game, player: usize, source: ObjId) {
    g.act(player, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
}

fn resolve(g: &mut Game) {
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r106_12a_burning_earth_triggers_once_for_the_player_who_produced_mana() {
    for player in 0..3 {
        let mut g = game("{T}: Add {G}{G}.", EARTH);
        let source = g.put(player, 1, Zone::Battlefield);
        while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
        activate(&mut g, player, source);
        assert_eq!(g.players[player].pool.total(), 2);
        cite!(["106.12a"]; case: format!("player {player}"); assert_eq!(g.stack.len(), 1, "one trigger, not one per mana"));
        cite!(["605.1b", "605.5a"]; case: format!("player {player}"); assert_eq!(g.players[player].life, 20, "the damage ability uses the stack"));
        resolve(&mut g);
        for p in 0..3 { assert_eq!(g.players[p].life, if p == player { 19 } else { 20 }); }
    }
}

#[test]
fn r106_12_an_ordinary_tap_and_a_basic_land_do_not_trigger_burning_earth() {
    let mut g = game("{T}: Add {G}.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    g.tap(source);
    g.act(0, Move::Pass).unwrap();
    cite!(["106.12"]; assert!(g.stack.is_empty()));
    let forest = g.put(1, 2, Zone::Battlefield);
    g.act(1, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert!(g.stack.is_empty());
    assert_eq!(g.players[1].pool.total(), 1);
}

#[test]
fn r106_12a_a_zero_output_mana_ability_does_not_trigger_tapped_for_mana() {
    let mut g = game("{T}: Add {G} for each creature you control.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert!(g.obj(source).tapped);
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["106.12a"]; assert!(g.stack.is_empty()));
}

#[test]
fn r106_12_a_mana_ability_without_the_tap_symbol_does_not_tap_for_mana() {
    let mut g = game("Sacrifice this land: Add {G}.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert_eq!(g.players[0].pool.total(), 1);
    cite!(["106.12"]; assert!(g.stack.is_empty()));
}

#[test]
fn r106_12_a_tap_ability_that_uses_the_stack_is_not_tapping_for_mana() {
    let mut g = game("{T}: Add {G}. Draw a card.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Library);
    activate(&mut g, 0, source);
    cite!(["605.1a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.players[0].pool.total(), 0);
    resolve(&mut g);
    assert!(g.stack.is_empty());
    assert_eq!(g.players[0].pool.total(), 1);
    cite!(["106.12"]; assert_eq!(g.players[0].life, 20));
}

#[test]
fn r106_12a_a_sacrificed_mana_source_is_described_as_it_last_existed() {
    let mut g = game("{T}, Sacrifice this land: Add {G}.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert_eq!(g.obj(source).zone, Zone::Gone);
    cite!(["106.12a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 19);
}

#[test]
fn r106_12a_a_passive_tapped_for_mana_pattern_names_the_same_event() {
    let mut g = game("{T}: Add {G}.", "Whenever a land is tapped for mana, you gain 1 life.");
    let source = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    activate(&mut g, 1, source);
    resolve(&mut g);
    cite!(["106.12a"]; assert_eq!((g.players[0].life, g.players[1].life), (21, 20)));
}

/// A triggered mana ability resolves at once, without the stack (605.4a); one with modes
/// is still refused.
#[test]
fn r605_1b_a_triggered_mana_ability_resolves_with_the_mana_ability() {
    let mut g = game("{T}: Add {G}.", "Whenever a land is tapped for mana, add {G}.");
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    cite!(["605.1b", "605.4a"]; assert_eq!((g.players[0].pool.colors[4], g.stack.len()), (2, 0)));
    let d = CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a land is tapped for mana, choose one —\n• Add {G}.\n• Add {U}.", None).unwrap();
    assert_eq!(game::play::check(&d).expect_err("modal triggered mana is unsupported").0,"triggered mana ability with modes (605.1b)");
    assert_eq!(CardDef::new("Nonsense", "{0}", "Enchantment", "Whenever a player taps a player for mana, you gain 1 life.", None).expect_err("a player is not a tap-for-mana object"), "text the grammar does not read");
}

#[test]
fn r603_10_a_departed_watcher_does_not_trigger_when_its_mana_ability_resolves() {
    let mut g = game("{T}, Sacrifice this land: Add {G}.\nWhenever a land is tapped for mana, you gain 2 life.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert_eq!(g.obj(source).zone, Zone::Gone);
    cite!(["603.10"]; assert_eq!(g.stack.len(), 1, "only the still-present Burning Earth triggers"));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 19);
}

#[test]
fn r106_12a_multiple_production_instructions_trigger_once_and_failed_activations_add_nothing() {
    let mut g = game("{T}: Add {G}. Add {G}.", EARTH);
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert_eq!(g.players[0].pool.total(), 2);
    cite!(["106.12a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::ManaAdded { source: s, player: 0, mana } if *s == source && mana.len() == 1)).count(), 2);
    cite!(["106.12a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::TappedForMana { object, player: 0 } if *object == source)).count(), 1));
    let before = format!("{g:?}");
    cite!(["107.5"]; refused("Source can't pay {T} as it is (107.5, 118.3)"): g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r605_1b_a_mana_producing_trigger_with_a_target_is_an_ordinary_stack_ability() {
    let mut g = game("{T}: Add {G}.", "Whenever a land is tapped for mana, target player adds {G}.");
    let watcher = g.battlefield[0];
    let source = g.put(0, 1, Zone::Battlefield);
    activate(&mut g, 0, source);
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    cite!(["605.5a"]; assert_eq!(g.players[1].pool.total(), 0));
    cite!(["605.5a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.players[1].pool.total(), 1);
    assert!(g.log.iter().any(|e| matches!(e, Event::ManaAdded { source: s, player: 1, .. } if *s == watcher)));
}

#[test]
fn r601_2g_casting_plans_allow_payment_triggers_but_floating_mana_puts_them_on_stack() {
    let mut g = game("{T}: Add {G}.", EARTH);
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Reward", "{G}", "Sorcery", "You gain 1 life.", None).unwrap());
    let source = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    let mut during_cast = g.clone();
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    cite!(["601.2g"]; ok: during_cast.act(0, Move::WithMana(vec![step], Box::new(paid("G",Move::Cast { card, targets: vec![] })))));
    assert_eq!(during_cast.stack.len(), 2, "spell cast before its payment trigger is stacked");
    activate(&mut g, 0, source);
    cite!(["117.1a"]; refused("cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, paid("G",Move::Cast { card, targets: vec![] })));
    resolve(&mut g);
    g.act(0, paid("G",Move::Cast { card, targets: vec![] })).unwrap();
}

#[test]
fn r601_2g_casting_plans_delay_trigger_choices_but_floating_mana_asks_immediately() {
    let mut g = game("{T}: Add {G}.", "Whenever a land is tapped for mana, target player loses 1 life.");
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Reward", "{G}", "Instant", "You gain 1 life.", None).unwrap());
    let source = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    let mut during_cast = g.clone();
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    cite!(["601.2g"]; ok: during_cast.act(0, Move::WithMana(vec![step], Box::new(paid("G",Move::Cast { card, targets: vec![] })))));
    cite!(["603.3d"]; assert!(matches!(during_cast.decision, Some(Decision::Targets { .. }))));
    assert_eq!(during_cast.obj(card).zone, Zone::Gone, "cast completed before choosing the payment trigger's target");
    activate(&mut g, 0, source);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { .. }))));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    g.act(0, paid("G",Move::Cast { card, targets: vec![] })).unwrap();
}
