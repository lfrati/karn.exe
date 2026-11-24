//! Procedures: what a shuffle is done to, and the repetition of an ability's own instructions.

use crate::action::Whose;
use crate::amount::Amount;
use crate::condition::Condition;
use serde::Serialize;

/// How often an ability does its own instructions over again: "repeat this process".
///
/// No rule of the Comprehensive Rules defines the wording; 608.2c only says the controller
/// follows the instructions in the order written. "This process" is always the instructions of
/// the ability before the repeat, from its start.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Repeats {
    /// "repeat this process": the process is run again, including this instruction, so a
    /// condition in front ("If you win, repeat this process") decides each time.
    Process,
    /// "repeat this process once", "repeat this process X more times": that many extra times.
    Times(Amount),
    /// "repeat this process any number of times": the actor chooses how many.
    AnyNumber,
    /// "repeat this process until no one pays life": run again until the condition holds.
    Until(Box<Condition>),
    /// "repeat this process for an enchantment and a planeswalker", "… for artifacts and
    /// creatures": run again for each of these in turn, each in place of the kind of object the
    /// process names ("a creature", "land") (Invoke Despair, Equipoise).
    For(Vec<crate::statics::Who>),
}

/// A pile of objects. "Some effects cause objects to be temporarily grouped into piles" (CR
/// 700.3). "Each of the affected objects must be put into exactly one of those piles, unless the
/// effect specifies otherwise" (700.3a). "Each object in a pile is still an individual object.
/// The pile is not an object" (700.3b), so a pile is a sort of its own, not a selector domain.
/// "Objects grouped into piles don't leave the zone they're currently in" (700.3c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PileRef {
    /// "one pile": the actor's pick (after piles were made).
    One,
    /// "one of those piles": a choice among the piles just made.
    OneOfThose,
    /// "that pile": the pile just chosen or made.
    That,
    /// "the other": the pile that was not named.
    TheOther,
}

/// How many piles, and whether they are told apart by facing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Piles {
    /// "two piles", "three piles".
    Count(Amount),
    /// "a face-down pile and a face-up pile".
    FaceDownAndUp,
}

/// Which way up the cards of a pile made by exiling them are: "exile them in a face-down pile".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PileKind {
    FaceDown,
    FaceUp,
}

/// What a shuffle is done to. "To shuffle a library or a face-down pile of cards, randomize the
/// cards within it so that no player knows their order" (CR 701.24a).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Shuffled {
    /// "shuffle", "shuffles": the library the ability just searched, or else the actor's own
    /// (701.24b). It differs from naming a library: "search target opponent's library for a
    /// card, then shuffle" shuffles that opponent's library.
    Default,
    /// "shuffle your library", "shuffles their library": the named player's library.
    Library(Whose),
    /// "shuffle that pile": a face-down pile (701.24a).
    Pile(PileRef),
}
