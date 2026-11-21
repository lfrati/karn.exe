//! Deterministic semantic snapshots of distinct corpus noun phrases. Unlike the grammar
//! gate, this records every selector interpretation and its IR, not just the best verdict.
//! `scripts/selector_snapshot.py` records input/revision fingerprints and compares snapshots.
use grammar::kernel::{Outcome, P, Val};
use grammar::selector::selectors;
use oracle::token::{Token, tokenize};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{BufRead, Write};

fn reading<T: Val + Serialize>(rule: &P<T>, text: &str, tokens: &[Token]) -> Value {
    match rule.parse_all(tokens) {
        Outcome::Accepted { value, .. } => {
            let printed = rule.print_text(&value, false);
            let roundtrips = printed.as_ref().is_some_and(|p|
                matches!(rule.parse_all(&tokenize(p)), Outcome::Accepted { value: other, .. } if other == value));
            let verdict = if !roundtrips { "mismatch" }
                else if printed.as_deref() == Some(text) { "round_trip" } else { "variant" };
            json!({"verdict": verdict, "value": value, "printed": printed})
        }
        Outcome::Ambiguous(values) => {
            let mut values: Vec<Value> = values.iter().map(|v| serde_json::to_value(v).unwrap()).collect();
            values.sort_by_key(Value::to_string);
            json!({"verdict": "ambiguous", "values": values})
        }
        Outcome::Declined { .. } => json!({"verdict": "declined"}),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: selector-snapshot noun_phrases.jsonl")?;
    let input = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut phrases = BTreeSet::new();
    for (i, line) in input.lines().enumerate() {
        let row: Value = serde_json::from_str(&line?)?;
        let text = row["text"].as_str().ok_or_else(|| format!("line {} has no text string", i + 1))?;
        phrases.insert(text.replace("SELF", "~"));
    }
    let s = selectors();
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for phrase in phrases {
        let row = if phrase.contains("QUOTED") {
            // The extractor's placeholder is not Oracle text. Keep it visible in the
            // snapshot denominator rather than silently discarding it.
            json!({"phrase":phrase, "skip":"quoted-placeholder"})
        } else {
            let tokens = tokenize(&phrase);
            json!({"phrase":phrase, "readings": {
                "selector": reading(&s.selector, &phrase, &tokens),
                "selector_from": reading(&s.selector_from, &phrase, &tokens),
                "set_sg": reading(&s.set_sg, &phrase, &tokens),
                "set_pl": reading(&s.set_pl, &phrase, &tokens),
            }})
        };
        serde_json::to_writer(&mut output, &row)?;
        writeln!(output)?;
    }
    Ok(())
}
