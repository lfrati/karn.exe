//! How much of the corpus the engine runs, and what it needs next.
//!
//! `cargo run --profile fast -p game --bin game-gate -- [--examples REASON] [--limit N] [--deck NAME] [--runs TEXT] [--list] [--admission-snapshot PATH [--all-blockers]] [--program-dump PATH]`
//!
//! Every face is read by the grammar into a card definition and checked by the engine
//! ([`game::play::check`]): can all of its text be run? The faces that cannot are grouped by the
//! first thing the engine stopped at, ranked by faces: what the model lacks, in the grammar's
//! own terms; those the grammar does not read, by where it first stopped. Every refused face
//! has a reason; `--all-blockers` lists all of them ([`game::play::blockers`],
//! [`CardDef::unread_face`]). Then the decks of `decks/`, card by card for the one `--deck` names: a deck all
//! of whose cards run is the milestone.

#![forbid(unsafe_code)]

use game::card::CardDef;
use game::play::{Blocker, blockers, check};
use oracle::corpus;
use std::collections::BTreeMap;
use std::process::ExitCode;

const HELP: &str = "\
game-gate: how much of the corpus the engine runs, and what it needs next.

usage: game-gate [OPTIONS]

  --limit N                  examples kept per refusal reason (default 20)
  --examples REASON          list the examples of the reasons containing REASON
  --runs TEXT                list admitted faces whose Oracle text contains TEXT
  --deck NAME                list the cards of decks/NAME.dck the engine does not run
  --list                     print the single-faced cards the engine runs (RUN lines) and stop
  --admission-snapshot PATH  write one JSON line per card face: {oracle_id, face, name, layout,
                             stage: parse|engine|admitted, reason, compiled_abilities}, where
                             reason is the first refusal (null when admitted)
  --all-blockers             with --admission-snapshot: each line also has \"blockers\": every
                             distinct refusal of the face, [{stage, reason, label, face?, row?,
                             unanalyzed?}, ...] in the order admission meets them (the first is
                             `reason`; empty when admitted). The grammar reads each text row on
                             its own: each row it declines is one blocker, with where it stopped
                             (row.stop); the engine checks go on past each refusal. `label` is
                             what reports count: the reason, and of a declined row the token it
                             stopped at. A blocker's `unanalyzed` says what it kept from being
                             analyzed (the rest of its row, ability or checks), the line's
                             `unanalyzed` what was not analyzed of the face as a whole (engine
                             admission of a face not read); `fully_analyzed` when neither has
                             anything. Exits with failure when the blockers disagree with
                             admission. Slower: admission is rerun.
  --program-dump PATH       write one JSON line per admitted face, sorted by {oracle_id, face}:
                             {oracle_id, face, name, layout, parts}, where parts maps whole,
                             half:N, adventure, omen and prepare to what that definition
                             compiled (CardDef::program_dump), each program by its id and
                             canonical text. Prints a PROGRAMS line: faces, programs, distinct
                             ids and the FNV-1a digest of the file. Fails if an id is not the
                             digest of its canonical text or two texts share one.
  -h, --help                 this help
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let all_blockers = args.iter().any(|a| a == "--all-blockers");
    let examples_for = flag("--examples");
    let limit: usize = flag("--limit").and_then(|n| n.parse().ok()).unwrap_or(20);
    let faces = match corpus::load(&corpus::default_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read the corpus {}: {e}", corpus::default_path().display());
            return ExitCode::FAILURE;
        }
    };
    let catalog = CardDef::catalog(&faces);
    let program_dump = flag("--program-dump");
    // per face: Ok, or (stage, reason); every refusal: of reading a face the grammar does not
    // read (each with the face it is of, of a card read whole), and with --all-blockers of the
    // engine stage; with --program-dump, what an admitted face compiled
    let (verdicts, routes, all, programs) = {
        use rayon::prelude::*;
        let rows: Vec<(Verdict, Option<BTreeMap<String, _>>, Vec<(Option<u8>, Blocker)>, Option<serde_json::Value>)> = faces
            .par_iter()
            .map(|f| {
                let whole = matches!(f.layout.as_str(), "split" | "adventure" | "prepare");
                match if whole { CardDef::from_faces(&catalog[f.name.as_str()]) } else { CardDef::from_face(f) } {
                    Err(e) => {
                        let read: &[&corpus::Face] = if whole { &catalog[f.name.as_str()] } else { std::slice::from_ref(&f) };
                        let unread = read.iter().flat_map(|g| CardDef::unread_face(g).into_iter().map(|b| (whole.then_some(g.face), b))).collect();
                        (Err(("not read", e)), None, unread, None)
                    }
                    Ok(def) => {
                        let compiled = parts(&def).into_iter().map(|(part, d)| (part, d.compiled_abilities())).collect();
                        let blockers = if all_blockers { blockers(&def).into_iter().map(|b| (None, b)).collect() } else { Vec::new() };
                        let verdict = check(&def).map_err(|u| ("not run", u.0));
                        // a preparation card's inset spell too (722.2)
                        let dumped = (program_dump.is_some() && verdict.is_ok()).then(|| serde_json::Value::Object(
                            parts(&def).into_iter().chain(def.prepare_spell().map(|d| ("prepare".to_string(), d)))
                                .map(|(part, d)| (part, d.program_dump())).collect()));
                        (verdict, Some(compiled), blockers, dumped)
                    }
                }
            })
            .collect();
        let mut columns = (Vec::with_capacity(rows.len()), Vec::with_capacity(rows.len()), Vec::with_capacity(rows.len()), Vec::with_capacity(rows.len()));
        for (verdict, compiled, blockers, dumped) in rows {
            columns.0.push(verdict);
            columns.1.push(compiled);
            columns.2.push(blockers);
            columns.3.push(dumped);
        }
        columns
    };
    if let Some(path) = program_dump {
        match write_programs(&path, &faces, &programs) {
            Ok(summary) => println!("{summary}"),
            Err(error) => {
                eprintln!("cannot write program dump: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    // each refused face's blockers, the first its refusal: else the collecting checks disagree
    // with the ones admission consults, and the report is not one of this admission
    let mut disagree = 0;
    let listed: Vec<Listed> = faces.iter().zip(&verdicts).zip(all).map(|((f, verdict), found)| {
        listed(verdict, found, all_blockers).unwrap_or_else(|why| {
            eprintln!("blockers disagree with admission for {} (face {}): {why}", f.face_name, f.face);
            disagree += 1;
            Listed::default()
        })
    }).collect();
    if let Some(path) = flag("--admission-snapshot") {
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            use std::io::Write;
            let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
            for (((f, verdict), compiled), listed) in faces.iter().zip(&verdicts).zip(&routes).zip(&listed) {
                let (stage, reason) = match verdict {
                    Ok(()) => ("admitted", None),
                    Err(("not read", reason)) => ("parse", Some(reason)),
                    Err((_, reason)) => ("engine", Some(reason)),
                };
                let mut row = serde_json::json!({
                    "oracle_id": f.oracle_id, "face": f.face, "name": f.face_name,
                    "layout": f.layout, "stage": stage, "reason": reason,
                    "compiled_abilities": compiled,
                });
                if all_blockers {
                    let blockers = listed.blockers.iter().map(|(face, b)| {
                        let mut v = serde_json::to_value(b)?;
                        v["label"] = b.label().into();
                        if let Some(face) = face { v["face"] = (*face).into(); }
                        Ok(v)
                    }).collect::<Result<Vec<_>, serde_json::Error>>()?;
                    let fully = listed.unanalyzed.is_empty() && listed.blockers.iter().all(|(_, b)| b.unanalyzed.is_empty());
                    row["blockers"] = blockers.into();
                    row["unanalyzed"] = listed.unanalyzed.clone().into();
                    row["fully_analyzed"] = fully.into();
                }
                serde_json::to_writer(&mut out, &row)?;
                writeln!(out)?;
            }
            out.flush()?;
            Ok(())
        };
        if let Err(error) = write() {
            eprintln!("cannot write admission snapshot: {error}");
            return ExitCode::FAILURE;
        }
    }
    // the single-faced cards the engine runs, one per line, for scripts/compare.py. Not the
    // melded face of a meld pair, which is no card of its own (712.4b: it has no mana cost
    // and is no land, and stands alone in the corpus)
    if args.iter().any(|a| a == "--list") {
        for (f, v) in faces.iter().zip(&verdicts) {
            let melded = f.layout == "meld" && f.mana_cost.as_deref().unwrap_or("").is_empty() && !f.type_line.contains("Land");
            if v.is_ok() && f.face_name == f.name && !melded {
                println!("RUN\t{}\t{}\t{}", f.face_name, f.mana_cost.as_deref().unwrap_or(""), f.type_line);
            }
        }
        return ExitCode::SUCCESS;
    }
    let total = faces.len();
    let run = verdicts.iter().filter(|v| v.is_ok()).count();
    let read = verdicts.iter().filter(|v| !matches!(v, Err(("not read", _)))).count();
    println!("faces: {total}");
    println!("  read by the grammar: {read} ({:.1}%)", 100.0 * read as f64 / total as f64);
    println!("  run by the engine:   {run} ({:.1}%)", 100.0 * run as f64 / total as f64);
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for (f, v) in faces.iter().zip(&verdicts) {
        if v.is_ok() {
            let kind = f.type_line.split('—').next().unwrap_or("").trim().to_string();
            *by_kind.entry(kind).or_default() += 1;
        }
    }
    let mut kinds: Vec<_> = by_kind.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nrun, by type line:");
    for (k, n) in kinds.iter().take(8) {
        println!("  {n:6}  {k}");
    }
    let mut reasons: BTreeMap<String, (usize, Vec<&str>)> = BTreeMap::new();
    for (f, v) in faces.iter().zip(&verdicts) {
        if let Err(("not run", why)) = v {
            let r = reasons.entry(why.clone()).or_default();
            r.0 += 1;
            if r.1.len() < limit {
                r.1.push(&f.face_name);
            }
        }
    }
    let mut ranked: Vec<_> = reasons.into_iter().collect();
    ranked.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!("\nread but not run, by what the engine stopped at (top 30):");
    for (why, (n, ex)) in ranked.iter().take(30) {
        println!("  {n:6}  {why}   e.g. {}", ex.iter().take(3).copied().collect::<Vec<_>>().join(", "));
    }
    // of the faces the grammar does not read, the first thing it stopped at: the reason, and
    // of a row, the token (as grammar-gate ranks declines)
    let mut unread: BTreeMap<String, (usize, Vec<&str>)> = BTreeMap::new();
    for ((f, v), listed) in faces.iter().zip(&verdicts).zip(&listed) {
        if let (Err(("not read", _)), Some((_, first))) = (v, listed.blockers.first()) {
            let r = unread.entry(first.label()).or_default();
            r.0 += 1;
            if r.1.len() < limit { r.1.push(&f.face_name); }
        }
    }
    let mut unread: Vec<_> = unread.into_iter().collect();
    unread.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
    println!("\nnot read, by where the grammar first stopped (top 30):");
    for (why, (n, ex)) in unread.iter().take(30) {
        println!("  {n:6}  {why}   e.g. {}", ex.iter().take(3).copied().collect::<Vec<_>>().join(", "));
    }
    if let Some(want) = examples_for {
        println!("\nexamples of {want:?}:");
        for (why, (n, ex)) in &ranked {
            if why.contains(&want) {
                println!("  {n:6}  {why}: {}", ex.join(", "));
            }
        }
    }
    // the faces the engine runs whose text says this: real cards to test a concept with
    if let Some(text) = flag("--runs") {
        println!("\nrun, with {text:?} in the text:");
        let said = faces.iter().zip(&verdicts).filter(|(f, v)| v.is_ok() && f.oracle_text.contains(&text));
        for (f, _) in said.take(limit) {
            println!("  {} | {} | {} | {}", f.face_name, f.mana_cost.as_deref().unwrap_or(""), f.type_line, f.oracle_text.replace('\n', " / "));
        }
    }
    let (deck_cards, deck_read, deck_run) = decks(&faces, &verdicts, flag("--deck").as_deref());
    // one summary line for reporting tools
    println!("\nSUMMARY faces={total} read={read} run={run} deck_cards={deck_cards} deck_read={deck_read} deck_run={deck_run}");
    if disagree > 0 {
        eprintln!("{disagree} faces whose blockers disagree with their admission");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

type Verdict = Result<(), (&'static str, String)>;

/// The definitions a card is read as, by name: the whole card, each half of a split card,
/// its adventure or omen spell.
fn parts(def: &CardDef) -> Vec<(String, &CardDef)> {
    let mut parts = vec![("whole".to_string(), def)];
    parts.extend(def.halves().iter().enumerate().map(|(i, half)| (format!("half:{i}"), half)));
    parts.extend(def.adventure().map(|d| ("adventure".to_string(), d)));
    parts.extend(def.omen().map(|d| ("omen".to_string(), d)));
    parts
}

/// Write the admitted faces' compiled programs, sorted by face, and check each identity
/// against its canonical text: the summary line, or what is wrong.
fn write_programs(path: &str, faces: &[corpus::Face], programs: &[Option<serde_json::Value>]) -> Result<String, Box<dyn std::error::Error>> {
    use game::program::{ProgramId, digest};
    let mut rows: Vec<_> = faces.iter().zip(programs).filter_map(|(f, parts)| Some((f, parts.as_ref()?))).collect();
    rows.sort_by(|(a, _), (b, _)| (&a.oracle_id, a.face).cmp(&(&b.oracle_id, b.face)));
    let mut text = String::new();
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut count = 0;
    for (f, parts) in &rows {
        let row = serde_json::json!({"oracle_id": f.oracle_id, "face": f.face, "name": f.face_name, "layout": f.layout, "parts": parts});
        text += &serde_json::to_string(&row)?;
        text.push('\n');
        // every program entry: {id, canonical}, wherever it is nested
        let mut stack: Vec<&serde_json::Value> = vec![parts];
        while let Some(value) = stack.pop() {
            match value {
                serde_json::Value::Object(map) => {
                    if let (Some(serde_json::Value::String(id)), Some(serde_json::Value::String(canonical))) = (map.get("id"), map.get("canonical")) {
                        count += 1;
                        if ProgramId::of_canonical(canonical).to_string() != *id {
                            return Err(format!("{}: program {id} is not the digest of its canonical text", f.face_name).into());
                        }
                        match seen.get(id.as_str()) {
                            Some(other) if other != canonical => return Err(format!("{}: two canonical texts share program {id}", f.face_name).into()),
                            _ => { seen.insert(id.clone(), canonical); }
                        }
                    }
                    stack.extend(map.values());
                }
                serde_json::Value::Array(items) => stack.extend(items),
                _ => {}
            }
        }
    }
    std::fs::write(path, &text)?;
    Ok(format!("PROGRAMS faces={} programs={count} distinct={} digest={:016x}", rows.len(), seen.len(), digest(text.as_bytes())))
}

/// A refused face's blockers as reports list them, each with the face it is of when the
/// card is read whole, and what was left unanalyzed of the face as a whole.
#[derive(Default)]
struct Listed {
    blockers: Vec<(Option<u8>, Blocker)>,
    unanalyzed: Vec<&'static str>,
}

/// The listed blockers of a face from what the collecting checks `found`: its admission's
/// refusal first. A card read whole may be refused for its layout or the joining of its faces,
/// before or without any face's own reading refusing: that is its first blocker. Of the engine
/// stage, the blockers are found only `collected` (--all-blockers). Err: they disagree.
fn listed(verdict: &Verdict, mut found: Vec<(Option<u8>, Blocker)>, collected: bool) -> Result<Listed, String> {
    let first = |found: &[(Option<u8>, Blocker)]| found.first().map(|(_, b)| b.reason.clone());
    match verdict {
        Ok(()) if found.is_empty() => Ok(Listed::default()),
        Ok(()) => Err(format!("admitted, but blocked by {:?}", first(&found))),
        Err(("not read", reason)) => {
            if first(&found).as_ref() != Some(reason) {
                if found.iter().any(|(_, b)| &b.reason == reason) { return Err(format!("read refused by {reason:?}, met after {:?}", first(&found))); }
                found.insert(0, (None, Blocker::parse(reason.clone())));
            }
            // lowering and the engine's checks need every ability of the card read
            Ok(Listed { blockers: found, unanalyzed: vec!["engine admission: not analyzed, as the grammar does not read the whole card"] })
        }
        Err((_, _)) if !collected => Ok(Listed::default()),
        Err((_, reason)) if first(&found).as_ref() == Some(reason) => Ok(Listed { blockers: found, unanalyzed: Vec::new() }),
        Err((_, reason)) => Err(format!("refused by {reason:?}, but the first blocker found is {:?}", first(&found))),
    }
}

/// The decks of `decks/*.dck`: how many of each one's cards
/// the grammar reads and the engine runs. A deck is playable when all of them run; that, not
/// the count of faces, is the milestone. `detail`: a deck to list card by card. Returns the
/// distinct cards over all decks: how many, read, run.
fn decks(faces: &[corpus::Face], verdicts: &[Verdict], detail: Option<&str>) -> (usize, usize, usize) {
    let dir = corpus::root();
    if !dir.join("decks").is_dir() {
        eprintln!("no decks directory under {}", dir.display());
        return (0, 0, 0);
    }
    // a card by its name, or one face of it by its own: every face must be read and run
    let mut by_name: BTreeMap<&str, Vec<&Verdict>> = BTreeMap::new();
    for (f, v) in faces.iter().zip(verdicts) {
        by_name.entry(f.name.as_str()).or_default().push(v);
        if f.face_name != f.name {
            by_name.entry(f.face_name.as_str()).or_default().push(v);
        }
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir.join("decks")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    paths.retain(|p| p.extension().is_some_and(|x| x == "dck"));
    paths.sort();
    println!("\ndecks: distinct cards, read by the grammar, run by the engine (copies run of copies)");
    let mut all: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for path in paths {
        let deck = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // "4 [C14:204] Llanowar Elves", "SB: 2 [M14:171] Duress"
        let mut cards: BTreeMap<String, usize> = BTreeMap::new();
        for line in text.lines() {
            let Some((head, name)) = line.split_once("] ") else { continue };
            let Some(copies) = head.trim_start_matches("SB:").split_whitespace().next().and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0) else { continue };
            *cards.entry(name.trim().to_string()).or_default() += copies;
        }
        let (mut read, mut run, mut copies, mut copies_run) = (0, 0, 0, 0);
        let mut stops = Vec::new();
        for (name, n) in &cards {
            let v = by_name.get(name.as_str());
            let is_read = v.is_some_and(|v| v.iter().all(|v| !matches!(v, Err(("not read", _)))));
            let is_run = v.is_some_and(|v| v.iter().all(|v| v.is_ok()));
            read += is_read as usize;
            run += is_run as usize;
            copies += n;
            copies_run += if is_run { *n } else { 0 };
            all.insert(name.clone(), (is_read, is_run));
            if !is_run {
                let why = match v.and_then(|v| v.iter().find_map(|v| v.as_ref().err())) {
                    Some((stage, why)) => format!("{stage}: {why}"),
                    None => "not in the corpus".to_string(),
                };
                stops.push(format!("{name}: {why}"));
            }
        }
        println!("  {deck:16} {:3} {read:3} {run:3}   ({copies_run} of {copies})", cards.len());
        if detail == Some(deck.as_str()) {
            stops.iter().for_each(|s| println!("      {s}"));
        }
    }
    let (read, run) = (all.values().filter(|v| v.0).count(), all.values().filter(|v| v.1).count());
    println!("  {:16} {:3} {read:3} {run:3}", "all, distinct", all.len());
    (all.len(), read, run)
}
