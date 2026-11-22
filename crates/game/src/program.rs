//! Compiled ability programs. There is no parsed-effect interpreter fallback.
//! Definitions outside the supported lowering fail compilation. Shared semantic operations
//! live in their owning modules; execution follows immutable programs and retained frames.
//! Syntax lowering and instruction execution have separate owners below.

use std::borrow::Cow;
use crate::event::Target;
use crate::announcement::TargetSpec;
use crate::select::{Ctx, Support};
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, Zone};
use crate::{Res, unsupported};
use ir::action::{Destination, LibraryPosition, Order, Whose};
use ir::effect::Join;
use ir::selector::Slice;
use ir::{Action, Amount, Atom, Clause, Count, Domain, Effect, Lead, ObjectSet, Quant, Ref, Selector, Sentence, Subject, Verb};
use std::fmt;
use std::sync::Arc;

pub mod action;
pub mod choice;
mod connive;
mod explore;
mod library_scan;
mod endure;
mod purpose;
pub(crate) mod initial;
pub mod arrange;
pub mod piles;
mod compile;
mod execute;

use compile::lower;
pub(crate) use compile::{Lowering, compile_definition, continues_previous, initial_condition, instructions, replaces_previous, scalar_choice};
pub mod search;
pub(crate) mod registration;
pub(crate) mod footprint;
pub mod expr;
pub mod mana;
pub mod condition;
pub mod change;
pub mod input;

pub(crate) enum Halt { Unsupported(crate::Unsupported), Ask(Decision), Child }
/// Why a frame stopped before its end: a player must answer, or a child procedure it
/// started (an effect's payment) must run first and report back.
#[derive(Debug)]
pub(crate) enum Paused { Ask(Decision), Child }
impl From<crate::Unsupported> for Halt {
    fn from(value: crate::Unsupported) -> Self { Self::Unsupported(value) }
}


/// A typed register of a frame.
pub type Slot = usize;

/// A diagnostic identity: the 64-bit FNV-1a digest of [`Program::canonical`]. Equal canonical
/// text has the same identity regardless of process or compilation order. Referenced
/// templates contribute syntax and predefined tokens their kind; this is not a transitive
/// fingerprint of their compiled definitions or the runtime. Compiled-instance identity
/// is [`same_instance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramId(u64);
impl ProgramId {
    fn of(program: &Program) -> ProgramId {
        let mut digest = Digest::new();
        program.write_canonical(&mut digest).expect("a digest accepts every write");
        ProgramId(digest.0)
    }
    /// The identity of the program whose canonical form is this text: what a reader of a
    /// dump checks its `id` against.
    pub fn of_canonical(text: &str) -> ProgramId {
        ProgramId(digest(text.as_bytes()))
    }
}
impl fmt::Display for ProgramId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:016x}", self.0) }
}
/// The 64-bit FNV-1a digest of these bytes, the same in every process, build and platform.
pub fn digest(bytes: &[u8]) -> u64 {
    let mut digest = Digest::new();
    digest.bytes(bytes);
    digest.0
}
/// Whether these are one compiled instance: the program compiled for one ability of one
/// definition (or by one rule), which that definition's clones and every occurrence of the
/// ability share. Equal content is not enough: two abilities, or two faces of one card, can
/// compile to equal programs with one [`ProgramId`] ("Whenever you cast a blue spell, …" and
/// "Whenever you cast a black spell, …" doing the same), and which printed ability an
/// occurrence came from (113.7a, 603.7) is told by its instance.
pub(crate) fn same_instance(a: &Arc<Program>, b: &Arc<Program>) -> bool { Arc::ptr_eq(a, b) }

/// FNV-1a over the bytes written, without building the text.
struct Digest(u64);
impl Digest {
    fn new() -> Digest { Digest(0xcbf2_9ce4_8422_2325) }
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes { self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3); }
    }
}
impl fmt::Write for Digest {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.bytes(s.as_bytes());
        Ok(())
    }
}

pub use crate::binding::{Kind, Value};
use crate::binding::Scalar;

/// Which player an instruction is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The controller of the ability being followed.
    Controller,
    /// The player held in this slot: the one a clause says does what it says ("Its
    /// controller may search their library …"), named once as the clause begins.
    Of(Slot),
}

/// Who may see a shown group (701.20: everyone; looking at cards: one player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Everyone,
    Only(Who),
    /// Each card to its owner: what each player found in their own library (701.23i).
    Owners,
}

/// Selection preserves its domain: hidden-card filters exclude tokens; permanent selections
/// use the ordinary object query, which also includes token permanents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionFilter {
    Cards(expr::Cards), Objects(expr::Set),
    /// Choice operands must make the specified action possible (608.2d); this is
    /// neither target legality nor proof that a replacement will let it complete.
    /// `enters`: the action puts the object onto the battlefield, which can be prohibited.
    Action { set: expr::Set, verb: Verb, role: ir::Role, enters: bool },
}
impl From<expr::Cards> for SelectionFilter { fn from(cards: expr::Cards) -> Self { Self::Cards(cards) } }
impl SelectionFilter {
    fn scalars(&self) -> Vec<Scalar> { match self { Self::Cards(cards) => cards.scalars(), Self::Objects(set) | Self::Action { set, .. } => set.scalars() } }
    fn contains(&self, game: &Game, ctx: &Ctx, frame: &Frame, object: ObjId) -> bool {
        match self { Self::Cards(cards) => cards.contains(game, ctx, frame, object), Self::Objects(set) => set.contains(game, ctx, frame, Target::Object(object)),
            // "you" remains the effect controller, while the retained clause actor
            // supplies "they". Only action feasibility uses the chooser's seat.
            // 608.2d: an option that is impossible, such as destroying an indestructible
            // permanent (702.12b) or putting a card onto the battlefield it can't enter,
            // can't be chosen.
            Self::Action { set, verb, role, enters } => set.contains(game, &frame.ctx, frame, Target::Object(object))
                && if *role == ir::Role::What { game.can(*verb, ctx.controller, Target::Object(object)) } else { game.takes(*verb, *role, Target::Object(object)) }
                && !game.forbidden(&crate::trigger::Fact::new(*verb, None, vec![(*role, Target::Object(object))]))
                && !(*enters && game.forbidden(&crate::trigger::Fact::new(Verb::Enter, None, vec![(ir::Role::What, Target::Object(object))]))),
        }
    }
}

/// How many of a group a selection takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// One optional object action: consent and its conditional selection are one
    /// choice (608.2d). The retained outcome distinguishes decline from impossibility.
    Optional { count: usize, paid: Slot, possible: Slot },
    /// Exact evaluated quantity, taking all available when fewer fit (609.3).
    Quantity(expr::Amount),
    /// Any subset, including none (608.2d).
    AnyNumber,
    /// Every one that fits.
    All,
    /// So many, chosen by the chooser when more fit (608.2d).
    Exactly(usize),
    /// Any subset of at most this size, including none (608.2d).
    UpTo(usize),
    /// So many, or none when fewer fit: the objects of an instruction a later "if you do" or
    /// "if you can't" asks about, which is followed wholly or not at all ("If there's only
    /// one card in your graveyard, you won't exile it", Egon, God of Death ruling 2021-02-05).
    Whole(usize),
}

/// What a decline of "you may" depends on: the group, the description and how many are needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub from: Slot,
    pub set: expr::Cards,
    pub count: expr::Amount,
    pub still_in: Zone,
}

/// Where a group goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dest {
    Hand,
    Graveyard,
    Library { position: LibraryPosition, order: Option<Order> },
    /// Entry details of a partition of one coordinated put instruction.
    Battlefield(action::Destination),
}

/// How a group is exiled face down (406.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hide {
    /// Who looked at the cards before they were exiled, and so may go on looking at them.
    lookers: Vec<Who>,
    /// The cards gain "the player who controls the permanent that exiled this card may look
    /// at this card in the exile zone" (702.75a).
    watched: bool,
}

/// Where a resolution-time cast's exact retained card may be offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastOrigin {
    /// The text imposes a zone, or a nonmoving producer reads that zone.
    Fixed(Zone),
    /// Exact card creations or successful public movement successors. Their
    /// actual zone is captured when offered, including replacements (400.7j, 707.12).
    Produced(Slot),
    /// An exact pre-movement reference and its attempted public move. The live
    /// original survives a failed move; only this occurrence's public transitions
    /// may supply its successor (609.3, 400.7j).
    Retained { original: Slot, moved: Slot },
}

/// One instruction. Operands are slots; every slot an instruction reads is produced by an
/// earlier one ([`Program::new`] checks it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instr {
    /// The exact permanent endures N, with one optional counter cost (701.63).
    Endure { permanent: action::Operand, amount: expr::Amount },
    /// A typed ordinary action with operands evaluated once before execution.
    Action { action: action::Action, created: Option<Slot>, attempt: Option<Attempt> },
    /// 608.2d: choose one result-free token instruction during resolution.
    Alternative { actions: Vec<action::Action>, options: Vec<crate::state::ActionOption> },
    /// 603.12: queue a captured occurrence when what this resolution did is as it says:
    /// its payment completed ("When you do"), or the event happened ("When you discard a
    /// nonland card this way").
    Reflexive { when: condition::Condition, each: Option<ReflexiveEach>, program: Arc<Program> },
    Delayed { pattern: crate::trigger::DelayedPattern, subjects: Vec<action::Operand>, program: Arc<Program>, lasts: crate::state::Lasts, once: bool },
    /// Gather the instructed players' choices before committing their shared action.
    /// `chosen`: what they settled on, as it was before the action, where the text goes on
    /// to speak of it ("sacrifices a creature … that creature's toughness").
    Choice { action: choice::Action, optional: bool, payment: bool, paid: Slot, possible: Slot, chosen: Option<Slot>, outcomes: Option<[Slot; 3]>, actual: Option<Slot>, discarded_before: Option<Slot> },
    /// Capture the legal identities named by an instruction before that instruction acts.
    Capture { operand: action::Operand, out: Slot },
    Input { input: input::Input, out: Slot },
    Mana { players: action::Operand, production: mana::Production },
    /// Choose counter holders, then run one result-free placement child (701.34a).
    Proliferate { by: Who },
    /// A player searches a library for cards (701.23): `by`, in their own, or in each of
    /// these players', one after another. `out`: every card found, which `by` alone has seen.
    /// `own`: each of these players searches their own instead, choosing in APNAP order
    /// (701.23i), and sees only what they found.
    Search { action: search::Action, by: Who, whose: Option<action::Operand>, own: bool, out: Slot },
    Shuffle { players: action::Operand, found: Option<Slot> },
    /// Reveal the cards publicly as the instructed player (701.20).
    Observe { objects: action::Operand, by: Who, out: Slot },
    /// Retain a described public card group without revealing it again.
    Gather { objects: action::Operand, out: Slot },
    Subjects { cards: Slot, out: Slot, still_in: Option<Zone> },
    /// `by`: who orders them, where the text has a player put them "in any order" (into
    /// another player's library too); None: each owner (401.4). `before_shuffle` marks
    /// internal insertion for ShuffleInto, whose only random event is the following shuffle.
    Place { objects: Vec<action::Operand>, position: ir::Position, chooser: Option<action::Operand>, order: Option<Order>, attempt: Option<Attempt>, by: Option<Who>, before_shuffle: bool },
    /// "You may pay {B}" (118.12): ask the payer, then run the payment as a child procedure.
    /// `paid`: the supplied payment completed. Impossibility predicates are refused.
    Pay { payer: action::Operand, cost: crate::cost::Plan, each: Option<expr::Amount>, paid: Slot },
    /// "You may play it this turn", "until the end of your next turn, you may play those
    /// cards": the permission is the effect; nothing is asked now. `play`: a land may be
    /// played that way too, not only a spell cast. `until`: when it ends (611.2a).
    Permit { player: action::Operand, cards: Vec<action::Operand>, play: bool, until: crate::permission::Deadline },
    /// "You may cast it without paying its mana cost": offer the player these cards to cast
    /// as the effect is applied (608.2g), one cast at a time in the order they choose, until
    /// they decline. Each cast is a child of this resolution; no player receives priority.
    /// `offered`: the cards of this offer, which no other effect being applied offers or ends.
    /// `costs`: what they may be cast for, without paying their mana costs or for a cost the
    /// card states (madness, 702.35a). `cast`: whether a cast of one was made, its payment
    /// completed (601.2h), and `possible`, whether one was offered: "If you don't, …".
    /// `from`: the fixed source zone supplied by the bound card producer;
    /// `rider`: what a spell cast this way carries; `one`: "cast a spell": one cast at most.
    Cast { player: action::Operand, cards: Vec<action::Operand>, offered: Slot, costs: crate::permission::Costs, cast: Slot, possible: Slot, from: CastOrigin, rider: Option<crate::permission::Rider>, one: bool, ceiling: Option<Slot>, haste: bool },
    /// "Copy it three times": for each of these cards, that many copies of it made in the
    /// zone it is in, for the controller to cast (707.12). `out`: the copies.
    CopyCards { cards: action::Operand, times: expr::Amount, out: Slot },
    /// Copies of already-announced spells, with retained per-copy optional new targets (707.10).
    CopyStack { cards: action::Operand, times: expr::Amount, retarget: bool, out: Slot },
    ChangeTargets { object: action::Operand, policy: crate::state::TargetPolicy },
    Connive { objects: action::Operand },
    /// 701.44: the permanents explore, one at a time.
    Explore { objects: action::Operand },
    Arrange { players: action::Operand, verb: Verb, amount: expr::Amount, optional: bool },
    Register { subject: action::Operand, rule: crate::restrict::Rule, lifetime: crate::restrict::Lifetime },
    ControlTurn { player: action::Operand, out: Slot },
    ExtraTurns { players: action::Operand, count: expr::Amount, after: Option<Slot>, out: Slot },
    RuleDuringCreatedTurns { turns: Slot, rule: crate::restrict::Rule },
    SkipTurns { players: action::Operand, count: expr::Amount },
    /// "Target player skips their next draw step" (614.10): each of these players' next such
    /// occurrence is skipped; with `this_turn`, only one of this turn.
    Skip { players: action::Operand, what: crate::turn::Occurrence, this_turn: bool },
    /// "After this main phase, there is an additional combat phase followed by an additional
    /// main phase" (500.8): `count` times these phases, after the phase in progress when it is
    /// the one named.
    AddPhases { after: ir::action::AfterPhase, kinds: Vec<ir::turn::Phase>, count: expr::Amount },
    Copy { objects: Vec<action::Operand>, from: action::Operand, turn: bool },
    Continuous { objects: Vec<action::Operand>, changes: Vec<change::Change>, each: Option<expr::Amount>, duration: change::Duration },
    /// Test once, then follow or skip a forward block of instructions.
    Branch { condition: condition::Condition, end: usize, remember: Option<Slot>, skipped_values: Vec<(Slot, Value)> },
    /// The sentence-defined X, evaluated once before its instruction (107.3c).
    SetX(expr::Amount),
    /// Skip optional instructions if consent was declined. Produced groups are empty;
    /// unmade scalar choices remain unavailable. The constructor checks every path.
    SkipUnless { consent: Slot, count: usize },
    /// A resolution-time scalar choice, before subsequent instructions (608.2d).
    Choose { kind: crate::binding::Domain, who: Who, out: Slot },
    /// A called flip: its player's call suspends before the fair random result (705.2).
    FlipCoin { out: Slot },
    /// The top `n` cards of the library of `whose`, as many as there are. Changes nothing.
    TakeTop { n: expr::Amount, whose: action::Operand, out: Slot },
    /// Sequential actual exiles, with a nonland hit and final-card designation (701.57a, 702.85a).
    /// `bound` is the inclusive resulting-spell ceiling, usable only when a hit exists.
    LibraryScan { by: Who, amount: expr::Amount, limit: ScanLimit, all: Slot, hit: Slot, designated: Slot, bound: Slot },
    /// 701.57b: emitted only after cast/hand and random-bottom continuations complete.
    Discovered { by: Who, bound: Slot },
    /// Read a library prefix through its first matching card; no mutation or disclosure.
    TakeUntil { set: expr::Cards, whose: action::Operand, out: Slot, matched: Slot },
    /// Show a group to an audience. To everyone it is a reveal (701.20): the cards that can be
    /// shown are, and the others are dropped from `out`. Nothing moves.
    Show { from: Slot, by: Who, to: Audience, out: Slot },
    /// Exile a group and retain the new public-zone identities (400.7j, 701.13a); face down
    /// with `hidden` (406.3).
    Exile { from: Slot, out: Slot, hidden: Option<Hide> },
    /// "You may …": ask whether to, unless `probe` shows it would be impossible (608.2d).
    Gate { who: Who, probe: Option<Probe>, out: Slot, possible: Option<Slot> },
    /// The cards of a group, still in `still_in`, that the description fits: all of them or
    /// so many chosen by `chooser`. With a `gate` that was declined, none (but a group is still
    /// produced).
    Select { from: Slot, set: SelectionFilter, pick: Pick, chooser: Who, gate: Option<Slot>, still_in: Zone, out: Slot },
    /// What is left of `of`, still in `still_in`, after the groups in `minus`.
    Remainder { of: Slot, minus: Vec<Slot>, still_in: Zone, out: Slot },
    /// Those of a group still where it was are separated into two piles, by the controller
    /// or by an opponent the controller chooses, and the stated player chooses one pile
    /// (700.3). `one`: the pile chosen; `other`: the pile that was not.
    Piles { from: Slot, still_in: Zone, opponent: bool, chooser: action::Operand, one: Slot, other: Slot },
    /// Move a group to a zone, by the existing movement code (400.7, 401.4).
    Move { src: Slot, to: Dest, by: Who, when: Option<condition::Condition> },
    /// One put instruction partitions fixed inputs between destinations. Every
    /// selection/order precedes the whole move; public entry results stay scoped
    /// to the exact input partition rather than the combined operation result.
    MoveTogether { parts: Vec<(Slot, Dest)>, entered: Vec<(Slot, Slot)> },
    /// "Repeat this process": follow the instructions from `from` again, once the act this
    /// ends is done. What one time through made (`locals`) is forgotten first, so each time
    /// names its own cards; it ends the conditional block that stops it (608.2c).
    Repeat { from: usize, locals: Vec<Slot> },
}

impl Instr {
    /// These producers retain exact castable creations or successful public successors.
    fn cast_result(&self) -> Option<(Slot, Kind)> {
        match self {
            Self::CopyCards { out, .. } => Some((*out, Kind::Subjects)),
            Self::Exile { out, .. } => Some((*out, Kind::Cards)),
            Self::Action { action, created: Some(out), .. } if matches!(action.output(),
                Some(action::Output::Moved { destination: Some(Zone::Battlefield | Zone::Graveyard | Zone::Exile | Zone::Stack) } | action::Output::Milled { .. })) => Some((*out, Kind::Cards)),
            _ => None,
        }
    }
    /// What this instruction's result is when the block it is in was skipped: an instruction
    /// that makes a group of objects made none, which later text can still name ("the cards
    /// found", "the tokens created") and count as zero. No output for every other instruction,
    /// a scalar choice among them: a choice that was never made has no value, and text that
    /// asks for one outside its block is refused rather than given one.
    fn skipped_result(&self) -> Vec<(Slot, Value)> {
        if let Instr::MoveTogether { entered, .. } = self {
            return entered.iter().map(|(_, out)| (*out, Value::Cards(Vec::new()))).collect();
        }
        if let Instr::Choice { actual, discarded_before, .. } = self {
            return actual.iter().chain(discarded_before).map(|slot| (*slot, Value::Cards(Vec::new()))).collect();
        }
        match self {
            Instr::Search { out, .. } | Instr::CopyStack { out, .. } => Some((*out, Value::Subjects(Vec::new()))),
            Instr::Select { out, .. } | Instr::Observe { out, .. } | Instr::Show { out, .. } => Some((*out, Value::Cards(Vec::new()))),
            Instr::ExtraTurns { out, .. } => Some((*out,Value::CreatedTurns(Vec::new()))),
            Instr::Subjects { out, still_in: Some(_), .. } => Some((*out, Value::Subjects(Vec::new()))),
            Instr::Action { action, created: Some(out), .. } => action.output().map(|result| (*out, result.empty())),
            _ => None,
        }.into_iter().collect()
    }
}

/// "When you discard a nonland card this way, … that card's mana value": the reflexive
/// ability triggers once for each such card (603.12a, 603.2c), and "that card" is the one
/// it triggered for, held in `member` as it is queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflexiveEach { members: expr::Number, member: Slot }

/// The stopping comparison of a sequential library exile (701.57a, 702.85a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLimit { LessThan, AtMost }
impl ScanLimit {
    fn allows(self, value: u32, bound: u32) -> bool {
        match self { Self::LessThan => value < bound, Self::AtMost => value <= bound }
    }
    /// An inclusive ceiling for the resulting spell. LessThan(0) has no hit and
    /// therefore no cast offer; zero is never used to manufacture a candidate.
    fn ceiling(self, bound: u32) -> u32 {
        bound.saturating_sub(u32::from(self == Self::LessThan))
    }
}

/// A resolution-time optional action or cost (118.12). Its result describes starting
/// payment, independently of what replacement effects ultimately do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt { actor: action::Operand, optional: bool, payment: bool, distributed: bool, paid: Slot, possible: Slot, count: Option<BoundedCount>,
    /// A cost, paid in full or not at all, even where it is required (118.3): "Pay {E}{E}.
    /// If you can't, …". Anything else is followed as far as it can be (609.3).
    cost: bool }

/// A standalone voluntary draw chooses its count before the shared draw action (121.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundedCount { max: u32, out: Slot }

/// A compiled ability: instructions, and where each act of the text ends (an act is what the
/// engine treats as one simultaneous event; the core settles triggers and combat after it).
/// Two are equal when they are one compiled instance ([`same_instance`]), as every compared
/// occurrence, trigger and definition relies on; equal content is told by [`ProgramId`].
#[derive(Debug)]
pub struct Program {
    pub(crate) announcement: crate::announcement::Facts,
    action_limit: bool,
    pub(crate) id: ProgramId,
    pub(crate) instrs: Vec<Instr>,
    /// One per act of the text: the index after its last instruction, and whether it moves
    /// things (so that what leaves the battlefield is looked back at, 603.10a).
    pub(crate) acts: Vec<ActSpan>,
    /// Announcement metadata, compiled with the operation rather than rediscovered by dry execution.
    pub(crate) targets: Vec<TargetSpec>,
    slots: usize,
    initial: Vec<Option<Kind>>,
    kinds: Vec<Option<Kind>>,
    /// Of a delayed or reflexive triggered ability's instructions: that ability's sentences,
    /// as the text that creates it says them (603.7, 603.12), which is the text it has on the
    /// stack (603.3). Presentation only: no part of what it does or of its identity.
    pub(crate) text: Option<Effect>,
}

impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool { std::ptr::eq(self, other) }
}
impl Eq for Program {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActSpan {
    pub end: usize,
    pub moves: bool,
    /// The X its sentence defines is also the next act's ("draws X cards and loses X life,
    /// where X is …", 107.3c): computed once, and kept until an act that does not keep it.
    pub keeps_x: bool,
}

impl Program {
    /// Its canonical form: every compiled field but its identity, as the derived `Debug`
    /// writes them, nested delayed and reflexive programs in full. Nothing in a program is
    /// kept in hash order or by address, so the text depends only on what was compiled.
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        self.write_canonical(&mut out).expect("a String accepts every write");
        out
    }
    fn write_canonical(&self, out: &mut impl fmt::Write) -> fmt::Result {
        let Program { announcement, action_limit, id: _, instrs, acts, targets, slots, initial, kinds, text: _ } = self;
        write!(out, "{:?}", (announcement, action_limit, instrs, acts, targets, slots, initial, kinds))
    }
    /// The program with the identity of what it now is: called once its last field is set.
    fn sealed(mut self) -> Program {
        self.id = ProgramId::of(&self);
        self
    }
    /// A program, if every slot read is produced by an earlier instruction and has the kind the
    /// reader expects. This is the compile-time difference between a reference with no producer
    /// and an empty group.
    pub fn new(instrs: Vec<Instr>, acts: Vec<ActSpan>) -> Res<Program> {
        Self::with_targets(instrs, acts, Vec::new())
    }

    // What was validated is read, not changed: the slot kinds and announcement facts were
    // derived from these instructions, and a changed program is another program.
    pub fn id(&self) -> ProgramId { self.id }
    pub fn instrs(&self) -> &[Instr] { &self.instrs }
    pub fn acts(&self) -> &[ActSpan] { &self.acts }
    pub fn targets(&self) -> &[TargetSpec] { &self.targets }

    fn with_targets(instrs: Vec<Instr>, acts: Vec<ActSpan>, targets: Vec<TargetSpec>) -> Res<Program> {
        Self::with_initial(instrs, acts, targets, Vec::new())
    }
    fn with_initial(instrs: Vec<Instr>, acts: Vec<ActSpan>, targets: Vec<TargetSpec>, initial: Vec<Option<Kind>>) -> Res<Program> {
        let mut kinds = initial.clone();
        let mut incoming: Vec<Option<Vec<Option<Kind>>>> = vec![None; instrs.len() + 1];
        fn merge(into: &mut Vec<Option<Kind>>, from: &[Option<Kind>]) {
            into.resize(into.len().max(from.len()), None);
            for (i, kind) in into.iter_mut().enumerate() {
                if Some(*kind) != from.get(i).copied() { *kind = None; }
            }
        }
        fn edge(incoming: &mut [Option<Vec<Option<Kind>>>], end: usize, kinds: &[Option<Kind>]) {
            match &mut incoming[end] { Some(old) => merge(old, kinds), slot @ None => *slot = Some(kinds.to_vec()) }
        }
        // a player held in a slot was named there first
        fn who(kinds: &[Option<Kind>], who: Who) -> Res<()> {
            match who { Who::Controller => Ok(()), Who::Of(slot) => need(kinds, slot, Kind::Subjects) }
        }
        fn skipped_kind(value: &Value) -> Res<Kind> {
            Ok(match value {
                Value::Damage(_) => Kind::Damage,
                Value::Destructions(_) => Kind::Destructions, Value::Removals(_) => Kind::Removals,
                Value::Consent(_) => Kind::Consent, Value::Number(_) => Kind::Number,
                Value::Subjects(_) => Kind::Subjects, Value::Cards(_) => Kind::Cards,
                Value::CreatedTurns(_) => Kind::CreatedTurns,
                _ => return unsupported("a skipped branch cannot invent a scalar choice"),
            })
        }
        fn need(kinds: &[Option<Kind>], slot: Slot, kind: Kind) -> Res<()> {
            match kinds.get(slot).copied().flatten() {
                Some(k) if k == kind => Ok(()),
                Some(k) => unsupported(format!("slot {slot} holds {k:?}, not {kind:?}")),
                None => unsupported(format!("slot {slot} is read before any instruction produces it")),
            }
        }
        fn make(kinds: &mut Vec<Option<Kind>>, slot: Slot, kind: Kind) -> Res<()> {
            if kinds.len() <= slot {
                kinds.resize(slot + 1, None);
            }
            if kinds[slot].is_some() {
                return unsupported(format!("slot {slot} is produced twice"));
            }
            kinds[slot] = Some(kind);
            Ok(())
        }
        // A repeated process: where it starts, the kinds there, and after it, what one time
        // through made, which no later instruction may read (the slots are forgotten).
        let entries: Vec<usize> = instrs.iter().filter_map(|i| match i { Instr::Repeat { from, .. } => Some(*from), _ => None }).collect();
        if entries.len() > 1 { return unsupported("repeat this process: one repeated process in a program"); }
        let mut entry_kinds = None;
        let mut escaped: Vec<Slot> = Vec::new();
        // nothing falls through a repeat: what follows it is reached only by a branch
        let mut dead = false;
        let checked = (|| -> Res<()> { for (pc, i) in instrs.iter().enumerate() {
            match incoming[pc].take() {
                Some(from) if dead => kinds = from,
                Some(from) => merge(&mut kinds, &from),
                None if dead => kinds = vec![None; kinds.len()],
                None => {}
            }
            dead = false;
            for slot in &escaped { if let Some(kind) = kinds.get_mut(*slot) { *kind = None; } }
            if entries.contains(&pc) { entry_kinds = Some(kinds.clone()); }
            match i {
                Instr::Repeat { from, locals } => {
                    let boundary = |at: usize| acts.iter().any(|a| a.end == at);
                    if *from >= pc || !(*from == 0 || boundary(*from)) || !boundary(pc + 1) {
                        return unsupported("repeat this process: from the start of an act back, at the end of an act");
                    }
                    // the condition before it is what stops it: the repeat ends that block
                    if !instrs[*from..pc].iter().any(|i| matches!(i, Instr::Branch { end, .. } if *end == pc + 1)) {
                        return unsupported("repeat this process: must end a conditional block of the process");
                    }
                    let at_entry = entry_kinds.as_ref().expect("the entry precedes the repeat");
                    let made: Vec<Slot> = (0..kinds.len()).filter(|s| kinds[*s].is_some() && at_entry.get(*s).copied().flatten().is_none()).collect();
                    let mut forgotten = locals.clone();
                    forgotten.sort_unstable();
                    if made != forgotten { return unsupported("repeat this process: forgets exactly what one time through made"); }
                    escaped = forgotten;
                    dead = true;
                }
                Instr::Reflexive { when, each, program } => {
                    let mut scalars = when.scalars();
                    if let Some(each) = each { each.members.scalars(&mut scalars); }
                    for scalar in scalars {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    // the member it triggers for is held as it is queued
                    let member = each.as_ref().map(|each| each.member);
                    for (slot, kind) in program.initial.iter().enumerate() {
                        if Some(slot) == member { continue; }
                        if let Some(kind) = kind { need(&kinds, slot, *kind)?; }
                    }
                }
                Instr::Delayed { subjects, program, .. } => {
                    for scalar in subjects.iter().flat_map(action::Operand::scalars) { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    for (slot, kind) in program.initial.iter().enumerate() { if let Some(kind) = kind { need(&kinds, slot, *kind)?; } }
                }
                Instr::Branch { condition, end, remember, skipped_values } => {
                    if *end <= pc || *end > instrs.len() || !acts.iter().any(|a| a.end == *end) {
                        return unsupported("conditional branch must go forward to an act boundary");
                    }
                    for scalar in condition.scalars() {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    if let Some(out) = remember { make(&mut kinds, *out, Kind::Consent)?; }
                    let mut skipped = kinds.clone();
                    for (slot, value) in skipped_values {
                        let kind = skipped_kind(value)?;
                        make(&mut skipped, *slot, kind)?;
                    }
                    edge(&mut incoming, *end, &skipped);
                }
                Instr::ControlTurn { player, out } => {
                    for scalar in player.scalars() { if let Scalar::Local { slot, kind }=scalar { need(&kinds,slot,kind)?; } }
                    make(&mut kinds,*out,Kind::Turn)?;
                }
                Instr::ExtraTurns { players, count, after, out } => {
                    for scalar in players.scalars().into_iter().chain(count.scalars()) { if let Scalar::Local { slot, kind }=scalar { need(&kinds,slot,kind)?; } }
                    if let Some(slot)=after { need(&kinds,*slot,Kind::Turn)?; }
                    make(&mut kinds,*out,Kind::CreatedTurns)?;
                }
                Instr::RuleDuringCreatedTurns { turns, .. } => need(&kinds,*turns,Kind::CreatedTurns)?,
                Instr::SkipTurns { players,count } => {
                    for scalar in players.scalars().into_iter().chain(count.scalars()) { if let Scalar::Local { slot,kind } = scalar { need(&kinds,slot,kind)?; } }
                }
                Instr::Skip { players, .. } => {
                    for scalar in players.scalars() { if let Scalar::Local { slot, kind }=scalar { need(&kinds,slot,kind)?; } }
                }
                Instr::AddPhases { count, .. } => {
                    for scalar in count.scalars() { if let Scalar::Local { slot, kind }=scalar { need(&kinds,slot,kind)?; } }
                }
                Instr::Copy { objects, from, .. } => {
                    for scalar in objects.iter().chain(std::iter::once(from)).flat_map(action::Operand::scalars) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::Continuous { objects, changes, each, .. } => {
                    for scalar in objects.iter().flat_map(action::Operand::scalars)
                        .chain(changes.iter().flat_map(change::Change::scalars))
                        .chain(each.iter().flat_map(expr::Amount::scalars)) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::Register { subject, rule, .. } => {
                    for scalar in subject.scalars().into_iter().chain(rule.scalars()) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::Search { action, by, whose, out, .. } => {
                    who(&kinds, *by)?;
                    for scalar in action.scalars().into_iter().chain(whose.iter().flat_map(action::Operand::scalars)) { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *out, Kind::Subjects)?;
                }
                Instr::Shuffle { players, found } => {
                    for scalar in players.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    if let Some(slot) = found { need(&kinds, *slot, Kind::Subjects)?; }
                }
                Instr::Place { objects, chooser, attempt, .. } => {
                    for scalar in objects.iter().chain(chooser.iter()).flat_map(action::Operand::scalars) { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    if let Some(attempt) = attempt {
                        for scalar in attempt.actor.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                        make(&mut kinds, attempt.paid, Kind::Consent)?; make(&mut kinds, attempt.possible, Kind::Consent)?;
                    }
                }
                Instr::Pay { payer, each, paid, .. } => {
                    for scalar in payer.scalars().into_iter().chain(each.iter().flat_map(expr::Amount::scalars)) { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *paid, Kind::Consent)?;
                }
                Instr::Permit { player, cards, .. } => {
                    for scalar in player.scalars().into_iter().chain(cards.iter().flat_map(action::Operand::scalars)) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::Cast { player, cards, offered, cast, possible, ceiling, from, .. } => {
                    if let CastOrigin::Produced(slot) | CastOrigin::Retained { moved: slot, .. } = from {
                        let kind = instrs[..pc].iter().find_map(|i| match i { Instr::MoveTogether { entered, .. } if entered.iter().any(|(_,out)|out==slot) => Some(Kind::Cards), _ => i.cast_result().filter(|(out, _)| out == slot).map(|(_, kind)| kind) })
                            .ok_or_else(|| crate::Unsupported("cast origin does not name an earlier successful card producer".into()))?;
                        need(&kinds, *slot, kind)?;
                    }
                    if let CastOrigin::Retained { original, moved } = from {
                        need(&kinds, *original, Kind::Subjects)?;
                        if !instrs[..pc].iter().any(|instruction| match instruction {
                            Instr::MoveTogether {entered,..}=>entered.contains(&(*original,*moved)),
                            Instr::Action {action:action::Action::Move {groups,..},created:Some(out),..}=>out==moved && groups.iter().any(|(objects,_)|objects.contains(&action::Operand::Reference(*original))),
                            _=>false,
                        }) {
                            return unsupported("retained cast origin does not name its exact movement input");
                        }
                    }
                    for scalar in player.scalars().into_iter().chain(cards.iter().flat_map(action::Operand::scalars)) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    if let Some(slot) = ceiling { need(&kinds, *slot, Kind::Number)?; }
                    make(&mut kinds, *offered, Kind::Cards)?;
                    make(&mut kinds, *cast, Kind::Consent)?; make(&mut kinds, *possible, Kind::Consent)?;
                }
                Instr::ChangeTargets { object, .. } => {
                    for scalar in object.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                }
                Instr::CopyCards { cards, times, out } | Instr::CopyStack { cards, times, out, .. } => {
                    let mut scalars = cards.scalars(); scalars.extend(times.scalars());
                    for scalar in scalars { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *out, Kind::Subjects)?;
                }
                Instr::Observe { objects, out, .. } | Instr::Gather { objects, out } => {
                    if let Instr::Observe { by, .. } = i { who(&kinds, *by)?; }
                    for scalar in objects.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *out, Kind::Cards)?;
                }
                Instr::Subjects { cards, out, .. } => { need(&kinds, *cards, Kind::Cards)?; make(&mut kinds, *out, Kind::Subjects)?; }
                Instr::Input { out, .. } => make(&mut kinds, *out, Kind::Subjects)?,
                Instr::Capture { operand, out } => {
                    for scalar in operand.scalars() {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    if let action::Operand::Targets(index) = operand {
                        if *index >= targets.len() { return unsupported("capture reads an undeclared target group"); }
                    }
                    make(&mut kinds, *out, Kind::Subjects)?;
                }
                Instr::Choice { action, paid, possible, chosen, outcomes, actual, discarded_before, .. } => {
                    for scalar in action.scalars() {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    for operand in action.operands() {
                        if let action::Operand::Targets(index) = operand {
                            if *index >= targets.len() { return unsupported("choice reads an undeclared target group"); }
                        }
                    }
                    if let Some(out) = chosen { make(&mut kinds, *out, Kind::Subjects)?; }
                    if let Some(outputs) = outcomes { for out in outputs { make(&mut kinds, *out, Kind::Subjects)?; } }
                    if let Some(out) = actual { make(&mut kinds, *out, Kind::Cards)?; }
                    if let Some(out) = discarded_before { make(&mut kinds, *out, Kind::Cards)?; }
                    make(&mut kinds, *paid, Kind::Consent)?;
                    make(&mut kinds, *possible, Kind::Consent)?;
                }
                Instr::Alternative { actions, options } => {
                    if actions.len() < 2 || actions.len() != options.len() { return unsupported("instruction alternatives require matching options"); }
                    for (index, (action, option)) in actions.iter().zip(options).enumerate() {
                        action.check()?;
                        if option.index != index || !matches!(action, action::Action::Create { players: action::Operand::Controller, .. })
                            || !matches!(option.body, crate::state::ActionOptionBody::Create { .. }) || !action.scalars().is_empty() {
                            return unsupported("only controller literal token alternatives are implemented");
                        }
                    }
                }
                Instr::Action { action, created, attempt } => {
                    action.check()?;
                    for scalar in action.scalars() {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                    for operand in action.operands() {
                        if let action::Operand::Targets(index) = operand {
                            if *index >= targets.len() { return unsupported("instruction reads an undeclared target group"); }
                        }
                    }
                    if let Some(attempt) = attempt {
                        for scalar in attempt.actor.scalars() {
                            if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                        }
                        make(&mut kinds, attempt.paid, Kind::Consent)?;
                        make(&mut kinds, attempt.possible, Kind::Consent)?;
                        if let Some(count) = &attempt.count { make(&mut kinds, count.out, Kind::Number)?; }
                    }
                    if let Some(out) = created {
                        let Some(result) = action.output() else { return unsupported("action does not produce this result kind"); };
                        make(&mut kinds, *out, result.kind())?;
                    }
                }
                Instr::Proliferate { by } => who(&kinds, *by)?,
                Instr::Connive { objects } | Instr::Explore { objects } => {for scalar in objects.scalars() {if let Scalar::Local{slot,kind}=scalar {need(&kinds,slot,kind)?;}}}
                Instr::Endure { permanent, amount } => {
                    for scalar in permanent.scalars().into_iter().chain(amount.scalars()) { if let Scalar::Local { slot,kind }=scalar { need(&kinds,slot,kind)?; } }
                }
                Instr::Arrange { players, amount, .. } => {
                    for scalar in players.scalars().into_iter().chain(amount.scalars()) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::Mana { players, production } => {
                    for scalar in players.scalars().into_iter().chain(production.scalars()) {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::SetX(amount) => {
                    for scalar in amount.scalars() {
                        if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; }
                    }
                }
                Instr::SkipUnless { consent, count } => {
                    need(&kinds, *consent, Kind::Consent)?;
                    let end = pc.checked_add(*count + 1).ok_or_else(|| crate::Unsupported("branch overflow".into()))?;
                    let skipped = instrs.get(pc + 1..end).ok_or_else(|| crate::Unsupported("branch past program".into()))?;
                    if skipped.iter().any(|i| !matches!(i, Instr::Action { .. } | Instr::ChangeTargets { .. } | Instr::Proliferate { .. } | Instr::Choice { .. } | Instr::Connive { .. } | Instr::Explore { .. } | Instr::Endure { .. } | Instr::Arrange { .. } | Instr::Place { .. } | Instr::Shuffle { .. } | Instr::Observe { .. } | Instr::Gather { .. } | Instr::Mana { .. } | Instr::Capture { .. } | Instr::Input { .. } | Instr::Register { .. } | Instr::Continuous { .. } | Instr::Copy { .. } | Instr::ControlTurn { .. } | Instr::ExtraTurns { .. } | Instr::RuleDuringCreatedTurns { .. } | Instr::SkipTurns { .. } | Instr::Skip { .. } | Instr::AddPhases { .. } | Instr::SetX(_)
                        // a manifest or a manifest dread: the cards it takes, looks at, chooses and puts
                        | Instr::TakeTop { .. } | Instr::Subjects { .. } | Instr::Show { .. } | Instr::Select { gate: None, .. } | Instr::Remainder { .. } | Instr::Move { .. } | Instr::MoveTogether { .. })) {
                        return unsupported("optional block contains an unsupported instruction");
                    }
                    if !acts.iter().any(|a| a.end == end) { return unsupported("optional block must end at its act boundary"); }
                    let mut empty=kinds.clone();
                    for (slot,value) in skipped.iter().flat_map(Instr::skipped_result) { make(&mut empty,slot,skipped_kind(&value)?)?; }
                    edge(&mut incoming, end, &empty);
                }
                Instr::Choose { kind, out, .. } => {
                    if !matches!(kind.kind, Kind::CardType | Kind::CreatureType | Kind::Color | Kind::CardName) {
                        return unsupported("choice has no supported scalar domain");
                    }
                    make(&mut kinds, *out, kind.kind)?;
                }
                Instr::FlipCoin { out } => make(&mut kinds, *out, Kind::Coin)?,
                Instr::LibraryScan { by, amount, limit: _, all, hit, designated, bound } => {
                    who(&kinds, *by)?;
                    for scalar in amount.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    for slot in [all, hit, designated] { make(&mut kinds, *slot, Kind::Cards)?; }
                    make(&mut kinds, *bound, Kind::Number)?;
                }
                Instr::Discovered { by, bound } => { who(&kinds, *by)?; need(&kinds, *bound, Kind::Number)?; }
                Instr::TakeUntil { set, whose, out, matched } => {
                    for scalar in whose.scalars().into_iter().chain(set.scalars()) { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *out, Kind::Cards)?;
                    make(&mut kinds, *matched, Kind::Cards)?;
                }
                Instr::TakeTop { n, whose, out } => {
                    for scalar in whose.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    for scalar in n.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *out, Kind::Cards)?;
                },
                Instr::Show { from, out, .. } => {
                    need(&kinds, *from, Kind::Cards)?;
                    make(&mut kinds, *out, Kind::Cards)?;
                }
                Instr::Exile { from, out, hidden } => {
                    need(&kinds, *from, Kind::Cards)?;
                    for looker in hidden.iter().flat_map(|h| &h.lookers) { who(&kinds, *looker)?; }
                    make(&mut kinds, *out, Kind::Cards)?;
                }
                Instr::Gate { who: by, probe, out, possible } => {
                    who(&kinds, *by)?;
                    if let Some(p) = probe {
                        need(&kinds, p.from, Kind::Cards)?;
                        for operand in p.set.scalars().into_iter().chain(p.count.scalars()) {
                            if let Scalar::Local { slot, kind } = operand { need(&kinds, slot, kind)?; }
                        }
                    }
                    make(&mut kinds, *out, Kind::Consent)?;
                    if let Some(slot) = possible { make(&mut kinds, *slot, Kind::Consent)?; }
                }
                Instr::Select { from, set, gate, out, chooser, pick, .. } => {
                    who(&kinds, *chooser)?;
                    for operand in set.scalars().into_iter().chain(match pick { Pick::Quantity(n) => n.scalars(), _ => Vec::new() }) {
                        if let Scalar::Local { slot, kind } = operand { need(&kinds, slot, kind)?; }
                    }
                    need(&kinds, *from, Kind::Cards)?;
                    if let Some(g) = gate {
                        need(&kinds, *g, Kind::Consent)?;
                    }
                    make(&mut kinds, *out, Kind::Cards)?;
                    if let Pick::Optional { count, paid, possible } = pick {
                        if *count != 1 || gate.is_some() { return unsupported("optional object selection: one ungated object"); }
                        make(&mut kinds, *paid, Kind::Consent)?;
                        make(&mut kinds, *possible, Kind::Consent)?;
                    }
                }
                Instr::Remainder { of, minus, out, .. } => {
                    need(&kinds, *of, Kind::Cards)?;
                    for m in minus {
                        need(&kinds, *m, Kind::Cards)?;
                    }
                    make(&mut kinds, *out, Kind::Cards)?;
                }
                Instr::Piles { from, chooser, one, other, .. } => {
                    need(&kinds, *from, Kind::Cards)?;
                    for scalar in chooser.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } }
                    make(&mut kinds, *one, Kind::Cards)?;
                    make(&mut kinds, *other, Kind::Cards)?;
                }
                Instr::MoveTogether { parts, entered } => {
                    for (src, to) in parts {
                        need(&kinds, *src, if matches!(to,Dest::Battlefield(_)) {Kind::Subjects}else{Kind::Cards})?;
                        if let Dest::Battlefield(to) = to {
                            if !matches!(to, action::Destination::Battlefield { .. }) { return unsupported("partition entry requires a battlefield destination"); }
                            let action = action::Action::Move { groups: vec![(vec![action::Operand::Reference(*src)],to.clone())], said: ir::action::MoveVerb::Put };
                            action.check()?;
                            for scalar in action.scalars() { if let Scalar::Local {slot,kind}=scalar {need(&kinds,slot,kind)?;} }
                        }
                    }
                    for (src,out) in entered {
                        if !parts.iter().any(|(input,to)|input==src && matches!(to,Dest::Battlefield(_))) { return unsupported("partition result requires its exact entry input"); }
                        make(&mut kinds,*out,Kind::Cards)?;
                    }
                }
                Instr::Move {to:Dest::Battlefield(_),..} => return unsupported("battlefield result moves require a declared entry output"),
                Instr::Move { src, by, when, .. } => { who(&kinds, *by)?; need(&kinds, *src, Kind::Cards)?; if let Some(condition) = when { for scalar in condition.scalars() { if let Scalar::Local { slot, kind } = scalar { need(&kinds, slot, kind)?; } } } }
            }
        } Ok(()) })();
        if let Err(u) = checked {
            // "… and repeat this process. You gain life equal to its mana value": which time's card?
            if escaped.iter().any(|slot| u.0 == format!("slot {slot} is read before any instruction produces it")) {
                return unsupported("repeat this process: what one time through it made is not known after it");
            }
            return Err(u);
        }
        let mut last = 0;
        for a in &acts {
            if a.end < last || a.end > instrs.len() {
                return unsupported("act boundaries out of order");
            }
            last = a.end;
        }
        if last != instrs.len() {
            return unsupported("instructions after the last act");
        }
        Ok(Program { action_limit: false, announcement: Default::default(), id: ProgramId(0), slots: kinds.len(), initial, kinds, instrs, acts, targets, text: None }.sealed())
    }

}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instr::Choice { action, paid, possible, .. } => write!(f, "s{paid}, s{possible} = choose {action:?}"),
            Instr::Reflexive { when, each: None, program } => write!(f, "when {when:?}, queue program #{}", program.id),
            Instr::Reflexive { each: Some(each), program, .. } => write!(f, "for each of {:?} as s{}, queue program #{}", each.members, each.member, program.id),
            Instr::Delayed { program, .. } => write!(f, "register delayed program #{}", program.id),
            Instr::Alternative { options, .. } => write!(f, "choose instruction {options:?}"),
            Instr::Action { action, created, .. } => write!(f, "{action:?} -> {created:?}"),
            Instr::Input { input, out } => write!(f, "s{out} = input {input:?}"),
            Instr::Capture { operand, out } => write!(f, "s{out} = capture {operand:?}"),
            Instr::Register { subject, rule, lifetime } => write!(f, "keep {subject:?}: {rule:?}; {lifetime:?}"),
            Instr::Proliferate { by } => write!(f, "{by:?} proliferate"),
            Instr::Connive { objects } => write!(f,"{objects:?} connive"),
            Instr::Explore { objects } => write!(f,"{objects:?} explore"),
            Instr::Arrange { players, verb, amount, .. } => write!(f, "{players:?} {verb:?} {amount:?}"),
            Instr::Place { objects, position, order, .. } => write!(f, "place {objects:?} {position:?} {order:?}"),
            Instr::Pay { payer, cost, each, paid } => write!(f, "s{paid} = {payer:?} may pay {cost:?} each {each:?}"),
            Instr::Permit { player, cards, play, until } => write!(f, "{player:?} may {} {cards:?} until {until:?}", if *play { "play" } else { "cast" }),
            Instr::Cast { player, cards, offered, costs, cast, possible, from, rider, .. } => write!(f, "#{offered}, s{cast}, s{possible} = {player:?} casts of {cards:?} from {from:?} now, for {costs:?}, carrying {rider:?}, or declines"),
            Instr::ChangeTargets { object, policy } => write!(f, "change {object:?} targets: {policy:?}"),
            Instr::CopyStack { cards, times, retarget, out } => write!(f, "s{out} = {times:?} stack copies of {cards:?}, new targets={retarget}"),
            Instr::CopyCards { cards, times, out } => write!(f, "s{out} = {times:?} copies of each of {cards:?}"),
            Instr::Search { action, by, whose, own: false, out } => write!(f, "s{out} = {by:?} searches {action:?} in {whose:?}"),
            Instr::Search { action, whose, own: true, out, .. } => write!(f, "s{out} = each of {whose:?} searches their own: {action:?}"),
            Instr::Shuffle { players, found } => write!(f, "shuffle {players:?}, aside {found:?}"),
            Instr::Gather { objects, out } => write!(f, "s{out} = gather {objects:?}"),
            Instr::Observe { objects, by, out } => write!(f, "s{out} = {by:?} reveals {objects:?}"),
            Instr::Subjects { cards, out, still_in } => { write!(f, "s{out} = subjects of s{cards}")?; if let Some(zone) = still_in { write!(f, " in {zone:?}")?; } Ok(()) },
            Instr::Mana { players, production } => write!(f, "{players:?} add {production:?}"),
            Instr::ControlTurn { player, out } => write!(f,"s{out} = control {player:?} during their next turn"),
            Instr::Endure { permanent, amount } => write!(f,"{permanent:?} endures {amount:?}"),
            Instr::ExtraTurns { players, count, after, out } => write!(f,"s{out} = {players:?} take {count:?} extra turns after {after:?}"),
            Instr::RuleDuringCreatedTurns { turns, rule } => write!(f,"during turns s{turns}: {rule:?}"),
            Instr::SkipTurns { players,count } => write!(f,"{players:?} skip next {count:?} turns"),
            Instr::Skip { players, what, this_turn } => write!(f, "{players:?} skip their next {what:?}; this_turn={this_turn}"),
            Instr::AddPhases { after, kinds, count } => write!(f, "{count:?} times {kinds:?} after {after:?}"),
            Instr::Copy { objects, from, turn } => write!(f, "copy {from:?} onto {objects:?}; until_end_of_turn={turn}"),
            Instr::Continuous { objects, changes, duration, .. } => write!(f, "change {objects:?}: {changes:?}; duration={duration:?}"),
            Instr::Branch { condition, end, .. } => write!(f, "unless {condition:?}, jump {end}"),
            Instr::SetX(amount) => write!(f, "X = {amount:?}"),
            Instr::SkipUnless { consent, count } => write!(f, "unless s{consent}, skip {count}"),
            Instr::Choose { kind, who, out } => write!(f, "s{out} = choose {kind:?} by {who:?}"),
            Instr::FlipCoin { out } => write!(f, "s{out} = called coin flip by controller"),
            Instr::LibraryScan { by, amount, limit, all, hit, designated, bound } => write!(f, "s{all}, s{hit}, s{designated}, s{bound} = {by:?} scans {limit:?} {amount:?}"),
            Instr::Discovered { by, bound } => write!(f, "{by:?} completed discover s{bound}"),
            Instr::TakeUntil { set, whose, out, matched } => write!(f, "s{out}, s{matched} = take_until {set:?} of {whose:?}"),
            Instr::TakeTop { n, whose, out } => write!(f, "s{out} = take_top {n:?} of {whose:?}"),
            Instr::Exile { from, out, hidden: None } => write!(f, "s{out} = exile s{from}"),
            Instr::Exile { from, out, hidden: Some(hide) } => write!(f, "s{out} = exile s{from} face down, looked at by {:?}{}", hide.lookers, if hide.watched { ", watched" } else { "" }),
            Instr::Show { from, by, to, out } => write!(f, "s{out} = show s{from} by {by:?} to {to:?}"),
            Instr::Gate { who, probe, out, possible: _ } => {
                write!(f, "s{out} = gate {who:?}")?;
                if let Some(p) = probe {
                    write!(f, " unless fewer than {:?} of s{} fit", p.count, p.from)?;
                }
                Ok(())
            }
            Instr::Select { from, pick, chooser, gate, out, .. } => {
                write!(f, "s{out} = select {pick:?} of s{from} chosen by {chooser:?}")?;
                if let Some(g) = gate {
                    write!(f, " if s{g}")?;
                }
                Ok(())
            }
            Instr::Remainder { of, minus, out, .. } => {
                let minus: Vec<String> = minus.iter().map(|m| format!("s{m}")).collect();
                write!(f, "s{out} = remainder of s{of} minus [{}]", minus.join(", "))
            }
            Instr::Piles { from, opponent, chooser, one, other, .. } => {
                write!(f, "s{one}, s{other} = the pile chosen by {chooser:?} and the other of s{from}, separated by {}", if *opponent { "an opponent" } else { "the controller" })
            }
            Instr::MoveTogether { parts, entered } => write!(f, "move partitions {parts:?} together; entry results {entered:?}"),
            Instr::Move { src, to, by, when } => write!(f, "move s{src} to {to:?} by {by:?}, when {when:?}"),
            Instr::Repeat { from, locals } => write!(f, "forget {locals:?}, repeat from {from}"),
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "program #{}", self.id)?;
        for (pc, i) in self.instrs.iter().enumerate() {
            let end = self.acts.iter().any(|a| a.end == pc + 1);
            writeln!(f, "  {pc:>2}: {i}{}", if end { "   ; end of act" } else { "" })?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- references and recognition

pub use ir::reference::{Qualifier, RefForm, UnresolvedRef};

/// A phrase that takes some of a referred group: the reference, how many it takes and the
/// description of the cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub reference: UnresolvedRef,
    pub taking: Taking,
    pub set: ObjectSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taking {
    Quantity(Amount),
    AnyNumber,
    All,
    Exactly(usize),
    UpTo(usize),
    Remainder,
}

/// Extract the selection surrounding a typed reference emitted by the grammar.
/// Producer resolution belongs to the binder; surface forms do not appear here.
/// Where an object an instruction says "a"/"an" of is chosen from as it is followed
/// (608.2d): a permanent, or a card in a graveyard. Other zones, and "another", are not
/// lowered here.
fn chosen_on_resolution(set: &ObjectSet) -> Option<Zone> {
    if set.other { return None; }
    match (&set.domain, &set.zone) {
        (Domain::Permanent, None) => Some(Zone::Battlefield),
        (Domain::Card, Some(zone)) if zone.zones.items == [ir::Zone::Graveyard] && zone.slice.is_none() => Some(Zone::Graveyard),
        _ => None,
    }
}

pub fn mention(s: &Selector) -> Option<Mention> {
    // "one" elides its noun: select one member of the uniquely bound card group.
    // The binder refuses absent/ambiguous groups; this never means a new target.
    if s == &Selector::new(Quant::Definite(Ref::Anaphor(ir::Anaphor::One)),ObjectSet::of(Domain::Object)) {
        return Some(Mention { reference: UnresolvedRef::cards(RefForm::Anaphor), taking: Taking::Exactly(1), set: ObjectSet::of(Domain::Card) });
    }
    let remainder = UnresolvedRef::cards(RefForm::Remainder);
    if s.quant == Quant::Definite(Ref::Unresolved(remainder.clone()))
        || s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::Other { plural: false })) {
        // A qualified partitive such as "the rest of your opponents" is not the
        // unqualified instruction remainder. Never discard its group constraints.
        if s != &Selector::new(s.quant.clone(), ObjectSet::of(Domain::Object)) { return None; }
        return Some(Mention { reference: remainder, taking: Taking::Remainder, set: ObjectSet::of(Domain::Card) });
    }
    let ir::Filter::All(atoms) = &s.set.filter else { return None };
    let mut reference: Option<UnresolvedRef> = None;
    let mut kept = Vec::new();
    for f in atoms {
        match f {
            ir::Filter::Atom(Atom::Reference(r)) if r.expected == Kind::Cards => {
                if let Some(previous) = &mut reference {
                    if r.form == RefForm::ActionResult { previous.form = r.form; }
                    previous.qualifiers.extend(r.qualifiers.clone());
                } else { reference = Some(r.clone()); }
            }
            ir::Filter::Atom(Atom::FromIt) => {
                if reference.is_some() { return None; }
                reference = Some(UnresolvedRef::cards(RefForm::Anaphor));
            }
            // "one of those cards": a partitive of the group just named, as "one of them" is
            ir::Filter::Atom(Atom::Among(of)) if matches!(of.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::Those | ir::Anaphor::Them | ir::Anaphor::The { plural: true })))
                && of.set.domain == Domain::Card && of.set.filter == ir::Filter::All(Vec::new()) && of.chooser.is_none() && of.random.is_none() => {
                if reference.is_some() { return None; }
                reference = Some(UnresolvedRef::cards(RefForm::Anaphor));
            }
            // "from among the milled cards": the group that instruction produced, as "the
            // cards milled this way" names it (608.2c)
            ir::Filter::Atom(Atom::Among(of)) if matches!(of.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { plural: true, .. })))
                && of.set == ObjectSet::of(Domain::Card) && of.chooser.is_none() && of.random.is_none() => {
                let Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what, .. })) = of.quant else { unreachable!() };
                if reference.is_some() { return None; }
                reference = Some(UnresolvedRef { expected: Kind::Cards, form: RefForm::ActionResult, qualifiers: vec![Qualifier::ThisWay(what)] });
            }
            other => kept.push(other.clone()),
        }
    }
    let reference = reference?;
    if !matches!(s.set.domain, Domain::Card | Domain::Object) || s.random.is_some() || s.chooser.is_some() { return None; }
    // A nounless partitive ("one of them") gets its card sort from the typed
    // reference, not from an English noun that is absent. Keep all other constraints.
    let set = ObjectSet { domain: Domain::Card, filter: ir::Filter::All(kept), ..s.set.clone() };
    let taking = match &s.quant {
        Quant::All | Quant::Each => Taking::All,
        Quant::Indefinite(Count::AnyNumber) => Taking::AnyNumber,
        Quant::Indefinite(Count::Exactly(Amount::Const(n))) => Taking::Exactly(*n as usize),
        Quant::Indefinite(Count::UpTo(Amount::Const(n))) => Taking::UpTo(*n as usize),
        Quant::Indefinite(Count::Exactly(n)) => Taking::Quantity(n.clone()),
        _ => return None,
    };
    Some(Mention { reference, taking, set })
}

/// Compile text against a runtime context using the same lowering as card definitions.
pub fn compile(effect: &Effect, game: &Game, ctx: &Ctx) -> Res<Program> {
    lower(effect, &Support::runtime(game, ctx))
}

// ---------------------------------------------------------------- the frame

#[derive(Debug, Clone, PartialEq, Eq)]
enum Request { Action(crate::perform::Action), Place(Vec<ObjId>) }
impl Request {
    fn possible(&self, game: &Game, payment: bool) -> bool { match self { Self::Action(a) => a.possible(game, payment), Self::Place(cards) => !cards.is_empty() } }
    fn retain_players(&mut self, accepted: &[PlayerId]) { if let Self::Action(a) = self { a.retain_players(accepted); } }
    fn draw_count(&mut self, player: PlayerId, count: u32) {
        let Self::Action(crate::perform::Action::Players { kind: crate::perform::PlayerAction::Draw, players }) = self else { unreachable!("numeric attempt owns a draw request") };
        players.iter_mut().find(|(p, _)| *p == player).expect("chooser belongs to retained draw request").1 = count;
    }
}

/// Exactly one child can own this frame's pending decision.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    Operation { result: Option<crate::operation::Completed>, created: Option<Slot> },
    Attempt { request: Box<Request>, players: Vec<PlayerId>, accepted: Vec<PlayerId>, cursor: usize, answer: Option<Answer> },
    Objects { procedure: choice::Pending, published: usize },
    Arrange(arrange::Pending),
    Connive(connive::Pending),
    Explore(explore::Pending),
    LibraryScan(library_scan::Pending),
    Endure(endure::Pending),
    Search(search::Pending),
    Piles(piles::Pending),
    Library(crate::library::Placement),
    Answer { decision: Decision, answer: Option<Answer> },
    Mana(mana::Pending),
    Copies(crate::copy::StackCopies),
    /// The payment this frame started is a child procedure; it reports whether it was made.
    Payment { result: Option<bool> },
}

/// A program being followed: where it is, what it has produced, and the context it was started
/// with (copied, so it never looks up the live source, 113.7a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Successful, legally trackable successors, separate from characteristic/result
    /// bookkeeping that may also retain old or hidden-zone identities (400.7j).
    cast_results: Vec<(Slot, Vec<ObjId>)>,
    cast_origins: Vec<(Slot, ObjId, Zone)>,
    inputs: Vec<(input::Input, Vec<Target>)>,
    event_amount: Option<u32>,
    transitions: Vec<(ObjId, ObjId)>,
    program: Arc<Program>,
    pc: usize,
    slots: Vec<Option<Value>>,
    ctx: Ctx<'static>,
    /// Stack occurrence identity; distinct from the semantic source.
    occurrence: Option<ObjId>,
    /// Unique instruction span within this resolution, including selected modal bodies.
    occurrence_base: usize,
    /// An occurrence result, independent of event-history retention (106.12a).
    produced_mana: bool,
    base_x: Option<u32>,
    /// This ability's announcement groups, independent of other abilities on the spell.
    targets: Vec<Vec<Target>>,
    /// What each target of a divided group gets, as announced (601.2d); empty where nothing
    /// is divided.
    division: Vec<Vec<u32>>,
    /// An evaluated child owns its request and progress until completion.
    pending: Option<Pending>,
    /// The act in progress: where its events begin in the log, and what it may take from the
    /// battlefield (looked back at once the act is done).
    open: Option<(crate::event::Seq, crate::layers::Working)>,
    /// Groups shown, and to whom.
    shown: Vec<(Slot, Audience)>,
    pub(crate) choices: Vec<crate::state::PublicChoice>,
}

impl expr::Locals for Frame {
    fn successor(&self, was: ObjId) -> Option<ObjId> {
        self.transitions.iter().rev().find(|(old, _)| *old == was).map(|(_, now)| *now)
    }
    fn revealed(&self, o: ObjId) -> bool {
        self.exploring() == Some(o) || self.shown.iter().any(|(slot, audience)| matches!(audience, Audience::Everyone) && match self.slot(*slot) {
            Some(Value::Cards(cards)) => cards.contains(&o),
            Some(Value::Subjects(subjects)) => subjects.contains(&Target::Object(o)),
            _ => false,
        })
    }
    fn targets(&self, group:usize)->Option<&[Target]>{self.targets.get(group).map(Vec::as_slice)}
    fn local(&self, slot: usize) -> Option<&Value> { self.slot(slot) }
    fn event_amount(&self) -> Option<u32> { self.event_amount }
    fn input(&self, key: input::Input) -> Option<&[Target]> {
        self.inputs.iter().find(|(input, _)| *input == key).map(|(_, value)| value.as_slice())
    }
}

impl Frame {
    fn concealed_pool(&self, game: &Game, chooser: PlayerId, cards: impl IntoIterator<Item = ObjId>) -> bool {
        game.concealed_pool(chooser, cards.into_iter().filter(|o| !expr::Locals::revealed(self, *o)))
    }
    pub fn new(program: Arc<Program>, ctx: Ctx<'static>) -> Frame {
        Frame { cast_results: Vec::new(), cast_origins: Vec::new(), occurrence: None, occurrence_base: 0, produced_mana: false, event_amount: None, inputs: Vec::new(), transitions: Vec::new(), slots: vec![None; program.slots], program, pc: 0, base_x: ctx.x, ctx, targets: Vec::new(), division: Vec::new(), pending: None, open: None, shown: Vec::new(), choices: Vec::new() }
    }

    /// Supply this invocation's already-announced target groups. Announcement legality is
    /// owned by the game driver; each operation rechecks its targets on resolution (608.2b).
    /// Low-level callers of `run` must supply these for a targeted program.
    pub fn set_targets(&mut self, targets: &[Vec<Target>]) -> Res<()> {
        if targets.len() != self.program.targets.len() {
            return unsupported("compiled invocation has the wrong number of target groups");
        }
        self.targets = targets.to_vec();
        Ok(())
    }

    pub fn program_id(&self) -> ProgramId {
        self.program.id
    }

    pub fn pc(&self) -> usize {
        self.pc
    }

    pub fn finished(&self) -> bool {
        self.pc >= self.program.instrs.len()
    }

    /// The value a slot holds; None while nothing has produced it.
    pub fn slot(&self, slot: Slot) -> Option<&Value> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// The answer to the choice the frame waits for.
    pub fn answer(&mut self, answer: Answer) {
        match &mut self.pending {
            Some(Pending::Attempt { answer: slot, .. }) => *slot = Some(answer),
            Some(Pending::Objects { procedure, .. }) => procedure.answer(answer),
            Some(Pending::Connive(procedure)) => procedure.answer(answer),
            Some(Pending::Explore(procedure)) => procedure.answer(answer),
            Some(Pending::Endure(procedure)) => procedure.answer(answer),
            Some(Pending::Arrange(procedure)) => procedure.answer(answer),
            Some(Pending::Search(procedure)) => procedure.answer(answer),
            Some(Pending::Piles(procedure)) => procedure.answer(answer),
            Some(Pending::Library(procedure)) => {
                let published = procedure.choices().len();
                procedure.answer(answer);
                self.choices.extend_from_slice(&procedure.choices()[published..]);
            },
            Some(Pending::Mana(procedure)) => procedure.answer(answer),
            Some(Pending::Copies(procedure)) => procedure.answer(answer),
            Some(Pending::Operation { .. }) | Some(Pending::LibraryScan(_)) => unreachable!("the operation owns this answer"),
            Some(Pending::Payment { .. }) => unreachable!("the payment owns this answer"),
            Some(Pending::Answer { answer: slot, .. }) => *slot = Some(answer),
            None => panic!("a frame accepts an answer only while awaiting a decision"),
        }
    }

    /// May this player see this object because the frame showed it to them?
    pub fn shown_to(&self, game: &Game, viewer: PlayerId, o: ObjId) -> bool {
        self.exploring() == Some(o) || self.shown.iter().any(|(slot, audience)| {
            let allowed = match audience {
                Audience::Everyone => true,
                Audience::Only(who) => self.actor(*who) == Some(viewer),
                Audience::Owners => game.obj(o).owner == viewer,
            };
            allowed && match self.slot(*slot) {
                Some(Value::Cards(cards)) => cards.contains(&o),
                Some(Value::Subjects(subjects)) => subjects.contains(&Target::Object(o)),
                _ => false,
            }
        })
    }

    /// The complete privately looked-at top group, put back in any order by
    /// its looker. This is an arrange instruction even in another player's library.
    pub(crate) fn library_arrangement(&self, game:&Game, player:PlayerId, cards:&[ObjId]) -> Option<ObjId> {
        let Instr::Place { objects, position:ir::Position::One(ir::action::LibraryPosition::Top),
            order:Some(Order::Any), by:Some(by), before_shuffle:false, .. } = self.program.instrs.get(self.pc)? else {return None};
        let [action::Operand::Reference(placed)] = objects.as_slice() else {return None};
        let shown=self.program.instrs[..self.pc].iter().rev().find_map(|instr|match instr {
            Instr::Subjects {cards,out,still_in:None} if out==placed => Some(*cards), _=>None,
        }).unwrap_or(*placed);
        let (index,from,looker)=self.program.instrs[..self.pc].iter().enumerate().rev().find_map(|(i,instr)|match instr {
            Instr::Show {from,by:looker,to:Audience::Only(to),out} if *out==shown && looker==to => Some((i,*from,*looker)), _=>None,
        })?;
        if looker!=*by || self.actor(looker)!=Some(player)
            || !matches!(self.program.instrs.get(index.checked_sub(1)?),Some(Instr::TakeTop {out,..}) if *out==from) {return None;}
        let group=self.cards(shown);
        (group.len()==cards.len() && group.iter().all(|c|cards.contains(c) && game.obj(*c).zone==Zone::Library))
            .then(||self.ctx.object_source())
    }

    /// What the "may" this frame asks lets its player do (608.2d): explore's putting the
    /// revealed card into the graveyard (701.44a); the action done to the objects the
    /// consent lets them choose ("you may put a permanent card from among them into your
    /// hand"); what the instructions a "you may" governs do ("you may search your library");
    /// or the optional action itself ("you may draw a card", "unless you discard a creature
    /// card"). None when no "may" is asked, or it is of an action no verb names.
    pub(crate) fn optional(&self) -> Option<(ir::Verb, Option<crate::state::Placement>)> {
        use crate::state::Placement;
        let instrs = &self.program.instrs;
        match &self.pending {
            Some(Pending::Explore(procedure)) if procedure.revealed().is_some() =>
                Some((ir::Verb::Move, Some(Placement::Zone(Zone::Graveyard)))),
            Some(Pending::Answer { decision: Decision::Choose { choice: Choice::May { .. }, .. }, answer: None }) => {
                let Some(Instr::Gate { out: consent, .. }) = instrs.get(self.pc) else { return None };
                match instrs.iter().enumerate().skip(self.pc + 1).find_map(|(i, instr)| match instr {
                    Instr::Select { gate: Some(gate), out, .. } if gate == consent => Some((i, *out)),
                    _ => None,
                }) {
                    Some((i, selected)) => purpose::chosen_for(instrs, i, selected),
                    None => purpose::gated(instrs, self.pc),
                }
            }
            // an optional action, choice of objects or arrangement asks as it is followed
            Some(Pending::Attempt { .. } | Pending::Objects { .. } | Pending::Arrange(_)) => purpose::done_by(instrs.get(self.pc)?),
            _ => None,
        }
    }

    /// Whether the text says "return" or "put" of the objects the pending choice is of.
    pub(crate) fn moved_as(&self) -> Option<ir::action::MoveVerb> {
        match self.program.instrs.get(self.pc)? {
            Instr::Select { out, .. } => purpose::moved_as(&self.program.instrs, self.pc, *out),
            Instr::Choice { action, .. } if action.verb() == choice::Verb::Return => Some(ir::action::MoveVerb::Return),
            _ => None,
        }
    }

    /// The card an explore under way has revealed (701.44a, 701.20a).
    fn exploring(&self) -> Option<ObjId> {
        if let Some(Pending::Explore(procedure)) = &self.pending { procedure.revealed() } else { None }
    }
    /// Is it waiting for cards to be separated into piles (700.3)?
    pub(crate) fn separating(&self) -> bool {
        matches!(&self.pending, Some(Pending::Piles(piles)) if piles.separating())
    }

    pub(crate) fn known_cards(&self) -> Vec<ObjId> {
        self.exploring().into_iter().chain(self.shown.iter().flat_map(|(slot, _)| match self.slot(*slot) {
            Some(Value::Cards(cards)) => cards.clone(),
            Some(Value::Subjects(subjects)) => subjects.iter().filter_map(|t| if let Target::Object(o) = t { Some(*o) } else { None }).collect(),
            _ => Vec::new(),
        })).collect()
    }

    /// The instructions with the position marked, and the slots: for failing tests.
    pub fn dump(&self) -> String {
        let mut out = format!("frame of program #{} at {} of {}\n", self.program.id, self.pc, self.program.instrs.len());
        for (pc, i) in self.program.instrs.iter().enumerate() {
            out += &format!("{} {pc:>2}: {i}\n", if pc == self.pc { ">" } else { " " });
        }
        for (s, v) in self.slots.iter().enumerate() {
            match v {
                None => out += &format!("  s{s} = (not produced)\n"),
                Some(Value::Cards(c)) => out += &format!("  s{s} = {:?}\n", c.iter().map(|o| o.0).collect::<Vec<_>>()),
                Some(value) => out += &format!("  s{s} = {value:?}\n"),
            }
        }
        out
    }

    fn cards(&self, slot: Slot) -> Vec<ObjId> {
        match self.slot(slot) {
            Some(Value::Cards(c)) => c.clone(),
            _ => unreachable!("Program::new checked that the slot holds cards"),
        }
    }

    fn ask(&mut self, player: PlayerId, choice: Choice) -> Result<Answer, Halt> {
        match self.pending.take() {
            Some(Pending::Answer { answer: Some(answer), .. }) => Ok(answer),
            Some(Pending::Answer { decision, answer: None }) => {
                self.pending = Some(Pending::Answer { decision: decision.clone(), answer: None });
                Err(Halt::Ask(decision))
            }
            None => {
                let decision = Decision::Choose { player, choice };
                self.pending = Some(Pending::Answer { decision: decision.clone(), answer: None });
                Err(Halt::Ask(decision))
            }
            _ => unreachable!("instruction resumes its own decision"),
        }
    }

    fn operation_result(&mut self, game: &Game, mut result: crate::operation::Completed, created: Option<Slot>) {
        if let Instr::MoveTogether { entered, .. } = &self.program.instrs[self.pc] {
            for (src,out) in entered {
                let original:Vec<_> = match self.slot(*src) {Some(Value::Subjects(subjects))=>subjects.iter().filter_map(|target|match target {Target::Object(card)=>Some(*card),_=>None}).collect(),_=>unreachable!("entry partition has typed subjects")};
                let transitions:Vec<_> = result.transitions.iter().filter(|(old,now)|original.contains(old) && result.arrived.contains(now) && game.obj(*now).zone==Zone::Battlefield).copied().collect();
                self.cast_results.retain(|(slot,_)|slot!=out);
                self.cast_results.push((*out,transitions.iter().map(|(_,now)|*now).collect()));
                self.slots[*out] = Some(Value::Cards(transitions.into_iter().flat_map(|(old,now)|[old,now]).collect()));
            }
        }
        if let Some(slot) = created {
            let arrived = result.transitions.iter().filter_map(|(_, now)| result.arrived.contains(now).then_some(*now)).collect();
            self.cast_results.retain(|(out, _)| *out != slot);
            self.cast_results.push((slot, arrived));
            self.slots[slot] = Some(self.action_output().value(&mut result));
        }
        self.transitions.extend(result.transitions);
    }

    fn action_output(&self) -> action::Output {
        let Instr::Action { action, .. } = &self.program.instrs[self.pc] else {
            unreachable!("only ordinary actions receive operation results")
        };
        action.output().expect("the compiler validates operation result producers")
    }

    fn player(&self, who: Who) -> PlayerId {
        self.actor(who).expect("a clause whose player is gone is not begun (its gate declines)")
    }

    /// The player `who` names, if there is one: a player held in a slot may have left the
    /// game (800.4a), or there was none to name.
    fn actor(&self, who: Who) -> Option<PlayerId> {
        match who {
            Who::Controller => Some(self.ctx.controller),
            Who::Of(slot) => match self.slot(slot) {
                Some(Value::Subjects(said)) => match said.as_slice() {
                    [Target::Player(p)] => Some(*p),
                    _ => None,
                },
                _ => unreachable!("the compiler validates reference producers"),
            },
        }
    }
}

/// Follow the program from where it stopped. None: it is done. Some: it waits for that
/// decision, and nothing of the instruction that asked has happened.
pub fn run(game: &mut Game, f: &mut Frame) -> Res<Option<Decision>> {
    match advance(game, f)? {
        None => Ok(None),
        Some(Paused::Ask(decision)) => Ok(Some(decision)),
        Some(Paused::Child) => unsupported("a child procedure runs under the procedure driver"),
    }
}

/// [`run`], for the procedure driver: the frame may also stop for a child it started.
pub(crate) fn advance(game: &mut Game, f: &mut Frame) -> Res<Option<Paused>> {
    if f.pc == 0 && f.program.action_limit && game.ability_used(&f.ctx, crate::ability_limit::Limit::Action) { return Ok(None); }
    while f.pc < f.program.instrs.len() {
        if f.open.is_none() {
            let act = f.program.acts.iter().find(|a| a.end > f.pc).expect("every instruction is in an act");
            let before = if act.moves { game.departure_snapshot(&game.battlefield) } else { crate::layers::Working::before(Vec::new()) };
            f.open = Some((game.log.end(), before));
        }
        let program = f.program.clone();
        // a branch not taken sets the pc to the end of its block: a repeat there was skipped
        let repeat = match &program.instrs[f.pc] { Instr::Repeat { from, .. } if f.pending.is_none() => Some(*from), _ => None };
        let result = if matches!(f.pending, Some(Pending::Operation { .. })) {
            let Some(Pending::Operation { result: Some(result), created }) = f.pending.take() else {
                return unsupported("a parent cannot advance before its child returns");
            };
            f.operation_result(game,result, created);
            Ok(())
        } else { execute::step(game, f, &program.instrs[f.pc]) };
        match result {
            Ok(()) => {}
            Err(Halt::Ask(d)) => return Ok(Some(Paused::Ask(d))),
            Err(Halt::Child) => return Ok(Some(Paused::Child)),
            Err(Halt::Unsupported(u)) => return Err(u),
        }
        let ended = f.pc + 1;
        // a repeat goes back to the start of its process once the act it ends is done, so
        // what that act did is settled (and triggers noted) each time through
        f.pc = repeat.unwrap_or(ended);
        if let Some(act) = program.acts.iter().find(|a| a.end == ended) {
            if !act.keeps_x { f.ctx.x = f.base_x; }
            let (from, before) = f.open.take().expect("an act is open");
            // what one act did, it did at once; then triggers and combat see it (603.2, 506.4)
            game.finish_simultaneous(from, before);
            game.update_combat();
            game.check_triggers();
            if game.returning() { return Ok(Some(Paused::Child)); }
            if let Some(d) = game.decision.take() {
                return Ok(Some(Paused::Ask(d)));
            }
        }
    }
    Ok(None)
}

/// Retain evaluated operands and collect consent once; action-specific procedures commit
/// the accepted request. Library ordering and ordinary actions share payment outcomes.
/// Cached compilation result for one effect, including explicit lowering failures.
/// Identity is the ability index within its definition.
#[derive(Debug, Clone)]
pub(crate) struct CompiledEffect {
    pub(crate) ability: usize,
    /// Actual linked exile reads lowered in this row, not inferred from donor wording.
    pub(crate) linked_exile_writers: Vec<usize>,
    result: Res<Body>,
    /// Of a refused body, the instructions its lowering never reached.
    unreached: usize,
    intervening: Res<Option<condition::Condition>>,
    triggered: bool,
    pub(crate) chapter: bool,
}

/// Modal bodies keep one compiled program per printed mode. Selecting modes does not
/// compile combinations or duplicate executable definitions (700.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Body {
    Plain(Arc<Program>),
    /// `repeats`: a mode may be chosen more than once, each time as if printed again (700.2d).
    /// `instead`: "If this spell was kicked, choose both instead.": how few and how many when
    /// its announcement unlocks it (601.2b).
    /// `paws`: "Choose up to five {P} worth of modes.": the most {P} the modes chosen are worth
    /// together, and what each is worth (700.2i).
    Modes { min: usize, max: usize, programs: Vec<Arc<Program>>, repeats: bool, instead: Option<(Unlock, usize, usize)>, paws: Option<(u32, Vec<u32>)> },
}

/// What a spell's announcement says that "choose both instead" asks of (601.2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unlock {
    /// "If this spell was kicked" (702.33d)
    Kicked,
    /// "If this spell was cast using teamwork", "if it was bargained": its optional additional
    /// cost was paid (118.8b)
    Optional,
}

/// How a spell was announced, as far as how many of its modes are chosen (601.2b): entwined,
/// every mode (702.42a); kicked or its optional additional cost paid, as "choose both instead"
/// asks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ModeCount { pub(crate) entwined: bool, pub(crate) kicked: bool, pub(crate) optional: bool }
impl Body {
    pub(crate) fn programs(&self) -> Vec<&Arc<Program>> {
        match self { Self::Plain(p) => vec![p], Self::Modes { programs, .. } => programs.iter().collect() }
    }
    fn selected(&self, modes: &[usize]) -> Res<Vec<Arc<Program>>> {
        self.selected_for(modes, ModeCount::default())
    }
    // Only spell announcement/capture supplies the paid all-mode policy (702.42).
    fn selected_for(&self, modes: &[usize], count: ModeCount) -> Res<Vec<Arc<Program>>> {
        let entwined = count.entwined;
        match self {
            Self::Plain(program) if modes.is_empty() => Ok(vec![program.clone()]),
            Self::Plain(_) => unsupported("that is not modal: no mode is chosen (700.2)"),
            Self::Modes { programs, repeats, .. } => {
                let (min, max) = self.bounds(count).expect("modal");
                if modes.len() < min || modes.len() > max { return unsupported(format!("choose {min} to {max} of its modes (700.2a)")); }
                if !self.within(modes) { return unsupported("choose modes worth no more {P} than its text allows (700.2i)"); }
                // in printed order, a mode chosen n times n times in sequence (608.2c, 700.2d)
                let mut order = modes.to_vec(); order.sort_unstable();
                if (entwined || !repeats) && order.windows(2).any(|w| w[0] == w[1]) { return unsupported("a mode is chosen once (700.2d)"); }
                order.into_iter().map(|i| programs.get(i).cloned().ok_or_else(|| crate::Unsupported("a mode the ability does not have".into()))).collect()
            }
        }
    }
    /// How few and how many of its modes are chosen, as the spell was announced (601.2b):
    /// all when entwined (702.42a), as "choose both instead" says when it holds.
    fn bounds(&self, count: ModeCount) -> Option<(usize, usize)> {
        let Self::Modes { min, max, programs, instead, .. } = self else { return None };
        Some(match instead {
            _ if count.entwined => (programs.len(), programs.len()),
            Some((Unlock::Kicked, a, b)) if count.kicked => (*a, *b),
            Some((Unlock::Optional, a, b)) if count.optional => (*a, *b),
            _ => (*min, *max),
        })
    }
    fn choices(&self) -> Option<Vec<Vec<usize>>> { self.choices_for(ModeCount::default()) }
    fn choices_for(&self, count: ModeCount) -> Option<Vec<Vec<usize>>> {
        let Self::Modes { programs, repeats, .. } = self else { return None };
        let (min, max) = self.bounds(count)?;
        Some((min..=max).flat_map(|k| crate::modes::combinations(programs.len(), k, *repeats && !count.entwined)).filter(|choice| self.within(choice)).collect())
    }
    /// Are the modes chosen worth no more {P} than its text allows (700.2i)?
    fn within(&self, modes: &[usize]) -> bool {
        match self {
            Self::Modes { paws: Some((most, worth)), .. } => modes.iter().map(|m| worth.get(*m).copied().unwrap_or(1)).sum::<u32>() <= *most,
            _ => true,
        }
    }
}

/// Executable identity captured when an ability occurs, before source changes or costs.
/// Clones share immutable code; only instantiating it creates mutable frame state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) ctx: Ctx<'static>,
    pub(crate) program: Arc<Program>,
    pub(crate) intervening: Option<condition::Condition>,
    pub(crate) triggered: bool,
    pub(crate) chapter: bool,
    values: Vec<Option<Value>>,
    sparse_checked: bool,
    /// Frozen declaring keyword for a rule-created body, shared by its occurrences.
    pub(crate) keyword_producer: Option<Arc<(crate::ability::FrozenDefinition, crate::binding::AbilityKey)>>,
    /// As [`Captured::granted_keyword`].
    pub(crate) granted_keyword: Option<Arc<ir::token::KeywordInstance>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Captured {
    ctx: Ctx<'static>,
    body: Body,
    intervening: Option<condition::Condition>,
    triggered: bool,
    pub(crate) chapter: bool,
    values: Vec<Option<Value>>,
    sparse_checked: bool,
    /// Frozen declaring keyword for a rule-created body, shared by its occurrences.
    pub(crate) keyword_producer: Option<Arc<(crate::ability::FrozenDefinition, crate::binding::AbilityKey)>>,
    /// Of a rule-created body whose keyword an effect gave the spell (613.1f), which no
    /// definition declares: that keyword, as it was when the body was created, which is the
    /// ability that created it (603.7a, 113.7a): "Rebound", "Warp {2}{R}". Presentation only.
    pub(crate) granted_keyword: Option<Arc<ir::token::KeywordInstance>>,
}
/// The reflexive triggered ability that applying a replacement effect triggers: "If a
/// creature an opponent controls would die, exile it instead. When you do, create a 2/2 green
/// Wolf creature token." (603.12)
/// `whole` is the "When you do, …" sentence, `rest` what follows its lead.
pub(crate) fn reflexive(whole: &Sentence, rest: &Sentence, support: &Support<'_>) -> Res<Arc<Program>> {
    lower(&Effect(vec![rest.clone()]), support).map(|program| said(program, whole, &[]))
}
/// A delayed or reflexive ability's instructions, with the sentences its text is: the one its
/// lead begins and those after it that are its own (603.7, 603.12).
fn said(mut program: Program, whole: &Sentence, more: &[Sentence]) -> Arc<Program> {
    program.text = Some(Effect(std::iter::once(whole.clone()).chain(more.iter().cloned()).collect()));
    Arc::new(program)
}
/// The text of a delayed or reflexive triggered ability whose instructions these are, with "~"
/// left for its source's name; None for other instructions, or a text the grammar cannot say.
pub(crate) fn delayed_text(program: &Program) -> Option<String> {
    grammar::ability::effect().print_text(program.text.as_ref()?, true)
}
/// It triggered, for its source's controller as the replacement applied.
pub(crate) fn reflexive_capture(ctx: Ctx<'static>, program: Arc<Program>) -> Captured {
    Captured { ctx, body: Body::Plain(program), intervening: None, triggered: true, chapter: false, values: Vec::new(), keyword_producer: None, granted_keyword: None, sparse_checked: false }
}
/// 702.88a: a retained exile incarnation, independently of the delayed source spell.
pub(crate) fn rebound_cast(game: &Game, ctx: Ctx<'static>, card: ObjId) -> Captured {
    let keyword_producer=ctx.source.and_then(|source| {
        let crate::completion::DestinationSource::Ability {ability:Some(key),..}=
            game.spell_destination_source(source,crate::state::DestinationEffect::Rebound) else {return None};
        Some(Arc::new((game.frozen_definition(source,&key.origin)?,key)))
    });
    // rebound an effect gave the spell (613.1f), which no definition of it declares
    let granted_keyword=ctx.source.filter(|_| keyword_producer.is_none()).and_then(|source|
        game.seen_for(source,crate::layers::Aspect::Abilities).keywords.iter().find(|held| held.keyword.name()=="Rebound")
            .map(|held| Arc::new(ir::token::KeywordInstance{keyword:held.keyword,param:held.param.clone()})));
    Captured {ctx,body:Body::Plain(rebound_program().clone()),intervening:None,triggered:true,chapter:false,
        values:vec![Some(Value::Subjects(vec![Target::Object(card)]))],keyword_producer,granted_keyword,sparse_checked:false}
}
fn rebound_program() -> &'static Arc<Program> {
    static PROGRAM: std::sync::OnceLock<Arc<Program>> = std::sync::OnceLock::new();
    PROGRAM.get_or_init(|| Arc::new(Program::with_initial(vec![Instr::Cast {
        player: action::Operand::Controller, cards: vec![action::Operand::Reference(0)],
        offered: 1, cast: 2, possible: 3, costs: crate::permission::Costs::Free,
        from: CastOrigin::Fixed(Zone::Exile), rider: None, haste: false, one: true, ceiling: None,
    }], vec![ActSpan{end:1,moves:true,keeps_x:false}], Vec::new(), vec![Some(Kind::Subjects)])
        .expect("rebound uses the ordinary retained free-cast instruction")))
}
/// The text of the delayed ability a keyword's rule creates, for a program no card prints:
/// rebound's upkeep cast (702.88a), or warp's end-step exile (702.185a).
pub(crate) fn keyword_delayed_text(program: &Arc<Program>) -> Option<&'static str> {
    keyword_delayed_identity(program).map(|(_, _, text)| text)
}
/// Only these exact rule-owned executables have generated keyword occurrence text.
/// Their declaring row must separately authenticate the corresponding keyword.
pub(crate) fn keyword_delayed_identity(program: &Arc<Program>) -> Option<(&'static str, &'static str, &'static str)> {
    if same_instance(program, rebound_program()) {
        Some(("Rebound", "702.88a", "At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost."))
    } else if same_instance(program, warp_program()) {
        Some(("Warp", "702.185a", "At the beginning of the next end step, exile the permanent this spell becomes. Its owner may cast this card after the current turn has ended for as long as it remains exiled."))
    } else if same_instance(program, dash_program()) {
        Some(("Dash", "702.109a", "At the beginning of the next end step, return the permanent this spell becomes to its owner's hand."))
    } else { None }
}

/// 702.185a: "exile the permanent this spell becomes at the beginning of the next end step.
/// Its owner may cast this card after the current turn has ended for as long as it remains
/// exiled." Only that permanent is exiled (400.7); a card that never reached exile this way
/// is given nothing.
pub(crate) fn warp_exile(game: &Game, ctx: Ctx<'static>, permanent: ObjId) -> Captured {
    alternative_delayed(game, ctx, permanent, warp_program())
}
/// 702.109a: retain the resulting permanent, not a later incarnation of its card.
pub(crate) fn dash_return(game: &Game, ctx: Ctx<'static>, permanent: ObjId) -> Captured {
    alternative_delayed(game, ctx, permanent, dash_program())
}
fn alternative_delayed(game: &Game, ctx: Ctx<'static>, permanent: ObjId, program: &Arc<Program>) -> Captured {
    let keyword_producer=ctx.source.zip(ctx.ability.as_ref()).and_then(|(source,key)|
        Some(Arc::new((game.frozen_definition(source,&key.origin)?,key.clone()))));
    // a warp an effect gave the card in its hand (613.1f), kept as it was cast for its cost
    let granted_keyword=ctx.source.filter(|_| keyword_producer.is_none())
        .and_then(|spell| game.obj(spell).cast_announcement.as_ref()?.given_alternative.clone());
    Captured {ctx,body:Body::Plain(program.clone()),intervening:None,triggered:true,chapter:false,
        values:vec![Some(Value::Subjects(vec![Target::Object(permanent)]))],keyword_producer,granted_keyword,sparse_checked:false}
}
fn dash_program() -> &'static Arc<Program> {
    static PROGRAM: std::sync::OnceLock<Arc<Program>> = std::sync::OnceLock::new();
    PROGRAM.get_or_init(|| Arc::new(Program::with_initial(vec![
        Instr::Action { action: action::Action::Move { groups: vec![(vec![action::Operand::Reference(0)], action::Destination::Zone(Zone::Hand))], said: ir::action::MoveVerb::Return }, created: None, attempt: None },
    ], vec![ActSpan { end:1, moves:true, keeps_x:false }], Vec::new(), vec![Some(Kind::Subjects)])
        .expect("dash returns with the ordinary move instruction")))
}
fn warp_program() -> &'static Arc<Program> {
    static PROGRAM: std::sync::OnceLock<Arc<Program>> = std::sync::OnceLock::new();
    PROGRAM.get_or_init(|| Arc::new(Program::with_initial(vec![
        Instr::Action { action: action::Action::Move { groups: vec![(vec![action::Operand::Reference(0)], action::Destination::Zone(Zone::Exile))], said: ir::action::MoveVerb::Put }, created: None, attempt: None },
        Instr::Permit { player: action::Operand::OwnerOf(Box::new(action::Operand::Reference(0))), cards: vec![action::Operand::Reference(0)], play: false, until: crate::permission::Deadline::Warped },
    ], vec![ActSpan{end:1,moves:true,keeps_x:false}, ActSpan{end:2,moves:false,keeps_x:false}], Vec::new(), vec![Some(Kind::Subjects)])
        .expect("warp exiles with the ordinary move and permits with the ordinary permission")))
}

impl Captured {
    /// Check sparse semantic captures against the same dependency/type checker
    /// as compiled programs. Missing unused prefix cells are not invented facts.
    fn checked_sparse(ctx:Ctx<'static>, program:Arc<Program>, values:Vec<Option<Value>>) -> Res<Self> {
        let mut initial=vec![None;program.initial.len()];
        for (slot,value) in values.iter().enumerate().take(initial.len()) {
            let Some(value)=value else {continue};
            let kind=match value {
                Value::Cards(_)=>Kind::Cards,Value::Subjects(_)=>Kind::Subjects,
                Value::Consent(_)=>Kind::Consent,Value::Number(_)=>Kind::Number,
                _=>return unsupported("sparse capture value kind is not represented"),
            };
            if program.initial[slot]!=Some(kind) {return unsupported("sparse capture has the wrong retained value kind");}
            initial[slot]=Some(kind);
        }
        Program::with_initial(program.instrs.clone(),program.acts.clone(),program.targets.clone(),initial)
            .map_err(|_|crate::Unsupported("sparse capture is missing a required historical binding".into()))?;
        Ok(Self{ctx,body:Body::Plain(program),intervening:None,triggered:true,chapter:false,values,
            keyword_producer:None,granted_keyword:None,sparse_checked:true})
    }
    pub(crate) fn context(&self) -> Ctx<'static> { self.ctx.clone() }
    /// Has it no target, and could it add mana (605.1b)? Of instructions with modes, none is.
    pub(crate) fn mana_ability(&self) -> bool {
        matches!(&self.body, Body::Plain(p) if p.targets.is_empty() && p.announcement.adds_mana)
    }
    /// The compiled instance it runs (of a modal body, its first mode's): what it is told
    /// apart from another occurrence by ([`same_instance`]).
    pub(crate) fn program(&self) -> &Arc<Program> {
        match &self.body { Body::Plain(p) => p, Body::Modes { programs, .. } => &programs[0] }
    }
    pub(crate) fn identity(&self) -> ProgramId { self.program().id }
    pub(crate) fn legal_modes(&self, modes: &[usize]) -> Result<(), String> {
        let Body::Modes { min, max, programs, repeats, .. } = &self.body else { return if modes.is_empty() { Ok(()) } else { Err("that is not modal (700.2)".into()) }; };
        if modes.len() < *min || modes.len() > *max || modes.iter().any(|i| *i >= programs.len()) {
            return Err(format!("choose {min} to {max} modes of {} (700.2)", programs.len()));
        }
        if !repeats && modes.iter().enumerate().any(|(i,m)| modes[..i].contains(m)) {
            return Err(format!("choose {min} to {max} distinct modes of {} (700.2d)", programs.len()));
        }
        if !self.body.within(modes) { return Err("choose modes worth no more {P} than its text allows (700.2i)".into()); }
        Ok(())
    }
    pub(crate) fn select(&self, modes: &[usize]) -> Res<Vec<Invocation>> { self.select_spell(modes, ModeCount::default()) }
    pub(crate) fn select_spell(&self, modes: &[usize], count: ModeCount) -> Res<Vec<Invocation>> {
        Ok(self.body.selected_for(modes, count)?.into_iter().map(|program| Invocation {
            ctx: self.ctx.clone(), program, intervening: self.intervening.clone(), triggered: self.triggered, chapter: self.chapter, values: self.values.clone(), keyword_producer: self.keyword_producer.clone(), granted_keyword: self.granted_keyword.clone(), sparse_checked:self.sparse_checked,
        }).collect())
    }
    pub(crate) fn targets(&self, modes: &[usize]) -> Res<Vec<TargetSpec>> {
        let mut slots = Vec::new();
        for program in self.body.selected(modes)? { crate::announcement::append_targets(&mut slots, program.targets.clone()); }
        Ok(slots)
    }
    pub(crate) fn choices(&self) -> Option<Vec<Vec<usize>>> { self.body.choices() }
}

pub(crate) fn capture(game: &Game, mut ctx: Ctx<'static>) -> Res<Captured> {
    ctx.link_generation = game.link_generation(ctx.source, ctx.ability.as_ref());
    let mut def = ctx.definition(game);
    if let Some((_, face)) = ctx.casting.filter(|(source, _)| Some(*source) == ctx.source) { def = def.and_then(|d| d.spell(face).ok()); }
    let cached = def.and_then(|d| d.programs.iter().find(|p| Some(p.ability) == ctx.effect));
    if let Some(cached) = cached {
        return Ok(Captured { ctx: ctx.clone(), body: cached.result.clone()?, intervening: cached.intervening.clone()?, triggered: cached.triggered, chapter: cached.chapter, values: Vec::new(), keyword_producer: None, granted_keyword: None, sparse_checked: false });
    }
    Ok(Captured { ctx: ctx.clone(), body: Body::Plain(required(game, &ctx)?), intervening: None, triggered: false, chapter: false, values: Vec::new(), keyword_producer: None, granted_keyword: None, sparse_checked: false })
}

pub(crate) fn selected(game: &Game, ctx: Ctx<'static>, modes: &[usize]) -> Res<Vec<Arc<Program>>> {
    capture(game, ctx)?.body.selected(modes)
}

/// Definition-scoped identity, separate from a delayed effect's captured linked context.
pub(crate) fn compiled(game: &Game, ctx: &Ctx) -> Option<Res<Arc<Program>>> {
    let ability = ctx.effect?;
    let mut def = ctx.definition(game)?;
    if let Some((_, choice)) = ctx.casting.filter(|(card, _)| Some(*card) == ctx.source) {
        def = def.spell(choice).ok()?;
    }
    definition_program(def, ability)
}

/// Spell announcement consumes the same cached bodies that occurrences will retain.
pub(crate) fn spell_bodies(def: &crate::CardDef) -> Res<Vec<(usize, &Body)>> {
    let bodies = def.spell_abilities()?.into_iter().map(|i| {
        let body = def.programs.iter().find(|p| p.ability == i).expect("an executable spell body");
        Ok((i, body.result.as_ref().map_err(Clone::clone)?))
    }).collect::<Res<Vec<_>>>()?;
    if bodies.iter().filter(|(_, b)| matches!(b, Body::Modes { .. })).count() > 1 { return unsupported("a spell with more than one modal ability"); }
    Ok(bodies)
}
/// Every choice of the spell's modes, entwined or not, of any announcement of whether it is
/// kicked or its optional additional cost paid ("choose both instead", 601.2b): which of them
/// an announcement allows is checked as it is cast ([`spell_programs`]).
pub(crate) fn spell_choices(def: &crate::CardDef, entwined: bool) -> Res<Vec<Vec<usize>>> {
    let bodies = spell_bodies(def)?;
    let mut choices: Option<Vec<Vec<usize>>> = None;
    for (kicked, optional) in [(false, false), (true, false), (false, true)] {
        for choice in bodies.iter().find_map(|(_, b)| b.choices_for(ModeCount { entwined, kicked, optional })).into_iter().flatten() {
            let all = choices.get_or_insert_with(Vec::new);
            if !all.contains(&choice) { all.push(choice); }
        }
    }
    if entwined && choices.is_none() { return unsupported("entwine requires a modal spell (702.42)"); }
    Ok(choices.unwrap_or_else(|| vec![Vec::new()]))
}
/// The spell's modal ability, by its index: the one its modes are chosen for (700.2a).
pub(crate) fn spell_modal(def: &crate::CardDef) -> Res<Option<usize>> {
    Ok(spell_bodies(def)?.into_iter().find(|(_, b)| matches!(b, Body::Modes { .. })).map(|(i, _)| i))
}
pub(crate) fn spell_programs(def: &crate::CardDef, modes: &[usize], count: ModeCount) -> Res<Vec<(usize, Vec<Arc<Program>>)>> {
    let bodies = spell_bodies(def)?;
    let entwined = count.entwined;
    if (entwined || !modes.is_empty()) && !bodies.iter().any(|(_, b)| matches!(b, Body::Modes { .. })) { return unsupported("that spell is not modal: no mode is chosen (700.2)"); }
    bodies.into_iter().map(|(i, b)| b.selected_for(if matches!(b, Body::Modes { .. }) { modes } else { &[] }, count).map(|p| (i, p))).collect()
}

/// Which of this definition's abilities compiled to this program, or to one that makes it (a
/// delayed or reflexive triggered ability's, 603.7, 603.12), or is a replacement effect whose
/// application triggers it (603.12): what an occurrence runs names the printed ability it came
/// from, whatever its source is now.
pub(crate) fn ability_compiled_to(def: &crate::card::CardDef, program: &Arc<Program>) -> Option<usize> {
    def.programs.iter().find(|p| p.result.as_ref().is_ok_and(|b| b.programs().into_iter().any(|q| same_instance(q, program) || q.makes(program)))).map(|p| p.ability)
        .or_else(|| def.replacements.iter().find(|r| r.result.as_ref().ok().and_then(|r| r.reflexive())
            .is_some_and(|q| same_instance(q, program) || q.makes(program))).map(|r| r.ability))
}

impl Program {
    /// Refuse unrepresentable announcement-only quantities before paying a newly
    /// executable variable result partition. Dynamic values are checked on execution.
    pub(crate) fn check_partition_quantities(&self, game: &Game, ctx: &Ctx) -> Res<()> {
        if !self.instrs.iter().any(|i| matches!(i, Instr::Select { pick: Pick::Quantity(_), .. })) { return Ok(()); }
        for instr in &self.instrs {
            if matches!(instr, Instr::SetX(_)) { break; }
            let amount = match instr {
                Instr::TakeTop { n, .. } | Instr::Select { pick: Pick::Quantity(n), .. } => n,
                _ => continue,
            };
            if amount.announcement_only() { amount.checked(game, ctx, &expr::NoLocals)?; }
        }
        Ok(())
    }

    /// Whether this program makes that one: a delayed or reflexive triggered ability's
    /// instructions, at any depth; of mana it adds, the one that triggers when that mana is
    /// spent (106.6).
    fn makes(&self, made: &Arc<Program>) -> bool {
        self.instrs.iter().any(|i| match i {
            Instr::Reflexive { program, .. } | Instr::Delayed { program, .. } => same_instance(program, made) || program.makes(made),
            Instr::Mana { production, .. } => production.spending_trigger().is_some_and(|program| same_instance(program, made) || program.makes(made)),
            _ => false,
        })
    }
}

pub(crate) fn definition_program(def: &crate::card::CardDef, ability: usize) -> Option<Res<Arc<Program>>> {
    def.programs.iter().find(|p| p.ability == ability).map(|p| match p.result.clone()? {
        Body::Plain(program) => Ok(program), Body::Modes { .. } => unsupported("modal executable needs its announced modes"),
    })
}

/// Opt-in audit proof: the exact cached executables selected from one definition
/// row, including printed order and repeated modes. This never executes a body.
pub(crate) fn definition_selected(def: &crate::card::CardDef, ability: usize,
    modes: &[usize], count: ModeCount) -> Option<Res<Vec<Arc<Program>>>> {
    def.programs.iter().find(|row|row.ability==ability).map(|row| {
        let body=row.result.as_ref().map_err(Clone::clone)?;
        let modes=if matches!(body,Body::Modes{..}) {modes} else {&[]};
        body.selected_for(modes,count)
    })
}

/// A delayed/reflexive body retains the identity of the program that created it
/// (603.7, 603.12), rather than running its declaring paragraph again. This is
/// deliberately narrower than `ability_compiled_to`: an ordinary selected mode
/// is not a child and must still pass the complete selected-row audit.
pub(crate) fn definition_creates_program(def: &crate::card::CardDef, ability: usize,
    program: &Arc<Program>) -> bool {
    def.programs.iter().find(|row|row.ability==ability)
        .and_then(|row|row.result.as_ref().ok())
        .is_some_and(|body|body.programs().into_iter().any(|parent|parent.makes(program)))
}

pub(crate) fn required(game: &Game, ctx: &Ctx) -> Res<Arc<Program>> {
    compiled(game, ctx).unwrap_or_else(|| unsupported("program: no compiled definition for this ability"))
}

#[cfg(test)]
mod completion_tests {
    use super::*;

    #[test]
    fn produced_origin_must_name_an_earlier_successful_result_producer() {
        use super::action::Operand;
        for prefix in [vec![], vec![Instr::TakeTop { n: 1.into(), whose: Operand::Controller, out: 0 }]] {
            let slot = if prefix.is_empty() { 999 } else { 0 };
            let mut instructions = prefix;
            instructions.push(Instr::Cast { player: Operand::Controller, cards: vec![], offered: 1,
                costs: crate::permission::Costs::Free, cast: 2, possible: 3,
                from: CastOrigin::Produced(slot), rider: None, haste: false, one: true, ceiling: None });
            let end = instructions.len();
            let error = Program::new(instructions, vec![ActSpan { end, moves: false, keeps_x: false }]).unwrap_err();
            assert!(error.0.contains("cast origin does not name an earlier successful card producer"), "{error:?}");
        }
    }

    #[test]
    fn retained_origin_must_name_that_moves_captured_input() {
        use super::action::Operand;
        let instructions = vec![
            Instr::Capture { operand: Operand::Source, out: 0 },
            Instr::Action { action: action::Action::Move { groups: vec![(vec![Operand::Reference(0)], action::Destination::Zone(Zone::Exile))], said: ir::action::MoveVerb::Put }, created: Some(1), attempt: None },
            Instr::Capture { operand: Operand::Controller, out: 2 },
            Instr::Cast { player: Operand::Controller, cards: vec![], offered: 3, costs: crate::permission::Costs::Free,
                cast: 4, possible: 5, from: CastOrigin::Retained { original: 2, moved: 1 }, rider: None, haste: false, one: true, ceiling: None },
        ];
        let end = instructions.len();
        let error = Program::new(instructions, vec![ActSpan { end, moves: true, keeps_x: false }]).unwrap_err();
        assert!(error.0.contains("retained cast origin does not name its exact movement input"), "{error:?}");
    }

    #[test]
    fn r608_3b_rejected_aura_entry_still_completes_into_owners_graveyard() {
        let aura = crate::CardDef::new("Aura", "{0}", "Enchantment — Aura", "Enchant creature", None).unwrap();
        let mut game = Game::new(vec![aura], 2, 20);
        let spell = game.put(1, 0, Zone::Stack);
        game.obj_mut(spell).controller = 0;
        // Exercise completion after entry itself refuses, independently of resolve_top's
        // earlier target check. No legal attachment is available (608.3b).
        let mut at = crate::state::Resolving::new(spell, game.obj(spell).bound.clone());
        at.execution = Some(Resolution::permanent(&game, spell, crate::replace::Choices::default()));
        game.start_resolution(at);
        game.go_on().unwrap();
        assert!(game.stack.is_empty() && game.resolving().is_none() && game.decision.is_none());
        assert!(game.battlefield.is_empty() && game.players[0].graveyard.is_empty());
        assert_eq!(game.players[1].graveyard.len(), 1);
        assert_eq!(game.obj(game.players[1].graveyard[0]).card, Some(0));
    }
}

impl crate::card::CardDef {
    pub(crate) fn executable_body(&self, index: usize) -> Res<&Body> {
        self.programs.iter().find(|p| p.ability == index).ok_or_else(|| crate::Unsupported("no compiled executable body".into()))?.result.as_ref().map_err(Clone::clone)
    }
    /// Of the ability at `index` whose body was refused, the instructions its lowering, which
    /// stops at its first refusal, never reached: what may hide further refusals.
    pub(crate) fn unreached_instructions(&self, index: usize) -> usize {
        self.programs.iter().find(|p| p.ability == index).map_or(0, |p| p.unreached)
    }
    /// Successful cached executable effects, including keyword-generated abilities.
    /// This does not classify static registrations or claim that the whole card is compiled.
    pub fn compiled_abilities(&self) -> Vec<usize> {
        self.programs.iter().filter(|p| p.result.is_ok()).map(|p| p.ability).collect()
    }

    /// What this definition compiled, in canonical form, to diff the compiler's output
    /// program by program across changes (`game-gate --program-dump`). Each executable
    /// ability by its index: its body (each program with its [`ProgramId`], its canonical
    /// text and, after it, the delayed and reflexive programs it makes) or why it was refused,
    /// and its intervening condition; then the reflexive programs of its replacement effects
    /// (603.12). Faces it has (halves, a back face, an alternative spell) are not included.
    pub fn program_dump(&self) -> serde_json::Value {
        use serde_json::json;
        fn refused(u: &crate::Unsupported) -> serde_json::Value { json!({"refused": u.0}) }
        fn entries(program: &Program, out: &mut Vec<serde_json::Value>) {
            out.push(json!({"id": program.id.to_string(), "canonical": program.canonical()}));
            for instr in &program.instrs {
                if let Instr::Reflexive { program, .. } | Instr::Delayed { program, .. } = instr { entries(program, out); }
            }
        }
        let programs = |program: &Program| { let mut out = Vec::new(); entries(program, &mut out); out };
        let abilities: Vec<_> = self.programs.iter().map(|p| json!({
            "ability": p.ability, "triggered": p.triggered, "chapter": p.chapter,
            "body": match &p.result {
                Err(u) => refused(u),
                Ok(Body::Plain(program)) => json!({"plain": programs(program)}),
                Ok(Body::Modes { min, max, programs: modes, repeats, instead, paws }) => json!({"modes": {
                    "min": min, "max": max, "repeats": repeats, "instead": instead.map(|i| format!("{i:?}")), "paws": paws,
                    "programs": modes.iter().map(|m| programs(m)).collect::<Vec<_>>()}}),
            },
            "intervening": match &p.intervening { Err(u) => refused(u), Ok(c) => json!(c.as_ref().map(|c| format!("{c:?}"))) },
        })).collect();
        fn reflexive(r: &crate::replace::Replacement) -> Option<&Arc<Program>> {
            match r {
                crate::replace::Replacement::Elsewhere { then, .. } => then.as_ref(),
                crate::replace::Replacement::Solved(inner) => reflexive(inner),
                _ => None,
            }
        }
        let replacements: Vec<_> = self.replacements.iter().filter_map(|r| {
            let program = reflexive(r.result.as_ref().ok()?)?;
            Some(json!({"ability": r.ability, "reflexive": programs(program)}))
        }).collect();
        json!({"abilities": abilities, "replacements": replacements})
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Execution {
    Program(Frame),
    /// Resolving a permanent spell (608.3), including its before-entry choice (614.12a).
    Permanent { spell: ObjId, controller: PlayerId, chosen: crate::replace::Choices, waiting: bool, completed: bool },
}

/// The operation selected at resolution start: a permanent's entry, or a sequence of whole
/// abilities. Each ability retains its definition identity and consumes its own announcement
/// groups. Both forms own their pending decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolution {
    parts: Vec<Execution>,
    next: usize,
}

impl Resolution {
    pub(crate) fn permanent(game: &Game, spell: ObjId, chosen: crate::replace::Choices) -> Self {
        let controller = game.controller(spell);
        Self { parts: vec![Execution::Permanent { spell, controller, chosen, waiting: false, completed: false }], next: 0 }
    }

    pub(crate) fn new(invocations: Vec<Invocation>, bound: &crate::state::Bound) -> Self {
        let mut base=0;
        let parts = invocations.into_iter().map(|invocation| {
            let mut frame = Frame::new(invocation.program, invocation.ctx);
            frame.occurrence_base=base;
            base+=frame.program.instrs.len();
            for (slot, kind) in frame.program.initial.iter().enumerate() {
                if kind.is_some() {
                    let value=invocation.values.get(slot).cloned().flatten();
                    assert!(value.is_some() || invocation.sparse_checked,"captured program requires a defined value");
                    frame.slots[slot]=value;
                }
            }
            frame.inputs = bound.inputs.clone();
            frame.event_amount = bound.event_amount;
            frame.transitions = bound.became.clone();
            Execution::Program(frame)
        }).collect();
        Self { parts, next: 0 }
    }

    pub(crate) fn event_floor(&self) -> Option<crate::event::Seq> {
        self.parts.iter().filter_map(|part| match part {
            Execution::Program(frame) => frame.open.as_ref().map(|(from, _)| *from),
            Execution::Permanent { .. } => None,
        }).min()
    }

    /// The new object this resolution's own effect moved this one to a zone as (400.7j).
    pub(crate) fn moved(&self, was: ObjId) -> Option<ObjId> {
        self.parts.iter().find_map(|part| match part {
            Execution::Program(frame) => frame.transitions.iter().rev().find(|(old, _)| *old == was).map(|(_, now)| *now),
            _ => None,
        })
    }

    pub(crate) fn produced_mana(&self, source: ObjId) -> bool {
        self.parts.iter().any(|part| matches!(part, Execution::Program(frame)
            if frame.ctx.source == Some(source) && frame.produced_mana))
    }

    pub(crate) fn entering_card(&self) -> Option<ObjId> {
        match self.parts.get(self.next) { Some(Execution::Permanent { spell, .. }) => Some(*spell), _ => None }
    }

    pub(crate) fn frame(&self) -> Option<&Frame> {
        match self.parts.get(self.next) { Some(Execution::Program(f)) => Some(f), _ => None }
    }

    /// The ability whose instructions these are, as its first program was captured with it.
    pub(crate) fn ability(&self) -> Option<crate::binding::AbilityKey> {
        self.parts.iter().find_map(|p| match p { Execution::Program(f) => f.ctx.ability.clone(), _ => None })
    }

    pub(crate) fn frame_mut(&mut self) -> Option<&mut Frame> {
        match self.parts.get_mut(self.next) { Some(Execution::Program(f)) => Some(f), _ => None }
    }

    /// The player took the suspended instruction's offer by beginning to cast: the spell's
    /// payment is its child, and reports like an effect's payment.
    pub(crate) fn cast_begun(&mut self) {
        match self.parts.get_mut(self.next) {
            Some(Execution::Program(Frame { pending: pending @ Some(Pending::Answer { decision: Decision::Choose { choice: Choice::Cast { .. }, .. }, answer: None }), .. })) => {
                *pending = Some(Pending::Payment { result: None });
            }
            _ => unreachable!("the program is offering a cast"),
        }
    }

    /// Whether the payment the suspended instruction started was made (118.12).
    pub(crate) fn payment_complete(&mut self, paid: bool) {
        match self.parts.get_mut(self.next) {
            Some(Execution::Program(Frame { pending: Some(Pending::Payment { result }), .. })) => {
                assert!(result.is_none(), "payment completion is delivered once");
                *result = Some(paid);
            }
            _ => unreachable!("the program is awaiting its payment"),
        }
    }

    /// A child returns to the exact suspended occurrence, never by searching for a caller.
    pub(crate) fn operation_complete(&mut self, result: crate::operation::Completed) {
        match self.parts.get_mut(self.next).expect("a child has a retained parent") {
            Execution::Program(frame) => match &mut frame.pending {
                Some(Pending::Operation { result: destination, .. }) => {
                    assert!(destination.is_none(), "child completion is delivered once");
                    *destination = Some(result);
                }
                Some(Pending::Objects { procedure, .. }) => procedure.operation_complete(result),
                Some(Pending::Connive(procedure)) => procedure.operation_complete(result),
                Some(Pending::Explore(procedure)) => procedure.operation_complete(result),
                Some(Pending::LibraryScan(procedure)) => procedure.operation_complete(result),
                Some(Pending::Arrange(procedure)) => procedure.operation_complete(result),
                _ => unreachable!("the program is awaiting its child operation"),
            },
            Execution::Permanent { waiting: true, completed, .. } => {
                assert!(!*completed, "entry completion is delivered once");
                *completed = true;
            }
            _ => unreachable!("permanent resolution is waiting for entry"),
        }
    }

    pub(crate) fn advance(&mut self, game: &mut Game, targets: &[Vec<Target>], division: &[Vec<u32>], at: &mut crate::state::Resolving) -> Res<Option<Paused>> {
        while let Some(part) = self.parts.get_mut(self.next) {
            let decision = match part {
                Execution::Program(frame) => {
                    let count = frame.program.targets.len();
                    let announced = targets.get(at.next..at.next + count)
                        .ok_or_else(|| crate::Unsupported("resolution is missing an announced target group".into()))?;
                    frame.occurrence = matches!(at.completion, crate::state::Completion::Stack).then_some(at.object);
                    frame.set_targets(announced)?;
                    frame.division = division.get(at.next..at.next + count).map(<[Vec<u32>]>::to_vec).unwrap_or_default();
                    let decision = advance(game, frame)?;
                    if decision.is_none() { at.next += count; }
                    decision
                }
                Execution::Permanent { spell, controller, chosen, waiting, completed } => {
                    if *completed {
                        None
                    } else if *waiting {
                        return unsupported("a permanent resolution awaits its entry result");
                    } else {
                        let request = crate::entry::EntryRequest::card(*spell, *controller, chosen.clone(), false);
                        match game.prepare_entry_batch(vec![request], Vec::new())? {
                            Ok(_) => None,
                            Err(decision) => { *waiting = true; Some(Paused::Ask(decision)) }
                        }
                    }
                }
            };
            if decision.is_some() { return Ok(decision); }
            self.next += 1;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod retained_tests;

impl From<u32> for expr::Amount { fn from(n: u32) -> Self { Self::constant(n) } }

impl crate::retention::Trace for Captured {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { ctx, values, body: _, intervening: _, triggered: _, chapter: _, keyword_producer: _, granted_keyword: _, sparse_checked: _ } = self;
        ctx.trace(visit);
        values.trace(visit);
    }
}
impl crate::retention::Trace for Invocation {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { ctx, values, program: _, intervening: _, triggered: _, chapter: _, keyword_producer: _, granted_keyword: _, sparse_checked: _ } = self;
        ctx.trace(visit);
        values.trace(visit);
    }
}
