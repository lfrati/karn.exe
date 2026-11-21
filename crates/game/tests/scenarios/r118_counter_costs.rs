//! Fixed named counters are a consumable payment resource (118.3, 118.10, 122.1).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::legal::Step;
fn green(n:u32) -> game::state::Pool {game::state::Pool {colors:[0,0,0,0,n],..Default::default()}}
fn step(source: ObjId) -> Step { Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None } }
use ir::{Color, mana::ManaSymbol};

fn card(name: &str, types: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, None).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn game(text: &str) -> Game {
    Game::new(vec![
        card("Resource", "Artifact", text),
        card("Spell", "Instant", "You gain 1 life."),
    ], 2, 20)
}
fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] })
}
fn resolve(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
fn removed(g: &Game, source: ObjId) -> Vec<u32> {
    g.log.iter().filter_map(|e| match e { Event::CountersRemoved { from: Target::Object(o), count, .. } if *o == source => Some(*count), _ => None }).collect()
}

#[test]
fn r118_3_tumble_magnet_pays_before_resolution_and_cannot_spend_an_absent_counter() {
    let magnet = card("Tumble Magnet", "Artifact", "This artifact enters with three charge counters on it.\n{T}, Remove a charge counter from this artifact: Tap target artifact or creature.");
    let mut g = Game::new(vec![magnet, card("Target", "Artifact", "")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let target = g.put(1, 1, Zone::Battlefield);
    g.put_counters(source, "charge", 1);
    activate(&mut g, source, 1, vec![vec![Target::Object(target)]]).unwrap();
    assert!(g.obj(source).tapped);
    cite!(["602.2b", "601.2h"]; assert!(!g.obj(source).counters.contains_key("charge")));
    assert!(!g.obj(target).tapped);
    assert_eq!(removed(&g, source), vec![1]);
    resolve(&mut g);
    assert!(g.obj(target).tapped);
    g.untap(source);
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("Tumble Magnet can't have 1 charge counters removed to pay this cost (118.3, 118.10)"): activate(&mut g, source, 1, vec![vec![Target::Object(target)]]));
    assert_eq!(format!("{g:?}"), before, "failure neither taps nor removes anything");
}

#[test]
fn r118_10_two_parts_cannot_remove_the_same_counter() {
    let mut g = game("Remove a charge counter from this artifact, Remove a charge counter from this artifact: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "charge", 1);
    cite!(["118.10"]; refused("Resource can't have 1 charge counters removed to pay this cost (118.3, 118.10)"): activate(&mut g, source, 0, vec![]));
    assert_eq!(g.obj(source).counters["charge"], 1);
    g.put_counters(source, "charge", 1);
    activate(&mut g, source, 0, vec![]).unwrap();
    assert!(!g.obj(source).counters.contains_key("charge"));
    cite!(["118.10"]; assert_eq!(removed(&g, source), vec![1, 1]));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r118_3_only_the_named_counter_kind_pays() {
    let mut g = game("Remove two charge counters from this artifact: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "charge", 1);
    g.put_counters(source, "storage", 9);
    cite!(["118.3"]; refused("Resource can't have 2 charge counters removed to pay this cost (118.3, 118.10)"): activate(&mut g, source, 0, vec![]));
    g.put_counters(source, "charge", 1);
    activate(&mut g, source, 0, vec![]).unwrap();
    assert_eq!(g.obj(source).counters["storage"], 9);
    assert_eq!(removed(&g, source), vec![2]);
}

#[test]
fn r601_2h_remove_before_sacrifice_regardless_of_printed_cost_order() {
    for cost in ["Sacrifice this artifact, Remove a charge counter from this artifact", "Remove a charge counter from this artifact, Sacrifice this artifact"] {
        let mut g = game(&format!("{cost}: You gain 1 life."));
        let source = g.put(0, 0, Zone::Battlefield);
        g.put_counters(source, "charge", 1);
        activate(&mut g, source, 0, vec![]).unwrap();
        assert_eq!(removed(&g, source), vec![1]);
        let events: Vec<_> = g.log.iter().collect();
        let removal = events.iter().position(|e| matches!(e, Event::CountersRemoved { .. })).unwrap();
        let departure = events.iter().position(|e| matches!(e, Event::ZoneChanged { was, .. } if *was == source)).unwrap();
        assert!(removal < departure);
        resolve(&mut g);
        assert_eq!(g.players[0].life, 21);
    }
}

#[test]
fn r605_3b_tendo_ice_bridge_offers_colored_mana_only_while_it_can_pay() {
    let d = card("Tendo Ice Bridge", "Land", "This land enters with a charge counter on it.\n{T}: Add {C}.\n{T}, Remove a charge counter from this land: Add one mana of any color.");
    let mut g = Game::new(vec![d], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "charge", 1);
    let m = Move::Activate { source, ability: 2, targets: vec![], paying: vec![] };
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(m))).unwrap();
    assert_eq!(g.players[0].pool.total(), 1);
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    g.untap(source);
    let m = Move::Activate { source, ability: 2, targets: vec![], paying: vec![] };
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("Tendo Ice Bridge can't have 1 charge counters removed to pay this cost"): g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(m))));
    assert_eq!(format!("{g:?}"), before);
    cite!(["605.3b"]; ok: activate(&mut g, source, 1, vec![]));
    assert_eq!(g.players[0].pool.total(), 2);
}

#[test]
fn r601_2g_repeatable_counter_mana_production_is_finite_and_listed_plan_executes() {
    // Same primitive as a charge-counter mana artifact, without a tap: repeated use must
    // consume separate counters in the explicitly authored two-activation plan.
    let resource = card("Resource", "Artifact", "Remove a charge counter from this artifact: Add {G}.");
    let spell = CardDef::new("Spell", "{G}{G}", "Instant", "You gain 1 life.", None).unwrap();
    let mut g = Game::new(vec![resource, spell], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.put_counters(source, "charge", 1);
    let plan = vec![step(source), step(source)];
    let before = format!("{g:?}");
    cite!(["118.3", "118.10"]; refused("Resource can't have 1 charge counters removed to pay this cost"): g.act(0, Move::Paying(green(2),Box::new(Move::WithMana(plan.clone(), Box::new(Move::Cast { card: spell, targets: vec![] }))))));
    assert_eq!(format!("{g:?}"), before);
    g.put_counters(source, "charge", 1);
    g.act(0, Move::Paying(green(2),Box::new(Move::WithMana(plan, Box::new(Move::Cast { card: spell, targets: vec![] }))))).unwrap();
    cite!(["601.2g"]; assert_eq!(removed(&g, source), vec![1, 1]));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r122_2_departure_is_not_removal_and_actual_removal_is_bounded() {
    let mut g = game("");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "charge", 2);
    cite!(["609.3"]; assert_eq!(g.remove_counters(source, "charge", 5), 2));
    cite!(["609.3"]; assert_eq!(g.remove_counters(source, "charge", 1), 0));
    assert_eq!(removed(&g, source), vec![2]);
    g.put_counters(source, "charge", 3);
    g.exile(source);
    cite!(["122.2"]; assert_eq!(removed(&g, source), vec![2]));
    assert_eq!(g.remove_counters(source, "charge", 1), 0);
}

#[test]
fn r118_3_counter_choices_and_variable_amounts_remain_explicit_refusals() {
    for text in [
        "Remove a counter from this artifact: You gain 1 life.",
        "Remove X charge counters from this artifact: You gain 1 life.",
        "Remove a +1/+1 counter from a creature you control: You gain 1 life.",
    ] {
        let d = CardDef::new("Resource", "{0}", "Artifact", text, None).unwrap();
        assert!(game::play::check(&d).unwrap_err().0.contains("removing counters requires"), "{text}");
    }
}

#[test]
fn r118_10_mana_production_reserves_the_final_counter_cost() {
    let mut g = game("Remove a charge counter from this artifact: Add {G}.\n{G}, Remove a charge counter from this artifact: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "charge", 1);
    let before = format!("{g:?}");
    cite!(["118.10"]; refused("Resource can't have 1 charge counters removed to pay this cost"): g.act(0, Move::Paying(green(1),Box::new(Move::WithMana(vec![step(source)], Box::new(Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }))))));
    assert_eq!(format!("{g:?}"), before);
    g.put_counters(source, "charge", 1);
    let plan = vec![step(source)];
    g.act(0, Move::Paying(green(1),Box::new(Move::WithMana(plan, Box::new(Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }))))).unwrap();
    cite!(["118.10"]; assert_eq!(removed(&g, source), vec![1, 1]));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r118_3_remove_a_keyword_counter_changes_effective_abilities_immediately() {
    let mut g = game("Remove an indestructible counter from this artifact: You gain 1 life.");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "indestructible", 1);
    cite!(["122.1b"]; assert!(g.has(source, "Indestructible")));
    activate(&mut g, source, 0, vec![]).unwrap();
    cite!(["122.1b"]; assert!(!g.has(source, "Indestructible")));
}
