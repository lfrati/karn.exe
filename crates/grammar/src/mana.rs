//! Mana costs: a run of `{…}` mana symbols, printed without spaces ("{2}{U}{U}").

use crate::amount::amounts;
use crate::kernel::{P, alt, leaf, lit};
use crate::selector::selectors;
use ir::mana::{Pooled, SpendOn, SpendableMana};
use ir::{Amount, ManaColor, ManaCost, ManaSpec, ManaSymbol, ManaTypes, Sort, Who};
use oracle::token::Kind;

pub fn mana_symbol() -> P<ManaSymbol> {
    leaf(
        "mana symbol",
        |k| match k {
            Kind::Symbol(s) => ManaSymbol::parse(s),
            _ => None,
        },
        |m| Some(Kind::Symbol(m.text())),
    )
}

pub fn mana_cost() -> P<ManaCost> {
    let sym = mana_symbol();
    sym.then(&sym.clone().tight().many()).map(
        |(first, mut rest)| {
            rest.insert(0, first);
            Some(ManaCost(rest))
        },
        |c| c.0.split_first().map(|(f, r)| (*f, r.to_vec())),
    )
}

/// What "add" produces (CR 106.1, 106.3): how much mana, of what type, or one of several such.
///
/// ```text
/// mana     := one | one ("," one)* "or" one       "{R} or {G}", "{W} or one mana of the chosen color"
/// one      := symbols | number symbol | quantity(symbol) | quantity("mana" type)
/// quantity := count N | "that much" N | "an amount of" N "equal to" amount
/// type     := "of any" types | "in any combination of" types | "of any one color" | "of that color" | …
/// ```
pub fn mana_spec() -> P<ManaSpec> {
    // "{G}{G}", and a number before one symbol, that many of it ("six {G}", 107.4a): one
    // meaning, and how a run of six or more of one symbol prints
    let long_run = |c: &ManaCost| c.0.len() >= 6 && c.0.iter().all(|s| *s == c.0[0]);
    let counted = crate::words::number_words().then(&mana_symbol()).map(
        |(n, symbol)| (n >= 1).then(|| ManaSpec::Symbols(ManaCost(vec![symbol; n as usize]))),
        move |s| match s {
            ManaSpec::Symbols(c) if long_run(c) => Some((c.0.len() as u32, c.0[0])),
            _ => None,
        },
    );
    let run = mana_cost().map(
        |c| Some(ManaSpec::Symbols(c)),
        |s| match s {
            ManaSpec::Symbols(c) => Some(c.clone()),
            _ => None,
        },
    );
    // an amount of one symbol that the text does not fix: "X {R}", "that much {C}"
    let fixed = |a: &Amount| matches!(a, Amount::Const(_));
    let of_symbol = quantity_of(&mana_symbol()).map(
        move |(amount, symbol)| (!fixed(&amount)).then_some(ManaSpec::AmountOf { symbol, amount }),
        move |s| match s {
            ManaSpec::AmountOf { symbol, amount } if !fixed(amount) => Some((amount.clone(), *symbol)),
            _ => None,
        },
    );
    // an amount of mana said by its type. A choice among types is spelled by number: of one
    // mana "of any …", of several "in any combination of …"
    let colors = mana_colors();
    let of = |color: &P<ManaColor>, fits: fn(&Amount) -> bool| {
        quantity_of(&lit("mana").then_keep(color)).map(
            move |(amount, color)| fits(&amount).then_some(ManaSpec::Of { amount, color }),
            move |s| match s {
                ManaSpec::Of { amount, color } if fits(amount) => Some((amount.clone(), color.clone())),
                _ => None,
            },
        )
    };
    let of_type = alt(
        "mana of a type",
        vec![
            of(&colors.said, |_| true),
            of(&colors.any_one_mana, |a| *a == Amount::Const(1)),
            of(&colors.any_several, |a| *a != Amount::Const(1)),
        ],
    );
    // an alternative is never itself a choice of several: the list is flat
    let one = alt("one production", vec![counted, run, of_symbol, of_type]);
    let one_of = one.list("or").map(
        |v| Some(ManaSpec::OneOf(v)),
        |s| match s {
            ManaSpec::OneOf(v) => Some(v.clone()),
            _ => None,
        },
    );
    alt("mana", vec![one, one_of])
}

/// How much of a thing an "add" produces, with the thing (CR 107.1, 107.3): a number or X
/// before it ("two mana in any combination of colors", "X mana of any one color"), "that
/// much" or "twice that much" of it, or "an amount of" it "equal to" any other amount ("an
/// amount of {G} equal to ~'s power", "an amount of {C} equal to X plus one", "an amount of
/// mana of that color equal to your devotion to that color"). A number, X and "that much"
/// are never said with "equal to".
fn quantity_of<T: crate::kernel::Val>(noun: &P<T>) -> P<(Amount, T)> {
    let before = alt(
        "mana amount",
        vec![
            crate::words::number_words().map(
                |n| Some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) => Some(*n),
                    _ => None,
                },
            ),
            lit("X").to(Amount::X),
            lit("that much").to(Amount::That { many: false }),
            lit("twice that much").to(Amount::Multiple(2, Box::new(Amount::That { many: false }))),
        ],
    )
    .then(noun);
    let plain = |a: &Amount| matches!(a, Amount::Const(_) | Amount::X | Amount::That { .. });
    let equal_to = lit("an amount of").then_keep(noun).skip(&lit("equal to")).then(&amounts().amount).map(
        move |(noun, amount)| (!plain(&amount)).then_some((amount, noun)),
        move |(amount, noun): &(Amount, T)| (!plain(amount)).then(|| (noun.clone(), amount.clone())),
    );
    alt("how much", vec![before, equal_to])
}

/// The type of the mana in "N mana …" (CR 106.1b), by how it is said.
struct ManaColors {
    /// Said outright or fixed by the text before, whatever the amount: "of any one color", "of
    /// the chosen color", "of that color", "of that type", "of different colors".
    said: P<ManaColor>,
    /// One mana's choice among types: "of any color", "of any of its colors".
    any_one_mana: P<ManaColor>,
    /// Several mana's choice among types, each for itself: "in any combination of colors".
    any_several: P<ManaColor>,
}

fn mana_colors() -> ManaColors {
    // only an object has colors (105.2) and only a permanent produces mana (106.7): a player
    // is neither
    let object = |domain: &ir::Domain| *domain != ir::Domain::Player;
    // "{R} and/or {G}": the types themselves
    let symbols = mana_symbol().list("and/or").map(
        |v| Some(ManaTypes::Symbols(v)),
        |t| match t {
            ManaTypes::Symbols(v) => Some(v.clone()),
            _ => None,
        },
    );
    // "the exiled card's colors", "its colors": the colors an object is (105.2)
    let colors_of = crate::kernel::lazy("possessive", crate::amount::owner_of).skip(&lit("colors")).map(
        move |s| object(&s.set.domain).then(|| ManaTypes::ColorsOf(Box::new(s))),
        move |t| match t {
            ManaTypes::ColorsOf(s) if object(&s.set.domain) => Some((**s).clone()),
            _ => None,
        },
    );
    // "among legendary permanents you control": the colors the objects of a set are (105.2)
    let colors_among = lit("color among").then_keep(&crate::kernel::lazy("plural set", || selectors().set_pl.clone())).map(
        move |set| object(&set.domain).then(|| ManaTypes::ColorsAmong(Box::new(set))),
        move |t| match t {
            ManaTypes::ColorsAmong(set) if object(&set.domain) => Some((**set).clone()),
            _ => None,
        },
    );
    // "type that land produced", "color that a land an opponent controls could produce" (106.7):
    // "that" introduces the clause, and is dropped before a definite noun phrase ("that land
    // produced", "the sacrificed land could produce")
    let kind = alt("type or color", vec![lit("type").to(true), lit("color").to(false)]);
    let definite = |s: &ir::Selector| matches!(s.quant, ir::Quant::Definite(_));
    let producer = move || {
        crate::kernel::lazy("selector", || selectors().singular.clone())
            .map(move |s| object(&s.set.domain).then_some(s), move |s: &ir::Selector| object(&s.set.domain).then(|| s.clone()))
    };
    let by = alt(
        "producer",
        vec![
            lit("that").then_keep(&producer()).map(move |s| (!definite(&s)).then_some(s), move |s: &ir::Selector| (!definite(s)).then(|| s.clone())),
            producer().map(move |s| definite(&s).then_some(s), move |s: &ir::Selector| definite(s).then(|| s.clone())),
        ],
    );
    let produced = kind.then(&by).then(&alt("produced", vec![lit("produced").to(false), lit("could produce").to(true)])).map(
        |((types, by), could)| Some(ManaTypes::Produced { types, by: Box::new(by), could }),
        |t| match t {
            ManaTypes::Produced { types, by, could } => Some(((*types, (**by).clone()), *could)),
            _ => None,
        },
    );
    // what one mana may be: "of any color", "of any of its colors", "of any color among …"
    let any_of = alt(
        "any type",
        vec![
            lit("color in your commander's color identity").to(ManaTypes::CommanderIdentity),
            lit("color").to(ManaTypes::Colors),
            // "That player adds one mana of any color they choose": the one adding chooses anyway
            lit("color they choose").to(ManaTypes::Colors).alias(),
            lit("of").then_keep(&colors_of),
            colors_among,
            produced,
        ],
    );
    let any = |types: &P<ManaTypes>| {
        types.map(
            |t| Some(ManaColor::Any(t)),
            |c| match c {
                ManaColor::Any(t) => Some(t.clone()),
                _ => None,
            },
        )
    };
    let any_one_mana = any(&lit("of any").then_keep(&any_of));
    // what several mana may be, each for itself: "in any combination of colors", "… of {R}
    // and/or {G}", "… of its colors"
    let combined = alt("combined types", vec![lit("colors").to(ManaTypes::Colors), symbols, colors_of]);
    let any_several = any(&lit("in any combination of").then_keep(&combined));
    let said = alt(
        "mana color",
        vec![
            lit("of any one color").to(ManaColor::AnyOne),
            lit("of any one color they choose").to(ManaColor::AnyOne).alias(),
            lit("of the chosen color").to(ManaColor::Chosen),
            lit("of that color").to(ManaColor::That),
            lit("of that type").to(ManaColor::ThatType),
            lit("of different colors").to(ManaColor::Different),
        ],
    );
    ManaColors { said, any_one_mana, any_several }
}

/// A selector or a bare plural set: "that spell", "~", "creature spells", "creatures with flying".
/// A bare plural is a set, not a selector, so the two are different values of one meaning, as
/// everywhere a noun phrase may be singular with a determiner or plural without one.
pub fn who() -> P<Who> {
    let s = selectors();
    alt(
        "who",
        vec![
            s.selector.map(
                |x| Some(Who::One(x)),
                |w| match w {
                    Who::One(x) => Some(x.clone()),
                    _ => None,
                },
            ),
            s.set_pl.map(
                |x| Some(Who::Set(x)),
                |w| match w {
                    Who::Set(x) => Some(x.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// The noun phrase as a selector, to ask the lexicon what it can be (a bare set denotes all of
/// its members).
fn as_selector(w: &Who) -> ir::Selector {
    match w {
        Who::One(s) => s.clone(),
        Who::Set(set) => ir::Selector::new(ir::Quant::All, set.clone()),
    }
}

/// Does the noun phrase name only things of these sorts (the typed slot of a role, CR 601.2,
/// 602.2)? "Cast a creature spell" does, "cast target player" does not. A phrase that says
/// nothing of what it is ("X of …") names a thing only by reference ("it", "that card").
fn fits(w: &Who, sorts: &[Sort]) -> bool {
    let said = match w {
        Who::One(s) => s.set.domain != ir::Domain::Object || matches!(s.quant, ir::Quant::Definite(_)),
        Who::Set(set) => set.domain != ir::Domain::Object,
    };
    said && ir::lexicon::of_kind(&as_selector(w), sorts)
}

fn who_of(p: &P<Who>, sorts: &'static [Sort]) -> P<Who> {
    p.map(move |w| fits(&w, sorts).then_some(w), move |w: &Who| fits(w, sorts).then(|| w.clone()))
}

/// What has abilities: a permanent, a card, a spell (113.3).
const SOURCES: &[Sort] = &[Sort::Permanent, Sort::Card, Sort::Spell, Sort::Ability];

/// What mana is spent on, after "to" or "on": "cast a creature spell", "cast artifact spells",
/// "activate abilities of artifacts", "activate an ability of an artifact source", "activate
/// those abilities" (CR 601.2, 602.2, 106.6).
pub fn spend_on() -> P<SpendOn> {
    let s = selectors();
    let spells = crate::action::coord(&who_of(&who(), &[Sort::Spell]));
    let sources_pl = s.set_pl.map(|x| ir::lexicon::of_kind(&as_selector(&Who::Set(x.clone())), SOURCES).then_some(x), |x| ir::lexicon::of_kind(&as_selector(&Who::Set(x.clone())), SOURCES).then(|| x.clone()));
    let sources_sg = s.selector.map(|x| ir::lexicon::of_kind(&x, SOURCES).then_some(x), |x| ir::lexicon::of_kind(x, SOURCES).then(|| x.clone()));
    // abilities named by what they are: "this ability", "those abilities", "equip abilities",
    // "loyalty abilities" (a keyword's own ability, 702; a kind, 113.3); bare "abilities" are
    // said by their source below
    fn what_it_is(f: &ir::Filter) -> bool {
        match f {
            ir::Filter::Atom(ir::Atom::Keyword(_) | ir::Atom::AbilityKind(_)) => true,
            ir::Filter::Any(v) => v.iter().all(what_it_is),
            _ => false,
        }
    }
    let referred = |w: &Who| match w {
        Who::One(s) => matches!(s.quant, ir::Quant::Definite(_)),
        Who::Set(set) => {
            let plain = ir::ObjectSet { filter: set.filter.clone(), ..ir::ObjectSet::of(ir::Domain::Ability) };
            *set == plain && matches!(&set.filter, ir::Filter::All(v) if !v.is_empty() && v.iter().all(what_it_is))
        }
    };
    let named = who_of(&who(), &[Sort::Ability]).map(move |w| referred(&w).then_some(w), move |w: &Who| referred(w).then(|| w.clone()));
    alt(
        "spend on",
        vec![
            lit("pay generic mana costs").to(SpendOn::Generic),
            lit("cast").then_keep(&spells).map(
                |w| Some(SpendOn::Cast(w)),
                |u| match u {
                    SpendOn::Cast(w) => Some(w.clone()),
                    _ => None,
                },
            ),
            lit("activate").then_keep(&named.clone()).map(
                |w| Some(SpendOn::Activate(w)),
                |u| match u {
                    SpendOn::Activate(w) => Some(w.clone()),
                    _ => None,
                },
            ),
            lit("activate abilities").then_keep(&lit("of").then_keep(&sources_pl).opt()).map(
                |o| Some(SpendOn::ActivateAbilities(o)),
                |u| match u {
                    SpendOn::ActivateAbilities(o) => Some(o.clone()),
                    _ => None,
                },
            ),
            lit("activate an ability").then_keep(&lit("of").then_keep(&sources_sg).opt()).map(
                |o| Some(SpendOn::ActivateAnAbility(o)),
                |u| match u {
                    SpendOn::ActivateAnAbility(o) => Some(o.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// What one mana was spent on, after "was spent": "to cast it", "to activate this ability", "on a
/// creature spell" ("on" a spell is the same as "to cast" it).
pub fn spent_on() -> P<SpendOn> {
    let spells = crate::action::coord(&who_of(&who(), &[Sort::Spell]));
    let on = lit("on").then_keep(&spells).map(
        |w| Some(SpendOn::Cast(w)),
        |u| match u {
            SpendOn::Cast(w) => Some(w.clone()),
            _ => None,
        },
    );
    alt("spent on", vec![lit("to").then_keep(&spend_on()), on.alias()])
}

/// "unspent mana", "unspent green mana": what remains in a player's mana pool (CR 106.4).
pub fn unspent() -> P<Option<ir::Color>> {
    let color = crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    lit("unspent").then_keep(&color.opt()).skip(&lit("mana"))
}

/// Mana that stays or goes with the pool: "this mana", "unspent red mana" (CR 106.4, 106.6).
pub fn pooled() -> P<Pooled> {
    alt(
        "pooled mana",
        vec![
            lit("this mana").to(Pooled::This),
            unspent().map(
                |c| Some(Pooled::Unspent(c)),
                |p| match p {
                    Pooled::Unspent(c) => Some(*c),
                    _ => None,
                },
            ),
        ],
    )
}

/// The mana a rule about spending names by kind or source: "black and/or red mana", "colored
/// mana", "mana produced by basic lands", "mana of the chosen color" (CR 106.6).
pub fn spendable_mana() -> P<SpendableMana> {
    let color = crate::kernel::table("color", ir::Color::ALL.iter().map(|c| (c.word().to_string(), *c)).collect());
    let one = color.map(|c| Some(vec![c]), |v: &Vec<ir::Color>| (v.len() == 1).then(|| v[0]));
    let colors = alt("colors", vec![one, color.list("and/or")]).skip(&lit("mana")).map(
        |v| Some(SpendableMana::Colors(v)),
        |m| match m {
            SpendableMana::Colors(v) => Some(v.clone()),
            _ => None,
        },
    );
    alt(
        "spendable mana",
        vec![
            colors,
            lit("colored mana").to(SpendableMana::Colored),
            lit("mana produced by").then_keep(&selectors().set_pl).map(
                |set| Some(SpendableMana::ProducedBy(set)),
                |m| match m {
                    SpendableMana::ProducedBy(set) => Some(set.clone()),
                    _ => None,
                },
            ),
            lit("mana of the chosen color").to(SpendableMana::Chosen),
        ],
    )
}

/// What mana named by kind is restricted to: "on X" (107.3), or "to cast ~", "to activate this
/// ability".
pub fn spendable_on() -> P<ir::Coord<SpendOn>> {
    let x = lit("on X").to(SpendOn::X).map(|u| Some(ir::Coord::one(u)), |c: &ir::Coord<SpendOn>| match c.items.as_slice() {
        [SpendOn::X] => Some(SpendOn::X),
        _ => None,
    });
    alt("spendable on", vec![x, spend_to().map(|c| (!c.items.contains(&SpendOn::X)).then_some(c), |c: &ir::Coord<SpendOn>| (!c.items.contains(&SpendOn::X)).then(|| c.clone()))])
}

/// Casting and activating are each named once in a list; several spells are one phrase.
fn distinct(items: &[SpendOn]) -> bool {
    let casts = items.iter().filter(|u| matches!(u, SpendOn::Cast(_))).count();
    casts <= 1 && items.len() - casts <= 1
}

/// Does mana spent on this cast a spell (so that spell can be given a rider, 106.6)?
pub fn casts(on: &ir::Coord<SpendOn>) -> bool {
    on.items.iter().any(|u| matches!(u, SpendOn::Cast(_)))
}

/// "to cast artifact spells or activate abilities of artifacts", "to cast a Dragon spell or to
/// activate an ability of a Dragon": the second "to" is an alias (CR 106.6).
pub fn spend_to() -> P<ir::Coord<SpendOn>> {
    let on = spend_on();
    let more = |word: &str, conj: ir::Conj| {
        let sep = alt("or to", vec![lit(word), lit(&format!("{word} to")).alias()]);
        on.then(&sep.then_keep(&on).many()).map(
            move |(first, rest)| {
                let mut items = vec![first];
                items.extend(rest);
                (items.len() >= 2 && distinct(&items)).then(|| ir::Coord { items, conj })
            },
            move |c: &ir::Coord<SpendOn>| {
                (c.conj == conj && c.items.len() >= 2 && distinct(&c.items)).then(|| (c.items[0].clone(), c.items[1..].to_vec()))
            },
        )
    };
    let one = on.map(|u| Some(ir::Coord::one(u)), |c: &ir::Coord<SpendOn>| match c.items.as_slice() {
        [u] if c.conj == ir::Conj::And => Some(u.clone()),
        _ => None,
    });
    lit("to").then_keep(&alt("spend on list", vec![one, more("or", ir::Conj::Or), more("and/or", ir::Conj::AndOr), more("and", ir::Conj::And)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn costs() {
        for t in ["{2}{U}{U}", "{X}{R}", "{W/U}{W/P}", "{0}"] {
            let Outcome::Accepted { value, .. } = mana_cost().parse_all(&tokenize(t)) else { panic!("{t}") };
            assert_eq!(mana_cost().print_text(&value, false).as_deref(), Some(t));
        }
        assert!(matches!(mana_cost().parse_all(&tokenize("{T}")), Outcome::Declined { .. }));
    }

    fn sentence_reads(t: &str) -> Option<String> {
        let s = &crate::effect::effects().sentence;
        let Outcome::Accepted { value, .. } = s.parse_all(&tokenize(t)) else { return None };
        s.print_text(&value, false)
    }

    /// 106.6: mana restricted to what it may be spent on; the use is typed (601.2, 602.2).
    #[test]
    fn r106_6_spend_restrictions() {
        for t in [
            "Spend this mana only to cast a creature spell",
            "Spend this mana only to cast creature spells",
            "Spend this mana only to cast artifact spells or activate abilities of artifacts",
            "Spend this mana only to cast a Dragon spell or activate an ability of a Dragon",
            "Spend this mana only to cast instant and sorcery spells",
            "Spend this mana only to cast spells with mana value 4 or greater",
            "This mana can't be spent to cast nonartifact spells",
            "Spend this mana only to cast Elf spells and activate abilities of Elf sources",
            "Spend only black mana on X",
            "Spend only black and/or red mana on X",
            "Spend only colored mana on X",
            "Spend only mana produced by basic lands to cast ~",
            "Spend only mana of the chosen color to activate this ability",
        ] {
            assert_eq!(sentence_reads(t).map(|p| p.to_lowercase()), Some(t.to_lowercase()), "{t}");
        }
        // the second "to" is the same meaning, printed once
        assert_eq!(
            sentence_reads("Spend this mana only to cast a Dragon spell or to activate an ability of a Dragon").as_deref(),
            Some("spend this mana only to cast a Dragon spell or activate an ability of a Dragon")
        );
        // mana is spent on spells and abilities, not on players or permanents
        for t in [
            "Spend this mana only to cast target player",
            "Spend this mana only to cast a creature",
            "Spend this mana only to activate target creature",
            "Spend this mana only to destroy a creature",
            "Spend this mana only to activate abilities of players",
            "Spend this mana only to activate abilities and activate abilities",
            "Spend this mana only to cast Elf spells and cast Dragon spells",
            "Spend this mana only to activate abilities, and that spell can't be countered",
            "Spend only black mana on a creature",
            "Spend only black mana to X",
        ] {
            assert_eq!(sentence_reads(t), None, "{t}");
        }
    }

    /// 106.6, 702, 113.3: mana may be restricted to abilities named by what they are, a keyword's
    /// own ability or a kind of ability; an ability is not named by what its source is.
    #[test]
    fn r106_6_spend_on_abilities_named_by_what_they_are() {
        for t in [
            "Spend this mana only to activate equip abilities",
            "Spend this mana only to activate power-up abilities",
            "Spend this mana only to activate loyalty abilities",
            "Spend this mana only to activate abilities",
            "Spend this mana only to activate this ability",
        ] {
            assert_eq!(sentence_reads(t).map(|p| p.to_lowercase()), Some(t.to_lowercase()), "{t}");
        }
        for t in [
            "Spend this mana only to activate creature abilities",
            "Spend this mana only to activate black abilities",
            "Spend this mana only to activate equip abilities you control",
            "Spend this mana only to activate equip spells",
            "Spend this mana only to activate flying",
        ] {
            assert_eq!(sentence_reads(t), None, "{t}");
        }
    }

    /// 106.4: the pool empties as steps and phases end; unspent mana is what remains in it.
    #[test]
    fn r106_4_unspent_mana() {
        for t in [
            "You don't lose unspent red mana as steps and phases end",
            "You don't lose this mana as steps and phases end",
            "Each player loses all unspent mana",
            "~ gets +1/+1 for each unspent green mana you have",
            "~ has flying as long as you have six or more unspent mana",
            "Destroy target artifact if its mana value is equal to the amount of unspent mana you have",
        ] {
            assert_eq!(sentence_reads(t).map(|p| p.to_lowercase()), Some(t.to_lowercase()), "{t}");
        }
        // mana is lost, kept or counted; a creature or a card has none
        for t in [
            "Target player loses all unspent creature",
            "You don't lose unspent cards as steps and phases end",
            "Draw a card for each unspent mana",
            "~ gets +1/+1 for each unspent mana target creature has",
        ] {
            assert_eq!(sentence_reads(t), None, "{t}");
        }
    }

    #[test]
    fn specs() {
        for t in [
            "{G}",
            "{C}{C}",
            "{R} or {G}",
            "{W}, {U}, or {B}",
            "one mana of any color",
            "X mana of any one color",
            "two mana in any combination of colors",
            "one mana of the chosen color",
            "one mana of that color",
            "an amount of {G} equal to ~'s power",
            "that much {C}",
            "two mana in any combination of {R} and/or {G}",
            "one mana of any color in your commander's color identity",
        ] {
            let Outcome::Accepted { value, .. } = mana_spec().parse_all(&tokenize(t)) else { panic!("{t}") };
            assert_eq!(mana_spec().print_text(&value, false).as_deref(), Some(t));
        }
    }

    fn spec(t: &str) -> Option<ManaSpec> {
        let Outcome::Accepted { value, .. } = mana_spec().parse_all(&tokenize(t)) else { return None };
        assert_eq!(mana_spec().print_text(&value, false).as_deref(), Some(t), "{t} prints as it reads");
        Some(value)
    }

    fn reads(t: &str) -> Option<ManaSpec> {
        match mana_spec().parse_all(&tokenize(t)) {
            Outcome::Accepted { value, .. } => Some(value),
            _ => None,
        }
    }

    /// 107.4a, 107.4c: a number before a symbol is that many of it, the same mana as the run of
    /// symbols; a run of six or more of one symbol is said with the number.
    #[test]
    fn r107_4a_a_number_before_a_symbol_is_that_many_of_it() {
        let run = |symbol: ManaSymbol, n: usize| ManaSpec::Symbols(ManaCost(vec![symbol; n]));
        let (green, red) = (ManaSymbol::Color(ir::Color::Green), ManaSymbol::Color(ir::Color::Red));
        assert_eq!(spec("six {G}"), Some(run(green, 6)));
        assert_eq!(spec("eight {C}"), Some(run(ManaSymbol::Colorless, 8)));
        assert_eq!(spec("{G}{G}"), Some(run(green, 2)));
        // one meaning, one value: each spelling reads, one prints (Geosurge writes the run)
        assert_eq!(reads("{R}{R}{R}{R}{R}{R}{R}"), Some(run(red, 7)));
        assert_eq!(mana_spec().print_text(&run(red, 7), false).as_deref(), Some("seven {R}"));
        assert_eq!(reads("two {G}"), Some(run(green, 2)));
        assert_eq!(mana_spec().print_text(&run(green, 2), false).as_deref(), Some("{G}{G}"));
        // six different symbols are no count of one
        assert_eq!(spec("{R}{R}{R}{G}{G}{G}").map(|s| matches!(s, ManaSpec::Symbols(_))), Some(true));
        for t in ["zero {G}", "six {G}{G}", "six {T}", "that many {G}"] {
            assert_eq!(reads(t), None, "{t}");
        }
    }

    /// 107.1, 107.3: how much mana the text does not fix is X or "that much" before what it is
    /// of, or "an amount of" it "equal to" any other amount; the same before a symbol and
    /// before "mana".
    #[test]
    fn r107_3_how_much_mana_is_said_the_same_way_of_a_symbol_and_of_a_type() {
        let green = ManaSymbol::Color(ir::Color::Green);
        assert_eq!(spec("X {G}"), Some(ManaSpec::AmountOf { symbol: green, amount: Amount::X }));
        assert_eq!(spec("X mana of any one color"), Some(ManaSpec::Of { amount: Amount::X, color: ManaColor::AnyOne }));
        assert!(matches!(spec("an amount of {G} equal to ~'s power"), Some(ManaSpec::AmountOf { amount: Amount::Stat { .. }, .. })));
        assert!(matches!(
            spec("an amount of mana of that color equal to your devotion to that color"),
            Some(ManaSpec::Of { amount: Amount::Devotion { to: ir::amount::DevotionTo::ThatColor, .. }, color: ManaColor::That })
        ));
        for t in ["twice that much {R}", "that much mana of any one color"] {
            assert!(spec(t).is_some(), "{t}");
        }
        // arithmetic is an amount like any other after "equal to" (the amount grammar prints
        // its number as a numeral)
        let plus_one = Amount::Plus(Box::new(Amount::X), Box::new(Amount::Const(1)));
        assert_eq!(spec("an amount of {C} equal to X plus 1"), Some(ManaSpec::AmountOf { symbol: ManaSymbol::Colorless, amount: plus_one.clone() }));
        let Outcome::Accepted { value, .. } = mana_spec().parse_all(&tokenize("an amount of {C} equal to X plus one")) else { panic!("Kyren Toy") };
        assert_eq!(value, ManaSpec::AmountOf { symbol: ManaSymbol::Colorless, amount: plus_one });
        // a number and X are never said with "equal to", other arithmetic never stands before
        // the mana, "that many" counts things and not mana, and an amount of nothing is no mana
        for t in [
            "an amount of {G} equal to two",
            "an amount of mana of that color equal to X",
            "X plus one {C}",
            "twice X mana of any one color",
            "that many mana of any one color",
            "six",
            "six mana",
            "an amount of {G}",
        ] {
            assert_eq!(reads(t), None, "{t}");
        }
    }

    /// 106.1b, 105.2, 106.7: the type of each mana is chosen among types the text names: the
    /// colors, the colors an object is, the colors among a set, what a permanent produced or
    /// could produce. One mana's choice is said "of any …", several mana's "in any
    /// combination of …": one meaning, spelled by number.
    #[test]
    fn r106_1b_the_types_a_mana_is_chosen_among() {
        let types = |t: &str| match spec(t) {
            Some(ManaSpec::Of { color: ManaColor::Any(types), .. }) => Some(types),
            _ => None,
        };
        assert_eq!(types("one mana of any color"), Some(ManaTypes::Colors));
        assert_eq!(types("two mana in any combination of colors"), Some(ManaTypes::Colors));
        assert_eq!(types("one mana of any color in your commander's color identity"), Some(ManaTypes::CommanderIdentity));
        for t in ["one mana of any of the exiled card's colors", "one mana of any of the exiled cards' colors", "three mana in any combination of its colors"] {
            assert!(matches!(types(t), Some(ManaTypes::ColorsOf(_))), "{t}");
        }
        for t in ["one mana of any color among legendary permanents you control", "one mana of any color among legendary creature cards in your graveyard"] {
            assert!(matches!(types(t), Some(ManaTypes::ColorsAmong(_))), "{t}");
        }
        for (t, could) in [
            ("one mana of any type that land produced", false),
            ("one mana of any color that a land an opponent controls could produce", true),
            ("one mana of any type the sacrificed land could produce", true),
        ] {
            assert!(matches!(types(t), Some(ManaTypes::Produced { could: c, .. }) if c == could), "{t}");
        }
        assert_eq!(spec("three mana of that type"), Some(ManaSpec::Of { amount: Amount::Const(3), color: ManaColor::ThatType }));
        // the spelling follows the number of mana
        for t in ["two mana of any color", "one mana in any combination of colors", "X mana of any of its colors", "one mana in any combination of {R} and/or {G}"] {
            assert_eq!(reads(t), None, "{t}");
        }
        // colors are an object's or a set of objects' (105.2) and a permanent produces mana
        // (106.7), never a player; "that" introduces an indefinite producer only
        for t in [
            "one mana of any of the exiled card's",
            "one mana of any of colors",
            "one mana of any color among target creature",
            "one mana of any of target opponent's colors",
            "one mana of any of your colors",
            "one mana of any color among players",
            "one mana of any type that a player could produce",
            "one mana of any type that the sacrificed land could produce",
            "one mana of any type a land you control could produce",
            "one mana of that types",
            "two mana in any combination of color",
        ] {
            assert_eq!(reads(t), None, "{t}");
        }
    }

    /// 608.2d: an "add" that offers a choice offers whole productions, of any of the kinds.
    /// The grammar keeps the list flat: no alternative is itself a list.
    #[test]
    fn r608_2d_one_of_several_productions() {
        let white = ManaSpec::Symbols(ManaCost(vec![ManaSymbol::Color(ir::Color::White)]));
        assert_eq!(
            spec("{W} or one mana of the chosen color"),
            Some(ManaSpec::OneOf(vec![white, ManaSpec::Of { amount: Amount::Const(1), color: ManaColor::Chosen }]))
        );
        assert!(matches!(spec("X {G} or X {W}"), Some(ManaSpec::OneOf(v)) if v.iter().all(|one| matches!(one, ManaSpec::AmountOf { amount: Amount::X, .. }))));
        assert!(matches!(spec("{W}, {U}, or {B}"), Some(ManaSpec::OneOf(v)) if v.len() == 3));
        for t in ["{W} or", "or {W}", "{W} or {U} or {B}", "{W} or one mana"] {
            assert_eq!(reads(t), None, "{t}");
        }
    }
}
