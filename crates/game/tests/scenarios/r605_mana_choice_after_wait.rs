//! A mana ability's announced mana choice stays owned by its retained resolution when an
//! earlier instruction waits for another player decision (605.3b, 616.1).
use crate::support::cite;
use game::operation::ReplacementId;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Zone};
use ir::mana::ManaSymbol;

#[test]
fn r605_3b_announced_mana_choice_survives_an_earlier_replacement_wait() {
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: Put a +1/+1 counter on this creature. Add one mana of any color.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.turn = 2;
    let scales = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    // The move the engine itself offers: the ability, with the mana it is to add.
    let green = vec![ManaSymbol::Color(ir::Color::Green)];
    let activate = Move::Adding(vec![green], Box::new(Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }));
    g.act(0, activate).expect("an offered mana ability can be activated");
    // Its cost is paid and the replacement order (616.1) is asked before any mana is added.
    assert!(g.obj(source).tapped);
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["616.1"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Replacement { .. } }))));
    g.act(0, Move::Choose(Answer::Replacement(ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(0), 0)))).unwrap();
    // The announced choice is used once the resolution goes on: it is not asked again.
    cite!(["616.1e", "616.1f"]; assert_eq!(g.obj(source).counters["+1/+1"], 4));
    assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 1);
    cite!(["605.3b"]; assert!(g.decision.is_none() && g.resolving().is_none() && g.stack.is_empty()));
    assert_eq!(g.priority, 0);
}

#[test]
fn r608_2h_saved_mana_answer_is_rechecked_after_replacement_and_clones_keep_its_owner() {
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 3, 20);
    let source = g.put(1, 0, Zone::Battlefield);
    let scales = g.put(1, 1, Zone::Battlefield);
    let evolution = g.put(1, 2, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    let green = ManaSymbol::Color(ir::Color::Green);
    let activate = Move::Activate { source, ability: 0, targets: vec![], paying: vec![] };
    g.act(1, Move::Adding(vec![vec![green; 3]], Box::new(activate))).unwrap();
    assert_eq!(g.players[1].life, 22);
    let mut other = g.clone();
    other.act(1, Move::Choose(Answer::Replacement(ReplacementId::from_ability(evolution, game::binding::AbilityKey::printed(0), 0)))).unwrap();
    cite!(["608.2h"]; assert_eq!(other.players[1].pool.total(), 3));
    assert!(other.decision.is_none());
    assert_eq!(other.priority, 1);

    g.act(1, Move::Choose(Answer::Replacement(ReplacementId::from_ability(scales, game::binding::AbilityKey::printed(0), 0)))).unwrap();
    assert_eq!((g.players[1].life, g.players[1].pool.total()), (22, 0));
    cite!(["616.1e", "616.1f"]; assert_eq!(g.obj(source).counters["+1/+1"], 4));
    assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Mana { .. } })));
    crate::support::refused(g.act(0, Move::Choose(Answer::Mana(vec![green; 4]))), "that decision is another player's");
    cite!(["608.2d"]; refused("the choice is which of the mana offered to add (608.2d)"): g.act(1, Move::Choose(Answer::Mana(vec![green; 3]))));
    assert_eq!((g.players[1].life, g.players[1].pool.total()), (22, 0));
    g.act(1, Move::Choose(Answer::Mana(vec![green; 4]))).unwrap();
    cite!(["608.2h"]; assert_eq!((g.players[1].life, g.players[1].pool.total()), (22, 4)));
    assert!(g.decision.is_none() && g.resolving().is_none());
    assert_eq!(g.priority, 1);
    for game in [&g, &other] {
        assert_eq!(game.log.iter().filter(|e| matches!(e, game::Event::Activated { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, game::Event::LifeGained { .. })).count(), 1);
        assert_eq!(game.log.iter().filter(|e| matches!(e, game::Event::TappedForMana { .. })).count(), 1);
    }
}

/// A question asked while a mana ability resolves (605.3a: no stack) is for that ability: of
/// its source, with its text as the seat reads it, and no stack object.
#[test]
fn r605_3a_a_question_during_a_mana_ability_is_for_that_ability() {
    let text = "{T}: Put a +1/+1 counter on this creature. Add one mana of any color.";
    let defs = vec![
        CardDef::new("Mana creature", "{0}", "Creature — Elf", text, Some((1, 1))).unwrap(),
        CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.turn = 2;
    g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    let green = vec![ManaSymbol::Color(ir::Color::Green)];
    g.act(0, Move::Adding(vec![green], Box::new(Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }))).unwrap();
    let mut sim = game::simulation::Simulation::new(g);
    let prompt = sim.prompt(0).unwrap();
    let occurrence = prompt.decision.expect("the replacement order is asked").occurrence.expect("for the ability");
    cite!(["605.3b"]; assert_eq!((occurrence.kind, occurrence.object, occurrence.source, occurrence.source_name.as_str(), occurrence.text.as_deref()),
               (game::prompt::OccurrenceKind::Activated, None, Some(source), "Mana creature", Some("{T}: Put a +1/+1 counter on Mana creature. Add one mana of any color."))));
}
