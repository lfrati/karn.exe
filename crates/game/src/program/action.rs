//! The ordinary actions of a compiled program: what a player or the source does to players
//! and permanents, said with typed operands and done by the game's own operations.
//!
//! An action reads the game until it knows everyone and everything it is about and every
//! number it says ([`evaluate`]), and only then is anything changed, by [`crate::perform`].
//! Its operands are compiled expressions ([`super::expr`]): reading them cannot fail, and
//! what the engine cannot do with an operand is refused when the action is lowered
//! ([`Action::check`]).
use super::Frame;
use super::expr;
use crate::entry::EntryRequest;
use crate::event::Target;
use crate::operation::Placement;
use crate::perform::{self, PlayerAction};
use crate::state::{Game, ObjId, PlayerId, Zone};
use crate::{Res, unsupported};

/// Who or what an action is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// The player whose text this is.
    Controller,
    /// Runtime actor of a retained rule operation.
    Player(PlayerId),
    /// Successful tapping resources retained by this spell.
    Convoked,
    /// The source incarnation of the ability, retained even after it leaves.
    Source,
    /// "defending player" said by an attacking creature's ability: the player it is attacking,
    /// or was attacking before it was removed from combat (508.5).
    DefendingPlayer,
    /// An announced target group, each member as long as it is still legal (608.2b).
    Targets(usize),
    /// Identities retained by a named producer; zone changes do not retarget this value.
    Reference(super::Slot),
    Subject(expr::Subject),
    Union(Vec<Operand>),
    /// Matching members of a retained group, queried as the action runs. A
    /// separate Capture freezes a condition's subset before its body.
    Matching(Matching),
    ControllerOf(Box<Operand>),
    /// Distinct former controllers of qualified completed destruction records.
    FormerControllers(FormerControllers),
    OwnerOf(Box<Operand>),
    /// Everything the description fits as the action is done (608.2h).
    All(expr::Set),
    /// The described cards in a bound player's zone (400.1).
    Zone(expr::ZoneCards),
    /// "That card from your graveyard", "that card from exile": what it names, only while it
    /// is (or what it was followed to is) there; elsewhere it is not what the text means.
    Within { of: Box<Operand>, place: Place },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matching { pub(super) of: Box<Operand>, pub(super) filter: Box<expr::Filter>, pub(super) controller: Option<expr::Controller> }

/// A compiled player projection; construction stays with the typed result binder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormerControllers { pub(super) slot: super::Slot, pub(super) filter: Box<expr::Filter> }

/// Where a reference says what it names is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A player's hand, library or graveyard, of the cards it describes.
    Pile(expr::ZoneCards),
    /// A zone all players share: exile (400.1).
    Shared(Zone),
}

/// Compiled movement destination. Control and entry status are independent of card ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Zone(Zone),
    /// Onto the battlefield, under the control of the player `by` who puts it there (110.2a),
    /// or of its owner where the text says so.
    /// `face_down`: it enters face down this way (701.40a).
    Battlefield { owner_controls: bool, by: super::Who, tapped: bool, counters: Vec<(String, expr::Amount)>, attacking: Option<super::input::Input>, transformed: bool, face_down: Option<crate::face::Concealment> },
}

/// The copiable baseline a token is created with (111.3, 707.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token { Defined(crate::copy::Token), Copy { of: Box<Operand>, changes: Vec<ir::CopyChange> } }

/// Which counters an instruction removes (122.1): a stated kind, or every kind.
/// An unnamed fixed number requires a choice and is not an all-kinds instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterRemoval {
    Named { kind: String, count: Option<u32> },
    All,
}

/// A successful counter's destination, including its own self-replacement (614.15).
/// Ordinary graveyard replacements are considered only after this policy is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterDestination { Graveyard, Exile, Hand, Library(ir::action::LibraryPosition) }

/// One ordinary action. What one action does, it does at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    BecomeMonarch { players: Operand },
    /// "Your speed increases by 1" (702.179d).
    IncreaseSpeed { players: Operand },
    GetEmblem { players: Operand, definition: crate::ability::Template },
    Destroy { objects: Vec<Operand>, regenerable: bool },
    Counter { objects: Vec<Operand>, destination: CounterDestination },
    ExileUntil { objects: Vec<Operand> },
    /// `said`: whether the text says "return" or "put" (the rules treat them alike).
    Move { groups: Vec<(Vec<Operand>, Destination)>, said: ir::action::MoveVerb },
    Draw { players: Operand, amount: expr::Amount },
    Mill { players: Operand, amount: expr::Amount },
    GainLife { players: Operand, amount: expr::Amount },
    LoseLife { players: Operand, amount: expr::Amount },
    /// Tap, or untap (701.26).
    Tap { objects: Vec<Operand>, tapped: bool },
    /// Transform (701.27): the other face of each double-faced permanent among them is up.
    Transform { objects: Vec<Operand> },
    /// Each permanent among them has this level (716.2a).
    Level { objects: Vec<Operand>, level: u32 },
    /// "it becomes monstrous", "it becomes renowned" (701.37a, 702.112a).
    Unprepare { objects: Vec<Operand> },
    Designate { objects: Vec<Operand>, designation: ir::condition::ObjectDesignation },
    /// Turn each face-down permanent among them face up (708.8, 701.40g), or each face-up
    /// one face down (708.2a).
    TurnFace { objects: Vec<Operand>, up: bool },
    /// Fight (701.14): this creature and that one each deal damage equal to its power to
    /// the other. `other`: none said, of "those creatures fight each other", the two `one`
    /// names.
    Fight { one: Operand, other: Vec<Operand> },
    /// Damage dealt by the named sources, as they are or last were (113.7a, 120.4).
    Damage { source: Operand, hits: Vec<(Vec<Operand>, expr::Amount)> },
    /// Remove at most Some(n), or all with None, of one named kind (122.1, 609.3).
    RemoveCounters { from: Operand, counters: CounterRemoval },
    /// Counters put on supported holders: each group, and the counters put on each of its members.
    PutCounters { by: Operand, placements: Vec<(Operand, Vec<(String, expr::Amount)>)> },
    /// Damage the source deals divided among a target group as announced (601.2d): each
    /// target still legal its share (608.2b), in one event (120.4).
    DealDivided { source: Operand, group: usize },
    /// Counters of one kind distributed among a target group as announced (601.2d): each
    /// target still legal its share (608.2b).
    Distribute { by: Operand, group: usize, kind: String },
    /// Tokens created by each player: what each is, how many, and whether tapped (701.7).
    /// `attacking`: each actual token's defender is chosen after replacement ordering,
    /// before its battlefield entry (508.4).
    Create { players: Operand, tokens: Vec<(Token, expr::Amount, bool)>, attacking: bool,
        /// "a Monster Role token attached to it": what each Aura token enters attached to (303.4f).
        attached: Option<Operand> },
    /// Turn-long damage prevention, evaluated once by its resolving occurrence.
    Prevent { to: Vec<Operand>, by: Vec<Operand>, to_or_by: bool, kind: ir::event::DamageKind, next: Option<u32> },
    /// A regeneration shield for each permanent (701.19a).
    Regenerate { objects: Vec<Operand> },
    /// Each Aura or Equipment named is attached to the one object or player `to` names
    /// (701.3a).
    Attach { objects: Vec<Operand>, to: Operand },
}

/// The result an ordinary operation exposes to subsequent instructions (608.2h).
/// Lowered keywords inherit this contract; the binder decides which words name it.
#[derive(Debug, Clone, Copy)]
pub(super) enum Output {
    Countered,
    Destroyed,
    RemovedCounters,
    Created { max: u32 },
    LostLife,
    Damage,
    Milled { controller: bool },
    Moved { destination: Option<Zone> },
}
impl Output {
    pub(super) fn kind(self) -> super::Kind {
        match self {
            Self::Countered => super::Kind::Subjects,
            Self::Destroyed => super::Kind::Destructions,
            Self::RemovedCounters => super::Kind::Removals,
            Self::Created { .. } => super::Kind::Subjects,
            Self::LostLife => super::Kind::Number,
            Self::Damage => super::Kind::Damage,
            Self::Milled { .. } | Self::Moved { .. } => super::Kind::Cards,
        }
    }

    pub(super) fn value(self, result: &mut crate::operation::Completed) -> super::Value {
        match self {
            Self::Countered => super::Value::Subjects(std::mem::take(&mut result.countered).into_iter().map(Target::Object).collect()),
            Self::RemovedCounters => super::Value::Removals(std::mem::take(&mut result.removed)),
            Self::Destroyed => super::Value::Destructions(std::mem::take(&mut result.destroyed)),
            Self::Created { .. } => super::Value::Subjects(std::mem::take(&mut result.arrived).into_iter().map(Target::Object).collect()),
            Self::LostLife => super::Value::Number(result.lost),
            Self::Damage => super::Value::Damage(std::mem::take(&mut result.damage)),
            Self::Milled { .. } => super::Value::Cards(std::mem::take(&mut result.arrived)),
            // A movement's public-zone relationship includes its old and new identities
            // (400.7). Retain only transitions belonging to the completed result.
            Self::Moved { destination: Some(Zone::Battlefield | Zone::Exile) } => super::Value::Cards(result.transitions.iter()
                .filter(|(_, now)| result.arrived.contains(now)).flat_map(|(old, now)| [*old, *now]).collect()),
            // into a hand or a library, or wherever each went: what was moved, as it was and as
            // it is now, for "a permanent was returned this way" (400.7, 608.2c)
            Self::Moved { .. } => super::Value::Cards(result.moved.iter().flat_map(|(old, now)| [*old, *now]).collect()),
        }
    }

    pub(super) fn empty(self) -> super::Value { self.value(&mut crate::operation::Completed::default()) }
}

impl Action {
    pub(super) fn output(&self) -> Option<Output> {
        match self {
            Self::Counter { .. } => Some(Output::Countered),
            Self::Create { players, tokens, .. } => {
                // Printed cardinality describes the noun, never the actual result group.
                let max=if *players==Operand::Controller {tokens.iter().try_fold(0u32,|n,(_,amount,_)|n.checked_add(amount.as_constant()?))} else {None};
                Some(Output::Created {max:max.unwrap_or(u32::MAX)})
            }
            Self::Destroy { .. } => Some(Output::Destroyed),
            Self::RemoveCounters { .. } => Some(Output::RemovedCounters),
            Self::LoseLife { .. } => Some(Output::LostLife),
            Self::Damage { .. } | Self::Fight { .. } | Self::DealDivided { .. } => Some(Output::Damage),
            Self::Mill { players, .. } => Some(Output::Milled { controller: *players == Operand::Controller }),
            Self::Move { groups, .. } => {
                let zone = |to: &Destination| match to {
                    Destination::Battlefield { .. } => Zone::Battlefield,
                    Destination::Zone(zone) => *zone,
                };
                let destination = groups.first().map(|(_, to)| zone(to))
                    .filter(|first| groups.iter().all(|(_, to)| zone(to) == *first));
                Some(Output::Moved { destination })
            }
            Self::BecomeMonarch { .. } | Self::IncreaseSpeed { .. } | Self::GetEmblem { .. }
            | Self::ExileUntil { .. } | Self::Draw { .. } | Self::GainLife { .. }
            | Self::Tap { .. } | Self::Transform { .. } | Self::Level { .. } | Self::Unprepare { .. } | Self::Designate { .. } | Self::TurnFace { .. }
            | Self::Prevent { .. } | Self::PutCounters { .. } | Self::Regenerate { .. }
            | Self::Attach { .. } | Self::Distribute { .. } => None,
        }
    }
    /// "For each" contributes to one action's quantity (608.2f), not repeated individual
    /// damage/draw/life/counter events with intervening state changes.
    pub(super) fn scaled(mut self, each: expr::Amount) -> Res<Self> {
        let scale = |n: &mut expr::Amount| *n = n.clone().times(each.clone());
        match &mut self {
            Self::Draw { amount, .. } | Self::Mill { amount, .. } | Self::GainLife { amount, .. } | Self::LoseLife { amount, .. } => scale(amount),
            Self::Damage { hits, .. } => hits.iter_mut().for_each(|(_, n)| scale(n)),
            Self::PutCounters { placements, .. } => placements.iter_mut().flat_map(|(_, cs)| cs).for_each(|(_, n)| scale(n)),
            Self::Create { tokens, .. } => tokens.iter_mut().for_each(|(_, n, _)| scale(n)),
            _ => return unsupported("for-each: action has no quantity to scale"),
        }
        Ok(self)
    }

    /// Every operand the action reads.
    pub(super) fn operands(&self) -> Vec<&Operand> {
        match self {
            Action::Draw { players, .. } | Action::Mill { players, .. } | Action::GainLife { players, .. }
            | Action::LoseLife { players, .. } | Action::BecomeMonarch { players } | Action::IncreaseSpeed { players } | Action::GetEmblem { players, .. } => vec![players],
            Action::Create { players, tokens, attached, .. } => std::iter::once(players).chain(tokens.iter().filter_map(|(token,_,_)|
                if let Token::Copy { of, .. } = token { Some(of.as_ref()) } else { None })).chain(attached.as_ref()).collect(),
            Action::Move { groups, .. } => groups.iter().flat_map(|(objects, _)| objects).collect(),
            Action::Tap { objects, .. } | Action::Transform { objects } | Action::Level { objects, .. } | Action::Unprepare { objects } | Action::Designate { objects, .. } | Action::TurnFace { objects, .. } | Action::Destroy { objects, .. } | Action::Counter { objects, .. } | Action::ExileUntil { objects } | Action::Regenerate { objects } => objects.iter().collect(),
            Action::Attach { objects, to } => objects.iter().chain(std::iter::once(to)).collect(),
            Action::Damage { source, hits } => std::iter::once(source).chain(hits.iter().flat_map(|(to, _)| to)).collect(),
            Action::Prevent { to, by, .. } => to.iter().chain(by).collect(),
            Action::Fight { one, other } => std::iter::once(one).chain(other).collect(),
            Action::RemoveCounters { from, .. } => vec![from],
            Action::PutCounters { by, placements } => std::iter::once(by).chain(placements.iter().map(|(on, _)| on)).collect(),
            Action::DealDivided { source, .. } => vec![source],
            Action::Distribute { by, .. } => vec![by],
        }
    }

    /// Every number the action says.
    pub(super) fn amounts(&self) -> Vec<&expr::Amount> {
        match self {
            Action::Draw { amount, .. } | Action::Mill { amount, .. } | Action::GainLife { amount, .. }
            | Action::LoseLife { amount, .. } => vec![amount],
            Action::Tap { .. } | Action::Fight { .. } | Action::Transform { .. } | Action::Level { .. } | Action::Unprepare { .. } | Action::Designate { .. } | Action::TurnFace { .. } | Action::Destroy { .. } | Action::Counter { .. } | Action::ExileUntil { .. }
            | Action::Prevent { .. } | Action::RemoveCounters { .. } | Action::BecomeMonarch { .. } | Action::IncreaseSpeed { .. } | Action::Regenerate { .. } | Action::Attach { .. } | Action::GetEmblem { .. }
            | Action::DealDivided { .. } | Action::Distribute { .. } => Vec::new(),
            Action::Damage { hits, .. } => hits.iter().map(|(_, n)| n).collect(),
            Action::PutCounters { placements, .. } => placements.iter().flat_map(|(_, counters)| counters.iter().map(|(_, n)| n)).collect(),
            Action::Create { tokens, .. } => tokens.iter().map(|(_, n, _)| n).collect(),
            Action::Move { groups, .. } => groups.iter().flat_map(|(_, to)| match to {
                Destination::Battlefield { counters, .. } => counters.iter().map(|(_, n)| n).collect::<Vec<_>>(),
                _ => Vec::new(),
            }).collect(),
        }
    }

    /// The chosen values the action reads, for checking they are produced before it.
    pub(super) fn scalars(&self) -> Vec<crate::binding::Scalar> {
        // and the player who puts a permanent onto the battlefield, where a slot holds them
        let putters = match self {
            Action::Move { groups, .. } => groups.iter().filter_map(|(_, to)| match to {
                Destination::Battlefield { by: super::Who::Of(slot), .. } => Some(crate::binding::Scalar::Local { slot: *slot, kind: crate::binding::Kind::Subjects }),
                _ => None,
            }).collect(),
            _ => Vec::new(),
        };
        self.operands().into_iter().flat_map(Operand::scalars)
            .chain(self.amounts().into_iter().flat_map(expr::Amount::scalars)).chain(putters).collect()
    }

    /// Is this an action the engine does? Asked once, as the action is lowered: a player
    /// where the action is done to an object, an object where it is a player's, a counter
    /// the engine does not know. A described set or a target group may hold both kinds;
    /// the action is then done to those it can be done to.
    pub(super) fn check(&self) -> Res<()> {
        let no = |operand: Operand, what: &str| {
            if self.operands().contains(&&operand) { unsupported(what.to_string()) } else { Ok(()) }
        };
        match self {
            Action::Draw { players, .. } | Action::Mill { players, .. } | Action::GainLife { players, .. }
            | Action::LoseLife { players, .. } | Action::Create { players, .. } | Action::BecomeMonarch { players } | Action::IncreaseSpeed { players } | Action::GetEmblem { players, .. } => {
                if *players == Operand::Source { unsupported("an object where the action is a player's") } else { Ok(()) }
            }
            Action::Tap { tapped, .. } => no(Operand::Controller, if *tapped { "tap a player" } else { "untap a player" }),
            Action::Destroy { .. } | Action::Counter { .. } | Action::ExileUntil { .. } | Action::Move { .. } | Action::Transform { .. } | Action::Level { .. } | Action::Unprepare { .. } | Action::Designate { .. } | Action::TurnFace { .. } => no(Operand::Controller, "an object action on a player"),
            Action::Regenerate { .. } => no(Operand::Controller, "regenerate a player"),
            Action::Attach { objects, to } => {
                // what is attached is an object; what it is attached to may be a player (303.4)
                if objects.contains(&Operand::Controller) {
                    return unsupported("attach a player");
                }
                // one destination, which the text names: the engine does not choose among
                // several, and a described set or a union may be several
                match to {
                    Operand::All(_) | Operand::Union(_) => unsupported("attach: choosing between multiple destinations"),
                    _ => Ok(()),
                }
            }
            Action::Damage { .. } | Action::Prevent { .. } | Action::DealDivided { .. } => Ok(()),
            Action::RemoveCounters { counters: CounterRemoval::Named { kind, .. }, .. } | Action::Distribute { kind, .. } => crate::layers::counter_ok(kind),
            Action::RemoveCounters { counters: CounterRemoval::All, .. } => Ok(()),
            Action::Fight { .. } => no(Operand::Controller, "a player that fights"),
            Action::PutCounters { placements, .. } => {
                placements.iter().flat_map(|(_, counters)| counters).try_for_each(|(kind, _)| crate::layers::counter_ok(kind))
            }
        }
    }
}

impl Operand {
    pub(super) fn scalars(&self) -> Vec<crate::binding::Scalar> {
        match self {
            Self::Reference(slot) => vec![crate::binding::Scalar::Local { slot: *slot, kind: crate::binding::Kind::Subjects }],
            Self::ControllerOf(of) | Self::OwnerOf(of) => of.scalars(),
            Self::Matching(Matching {of,filter,..}) => {let mut out=of.scalars();filter.scalars(&mut out);out},
            Self::FormerControllers(FormerControllers { slot, filter }) => {
                let mut out = vec![crate::binding::Scalar::Local { slot: *slot, kind: crate::binding::Kind::Destructions }];
                filter.scalars(&mut out); out
            },
            Self::All(set) => set.scalars(),
            Self::Zone(zone) => zone.scalars(),
            Self::Within { of, place: Place::Pile(zone) } => of.scalars().into_iter().chain(zone.scalars()).collect(),
            Self::Within { of, place: Place::Shared(_) } => of.scalars(),
            Self::Subject(subject) => { let mut scalars = Vec::new(); subject.scalars(&mut scalars); scalars },
            Self::Union(items) => items.iter().flat_map(Self::scalars).collect(),
            _ => Vec::new(),
        }
    }
}

/// What an operand is, now. Nothing is named twice.
pub(super) fn named(game: &Game, f: &Frame, operand: &Operand) -> Vec<Target> {
    match operand {
        Operand::Player(player) => vec![Target::Player(*player)],
        Operand::Convoked => f.inputs.iter().find(|(i,_)|*i==super::input::Input::Convoked).map(|(_,v)|v.clone()).unwrap_or_default(),
        Operand::Subject(subject) => subject.evaluate(game, &f.ctx, f),
        Operand::Union(items) => {
            let mut all = Vec::new();
            for t in items.iter().flat_map(|item| named(game, f, item)) {
                if !all.contains(&t) { all.push(t); }
            }
            all
        }
        Operand::Matching(Matching {of,filter,controller}) => named(game,f,of).into_iter().filter(|t|
            filter.matches(game,&f.ctx,f,*t) && controller.is_none_or(|who| match t {
                Target::Object(object) => who.matches(&f.ctx,game.controller(*object)), Target::Player(_) => false,
            })).collect(),
        Operand::Reference(slot) => match f.slot(*slot) {
            Some(super::Value::Subjects(subjects)) => subjects.clone(),
            _ => unreachable!("the compiler validates reference producers"),
        },
        Operand::ControllerOf(of) | Operand::OwnerOf(of) => named(game, f, of).into_iter().filter_map(|t| match t {
            Target::Object(o) => Some(Target::Player(if matches!(operand, Operand::ControllerOf(_)) { game.controller(o) } else { game.obj(o).owner })),
            Target::Player(_) => None,
        }).collect(),
        Operand::FormerControllers(FormerControllers { slot, filter }) => {
            let mut controllers = Vec::new();
            for before in expr::qualifying_deaths(game, &f.ctx, f, *slot, filter) {
                let player = Target::Player(game.controller(before));
                if !controllers.contains(&player) { controllers.push(player); }
            }
            controllers
        },
        Operand::Controller => vec![Target::Player(f.ctx.controller)],
        Operand::Source => f.ctx.source.into_iter().map(Target::Object).collect(),
        Operand::DefendingPlayer => f.ctx.source.and_then(|source| game.defending_player_of(source)).map(Target::Player).into_iter().collect(),
        Operand::Targets(group) => {
            let spec = f.program.targets.get(*group).expect("a compiled action names a target group of its program");
            let said = f.targets.get(*group).expect("a frame runs with every target group of its program announced");
            let mut legal = Vec::new();
            for t in said {
                if !legal.contains(t) && spec.legal_target(game, &f.ctx, f, said, *t) {
                    legal.push(*t);
                }
            }
            legal
        }
        Operand::All(set) => set.members(game, &f.ctx, f),
        Operand::Zone(zone) => zone.members(game, &f.ctx, f).into_iter().map(Target::Object).collect(),
        // named as it was, for what it last was (608.2h); kept only where it is now there
        Operand::Within { of, place } => {
            let members = match place { Place::Pile(zone) => zone.members(game, &f.ctx, f), Place::Shared(_) => Vec::new() };
            let there = |now: ObjId| match place { Place::Pile(_) => members.contains(&now), Place::Shared(zone) => game.obj(now).zone == *zone };
            named(game, f, of).into_iter().filter(|t| match t {
                Target::Object(o) => move_identity(game, f, *o).is_some_and(there),
                Target::Player(_) => false,
            }).collect()
        }
    }
}

/// Information projections may read departed identities (608.2h); actions cannot act on
/// their former incarnations (400.7). Do not silently follow a zone-change successor.
pub(super) fn found(game: &Game, f: &Frame, operand: &Operand) -> Vec<Target> {
    named(game, f, operand).into_iter().filter(|t| match t {
        Target::Object(o) => game.obj(*o).zone != Zone::Gone,
        Target::Player(p) => !game.players[*p].lost,
    }).collect()
}

/// Everything these operands name together, each once.
/// Each target of a divided group that is still legal, with what the division announced
/// gives it (601.2d, 608.2b): what an illegal target would have got, no one gets.
fn divided(game: &Game, f: &Frame, group: usize) -> Vec<(Target, u32)> {
    let legal = named(game, f, &Operand::Targets(group));
    let said = f.targets.get(group).map_or(&[][..], Vec::as_slice);
    let shares = f.division.get(group).map_or(&[][..], Vec::as_slice);
    said.iter().zip(shares).filter(|(t, _)| legal.contains(t)).map(|(t, n)| (*t, *n)).collect()
}

fn recipients(game: &Game, f: &Frame, operands: &[Operand]) -> Vec<Target> {
    let mut all = Vec::new();
    for t in operands.iter().flat_map(|operand| found(game, f, operand)) {
        if !all.contains(&t) {
            all.push(t);
        }
    }
    all
}

/// The players an operand names, in APNAP order (101.4).
pub(super) fn players(game: &Game, f: &Frame, operand: &Operand) -> Vec<PlayerId> {
    let mut players: Vec<PlayerId> =
        recipients(game, f, std::slice::from_ref(operand)).into_iter().filter_map(|t| if let Target::Player(p) = t { Some(p) } else { None }).collect();
    let (n, active) = (game.players.len(), game.active);
    players.sort_by_key(|p| (p + n - active) % n);
    players
}

/// The objects these operands name together, each once.
pub(super) fn objects(game: &Game, f: &Frame, operands: &[Operand]) -> Vec<ObjId> {
    recipients(game, f, operands).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None }).collect()
}

/// Follow only the public-zone transition supplied by this occurrence (400.7e/j), and then
/// the one a madness trigger left the discarded card (400.7k). Reading characteristics still
/// uses the original identity for last known information.
pub(super) fn move_identity(game: &Game, f: &Frame, old: ObjId) -> Option<ObjId> {
    let live = |o: ObjId| game.obj(o).zone != Zone::Gone;
    if live(old) { return Some(old); }
    // Follow only public transitions caused by this occurrence (400.7e/j), never
    // the game's unrelated identity lineage. Every zone change creates a fresh ID.
    let mut now = old;
    while let Some((_, after)) = f.transitions.iter().find(|(was, _)| *was == now) {
        now = *after;
        if live(now) { return Some(now); }
    }
    // the card discarded into exile, which its madness trigger moved on without casting it
    game.madness_found.iter().find(|(was, _)| *was == now).map(|(_, found)| *found).filter(|found| live(*found))
}

/// The objects these operands name, each once, as this occurrence's own moves left them:
/// "exile this creature. You may cast it" is of the card the creature became.
pub(super) fn moved(game: &Game, f: &Frame, operands: &[Operand]) -> Vec<ObjId> {
    let mut all = Vec::new();
    for t in operands.iter().flat_map(|operand| named(game, f, operand)) {
        if let Target::Object(old) = t && let Some(now) = move_identity(game, f, old) && !all.contains(&now) {
            all.push(now);
        }
    }
    all
}

/// The action with its operands read from the game as it now is. Nothing changes here, and
/// what is returned no longer reads the game: it is done as it was read, however long the
/// doing waits on a question.
pub(super) fn evaluate(game: &Game, f: &Frame, action: &Action) -> crate::Res<perform::Action> {
    Ok(match action {
        Action::BecomeMonarch { players: who } => perform::Action::BecomeMonarch { players: players(game,f,who) },
        Action::IncreaseSpeed { players: who } => perform::Action::IncreaseSpeed { players: players(game,f,who) },
        Action::GetEmblem { players: who, definition } => perform::Action::GetEmblem { players: players(game, f, who), definition: *definition },
        Action::Destroy { objects: on, regenerable } => perform::Action::Destroy { objects: objects(game, f, on), regenerable: *regenerable },
        Action::Counter { objects: on, destination } => perform::Action::Counter { objects: objects(game, f, on), destination: *destination },
        Action::ExileUntil { objects: on } => perform::Action::ExileUntil { objects: objects(game, f, on), source: f.ctx.object_source(), ability: f.ctx.ability.clone(), generation: f.ctx.link_generation },
        Action::Move { groups, .. } => {
            // 400.7e/j: the triggering move or a paid cost supplies a specific public-zone
            // successor. This permission is not a general search for the same physical card.
            // Other reads still use the old identity for LKI, and ordinary target captures
            // do not acquire permission to follow an unrelated zone change.
            let mut moves = Vec::new();
            for (on, zone) in groups {
                for target in on.iter().flat_map(|operand| named(game, f, operand)) {
                    let Target::Object(old) = target else { continue };
                    let object = move_identity(game, f, old);
                    if let Some(object) = object {
                        let movement = match zone {
                            Destination::Zone(Zone::Exile) => perform::Movement::Exile { object, source: f.ctx.object_source(), ability: f.ctx.ability.clone(), generation: f.ctx.link_generation },
                            Destination::Zone(zone) => perform::Movement::Zone { object, zone: *zone },
                            Destination::Battlefield { owner_controls, by, tapped, counters, attacking, transformed, face_down } => {
                                let controller = if *owner_controls { game.obj(object).owner } else { f.player(*by) };
                                let mut arrivals = if *tapped { vec![crate::replace::Enters::Tapped] } else { Vec::new() };
                                for (kind,count) in counters {
                                    arrivals.push(crate::replace::Enters::Counters {kind:kind.clone(),count:expr::Amount::constant(count.checked(game,&f.ctx,f)?)});
                                }
                                let mut chosen = crate::replace::Choices::arriving(arrivals);
                                chosen.transformed = *transformed;
                                chosen.face_down = *face_down;
                                chosen.attacking = attacking.and_then(|input| f.inputs.iter().find(|(key,_)| *key == input)).and_then(|(_,targets)| targets.first()).copied();
                                perform::Movement::Enter(EntryRequest::card(object, controller, chosen, true))
                            }
                        };
                        if !moves.contains(&movement) { moves.push(movement); }
                    }
                }
            }
            perform::Action::Move(moves)
        },
        Action::Draw { players: who, amount: n } | Action::Mill { players: who, amount: n }
        | Action::GainLife { players: who, amount: n } | Action::LoseLife { players: who, amount: n } => {
            let kind = match action {
                Action::Draw { .. } => PlayerAction::Draw,
                Action::Mill { .. } => PlayerAction::Mill,
                Action::GainLife { .. } => PlayerAction::GainLife,
                _ => PlayerAction::LoseLife,
            };
            // An amount of life past what the engine counts in is refused, never taken as
            // the most it counts (107.1b): either would change the life total it reaches.
            let life = matches!(kind, PlayerAction::GainLife | PlayerAction::LoseLife);
            let players = players(game, f, who).into_iter().map(|p| Ok((p, if life { n.checked_for_actor(game, &f.ctx, f, p)? }
                else { n.for_actor(game, &f.ctx, f, p) }))).collect::<crate::Res<_>>()?;
            perform::Action::Players { kind, players }
        }
        Action::RemoveCounters { from, counters } => perform::Action::RemoveCounters { from: found(game, f, from), counters: counters.clone() },
        Action::Tap { objects: of, tapped } => perform::Action::Tap { objects: objects(game, f, of), tapped: *tapped },
        Action::Transform { objects: of } => {
            // an ability of the permanent itself, and when it was put on the stack (701.27f)
            let own = game.stack.last().map(|resolving| game.obj(*resolving))
                .filter(|resolving| resolving.ability.is_some_and(|(source, _)| source == f.ctx.source))
                .map(|resolving| (f.ctx.object_source(), resolving.stamp));
            perform::Action::Transform { objects: objects(game, f, of), own }
        }
        Action::Level { objects: of, level } => perform::Action::Level { objects: objects(game, f, of), level: *level },
        Action::Unprepare { objects: of } => perform::Action::Unprepare { objects: objects(game, f, of) },
        Action::Designate { objects: of, designation } => perform::Action::Designate { objects: objects(game, f, of), designation: *designation },
        Action::TurnFace { objects: of, up } => perform::Action::TurnFace { objects: objects(game, f, of), up: *up },
        Action::Fight { one, other } => {
            let mut fighters: Vec<ObjId> = named(game, f, one).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None }).collect();
            if other.is_empty() {
                // "those creatures fight each other": the two it names
            } else if let ([_], [other]) = (fighters.as_slice(), objects(game, f, other).as_slice()) {
                fighters.push(*other);
            } else {
                fighters.clear();
            }
            // neither fights unless both are creatures on the battlefield, and legal targets
            // where they are targets (701.14b); one that fights itself is dealt twice its
            // power (701.14c). The damage is dealt at once, and is not combat damage (701.14d)
            let fights = |o: ObjId| game.obj(o).zone == Zone::Battlefield && game.is_a(o, ir::CardType::Creature);
            let damage = match fighters[..] {
                [a, b] if fights(a) && fights(b) => [(a, b), (b, a)].into_iter()
                    .map(|(from, to)| (from, Target::Object(to), u32::try_from(game.power(from).unwrap_or(0)).unwrap_or(0))).collect(),
                _ => Vec::new(),
            };
            perform::Action::Damage(damage)
        }
        Action::Prevent { to, by, to_or_by, kind, next } => {
            fn affected(game: &Game, f: &Frame, op: &Operand) -> crate::statement::Affected {
                match op {
                    Operand::All(set) => crate::statement::Affected::Compiled { set: set.clone(), locals: f.slots.clone() },
                    Operand::Union(items) => crate::statement::Affected::Union(items.iter().map(|o| affected(game,f,o)).collect()),
                    other => crate::statement::Affected::Things(found(game,f,other)),
                }
            }
            let party = |ops: &[Operand]| (!ops.is_empty()).then(|| crate::statement::Affected::Union(ops.iter().map(|o|affected(game,f,o)).collect()));
            let to = if next.is_some() { Some(crate::statement::Affected::Things(to.iter().flat_map(|o|found(game,f,o)).collect())) } else {party(to)};
            let effect = crate::prevention::Effect {
                id: crate::operation::ReplacementId::from_ability(f.occurrence.expect("prevention requires a resolution occurrence"), f.ctx.ability.clone().expect("compiled prevention ability"), f.occurrence_base + f.pc),
                ctx: f.ctx.clone(), to, by: party(by), to_or_by: *to_or_by, kind: *kind, remaining: *next,
            };
            perform::Action::Prevent(effect)
        }
        Action::Damage { source, hits } => {
            // one simultaneous event, of numbers counted before any of it is dealt (120.4)
            let mut damage = Vec::new();
            for (to, n) in hits {
                for source in named(game, f, source) {
                    if let Target::Object(source) = source {
                        for t in recipients(game,f,to) {
                            let count=match t {
                                Target::Player(p)=>n.checked_for_actor(game,&f.ctx,f,p)?,
                                _=>n.checked_for_source(game,&f.ctx,f,source)?,
                            };
                            damage.push((source,t,count));
                        }
                    }
                }
            }
            perform::Action::Damage(damage)
        }
        Action::DealDivided { source, group } => {
            let mut damage = Vec::new();
            for source in named(game, f, source) {
                if let Target::Object(source) = source {
                    damage.extend(divided(game, f, *group).into_iter().map(|(to, n)| (source, to, n)));
                }
            }
            perform::Action::Damage(damage)
        }
        Action::Distribute { by, group, kind } => {
            let mut batch = Vec::new();
            for by in players(game, f, by) {
                batch.extend(divided(game, f, *group).into_iter()
                    .map(|(on, count)| Placement { on, kind: kind.clone(), count, by }));
            }
            perform::Action::PutCounters(batch)
        },
        Action::PutCounters { by, placements } => {
            let mut batch = Vec::new();
            for by in players(game, f, by) {
                for (on, counters) in placements {
                    let on = named(game, f, on);
                    for (kind, n) in counters {
                        for on in &on {
                            let count = match on {
                                Target::Player(p) => n.checked_for_actor(game, &f.ctx, f, *p)?,
                                Target::Object(_) => n.checked(game, &f.ctx, f)?,
                            };
                            batch.push(Placement { on: *on, kind: kind.clone(), count, by });
                        }
                    }
                }
            }
            perform::Action::PutCounters(batch)
        }
        Action::Create { players: who, tokens, attacking, attached } => {
            let who = players(game, f, who);
            // what the text attaches it to, where it still is (400.7); an Aura token attached
            // to what is not there, or what it cannot enchant, is not created (303.4i)
            let attached = match attached {
                Some(to) => match found(game, f, to).into_iter().next() {
                    Some(to) => Some(to),
                    None => return Ok(perform::Action::Create(Vec::new())),
                },
                None => None,
            };
            let mut requests = Vec::new();
            for (token, n, tapped) in tokens {
                let values = match token {
                    Token::Defined(token) => token.clone(),
                    Token::Copy { of, changes } => {
                        let sources=named(game,f,of);
                        let source=match sources.as_slice() {[]=>continue,[Target::Object(source)]=>*source,
                            _=>return Err(crate::Unsupported("token copy requires one exact source, not a group".into()))};
                        game.token_copy(source).changed(game, changes)
                    },
                };
                for p in &who {
                    let n = n.checked_for_actor(game, &f.ctx, f, *p)?;
                    let mut request=crate::creation::Request::new(*p,n,values.clone(),*tapped);
                    request.choose_defender=*attacking;request.attached=attached;
                    requests.push(request);
                }
            }
            perform::Action::Create(requests)
        }
        // a permanent is regenerated (701.19a): what is named elsewhere gets no shield
        Action::Regenerate { objects: of } => {
            perform::Action::Regenerate(objects(game, f, of).into_iter().filter(|o| game.obj(*o).zone == Zone::Battlefield).collect())
        }
        Action::Attach { objects: what, to } => {
            // The destination is the identity the text names, where it still is: a departed
            // one is not followed to what it became (400.7), and an illegal target is not
            // attached to (608.2b). Whether the attachment is legal is the operation's to
            // say, as it is done (701.3b).
            let to = found(game, f, to).into_iter().next();
            perform::Action::Attach(to.into_iter().flat_map(|to| objects(game, f, what).into_iter().map(move |o| (o, to))).collect())
        }
        })
}
