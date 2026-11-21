//! Costs (CR 118, 602.1): the part of an activated ability before the colon, the loyalty cost of
//! a planeswalker ability (606.4), and additional casting costs.
//!
//! A cost is an ordered list of parts paid together (601.2h): mana, {T}/{Q} (107.5, 107.6), a
//! loyalty change, and actions ("Sacrifice a creature", "Pay 2 life").

use crate::action::Action;
use crate::amount::Amount;
use crate::mana::ManaCost;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Cost(pub Vec<CostPart>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CostPart {
    Mana(ManaCost),
    /// {T} (107.5).
    Tap,
    /// {Q} (107.6).
    Untap,
    /// "+1", "−2", "0", "−X" (606.4); written "[+1]" on some faces.
    Loyalty(Loyalty),
    Action(Action),
    /// A keyword rule keeps this reveal in force until the activated occurrence leaves
    /// the stack, rather than only until payment finishes (702.49b).
    RevealUntilLeavesStack(crate::Objects),
    /// The selected group must have at least this many distinct card types (205.2).
    WithCardTypes(Action, u32),
    /// "Pay 1 life for each card in your hand", "Pay {1} for each +1/+1 counter on ~": the
    /// action done once for each counted thing, as [`crate::effect::Modifier::ForEach`] says of
    /// an effect's act (a cost is an action, 118.1).
    Each(Action, Amount),
    /// An explicitly defined X for this cost, fixed after announcement targets.
    DefinedX(Action, Amount),
    /// Actions written as one part: "Remove five quest counters from ~ and sacrifice it" (paid
    /// in order; the later ones may refer back with "it"), "Pay {3} or discard a card" (the
    /// payer picks one). One "pay" may govern several payments: "Pay {B} and 1 life" is "pay
    /// {B} and pay 1 life", "Pay {2} or {E}{E}{E}" is "pay {2} or pay {E}{E}{E}".
    Joined(crate::action::Coord<Action>),
}

impl CostPart {
    /// A mana payment, whether written as symbols or the action “pay {…}” (118.1).
    pub fn mana(&self) -> Option<&ManaCost> {
        match self {
            CostPart::Mana(cost) | CostPart::Action(Action::PayMana(cost)) => Some(cost),
            _ => None,
        }
    }
}

/// A cost an object has, named as a noun: "its mana cost" (the mana cost is a characteristic,
/// 202.1), "~'s echo cost", "its suspend cost" (the cost a keyword ability gives it: equip
/// 702.6a, cumulative upkeep 702.24a, echo 702.30a, flashback 702.34a, suspend 702.62a, …).
/// Whether "cost" or "costs" is written follows the number of the possessor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CostRef {
    pub which: CostKind,
    pub of: crate::selector::Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CostKind {
    /// "mana cost" (202.1).
    Mana,
    /// "[keyword] cost": the cost of that keyword ability of the object (702).
    Keyword(crate::vocab::Keyword),
}

/// A cost as an amount to pay, where it is not only mana symbols: a cost an object has, and
/// what is done to it ("its mana cost reduced by {2}", 118.7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CostExpr {
    /// "its mana cost", "~'s suspend cost".
    Of(CostRef),
    /// "its mana cost reduced by {2}": the generic part reduced (118.7, 601.2f).
    Reduced(Box<CostExpr>, ManaCost),
    /// "{2}", where it is part of a larger cost (a cost of symbols alone is
    /// [`Action::PayMana`]).
    Mana(ManaCost),
    /// "{2} plus an additional {1} for each Faerie you control": both costs, paid together.
    Plus(Box<CostExpr>, Box<CostExpr>),
    /// "{1} for each Faerie you control": the cost once for each thing counted.
    Each(Box<CostExpr>, Amount),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Loyalty {
    Plus(Amount),
    Minus(Amount),
    Zero,
}
