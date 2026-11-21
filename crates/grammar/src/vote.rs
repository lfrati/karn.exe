//! Votes (CR 701.38): the options players vote for, which are words with no rules meaning.

use crate::kernel::{P, alt, leaf, name_run};
use oracle::token::Kind;

/// A vote option: one lowercase word other than a color ("grace", "planeswalk") or a name ("Redhorn Pass").
pub fn option() -> P<String> {
    let word = leaf(
        "vote option",
        |k| match k {
            // color names vote as colors (`VoteFor::Colors`)
            Kind::Word(w)
                if w.chars().all(|c| c.is_ascii_lowercase())
                    && !matches!(w.as_str(), "or" | "and" | "the")
                    && !ir::Color::ALL.iter().any(|c| c.word() == w) =>
            {
                Some(w.clone())
            }
            _ => None,
        },
        |w: &String| (!w.contains(' ')).then(|| Kind::Word(w.clone())),
    );
    alt("vote option", vec![word, name_run()])
}

/// The words of a choice among words that have no rules meaning (CR 607.2f), such as anchor
/// words (614.12c): "Khans or Dragons", "odd or even", "money, friends, or secrets". Not a number
/// ("choose one or more"), a keyword, a card type, a color or a land type: those have a meaning
/// and are choices of their own kind.
pub fn words() -> P<Vec<String>> {
    word().list("or")
}

/// One word of such a choice, or the anchor word (614.12c) a mode row starts with.
pub fn word() -> P<String> {
    fn counts(w: &str) -> bool {
        static N: std::sync::OnceLock<P<u32>> = std::sync::OnceLock::new();
        let n = N.get_or_init(crate::words::number_words);
        matches!(n.parse_all(&oracle::token::tokenize(w)), crate::kernel::Outcome::Accepted { .. })
    }
    leaf(
        "choice word",
        |k| match k {
            Kind::Word(w)
                if w.chars().all(|c| c.is_ascii_alphabetic())
                    && !matches!(w.as_str(), "or" | "and" | "the" | "a" | "an" | "both" | "more" | "instead" | "any" | "up" | "X")
                    && !ir::Color::ALL.iter().any(|c| c.word() == w)
                    && ir::CardType::from_word(&w.to_ascii_lowercase()).is_none()
                    && ir::Keyword::from_name(w).is_none()
                    && !ir::Subtype::from_name(w).is_some_and(|s| s.kind() == ir::vocab::SubtypeKind::Land)
                    && !counts(&w.to_ascii_lowercase()) =>
            {
                Some(w.clone())
            }
            _ => None,
        },
        |w: &String| (!w.contains(' ')).then(|| Kind::Word(w.clone())),
    )
}
