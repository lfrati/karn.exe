//! A delayed triggered ability triggers only once, the next time its trigger event occurs, unless
//! it has a stated duration such as "this turn" (603.7b). "When you next cast …" is once even
//! with "this turn" (the duration only bounds when it can happen). The engine follows delayed
//! triggers about named objects and explicit next event predicates.
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
