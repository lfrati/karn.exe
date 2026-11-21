//! Closed lists from the Comprehensive Rules, folded for matching against Oracle text. The lists
//! live in the `ir` crate (`crates/ir/data/cr_lists.json`, written by `scripts/inventory.py`).

use std::sync::OnceLock;

/// One list by its key in `cr_lists.json` ("ability words", "keyword abilities (702)", …).
pub use ir::vocab::list;

/// The CR prints typographic apostrophes ("council’s dilemma"); Oracle text prints straight ones.
pub fn fold(s: &str) -> String {
    s.replace('’', "'").to_lowercase()
}

/// CR 207.2c ability words, folded.
pub fn ability_words() -> &'static [String] {
    static W: OnceLock<Vec<String>> = OnceLock::new();
    W.get_or_init(|| list("ability words").iter().map(|w| fold(w)).collect())
}

/// CR 702 keyword abilities, folded.
pub fn keyword_abilities() -> &'static [String] {
    static W: OnceLock<Vec<String>> = OnceLock::new();
    W.get_or_init(|| list("keyword abilities (702)").iter().map(|w| fold(w)).collect())
}

/// CR 701 keyword actions, folded.
pub fn keyword_actions() -> &'static [String] {
    static W: OnceLock<Vec<String>> = OnceLock::new();
    W.get_or_init(|| list("keyword actions (701)").iter().map(|w| fold(w)).collect())
}

/// Every CR 205.3 subtype and 205.2 card type/supertype word, folded (a word that names a type
/// is never a shortened card name).
pub fn subtypes() -> &'static [String] {
    static W: OnceLock<Vec<String>> = OnceLock::new();
    W.get_or_init(|| {
        [
            "card types",
            "supertypes",
            "artifact types",
            "enchantment types",
            "land types",
            "planeswalker types",
            "spell types",
            "creature types",
            "battle types",
        ]
        .iter()
        .flat_map(|k| list(k).iter().map(|w| fold(w)))
        .collect()
    })
}

/// CR 205.3j planeswalker types, folded. A planeswalker's type is its name's first word, so the
/// word is both a type and a way to say the card.
pub fn planeswalker_types() -> &'static [String] {
    static W: OnceLock<Vec<String>> = OnceLock::new();
    W.get_or_init(|| list("planeswalker types").iter().map(|w| fold(w)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_load() {
        assert!(ability_words().iter().any(|w| w == "council's dilemma"));
        assert!(keyword_abilities().iter().any(|w| w == "flying"));
        assert!(keyword_abilities().iter().any(|w| w == "max speed"));
        assert!(keyword_actions().iter().any(|w| w == "scry"));
    }
}
