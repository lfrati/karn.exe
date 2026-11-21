//! What each unread row lacks, asked of the grammar itself.
//!
//! `cargo run --profile fast -p grammar --bin grammar-gaps -- OUT.jsonl [--card NAME]`
//!
//! The gates say where reading stopped. That is a word, and one word is the stop of many
//! different constructions. This asks what is missing there: for each row the grammar does not
//! read, it looks for the shortest span touching the stop that, taken out or replaced by a
//! phrase the grammar knows, lets the sentence read to its period. What it finds is a gap:
//!
//! - **removed**: the sentence reads without the span, so the span is a phrase the grammar has
//!   no place for where it stands (a modifier, a rider, a second clause);
//! - **stands for** a kind: the sentence reads with a known phrase of that kind in the span's
//!   place, so the span is a way of saying that kind of thing which the grammar lacks.
//!
//! The row is then read on with the edit in place, so a row with several gaps reports each,
//! up to [`MAX_GAPS`]. A quoted ability the grammar does not read is diagnosed on its own. A
//! stop no such edit explains is reported as it is, for reading by eye: the sentence's frame
//! itself is unknown.
//!
//! Each gap also carries a template: its words with the phrases the grammar does know inside
//! it abstracted ("among ⟨things⟩", "without paying ⟨thing⟩ 's mana cost"), so that gaps group
//! by construction and not by card. One JSON line per unread row; an edit is evidence of what
//! is missing, never a reading of the card.

#![forbid(unsafe_code)]

use grammar::ability::abilities;
use grammar::amount::amounts;
use grammar::kernel::{Outcome, P, Val};
use grammar::mana::mana_cost;
use grammar::selector::selectors;
use oracle::classify::{Kind as Row, classify};
use oracle::corpus;
use oracle::normalize::normalize;
use oracle::token::{Kind, Token, detokenize, tokenize};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::process::ExitCode;

/// How far before the stop a gap may begin: the grammar may have read the gap's first words
/// as the start of something else ("of any" of "of any color").
const BACK: usize = 4;
/// The longest span tried, in tokens.
const MAX_SPAN: usize = 12;
/// The most gaps reported for one row.
const MAX_GAPS: usize = 4;

/// Phrases the grammar reads, by the kind of thing each says: what a span is replaced by to ask
/// what the sentence wanted in its place. Several of a kind, as a slot may want one number,
/// person or sort.
const STAND_INS: &[(&str, &str)] = &[
    ("a thing", "it"),
    ("a thing", "that creature"),
    ("a thing", "a creature"),
    ("a thing", "target creature"),
    ("a thing", "that card"),
    ("a thing", "a card"),
    ("a thing", "that spell"),
    ("a thing", "~"),
    ("things", "them"),
    ("things", "those creatures"),
    ("things", "creatures"),
    ("things", "cards"),
    ("a player", "you"),
    ("a player", "that player"),
    ("a player", "target player"),
    ("a player", "each player"),
    ("whose", "your"),
    ("whose", "its"),
    ("whose", "their"),
    ("whose", "that player's"),
    ("a noun", "creature"),
    ("a noun", "creatures"),
    ("a noun", "card"),
    ("a noun", "cards"),
    ("a noun", "spell"),
    ("a noun", "permanent"),
    ("a noun", "color"),
    ("a subtype", "Goblin"),
    ("an adjective", "red"),
    ("an adjective", "tapped"),
    ("an adjective", "legendary"),
    ("an adjective", "attacking"),
    ("an amount", "X"),
    ("an amount", "two"),
    ("an amount", "2"),
    ("an amount", "that much"),
    ("an amount", "the number of creatures you control"),
    ("a condition", "you control a creature"),
    ("a condition", "it's a creature"),
    ("an event", "a creature dies"),
    ("an event", "you cast a spell"),
    ("an instruction", "draw a card"),
    ("an instruction", "you draw a card"),
    ("an instruction", "draws a card"),
    ("an instruction", "destroy it"),
    ("an instruction", "it gains flying"),
    ("a duration", "until end of turn"),
    ("a duration", "this turn"),
    ("a zone", "your graveyard"),
    ("a zone", "the battlefield"),
    ("a zone", "your hand"),
    ("a zone", "your library"),
    ("a keyword", "flying"),
    ("a counter", "a +1/+1 counter"),
    ("a cost", "{1}"),
    ("a quoted ability", "\"{T}: Add {G}.\""),
];

#[derive(Serialize, Clone)]
struct Gap {
    /// "removed", or the kind of phrase that stood in.
    edit: &'static str,
    /// The phrase that stood in, when one did.
    stand_in: Option<&'static str>,
    /// The words of the gap.
    span: String,
    /// The gap with the phrases the grammar knows inside it abstracted.
    template: String,
    /// The kinds of phrase the gap's own words read as, alone. One that reads is a phrase the
    /// grammar knows, standing where the grammar has no place for it; one that reads as
    /// nothing is a phrase the grammar does not know.
    reads_as: Vec<&'static str>,
    before: String,
    after: String,
    /// Inside a quoted ability of the row.
    quoted: bool,
}

/// A stop no edit explains.
#[derive(Serialize)]
struct Unexplained {
    read: String,
    at: String,
    rest: String,
    expected: Vec<String>,
}

#[derive(Serialize)]
struct RowGaps {
    oracle_id: String,
    card: String,
    face: String,
    row: String,
    mode: bool,
    gaps: Vec<Gap>,
    unexplained: Option<Unexplained>,
}

fn unspaced(t: &[Token]) -> String {
    let mut v = t.to_vec();
    if let Some(f) = v.first_mut() {
        f.space_before = 0;
    }
    detokenize(&v)
}

fn is_period(t: &Token) -> bool {
    matches!(t.kind, Kind::Punct('.'))
}

/// Where the rule stops reading these tokens, and what it expected there; None when it reads
/// them (an ambiguous row is read, more than once).
fn stop<T: Val>(rule: &P<T>, toks: &[Token]) -> Option<(usize, Vec<String>)> {
    match rule.parse_all(toks) {
        Outcome::Declined { at, expected, .. } => Some((at.min(toks.len()), expected)),
        _ => None,
    }
}

/// The sentence around token `at`: from after the period before it to its own period (or the
/// row's end). A stop at a period is in the sentence that period ends.
fn sentence(toks: &[Token], at: usize) -> (usize, usize) {
    let start = toks[..at].iter().rposition(is_period).map_or(0, |p| p + 1);
    let end = (at..toks.len()).find(|&k| is_period(&toks[k])).unwrap_or(toks.len());
    (start, end)
}

struct Edit {
    from: usize,
    to: usize,
    edit: &'static str,
    stand_in: Option<&'static str>,
    toks: Vec<Token>,
}

/// The shortest edit touching the stop that lets the sentence read to its period: spans by
/// length, the nearest to the stop first, each removed and then replaced by every stand-in.
fn repair<T: Val>(rule: &P<T>, toks: &[Token], at: usize, stand_ins: &[(&'static str, &'static str, Vec<Token>)]) -> Option<Edit> {
    let (start, end) = sentence(toks, at);
    for len in 0..=MAX_SPAN {
        for from in (start.max(at.saturating_sub(BACK))..=at).rev() {
            let to = from + len;
            if to < at || to > end {
                continue;
            }
            let removal = (len > 0).then_some(("removed", None, &[][..]));
            let replaced = stand_ins.iter().map(|(kind, text, with)| (*kind, Some(*text), with.as_slice()));
            for (edit, stand_in, with) in removal.into_iter().chain(replaced) {
                let mut edited = Vec::with_capacity(toks.len() + with.len());
                edited.extend_from_slice(&toks[..from]);
                edited.extend(with.iter().cloned());
                edited.extend_from_slice(&toks[to..]);
                // read, or stopped in a later sentence
                let period = from + with.len() + (end - to);
                if stop(rule, &edited).is_none_or(|(next, _)| next > period) {
                    // a whole sentence replaced says only that the sentence is unknown, not
                    // what kind of phrase it is
                    let edit = if stand_in.is_some() && from == start && to == end { "a sentence" } else { edit };
                    return Some(Edit { from, to, edit, stand_in, toks: edited });
                }
            }
        }
    }
    None
}

/// The span's words, with each phrase the grammar knows inside it named by its kind: the longest
/// such phrase at each word, of two or more tokens (a lone "it" or "one" stays a word).
fn template(span: &[Token]) -> String {
    let (s, a) = (selectors(), amounts());
    let mut out: Vec<String> = Vec::new();
    let mut k = 0;
    while k < span.len() {
        let furthest = |ends: Vec<usize>| ends.into_iter().max().filter(|e| *e >= k + 2);
        let known = [
            ("⟨things⟩", furthest(s.set_pl.ends(span, k))),
            ("⟨thing⟩", furthest(s.selector.ends(span, k))),
            ("⟨amount⟩", furthest(a.amount.ends(span, k))),
            ("⟨zone⟩", furthest(s.zone.ends(span, k))),
            ("⟨mana⟩", mana_cost().ends(span, k).into_iter().max()),
        ];
        let best = known.iter().filter_map(|(name, end)| end.map(|e| (e, *name))).max_by_key(|(e, _)| *e);
        match (best, &span[k].kind) {
            (Some((e, name)), _) => {
                out.push(name.to_string());
                k = e;
            }
            (None, Kind::Number { .. }) => {
                out.push("⟨n⟩".into());
                k += 1;
            }
            (None, Kind::Quote { .. }) => {
                out.push("⟨quote⟩".into());
                k += 1;
            }
            (None, _) => {
                out.push(unspaced(&span[k..k + 1]).to_lowercase());
                k += 1;
            }
        }
    }
    out.join(" ")
}

/// The kinds of phrase these words are, read alone: every sub-grammar that reads all of them.
fn reads_as(span: &[Token]) -> Vec<&'static str> {
    use grammar::{condition::conditions, cost::costs, effect::effects, event::events};
    if span.is_empty() {
        return Vec::new();
    }
    fn reads<T: Val>(rule: &P<T>, span: &[Token]) -> bool {
        !matches!(rule.parse_all(span), Outcome::Declined { .. })
    }
    let (s, a, c, e, f) = (selectors(), amounts(), conditions(), events(), effects());
    let kinds = [
        ("a thing", reads(&s.selector, span) || reads(&s.selector_from, span)),
        ("things", reads(&s.set_pl, span) || reads(&s.set_pl_from, span)),
        ("a zone", reads(&s.zone, span)),
        ("an amount", reads(&a.amount, span)),
        ("a count", reads(&a.for_each, span)),
        ("a condition", reads(&c.condition, span)),
        ("restrictions", reads(&c.restrictions, span)),
        ("an event", reads(&e.pattern, span) || reads(&e.past, span) || reads(&e.perfect, span)),
        ("an instruction", reads(&f.body, span)),
        ("a modifier", reads(&grammar::effect::modifier(), span)),
        ("a cost", reads(&costs().cost, span)),
        ("a keyword", reads(&grammar::keyword::keyword(), span)),
    ];
    kinds.into_iter().filter_map(|(kind, read)| read.then_some(kind)).collect()
}

/// The gaps of a row the rule does not read, and the stop left when no edit explains it.
fn diagnose<T: Val>(rule: &P<T>, row: &[Token], quoted: bool, stand_ins: &[(&'static str, &'static str, Vec<Token>)]) -> (Vec<Gap>, Option<Unexplained>) {
    let mut toks = row.to_vec();
    let mut gaps: Vec<Gap> = Vec::new();
    loop {
        let Some((at, expected)) = stop(rule, &toks) else { return (gaps, None) };
        let unexplained = |toks: &[Token]| Unexplained {
            read: unspaced(&toks[..at]),
            at: toks.get(at).map_or("<end>".into(), |t| unspaced(std::slice::from_ref(t))),
            rest: unspaced(&toks[at..]),
            expected: expected.clone(),
        };
        if gaps.len() >= MAX_GAPS {
            return (gaps, Some(unexplained(&toks)));
        }
        // a quoted ability the grammar does not read has its own gaps; the row is read on with
        // a quoted ability it does read in its place
        if let Some(Token { kind: Kind::Quote { body, closed: true, mark }, space_before }) = toks.get(at) {
            let granted = &abilities().granted;
            if stop(granted, body).is_some() {
                let (inner, left) = diagnose(granted, body, true, stand_ins);
                if left.is_none() && !inner.is_empty() {
                    gaps.extend(inner);
                    let body = tokenize("{T}: Add {G}.");
                    toks[at] = Token { kind: Kind::Quote { body, closed: true, mark: *mark }, space_before: *space_before };
                    continue;
                }
            }
        }
        let Some(edit) = repair(rule, &toks, at, stand_ins) else { return (gaps, Some(unexplained(&toks))) };
        let span = &toks[edit.from..edit.to];
        gaps.push(Gap {
            edit: edit.edit,
            stand_in: edit.stand_in,
            span: unspaced(span),
            template: template(span),
            reads_as: reads_as(span),
            before: unspaced(&toks[edit.from.saturating_sub(3)..edit.from]),
            after: unspaced(&toks[edit.to..(edit.to + 3).min(toks.len())]),
            quoted,
        });
        toks = edit.toks;
    }
}

/// The stand-ins as tokens, each with the space that separates it from the word before.
fn stand_ins() -> Vec<(&'static str, &'static str, Vec<Token>)> {
    STAND_INS
        .iter()
        .map(|(kind, text)| {
            let mut toks = tokenize(text);
            if let Some(first) = toks.first_mut() {
                first.space_before = 1;
            }
            (*kind, *text, toks)
        })
        .collect()
}

/// An earlier run's rows with each gap's `template` and `reads_as` made again from its words.
fn reread_gaps(earlier: &str, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut lines: Vec<serde_json::Value> =
        std::fs::read_to_string(earlier)?.lines().map(serde_json::from_str).collect::<Result<_, _>>()?;
    lines.par_iter_mut().for_each(|row| {
        for gap in row["gaps"].as_array_mut().into_iter().flatten() {
            let span = tokenize(gap["span"].as_str().unwrap_or_default());
            gap["template"] = template(&span).into();
            gap["reads_as"] = reads_as(&span).into();
        }
    });
    let text: String = lines.iter().map(|row| format!("{row}\n")).collect();
    Ok(std::fs::write(out_path, text)?)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut out_path, mut only, mut reread) = (None, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--card" => only = args.next(),
            "--reread" => reread = args.next(),
            _ => out_path = Some(a),
        }
    }
    let Some(out_path) = out_path else {
        eprintln!("usage: grammar-gaps OUT.jsonl [--card NAME] [--reread GAPS.jsonl]");
        return ExitCode::FAILURE;
    };
    // the search is the slow part: an earlier run's gaps can be read again for what they are
    if let Some(earlier) = reread {
        return match reread_gaps(&earlier, &out_path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cannot reread {earlier} into {out_path}: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let faces = match corpus::load(&corpus::default_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read the corpus {}: {e}", corpus::default_path().display());
            return ExitCode::FAILURE;
        }
    };
    let ab = abilities();
    let stand_ins = stand_ins();
    let rows: Vec<RowGaps> = faces
        .par_iter()
        .filter(|f| only.as_ref().is_none_or(|name| &f.name == name || &f.face_name == name))
        .flat_map_iter(|f| {
            let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
            let mut out = Vec::new();
            for b in &classify(&n) {
                if b.kind == Row::Empty {
                    continue;
                }
                for (i, r) in b.rows.clone().enumerate() {
                    let toks = tokenize(&n.rows[r]);
                    if toks.is_empty() {
                        continue;
                    }
                    let unread = if i == 0 { stop(&ab.ability, &toks).is_some() } else { stop(&ab.mode, &toks).is_some() };
                    if !unread {
                        continue;
                    }
                    let (gaps, unexplained) = if i == 0 { diagnose(&ab.ability, &toks, false, &stand_ins) } else { diagnose(&ab.mode, &toks, false, &stand_ins) };
                    out.push(RowGaps {
                        oracle_id: f.oracle_id.clone(),
                        card: f.name.clone(),
                        face: f.face_name.clone(),
                        row: n.rows[r].clone(),
                        mode: i > 0,
                        gaps,
                        unexplained,
                    });
                }
            }
            out
        })
        .collect();
    let mut lines = String::new();
    for row in &rows {
        lines.push_str(&serde_json::to_string(row).expect("a row is plain data"));
        lines.push('\n');
    }
    if let Err(e) = std::fs::write(&out_path, lines) {
        eprintln!("cannot write {out_path}: {e}");
        return ExitCode::FAILURE;
    }
    let explained = rows.iter().filter(|r| r.unexplained.is_none()).count();
    let mut by_gaps: BTreeMap<usize, usize> = BTreeMap::new();
    let mut by_edit: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &rows {
        if row.unexplained.is_none() {
            *by_gaps.entry(row.gaps.len()).or_default() += 1;
        }
        for gap in &row.gaps {
            *by_edit.entry(gap.edit).or_default() += 1;
        }
    }
    println!("unread rows: {}  explained by edits: {explained}  not explained: {}", rows.len(), rows.len() - explained);
    println!("explained rows by number of gaps: {by_gaps:?}");
    println!("gaps by edit: {by_edit:?}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gaps of a row, and whether edits explain all of it. The unknown words are no
    /// Magic words, so the grammar never comes to read them.
    fn gaps_of(row: &str) -> (Vec<Gap>, bool) {
        let (gaps, left) = diagnose(&abilities().ability, &tokenize(row), false, &stand_ins());
        (gaps, left.is_none())
    }

    #[test]
    fn a_row_the_grammar_reads_has_no_gap() {
        let (gaps, explained) = gaps_of("Destroy target creature.");
        assert!(gaps.is_empty() && explained);
    }

    #[test]
    fn an_unknown_word_the_sentence_reads_without_is_removed() {
        let (gaps, explained) = gaps_of("Destroy target frobnicated creature.");
        assert!(explained);
        let [gap] = gaps.as_slice() else { panic!("one gap, not {}", gaps.len()) };
        assert_eq!((gap.edit, gap.span.as_str(), gap.stand_in), ("removed", "frobnicated", None));
        assert_eq!((gap.before.as_str(), gap.after.as_str()), ("Destroy target", "creature."));
        assert!(gap.reads_as.is_empty(), "an unknown word is no kind of phrase");
    }

    #[test]
    fn a_gap_is_found_in_the_sentence_it_is_in_and_each_sentence_has_its_own() {
        let (gaps, explained) = gaps_of("Destroy target frobnicated creature. Draw a card zorblaxly.");
        assert!(explained);
        let spans: Vec<&str> = gaps.iter().map(|gap| gap.span.as_str()).collect();
        assert_eq!(spans, ["frobnicated", "zorblaxly"]);
    }

    #[test]
    fn an_unknown_phrase_where_a_known_kind_stands_is_named_by_that_kind() {
        let (gaps, explained) = gaps_of("Destroy frobnicated zorblax.");
        assert!(explained);
        let [gap] = gaps.as_slice() else { panic!("one gap, not {}", gaps.len()) };
        assert_eq!((gap.edit, gap.span.as_str(), gap.stand_in), ("a thing", "frobnicated zorblax", Some("it")));
    }

    #[test]
    fn a_sentence_replaced_whole_is_an_unknown_sentence() {
        let (gaps, explained) = gaps_of("Draw a card. Frobnicate zorblaxly.");
        assert!(explained);
        let [gap] = gaps.as_slice() else { panic!("one gap, not {}", gaps.len()) };
        assert_eq!((gap.edit, gap.span.as_str()), ("a sentence", "Frobnicate zorblaxly"));
    }

    #[test]
    fn a_span_reads_as_the_kinds_of_phrase_it_is() {
        assert!(reads_as(&tokenize("until end of turn")).contains(&"a modifier"));
        assert!(reads_as(&tokenize("target creature you control")).contains(&"a thing"));
        assert!(reads_as(&tokenize("frobnicated")).is_empty());
        assert!(reads_as(&[]).is_empty());
    }

    #[test]
    fn a_template_names_the_phrases_the_grammar_knows_inside_a_gap() {
        assert_eq!(template(&tokenize("among legendary permanents you control")), "among ⟨things⟩");
        assert_eq!(template(&tokenize("zorblaxly")), "zorblaxly");
    }
}
