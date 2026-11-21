//! Replacement of a life-gain event, once per applicable effect (119.10, 614.5–6).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};

const DOUBLE: usize = 0;
const MONK: usize = 1;
const GAIN: usize = 2;
const WATCHER: usize = 3;
const BAN: usize = 4;

fn game() -> Game {
    let defs = vec![
        CardDef::new(
            "Boon Reflection",
            "{4}{W}",
            "Enchantment",
            "If you would gain life, you gain twice that much life instead.",
            None,
        )
        .unwrap(),
        CardDef::new(
            "Rhox Faithmender",
            "{3}{W}",
            "Creature — Rhino Monk",
            "Lifelink\nIf you would gain life, you gain twice that much life instead.",
            Some((1, 5)),
        )
        .unwrap(),
        CardDef::new("Mend", "{0}", "Instant", "You gain 3 life.", None).unwrap(),
        CardDef::new(
            "Ajani's Pridemate",
            "{1}{W}",
            "Creature — Cat Soldier",
            "Whenever you gain life, put a +1/+1 counter on this creature.",
            Some((2, 2)),
        )
        .unwrap(),
        CardDef::new("Embargo", "{0}", "Enchantment", "Players can't gain life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0));
    }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn gains(g: &Game) -> Vec<(usize, u32)> {
    g.log
        .iter()
        .filter_map(|e| if let Event::LifeGained { player, amount } = e { Some((*player, *amount)) } else { None })
        .collect()
}

#[test]
fn r614_6_only_the_replaced_gain_happens_and_triggers_once() {
    let mut g = game();
    g.put(0, DOUBLE, Zone::Battlefield);
    let watcher = g.put(0, WATCHER, Zone::Battlefield);
    let spell = g.put(0, GAIN, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["614.6"]; assert_eq!(gains(&g), vec![(0, 6)]));
    assert_eq!(g.players[0].life, 26);
    cite!(["614.6"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.power(watcher), Some(3));
}

#[test]
fn r614_5_each_life_gain_replacement_applies_once_to_the_modified_event() {
    let mut g = game();
    g.put(0, DOUBLE, Zone::Battlefield);
    g.put(0, DOUBLE, Zone::Battlefield);
    g.gain_life(0, 3).unwrap();
    cite!(["614.5"]; assert_eq!(gains(&g), vec![(0, 12)]));
    assert_eq!(g.players[0].life, 32);
}

#[test]
fn r113_6_life_gain_replacements_function_only_on_the_battlefield() {
    let mut g = game();
    g.put(0, DOUBLE, Zone::Hand);
    g.put(0, DOUBLE, Zone::Graveyard);
    g.put(0, DOUBLE, Zone::Exile);
    let active = g.put(0, DOUBLE, Zone::Battlefield);
    g.gain_life(0, 1).unwrap();
    g.exile(active);
    g.gain_life(0, 1).unwrap();
    cite!(["113.6"]; assert_eq!(gains(&g), vec![(0, 2), (0, 1)]));
}

#[test]
fn r119_10_your_life_gain_replacement_does_not_modify_an_opponents_gain() {
    let mut g = game();
    g.put(1, DOUBLE, Zone::Battlefield);
    for p in 0..3 {
        g.gain_life(p, 2).unwrap();
    }
    cite!(["109.5"]; assert_eq!(gains(&g), vec![(0, 2), (1, 4), (2, 2)]));
}

#[test]
fn r119_7_prohibited_and_zero_gains_have_no_event_to_replace() {
    let mut g = game();
    g.put(0, DOUBLE, Zone::Battlefield);
    g.gain_life(0, 0).unwrap();
    let ban = g.put(1, BAN, Zone::Battlefield);
    g.gain_life(0, 3).unwrap();
    cite!(["119.10", "119.7"]; assert!(gains(&g).is_empty()));
    assert_eq!(g.players[0].life, 20);
    g.exile(ban);
    g.gain_life(0, 3).unwrap();
    assert_eq!(gains(&g), vec![(0, 6)]);
}

#[test]
fn r120_4c_rhox_replaces_lifelink_results_before_lethal_damage_sbas() {
    let mut g = game();
    let rhox = g.put(0, MONK, Zone::Battlefield);
    g.damage(rhox, Target::Object(rhox), 5);
    cite!(["120.4c", "702.15b"]; assert_eq!(gains(&g), vec![(0, 10)]));
    assert_eq!(g.obj(rhox).damage, 5);
    assert_eq!(g.obj(rhox).zone, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.obj(rhox).zone, Zone::Gone);
    g.gain_life(0, 1).unwrap();
    assert_eq!(gains(&g), vec![(0, 10), (0, 1)]);
}

#[test]
fn r614_5_two_instances_on_one_permanent_each_replace_the_gain_once() {
    let card = CardDef::new("Repeated Blessing", "{0}", "Enchantment",
        "If you would gain life, you gain twice that much life instead.\nIf you would gain life, you gain twice that much life instead.", None).unwrap();
    game::play::check(&card).unwrap();
    let mut g = Game::new(vec![card], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    g.gain_life(0, 2).unwrap();
    cite!(["614.5"]; assert_eq!(gains(&g), vec![(0, 8)]));
}

#[test]
fn r119_10_player_and_opponent_replacement_patterns_keep_the_affected_recipient() {
    for (text, expected) in [
        ("If a player would gain life, that player gains twice that much life instead.", vec![(0, 4), (1, 4), (2, 4)]),
        (
            "If an opponent would gain life, that player gains twice that much life instead.",
            vec![(0, 4), (1, 2), (2, 4)],
        ),
    ] {
        let card = CardDef::new("Shared Blessing", "{0}", "Enchantment", text, None).unwrap();
        game::play::check(&card).unwrap();
        let mut g = Game::new(vec![card], 3, 20);
        g.put(1, 0, Zone::Battlefield);
        for p in 0..3 {
            g.gain_life(p, 2).unwrap();
        }
        assert_eq!(gains(&g), expected, "{text}");
    }
}

#[test]
fn r616_1_ordering_sensitive_and_optional_life_gain_replacements_remain_refused() {
    for text in [
        "If you would gain life, you gain that much life plus 1 instead.",
        "If you would gain life, you gain half that much life, rounded up, instead.",
        "If you would gain life, you may gain twice that much life instead.",
        "If a player would gain life, you gain twice that much life instead.",
        "If you would gain life, you gain twice that much life instead and draw a card.",
        "The next time you would gain life this turn, you gain twice that much life instead.",
    ] {
        // Uncited (admission): half-life wording is outside the grammar; the other
        // shapes parse but are outside the unconditional, same-recipient multiplier.
        if text.contains("half") {
            crate::support::refused(CardDef::new("Unmodeled Blessing", "{0}", "Enchantment", text, None), "text the grammar does not read");
        } else {
            let card = CardDef::new("Unmodeled Blessing", "{0}", "Enchantment", text, None).unwrap();
            crate::support::refused(game::play::check(&card).map_err(|e| e.0), "ability on a permanent: sentence: Replacement");
        }
    }
    for text in [
        "If a creature would gain life, that creature gains twice that much life instead.",
        "If you would gain life, destroy target player instead.",
    ] {
        // Uncited (grammar): neither objects gaining life nor destroying players are valid operations.
        crate::support::refused(CardDef::new("Nonsense", "{0}", "Enchantment", text, None), "text the grammar does not read");
    }
}
