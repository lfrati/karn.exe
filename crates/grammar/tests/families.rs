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
