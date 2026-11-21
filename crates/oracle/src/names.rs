//! The closed set of card names (CR 201.1).
//!
//! A name is not free text: text that mentions another card ("a card named Rowan, Fearless
//! Sparkmage", "Partner with Sam, Loyal Attendant") says one of a fixed set of strings, and a name
//! may hold a comma, an apostrophe or a lowercase word that a run of capitalized words cannot
//! spell. The set is baked into `data/card_names.json` by `scripts/card_names.py`, with the hash
//! of the corpus it came from, and checked against the corpus whenever the corpus is present
//! (the test below), so it cannot go stale silently.
//!
//! Two readers use it. [`crate::normalize`] does not take a card's own name for a reference when
//! it sits inside a longer name (CR 201.5: only text that names the object itself is a
//! self-reference). The grammar reads a name in a name position (`named …`, `Partner with …`) as
//! the longest known name starting there ([`longest`]), or else as a run of capitalized words.

use crate::token::{Token, detokenize};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Baked {
    names: Vec<String>,
    types: Vec<String>,
}

fn baked() -> &'static HashMap<String, String> {
    static BAKED: OnceLock<HashMap<String, String>> = OnceLock::new();
    BAKED.get_or_init(|| {
        let baked: Baked =
            serde_json::from_str(include_str!("../data/card_names.json")).expect("data/card_names.json is valid");
        assert_eq!(baked.names.len(), baked.types.len(), "data/card_names.json has types for each name");
        baked.names.into_iter().zip(baked.types).collect()
    })
}

/// Every card name in the corpus.
pub fn all() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| baked().keys().cloned().collect())
}

/// The types of the face a name names: its type line before the dash ("Legendary Creature"),
/// one for each face of that name whose types differ. A choice of a card name with certain
/// characteristics reads these (201.4a–f).
pub fn types(name: &str) -> impl Iterator<Item = &'static str> {
    baked().get(name).into_iter().flat_map(|t| t.split('|'))
}

pub fn is_name(s: &str) -> bool {
    all().contains(s)
}

/// The most tokens a name spans (checked by the test below against the corpus).
const MAX_TOKENS: usize = 24;

/// How many tokens the longest card name starting at `tokens[0]` spans, when one does. Spacing
/// counts as printed; a name never starts or ends inside a word.
pub fn longest(tokens: &[Token]) -> Option<usize> {
    let upto = tokens.len().min(MAX_TOKENS);
    (1..=upto).rev().find(|&n| is_name(&detokenize(&tokens[..n]).trim_start()))
}

/// The card names that cover byte `at..at + len` of `text` and are longer than that span.
pub fn inside_longer_name(text: &str, at: usize, len: usize) -> bool {
    // candidate starts and ends fall on word edges, at most MAX_TOKENS words away
    let starts: Vec<usize> = std::iter::once(at)
        .chain(text[..at].char_indices().rev().filter(|&(i, c)| c == ' ' && i + 1 < at).map(|(i, _)| i + 1))
        .take(MAX_TOKENS)
        .collect();
    let ends: Vec<usize> = std::iter::once(at + len)
        .chain(text[at + len..].char_indices().filter(|&(_, c)| c == ' ').map(|(i, _)| at + len + i))
        .chain(std::iter::once(text.len()))
        .take(MAX_TOKENS + 1)
        .collect();
    let edge = |i: usize| i == text.len() || !text[i..].starts_with(|c: char| c.is_alphanumeric());
    starts.iter().any(|&s| {
        ends.iter().any(|&e| {
            if !(text.is_char_boundary(s) && text.is_char_boundary(e) && edge(e)) {
                return false;
            }
            let name = trim_punct(&text[s..e]);
            name.len() > len && is_name(name)
        })
    })
}

/// A name inside a sentence is followed by its punctuation; the comma or period is not part of it.
fn trim_punct(s: &str) -> &str {
    s.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ':'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{corpus, token::tokenize};

    #[test]
    fn longest_known_name() {
        let t = tokenize("Frodo, Adventurous Hobbit");
        assert_eq!(longest(&t), Some(t.len()));
        let t = tokenize("Dovin, Architect of Law, reveal it");
        assert_eq!(longest(&t), Some(5));
        assert_eq!(longest(&tokenize("Frodo, and then")), None);
        let t = tokenize("God-Pharaoh's Gift and put it");
        assert_eq!(longest(&t), Some(3));
    }

    #[test]
    fn inside_a_longer_name() {
        let text = "a creature named Silver Surfer, Galactus's Herald.";
        let at = text.find("Galactus").unwrap();
        assert!(inside_longer_name(text, at, "Galactus".len()));
        let text = "Galactus attacks.";
        assert!(!inside_longer_name(text, 0, "Galactus".len()));
    }

    /// The baked set is what the corpus gives (regenerate with `uv run python scripts/card_names.py`).
    #[test]
    fn baked_names_are_the_corpus_names() {
        let path = corpus::default_path();
        let Ok(faces) = corpus::load(&path) else { return };
        let mut fresh: HashSet<String> = HashSet::new();
        for f in &faces {
            fresh.insert(f.face_name.clone());
            if !f.name.contains(" // ") {
                fresh.insert(f.name.clone());
            }
        }
        assert_eq!(fresh.len(), all().len(), "card_names.json is stale: run scripts/card_names.py");
        assert!(fresh.iter().all(|n| is_name(n)), "card_names.json is stale: run scripts/card_names.py");
        let widest = fresh.iter().map(|n| tokenize(n).len()).max().unwrap();
        assert!(widest <= MAX_TOKENS, "a name spans {widest} tokens");
    }
}
