//! Distinct granted abilities may have the same index within their separate definitions.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::state::Decision;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Bear", "", "Creature", "", Some((2, 2))).unwrap(),
        CardDef::new("First grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"When this creature dies, you gain 2 life.\"", None).unwrap(),
        CardDef::new("Second grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"When this creature dies, you lose 1 life.\"", None).unwrap(),
        CardDef::new("Altar", "{0}", "Artifact", "Sacrifice a creature: Add {C}.", None).unwrap(),
        CardDef::new("Targeted grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"When this creature dies, destroy target creature.\"", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn grant(g: &mut Game, definition: usize, target: game::ObjId) {
    let card = g.put(0, definition, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
    pass(g);
}

#[test]
fn r603_3b_distinct_granted_triggers_on_one_source_are_offered_in_controller_chosen_order() {
    let mut g = game();
    let bear = g.put(0, 0, Zone::Battlefield);
    let altar = g.put(0, 3, Zone::Battlefield);
    grant(&mut g, 1, bear); grant(&mut g, 2, bear);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![bear]] }).unwrap();
    let Some(Decision::Order { player: 0, abilities }) = &g.decision else {
        panic!("the two different granted triggers need an order: {:?}", g.decision)
    };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    assert_eq!(abilities[0].source, abilities[1].source);
    assert_eq!(abilities[0].index, abilities[1].index);
    assert_ne!(abilities[0].origin, abilities[1].origin);
    let mut other = g.clone();
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    other.act(0, Move::Order(vec![1, 0])).unwrap();
    pass(&mut g); pass(&mut other);
    cite!(["603.3b"]; assert_eq!(g.players[0].life, 19, "second grant was placed on top"));
    cite!(["603.3b"]; assert_eq!(other.players[0].life, 22, "first grant was placed on top"));
    pass(&mut g); pass(&mut other);
    assert_eq!((g.players[0].life, other.players[0].life), (21, 21));
}

#[test]
fn r603_3d_a_granted_trigger_keeps_its_targets_after_the_source_has_left() {
    let mut g = game();
    let bear = g.put(0, 0, Zone::Battlefield);
    let victim = g.put(1, 0, Zone::Battlefield);
    let altar = g.put(0, 3, Zone::Battlefield);
    grant(&mut g, 4, bear);
    g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![bear]] }).unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    cite!(["603.3d"]; refused("a decision is pending: choose the targets of the triggered ability"): g.act(0, Move::Pass));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(victim)]])).unwrap();
    pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.obj(victim).zone, Zone::Gone));
}
