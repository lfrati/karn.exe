//! Explicit extrema compose with effective characteristics and retained simultaneous choices.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use std::sync::OnceLock;
fn full() -> CardDef {
    static D: OnceLock<CardDef> = OnceLock::new();
    D.get_or_init(|| {
        let fs = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let cat = CardDef::catalog(&fs);
        let d = CardDef::from_faces(&cat["Papalymo Totolymo"]).unwrap();
        game::play::check(&d).unwrap();
        d
    })
    .clone()
}
fn creature(n: &str, p: i32, t: &str) -> CardDef {
    CardDef::new(n, "{0}", "Creature", t, Some((p, 12))).unwrap()
}
fn setup() -> (Game, ObjId) {
    let mut g = Game::new(
        vec![
            full(),
            creature("Bear", 2, ""),
            creature("Large", 4, ""),
            creature("Negative", -2, ""),
            creature("More negative", -4, ""),
            CardDef::new("Boost", "{0}", "Instant", "Target creature gets +3/+0 until end of turn.", None).unwrap(),
            CardDef::new("Cantrip", "{0}", "Instant", "You draw a card.", None).unwrap(),
            creature("Watcher", 4, "Whenever another creature dies, you gain 1 life."),
        ],
        4,
        20,
    );
    for p in 0..4 {
        for _ in 0..8 {
            g.put(p, 1, Zone::Library);
        }
    }
    let source = g.put(0, 0, Zone::Battlefield);
    (g, source)
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn activate(g: &mut Game, s: ObjId) {
    g.players[0].pool.colorless = 4;
    g.act(0, paid("CCCC", Move::Activate { source: s, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    assert_eq!(g.obj(s).zone, Zone::Gone);
    pass(g);
}
fn asked(g: &Game, p: usize) -> Vec<ObjId> {
    let Some(Decision::Choose { player, choice: Choice::Objects { among, verb, count, .. } }) = &g.decision else {
        panic!("{:?}", g.decision)
    };
    assert_eq!((*player, *verb, *count), (p, ir::Verb::Sacrifice, 1));
    among.clone()
}
#[test]
fn full_card_chooses_ties_separately_and_sacrifices_simultaneously() {
    let (mut g, s) = setup();
    g.lose_life(1, 1).unwrap();
    g.pay_life(2, 1).unwrap();
    let a = g.put(1, 1, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    let c = g.put(2, 2, Zone::Battlefield);
    let d = g.put(2, 2, Zone::Battlefield);
    g.put(3, 2, Zone::Battlefield);
    activate(&mut g, s);
    assert_eq!(asked(&g, 1), vec![a, b]);
    g.act(1, Move::Choose(Answer::Objects(vec![a]))).unwrap();
    cite!(["608.2e"]; assert_eq!(g.obj(a).zone, Zone::Battlefield));
    cite!(["608.2e", "101.4", "119.4"]; assert_eq!(asked(&g, 2), vec![c, d]));
    let clone = g.clone();
    for mut g in [g, clone] {
        g.act(2, Move::Choose(Answer::Objects(vec![d]))).unwrap();
        cite!(["608.2e"]; assert_eq!(g.obj(a).zone, Zone::Gone));
        cite!(["608.2e"]; assert_eq!(g.obj(d).zone, Zone::Gone));
        assert_eq!(g.obj(b).zone, Zone::Battlefield);
        assert_eq!(g.obj(c).zone, Zone::Battlefield);
        assert_eq!(g.players[3].life, 20);
    }
}

#[test]
fn negative_power_empty_pools_and_gain_after_loss() {
    let (mut g, s) = setup();
    g.lose_life(1, 2).unwrap();
    g.gain_life(1, 8).unwrap();
    g.pay_life(2, 1).unwrap();
    let a = g.put(1, 3, Zone::Battlefield);
    let tie = g.put(1, 3, Zone::Battlefield);
    let lower = g.put(1, 4, Zone::Battlefield);
    g.put(3, 2, Zone::Battlefield);
    activate(&mut g, s);
    cite!(["107.1b"]; assert_eq!(asked(&g, 1), vec![a, tie]));
    crate::support::refused(g.act(2, Move::Choose(Answer::Objects(vec![a]))), "that decision is another player's");
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects to sacrifice (608.2d)"): g.act(1, Move::Choose(Answer::Objects(vec![lower]))));
    assert_eq!(asked(&g, 1), vec![a, tie]);
    g.act(1, Move::Choose(Answer::Objects(vec![a]))).unwrap();
    assert!(g.decision.is_none());
    assert_eq!(g.obj(lower).zone, Zone::Battlefield);
}
#[test]
fn effective_power_is_read_at_resolution() {
    let (mut g, s) = setup();
    g.pay_life(1, 1).unwrap();
    let a = g.put(1, 1, Zone::Battlefield);
    let b = g.put(1, 2, Zone::Battlefield);
    g.players[0].pool.colorless = 4;
    g.act(0, paid("CCCC", Move::Activate { source: s, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    let boost = g.put(0, 5, Zone::Hand);
    g.act(0, Move::Cast { card: boost, targets: vec![vec![Target::Object(a)]] }).unwrap();
    pass(&mut g);
    pass(&mut g);
    assert!(g.decision.is_none());
    cite!(["608.2h"]; assert_eq!(g.obj(a).zone, Zone::Gone));
    cite!(["608.2h"]; assert_eq!(g.obj(b).zone, Zone::Battlefield));
}
#[test]
fn noncreature_trigger_precedes_spell_and_gains_life() {
    let (mut g, _) = setup();
    let n = g.players[0].hand.len();
    let c = g.put(0, 6, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    cite!(["603.3"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g);
    cite!(["405.5"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![21, 19, 19, 19]));
    cite!(["405.5"]; assert_eq!(g.players[0].hand.len(), n));
    pass(&mut g);
    assert_eq!(g.players[0].hand.len(), n + 1);
    let c = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    assert_eq!(g.stack.len(), 1);
}
#[test]
fn simultaneous_deaths_capture_other_creature_trigger() {
    let (mut g, s) = setup();
    g.lose_life(1, 1).unwrap();
    g.lose_life(2, 1).unwrap();
    let a = g.put(1, 7, Zone::Battlefield);
    let b = g.put(2, 2, Zone::Battlefield);
    g.players[0].pool.colorless = 4;
    g.act(0, paid("CCCC", Move::Activate { source: s, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    pass(&mut g);
    assert_eq!(g.players[1].life, 20);
    pass(&mut g);
    assert_eq!(g.obj(a).zone, Zone::Gone);
    assert_eq!(g.obj(b).zone, Zone::Gone);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    cite!(["603.10a"]; assert_eq!(g.players[1].life, 21));
}
#[test]
fn explicit_you_remains_effect_controller() {
    let d = CardDef::new(
        "Relative",
        "{0}",
        "Sorcery",
        "Each opponent sacrifices a creature with the greatest power among creatures you control.",
        None,
    )
    .unwrap();
    let mut g = Game::new(vec![d, creature("Small", 2, ""), creature("Large", 4, "")], 3, 20);
    g.put(0, 1, Zone::Battlefield);
    let a = g.put(1, 1, Zone::Battlefield);
    let tie = g.put(1, 1, Zone::Battlefield);
    g.put(1, 2, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    pass(&mut g);
    cite!(["109.5"]; assert_eq!(asked(&g, 1), vec![a, tie]));
}

#[test]
fn explicit_least_toughness_and_greatest_mana_value() {
    for (text, small_cost, small_pt, large_cost, large_pt) in [
        (
            "Each opponent sacrifices a creature with the least toughness among creatures they control.",
            "{0}",
            (2, 1),
            "{0}",
            (2, 4),
        ),
        (
            "Each opponent sacrifices a creature with the greatest mana value among creatures they control.",
            "{4}",
            (2, 2),
            "{1}",
            (4, 4),
        ),
    ] {
        let d = CardDef::new("Extrema", "{0}", "Sorcery", text, None).unwrap();
        let mut g = Game::new(
            vec![
                d,
                CardDef::new("Chosen", small_cost, "Creature", "", Some(small_pt)).unwrap(),
                CardDef::new("Other", large_cost, "Creature", "", Some(large_pt)).unwrap(),
            ],
            2,
            20,
        );
        let chosen = g.put(1, 1, Zone::Battlefield);
        let other = g.put(1, 2, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
        pass(&mut g);
        // Uncited (card text): "a creature with the least toughness / greatest mana value among creatures they control": only "Chosen" qualifies
        assert_eq!(g.obj(chosen).zone, Zone::Gone);
        assert_eq!(g.obj(other).zone, Zone::Battlefield);
    }
}
#[test]
fn implicit_comparison_set_stays_refused() {
    let d = CardDef::new(
        "Implicit",
        "{0}",
        "Sorcery",
        "Each opponent sacrifices a creature with the greatest power.",
        None,
    );
    let d=d.expect("implicit comparison grammar is accepted");
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"extrema description: an explicit comparison set is required");
}
#[test]
fn stolen_creature_is_chosen_by_controller_and_returns_to_owner() {
    let (mut g, s) = setup();
    g.lose_life(1, 1).unwrap();
    let a = g.put(2, 2, Zone::Battlefield);
    std::sync::Arc::make_mut(&mut g.defs)
        .push(CardDef::new("Steal", "{0}", "Instant", "Gain control of target creature.", None).unwrap());
    let steal = g.put(1, 8, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(a)]] }).unwrap();
    pass(&mut g);
    assert_eq!(g.controller(a), 1);
    activate(&mut g, s);
    assert_eq!(g.obj(a).zone, Zone::Gone);
    cite!(["701.21a"]; assert_eq!(g.players[1].graveyard.len(), 1)); // The resolved control spell only.
    assert_eq!(g.obj(g.players[1].graveyard[0]).chars.name, "Steal");
    cite!(["701.21a"]; assert_eq!(g.players[2].graveyard.len(), 1));
}

#[test]
fn captured_trigger_survives_source_sacrifice_cost() {
    let (mut g, s) = setup();
    let c = g.put(0, 6, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    g.players[0].pool.colorless = 4;
    g.act(0, paid("CCCC", Move::Activate { source: s, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    assert_eq!(g.obj(s).zone, Zone::Gone);
    pass(&mut g);
    assert_eq!(g.stack.len(), 2);
    pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![21, 19, 19, 19]));
}
