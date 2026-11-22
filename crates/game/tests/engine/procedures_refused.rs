//! What the grammar reads but the engine does not yet run stays refused.
use crate::support::cite;
use game::CardDef;

fn checked(text: &str) -> Result<(), String> {
    let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&d).map_err(|u| u.0)
}

#[test]
fn r401_library_placements_accept_supported_choices_and_refuse_unbound_owners() {
    for (text, why) in [
        ("Look at the top three cards of your library. Put the rest on the bottom in a random order.", "library destination has no bound owner"),
    ] {
        crate::support::refused(checked(text), why);
    }
    for text in [
        // Execution, chooser and visibility expectations live in library_destinations.rs.
        "Put target creature on your choice of the top or bottom of its owner's library.",
        "Put target creature into its owner's library second from the top or on the bottom.",
        "Look at the top three cards of your library. Put them back in any order.",
        "Look at the top three cards of your library. Put one of them back on top of your library.",
        "Put target creature on top of its owner's library.",
        "Put target creature into its owner's library third from the top.",
        "Put target creature on the bottom of its owner's library.",
    ] {
        assert!(checked(text).is_ok(), "{text}");
    }
}

#[test]
fn r400_7_qualified_graveyard_moves_compile_and_the_command_zone_stays_refused() {
    // Uncited (admission): explicit graveyard origins qualify targets in either word order.
    for text in [
        "Put target creature card into your hand from your graveyard.",
        "Return target creature card from your graveyard to your hand.",
    ] {
        assert!(checked(text).is_ok(), "{text}");
    }
    crate::support::refused(checked("Return target creature card to the command zone."),
        "program: movement destination not lowered");
}

#[test]
fn r700_3_piles_are_refused() {
    for (text, why) in [
        ("Look at the top four cards of your library and separate them into two piles. An opponent chooses one of those piles. Put that pile into your hand and the other into your graveyard.", "program: action not lowered: ChoosePile"),
        ("Exile the top four cards of your library in a face-down pile. Shuffle that pile.", "modifier: InPile of Exile"),
    ] {
        crate::support::refused(checked(text), why);
    }
}

#[test]
fn r608_2c_repeating_the_process_is_refused() {
    for (text, why) in [("Draw a card. Repeat this process once.", "repeat this process: a number of times, any number or until is not lowered"), ("Mill a card. If you win the flip, repeat this process.", "coin outcome: no preceding local flip producer")] {
        crate::support::refused(checked(text), why);
    }
}

#[test]
fn r701_24_shuffling_a_named_library_is_refused_and_the_bare_shuffle_runs() {
    assert!(checked("Shuffle.").is_ok());
    crate::support::refused(checked("Shuffle your library."), "program: action not lowered: Shuffle");
    crate::support::refused(checked("Target player shuffles their library."), "program: action not lowered: Shuffle");
}

#[test]
fn r401_4_putting_looked_at_cards_back_preserves_the_chosen_private_order() {
    use game::{Game, Move, Zone};
    use game::state::Answer;
    let effect = CardDef::new("Procedure", "{0}", "Sorcery",
        "Look at the top three cards of your library. Put them back in any order.", None).unwrap();
    game::play::check(&effect).unwrap();
    let mut defs = vec![effect];
    for name in ["Untouched", "First", "Second", "Third"] {
        defs.push(CardDef::new(name, "", "Land", "", None).unwrap());
    }
    let mut g = Game::new(defs, 2, 20);
    let untouched = g.put(0, 1, Zone::Library);
    let a = g.put(0, 2, Zone::Library);
    let b = g.put(0, 3, Zone::Library);
    let c = g.put(0, 4, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    for o in [a, b, c] { assert!(g.sees(0, o)); assert!(!g.sees(1, o)); }
    cite!(["401.4"]; refused("order every offered card exactly once (401.4, 404.3)"): g.act(0, Move::Choose(Answer::Objects(vec![a, a, b]))));
    g.act(0, Move::Choose(Answer::Objects(vec![b, a, c]))).unwrap();
    assert_eq!(g.players[0].library[0], untouched);
    let order: Vec<_> = g.players[0].library.iter().map(|id| g.obj(*id).card.unwrap()).collect();
    assert_eq!(order, vec![1, 4, 2, 3]);
    assert!(g.stack.is_empty());
    assert!(g.players[0].hand.is_empty());
    for &o in &g.players[0].library { assert!(!g.sees(0, o)); assert!(!g.sees(1, o)); }
}
