//! Quantified player queries in conditions: an opponent versus every opponent (102.2, 608.2h).
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
fn baron() -> CardDef {
    CardDef::new("Blood Baron of Vizkopa", "{3}{W}{B}", "Creature — Vampire",
        "Lifelink, protection from white and from black\nAs long as you have 30 or more life and an opponent has 10 or less life, this creature gets +6/+6 and has flying.",Some((4,4))).unwrap()
}
#[test]
fn r611_3a_blood_baron_rechecks_each_threshold_and_any_living_opponent() {
    let d = baron();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    for (life, others, big) in [
        (30, [20, 20], false),
        (29, [10, 20], false),
        (30, [10, 20], true),
        (30, [11, 10], true),
        (30, [11, 11], false),
    ] {
        g.players[0].life = life;
        g.players[1].life = others[0];
        g.players[2].life = others[1];
        cite!(["611.3a"]; case: format!("{life} {others:?}"); assert_eq!(g.power(source), Some(if big { 10 } else { 4 })));
        assert_eq!(g.has(source, "Flying"), big);
    }
    g.players[1].life = 0;
    assert_eq!(g.power(source), Some(10));
    g.act(0, Move::Pass).unwrap();
    assert!(g.players[1].lost);
    cite!(["611.3a"]; assert_eq!(g.power(source), Some(4)));
}
fn reward(condition: &str, lives: [i32; 3]) -> i32 {
    let text = format!("If {condition}, you gain 3 life.");
    let d = CardDef::new("Reward", "{0}", "Sorcery", &text, None).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    for (p, life) in g.players.iter_mut().zip(lives) {
        p.life = life;
    }
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.players[0].life - lives[0]
}
#[test]
fn r102_2_an_opponent_and_each_opponent_have_different_condition_quantifiers() {
    assert_eq!(reward("an opponent has 10 or less life", [20, 10, 30]), 3);
    assert_eq!(reward("each opponent has 10 or less life", [20, 10, 30]), 0);
    assert_eq!(reward("each opponent has 10 or less life", [20, 10, 10]), 3);
    cite!(["806.1"]; assert_eq!(reward("an opponent has 10 or less life", [10, 20, 30]), 0, "you are not your opponent"));
    assert_eq!(reward("a player has 10 or less life", [10, 20, 30]), 3, "a player includes you");
}
#[test]
fn r608_2h_player_life_comparisons_quantify_either_side_of_the_comparison() {
    for text in [
        "your life total is greater than an opponent's life total",
        "an opponent's life total is less than your life total",
    ] {
        assert_eq!(reward(text, [20, 10, 30]), 3);
        assert_eq!(reward(text, [10, 10, 30]), 0);
    }
}
#[test]
fn r109_5_static_player_queries_are_relative_to_the_permanents_controller() {
    let d = baron();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(1, 0, Zone::Battlefield);
    g.players[1].life = 30;
    g.players[0].life = 10;
    cite!(["109.5"]; assert_eq!(g.power(source), Some(10)));
    g.players[0].life = 11;
    cite!(["109.5"]; assert_eq!(g.power(source), Some(4)));
}

#[test]
fn r608_2h_life_conditions_are_checked_on_resolution_not_when_cast() {
    for (initial, later, gain) in [(11, 10, 3), (10, 11, 0)] {
        let d = CardDef::new("Reward", "{0}", "Sorcery", "If an opponent has 10 or less life, you gain 3 life.", None)
            .unwrap();
        let mut g = Game::new(vec![d], 3, 20);
        g.players[1].life = initial;
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        g.players[1].life = later;
        for _ in 0..3 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        cite!(["608.2h"]; case: initial; assert_eq!(g.players[0].life, 20 + gain));
    }
}

#[test]
fn r119_1_life_conditions_do_not_assign_life_totals_to_permanents() {
    for noun in ["a creature", "each artifact", "an enchantment"] {
        let text = format!("If {noun} has 10 or less life, you gain 3 life.");
        // Uncited (grammar): these holders do not have life totals (119.1).
        crate::support::refused(CardDef::new("Nonsense", "{0}", "Sorcery", &text, None),"text the grammar does not read");
    }
}
