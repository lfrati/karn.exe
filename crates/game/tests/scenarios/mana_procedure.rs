//! Mana abilities use the retained resolution driver without using the Magic stack (605.3b).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};
use game::operation::ReplacementId;

#[test]
fn r605_3b_mana_ability_replacement_wait_retains_cost_prefix_and_priority_owner() {
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add {G}.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 3, 20);
    let source = g.put(1, 0, Zone::Battlefield);
    g.turn = 2;
    let scales = g.put(1, 1, Zone::Battlefield);
    let evolution = g.put(1, 2, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.3b"]; assert!(g.stack.is_empty()));
    assert!(g.obj(source).tapped);
    assert_eq!((g.players[1].life, g.players[1].pool.total()), (22, 0));
    assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Replacement { .. } })));
    for player in 0..3 { cite!(["616.1"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(player, Move::Pass)); }
    crate::support::refused(g.act(0, Move::Choose(Answer::Replacement(ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(0), 0)))), "that decision is another player's");
    let mut clone = g.clone();
    for (game, first, count) in [(&mut g, scales, 4), (&mut clone, evolution, 3)] {
        game.act(1, Move::Choose(Answer::Replacement(ReplacementId::from_ability(first, game::binding::AbilityKey::printed(0), 0)))).unwrap();
        cite!(["616.1e", "616.1f"]; case: format!("{count} counters"); assert_eq!(game.obj(source).counters["+1/+1"], count));
        assert_eq!((game.players[1].life, game.players[1].pool.total()), (22, 1));
        assert_eq!((game.active, game.priority), (0, 1));
        assert!(game.decision.is_none() && game.resolving().is_none() && game.stack.is_empty());
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::LifeGained { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::Activated { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, Event::TappedForMana { .. })).count(), 1);
        assert!(!game.log.iter().any(|e| matches!(e, Event::Resolved { .. })));
    }
}
