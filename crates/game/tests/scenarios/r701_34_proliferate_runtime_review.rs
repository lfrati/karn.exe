//! Proliferate (701.34a) beside
//! the instructions around it. "You may proliferate" asks first and nothing more when
//! declined; a following instruction happens exactly once on every branch of a game copied
//! at the holder or replacement question; forms that would proliferate more than once, or
//! have several players proliferate, stay refused.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};

fn def(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    d
}
fn game() -> Game {
    let defs = vec![
        def("Maybe", "Sorcery", "You may proliferate. You gain 1 life.", None),
        def("Progress", "Sorcery", "Proliferate. Draw a card.", None),
        def("Bear", "Creature — Bear", "", Some((2, 2))),
        def("Scale", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None),
        def("Double", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 5, Zone::Library); } }
    g
}
fn cast(g: &mut Game, def: usize) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn holders(g: &Game) -> Vec<Target> {
    match &g.decision { Some(Decision::Choose { player: 0, choice: Choice::Holders { among, .. } }) => among.clone(), d => panic!("{d:?}") }
}

#[test]
fn you_may_proliferate_asks_first_and_declining_asks_no_holders() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 1);
    cast(&mut g, 0);
    cite!(["608.2d"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })));
    let mut yes = g.clone();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert!(g.decision.is_none() && g.stack.is_empty());
    cite!(["608.2d", "608.2c"]; assert_eq!((g.counters(bear).get("+1/+1"), g.players[0].life), (Some(&1), 21)));
    yes.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["701.34a"]; assert_eq!(holders(&yes), vec![Target::Object(bear)]));
    yes.act(0, Move::Choose(Answer::Holders(vec![Target::Object(bear)]))).unwrap();
    cite!(["701.34a"]; assert_eq!((yes.counters(bear).get("+1/+1"), yes.players[0].life), (Some(&2), 21)));
}

/// With nothing to choose, "you may" is an impossible option (608.2d): it is not asked, or
/// a yes does nothing; either way the life is gained once.
#[test]
fn you_may_proliferate_with_no_holder_still_gains_the_life_once() {
    let mut g = game();
    cast(&mut g, 0);
    if g.decision.is_some() { g.act(0, Move::Choose(Answer::Yes)).unwrap(); }
    assert!(g.decision.is_none() && g.stack.is_empty());
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 21));
}

/// Copies taken at the holder question and at the replacement-order question each finish
/// with exactly one draw and their own counters (616.1: P1 orders P1's replacements).
#[test]
fn copies_at_each_question_draw_once_and_keep_their_own_counters() {
    let mut g = game();
    let bear = g.put(1, 2, Zone::Battlefield);
    g.put(1, 3, Zone::Battlefield);
    g.put(1, 4, Zone::Battlefield);
    g.put_counters(bear, "+1/+1", 1);
    g.put_counters(Target::Player(1), "poison", 1);
    let hand = g.players[0].hand.len();
    cast(&mut g, 1);
    cite!(["701.34a"]; assert_eq!(holders(&g), vec![Target::Player(1), Target::Object(bear)]));
    let mut none = g.clone();
    let mut poison = g.clone();
    none.act(0, Move::Choose(Answer::Holders(vec![]))).unwrap();
    poison.act(0, Move::Choose(Answer::Holders(vec![Target::Player(1)]))).unwrap();
    g.act(0, Move::Choose(Answer::Holders(vec![Target::Object(bear), Target::Player(1)]))).unwrap();
    let Some(Decision::Choose { player: 1, choice: Choice::Replacement { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    assert_eq!((g.counters(bear).get("+1/+1"), g.counters(Target::Player(1)).get("poison")), (Some(&1), Some(&1)), "nothing is put before the batch");
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), hand, "no draw before the proliferate is done"));
    let mut other = g.clone();
    g.act(1, Move::Choose(Answer::Replacement(among[0].clone()))).unwrap();
    other.act(1, Move::Choose(Answer::Replacement(among[1].clone()))).unwrap();
    let mut bears = Vec::new();
    for state in [&none, &poison, &g, &other] {
        assert!(state.decision.is_none() && state.stack.is_empty());
        assert_eq!(state.players[0].hand.len(), hand + 1, "exactly one draw");
        bears.push(*state.counters(bear).get("+1/+1").unwrap());
    }
    cite!(["701.34a"]; assert_eq!((none.counters(Target::Player(1)).get("poison"), poison.counters(Target::Player(1)).get("poison")), (Some(&1), Some(&2))));
    assert_eq!(&bears[..2], &[1, 1]);
    let mut ordered = vec![bears[2], bears[3]];
    ordered.sort();
    cite!(["616.1", "616.1e"]; assert_eq!(ordered, vec![4, 5], "1 + (1*2+1) or 1 + (1+1)*2"));
}

#[test]
fn repeated_and_several_player_forms_stay_refused() {
    for text in ["Proliferate twice.", "Proliferate three times.", "Proliferate X times.", "Each player proliferates.",
                 "Each opponent proliferates.", "For each creature you control, proliferate."] {
        let d = CardDef::new("Form", "{0}", "Sorcery", text, None).unwrap();
        let reason=if text.starts_with("Proliferate") {"modifier: Times of Keyword"}
            else if text.starts_with("Each") {"proliferate: one instructed player"}
            else {"for-each quantity lead: compound, optional or non-quantity action"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
