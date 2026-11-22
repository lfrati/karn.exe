//! What a "choose" instruction picks (CR 608.2d): a value of a class ("a color", "two basic land
//! types", "a creature type other than Wall"), a number, one option of an explicit list ("Khans or
//! Dragons", "odd or even", CR 607.2f), objects, or several of these together.
//!
//! choice  := one ("and" one)*
//! one     := objects | value | number | options | modes
//! value   := ("a" | count) class ["other than" excluded ("or" excluded)*] ["at random"]
//! number  := "a number" ["between" amount "and" amount]
//! options := word ("," word)* "or" word ["at random"]
//!
//! A class with exclusions and a list of members are different things: "a color other than red" is
//! always a [`Choice::Value`], "black or red" always a [`Choice::Among`].

use crate::kernel::{P, alt, lit, table};
use crate::words::{Number, digits, number_words, plural};
use ir::action::{Exclusion, Options, ValueKind};
use ir::vocab::SubtypeKind;
use ir::{Amount, CardType, Choice, Color, Modes, ObjectSet, Objects, Subtype};

/// Does an exclusion belong to the class chosen from?
fn fits(kind: &ValueKind, except: &[Exclusion]) -> bool {
    except.iter().all(|e| match (kind, e) {
        (ValueKind::Color, Exclusion::Color(_)) => true,
        (ValueKind::CardType, Exclusion::CardType(_)) => true,
        (ValueKind::CreatureType, Exclusion::Subtype(s)) => s.kind() == SubtypeKind::Creature,
        (ValueKind::CardName(_), Exclusion::CardName(_)) => true,
        _ => false,
    })
}

/// "at random" after the choice (701.9b-style wording: the choice is made by chance).
fn at_random() -> P<bool> {
    lit("at random").opt().map(|o| Some(o.is_some()), |b| Some(b.then_some(())))
}

/// Finite or rules-defined value classes (105.1, 205.2a, 205.3): shared by choosing and iteration.
pub(crate) fn value_class(number: Number) -> P<ValueKind> {
    let classes = [
        ("color", ValueKind::Color),
        ("creature type", ValueKind::CreatureType),
        ("card type", ValueKind::CardType),
        ("basic land type", ValueKind::BasicLandType),
        ("land type", ValueKind::LandType),
        ("planeswalker type", ValueKind::PlaneswalkerType),
    ];
    table("value class", classes.into_iter().map(|(word, kind)| {
        (if number == Number::Pl { plural(word) } else { word.to_owned() }, kind)
    }).collect())
}

pub(crate) fn choice(objects: &P<Objects>, modes: &P<Modes>, set_sg: &P<ObjectSet>) -> P<Choice> {
    let article = alt("a", vec![lit("a"), lit("an").alias()]);
    let random = at_random();
    let singular = value_class(Number::Sg);
    let plurals = value_class(Number::Pl);
    let card_name = set_sg.skip(&lit("name")).map(|set| Some(ValueKind::CardName(set)), |k| match k {
        ValueKind::CardName(set) => Some(set.clone()),
        _ => None,
    });

    // "other than red", "other than creature or land", "other than Wall", "other than a basic land card name"
    let colors = table("color", Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let card_types = table("card type", CardType::ALL.iter().map(|t| (t.word().to_string(), *t)).collect());
    let subtypes = table("creature type", Subtype::all().filter(|s| s.kind() == SubtypeKind::Creature).map(|s| (s.name().to_string(), s)).collect());
    let named = article.then_keep(&card_name).map(
        |k| match k {
            ValueKind::CardName(set) => Some(Exclusion::CardName(set)),
            _ => None,
        },
        |e| match e {
            Exclusion::CardName(set) => Some(ValueKind::CardName(set.clone())),
            _ => None,
        },
    );
    let excluded = alt(
        "excluded",
        vec![
            colors.map(|c| Some(Exclusion::Color(c)), |e| match e { Exclusion::Color(c) => Some(*c), _ => None }),
            card_types.map(|t| Some(Exclusion::CardType(t)), |e| match e { Exclusion::CardType(t) => Some(*t), _ => None }),
            subtypes.map(|s| Some(Exclusion::Subtype(s)), |e| match e { Exclusion::Subtype(s) => Some(*s), _ => None }),
            named,
        ],
    );
    let one_excluded = excluded.map(|e| Some(vec![e]), |v: &Vec<Exclusion>| (v.len() == 1).then(|| v[0].clone()));
    let except = lit("other than").then_keep(&alt("exclusions", vec![one_excluded, excluded.list("or")])).opt().map(
        |o| Some(o.unwrap_or_default()),
        |v: &Vec<Exclusion>| Some((!v.is_empty()).then(|| v.clone())),
    );

    let one = article.then_keep(&alt("class", vec![singular, card_name.clone()])).then(&except).then(&random).map(
        |((kind, except), random)| fits(&kind, &except).then_some(Choice::Value { kind, count: Amount::Const(1), except, random }),
        |c| match c {
            Choice::Value { kind, count, except, random } if *count == Amount::Const(1) => Some(((kind.clone(), except.clone()), *random)),
            _ => None,
        },
    );
    // "choose any card name": the same choice as "a card name"
    let any_name = lit("any").then_keep(&card_name).then(&except).then(&random).map(
        |((kind, except), random)| fits(&kind, &except).then_some(Choice::Value { kind, count: Amount::Const(1), except, random }),
        |_| None,
    );
    let several = number_words().then(&plurals).then(&except).then(&random).map(
        |(((n, kind), except), random)| {
            (n >= 2 && fits(&kind, &except)).then_some(Choice::Value { kind, count: Amount::Const(n), except, random })
        },
        |c| match c {
            Choice::Value { kind, count: Amount::Const(n), except, random } if *n >= 2 => Some((((*n, kind.clone()), except.clone()), *random)),
            _ => None,
        },
    );

    // the bounds are printed as digits, "between 0 and 13"
    let bound = digits().map(|n| Some(Amount::Const(n)), |a| match a {
        Amount::Const(n) => Some(*n),
        _ => None,
    });
    let bounds = lit("between").then_keep(&bound).skip(&lit("and")).then(&bound).map(
        |(lo, hi)| match (&lo, &hi) {
            (Amount::Const(a), Amount::Const(b)) if a > b => None,
            _ => Some((lo, hi)),
        },
        |r: &(Amount, Amount)| Some(r.clone()),
    );
    let number = lit("a number").then_keep(&bounds.opt()).map(
        |range| Some(Choice::Number { range }),
        |c| match c {
            Choice::Number { range } => Some(range.clone()),
            _ => None,
        },
    );

    let words = crate::vote::words().then(&random).map(
        |(w, random)| Some(Choice::Among { options: Options::Words(w), random }),
        |c| match c {
            Choice::Among { options: Options::Words(w), random } => Some((w.clone(), *random)),
            _ => None,
        },
    );
    let among_colors = colors.list("or").then(&random).map(
        |(c, random)| Some(Choice::Among { options: Options::Colors(c), random }),
        |c| match c {
            Choice::Among { options: Options::Colors(c), random } => Some((c.clone(), *random)),
            _ => None,
        },
    );
    let among_keywords = crate::keyword::keyword().list("or").then(&random).map(
        |(k, random)| Some(Choice::Among { options: Options::Keywords(k), random }),
        |c| match c {
            Choice::Among { options: Options::Keywords(k), random } => Some((k.clone(), *random)),
            _ => None,
        },
    );

    let of_objects = objects.map(
        |o| Some(Choice::Objects(o)),
        |c| match c {
            Choice::Objects(o) => Some(o.clone()),
            _ => None,
        },
    );
    let modal = modes.map(
        |m| Some(Choice::Modes(m)),
        |c| match c {
            Choice::Modes(m) => Some(m.clone()),
            _ => None,
        },
    );
    // The ellipsis "choose one" names one member of the group in scope (608.2d), not a number.
    let one_member = lit("one").to(Choice::Objects(ir::Coord::one(ir::Selector::new(
        ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::One)), ir::ObjectSet::of(ir::Domain::Object),
    ))));
    let single = alt("choice", vec![of_objects, one_member, one, any_name, several, number, words, among_colors, among_keywords, modal]);
    // "a color and an opponent": two kinds of thing; objects joined to objects are one object phrase
    let together = single.list("and").map(
        |v| (!v.iter().any(|c| matches!(c, Choice::All(_))) && !v.windows(2).any(|w| matches!(w, [Choice::Objects(_), Choice::Objects(_)])))
            .then_some(Choice::All(v)),
        |c| match c {
            Choice::All(v) => Some(v.clone()),
            _ => None,
        },
    );
    alt("choice", vec![single, together])
}
