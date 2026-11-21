//! What the grammar reads of effects but the engine does not run stays refused, and the refusal
//! names what is not run.
use game::CardDef;

fn refusal(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> String {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    match game::play::check(&d) {
        Ok(()) => panic!("{name} should be refused"),
        Err(u) => u.0,
    }
}

#[test]
fn r614_1c_an_act_done_as_this_permanent_enters_is_refused() {
    let why = refusal(
        "Brimstone Vandal",
        "{2}{R}",
        "Creature — Devil",
        "If it's neither day nor night, it becomes day as this creature enters.",
        Some((2, 3)),
    );
    assert!(why.contains("as this permanent enters"), "{why}");
}

#[test]
fn r701_55a_a_villainous_choice_is_refused() {
    let why = refusal(
        "Choice",
        "{2}{B}",
        "Sorcery",
        "Target opponent faces a villainous choice — You draw a card, or that player discards a card.",
        None,
    );
    assert!(why.contains("FaceVillainousChoice"), "{why}");
}

#[test]
fn r700_2e_modes_chosen_by_another_player_are_refused() {
    let why = refusal(
        "Misfortune",
        "{1}{B}{R}{G}",
        "Sorcery",
        "An opponent chooses one —\n• You put a +1/+1 counter on each creature you control and gain 4 life.\n• You put a -1/-1 counter on each creature that player controls and Misfortune deals 4 damage to that player.",
        None,
    );
    assert!(why.contains("700.2e"), "{why}");
    let why = refusal("Lore", "{2}{B}{B}", "Sorcery", "An opponent chooses one —\n• You draw three cards.\n• You gain 3 life.", None);
    assert!(why.contains("700.2e"), "{why}");
    // the controller's own choice of the same modes is not refused for who chooses
    let d = CardDef::new("Lore", "{2}{B}{B}", "Sorcery", "Choose one —\n• You draw three cards.\n• You gain 3 life.", None).expect("Lore");
    if let Err(u) = game::play::check(&d) {
        assert!(!u.0.contains("700.2e"), "{}", u.0);
    }
}
