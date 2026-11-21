//! Costs: activation costs (CR 602.1) and loyalty costs (606.4).
//!
//! ```text
//! cost    := loyalty | part ("," Part)*          "{2}, {T}, Sacrifice a creature"
//! part    := mana | "{T}" | "{Q}" | action(base) | action(base) for_each
//!          | "pay" payment (("and" | "or") payment)+ | action(base) (("and" | "or") action(base))+
//! loyalty := ("+" | "−") (N | "X") | "0"            bracketed inside quotes: "[+1]"
//! ```
//!
//! Parts after the first are capitalized ([`P::capital`]).

use crate::action::action;
use crate::amount::amounts;
use crate::event::Form;
use crate::kernel::{P, alt, lazy, leaf, lit};
use crate::mana::mana_cost;
use crate::selector::is_plural;
use crate::words::digits;
use ir::{Amount, Cost, CostExpr, CostKind, CostPart, CostRef, Loyalty};
use oracle::token::Kind;
use std::sync::OnceLock;

pub struct Costs {
    /// A cost as printed on the card: loyalty costs bare ("+1").
    pub cost: P<Cost>,
    /// A cost inside a granted ability's quotes: loyalty costs bracketed ("[+1]"), as Oracle
    /// writes all 78 of them (and none of the 986 printed ones).
    pub granted_cost: P<Cost>,
}

pub fn costs() -> &'static Costs {
    static C: OnceLock<Costs> = OnceLock::new();
    C.get_or_init(build)
}

fn symbol(s: &'static str) -> P<()> {
    leaf(s, move |k| matches!(k, Kind::Symbol(x) if x == s).then_some(()), move |()| Some(Kind::Symbol(s.into())))
}

/// A loyalty cost, bare or `[bracketed]`.
fn loyalty(bracketed: bool) -> P<Loyalty> {
    let n = alt(
        "N",
        vec![
            digits().map(
                |n| (n > 0).then_some(Amount::Const(n)),
                |a| match a {
                    Amount::Const(n) => Some(*n),
                    _ => None,
                },
            ),
            lit("X").to(Amount::X),
        ],
    );
    let plus = lit("+").then_keep(&n.clone().tight()).map(
        |a| Some(Loyalty::Plus(a)),
        |l| match l {
            Loyalty::Plus(a) => Some(a.clone()),
            _ => None,
        },
    );
    let minus = alt("minus", vec![lit("−"), lit("-").alias()]).then_keep(&n.tight()).map(
        |a| Some(Loyalty::Minus(a)),
        |l| match l {
            Loyalty::Minus(a) => Some(a.clone()),
            _ => None,
        },
    );
    let zero = digits().map(|n| (n == 0).then_some(Loyalty::Zero), |l| (*l == Loyalty::Zero).then_some(0));
    let bare = alt("loyalty", vec![plus, minus, zero]);
    if bracketed { lit("[").then_keep(&bare.tight()).skip(&lit("]").tight()) } else { bare }
}

/// A cost an object has, named as a noun: its mana cost (202.1) or the cost of one of its
/// keyword abilities (702): "its mana cost", "~'s echo cost", "enchanted creature's mana cost".
/// "cost" or "costs" agrees with the possessor ("their mana costs"). Only a card, a spell or a
/// permanent has one (202.1, 702.1); a player or an ability has none.
pub(crate) fn cost_ref() -> P<CostRef> {
    let keyword = crate::selector::keyword_name().map(
        |k| k.has_cost().then_some(CostKind::Keyword(k)),
        |c| match c {
            CostKind::Keyword(k) if k.has_cost() => Some(*k),
            _ => None,
        },
    );
    let kind = alt("cost kind", vec![lit("mana").to(CostKind::Mana), keyword]);
    let noun = alt("cost", vec![lit("cost").to(false), lit("costs").to(true)]);
    use ir::lexicon::Sort;
    let object = |s: &ir::Selector| ir::lexicon::of_kind(s, &[Sort::Card, Sort::Spell, Sort::Permanent]);
    crate::amount::owner_of().then(&kind).then(&noun).map(
        move |((of, which), plural)| (object(&of) && is_plural(&of) == plural).then_some(CostRef { which, of }),
        move |r| object(&r.of).then(|| ((r.of.clone(), r.which), is_plural(&r.of))),
    )
}

/// A cost to pay that is named rather than written in symbols, or that no symbols can hold:
/// "its mana cost", "its mana cost reduced by {2}" (118.7), "{2} plus an additional {1} for each
/// Faerie you control".
pub(crate) fn cost_expr() -> P<CostExpr> {
    let of = cost_ref().map(
        |r| Some(CostExpr::Of(r)),
        |e| match e {
            CostExpr::Of(r) => Some(r.clone()),
            _ => None,
        },
    );
    let reduced = cost_ref().skip(&lit("reduced by")).then(&mana_cost()).map(
        |(r, m)| Some(CostExpr::Reduced(Box::new(CostExpr::Of(r)), m)),
        |e| match e {
            CostExpr::Reduced(base, m) => match base.as_ref() {
                CostExpr::Of(r) => Some((r.clone(), m.clone())),
                _ => None,
            },
            _ => None,
        },
    );
    // "{2} plus an additional {1} for each Faerie you control": what "plus an additional N for
    // each" says of a number (`Plus(N, Multiple(M, count))`), said of mana, which no number of
    // symbols can hold
    let mana = mana_cost().map(
        |m| Some(CostExpr::Mana(m)),
        |e| match e {
            CostExpr::Mana(m) => Some(m.clone()),
            _ => None,
        },
    );
    let base = alt("cost", vec![mana, of.clone()]);
    let additional = base
        .skip(&lit("plus an additional"))
        .then(&mana_cost())
        .then(&lazy("for each", || amounts().for_each.clone()))
        .map(
            |((base, m), n)| Some(CostExpr::Plus(Box::new(base), Box::new(CostExpr::Each(Box::new(CostExpr::Mana(m)), n)))),
            |e| match e {
                CostExpr::Plus(base, more) => match more.as_ref() {
                    CostExpr::Each(unit, n) => match unit.as_ref() {
                        CostExpr::Mana(m) => Some((((**base).clone(), m.clone()), n.clone())),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
        );
    alt("named cost", vec![of, reduced, additional])
}

/// Does the action target (115.1)? Any noun phrase in it, however deep, with "target" (115.1)
/// or "any target" (115.4). The IR has no walker over every action's noun phrases, so this
/// looks at the serialized form, where each [`ir::Selector`] is an object with `quant` and
/// `set.domain`.
fn targets(a: &ir::Action) -> bool {
    fn walk(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                let quant = m.get("quant").is_some_and(|q| q.get("Target").is_some());
                let domain = m.get("domain").is_some_and(|d| d == "AnyTarget");
                quant || domain || m.values().any(walk)
            }
            serde_json::Value::Array(v) => v.iter().any(walk),
            _ => false,
        }
    }
    serde_json::to_value(a).is_ok_and(|v| walk(&v))
}

fn build() -> Costs {
    // a cost is paid by a player (118.1): only what a player does, and nothing targeted. Only a
    // spell or ability targets, by the "target" in its text, and its targets are chosen before
    // its costs are paid (115.1, 601.2c, 601.2h); a cost chooses among objects ("Sacrifice a
    // creature") as it is paid. Facing a villainous choice is an instruction whose options need
    // not be possible (701.55b), never a payment
    let payable =
        |a: &ir::Action| crate::action::does(None, a) && !targets(a) && !matches!(a, ir::Action::FaceVillainousChoice(_));
    let act = action(Form::Base).map(move |a| payable(&a).then_some(a), move |a| payable(a).then(|| a.clone()));
    let collective = |a: &ir::Action| match a {
        ir::Action::Exile(objects) | ir::Action::Discard(objects) | ir::Action::Sacrifice(objects) | ir::Action::Tap(objects) =>
            objects.items.len() == 1 && is_plural(&objects.items[0]),
        _ => false,
    };
    let part = alt(
        "cost part",
        vec![
            mana_cost().map(
                |m| Some(CostPart::Mana(m)),
                |p| match p {
                    CostPart::Mana(m) => Some(m.clone()),
                    _ => None,
                },
            ),
            symbol("T").to(CostPart::Tap),
            symbol("Q").to(CostPart::Untap),
            act.map(
                |a| Some(CostPart::Action(a)),
                |p| match p {
                    CostPart::Action(a) => Some(a.clone()),
                    _ => None,
                },
            ),
            act.skip(&lit(", where X is")).then(&amounts().amount).map(
                |(action, amount)| Some(CostPart::DefinedX(action, amount)),
                |p| match p { CostPart::DefinedX(action, amount) => Some((action.clone(), amount.clone())), _ => None },
            ),
            // A property of the selected group, not a filter on each selected card.
            act.skip(&lit("with")).then(&crate::words::number_words()).skip(&lit("or more card types among them")).map(
                move |(action, n)| collective(&action).then_some(CostPart::WithCardTypes(action, n)),
                move |part| match part { CostPart::WithCardTypes(action, n) if collective(action) => Some((action.clone(), *n)), _ => None },
            ),
            // "Pay 1 life for each card in your hand": the "for each" an effect's act takes
            act.then(&lazy("for each", || amounts().for_each.clone())).map(
                |(a, n)| Some(CostPart::Each(a, n)),
                |p| match p {
                    CostPart::Each(a, n) => Some((a.clone(), n.clone())),
                    _ => None,
                },
            ),
            // "Pay {B} and 1 life": one "pay" for all of them, as payments print
            crate::action::payments(Form::Base).map(
                move |c| c.items.iter().all(payable).then_some(CostPart::Joined(c)),
                |p| match p {
                    CostPart::Joined(c) => Some(c.clone()),
                    _ => None,
                },
            ),
            crate::action::coord(&act).map(
                |c| (c.items.len() >= 2).then_some(CostPart::Joined(c)),
                |p| match p {
                    CostPart::Joined(c) if c.items.len() >= 2 => Some(c.clone()),
                    _ => None,
                },
            ),
        ],
    );
    let later = lit(",").then_keep(&part.clone().capital());
    let parts = part.then(&later.many()).map(
        |(first, rest)| Some(Cost(std::iter::once(first).chain(rest).collect())),
        |c| c.0.split_first().map(|(f, r)| (f.clone(), r.to_vec())),
    );
    let as_cost = |bracketed: bool| {
        loyalty(bracketed).map(
            |l| Some(Cost(vec![CostPart::Loyalty(l)])),
            |c| match c.0.as_slice() {
                [CostPart::Loyalty(l)] => Some(l.clone()),
                _ => None,
            },
        )
    };
    Costs {
        cost: alt("cost", vec![as_cost(false), parts.clone()]),
        granted_cost: alt("granted cost", vec![as_cost(true), parts]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Outcome;
    use oracle::token::tokenize;

    #[test]
    fn costs_round_trip() {
        let c = &costs().cost;
        for text in [
            "{T}",
            "{2}, {T}",
            "{1}{G}, {T}, Sacrifice ~",
            "Sacrifice a creature",
            "{T}, Pay 2 life",
            "{2}, Discard a card",
            "Exile ~ from your graveyard",
            "{2}{B}, Exile any number of other cards from your graveyard with four or more card types among them",
            "Remove a +1/+1 counter from ~",
            "{T}, Tap an untapped creature you control",
            "Pay {E}{E}",
            "Pay six {E}",
            "Return ~ to its owner's hand",
            "Remove any number of storage counters from ~",
            "+1",
            "−2",
            "0",
            "−X",
            "{Q}",
            "{2}{B}, {T}, Discard two cards",
            "Collect evidence 4",
            "Blight 1",
            "Waterbend {3}",
            "Put a -1/-1 counter on ~",
            "{T}, Mill a card",
            "Remove five quest counters from ~ and sacrifice it",
            "{T}, Sacrifice a creature and a Swamp",
            "{2}, Sacrifice another creature or an enchantment",
            "{T}, Discard a card with mana value X",
            "{2}{W}, Remove two +1/+1 counters from among creatures you control",
            "{3}, Unattach an Equipment from ~",
            "{2}, {T}, Put a card exiled with ~ into its owner's graveyard",
            "Return two lands you control to their owner's hand",
            "Pay 1 life for each card in your hand",
            "{2}{G}, Pay 3 life for each velocity counter on ~",
            "Pay {1} for each +1/+1 counter on ~",
            "Pay {B} and 1 life",
            "Pay {2} or {E}{E}{E}",
            "Pay 2 life or {2}",
            "Pay {3} or discard a card",
        ] {
            let toks = tokenize(text);
            match c.parse_all(&toks) {
                Outcome::Accepted { value, .. } => {
                    assert_eq!(c.print_text(&value, true).as_deref(), Some(text), "{value:#?}")
                }
                other => panic!("{text}: {other:#?}"),
            }
        }
    }

    #[test]
    fn r205_2_collective_cost_suffix_requires_a_plural_selection() {
        for text in ["Pay 2 life with four or more card types among them", "Exile a card from your graveyard with four or more card types among them"] {
            assert!(!matches!(costs().cost.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn r115_1c_costs_dont_target() {
        let c = &costs().cost;
        for text in [
            "Sacrifice target creature",
            "Tap target creature",
            "Discard target card",
            "{T}, Return target creature you control to its owner's hand",
            "Put a -1/-1 counter on target creature",
            "Remove a +1/+1 counter from target creature",
            "Sacrifice another target creature",
            "Exile up to one target card from a graveyard",
        ] {
            assert!(!matches!(c.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
        // what a cost chooses among is not a target
        for text in ["Sacrifice a creature", "Tap an untapped creature you control", "Remove a +1/+1 counter from a creature you control"] {
            assert!(matches!(c.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn r202_1_r702_a_cost_an_object_has_is_paid_by_name() {
        let c = &costs().cost;
        let e = &crate::effect::effects().sentence;
        fn round_trip<T: crate::kernel::Val + std::fmt::Debug>(p: &P<T>, text: &str) {
            match p.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(p.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        round_trip(c, "Pay enchanted creature's mana cost");
        round_trip(c, "Exile a creature card from your graveyard and pay its mana cost");
        round_trip(e, "At the beginning of your upkeep, sacrifice ~ unless you pay its mana cost");
        round_trip(e, "Sacrifice it unless you pay its mana cost reduced by {2}");
        round_trip(e, "Pay its suspend cost");
        let Outcome::Accepted { value, .. } = c.parse_all(&tokenize("Pay ~'s echo cost")) else { panic!() };
        let ir::CostPart::Action(ir::Action::PayCost(CostExpr::Of(r))) = &value.0[0] else { panic!("{value:?}") };
        assert_eq!(r.which, CostKind::Keyword(ir::Keyword::from_name("Echo").unwrap()));
        // a player has no mana cost, "cost" agrees with its possessor, and a stat is not a cost
        // nor has a keyword without a cost one (702.9 flying)
        for text in ["Pay your mana cost", "Pay its mana costs", "Pay their mana cost", "Pay its power", "Pay its mana cost reduced by 2", "Pay its flying cost"] {
            assert!(!matches!(c.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn r107_1_plus_an_additional_amount_for_each() {
        let e = &crate::effect::effects().sentence;
        for text in [
            "Counter target spell unless its controller pays {2} plus an additional {1} for each Faerie you control",
            "Counter target noncreature spell unless its controller pays {1} plus an additional {1} for each creature in your party",
            "Target player draws X cards plus an additional card for each creature you control",
            "You gain 3 life plus an additional 3 life for each creature you control",
            "~ deals 2 damage plus an additional 1 damage for each Goblin you control to any target",
        ] {
            match e.parse_all(&tokenize(text)) {
                Outcome::Accepted { value, .. } => assert_eq!(e.print_text(&value, true).as_deref(), Some(text), "{value:#?}"),
                other => panic!("{text}: {other:#?}"),
            }
        }
        let Outcome::Accepted { value, .. } = e.parse_all(&tokenize("You gain 3 life plus an additional 3 life for each creature you control")) else { panic!() };
        let s = format!("{value:?}");
        assert!(s.contains("Plus(Const(3), Multiple(3, Count("), "{s}");
        // the same noun, and only "for each" makes the addition
        for text in [
            "You gain 3 life plus an additional card for each creature you control",
            "Draw a card plus an additional card",
            "Counter target spell unless its controller pays {2} plus an additional {1}",
            "Draw a card plus an additional 3 life for each creature you control",
        ] {
            assert!(!matches!(e.parse_all(&tokenize(text)), Outcome::Accepted { .. }), "{text}");
        }
    }

    #[test]
    fn granted_loyalty_is_bracketed() {
        let g = &costs().granted_cost;
        let Outcome::Accepted { value, .. } = g.parse_all(&tokenize("[−2]")) else { panic!() };
        assert_eq!(g.print_text(&value, true).as_deref(), Some("[−2]"));
        assert!(matches!(costs().cost.parse_all(&tokenize("[−2]")), Outcome::Declined { .. }));
        assert!(matches!(g.parse_all(&tokenize("−2")), Outcome::Declined { .. }));
    }
}
