//! Sub-grammar gates over their sub-corpora.
//!
//! `cargo run --release -p grammar --bin grammar-gate -- (selectors|amounts|triggers|conditions|costs|effects) [--examples VERDICT] [--limit N] [--dump FILE.jsonl]`
//!
//! **effects**: every effect sentence (triggered, activated, loyalty, chapter and mode rows,
//! quoted abilities, instant and sorcery text), split at top-level periods.
//!
//! **costs**: the costs of activated, loyalty and Class-level abilities and of "Keyword — cost:"
//! rows, found by the classifier in every normalized row and quoted ability (no dump needed).
//!
//! **conditions**: "if …", "as long as …", "unless …", intervening ifs and "only …" restrictions,
//! from `--dump-conditions`.
//!
//! **triggers**: the trigger conditions of triggered abilities ("When ~ enters", "At the beginning
//! of your upkeep"), from `--dump-triggers`.
//!
//! **amounts**: the amount expressions `scripts/corpus_structure.py --dump-amounts` found
//! (`.cache/oracle/amounts.jsonl`) with their introducer ("equal to …", "where X is …", "for each
//! …", "the number of …", "that much"), parsed with the introducer.
//!
//! **selectors**: the 67,820 noun phrases `scripts/corpus_structure.py --dump-nps` found
//! (`.cache/oracle/noun_phrases.jsonl`), each located in its face's normalized, tokenized rows
//! and parsed as a selector (zone spelled "in" or "from"), a bare set (singular or plural) or a
//! token description (the object of "create").
//! Verdicts:
//!
//! | verdict | meaning |
//! |---|---|
//! | ROUND_TRIP | one reading, printed back byte for byte |
//! | VARIANT | one reading; the printed text differs but reparses to the same value |
//! | MISMATCH | one reading, but its printed form reparses to something else (a grammar bug) |
//! | AMBIGUOUS | two or more different readings (a grammar bug) |
//! | PREFIX | no reading of the whole phrase, but of a prefix; what follows is ranked separately |
//! | DECLINED | no reading; the worklist, ranked by the token parsing died on |
//!
//! The chunker is regex-based and often runs past the noun phrase ("that card onto the
//! battlefield"); PREFIX separates that noise from real gaps, and its remainders show which
//! continuations the chunker swallowed (and which postmodifiers are still missing).

#![forbid(unsafe_code)]

use grammar::amount::amounts;
use grammar::condition::conditions;
use grammar::cost::costs;
use grammar::effect::effects;
use grammar::event::events;
use grammar::kernel::lit;
use grammar::kernel::{Outcome, P, Val, sentence_start};
use grammar::selector::selectors;
use oracle::corpus;
use oracle::normalize::normalize;
use oracle::token::{Kind, Token, detokenize, tokenize};
use std::collections::{BTreeMap, HashMap};
use std::process::ExitCode;

/// One sub-corpus entry: where it is, its text, and the corpus script's classes for it.
struct Item {
    oracle_id: String,
    face: String,
    text: String,
    classes: Vec<String>,
}

fn load_items(file: &str, flag: &str, class_fields: &[&str]) -> Result<Vec<Item>, String> {
    // These inventories are generated evidence, separate from pinned source inputs.
    let path = corpus::root().join(".cache/oracle").join(file);
    let s = std::fs::read_to_string(&path).map_err(|e| {
        format!("cannot read {}: {e} (run `uv run python scripts/corpus_structure.py {flag}`)", path.display())
    })?;
    Ok(s.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            let field = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            Item {
                oracle_id: field("oracle_id"),
                face: field("face"),
                text: field("text"),
                classes: class_fields.iter().map(|k| field(k)).collect(),
            }
        })
        .collect())
}

/// Every cost the classifier finds: one item per activated/loyalty/level/keyword-prefixed row or
/// quoted ability, its text detokenized from the normalized tokens.
fn cost_items(faces: &[corpus::Face]) -> Vec<Item> {
    let mut out = Vec::new();
    for f in faces {
        let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        let mut arrs = Vec::new();
        for r in &n.rows {
            arrays(&tokenize(r), &mut arrs);
        }
        for a in &arrs {
            if let Some((kind, span)) = oracle::classify::cost_span(a) {
                out.push(Item {
                    oracle_id: f.oracle_id.clone(),
                    face: f.face_name.clone(),
                    text: detokenize(&strip_space(&a[span])),
                    classes: vec![format!("{kind:?}")],
                });
            }
        }
    }
    out
}

/// Every effect sentence: the effect span of each ability row or quoted ability (and every row
/// of an instant or sorcery), split at its top-level periods. Classes: the row kind, and the
/// sentence's first word.
fn effect_items(faces: &[corpus::Face]) -> Vec<Item> {
    let mut out = Vec::new();
    for f in faces {
        let spell = f.type_line.contains("Instant") || f.type_line.contains("Sorcery");
        let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        let mut arrs = Vec::new();
        for r in &n.rows {
            let mut row = Vec::new();
            arrays(&tokenize(r), &mut row);
            // the row itself first, then its quoted abilities (never spell text)
            arrs.extend(row.into_iter().enumerate().map(|(i, a)| (i == 0, a)));
        }
        for (top, a) in &arrs {
            let Some((kind, span)) = oracle::classify::effect_span(a, spell && *top) else {
                continue;
            };
            let toks = &a[span];
            for sentence in oracle::classify::sentences(toks) {
                let first = sentence.iter().find_map(|t| t.word().map(str::to_string)).unwrap_or_default();
                out.push(Item {
                    oracle_id: f.oracle_id.clone(),
                    face: f.face_name.clone(),
                    text: detokenize(&strip_space(sentence)),
                    classes: vec![format!("{kind:?}"), first],
                });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    RoundTrip,
    Variant,
    Mismatch,
    Ambiguous,
    Prefix,
    Declined,
}

/// Token arrays to search: each row, and each quoted body inside it, recursively.
fn arrays(toks: &[Token], out: &mut Vec<Vec<Token>>) {
    out.push(toks.to_vec());
    for t in toks {
        if let Kind::Quote { body, .. } = &t.kind {
            arrays(body, out);
        }
    }
}

fn same(a: &Kind, b: &Kind) -> bool {
    match (a, b) {
        (Kind::Word(x), Kind::Word(y)) => x == y,
        (x, y) => x == y,
    }
}

fn find(hay: &[Token], needle: &[Token]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| needle.iter().zip(&hay[i..]).all(|(n, h)| same(&n.kind, &h.kind)))
}

/// A rule applied to a span: a verdict, or where and why it declined.
type Judge<'a> = Box<dyn Fn(&[Token], usize, usize) -> Result<Judged, (usize, Vec<String>)> + Sync + 'a>;

/// An item found in its row, with what the rules made of it.
struct Located<'a> {
    arr: &'a [Token],
    start: usize,
    end: usize,
    judged: Option<Judged>,
    best_decline: Option<(usize, Vec<String>)>,
    prefix_end: Option<usize>,
}

struct Judged {
    verdict: Verdict,
    rule: &'static str,
    detail: String,
}

fn judge<T: Val + std::fmt::Debug>(
    rule: &P<T>,
    toks: &[Token],
    start: usize,
    end: usize,
) -> Result<Judged, (usize, Vec<String>)> {
    match rule.parse_span(toks, start, end) {
        Outcome::Accepted { value, .. } => {
            let cap = sentence_start(toks, start);
            let original = detokenize(&strip_space(&toks[start..end]));
            let Some(printed) = rule.print_text(&value, cap) else {
                return Ok(Judged { verdict: Verdict::Mismatch, rule: "", detail: format!("cannot print {value:?}") });
            };
            if printed == original {
                return Ok(Judged { verdict: Verdict::RoundTrip, rule: "", detail: String::new() });
            }
            let reparsed = tokenize(&printed);
            let verdict = match rule.parse_all(&reparsed) {
                Outcome::Accepted { value: v2, .. } if v2 == value => Verdict::Variant,
                _ => Verdict::Mismatch,
            };
            Ok(Judged { verdict, rule: "", detail: format!("{original}  →  {printed}") })
        }
        Outcome::Ambiguous(vs) => Ok(Judged { verdict: Verdict::Ambiguous, rule: "", detail: format!("{vs:?}") }),
        Outcome::Declined { at, expected, .. } => Err((at, expected)),
    }
}

fn strip_space(t: &[Token]) -> Vec<Token> {
    let mut v = t.to_vec();
    if let Some(f) = v.first_mut() {
        f.space_before = 0;
    }
    v
}

fn token_text(t: Option<&Token>) -> String {
    match t {
        None => "<end>".into(),
        Some(t) => detokenize(&strip_space(std::slice::from_ref(t))),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let examples_for = flag("--examples");
    let limit: usize = flag("--limit").and_then(|n| n.parse().ok()).unwrap_or(40);
    // one JSON line per item: verdict, and where reading stopped (for worklist analysis)
    let dump_path = flag("--dump");
    let mut dump: Vec<String> = Vec::new();
    let sub = args.first().cloned().unwrap_or_default();
    if sub == "cards" {
        return cards(flag("--dump"), examples_for.as_deref(), limit);
    }
    let (file, flag, class_fields, class_titles): (&str, &str, Vec<&str>, Vec<&str>) = match sub.as_str() {
        "selectors" => {
            ("noun_phrases.jsonl", "--dump-nps", vec!["quant", "head"], vec!["quantifier class", "head class"])
        }
        "amounts" => ("amounts.jsonl", "--dump-amounts", vec!["intro"], vec!["introducer"]),
        "triggers" => ("triggers.jsonl", "--dump-triggers", vec!["kind"], vec!["ability kind"]),
        "conditions" => ("conditions.jsonl", "--dump-conditions", vec!["intro"], vec!["introducer"]),
        "costs" => ("", "", vec!["kind"], vec!["ability kind"]),
        "effects" => ("", "", vec!["kind", "first"], vec!["ability kind", "first word"]),
        _ => {
            eprintln!(
                "usage: grammar-gate (selectors|amounts|triggers|conditions|costs|effects|cards) [--examples VERDICT] [--limit N]"
            );
            return ExitCode::FAILURE;
        }
    };
    let faces = match corpus::load(&corpus::default_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read the corpus {}: {e}", corpus::default_path().display());
            return ExitCode::FAILURE;
        }
    };
    let loaded = match sub.as_str() {
        "costs" => Ok(cost_items(&faces)),
        "effects" => Ok(effect_items(&faces)),
        _ => load_items(file, flag, &class_fields),
    };
    let nps = match loaded {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let normalized: Vec<Vec<Vec<Token>>> = {
        use rayon::prelude::*;
        faces
            .par_iter()
            .map(|f| {
                let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
                let mut arrs = Vec::new();
                for r in &n.rows {
                    arrays(&tokenize(r), &mut arrs);
                }
                arrs
            })
            .collect()
    };
    let mut rows: HashMap<(&str, &str), Vec<Vec<Token>>> = HashMap::new();
    for (f, arrs) in faces.iter().zip(normalized) {
        rows.entry((f.oracle_id.as_str(), f.face_name.as_str())).or_default().extend(arrs);
    }

    let s = selectors();
    let tokens = grammar::token::token_spec();
    let a = amounts();
    let equal_to = lit("equal to").then_keep(&a.amount);
    let where_x = lit("where X is").then_keep(&a.amount);
    let rules: Vec<(&'static str, Judge)> = match sub.as_str() {
        "selectors" => vec![
            ("selector", Box::new(|t, a, b| judge(&s.selector, t, a, b))),
            ("selector (from)", Box::new(|t, a, b| judge(&s.selector_from, t, a, b))),
            ("set", Box::new(|t, a, b| judge(&s.set_sg, t, a, b))),
            ("plural set", Box::new(|t, a, b| judge(&s.set_pl, t, a, b))),
            ("token", Box::new(|t, a, b| judge(tokens, t, a, b))),
        ],
        "triggers" => vec![("trigger", Box::new(|t, x, y| judge(&events().trigger, t, x, y)))],
        "effects" => vec![("sentence", Box::new(|t, x, y| judge(&effects().sentence, t, x, y)))],
        "costs" => vec![
            ("cost", Box::new(|t, x, y| judge(&costs().cost, t, x, y))),
            ("granted cost", Box::new(|t, x, y| judge(&costs().granted_cost, t, x, y))),
        ],
        "conditions" => vec![
            ("condition", Box::new(|t, x, y| judge(&conditions().condition, t, x, y))),
            ("restrictions", Box::new(|t, x, y| judge(&conditions().restrictions, t, x, y))),
        ],
        _ => vec![
            ("equal to", Box::new(|t, a, b| judge(&equal_to, t, a, b))),
            ("where X is", Box::new(|t, a, b| judge(&where_x, t, a, b))),
            ("for each", Box::new(|t, x, y| judge(&a.for_each, t, x, y))),
            ("amount", Box::new(|t, x, y| judge(&a.amount, t, x, y))),
        ],
    };

    let mut verdicts: BTreeMap<Verdict, usize> = BTreeMap::new();
    let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unlocated = 0usize;
    let mut quoted = 0usize;
    let mut dead: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut by_class: Vec<BTreeMap<String, (usize, usize)>> = vec![BTreeMap::new(); class_fields.len()];
    let mut examples: Vec<String> = Vec::new();
    let mut declined_literal: BTreeMap<String, usize> = BTreeMap::new();
    let mut remainders: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();

    // parsing is independent per item: run it on all cores, then tally in order
    use rayon::prelude::*;
    let judged_items: Vec<Option<Located>> = nps
        .par_iter()
        .map(|np| {
            if np.text.contains("QUOTED") {
                return None;
            }
            let needle = tokenize(&np.text.replace("SELF", "~"));
            let arrs = rows.get(&(np.oracle_id.as_str(), np.face.as_str()))?;
            let (arr, start) = arrs.iter().find_map(|a| find(a, &needle).map(|i| (a, i)))?;
            let end = start + needle.len();
            let mut best_decline: Option<(usize, Vec<String>)> = None;
            let mut judged = None;
            for (name, rule) in &rules {
                match rule(arr, start, end) {
                    Ok(mut j) => {
                        j.rule = name;
                        judged = Some(j);
                        break;
                    }
                    Err((at, exp)) => {
                        if best_decline.as_ref().is_none_or(|(b, _)| at > *b) {
                            best_decline = Some((at, exp));
                        }
                    }
                }
            }
            // the longest prefix some rule reads, when the whole phrase is declined
            let prefix_end = if judged.is_none() {
                (start + 1..end).rev().find(|&e| rules.iter().any(|(_, r)| matches!(r(arr, start, e), Ok(j) if matches!(j.verdict, Verdict::RoundTrip | Verdict::Variant))))
            } else {
                None
            };
            Some(Located { arr, start, end, judged, best_decline, prefix_end })
        })
        .collect();
    for (np, located) in nps.iter().zip(judged_items) {
        let Some(Located { arr, start, end, judged, best_decline, prefix_end }) = located else {
            if np.text.contains("QUOTED") {
                quoted += 1;
            } else {
                unlocated += 1;
            }
            continue;
        };
        if let Some(e) = prefix_end {
            *verdicts.entry(Verdict::Prefix).or_default() += 1;
            if dump_path.is_some() {
                dump.push(
                    serde_json::json!({"oracle_id": np.oracle_id, "face": np.face, "text": detokenize(&strip_space(&arr[start..end])),
                        "classes": np.classes, "verdict": "Prefix", "read": detokenize(&strip_space(&arr[start..e])),
                        "rest": detokenize(&strip_space(&arr[e..end]))})
                    .to_string(),
                );
            }
            let words: Vec<String> = arr[e..end.min(e + 2)].iter().map(|t| token_text(Some(t))).collect();
            let r = remainders.entry(words.join(" ")).or_default();
            r.0 += 1;
            if r.1.len() < 2 {
                r.1.push(format!("{} ⟦{}⟧", detokenize(&strip_space(&arr[start..e])), detokenize(&arr[e..end])));
            }
            for (map, key) in by_class.iter_mut().zip(&np.classes) {
                map.entry(key.clone()).or_default().0 += 1;
            }
            if examples_for.as_deref() == Some("Prefix") && examples.len() < limit {
                examples.push(format!(
                    "{}: {} ⟦{}⟧",
                    np.face,
                    detokenize(&strip_space(&arr[start..e])),
                    detokenize(&arr[e..end])
                ));
            }
            continue;
        }
        let verdict = judged.as_ref().map_or(Verdict::Declined, |j| j.verdict);
        *verdicts.entry(verdict).or_default() += 1;
        if dump_path.is_some() {
            let at = best_decline.as_ref().map_or(start, |(a, _)| (*a).clamp(start, end));
            let (read, rest) = if judged.is_some() { (String::new(), String::new()) } else {
                (detokenize(&strip_space(&arr[start..at])), detokenize(&strip_space(&arr[at..end])))
            };
            dump.push(
                serde_json::json!({"oracle_id": np.oracle_id, "face": np.face, "text": detokenize(&strip_space(&arr[start..end])),
                    "classes": np.classes, "verdict": format!("{verdict:?}"), "read": read, "rest": rest,
                    "expected": best_decline.as_ref().filter(|_| judged.is_none()).map(|(_, e)| e.clone()).unwrap_or_default()})
                .to_string(),
            );
        }
        let ok = matches!(verdict, Verdict::RoundTrip | Verdict::Variant);
        for (map, key) in by_class.iter_mut().zip(&np.classes) {
            let e = map.entry(key.clone()).or_default();
            e.0 += 1;
            e.1 += ok as usize;
        }
        match &judged {
            Some(j) => {
                *by_rule.entry(j.rule).or_default() += 1;
                if examples_for.as_deref() == Some(&format!("{verdict:?}")) && examples.len() < limit {
                    examples.push(format!(
                        "[{}] {}: {}",
                        j.rule,
                        np.face,
                        if j.detail.is_empty() { np.text.clone() } else { j.detail.clone() }
                    ));
                }
            }
            None => {
                let (at, exp) = best_decline.unwrap_or_default();
                let at = at.max(start);
                let key = token_text(arr.get(at));
                let e = dead.entry(key).or_default();
                e.0 += 1;
                if e.1.len() < 4 {
                    let marked = format!(
                        "{} ⟨{}⟩ {}",
                        detokenize(&strip_space(&arr[start..at.min(end)])),
                        token_text(arr.get(at)),
                        if at + 1 < end { detokenize(&arr[at + 1..end]) } else { String::new() }
                    );
                    e.1.push(format!("{marked}   (expected {})", exp.join(" | ")));
                }
                let lit: String = np
                    .text
                    .split(' ')
                    .map(|w| if w.chars().all(|c| c.is_ascii_digit()) { "N" } else { w })
                    .collect::<Vec<_>>()
                    .join(" ");
                *declined_literal.entry(lit).or_default() += 1;
                if examples_for.as_deref() == Some("Declined") && examples.len() < limit {
                    examples.push(format!("{}: {}", np.face, np.text));
                }
            }
        }
    }

    let located: usize = verdicts.values().sum();
    let pct = |n: usize| 100.0 * n as f64 / located.max(1) as f64;
    println!("{sub}: {}  located {located}  unlocated {unlocated}  with quoted text (skipped) {quoted}", nps.len());
    for (v, n) in &verdicts {
        println!("  {v:?}: {n} ({:.1}%)", pct(*n));
    }
    println!("accepted by rule: {by_rule:?}");
    let table = |title: &str, m: &BTreeMap<String, (usize, usize)>| {
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        println!("\n{title}:");
        for (k, (n, ok)) in v {
            println!("  {:>6} {:>5.1}%  {k}", n, 100.0 * *ok as f64 / *n as f64);
        }
    };
    for (title, m) in class_titles.iter().zip(&by_class) {
        table(&format!("by {title} (count, % round trip or variant)"), m);
    }
    let mut d: Vec<_> = dead.into_iter().collect();
    d.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!("\ndeclines by the token parsing died on (top 40):");
    for (tok, (n, ex)) in d.iter().take(40) {
        println!("  {n:>6}  {tok:?}");
        for e in ex.iter().take(2) {
            println!("            {e}");
        }
    }
    let mut r: Vec<_> = remainders.into_iter().collect();
    r.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!("\nPREFIX: what follows the longest readable prefix (top 30):");
    for (w, (n, ex)) in r.iter().take(30) {
        println!("  {n:>6}  {w:?}   e.g. {}", ex.join("  |  "));
    }
    let mut lits: Vec<_> = declined_literal.into_iter().collect();
    lits.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nmost frequent declined phrases:");
    for (l, n) in lits.iter().take(30) {
        println!("  {n:>5}  {l}");
    }
    if !examples.is_empty() {
        println!("\nexamples of {}:", examples_for.unwrap_or_default());
        for e in &examples {
            println!("  {e}");
        }
    }
    let bugs = verdicts.get(&Verdict::Mismatch).copied().unwrap_or(0)
        + verdicts.get(&Verdict::Ambiguous).copied().unwrap_or(0);
    if let Some(p) = &dump_path
        && let Err(e) = std::fs::write(p, dump.join("\n") + "\n")
    {
        eprintln!("cannot write {p}: {e}");
    }
    println!("\nGATE no mismatch, no ambiguity: {}", if bugs == 0 { "ok".to_string() } else { format!("{bugs} FAIL") });
    if bugs == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// The whole-card gate: every row of every face read as an ability (or a mode under one); a face
/// passes when all its rows round-trip. Failing faces with a single failing row are the
/// worklist, grouped by where that row's reading stopped.
fn cards(dump_path: Option<String>, examples_for: Option<&str>, limit: usize) -> ExitCode {
    use grammar::ability::abilities;
    use oracle::classify::{Kind as Row, classify};
    let faces = match corpus::load(&corpus::default_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read the corpus {}: {e}", corpus::default_path().display());
            return ExitCode::FAILURE;
        }
    };
    let ab = abilities();
    let mut rows_by_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut with_text, mut passing, mut bugs) = (0usize, 0usize, 0usize);
    let mut failing_rows_per_face: BTreeMap<usize, usize> = BTreeMap::new();
    let mut worklist: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut dump: Vec<String> = Vec::new();
    // each face is read on its own: all cores, then tallied in order
    struct FaceRead {
        failures: Vec<(String, String, String)>,
        rows: Vec<(String, bool)>,
        bugs: usize,
        bug_details: Vec<(String, String)>,
        any: bool,
    }
    let reads: Vec<FaceRead> = {
        use rayon::prelude::*;
        faces.par_iter().map(|f| {
        let mut rows_by_kind: Vec<(String, bool)> = Vec::new();
        let mut bugs = 0usize;
        let mut bug_details: Vec<(String, String)> = Vec::new();
        let printed = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        let mut failures: Vec<(String, String, String)> = Vec::new(); // (kind, stop key, row)
        let mut any = false;
        // a face with cleave (702.148a) is read twice: as printed, and with the text in square
        // brackets removed
        for n in std::iter::once(&printed).chain(printed.cleaved()) {
        let blocks = classify(n);
        for b in &blocks {
            if b.kind == Row::Empty {
                continue;
            }
            for (i, r) in b.rows.clone().enumerate() {
                let toks = tokenize(&n.rows[r]);
                if toks.is_empty() {
                    continue;
                }
                any = true;
                let kind = if i == 0 { format!("{:?}", b.kind) } else { "Mode".to_string() };
                let judged = if i == 0 { judge(&ab.ability, &toks, 0, toks.len()) } else { judge(&ab.mode, &toks, 0, toks.len()) };
                // each quoted ability must read too (a row reads with a quote it can't read as
                // text); a quotation the row reads as a term is no ability
                let mut nested = Vec::new();
                let granted = |t: &[Token]| reading(&ab.granted, t);
                if i == 0 {
                    quoted_abilities(&toks, &|t| reading(&ab.ability, t), &granted, &mut nested);
                } else {
                    quoted_abilities(&toks, &|t| reading(&ab.mode, t), &granted, &mut nested);
                }
                for body in &nested {
                        match judge(&ab.granted, body, 0, body.len()) {
                        Ok(j) if matches!(j.verdict, Verdict::RoundTrip | Verdict::Variant) => rows_by_kind.push(("Quoted".into(), true)),
                        Ok(j) => {
                            rows_by_kind.push(("Quoted".into(), false));
                            bugs += 1;
                            bug_details.push((format!("{:?}", j.verdict), format!("{}: {}", f.face_name, j.detail)));
                            failures.push(("Quoted".into(), format!("<{:?}>", j.verdict), detokenize(body)));
                        }
                        Err((at, _)) => {
                            rows_by_kind.push(("Quoted".into(), false));
                            let stop: Vec<String> = body[at.min(body.len())..].iter().take(2).map(|t| token_text(Some(t))).collect();
                            let key = if at == 0 { format!("<start> {}", stop.join(" ")) } else { stop.join(" ") };
                            failures.push((
                                "Quoted".into(),
                                key,
                                format!("{} ⟨{}", detokenize(&body[..at.min(body.len())]), detokenize(&strip_space(&body[at.min(body.len())..]))),
                            ));
                        }
                    }
                }
                let row = detokenize(&toks);
                let ok = matches!(&judged, Ok(j) if matches!(j.verdict, Verdict::RoundTrip | Verdict::Variant));
                rows_by_kind.push((kind.clone(), ok));
                match judged {
                    Ok(_) if ok => {}
                    Ok(j) => {
                        bugs += 1;
                        bug_details.push((format!("{:?}", j.verdict), format!("{}: {}", f.face_name, j.detail)));
                        failures.push((kind, format!("<{:?}>", j.verdict), row));
                    }
                    Err((at, _)) => {
                        let stop: Vec<String> = toks[at.min(toks.len())..].iter().take(2).map(|t| token_text(Some(t))).collect();
                        let key = if at == 0 { format!("<start> {}", stop.join(" ")) } else { stop.join(" ") };
                        let marked = format!("{} ⟨{}", detokenize(&toks[..at.min(toks.len())]), detokenize(&strip_space(&toks[at.min(toks.len())..])));
                        failures.push((kind, key, marked));
                    }
                }
            }
        }
        }
        FaceRead { failures, rows: rows_by_kind, bugs, bug_details, any }
        }).collect()
    };
    for (f, read) in faces.iter().zip(reads) {
        let FaceRead { failures, rows, bugs: b, bug_details, any } = read;
        bugs += b;
        for (kind, ok) in rows {
            let e = rows_by_kind.entry(kind).or_default();
            e.0 += 1;
            e.1 += ok as usize;
        }
        for (verdict, detail) in bug_details {
            if examples_for == Some(verdict.as_str()) && examples.len() < limit {
                examples.push(detail);
            }
        }
        if !any {
            continue;
        }
        with_text += 1;
        *failing_rows_per_face.entry(failures.len().min(5)).or_default() += 1;
        if failures.is_empty() {
            passing += 1;
        }
        if let [(kind, key, row)] = failures.as_slice() {
            let w = worklist.entry(format!("{kind:>15}  {key}")).or_default();
            w.0 += 1;
            if w.1.len() < 2 {
                w.1.push(format!("{}: {row}", f.face_name));
            }
        }
        if examples_for == Some("Failing") && !failures.is_empty() && examples.len() < limit {
            examples.push(format!("{}: {}", f.face_name, failures.iter().map(|x| x.2.as_str()).collect::<Vec<_>>().join("  ||  ")));
        }
        if dump_path.is_some() {
            dump.push(
                serde_json::json!({"oracle_id": f.oracle_id, "face": f.face_name, "ok": failures.is_empty(),
                    "failures": failures.iter().map(|(k, s, r)| serde_json::json!({"kind": k, "stop": s, "row": r})).collect::<Vec<_>>()})
                .to_string(),
            );
        }
    }
    println!("faces: {}  with text: {with_text}  all rows round-trip: {passing} ({:.1}%)", faces.len(), 100.0 * passing as f64 / with_text.max(1) as f64);
    println!("failing rows per face: {failing_rows_per_face:?}  (5 = five or more)");
    println!("\nrows by kind (count, % round trip or variant):");
    for (k, (c, ok)) in &rows_by_kind {
        println!("  {c:7}  {:5.1}%  {k}", 100.0 * *ok as f64 / (*c).max(1) as f64);
    }
    let mut w: Vec<_> = worklist.into_iter().collect();
    w.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!("\nfaces one row away, by row kind and where reading stopped (top 60):");
    for (k, (c, ex)) in w.iter().take(60) {
        println!("  {c:6}  {k}");
        for e in ex {
            println!("            {}", e.chars().take(200).collect::<String>());
        }
    }
    if !examples.is_empty() {
        println!("\nexamples:");
        for e in &examples {
            println!("  {e}");
        }
    }
    if let Some(p) = &dump_path
        && let Err(e) = std::fs::write(p, dump.join("\n") + "\n")
    {
        eprintln!("cannot write {p}: {e}");
    }
    println!("\nGATE no mismatch, no ambiguity: {}", if bugs == 0 { "ok".to_string() } else { format!("{bugs} FAIL") });
    if bugs == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// A quoted ability's body with the period it drops mid-sentence put back (inside a nested quote
/// that ends it).
fn with_final_period(body: &[Token]) -> Vec<Token> {
    let mut v = body.to_vec();
    match v.last_mut().map(|t| &mut t.kind) {
        Some(Kind::Punct('.' | '!' | '—')) | None => {}
        Some(Kind::Quote { body: inner, .. }) => {
            let fixed = with_final_period(inner);
            *inner = fixed;
        }
        Some(_) => v.push(Token { kind: Kind::Punct('.'), space_before: 0 }),
    }
    v
}

/// The quoted abilities a reading holds, not counting those inside them: each quotation the
/// grammar reads as an ability is a `Granted::Ability`, or a `Granted::Quoted` when the ability
/// grammar declines it; nothing else in the IR is serialized as an object keyed `Ability` or
/// `Quoted`.
fn granted_quotes(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Object(m) if m.len() == 1 && (m.contains_key("Ability") || m.contains_key("Quoted")) => 1,
        serde_json::Value::Object(m) => m.values().map(granted_quotes).sum(),
        serde_json::Value::Array(a) => a.iter().map(granted_quotes).sum(),
        _ => 0,
    }
}

/// The quotations to judge as abilities in tokens that are a row (or a quoted ability's body),
/// with those inside each, outermost first. What a quotation is, the tokens' own reading says
/// (`read`): a quotation read as an ability leaves a granted ability in the reading; one read as a
/// term (the "legend rule", 704.5j) leaves none. So when the tokens read and the reading holds no
/// granted ability, their quotations are terms and none is judged. Otherwise every quotation is
/// judged as an ability, as before: when the tokens do not read, nothing says what a quotation
/// is, and when the reading holds some granted abilities it is not told here which quotation is
/// which, so no failure is hidden that the reading does not account for.
fn quoted_abilities(
    toks: &[Token],
    read: &dyn Fn(&[Token]) -> Option<serde_json::Value>,
    granted: &dyn Fn(&[Token]) -> Option<serde_json::Value>,
    out: &mut Vec<Vec<Token>>,
) {
    let quotes: Vec<&Vec<Token>> = toks
        .iter()
        .filter_map(|t| match &t.kind {
            Kind::Quote { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    if quotes.is_empty() {
        return;
    }
    if read(toks).is_some_and(|v| granted_quotes(&v) == 0) {
        return;
    }
    for body in quotes {
        // a quote the sentence goes on after dropped its period: judge the ability with it, as
        // it stands alone
        let body = with_final_period(body);
        out.push(body.clone());
        quoted_abilities(&body, granted, granted, out);
    }
}

/// The value of the one reading of the tokens, if they have one.
fn reading<T: Val + serde::Serialize>(rule: &P<T>, toks: &[Token]) -> Option<serde_json::Value> {
    match rule.parse_all(toks) {
        Outcome::Accepted { value, .. } => serde_json::to_value(&value).ok(),
        _ => None,
    }
}
