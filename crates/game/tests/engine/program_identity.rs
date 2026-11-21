//! A compiled program's identity ([`ProgramId`]) is the digest of its canonical form: one text
//! compiles to one identity in every process, whatever else was compiled first and in whatever
//! order, and a program compiled differently has another. Which printed ability an occurrence
//! came from is still told by its compiled instance, not by equal content.
use crate::support::cite;
use game::program::ProgramId;
use game::simulation::Simulation;
use game::{CardDef, Game, Move, Zone};
use serde_json::Value;
use std::process::{Command, Stdio};

const CASE: &str = "KARN_PROGRAM_IDENTITY_CASE";
const TEST: &str = "engine::program_identity::program_identity_is_the_same_in_every_process_and_compile_order";

/// Plain, modal, delayed (603.7), reflexive (603.12) and replacement-reflexive programs.
const FIXTURES: &[(&str, &str, &str, &str, Option<(i32, i32)>)] = &[
    ("Banish", "{0}", "Instant", "Exile target creature. At the beginning of the next end step, return that card to the battlefield under its owner's control with a +1/+1 counter on it.", None),
    ("Bargain", "{0}", "Sorcery", "Sacrifice two creatures. When you do, you gain 3 life.", None),
    ("Head of the Hunt", "{2}{B}{B}", "Creature — Wolf", "Flash\nIf a creature an opponent controls would die, exile it instead. When you do, create a 2/2 green Wolf creature token.", Some((4, 3))),
    ("Lore", "{2}{B}{B}", "Sorcery", "Choose one —\n• You draw three cards.\n• You gain 3 life.", None),
    ("Mend", "{0}", "Instant", "You gain 3 life.", None),
    ("Twin", "{0}", "Enchantment", "Whenever you cast a creature spell, you gain 1 life.\nWhenever you cast an instant spell, you gain 1 life.", None),
];

fn define((name, cost, types, text, pt): (&str, &str, &str, &str, Option<(i32, i32)>)) -> CardDef {
    let def = CardDef::new(name, cost, types, text, pt).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    def
}

/// The distinct main-deck cards of the first roster decks (`resources/decks/selected-decks.jsonl`)
/// that the engine runs, by name: real Oracle text compiled to many kinds of program.
fn deck_cards() -> &'static [Vec<&'static oracle::corpus::Face>] {
    static FACES: std::sync::OnceLock<Vec<oracle::corpus::Face>> = std::sync::OnceLock::new();
    static CARDS: std::sync::OnceLock<Vec<Vec<&'static oracle::corpus::Face>>> = std::sync::OnceLock::new();
    CARDS.get_or_init(|| {
        let catalog = CardDef::catalog(FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap()));
        let roster = std::fs::read_to_string(oracle::corpus::root().join("resources/decks/selected-decks.jsonl")).unwrap();
        let mut names = std::collections::BTreeSet::new();
        for line in roster.lines().take(60) {
            let deck: Value = serde_json::from_str(line).unwrap();
            names.extend(deck["main"].as_object().unwrap().keys().cloned());
        }
        // a name the catalog does not hold (a source spelling under review) is no deck card here
        names.iter().filter_map(|name| catalog.get(name.as_str()).cloned())
            .filter(|faces| CardDef::from_faces(faces).is_ok_and(|def| game::play::check(&def).is_ok())).collect()
    })
}

/// A deck card's definition, read from its Oracle faces.
fn read(faces: &[&oracle::corpus::Face]) -> CardDef { CardDef::from_faces(faces).unwrap() }

/// One line per definition, by name: what it compiled (`CardDef::program_dump`).
fn dump(defs: &[CardDef]) -> String {
    let mut lines: Vec<_> = defs.iter().map(|d| format!("{}\t{}", d.name(), d.program_dump())).collect();
    lines.sort();
    lines.join("\n")
}

/// Every program entry of a dump, wherever it is nested: (id, canonical).
fn programs(value: &Value, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            if let (Some(Value::String(id)), Some(Value::String(canonical))) = (map.get("id"), map.get("canonical")) {
                out.push((id.clone(), canonical.clone()));
            }
            map.values().for_each(|v| programs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| programs(v, out)),
        _ => {}
    }
}

/// The plain body's first program of an ability.
fn plain(def: &CardDef, ability: usize) -> String {
    let dumped = def.program_dump();
    let row = dumped["abilities"].as_array().unwrap().iter().find(|a| a["ability"] == ability).expect("an executable ability");
    row["body"]["plain"][0]["id"].as_str().expect("a plain body").to_string()
}

#[test]
fn program_identity_is_the_same_in_every_process_and_compile_order() {
    if let Ok(case) = std::env::var(CASE) {
        let defs: Vec<CardDef> = match case.as_str() {
            "forward" => FIXTURES.iter().copied().map(define).chain(deck_cards().iter().map(|faces| read(faces))).collect(),
            // other programs first, then the deck cards and the fixtures in reverse on threads
            // of their own
            "reverse" => {
                for text in ["Draw two cards.", "You lose 2 life.", "Choose one —\n• You gain 1 life.\n• Draw a card."] {
                    define(("Other", "{0}", "Sorcery", text, None));
                }
                let decks: Vec<_> = deck_cards().iter().rev().map(|faces| std::thread::spawn(move || read(faces))).collect();
                let threads: Vec<_> = FIXTURES.iter().rev().copied().map(|f| std::thread::spawn(move || define(f))).collect();
                decks.into_iter().chain(threads).map(|t| t.join().unwrap()).collect()
            }
            _ => panic!("unknown compile order"),
        };
        for line in dump(&defs).lines() { println!("PROGRAMS\t{line}"); }
        return;
    }
    let run = |case: &str| -> String {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env(CASE, case).stdout(Stdio::piped()).stderr(Stdio::piped()).output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(output.status.success() && stdout.contains("test result: ok. 1 passed;"),
            "{case}: {stdout}{}", String::from_utf8_lossy(&output.stderr));
        // libtest writes "test … ... " before the first line the test prints
        stdout.lines().filter_map(|l| l.split_once("PROGRAMS\t").map(|(_, line)| line)).collect::<Vec<_>>().join("\n")
    };
    let (forward, reverse) = (run("forward"), run("reverse"));
    // the roster's first decks are well over a hundred distinct cards the engine runs
    assert!(deck_cards().len() > 100, "{} deck cards", deck_cards().len());
    assert_eq!(forward.lines().count(), FIXTURES.len() + deck_cards().len());
    assert_eq!(forward, reverse, "two processes compiling in different orders dump different programs");
    // and this process, which compiled other definitions of its own before
    let here: Vec<CardDef> = FIXTURES.iter().copied().map(define).collect();
    let decks: Vec<CardDef> = deck_cards().iter().map(|faces| read(faces)).collect();
    assert_eq!(dump(&[here.clone(), decks].concat()), forward);
    let mut entries = Vec::new();
    for line in forward.lines() {
        let (_, json) = line.split_once('\t').unwrap();
        programs(&serde_json::from_str(json).unwrap(), &mut entries);
    }
    for (id, canonical) in &entries {
        assert_eq!(&ProgramId::of_canonical(canonical).to_string(), id, "an identity is the digest of its canonical text");
    }
    // each kind of program is among them: a mode, a delayed and a reflexive one, a replacement's
    let at = |name: &str| -> Value { here.iter().find(|d| d.name() == name).unwrap().program_dump() };
    assert_eq!(at("Lore")["abilities"][0]["body"]["modes"]["programs"].as_array().unwrap().len(), 2);
    assert_eq!(at("Banish")["abilities"][0]["body"]["plain"].as_array().unwrap().len(), 2, "the spell's and the delayed ability's");
    assert_eq!(at("Bargain")["abilities"][0]["body"]["plain"].as_array().unwrap().len(), 2, "the spell's and the reflexive ability's");
    assert_eq!(at("Head of the Hunt")["replacements"][0]["reflexive"].as_array().unwrap().len(), 1);
}

#[test]
fn a_program_compiled_differently_has_another_identity() {
    let mend = define(FIXTURES[4]);
    let more = define(("Mend", "{0}", "Instant", "You gain 4 life.", None));
    assert_ne!(plain(&mend, 0), plain(&more, 0));
    // the same text compiles to the same program, whatever the card that prints it is called
    let again = define(("Salve", "{1}", "Instant", "You gain 3 life.", None));
    assert_eq!(plain(&mend, 0), plain(&again, 0));
    // so does the delayed ability a spell makes, nested in the program that makes it
    let banish = define(FIXTURES[0]);
    let twice = define(("Banish", "{0}", "Instant", "Exile target creature. At the beginning of the next end step, return that card to the battlefield under its owner's control with two +1/+1 counters on it.", None));
    let ids = |def: &CardDef| { let mut out = Vec::new(); programs(&def.program_dump(), &mut out); out };
    let (once, two) = (ids(&banish), ids(&twice));
    assert_eq!((once.len(), two.len()), (2, 2));
    assert!(once[0].0 != two[0].0 && once[1].0 != two[1].0, "a changed delayed ability changes it and the program that makes it");
}

#[test]
fn r603_3_two_abilities_compiled_alike_are_each_put_on_the_stack_with_their_own_text() {
    let twin = define(FIXTURES[5]);
    // both effects are "you gain 1 life": equal programs, one identity
    assert_eq!(plain(&twin, 0), plain(&twin, 1));
    let spell = define(("Spark", "{0}", "Instant", "You lose 1 life.", None));
    let mut game = Game::new(vec![twin, spell], 2, 20);
    let twin = game.put(0, 0, Zone::Battlefield);
    let spark = game.put(0, 1, Zone::Hand);
    let mut s = Simulation::new(game);
    let id = s.request().unwrap().unwrap().id;
    s.answer(0, id, Move::Cast { card: spark, targets: vec![] }).unwrap();
    let prompt = s.prompt(0).unwrap();
    let top = *prompt.view.stack.last().unwrap();
    let on_stack = prompt.entities[&top].occurrence.as_ref().expect("the trigger is on the stack");
    // 603.2: casting an instant triggers only the second ability; 603.3: that one is put on
    // the stack, and it is what the stack shows, not the first ability that compiled alike
    cite!(["603.2", "603.3"]; assert_eq!((on_stack.source, on_stack.text.as_deref()),
        (Some(twin), Some("Whenever you cast an instant spell, you gain 1 life."))));
    assert_eq!(prompt.view.stack.len(), 2);
}
