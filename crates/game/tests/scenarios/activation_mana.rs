//! Announcement and payment of activated abilities (602.2b follows 601.2b–i).
use crate::support::cite;
use game::legal::Step;
use game::state::Pool;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn step(source: ObjId, ability: usize) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability, paying: vec![], adding: vec![] } }
fn colorless(n: u32) -> Pool { Pool { colorless:n, ..Pool::default() } }
fn activate(source: ObjId, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>, plan: Vec<Step>, with: Pool) -> Move {
    Move::Paying(with, Box::new(Move::WithMana(plan, Box::new(Move::Activate { source, ability: 0, targets, paying }))))
}
fn game(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Source", "{0}", "Creature — Spirit", text, Some((2, 2))).unwrap(),
        CardDef::new("Altar", "{0}", "Artifact", "Sacrifice a creature: Add {C}{C}.", None).unwrap(),
        CardDef::new("Land", "", "Land", "{T}: Add {C}.", None).unwrap(),
        CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a land is tapped for mana, target player loses 1 life.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r602_2b_an_ability_survives_its_source_sacrificed_for_mana() {
    let mut g = game("{2}: You gain 3 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let altar = g.put(0, 1, Zone::Battlefield);
    g.act(0, activate(source, vec![], vec![], vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  paying: vec![vec![source]], ..step(altar, 0) }], colorless(2))).unwrap();
    assert_eq!(g.obj(source).zone, Zone::Gone);
    assert_eq!(g.stack.len(), 1);
    resolve(&mut g);
    // the ability resolves without its source (113.7a)
    cite!(["113.7a"]; assert_eq!(g.players[0].life, 23));
}
#[test]
fn r602_2b_targets_are_chosen_before_activating_mana_abilities() {
    let mut g = game("{2}: Target creature gets +2/+2 until end of turn.");
    let source = g.put(0, 0, Zone::Battlefield);
    let altar = g.put(0, 1, Zone::Battlefield);
    // the source is a legal target when targets are chosen (601.2c), before it is sacrificed
    // for mana (601.2g)
    cite!(["602.2b", "601.2c", "601.2g"]; ok: g.act(0, activate(source, vec![vec![Target::Object(source)]], vec![], vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  paying: vec![vec![source]], ..step(altar, 0) }], colorless(2))));
    resolve(&mut g);
    assert!(g.stack.is_empty());
}
#[test]
fn r602_2b_payment_triggers_wait_above_the_completed_activation() {
    let mut g = game("{1}: You gain 3 life. Activate only as a sorcery.");
    let source = g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    g.put(1, 3, Zone::Battlefield);
    g.act(0, activate(source, vec![], vec![], vec![step(land, 0)], colorless(1))).unwrap();
    assert_eq!(g.stack.len(), 1);
    g.act(1, Move::ChooseTargets(vec![vec![Target::Player(0)]])).unwrap();
    resolve(&mut g);
    // the Watcher's trigger went on the stack above the completed activation (603.3)
    cite!(["603.3"]; assert_eq!(g.players[0].life, 19));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 22);
}
#[test]
fn r118_10_sacrificing_a_source_for_mana_cannot_also_pay_its_sacrifice_cost() {
    let mut g = game("{2}, Sacrifice this creature: You gain 3 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let altar = g.put(0, 1, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["118.10"]; refused("you can't sacrifice Source (701.21a, 118.3)"): g.act(0, activate(source, vec![], vec![], vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  paying: vec![vec![source]], ..step(altar, 0) }], colorless(2))));
    assert_eq!(format!("{g:?}"), before);
}
#[test]
fn r602_2b_a_supplied_mana_action_can_sacrifice_an_abilitys_source() {
    let mut g = game("{2}: You gain 3 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let altar = g.put(0, 1, Zone::Battlefield);
    let plan = vec![Step { plan: vec![], x: None, life: vec![], with: None, paying: vec![vec![source]], ..step(altar, 0) }];
    cite!(["605.3a"]; ok: g.act(0, activate(source, vec![], vec![], plan, colorless(2))));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r704_4_state_based_actions_wait_for_the_complete_activation_payment() {
    let mut g = game("{2}: You gain 3 life.");
    std::sync::Arc::make_mut(&mut g.defs).extend([
        CardDef::new("Life", "{0}", "Artifact", "{T}, Pay 20 life: Add {C}.", None).unwrap(),
        CardDef::new("Recovery", "{0}", "Artifact", "{T}: Add {C}. You gain 1 life.", None).unwrap(),
    ]);
    let source = g.put(0, 0, Zone::Battlefield);
    let life = g.put(0, 4, Zone::Battlefield);
    let recovery = g.put(0, 5, Zone::Battlefield);
    g.act(0, activate(source, vec![], vec![], vec![step(life, 0), step(recovery, 0)], colorless(2))).unwrap();
    assert_eq!(g.players[0].life, 1);
    // at 0 life mid-payment, but no player received priority until it was complete (704.3)
    cite!(["704.3"]; assert!(!g.players[0].lost));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 4);
}

#[test]
fn r605_3a_a_mana_ability_can_activate_another_mana_ability_to_pay_its_cost() {
    let mut g = game("{1}: Add {G}{G}.");
    let source = g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    cite!(["605.3a"]; ok: g.act(0, activate(source, vec![], vec![], vec![step(land, 0)], colorless(1))));
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    assert_eq!(g.players[0].pool.colors[4], 2);
    assert_eq!(g.players[0].pool.total(), 2);
}

#[test]
fn r733_1_an_activation_with_an_incomplete_mana_plan_leaves_no_stack_object_or_cost() {
    let mut g = game("{2}: You gain 3 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["601.2h", "118.3"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, activate(source, vec![], vec![], vec![step(land, 0)], colorless(1))));
    // the land's mana ability, activated in the plan, is reversed with the rest (733.1)
    cite!(["733.1"]; assert_eq!(format!("{g:?}"), before));
}

#[test]
fn r605_3a_an_activation_without_a_mana_payment_does_not_open_a_mana_window() {
    let mut g = game("Pay 1 life: You gain 3 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["605.3a"]; refused("there is no mana payment during which to activate mana abilities (605.3a)"): g.act(0, activate(source, vec![], vec![], vec![step(land, 0)], colorless(0))));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r602_5b_nested_mana_activation_does_not_bypass_a_once_per_turn_limit() {
    let mut g = game("{1}: Add {C}{C}. Activate only once each turn.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colorless = 1;
    let before = format!("{g:?}");
    cite!(["602.5"]; refused("that ability is activated only once each turn (602.1b)"): g.act(0, activate(source, vec![], vec![], vec![Step { plan: vec![], x: None, life: vec![], with:Some(colorless(1)), ..step(source,0) }], colorless(1))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, activate(source, vec![], vec![], vec![], colorless(1))).unwrap();
    assert_eq!(g.players[0].pool.total(), 2);
    cite!(["602.5"]; refused("that ability is activated only once each turn (602.1b)"): g.act(0, activate(source, vec![], vec![], vec![], colorless(1))));
}
