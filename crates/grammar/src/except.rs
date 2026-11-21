//! Exceptions to a copy (CR 707.9): ", except it's an artifact in addition to its other types and
//! it has haste".
//!
//! ```text
//! except    := ", except" exception ("and" item | ", " item ", and" item)
//! item      := exception | change | copy ("has" | "have") granted "and this ability"
//! exception := copy change | "its name is ~"
//! change    := ("'s" | "is" | "'re" | "are") becoming | ("isn't" | "aren't") "legendary"
//!            | ("has" | "have") (granted | "this ability") | ("loses" | "lose") lost
//! copy      := "it" | "the copy" | "the token" | "they" | "the tokens"
//! ```
//!
//! A change without its copy is about the copy the item before named, and agrees with it in
//! number; it prints with the copy named.

use crate::becomes::becoming;
use crate::kernel::{P, alt, leaf, lit};
use crate::token::pt;
use crate::words::Number;
use ir::{Becoming, CopyChange, CopyRef, Exception, Modifier};
use oracle::token::Kind;

fn clitic(c: &'static str) -> P<()> {
    leaf(c, move |k| matches!(k, Kind::Clitic(x) if x == c).then_some(()), move |()| Some(Kind::Clitic(c.into())))
}

fn number(r: CopyRef) -> Number {
    if matches!(r, CopyRef::They | CopyRef::TheTokens) { Number::Pl } else { Number::Sg }
}

/// The exceptions of one copy reference, with its verbs.
fn of(r: CopyRef) -> P<Exception> {
    // the subject with "is": "it's", "they're" (one word), "the copy is"
    let (is, isnt, has, loses): (P<()>, P<()>, &str, &str) = match r {
        CopyRef::It => (lit("it").skip(&clitic("'s").tight()), alt("isn't", vec![lit("it isn't"), lit("it").skip(&clitic("'s").tight()).skip(&lit("not")).alias()]), "it has", "it loses"),
        CopyRef::TheCopy => (lit("the copy is"), lit("the copy isn't"), "the copy has", "the copy loses"),
        CopyRef::TheToken => (lit("the token is"), lit("the token isn't"), "the token has", "the token loses"),
        CopyRef::They => (lit("they're"), alt("aren't", vec![lit("they aren't"), lit("they're not").alias()]), "they have", "they lose"),
        CopyRef::TheTokens => (lit("the tokens are"), lit("the tokens aren't"), "the tokens have", "the tokens lose"),
    };
    let mut changes = changes(number(r), is, isnt, has, loses);
    if r == CopyRef::It {
        let self_ref = leaf("~", |k| matches!(k, Kind::SelfRef).then_some(()), |()| Some(Kind::SelfRef));
        changes.push(lit("its name is").skip(&self_ref).to(CopyChange::NamedThis));
        // "it enters with a shield counter on it if you control that creature"
        let condition = crate::kernel::lazy("condition", || crate::condition::conditions().condition.clone());
        changes.push(
            lit("it enters with")
                .then_keep(&crate::selector::some_counters())
                .skip(&lit("on it"))
                .then(&lit("if").then_keep(&condition).opt())
                .map(
                    |(counters, if_)| Some(CopyChange::EntersWith { counters, if_: if_.map(Box::new) }),
                    |c| match c {
                        CopyChange::EntersWith { counters, if_ } => Some((counters.clone(), if_.as_deref().cloned())),
                        _ => None,
                    },
                ),
        );
    }
    alt("exception", changes).map(
        move |change| Some(Exception { subject: r, change }),
        move |e: &Exception| (e.subject == r).then(|| e.change.clone()),
    )
}

/// After an exception, the next one may leave out the copy it is about ("it isn't legendary and
/// is a Mutant", "it's 1/1 and has toxic 1"): the same copy, its verb agreeing with it.
fn elided(n: Number) -> P<CopyChange> {
    let (is, isnt, has, loses) = match n {
        Number::Sg => (lit("is"), lit("isn't"), "has", "loses"),
        Number::Pl => (lit("are"), lit("aren't"), "have", "lose"),
    };
    alt("exception", changes(n, is, isnt, has, loses))
}

/// What the copy is, has or loses, after its verbs (with the subject or without).
fn changes(n: Number, is: P<()>, isnt: P<()>, has: &str, loses: &str) -> Vec<P<CopyChange>> {
    let lost = lit(loses).then_keep(&crate::statics::lost()).map(
        |l| Some(CopyChange::Loses(l)),
        |c| match c {
            CopyChange::Loses(l) => Some(l.clone()),
            _ => None,
        },
    );
    let pt_only = pt().map(
        |(p, t)| Some(Becoming::Pt(p, t)),
        |b| match b {
            Becoming::Pt(p, t) => Some((p.clone(), t.clone())),
            _ => None,
        },
    );
    let becomes = is.then_keep(&alt("becoming", vec![pt_only, becoming(&[n])])).map(
        |b| Some(CopyChange::Is(b)),
        |c| match c {
            CopyChange::Is(b) => Some(b.clone()),
            _ => None,
        },
    );
    let not_legendary = isnt.skip(&lit("legendary")).to(CopyChange::NotLegendary);
    let granted = lit(has).then_keep(&crate::action::granted()).map(
        |g| Some(CopyChange::Has(g)),
        |c| match c {
            CopyChange::Has(g) => Some(g.clone()),
            _ => None,
        },
    );
    let this_ability = lit(has).skip(&lit("this ability")).to(CopyChange::HasThisAbility);
    vec![becomes, not_legendary, granted, this_ability, lost]
}

/// One item of the list of exceptions as it is read: with its copy named, or without
/// ([`elided`]), or "it has flying and this ability" (one "has" for both, `Shared`).
#[derive(Clone, PartialEq)]
enum Item {
    Full(Exception),
    Elided(Number, CopyChange),
    Shared(CopyRef, Vec<ir::Granted>),
}

/// The exceptions an item list says; the copy of an item that does not name it is the one the
/// item before named, and the first item names it.
fn said(items: Vec<Item>) -> Option<Vec<Exception>> {
    let mut out: Vec<Exception> = Vec::new();
    for item in items {
        match item {
            Item::Full(e) => out.push(e),
            Item::Elided(n, change) => {
                let subject = out.last()?.subject;
                if number(subject) != n {
                    return None;
                }
                out.push(Exception { subject, change });
            }
            Item::Shared(subject, granted) => {
                out.push(Exception { subject, change: CopyChange::Has(granted) });
                out.push(Exception { subject, change: CopyChange::HasThisAbility });
            }
        }
    }
    Some(out)
}

/// ", except …" after a copy action. Each exception prints with its copy named ("except it
/// isn't legendary and it is a Mutant"); the shorter spellings are read as the same exceptions
/// (707.9a, 707.9b).
pub fn except() -> P<Modifier> {
    let refs = [CopyRef::It, CopyRef::TheCopy, CopyRef::TheToken, CopyRef::They, CopyRef::TheTokens];
    let one = alt("exception", refs.into_iter().map(of).collect());
    let single = one.map(|e| Some(vec![e]), |v: &Vec<Exception>| (v.len() == 1).then(|| v[0].clone()));
    let full = one.map(|e| Some(Item::Full(e)), |i| match i {
        Item::Full(e) => Some(e.clone()),
        _ => None,
    });
    let elide = |n: Number| {
        elided(n).map(move |c| Some(Item::Elided(n, c)), |_: &Item| None).alias()
    };
    // "it has flying and this ability": "this ability" shares the "has" (707.9a)
    let shared = alt(
        "shared has",
        refs.into_iter()
            .map(|r| {
                let has = match number(r) {
                    Number::Sg => "has",
                    Number::Pl => "have",
                };
                let subject = match r {
                    CopyRef::It => "it",
                    CopyRef::TheCopy => "the copy",
                    CopyRef::TheToken => "the token",
                    CopyRef::They => "they",
                    CopyRef::TheTokens => "the tokens",
                };
                lit(subject)
                    .skip(&lit(has))
                    .then_keep(&crate::action::granted())
                    .skip(&lit("and this ability"))
                    .map(move |g| Some(Item::Shared(r, g)), |_: &Item| None)
                    .alias()
            })
            .collect(),
    );
    let shared_alone = shared.map(|i| Some(vec![i]), |_: &Vec<Item>| None);
    let item = alt("exception", vec![full, elide(Number::Sg), elide(Number::Pl), shared]);
    let items = alt("exception items", vec![item.list("and"), shared_alone]).map(said, |v: &Vec<Exception>| {
        (v.len() >= 2).then(|| v.iter().cloned().map(Item::Full).collect())
    });
    lit(", except").then_keep(&alt("exceptions", vec![single, items])).map(
        |v| Some(Modifier::Except(v)),
        |m| match m {
            Modifier::Except(v) => Some(v.clone()),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn exceptions_round_trip() {
        let e = except();
        for t in [
            ", except it has this ability",
            ", except it isn't legendary",
            ", except it's an artifact in addition to its other types",
            ", except the copy isn't legendary",
            ", except it's a 1/1 black Zombie",
            ", except it's 4/3, it's a Vehicle artifact in addition to its other types, and it has flying",
            ", except its name is ~ and it has this ability",
            ", except it's an artifact and it loses all other card types",
            ", except it's legendary and snow in addition to its other types",
        ] {
            let toks = tokenize(t);
            let Outcome::Accepted { value, .. } = e.parse_all(&toks) else { panic!("{t}: {:?}", e.parse_all(&toks)) };
            assert_eq!(e.print_text(&value, false).as_deref(), Some(t));
        }
    }

    /// The exception read from `short` is the one `full` says, and prints as `full`.
    fn same(short: &str, full: &str) {
        let e = except();
        let read = |t: &str| match e.parse_all(&tokenize(t)) {
            Outcome::Accepted { value, .. } => value,
            other => panic!("{t}: {other:?}"),
        };
        let value = read(short);
        assert_eq!(value, read(full), "{short}");
        assert_eq!(e.print_text(&value, false).as_deref(), Some(full));
    }

    fn rejected(t: &str) {
        assert!(!matches!(except().parse_all(&tokenize(t)), Outcome::Accepted { .. }), "{t} must not read");
    }

    /// CR 707.9b: a later exception about the same copy may leave the copy unnamed; its verb
    /// still agrees with the copy, and the first exception names it.
    #[test]
    fn r707_9b_a_later_exception_leaves_out_the_copy() {
        same(
            ", except it isn't legendary and is a Mutant in addition to its other types",
            ", except it isn't legendary and it's a Mutant in addition to its other types",
        );
        same(
            ", except it isn't legendary, is a Reflection in addition to its other types, and has haste",
            ", except it isn't legendary, it's a Reflection in addition to its other types, and it has haste",
        );
        same(", except it's an enchantment and loses all other card types", ", except it's an enchantment and it loses all other card types");
        same(", except they're 1/1 and have haste", ", except they're 1/1 and they have haste");
        same(", except the copy isn't legendary and has haste", ", except the copy isn't legendary and the copy has haste");
        rejected(", except isn't legendary");
        rejected(", except is a Mutant in addition to its other types and it has haste");
        rejected(", except they're 1/1 and has haste");
        rejected(", except it's 1/1 and have haste");
    }

    /// CR 707.9a: "this ability" may share the "has" of the abilities before it.
    #[test]
    fn r707_9a_this_ability_shares_the_verb() {
        same(", except it has flying and this ability", ", except it has flying and it has this ability");
        same(", except it's 0/0 and has this ability", ", except it's 0/0 and it has this ability");
        same(
            ", except its name is ~ and it has vigilance and this ability",
            ", except its name is ~, it has vigilance, and it has this ability",
        );
        same(", except they have haste and this ability", ", except they have haste and they have this ability");
        rejected(", except this ability");
        rejected(", except it has flying and this ability and this ability");
        rejected(", except they has haste and this ability");
    }
}
