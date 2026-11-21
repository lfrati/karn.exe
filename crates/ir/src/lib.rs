//! The card IR: what a card's text means, as closed types.
//!
//! Text never appears here except for genuinely open sets (card names in "named …"). Every value
//! is produced by the grammar (`crates/grammar`), which also prints it back; the engine compiles
//! it into its own runtime form.

#![forbid(unsafe_code)]

pub mod ability;
pub mod action;
pub mod amount;
pub mod condition;
pub mod cost;
pub mod effect;
pub mod event;
pub mod lexicon;
pub mod mana;
pub mod procedure;
pub mod reference;
pub mod selector;
pub mod statics;
pub mod token;
pub mod turn;
pub mod vocab;
pub mod zone_move;

pub use ability::{Ability, Block, Mode, ResultRange};
pub use action::{
    AbilityKind, Action, Becoming, Characteristics, Choice, Exclusion, Options, ValueKind, Conj, Coord, AnyAmountOf, Counters, CountersOf, CountersOn, On, SetTo, Setting, Extra, Hit, CounterCount, Delta, Destination, Entry, Every, Factor, Granted, Keeps, Kept, KeywordAction, LibraryPosition, MoveVerb,
    NumberedAction, ObjectAction, Objects, Order, Placement, PlayerCounter, Prevented, Prevention, UntilVerb, UntapWhose, VoteFor, Whose,
};
pub use amount::{Aggregate, Amount, Characteristic, Cmp};
pub use condition::{CastFact, Condition, Designation, Predicate, Restriction, Window};
pub use cost::{Cost, CostExpr, CostKind, CostPart, CostRef, Loyalty};
pub use effect::{
    Act, Body, Clause, CopyChange, CopyRef, DelayLimit, Duration, Effect, Exception, Join, Lead, Modes, Modifier, Rider, Sentence, Subject, Unchosen,
};
pub use event::{Arg, CounterNth, DamageKind, Deed, EventAction, EventCause, EventPattern, Step, StepOwner, Trigger, TriggerWord};
pub use lexicon::{Doer, Inflection, Needs, Role, Sort, SubjectIs, Verb};
pub use turn::{Part, Phase};
pub use mana::{ManaColor, ManaCost, ManaSpec, ManaSymbol, ManaTypes, Pooled, SpendOn, SpendableMana};
pub use selector::{
    Anaphor, Atom, Attachment, Chosen, Count, CounterKind, Domain, Filter, Name, ObjectSet, Quant, Ref, Relation,
    Selector, Slice, Stat, Status, ZoneSpec,
};
pub use statics::{Arrival, CostOf, Forbidden, Lose, Lost, AbilitiesOf, AbilityRule, AnyMana, HandSize, Elsewhere, Party, Playable, Replacement, SkipStep, Skipped, SpentMana, Substitute, TurnAt, Static, StaticAct, Who, Within, XLimit};
pub use token::{GiftOf, KeywordInstance, KeywordParam, Predefined, Pt, Quality, TokenDesc, TokenKind, TokenSpec};
pub use vocab::{CardType, Color, Keyword, Subtype, Supertype, Zone};
pub use zone_move::{LibraryPlace, Position};
