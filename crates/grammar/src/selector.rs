//! The selector grammar: noun phrases that name objects and players.
//!
//! ```text
//! selector  := det(n) core(n) post*          "up to two target nonland permanents you don't control"
//!            | special                       "you", "~", "any target", "defending player"
//! set       := ["other"] core(n) post*       bare: "Creatures you control" (a static's affected set)
//! core(n)   := adj* types(sg) head(n)        "nontoken artifact or creature cards"
//!            | adj* types(n)                 "legendary creatures"
//!            | adj* head(n)                  "nonland permanent"
//!            | term (or term)* head(n)       "a land or legendary Turtle card" (terms of any kind)
//!            | core(n) (or core(n))*         "a basic land card or Gate card" (a head per term)
//! post      := controller | owner | with | zone | "of their choice" | "at random"
//!            | relative                      "that's attacking", "that isn't a Demon, Devil, or Imp"
//!            | event relative                "that died this turn", "who lost life this turn"
//! with      := "with flying" | "with the same name as" sel | "that targets" sel | "blocking" sel
//!            | "with the greatest power among" set | "with different names" (collective) | …
//! ```
//!
//! Grammatical number is threaded through: a determiner fixes it ("two target creatures", "each
//! creature"), the last noun inflects for it, and printing chooses the branch the value can be
//! expressed in. Prenominal items and "with" items are one conjunction in the IR, in text order;
//! printing puts each item back where its kind goes (adjectives and types before the head,
//! keywords and stats after), so the split is determined by the value. A copular relative clause
//! adds its complement's items to the same conjunction; it prints after the head only when they
//! cannot stand before it ("creature that's attacking", but "permanent that's an artifact" prints
//! "artifact"). A postmodifier ending in a nested noun phrase comes last (right association).

use ir::reference::{Kind as ReferenceKind, Qualifier, RefForm, UnresolvedRef};
use crate::kernel::name_run;
use crate::kernel::{P, alt, empty, forward, leaf, lit, table};
use crate::words::{Number, adjective, own_reference, count_amount, digits, inflect, stat_amount, type_word};
use ir::amount::When;
use ir::condition::{Measure, Outcome};
use ir::event::{Arg, Deed};
use ir::{DamageKind, EventAction};
use ir::lexicon::{Inflection, Role, Verb};
use ir::selector::{Anaphor, Attachment, Chosen, CounterKind, Name, Relation, Slice, Stat};
use ir::{Amount, Atom, Characteristic, Cmp, Count, Domain, Filter, Keyword, ObjectSet, Quant, Ref, Selector, Status, Zone, ZoneSpec};
use oracle::token::Kind;
use std::sync::OnceLock;

/// Whether a zone postmodifier is spelled "in your graveyard" or "from your graveyard". The
/// meaning is the same (the object is in that zone); motion verbs print "from".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZonePrep {
    In,
    From,
    /// "from anywhere other than your hand": a cast's origin, every zone but that one.
    OtherThan,
}

pub struct Selectors {
    /// Selectors whose zone is spelled "in …".
    pub selector: P<Selector>,
    /// One predicate role, including explicit complete noun-phrase alternatives.
    /// Enclosing coordination/quantity/choice grammars use `selector` instead.
    pub predicate: P<Selector>,
    /// Selectors whose zone is spelled "from …".
    pub selector_from: P<Selector>,
    /// A bare singular set description ("Enchant creature", "protection from creatures" aside).
    pub set_sg: P<ObjectSet>,
    /// A bare plural set description: a static's affected set ("Creatures you control").
    pub set_pl: P<ObjectSet>,
    /// A bare plural set whose zone is spelled "from …": "cards from the top of your library".
    pub set_pl_from: P<ObjectSet>,
    /// A bare singular set after "each" in "for each": unions print with "and", as after the
    /// determiner "each" ("for each instant and sorcery card in your graveyard").
    pub set_each: P<ObjectSet>,
    /// A possessive player: "your", "their", "target opponent's", "that player's".
    pub possessive: P<Selector>,
    /// Singular selectors ("a creature", "~", "defending player"), for subject-verb agreement;
    /// "you" is not among them (it takes the base form of the verb).
    pub singular: P<Selector>,
    /// Plural selectors ("one or more creatures", "your opponents").
    pub plural: P<Selector>,
    /// A zone phrase spelled with "in": "in your graveyard", "on the battlefield", "in hand".
    pub zone: P<ZoneSpec>,
}

pub fn selectors() -> &'static Selectors {
    static S: OnceLock<Selectors> = OnceLock::new();
    S.get_or_init(build)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Head {
    Permanent,
    Card,
    Spell,
    Token,
    Player,
    Opponent,
    Source,
    Ability,
    /// "commander": a card designated as a commander (CR 903.3), in whatever zone.
    Commander,
    /// "outlaw": an object with one of the creature types the rules list (CR 700.12).
    Outlaw,
    /// "that target", "the target": something targeted, of whatever kind.
    Target,
    /// "a spell or ability".
    SpellOrAbility,
    PlayerOrPlaneswalker,
    OpponentOrPlaneswalker,
    PermanentOrPlayer,
    SpellOrPermanent,
    /// "spell or nonland permanent".
    SpellOrNonlandPermanent,
    PlayerOrBattle,
}

fn head(n: Number) -> P<Head> {
    let mut e = Vec::new();
    for (w, h) in [
        ("permanent", Head::Permanent),
        ("card", Head::Card),
        ("spell", Head::Spell),
        ("token", Head::Token),
        ("player", Head::Player),
        ("opponent", Head::Opponent),
        ("source", Head::Source),
        ("commander", Head::Commander),
        ("outlaw", Head::Outlaw),
        ("target", Head::Target),
    ] {
        e.push((inflect(w, n), h));
    }
    e.push((
        if n == Number::Sg { "spell or ability" } else { "spells or abilities" }.to_string(),
        Head::SpellOrAbility,
    ));
    // A plural subject can name the same union with "and" (causal prohibitions).
    if n == Number::Pl { e.push(("spells and abilities".to_string(), Head::SpellOrAbility)); }
    e.push((if n == Number::Sg { "ability" } else { "abilities" }.to_string(), Head::Ability));
    if n == Number::Sg {
        e.push(("player or planeswalker".to_string(), Head::PlayerOrPlaneswalker));
        e.push(("opponent or planeswalker".to_string(), Head::OpponentOrPlaneswalker));
        e.push(("permanent or player".to_string(), Head::PermanentOrPlayer));
        e.push(("spell or permanent".to_string(), Head::SpellOrPermanent));
        e.push(("spell or nonland permanent".to_string(), Head::SpellOrNonlandPermanent));
        e.push(("player or battle".to_string(), Head::PlayerOrBattle));
    }
    table("head noun", e)
}

/// A type group: juxtaposed type words, all of which apply ("artifact creature", "Human
/// Soldier"); only the last inflects.
fn type_group(n: Number, permanent: bool) -> P<Filter> {
    type_word(Number::Sg, permanent).many().then(&type_word(n, permanent)).map(
        |(mut v, last)| {
            v.push(last);
            Some(group(v))
        },
        |f| {
            let mut atoms = ungroup(f)?;
            let last = atoms.pop()?;
            Some((atoms, last))
        },
    )
}

fn group(mut atoms: Vec<Atom>) -> Filter {
    if atoms.len() == 1 {
        Filter::Atom(atoms.pop().expect("one"))
    } else {
        Filter::All(atoms.into_iter().map(Filter::Atom).collect())
    }
}

fn is_type_atom(a: &Atom) -> bool {
    matches!(a, Atom::Type(_) | Atom::Subtype(_) | Atom::PermanentType)
}

fn ungroup(f: &Filter) -> Option<Vec<Atom>> {
    match f {
        Filter::Atom(a) if is_type_atom(a) => Some(vec![a.clone()]),
        Filter::All(v) if v.len() >= 2 => v
            .iter()
            .map(|x| match x {
                Filter::Atom(a) if is_type_atom(a) => Some(a.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// How a union of types is spelled, which follows the determiner: "target artifact or creature",
/// "two artifacts and/or creatures", "all artifacts and enchantments". Every spelling parses in
/// every context; this decides only which one prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Union {
    /// Singulars, except after "each".
    Or,
    /// Counted plurals: "two", "one or more", "any number of target".
    AndOr,
    /// "each", and generic plurals: "all", "those", bare "artifacts and creatures you control".
    And,
}

impl Union {
    /// The determiner's union spelling: "each creature and planeswalker", "target artifact or
    /// creature", "two artifacts and/or creatures", "all artifacts and enchantments".
    fn of(q: &Quant, num: Number) -> Union {
        match (q, num) {
            (Quant::Each, _) => Union::And,
            (_, Number::Sg) => Union::Or,
            (Quant::Target(_) | Quant::Indefinite(_), Number::Pl) => Union::AndOr,
            (_, Number::Pl) => Union::And,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Union::Or => "or",
            Union::AndOr => "and/or",
            Union::And => "and",
        }
    }
}

/// One type group, or a disjunction of them ("artifact or creature", "artifacts and/or
/// enchantments", "artifacts and enchantments").
fn types(n: Number, permanent: bool, union: Union) -> P<Filter> {
    let g = type_group(n, permanent);
    let any = |v: Vec<Filter>| (!has_shared_type_head(&v)).then_some(Filter::Any(v));
    let unany = |f: &Filter| match f {
        Filter::Any(v) => Some(v.clone()),
        _ => None,
    };
    let mut v = vec![g.clone(), g.list(union.word()).map(any, unany)];
    for u in [Union::Or, Union::AndOr, Union::And] {
        if u != union {
            v.push(g.list(u.word()).map(any, unany).alias());
        }
    }
    alt("types", v)
}

fn is_type_filter(f: &Filter) -> bool {
    ungroup(f).is_some() || matches!(f, Filter::Any(v) if v.len() >= 2 && v.iter().all(|g| ungroup(g).is_some()))
}

/// An adjective group: one adjective, a disjunction ("white or blue", "attacking or blocking"),
/// or a comma conjunction ("noncreature, nonland").
pub fn adjective_group() -> P<Filter> {
    adjectives(Union::Or)
}

/// The power and toughness a pair of predicates says a thing has: `power` is N and `toughness`
/// is M, both exactly, as one item of a noun phrase.
fn pt_pair(f: &Filter, power: Stat, toughness: Stat) -> Option<(u32, u32)> {
    let Filter::All(v) = f else { return None };
    match v.as_slice() {
        [
            Filter::Atom(Atom::Stat { stat: p, cmp: Cmp::Eq, value: Amount::Const(n) }),
            Filter::Atom(Atom::Stat { stat: t, cmp: Cmp::Eq, value: Amount::Const(m) }),
        ] if *p == power && *t == toughness => Some((*n, *m)),
        _ => None,
    }
}

/// Two or more stats joined by "or": "power or toughness", "mana value, power, or toughness".
/// "base" said once is said of each, as in rule 208.4's "base power and toughness" (Sword of
/// the Squeak: "base power or toughness 1").
fn stat_list() -> P<Vec<Stat>> {
    let base_of = |stat: &Stat| match stat {
        Stat::Power => Some(Stat::BasePower),
        Stat::Toughness => Some(Stat::BaseToughness),
        _ => None,
    };
    let plain_of = |stat: &Stat| match stat {
        Stat::BasePower => Some(Stat::Power),
        Stat::BaseToughness => Some(Stat::Toughness),
        _ => None,
    };
    // "base power or toughness" is the base of both, never base power or current toughness: a
    // list that names a base stat is said with "base" once
    let any_base = |v: &Vec<Stat>| v.iter().any(|s| matches!(s, Stat::BasePower | Stat::BaseToughness));
    let based = lit("base").then_keep(&stat().list("or")).map(
        move |v| v.iter().map(base_of).collect::<Option<Vec<Stat>>>(),
        move |v: &Vec<Stat>| v.iter().map(plain_of).collect::<Option<Vec<Stat>>>(),
    );
    // each stat is named once
    let distinct = |v: &Vec<Stat>| v.iter().enumerate().all(|(i, s)| !v[..i].contains(s));
    let said = stat().list("or").map(
        move |v| (!any_base(&v) && distinct(&v)).then_some(v),
        move |v: &Vec<Stat>| (!any_base(v) && distinct(v)).then(|| v.clone()),
    );
    alt("stats", vec![based.map(move |v| distinct(&v).then_some(v), move |v: &Vec<Stat>| distinct(v).then(|| v.clone())), said])
}

/// The stats of a disjunction of stat predicates that share one comparison and one value.
fn shared_comparison(f: &Filter) -> Option<(Vec<Stat>, Cmp, Amount)> {
    let Filter::Any(v) = f else { return None };
    let Some(Filter::Atom(Atom::Stat { cmp, value, .. })) = v.first() else { return None };
    let stats = v
        .iter()
        .map(|item| match item {
            Filter::Atom(Atom::Stat { stat, cmp: c, value: a }) if c == cmp && a == value => Some(*stat),
            _ => None,
        })
        .collect::<Option<Vec<Stat>>>()?;
    (stats.len() >= 2).then(|| (stats, *cmp, value.clone()))
}

/// "N/M": power N and toughness M (CR 208.1: "2/3 means the object has power 2 and toughness
/// 3"), of these two stats: the current ones, or the base ones (208.4b).
fn pt_predicate(power: Stat, toughness: Stat) -> P<Filter> {
    let eq = |stat: Stat, n: u32| Filter::Atom(Atom::Stat { stat, cmp: Cmp::Eq, value: Amount::Const(n) });
    digits().skip(&lit("/").tight()).then(&digits().tight()).map(
        move |(n, m)| Some(Filter::All(vec![eq(power, n), eq(toughness, m)])),
        move |f| pt_pair(f, power, toughness),
    )
}

/// Adjectives join with "or" ("white or blue"), or "and/or" after a counted plural ("two untapped
/// white and/or blue creatures"); "and" between adjectives is not a union.
fn adjectives(union: Union) -> P<Filter> {
    // "1/1" stands where an adjective does: "target 1/1 creature"
    let a = alt("adjective", vec![adjective(), pt_predicate(Stat::Power, Stat::Toughness)]);
    let (canonical, other) = if union == Union::AndOr { ("and/or", "or") } else { ("or", "and/or") };
    let any = |v: Vec<Filter>| Some(Filter::Any(v));
    let unany = |f: &Filter| match f {
        Filter::Any(v) if v.iter().all(|x| !is_type_filter(x)) => Some(v.clone()),
        _ => None,
    };
    alt(
        "adjectives",
        vec![
            a.clone(),
            a.list(canonical).map(any, unany),
            a.list(other).map(any, unany).alias(),
            a.comma_list().map(
                |v| Some(Filter::All(v)),
                |f| match f {
                    Filter::All(v) if v.iter().all(|x| !is_type_filter(x)) => Some(v.clone()),
                    _ => None,
                },
            ),
        ],
    )
}

/// The noun phrase without determiner or postmodifiers, as (domain, conjunction items).
fn core(n: Number, union: Union) -> P<(Domain, Vec<Filter>)> {
    // "a kicked spell": prenominal only ("~ was kicked" is a fact about its casting)
    let kicked = lit("kicked").to(Filter::Atom(Atom::Status(Status::Kicked)));
    // "a suspended card": in exile with suspend and a time counter (702.62b); a condition's "~ is
    // suspended" is the designation of the conditions
    let suspended = lit("suspended").to(Filter::Atom(Atom::Status(Status::Suspended)));
    // "target face-up exiled card": a card in exile (moved to the set's zone when assembled)
    let exiled = lit("exiled").to(exiled_adjective());
    let adjs = alt("adjectives", vec![adjectives(union), kicked, suspended, exiled, lit("prepared").to(Filter::Atom(Atom::PreparedSpell)), ability_keywords()]).many();
    let with_head = adjs.then(&types(Number::Sg, true, union).opt()).then(&head(n));
    let types_only = adjs.then(&types(n, false, union));
    let mut forms = vec![
            with_head.map(
                |((adjs, types), head)| core_to(adjs, types, Some(head)),
                |c| match core_from(c)? {
                    (adjs, types, Some(head)) => Some(((adjs, types), head)),
                    _ => None,
                },
            ),
            types_only.map(
                |(adjs, types)| core_to(adjs, Some(types), None),
                |c| match core_from(c)? {
                    (adjs, Some(types), None) => Some((adjs, types)),
                    _ => None,
                },
            ),
    ];
    // only phrases with a head noun: "artifacts and enchantments" is one type union
    let one = forms[0].clone();
    // "a land or legendary Turtle card", "a blue or artifact card", "artifact, enchantment, or
    // tapped creature": terms of different kinds (an adjective, a type group, either with its own
    // adjectives) under one head noun or, when every term has its own type, none
    let head_terms = unit_union(&unit(Number::Sg, true, true), union, |v| new_union(v, true));
    forms.push(head_terms.then(&head(n)).map(
        // "permanent" is the noun of a phrase that names no type ("artifact permanent" is not
        // said): a union that names types has no "permanent" to follow
        |(terms, head)| (head != Head::Permanent).then(|| core_to(vec![terms], None, Some(head))).flatten(),
        |c| match core_from(c)? {
            (adjs, None, Some(head)) if head != Head::Permanent => match adjs.as_slice() {
                [terms] => Some((terms.clone(), head)),
                _ => None,
            },
            _ => None,
        },
    ));
    let type_terms = unit_union(&unit(n, false, false), union, |v| new_union(v, false));
    forms.push(type_terms.map(
        |terms| core_to(vec![terms], None, None),
        |c| match core_from(c)? {
            (adjs, None, Some(Head::Permanent)) => match adjs.as_slice() {
                [terms] => Some(terms.clone()),
                _ => None,
            },
            _ => None,
        },
    ));
    // "Elemental spells and Warrior spells (you cast)", "a basic land card or Gate card": whole
    // noun phrases of one domain, each with its own modifiers, sharing the determiner and what
    // follows; an object in either. The union is taken over terms, not over one modifier.
    for u in std::iter::once(union).chain([Union::Or, Union::AndOr].into_iter().filter(|u| *u != union)) {
        let list = one.list(u.word()).map(
            |cores| {
                let domain = cores[0].0.clone();
                // a prenominal "exiled" belongs to the whole phrase ([`absorb_exiled`]), not to a term
                let exiled = exiled_adjective();
                if cores.iter().any(|(d, _)| *d != domain) {
                    if cores.iter().any(|(_, items)| items.contains(&exiled)) { return None; }
                    let arms = cores.into_iter().map(|(domain, items)| ObjectSet { filter: Filter::All(items), ..ObjectSet::of(domain) }).collect();
                    return Some((Domain::Union(arms), Vec::new()));
                }
                cores.iter().all(|(_, items)| !items.is_empty() && !items.contains(&exiled)).then_some(())?;
                Some((domain, vec![Filter::Any(cores.into_iter().map(|(_, items)| Filter::All(items)).collect())]))
            },
            |(domain, items)| match (domain, items.as_slice()) {
                (Domain::Union(arms), []) if arms.len() >= 2 && arms.iter().any(|arm| arm.domain != arms[0].domain) => arms.iter().map(|arm| {
                    if arm != &(ObjectSet { filter: arm.filter.clone(), ..ObjectSet::of(arm.domain.clone()) }) { return None; }
                    let Filter::All(items) = &arm.filter else { return None; };
                    Some((arm.domain.clone(), items.clone()))
                }).collect(),
                (_, [Filter::Any(v)]) if v.len() >= 2 => v
                    .iter()
                    .map(|f| match f {
                        Filter::All(items) if !items.is_empty() => Some((domain.clone(), items.clone())),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            },
        );
        forms.push(if u == union { list } else { list.alias() });
    }
    // A common type noun constrains every alternative, independently of whether its
    // modifiers are subtypes, colors or supertypes (109.2, 205.2, 205.3m, 608.2c).
    // It is singular inside "... creature cards" but inflects in "... creatures".
    for with_head in [false, true] {
        let suffix_number = if with_head { Number::Sg } else { n };
        let modifier = alt("noun modifier", vec![
            adjective(),
            type_word(Number::Sg, false).map(|a| Some(Filter::Atom(a)), |f| match f {
                Filter::Atom(a) if is_type_atom(a) => Some(a.clone()), _ => None,
            }),
        ]);
        let modifiers = unit_union(&modifier, union, shared_modifiers);
        let noun = type_word(suffix_number, false).map(
            |a| matches!(a, Atom::Type(_)).then_some(Filter::Atom(a)),
            |f| match f { Filter::Atom(a @ Atom::Type(_)) => Some(a.clone()), _ => None },
        );
        let tail = if with_head { head(n).map(|h| Some(Some(h)), |h| *h) } else { crate::kernel::empty().to(None) };
        forms.push(adjs.then(&modifiers).then(&noun).then(&tail).map(
            |(((mut prefix, modifiers), noun), head)| {
                prefix.push(modifiers);
                core_to(prefix, Some(noun), head)
            },
            move |c| match core_from(c)? {
                (modifiers, Some(noun @ Filter::Atom(Atom::Type(_))), head)
                    if head.is_some() == with_head => match modifiers.split_last() {
                        Some((m @ Filter::Any(v), prefix)) if shared_modifiers(v) => Some((((prefix.to_vec(), m.clone()), noun), head)),
                        _ => None,
                    },
                _ => None,
            },
        ));
    }
    alt("noun phrase", forms)
}

/// A common noun can complete coordinated modifiers: "Ninja or Rogue creature",
/// "black or artifact creature", "snow and Zombie creatures". Whole type nouns
/// ("artifact or enchantment creature") retain their separate noun-phrase reading.
/// Homogeneous adjective coordination already has its canonical form in `adjectives`.
fn shared_modifiers(terms: &[Filter]) -> bool {
    let subtype = |f: &Filter| matches!(f, Filter::Atom(Atom::Subtype(_)));
    let type_word = |f: &Filter| matches!(f, Filter::Atom(Atom::Type(_) | Atom::Subtype(_)));
    terms.len() >= 2 && terms.iter().all(|f| type_word(f) || is_adjective(f))
        && terms.iter().any(type_word)
        && (terms.iter().all(subtype) || terms[..terms.len() - 1].iter().any(|f| !type_word(f)))
}

/// Legacy type-group coordination must not also read a common noun as belonging
/// only to its final arm. The shared-head rule owns that syntactic construction.
fn has_shared_type_head(groups: &[Filter]) -> bool {
    let Some((last, before)) = groups.split_last() else { return false };
    let Some(atoms) = ungroup(last) else { return false };
    let Some((Atom::Type(_), prefix)) = atoms.split_last() else { return false };
    if prefix.is_empty() || before.is_empty() { return false; }
    before.iter().all(|f| matches!(f, Filter::Atom(Atom::Subtype(_))))
        && prefix.iter().all(|a| matches!(a, Atom::Subtype(_)))
}

/// How a term of a union is built: an adjective alone ("red"), a type group ("Gate", "artifact
/// creature"), or adjectives before a type group ("legendary Turtle"). A term is the conjunction
/// of its items, as the terms of whole noun phrases are ("Elemental spells and Warrior spells").
fn unit(n: Number, permanent: bool, adjective_alone: bool) -> P<Filter> {
    adjective().many().then(&type_group(n, permanent).opt()).map(
        move |(mut adjs, group)| match group {
            None => (adjective_alone && adjs.len() == 1).then(|| Filter::All(adjs)),
            Some(group) => {
                adjs.push(group);
                Some(Filter::All(adjs))
            }
        },
        move |f: &Filter| {
            let Filter::All(v) = f else { return None };
            match v.split_last() {
                Some((group, adjs)) if ungroup(group).is_some() => Some((adjs.to_vec(), Some(group.clone()))),
                _ => (adjective_alone && v.len() == 1).then(|| (v.clone(), None)),
            }
        },
    )
}

#[derive(PartialEq)]
enum Term {
    /// "Gate", "artifact creature".
    Bare,
    /// "red".
    Adjective,
    /// "legendary Turtle".
    Typed,
}

fn term_kind(f: &Filter) -> Term {
    match f {
        Filter::All(v) => match v.split_last() {
            Some((group, rest)) if ungroup(group).is_some() => {
                if rest.is_empty() {
                    Term::Bare
                } else {
                    Term::Typed
                }
            }
            _ => Term::Adjective,
        },
        _ => Term::Typed,
    }
}

/// Whether a union of terms is one that the type group and adjective unions cannot say: those
/// read only terms of one kind ("white or blue", "artifact or creature") or leading adjectives
/// that apply to the whole union ("nontoken artifact or creature"). Without a head noun every
/// term must carry its own type.
fn new_union(terms: &[Filter], head: bool) -> bool {
    let kinds: Vec<Term> = terms.iter().map(term_kind).collect();
    // leading adjectives, then one term with its own type, then bare types: the adjective rule
    // reads these as adjectives ("white or blue creature", "nontoken artifact or creature") and
    // two or more of them, listed, as one conjunction ("non-Sliver, non-Prism, non-Attraction Book
    // Artificer")
    let lead = kinds.iter().take_while(|k| **k == Term::Adjective).count();
    let old = lead == kinds.len()
        || lead >= 2
        || kinds.iter().all(|k| *k == Term::Bare)
        || (kinds[lead] == Term::Typed && kinds[lead + 1..].iter().all(|k| *k == Term::Bare));
    !old && !shared_term_head(terms) && (head || kinds.iter().all(|k| *k != Term::Adjective))
}

/// The same shared-head construction, before legacy term groups are assembled.
fn shared_term_head(terms: &[Filter]) -> bool {
    let Some((Filter::All(last), before)) = terms.split_last() else { return false };
    let Some((group, adjs)) = last.split_last() else { return false };
    let Some(mut atoms) = ungroup(group) else { return false };
    if !matches!(atoms.pop(), Some(Atom::Type(_))) { return false; }
    let mut modifiers: Vec<Filter> = Vec::new();
    for t in before {
        let Filter::All(items) = t else { return false };
        let [item] = items.as_slice() else { return false };
        modifiers.push(item.clone());
    }
    match (adjs, atoms.as_slice()) {
        ([], [a]) => modifiers.push(Filter::Atom(a.clone())),
        ([a], []) => modifiers.push(a.clone()),
        _ => return false,
    }
    shared_modifiers(&modifiers)
}

/// Terms joined by the union word the determiner calls for, or by "or" and "and/or" (a plain
/// "and" between terms of a singular phrase joins properties: "a white and black creature").
fn unit_union(unit: &P<Filter>, union: Union, ok: fn(&[Filter]) -> bool) -> P<Filter> {
    let any = move |v: Vec<Filter>| ok(&v).then(|| Filter::Any(v));
    let unany = move |f: &Filter| match f {
        Filter::Any(v) if ok(v) => Some(v.clone()),
        _ => None,
    };
    let mut v = vec![unit.list(union.word()).map(any, unany)];
    for u in [Union::Or, Union::AndOr] {
        if u != union {
            v.push(unit.list(u.word()).map(any, unany).alias());
        }
    }
    alt("terms", v)
}

/// The prenominal "exiled" ("an exiled card"): in exile. Only a parse-time item; assembling the
/// noun phrase moves it to the set's zone, which prints "in exile".
fn exiled_adjective() -> Filter {
    Filter::Atom(Atom::InZone(ZoneSpec { zones: ir::Coord::one(Zone::Exile), owner: None, slice: None }))
}

/// Move a prenominal "exiled" to the set's zone; None when it cannot stand there (after "the",
/// which makes "the exiled card" the participle reference, or with another zone).
fn absorb_exiled(set: &mut ObjectSet, after_the: bool) -> Option<()> {
    if let Filter::All(items) = &mut set.filter
        && let Some(i) = items.iter().position(|f| *f == exiled_adjective())
    {
        if after_the || set.zone.is_some() || set.domain != Domain::Card {
            return None;
        }
        items.remove(i);
        set.zone = Some(ZoneSpec { zones: ir::Coord::one(Zone::Exile), owner: None, slice: None });
    }
    Some(())
}

fn mentions(f: &Filter, a: &Atom) -> bool {
    match f {
        Filter::Atom(x) => x == a,
        Filter::Not(b) => mentions(b, a),
        Filter::All(v) | Filter::Any(v) => v.iter().any(|x| mentions(x, a)),
    }
}

fn head_atom(h: Head) -> Option<Filter> {
    match h {
        Head::Token => Some(Filter::Atom(Atom::Status(Status::Token))),
        Head::Opponent => Some(Filter::Atom(Atom::Opponent)),
        Head::Commander => Some(Filter::Atom(Atom::Status(Status::Commander))),
        Head::Outlaw => Some(Filter::Atom(Atom::Status(Status::Outlaw))),
        _ => None,
    }
}

// Surface spellings map once to ordinary set expressions. No engine or lexicon
// consumer needs a separate enum case for these English noun combinations.
const COMPOUND_HEADS: &[Head] = &[Head::SpellOrAbility, Head::PlayerOrPlaneswalker,
    Head::OpponentOrPlaneswalker, Head::PermanentOrPlayer, Head::SpellOrPermanent,
    Head::SpellOrNonlandPermanent, Head::PlayerOrBattle];

fn head_domain(head: Head) -> Domain {
    let filtered = |domain, atom| ObjectSet { filter: Filter::All(vec![Filter::Atom(atom)]), ..ObjectSet::of(domain) };
    let permanent = |ty| filtered(Domain::Permanent, Atom::Type(ty));
    let union = |a, b| Domain::Union(vec![a, b]);
    let player = || ObjectSet::of(Domain::Player);
    let spell = || ObjectSet::of(Domain::Spell);
    match head {
        Head::Permanent | Head::Token | Head::Outlaw => Domain::Permanent,
        Head::Card | Head::Commander => Domain::Card,
        Head::Spell => Domain::Spell,
        Head::Player | Head::Opponent => Domain::Player,
        Head::Source => Domain::Source,
        Head::Ability => Domain::Ability,
        Head::Target => Domain::AnyTarget,
        Head::SpellOrAbility => union(spell(), ObjectSet::of(Domain::Ability)),
        Head::PlayerOrPlaneswalker => union(player(), permanent(ir::CardType::Planeswalker)),
        Head::OpponentOrPlaneswalker => union(filtered(Domain::Player, Atom::Opponent), permanent(ir::CardType::Planeswalker)),
        Head::PermanentOrPlayer => union(ObjectSet::of(Domain::Permanent), player()),
        Head::SpellOrPermanent => union(spell(), ObjectSet::of(Domain::Permanent)),
        Head::SpellOrNonlandPermanent => union(spell(), ObjectSet {
            filter: Filter::All(vec![Filter::Not(Box::new(Filter::Atom(Atom::Type(ir::CardType::Land))))]),
            ..ObjectSet::of(Domain::Permanent)
        }),
        Head::PlayerOrBattle => union(player(), permanent(ir::CardType::Battle)),
    }
}

fn core_to(adjs: Vec<Filter>, types: Option<Filter>, head: Option<Head>) -> Option<(Domain, Vec<Filter>)> {
    let domain = head_domain(head.unwrap_or(Head::Permanent));
    if adjs.contains(&Filter::Atom(Atom::PreparedSpell)) && domain != Domain::Spell { return None; }
    // players take no type words or adjectives; "creature permanent" is not English
    let mixed = domain == Domain::Player || domain == Domain::AnyTarget
        || matches!(&domain, Domain::Union(_));
    if mixed && (types.is_some() || !adjs.is_empty()) {
        return None;
    }
    if matches!(head, Some(Head::Permanent | Head::Commander | Head::Outlaw)) && types.is_some() {
        return None;
    }
    // "permanent" as a type word only before "card" / "spell" / "source"
    let permanent_type = types.as_ref().is_some_and(|t| mentions(t, &Atom::PermanentType));
    if permanent_type && !matches!(head, Some(Head::Card | Head::Spell | Head::Source)) {
        return None;
    }
    // "suspended" is said of a card in exile (702.62b), never of a permanent or a spell
    let suspended = Filter::Atom(Atom::Status(Status::Suspended));
    if adjs.contains(&suspended) && !matches!(head, Some(Head::Card)) {
        return None;
    }
    // "activated", "mana", "loyalty" … only before "ability"
    if head != Some(Head::Ability) && adjs.iter().any(names_ability_kind) {
        return None;
    }
    // "1/1" is said once, of something that can have power and toughness (208.1, 208.3): an
    // ability has none
    let is_pair = |f: &Filter| pt_pair(f, Stat::Power, Stat::Toughness).is_some();
    let pairs = adjs.iter().filter(|f| is_pair(f) || matches!(f, Filter::Any(v) | Filter::All(v) if v.iter().any(is_pair))).count();
    let nested = adjs.iter().any(|f| !is_pair(f) && matches!(f, Filter::Any(v) | Filter::All(v) if v.iter().any(is_pair)));
    if pairs > 1 || nested || (pairs == 1 && domain == Domain::Ability) {
        return None;
    }
    let mut items = adjs;
    items.extend(types);
    items.extend(head.and_then(head_atom));
    Some((domain, items))
}

fn names_ability_kind(f: &Filter) -> bool {
    match f {
        Filter::Atom(a) => matches!(a, Atom::AbilityKind(_) | Atom::Keyword(_)),
        Filter::Not(b) => names_ability_kind(b),
        Filter::All(v) | Filter::Any(v) => v.iter().any(names_ability_kind),
    }
}

type CoreParts = (Vec<Filter>, Option<Filter>, Option<Head>);

fn core_from((domain, items): &(Domain, Vec<Filter>)) -> Option<CoreParts> {
    let mut items = items.clone();
    let last_is = |items: &Vec<Filter>, f: &Filter| items.last() == Some(f);
    let head = match domain {
        Domain::Player if last_is(&items, &Filter::Atom(Atom::Opponent)) => {
            items.pop();
            Some(Head::Opponent)
        }
        Domain::Player => Some(Head::Player),
        Domain::Card if last_is(&items, &Filter::Atom(Atom::Status(Status::Commander))) => {
            items.pop();
            Some(Head::Commander)
        }
        Domain::Card => Some(Head::Card),
        Domain::Spell => Some(Head::Spell),
        Domain::Source => Some(Head::Source),
        Domain::Ability => Some(Head::Ability),
        Domain::Permanent if last_is(&items, &Filter::Atom(Atom::Status(Status::Token))) => {
            items.pop();
            Some(Head::Token)
        }
        Domain::Permanent if last_is(&items, &Filter::Atom(Atom::Status(Status::Outlaw))) => {
            items.pop();
            Some(Head::Outlaw)
        }
        Domain::Permanent => None,
        Domain::AnyTarget => Some(Head::Target),
        Domain::Union(_) => Some(*COMPOUND_HEADS.iter().find(|h| head_domain(**h) == *domain)?),
        Domain::Object => return None,
    };
    let types = if items.last().is_some_and(is_type_filter) { items.pop() } else { None };
    let head = match (head, &types) {
        (None, None) => Some(Head::Permanent),
        (h, _) => h,
    };
    Some((items, types, head))
}

/// A postmodifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Post {
    Controller(Relation),
    Owner(Relation),
    Caster(ir::selector::Casting),
    With(Filter),
    /// One surface phrase that supplies several ordinary postnominal predicates.
    WithAll(Vec<Filter>),
    Zone(ZoneSpec),
    Chooser(Selector),
    Random(ir::selector::Random),
    /// "that's attacking", "that's an artifact, creature, or enchantment", "that isn't a Demon,
    /// Devil, or Imp", "that aren't mana abilities": a copular relative clause, whose
    /// complement's items join the noun phrase's conjunction (negated: as one `Not`).
    Relative { negated: bool, complement: Complement },
    /// "that died this turn", "who lost life this turn": an [`Atom::Did`]; `who` after players.
    Event { who: bool, filter: Filter },
    /// "from a single graveyard": the zone, and [`Atom::SameZone`] for the selection.
    SingleZone(ZoneSpec),
    /// "you both own and control", "you control but don't own", "you own but don't control": one
    /// subject's controlling and owning (CR 108.3, 110.2) said together; at least one holds.
    OwnControl { who: Selector, owns: bool, controls: bool },
    /// "you own that your opponents control": whose it is and who controls it, said as a relative
    /// clause on the owner (the owner first).
    OwnedThatControlled { owner: Relation, controller: Relation },
}

/// The complement of a copular relative clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Complement {
    /// "attacking", "tapped and attacking", "white or blue".
    Adjectives(Vec<Filter>),
    /// "an artifact, creature, or enchantment", "a mana ability", "Spirits": a noun phrase
    /// without postmodifiers; `Domain::Permanent` when it names types only.
    Noun(Domain, Vec<Filter>),
}

/// "an exhaust ability", "an eternalize or embalm ability", "equip abilities": the ability that
/// is a keyword's own (CR 702: each keyword is an ability or a group of them). Only before the
/// head noun "ability" (checked where the noun phrase is assembled), as "activated" is.
fn ability_keywords() -> P<Filter> {
    let kw = keyword_name();
    let one = kw.map(
        |k| Some(Filter::Atom(Atom::Keyword(k))),
        |f| match f {
            Filter::Atom(Atom::Keyword(k)) => Some(*k),
            _ => None,
        },
    );
    let either = kw.list("or").map(
        |ks| Some(Filter::Any(ks.into_iter().map(|k| Filter::Atom(Atom::Keyword(k))).collect())),
        |f| match f {
            Filter::Any(v) => v
                .iter()
                .map(|x| match x {
                    Filter::Atom(Atom::Keyword(k)) => Some(*k),
                    _ => None,
                })
                .collect(),
            _ => None,
        },
    );
    alt("keyword ability", vec![one, either])
}

/// Whether this is what [`ability_keywords`] reads: of an ability, not of the object that has it.
fn keyword_ability(f: &Filter) -> bool {
    match f {
        Filter::Atom(Atom::Keyword(_)) => true,
        Filter::Any(v) => v.len() >= 2 && v.iter().all(|x| matches!(x, Filter::Atom(Atom::Keyword(_)))),
        _ => false,
    }
}

fn has_ability(f: &Filter) -> bool {
    match f {
        Filter::Atom(a) => matches!(a, Atom::HasAbility(_)),
        Filter::Not(b) => has_ability(b),
        _ => false,
    }
}

/// The kinds of ability a rule tells apart (CR 113.3, 605.1, 606.1), as a word before "ability".
fn ability_kind() -> P<ir::selector::AbilityKind> {
    use ir::selector::AbilityKind::*;
    table(
        "ability kind",
        vec![("activated".into(), Activated), ("triggered".into(), Triggered), ("mana".into(), Mana), ("loyalty".into(), Loyalty)],
    )
}

pub fn keyword_name() -> P<Keyword> {
    // the CR headings are title case; Oracle prints keywords in lower case, except a proper noun
    // ("For Mirrodin!")
    let spelled = |k: Keyword| match k.name() {
        "For Mirrodin!" => "for Mirrodin!".to_string(),
        n => n.to_lowercase(),
    };
    table("keyword", Keyword::all().map(|k| (spelled(k), k)).collect())
}

pub fn stat() -> P<Stat> {
    table(
        "stat",
        vec![
            ("power".into(), Stat::Power),
            ("toughness".into(), Stat::Toughness),
            ("base power".into(), Stat::BasePower),
            ("base toughness".into(), Stat::BaseToughness),
            ("mana value".into(), Stat::ManaValue),
            ("loyalty".into(), Stat::Loyalty),
            ("defense".into(), Stat::Defense),
        ],
    )
}

/// A bare plural where a selector stands, in a slot that asks whether such things exist or
/// took part: "you attacked with creatures with total power 6 or greater", "you control
/// artifacts named A and B". It is one or more of them: read as that, never printed.
pub fn bare_plural() -> P<Selector> {
    crate::kernel::lazy("plural set", || selectors().set_pl.clone())
        .map(|set| Some(Selector::new(Quant::Indefinite(Count::AtLeast(Amount::Const(1))), set)), |_: &Selector| None)
        .alias()
}

/// A stat of one object, as "with …" says it: the stats, and "total power and toughness",
/// the sum of the two (ruling of 2022-09-09 on Cut Down: "the total power and toughness of a
/// 3/2 creature is 5"). The sum is said of one object only: "total" before a stat elsewhere
/// sums that stat over a group ("with total power 6 or greater").
fn own_stat() -> P<Stat> {
    alt("stat", vec![lit("total power and toughness").to(Stat::PowerAndToughness), stat()])
}

pub(crate) fn exiled_with() -> P<Filter> {
    let atom = Filter::Atom;
    alt(
        "exiled with",
        vec![
            lit("exiled with").then_keep(&own_reference()).map(
                |r| Some(Filter::Atom(Atom::ExiledWith(r))),
                |f| match f {
                    Filter::Atom(Atom::ExiledWith(r @ (Ref::This | Ref::Granting))) => Some(r.clone()),
                    _ => None,
                },
            ),
            lit("exiled with it").to(atom(Atom::ExiledWith(Ref::Anaphor(Anaphor::It)))),
            // "tokens created with ~"
            lit("created with").then_keep(&own_reference()).map(
                |r| Some(Filter::Atom(Atom::CreatedWith(r))),
                |f| match f {
                    Filter::Atom(Atom::CreatedWith(r @ (Ref::This | Ref::Granting))) => Some(r.clone()),
                    _ => None,
                },
            ),
            lit("created with it").to(atom(Atom::CreatedWith(Ref::Anaphor(Anaphor::It)))),
            // "an exiled creature card used to craft ~"
            lit("used to craft").then_keep(&own_reference()).map(
                |r| Some(Filter::Atom(Atom::UsedToCraft(r))),
                |f| match f {
                    Filter::Atom(Atom::UsedToCraft(r @ (Ref::This | Ref::Granting))) => Some(r.clone()),
                    _ => None,
                },
            ),
            lit("used to craft it").to(atom(Atom::UsedToCraft(Ref::Anaphor(Anaphor::It)))),
            lit("with the same controller").to(atom(Atom::SameController)),
            // "the creature put onto the battlefield with ~"
            lit("put onto the battlefield with").then_keep(&own_reference()).map(
                |r| Some(Filter::Atom(Atom::PutOntoBattlefieldWith(r))),
                |f| match f {
                    Filter::Atom(Atom::PutOntoBattlefieldWith(r @ (Ref::This | Ref::Granting))) => Some(r.clone()),
                    _ => None,
                },
            ),
            lit("who voted for a choice you voted for").to(atom(Atom::VotedAsYou { with: true })),
            lit("who voted for a choice you didn't vote for").to(atom(Atom::VotedAsYou { with: false })),
        ],
    )
}

/// Postnominal filters: "with flying", "without flying", "with power 2 or less", "with a +1/+1
/// counter on it", "named ~", "exiled with ~", "of the chosen type". `n` is the noun phrase's
/// number, for "on it" / "on them".
fn post_filter(n: Number, sel: &P<Selector>) -> P<Filter> {
    let atom = |a: Atom| Filter::Atom(a);
    let has = alt("with", vec![lit("with"), lit(if n == Number::Sg { "that has" } else { "that have" }).alias()]);
    let kw = has.then_keep(&keyword_name()).map(
        move |k| Some(atom(Atom::Keyword(k))),
        |f| match f {
            Filter::Atom(Atom::Keyword(k)) => Some(*k),
            _ => None,
        },
    );
    // "with flying or reach": it has one of these abilities, as the rules say of what can block
    // a creature with flying ("flying and/or reach", 702.9b: one or both is the same question)
    let either = |word: &str| {
        has.then_keep(&keyword_name().list(word)).map(
            move |ks| Some(Filter::Any(ks.into_iter().map(|k| atom(Atom::Keyword(k))).collect())),
            |f| match f {
                Filter::Any(v) if v.len() >= 2 => v
                    .iter()
                    .map(|x| match x {
                        Filter::Atom(Atom::Keyword(k)) => Some(*k),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            },
        )
    };
    let kw_or = alt("with one of", vec![either("or"), either("and/or").alias()]);
    let without = lit("without").then_keep(&keyword_name()).map(
        |k| Some(Filter::Not(Box::new(Filter::Atom(Atom::Keyword(k))))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::Keyword(k)) => Some(*k),
                _ => None,
            },
            _ => None,
        },
    );
    // "with mana value 3 or less", "with mana value X" (equal)
    let cmp = alt(
        "comparison",
        vec![lit("or less").to(Cmp::Le), lit("or greater").to(Cmp::Ge), crate::kernel::empty().to(Cmp::Eq)],
    );
    // "up to two target creature cards each with mana value 2 or less": said of each of several
    let with_each = if n == Number::Pl { alt("with", vec![lit("with"), lit("each with").alias()]) } else { lit("with") };
    let stat_cmp = with_each.then_keep(&own_stat()).then(&stat_amount()).then(&cmp).map(
        |((stat, value), cmp)| Some(Filter::Atom(Atom::Stat { stat, cmp, value })),
        |f| match f {
            Filter::Atom(Atom::Stat { stat, cmp, value }) => Some(((*stat, value.clone()), *cmp)),
            _ => None,
        },
    );
    // "with power or toughness 2 or less", "with base power or toughness 1 or less": any of
    // several stats under one comparison
    let any_of = |stats: Vec<Stat>, cmp: Cmp, value: Amount| {
        Filter::Any(stats.into_iter().map(|stat| Filter::Atom(Atom::Stat { stat, cmp, value: value.clone() })).collect())
    };
    let either_stat = lit("with").then_keep(&stat_list()).then(&stat_amount()).then(&cmp).map(
        move |((stats, value), cmp)| Some(any_of(stats, cmp, value)),
        |f| shared_comparison(f).map(|(stats, cmp, value)| ((stats, value), cmp)),
    );
    // "with an even mana value", "with odd mana values", "with mana value of the chosen quality"
    let parity_word = alt(
        "parity",
        vec![lit("even").to(ir::selector::Parity::Even), lit("odd").to(ir::selector::Parity::Odd)],
    );
    let said_parity = if n == Number::Sg {
        lit("with an").then_keep(&parity_word).then(&stat())
    } else {
        lit("with").then_keep(&parity_word).then(&lit("mana values").to(Stat::ManaValue))
    };
    let chosen_parity = lit("with").then_keep(&stat()).skip(&lit("of the chosen quality")).map(
        |stat| Some((ir::selector::Parity::Chosen, stat)),
        |(parity, stat): &(ir::selector::Parity, Stat)| (*parity == ir::selector::Parity::Chosen).then_some(*stat),
    );
    let not_chosen = said_parity.map(
        |(parity, stat)| Some((parity, stat)),
        |(parity, stat): &(ir::selector::Parity, Stat)| (*parity != ir::selector::Parity::Chosen).then_some((*parity, *stat)),
    );
    let stat_parity = alt("stat parity", vec![not_chosen, chosen_parity]).map(
        |(parity, stat)| Some(Filter::Atom(Atom::Parity { stat, parity })),
        |f| match f {
            Filter::Atom(Atom::Parity { stat, parity }) => Some((*parity, *stat)),
            _ => None,
        },
    );
    // "with that spell's mana value": the same value of the stat as that object has
    let possessive_s = leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())));
    let stat_same = lit("with")
        .then_keep(&crate::kernel::lazy("selector", || selectors().selector.clone()))
        .skip(&possessive_s)
        .then(&stat())
        .map(
            |(of, stat)| Some(Filter::Atom(Atom::Stat { stat, cmp: Cmp::Eq, value: Amount::Stat { of: Box::new(of), stat } })),
            |_: &Filter| None,
        )
        .alias();
    // "with base power and toughness 2/2": both, exactly (208.4b)
    let base_pt = lit("with base power and toughness").then_keep(&pt_predicate(Stat::BasePower, Stat::BaseToughness));
    // "with power less than ~'s power", "with mana value less than or equal to the number of …";
    // a literal takes "2 or less" above, except strict comparisons ("power less than 0")
    let literal = |a: &Amount| matches!(a, Amount::Const(_) | Amount::X);
    let fits = move |cmp: Cmp, a: &Amount| matches!(cmp, Cmp::Lt | Cmp::Gt) || !literal(a);
    let cmp_words = comparison_words();
    let stat_vs = lit("with")
        .then_keep(&own_stat())
        .then(&cmp_words)
        .then(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
        .map(
            move |((stat, cmp), value)| fits(cmp, &value).then_some(Filter::Atom(Atom::Stat { stat, cmp, value })),
            move |f| match f {
                Filter::Atom(Atom::Stat { stat, cmp, value }) if fits(*cmp, value) => Some(((*stat, *cmp), value.clone())),
                _ => None,
            },
        );
    // "with mana value, power, or toughness equal to the chosen number": any of several stats
    // against an amount
    let either_stat_vs = lit("with")
        .then_keep(&stat_list())
        .then(&cmp_words)
        .then(&crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone()))
        .map(
            move |((stats, cmp), value)| fits(cmp, &value).then(|| any_of(stats, cmp, value)),
            move |f| shared_comparison(f).filter(|(_, cmp, value)| fits(*cmp, value)).map(|(stats, cmp, value)| ((stats, cmp), value)),
        );
    let named = lit("named").then_keep(&names()).map(
        |nm| Some(Filter::Atom(Atom::Named(nm))),
        |f| match f {
            Filter::Atom(Atom::Named(nm)) => Some(nm.clone()),
            _ => None,
        },
    );
    // "not named A or B": neither name; "and"/"and/or" would not say which
    let not_named = lit("not named").then_keep(&names()).map(
        |nm| (nm.items.len() == 1 || nm.conj == ir::Conj::Or).then(|| Filter::Not(Box::new(Filter::Atom(Atom::Named(nm))))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::Named(nm)) if nm.items.len() == 1 || nm.conj == ir::Conj::Or => Some(nm.clone()),
                _ => None,
            },
            _ => None,
        },
    );
    let chosen = |text: &str, c: Chosen| {
        use ir::reference::{Kind, UnresolvedRef};
        let a = match c {
            Chosen::Type => Atom::Reference(UnresolvedRef::choice(Kind::Type)),
            Chosen::CreatureType => Atom::Reference(UnresolvedRef::choice(Kind::CreatureType)),
            Chosen::Color => Atom::Reference(UnresolvedRef::choice(Kind::Color)),
            Chosen::Name => Atom::Reference(UnresolvedRef::choice(Kind::CardName)),
        };
        lit(text).to(atom(a))
    };
    // "an Aura card with enchant creature": a keyword with its parameter
    let kw_with = has.then_keep(&crate::kernel::lazy("keyword", crate::keyword::keyword)).map(
        |k| (k.param != ir::KeywordParam::None).then(|| Filter::Atom(Atom::KeywordWith(Box::new(k)))),
        |f| match f {
            Filter::Atom(Atom::KeywordWith(k)) => Some((**k).clone()),
            _ => None,
        },
    );
    // "with a cycling ability", "with landwalk abilities": the same as "with cycling" (a keyword
    // is the ability it names); "with a mana ability", "without mana abilities": an ability of
    // that kind (CR 113.3, 605.1)
    // The abilities are counted on their own: "spells that have a cycling ability", "a creature
    // with landwalk abilities". Printed in the noun phrase's number.
    let abilities = |what: P<ir::selector::AbilityKind>| {
        let one = alt("a", vec![lit("a"), lit("an").alias()]).then_keep(&what).skip(&lit("ability"));
        let some = what.skip(&lit("abilities"));
        match n {
            Number::Sg => alt("ability", vec![one, some.alias()]),
            Number::Pl => alt("abilities", vec![some, one.alias()]),
        }
    };
    let keyword_abilities = |what: P<Keyword>| {
        let one = alt("a", vec![lit("a"), lit("an").alias()]).then_keep(&what).skip(&lit("ability"));
        alt("ability", vec![one, what.skip(&lit("abilities"))])
    };
    let kw_ability = has.then_keep(&keyword_abilities(keyword_name())).map(
        move |k| Some(atom(Atom::Keyword(k))),
        |_| None,
    );
    let no_kw_ability = lit("without").then_keep(&keyword_abilities(keyword_name())).map(
        move |k| Some(Filter::Not(Box::new(atom(Atom::Keyword(k))))),
        |_| None,
    );
    let has_ability = has.then_keep(&abilities(ability_kind())).map(
        move |k| Some(atom(Atom::HasAbility(k))),
        |f| match f {
            Filter::Atom(Atom::HasAbility(k)) => Some(*k),
            _ => None,
        },
    );
    let no_ability = lit("without").then_keep(&abilities(ability_kind())).map(
        move |k| Some(Filter::Not(Box::new(atom(Atom::HasAbility(k))))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::HasAbility(k)) => Some(*k),
                _ => None,
            },
            _ => None,
        },
    );
    // "that are instant cards, sorcery cards, and/or have an Adventure" (plural only)
    let predicates = match n {
        Number::Pl => vec![relative_union()],
        Number::Sg => Vec::new(),
    };
    alt(
        "postnominal filter",
        vec![
            lit("with a basic land type").to(atom(Atom::BasicLandType)),
            kw_or,
            kw,
            kw_ability.alias(),
            no_kw_ability.alias(),
            has_ability,
            no_ability,
            source(n, sel),
            kw_with,
            without,
            stat_cmp,
            stat_parity,
            stat_same,
            either_stat,
            base_pt,
            stat_vs,
            either_stat_vs,
            counters(n),
            named,
            not_named,
            exiled_with(),
            chosen("of the chosen type", Chosen::Type),
            chosen("of the chosen creature type", Chosen::CreatureType),
            chosen("of the chosen color", Chosen::Color),
            chosen("with the chosen name", Chosen::Name),
            lit(if n == Number::Sg { "that isn't of the chosen type" } else { "that aren't of the chosen type" })
                .to(Filter::Not(Box::new(atom(Atom::Reference(ir::reference::UnresolvedRef::choice(ir::reference::Kind::Type)))))),
            lit("of the creature type of your choice").to(atom(Atom::OfChoice(Chosen::CreatureType))),
            lit("of the color of your choice").to(atom(Atom::OfChoice(Chosen::Color))),
            attached_to(sel),
            lit("from it").to(Filter::Atom(Atom::FromIt)),
            pile_membership(),
            lit("dealt damage by").then_keep(sel).then(&lit("this turn").opt()).map(
                |(s, this_turn)| ir::lexicon::fills(&s, ir::Verb::DealDamage, ir::Role::By)
                    .then(|| Filter::Atom(Atom::DealtDamageBy { by: Box::new(s), this_turn: this_turn.is_some() })),
                |f| match f {
                    Filter::Atom(Atom::DealtDamageBy { by, this_turn: false }) => Some(((**by).clone(), None)),
                    _ => None,
                },
            ),
            lit("dealt damage this turn by").then_keep(sel).map(
                |s| ir::lexicon::fills(&s, ir::Verb::DealDamage, ir::Role::By)
                    .then(|| Filter::Atom(Atom::DealtDamageBy { by: Box::new(s), this_turn: true })),
                |f| match f {
                    Filter::Atom(Atom::DealtDamageBy { by, this_turn: true }) => Some((**by).clone()),
                    _ => None,
                },
            ),
            // "each creature except for creatures you control with flying": not one of all of them
            lit("except for").then_keep(&crate::kernel::lazy("plural set", || selectors().set_pl.clone())).map(
                |set| Some(Filter::Not(Box::new(Filter::Atom(Atom::Among(Box::new(Selector::new(Quant::All, set))))))),
                |f| match f {
                    Filter::Not(b) => match b.as_ref() {
                        Filter::Atom(Atom::Among(s)) if s.quant == Quant::All && s.chooser.is_none() && s.random.is_none() => {
                            Some(s.set.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                },
            ),
            // "other than ~", "other than that creature": not one of them
            lit("other than").then_keep(sel).map(
                |s| Some(Filter::Not(Box::new(Filter::Atom(Atom::Among(Box::new(s)))))),
                |f| match f {
                    Filter::Not(b) => match b.as_ref() {
                        Filter::Atom(Atom::Among(s)) => Some((**s).clone()),
                        _ => None,
                    },
                    _ => None,
                },
            ),
            lit("from among").then_keep(sel).map(
                |s| Some(among(s)),
                unamong,
            ),
            lit("from outside the game").to(Filter::Atom(Atom::OutsideTheGame)),
            lit("able to block").then_keep(sel).then(&lit("this turn").opt()).map(
                |(what, t)| Some(Filter::Atom(Atom::AbleToBlock { what: Box::new(what), this_turn: t.is_some() })),
                |f| match f {
                    Filter::Atom(Atom::AbleToBlock { what, this_turn }) => Some(((**what).clone(), this_turn.then_some(()))),
                    _ => None,
                },
            ),
            lit("from among").then_keep(&crate::kernel::lazy("plural set", || selectors().set_pl.clone())).map(
                |s| Some(Filter::Atom(Atom::AmongSet(Box::new(s)))),
                |f| match f {
                    Filter::Atom(Atom::AmongSet(s)) => Some((**s).clone()),
                    _ => None,
                },
            ),
            shares(n, sel),
            combat(n, sel),
            targets(n, sel),
            most(),
            most_votes(),
            compared(sel),
            // "each other creature you control that spell could target"
            alt("could target", vec![crate::kernel::empty(), lit("that").alias()]).then_keep(sel).skip(&lit("could target")).map(
                |s| Some(Filter::Atom(Atom::CouldTarget(Box::new(s)))),
                |f| match f {
                    Filter::Atom(Atom::CouldTarget(s)) => Some((**s).clone()),
                    _ => None,
                },
            ),
            // "an Aura card that could enchant it"
            lit("that could enchant").then_keep(sel).map(
                |s| Some(Filter::Atom(Atom::CouldEnchant(Box::new(s)))),
                |f| match f {
                    Filter::Atom(Atom::CouldEnchant(s)) => Some((**s).clone()),
                    _ => None,
                },
            ),
            in_mana_cost(n),
            with_mana_cost(),
            lit(if n == Number::Sg { "that has an Adventure" } else { "that have an Adventure" }).to(atom(Atom::HasAdventure)),
            lit("with a single target").to(atom(Atom::TargetCount(Count::Exactly(Amount::Const(1))))),
            lit("with one or more targets").to(atom(Atom::TargetCount(Count::AtLeast(Amount::Const(1))))),
            player_measures(n, sel),
            lit("with that name").to(atom(Atom::ThatName)),
            lit("of that color").to(atom(Atom::ThatColor)),
            lit("of that type").to(atom(Atom::ThatType)),
            collective(n),
        ].into_iter().chain(predicates).collect(),
    )
}

/// One predicate of cards in a relative clause: a noun ("instant cards", with the copula said
/// once before the nouns) or a verb phrase ("have an Adventure", "have flying").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CardPredicate {
    /// the noun, and whether "are" is said before it (only the first must; a later one may
    /// repeat it: "that are instant cards, are sorcery cards, and/or have an Adventure")
    Noun(Filter, bool),
    Has(Filter),
}

/// "that are instant cards, sorcery cards, and/or have an Adventure": predicates of one card
/// joined by "and/or", any one of which it satisfies (a union, as "two artifacts and/or
/// creatures"). The nouns come first, after the one "are"; the verb phrases after them. Each
/// noun is of cards ("___ cards") and says what kind; a verb phrase says what the card has: an
/// Adventure (715.1), a keyword. At least one verb phrase: nouns alone are the noun phrase's own
/// union ("that are instant cards and/or sorcery cards" reads as "instant and/or sorcery cards").
fn relative_union() -> P<Filter> {
    let kind = core(Number::Pl, Union::Or).map(
        |(domain, items)| match (domain, items.as_slice()) {
            (Domain::Card, [one]) => Some(one.clone()),
            (Domain::Card, [_, _, ..]) => Some(Filter::All(items)),
            _ => None,
        },
        |f| match f {
            Filter::All(items) => Some((Domain::Card, items.clone())),
            one => Some((Domain::Card, vec![one.clone()])),
        },
    );
    let said = |are: bool| move |f| Some(CardPredicate::Noun(f, are));
    let unsaid = |are: bool| move |p: &CardPredicate| match p { CardPredicate::Noun(f, a) if *a == are => Some(f.clone()), _ => None };
    let noun = alt("card noun", vec![
        lit("are").then_keep(&kind).map(said(true), unsaid(true)),
        kind.map(said(false), unsaid(false)),
    ]);
    let has = lit("have").then_keep(&alt("had", vec![
        lit("an Adventure").to(Filter::Atom(Atom::HasAdventure)),
        keyword_name().map(|k| Some(Filter::Atom(Atom::Keyword(k))), |f| match f { Filter::Atom(Atom::Keyword(k)) => Some(*k), _ => None }),
    ])).map(
        |f| Some(CardPredicate::Has(f)),
        |p| match p { CardPredicate::Has(f) => Some(f.clone()), CardPredicate::Noun(..) => None },
    );
    let predicate = alt("card predicate", vec![noun, has]);
    lit("that").then_keep(&predicate.list("and/or")).map(
        |predicates| {
            // the nouns, the first after "are", then the verb phrases, and some of each
            let nouns = predicates.iter().take_while(|p| matches!(p, CardPredicate::Noun(..))).count();
            let ordered = nouns > 0 && matches!(predicates[0], CardPredicate::Noun(_, true))
                && nouns < predicates.len() && predicates[nouns..].iter().all(|p| matches!(p, CardPredicate::Has(_)));
            ordered.then(|| Filter::Any(predicates.into_iter().map(|p| match p { CardPredicate::Noun(f, _) | CardPredicate::Has(f) => f }).collect()))
        },
        // printed with "are" said once, before the first noun
        |f| match f {
            Filter::Any(v) if is_relative_union(v) => Some(v.iter().enumerate()
                .map(|(i, f)| if is_card_verb(f) { CardPredicate::Has(f.clone()) } else { CardPredicate::Noun(f.clone(), i == 0) }).collect()),
            _ => None,
        },
    )
}

/// What a card has, said with "have" in [`relative_union`].
fn is_card_verb(f: &Filter) -> bool {
    matches!(f, Filter::Atom(Atom::HasAdventure | Atom::Keyword(_)))
}

/// The arms of a [`relative_union`]: kinds of card, then what a card has, some of each.
fn is_relative_union(v: &[Filter]) -> bool {
    let nouns = v.iter().take_while(|f| !is_card_verb(f)).count();
    nouns > 0 && nouns < v.len() && v[nouns..].iter().all(is_card_verb)
        && v[..nouns].iter().all(|f| core_printable(Domain::Card, std::slice::from_ref(f))
            || matches!(f, Filter::All(items) if core_printable(Domain::Card, items)))
}

/// Who a creature fights: "blocking it", "blocked by ~", "attacking you", "blocking or blocked
/// by ~"; "that's blocking it" / "that are attacking you" are alternate spellings, and "it's
/// blocking" says "blocked by it".
fn combat(n: Number, sel: &P<Selector>) -> P<Filter> {
    let that = if n == Number::Sg { "that's" } else { "that are" };
    let rel = |word: &str, mk: fn(Box<Selector>) -> Atom, un: fn(&Atom) -> Option<&Selector>| {
        let intro = alt(word, vec![lit(word), lit(&format!("{that} {word}")).alias()]);
        intro.then_keep(sel).map(
            move |s| Some(Filter::Atom(mk(Box::new(s)))),
            move |f| match f {
                Filter::Atom(a) => un(a).cloned(),
                _ => None,
            },
        )
    };
    let blocking = rel("blocking", Atom::Blocking, |a| match a {
        Atom::Blocking(s) => Some(s),
        _ => None,
    });
    let blocked_by = rel("blocked by", Atom::BlockedBy, |a| match a {
        Atom::BlockedBy(s) => Some(s),
        _ => None,
    });
    let attacking = rel("attacking", Atom::Attacking, |a| match a {
        Atom::Attacking(s) => Some(s),
        _ => None,
    });
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let its_blocking = lit("it's blocking").to(Filter::Atom(Atom::BlockedBy(Box::new(it)))).alias();
    let either = lit("blocking or blocked by").then_keep(sel).map(
        |s| {
            Some(Filter::Any(vec![
                Filter::Atom(Atom::Blocking(Box::new(s.clone()))),
                Filter::Atom(Atom::BlockedBy(Box::new(s))),
            ]))
        },
        |f| match f {
            Filter::Any(v) => match v.as_slice() {
                [Filter::Atom(Atom::Blocking(a)), Filter::Atom(Atom::BlockedBy(b))] if a == b => Some((**a).clone()),
                _ => None,
            },
            _ => None,
        },
    );
    alt("combat relation", vec![blocking, blocked_by, attacking, either, its_blocking])
}

/// Players compared by what they control or have: "who controls more lands than you", "who has
/// more life than you", "who has more cards in hand than you", "who controls the most
/// creatures", "with the most life or tied for most life", "with the most cards in hand".
fn player_measures(n: Number, sel: &P<Selector>) -> P<Filter> {
    let sg = n == Number::Sg;
    let set_pl = crate::kernel::lazy("plural set", || selectors().set_pl.clone());
    let (controls, has) = if sg { ("who controls", "who has") } else { ("who control", "who have") };
    let more = alt("more", vec![lit("more").to(false), lit("fewer").to(true)]);
    let than = lit("than").then_keep(&alt("comparison player", vec![
        sel.clone(), lit("they").to(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player))),
    ])).skip(&alt("comparison ellipsis", vec![empty(), lit("do").alias()]));
    let more_than = alt(
        "more than",
        vec![
            lit(controls).then_keep(&more).then(&set_pl).map(|(f, s)| Some((Measure::Controls(s), f)), |(m, f): &(Measure, bool)| match m {
                Measure::Controls(s) => Some((*f, s.clone())),
                _ => None,
            }),
            lit(has).skip(&lit("more life")).map(|()| Some((Measure::Life, false)), |(m, f): &(Measure, bool)| (*m == Measure::Life && !f).then_some(())),
            lit(has).then_keep(&more).then(&set_pl).map(|(f, s)| Some((Measure::Has(s), f)), |(m, f): &(Measure, bool)| match m {
                Measure::Has(s) => Some((*f, s.clone())),
                _ => None,
            }),
        ],
    )
    .then(&than)
    .map(
        |((measure, fewer), than)| {
            Some(Filter::Atom(Atom::MoreThan { measure: Box::new(measure), fewer, than: Box::new(than) }))
        },
        |f| match f {
            Filter::Atom(Atom::MoreThan { measure, fewer, than }) => Some((((**measure).clone(), *fewer), (**than).clone())),
            _ => None,
        },
    );
    let most = alt("most", vec![lit("most").to(false), lit("fewest").to(true)]);
    let mk = |(measure, least): (Measure, bool), tied: bool| {
        Some(Filter::Atom(Atom::MostOf { measure: Box::new(measure), least, tied }))
    };
    let un = |f: &Filter| match f {
        Filter::Atom(Atom::MostOf { measure, least, tied }) => Some((((**measure).clone(), *least), *tied)),
        _ => None,
    };
    let controls_most = lit(controls).skip(&lit("the")).then_keep(&most).then(&set_pl).map(
        move |(least, s)| mk((Measure::Controls(s), least), false),
        move |f| match un(f)? {
            ((Measure::Controls(s), least), false) => Some((least, s)),
            _ => None,
        },
    );
    // "an opponent with the most life among your opponents" says no more than "with the most life"
    let the_most_life = alt("with the most life", vec![lit("with the most life"), lit("with the most life among your opponents").alias()]);
    let most_life = the_most_life
        .then_keep(&lit("or tied for most life").opt())
        .map(move |t| mk((Measure::Life, false), t.is_some()), move |f| match un(f)? {
            ((Measure::Life, false), tied) => Some(tied.then_some(())),
            _ => None,
        });
    let has_most = alt("with the most", vec![lit("with the most"), lit(&format!("{has} the most")).alias()]).then_keep(&set_pl).map(
        move |s| mk((Measure::Has(s), false), false),
        move |f| match un(f)? {
            ((Measure::Has(s), false), false) => Some(s),
            _ => None,
        },
    );
    // "who controls eight or more lands", "who doesn't control a creature"
    let doesnt = if sg { "who doesn't control" } else { "who don't control" };
    let controls_what = lit(controls).then_keep(sel).map(
        |s| Some(Filter::Atom(Atom::Controls(Box::new(s)))),
        |f| match f {
            Filter::Atom(Atom::Controls(s)) => Some((**s).clone()),
            _ => None,
        },
    );
    let controls_not = lit(doesnt).then_keep(sel).map(
        |s| Some(Filter::Not(Box::new(Filter::Atom(Atom::Controls(Box::new(s)))))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::Controls(s)) => Some((**s).clone()),
                _ => None,
            },
            _ => None,
        },
    );
    let opponent_of = alt("opponent relative", vec![
        lit(if sg { "who is" } else { "who are" }),
        lit(if sg { "and is" } else { "and are" }).alias(),
    ]).then_keep(&crate::kernel::lazy("possessive", || selectors().possessive.clone())).skip(&lit("opponent")).map(
        |player| Some(Filter::Atom(Atom::OpponentOf(Box::new(player)))),
        |f| match f { Filter::Atom(Atom::OpponentOf(player)) => Some((**player).clone()), _ => None },
    );
    // "each opponent who does", "each player who can't"
    let (does, doesnt) = if sg { ("who does", "who doesn't") } else { ("who do", "who don't") };
    let did_so = alt(
        "who does",
        vec![lit(does).to(Outcome::Did), lit(doesnt).to(Outcome::Didnt), lit("who can't").to(Outcome::Couldnt)],
    )
    .map(
        |o| Some(Filter::Atom(Atom::DidSo(o))),
        |f| match f {
            Filter::Atom(Atom::DidSo(o)) => Some(*o),
            _ => None,
        },
    );
    // "each opponent who has three or more poison counters" (players have counters, CR 122.1)
    let player_counters = alt("who has", vec![lit(has), lit("with").alias()])
        .then_keep(&count_amount())
        .skip(&lit("or more"))
        .then(&counter_kind())
        .skip(&lit("counters"))
        .map(
            |(n, kind)| Some(Filter::Atom(Atom::Counters { kind: Some(kind), count: Count::AtLeast(n) })),
            |f| match f {
                Filter::Atom(Atom::Counters { kind: Some(kind), count: Count::AtLeast(n) }) => Some((n.clone(), kind.clone())),
                _ => None,
            },
        );
    alt(
        "player measure",
        vec![more_than, controls_most, most_life, has_most, controls_what, controls_not, opponent_of, did_so, player_counters],
    )
}

/// The ability class and source of an otherwise unqualified ability description (113.7a).
pub(crate) fn ability_source(set: &ObjectSet) -> Option<(Option<ir::selector::AbilityKind>, &ObjectSet)> {
    if *set != (ObjectSet { filter: set.filter.clone(), ..ObjectSet::of(Domain::Ability) }) { return None; }
    match &set.filter {
        Filter::All(parts) => match parts.as_slice() {
            [Filter::Atom(Atom::Source(source))] => Some((None, source)),
            [Filter::Atom(Atom::AbilityKind(kind)), Filter::Atom(Atom::Source(source))] => Some((Some(*kind), source)),
            _ => None,
        },
        _ => None,
    }
}

/// "from an artifact", "from artifact sources", "from a noncreature source": the source of an
/// ability (CR 113.7a), one object or all of a kind by the ability's number. "Activated
/// abilities of artifacts" is the possessor's own construction (statics, effects), not this.
fn source(n: Number, sel: &P<Selector>) -> P<Filter> {
    let set = match n {
        Number::Sg => sel.map(
            |s| {
                (s.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1))) && s.chooser.is_none() && s.random.is_none())
                    .then_some(s.set)
            },
            |set: &ObjectSet| Some(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set.clone())),
        ),
        Number::Pl => crate::kernel::lazy("plural set", || selectors().set_pl.clone()),
    };
    let objects_only = |set: &ObjectSet| ir::lexicon::of_kind(&Selector::new(Quant::All, set.clone()), &[
        ir::Sort::Permanent, ir::Sort::Card, ir::Sort::Spell, ir::Sort::Ability,
    ]);
    let set = set.map(move |set| objects_only(&set).then_some(set), move |set| objects_only(set).then(|| set.clone()));
    // Singular stack abilities can name their source with "of" (113.7a). The plural
    // "abilities of creatures" describes owned ability families, in the static grammar.
    let prep = if n == Number::Sg { alt("ability source", vec![lit("from"), lit("of").alias()]) } else { lit("from") };
    prep.then_keep(&set).map(
        |set| Some(Filter::Atom(Atom::Source(Box::new(set)))),
        |f| match f {
            Filter::Atom(Atom::Source(set)) => Some((**set).clone()),
            _ => None,
        },
    )
}

/// Membership in a temporary pile is a predicate on its objects, not a pile object (700.3b).
fn pile_membership() -> P<Filter> {
    use ir::procedure::PileRef;
    let in_or_from = alt("membership", vec![lit("in"), lit("from").alias()]);
    let named = in_or_from.then_keep(&alt("pile", vec![
        lit("that pile").to(PileRef::That),
        lit("the other pile").to(PileRef::TheOther),
    ])).map(
        |pile| Some(Filter::Atom(Atom::InPile { pile, chooser: None })),
        |f| match f {
            Filter::Atom(Atom::InPile { pile, chooser: None }) => Some(*pile),
            _ => None,
        },
    );
    let chosen = in_or_from.skip(&lit("the pile of")).then_keep(&crate::kernel::lazy("possessive", || selectors().possessive.clone()))
        .skip(&lit("choice")).map(
            |who| Some(Filter::Atom(Atom::InPile { pile: PileRef::One, chooser: Some(Box::new(who)) })),
            |f| match f {
                Filter::Atom(Atom::InPile { pile: PileRef::One, chooser: Some(who) }) => Some((**who).clone()),
                _ => None,
            },
        );
    alt("pile membership", vec![named, chosen])
}

/// "with mana cost {0} or {1}", "with mana cost {1}": the printed mana cost, exactly (202.1).
fn with_mana_cost() -> P<Filter> {
    let cost = crate::mana::mana_cost();
    let one = cost.map(|c| Some(vec![c]), |v: &Vec<ir::mana::ManaCost>| match v.as_slice() { [c] => Some(c.clone()), _ => None });
    let costs = alt("mana costs", vec![cost.list("or"), one]);
    lit("with mana cost").then_keep(&costs).map(
        |v| Some(Filter::Atom(Atom::ManaCost(v))),
        |f| match f {
            Filter::Atom(Atom::ManaCost(v)) => Some(v.clone()),
            _ => None,
        },
    )
}

/// "with {X} in its mana cost", "with {X} in their mana costs", "without {X} in its mana cost".
fn in_mana_cost(n: Number) -> P<Filter> {
    let place = match n {
        Number::Sg => lit("in its mana cost"),
        Number::Pl => alt("in their mana costs", vec![lit("in their mana costs"), lit("in their mana cost").alias()]),
    };
    let sym = crate::mana::mana_symbol();
    let with = lit("with").then_keep(&sym).skip(&place).map(
        |s| Some(Filter::Atom(Atom::InManaCost(s))),
        |f| match f {
            Filter::Atom(Atom::InManaCost(s)) => Some(*s),
            _ => None,
        },
    );
    let without = lit("without").then_keep(&sym).skip(&place).map(
        |s| Some(Filter::Not(Box::new(Filter::Atom(Atom::InManaCost(s))))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::InManaCost(s)) => Some(*s),
                _ => None,
            },
            _ => None,
        },
    );
    alt("in mana cost", vec![with, without])
}

/// The zones of "on the battlefield or in the command zone", as a union of [`Atom::InZone`].
fn in_zones(f: &Filter) -> Option<Vec<ZoneSpec>> {
    match f {
        Filter::Any(v) if v.len() >= 2 => v
            .iter()
            .map(|x| match x {
                Filter::Atom(Atom::InZone(z)) => Some(z.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// Filters a player can take: comparisons with other players and events they took part in.
fn is_player_filter(f: &Filter) -> bool {
    match f {
        Filter::Atom(a) => matches!(
            a,
            Atom::MoreThan { .. }
                | Atom::MostOf { .. }
                | Atom::OpponentOf(_)
                | Atom::Did { .. }
                | Atom::Controls(_)
                | Atom::Attacking(_)
                | Atom::DidSo(_)
                | Atom::Counters { .. }
        ),
        // "each opponent other than defending player": not one of the players named
        Filter::Not(b) => matches!(b.as_ref(), Filter::Atom(Atom::Controls(_)))
            || matches!(b.as_ref(), Filter::Atom(Atom::Among(s)) if s.set.domain == Domain::Player && matches!(s.quant, Quant::Definite(_))),
        _ => false,
    }
}

fn comparison_words() -> P<Cmp> {
    alt("comparison", vec![
        lit("less than or equal to").to(Cmp::Le),
        lit("greater than or equal to").to(Cmp::Ge),
        lit("less than").to(Cmp::Lt),
        lit("greater than").to(Cmp::Gt),
        lit("equal to").to(Cmp::Eq),
    ])
}

/// "with lesser mana value", "with greater power", "with equal or lesser mana value"; with the
/// object named, "with lesser mana value than that creature" is an alternate spelling of "with
/// mana value less than that creature's mana value".
fn compared(sel: &P<Selector>) -> P<Filter> {
    let cmp = alt(
        "comparison",
        vec![
            lit("lesser").to(Cmp::Lt),
            lit("greater").to(Cmp::Gt),
            lit("equal or lesser").to(Cmp::Le),
            lit("equal or greater").to(Cmp::Ge),
        ],
    );
    let implicit = lit("with").then_keep(&cmp).then(&stat()).map(
        |(cmp, stat)| Some(Filter::Atom(Atom::Compared { stat, cmp })),
        |f| match f {
            Filter::Atom(Atom::Compared { stat, cmp }) => Some((*cmp, *stat)),
            _ => None,
        },
    );
    let explicit = lit("with")
        .then_keep(&cmp)
        .then(&stat())
        .skip(&lit("than"))
        .then(sel)
        .map(
            |((cmp, stat), of)| Some(Filter::Atom(Atom::Stat { stat, cmp, value: Amount::Stat { of: Box::new(of), stat } })),
            |_: &Filter| None,
        )
        .alias();
    // A previously named object's stat can be omitted after the comparison: "with mana
    // value less than or equal to that Aura". This is the same scalar as its explicit stat,
    // never an aggregate over "each Aura" or a player's unrelated numeric property.
    let elided = lit("with").then_keep(&own_stat()).then(&comparison_words()).then(sel).map(
        |((stat, cmp), of)| {
            (matches!(of.quant, Quant::Definite(Ref::This | Ref::Granting | Ref::Anaphor(Anaphor::It | Anaphor::That)))
                && ir::lexicon::of_kind(&of, &[ir::lexicon::Sort::Permanent, ir::lexicon::Sort::Card, ir::lexicon::Sort::Spell]))
                .then(|| Filter::Atom(Atom::Stat { stat, cmp, value: Amount::Stat { of: Box::new(of), stat } }))
        },
        |_| None,
    ).alias();
    alt("compared", vec![implicit, explicit, elided])
}

/// "with the greatest power among creatures they control", "with the least toughness", "with the
/// greatest mana value or tied for the greatest mana value".
fn most() -> P<Filter> {
    let which = alt("superlative", vec![lit("greatest").to(false), lit("least").to(true)]);
    let set_pl = crate::kernel::lazy("plural set", || selectors().set_pl.clone());
    let among = lit("among").then_keep(&set_pl).opt();
    // the tie repeats the superlative: "or tied for the greatest power"
    let tied = lit("or tied for the").then_keep(&which).then(&stat()).opt();
    lit("with the").then_keep(&which).then(&stat()).then(&among).then(&tied).map(
        |(((least, stat), among), tied)| {
            if tied.is_some_and(|t| t != (least, stat)) {
                return None;
            }
            Some(Filter::Atom(Atom::Most { stat, least, among: among.map(Box::new), tied: tied.is_some() }))
        },
        |f| match f {
            Filter::Atom(Atom::Most { stat, least, among, tied }) => {
                Some((((*least, *stat), among.as_deref().cloned()), tied.then_some((*least, *stat))))
            }
            _ => None,
        },
    )
}

/// "with the most votes or tied for most votes": what the vote gave the most votes (701.38).
fn most_votes() -> P<Filter> {
    lit("with the most votes").then_keep(&lit("or tied for most votes").opt()).map(
        |tied| Some(Filter::Atom(Atom::MostVotes { tied: tied.is_some() })),
        |f| match f {
            Filter::Atom(Atom::MostVotes { tied }) => Some(tied.then_some(())),
            _ => None,
        },
    )
}

/// "that targets ~", "that target you or a creature you control", "that targets only a single
/// creature".
fn targets(n: Number, sel: &P<Selector>) -> P<Filter> {
    let verb = if n == Number::Sg { "that targets" } else { "that target" };
    let only = alt("only", vec![lit("only").to(true), crate::kernel::empty().to(false)]);
    lit(verb).then_keep(&only).then(&crate::action::coord(sel)).map(
        |(only, what)| Some(Filter::Atom(Atom::Targets { what, only })),
        |f| match f {
            Filter::Atom(Atom::Targets { what, only }) => Some((*only, what.clone())),
            _ => None,
        },
    )
}

fn is_combat(f: &Filter) -> bool {
    match f {
        Filter::Atom(Atom::Blocking(_) | Atom::BlockedBy(_) | Atom::Attacking(_)) => true,
        Filter::Any(v) => {
            matches!(v.as_slice(), [Filter::Atom(Atom::Blocking(a)), Filter::Atom(Atom::BlockedBy(b))] if a == b)
        }
        _ => false,
    }
}

/// What "the same …" compares: "name", "mana value", "power".
pub(crate) fn same_word() -> P<Characteristic> {
    table(
        "characteristic",
        vec![
            ("name".into(), Characteristic::Name),
            ("mana value".into(), Characteristic::ManaValue),
            ("power".into(), Characteristic::Power),
        ],
    )
}

/// What "shares a …" shares: "a color", "a card type", "a creature type", "a permanent type".
fn shared_word() -> P<Characteristic> {
    table(
        "characteristic",
        vec![
            ("a color".into(), Characteristic::Color),
            ("a card type".into(), Characteristic::CardType),
            ("a creature type".into(), Characteristic::CreatureType),
            ("a permanent type".into(), Characteristic::PermanentType),
        ],
    )
}

/// "with the same name as that creature", "that shares a color with ~", "that share a creature
/// type with it"; negated: "that doesn't have the same name as …", "that doesn't share a card
/// type with …". The verb agrees with the noun phrase's number `n`.
fn shares(n: Number, sel: &P<Selector>) -> P<Filter> {
    let sg = n == Number::Sg;
    let is_same = |c: &Characteristic| matches!(c, Characteristic::Name | Characteristic::ManaValue | Characteristic::Power);
    let same = same_word().map(move |c| is_same(&c).then_some(c), move |c| is_same(c).then_some(*c));
    let shared = shared_word().map(move |c| (!is_same(&c)).then_some(c), move |c| (!is_same(c)).then_some(*c));
    let (shares_verb, doesnt) = if sg { ("that shares", "that doesn't") } else { ("that share", "that don't") };
    let positive = alt(
        "shares",
        vec![
            lit("with the same").then_keep(&same).skip(&lit("as")).then(sel),
            lit(shares_verb).then_keep(&shared).skip(&lit("with")).then(sel),
        ],
    )
    .map(
        |(what, with)| Some(Filter::Atom(Atom::Shares { what, with: Box::new(with) })),
        |f| match f {
            Filter::Atom(Atom::Shares { what, with }) => Some((*what, (**with).clone())),
            _ => None,
        },
    );
    let negative = alt(
        "doesn't share",
        vec![
            lit(&format!("{doesnt} have the same")).then_keep(&same).skip(&lit("as")).then(sel),
            lit(&format!("{doesnt} share")).then_keep(&shared).skip(&lit("with")).then(sel),
        ],
    )
    .map(
        |(what, with)| Some(Filter::Not(Box::new(Filter::Atom(Atom::Shares { what, with: Box::new(with) })))),
        |f| match f {
            Filter::Not(b) => match b.as_ref() {
                Filter::Atom(Atom::Shares { what, with }) => Some((*what, (**with).clone())),
                _ => None,
            },
            _ => None,
        },
    );
    let different = lit("with a different name")
        .skip(&alt("name comparison", vec![lit("than"), lit("from").alias()])).then_keep(sel).map(
            |source| {
                ir::lexicon::of_kind(&source, &[ir::lexicon::Sort::Permanent, ir::lexicon::Sort::Card,
                    ir::lexicon::Sort::Spell, ir::lexicon::Sort::Ability])
                    .then(|| Filter::Atom(Atom::DifferentNameFrom(Box::new(source))))
            },
            |f| match f {
                Filter::Atom(Atom::DifferentNameFrom(source))
                    if ir::lexicon::of_kind(source, &[ir::lexicon::Sort::Permanent, ir::lexicon::Sort::Card,
                        ir::lexicon::Sort::Spell, ir::lexicon::Sort::Ability]) => Some((**source).clone()),
                _ => None,
            },
        );
    alt("shares", vec![positive, negative, different])
}

/// A distinct value for every selected object, with a shared bound (202.3, 208.1).
/// Printing uses the ordinary independent predicates, so this spelling is an alias.
fn collective_each(n: Number) -> P<Post> {
    let characteristic = table("distinct characteristic", vec![
        ("mana value".into(), (Characteristic::ManaValue, Stat::ManaValue)),
        ("power".into(), (Characteristic::Power, Stat::Power)),
    ]);
    let cmp = alt("comparison", vec![lit("or less").to(Cmp::Le), lit("or greater").to(Cmp::Ge), empty().to(Cmp::Eq)]);
    lit("that each have a different").then_keep(&characteristic).then(&stat_amount()).then(&cmp).map(
        move |(((what, stat), value), cmp)| (n == Number::Pl).then(|| Post::WithAll(vec![
            Filter::Atom(Atom::Different(what)), Filter::Atom(Atom::Stat { stat, cmp, value }),
        ])),
        |_| None,
    ).alias()
}

/// Collective predicates of a plural selection: "with different names", "with different
/// powers", "that share a color", "with the same name", "with total power 6 or greater".
fn collective(n: Number) -> P<Filter> {
    if n == Number::Sg {
        return alt("collective", Vec::new());
    }
    let different = lit("with different")
        .then_keep(&table(
            "characteristic",
            vec![
                ("names".into(), Characteristic::Name),
                ("powers".into(), Characteristic::Power),
                ("mana values".into(), Characteristic::ManaValue),
            ],
        ))
        .map(
            |c| Some(Filter::Atom(Atom::Different(c))),
            |f| match f {
                Filter::Atom(Atom::Different(c)) => Some(*c),
                _ => None,
            },
        );
    let is_same = |c: &Characteristic| matches!(c, Characteristic::Name | Characteristic::ManaValue | Characteristic::Power);
    let alike = alt(
        "alike",
        vec![
            lit("with the same").then_keep(&same_word()).map(move |c| is_same(&c).then_some(c), move |c| is_same(c).then_some(*c)),
            lit("that share").then_keep(&shared_word()).map(move |c| (!is_same(&c)).then_some(c), move |c| (!is_same(c)).then_some(*c)),
            // "that have a creature type in common" says "that share a creature type"
            lit("that have").then_keep(&shared_word()).skip(&lit("in common")).map(move |c| (!is_same(&c)).then_some(c), |_| None).alias(),
        ],
    )
    .map(
        |c| Some(Filter::Atom(Atom::Alike(c))),
        |f| match f {
            Filter::Atom(Atom::Alike(c)) => Some(*c),
            _ => None,
        },
    );
    let cmp = alt(
        "comparison",
        vec![lit("or less").to(Cmp::Le), lit("or greater").to(Cmp::Ge), crate::kernel::empty().to(Cmp::Eq)],
    );
    let total = lit("with total").then_keep(&stat()).then(&stat_amount()).then(&cmp).map(
        |((stat, value), cmp)| Some(Filter::Atom(Atom::Total { stat, cmp, value })),
        |f| match f {
            Filter::Atom(Atom::Total { stat, cmp, value }) => Some(((*stat, value.clone()), *cmp)),
            _ => None,
        },
    );
    let controllers = alt(
        "controllers",
        vec![
            lit("controlled by different players").to(Filter::Atom(Atom::Controllers { same: false })),
            lit("controlled by the same player").to(Filter::Atom(Atom::Controllers { same: true })),
        ],
    );
    alt("collective", vec![different, alike, total, controllers])
}

/// The actions whose participle names what they were done to ("the exiled card", "cards
/// revealed this way"): every action of the lexicon that is done to something.
fn done_to() -> impl Iterator<Item = Verb> {
    Verb::ALL.iter().copied().filter(|v| v.passive() == Some(Role::What))
}

/// "one of them", "each of those creatures", "two of those cards", "up to two of them", "one or
/// more of them": members of a plural group the text already named.
fn partitive(num: Number, group: &P<Selector>) -> P<Selector> {
    // a group of several, or of up to some number ("each of up to one target creature")
    let several = |g: &Selector| is_plural(g) || matches!(g.quant, Quant::Target(Count::UpTo(_)) | Quant::Indefinite(Count::UpTo(_)));
    let group = group.map(move |g| several(&g).then_some(g), move |g| several(g).then(|| g.clone()));
    let many = |n: &Amount| *n != Amount::Const(1);
    let quant = match num {
        Number::Sg => alt(
            "partitive",
            vec![
                lit("one").to(Quant::Indefinite(Count::Exactly(Amount::Const(1)))),
                // "either of them": one of the two
                lit("either").to(Quant::Indefinite(Count::Exactly(Amount::Const(1)))).alias(),
                lit("each").to(Quant::Each),
                lit("up to one").to(Quant::Indefinite(Count::UpTo(Amount::Const(1)))),
                lit("the other").to(Quant::Definite(Ref::Anaphor(Anaphor::Other { plural: false }))),
            ],
        ),
        Number::Pl => alt(
            "partitive",
            vec![
                count_amount().map(
                    move |n| many(&n).then_some(Quant::Indefinite(Count::Exactly(n))),
                    move |q| match q {
                        Quant::Indefinite(Count::Exactly(n)) if many(n) => Some(n.clone()),
                        _ => None,
                    },
                ),
                lit("up to").then_keep(&count_amount()).map(
                    move |n| many(&n).then_some(Quant::Indefinite(Count::UpTo(n))),
                    move |q| match q {
                        Quant::Indefinite(Count::UpTo(n)) if many(n) => Some(n.clone()),
                        _ => None,
                    },
                ),
                lit("one or more").to(Quant::Indefinite(Count::AtLeast(Amount::Const(1)))),
                lit("any number").to(Quant::Indefinite(Count::AnyNumber)),
                // what the group leaves after some were taken, or whichever of it, or all
                lit("the rest").to(Quant::Definite(Ref::Unresolved(UnresolvedRef::cards(RefForm::Remainder)))),
                lit("the others").to(Quant::Definite(Ref::Anaphor(Anaphor::Other { plural: true }))),
                lit("any").to(Quant::Any),
                lit("all").to(Quant::All),
            ],
        ),
    };
    let of = |other: bool| {
        move |(quant, g): (Quant, Selector)| {
            let mut set = ObjectSet::of(Domain::Object);
            set.filter = Filter::All(vec![among(g)]);
            set.other = other;
            Some(Selector::new(quant, set))
        }
    };
    let unof = |other: bool| {
        move |s: &Selector| {
            let mut bare = ObjectSet::of(Domain::Object);
            let Filter::All(v) = &s.set.filter else { return None };
            let [f] = v.as_slice() else { return None };
            let g = unamong(f)?;
            bare.filter = s.set.filter.clone();
            bare.other = other;
            (s.set == bare && s.chooser.is_none() && s.random.is_none()).then(|| (s.quant.clone(), g))
        }
    };
    let plain = quant.skip(&lit("of")).then(&group).map(of(false), unof(false));
    // "another of your opponents", "another one of …": one of the group other than the one the text
    // is about (as "another" before a noun)
    let another = alt("another", vec![lit("another"), lit("another one").alias()]).skip(&lit("of")).then_keep(&group);
    let another = if num == Number::Sg {
        another
            .map(
                |g| Some((Quant::Indefinite(Count::Exactly(Amount::Const(1))), g)),
                |(q, g): &(Quant, Selector)| (*q == Quant::Indefinite(Count::Exactly(Amount::Const(1)))).then(|| g.clone()),
            )
            .map(of(true), unof(true))
    } else {
        alt("another", Vec::new())
    };
    // "one of them at random"
    let at_random = plain.skip(&lit("at random")).map(
        |mut s: Selector| {
            s.random = Some(ir::selector::Random::AtRandom);
            Some(s)
        },
        |s: &Selector| {
            (s.random == Some(ir::selector::Random::AtRandom)).then(|| {
                let mut plain = s.clone();
                plain.random = None;
                plain
            })
        },
    );
    alt("partitive", vec![plain, at_random, another])
}

/// "attached to it", "attached to a creature you control"; "that's attached to" is an alternate
/// spelling.
fn attached_to(sel: &P<Selector>) -> P<Filter> {
    let intro = alt(
        "attached to",
        vec![lit("attached to"), lit("that's attached to").alias(), lit("that are attached to").alias()],
    );
    let one = intro.then_keep(sel).map(
        |s| Some(Filter::Atom(Atom::AttachedTo(Box::new(s)))),
        |f| match f {
            Filter::Atom(Atom::AttachedTo(s)) => Some((**s).clone()),
            _ => None,
        },
    );
    // "Destroy all creatures and all permanents attached to creatures", "Auras attached to
    // permanents you control": attached to any one of them, as "attached to a creature" says
    let set_pl = crate::kernel::lazy("plural set", || selectors().set_pl.clone());
    let any_of = intro.then_keep(&set_pl).map(
        |set| Some(Filter::Atom(Atom::AttachedTo(Box::new(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), set))))),
        |_: &Filter| None,
    ).alias();
    alt("attached to", vec![one, any_of])
}

/// "with a +1/+1 counter on it", "with a counter on it", "with three or more time counters on
/// them", "with two +1/+1 counters on it".
fn counters(n: Number) -> P<Filter> {
    // "without a +1/+1 counter on it": none of that kind
    let one = |f: &Filter| matches!(f, Filter::Atom(Atom::Counters { count: Count::AtLeast(Amount::Const(1)), .. }));
    let without = lit("without").then_keep(&counters_body(n)).map(
        move |f| one(&f).then(|| Filter::Not(Box::new(f))),
        move |f| match f {
            Filter::Not(b) if one(b) => Some((**b).clone()),
            _ => None,
        },
    );
    alt("counters", vec![lit("with").then_keep(&counters_body(n)), without])
}

/// "a +1/+1 counter on it", "three or more time counters on them": the counter filter without
/// its "with" (also read after "has": "~ has a +1/+1 counter on it").
pub fn counters_body(n: Number) -> P<Filter> {
    let on = lit(if n == Number::Sg { "on it" } else { "on them" });
    let kind = counter_kind().opt();
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let one = a.then_keep(&kind).skip(&lit("counter")).map(
        |k| Some((k, Count::AtLeast(Amount::Const(1)))),
        |(k, c): &(Option<CounterKind>, Count)| (*c == Count::AtLeast(Amount::Const(1))).then(|| k.clone()),
    );
    // "with +1/+1 counters on them": the same as "a +1/+1 counter", which plurals say this way
    let bare = kind.skip(&lit("counters")).map(
        |k| Some((k, Count::AtLeast(Amount::Const(1)))),
        |(k, c): &(Option<CounterKind>, Count)| (*c == Count::AtLeast(Amount::Const(1))).then(|| k.clone()),
    );
    let one = match n {
        Number::Sg => alt("counter", vec![one, bare.alias()]),
        Number::Pl => alt("counter", vec![bare, one.alias()]),
    };
    // "with no counters on it"
    let none = lit("no").then_keep(&kind).skip(&lit("counters")).map(
        |k| Some((k, Count::Exactly(Amount::Const(0)))),
        |(k, c): &(Option<CounterKind>, Count)| (*c == Count::Exactly(Amount::Const(0))).then(|| k.clone()),
    );
    let one = alt("counter", vec![one, none]);
    let or_more = count_amount().skip(&lit("or more")).then(&kind).skip(&lit("counters")).map(
        |(a, k)| Some((k, Count::AtLeast(a))),
        |(k, c): &(Option<CounterKind>, Count)| match c {
            Count::AtLeast(a) if *a != Amount::Const(1) => Some((a.clone(), k.clone())),
            _ => None,
        },
    );
    // "two or fewer oil counters on it": at most that many
    let or_fewer = count_amount().skip(&lit("or fewer")).then(&kind).skip(&lit("counters")).map(
        |(a, k)| Some((k, Count::UpTo(a))),
        |(k, c): &(Option<CounterKind>, Count)| match c {
            Count::UpTo(a) => Some((a.clone(), k.clone())),
            _ => None,
        },
    );
    let exactly = count_amount().then(&kind).skip(&lit("counters")).map(
        |(a, k)| (a != Amount::Const(1)).then_some((k, Count::Exactly(a))),
        |(k, c): &(Option<CounterKind>, Count)| match c {
            Count::Exactly(a) => Some((a.clone(), k.clone())),
            _ => None,
        },
    );
    alt("counters", vec![one, or_more, or_fewer, exactly]).skip(&on).map(
        |(kind, count)| Some(Filter::Atom(Atom::Counters { kind, count })),
        |f| match f {
            Filter::Atom(Atom::Counters { kind, count }) => Some((kind.clone(), count.clone())),
            _ => None,
        },
    )
}

/// A counter kind: "+1/+1", "-1/-1", or one word ("time", "loyalty", "flood").
pub fn counter_kind() -> P<CounterKind> {
    let sign = alt("sign", vec![lit("+").to('+'), lit("-").to('-')]);
    let pt = sign.then(&digits().tight()).skip(&lit("/").tight()).then(&sign.tight()).then(&digits().tight()).map(
        |(((s1, a), s2), b)| Some(CounterKind(format!("{s1}{a}/{s2}{b}"))),
        |k: &CounterKind| {
            let c: Vec<char> = k.0.chars().collect();
            let slash = k.0.find('/')?;
            let (a, b) = (&k.0[1..slash], &k.0[slash + 2..]);
            Some((((c[0], a.parse().ok()?), *c.get(slash + 1)?), b.parse().ok()?))
        },
    );
    // a kind is a content word: "no counters" counts none, it is not a kind
    const NOT_KINDS: &[&str] = &["counter", "counters", "no", "a", "an", "the", "any", "each", "all", "those", "that", "more"];
    let word = leaf(
        "counter kind",
        |k| match k {
            Kind::Word(w) if !NOT_KINDS.contains(&w.as_str()) && w.chars().all(|c| c.is_lowercase() || c == '-') => {
                Some(CounterKind(w.clone()))
            }
            _ => None,
        },
        |k| (!k.0.contains('/') && !k.0.contains(' ')).then(|| Kind::Word(k.0.clone())),
    );
    // the keyword counters named with two words (CR 122.1b lists the keywords a counter can be)
    let keyword = table(
        "keyword counter",
        ["first strike", "double strike"].iter().map(|w| (w.to_string(), CounterKind(w.to_string()))).collect(),
    );
    alt("counter kind", vec![pt, keyword, word])
}

/// "a +1/+1 counter", "an additional +1/+1 counter", "another +1/+1 counter", "two +1/+1
/// counters", "two additional +1/+1 counters", "up to X +1/+0 counters".
pub fn some_counters() -> P<ir::Counters> {
    static C: OnceLock<P<ir::Counters>> = OnceLock::new();
    C.get_or_init(|| {
        use ir::{Counters, Extra};
        let a = alt("a", vec![lit("a"), lit("an").alias()]);
        let additional = lit("additional").opt().map(
            |o| Some(o.map(|()| Extra::Additional)),
            |e: &Option<Extra>| match e {
                None => Some(None),
                Some(Extra::Additional) => Some(Some(())),
                Some(Extra::Another) => None,
            },
        );
        let article = alt(
            "a or another",
            vec![
                a.then_keep(&additional),
                lit("another").to(Some(Extra::Another)),
            ],
        );
        let one = article.then(&counter_kind()).skip(&lit("counter")).map(
            |(extra, kind)| Some(Counters { count: Amount::Const(1), up_to: false, extra, kind }),
            |c| (c.count == Amount::Const(1) && !c.up_to).then(|| (c.extra, c.kind.clone())),
        );
        let many = lit("up to").opt().then(&count_amount()).then(&additional).then(&counter_kind()).skip(&lit("counters")).map(
            |(((up_to, count), extra), kind)| {
                (count != Amount::Const(1) || up_to.is_some()).then_some(Counters { count, up_to: up_to.is_some(), extra, kind })
            },
            |c| {
                (c.count != Amount::Const(1) || c.up_to)
                    .then(|| (((c.up_to.then_some(()), c.count.clone()), c.extra), c.kind.clone()))
            },
        );
        // "a number of +1/+1 counters equal to its power": the amount after the noun (CR 107.1)
        let amount = crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone());
        let numbered = lit("a number of").then_keep(&counter_kind()).skip(&lit("counters equal to")).then(&amount).map(
            |(kind, count)| (!crate::words::is_quantity(&count)).then(|| Counters::new(count, kind)),
            |c| {
                (!crate::words::is_quantity(&c.count) && !c.up_to && c.extra.is_none())
                    .then(|| (c.kind.clone(), c.count.clone()))
            },
        );
        // "an additional X +1/+1 counters": the other order of "X additional +1/+1 counters"
        let additional_first = lit("an additional")
            .then_keep(&count_amount())
            .then(&counter_kind())
            .skip(&lit("counters"))
            .map(
                |(count, kind)| (count != Amount::Const(1)).then_some(Counters { count, up_to: false, extra: Some(Extra::Additional), kind }),
                |_: &Counters| None,
            )
            .alias();
        alt("counters", vec![one, many, numbered, additional_first])
    })
    .clone()
}

/// "two +1/+1 counters on it", "a finality counter on him" (after "with").
pub fn counters_on() -> P<ir::CountersOn> {
    static C: OnceLock<P<ir::CountersOn>> = OnceLock::new();
    C.get_or_init(|| {
        use ir::On;
        let on = table(
            "on whom",
            vec![("on it".into(), On::It), ("on them".into(), On::Them)],
        );
        some_counters().then(&on).map(
            |(counters, on)| Some(ir::CountersOn { counters, on }),
            |c: &ir::CountersOn| Some((c.counters.clone(), c.on)),
        )
    })
    .clone()
}

/// A name after "named": `~`, or a card name (a run of capitalized words, with "of"/"the"
/// inside: "Relentless Rats", "Kaya, Ghost Assassin" is not reached by this).
pub(crate) fn name() -> P<Name> {
    alt(
        "name",
        vec![
            own_reference().map(
                |r| match r {
                    Ref::This => Some(Name::This),
                    Ref::Granting => Some(Name::Granting),
                    _ => None,
                },
                |n| match n {
                    Name::This => Some(Ref::This),
                    Name::Granting => Some(Ref::Granting),
                    Name::Card(_) => None,
                },
            ),
            name_run().map(
                |s| Some(Name::Card(s)),
                |n| match n {
                    Name::Card(s) => Some(s.clone()),
                    Name::This | Name::Granting => None,
                },
            ),
        ],
    )
}

/// One name, or several joined by "and", "or", "and/or" (2.6): "named A or B".
fn names() -> P<ir::Coord<Name>> {
    crate::action::coord(&name())
}

fn is_post_filter(f: &Filter) -> bool {
    match f {
        f if is_combat(f) || in_zones(f).is_some() => true,
        Filter::Any(v) if v.len() >= 2 && v.iter().all(|x| matches!(x, Filter::Atom(Atom::Stat { .. }))) => true,
        // "with base power and toughness 2/2"
        f if pt_pair(f, Stat::BasePower, Stat::BaseToughness).is_some() => true,
        // "with flying or reach"
        Filter::Any(v) if v.len() >= 2 && v.iter().all(|x| matches!(x, Filter::Atom(Atom::Keyword(_)))) => true,
        // "that are instant cards, sorcery cards, and/or have an Adventure"
        Filter::Any(v) if is_relative_union(v) => true,
        Filter::Atom(
            Atom::Keyword(_)
            | Atom::BasicLandType
            | Atom::KeywordWith(_)
            | Atom::Stat { .. }
            | Atom::Parity { .. }
            | Atom::Counters { .. }
            | Atom::Named(_)
            | Atom::ExiledWith(_)
            | Atom::CreatedWith(_)
            | Atom::UsedToCraft(_)
            | Atom::VotedAsYou { .. }
            | Atom::SameController
            | Atom::PutOntoBattlefieldWith(_)
            | Atom::Chosen(_)
            | Atom::OfChoice(_)
            | Atom::AttachedTo(_)
            | Atom::Among(_)
            | Atom::Reference(_)
            | Atom::AmongSet(_)
            | Atom::OutsideTheGame
            | Atom::AbleToBlock { .. }
            | Atom::FromIt
            | Atom::InPile { .. }
            | Atom::DealtDamageBy { .. }
            | Atom::Shares { .. }
            | Atom::DifferentNameFrom(_)
            | Atom::Targets { .. }
            | Atom::Did { .. }
            | Atom::Most { .. }
            | Atom::MostVotes { .. }
            | Atom::Compared { .. }
            | Atom::TargetCount(_)
            | Atom::CouldTarget(_)
            | Atom::CouldEnchant(_)
            | Atom::InManaCost(_)
            | Atom::ManaCost(_)
            | Atom::HasAdventure
            | Atom::HasAbility(_)
            | Atom::Source(_)
            | Atom::MoreThan { .. }
            | Atom::MostOf { .. }
            | Atom::OpponentOf(_)
            | Atom::Controls(_)
            | Atom::DidSo(_)
            | Atom::ThatName
            | Atom::ThatColor
            | Atom::ThatType
            | Atom::Different(_)
            | Atom::Alike(_)
            | Atom::Total { .. }
            | Atom::SameZone
            | Atom::Controllers { .. },
        ) => true,
        Filter::Not(b) if matches!(b.as_ref(), Filter::Atom(Atom::Reference(r))
            if r.form == ir::reference::RefForm::Choice) => true,
        Filter::Not(b) => matches!(
            b.as_ref(),
            Filter::Atom(
                Atom::Keyword(_)
                    | Atom::Named(_)
                    | Atom::Among(_)
                    | Atom::Shares { .. }
                    | Atom::InManaCost(_)
                    | Atom::InZone(_)
                    | Atom::HasAbility(_)
                    | Atom::Controls(_)
                    | Atom::Chosen(_)
                    | Atom::Counters { .. }
            )
        ),
        _ => false,
    }
}

/// "you control", "an opponent controls", "you don't control", "your opponents control",
/// "they control" (and the same for own/owns); with a past form, "you controlled", "they didn't
/// control".
fn relation(base: &str, third: &str, past: Option<&str>, player_sg: &P<Selector>, player_pl: &P<Selector>) -> P<Relation> {
    let rel = |negated: bool, past: bool| move |who: Selector| Some(Relation { who: Box::new(who), negated, past });
    let unrel =
        |negated: bool, past: bool| move |r: &Relation| (r.negated == negated && r.past == past).then(|| (*r.who).clone());
    let you = lit("you").to(Selector::you());
    let they =
        lit("they").to(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player)));
    let plural_subject = alt("plural subject", vec![you, they, player_pl.clone()]);
    let mut v = vec![
        plural_subject.skip(&lit(base)).map(rel(false, false), unrel(false, false)),
        plural_subject.skip(&lit(&format!("don't {base}"))).map(rel(true, false), unrel(true, false)),
        player_sg.skip(&lit(third)).map(rel(false, false), unrel(false, false)),
        player_sg.skip(&lit(&format!("doesn't {base}"))).map(rel(true, false), unrel(true, false)),
    ];
    if let Some(past) = past {
        let any = alt("subject", vec![plural_subject.clone(), player_sg.clone()]);
        v.push(any.skip(&lit(past)).map(rel(false, true), unrel(false, true)));
        v.push(any.skip(&lit(&format!("didn't {base}"))).map(rel(true, true), unrel(true, true)));
        // A passive modifier names the current holder: "controlled by target player".
        // Canonical printing uses "target player controls", keeping historical control distinct.
        v.push(lit(&format!("{past} by")).then_keep(&any).map(rel(false, false), |_| None).alias());
    }
    alt("controller", v)
}

pub(crate) fn zone_noun(n: Number) -> P<Zone> {
    table(
        "zone",
        [Zone::Graveyard, Zone::Hand, Zone::Library]
            .iter()
            .map(|z| (inflect(z.word(), n).replace("librarys", "libraries"), *z))
            .collect(),
    )
}

/// One owned zone or several of one owner: "graveyard", "hand or graveyard", "graveyard, hand,
/// and library". The one rule for zone lists, after "from" or "in", as a whole zone ("their hand
/// and graveyard") and after "search".
pub(crate) fn zones() -> P<ir::Coord<Zone>> {
    crate::action::coord(&zone_noun(Number::Sg))
}

/// "in your graveyard", "from a graveyard", "from your hand or graveyard", "in all graveyards",
/// "in exile", "on the battlefield".
fn zone_spec(prep: ZonePrep, player_sg: &P<Selector>) -> P<ZoneSpec> {
    let p = match prep {
        ZonePrep::In => "in",
        ZonePrep::From => "from",
        ZonePrep::OtherThan => "from anywhere other than",
    };
    let owner = zone_owner(player_sg);
    let all_players = Selector::new(Quant::Each, ObjectSet::of(Domain::Player));
    let ap = all_players.clone();
    let ap2 = all_players.clone();
    let owned = lit(p).then_keep(&owner).then(&zones()).map(
        |(o, zones)| Some(ZoneSpec { zones, owner: Some(Box::new(o)), slice: None }),
        move |zs| {
            let o = (**zs.owner.as_ref()?).clone();
            (o != ap2 && o != that_zone() && zs.slice.is_none()).then(|| (o, zs.zones.clone()))
        },
    );
    // "from that graveyard", "in that hand": the zone the text last spoke of, said again
    let again = lit(p).skip(&lit("that")).then_keep(&zone_noun(Number::Sg)).map(
        |z| Some(ZoneSpec::of(z, Some(that_zone()))),
        |zs| (zs.owner.as_deref() == Some(&that_zone()) && zs.slice.is_none()).then(|| zs.zone()).flatten(),
    );
    // "in your opponents' graveyards"
    let theirs = lit(&format!("{p} your opponents'")).then_keep(&zone_noun(Number::Pl)).map(
        |z| Some(ZoneSpec::of(z, Some(your_opponents()))),
        |zs| if zs.owner.as_deref() == Some(&your_opponents()) && zs.slice.is_none() { zs.zone() } else { None },
    );
    // "from graveyards" names the same cards as "from all graveyards"
    let all_word = alt("all", vec![lit(&format!("{p} all")), lit(p).alias()]);
    let all = all_word.then_keep(&crate::action::coord(&zone_noun(Number::Pl))).map(
        move |zones| Some(ZoneSpec { zones, owner: Some(Box::new(ap.clone())), slice: None }),
        move |zs| if zs.owner.as_deref() == Some(&all_players) && zs.slice.is_none() { Some(zs.zones.clone()) } else { None },
    );
    let shared = |text: &str, zone: Zone| lit(text).to(ZoneSpec::of(zone, None));
    // "in exile and in your graveyard": in either, the owner said of the second
    let exile_and = lit(&format!("{p} exile and {p}"))
        .then_keep(&owner)
        .then(&zone_noun(Number::Sg))
        .map(
            |(o, z)| (z != Zone::Exile).then(|| ZoneSpec { zones: ir::Coord { items: vec![Zone::Exile, z], conj: ir::Conj::And }, owner: Some(Box::new(o)), slice: None }),
            |zs: &ZoneSpec| match (zs.zones.items.as_slice(), &zs.owner, &zs.slice) {
                ([Zone::Exile, z], Some(o), None) if zs.zones.conj == ir::Conj::And && *z != Zone::Exile => Some(((**o).clone(), *z)),
                _ => None,
            },
        );
    // "in each graveyard" names the same cards as "in all graveyards"
    let each = lit(&format!("{p} each")).then_keep(&zone_noun(Number::Sg)).map(
        |z| Some(ZoneSpec::of(z, Some(Selector::new(Quant::Each, ObjectSet::of(Domain::Player))))),
        |_: &ZoneSpec| None,
    ).alias();
    // "in all players' hands": the same
    let all_players_zones = lit(&format!("{p} all players'")).then_keep(&zone_noun(Number::Pl)).map(
        |z| Some(ZoneSpec::of(z, Some(Selector::new(Quant::Each, ObjectSet::of(Domain::Player))))),
        |_: &ZoneSpec| None,
    ).alias();
    // "from graveyards or from exile": any graveyard, or exile (Aven Interrupter)
    let any_player = Selector::new(Quant::Each, ObjectSet::of(Domain::Player));
    let or_exile_spec = ZoneSpec { zones: ir::Coord { items: vec![Zone::Graveyard, Zone::Exile], conj: ir::Conj::Or }, owner: Some(Box::new(any_player)), slice: None };
    let or_exile = alt("graveyards or exile", vec![
        lit(&format!("{p} graveyards or {p} exile")),
        lit(&format!("{p} graveyards or exile")).alias(),
    ]).to(or_exile_spec);
    let mut v = vec![
        or_exile,
        exile_and,
        owned,
        again,
        all,
        theirs,
        each,
        all_players_zones,
        shared(&format!("{p} exile"), Zone::Exile),
        shared(&format!("{p} the command zone"), Zone::Command),
    ];
    match prep {
        ZonePrep::In => {
            v.push(shared("on the battlefield", Zone::Battlefield));
            v.push(shared("on the stack", Zone::Stack));
            // "you have no cards in hand": the hand of the player the clause is about
            v.push(shared("in hand", Zone::Hand));
        }
        ZonePrep::From => {
            v.push(shared("from the battlefield", Zone::Battlefield));
            v.push(shared("from the stack", Zone::Stack));
            // "a spell from the top of your library": the one card on top of that library
            // (401.1), where a cast or play permission finds it (601.3). A bare card there is
            // the slice "the top card of your library"; `fold_posts` leaves it to that rule.
            v.push(lit("from the top of").then_keep(&zone_said(player_sg)).skip(&lit("library")).map(
                |o| Some(ZoneSpec { zones: ir::Coord::one(Zone::Library), owner: Some(Box::new(o)), slice: Some(Slice::Top(Amount::Const(1))) }),
                |zs| match (zs.zone(), &zs.owner, &zs.slice) {
                    (Some(Zone::Library), Some(o), Some(Slice::Top(Amount::Const(1)))) => Some((**o).clone()),
                    _ => None,
                },
            ));
        }
        ZonePrep::OtherThan => {}
    }
    alt("zone", v)
}

/// The owner of "that library", "that graveyard": whoever owns the zone of that kind the text
/// last spoke of ([`Ref::ThatZone`]).
fn that_zone() -> Selector {
    Selector::new(Quant::Definite(Ref::ThatZone), ObjectSet::of(Domain::Player))
}

/// Whose zone, said before one zone noun: as [`zone_owner`], or "that" ("the top two cards of
/// that library").
fn zone_said(player_sg: &P<Selector>) -> P<Selector> {
    alt("zone said", vec![zone_owner(player_sg), lit("that").to(that_zone())])
}

/// Whose zone: "your", "their", "a" ("a graveyard"), "target player's", "an opponent's".
fn zone_owner(player_sg: &P<Selector>) -> P<Selector> {
    alt(
        "zone owner",
        vec![
            lit("your").to(Selector::you()),
            lit("their").to(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player))),
            alt("a", vec![lit("a"), lit("an").alias()])
                .to(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player))),
            player_sg
                .then(&leaf(
                    "'s",
                    |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()),
                    |()| Some(Kind::Clitic("'s".into())),
                ))
                .map(|(s, ())| Some(s), |s| Some((s.clone(), ()))),
        ],
    )
}

/// "that's attacking", "that's tapped and attacking", "that's an artifact, creature, or
/// enchantment", "that isn't a Demon, Devil, or Imp", "that are Spirits", "that aren't mana
/// abilities"; "that is" is an alternate spelling of "that's".
fn relative(n: Number) -> P<Post> {
    let (is, isnt) = match n {
        Number::Sg => (alt("that's", vec![lit("that's"), lit("that is").alias()]), lit("that isn't")),
        Number::Pl => (lit("that are"), lit("that aren't")),
    };
    let a = alt("a", vec![lit("a"), lit("an").alias()]);
    let noun = match n {
        Number::Sg => {
            let one = a.then_keep(&core(Number::Sg, Union::Or));
            // "that's an Ape or a Monkey": the same as "an Ape or Monkey" when both are types
            let either = one.skip(&lit("or")).then(&one).map(
                |((d1, x), (d2, y))| match (x.as_slice(), y.as_slice()) {
                    ([x], [y]) if d1 == d2 && is_type_filter(x) && is_type_filter(y) => {
                        Some((d1, vec![Filter::Any(vec![x.clone(), y.clone()])]))
                    }
                    _ => None,
                },
                |_| None,
            );
            alt("noun", vec![one, either.alias()])
        }
        Number::Pl => core(Number::Pl, Union::And),
    };
    // "that's one or more colors": not colorless (CR 105.2c), said only after the verb
    let colored = lit("one or more colors").to(Filter::Not(Box::new(Filter::Atom(Atom::Colorless))));
    let adj = alt("adjectives", vec![adjectives(Union::Or), colored]);
    let two = adj.then(&lit("and").then_keep(&adj)).map(|(x, y)| Some(vec![x, y]), |v: &Vec<Filter>| match v.as_slice() {
        [x, y] => Some((x.clone(), y.clone())),
        _ => None,
    });
    let one = adj.map(|x| Some(vec![x]), |v: &Vec<Filter>| match v.as_slice() {
        [x] => Some(x.clone()),
        _ => None,
    });
    let complement = alt(
        "complement",
        vec![
            alt("adjectives", vec![one, two]).map(
                |v| Some(Complement::Adjectives(v)),
                |c| match c {
                    Complement::Adjectives(v) => Some(v.clone()),
                    _ => None,
                },
            ),
            noun.map(
                |(d, items)| (!items.is_empty()).then_some(Complement::Noun(d, items)),
                |c| match c {
                    Complement::Noun(d, items) => Some((d.clone(), items.clone())),
                    _ => None,
                },
            ),
        ],
    );
    let mk = |negated: bool| move |complement| Some(Post::Relative { negated, complement });
    let un = |negated: bool| {
        move |p: &Post| match p {
            Post::Relative { negated: n, complement } if *n == negated => Some(complement.clone()),
            _ => None,
        }
    };
    alt(
        "relative clause",
        vec![is.then_keep(&complement).map(mk(false), un(false)), isnt.then_keep(&complement).map(mk(true), un(true))],
    )
}

/// An event verb phrase as a relative clause uses it (past tense, or the base form after
/// "didn't"); built once, on first use, since the event grammar is built from the selectors.
fn event_verb(form: crate::event::Form, plural: bool) -> P<EventAction> {
    use crate::event::Form;
    static CELLS: [OnceLock<P<EventAction>>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let i = match (form, plural) {
        (Form::Past, false) => 0,
        (Form::Past, true) => 1,
        _ => 2,
    };
    CELLS[i].get_or_init(|| crate::event::action(form, plural)).clone()
}

/// Membership in a named group; an unqualified pronoun awaits lexical binding.
fn among(s: Selector) -> Filter {
    let them = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::Them)), ObjectSet::of(Domain::Object));
    if s == them {
        Filter::Atom(Atom::Reference(UnresolvedRef::cards(RefForm::Anaphor)))
    } else {
        Filter::Atom(Atom::Among(Box::new(s)))
    }
}

fn unamong(f: &Filter) -> Option<Selector> {
    match f {
        Filter::Atom(Atom::Reference(r)) if *r == UnresolvedRef::cards(RefForm::Anaphor) =>
            Some(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::Them)), ObjectSet::of(Domain::Object))),
        Filter::Atom(Atom::Among(s)) => Some((**s).clone()),
        _ => None,
    }
}

// A positive, argument-free passive in this instruction scope names a producer's
// result. Other event relations retain their arguments, negation and time window.
fn event_filter(action: EventAction, when: When, negated: bool) -> Filter {
    if when == When::ThisWay && !negated
        && let EventAction::Deed(d) = &action
        && d.passive && d.args.is_empty()
    {
        return Filter::Atom(Atom::Reference(UnresolvedRef::action_result(d.verb)));
    }
    Filter::Atom(Atom::Did { action: Box::new(action), when, negated })
}

fn unevent_filter(f: &Filter) -> Option<(EventAction, When, bool)> {
    match f {
        Filter::Atom(Atom::Reference(UnresolvedRef { expected: ReferenceKind::Cards, form: RefForm::ActionResult, qualifiers })) => {
            let [Qualifier::ThisWay(verb)] = qualifiers.as_slice() else { return None };
            Some((EventAction::Deed(Deed { verb: *verb, passive: true, args: Vec::new() }), When::ThisWay, false))
        }
        Filter::Atom(Atom::Did { action, when, negated }) => Some(((**action).clone(), *when, *negated)),
        _ => None,
    }
}

/// A past event predicate, or membership in a result produced in this instruction scope.
fn event_relative(n: Number, sel: &P<Selector>) -> P<Post> {
    use crate::event::Form;
    use crate::kernel::lazy;
    let past = match n {
        Number::Sg => lazy("past event", || event_verb(Form::Past, false)),
        Number::Pl => lazy("past event", || event_verb(Form::Past, true)),
    };
    let base = lazy("event", || event_verb(Form::Base, true));
    let window = alt(
        "window",
        vec![lit("this turn").to(When::ThisTurn), lit("this way").to(When::ThisWay), lit("this game").to(When::ThisGame)],
    );
    // "a card in your graveyard that was put there from the battlefield": into its owner's
    // graveyard, the only one a card can go to (CR 400.3), spelled with "there"
    let owner = Selector::new(
        Quant::Definite(Ref::OwnerOf(Box::new(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object))))),
        ObjectSet::of(Domain::Player),
    );
    let from = alt(
        "from zone",
        vec![
            lit("from the battlefield").to(Some(Zone::Battlefield)),
            lit("from anywhere").to(None),
            lit("from your library").to(Some(Zone::Library)),
        ],
    );
    let there = lit(if n == Number::Sg { "was put there" } else { "were put there" })
        .then_keep(&from)
        .map(move |from| Some((false, EventAction::PutIntoGraveyard { into: Box::new(owner.clone()), from })), |_| None)
        .alias();
    // "that blocked or was blocked by a Zombie this turn"
    let either_block = lit("blocked or was blocked").then_keep(&lit("by").then_keep(sel).opt()).map(
        |by| Some((false, EventAction::BlockedOrWasBlocked { by: by.map(Box::new) })),
        |(neg, a): &(bool, EventAction)| match a {
            EventAction::BlockedOrWasBlocked { by } if !neg => Some(by.as_deref().cloned()),
            _ => None,
        },
    );
    let verb = alt(
        "event",
        vec![
            either_block,
            past.map(|a| Some((false, a)), |(neg, a)| (!neg).then(|| a.clone())),
            lit("didn't").then_keep(&base).map(|a| Some((true, a)), |(neg, a)| neg.then(|| a.clone())),
            there,
        ],
    );
    let pronoun = alt("relative pronoun", vec![lit("that").to(false), lit("who").to(true)]);
    // what an earlier instruction did to the objects (CR 608.2c) is said with the participle
    // alone: "cards exiled this way"; the clause with "that were" is its alias
    let done = lazy("reduced passive", crate::event::reduced_passive);
    let said_reduced = done.clone();
    let full = pronoun.then(&verb).then(&window).map(
        |((who, (negated, action)), when)| {
            Some(Post::Event { who, filter: if who { Filter::Atom(Atom::Did { action: Box::new(action), when, negated }) } else { event_filter(action, when, negated) } })
        },
        move |p| {
            let Post::Event { who, filter } = p else { return None };
            let (action, when, negated) = unevent_filter(filter)?;
            (!(when == When::ThisWay && !negated && !*who && said_reduced.print_text(&action, false).is_some()))
                .then_some(((*who, (negated, action)), when))
        },
    );
    let this_way = lit("this way");
    let event = |negated: bool, action: EventAction| {
        Post::Event { who: false, filter: event_filter(action, When::ThisWay, negated) }
    };
    let unevent = |p: &Post, negated: bool| {
        let Post::Event { who: false, filter } = p else { return None };
        let (action, when, n) = unevent_filter(filter)?;
        (when == When::ThisWay && n == negated).then_some(action)
    };
    // "each creature put into your graveyard from the battlefield this turn": the reduced
    // relative of "that was put … this turn"
    let reduced_then = done
        .then(&alt("window", vec![lit("this turn").to(When::ThisTurn), lit("this game").to(When::ThisGame)]))
        .map(
            |(action, when)| {
                matches!(action, EventAction::PutIntoGraveyard { .. } | EventAction::Deed(Deed { verb: Verb::Draw, passive: true, .. }))
                    .then(|| Post::Event { who: false, filter: event_filter(action, when, false) })
            },
            |p| {
                let Post::Event { who: false, filter } = p else { return None };
                let (action, when, negated) = unevent_filter(filter)?;
                (!negated && matches!(when, When::ThisTurn | When::ThisGame)
                    && matches!(action, EventAction::Deed(Deed { verb: Verb::Draw, passive: true, .. })))
                    .then_some((action, when))
            },
        );
    // "cards exiled this way", "a card put into your hand this way"
    let reduced_done = done.skip(&this_way).map(move |a| Some(event(false, a)), move |p| unevent(p, false));
    // "cards that weren't cast this way", "creatures not chosen this way"
    let wasnt = lit(if n == Number::Sg { "that wasn't" } else { "that weren't" });
    // With no stated window, the compiler must identify the contextual action (608.2c).
    let reduced_not_context = wasnt.then_keep(&done).map(
        |action| Some(Post::Event { who: false, filter: Filter::Atom(Atom::Did { action: Box::new(action), when: When::Context, negated: true }) }),
        |p| match p {
            Post::Event { who: false, filter: Filter::Atom(Atom::Did { action, when: When::Context, negated: true }) } => Some((**action).clone()),
            _ => None,
        },
    );
    let reduced_not = wasnt.then_keep(&done).skip(&this_way).map(move |a| Some(event(true, a)), move |p| unevent(p, true));
    let reduced_not_bare =
        lit("not").then_keep(&done).skip(&this_way).map(move |a| Some(event(true, a)), |_| None).alias();
    // "creatures that were destroyed this way": the same said with a clause
    let was = lit(if n == Number::Sg { "that was" } else { "that were" });
    let reduced_was = was.then_keep(&done).skip(&this_way).map(move |a| Some(event(false, a)), |_| None).alias();
    // "cards they exiled this way", "a creature you sacrificed this way": the doer is named, the
    // passive's `by` role; "cards exiled by them this way" is its alias
    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player));
    let player = sel.map(|s| (s.set.domain == Domain::Player).then_some(s), |s| (s.set.domain == Domain::Player).then(|| s.clone()));
    let doer = alt("doer", vec![lit("they").to(they), player]);
    let did = table(
        "participle",
        done_to().map(|v| (v.bare(Inflection::Past), v)).collect(),
    );
    let by = doer.then(&did).skip(&this_way).map(
        move |(by, verb)| {
            Some(event(false, EventAction::Deed(Deed { verb, passive: true, args: vec![(Role::By, Arg::Noun(by))] })))
        },
        move |p| match unevent(p, false)? {
            EventAction::Deed(Deed { verb, passive: true, args }) => match args.as_slice() {
                [(Role::By, Arg::Noun(by))] => Some((by.clone(), verb)),
                _ => None,
            },
            _ => None,
        },
    );
    // "each creature dealt damage this turn", "a creature dealt damage this way": the reduced
    // relative of "that was dealt damage …" (after objects or players alike)
    let kind =
        alt("damage", vec![lit("dealt damage").to(DamageKind::Any), lit("dealt combat damage").to(DamageKind::Combat)]);
    let reduced = |who: bool| {
        kind.then(&window)
            .map(
                move |(kind, when)| {
                    let action = Box::new(EventAction::IsDealtDamage { kind, excess: false, by: Vec::new() });
                    Some(Post::Event { who, filter: Filter::Atom(Atom::Did { action, when, negated: false }) })
                },
                |_| None,
            )
            .alias()
    };
    alt("event relative", vec![reduced_done, reduced_not, reduced_not_context, reduced_not_bare, reduced_was, by, full, reduced(false), reduced(true), reduced_then])
}

/// What the lexicon's actions say of the object they were done to, with no window: "a creature
/// devoured by it", "the creature it haunts" (CR 702.55b), "creatures it devoured" (702.82b), "a
/// Vehicle crewed by ~" (702.122c); and of the one that did them: "each creature that convoked
/// it" (702.51c). The gap is the action's object (a passive deed with the doer under "by"; the
/// active clause is the same reading) or its subject (an [`Atom::Did`] as in [`event_relative`],
/// when the text names no window, which is the event the ability is about).
fn lexicon_relative(n: Number) -> P<Post> {
    use crate::event::{Form, own_rule, plain};
    use crate::kernel::lazy;
    use ir::{Arg, Deed};
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player));
    let this = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent));
    let self_ref = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef)).to(this.clone());
    // the doer as a subject ("they chose") and under "by" ("chosen by them"): only "they" has
    // two cases
    let doer_as = |they_word: &'static str| {
        alt("doer", vec![lit("it").to(it.clone()), self_ref.clone(), lit("you").to(Selector::you()), lit(they_word).to(they.clone())])
    };
    let (doer, doer_object) = (doer_as("they"), doer_as("them"));
    // the actions done to something, said the regular way
    let verbs = || Verb::ALL.iter().copied().filter(|v| plain(*v) && !own_rule(*v) && v.passive() == Some(Role::What));
    let words = |form: Inflection| table("verb", verbs().map(|v| (v.phrase(form), v)).collect());
    let deed = |verb: Verb, by: Selector| {
        let args = vec![(Role::By, Arg::Noun(by))];
        let action = Box::new(EventAction::Deed(Deed { verb, passive: true, args }));
        Post::Event { who: false, filter: Filter::Atom(Atom::Did { action, when: When::Context, negated: false }) }
    };
    let undeed = |p: &Post| match p {
        Post::Event { who: false, filter: Filter::Atom(Atom::Did { action, when: When::Context, negated: false }) } => match action.as_ref() {
            EventAction::Deed(Deed { verb, passive: true, args }) => match args.as_slice() {
                [(Role::By, Arg::Noun(by))] => Some((*verb, by.clone())),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    let fits = |verb: Verb, by: &Selector| ir::lexicon::of_kind(by, verb.performer());
    let (d1, u1) = (deed.clone(), undeed.clone());
    let passive = words(Inflection::Participle).skip(&lit("by")).then(&doer_object).map(
        move |(verb, by)| fits(verb, &by).then(|| d1(verb, by)),
        move |p| u1(p).filter(|(verb, by)| fits(*verb, by)),
    );
    // "creatures it devoured", "creatures that ~ devoured", "the creature it haunts"
    let that = alt("that", vec![crate::kernel::empty(), lit("that").alias()]);
    let (it3, this3) = (it.clone(), this.clone());
    let third = move |s: &Selector| *s == it3 || *s == this3;
    let (d2, d3) = (deed.clone(), deed.clone());
    let active = that.then_keep(&doer).then(&words(Inflection::Past)).map(
        move |(by, verb)| fits(verb, &by).then(|| d2(verb, by)),
        |_| None,
    );
    let present = that.then_keep(&doer).then(&words(Inflection::Third)).map(
        move |(by, verb)| (fits(verb, &by) && third(&by)).then(|| d3(verb, by)),
        |_| None,
    );
    // "each creature that convoked it": its subject, no window
    let past = match n {
        Number::Sg => lazy("past event", || event_verb(Form::Past, false)),
        Number::Pl => lazy("past event", || event_verb(Form::Past, true)),
    };
    let subject = lit("that").then_keep(&past).map(
        |action| match &action {
            EventAction::Deed(Deed { verb, passive: false, .. }) if verb.relation() => Some(Post::Event {
                who: false,
                filter: Filter::Atom(Atom::Did { action: Box::new(action), when: When::Context, negated: false }),
            }),
            _ => None,
        },
        |p| match p {
            Post::Event { who: false, filter: Filter::Atom(Atom::Did { action, when: When::Context, negated: false }) } => match action.as_ref() {
                a @ EventAction::Deed(Deed { verb, passive: false, .. }) if verb.relation() => Some(a.clone()),
                _ => None,
            },
            _ => None,
        },
    );
    alt("lexicon relative", vec![passive, active.alias(), present.alias(), subject])
}

/// Object properties cannot be distributed onto a player/ability alternative.
fn object_qualities(domain: &Domain) -> bool {
    match domain {
        Domain::Union(arms) => !arms.is_empty() && arms.iter().all(|a| object_qualities(&a.domain)),
        Domain::Player | Domain::Ability => false,
        _ => true,
    }
}

/// The conjunction items a relative clause adds to a noun phrase of domain `subject`, or None
/// when its complement does not fit ("a card that's a mana ability").
fn relative_items(subject: Domain, negated: bool, complement: Complement) -> Option<Vec<Filter>> {
    let objects = object_qualities(&subject);
    let items = match complement {
        Complement::Adjectives(v) => {
            if !objects {
                return None;
            }
            v
        }
        Complement::Noun(d, v) if d == subject || (d == Domain::Permanent && objects) => v,
        Complement::Noun(..) => return None,
    };
    Some(if negated {
        let inner = if items.len() == 1 { items.into_iter().next().expect("one") } else { Filter::All(items) };
        vec![Filter::Not(Box::new(inner))]
    } else {
        items
    })
}

/// Can `filter` stand among the adjectives before a noun ("white or blue", "nonland",
/// "noncreature, nonland")? Mirrors [`adjective`] and [`adjectives`].
fn is_adjective(f: &Filter) -> bool {
    match f {
        Filter::Atom(a) => matches!(
            a,
            Atom::Color(_)
                | Atom::Colorless
                | Atom::Multicolored
                | Atom::Monocolored
                | Atom::Supertype(_)
                | Atom::AbilityKind(_)
        ) || matches!(a, Atom::Status(s) if matches!(s, Status::Commander | Status::Kicked | Status::Suspended) || crate::words::STATUS_WORDS.iter().any(|(_, x)| x == s)),
        Filter::Not(b) => matches!(
            b.as_ref(),
            Filter::Atom(Atom::Color(_) | Atom::Supertype(_) | Atom::Type(_) | Atom::Subtype(_) | Atom::Status(Status::Token | Status::Attacking | Status::Outlaw | Status::Paired))
        ),
        Filter::All(_) if pt_pair(f, Stat::Power, Stat::Toughness).is_some() => true,
        Filter::Any(v) | Filter::All(v) => v.len() >= 2 && v.iter().all(|x| matches!(x, Filter::Atom(_) | Filter::Not(_)) && is_adjective(x)),
    }
}

/// Can `items` be printed as the core of a noun phrase of `domain` (adjectives, types, head)?
fn core_printable(domain: Domain, items: &[Filter]) -> bool {
    // whole noun phrases joined by a union: "mana abilities or loyalty abilities"
    if let [Filter::Any(terms)] = items
        && terms.len() >= 2
        && terms.iter().all(|t| matches!(t, Filter::All(i) if !i.is_empty() && core_printable(domain.clone(), i)))
    {
        return true;
    }
    let c = (domain.clone(), items.to_vec());
    match core_from(&c) {
        // terms of different kinds: "a blue or artifact card", "artifact or tapped creature"
        Some((adjs, None, head)) if matches!(adjs.as_slice(), [Filter::Any(v)] if new_union(v, head != Some(Head::Permanent))) => {
            let permanent = head == Some(Head::Permanent);
            core_to(adjs, None, if permanent { None } else { head }).as_ref() == Some(&c)
        }
        Some((adjs, types, head)) => {
            adjs.iter().all(|a| is_adjective(a) || matches!(a, Filter::Any(v) if shared_modifiers(v)) || (domain == Domain::Ability && keyword_ability(a)))
                && core_to(adjs, types, head).as_ref() == Some(&c)
        }
        None => false,
    }
}

/// Split the core items of a noun phrase into the part printed before the head and a relative
/// clause after it, preferring no relative clause.
fn split_relative(domain: Domain, core: &[Filter]) -> Option<(Vec<Filter>, Option<Post>)> {
    let objects = object_qualities(&domain);
    for k in (0..=core.len()).rev() {
        let (pre, rel) = core.split_at(k);
        if !core_printable(domain.clone(), pre) {
            continue;
        }
        if rel.is_empty() {
            return Some((pre.to_vec(), None));
        }
        if !objects && domain != Domain::Ability {
            continue;
        }
        let colored = Filter::Not(Box::new(Filter::Atom(Atom::Colorless)));
        let (negated, items) = match rel {
            // "that's one or more colors" says Not(Colorless) without a negation
            [f] if *f == colored => (false, rel.to_vec()),
            [Filter::Not(b)] => match b.as_ref() {
                Filter::All(v) if v.len() >= 2 && !is_adjective(b) => (true, v.clone()),
                x => (true, vec![x.clone()]),
            },
            _ => (false, rel.to_vec()),
        };
        let complement = if items.iter().all(|f| is_adjective(f) || *f == colored) && items.len() <= 2 && objects {
            Complement::Adjectives(items)
        } else if objects && core_printable(Domain::Permanent, &items) {
            Complement::Noun(Domain::Permanent, items)
        } else if core_printable(domain.clone(), &items) {
            Complement::Noun(domain, items)
        } else {
            continue;
        };
        return Some((pre.to_vec(), Some(Post::Relative { negated, complement })));
    }
    None
}

/// One subject's owning and controlling, said together (CR 108.3 owner, 110.2 controller): "you
/// both own and control", "you control but don't own", "you own but don't control", and for one
/// player "an opponent controls but doesn't own". "Both" is said of the two holding.
fn own_control(player_sg: &P<Selector>, player_pl: &P<Selector>) -> P<Post> {
    let you = lit("you").to(Selector::you());
    let they = lit("they").to(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player)));
    let plural_subject = alt("plural subject", vec![you, they, player_pl.clone()]);
    let mk = |owns: bool, controls: bool| move |who: Selector| Some(Post::OwnControl { who, owns, controls });
    let un = |owns: bool, controls: bool| {
        move |p: &Post| match p {
            Post::OwnControl { who, owns: o, controls: c } if *o == owns && *c == controls => Some(who.clone()),
            _ => None,
        }
    };
    let mut v = Vec::new();
    // (subject, own, control, don't) for the plural and the singular subject
    for (subject, own, control, dont) in [
        (&plural_subject, "own", "control", "don't"),
        (player_sg, "owns", "controls", "doesn't"),
    ] {
        let (own_w, control_w) = (lit(own), lit(control));
        let (own_base, control_base) = (own.trim_end_matches('s'), control.trim_end_matches('s'));
        let both = subject.skip(&lit("both")).skip(&own_w).skip(&lit("and")).skip(&control_w);
        v.push(both.map(mk(true, true), un(true, true)));
        v.push(subject.skip(&own_w).skip(&lit("and")).skip(&control_w).map(mk(true, true), un(true, true)).alias());
        v.push(subject.skip(&control_w).skip(&lit("but")).skip(&lit(&format!("{dont} {own_base}"))).map(mk(false, true), un(false, true)));
        v.push(subject.skip(&own_w).skip(&lit("but")).skip(&lit(&format!("{dont} {control_base}"))).map(mk(true, false), un(true, false)));
    }
    alt("own and control", v)
}

/// "you own that your opponents control", "they own that you control": the owner's relative
/// clause names who controls it (the owner is the one who holds the object when the game ends or
/// it leaves, 108.3; the controller is who decides, 110.2).
fn owned_that_controlled(player_sg: &P<Selector>, player_pl: &P<Selector>) -> P<Post> {
    let own = relation("own", "owns", None, player_sg, player_pl);
    let control = relation("control", "controls", None, player_sg, player_pl);
    own.skip(&lit("that")).then(&control).map(
        |(owner, controller)| Some(Post::OwnedThatControlled { owner, controller }),
        |p| match p {
            Post::OwnedThatControlled { owner, controller } if !owner.past && !controller.past => {
                Some((owner.clone(), controller.clone()))
            }
            _ => None,
        },
    )
}

fn post(prep: ZonePrep, n: Number, player_sg: &P<Selector>, player_pl: &P<Selector>, sel: &P<Selector>) -> P<Post> {
    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player));
    alt(
        "postmodifier",
        vec![
            relation("control", "controls", Some("controlled"), player_sg, player_pl).map(
                |r| Some(Post::Controller(r)),
                |p| match p {
                    Post::Controller(r) => Some(r.clone()),
                    _ => None,
                },
            ),
            relation("own", "owns", None, player_sg, player_pl).map(
                |r| Some(Post::Owner(r)),
                |p| match p {
                    Post::Owner(r) => Some(r.clone()),
                    _ => None,
                },
            ),
            relation("cast", "casts", None, player_sg, player_pl)
                .then(&alt("cast from", vec![
                    zone_spec(ZonePrep::From, player_sg).map(|z| Some((Some(z), false)), |(z, other): &(Option<ZoneSpec>, bool)| if *other { None } else { z.clone() }),
                    // "spells you cast from anywhere other than your hand"
                    zone_spec(ZonePrep::OtherThan, player_sg).map(|z| Some((Some(z), true)), |(z, other): &(Option<ZoneSpec>, bool)| if *other { z.clone() } else { None }),
                    empty().to((None, false)),
                ]))
                .then(&lit("this turn").opt())
                .map(
                    |((by, (from, other_than)), this_turn)| Some(Post::Caster(ir::selector::Casting { by, from, other_than, this_turn: this_turn.is_some() })),
                    |p| match p {
                        Post::Caster(c) => Some(((c.by.clone(), (c.from.clone(), c.other_than)), c.this_turn.then_some(()))),
                        _ => None,
                    },
                ),
            collective_each(n),
            post_filter(n, sel).map(
                |f| Some(Post::With(f)),
                |p| match p {
                    Post::With(f) => Some(f.clone()),
                    _ => None,
                },
            ),
            zone_spec(prep, player_sg).map(
                |z| Some(Post::Zone(z)),
                |p| match p {
                    Post::Zone(z) => Some(z.clone()),
                    _ => None,
                },
            ),
            // "from a single graveyard"
            lit(match prep {
                ZonePrep::In => "in a single",
                ZonePrep::From | ZonePrep::OtherThan => "from a single",
            })
            .then_keep(&zone_noun(Number::Sg))
            .map(
                |zone| {
                    let a = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
                    Some(Post::SingleZone(ZoneSpec::of(zone, Some(a))))
                },
                |p| match p {
                    Post::SingleZone(z) => {
                        let a = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
                        if z.owner.as_deref() == Some(&a) && z.slice.is_none() { z.zone() } else { None }
                    }
                    _ => None,
                },
            ),
            // "on the battlefield or in the command zone"
            zone_spec(prep, player_sg).list("or").map(
                |v| Some(Post::With(Filter::Any(v.into_iter().map(|z| Filter::Atom(Atom::InZone(z))).collect()))),
                |p| match p {
                    Post::With(f) => in_zones(f),
                    _ => None,
                },
            ),
            // "creature cards you own that aren't on the battlefield"
            alt("not in zone", vec![lit(if n == Number::Sg { "that isn't" } else { "that aren't" }), lit("not").alias()])
                .then_keep(&zone_spec(ZonePrep::In, player_sg)).map(
                |z| Some(Post::With(Filter::Not(Box::new(Filter::Atom(Atom::InZone(z)))))),
                |p| match p {
                    Post::With(Filter::Not(b)) => match b.as_ref() {
                        Filter::Atom(Atom::InZone(z)) => Some(z.clone()),
                        _ => None,
                    },
                    _ => None,
                },
            ),
            relative(n),
            event_relative(n, sel),
            lexicon_relative(n),
            own_control(player_sg, player_pl),
            owned_that_controlled(player_sg, player_pl),
            lit("of their choice").to(Post::Chooser(they)),
            lit("of your choice").to(Post::Chooser(Selector::you())),
            // "of an opponent's choice", "of that player's choice"
            lit("of").then_keep(&zone_owner(player_sg)).skip(&lit("choice")).map(
                |who| {
                    let a = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
                    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Player));
                    (who != a && who != they && who != Selector::you()).then_some(Post::Chooser(who))
                },
                |p| match p {
                    Post::Chooser(s) => Some(s.clone()),
                    _ => None,
                },
            ),
            lit("at random").to(Post::Random(ir::selector::Random::AtRandom)),
            lit("chosen at random").to(Post::Random(ir::selector::Random::ChosenAtRandom)),
        ],
    )
}

/// Properties can repeat their introducer after "and": "with power 2 and with flying".
/// Require an earlier modifier, and retain the usual nested-phrase attachment checks.
fn post_sequence(prep: ZonePrep, n: Number, player_sg: &P<Selector>, player_pl: &P<Selector>, sel: &P<Selector>) -> P<Vec<Post>> {
    let one = post(prep, n, player_sg, player_pl, sel);
    let property = one.map(
        |p| matches!(p, Post::With(_) | Post::WithAll(_)).then_some(p),
        |p| matches!(p, Post::With(_) | Post::WithAll(_)).then(|| p.clone()),
    );
    let later = alt("later postmodifier", vec![one.clone(), lit("and").then_keep(&property).alias()]);
    let some = one.then(&later.many()).map(
        |(first, mut rest)| { rest.insert(0, first); Some(rest) },
        |v: &Vec<Post>| v.split_first().map(|(first, rest)| (first.clone(), rest.to_vec())),
    );
    alt("postmodifiers", vec![empty().to(Vec::new()), some])
}

/// Fold postmodifiers into a set and a selector's extras; each kind at most once except "with".
///
/// A postmodifier that ends in a nested noun phrase ("attached to target creature") must come
/// last: anything after it belongs to the nested phrase (right association), which keeps "a Role
/// token attached to target creature you control" to one reading. Players take no "with" filters.
fn fold_posts(set: &mut ObjectSet, posts: Vec<Post>) -> Option<(Option<Box<Selector>>, Option<ir::selector::Random>)> {
    let (mut chooser, mut random) = (None, None);
    let mut withs = Vec::new();
    let mut relatives = Vec::new();
    let mut events = Vec::new();
    let mut same_zone = false;
    let nested_at = posts.iter().position(ends_nested);
    if nested_at.is_some_and(|i| i + 1 != posts.len()) {
        return None;
    }
    for p in posts {
        if set.domain == Domain::Player && matches!(&p, Post::With(f) if !is_player_filter(f)) {
            return None;
        }
        // an ability has a source and a keyword of its own, not an ability that has abilities
        if let Post::With(f) = &p {
            let ability = set.domain == Domain::Ability;
            let source = matches!(f, Filter::Atom(Atom::Source(_)));
            if (source && !ability) || (ability && (keyword_ability(f) || has_ability(f))) {
                return None;
            }
        }
        match p {
            Post::Controller(r) if set.controller.is_none() => set.controller = Some(r),
            Post::Owner(r) if set.owner.is_none() => set.owner = Some(r),
            Post::Caster(r) if set.caster.is_none() => set.caster = Some(r),
            // the top of a library holds a spell to cast or a land to play there; a card
            // there is what the slice rule reads ("a card from the top of your library")
            Post::Zone(z) if set.zone.is_none() && (z.slice.is_none() || cast_or_played(set)) => set.zone = Some(z),
            Post::OwnControl { who, owns, controls } if set.controller.is_none() && set.owner.is_none() => {
                set.controller = Some(Relation { who: Box::new(who.clone()), negated: !controls, past: false });
                set.owner = Some(Relation { who: Box::new(who), negated: !owns, past: false });
            }
            Post::OwnedThatControlled { owner, controller } if set.controller.is_none() && set.owner.is_none() => {
                set.controller = Some(controller);
                set.owner = Some(owner);
            }
            Post::SingleZone(z) if set.zone.is_none() => {
                set.zone = Some(z);
                same_zone = true;
            }
            // what an earlier instruction did ("exiled this way") may be followed by a "with" of
            // the same noun phrase; it prints before it (a variant)
            Post::WithAll(filters) if set.domain != Domain::Player && events.is_empty() => withs.extend(filters),
            Post::With(f) if events.iter().all(|e| unevent_filter(e).is_some_and(|(_, when, _)| when == When::ThisWay)) => withs.push(f),
            // "that died this turn" after objects, "who lost life this turn" after players
            Post::Event { who, filter } if who == (set.domain == Domain::Player) => events.push(filter),
            // One relative clause. Independent adjective predicates may follow other
            // postmodifiers; nested-source attachment is already checked above.
            Post::Relative { negated, complement }
                if relatives.is_empty() && (withs.is_empty() || matches!(complement, Complement::Adjectives(_))) => {
                relatives = relative_items(set.domain.clone(), negated, complement)?;
            }
            Post::Chooser(s) if chooser.is_none() => chooser = Some(Box::new(s)),
            Post::Random(r) if random.is_none() => random = Some(r),
            _ => return None,
        }
    }
    if let Filter::All(items) = &mut set.filter {
        // "artifact, enchantment, or creature with flying": what only a creature has is
        // said of the creature, not of the artifacts and enchantments listed before it
        if let Some(Filter::Any(types)) = items.iter_mut().find(|f| types_then_creature(f)) {
            let (of_the_creature, rest): (Vec<Filter>, Vec<Filter>) = withs.into_iter().partition(of_creatures);
            if !of_the_creature.is_empty() {
                let creature = types.pop()?;
                types.push(Filter::All(std::iter::once(creature).chain(of_the_creature).collect()));
            }
            withs = rest;
        }
        items.extend(relatives);
        items.extend(withs);
        items.extend(events);
        if same_zone {
            items.push(Filter::Atom(Atom::SameZone));
        }
    } else {
        return None;
    }
    Some((chooser, random))
}

/// A spell, or a land as "play a land" names it: what a position in a zone, such as the top
/// of a library, is said to hold when it is cast or played from there (601.3, 305.1).
fn cast_or_played(set: &ObjectSet) -> bool {
    let land = matches!(&set.filter, Filter::All(items) if items.contains(&Filter::Atom(Atom::Type(ir::CardType::Land))));
    set.domain == Domain::Spell || (set.domain == Domain::Permanent && land)
}

/// Is this "with …" about what only a creature has: a keyword ability, power, toughness
/// (208.3), their base values or their sum, alone or several under one comparison?
fn of_creatures(f: &Filter) -> bool {
    let pt = |stat: &Stat| matches!(stat, Stat::Power | Stat::Toughness | Stat::BasePower | Stat::BaseToughness | Stat::PowerAndToughness);
    match f {
        Filter::Atom(Atom::Keyword(_)) => true,
        Filter::Atom(Atom::Stat { stat, .. }) => pt(stat),
        Filter::Not(f) => of_creatures(f),
        Filter::Any(v) | Filter::All(v) => !v.is_empty() && v.iter().all(|x| matches!(x, Filter::Atom(Atom::Stat { stat, .. }) if pt(stat))),
        _ => false,
    }
}

/// Is this a list of card types of which the last is creature and the ones before are not:
/// "artifact, battle, enchantment, or creature"?
fn types_then_creature(f: &Filter) -> bool {
    let Filter::Any(types) = f else { return false };
    let creature = Filter::Atom(Atom::Type(ir::CardType::Creature));
    match types.split_last() {
        Some((last, before)) => *last == creature && !before.is_empty() && before.iter().all(|t| matches!(t, Filter::Atom(Atom::Type(_))) && *t != creature),
        None => false,
    }
}

/// The inverse of what [`fold_posts`] does with "artifact, enchantment, or creature with
/// flying": the list of types with its last item plain again, and what was said of the
/// creature. None when the items have no such list.
fn creature_withs(items: &[Filter]) -> Option<(Vec<Filter>, Vec<Filter>)> {
    let at = items.iter().position(|f| match f {
        Filter::Any(types) => match types.last() {
            Some(Filter::All(said)) => {
                let plain: Vec<Filter> = types[..types.len() - 1].iter().cloned().chain(said.first().cloned()).collect();
                said.len() > 1 && said[1..].iter().all(of_creatures) && types_then_creature(&Filter::Any(plain))
            }
            _ => false,
        },
        _ => false,
    })?;
    let Filter::Any(types) = &items[at] else { return None };
    let Some(Filter::All(said)) = types.last() else { return None };
    let plain: Vec<Filter> = types[..types.len() - 1].iter().cloned().chain(said.first().cloned()).collect();
    let mut core = items.to_vec();
    core[at] = Filter::Any(plain);
    Some((core, said[1..].to_vec()))
}

/// The inverse of [`fold_posts`], in the canonical order: controller, owner, caster, with, zone,
/// chooser, random. Returns the core items (with-items removed).
fn unfold_posts(set: &ObjectSet, chooser: &Option<Box<Selector>>, random: Option<ir::selector::Random>) -> Option<(Vec<Filter>, Vec<Post>)> {
    let Filter::All(items) = &set.filter else {
        return None;
    };
    // the keyword of an ability stands before "ability", not after it
    let ability = set.domain == Domain::Ability;
    let split = items.iter().rposition(|f| !is_post_filter(f) || (ability && keyword_ability(f))).map_or(0, |i| i + 1);
    // what is said of the creature of "artifact, enchantment, or creature" prints after the
    // list, first among what "with" says
    let (items, split) = match creature_withs(&items[..split]) {
        Some((core, of_the_creature)) => {
            let split = core.len();
            (core.into_iter().chain(of_the_creature).chain(items[split..].iter().cloned()).collect::<Vec<_>>(), split)
        }
        None => (items.clone(), split),
    };
    let (core, withs) = items.split_at(split);
    // "from a single graveyard" folds its collective predicate last
    let single = withs.last() == Some(&Filter::Atom(Atom::SameZone));
    let withs = if single { &withs[..withs.len() - 1] } else { withs };
    if withs.contains(&Filter::Atom(Atom::SameZone)) || (single && set.zone.is_none()) {
        return None;
    }
    let (core, relative) = split_relative(set.domain.clone(), core).unwrap_or((core.to_vec(), None));
    let mut posts = Vec::new();
    match (&set.controller, &set.owner) {
        (Some(c), Some(o)) if c.who == o.who && !(c.negated && o.negated) && !c.past && !o.past => {
            posts.push(Post::OwnControl { who: (*c.who).clone(), owns: !o.negated, controls: !c.negated });
        }
        (Some(c), Some(o)) if !c.past && !o.past => {
            posts.push(Post::OwnedThatControlled { owner: o.clone(), controller: c.clone() });
        }
        _ => {
            posts.extend(set.controller.clone().map(Post::Controller));
            posts.extend(set.owner.clone().map(Post::Owner));
        }
    }
    posts.extend(set.caster.clone().map(Post::Caster));
    posts.extend(relative);
    // event relatives come last among the postnominal filters, and print after the zone
    let is_event = |f: &Filter| unevent_filter(f).is_some();
    let first_event = withs.iter().position(is_event).unwrap_or(withs.len());
    let (withs, events) = withs.split_at(first_event);
    if !events.iter().all(is_event) {
        return None;
    }
    // a relative clause of predicates ("that are instant cards … and/or have an Adventure")
    // prints after the zone, as the event relatives do
    let (after_zone, withs): (Vec<Filter>, Vec<Filter>) = withs.iter().cloned().partition(|f| matches!(f, Filter::Any(v) if is_relative_union(v)));
    posts.extend(withs.into_iter().map(Post::With));
    posts.extend(set.zone.clone().map(if single { Post::SingleZone } else { Post::Zone }));
    posts.extend(after_zone.into_iter().map(Post::With));
    let who = set.domain == Domain::Player;
    posts.extend(events.iter().map(|f| Post::Event { who, filter: f.clone() }));
    posts.extend(chooser.as_deref().cloned().map(Post::Chooser));
    if let Some(r) = random {
        posts.push(Post::Random(r));
    }
    // a nested-phrase postmodifier prints last (see fold_posts); two of them cannot be printed
    let nested: Vec<Post> = posts.iter().filter(|p| ends_nested(p)).cloned().collect();
    if nested.len() > 1 {
        return None;
    }
    posts.retain(|p| !ends_nested(p));
    posts.extend(nested);
    Some((core, posts))
}

fn ends_nested(p: &Post) -> bool {
    if let Post::With(f) = p {
        // "from among them" ends in a pronoun, which takes nothing after it: "a spell from among
        // them with mana value 3 or less"
        if let Some(s) = unamong(f) {
            return !(matches!(s.quant, Quant::Definite(Ref::Anaphor(Anaphor::Them))) && s.set == ObjectSet::of(s.set.domain.clone()));
        }
    }
    // "it", "them" and "~" take nothing after them, so what follows belongs to the outer phrase:
    // "a spell that shares a card type with it from your hand"
    let closed = |s: &Selector| {
        matches!(s.quant, Quant::Definite(Ref::Anaphor(Anaphor::It | Anaphor::Them | Anaphor::They) | Ref::This)) && s.set == ObjectSet::of(s.set.domain.clone())
    };
    match p {
        Post::With(Filter::Not(b)) if matches!(&**b, Filter::Atom(Atom::Among(s)) if closed(s)) => false,
        Post::With(Filter::Atom(Atom::Shares { with, .. } | Atom::DifferentNameFrom(with))) if closed(with) => false,
        Post::With(Filter::Atom(Atom::DealtDamageBy { by, .. })) if closed(by) => false,
        Post::With(Filter::Atom(Atom::DealtDamageBy { .. })) => true,
        Post::With(Filter::Atom(Atom::MoreThan { than, .. })) if closed(than) => false,
        Post::With(Filter::Atom(Atom::Stat { value: Amount::Counters { on, .. }, .. })) if closed(on) => false,
        // "… the amount of life you gained this turn from your graveyard": the tally ends in
        // its window
        Post::With(Filter::Atom(Atom::Stat { value: Amount::LifeChange { .. }, .. })) => false,
        Post::With(f) if is_combat(f) => true,
        // "with enchant creature you control": the set is the keyword's
        Post::With(Filter::Atom(Atom::KeywordWith(_) | Atom::Source(_))) => true,
        Post::With(Filter::Atom(Atom::Most { among: Some(_), tied: false, .. })) => true,
        Post::With(Filter::Atom(Atom::MoreThan { .. } | Atom::Controls(_) | Atom::CouldEnchant(_))) => true,
        Post::With(Filter::Not(b)) if matches!(b.as_ref(), Filter::Atom(Atom::Controls(_))) => true,
        Post::With(Filter::Atom(Atom::MostOf { measure, tied: false, .. })) => **measure != Measure::Life,
        Post::With(Filter::Atom(Atom::AttachedTo(_) | Atom::Among(_) | Atom::AmongSet(_) | Atom::Shares { .. } | Atom::DifferentNameFrom(_) | Atom::Targets { .. })) => {
            true
        }
        // "with power less than the number of cards in your hand"; a stat of a thing ("~'s
        // power") ends in no noun phrase, so a zone may follow it ("… from your graveyard")
        Post::With(Filter::Atom(Atom::Stat { value, .. })) => !matches!(value, Amount::Const(_) | Amount::X | Amount::Stat { .. } | Amount::That { .. }),
        Post::With(Filter::Not(b)) => matches!(b.as_ref(), Filter::Atom(Atom::Among(_) | Atom::Shares { .. })),
        _ => false,
    }
}

/// "no" takes the singular before players ("no opponent") and the plural otherwise ("no
/// creatures"), so each value has one spelling.
fn no_fits(s: &Selector, num: Number) -> bool {
    s.quant != Quant::No || (num == Number::Sg) == (s.set.domain == Domain::Player)
}

fn agrees(n: &Amount, num: Number) -> bool {
    match num {
        Number::Sg => *n == Amount::Const(1),
        Number::Pl => *n != Amount::Const(1),
    }
}

type Det = (Quant, bool);

/// Determiners for number `num`, as (quantifier, "other").
fn determiner(num: Number) -> P<Det> {
    let other = lit("other").opt();
    let target = lit("target").opt();
    // "[prefix] N [other] [target]"
    let counted = |prefix: Option<&str>, mk: fn(Amount) -> Count, unmk: fn(&Count) -> Option<Amount>| {
        let amount = match prefix {
            Some(p) => lit(p).then_keep(&count_amount()),
            None => count_amount(),
        };
        amount.then(&other).then(&target).map(
            move |((n, o), t)| {
                agrees(&n, num)
                    .then(|| (if t.is_some() { Quant::Target(mk(n)) } else { Quant::Indefinite(mk(n)) }, o.is_some()))
            },
            move |(q, o): &Det| {
                let (c, t) = match q {
                    Quant::Target(c) => (c, true),
                    Quant::Indefinite(c) => (c, false),
                    _ => return None,
                };
                let n = unmk(c)?;
                agrees(&n, num).then(|| ((n, o.then_some(())), t.then_some(())))
            },
        )
    };
    let exactly: fn(Amount) -> Count = Count::Exactly;
    let unexactly: fn(&Count) -> Option<Amount> = |c| match c {
        Count::Exactly(n) => Some(n.clone()),
        _ => None,
    };
    let upto: fn(Amount) -> Count = Count::UpTo;
    let unupto: fn(&Count) -> Option<Amount> = |c| match c {
        Count::UpTo(n) => Some(n.clone()),
        _ => None,
    };
    let one = || Count::Exactly(Amount::Const(1));
    let c = |text: &str, q: Quant, o: bool| lit(text).to((q, o));
    let precisely: fn(Amount) -> Count = Count::Precisely;
    let unprecisely: fn(&Count) -> Option<Amount> = |c| match c {
        Count::Precisely(n) => Some(n.clone()),
        _ => None,
    };
    let mut v = vec![counted(Some("up to"), upto, unupto), counted(Some("exactly"), precisely, unprecisely)];
    match num {
        Number::Sg => {
            // "that targets only a single creature": "a single" stresses the "a"
            let a = alt("a", vec![lit("a"), lit("an").alias(), lit("a single").alias()]);
            v.push(a.to((Quant::Indefinite(one()), false)));
            v.extend([
                c("another", Quant::Indefinite(one()), true),
                c("target", Quant::Target(one()), false),
                c("another target", Quant::Target(one()), true),
                // "onto a second target creature": another one
                c("a second target", Quant::Target(one()), true).alias(),
                c("a third target", Quant::Target(one()), true).alias(),
                c("each", Quant::Each, false),
                c("each other", Quant::Each, true),
                // "no opponent" (players only; "no creatures" is plural)
                c("no", Quant::No, false),
                c("any", Quant::Any, false),
                c("the", Quant::Definite(Ref::Anaphor(Anaphor::The { plural: false })), false),
                count_amount()
                    .then(&other)
                    .map(
                        |(n, o)| {
                            (n == Amount::Const(1))
                                .then_some((Quant::Indefinite(Count::Exactly(Amount::Const(1))), o.is_some()))
                        },
                        |_: &Det| None,
                    )
                    .alias(),
                c("that", Quant::Definite(Ref::Anaphor(Anaphor::That)), false),
                c("enchanted", Quant::Definite(Ref::AttachedTo(Attachment::Enchanted)), false),
                c("equipped", Quant::Definite(Ref::AttachedTo(Attachment::Equipped)), false),
                c("fortified", Quant::Definite(Ref::AttachedTo(Attachment::Fortified)), false),
            ]);
        }
        Number::Pl => {
            v.push(counted(None, exactly, unexactly));
            // "one or more", "two or more other"
            v.push(count_amount().skip(&lit("or more")).then(&other).then(&target).map(
                |((n, o), t)| {
                    Some((
                        if t.is_some() {
                            Quant::Target(Count::AtLeast(n))
                        } else {
                            Quant::Indefinite(Count::AtLeast(n))
                        },
                        o.is_some(),
                    ))
                },
                |(q, o): &Det| {
                    let (n, t) = match q {
                        Quant::Target(Count::AtLeast(n)) => (n, true),
                        Quant::Indefinite(Count::AtLeast(n)) => (n, false),
                        _ => return None,
                    };
                    Some(((n.clone(), o.then_some(())), t.then_some(())))
                },
            ));
            // "at least three other creatures": the same count as "three or more other creatures"
            v.push(
                lit("at least")
                    .then_keep(&count_amount())
                    .then(&other)
                    .then(&target)
                    .map(
                        |((n, o), t)| {
                            Some((
                                if t.is_some() {
                                    Quant::Target(Count::AtLeast(n))
                                } else {
                                    Quant::Indefinite(Count::AtLeast(n))
                                },
                                o.is_some(),
                            ))
                        },
                        |_: &Det| None,
                    )
                    .alias(),
            );
            // "one or two target creatures", "one, two, or three target creatures": every count
            // from the first to the last is listed
            let spelled = |min: &Amount, max: &Amount| -> Option<Vec<Amount>> {
                match (min, max) {
                    (Amount::Const(a), Amount::Const(b)) if b >= a && b - a >= 2 => Some((*a..=*b).map(Amount::Const).collect()),
                    (Amount::Const(a), Amount::Const(b)) if b <= a => None,
                    _ => Some(vec![min.clone(), max.clone()]),
                }
            };
            v.push(count_amount().list("or").then(&other).then(&target).map(
                move |((counts, o), t)| {
                    let (min, max) = (counts.first()?.clone(), counts.last()?.clone());
                    (spelled(&min, &max)? == counts).then_some(())?;
                    let c = Count::Range { min: Box::new(min), max: Box::new(max) };
                    Some((if t.is_some() { Quant::Target(c) } else { Quant::Indefinite(c) }, o.is_some()))
                },
                move |(q, o): &Det| {
                    let (min, max, t) = match q {
                        Quant::Target(Count::Range { min, max }) => (min, max, true),
                        Quant::Indefinite(Count::Range { min, max }) => (min, max, false),
                        _ => return None,
                    };
                    Some(((spelled(min, max)?, o.then_some(())), t.then_some(())))
                },
            ));
            v.extend([
                c("the other", Quant::Definite(Ref::Anaphor(Anaphor::Other { plural: true })), false),
                c("any number of", Quant::Indefinite(Count::AnyNumber), false),
                c("any number of other", Quant::Indefinite(Count::AnyNumber), true),
                c("any number of target", Quant::Target(Count::AnyNumber), false),
                c("any number of other target", Quant::Target(Count::AnyNumber), true),
                c("all", Quant::All, false),
                c("all the", Quant::All, false).alias(),
                c("all other", Quant::All, true),
                c("no other", Quant::No, true),
                c("those", Quant::Definite(Ref::Anaphor(Anaphor::Those)), false),
                c("no", Quant::No, false),
                c("the", Quant::Definite(Ref::Anaphor(Anaphor::The { plural: true })), false),
            ]);
        }
    }
    // "the exiled card", "the sacrificed creatures", "the chosen player"
    let plural = num == Number::Pl;
    // "that exiled card", "those exiled cards" point back the same way
    let the = alt("the", vec![lit("the"), lit(if plural { "those" } else { "that" }).alias()]);
    v.push(the.then_keep(&participle()).map(
        move |what| Some((Quant::Definite(Ref::Anaphor(Anaphor::TheParticiple { what, plural })), false)),
        move |(q, o): &Det| match q {
            Quant::Definite(Ref::Anaphor(Anaphor::TheParticiple { what, plural: p })) if !o && *p == plural => {
                Some(*what)
            }
            _ => None,
        },
    ));
    alt(if num == Number::Sg { "determiner" } else { "plural determiner" }, v)
}

/// The participle before a noun, pointing back at what an action was done to: "the exiled
/// card", "the sacrificed creature", "the chosen player". Any action of the lexicon that is done
/// to something and is one word; a word that names a state ("the tapped creature") reads as
/// the state.
fn participle() -> P<Verb> {
    let state = |w: &str| crate::words::STATUS_WORDS.iter().any(|(s, _)| *s == w);
    table(
        "participle",
        done_to()
            .map(|v| (v.bare(Inflection::Participle), v))
            .filter(|(w, _)| !w.contains(' ') && !state(w))
            .collect(),
    )
}

fn assemble(((q, other), (domain, items)): (Det, (Domain, Vec<Filter>)), posts: Vec<Post>) -> Option<Selector> {
    let mut set = ObjectSet {
        domain,
        filter: Filter::All(items),
        controller: None,
        owner: None,
        caster: None,
        zone: None,
        other,
    };
    // The noun "targets" carries targeting itself: "three targets", not "three
    // target targets". Retargeting constructions consume their target-set wording
    // before selecting the spell, rather than going through this noun phrase.
    let q = if set.domain == Domain::AnyTarget {
        match q {
            Quant::Indefinite(count) => Quant::Target(count),
            Quant::Target(_) | Quant::Any => return None,
            q => q,
        }
    } else { q };
    // "an exiled card" is a card in exile; "the exiled card" is the participle reference
    let the = matches!(&q, Quant::Definite(Ref::Anaphor(Anaphor::The { .. } | Anaphor::That | Anaphor::Those)));
    absorb_exiled(&mut set, the)?;
    let (chooser, random) = fold_posts(&mut set, posts)?;
    Some(Selector { quant: q, set, chooser, random })
}

/// "the next instant or sorcery spell you cast this turn".
fn next_this_turn(posts: &P<Vec<Post>>) -> P<Selector> {
    lit("the next").then_keep(&core(Number::Sg, Union::Or)).then(posts).skip(&lit("this turn")).map(
        |(core, posts)| assemble(((Quant::NextThisTurn, false), core), posts),
        |s: &Selector| {
            let ((det, core), posts) = disassemble(s)?;
            (det == (Quant::NextThisTurn, false)).then_some((core, posts))
        },
    )
}

/// "the first creature spell you cast each turn".
fn nth_each_turn(posts: &P<Vec<Post>>) -> P<Selector> {
    lit("the").then_keep(&crate::event::ordinal()).then(&core(Number::Sg, Union::Or)).then(posts).skip(&lit("each turn")).map(
        |((n, core), posts)| assemble(((Quant::NthEachTurn(n), false), core), posts),
        |s: &Selector| {
            let ((det, core), posts) = disassemble(s)?;
            match det {
                (Quant::NthEachTurn(n), false) => Some(((n, core), posts)),
                _ => None,
            }
        },
    )
}

/// "half the creatures they control of their choice, rounded up".
fn half_of(posts: &P<Vec<Post>>) -> P<Selector> {
    let rounding = alt(
        "rounding",
        vec![lit(", rounded up").to(Some(true)), lit(", rounded down").to(Some(false)), crate::kernel::empty().to(None)],
    );
    lit("half the").then_keep(&core(Number::Pl, Union::And)).then(posts).then(&rounding).map(
        |((core, posts), r)| assemble(((Quant::Half(r), false), core), posts),
        |s: &Selector| {
            let ((det, core), posts) = disassemble(s)?;
            match det {
                (Quant::Half(r), false) => Some(((core, posts), r)),
                _ => None,
            }
        },
    )
}

/// "up to three other target creatures from the battlefield and/or creature cards from
/// graveyards": one determiner over the objects of one description wherever they are of two
/// places, the permanents and the cards in graveyards. The description is said twice, once of
/// each; the set is of objects ([`Domain::Object`]) in either zone.
fn battlefield_and_graveyards() -> P<Selector> {
    fn zone() -> ZoneSpec {
        let all_players = Selector::new(Quant::Each, ObjectSet::of(Domain::Player));
        ZoneSpec {
            zones: ir::Coord { items: vec![Zone::Battlefield, Zone::Graveyard], conj: ir::Conj::AndOr },
            owner: Some(Box::new(all_players)),
            slice: None,
        }
    }
    determiner(Number::Pl)
        .then(&core(Number::Pl, Union::AndOr))
        .skip(&lit("from the battlefield and/or"))
        .then(&core(Number::Pl, Union::AndOr))
        .skip(&lit("from graveyards"))
        .map(
            |(((q, other), (permanents, items)), (cards, again))| {
                let same = permanents == Domain::Permanent && cards == Domain::Card && items == again;
                same.then(|| {
                    let set = ObjectSet { filter: Filter::All(items), zone: Some(zone()), other, ..ObjectSet::of(Domain::Object) };
                    Selector::new(q, set)
                })
            },
            |s: &Selector| {
                let set = &s.set;
                let Filter::All(items) = &set.filter else { return None };
                let plain = set.controller.is_none() && set.owner.is_none() && set.caster.is_none() && s.chooser.is_none() && s.random.is_none();
                (set.domain == Domain::Object && set.zone == Some(zone()) && plain)
                    .then(|| (((s.quant.clone(), set.other), (Domain::Permanent, items.clone())), (Domain::Card, items.clone())))
            },
        )
}

type Parts = ((Det, (Domain, Vec<Filter>)), Vec<Post>);

fn disassemble(s: &Selector) -> Option<Parts> {
    let quant = if s.set.domain == Domain::AnyTarget {
        match &s.quant {
            Quant::Target(Count::Exactly(Amount::Const(1))) | Quant::Any => return None,
            Quant::Target(count) => Quant::Indefinite(count.clone()),
            _ => s.quant.clone(),
        }
    } else { s.quant.clone() };
    let (core, posts) = unfold_posts(&s.set, &s.chooser, s.random)?;
    Some((((quant, s.set.other), (s.set.domain.clone(), core)), posts))
}

/// "the top card of your library", "the top three cards of target player's library", "the bottom
/// card of your library".
fn slice(player_sg: &P<Selector>) -> P<Selector> {
    let end = alt("end", vec![lit("top").to(true), lit("bottom").to(false)]);
    let sg = lit("card").to(Amount::Const(1));
    let pl = count_amount()
        .skip(&lit("cards"))
        .map(|a| (a != Amount::Const(1)).then_some(a), |a| (*a != Amount::Const(1)).then(|| a.clone()));
    let how_many = alt("cards", vec![sg, pl.clone()]);
    let the_top = lit("the").then_keep(&end).then(&how_many).skip(&lit("of"));
    // "that many cards from the top of your library" names the same cards
    let a_card = alt("a", vec![lit("a"), lit("an")]).skip(&lit("card")).to(Amount::Const(1));
    let from_top = alt("cards", vec![a_card, pl])
        .skip(&lit("from the"))
        .then(&end)
        .skip(&lit("of"))
        .map(|(n, top)| Some((top, n)), |_| None)
        .alias();
    // "a number of cards from the top of your library equal to its power": how many is an amount
    // after the noun (CR 107.1, 608.2h); the same cards as "the top N cards"
    let amount = crate::kernel::lazy("amount", || crate::amount::amounts().amount.clone());
    let measured = lit("a number of cards from the")
        .then_keep(&end)
        .skip(&lit("of"))
        .then(&zone_said(player_sg))
        .skip(&lit("library equal to"))
        .then(&amount)
        .map(
            |((top, owner), n)| {
                (!crate::words::is_quantity(&n)).then(|| {
                    let mut set = ObjectSet::of(Domain::Card);
                    set.zone = Some(ZoneSpec {
                        zones: ir::Coord::one(Zone::Library),
                        owner: Some(Box::new(owner)),
                        slice: Some(if top { Slice::Top(n) } else { Slice::Bottom(n) }),
                    });
                    Selector::new(Quant::The, set)
                })
            },
            |s: &Selector| {
                if s.quant != Quant::The || s.set != slice_set(&s.set) {
                    return None;
                }
                let z = s.set.zone.as_ref()?;
                let (top, n) = match z.slice.as_ref()? {
                    Slice::Top(n) => (true, n.clone()),
                    Slice::Bottom(n) => (false, n.clone()),
                    Slice::TopOfKind => return None,
                };
                let owner = (**z.owner.as_ref()?).clone();
                (!crate::words::is_quantity(&n)).then_some(((top, owner), n))
            },
        );
    // "the top creature card of your graveyard": the topmost card of that type
    let top_of_kind = lit("the top")
        .then_keep(&crate::words::type_word(Number::Sg, false))
        .skip(&lit("card of"))
        .then(&zone_said(player_sg))
        .skip(&lit("graveyard"))
        .map(
            |(kind, owner)| {
                let mut set = ObjectSet::of(Domain::Card);
                set.filter = Filter::Atom(kind);
                set.zone = Some(ZoneSpec {
                    zones: ir::Coord::one(Zone::Graveyard),
                    owner: Some(Box::new(owner)),
                    slice: Some(Slice::TopOfKind),
                });
                Some(Selector::new(Quant::The, set))
            },
            |s: &Selector| {
                let z = s.set.zone.as_ref()?;
                let Filter::Atom(kind) = &s.set.filter else { return None };
                let mut plain = ObjectSet::of(Domain::Card);
                plain.filter = s.set.filter.clone();
                plain.zone = s.set.zone.clone();
                let owner = (**z.owner.as_ref()?).clone();
                (s.quant == Quant::The && z.slice == Some(Slice::TopOfKind) && z.zones == ir::Coord::one(Zone::Graveyard) && s.set == plain)
                    .then(|| (kind.clone(), owner))
            },
        );
    // "Exile the top two cards of your library. If …, exile the top three cards instead": the
    // library the text spoke of, unsaid; the same cards as "the top three cards of that library"
    let unsaid = lit("the")
        .then_keep(&end)
        .then(&how_many)
        .not_before(&lit("of"))
        .map(|top_n| Some((top_n, that_zone())), |_| None)
        .alias();
    let said = alt("slice", vec![the_top, from_top]).then(&zone_said(player_sg)).skip(&lit("library"));
    let by_count = alt("slice", vec![said, unsaid]).map(
        |((top, n), owner)| {
            let mut set = ObjectSet::of(Domain::Card);
            set.zone = Some(ZoneSpec {
                zones: ir::Coord::one(Zone::Library),
                owner: Some(Box::new(owner)),
                slice: Some(if top { Slice::Top(n) } else { Slice::Bottom(n) }),
            });
            Some(Selector::new(Quant::The, set))
        },
        |s: &Selector| {
            if s.quant != Quant::The || s.set != slice_set(&s.set) {
                return None;
            }
            let z = s.set.zone.as_ref()?;
            let (top, n) = match z.slice.as_ref()? {
                Slice::Top(n) => (true, n.clone()),
                Slice::Bottom(n) => (false, n.clone()),
                Slice::TopOfKind => return None,
            };
            Some(((top, n), (**z.owner.as_ref()?).clone()))
        },
    );
    alt("slice", vec![by_count, measured, top_of_kind])
}

/// The set a slice selector has: cards in a library slice and nothing else.
/// A whole zone as a definite description: "your hand", "target player's graveyard", "your
/// library" (every card in it), and several of one player: "their hand and graveyard". "That
/// library" is one zone said again.
fn whole_zone(player_sg: &P<Selector>) -> P<Selector> {
    let one = |owner: &Selector, zones: &ir::Coord<Zone>| *owner != that_zone() || zones.items.len() == 1;
    zone_said(player_sg).then(&zones()).map(
        move |(owner, zones)| {
            if !one(&owner, &zones) {
                return None;
            }
            let mut set = ObjectSet::of(Domain::Card);
            set.zone = Some(ZoneSpec { zones, owner: Some(Box::new(owner)), slice: None });
            Some(Selector::new(Quant::The, set))
        },
        move |s: &Selector| {
            let z = s.set.zone.as_ref()?;
            let owner = z.owner.as_deref()?;
            let bare = ObjectSet { zone: s.set.zone.clone(), ..ObjectSet::of(Domain::Card) };
            (s.quant == Quant::The && s.set == bare && z.slice.is_none() && s.chooser.is_none() && s.random.is_none() && one(owner, &z.zones))
                .then(|| (owner.clone(), z.zones.clone()))
        },
    )
}

/// "all opponents' graveyards", "any number of target players' graveyards": the whole zones of
/// several players.
fn whole_zones(player_pl: &P<Selector>) -> P<Selector> {
    let clitic =
        leaf("'", |k| matches!(k, Kind::Clitic(c) if c == "'").then_some(()), |()| Some(Kind::Clitic("'".into())));
    // "all graveyards": every player's, as "in all graveyards" says it
    let every = || Selector::new(Quant::Each, ObjectSet::of(Domain::Player));
    let owner = alt(
        "zones owner",
        vec![
            player_pl.skip(&clitic).map(move |o| (o != every()).then_some(o), move |o| (*o != every()).then(|| o.clone())),
            lit("all").to(every()),
        ],
    );
    owner.then(&zone_noun(Number::Pl)).map(
        |(owner, zone)| {
            let mut set = ObjectSet::of(Domain::Card);
            set.zone = Some(ZoneSpec::of(zone, Some(owner)));
            Some(Selector::new(Quant::The, set))
        },
        move |s: &Selector| {
            let z = s.set.zone.as_ref()?;
            let owner = z.owner.as_deref()?;
            let bare = ObjectSet { zone: s.set.zone.clone(), ..ObjectSet::of(Domain::Card) };
            if s.quant == Quant::The
                && s.set == bare
                && z.slice.is_none()
                && s.chooser.is_none()
                && s.random.is_none()
                && (is_plural(owner) || *owner == every())
            {
                Some((owner.clone(), z.zone()?))
            } else {
                None
            }
        },
    )
}

/// "your commander", "that player's commander": the commander a player owns (CR 903.3; with two
/// commanders, either one, 702.124e).
fn possessed_commander(player_sg: &P<Selector>) -> P<Selector> {
    // the possessive's "a" ("a graveyard") is the indefinite "a commander" here
    let a_player = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player));
    let set = |owner: Selector| {
        let mut set = ObjectSet::of(Domain::Card);
        set.filter = Filter::All(vec![Filter::Atom(Atom::Status(Status::Commander))]);
        set.owner = Some(Relation { who: Box::new(owner), negated: false, past: false });
        set
    };
    zone_owner(player_sg).skip(&lit("commander")).map(
        move |p| (p != a_player).then(|| Selector::new(Quant::The, set(p))),
        move |s: &Selector| {
            let who = s.set.owner.as_ref()?.who.as_ref().clone();
            (*s == Selector::new(Quant::The, set(who.clone()))).then_some(who)
        },
    )
}

fn slice_set(set: &ObjectSet) -> ObjectSet {
    let mut s = ObjectSet::of(Domain::Card);
    s.zone = set.zone.clone();
    s
}

/// `possessor`: what may stand before "'s controller" / "'s owner"; it must not start with a
/// special itself, or the rule would be left-recursive.
fn specials(possessor: &P<Selector>) -> P<Selector> {
    let def = |r: Ref, d: Domain| Selector::new(Quant::Definite(r), ObjectSet::of(d));
    let any_target = |other: bool| {
        let mut set = ObjectSet::of(Domain::AnyTarget);
        set.other = other;
        Selector::new(Quant::Target(Count::Exactly(Amount::Const(1))), set)
    };
    let self_ref = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef));
    let granting = leaf("§", |k| matches!(k, Kind::GrantingSelf).then_some(()), |()| Some(Kind::GrantingSelf));
    alt(
        "special",
        vec![
            lit("you").to(Selector::you()),
            lit("the").then_keep(&crate::event::ordinal()).skip(&lit("player")).map(
                |n| Some(Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::Ordinal { n })), ObjectSet::of(Domain::Player))),
                |s| match s.quant {
                    Quant::Definite(Ref::Anaphor(Anaphor::Ordinal { n })) if s.set == ObjectSet::of(Domain::Player) => Some(n),
                    _ => None,
                },
            ),
            self_ref.to(def(Ref::This, Domain::Permanent)),
            lit("this permanent").to(def(Ref::This, Domain::Permanent)).alias(),
            // Generated rules retain a typed spell source. "This spell" round-trips
            // that type; "~" parses as a permanent selector.
            lit("this spell").to(def(Ref::This, Domain::Spell)),
            // the ability whose text this is ("This ability costs {1} less to activate")
            lit("this ability").to(def(Ref::This, Domain::Ability)),
            granting.to(def(Ref::Granting, Domain::Permanent)),
            lit("defending player").to(def(Ref::DefendingPlayer, Domain::Player)),
            // "the player or planeswalker it's attacking"
            lit("the player or planeswalker it's attacking").to(Selector::new(
                Quant::Definite(Ref::AttackedBy(Box::new(def(Ref::Anaphor(Anaphor::It), Domain::Object)))),
                ObjectSet::of(Domain::AnyTarget),
            )),
            // "the player or planeswalker that creature is attacking"
            lit("the player or planeswalker that creature is attacking").to(Selector::new(
                Quant::Definite(Ref::AttackedBy(Box::new(Selector::new(
                    Quant::Definite(Ref::Anaphor(Anaphor::That)),
                    {
                        let mut set = ObjectSet::of(Domain::Permanent);
                        set.filter = Filter::All(vec![Filter::Atom(Atom::Type(ir::CardType::Creature))]);
                        set
                    },
                )))),
                ObjectSet::of(Domain::AnyTarget),
            )),
            lit("the permanent it becomes").to(def(Ref::PermanentItBecomes, Domain::Permanent)),
            lit("your team").to(def(Ref::YourTeam, Domain::Player)),
            lit("the active player").to(def(Ref::ActivePlayer, Domain::Player)),
            lit("the monarch").to(def(Ref::Monarch, Domain::Player)),
            lit("the amassed Army").to(def(Ref::AmassedArmy, Domain::Permanent)),
            lit("the attacking player").to(def(Ref::AttackingPlayer, Domain::Player)),
            lit("that attacking player").to(def(Ref::AttackingPlayer, Domain::Player)).alias(),
            lit("attacking player").to(def(Ref::AttackingPlayer, Domain::Player)).alias(),
            lit("it").to(def(Ref::Anaphor(Anaphor::It), Domain::Object)),
            lit("them").to(def(Ref::Anaphor(Anaphor::Them), Domain::Object)),
            lit("the rest").to(def(Ref::Unresolved(UnresolvedRef::cards(RefForm::Remainder)), Domain::Object)),
            // "all revealed cards not cast this way": the offered card too, unless it was cast
            lit("all revealed cards not cast this way").to(def(Ref::RevealedNotCast, Domain::Object)),
            lit("the other").to(def(Ref::Anaphor(Anaphor::Other { plural: false }), Domain::Object)),
            lit("the others").to(def(Ref::Anaphor(Anaphor::Other { plural: true }), Domain::Object)),
            lit("the copy").to(def(Ref::Anaphor(Anaphor::Copy { plural: false }), Domain::Object)),
            lit("the copies").to(def(Ref::Anaphor(Anaphor::Copy { plural: true }), Domain::Object)),
            lit("that copy").to(def(Ref::Anaphor(Anaphor::ThatCopy), Domain::Object)),
            lit("both creatures").to(def(Ref::Anaphor(Anaphor::BothPaired), Domain::Permanent)),
            lit("itself").to(def(Ref::Anaphor(Anaphor::Itself), Domain::Object)),
            relative_player(possessor, true),
            relative_player(possessor, false),
            lit("any target").to(any_target(false)),
            // "any target that isn't a Dragon"
            lit("any target").then_keep(&relative(Number::Sg)).map(
                move |post| {
                    let mut s = any_target(false);
                    fold_posts(&mut s.set, vec![post])?;
                    Some(s)
                },
                move |s: &Selector| {
                    let plain = any_target(false);
                    if s.quant != plain.quant || s.set.other || s.chooser.is_some() || s.random.is_some() {
                        return None;
                    }
                    match unfold_posts(&s.set, &None, None)? {
                        (core, posts) if core.is_empty() => match posts.as_slice() {
                            [p @ Post::Relative { .. }] => Some(p.clone()),
                            _ => None,
                        },
                        _ => None,
                    }
                },
            ),
            lit("any other target").to(any_target(true)),
            // "1 damage to any target of an opponent's choice" (Cuombajj Witches): chosen by an
            // opponent, as "target creature of an opponent's choice" is
            lit("any target of an opponent's choice").to({
                let mut s = any_target(false);
                let mut opponents = ObjectSet::of(Domain::Player);
                opponents.filter = Filter::All(vec![Filter::Atom(Atom::Opponent)]);
                s.chooser = Some(Box::new(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), opponents)));
                s
            }),
        ],
    )
}

/// "its controller", "enchanted creature's controller", "that card's owner".
fn relative_player(possessor: &P<Selector>, controller: bool) -> P<Selector> {
    let it = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::It)), ObjectSet::of(Domain::Object));
    let own = own_reference().map(
        |r| Some(Selector::new(Quant::Definite(r), ObjectSet::of(Domain::Permanent))),
        |s: &Selector| match &s.quant {
            Quant::Definite(r @ (Ref::This | Ref::Granting)) if s.set == ObjectSet::of(Domain::Permanent) => Some(r.clone()),
            _ => None,
        },
    );
    let clitic =
        leaf("'s", |k| matches!(k, Kind::Clitic(c) if c == "'s").then_some(()), |()| Some(Kind::Clitic("'s".into())));
    // "their controller": of the objects "they" stands for ("creatures can't attack you unless
    // their controller pays {2}")
    let they = Selector::new(Quant::Definite(Ref::Anaphor(Anaphor::They)), ObjectSet::of(Domain::Object));
    let whose = alt(
        "possessor",
        vec![lit("its").to(it), lit("their").to(they), own.skip(&clitic), possessor.skip(&clitic)],
    );
    let noun = if controller { "controller" } else { "owner" };
    // "the owner of target permanent" says "target permanent's owner"
    let of = lit(&format!("the {noun} of")).then_keep(possessor).alias();
    // "the controller of the permanent it becomes"
    let of_becomes = lit(&format!("the {noun} of the permanent it becomes"))
        .to(Selector::new(Quant::Definite(Ref::PermanentItBecomes), ObjectSet::of(Domain::Permanent)));
    alt("relative player", vec![whose.skip(&lit(noun)), of_becomes, of]).map(
        move |s| {
            let r = if controller { Ref::ControllerOf(Box::new(s)) } else { Ref::OwnerOf(Box::new(s)) };
            Some(Selector::new(Quant::Definite(r), ObjectSet::of(Domain::Player)))
        },
        move |s: &Selector| match (&s.quant, controller) {
            (Quant::Definite(Ref::ControllerOf(o)), true) | (Quant::Definite(Ref::OwnerOf(o)), false) => {
                Some((**o).clone())
            }
            _ => None,
        },
    )
}

/// Does this selector take a plural verb ("two target creatures", "those cards", "the top three
/// cards of your library", "your opponents")?
pub fn is_plural(s: &Selector) -> bool {
    let many = |n: &Amount| *n != Amount::Const(1);
    match &s.quant {
        Quant::Target(c) | Quant::Indefinite(c) => match c {
            Count::Exactly(n) | Count::UpTo(n) | Count::Precisely(n) => many(n),
            Count::AtLeast(_) | Count::AnyNumber | Count::Range { .. } => true,
        },
        Quant::All => true,
        Quant::Each | Quant::No | Quant::Any | Quant::NextThisTurn | Quant::NthEachTurn(_) => false,
        Quant::Half(_) => true,
        Quant::Definite(Ref::Anaphor(a)) => {
            matches!(
                a,
                Anaphor::Those
                    | Anaphor::They
                    | Anaphor::Them
                    | Anaphor::Other { plural: true }
                    | Anaphor::Copy { plural: true }
                    | Anaphor::BothPaired
                    | Anaphor::The { plural: true }
                    | Anaphor::TheParticiple { plural: true, .. }
            )
        }
        Quant::Definite(Ref::Unresolved(r)) => r.expected == ReferenceKind::Cards,
        Quant::Definite(_) => false,
        Quant::The => match s.set.zone.as_ref().and_then(|z| z.slice.as_ref()) {
            Some(Slice::Top(n) | Slice::Bottom(n)) => many(n),
            Some(Slice::TopOfKind) | None => false,
        },
    }
}

fn your_opponents() -> Selector {
    let mut set = ObjectSet::of(Domain::Player);
    set.filter = Filter::All(vec![Filter::Atom(Atom::Opponent)]);
    Selector::new(Quant::All, set)
}

/// Complete arms of different domains, with a trailing controller shared when no
/// earlier arm has its own controller. Zone/owner/caster scoping is deliberately left
/// unsupported here until its cross-domain typing is implemented.
fn join_sets(mut arms: Vec<ObjectSet>) -> Option<ObjectSet> {
    if arms.len() < 2 || arms.iter().all(|a| a.domain == arms[0].domain)
        || arms.iter().any(|a| a.other || a.zone.is_some() || a.owner.is_some() || a.caster.is_some())
    { return None; }
    let last = arms.len() - 1;
    // A nested noun consumes its own trailing coordination, just as it consumes its
    // following modifiers. Do not split "abilities targeting a permanent or player".
    for arm in &arms[..last] {
        if unfold_posts(arm, &None, None)?.1.iter().any(ends_nested) { return None; }
    }
    let controller = if arms[..last].iter().all(|a| a.controller.is_none()) {
        arms[last].controller.take()
    } else { None };
    Some(ObjectSet { controller, ..ObjectSet::of(Domain::Union(arms)) })
}

fn split_sets(set: &ObjectSet) -> Option<Vec<ObjectSet>> {
    let Domain::Union(arms) = &set.domain else { return None };
    if set.filter != Filter::any_object() || set.other || set.zone.is_some()
        || set.owner.is_some() || set.caster.is_some() { return None; }
    let mut arms = arms.clone();
    if let Some(controller) = &set.controller {
        if arms.iter().any(|a| a.controller.is_some()) { return None; }
        arms.last_mut()?.controller = Some(controller.clone());
    }
    (join_sets(arms.clone()).as_ref() == Some(set)).then_some(arms)
}

/// Repeated determiners make complete noun-phrase alternatives, rather than a
/// shared adjective/head union. Each arm retains its own postmodifiers.
fn join_noun_phrases(arms: Vec<Selector>) -> Option<Selector> {
    let one = Quant::Indefinite(Count::Exactly(Amount::Const(1)));
    if arms.len() < 2 || arms.iter().any(|s| s.quant != one || s.chooser.is_some() || s.random.is_some()
        || matches!(s.set.domain, Domain::Union(_))) { return None; }
    for arm in &arms[..arms.len() - 1] {
        if unfold_posts(&arm.set, &None, None)?.1.iter().any(ends_nested) { return None; }
    }
    Some(Selector::new(one, ObjectSet::of(Domain::Union(arms.into_iter().map(|s| s.set).collect()))))
}

fn split_noun_phrases(s: &Selector) -> Option<Vec<Selector>> {
    let Domain::Union(arms) = &s.set.domain else { return None };
    if s.set != ObjectSet::of(Domain::Union(arms.clone())) { return None; }
    let values: Vec<_> = arms.iter().map(|set| Selector::new(s.quant.clone(), set.clone())).collect();
    (join_noun_phrases(values.clone()).as_ref() == Some(s)).then_some(values)
}

/// One shared target over complete, differently typed alternatives. A retained
/// postmodifier or an untyped head marks the boundary; shared type/head unions stay in core.
/// Controllers, owners, zones and predicates remain local to their arms.
fn target_union(set: &P<ObjectSet>) -> P<Selector> {
    fn arm_ok(set: &ObjectSet) -> bool { !matches!(set.domain, Domain::Union(_)) && !set.other }
    fn qualified(set: &ObjectSet) -> bool {
        unfold_posts(set, &None, None).is_some_and(|(_, posts)| !posts.is_empty() && !posts.iter().any(ends_nested))
    }
    fn bare_permanent(set: &ObjectSet) -> bool {
        fn permanent_types(f: &Filter) -> bool {
            match f {
                Filter::Atom(Atom::Type(t)) => t.permanent(),
                Filter::Atom(Atom::Subtype(_)) => true,
                Filter::All(v) | Filter::Any(v) => !v.is_empty() && v.iter().all(permanent_types),
                _ => false,
            }
        }
        let Some((items, posts)) = unfold_posts(set, &None, None) else { return false };
        let Some((_, Some(types), None)) = core_from(&(set.domain.clone(), items)) else { return false };
        set.domain == Domain::Permanent && posts.is_empty() && permanent_types(&types)
    }
    fn untyped_head(set: &ObjectSet) -> bool {
        matches!(set.domain, Domain::Card | Domain::Spell | Domain::Ability | Domain::Source | Domain::Player)
            && *set == ObjectSet::of(set.domain.clone())
    }
    fn alternatives(first: &ObjectSet, second: &ObjectSet) -> bool {
        first.domain != second.domain && (qualified(first)
            || (bare_permanent(first) && untyped_head(second))
            || (untyped_head(first) && bare_permanent(second)))
    }
    fn target(det: &Det) -> bool { matches!(det.0, Quant::Target(_)) }
    let arm = set.map(|s| arm_ok(&s).then_some(s), |s| arm_ok(s).then(|| s.clone()));
    determiner(Number::Sg).map(|d| target(&d).then_some(d), |d| target(d).then(|| d.clone()))
        .then(&arm.skip(&lit("or")).then(&arm)).map(
            |((quant, other), (first, second))| alternatives(&first, &second).then(||
                Selector::new(quant, ObjectSet { other, ..ObjectSet::of(Domain::Union(vec![first, second])) })),
            |s: &Selector| {
                let Domain::Union(arms) = &s.set.domain else { return None };
                let [first, second] = arms.as_slice() else { return None };
                (alternatives(first, second) && s.chooser.is_none() && s.random.is_none()
                    && s.set == (ObjectSet { other: s.set.other, ..ObjectSet::of(Domain::Union(arms.clone())) }))
                    .then(|| ((s.quant.clone(), s.set.other), (first.clone(), second.clone())))
            },
        )
}

fn build() -> Selectors {
    let sg_in = forward::<Selector>("selector");
    let any_sel = forward::<Selector>("any selector");
    let pl_in = forward::<Selector>("plural selector");
    let is_player = |s: &Selector| s.set.domain == Domain::Player;
    let player = |f: &P<Selector>| f.map(move |s| is_player(&s).then_some(s), move |s| is_player(s).then(|| s.clone()));
    let (player_sg, player_pl) = (player(&sg_in.rule()), player(&pl_in.rule()));
    let special = specials(&sg_in.rule());
    let player_one = alt("player", vec![player_sg.clone(), special.clone()])
        .map(move |s| is_player(&s).then_some(s), move |s| is_player(s).then(|| s.clone()));
    // "that player or that planeswalker's controller"
    let definite = |s: &Selector| matches!(s.quant, Quant::Definite(_)) && *s != Selector::you();
    let one_definite = player_one.map(move |s| definite(&s).then_some(s), move |s| definite(s).then(|| s.clone()));
    let either = one_definite.skip(&lit("or")).then(&one_definite).map(
        |(a, b)| Some(Selector::new(Quant::Definite(Ref::Either(vec![a, b])), ObjectSet::of(Domain::Player))),
        |s: &Selector| match (&s.quant, s.set == ObjectSet::of(Domain::Player)) {
            (Quant::Definite(Ref::Either(v)), true) if s.chooser.is_none() && s.random.is_none() => match v.as_slice() {
                [a, b] => Some((a.clone(), b.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    let player_sg_any = alt("player", vec![player_one, either.clone()]);

    let branch = |num: Number, prep: ZonePrep| {
        let posts = post_sequence(prep, num, &player_sg_any, &player_pl, &any_sel.rule());
        let unions = match num {
            Number::Sg => vec![Union::Or, Union::And],
            Number::Pl => vec![Union::AndOr, Union::And],
        };
        let alternatives = unions
            .into_iter()
            .map(|u| {
                let fits = move |(q, _): &Det| Union::of(q, num) == u;
                determiner(num)
                    .map(move |d| fits(&d).then_some(d), move |d| fits(d).then(|| d.clone()))
                    .then(&core(num, u))
                    .then(&posts)
                    .map(
                        move |(dc, posts)| assemble(dc, posts).filter(|s| no_fits(s, num)),
                        move |s| no_fits(s, num).then(|| disassemble(s)).flatten(),
                    )
            })
            .collect::<Vec<_>>();
        let mut alternatives = alternatives;
        if num == Number::Pl {
            alternatives.push(half_of(&posts));
        }
        if num == Number::Sg {
            alternatives.push(next_this_turn(&posts));
            alternatives.push(nth_each_turn(&posts));
        }
        alt(if num == Number::Sg { "selector" } else { "plural selector" }, alternatives)
    };
    let set = |prep: ZonePrep| {
        let other = lit("other").opt();
        let one = |num: Number, union: Union| {
            other
                .then(&core(num, union))
                .then(&post_sequence(prep, num, &player_sg_any, &player_pl, &any_sel.rule()))
                .map(
                    |((o, (domain, items)), posts)| {
                        let mut set = ObjectSet {
                            domain,
                            filter: Filter::All(items),
                            controller: None,
                            owner: None,
                            caster: None,
                            zone: None,
                            other: o.is_some(),
                        };
                        absorb_exiled(&mut set, false)?;
                        match fold_posts(&mut set, posts)? {
                            (None, None) => Some(set),
                            _ => None,
                        }
                    },
                    |set: &ObjectSet| {
                        let (core, posts) = unfold_posts(set, &None, None)?;
                        Some(((set.other.then_some(()), (set.domain.clone(), core)), posts))
                    },
                )
        };
        let coordinated = |num, union| {
            let plain = one(num, union);
            // Migrate bare plurals first. Singular type heads can scope across a union
            // ("instant or sorcery spell"); that surface stays with the existing core
            // until its head/modifier boundary is represented explicitly.
            if num == Number::Sg { return plain; }
            // Different domains require complete arms: a source qualifier belongs to an
            // ability arm (113.7a), not to a neighboring spell. Same-domain noun unions
            // retain their existing shared-head/property grammar.
            let arm = plain.map(
                |s| (!matches!(s.domain, Domain::Union(_))).then_some(s),
                |s| (!matches!(s.domain, Domain::Union(_))).then(|| s.clone()),
            );
            alt("set union", vec![plain.clone(), arm.list("or").map(join_sets, split_sets)])
        };
        (
            coordinated(Number::Sg, Union::Or).named("set"),
            coordinated(Number::Pl, Union::And).named("plural set"),
            coordinated(Number::Sg, Union::And).named("set"),
        )
    };

    let slices = slice(&player_sg_any);
    // not inside `sg`: its possessive would reach back into `sg` at the same position
    let zones = alt(
        "possessed",
        vec![
            whole_zone(&player_sg_any),
            possessed_commander(&player_sg_any),
            whole_zones(&player_pl),
        ],
    );
    let (part_sg, part_pl) = (partitive(Number::Sg, &any_sel.rule()), partitive(Number::Pl, &any_sel.rule()));
    let (set_sg, set_pl, set_each) = set(ZonePrep::In);
    let (set_sg_from, set_pl_from, _) = set(ZonePrep::From);
    let sg = alt("selector", vec![branch(Number::Sg, ZonePrep::In), target_union(&set_sg), slices.clone(), part_sg.clone()]);
    // "your opponents" is how Oracle says all opponents; listed first so it prints that way
    let your_opponents = lit("your opponents").to(your_opponents());
    let pl = alt("plural selector", vec![your_opponents.clone(), branch(Number::Pl, ZonePrep::In), part_pl.clone()]);
    sg_in.define(sg.clone());
    pl_in.define(pl.clone());
    let selector = alt("selector", vec![sg.clone(), pl.clone(), special.clone(), zones.clone(), either.clone()]);
    let predicate = alt("noun predicate", vec![selector.clone(),
        branch(Number::Sg, ZonePrep::In).list("or").map(join_noun_phrases, split_noun_phrases)]);
    let (you, you2) = (Selector::you(), Selector::you());
    let third_person = special.map(move |s| (s != you).then_some(s), move |s| (*s != you2).then(|| s.clone()));
    // "your commander" as a subject: "whenever your commander attacks"
    let singular = alt("singular selector", vec![sg, third_person, either, possessed_commander(&player_sg_any)]);
    any_sel.define(selector.clone());
    // "~ from your graveyard", "it from your hand": a definite card in a zone
    let definite = alt(
        "definite card",
        vec![own_reference(), lit("it").to(Ref::Anaphor(Anaphor::It)), lit("them").to(Ref::Anaphor(Anaphor::Them))],
    );
    let in_zone = definite.then(&zone_spec(ZonePrep::From, &player_sg_any)).map(
        |(r, z)| {
            if z.slice.is_some() {
                return None;
            }
            let mut set = ObjectSet::of(Domain::Card);
            set.zone = Some(z);
            Some(Selector::new(Quant::Definite(r), set))
        },
        |s: &Selector| match (&s.quant, &s.set) {
            (Quant::Definite(r @ (Ref::This | Ref::Granting | Ref::Anaphor(Anaphor::It | Anaphor::Them))), set)
                if *set == ObjectSet { zone: set.zone.clone(), ..ObjectSet::of(Domain::Card) }
                    && set.zone.is_some()
                    && s.chooser.is_none()
                    && s.random.is_none() =>
            {
                Some((r.clone(), set.zone.clone()?))
            }
            _ => None,
        },
    );
    let selector_from = alt(
        "selector",
        vec![
            battlefield_and_graveyards(),
            branch(Number::Sg, ZonePrep::From),
            target_union(&set_sg_from),
            slices,
            part_sg,
            part_pl,
            your_opponents,
            branch(Number::Pl, ZonePrep::From),
            in_zone,
            special,
            zones,
        ],
    );
    Selectors {
        possessive: zone_owner(&player_sg_any),
        singular,
        plural: pl,
        zone: zone_spec(ZonePrep::In, &player_sg_any),
        selector,
        predicate,
        selector_from,
        set_sg,
        set_pl,
        set_pl_from,
        set_each,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn explicit_noun_phrase_alternatives_preserve_each_complete_arm() {
        for text in ["a noncreature spell or a Dragon spell", "an Equipment spell or a spell that targets a creature you control",
            "a Dragon spell or an Omen spell"] {
            let value = rt(&selectors().predicate, text);
            let Domain::Union(arms) = value.set.domain else { panic!("complete noun-phrase alternatives") };
            assert_eq!(arms.len(), 2);
            assert!(arms.iter().all(|a| a.domain == Domain::Spell));
            assert_ne!(arms[0].filter, arms[1].filter);
        }
        let value = rt(&selectors().predicate, "a creature you control or a creature an opponent controls");
        let Domain::Union(arms) = value.set.domain else { panic!("separate controller scopes") };
        assert_ne!(arms[0].controller, arms[1].controller);
        let value = rt(&selectors().predicate, "a creature or a creature you control");
        let Domain::Union(arms) = value.set.domain else { panic!("separate controller scopes") };
        assert!(arms[0].controller.is_none());
        assert!(arms[1].controller.is_some());
        for text in ["two creatures or a land", "a creature of your choice or an artifact", "target creature or a creature"] {
            assert!(declines(&selectors().predicate, text), "{text}");
        }
    }

    #[test]
    fn enclosing_action_coordination_keeps_choices_quantities_and_and() {
        let objects = crate::action::objects(&selectors().selector);
        for (text, conjunction) in [("a creature or an artifact", ir::Conj::Or),
            ("two creatures or a land", ir::Conj::Or), ("a creature and an artifact", ir::Conj::And),
            ("a creature of your choice or an artifact", ir::Conj::Or)] {
            let Outcome::Accepted { value, .. } = objects.parse_all(&tokenize(text)) else { panic!("{text}") };
            assert_eq!(value.conj, conjunction);
            assert_eq!(value.items.len(), 2);
            assert_eq!(objects.print_text(&value, false).as_deref(), Some(text));
        }
    }

    #[test]
    fn r115_1_heterogeneous_target_nouns_keep_each_arms_predicates() {
        let s = &selectors().selector;
        for text in ["target triggered ability or colorless spell", "target colorless spell or triggered ability",
            "target activated ability or blue spell"] {
            let value = rt(s, text);
            let Domain::Union(arms) = value.set.domain else { panic!("separate semantic domains") };
            assert_eq!(arms.len(), 2);
            assert_ne!(arms[0].domain, arms[1].domain);
            assert_ne!(arms[0].filter, arms[1].filter);
        }
        assert!(declines(s, "target triggered player or colorless spell"));
    }

    #[test]
    fn r205_3i_basic_land_type_is_a_postmodifier_not_the_basic_supertype() {
        let s = &selectors().selector;
        rt(s, "a land card with a basic land type");
    }

    fn rt(rule: &P<Selector>, text: &str) -> Selector {
        let toks = tokenize(text);
        match rule.parse_all(&toks) {
            Outcome::Accepted { value, .. } => {
                assert_eq!(rule.print_text(&value, false).as_deref(), Some(text), "{value:#?}");
                value
            }
            other => panic!("{text}: {other:#?}"),
        }
    }

    #[test]
    fn r400_1_coordinated_plural_zones_preserve_ownership_and_conjunction() {
        for (word, conj) in [("and", ir::Conj::And), ("or", ir::Conj::Or), ("and/or", ir::Conj::AndOr)] {
            let text = format!("all creature cards in all graveyards {word} libraries");
            let one = rt(&selectors().plural, &text);
            let zone = one.set.zone.unwrap();
            assert_eq!(one.set.domain, Domain::Card);
            assert_eq!(zone.zones.items, vec![Zone::Graveyard, Zone::Library]);
            assert_eq!(zone.zones.conj, conj);
            assert_eq!(*zone.owner.unwrap(), Selector::new(Quant::Each, ObjectSet::of(Domain::Player)));
        }
        for text in ["all creature cards in graveyards and players", "all creature cards in graveyards libraries"] {
            assert!(!matches!(selectors().plural.parse_all(&tokenize(text)), Outcome::Accepted { .. }));
        }
    }

    #[test]
    fn r607_2d_choice_references_preserve_the_constraint_without_guessing_a_producer() {
        use ir::reference::{Kind, UnresolvedRef};
        for (text, kind) in [("all cards of the chosen type", Kind::Type),
            ("all cards of the chosen creature type", Kind::CreatureType),
            ("all cards of the chosen color", Kind::Color)] {
            let s = rt(&selectors().plural, text);
            assert_eq!(s.set.filter, Filter::All(vec![Filter::Atom(Atom::Reference(UnresolvedRef::choice(kind)))]));
        }
        for bad in ["all cards of chosen the type", "all cards of the chosen", "all cards of the chosen creature color"] {
            assert!(!matches!(selectors().plural.parse_all(&tokenize(bad)), Outcome::Accepted { .. }), "{bad}");
        }
    }

    /// CR 107.1: "at least three" and "three or more" are one lower bound; the printer says
    /// "or more" (the first spelling listed), "at least" only reads.
    #[test]
    fn r107_1_at_least_is_or_more() {
        let toks = tokenize("at least three other creatures");
        let Outcome::Accepted { value, .. } = selectors().plural.parse_all(&toks) else { panic!("must read") };
        assert_eq!(value, rt(&selectors().plural, "three or more other creatures"));
        assert!(!matches!(selectors().singular.parse_all(&tokenize("at least three creature")), Outcome::Accepted { .. }));
    }

    /// CR 722.3c-d: a prepare spell is a spell, distinct from its permanent's
    /// prepared designation. The restriction is an adjective of the spell noun.
    #[test]
    fn r722_3c_prepared_spell_modifier_keeps_its_literal_selector_meaning() {
        let set = ObjectSet { filter: Filter::All(vec![Filter::Atom(Atom::PreparedSpell)]),
            ..ObjectSet::of(Domain::Spell) };
        for (text, quant) in [
            ("a prepared spell", Quant::Indefinite(Count::Exactly(Amount::Const(1)))),
            ("target prepared spell", Quant::Target(Count::Exactly(Amount::Const(1)))),
        ] {
            let Outcome::Accepted { value, .. } = selectors().selector.parse_all(&tokenize(text)) else { panic!("{text}") };
            assert_eq!(value, Selector::new(quant, set.clone()));
            assert_eq!(selectors().selector.print_text(&value, false).as_deref(), Some(text));
        }
        let Outcome::Accepted { value, .. } = selectors().set_pl.parse_all(&tokenize("prepared spells")) else { panic!("plural spell set") };
        assert_eq!(value, set);
        assert_eq!(selectors().set_pl.print_text(&value, false).as_deref(), Some("prepared spells"));
    }

    #[test]
    fn typed_spell_self_reference_keeps_its_stack_spell_domain_when_printed() {
        let selector = Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Spell));
        let rule = &selectors().selector;
        let text = rule.print_text(&selector, false).unwrap();
        assert_eq!(text, "this spell");
        let Outcome::Accepted {value,..} = rule.parse_all(&tokenize(&text)) else {panic!("{text} must have one reading")};
        assert_eq!(value, selector);
    }

    #[test]
    fn common_selectors_round_trip() {
        let s = &selectors().selector;
        for t in [
            "target creature",
            "a card",
            "an opponent",
            "each opponent",
            "target player",
            "enchanted creature",
            "a creature you control",
            "target creature you control",
            "two target creatures",
            "up to one target creature",
            "up to two target creatures",
            "another target creature",
            "each other creature you control",
            "all creatures",
            "all nonland permanents",
            "target nonland permanent you don't control",
            "target artifact or creature",
            "target creature an opponent controls",
            "a creature card in your graveyard",
            "target creature card in a graveyard",
            "target creature with flying",
            "each creature with power 2 or less",
            "a creature with mana value X or less",
            "target noncreature, nonland permanent",
            "target attacking or blocking creature",
            "an artifact or enchantment",
            "any target",
            "one or more creatures you control",
            "any number of target creatures",
            "a black or red permanent",
            "target legendary creature",
            "a basic land card",
            "an untapped creature you control",
            "target creature or planeswalker an opponent controls",
            "target Goblin",
            "an Elf",
            "all Elves you control",
            "each creature your opponents control",
            "two or more creatures",
            "a nonland permanent card",
            "target permanent spell",
            "target creature with a +1/+1 counter on it",
            "each creature with three or more time counters on it",
            "all creatures with two -1/-1 counters on them",
            "the top card of your library",
            "the top three cards of target player's library",
            "a card named ~",
            "a card named §",
            "all cards exiled with §",
            "§'s controller",
            "~'s owner",
            "a creature card named Relentless Rats",
            "a card named Rowan, Fearless Sparkmage",
            "a card named Bogbrew Witch or Festering Newt",
            "a creature card not named ~",
            "a card named God-Pharaoh's Gift",
            "each creature of the chosen type",
            "all cards exiled with ~",
        ] {
            rt(s, t);
        }
    }

    /// CR 201.2: "named A or B" is a filter on the name; "named A and B" asks for one object per
    /// name. The name list is the one coordination (2.6), so every list shape reads and prints.
    #[test]
    fn r201_2_name_lists() {
        let s = &selectors().plural;
        for t in [
            "all artifacts you control named Crown of Empires and Throne of Empires",
            "all creatures you control named Mine Worker, Tower Worker, and Power Plant Worker",
            "all creatures named Wolves of the Hunt",
        ] {
            rt(s, t);
        }
        let one = rt(&selectors().selector, "a card named Heart-Piercer Bow or Vial of Dragonfire");
        let Filter::All(f) = &one.set.filter else { panic!("{one:#?}") };
        assert!(f.iter().any(|f| matches!(f, Filter::Atom(Atom::Named(c)) if c.items.len() == 2 && c.conj == ir::Conj::Or)));
        // a name is not a sentence: a lowercase word after "or" is no name
        assert!(!matches!(
            selectors().selector.parse_all(&tokenize("a card named Festering Newt or draw")),
            Outcome::Accepted { .. }
        ));
    }

    /// CR 608.2c: later text refers to what an earlier instruction did. The reduced relative
    /// "exiled this way" is one construction over the passive of any action, with the same value
    /// as its full passive alias. Argument-free positive results emit typed references; other
    /// event predicates retain their complete event relation.
    #[test]
    fn r608_2c_this_way_is_a_reduced_relative_over_any_passive() {
        let s = &selectors().selector;
        for t in [
            "all cards exiled this way",
            "all cards revealed this way",
            "each creature tapped this way",
            "a creature card put into your hand this way",
            "a creature card put into a graveyard this way",
            "all cards exiled from their hand this way",
            "all permanents put onto the battlefield this way",
            "all cards they exiled this way",
            "the exiled cards that weren't cast this way",
            "all cards revealed this way that weren't put onto the battlefield",
            "a card that wasn't put into your hand",
        ] {
            let one = rt(s, t);
            let Filter::All(filters) = &one.set.filter else { panic!("{one:?}") };
            assert!(filters.iter().any(|f| unevent_filter(f).is_some()), "{t}: {one:#?}");
        }
        // the clause and the negation without "that" say the same
        let a = rt_alias(s, "all creatures that were destroyed this way");
        assert_eq!(a, rt_alias(s, "all creatures destroyed this way"));
        let b = rt_alias(s, "all creatures not chosen this way");
        assert_eq!(b, rt_alias(s, "all creatures that weren't chosen this way"));
        // the reduced relative takes the participle of an action, not any word
        for t in ["all cards sneezed this way", "all cards exiled that way", "all cards exiled this way this way"] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    /// CR 608.2c: a later phrase picks from the group an earlier one named: some of it, any of
    /// it, all of it, or what is left. The leftover is a group's, never a lone object's.
    #[test]
    fn r608_2c_the_partitive_takes_what_remains() {
        let s = &selectors().selector;
        for t in [
            "the rest of the revealed cards",
            "the rest of those cards",
            "any of those cards",
            "any of them",
            "all of them",
            "the others of those cards",
            "the other of those cards",
            "one of them",
        ] {
            rt(s, t);
        }
        for t in ["the rest of target creature", "the rest of it", "any of target creature"] {
            assert!(!matches!(s.parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t}");
        }
    }

    #[test]
    fn postmodifiers_and_relatives_round_trip() {
        let s = &selectors().selector;
        for t in [
            "a card with the same name as that creature",
            "each other creature that shares a color with it",
            "all cards with that name",
            "two creatures that share a creature type",
            "up to four creature cards with different names",
            "your commander",
            "a commander creature you control",
            "each creature blocking it",
            "target creature blocking or blocked by ~",
            "target activated ability",
            "an ability that isn't a mana ability",
            "each creature that isn't a Demon, Devil, or Imp",
            "target creature that's attacking",
            "a spell that targets ~",
            "target spell that targets you",
            "target creature that was dealt damage this turn",
            "each player who lost life this turn",
            "all creatures that didn't attack this turn",
            "one or two target creatures",
            "a kicked spell",
            "a creature with the greatest power among creatures they control",
            "a spell with lesser mana value",
            "target spell or nonland permanent",
            "each opponent who controls more lands than you",
            "the player with the most life or tied for most life",
            "a creature you controlled",
            "all creatures with +1/+1 counters on them",
            "target spell with a single target",
            "a spell with {X} in its mana cost",
            "all creatures of that color",
            "a permanent that's one or more colors",
            "all creatures of the creature type of your choice",
            "any player",
            "the attacking player",
            "a player who controls eight or more lands",
            "each creature that was dealt damage this turn",
            "all cards exiled with it",
            "an Aura card with enchant creature",
            "any target that isn't a Dragon",
            "all cards they exiled this way",
            "a creature spell that has an Adventure",
            "target nonattacking creature",
            "each creature that player or that planeswalker's controller controls",
            "a creature with power or toughness 2 or less",
            "all creatures that aren't of the chosen type",
            "all cards in your opponents' graveyards",
            "the top three cards of your library",
            "the next instant or sorcery spell you cast this turn",
            "exactly one creature",
            "two target creatures controlled by different players",
            "your opponents' graveyards",
            "target permanent you both own and control",
            "each creature you control but don't own",
            "each creature except for creatures you control with flying",
        ] {
            rt(s, t);
        }
    }

    #[test]
    fn structure() {
        let s = rt(&selectors().selector, "up to two target nonland permanents you don't control");
        assert_eq!(s.quant, Quant::Target(Count::UpTo(Amount::Const(2))));
        assert_eq!(s.set.domain, Domain::Permanent);
        assert_eq!(
            s.set.filter,
            Filter::All(vec![Filter::Not(Box::new(Filter::Atom(Atom::Type(ir::CardType::Land))))])
        );
        let c = s.set.controller.unwrap();
        assert!(c.negated);
        assert_eq!(*c.who, Selector::you());
    }

    #[test]
    fn bare_sets() {
        let s = selectors();
        for (set, t) in [
            (&s.set_sg, "creature"),
            (&s.set_pl, "Creatures you control"),
            (&s.set_pl, "Other Elves you control"),
            (&s.set_pl, "Nontoken creatures"),
            // irregular plurals of subtypes
            (&s.set_pl, "Heroes you control"),
            (&s.set_pl, "Other Pegasi"),
        ] {
            let toks = tokenize(t);
            let Outcome::Accepted { value, .. } = set.parse_all(&toks) else { panic!("{t}") };
            assert_eq!(set.print_text(&value, true).as_deref(), Some(if t == "creature" { "Creature" } else { t }));
        }
    }

    /// Coordination is taken over terms, each with its own modifiers (CR 700.2 has no rule; the
    /// noun phrase is a union of descriptions): the value is one union under one head.
    #[test]
    fn term_coordination() {
        let s = &selectors().selector;
        for t in [
            // a head noun repeated: each term complete
            "a basic land card or Gate card",
            "up to two basic land cards and/or Gate cards",
            "a Doctor spell or creature spell",
            // terms of different kinds under one head noun
            "a land or legendary Turtle card",
            "a noncreature or Dragon spell",
            "a blue or artifact card",
            "each Mountain and red card",
            "target artifact, enchantment, or legendary card",
            // no head noun: every term has its own type
            "target artifact, enchantment, or tapped creature",
            "target land or nonblack creature",
            "target multicolored creature or multicolored enchantment",
        ] {
            rt(s, t);
        }
        // the new reading does not change the old ones: leading adjectives distribute over the
        // types ("basic" applies to the Gate), a pure adjective union stays one item
        let basic = rt(s, "a basic land or Gate card");
        let Filter::All(items) = &basic.set.filter else { panic!("{basic:?}") };
        assert_eq!(items.len(), 2);
        let each = rt(s, "a basic land card or Gate card");
        assert_ne!(basic.set.filter, each.set.filter);
        let Filter::All(items) = rt(s, "a white or blue creature").set.filter else { panic!() };
        assert_eq!(items.len(), 2);
        // a plain "and" between the terms of a singular phrase joins properties, not a union
        let toks = tokenize("a white and black Spirit enchantment creature token");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
        // players have no types: "an opponent or Gate card" is not a noun phrase
        let toks = tokenize("a land or legendary Turtle player");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
        // "artifact creature permanent" is not said, so neither is a union before "permanent"
        let toks = tokenize("a blue or artifact permanent");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
    }

    /// CR 613.4b: base power and toughness are characteristics a filter or comparison can ask
    /// about, wherever power and toughness can be asked.
    #[test]
    fn r613_4b_base_power_is_a_stat() {
        let s = &selectors().selector;
        let sel = rt(s, "a creature with base power 2 or less");
        let Filter::All(items) = &sel.set.filter else { panic!() };
        assert!(items.iter().any(|f| matches!(f, Filter::Atom(Atom::Stat { stat: Stat::BasePower, .. }))));
        rt(s, "a creature with base toughness 3");
        rt(s, "a creature with power greater than its base power");
        // "base" alone is not a stat
        let toks = tokenize("a creature with base 2");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
    }

    /// CR 113.3, 702, 113.7a: abilities as things a noun phrase names.
    #[test]
    fn r113_abilities_in_noun_phrases() {
        let s = &selectors().selector;
        for t in [
            "each creature you control with a mana ability",
            "a creature without a mana ability",
            "an exhaust ability",
            "an eternalize or embalm ability",
            "target activated ability from an artifact",
            "target activated or triggered ability you control from another legendary permanent",
        ] {
            rt(s, t);
        }
        // "a cycling ability" is the keyword, as "cycling" is; it reads and prints as the keyword
        let said = rt_alias(s, "a card with a cycling ability");
        assert_eq!(said, rt(s, "a card with cycling"));
        // a keyword is the ability's before "ability" only, a source is an ability's only
        let toks = tokenize("a creature with a mana ability from an artifact");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
        let toks = tokenize("an ability with flying");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
        let toks = tokenize("a flying creature");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
    }

    fn rt_alias(rule: &P<Selector>, text: &str) -> Selector {
        match rule.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => value,
            other => panic!("{text}: {other:#?}"),
        }
    }

    /// CR 108.3 (owner) and 110.2 (controller) are two relations of one object; either can be
    /// negated, and one subject can hold both.
    #[test]
    fn r108_3_owner_and_controller() {
        let s = &selectors().selector;
        for t in [
            "a permanent you both own and control",
            "a permanent you control but don't own",
            "a permanent you own but don't control",
            "a creature an opponent controls but doesn't own",
            "each land they own that you control",
        ] {
            rt(s, t);
        }
        let both = rt(s, "a permanent you own but don't control");
        assert!(both.set.controller.as_ref().is_some_and(|r| r.negated));
        assert!(both.set.owner.as_ref().is_some_and(|r| !r.negated));
        // neither holding is no relation at all
        let toks = tokenize("a permanent you don't own and don't control");
        assert!(!matches!(s.parse_all(&toks), Outcome::Accepted { .. }));
    }

    #[test]
    fn caster() {
        let toks = tokenize("Instant and sorcery spells you cast");
        let Outcome::Accepted { value, .. } = selectors().set_pl.parse_all(&toks) else { panic!() };
        assert_eq!(*value.caster.unwrap().by.who, Selector::you());
    }

    #[test]
    fn from_zone() {
        rt(&selectors().selector_from, "target creature card from your graveyard");
        rt(&selectors().selector_from, "a card from exile");
        rt(&selectors().selector_from, "up to two target cards from a single graveyard");
        // one determiner over the permanents and the cards in graveyards of one description
        let s = rt(&selectors().selector_from, "up to three other target creatures from the battlefield and/or creature cards from graveyards");
        assert_eq!(s.set.domain, Domain::Object);
        assert!(s.set.other);
    }

    fn declines(rule: &P<Selector>, text: &str) -> bool {
        !matches!(rule.parse_all(&tokenize(text)), Outcome::Accepted { .. })
    }

    /// CR 702.55b, 702.82b, 702.122c: what a keyword's action was done to, said with the doer
    /// under "by" or as an active clause (an alias of it); 702.51c: the subject of "convoked".
    #[test]
    fn r702_keyword_relations_in_noun_phrases() {
        let s = &selectors().selector;
        for t in ["a Vehicle crewed by ~", "the creature haunted by it", "each creature devoured by ~", "each creature that convoked it"] {
            rt(s, t);
        }
        let by = rt(s, "the creature haunted by ~");
        // the active clause is the same reading as the passive
        assert_eq!(rt_alias(s, "the creature ~ haunts"), by);
        assert_eq!(rt_alias(s, "each creature it devoured"), rt(s, "each creature devoured by it"));
        // the doer under "by" is in the object case
        assert_eq!(rt_alias(s, "the creature they chose"), rt(s, "the creature chosen by them"));
        assert!(declines(s, "the creature chosen by they"));
        assert!(declines(s, "the creature them chose"));
        // a player does not haunt
        assert!(declines(s, "the creature you haunts"));
        assert!(declines(s, "a creature you haunted"));
        // a window is the windowed relative's: another value
        assert_ne!(rt(s, "each creature that convoked it this turn"), rt(s, "each creature that convoked it"));
    }

    /// "Another of your opponents" is one of the group other than the one the text is about,
    /// as "another" before a noun is.
    #[test]
    fn another_of_a_group() {
        let s = &selectors().selector;
        let other = rt(s, "another of your opponents");
        assert!(other.set.other);
        assert!(!rt(s, "one of your opponents").set.other);
        assert_eq!(rt_alias(s, "another one of your opponents"), other);
        assert!(declines(s, "another of"));
    }

    /// CR 208.1: "2/3 means the object has power 2 and toughness 3". Before a noun it is a
    /// predicate of the thing, the same two stat predicates "with power 2" would say.
    #[test]
    fn r208_1_power_and_toughness_before_the_noun() {
        let s = &selectors().selector;
        let eq = |stat, n| Filter::Atom(Atom::Stat { stat, cmp: Cmp::Eq, value: Amount::Const(n) });
        let one_one = Filter::All(vec![eq(Stat::Power, 1), eq(Stat::Toughness, 1)]);
        let target = rt(s, "target 1/1 creature");
        assert_eq!(target.set.filter, Filter::All(vec![one_one.clone(), Filter::Atom(Atom::Type(ir::CardType::Creature))]));
        for t in ["a 1/1 creature", "each 1/1 creature you control", "a 2/3 white creature", "target 0/1 creature token"] {
            rt(s, t);
        }
        let two_three = rt(s, "a 2/3 creature");
        assert!(matches!(&two_three.set.filter, Filter::All(f) if f[0] == Filter::All(vec![eq(Stat::Power, 2), eq(Stat::Toughness, 3)])));
        // a pair is two numbers; one number alone, or a pair after the noun, is no predicate
        for t in ["target 1/ creature", "target /1 creature", "target 1 creature", "target creature 1/1", "target 1/1"] {
            assert!(declines(s, t), "{t}");
        }
        // it is said once, and of something that can have power and toughness (208.3)
        for t in ["target 1/1, 2/2 creature", "target 1/1 1/1 creature", "target 1/1 or 2/2 creature", "target 1/1 activated ability"] {
            assert!(declines(s, t), "{t}");
        }
        // only a creature has them: in a list of types the pair after "with" is the creature's
        let listed = rt(s, "target artifact, enchantment, or creature with base power and toughness 2/2");
        let base = Filter::All(vec![eq(Stat::BasePower, 2), eq(Stat::BaseToughness, 2)]);
        let creature = Filter::All(vec![Filter::Atom(Atom::Type(ir::CardType::Creature)), base]);
        assert!(
            matches!(&listed.set.filter, Filter::All(items) if matches!(items.as_slice(), [Filter::Any(arms)] if arms.len() == 3 && arms[2] == creature)),
            "{:?}",
            listed.set.filter
        );
    }

    /// CR 208.4b: the base power and toughness, both exactly, said after the noun.
    #[test]
    fn r208_4b_base_power_and_toughness_after_the_noun() {
        let s = &selectors().selector;
        let eq = |stat, n| Filter::Atom(Atom::Stat { stat, cmp: Cmp::Eq, value: Amount::Const(n) });
        let two_two = rt(s, "a creature with base power and toughness 2/2");
        assert!(matches!(&two_two.set.filter, Filter::All(f) if f.contains(&Filter::All(vec![eq(Stat::BasePower, 2), eq(Stat::BaseToughness, 2)]))));
        rt(s, "each creature you control with base power and toughness 1/1");
        for t in ["a creature with base power and toughness 2", "a creature with base power and toughness", "a creature with power and toughness 2/2"] {
            assert!(declines(s, t), "{t}");
        }
    }

    /// Several stats under one comparison: any of them satisfies it. "base" said once is said
    /// of each (CR 208.4), never the base of one and the current value of the other.
    #[test]
    fn r208_several_stats_under_one_comparison() {
        let s = &selectors().selector;
        let stats = |sel: &Selector| match &sel.set.filter {
            Filter::All(items) => items.iter().find_map(|f| match f {
                Filter::Any(v) => v.iter().map(|x| match x {
                    Filter::Atom(Atom::Stat { stat, .. }) => Some(*stat),
                    _ => None,
                }).collect::<Option<Vec<Stat>>>(),
                _ => None,
            }),
            _ => None,
        };
        assert_eq!(stats(&rt(s, "target creature with power or toughness 2 or less")), Some(vec![Stat::Power, Stat::Toughness]));
        assert_eq!(stats(&rt(s, "each creature with base power or toughness 1 or less")), Some(vec![Stat::BasePower, Stat::BaseToughness]));
        assert_eq!(
            stats(&rt(s, "a spell with mana value, power, or toughness equal to the chosen number")),
            Some(vec![Stat::ManaValue, Stat::Power, Stat::Toughness])
        );
        // one stat is no list; a base stat is named with "base" once; a list needs its comparison
        for t in [
            "a creature with base power or base toughness 1",
            "a creature with power or base toughness 1",
            "a creature with power or toughness",
            "a creature with power or 2",
            "a creature with power or power 2 or less",
            "a creature with base power or power 1",
        ] {
            assert!(declines(s, t), "{t}");
        }
    }

    /// CR 208.1 and the ruling of 2022-09-09 on Cut Down: a thing's total power and toughness
    /// is the sum of the two, one number of one object. Not the total of one stat over the
    /// objects selected together ("with total power 6 or greater").
    #[test]
    fn r208_1_total_power_and_toughness_is_one_things_sum() {
        let s = &selectors().selector;
        let one = rt(s, "target creature with total power and toughness 5 or less");
        assert!(matches!(&one.set.filter, Filter::All(f)
            if f.contains(&Filter::Atom(Atom::Stat { stat: Stat::PowerAndToughness, cmp: Cmp::Le, value: Amount::Const(5) }))));
        rt(s, "a creature card with total power and toughness 5 or less");
        for t in ["target creature with total toughness and power 5 or less", "target creature with total power and 5 or less"] {
            assert!(declines(s, t), "{t}");
        }
        // the sum is one object's: "total" over a group takes one stat
        rt(&selectors().plural, "any number of untapped creatures you control with total power 6 or greater");
        for t in [
            "any number of untapped creatures you control with total total power and toughness 6 or greater",
            "target creature with power or total power and toughness 5 or less",
        ] {
            assert!(declines(&selectors().plural, t) && declines(s, t), "{t}");
        }
        assert!(matches!(crate::amount::amounts().amount.parse_all(&oracle::token::tokenize("the total total power and toughness of creatures you control")), crate::kernel::Outcome::Declined { .. }));
    }

    /// CR 700.12: outlaw is the status of an object with one of five creature types, a noun as
    /// "historic" (700.6) and "modified" (700.9) are adjectives.
    #[test]
    fn r700_12_outlaw() {
        let s = &selectors().selector;
        for t in ["an outlaw you control", "target non-outlaw creature", "each outlaw creature card in your graveyard", "another outlaw"] {
            rt(s, t);
        }
        rt(&selectors().plural, "two outlaws you control");
        let one = rt(s, "an outlaw");
        assert!(matches!(&one.set.filter, Filter::All(f) if f == &[Filter::Atom(Atom::Status(Status::Outlaw))]));
        assert!(declines(s, "an outlaw player"));
    }

    /// CR 702.62b: a suspended card is in exile, has suspend and has a time counter.
    #[test]
    fn r702_62b_suspended_card() {
        let s = &selectors().selector;
        rt(s, "each suspended card you own");
        rt(s, "target suspended card");
        assert!(declines(s, "target suspended"));
    }

    /// CR 122.1b: a keyword counter is named by its keyword, of one or two words; "two or fewer"
    /// counters is at most two.
    #[test]
    fn r122_1b_keyword_counters_and_or_fewer() {
        let s = &selectors().selector;
        for t in ["a creature with a first strike counter on it", "a creature with a double strike counter on it", "a creature with three or fewer time counters on it"] {
            rt(s, t);
        }
        let one = rt(s, "a creature with two or fewer oil counters on it");
        let Filter::All(f) = &one.set.filter else { panic!("{one:#?}") };
        assert!(f.iter().any(|f| matches!(f, Filter::Atom(Atom::Counters { count: Count::UpTo(_), .. }))));
        assert!(declines(s, "a creature with two or fewer counters"));
    }

    /// 715.1, 2.6 "and/or": predicates of a card joined as a union, the copula said once
    /// before the nouns and the verb phrases after them.
    #[test]
    fn r715_card_predicates_join_as_a_union() {
        let amount = &crate::amount::amounts().amount;
        let read = |text: &str| match amount.parse_all(&tokenize(text)) {
            Outcome::Accepted { value, .. } => {
                assert_eq!(amount.print_text(&value, false).as_deref(), Some(text), "{value:#?}");
                Some(value)
            }
            _ => None,
        };
        let hearth = read("the number of cards in your graveyard that are instant cards, sorcery cards, and/or have an Adventure").expect("reads");
        let Amount::Count(s) = &hearth else { panic!("{hearth:#?}") };
        let Filter::All(items) = &s.filter else { panic!("{s:#?}") };
        assert!(items.contains(&Filter::Any(vec![
            Filter::Atom(Atom::Type(ir::CardType::Instant)),
            Filter::Atom(Atom::Type(ir::CardType::Sorcery)),
            Filter::Atom(Atom::HasAdventure),
        ])), "{items:#?}");
        // the copula said again before a later noun: the same predicates, printed with one
        let again = amount.parse_all(&tokenize("the number of cards in your graveyard that are instant cards, are sorcery cards, and/or have an Adventure"));
        let Outcome::Accepted { value, .. } = again else { panic!("the repeated copula must read: {again:?}") };
        assert_eq!(value, hearth);
        // two predicates, and another verb phrase: the same structure
        for text in [
            "the number of cards in your graveyard that are creature cards and/or have an Adventure",
            "the number of cards in your graveyard that are instant cards and/or have flying",
        ] {
            assert!(read(text).is_some(), "{text}");
        }
        // what is not the structure: a verb phrase first, a noun after a verb phrase, the copula
        // again, a noun of something other than cards, a verb that does not agree with "cards"
        for text in [
            "the number of cards in your graveyard that are have an Adventure, instant cards",
            "the number of cards in your graveyard that are instant cards, have an Adventure, and/or sorcery cards",
            "the number of cards in your graveyard that are instant cards and/or are have an Adventure",
            "the number of cards in your graveyard that are creatures and/or have an Adventure",
            "the number of cards in your graveyard that are instant cards, sorcery cards, and/or has an Adventure",
        ] {
            assert!(read(text).is_none(), "{text} must not read");
        }
        // the whole sentence of the card, normalized as the corpus is ("This spell" is ~)
        let printed = "This spell costs {X} less to cast, where X is the number of cards in your graveyard that are instant cards, sorcery cards, and/or have an Adventure.";
        let normalized = oracle::normalize::normalize(printed, "Hearth Elemental", "Hearth Elemental", "Creature — Elemental");
        let [text] = normalized.rows.as_slice() else { panic!("one row: {:?}", normalized.rows) };
        let text = text.clone();
        let ability = &crate::ability::abilities().ability;
        let outcome = ability.parse_all(&tokenize(&text));
        let Outcome::Accepted { value, .. } = outcome else { panic!("Hearth Elemental's ability must read: {text}: {outcome:?}") };
        assert_eq!(ability.print_text(&value, true).as_deref(), Some(text.as_str()));
    }
}
