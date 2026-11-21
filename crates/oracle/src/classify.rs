//! Rows → ability blocks with a coarse kind.
//!
//! A block is one ability as printed: a header row plus its continuation rows (modes "• …",
//! Spree "+ {1} — …", pawprint "{P} — …", die-roll table rows "1—9 | …"). The kind only says which
//! sub-grammar reads the block; it is decided from row shape, never from meaning, and anything the
//! shapes do not cover is [`Kind::Text`] (static ability or spell text) for the grammar to judge.
//!
//! Sections that change which abilities a permanent has are recorded per block: a Class's level
//! (CR 716), a leveler's band (711) and a Spacecraft's station threshold (702.184).

use crate::cr;
use crate::normalize::Normalized;
use crate::token::{Kind as T, Token, tokenize};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Nothing left after normalization (the row was reminder text only).
    Empty,
    /// Comma/semicolon-separated keyword abilities: "Flying, vigilance", "Ward {2}", "Equip {1}",
    /// "Protection from red", "Ward—Pay 3 life.".
    Keywords,
    /// "Keyword — ability": Max speed, Solved, ∞, Boast, Exhaust, Forecast, Power-up, Companion …
    /// (CR 702); and a Case's "To solve — condition" (719.3a).
    KeywordPrefixed,
    /// "When…/Whenever…/At…, effect" (CR 603.1).
    Triggered,
    /// "cost: effect" (602.1).
    Activated,
    /// "+1: …", "−2: …", "0: …", "[+1]: …" (606).
    Loyalty,
    /// "I, II — …" (714).
    Chapter,
    /// "{cost}: Level N" on a Class (716.2a).
    ClassLevel,
    /// "LEVEL 1-3" band header on a leveler (711.2); the next row is its P/T.
    LevelBand,
    /// The power/toughness row under a LEVEL band ("3/3").
    LevelPt,
    /// "9+ | Flying" on a Spacecraft (702.184).
    StationThreshold,
    /// Everything else: static abilities and spell text.
    Text,
}

/// Which section of a sectioned card a block belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// Abilities the object always has.
    Base,
    /// A Class's abilities from level N on.
    ClassLevel(u32),
    /// A leveler's band, from the band's first level.
    LevelBand(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    /// The rows this ability spans: the header and its continuation rows.
    pub rows: Range<usize>,
    pub section: Section,
}

/// Is this row a continuation of the ability above it? `station`: the face has the Station
/// keyword, so "9+ | …" rows are thresholds (702.184) rather than die-roll results (706.2).
fn is_continuation(tokens: &[Token], station: bool) -> bool {
    match tokens.first().map(|t| &t.kind) {
        Some(T::Punct('•')) => true,
        // Spree: "+ {2}{B} — …"
        Some(T::Punct('+')) => matches!(tokens.get(1).map(|t| &t.kind), Some(T::Symbol(_))),
        // pawprint modes: "{P} — …", "{P}{P} — …"
        Some(T::Symbol(s)) if s == "P" => true,
        // die-roll rows: "1—9 | …", "20 | …", "15+ | …"
        Some(T::Number { .. }) => !station && tokens.iter().any(|t| t.kind == T::Punct('|')),
        _ => false,
    }
}

pub fn classify(n: &Normalized) -> Vec<Block> {
    let tokens: Vec<Vec<Token>> = n.rows.iter().map(|r| tokenize(r)).collect();
    let mut blocks: Vec<Block> = Vec::new();
    let station = tokens.iter().any(|t| t.first().and_then(Token::word) == Some("Station"));
    let mut section = Section::Base;
    for (i, toks) in tokens.iter().enumerate() {
        if is_continuation(toks, station)
            // a label that starts the row makes it a new ability; one after the row's own marker
            // (a bullet, a die-roll range) does not
            && n.labels[i].as_ref().is_none_or(|l| !l.after.is_empty())
            && let Some(last) = blocks.last_mut()
            && last.rows.end == i
            && last.kind != Kind::Empty
        {
            last.rows.end = i + 1;
            continue;
        }
        let kind = row_kind(&n.rows[i], toks);
        match kind {
            Kind::ClassLevel => section = Section::ClassLevel(level_number(toks).unwrap_or(0)),
            Kind::LevelBand => section = Section::LevelBand(level_number(toks).unwrap_or(0)),
            _ => {}
        }
        blocks.push(Block { kind, rows: i..i + 1, section });
    }
    blocks
}

/// The cost of an activated, loyalty or Class-level ability, or of a "Keyword — cost: effect" row
/// (Boast, Exhaust, Forecast …): the row's kind and the token range before the colon. Works on any
/// token row, including the body of a granted ability in quotes.
pub fn cost_span(toks: &[Token]) -> Option<(Kind, Range<usize>)> {
    let kind = row_kind(&crate::token::detokenize(toks), toks);
    let c = colon(toks)?;
    let start = match kind {
        Kind::Activated | Kind::Loyalty | Kind::ClassLevel => 0,
        Kind::KeywordPrefixed => toks[..c].iter().position(|t| t.kind == T::Punct('—'))? + 1,
        _ => return None,
    };
    (start < c).then_some((kind, start..c))
}

/// The effect of an ability row: after the trigger condition's comma (triggered), the colon
/// (activated, loyalty, "Keyword — cost:"), or the dash (chapters, "Keyword — effect"); a mode
/// row's text after its bullet or Spree/pawprint dash. `spell`: Text rows count too (the face is
/// an instant or sorcery). Works on any token row, including quoted abilities.
pub fn effect_span(toks: &[Token], spell: bool) -> Option<(Kind, Range<usize>)> {
    let kind = row_kind(&crate::token::detokenize(toks), toks);
    let after = |p: T| toks.iter().position(|t| t.kind == p).map(|i| i + 1);
    let start = match toks.first().map(|t| &t.kind) {
        Some(T::Punct('•')) => Some(1),
        Some(T::Punct('+')) | Some(T::Symbol(_)) if kind != Kind::Activated => after(T::Punct('—')),
        _ => match kind {
            Kind::Triggered => after(T::Punct(',')),
            Kind::Activated | Kind::Loyalty => after(T::Punct(':')),
            Kind::KeywordPrefixed => after(T::Punct(':')).or_else(|| after(T::Punct('—'))),
            Kind::Chapter => after(T::Punct('—')),
            Kind::Text if spell => Some(0),
            _ => None,
        },
    }?;
    (start < toks.len()).then_some((kind, start..toks.len()))
}

fn level_number(toks: &[Token]) -> Option<u32> {
    let at = toks.iter().position(|t| t.word().is_some_and(|w| w.eq_ignore_ascii_case("level")))?;
    toks[at + 1..].iter().find_map(|t| match t.kind {
        T::Number { value, .. } => Some(value),
        _ => None,
    })
}

/// The index of the first top-level `:` (quoted abilities are single tokens, so a colon inside
/// one never counts).
fn colon(toks: &[Token]) -> Option<usize> {
    toks.iter().position(|t| t.kind == T::Punct(':'))
}

fn row_kind(row: &str, toks: &[Token]) -> Kind {
    let words: Vec<&str> = toks.iter().take(3).filter_map(Token::word).collect();
    if toks.is_empty() {
        return Kind::Empty;
    }
    // "LEVEL 1-3", "LEVEL 4+"
    if words.first() == Some(&"LEVEL") {
        return Kind::LevelBand;
    }
    // "3/3" under a band
    if let [
        Token { kind: T::Number { .. } | T::Punct('*'), .. },
        Token { kind: T::Punct('/'), .. },
        Token { kind: T::Number { .. } | T::Punct('*'), .. },
    ] = toks
    {
        return Kind::LevelPt;
    }
    // "9+ | Flying"
    if let [
        Token { kind: T::Number { .. }, .. },
        Token { kind: T::Punct('+'), .. },
        Token { kind: T::Punct('|'), .. },
        ..,
    ] = toks
    {
        return Kind::StationThreshold;
    }
    // Saga chapters: roman numerals and commas, then the dash
    if let Some(dash) = toks.iter().position(|t| t.kind == T::Punct('—')) {
        let numerals = toks[..dash].iter().all(|t| {
            t.kind == T::Punct(',') || t.word().is_some_and(|w| w.chars().all(|c| matches!(c, 'I' | 'V' | 'X')))
        });
        if dash > 0 && numerals {
            return Kind::Chapter;
        }
    }
    if let Some(c) = colon(toks) {
        let cost = &toks[..c];
        // loyalty: [+−]?(N|X) or the bracketed form "[+1]"
        let loyalty = |cost: &[Token]| match cost {
            [Token { kind: T::Punct('+' | '−' | '-'), .. }, Token { kind: T::Number { .. }, .. }]
            | [Token { kind: T::Number { .. }, .. }] => true,
            [Token { kind: T::Punct('+' | '−' | '-'), .. }, t] | [t] => t.word() == Some("X"),
            _ => false,
        };
        let bracketed = matches!(cost, [Token { kind: T::Punct('['), .. }, .., Token { kind: T::Punct(']'), .. }])
            && loyalty(&cost[1..cost.len() - 1]);
        if loyalty(cost) || bracketed {
            return Kind::Loyalty;
        }
        // "{3}{U}: Level 2"
        let rest: Vec<&str> = toks[c + 1..].iter().filter_map(Token::word).collect();
        if rest.first() == Some(&"Level") && toks.len() <= c + 3 {
            return Kind::ClassLevel;
        }
    }
    // "Keyword — …" and "To solve — …"
    if let Some(at) = row.find(" — ") {
        let folded = cr::fold(&row[..at]);
        if folded == "to solve" || cr::keyword_abilities().contains(&folded) || folded == "∞" {
            return Kind::KeywordPrefixed;
        }
    }
    if matches!(words.first(), Some(&("When" | "Whenever" | "At"))) {
        return Kind::Triggered;
    }
    if let Some(c) = colon(toks) {
        // a cost precedes the colon; statics with colons ("As ~ enters, …") never start this way
        if c > 0 && !matches!(words.first(), Some(&("As" | "If" | "Choose"))) {
            return Kind::Activated;
        }
    }
    if is_keyword_line(toks) {
        return Kind::Keywords;
    }
    Kind::Text
}

/// Every comma/semicolon-separated part starts with a CR 702 keyword (or a landwalk/typecycling
/// variant), and the row is not a sentence: it has no final period unless a keyword cost written
/// with a dash ("Ward—Pay 3 life.") precedes it.
fn is_keyword_line(toks: &[Token]) -> bool {
    let has_dash = toks.iter().any(|t| t.kind == T::Punct('—'));
    if toks.last().is_some_and(|t| t.kind == T::Punct('.')) && !has_dash {
        return false;
    }
    let mut parts: Vec<&[Token]> = Vec::new();
    let mut start = 0;
    for (i, t) in toks.iter().enumerate() {
        if matches!(t.kind, T::Punct(',' | ';')) {
            parts.push(&toks[start..i]);
            start = i + 1;
        }
    }
    parts.push(&toks[start..]);
    parts.iter().all(|p| starts_with_keyword(p))
}

fn starts_with_keyword(part: &[Token]) -> bool {
    let words: Vec<String> = part.iter().map_while(|t| t.word().map(str::to_lowercase)).collect();
    if words.is_empty() {
        return false;
    }
    let first = &words[0];
    // landwalk ("swampwalk", "nonbasic landwalk") and typecycling ("plainscycling",
    // "basic landcycling") are written inside their parent rules (702.14, 702.29)
    if first.ends_with("walk")
        || first.ends_with("cycling")
        || words.get(1).is_some_and(|w| w.ends_with("walk") || w.ends_with("cycling"))
    {
        return true;
    }
    cr::keyword_abilities().iter().any(|k| {
        let kw: Vec<&str> = k.split(' ').collect();
        kw.len() <= words.len() && kw.iter().zip(&words).all(|(a, b)| *a == b.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::normalize;

    fn kinds(text: &str) -> Vec<(Kind, usize)> {
        classify(&normalize(text, "Test Card", "Test Card", "Creature"))
            .into_iter()
            .map(|b| (b.kind, b.rows.len()))
            .collect()
    }

    #[test]
    fn basic_kinds() {
        assert_eq!(
            kinds(
                "Flying, first strike\nWhen ~ enters, draw a card.\n{2}, {T}: Add {C}.\n~ can't block.\n−3: Destroy target creature."
            ),
            vec![(Kind::Keywords, 1), (Kind::Triggered, 1), (Kind::Activated, 1), (Kind::Text, 1), (Kind::Loyalty, 1)]
        );
    }

    #[test]
    fn modes_group() {
        assert_eq!(
            kinds("Choose one —\n• Destroy target artifact.\n• Draw a card.\nFlash"),
            vec![(Kind::Text, 3), (Kind::Keywords, 1)]
        );
        assert_eq!(
            kinds(
                "Spree (Choose one or more additional costs.)\n+ {2}{B} — Destroy target creature.\n+ {1} — Draw a card."
            ),
            vec![(Kind::Keywords, 3)]
        );
    }

    #[test]
    fn sections() {
        let n = normalize(
            "When this Class enters, draw a card.\n{3}{U}: Level 2\nWhenever you cast a spell, scry 1.",
            "X",
            "X",
            "Enchantment — Class",
        );
        let b = classify(&n);
        assert_eq!(b[1].kind, Kind::ClassLevel);
        assert_eq!(b[2].section, Section::ClassLevel(2));
    }

    #[test]
    fn die_rows_vs_station() {
        assert_eq!(kinds("Roll a d20.\n1—14 | Draw a card.\n15+ | Draw two cards."), vec![(Kind::Text, 3)]);
        assert_eq!(
            kinds("Station (Tap another creature you control.)\n9+ | Flying"),
            vec![(Kind::Keywords, 1), (Kind::StationThreshold, 1)]
        );
    }

    #[test]
    fn keyword_prefixed_and_chapters() {
        assert_eq!(
            kinds("I, II — Draw a card.\nMax speed — {T}: Add {R}.\nWard—Pay 3 life."),
            vec![(Kind::Chapter, 1), (Kind::KeywordPrefixed, 1), (Kind::Keywords, 1)]
        );
    }
}

/// The sentences of an effect: split after "." and "!", and after a quoted ability whose own
/// last sentence ends the outer one (`… with "{T}: Add {G}." It's still a land.`). The
/// punctuation is dropped; a quote stays with its sentence.
pub fn sentences(toks: &[Token]) -> Vec<&[Token]> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, t) in toks.iter().enumerate() {
        let end = match &t.kind {
            T::Punct('.' | '!') => Some(i),
            T::Quote { body, closed: true, .. }
                if matches!(body.last().map(|b| &b.kind), Some(T::Punct('.' | '!'))) && i + 1 < toks.len() =>
            {
                Some(i + 1)
            }
            _ => None,
        };
        if let Some(e) = end {
            if e > start {
                out.push(&toks[start..e]);
            }
            start = if e == i { i + 1 } else { e };
        }
    }
    if start < toks.len() {
        out.push(&toks[start..]);
    }
    out
}

#[cfg(test)]
mod sentence_tests {
    use super::*;

    #[test]
    fn splits_after_a_quote_that_ends_a_sentence() {
        let toks = tokenize("~ becomes a 2/2 creature with \"{T}: Add {G}.\" It's still a land.");
        let s = sentences(&toks);
        assert_eq!(s.len(), 2);
        assert_eq!(crate::token::detokenize(s[1]).trim(), "It's still a land");
        let toks = tokenize("It gains \"{T}: Add {G}.\"");
        assert_eq!(sentences(&toks).len(), 1);
    }
}
