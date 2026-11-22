//! An explicitly named counter-payment source remains the ability's actor.
use crate::support::{cite, oracle, paid};
use game::{Game, Move, Target, Zone};

fn resolve(g: &mut Game) {
    for _ in 0..8 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("ability did not resolve");
}

#[test]
fn r107_3a_ballista_casts_for_twice_x_and_enters_with_x_counters() {
    let mut g = Game::new(vec![oracle("Walking Ballista")], 2, 20);
    g.players[0].pool.colorless = 4;
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("CCCC", Move::WithX(2, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(&mut g);
    let permanent = g.battlefield[0];
    cite!(["107.3a", "601.2f"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["614.1c", "122.1a"]; assert_eq!(g.counters(permanent).get("+1/+1"), Some(&2)));
    assert_eq!(g.power(permanent), Some(2));
}

#[test]
fn r113_7a_ballistas_last_counter_pays_before_its_departed_source_deals_damage() {
    let mut g = Game::new(vec![oracle("Walking Ballista")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "+1/+1", 1);
    g.act(0, Move::Activate { source, ability: 2, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    cite!(["602.2b", "704.5f"]; assert!(!g.battlefield.contains(&source)));
    cite!(["113.7a"]; assert_eq!(g.stack.len(), 1));
    let before = format!("{g:?}");
    cite!(["113.6"]; refused("an ability of a permanent on the battlefield is activated (113.6)"):
        g.act(0, Move::Activate { source, ability: 2, targets: vec![vec![Target::Player(1)]], paying: vec![] }));
    cite!(["113.6"]; assert_eq!(format!("{g:?}"), before));
    resolve(&mut g);
    cite!(["113.7a", "608.2h"]; assert_eq!(g.players[1].life, 19));
}

#[test]
fn r608_2h_cost_source_and_selected_cost_objects_keep_separate_references() {
    let def = game::CardDef::new("Witness", "", "Creature", "Remove a +1/+1 counter from this creature, Sacrifice a creature: You gain life equal to the sacrificed creature's power.", Some((2, 2))).unwrap();
    game::play::check(&def).unwrap();
    let other_def = oracle("Hill Giant");
    let mut g = Game::new(vec![def, other_def], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "+1/+1", 1);
    let other = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![vec![other]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2h", "602.2b"]; assert_eq!(g.players[0].life, 23));
    assert!(g.battlefield.contains(&source));
    assert!(!g.battlefield.contains(&other));
}
