//! Vanishing composes ordinary entry counters, intervening-if upkeep and last-counter triggers.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::Decision;
use ir::Part;

fn resolve(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn next_upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no upkeep");
}
fn stock(g: &mut Game, index: usize) { for p in 0..2 { for _ in 0..12 { g.put(p, index, Zone::Library); } } }
fn cast(g: &mut Game, index: usize, mana: &str) {
    let card = g.put(0, index, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    g.act(0, paid(mana, Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
}
fn named(g: &Game, name: &str) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).chars.name == name).unwrap() }

#[test]
fn r702_63a_aven_riftwatcher_enters_with_three_and_sacrifice_uses_a_separate_trigger() {
    let mut g = Game::new(vec![oracle("Aven Riftwatcher"), oracle("Forest")], 2, 20);
    stock(&mut g, 1);
    cast(&mut g, 0, "WWW");
    let bird = named(&g, "Aven Riftwatcher");
    cite!(["702.63a"]; assert_eq!(g.counters(bird).get("time"), Some(&3)));
    resolve(&mut g);
    // Uncited (Oracle): its ordinary entry ability gains two life.
    assert_eq!(g.players[0].life, 22);
    for count in [2, 1, 0] {
        next_upkeep(&mut g);
        cite!(["702.63a"]; assert_eq!(g.stack.len(), 1));
        resolve(&mut g);
        cite!(["702.63a"]; assert_eq!(g.counters(bird).get("time").copied().unwrap_or(0), count));
        assert!(g.battlefield.contains(&bird));
    }
    cite!(["702.63a", "603.3"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    cite!(["701.21a"]; assert!(!g.battlefield.contains(&bird)));
    resolve(&mut g);
    // Uncited (Oracle): its leave ability also gains two life.
    assert_eq!(g.players[0].life, 24);
}

#[test]
fn r702_63b_bare_and_fixed_zero_vanishing_have_no_upkeep_trigger_without_a_counter() {
    for keyword in ["Vanishing", "Vanishing 0"] {
        let artifact = CardDef::new("Counterless vanishing", "{0}", "Artifact", keyword, None).unwrap();
        game::play::check(&artifact).unwrap();
        let mut g = Game::new(vec![artifact, oracle("Forest")], 2, 20);
        stock(&mut g, 1);
        cast(&mut g, 0, "");
        let source = named(&g, "Counterless vanishing");
        cite!(["702.63a", "702.63b"]; assert!(g.counters(source).is_empty()));
        next_upkeep(&mut g);
        cite!(["603.4", "702.63b"]; assert!(g.stack.is_empty() && g.battlefield.contains(&source)));
    }
}

#[test]
fn r702_63a_replacing_the_last_counter_before_trigger_delivery_does_not_save_the_source() {
    let remover = CardDef::new("Counter exchange", "{0}", "Artifact",
        "{0}: Remove a time counter from target permanent. Put a time counter on it.", None).unwrap();
    let mut g = Game::new(vec![oracle("Soultether Golem"), remover], 2, 20);
    let tool = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, "CC");
    let golem = named(&g, "Soultether Golem");
    assert_eq!(g.counters(golem).get("time"), Some(&1));
    g.act(0, Move::Activate { source: tool, ability: 0, targets: vec![vec![golem.into()]], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["603.2", "702.63a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.counters(golem).get("time"), Some(&1));
    resolve(&mut g);
    cite!(["702.63a"]; assert!(!g.battlefield.contains(&golem)));
}

#[test]
fn r702_122b_crew_identity_survives_multiple_generated_vanishing_abilities() {
    let vehicle = CardDef::new("Vanishing vehicle", "{0}", "Artifact — Vehicle", "Vanishing 2\nCrew 2", Some((3, 3))).unwrap();
    game::play::check(&vehicle).unwrap();
    let mut g = Game::new(vec![vehicle, oracle("Veteran Motorist")], 2, 20);
    let vehicle = g.put(0, 0, Zone::Battlefield);
    let motorist = g.put(0, 1, Zone::Battlefield);
    let crew = g.abilities(vehicle).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    g.act(0, Move::Activate { source: vehicle, ability: crew, targets: vec![], paying: vec![vec![motorist]] }).unwrap();
    cite!(["702.122b"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    resolve(&mut g);
    cite!(["702.122a"]; assert_eq!((g.power(vehicle), g.toughness(vehicle)), (Some(4), Some(4))));
}

#[test]
fn r702_35a_madness_identity_survives_multiple_generated_vanishing_abilities() {
    let creature = CardDef::new("Vanishing madness", "{3}", "Creature", "Vanishing 2\nMadness {0}", Some((2, 2))).unwrap();
    game::play::check(&creature).unwrap();
    let discard = CardDef::new("Discard instruction", "{0}", "Sorcery", "Discard a card.", None).unwrap();
    let mut g = Game::new(vec![creature, discard], 2, 20);
    g.put(0, 0, Zone::Hand);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.exile.len(), 1);
    let exiled = g.exile[0];
    cite!(["702.35a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    cite!(["702.35a"]; assert!(matches!(g.asked(), Some((0, game::legal::Asked::Cast { cards })) if cards == vec![exiled])));
    g.act(0, Move::CastFor(game::announcement::CastCost::Alternative(1), Box::new(Move::Cast { card: exiled, targets: vec![] }))).unwrap();
    resolve(&mut g);
    let source = named(&g, "Vanishing madness");
    cite!(["702.35b", "702.63a"]; assert_eq!(g.counters(source).get("time"), Some(&2)));
}
