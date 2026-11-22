//! Oracle phrasing families: typed readings, canonical printing, and nearby refusals.
use grammar::{ability::abilities, kernel::Outcome};
use oracle::token::tokenize;

fn read(text: &str) -> (ir::Ability, String) {
    let rule = &abilities().ability;
    let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize(text)) else {
        panic!("{text}: {:?}", rule.parse_all(&tokenize(text)));
    };
    let printed = rule.print_text(&value, true).unwrap_or_else(|| panic!("{text}: no printing for {value:#?}"));
    let Outcome::Accepted { value: again, .. } = rule.parse_all(&tokenize(&printed)) else {
        panic!("{text}: printing does not read: {printed}");
    };
    assert_eq!(value, again, "{text}: {printed}");
    (value, printed)
}
fn declines(text: &str) {
    assert!(matches!(abilities().ability.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text}");
}

/// CR 700.3b: membership describes each object; the pile is never a permanent or player.
#[test]
fn r700_3b_pile_membership() {
    for text in [
        "−6: Separate all permanents target player controls into two piles. That player sacrifices all permanents in the pile of their choice.",
        "Destroy all creatures in the pile of your choice.",
        "Exile all cards in that pile.",
        "Return all cards in the other pile to your hand.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("InPile"));
    }
    for text in ["Destroy target pile.", "Target player in that pile draws a card.", "Sacrifice all permanents in the pile of choice."] {
        declines(text);
    }
}

/// CR 205.2a and 608.2c: iterate card types, retaining which type each instruction names.
#[test]
fn r205_2a_iterate_value_classes() {
    for text in [
        "When ~ enters, reveal the top ten cards of your library. For each card type, you may put a card of that type from among the revealed cards into your hand. Put the rest on the bottom of your library in a random order.",
        "Reveal the top X cards of your library. For each card type, you may exile a card of that type from among them. Put the rest into your graveyard.",
        "For each color, add one mana of that color.",
        "For each basic land type, choose a land of the chosen type.",
    ] {
        let (value, _) = read(text);
        assert!(serde_json::to_string(&value).unwrap().contains("ForEachValue"));
    }
    for text in ["For each card, draw a card.", "For each card types, draw a card.", "For each power, draw a card."] {
        declines(text);
    }
}
