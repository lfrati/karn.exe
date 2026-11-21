//! Gates and statistics for the lossless front end over the whole corpus.
//!
//! `cargo run --release -p oracle --bin oracle-gate [-- PATH] [--examples KIND [--limit N]]`
//!
//! Gates (non-zero exit when one fails):
//! - `restore(normalize(t)) == t` for every face;
//! - `detokenize(tokenize(row)) == row` for every row;
//! - no `(`/`)` left in rows (all reminder text stripped), no unclosed quotes.

#![forbid(unsafe_code)]

use oracle::classify::{Kind, classify};
use oracle::normalize::{LabelKind, normalize};
use oracle::token::{self, Kind as T, Token};
use oracle::{corpus, cr};
use std::collections::BTreeMap;
use std::process::ExitCode;

fn count<K: Ord>(m: &mut BTreeMap<K, usize>, k: K) {
    *m.entry(k).or_default() += 1;
}

fn top<K: std::fmt::Debug>(m: &BTreeMap<K, usize>, n: usize) -> String {
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    v.iter().take(n).map(|(k, c)| format!("{k:?} {c}")).collect::<Vec<_>>().join(", ")
}

fn walk(tokens: &[Token], f: &mut impl FnMut(&Token)) {
    for t in tokens {
        f(t);
        if let T::Quote { body, .. } = &t.kind {
            walk(body, f);
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let examples_for = args.iter().position(|a| a == "--examples").and_then(|i| args.get(i + 1)).cloned();
    let limit: usize = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(60);
    let path = args
        .iter()
        .enumerate()
        .find(|(i, a)| !(a.starts_with("--") || *i > 0 && args[i - 1].starts_with("--")))
        .map(|(_, a)| a)
        .map(Into::into)
        .unwrap_or_else(corpus::default_path);
    let faces = match corpus::load(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read {}: {e} (run `uv run python scripts/fetch_resources.py`)", path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut failures: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut rows = 0usize;
    let mut labels: BTreeMap<LabelKind, usize> = BTreeMap::new();
    let mut flavor: BTreeMap<String, usize> = BTreeMap::new();
    let mut self_forms: BTreeMap<&str, usize> = BTreeMap::new();
    let mut puncts: BTreeMap<char, usize> = BTreeMap::new();
    let mut symbols: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
    let mut block_rows: BTreeMap<usize, usize> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut short_collisions: Vec<String> = Vec::new();
    let pw_types: Vec<String> = cr::list("planeswalker types").to_vec();

    for f in &faces {
        let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        if n.restore() != f.oracle_text {
            failures.entry("normalize round trip").or_default().push(f.face_name.clone());
        }
        for (i, r) in n.self_refs().iter().enumerate() {
            let form = if r.starts_with("this ") || r.starts_with("This ") {
                "noun"
            } else if *r == f.face_name || *r == f.name {
                "full name"
            } else {
                "short name"
            };
            count(&mut self_forms, form);
            if form == "short name" && pw_types.iter().any(|p| p == r) {
                short_collisions.push(format!("{} [#{i} {r:?}]", f.face_name));
            }
        }
        for (row, label) in n.rows.iter().zip(&n.labels) {
            rows += 1;
            if let Some(l) = label {
                count(&mut labels, l.kind);
                if l.kind == LabelKind::FlavorWord {
                    count(&mut flavor, l.text.clone());
                }
            }
            if row.contains('(') || row.contains(')') {
                failures.entry("reminder left in row").or_default().push(format!("{}: {row}", f.face_name));
            }
            let toks = token::tokenize(row);
            if token::detokenize(&toks) != *row {
                failures.entry("token round trip").or_default().push(format!("{}: {row}", f.face_name));
            }
            walk(&toks, &mut |t| match &t.kind {
                T::Punct(p) => count(&mut puncts, *p),
                T::Symbol(s) => count(&mut symbols, s.clone()),
                T::Quote { closed: false, .. } => {
                    failures.entry("unclosed quote").or_default().push(format!("{}: {row}", f.face_name))
                }
                _ => {}
            });
        }
        for b in classify(&n) {
            count(&mut kinds, b.kind);
            count(&mut block_rows, b.rows.len());
            if examples_for.as_deref().is_some_and(|k| format!("{:?}", b.kind) == k) {
                examples.push(format!("{}: {}", f.face_name, n.rows[b.rows.clone()].join(" / ")));
            }
        }
    }

    println!("faces {}  rows {rows}", faces.len());
    println!("labels: {}", top(&labels, 5));
    println!("flavor words ({} distinct): {}", flavor.len(), top(&flavor, 25));
    println!("self-references: {}", top(&self_forms, 5));
    println!(
        "short names that are planeswalker types ({}): {:?}",
        short_collisions.len(),
        &short_collisions[..short_collisions.len().min(10)]
    );
    println!("punctuation: {}", top(&puncts, 40));
    println!("symbols ({} distinct): {}", symbols.len(), top(&symbols, 80));
    println!("blocks by kind: {}", top(&kinds, 20));
    println!("rows per block: {}", top(&block_rows, 20));
    if !examples.is_empty() {
        println!("\nexamples of {} ({}):", examples_for.unwrap_or_default(), examples.len());
        for e in examples.iter().take(limit) {
            println!("  {e}");
        }
    }
    let mut ok = true;
    for gate in ["normalize round trip", "token round trip", "reminder left in row", "unclosed quote"] {
        let v = failures.get(gate).map(Vec::as_slice).unwrap_or_default();
        println!("GATE {gate}: {}", if v.is_empty() { "ok".to_string() } else { format!("{} FAIL", v.len()) });
        for e in v.iter().take(10) {
            println!("    {e}");
        }
        ok &= v.is_empty();
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
