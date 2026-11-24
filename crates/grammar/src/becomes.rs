//! What something becomes: "becomes a 3/3 Elemental creature with haste", "become artifact
//! creatures", "becomes blue", "becomes the color of your choice", "becomes prepared".
//!
//! ```text
//! becoming        := characteristics(n) | colors | "the color of your choice" | … | designation
//! characteristics := ["a"] [P/T] [colors] subtype* type* noun(n) ["with" granted]
//!                    ["with base power and toughness" P/T] [keeps(n)]
//! keeps           := "in addition to its/their other types" | "that's still a" type
//!                  | "that are still" types
//! ```
//!
//! The last word of the subtypes and types is the noun; it agrees with the subject.

use crate::kernel::{P, alt, lit, table};
use crate::token::{colors, pt};
use crate::words::{Number, inflect};
use crate::event::{Form, verb};
use ir::lexicon::{Role, Verb};
use ir::{Action, Amount, Becoming, CardType, Characteristics, Every, Factor, Granted, Keeps, Kept, Selector, SetTo, Setting, Stat, Subtype, Supertype};

fn amount() -> P<Amount> {
    crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone())
}

/// A number or X, as a setting prints it bare ("base power 0").
fn literal(a: &Amount) -> bool {
    matches!(a, Amount::Const(_) | Amount::X)
}

/// The stats a setting names (613.4b): "base power and toughness", "base power", "base
/// toughness". Setting a stat is setting its base value (208.4a), so "base" is not a stat of
/// its own here; "base power and base toughness" reads as "base power and toughness".
fn stat_names(two: bool) -> P<Vec<Stat>> {
    if two {
        alt(
            "base power and toughness",
            vec![
                lit("base power and toughness").to(vec![Stat::Power, Stat::Toughness]),
                lit("base power and base toughness").to(vec![Stat::Power, Stat::Toughness]).alias(),
            ],
        )
    } else {
        alt(
            "base power or toughness",
            vec![lit("base power").to(vec![Stat::Power]), lit("base toughness").to(vec![Stat::Toughness])],
        )
    }
}

/// Something of an object, spelled by its owner: "~'s X" and "the X of target creature you
/// control" (a possessive takes no postmodifier; the other spelling reads for both).
fn of_owner<T: crate::kernel::Val>(names: &P<T>, fits: fn(&Selector) -> bool) -> P<(Selector, T)> {
    use crate::amount::{has_post, owner_of};
    let object = || crate::selector::selectors().selector.clone();
    let possessive = owner_of().then(names).map(
        move |(of, x)| fits(&of).then_some((of, x)),
        move |(of, x): &(Selector, T)| (fits(of) && !has_post(of)).then(|| (of.clone(), x.clone())),
    );
    let spelled = |canonical: bool| {
        lit("the").then_keep(names).skip(&lit("of")).then(&object()).map(
            move |(x, of)| fits(&of).then_some((of, x)),
            move |(of, x): &(Selector, T)| (fits(of) && has_post(of) == canonical).then(|| (x.clone(), of.clone())),
        )
    };
    let spelled_other = spelled(false).alias();
    alt("of owner", vec![possessive, spelled(true), spelled_other])
}

/// What a stat is set to when the verb is "has": "3/2", "each equal to its mana value", "equal
/// to ~'s power and toughness", "0".
fn to_has(two: bool) -> P<SetTo> {
    let each = |a: Amount| Some(SetTo::Each(a));
    if two {
        alt(
            "set to",
            vec![
                pt().map(|(p, t)| Some(SetTo::Profile(p, t)), |s| match s {
                    SetTo::Profile(p, t) => Some((p.clone(), t.clone())),
                    _ => None,
                }),
                lit("each equal to").then_keep(&amount()).map(each, |s| match s {
                    SetTo::Each(a) => Some(a.clone()),
                    _ => None,
                }),
                lit("equal to").then_keep(&matching()),
            ],
        )
    } else {
        alt(
            "set to",
            vec![
                amount().map(|a| literal(&a).then(|| SetTo::Each(a)), |s| match s {
                    SetTo::Each(a) if literal(a) => Some(a.clone()),
                    _ => None,
                }),
                lit("equal to").then_keep(&amount()).map(
                    |a| (!literal(&a)).then(|| SetTo::Each(a)),
                    |s| match s {
                        SetTo::Each(a) if !literal(a) => Some(a.clone()),
                        _ => None,
                    },
                ),
            ],
        )
    }
}

/// "~'s power and toughness", "the power and toughness of target creature": each stat takes the
/// same one of another object.
fn matching() -> P<SetTo> {
    of_owner(&lit("power and toughness"), |s| ir::lexicon::of_kind(s, Verb::SwitchPt.kinds(Role::Of))).map(
        |(of, ())| Some(SetTo::Matching(Box::new(of))),
        |s| match s {
            SetTo::Matching(of) => Some(((**of).clone(), ())),
            _ => None,
        },
    )
}

/// What a stat becomes: the same as `to_has`, where a number reads without "equal to" for one
/// stat and an expression with it ("becomes equal to the number of Towns you control"; "becomes
/// twice that card's power" reads as the alias).
fn to_become(two: bool) -> P<SetTo> {
    if two {
        alt(
            "become",
            vec![
                pt().map(|(p, t)| Some(SetTo::Profile(p, t)), |s| match s {
                    SetTo::Profile(p, t) => Some((p.clone(), t.clone())),
                    _ => None,
                }),
                lit("equal to").then_keep(&matching()),
                // "become equal to ~'s power": both stats to the one number
                lit("equal to").then_keep(&amount()).map(
                    |a| Some(SetTo::Each(a)),
                    |s| match s {
                        SetTo::Each(a) => Some(a.clone()),
                        _ => None,
                    },
                ),
            ],
        )
    } else {
        let expression = amount().map(
            |a| (!literal(&a)).then(|| SetTo::Each(a)),
            |s| match s {
                SetTo::Each(a) if !literal(a) => Some(a.clone()),
                _ => None,
            },
        );
        alt("become", vec![to_has(false), expression.alias()])
    }
}

/// "base power and toughness 3/2", "base power 0", "base power and toughness each equal to its
/// mana value", the part after "has" or "with".
pub fn setting() -> P<Setting> {
    let one = |two: bool| {
        stat_names(two).then(&to_has(two)).map(
            |(stats, to)| Some(Setting { stats, to }),
            move |s: &Setting| (s.stats.len() == if two { 2 } else { 1 }).then(|| (s.stats.clone(), s.to.clone())),
        )
    };
    alt("setting", vec![one(true), one(false)])
}

/// "double target creature's power", "triple the toughness of each creature you control": the
/// creature gets as much again (701.10b, 701.11b).
pub fn scale_stat(form: Form) -> P<Action> {
    let v = |third: &str, base: &str| verb(form, third, base, "");
    let stats = alt(
        "stats",
        vec![
            lit("power and toughness").to(vec![Stat::Power, Stat::Toughness]),
            lit("power").to(vec![Stat::Power]),
            lit("toughness").to(vec![Stat::Toughness]),
        ],
    );
    let factor = alt("scale", vec![v("doubles", "double").to(Factor::Double), v("triples", "triple").to(Factor::Triple)]);
    let fits: fn(&Selector) -> bool = |s| ir::lexicon::of_kind(s, Verb::DoublePt.kinds(Role::Of));
    factor.then(&of_owner(&stats, fits)).map(
        |(factor, (of, stats))| Some(Action::ScaleStat { of: Box::new(of), stats, factor }),
        |a| match a {
            Action::ScaleStat { of, stats, factor } => Some((*factor, ((**of).clone(), stats.clone()))),
            _ => None,
        },
    )
}

/// "double your life total", "double target player's life total" (701.10d): the player's life
/// changes by gaining or losing life, so it is not [`scale_stat`]'s layer effect; only a player
/// has a life total.
pub fn double_life(form: Form) -> P<Action> {
    let fits: fn(&Selector) -> bool = |s| ir::lexicon::of_kind(s, Verb::DoubleLife.kinds(Role::Of));
    let whose = crate::selector::selectors().possessive.map(move |p| fits(&p).then_some(p), move |p| fits(p).then(|| p.clone()));
    verb(form, "doubles", "double", "").then_keep(&whose).skip(&lit("life total")).map(
        |of| Some(Action::DoubleLife { of: Box::new(of) }),
        |a| match a {
            Action::DoubleLife { of } => Some((**of).clone()),
            _ => None,
        },
    )
}

/// A stat set without saying whose: "has base power and toughness 3/2"; or whose, with the
/// object named by it: "~'s base power becomes X", "you may have the base power and toughness
/// of each other creature you control become 0/2".
pub fn set_stat(form: Form) -> P<Action> {
    let v = |third: &str, base: &str| verb(form, third, base, "");
    let has = v("has", "have").then_keep(&setting()).map(
        |set| Some(Action::SetStat { of: None, caused: false, set }),
        |a| match a {
            Action::SetStat { of: None, caused: false, set } => Some(set.clone()),
            _ => None,
        },
    );
    if form != Form::Base {
        return has;
    }
    let fits: fn(&Selector) -> bool = |s| ir::lexicon::of_kind(s, Verb::SwitchPt.kinds(Role::Of));
    // "~'s base power becomes X": its own clause; "have ~'s base power become X": a player has it so
    let owned = |causative: bool| {
        let each = |two: bool| {
            let names = stat_names(two);
            let verb = if causative || two { "become" } else { "becomes" };
            of_owner(&names, fits).skip(&lit(verb)).then(&to_become(two)).map(
                move |((of, stats), to)| Some(Action::SetStat { of: Some(Box::new(of)), caused: causative, set: Setting { stats, to } }),
                move |a| match a {
                    Action::SetStat { of: Some(of), caused, set } if *caused == causative && set.stats.len() == if two { 2 } else { 1 } => {
                        Some((((**of).clone(), set.stats.clone()), set.to.clone()))
                    }
                    _ => None,
                },
            )
        };
        let both = alt("whose stats", vec![each(true), each(false)]);
        if causative { v("has", "have").then_keep(&both) } else { both }
    };
    alt("set stat", vec![has, owned(false), owned(true)])
}

/// The plural and singular noun of a class.
fn nouns(e: Every) -> (&'static str, &'static str) {
    match e {
        Every::CardTypes => ("card types", "card type"),
        Every::CreatureTypes => ("creature types", "creature type"),
        Every::LandTypes => ("land types", "land type"),
        Every::BasicLandTypes => ("basic land types", "basic land type"),
        Every::NonbasicLandTypes => ("nonbasic land types", "nonbasic land type"),
    }
}

/// The plural noun of a class, after "all" or "all other": "card types", "land types".
pub fn class_noun(classes: &[Every]) -> P<Every> {
    table("class of types", classes.iter().map(|e| (nouns(*e).0.to_string(), *e)).collect())
}

/// A whole class of subtypes or card types as one value (205.3m, 702.73a): "all creature types"
/// after "has", "gains", "with" and "loses"; "every creature type" after "is" (the wording of
/// 702.73a). Each place prints its own and reads the other as an alias, since the rules give
/// them one meaning.
pub fn every(plural_first: bool, classes: &[Every]) -> P<Every> {
    let mut entries = Vec::new();
    for e in classes {
        let (pl, sg) = nouns(*e);
        let (all, one) = (format!("all {pl}"), format!("every {sg}"));
        if plural_first {
            entries.extend([(all, *e), (one, *e)]);
        } else {
            entries.extend([(one, *e), (all, *e)]);
        }
    }
    table("class of types", entries)
}

/// The classes of subtypes an object can be or have.
pub const LAND_AND_CREATURE: [Every; 4] =
    [Every::CreatureTypes, Every::LandTypes, Every::BasicLandTypes, Every::NonbasicLandTypes];

fn supertype()-> P<Supertype> {
    table("supertype", Supertype::ALL.iter().map(|s| (s.word().to_string(), *s)).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Word {
    Sub(Subtype),
    Type(CardType),
}

fn words(n: Number) -> P<Word> {
    let mut e: Vec<(String, Word)> = Subtype::all().map(|s| (inflect(s.name(), n), Word::Sub(s))).collect();
    e.extend(CardType::ALL.iter().map(|t| (inflect(t.word(), n), Word::Type(*t))));
    table(if n == Number::Sg { "type word" } else { "type words" }, e)
}

/// Subtypes, then types, the last one in number `n`.
fn type_line(n: Number) -> P<(Vec<Subtype>, Vec<CardType>)> {
    words(Number::Sg).many().then(&words(n)).map(
        |(mut v, last)| {
            v.push(last);
            let split = v.iter().position(|w| matches!(w, Word::Type(_))).unwrap_or(v.len());
            let (subs, types) = v.split_at(split);
            let subtypes = subs.iter().map(|w| if let Word::Sub(s) = w { Some(*s) } else { None }).collect::<Option<_>>()?;
            let types = types.iter().map(|w| if let Word::Type(t) = w { Some(*t) } else { None }).collect::<Option<_>>()?;
            Some((subtypes, types))
        },
        |(subs, types): &(Vec<Subtype>, Vec<CardType>)| {
            let mut v: Vec<Word> = subs.iter().map(|s| Word::Sub(*s)).chain(types.iter().map(|t| Word::Type(*t))).collect();
            let last = v.pop()?;
            Some((v, last))
        },
    )
}

/// One kept type, with the subtypes before it: "land", "Cave land", "lands".
fn kept(n: Number) -> P<Kept> {
    type_line(n).map(
        |(subtypes, types)| (types.len() == 1).then(|| Kept { subtypes, card_type: types[0] }),
        |k: &Kept| Some((k.subtypes.clone(), vec![k.card_type])),
    )
}

/// "a" before a singular noun phrase, nothing before a plural one; "an" reads as "a".
fn article(n: Number) -> P<()> {
    match n {
        Number::Sg => alt("a", vec![lit("a"), lit("an").alias()]),
        Number::Pl => crate::kernel::empty(),
    }
}

fn keeps(n: Number) -> P<Keeps> {
    let (its, still) = match n {
        Number::Sg => ("in addition to its other", lit("that's still").then_keep(&article(n)).then_keep(&kept(n))),
        Number::Pl => ("in addition to their other", lit("that are still").then_keep(&kept(n))),
    };
    let other = alt(
        "other",
        vec![
            lit("types").to(Keeps::OtherTypes),
            lit("creature types").to(Keeps::OtherCreatureTypes),
            lit("land types").to(Keeps::OtherLandTypes),
            lit("colors and types").to(Keeps::OtherColorsAndTypes),
            lit("colors").to(Keeps::OtherColors),
        ],
    );
    alt(
        "keeps",
        vec![
            lit(its).then_keep(&other),
            still.map(
                |t| Some(Keeps::StillA(t)),
                |k| match k {
                    Keeps::StillA(t) => Some(t.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// One thing a description is "with".
#[derive(Debug, Clone, PartialEq)]
enum With {
    Set(Setting),
    Grant(Granted),
}

fn characteristics(n: Number) -> P<Characteristics> {
    // "with base power and toughness 3/3, flying, and haste": a setting and abilities in one
    // list, the setting first (the other order is the alias, "with haste and base power and
    // toughness equal to ~'s"); 208.4a: "base" is optional wording of setting a stat
    let item = alt(
        "with item",
        vec![
            setting().map(
                |s| Some(With::Set(s)),
                |w| match w {
                    With::Set(s) => Some(s.clone()),
                    _ => None,
                },
            ),
            crate::action::granted_one().map(
                |g| Some(With::Grant(g)),
                |w| match w {
                    With::Grant(g) => Some(g.clone()),
                    _ => None,
                },
            ),
        ],
    );
    let items = alt("with items", vec![item.map(|i| Some(vec![i]), |v: &Vec<With>| (v.len() == 1).then(|| v[0].clone())), item.list("and")]);
    let split = |v: &[With]| -> Option<(Option<Setting>, Vec<Granted>)> {
        let sets: Vec<&Setting> = v.iter().filter_map(|w| if let With::Set(s) = w { Some(s) } else { None }).collect();
        let grants: Vec<Granted> = v.iter().filter_map(|w| if let With::Grant(g) = w { Some(g.clone()) } else { None }).collect();
        (sets.len() <= 1).then(|| (sets.first().map(|s| (*s).clone()), grants))
    };
    // the setting, if any, is the first item
    let early = |v: &[With]| !v.iter().skip(1).any(|w| matches!(w, With::Set(_)));
    let with_items = alt(
        "with",
        vec![
            items.map(
                move |v| if early(&v) { split(&v) } else { None },
                |(set, grants): &(Option<Setting>, Vec<Granted>)| {
                    let mut v: Vec<With> = set.iter().cloned().map(With::Set).collect();
                    v.extend(grants.iter().cloned().map(With::Grant));
                    (!v.is_empty()).then_some(v)
                },
            ),
            items.map(move |v| if early(&v) { None } else { split(&v) }, |_| None).alias(),
            lit("power and toughness each equal to").then_keep(&amount()).map(
                |a| Some((Some(Setting { stats: vec![Stat::Power, Stat::Toughness], to: SetTo::Each(a) }), Vec::new())),
                |_: &(Option<Setting>, Vec<Granted>)| None,
            ).alias(),
        ],
    );
    let with = lit("with").then_keep(&with_items);
    // "a legendary 4/4 red Dragon creature with flying and all creature types" (205.3m)
    let nominal = article(n)
        .then_keep(&supertype().many())
        .then(&pt().opt())
        .then(&colors().opt())
        .then(&type_line(n))
        .then(&with.opt())
        .then(&lit("named").then_keep(&crate::kernel::name_run()).opt())
        .then(&keeps(n).opt())
        .map(
            |((((((supertypes, pt), colors), (subtypes, types)), with), name), keeps)| {
                let (base_pt, abilities) = with.unwrap_or_default();
                Some(Characteristics {
                    pt,
                    colors,
                    supertypes,
                    subtypes,
                    types,
                    name,
                    abilities,
                    base_pt,
                    keeps: keeps.unwrap_or(Keeps::Nothing),
                })
            },
            |c: &Characteristics| {
                let with = (c.base_pt.is_some() || !c.abilities.is_empty()).then(|| (c.base_pt.clone(), c.abilities.clone()));
                let keeps = (c.keeps != Keeps::Nothing).then(|| c.keeps.clone());
                let head = ((c.supertypes.clone(), c.pt.clone()), c.colors.clone());
                Some(((((head, (c.subtypes.clone(), c.types.clone())), with), c.name.clone()), keeps))
            },
        );
    // "legendary", "legendary and snow in addition to its other types", "every creature type":
    // no noun, so no article; what is named is a supertype or a whole class of subtypes
    let classes = LAND_AND_CREATURE;
    let one = supertype().map(|s| Some(vec![s]), |v: &Vec<Supertype>| (v.len() == 1).then(|| v[0]));
    let supertypes = alt("supertypes", vec![one, supertype().list("and")]).map(
        |s| Some(Characteristics { supertypes: s, ..Characteristics::default() }),
        |c: &Characteristics| (*c == Characteristics { supertypes: c.supertypes.clone(), ..Characteristics::default() }).then(|| c.supertypes.clone()).filter(|s| !s.is_empty()),
    );
    let all_of_a_class = every(false, &classes).map(
        |e| Some(Characteristics { abilities: vec![Granted::Every(e)], ..Characteristics::default() }),
        |c: &Characteristics| match c.abilities.as_slice() {
            [Granted::Every(e)] => (*c == Characteristics { abilities: c.abilities.clone(), ..Characteristics::default() }).then_some(*e),
            _ => None,
        },
    );
    let bare = alt("bare characteristics", vec![supertypes, all_of_a_class]).then(&keeps(n).opt()).map(
        |(c, keeps)| Some(Characteristics { keeps: keeps.unwrap_or(Keeps::Nothing), ..c }),
        |c: &Characteristics| Some((Characteristics { keeps: Keeps::Nothing, ..c.clone() }, (c.keeps != Keeps::Nothing).then(|| c.keeps.clone()))),
    );
    alt("characteristics", vec![nominal, bare])
}

/// What a subject becomes; `numbers`: the subject's possible numbers (a base-form verb follows
/// a plural subject or, after "have", a singular one).
pub fn becoming(numbers: &[Number]) -> P<Becoming> {
    let is = alt("characteristics", numbers.iter().map(|n| characteristics(*n)).collect()).map(
        |c| Some(Becoming::Is(c)),
        |b| match b {
            Becoming::Is(c) => Some(c.clone()),
            _ => None,
        },
    );
    // "your choice of a 3/3 creature, a 2/2 creature with flying, or a 1/6 creature with defender"
    let choice_of = lit("your choice of").then_keep(&is.list("or")).map(
        |v| Some(Becoming::ChoiceOf(v)),
        |b| match b {
            Becoming::ChoiceOf(v) => Some(v.clone()),
            _ => None,
        },
    );
    let additional_colors = alt("other colors", numbers.iter().map(|n| keeps(*n)).collect()).map(
        |k| (k == Keeps::OtherColors).then_some(()),
        |()| Some(Keeps::OtherColors),
    );
    let colors = colors().then(&additional_colors.opt()).map(
        |(colors, added)| Some(Becoming::Colors { colors, added: added.is_some() }),
        |b| match b {
            Becoming::Colors { colors, added } => Some((colors.clone(), added.then_some(()))),
            _ => None,
        },
    );
    // "are the chosen color in addition to their other colors" (Painter's Servant)
    let chosen_color = lit("the chosen color").then_keep(&additional_colors.opt()).map(
        |added| Some(Becoming::ChosenColor { added: added.is_some() }),
        |b| match b {
            Becoming::ChosenColor { added } => Some(added.then_some(())),
            _ => None,
        },
    );
    let still = alt(
        "still",
        numbers
            .iter()
            .map(|n| match n {
                Number::Sg => lit("still").then_keep(&article(*n)).then_keep(&kept(*n)),
                Number::Pl => lit("still").then_keep(&kept(*n)),
            })
            .collect(),
    )
    .map(
        |t| Some(Becoming::Still(t)),
        |b| match b {
            Becoming::Still(t) => Some(t.clone()),
            _ => None,
        },
    );
    let copy = alt(
        "copy",
        numbers
            .iter()
            .map(|n| lit(if *n == Number::Sg { "a copy of" } else { "copies of" }).then_keep(&crate::selector::selectors().selector))
            .collect(),
    )
    .map(
        |x| Some(Becoming::CopyOf(Box::new(x))),
        |b| match b {
            Becoming::CopyOf(x) => Some((**x).clone()),
            _ => None,
        },
    );
    let chosen_type = alt("chosen type", numbers.iter().map(|n| lit("the chosen type").then_keep(&keeps(*n).opt())).collect()).map(
        |k| Some(Becoming::ChosenType(k.unwrap_or(Keeps::Nothing))),
        |b| match b {
            Becoming::ChosenType(k) => Some((*k != Keeps::Nothing).then(|| k.clone())),
            _ => None,
        },
    );
    alt(
        "becoming",
        vec![
            copy,
            choice_of,
            chosen_type,
            // "Choose a creature type. Target creature becomes that type …"
            lit("that type").to(Becoming::ChosenType(Keeps::Nothing)).alias(),
            chosen_color,
            // "Add one mana of any color. ~ becomes that color …"
            lit("that color").to(Becoming::ChosenColor { added: false }).alias(),
            is,
            still,
            colors,
            lit("the color of your choice").to(Becoming::ColorOfChoice),
            lit("the color or colors of your choice").to(Becoming::ColorsOfChoice),
            lit("the creature type of your choice in addition to its other types").to(Becoming::TypeOfChoiceAlso { land: false }),
            lit("the basic land type of your choice in addition to its other types").to(Becoming::TypeOfChoiceAlso { land: true }),
            lit("the creature type of your choice").to(Becoming::CreatureTypeOfChoice),
            lit("the basic land type of your choice").to(Becoming::BasicLandTypeOfChoice),
            lit("blocked").to(Becoming::Blocked),
            lit("prepared").to(Becoming::Prepared),
            lit("solved").to(Becoming::Solved),
            lit("unprepared").to(Becoming::Unprepared),
            // what a class level bar's ability does (716.2a)
            lit("level").then_keep(&crate::words::digits()).map(
                |n| Some(Becoming::Level(n)),
                |b| match b {
                    Becoming::Level(n) => Some(*n),
                    _ => None,
                },
            ),
            lit("goaded").to(Becoming::Goaded),
            lit("plotted").to(Becoming::Plotted),
            lit("foretold").to(Becoming::Foretold),
            lit("monstrous").to(Becoming::Monstrous),
            lit("renowned").to(Becoming::Renowned),
            lit("saddled").to(Becoming::Saddled),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    fn sentences(texts: &[&str]) {
        let s = &crate::effect::effects().sentence;
        for text in texts {
            let toks = tokenize(text);
            match s.parse_all(&toks) {
                Outcome::Accepted { value, .. } => assert_eq!(s.print_text(&value, true).as_deref(), Some(*text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
    }

    #[test]
    fn r613_4b_setting_stats() {
        sentences(&[
            "~ has base power and toughness 3/2",
            "Target creature has base power 0 until end of turn",
            "Creatures you control have base power and toughness X/X",
            "Enchanted creature has base toughness 1",
            "~ has base power and toughness each equal to the number of creatures you control",
            "~ has base power and toughness equal to target creature's power and toughness",
            "~'s base power becomes equal to the number of Towns you control",
            "~'s base power and toughness become 4/2 until end of turn",
            "The base power and toughness of each creature you control become 0/2",
            "You may have ~'s base power become equal to 1 plus the greatest power among other creatures you control until end of turn",
            "You may have ~'s base power and toughness become equal to that creature's power and toughness until end of turn",
            "Target creature becomes a Bird with base power and toughness 1/1, flying, and haste",
            "Target creature becomes a Dragon with base power and toughness 4/4 and flying",
        ]);
        // the other order reads as the same, printed the canonical way
        let s = &crate::effect::effects().sentence;
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize("Target creature becomes a Dragon with flying and base power and toughness 4/4")) else {
            panic!("alias")
        };
        assert_eq!(s.print_text(&value, true).as_deref(), Some("Target creature becomes a Dragon with base power and toughness 4/4 and flying"));
    }

    #[test]
    fn r613_4b_what_does_not_set_a_stat_does_not_read() {
        let s = &crate::effect::effects().sentence;
        for t in [
            // a player has no power, and base power needs a number
            "Target player has base power and toughness 3/2",
            "~ has base power",
            "~ has base mana value 3",
            "~'s base power and toughness becomes 4/2",
        ] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn r701_10_and_701_11_doubling_and_tripling_a_stat() {
        sentences(&[
            "Double target creature's power and toughness until end of turn",
            "Double the toughness of each creature you control until end of turn",
            "Triple target creature's power and toughness until end of turn",
            "Triple the power of target creature you control",
        ]);
        // a player has no power, and only doubling and tripling are defined
        let s = &crate::effect::effects().sentence;
        for t in ["Double target player's power", "Quadruple target creature's power"] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn r731_1_it_becomes_day_or_night() {
        sentences(&[
            "If it's night, it becomes day",
            "It becomes night",
            "If it's neither day nor night, it becomes day",
        ]);
        // the game is no object's to change: day and night are not something a creature becomes
        let s = &crate::effect::effects().sentence;
        for t in ["Target creature becomes day", "Target creature it becomes night", "It becomes neither day nor night", "It becomes dusk"] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn r205_3m_what_is_not_a_class_of_types_does_not_read() {
        for t in ["every creature types", "all creature type", "every color", "every card", "all colors and legendary"] {
            let p = becoming(&[Number::Sg]);
            let toks = tokenize(t);
            assert!(!matches!(p.parse_all(&toks), Outcome::Accepted { .. }), "{t} should not read");
        }
    }

    #[test]
    fn round_trips() {
        for (n, t) in [
            (Number::Sg, "a 0/0 Elemental creature with haste"),
            (Number::Sg, "an artifact creature"),
            (Number::Sg, "an Island"),
            (Number::Sg, "a blue Serpent with base power and toughness 1/1"),
            (Number::Sg, "a Human Spirit Warrior with trample and lifelink"),
            (Number::Sg, "an artifact in addition to its other types"),
            (Number::Sg, "a 2/2 creature that's still a land"),
            (Number::Pl, "artifact creatures"),
            (Number::Pl, "artifact creatures with base power and toughness 2/2"),
            (Number::Sg, "colorless"),
            (Number::Sg, "blue in addition to its other colors"),
            (Number::Pl, "black and green in addition to their other colors"),
            (Number::Sg, "the color of your choice"),
            (Number::Sg, "prepared"),
            // 205.4: supertypes come first, bare or before a noun
            (Number::Sg, "a legendary 4/4 red Dragon creature with flying"),
            (Number::Sg, "legendary"),
            (Number::Sg, "legendary and snow in addition to its other types"),
            // 205.3m, 702.73a: a whole class of subtypes is one value, spelled by its place
            (Number::Sg, "every creature type"),
            (Number::Pl, "every basic land type in addition to their other types"),
            (Number::Sg, "a 3/3 creature with vigilance and all creature types"),
            // 205.1b: the kept type may carry subtypes
            (Number::Sg, "an Elemental creature that's still a Cave land"),
            (Number::Sg, "still a Cave land"),
            (Number::Pl, "still lands"),
            (Number::Sg, "every nonbasic land type"),
            // 201.1: a name after the description
            (Number::Sg, "a legendary Horror creature named Fenric"),
            // 105.1: all colors is the five colors
            (Number::Sg, "all colors"),
        ] {
            let p = becoming(&[n]);
            let toks = tokenize(t);
            let Outcome::Accepted { value, .. } = p.parse_all(&toks) else { panic!("{t}: {:?}", p.parse_all(&toks)) };
            assert_eq!(p.print_text(&value, false).as_deref(), Some(t));
        }
    }

    /// CR 701.10d: a player's life total is doubled; a creature has no life total, and the rules
    /// triple only power and toughness (701.11).
    #[test]
    fn r701_10d_double_a_life_total() {
        sentences(&["Double your life total", "Double target player's life total", "Double its controller's life total"]);
        let s = &crate::effect::effects().sentence;
        for text in ["Double target creature's life total", "Triple your life total", "Double your life"] {
            assert!(!matches!(s.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text} must not read");
        }
    }
}
