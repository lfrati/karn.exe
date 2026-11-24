//! A delayed triggered ability triggers only once, the next time its trigger event occurs, unless
//! it has a stated duration such as "this turn" (603.7b). "When you next cast …" is once even
//! with "this turn" (the duration only bounds when it can happen). The engine follows delayed
//! triggers about named objects and explicit next event predicates.
use crate::support::{Answer, Board, Choice, Decision, Move, Target, Zone, cite, mana_pool, player};
use game::CardDef;

fn card(text: &str) -> CardDef {
    CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap()
}

#[test]
fn r603_7b_a_delayed_trigger_that_says_next_is_admitted() {
    let next = card("Draw a card. When you next cast a creature spell this turn, you draw a card.");
    game::play::check(&next).unwrap();
}

#[test]
fn r603_7b_a_delayed_trigger_on_a_named_object_is_admitted() {
    let d = card("Destroy target creature. When that creature dies this turn, you draw a card.");
    game::play::check(&d).unwrap();
}

/// Twinferno's first mode: "When you cast your next instant or sorcery spell this turn, copy
/// that spell. You may choose new targets for the copy." A creature spell cast first is not
/// one; the copy is made once, for the next instant or sorcery spell only (603.7b), and new
/// targets may be chosen for it (707.10c).
#[test]
fn r603_7b_twinferno_copies_only_the_next_instant_or_sorcery_spell() {
    let mut t = Board::new(2).battlefield(1, &["Hill Giant"]).hand(0, &["Twinferno", "Grizzly Bears", "Lightning Bolt", "Shock"]).build();
    let card = t.find_in(0, Zone::Hand, "Twinferno");
    t.mana(0, "{1}{R}");
    t.g.act(0, Move::Paying(mana_pool("CR"), Box::new(Move::Modes(vec![0], Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
    t.resolve();
    t.mana(0, "{1}{G}").cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    cite!(["603.7b"]; assert_eq!(t.g.stack.len(), 1, "a creature spell is not an instant or sorcery spell"));
    t.resolve();
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![player(1)], mana_pool("R")).unwrap();
    cite!(["603.7b", "603.2"]; assert_eq!(t.g.stack.len(), 2, "the Bolt and the delayed trigger"));
    t.pass_all();
    let giant = t.find_in_battlefield("Hill Giant");
    match &t.g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Retarget { .. } }) => {}
        other => panic!("expected the copy's new targets: {other:?}"),
    }
    t.answer(0, Answer::Targets(vec![vec![Target::Object(giant)]])).unwrap();
    t.resolve();
    cite!(["707.10c"]; assert_eq!((t.g.players[1].life, t.g.players[1].graveyard.len()), (17, 1)));
    t.mana(0, "{R}").cast(0, "Shock", vec![player(1)], mana_pool("R")).unwrap();
    cite!(["603.7b"]; assert_eq!(t.g.stack.len(), 1, "once: the trigger is gone"));
    t.resolve();
    assert_eq!(t.g.players[1].life, 15);
}
