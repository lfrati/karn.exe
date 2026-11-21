//! Text generated from the grammar: is what the rules accept card text at all?
//!
//! `cargo run --profile fast -p grammar --bin grammar-sample -- RULE [--n N] [--seed S] [--depth D] [--show K]`
//!
//! RULE: ability | sentence | selector | condition | trigger | cost | amount.
//!
//! The gates ask whether the rules read real cards (wide enough). This asks the opposite: each
//! rule generates values at random ([`grammar::kernel::P::sample`]), which are printed and read
//! back. Verdicts:
//!
//! | verdict | meaning |
//! |---|---|
//! | OK | printed, and read back as the same value: text the grammar stands behind |
//! | UNPRINTABLE | a value the rule reads but cannot print (its two halves disagree) |
//! | MISMATCH | printed, but read back as another value |
//! | AMBIGUOUS | printed, but read back two ways |
//! | UNREADABLE | printed, but the rule cannot read its own text |
//!
//! Everything but OK is a defect of the rules found without any card. The OK texts are for
//! reading: whatever is not Magic among them shows where the rules are too wide.

#![forbid(unsafe_code)]

use grammar::kernel::{Gen, Outcome, P, Val};
use oracle::token::tokenize;
use std::collections::BTreeMap;
use std::process::ExitCode;

fn run<T: Val + std::fmt::Debug>(rule: &P<T>, n: usize, seed: u64, depth: usize, show: usize) {
    let mut verdicts: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut nowhere = 0usize;
    let mut made = 0usize;
    let mut s = seed;
    while made < n && s < seed + 50 * n as u64 {
        let mut g = Gen::new(s, depth, 4000);
        s += 1;
        let Some(value) = rule.sample(&mut g) else {
            nowhere += 1;
            continue;
        };
        made += 1;
        let Some(text) = rule.print_text(&value, true) else {
            verdicts.entry("UNPRINTABLE").or_default().push(format!("{value:?}"));
            continue;
        };
        let verdict = match rule.parse_all(&tokenize(&text)) {
            Outcome::Accepted { value: v2, .. } if v2 == value => "OK",
            Outcome::Accepted { .. } => "MISMATCH",
            Outcome::Ambiguous(_) => "AMBIGUOUS",
            Outcome::Declined { .. } => "UNREADABLE",
        };
        verdicts.entry(verdict).or_default().push(text);
    }
    println!("generated {made} (runs that led nowhere: {nowhere})");
    for (v, texts) in &verdicts {
        println!("  {v}: {} ({:.1}%)", texts.len(), 100.0 * texts.len() as f64 / made.max(1) as f64);
    }
    for (v, texts) in &verdicts {
        println!("\n{v}:");
        for t in texts.iter().take(if *v == "OK" { show } else { show.min(12) }) {
            println!("  {}", t.chars().take(300).collect::<String>());
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str, default: u64| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let (n, seed, depth, show) =
        (flag("--n", 500) as usize, flag("--seed", 1), flag("--depth", 60) as usize, flag("--show", 40) as usize);
    match args.first().map(String::as_str) {
        Some("ability") => run(&grammar::ability::abilities().ability, n, seed, depth, show),
        Some("sentence") => run(&grammar::effect::effects().sentence, n, seed, depth, show),
        Some("selector") => run(&grammar::selector::selectors().selector, n, seed, depth, show),
        Some("condition") => run(&grammar::condition::conditions().condition, n, seed, depth, show),
        Some("trigger") => run(&grammar::event::events().trigger, n, seed, depth, show),
        Some("cost") => run(&grammar::cost::costs().cost, n, seed, depth, show),
        Some("amount") => run(&grammar::amount::amounts().amount, n, seed, depth, show),
        _ => {
            eprintln!("usage: grammar-sample (ability|sentence|selector|condition|trigger|cost|amount) [--n N] [--seed S] [--depth D] [--show K]");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
