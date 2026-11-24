//! The surface lexicon: how closed vocabularies are spelled, in both numbers.
//!
//! Plurals are listed where English is irregular and derived otherwise; the corpus check in
//! `grammar-gate` reports capitalized words that look like an unlisted plural.

use crate::kernel::{P, alt, leaf, table};
use ir::selector::AbilityKind;
use ir::{Amount, Atom, CardType, Color, Filter, Status, Subtype, Supertype};
use oracle::token::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Number {
    Sg,
    Pl,
}

/// Subtypes whose plural is irregular.
const IRREGULAR: &[(&str, &str)] = &[
    ("Elf", "Elves"),
    ("Dwarf", "Dwarves"),
    ("Wolf", "Wolves"),
    ("Werewolf", "Werewolves"),
    ("Thief", "Thieves"),
    ("Mouse", "Mice"),
    ("Ox", "Oxen"),
    ("Child", "Children"),
    ("Cyclops", "Cyclopes"),
    ("Fungus", "Fungi"),
    ("Homunculus", "Homunculi"),
    ("Locus", "Loci"),
    ("Goose", "Geese"),
    ("Sorcery", "Sorceries"),
    ("Hero", "Heroes"),
    ("Pegasus", "Pegasi"),
];

/// Subtypes spelled the same in both numbers that no English ending tells: borrowed words, a mass
/// noun, a word already plural ("Other Kavu you control get +1/+1").
const INVARIANT: &[&str] = &["Arcane", "Eldrazi", "Kavu", "Kithkin", "Myr", "Nephilim", "Samurai", "Zubera", "Equipment", "Plains"];

/// English nouns spelled the same in both numbers, and the compounds ending in them: "Merfolk",
/// "Treefolk", "Moonfolk"; "Spacecraft"; "Fish", "Jellyfish"; "Sheep"; "Bison"; "Aetherborn",
/// "Graveborn".
const INVARIANT_HEADS: &[&str] = &["folk", "craft", "fish", "sheep", "bison", "born"];

pub fn plural(word: &str) -> String {
    if let Some((_, p)) = IRREGULAR.iter().find(|(s, _)| *s == word) {
        return p.to_string();
    }
    let lower = word.to_lowercase();
    if INVARIANT.contains(&word) || INVARIANT_HEADS.iter().any(|h| lower.ends_with(h)) {
        return word.to_string();
    }
    if lower.ends_with('s')
        || lower.ends_with('x')
        || lower.ends_with("sh")
        || lower.ends_with("ch")
        || lower.ends_with('z')
    {
        format!("{word}es")
    } else if lower.ends_with('y') && !lower[..lower.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']) {
        format!("{}ies", &word[..word.len() - 1])
    } else {
        format!("{word}s")
    }
}

pub fn inflect(word: &str, n: Number) -> String {
    match n {
        Number::Sg => word.to_string(),
        Number::Pl => plural(word),
    }
}

/// A card type or subtype as a noun: "creature", "Goblins", "Equipment", "sorceries".
/// `permanent`: also "permanent" as a type word, which only stands before "card" or "spell"
/// ("permanent card", CR 110.4b); elsewhere "permanent" is the head noun.
pub fn type_word(n: Number, permanent: bool) -> P<Atom> {
    let mut entries: Vec<(String, Atom)> = Vec::new();
    if permanent {
        entries.push((inflect("permanent", n), Atom::PermanentType));
    }
    for t in CardType::ALL {
        entries.push((inflect(t.word(), n), Atom::Type(*t)));
    }
    for s in Subtype::all() {
        entries.push((inflect(s.name(), n), Atom::Subtype(s)));
    }
    table(if n == Number::Sg { "type" } else { "types" }, entries)
}

/// Prenominal adjectives: colors, supertypes, statuses, and their "non" negations.
pub fn adjective() -> P<Filter> {
    let mut e: Vec<(String, Filter)> = Vec::new();
    let atom = |a: Atom| Filter::Atom(a);
    let not = |a: Atom| Filter::Not(Box::new(Filter::Atom(a)));
    for c in Color::ALL {
        e.push((c.word().into(), atom(Atom::Color(*c))));
        e.push((format!("non{}", c.word()), not(Atom::Color(*c))));
    }
    e.push(("colorless".into(), atom(Atom::Colorless)));
    e.push(("multicolored".into(), atom(Atom::Multicolored)));
    e.push(("monocolored".into(), atom(Atom::Monocolored)));
    for s in Supertype::ALL {
        e.push((s.word().into(), atom(Atom::Supertype(*s))));
        e.push((format!("non{}", s.word()), not(Atom::Supertype(*s))));
    }
    for t in CardType::ALL {
        e.push((format!("non{}", t.word()), not(Atom::Type(*t))));
    }
    for s in Subtype::all() {
        e.push((format!("non-{}", s.name()), not(Atom::Subtype(s))));
    }
    for (w, s) in STATUS_WORDS {
        e.push((w.to_string(), atom(Atom::Status(*s))));
    }
    e.push(("nontoken".into(), not(Atom::Status(Status::Token))));
    e.push(("nonattacking".into(), not(Atom::Status(Status::Attacking))));
    // "an unpaired creature": one not paired by soulbond (702.95b)
    e.push(("unpaired".into(), not(Atom::Status(Status::Paired))));
    e.push(("non-outlaw".into(), not(Atom::Status(Status::Outlaw))));
    // "commander creatures you own" (before a type; "a commander" alone is the head noun)
    e.push(("commander".into(), atom(Atom::Status(Status::Commander))));
    // only before "ability" (checked where the noun phrase is assembled)
    for (w, k) in [
        ("activated", AbilityKind::Activated),
        ("triggered", AbilityKind::Triggered),
        ("mana", AbilityKind::Mana),
        ("loyalty", AbilityKind::Loyalty),
    ] {
        e.push((w.into(), atom(Atom::AbilityKind(k))));
    }
    table("adjective", e)
}

pub const STATUS_WORDS: &[(&str, Status)] = &[
    ("tapped", Status::Tapped),
    ("untapped", Status::Untapped),
    ("attacking", Status::Attacking),
    ("blocking", Status::Blocking),
    ("blocked", Status::Blocked),
    ("unblocked", Status::Unblocked),
    ("face-down", Status::FaceDown),
    ("face-up", Status::FaceUp),
    ("modified", Status::Modified),
    ("historic", Status::Historic),
    ("outlaw", Status::Outlaw),
    ("enchanted", Status::Enchanted),
    ("equipped", Status::Equipped),
    ("suspected", Status::Suspected),
    ("saddled", Status::Saddled),
    ("goaded", Status::Goaded),
    ("transformed", Status::Transformed),
];

const NUMBER_WORDS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

pub fn digits() -> P<u32> {
    leaf(
        "digits",
        |k| match k {
            Kind::Number { value, .. } => Some(*value),
            _ => None,
        },
        |n| Some(Kind::Number { value: *n, text: n.to_string() }),
    )
}

pub fn number_words() -> P<u32> {
    table("number word", NUMBER_WORDS.iter().enumerate().map(|(i, w)| (w.to_string(), i as u32)).collect())
}

fn x() -> P<Amount> {
    crate::kernel::lit("X").to(Amount::X)
}

/// An amount as counts are spelled: words ("two target creatures"), digits as an alias; also
/// arithmetic on one ([`quantity`]): "twice X", "three times X", "X plus one", "half X".
pub fn count_amount() -> P<Amount> {
    quantity(&count_atom())
}

/// The quantity of a mass noun as stats are spelled ("3 life", "twice X damage", "X plus 1").
pub fn stat_quantity() -> P<Amount> {
    quantity(&stat_amount())
}

/// Does the amount depend on something the text does not fix? Arithmetic on literals alone
/// ("two plus one") is just a number spelled in a strange way, so it is not read.
fn varies(a: &Amount) -> bool {
    match a {
        Amount::X | Amount::That { .. } => true,
        Amount::Multiple(_, x) | Amount::Half { of: x, .. } => varies(x),
        Amount::Plus(x, y) | Amount::Minus(x, y) => varies(x) || varies(y),
        _ => false,
    }
}

/// Is the amount spelled as a quantity before its noun ([`count_amount`]), as opposed to after
/// "equal to"? A number, X, "that many", or arithmetic on those.
pub fn is_quantity(a: &Amount) -> bool {
    let atom = |a: &Amount| matches!(a, Amount::Const(_) | Amount::X | Amount::That { .. });
    match a {
        Amount::Multiple(_, x) | Amount::Half { of: x, round_up: None } => atom(x),
        Amount::Plus(x, y) | Amount::Minus(x, y) => atom(x) && atom(y),
        a => atom(a),
    }
}

/// A number of things, or arithmetic on one (CR 107.1): "twice X", "three times X", "half X",
/// "X plus 1", "5 minus X". `atom` is the plain number as the slot spells it. The arithmetic
/// wraps a number that varies; "that many" takes its "plus" after the noun, not here ("draw that
/// many cards plus one").
pub fn quantity(atom: &P<Amount>) -> P<Amount> {
    let twice = crate::kernel::lit("twice").then_keep(atom).map(
        |a| varies(&a).then(|| Amount::Multiple(2, Box::new(a))),
        |a| match a {
            Amount::Multiple(2, x) if varies(x) => Some((**x).clone()),
            _ => None,
        },
    );
    let times = number_words().skip(&crate::kernel::lit("times")).then(atom).map(
        |(n, a)| (n >= 3 && varies(&a)).then(|| Amount::Multiple(n, Box::new(a))),
        |a| match a {
            Amount::Multiple(n, x) if *n >= 3 && varies(x) => Some((*n, (**x).clone())),
            _ => None,
        },
    );
    let half = crate::kernel::lit("half").then_keep(atom).map(
        |a| varies(&a).then(|| Amount::Half { of: Box::new(a), round_up: None }),
        |a| match a {
            Amount::Half { of, round_up: None } if varies(of) => Some((**of).clone()),
            _ => None,
        },
    );
    let term = alt("quantity", vec![atom.clone(), twice, times, half]);
    let op = alt(
        "plus or minus",
        vec![crate::kernel::lit("plus").to(true), crate::kernel::lit("minus").to(false)],
    );
    let counted = |a: &Amount, b: &Amount| {
        (varies(a) || varies(b)) && !matches!(a, Amount::That { .. }) && *b != Amount::Const(0)
    };
    let tail = op.then(atom).opt();
    term.then(&tail).map(
        move |(first, rest)| match rest {
            None => Some(first),
            Some((plus, b)) => {
                if !counted(&first, &b) {
                    return None;
                }
                let (x, y) = (Box::new(first), Box::new(b));
                Some(if plus { Amount::Plus(x, y) } else { Amount::Minus(x, y) })
            }
        },
        move |a| match a {
            Amount::Plus(x, y) if counted(x, y) => Some(((**x).clone(), Some((true, (**y).clone())))),
            Amount::Minus(x, y) if counted(x, y) => Some(((**x).clone(), Some((false, (**y).clone())))),
            Amount::Plus(..) | Amount::Minus(..) => None,
            other => Some((other.clone(), None)),
        },
    )
}

/// A plain number of things: no arithmetic.
pub fn count_atom() -> P<Amount> {
    alt(
        "count",
        vec![
            number_words().map(
                |n| Some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) => Some(*n),
                    _ => None,
                },
            ),
            x(),
            crate::kernel::lit("that many").to(Amount::That { many: true }),
            // digits read anywhere; they print only past the number words ("200 or more cards")
            digits().map(
                |n| Some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) if *n as usize >= NUMBER_WORDS.len() => Some(*n),
                    _ => None,
                },
            ),
        ],
    )
}

/// An amount as stats are spelled: digits ("power 2 or less"), words as an alias.
pub fn stat_amount() -> P<Amount> {
    alt(
        "number",
        vec![
            digits().map(
                |n| Some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) => Some(*n),
                    _ => None,
                },
            ),
            x(),
            number_words()
                .map(
                    |n| Some(Amount::Const(n)),
                    |a| match a {
                        Amount::Const(n) => Some(*n),
                        _ => None,
                    },
                )
                .alias(),
        ],
    )
}

/// How text names the object it is on: `~`, the object itself, or `§`, the object that granted
/// the quoted ability the text is in (CR 201.5, 201.5a). Both are the same kind of reference to
/// an object by its name, so every place that takes one takes the other.
pub fn own_reference() -> P<ir::Ref> {
    let this = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef));
    let granting = leaf("§", |k| matches!(k, Kind::GrantingSelf).then_some(()), |()| Some(Kind::GrantingSelf));
    alt("own reference", vec![this.to(ir::Ref::This), granting.to(ir::Ref::Granting)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals() {
        assert_eq!(plural("Elf"), "Elves");
        assert_eq!(plural("creature"), "creatures");
        assert_eq!(plural("sorcery"), "sorceries");
        assert_eq!(plural("Fox"), "Foxes");
        assert_eq!(plural("Ally"), "Allies");
        assert_eq!(plural("Monkey"), "Monkeys");
        assert_eq!(plural("Merfolk"), "Merfolk");
        assert_eq!(plural("Sphinx"), "Sphinxes");
    }

    /// CR 205.3: a subtype is a noun of either number; one whose English head is spelled the same
    /// in both ("Spacecraft", "Moonfolk", "Sheep") reads bare where only a plural fits, and takes
    /// no regular plural.
    #[test]
    fn r205_3_subtypes_with_invariant_heads() {
        use crate::kernel::Outcome;
        let s = &crate::effect::effects().sentence;
        let reads = |t: &str| matches!(s.parse_all(&oracle::token::tokenize(t)), Outcome::Accepted { .. });
        for w in ["Spacecraft", "Moonfolk", "Sheep", "Jellyfish", "Merfolk", "Fish", "Aetherborn", "Kavu"] {
            assert_eq!(plural(w), w);
            assert!(reads(&format!("Exile all {w}")), "Exile all {w}");
            assert!(reads(&format!("Exile target {w}")), "Exile target {w}");
            assert!(!reads(&format!("Exile all {w}s")), "Exile all {w}s");
        }
        assert!(reads("Exile all Goblins") && !reads("Exile all Goblin"));
    }
}
