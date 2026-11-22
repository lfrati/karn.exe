//! Typed, unresolved references: grammar describes what is meant; a compiler binds a
//! producer in an instruction scope or an ability provenance group (608.2c, 607.2d).
//! These descriptions contain no runtime object IDs or compiler-assigned slot IDs.
use crate::lexicon::Verb;
use serde::Serialize;

/// Value shapes currently consumed by compiled programs and linked choices. This is
/// shared with runtime slots; choice-domain restrictions remain on the choice itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Kind {
    Cards,
    /// Operation-local destruction transitions, distinct from card identity lists.
    Destructions,
    /// Actual operation-local counter removal quantities.
    Removals,
    /// Actual completed damage of one instruction (120.10).
    Damage,
    /// Bound players/objects named by an instruction (608.2h).
    Subjects,
    Consent,
    /// A numeric result produced by an instruction.
    Number,
    /// A completed local coin flip, distinct from numeric and other outcome values (705.2).
    Coin,
    /// A named future turn, produced by a scheduling instruction.
    Turn,
    /// Exact turns already created by an instruction, distinct from next-unskipped anchors.
    CreatedTurns,
    /// An unresolved type constraint, refined from its card-type or creature-type producer.
    Type,
    CardType,
    CreatureType,
    Color,
    CardName,
    /// "a basic land type" (305.6): Plains, Island, Swamp, Mountain or Forest.
    BasicLandType,
}

/// How the text identifies an earlier value, independently of its eventual producer ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum RefForm {
    Anaphor,
    ActionResult,
    Remainder,
    Choice,
}

/// A restriction on the producer meant by a reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Qualifier {
    ThisWay(Verb),
}

/// The grammar-to-binder contract. Missing/ambiguous producers are compilation errors;
/// an identified producer returning an empty group is an ordinary runtime result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct UnresolvedRef {
    pub expected: Kind,
    pub form: RefForm,
    pub qualifiers: Vec<Qualifier>,
}

impl UnresolvedRef {
    pub fn choice(expected: Kind) -> Self {
        Self { expected, form: RefForm::Choice, qualifiers: Vec::new() }
    }

    pub fn cards(form: RefForm) -> Self {
        Self { expected: Kind::Cards, form, qualifiers: Vec::new() }
    }

    pub fn action_result(verb: Verb) -> Self {
        Self { expected: Kind::Cards, form: RefForm::ActionResult, qualifiers: vec![Qualifier::ThisWay(verb)] }
    }
}
