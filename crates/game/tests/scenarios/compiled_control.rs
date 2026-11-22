//! Conditional programs test once per block and retain target/slot invariants (608.2c).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use game::program::{self, ActSpan, Audience, Instr, Program, Who};
use game::select::Ctx;

fn card(text: &str) -> CardDef { CardDef::new("Conditional", "{0}", "Sorcery", text, None).unwrap() }
fn bear() -> CardDef { CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap() }
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r608_2c_a_compound_condition_is_checked_once_before_all_its_instructions() {
    for (life, gain, drawn) in [(20, 3, 1), (21, 0, 0)] {
        let d = card("If you have 20 or less life, you gain 3 life and draw a card. You gain 1 life.");
        game::play::check(&d).unwrap();
        let mut g = Game::new(vec![d, bear()], 2, 20);
        g.players[0].life = life;
        g.put(0, 1, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["608.2c"]; case: format!("{life}"); assert_eq!(g.players[0].life, life + gain + 1));
        cite!(["608.2c"]; case: format!("{life}"); assert_eq!(g.players[0].hand.len(), drawn));
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r601_2c_conditional_targets_are_announced_even_when_the_guard_is_false() {
    let d = card("If you have 10 or less life, tap target creature. You gain 1 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): g.act(0, Move::Cast { card: spell, targets: vec![] }));
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] }).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert!(!g.obj(target).tapped));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn skipped_producers_cannot_supply_uninitialized_values_after_a_branch() {
    let d = card("If you have 10 or less life, draw a card.");
    let ir::Ability::Text(effect) = &d.abilities()[0] else { panic!() };
    let mut g = Game::new(vec![d.clone()], 2, 20);
    let source = g.put(0, 0, Zone::Stack);
    let compiled = program::compile(effect, &g, &Ctx::new(0, source)).unwrap();
    let Instr::Branch { condition, .. } = &compiled.instrs()[0] else { panic!() };
    let malformed = Program::new(vec![
        Instr::Branch { condition: condition.clone(), end: 2, remember: None, skipped_values: Vec::new() },
        Instr::TakeTop { n: 1.into(), whose: game::program::action::Operand::Controller, out: 0 },
        Instr::Show { from: 0, by: Who::Controller, to: Audience::Everyone, out: 1 },
    ], vec![ActSpan { end: 1, moves: false, keeps_x: false }, ActSpan { end: 2, moves: false, keeps_x: false }, ActSpan { end: 3, moves: false, keeps_x: false }]);
    assert_eq!(malformed.unwrap_err().0, "slot 0 is read before any instruction produces it");
}

#[test]
fn r608_2d_local_choices_bind_into_conditions_and_ordinary_action_sets() {
    use game::state::Answer;
    use game::program::Value;
    for text in [
        "Choose a color. If you control a creature of the chosen color, you gain 3 life.",
        "Choose a color. Destroy all creatures of the chosen color.",
    ] {
        let d = card(text);
        game::play::check(&d).unwrap();
        let red = CardDef::new("Red", "{R}", "Creature — Goblin", "", Some((1, 1))).unwrap();
        for chosen in [ir::Color::Red, ir::Color::Blue] {
            let mut g = Game::new(vec![d.clone(), red.clone()], 2, 20);
            let object = g.put(0, 1, Zone::Battlefield);
            let spell = g.put(0, 0, Zone::Hand);
            g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
            resolve(&mut g);
            g.act(0, Move::Choose(Answer::Value(Value::Color(chosen)))).unwrap();
            let matches = chosen == ir::Color::Red;
            if text.contains("Destroy") {
                cite!(["608.2d"]; case: format!("{chosen:?}"); assert_eq!(g.obj(object).zone == Zone::Gone, matches));
            } else {
                cite!(["608.2d"]; case: format!("{chosen:?}"); assert_eq!(g.players[0].life, if matches { 23 } else { 20 }));
            }
            assert!(g.resolving().is_none());
        }
    }
}

#[test]
fn r608_2c_otherwise_uses_the_original_test_even_after_the_true_branch_changes_it() {
    let d = card("If you have 20 or less life, you gain 3 life. Otherwise, draw a card.");
    game::play::check(&d).unwrap();
    for (life, after, drawn) in [(20, 23, 0), (21, 21, 1)] {
        let mut g = Game::new(vec![d.clone(), bear()], 2, 20);
        g.players[0].life = life;
        g.put(0, 1, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["608.2c"]; case: format!("{life}"); assert_eq!(g.players[0].life, after));
        cite!(["608.2c"]; case: format!("{life}"); assert_eq!(g.players[0].hand.len(), drawn));
        assert!(g.resolving().is_none());
    }
}

#[test]
fn r118_12_a_partial_action_cannot_pay_the_cost_of_an_outcome_branch() {
    for text in [
        "Discard two cards. If you do, draw three cards.",
        "You may discard two cards. If you do, draw three cards.",
    ] {
        let d = card(text);
        game::play::check(&d).unwrap();
        let mut g = Game::new(vec![d, bear()], 2, 20);
        let held = g.put(0, 1, Zone::Hand);
        for _ in 0..3 { g.put(0, 1, Zone::Library); }
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        resolve(&mut g);
        assert!(g.decision.is_none() && g.resolving().is_none());
        cite!(["118.12", "118.3"]; case: text; assert_eq!(g.players[0].hand, vec![held], "an impossible cost is not paid partially"));
        cite!(["118.12"]; case: text; assert_eq!(g.players[0].library.len(), 3));
    }
}

#[test]
fn r118_12_an_optional_payment_resumes_between_consent_and_selection_without_repeating_either() {
    use game::state::{Answer, Choice, Decision};
    let d = card("You gain 2 life. You may discard a card. If you do, draw a card. Otherwise, you lose 1 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, bear()], 2, 20);
    let one = g.put(0, 1, Zone::Hand);
    let two = g.put(0, 1, Zone::Hand);
    g.put(0, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["118.12"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })));
    let mut declined = g.clone();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!(declined.players[0].life, 21));
    assert_eq!(declined.players[0].hand, vec![one, two]);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Objects { .. }, .. })));
    assert_eq!(g.players[0].life, 22);
    let mut cloned = g.clone();
    for (game, discarded) in [(&mut g, one), (&mut cloned, two)] {
        game.act(0, Move::Choose(Answer::Objects(vec![discarded]))).unwrap();
        assert_eq!(game.players[0].life, 22);
        assert_eq!(game.players[0].hand.len(), 2);
        cite!(["118.12"]; case: format!("{discarded:?}"); assert_eq!(game.players[0].library.len(), 0));
        assert!(game.decision.is_none() && game.resolving().is_none());
    }
}
