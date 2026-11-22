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
    for text in ["For each damage, draw a card.", "For each card types, draw a card.", "For each power, draw a card."] {
        declines(text);
    }
}

/// CR 606.3: timing permission remains scoped to the stated planeswalker's loyalty abilities.
#[test]
fn r606_3_scoped_activation_timing() {
    for text in [
        "As long as ~ entered this turn, you may activate her loyalty abilities any time you could cast an instant.",
        "You may activate loyalty abilities of target planeswalker any time you could cast an instant.",
        "You may activate its activated abilities any time you could cast an instant.",
    ] {
        let (value, _) = read(text);
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("ActivateAtInstantSpeed"));
        assert!(!json.contains("LoyaltyAtInstantSpeed"));
    }
    for text in ["You may activate her triggered abilities any time you could cast an instant.", "You may activate loyalty spells of ~ any time you could cast an instant."] {
        declines(text);
    }
}

/// CR 107.4b: generic mana is a cost component, not a color or type of produced mana.
#[test]
fn r107_4b_generic_mana_spending() {
    let (value, _) = read("{T}: Add {W}{U}{B}{R}{G}. This mana can't be spent to pay generic mana costs.");
    assert!(serde_json::to_string(&value).unwrap().contains("Generic"));
    for text in ["This mana can't be spent to pay generic mana creatures.", "This mana can't be spent to cast generic mana costs."] {
        declines(text);
    }
}
