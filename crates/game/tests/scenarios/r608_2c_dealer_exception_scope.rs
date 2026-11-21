//! The damage-dealer exception is confined to direct subject lookup (608.2c).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};

fn spell(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }

#[test]
fn dealer_exception_does_not_cover_selector_filter_or_later_amount() {
    for (text, refusal) in [
        ("This spell deals 1 damage to any target. Target creature with power equal to its power deals 2 damage to target creature.", "input reference: nothing before it supplies"),
        ("This spell deals 1 damage to any target. Then it deals 2 damage to each creature. You gain life equal to its power.", "reference: unrefined mixed target"),
    ] {
        let err = game::play::check(&spell(text)).expect_err("nested/later read must not inherit dealer exception");
        assert!(err.0.contains(refusal), "{text}: {}", err.0);
    }
}

#[test]
fn dealer_subject_keeps_source_and_its_own_power_without_rebinding_recipients() {
    let d = CardDef::new("Probe", "{0}", "Creature — Beast",
        "{T}: This creature deals 1 damage to any target. Then it deals damage equal to its power to each creature your opponents control.", Some((3, 9))).unwrap();
    game::play::check(&d).unwrap();
    let wall = CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((2, 9))).unwrap();
    for player_target in [false, true] {
        let mut g = Game::new(vec![d.clone(), wall.clone()], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let target = g.put(1, 1, Zone::Battlefield);
        let first = if player_target { Target::Player(1) } else { Target::Object(target) };
        g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![first]], paying: vec![] }).unwrap();
        for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
        let dealt: Vec<_> = g.log.iter().filter_map(|e| match e {
            Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None,
        }).collect();
        cite!(["608.2h"]; case: format!("player_target={player_target}"); assert_eq!(dealt, vec![(source, first, 1), (source, Target::Object(target), 3)]));
    }
}
