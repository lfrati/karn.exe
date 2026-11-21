//! An effect offers alternative payments; the chosen payment uses the same object procedure.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Zone};
fn setup(hand: usize, lands: usize) -> Game {
    // Highway Robbery's resolution text, isolated from its printed Plot ability.
    let defs = vec![
        CardDef::new(
            "Robbery effect",
            "{0}",
            "Sorcery",
            "You may discard a card or sacrifice a land. If you do, draw two cards.",
            None,
        )
        .unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Witness", "{0}", "Enchantment", "Whenever you discard a card, you gain 1 life.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{} {e:?}", d.name()));
    }
    let mut g = Game::new(defs, 2, 20);
    for _ in 0..10 {
        g.put(0, 1, Zone::Library);
        g.put(1, 1, Zone::Library);
    }
    for _ in 0..hand {
        g.put(0, 1, Zone::Hand);
    }
    for _ in 0..lands {
        g.put(0, 1, Zone::Battlefield);
    }
    g
}
fn cast(g: &mut Game) {
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    passes(g);
}
fn passes(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn options(g: &Game) -> Vec<usize> {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Alternative { among, optional: true } }) => {
            among.iter().map(|option| option.index).collect()
        }
        other => panic!("{other:?}"),
    }
}
fn drawn(g: &Game) -> usize {
    g.log.iter().filter(|e| matches!(e, Event::Drew { player: 0, .. })).count()
}
#[test]
fn r608_2d_only_payable_alternatives_are_offered_with_stable_printed_indices() {
    for (hand, lands, expected) in [(1, 0, vec![0]), (0, 1, vec![1]), (1, 1, vec![0, 1])] {
        let mut g = setup(hand, lands);
        cast(&mut g);
        cite!(["608.2d"]; case: format!("hand {hand}, lands {lands}"); assert_eq!(options(&g), expected));
        let choice = expected[0];
        g.act(0, Move::Choose(Answer::Alternative(choice))).unwrap();
        assert!(g.decision.is_none());
        cite!(["118.12"]; case: format!("hand {hand}, lands {lands}"); assert_eq!(drawn(&g), 2));
        assert_eq!(g.battlefield.len(), lands - usize::from(choice == 1));
        assert_eq!(g.players[0].hand.len(), hand + 2 - usize::from(choice == 0));
    }
}
#[test]
fn r118_3_an_impossible_optional_payment_is_not_offered_and_draws_nothing() {
    let mut g = setup(0, 0);
    cast(&mut g);
    cite!(["118.3", "608.2d"]; assert!(g.decision.is_none()));
    assert!(g.stack.is_empty());
    cite!(["118.12"]; assert_eq!(drawn(&g), 0));
}
#[test]
fn r118_12_declining_an_alternative_payment_does_not_take_the_paid_branch() {
    let mut g = setup(1, 1);
    cast(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!(drawn(&g), 0));
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(g.battlefield.len(), 1);
}
#[test]
fn r608_2d_an_invalid_answer_keeps_the_same_question_and_no_priority_is_given() {
    let mut g = setup(0, 1);
    cast(&mut g);
    let before = g.decision.clone();
    for (p, m) in [
        (0, Move::Pass),
        (1, Move::Choose(Answer::Alternative(1))),
        (0, Move::Choose(Answer::Alternative(0))),
        (0, Move::Choose(Answer::Yes)),
    ] {
        let why = match (p, &m) { (_, Move::Pass) => "a decision is pending: a choice of the effect being applied (608.2d)", (1, _) => "that decision is another player's", _ => "choose one of the offered actions, or decline if optional (608.2d)" };
        crate::support::refused(g.act(p, m), why);
        assert_eq!(g.decision, before);
        assert_eq!(drawn(&g), 0);
    }
}
#[test]
fn r608_2d_clones_keep_the_selected_action_and_object_choice_without_replaying_payment() {
    let mut g = setup(2, 2);
    g.put(0, 2, Zone::Battlefield);
    cast(&mut g);
    let mut other = g.clone();
    g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
    let card = g.players[0].hand[0];
    cite!(["608.2d"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Objects { .. }, .. }))));
    let mut paused = g.clone();
    for g in [&mut g, &mut paused] {
        g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
        cite!(["118.12"]; assert_eq!(drawn(g), 2));
        assert_eq!(g.stack.len(), 1);
        passes(g);
        cite!(["603.2"]; assert_eq!(g.players[0].life, 21));
    }
    other.act(0, Move::Choose(Answer::Alternative(1))).unwrap();
    let land = other.battlefield[0];
    other.act(0, Move::Choose(Answer::Objects(vec![land]))).unwrap();
    assert_eq!(drawn(&other), 2);
    assert_eq!(other.players[0].life, 20);
}
#[test]
fn an_unchosen_alternative_cannot_supply_an_antecedent() {
    let d = CardDef::new(
        "Ambiguous",
        "{0}",
        "Sorcery",
        "You may discard a card or sacrifice a creature. Draw cards equal to that creature's power.",
        None,
    )
    .unwrap();
    assert_eq!(game::play::check(&d).expect_err("unchosen branch cannot bind a creature").0, "input reference: nothing before it supplies the required participant/cardinality");
}

#[test]
fn r118_3_alternative_costs_require_the_whole_quantity_not_a_partial_payment() {
    let defs = vec![
        CardDef::new(
            "Payment",
            "{0}",
            "Sorcery",
            "You may discard two cards or sacrifice two lands. If you do, draw three cards.",
            None,
        )
        .unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    game::play::check(&defs[0]).unwrap();
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 1, Zone::Hand);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g);
    cite!(["118.3"]; assert!(g.decision.is_none()));
    cite!(["118.12"]; assert_eq!(drawn(&g), 0));
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(g.battlefield.len(), 1);
}

#[test]
fn r118_12_the_declined_branch_uses_the_alternative_group_outcome() {
    for decline in [true, false] {
        let defs = vec![
            CardDef::new(
                "Payment",
                "{0}",
                "Sorcery",
                "You may discard a card or sacrifice a land. If you don't, you gain 1 life.",
                None,
            )
            .unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        game::play::check(&defs[0]).unwrap();
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 1, Zone::Battlefield);
        cast(&mut g);
        g.act(0, Move::Choose(if decline { Answer::No } else { Answer::Alternative(1) })).unwrap();
        cite!(["118.12"]; case: format!("decline {decline}"); assert_eq!(g.players[0].life, 20 + i32::from(decline)));
    }
}
