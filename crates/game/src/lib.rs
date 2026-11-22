//! The rules engine.
//!
//! Card text is read by the grammar into the card IR; this crate runs it. The contract with the
//! grammar includes shared vocabulary and operand contracts in [`ir::lexicon`]. Compiled
//! programs execute through procedures and operations; resulting events drive triggers.
//!
//! What is here: players, zones, objects that become new objects when they change zones (CR
//! 400.7), casting with targets and costs (601.2), activated abilities with their costs and
//! mana abilities (602, 605, 118), the stack and priority (117), resolution of instructions
//! (608.2), state-based actions for damage, toughness, life and an empty library (704.5),
//! triggered abilities (603), the turn's phases and steps with their turn-based actions
//! (500–514, 703.4), combat (506–511), the keyword abilities of combat (702), and the
//! restrictions and requirements on attacking and blocking that static abilities and resolved
//! spells put in force (101.2, 508.1c–d, 509.1b).
//!
//! What is not, on purpose: nothing is guessed. Text the engine cannot run is refused with the
//! reason ([`Unsupported`]) before a card is cast. Continuous effects, replacement effects,
//! choices and reference binding are implemented in bounded subsets. The rule assessments in
//! `docs/rule-assessments.json` track their tested scope and remaining gaps; corpus acceptance
//! alone does not establish correctness.

#![forbid(unsafe_code)]

#[cfg(feature = "profile")]
pub mod profile;

pub mod audit;
pub mod ability;
pub mod activate;
pub mod attach;
pub mod card;
pub mod binding;
pub mod combat;
pub mod condition;
pub mod event;
pub mod face;
mod discard;
mod copy;
mod target_change;
pub mod functions;
pub mod keyword;
pub mod layers;
mod query;
mod prevention;
mod level;
mod station;
mod class;
mod room;
pub mod legal;
pub mod mana;
pub mod modes;
pub mod observation;
pub mod operation;
mod entry;
mod creation;
mod case;
mod draw;
mod damage;
mod perform;
mod procedure;
mod payment;
mod payment_audit;
mod permission;
mod preparation;
mod plot;
mod suspend;
pub mod foretell;
mod saga;
mod history;
mod retention;
mod linked_lifetime;
mod crime;
pub mod play;
pub mod prompt;
pub mod program;
pub mod protection;
pub mod announcement;
mod library;
mod departure;
mod amass;
mod numbered;
pub mod replace;
pub mod restrict;
pub mod select;
pub mod start;
pub mod state;
pub mod simulation;
pub mod snapshot;
pub mod statement;
pub mod token;
pub mod trigger;
pub mod turn;
pub mod turn_control;
mod until;
pub mod view;

pub use card::CardDef;
pub use event::{Event, Target};
pub use play::Move;
pub use state::{Game, ObjId, PlayerId, Zone};

/// Text the engine does not run yet, with what it stopped at. A card with any is refused whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

pub type Res<T> = Result<T, Unsupported>;

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Res<T> {
    Err(Unsupported(what.into()))
}

mod cost;
pub use cost::{Branch, BranchSelection, ForagePayment, HybridHalf, SelectedBranch};
mod cost_change;

mod monarch;
mod speed;
mod cast_permission;
mod library_top;
mod ability_limit;
pub mod companion;
mod commander;

mod rebound;
mod warp;
mod sneak;
pub mod completion;
