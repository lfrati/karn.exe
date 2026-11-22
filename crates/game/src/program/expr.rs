//! Compiled operand expressions: which objects and players a description names, and what
//! number an amount is.
//!
//! Lowering ([`Set::compile`], [`Cards::compile`], [`Amount::compile`]) is the one place a
//! description or amount with unsupported syntax is refused, using the definition rather
//! than a running game. Compiled expressions are owned and typed; checked quantity
//! evaluation can still refuse runtime values outside the represented numerical range.
//! The noun phrases of text not yet compiled ([`crate::select`]) are lowered by the same
//! functions and evaluated by the same evaluators.
use super::input::Input;
use crate::binding::{Kind, Scalar, Value, fits};
use crate::event::Target;
use crate::select::{Ctx, Support, counters_on, is_you, name};
use crate::state::{Game, ObjId, PlayerId, Zone};
use crate::{Res, unsupported};
use ir::amount::{Aggregate, Cmp};
use ir::reference::{RefForm, UnresolvedRef};
use ir::{Count, Quant, Ref, Sort, Status};

/// The values a running frame holds that an expression may read: this execution's own
/// choices. A linked cell is read from the source object by the evaluator itself.
pub trait Locals {
    fn local(&self, slot: usize) -> Option<&Value>;
    /// The lexical player of a per-player quantity, never the effect's controller.
    fn actor(&self) -> Option<PlayerId> { None }
    /// The current object source of a distributive damage amount.
    fn object_actor(&self) -> Option<ObjId> { None }
    fn targets(&self, _group: usize) -> Option<&[Target]> { None }
    /// Did this effect reveal this object to all players (701.20a)? What it then asks of a
    /// revealed face-down permanent, it asks of the card (708.12).
    fn revealed(&self, _o: ObjId) -> bool { false }

    /// What this occurrence of the ability was given under that name (608.2k): the
    /// identities as they were retained. None where there is no occurrence.
    fn event_amount(&self) -> Option<u32> { None }

    fn input(&self, _key: Input) -> Option<&[Target]> {
        None
    }
    /// The public-zone transition retained by this occurrence, not a later incarnation.
    fn successor(&self, _was: ObjId) -> Option<ObjId> { None }
}

impl Locals for Vec<Option<Value>> {
    fn local(&self, slot: usize) -> Option<&Value> { self.get(slot).and_then(Option::as_ref) }
}

/// What of the game a compiled description, number or condition can read, as far as a proof
/// that something leaves it unchanged needs to know. A set bit says it may read that; what
/// is not named here (types, colors, control, zones, counters, names, life) is in no bit.
/// A kind of expression this does not look into is [`Reads::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reads(u8);

impl Reads {
    pub(crate) const NONE: Reads = Reads(0);
    /// What abilities an object has: its keywords and the abilities given to it (613.1f).
    pub(crate) const ABILITIES: Reads = Reads(1);
    /// Whether a permanent is tapped (110.5).
    pub(crate) const TAP_STATE: Reads = Reads(2);
    /// What has happened: this turn's events, what entered this turn, how a spell was cast.
    pub(crate) const HISTORY: Reads = Reads(4);
    pub(crate) const ALL: Reads = Reads(u8::MAX);

    pub(crate) fn any(self, of: Reads) -> bool { self.0 & of.0 != 0 }
    fn each<'a, T: 'a>(items: impl IntoIterator<Item = &'a T>, read: impl Fn(&'a T) -> Reads) -> Reads {
        items.into_iter().fold(Reads::NONE, |all, item| all | read(item))
    }
}

impl std::ops::BitOr for Reads {
    type Output = Reads;
    fn bitor(self, other: Reads) -> Reads { Reads(self.0 | other.0) }
}

/// For whoever evaluates without a frame: there is no local choice to read.
pub struct NoLocals;

impl Locals for NoLocals {
    fn local(&self, _: usize) -> Option<&Value> {
        None
    }
}

/// How what the text refers back to is bound, by the caller who knows which producers are
/// in scope.
pub trait Binding {
    /// "The chosen color" and its like: a slot of this execution or a linked cell (607.2d).
    fn scalar(&mut self, r: &UnresolvedRef) -> Res<Scalar>;

    /// "It", "that card", "that player": what an earlier instruction or the occurrence
    /// named. With no such context only the source and its controller can be said.
    fn subject(&mut self, s: &ir::Selector) -> Res<Subject> {
        Subject::fixed(s)
    }

    /// Characteristic amounts read from the named card preserve its earlier identity
    /// (608.2h). Other predicates/actions may name its completed public-zone successor.
    fn amount_subject(&mut self, s: &ir::Selector) -> Res<Subject> { self.subject(s) }

    /// The retained input of the immediately preceding movement, before its zone change
    /// (608.2h/i). An ordinary reference alone does not prove that earlier time.
    fn past_subject(&mut self, _: &ir::Selector) -> Res<Subject> {
        unsupported("past condition: no retained movement input")
    }

    /// Refuse scalar interpretations of a potentially replacement-expanded created group.
    fn singular_read(&self, _: &Subject) -> Res<()> { Ok(()) }

    /// The explicit object acting in the current recipient query (608.2h). None preserves
    /// ordinary effect-source exclusions; this does not rebind “you” or other references.
    fn other_subject(&self) -> Option<Subject> { None }

    /// Lexical exclusion at a clause/condition root, never a nested filter or amount.
    fn root_other(&self, _: &ir::ObjectSet) -> Res<Option<Subject>> { Ok(None) }

    /// A Controls condition's player is part of its root description, even when the
    /// noun phrase itself has no controller qualifier. It is proof context, not a filter.
    fn root_controlled_other(&self, set: &ir::ObjectSet, _: &ir::Selector) -> Res<Option<Subject>> {
        self.root_other(set)
    }

    /// Whether the sentence being lowered says what X is ("where X is …", 107.3c). The
    /// binder that answers yes is told by the asking that X is read, and computes it once
    /// before what reads it.
    fn x(&mut self) -> bool {
        false
    }

    /// Cast payment is available to resolving instructions, never target announcement
    /// before costs are paid (601.2c/h). Other binding contexts refuse this read.
    fn cast_payment_available(&self) -> bool { false }

    /// Completed cards from a uniquely identified action-result producer.
    fn removals(&mut self) -> Res<super::Slot> { unsupported("counter removal result: no producer in this scope") }
    fn destructions(&mut self) -> Res<super::Slot> { unsupported("destruction result: no producer in this scope") }

    /// `original` requests the completed producer's explicit discard-time card projection.
    fn result_cards(&mut self, _: ir::Verb, _: bool) -> Res<super::Slot> {
        unsupported("result quantity: no action-result producer in this scope")
    }

    /// Every completed instruction of this scope that "this way" with this verb names, in
    /// order (608.2c): one group, or several the text names together.
    /// `bool`: each was done by the controller alone.
    /// `verb`: None names what any instruction did; `into`: only what it put there.
    fn this_way(&mut self, _: Option<ir::Verb>, _: Option<Zone>) -> Res<Vec<(super::Slot, Shape, bool)>> {
        unsupported("result group: no action-result producer in this scope")
    }

    /// The actor group for a uniquely identified prior object payment's outcome.
    fn player_outcome(&mut self, _: ir::condition::Outcome) -> Res<super::Slot> {
        unsupported("player outcome: no object-payment producer in this scope")
    }

    /// The checked completed quantity named by a positive condition in this branch.
    fn condition_amount(&mut self) -> Option<Amount> { None }

    /// The immediately preceding numeric definition, when "that number" names it.
    fn number(&mut self) -> Res<Amount> { unsupported("that number: no preceding numeric producer") }
    /// The group of objects the latest instruction before this one that made one made:
    /// "Exile all attacking creatures target player controls. That player may search their
    /// library for that many …".
    fn latest_group(&mut self) -> Res<(super::Slot, Shape)> { unsupported("that many: no preceding group of objects") }

    /// Actual exile results for an explicit "other cards exiled with" reader.
    fn exiled(&mut self) -> Res<super::Slot> { unsupported("other linked cards: no preceding exile result") }

    /// The actual objects moved by the uniquely identified entry instruction.
    fn entered(&mut self) -> Res<super::Slot> {
        unsupported("movement reference: no earlier battlefield-entry producer")
    }

    /// Identify the instruction that produced the referenced numeric result.
    fn damage(&mut self) -> Res<super::Slot> { unsupported("damage this way: no completed producer") }

    fn life_lost(&mut self) -> Res<super::Slot> {
        unsupported("life lost this way: no result producer")
    }

}

/// A binder of chosen values alone: text with no subject context.
impl<F: FnMut(&UnresolvedRef) -> Res<Scalar>> Binding for F {
    fn scalar(&mut self, r: &UnresolvedRef) -> Res<Scalar> {
        self(r)
    }
}

pub type Bind<'b> = &'b mut dyn Binding;

/// A statement whose explicit subject is its source binds "it" to that source.
/// Other references retain the surrounding binder's meaning.
pub(crate) struct SourceBinding<'a> { pub parent: &'a mut dyn Binding, pub source: bool }
impl Binding for SourceBinding<'_> {
    fn scalar(&mut self, r: &UnresolvedRef) -> Res<Scalar> { self.parent.scalar(r) }
    fn subject(&mut self, s: &ir::Selector) -> Res<Subject> {
        if self.source && s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) { Ok(Subject::Source) }
        else { self.parent.subject(s) }
    }
    fn singular_read(&self, subject: &Subject) -> Res<()> { self.parent.singular_read(subject) }
    fn other_subject(&self) -> Option<Subject> { self.parent.other_subject() }
    fn x(&mut self) -> bool { self.parent.x() }
    fn cast_payment_available(&self) -> bool { self.parent.cast_payment_available() }
    fn condition_amount(&mut self) -> Option<Amount> { self.parent.condition_amount() }
    fn number(&mut self) -> Res<Amount> { self.parent.number() }
    fn exiled(&mut self) -> Res<super::Slot> { self.parent.exiled() }
    fn entered(&mut self) -> Res<super::Slot> { self.parent.entered() }
    fn damage(&mut self) -> Res<super::Slot> { self.parent.damage() }
    fn life_lost(&mut self) -> Res<super::Slot> { self.parent.life_lost() }
}

/// Who or what a number is read from: "its power", "their hand", "that player's life". It
/// names identities that were retained, as they were retained: an object that has since
/// left its zone is still that object, with what it last was (608.2h), and nothing is
/// looked up again by what happened most recently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// The object whose text this is.
    Source,
    /// The player whose text this is ("you", 109.5).
    Controller,
    /// Physical commanders owned by this player, wherever their current incarnation is.
    Commanders(Box<Subject>),
    /// The player for whom an action's quantity is being evaluated.
    Actor,
    /// One source member of a distributive damage instruction, independent of its targets.
    ObjectActor,
    /// What an earlier instruction of this execution retained in that slot.
    Local(usize),
    /// What the occurrence was given under that name.
    Input(Input),
    /// A named occurrence participant in the public zone its event moved it to (400.7e).
    MovedInput(Input),
    Targets(usize),
    Union(Vec<Subject>),
    /// All players matching an explicit quantified description, not an anaphoric reference.
    Players(Box<Set>),
    ControllerOf(Box<Subject>),
    OwnerOf(Box<Subject>),
    Attached,
    LinkedExile(crate::binding::AbilityKey),
}

impl Subject {
    /// What can be said with no context: the source and its controller.
    fn fixed(s: &ir::Selector) -> Res<Subject> {
        match &s.quant {
            Quant::Definite(Ref::This) => Ok(Subject::Source),
            Quant::Definite(Ref::You) => Ok(Subject::Controller),
            _ => unsupported("reference: nothing in scope supplies it"),
        }
    }

    /// "Its controller" and "its owner" are of whatever the inner reference is bound to;
    /// everything else is the binder's to say.
    pub(crate) fn compile(s: &ir::Selector, bind: Bind<'_>) -> Res<Subject> { Self::compile_read(s, bind, false) }
    fn compile_amount(s: &ir::Selector, bind: Bind<'_>) -> Res<Subject> { Self::compile_read(s, bind, true) }
    fn compile_read(s: &ir::Selector, bind: Bind<'_>, amount: bool) -> Res<Subject> {
        if s.quant==Quant::The && s.set.domain==ir::Domain::Card && s.set.zone.is_none() && s.set.controller.is_none() && s.set.caster.is_none() && !s.set.other
            && s.set.filter==ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Status(ir::selector::Status::Commander))])
            && let Some(owner)=&s.set.owner && !owner.negated && !owner.past {
            return Ok(Subject::Commanders(Box::new(Self::compile_read(&owner.who,bind,amount)?)));
        }
        match &s.quant {
            Quant::Definite(Ref::Either(items)) => Ok(Subject::Union(items.iter().map(|s|Self::compile_read(s,bind,amount)).collect::<Res<_>>()?)),
            Quant::Definite(Ref::This | Ref::You) => Subject::fixed(s),
            Quant::Definite(Ref::AttachedTo(_)) => Ok(Subject::Attached),
            Quant::Definite(Ref::ControllerOf(of)) => Ok(Subject::ControllerOf(Box::new(Self::compile_read(of, bind, amount)?))),
            Quant::Definite(Ref::OwnerOf(of)) => Ok(Subject::OwnerOf(Box::new(Self::compile_read(of, bind, amount)?))),
            _ => if amount { bind.amount_subject(s) } else { bind.subject(s) },
        }
    }

    /// The identities it names, in the order they were retained. An unfilled slot or an
    /// input the occurrence does not have names nothing.
    pub fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<Target> {
        let players = |of: &Subject, who: &dyn Fn(&Game, ObjId) -> PlayerId| {
            let mut out = Vec::new();
            for t in of.evaluate(game, ctx, locals) {
                let p = match t {
                    Target::Object(o) => Target::Player(who(game, o)),
                    player => player,
                };
                if !out.contains(&p) {
                    out.push(p);
                }
            }
            out
        };
        match self {
            Subject::Players(set) => set.members(game, ctx, locals),
            Subject::Commanders(owner) => {
                let owners=owner.evaluate(game,ctx,locals);
                owners.into_iter().filter_map(|t|if let Target::Player(p)=t {Some(p)} else {None}).flat_map(|p|game.owned_commanders(p)).map(Target::Object).collect()
            },
            Subject::Targets(group) => locals.targets(*group).unwrap_or_default().to_vec(),
            Subject::Union(items) => {
                let mut out=Vec::new();
                for target in items.iter().flat_map(|s|s.evaluate(game,ctx,locals)) {if !out.contains(&target){out.push(target);}}
                out
            },
            Subject::Source => ctx.source.into_iter().map(Target::Object).collect(),
            Subject::Attached => ctx.source.and_then(|source| game.obj(source).attached_to).into_iter().collect(),
            Subject::LinkedExile(writer) => game.exiled_with.iter().filter_map(|(source, ability, card, generation)|
                (Some(*source) == ctx.source && *ability == Some(ctx.link(writer.clone())) && *generation == ctx.link_generation
                    && game.linked_generation_live(*source, &ctx.link(writer.clone()), *generation)
                    && game.obj(*card).zone == Zone::Exile).then_some(Target::Object(*card)))
                // or what paying its spell's cost exiled: "behold a Kithkin and exile it" (607.2q)
                .chain(ctx.source.into_iter().flat_map(|source| game.obj(source).cast_exiled.iter())
                    .filter(|(key, card)| key == writer && game.obj(*card).zone == Zone::Exile).map(|(_, card)| Target::Object(*card)))
                .collect(),
            Subject::Controller => vec![Target::Player(ctx.controller)],
            Subject::Actor => vec![Target::Player(locals.actor().expect("a per-player quantity owns its actor"))],
            Subject::ObjectActor => vec![Target::Object(locals.object_actor().expect("a per-source quantity owns its actor"))],
            Subject::Local(slot) => match locals.local(*slot) {
                Some(Value::Subjects(subjects)) => subjects.clone(),
                Some(Value::Cards(cards)) => cards.iter().copied().map(Target::Object).collect(),
                _ => Vec::new(),
            },
            Subject::Input(key) => locals.input(*key).map(<[Target]>::to_vec).unwrap_or_default(),
            Subject::MovedInput(key) => locals.input(*key).unwrap_or_default().iter().map(|t| match t {
                Target::Object(old) => Target::Object(locals.successor(*old).unwrap_or(*old)),
                other => *other,
            }).collect(),
            Subject::ControllerOf(of) => players(of, &|game, o| ctx.controlled_by(game, o)),
            Subject::OwnerOf(of) => players(of, &|game, o| game.obj(o).owner),
        }
    }

    fn objects(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> impl Iterator<Item = ObjId> {
        self.evaluate(game, ctx, locals).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None })
    }

    fn players(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> impl Iterator<Item = PlayerId> {
        self.evaluate(game, ctx, locals).into_iter().filter_map(|t| if let Target::Player(p) = t { Some(p) } else { None })
    }

    /// The slots it reads, for checking they are filled before it.
    pub fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Subject::Union(items) => items.iter().for_each(|s|s.scalars(out)),
            Subject::Players(set) => out.extend(set.scalars()),
            Subject::Targets(_) => {},
            Subject::Local(slot) => out.push(Scalar::Local { slot: *slot, kind: Kind::Subjects }),
            Subject::ControllerOf(of) | Subject::OwnerOf(of) | Subject::Commanders(of) => of.scalars(out),
            Subject::Source | Subject::Controller | Subject::Actor | Subject::ObjectActor | Subject::Input(_) | Subject::MovedInput(_) | Subject::Attached | Subject::LinkedExile(_) => {}
        }
    }
}

/// Per-player instruction scope, without changing the effect controller.
pub(crate) struct Actor<'a> {
    pub parent: &'a dyn Locals,
    pub player: PlayerId,
}
impl Locals for Actor<'_> {
    fn successor(&self, was: ObjId) -> Option<ObjId> { self.parent.successor(was) }
    fn local(&self, slot: usize) -> Option<&Value> { self.parent.local(slot) }
    fn input(&self, key: Input) -> Option<&[Target]> { self.parent.input(key) }
    fn event_amount(&self) -> Option<u32> { self.parent.event_amount() }
    fn actor(&self) -> Option<PlayerId> { Some(self.player) }
    fn targets(&self, group: usize) -> Option<&[Target]> { self.parent.targets(group) }
}

/// An ephemeral source binding while one grouped damage request is prepared.
struct ObjectActor<'a> { parent: &'a dyn Locals, source: ObjId }
impl Locals for ObjectActor<'_> {
    fn successor(&self, was: ObjId) -> Option<ObjId> { self.parent.successor(was) }
    fn local(&self, slot: usize) -> Option<&Value> { self.parent.local(slot) }
    fn input(&self, key: Input) -> Option<&[Target]> { self.parent.input(key) }
    fn event_amount(&self) -> Option<u32> { self.parent.event_amount() }
    fn actor(&self) -> Option<PlayerId> { self.parent.actor() }
    fn object_actor(&self) -> Option<ObjId> { Some(self.source) }
    fn targets(&self, group: usize) -> Option<&[Target]> { self.parent.targets(group) }
}

/// Binding through the definition's links alone, for text with no local producer in scope.
pub(crate) fn linked<'s>(support: &'s Support<'_>) -> impl FnMut(&UnresolvedRef) -> Res<Scalar> + 's {
    move |r| {
        let (link, kind) = support.choice(r.expected)?;
        Ok(Scalar::Linked { link, kind })
    }
}

/// A power, toughness or mana value: the numbers of an object the engine can say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stat {
    Power,
    Toughness,
    /// The sum of its power and its toughness: "total power and toughness" (ruling of
    /// 2022-09-09 on Cut Down).
    PowerAndToughness,
    ManaValue,
}

impl Stat {
    pub(crate) fn compile(stat: ir::Stat, what: &str) -> Res<Stat> {
        match stat {
            ir::Stat::Power => Ok(Stat::Power),
            ir::Stat::Toughness => Ok(Stat::Toughness),
            ir::Stat::PowerAndToughness => Ok(Stat::PowerAndToughness),
            ir::Stat::ManaValue => Ok(Stat::ManaValue),
            // base values are the layer 7b result (613.4b), which the engine does not keep apart
            other => unsupported(format!("{what}: {}", name(&other))),
        }
    }

    /// None: it has no such number (a noncreature's power).
    fn of(self, game: &Game, ctx: &Ctx, o: ObjId) -> Option<i64> {
        match self {
            Stat::Power => ctx.seen(game, o).pt.map(|(p, _)| i64::from(p)),
            Stat::Toughness => ctx.seen(game, o).pt.map(|(_, t)| i64::from(t)),
            Stat::PowerAndToughness => ctx.seen(game, o).pt.map(|(p, t)| i64::from(p) + i64::from(t)),
            Stat::ManaValue => Some(i64::from(ctx.mana_value(game, o))),
        }
    }

    /// An object's number now, or as it last was on the battlefield when it has left it
    /// (608.2h); zero where it has none.
    pub(crate) fn value(self, game: &Game, ctx: &Ctx, o: ObjId) -> i64 {
        self.of(game, ctx, o).unwrap_or(0)
    }
}

/// What is said of one object or player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Atom {
    /// At least one announced target matches one complete description (115.9b).
    Targets(Vec<Set>),
    Type(ir::CardType),
    PermanentType,
    HasAdventure,
    PreparedSpell,
    /// Its printed mana cost is one of these (202.1).
    ManaCost(Vec<ir::mana::ManaCost>),
    Subtype(ir::Subtype),
    BasicLandType,
    Supertype(ir::Supertype),
    Color(ir::Color),
    Colorless,
    Multicolored,
    Monocolored,
    Opponent,
    Keyword(ir::Keyword),
    Tapped(bool),
    Token,
    /// Saddled (702.171b).
    Saddled,
    Commander,
    /// A face-down permanent (708.2), or what was one as it last existed.
    FaceDown,
    /// Attacking, blocking, blocked or unblocked (506.4).
    Combat(Status),
    /// Inverse unqualified attachment relation (301.5a, 303.4b).
    Attached(ir::selector::Attachment),
    Stat { stat: Stat, cmp: Cmp, value: Number },
    /// Ties stay eligible; the instruction determines who chooses and how many.
    Most {stat:Stat,least:bool,among:Box<Set>},
    /// An activated ability on the stack, or a triggered one.
    Ability { triggered: bool },
    /// Number of chosen target occurrences, including illegal/repeated ones (115.9a).
    TargetCount(u32),
    /// "of the chosen type", "of the chosen color": what was chosen, where it is kept.
    Chosen(Scalar),
    SameName(Subject),
    SharesCreatureType(Subject),
    SharesColor(Subject),
    Named(String),
    /// "that entered this turn": a permanent that came onto the battlefield in this turn as
    /// the object it is now (400.7).
    EnteredThisTurn,
    /// "that was put there from anywhere this turn": it is what an event of this turn
    /// matching the pattern happened to, as the object it is now (608.2i, 400.7).
    HappenedTo(Box<crate::trigger::Pattern>),
    LostLifeThisTurn,
    /// "dealt damage by ~ this turn": what one of these sources dealt damage to (120.1).
    DealtDamageBy(Subject),
    /// "that saddled it this turn" (`saddle`), "that crewed it this turn": tapped to pay a
    /// saddle or crew cost of one of `what` this turn (702.171c, 702.122b).
    TappedFor { saddle: bool, what: Subject },
    /// "that was dealt damage this turn" (a recipient), "that dealt combat damage to you this
    /// turn" (`dealt`: a source, to one of `to` if said): damage of that kind this turn (120.1).
    Damage { dealt: bool, kind: ir::event::DamageKind, to: Option<Subject> },
    /// "that attacked this turn" (a creature or a player, 508.1), with `attacked` false "that
    /// blocked this turn" (509.1).
    FoughtThisTurn { attacked: bool },
    /// "with a +1/+1 counter on it", "with two or fewer loyalty counters on him": counters
    /// of this kind, or of any, on it number from `min` to `max` (122.1).
    Counters { kind: Option<ir::CounterKind>, min: u32, max: u32 },
    /// "level 2 or greater": its level, 1 if it has none, from `min` to `max` (716.2d).
    Level { min: u32, max: u32 },
    /// Membership in a particular instruction's completed move, before or after its zone change.
    Entered(super::Slot),
    InSubjects(super::Slot),
}

/// A description: what must be true of a thing for it to be one the text names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Filter {
    All(Vec<Filter>),
    Any(Vec<Filter>),
    Not(Box<Filter>),
    Atom(Atom),
}

impl Filter {
    fn has_quality(&self) -> bool {
        !matches!(self, Self::All(parts) if parts.iter().all(|p| !p.has_quality()))
    }
    /// What deciding whether it describes something can read ([`Reads`]). A power or
    /// toughness is neither an ability nor a status, whatever number it is held against reads.
    pub(crate) fn reads(&self) -> Reads {
        match self {
            Filter::All(parts) | Filter::Any(parts) => Reads::each(parts, Filter::reads),
            Filter::Not(inner) => inner.reads(),
            Filter::Atom(Atom::Keyword(_)) => Reads::ABILITIES,
            Filter::Atom(Atom::Tapped(_)) => Reads::TAP_STATE,
            // set by a saddle ability's resolving, as an event of the turn (702.171a)
            Filter::Atom(Atom::Saddled) => Reads::HISTORY,
            Filter::Atom(Atom::EnteredThisTurn | Atom::LostLifeThisTurn | Atom::DealtDamageBy(_) | Atom::TappedFor { .. } | Atom::Damage { .. } | Atom::FoughtThisTurn { .. }) => Reads::HISTORY,
            // an event pattern describes its objects as they were: it may read anything
            Filter::Atom(Atom::HappenedTo(_)) => Reads::ALL,
            Filter::Atom(Atom::Stat { value, .. }) => value.reads(),
            Filter::Atom(Atom::Most{among,..}) => among.reads(),
            Filter::Atom(Atom::Targets(sets)) => Reads::each(sets, Set::reads),
            Filter::Atom(Atom::Type(_) | Atom::PermanentType | Atom::HasAdventure | Atom::PreparedSpell | Atom::BasicLandType | Atom::ManaCost(_) | Atom::Subtype(_) | Atom::Supertype(_)
                | Atom::Color(_) | Atom::Colorless | Atom::Multicolored | Atom::Monocolored
                | Atom::Opponent | Atom::Token | Atom::Commander | Atom::FaceDown | Atom::Combat(_) | Atom::Attached(_) | Atom::Ability { .. } | Atom::TargetCount(_)
                | Atom::Chosen(_) | Atom::SameName(_) | Atom::SharesCreatureType(_) | Atom::SharesColor(_) | Atom::Named(_)
                | Atom::Counters { .. } | Atom::Level { .. } | Atom::Entered(_) | Atom::InSubjects(_)) => Reads::NONE,
        }
    }

    pub(crate) fn compile(f: &ir::Filter, support: &Support<'_>, bind: Bind<'_>) -> Res<Filter> {
        Ok(match f {
            ir::Filter::All(v) => Filter::All(v.iter().map(|x| Filter::compile(x, support, bind)).collect::<Res<_>>()?),
            ir::Filter::Any(v) => Filter::Any(v.iter().map(|x| Filter::compile(x, support, bind)).collect::<Res<_>>()?),
            ir::Filter::Not(x) => Filter::Not(Box::new(Filter::compile(x, support, bind)?)),
            // Outlaw is an object description, not a creature type or a designation
            // (700.12). Keep the surrounding domain and use the ordinary effective
            // subtype predicates, including their layer/LKI reads and negation.
            ir::Filter::Atom(ir::Atom::Status(Status::Outlaw)) => Filter::Any(
                ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"].into_iter()
                    .map(|name| Filter::Atom(Atom::Subtype(ir::Subtype::from_name(name)
                        .expect("outlaw types belong to the pinned subtype vocabulary"))))
                    .collect()),
            ir::Filter::Atom(ir::Atom::Named(names)) => {
                let all = names.conj == ir::Conj::And;
                // "creatures named A and B" asks for one of each name (Mine Worker ruling of
                // 2022-10-14); no one object has two of them (201.2), so one filter cannot say it
                if all && names.items.len() > 1 {
                    return unsupported("named predicate: several names joined by \"and\" ask for an object of each name");
                }
                let names = names.items.iter().map(|name| Ok(Filter::Atom(Atom::Named(match name {
                    ir::selector::Name::Card(name) => name.clone(),
                    ir::selector::Name::This => support.printed_name()?.to_owned(),
                    ir::selector::Name::Granting => return unsupported("named predicate: granting definition"),
                })))).collect::<Res<Vec<_>>>()?;
                if all { Filter::All(names) }
                else { Filter::Any(names) }
            }
            ir::Filter::Atom(a) => Filter::Atom(match a {
                ir::Atom::Shares { what: ir::amount::Characteristic::CreatureType, with } => Atom::SharesCreatureType(Subject::compile(with, bind)?),
                ir::Atom::Shares { what: ir::amount::Characteristic::Color, with } => Atom::SharesColor(Subject::compile(with, bind)?),
                ir::Atom::Shares { what: ir::amount::Characteristic::Name, with } => Atom::SameName(Subject::compile(with, bind)?),
                ir::Atom::DealtDamageBy { by, this_turn: true } => Atom::DealtDamageBy(Subject::compile(by, bind)?),
                ir::Atom::Type(t) => Atom::Type(*t),
                ir::Atom::PermanentType => Atom::PermanentType,
                ir::Atom::HasAdventure => Atom::HasAdventure,
                ir::Atom::PreparedSpell => Atom::PreparedSpell,
                ir::Atom::ManaCost(costs) => Atom::ManaCost(costs.clone()),
                ir::Atom::Subtype(s) => Atom::Subtype(s.clone()),
                ir::Atom::BasicLandType => Atom::BasicLandType,
                ir::Atom::Supertype(s) => Atom::Supertype(s.clone()),
                ir::Atom::Color(c) => Atom::Color(*c),
                ir::Atom::Colorless => Atom::Colorless,
                ir::Atom::Multicolored => Atom::Multicolored,
                ir::Atom::Monocolored => Atom::Monocolored,
                ir::Atom::Opponent => Atom::Opponent,
                ir::Atom::DidSo(outcome) => Atom::InSubjects(bind.player_outcome(*outcome)?),
                ir::Atom::Keyword(k) => Atom::Keyword(*k),
                ir::Atom::Status(Status::Equipped) => Atom::Attached(ir::selector::Attachment::Equipped),
                ir::Atom::Status(Status::Enchanted) => Atom::Attached(ir::selector::Attachment::Enchanted),
                ir::Atom::Status(Status::Tapped) => Atom::Tapped(true),
                ir::Atom::Status(Status::Untapped) => Atom::Tapped(false),
                ir::Atom::Status(Status::Token) => Atom::Token,
                ir::Atom::Status(Status::Saddled) => Atom::Saddled,
                ir::Atom::Status(Status::Commander) => Atom::Commander,
                ir::Atom::Status(Status::FaceDown) => Atom::FaceDown,
                ir::Atom::Status(s @ (Status::Attacking | Status::Blocking | Status::Blocked | Status::Unblocked)) => Atom::Combat(*s),
                ir::Atom::Stat { stat, cmp, value } => {
                    Atom::Stat { stat: Stat::compile(*stat, "description")?, cmp: *cmp, value: Number::compile(value, support, bind)? }
                }
                ir::Atom::Most{stat,least,among:Some(among),..} => Atom::Most{stat:Stat::compile(*stat,"extrema description")?,least:*least,among:Box::new(Set::compile(among,support,bind)?)},
                ir::Atom::Most{among:None,..} => return unsupported("extrema description: an explicit comparison set is required"),
                ir::Atom::TargetCount(Count::Exactly(ir::Amount::Const(n)) | Count::Precisely(ir::Amount::Const(n))) => Atom::TargetCount(*n),
                ir::Atom::Targets { what, only } => {
                    if *only || what.items.is_empty() || (what.items.len() > 1 && what.conj != ir::Conj::Or)
                        || what.items.iter().any(|s| s.quant != Quant::Indefinite(Count::Exactly(ir::Amount::Const(1)))
                            || s.chooser.is_some() || s.random.is_some()) {
                        return unsupported("target description: singular existential alternatives required (115.9b)");
                    }
                    Atom::Targets(what.items.iter().map(|s| Set::compile(&s.set, support, bind)).collect::<Res<_>>()?)
                }
                ir::Atom::AbilityKind(ir::selector::AbilityKind::Activated) => Atom::Ability { triggered: false },
                ir::Atom::AbilityKind(ir::selector::AbilityKind::Triggered) => Atom::Ability { triggered: true },
                ir::Atom::Reference(r) if r.form == RefForm::Choice && r.qualifiers.is_empty() => {
                    let scalar = bind.scalar(r)?;
                    if !fits(r.expected, scalar.clone().kind()) || !matches!(scalar.clone().kind(), Kind::CardType | Kind::CreatureType | Kind::Color | Kind::CardName) {
                        return unsupported("choice reference: unsupported scalar constraint");
                    }
                    Atom::Chosen(scalar)
                }
                // "Choose a card name. Search … for … cards with that name" (201.4): the name
                // chosen before it
                ir::Atom::ThatType => {
                    let scalar = bind.scalar(&UnresolvedRef::choice(Kind::Type))?;
                    if !matches!(scalar.clone().kind(), Kind::CardType | Kind::CreatureType) {
                        return unsupported("description: that type, of no chosen type");
                    }
                    Atom::Chosen(scalar)
                }
                ir::Atom::ThatName => {
                    let r = UnresolvedRef { expected: Kind::CardName, form: RefForm::Choice, qualifiers: Vec::new() };
                    let scalar = bind.scalar(&r)?;
                    if scalar.clone().kind() != Kind::CardName { return unsupported("description: that name, of no chosen card name"); }
                    Atom::Chosen(scalar)
                }
                // Excluding a completed move preserves the original group identities.
                // Positive selection from a moved group needs successor-group binding.
                ir::Atom::Did { action, when: ir::amount::When::ThisWay | ir::amount::When::Context, negated: true }
                    if matches!(&**action, ir::event::EventAction::Deed(ir::Deed { verb: ir::Verb::Move, passive: true, args })
                        if matches!(args.as_slice(), [(ir::Role::To, ir::Arg::Zone(zone))] if *zone == ir::event::ZoneArg { zone: ir::ZoneSpec::of(ir::Zone::Battlefield, None), other_than: false })) =>
                {
                    return Ok(Filter::Not(Box::new(Filter::Atom(Atom::Entered(bind.entered()?)))));
                }
                ir::Atom::Did { action, when: ir::amount::When::ThisTurn, negated }
                    if matches!(&**action, ir::event::EventAction::Deed(deed) if deed.verb == ir::Verb::Enter && !deed.passive && deed.args.is_empty()) =>
                {
                    let entered = Filter::Atom(Atom::EnteredThisTurn);
                    return Ok(if *negated { Filter::Not(Box::new(entered)) } else { entered });
                }
                ir::Atom::Did { action, when: ir::amount::When::ThisTurn, negated }
                    if matches!(&**action, ir::EventAction::IsDealtDamage { by, excess: false, .. } if by.is_empty())
                        || matches!(&**action, ir::EventAction::DealsDamage { amount: None, to, .. } if to.as_ref().is_none_or(|to| to.items.len() == 1)) =>
                {
                    let damage = match &**action {
                        ir::EventAction::IsDealtDamage { kind, .. } => Atom::Damage { dealt: false, kind: *kind, to: None },
                        ir::EventAction::DealsDamage { kind, to, .. } => Atom::Damage { dealt: true, kind: *kind,
                            to: to.as_ref().map(|to| Subject::compile(&to.items[0], bind)).transpose()? },
                        _ => unreachable!(),
                    };
                    return Ok(if *negated { Filter::Not(Box::new(Filter::Atom(damage))) } else { Filter::Atom(damage) });
                }
                ir::Atom::Did { action, when: ir::amount::When::ThisTurn, negated }
                    if matches!(&**action, ir::EventAction::Attacks { alone: false, defender, with: None } if defender.is_empty())
                        || matches!(&**action, ir::EventAction::Blocks { blocked: None }) =>
                {
                    let fought = Filter::Atom(Atom::FoughtThisTurn { attacked: matches!(&**action, ir::EventAction::Attacks { .. }) });
                    return Ok(if *negated { Filter::Not(Box::new(fought)) } else { fought });
                }
                ir::Atom::Did { action, when: ir::amount::When::ThisTurn, negated }
                    if matches!(&**action, ir::EventAction::Deed(d) if matches!(d.verb, ir::Verb::Saddle | ir::Verb::Crew) && !d.passive
                        && matches!(d.args.as_slice(), [(ir::Role::What, ir::Arg::Noun(_))])) =>
                {
                    let ir::EventAction::Deed(d) = &**action else { unreachable!() };
                    let [(_, ir::Arg::Noun(what))] = d.args.as_slice() else { unreachable!() };
                    let tapped = Filter::Atom(Atom::TappedFor { saddle: d.verb == ir::Verb::Saddle, what: Subject::compile(what, bind)? });
                    return Ok(if *negated { Filter::Not(Box::new(tapped)) } else { tapped });
                }
                ir::Atom::Did {action,when:ir::amount::When::ThisTurn,negated}
                    if matches!(&**action,ir::EventAction::LosesLife{amount:None}) => {
                    let lost=Filter::Atom(Atom::LostLifeThisTurn);
                    return Ok(if *negated {Filter::Not(Box::new(lost))}else{lost});
                }
                // any other event of the turn, as what it happened to is now: "that was put
                // there from anywhere this turn"
                ir::Atom::Did { action, when: ir::amount::When::ThisTurn, negated } => {
                    let pattern = ir::EventPattern::new(Vec::new(), vec![history_action(action)]);
                    let happened = Filter::Atom(Atom::HappenedTo(Box::new(crate::trigger::Pattern::compile(&pattern, support)?)));
                    return Ok(if *negated { Filter::Not(Box::new(happened)) } else { happened });
                }
                ir::Atom::Counters { kind, count } => {
                    use ir::{Amount::Const, selector::Count};
                    // a bare number of counters is at least that many: "a +1/+1 counter"
                    let (min, max) = match count {
                        Count::Exactly(Const(n)) | Count::AtLeast(Const(n)) => (*n, u32::MAX),
                        Count::UpTo(Const(n)) => (0, *n),
                        Count::Precisely(Const(n)) => (*n, *n),
                        Count::Range { min, max } => match (&**min, &**max) {
                            (Const(min), Const(max)) => (*min, *max),
                            _ => return unsupported("description: a number of counters that is not a constant"),
                        },
                        _ => return unsupported("description: a number of counters that is not a constant"),
                    };
                    Atom::Counters { kind: kind.clone(), min, max }
                }
                ir::Atom::Level(count) => {
                    use ir::{Amount::Const, selector::Count};
                    let (min, max) = match count {
                        Count::AtLeast(Const(n)) => (*n, u32::MAX),
                        Count::Precisely(Const(n)) => (*n, *n),
                        _ => return unsupported("description: a level that is not a constant"),
                    };
                    Atom::Level { min, max }
                }
                ir::Atom::Chosen(c) => return unsupported(format!("description: the chosen {}", name(c).to_lowercase())),
                other => return unsupported(format!("description: {}", name(other))),
            }),
        })
    }

    fn other_than(&self, other: bool) -> Res<Option<Subject>> {
        let mut references = Vec::new();
        if other { self.name_references(&mut references); }
        if references.len() > 1 { return unsupported("other: ambiguous name-comparison subjects"); }
        Ok(references.pop())
    }

    /// In "other ... with the same name as that creature", other excludes that
    /// explicit comparison subject, not the source of the effect (201.2a).
    fn name_references(&self, out: &mut Vec<Subject>) {
        match self {
            Filter::All(items) | Filter::Any(items) => items.iter().for_each(|f| f.name_references(out)),
            Filter::Atom(Atom::SameName(subject) | Atom::SharesCreatureType(subject) | Atom::SharesColor(subject)) => { if !out.contains(subject) { out.push(subject.clone()); } },
            _ => {},
        }
    }

    pub(crate) fn matches(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> bool {
        match self {
            Filter::All(v) => v.iter().all(|x| x.matches(game, ctx, locals, t)),
            Filter::Any(v) => v.iter().any(|x| x.matches(game, ctx, locals, t)),
            Filter::Not(x) => !x.matches(game, ctx, locals, t),
            Filter::Atom(a) => a.matches(game, ctx, locals, t),
        }
    }

    /// What of this description `t` is not, in words, when that can be said of one part.
    fn unlike(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> Option<String> {
        if self.matches(game, ctx, locals, t) {
            return None;
        }
        match self {
            Filter::All(v) => v.iter().find_map(|x| x.unlike(game, ctx, locals, t)),
            Filter::Any(v) if v.len() == 1 => v[0].unlike(game, ctx, locals, t),
            Filter::Any(v) => {
                let said: Option<Vec<String>> = v.iter().map(|x| match x { Filter::Atom(a) => a.said(), _ => None }).collect();
                said.map(|said| format!("it is neither {}", said.join(" nor ")))
            }
            Filter::Not(x) => match &**x {
                Filter::Atom(Atom::Keyword(k)) => Some(format!("it has {}", k.name().to_lowercase())),
                Filter::Atom(a) => a.said().map(|said| format!("it is {said}")),
                _ => None,
            },
            Filter::Atom(Atom::Keyword(k)) => Some(format!("it doesn't have {}", k.name().to_lowercase())),
            Filter::Atom(Atom::Stat { stat, cmp, value }) => {
                let Target::Object(o) = t else { return None };
                let what = match stat { Stat::Power => "power", Stat::Toughness => "toughness", Stat::PowerAndToughness => "total power and toughness", Stat::ManaValue => "mana value" };
                let want = value.evaluate(game, ctx, locals);
                let wanted = match cmp {
                    Cmp::Eq => format!("{want}"),
                    Cmp::Le => format!("{want} or less"),
                    Cmp::Ge => format!("{want} or greater"),
                    Cmp::Lt => format!("less than {want}"),
                    Cmp::Gt => format!("greater than {want}"),
                };
                Some(match stat.of(game, ctx, o) {
                    Some(have) => format!("its {what} is {have}, and the target says {what} {wanted}"),
                    None => format!("it has no {what}, and the target says {what} {wanted}"),
                })
            }
            Filter::Atom(a) => a.said().map(|said| format!("it isn't {said}")),
        }
    }

    /// Is this card as printed so: what "if it's a creature card" asks of the card a revealed
    /// face-down permanent represents, ignoring continuous effects (708.12), not of the
    /// face-down 2/2. Only for a description that is [`Filter::printable`].
    pub(crate) fn printed(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, card: &crate::CardDef) -> bool {
        match self {
            Filter::All(v) => v.iter().all(|x| x.printed(game, ctx, locals, card)),
            Filter::Any(v) => v.iter().any(|x| x.printed(game, ctx, locals, card)),
            Filter::Not(x) => !x.printed(game, ctx, locals, card),
            Filter::Atom(a) => match a {
                Atom::Named(name) => card.name().split(" // ").any(|part| part == name),
                Atom::Type(ty) => card.is(*ty),
                Atom::PermanentType => card.types().iter().any(|t| t.permanent()),
                Atom::Subtype(s) => card.subtypes().contains(s),
                Atom::BasicLandType => card.subtypes().iter().any(|s| s.is_basic_land_type()),
                Atom::Supertype(s) => card.supertypes().contains(s),
                Atom::Color(c) => card.colors().contains(c),
                Atom::Colorless => card.colors().is_empty(),
                Atom::Multicolored => card.colors().len() >= 2,
                Atom::Monocolored => card.colors().len() == 1,
                Atom::ManaCost(costs) => card.cost().is_some_and(|c| costs.contains(c)),
                Atom::Stat { stat, cmp, value } => {
                    let have = match stat {
                        Stat::Power => card.pt().map(|(p, _)| i64::from(p)),
                        Stat::Toughness => card.pt().map(|(_, t)| i64::from(t)),
                        Stat::PowerAndToughness => card.pt().map(|(p, t)| i64::from(p) + i64::from(t)),
                        Stat::ManaValue => Some(i64::from(card.mana_value())),
                    };
                    let want = value.evaluate(game, ctx, locals);
                    have.is_some_and(|have| match cmp {
                        Cmp::Eq => have == want,
                        Cmp::Le => have <= want,
                        Cmp::Ge => have >= want,
                        Cmp::Lt => have < want,
                        Cmp::Gt => have > want,
                    })
                }
                Atom::Chosen(scalar) => match chosen(game, ctx, locals, scalar.clone()) {
                    Some(Value::CreatureType(t)) => card.subtypes().contains(t),
                    Some(Value::CardType(t)) => card.types().iter().any(|kind| kind.word() == t),
                    Some(Value::Color(c)) => card.colors().contains(c),
                    Some(Value::CardName(name)) => card.name().split(" // ").any(|part| part == name),
                    // an undefined choice describes nothing (607.5a)
                    _ => false,
                },
                _ => unreachable!("a printable description"),
            },
        }
    }

    /// Does the description say only what a card says as printed ([`Filter::printed`])?
    pub(crate) fn printable(&self) -> bool {
        match self {
            Filter::All(v) | Filter::Any(v) => v.iter().all(Filter::printable),
            Filter::Not(x) => x.printable(),
            Filter::Atom(a) => matches!(a, Atom::Named(_) | Atom::Type(_) | Atom::PermanentType | Atom::Subtype(_) | Atom::BasicLandType
                | Atom::Supertype(_) | Atom::Color(_) | Atom::Colorless | Atom::Multicolored | Atom::Monocolored | Atom::ManaCost(_) | Atom::Stat { .. } | Atom::Chosen(_)),
        }
    }

    /// The description with each chosen value it names as that value is now, kept in
    /// `values` and read back from there: what a resolved effect locks in as it begins
    /// (608.2h) or a static ability says as it is asked (611.3a), asked later of other
    /// objects with no frame. Only a chosen value at its root is captured
    /// ([`Filter::captures_all`]).
    pub(crate) fn capture(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, values: &mut Vec<Option<Value>>) -> Filter {
        match self {
            Filter::All(v) => Filter::All(v.iter().map(|x| x.capture(game, ctx, locals, values)).collect()),
            Filter::Any(v) => Filter::Any(v.iter().map(|x| x.capture(game, ctx, locals, values)).collect()),
            Filter::Not(x) => Filter::Not(Box::new(x.capture(game, ctx, locals, values))),
            Filter::Atom(Atom::Chosen(scalar)) => {
                values.push(chosen(game, ctx, locals, scalar.clone()).cloned());
                Filter::Atom(Atom::Chosen(Scalar::Local { slot: values.len() - 1, kind: scalar.clone().kind() }))
            }
            other => other.clone(),
        }
    }

    /// Does [`Filter::capture`] capture every value it reads?
    pub(crate) fn captures_all(&self) -> bool {
        match self {
            Filter::All(v) | Filter::Any(v) => v.iter().all(Filter::captures_all),
            Filter::Not(x) => x.captures_all(),
            Filter::Atom(Atom::Chosen(_)) => true,
            atom => { let mut out = Vec::new(); atom.scalars(&mut out); out.is_empty() }
        }
    }

    pub(crate) fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Filter::All(v) | Filter::Any(v) => v.iter().for_each(|x| x.scalars(out)),
            Filter::Not(x) => x.scalars(out),
            Filter::Atom(Atom::Chosen(scalar)) => out.push(scalar.clone()),
            Filter::Atom(Atom::InSubjects(slot)) => out.push(Scalar::Local { slot: *slot, kind: Kind::Subjects }),
            Filter::Atom(Atom::Entered(slot)) => out.push(Scalar::Local { slot: *slot, kind: Kind::Cards }),
            Filter::Atom(Atom::SameName(subject) | Atom::SharesCreatureType(subject) | Atom::SharesColor(subject) | Atom::DealtDamageBy(subject) | Atom::TappedFor { what: subject, .. } | Atom::Damage { to: Some(subject), .. }) => subject.scalars(out),
            Filter::Atom(Atom::Stat { value, .. }) => value.scalars(out),
            Filter::Atom(Atom::Most{among,..}) => among.collect(out),
            Filter::Atom(Atom::Targets(sets)) => sets.iter().for_each(|set| set.collect(out)),
            Filter::Atom(_) => {}
        }
    }
}

/// What was chosen: in this execution's slot, or in the source's linked cell (607.2d).
pub(crate) fn chosen<'g>(game: &'g Game, ctx: &Ctx, locals: &'g dyn Locals, scalar: Scalar) -> Option<&'g Value> {
    match scalar {
        Scalar::Local { slot, .. } => locals.local(slot),
        Scalar::Linked { link, .. } => ctx.source.into_iter().flat_map(|source| game.obj(source).choices.iter()).find(|r| r.link == crate::binding::LinkId(ctx.link(link.0.clone()))).map(|r| &r.value),
    }
}

impl Atom {
    /// What it says of something, after "it is": "a creature", "tapped", "red".
    fn said(&self) -> Option<String> {
        Some(match self {
            Atom::Type(ty) => article(ty.word()),
            Atom::PermanentType => "a permanent card".into(),
            Atom::Subtype(s) => article(s.name()),
            Atom::BasicLandType => "a basic land type".into(),
            Atom::Supertype(s) => s.word().into(),
            Atom::Color(c) => c.word().into(),
            Atom::Colorless => "colorless".into(),
            Atom::Multicolored => "multicolored".into(),
            Atom::Monocolored => "monocolored".into(),
            Atom::Opponent => "an opponent".into(),
            Atom::Tapped(true) => "tapped".into(),
            Atom::Tapped(false) => "untapped".into(),
            Atom::Token => "a token".into(),
            Atom::Saddled => "saddled".into(),
            Atom::PreparedSpell => "a prepare spell".into(),
            Atom::Commander => "a commander".into(),
            Atom::FaceDown => "face down".into(),
            Atom::Attached(kind) => format!("{kind:?}").to_lowercase(),
            Atom::Combat(status) => format!("{status:?}").to_lowercase(),
            Atom::Named(name) => format!("named {name}"),
            Atom::EnteredThisTurn => "a permanent that entered this turn".into(),
            Atom::LostLifeThisTurn => "a player who lost life this turn".into(),
            Atom::Ability { triggered: true } => "a triggered ability".into(),
            Atom::Ability { triggered: false } => "an activated ability".into(),
            _ => return None,
        })
    }

    fn matches(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> bool {
        if let Atom::InSubjects(slot) = self {
            return matches!(locals.local(*slot), Some(Value::Subjects(subjects)) if subjects.contains(&t));
        }
        if let Atom::DealtDamageBy(by) = self {
            return by.objects(game, ctx, locals).any(|source| game.dealt_damage_this_turn(source, t));
        }
        if let Atom::TappedFor { saddle, what } = self {
            let Target::Object(creature) = t else { return false };
            return what.objects(game, ctx, locals).any(|permanent| game.tapped_for_this_turn(*saddle, creature, permanent));
        }
        if let Atom::HappenedTo(pattern) = self {
            return game.happened_to(ctx, pattern, t);
        }
        if let Atom::FoughtThisTurn { attacked } = self {
            return match (attacked, t) {
                (true, _) => game.attacked_this_turn(t),
                (false, Target::Object(o)) => game.blocked_this_turn(o),
                (false, Target::Player(_)) => false,
            };
        }
        if let Atom::Damage { dealt, kind, to } = self {
            return match (dealt, t) {
                (false, _) => game.damage_this_turn(None, Some(t), *kind),
                (true, Target::Object(source)) => match to {
                    None => game.damage_this_turn(Some(source), None, *kind),
                    Some(to) => to.evaluate(game, ctx, locals).into_iter().any(|to| game.damage_this_turn(Some(source), Some(to), *kind)),
                },
                (true, Target::Player(_)) => false,
            };
        }
        if let Atom::Attached(kind) = self { return game.has_attachment(ctx, t, *kind); }
        let o = match t {
            Target::Object(o) => game.obj(o),
            // what describes objects is false of a player
            Target::Player(p) => return match self {Atom::Opponent=>p!=ctx.controller,Atom::LostLifeThisTurn=>game.life_lost_this_turn(p)>0,_=>false},
        };
        // what it is as the layers before the one asking have left it (613.1): its types and
        // colors after layers 4 and 5
        // each asked as far as it reads: a type is not a question about power (613.1)
        use crate::layers::Aspect;
        let (types, colors, abilities) =
            (|| ctx.seen_for(game, o.id, Aspect::Types), || ctx.seen_for(game, o.id, Aspect::Colors), || ctx.seen_for(game, o.id, Aspect::Abilities));
        match self {
            Atom::Named(name) => ctx.name_of(game, o.id).split(" // ").any(|part| part == name),
            Atom::SharesCreatureType(with) => {
                let own=types();
                with.objects(game,ctx,locals).any(|other|ctx.seen_for(game,other,Aspect::Types).subtypes.iter().any(|t|t.kind()==ir::vocab::SubtypeKind::Creature && own.subtypes.contains(t)))
            },
            Atom::SharesColor(with) => {
                let own = colors();
                let others: Vec<_> = with.objects(game, ctx, locals).collect();
                !others.is_empty() && others.iter().all(|other| ctx.seen_for(game, *other, Aspect::Colors).colors.iter().any(|color| own.colors.contains(color)))
            },
            Atom::SameName(with) => {
                let others: Vec<_> = with.objects(game, ctx, locals).collect();
                // a spell being cast has the name of the half or face cast (709.3a)
                !others.is_empty() && others.iter().all(|other| game.shares_name(ctx.name_of(game, o.id), *other))
            },
            Atom::Type(ty) => types().is(*ty),
            Atom::PermanentType => types().types.iter().any(|t| t.permanent()),
            Atom::HasAdventure => game.def(o.id).is_some_and(crate::CardDef::has_adventure),
            Atom::PreparedSpell => game.def(o.id).is_some_and(|d| d.is_prepare_spell),
            // as printed, or as a copy effect says (707.2); no mana cost is none of these
            Atom::ManaCost(costs) => game.def(o.id).and_then(|d| d.cost()).is_some_and(|c| costs.contains(c)),
            Atom::Subtype(s) => types().subtypes.contains(s),
            Atom::BasicLandType => types().subtypes.iter().any(|s| s.is_basic_land_type()),
            Atom::Supertype(s) => types().supertypes.contains(s),
            Atom::Color(c) => colors().colors.contains(c),
            Atom::Colorless => colors().colors.is_empty(),
            Atom::Multicolored => colors().colors.len() >= 2,
            Atom::Monocolored => colors().colors.len() == 1,
            Atom::Opponent | Atom::LostLifeThisTurn => false,
            // "with flying": it has the ability
            Atom::Keyword(k) => abilities().has(*k),
            // Only permanents have tapped/untapped status (110.5d). A departed identity
            // can supply its last battlefield status when a trigger/effect asks about it
            // (603.10a, 608.2h); the new card in its destination has no such status.
            Atom::Tapped(tapped) => {
                let battlefield = o.zone == Zone::Battlefield || (o.zone == Zone::Gone && o.left == Some(Zone::Battlefield));
                battlefield && o.tapped == *tapped
            }
            Atom::TargetCount(n) => o.targets.iter().map(Vec::len).sum::<usize>() == *n as usize,
            Atom::Targets(sets) => {
                // 115.9b explicitly excludes a departed target's LKI. This is
                // membership in its current description, not target legality.
                let current = Ctx { was: None, ..ctx.clone() };
                o.targets.iter().flatten().any(|target| sets.iter().any(|set| set.contains(game, &current, locals, *target)))
            }
            Atom::Token => o.token(),
            // only a permanent is saddled (702.171b); one that left was, as it last existed
            Atom::Saddled => o.saddled && (o.zone == Zone::Battlefield || (o.zone == Zone::Gone && o.left == Some(Zone::Battlefield))),
            // a face-down spell or permanent, or one as it last existed there (708.2); what an
            // event turned face up or one that entered this turn, as the event happened (708.8,
            // 608.2i)
            Atom::FaceDown => match ctx.face_then {
                Some((then, down)) if then == o.id => down,
                _ => {
                    let zone = if o.zone == Zone::Gone { o.left } else { Some(o.zone) };
                    matches!(zone, Some(Zone::Battlefield | Zone::Stack)) && matches!(o.face, crate::face::Face::Down(_))
                }
            },
            Atom::Commander => game.is_commander(o.id),
            Atom::InSubjects(_) | Atom::DealtDamageBy(_) | Atom::TappedFor { .. } | Atom::HappenedTo(_) | Atom::Damage { .. } | Atom::FoughtThisTurn { .. } => unreachable!("tested before object lookup"),
            Atom::Entered(slot) => matches!(locals.local(*slot), Some(Value::Cards(cards)) if cards.contains(&o.id)),
            Atom::EnteredThisTurn => o.zone == Zone::Battlefield && o.entered != 0 && o.entered == game.turn,
            Atom::Counters { kind, min, max } => (u64::from(*min)..=u64::from(*max)).contains(&crate::select::counters_on(game, o.id, kind.as_ref())),
            Atom::Level { min, max } => (*min..=*max).contains(&o.level.unwrap_or(1)),
            Atom::Attached(_) => unreachable!("tested before object lookup"),
            Atom::Combat(s) => crate::combat::status(game, o.id, *s).unwrap_or(false),
            Atom::Stat { stat, cmp, value } => {
                let want = value.evaluate(game, ctx, locals);
                stat.of(game, ctx, o.id).is_some_and(|have| match cmp {
                    Cmp::Eq => have == want,
                    Cmp::Le => have <= want,
                    Cmp::Ge => have >= want,
                    Cmp::Lt => have < want,
                    Cmp::Gt => have > want,
                })
            }
            Atom::Most{stat,least,among} => stat.of(game,ctx,o.id).is_some_and(|value| {
                let values=among.members(game,ctx,locals).into_iter().filter_map(|t| match t {
                    Target::Object(id)=>stat.of(game,ctx,id), Target::Player(_)=>None,
                });
                let extreme=if *least { values.min() } else { values.max() };
                extreme==Some(value)
            }),
            Atom::Ability { triggered } => o.ability.is_some() && o.invocations.as_ref()
                .and_then(|result| result.as_ref().ok()).and_then(|invocations| invocations.first())
                .is_some_and(|invocation| invocation.triggered == *triggered),
            Atom::Chosen(scalar) => {
                match chosen(game, ctx, locals, scalar.clone()) {
                    Some(Value::CreatureType(t)) => types().subtypes.contains(t),
                    Some(Value::CardType(t)) => types().types.iter().any(|kind| kind.word() == t),
                    Some(Value::Color(c)) => colors().colors.contains(c),
                    Some(Value::CardName(name)) => ctx.name_of(game, o.id).split(" // ").any(|part| part == name),
                    // an undefined choice describes nothing (607.5a)
                    _ => false,
                }
            }
        }
    }
}

/// Whose control: "you control" (true), or "an opponent controls" (false); and "don't".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Controller {
    you: bool,
    negated: bool,
}

impl Controller {
    pub(crate) fn compile(rel: &ir::Relation) -> Res<Controller> {
        if rel.past {
            return unsupported("noun phrase: a past controller");
        }
        match crate::select::controlled_by_you(&rel.who) {
            Some(you) => Ok(Controller { you, negated: rel.negated }),
            None => unsupported("noun phrase: a controller other than you or an opponent"),
        }
    }

    /// Targeting restrictions ask this of the controller of the spell or ability being
    /// announced, which need not be on the stack yet; membership asks it of the object's
    /// current controller (110.2, 115.1).
    pub(crate) fn matches(self, ctx: &Ctx, controller: PlayerId) -> bool {
        ((controller == ctx.controller) == self.you) != self.negated
    }
}

/// A controller/owner relation may name a player already bound by this occurrence,
/// as well as "you" or "an opponent". Both query the same authoritative player ID.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Relation { Relative(Controller), Named { who: Subject, negated: bool } }
impl Relation {
    fn compile(rel: &ir::Relation, bind: Bind<'_>) -> Res<Self> {
        if rel.past { return unsupported("noun phrase: a past owner or controller"); }
        if crate::select::controlled_by_you(&rel.who).is_some() {
            Ok(Self::Relative(Controller::compile(rel)?))
        } else { Ok(Self::Named { who: Subject::compile(&rel.who,bind)?, negated: rel.negated }) }
    }
    fn matches(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, player: PlayerId) -> bool {
        match self {
            Self::Relative(relative) => relative.matches(ctx,player),
            Self::Named { who, negated } => who.players(game,ctx,locals).any(|p|p==player) != *negated,
        }
    }
    fn scalars(&self, out: &mut Vec<Scalar>) {
        if let Self::Named { who, .. } = self { who.scalars(out); }
    }
    /// "you control", "an opponent owns", "that player doesn't control".
    fn said(&self, verb: &str) -> String {
        match self {
            Self::Relative(Controller { you: true, negated }) => format!("you {}{verb}", if *negated { "don't " } else { "" }),
            Self::Relative(Controller { you: false, negated }) => if *negated { format!("an opponent doesn't {verb}") } else { format!("an opponent {verb}s") },
            Self::Named { negated, .. } => if *negated { format!("the player it names doesn't {verb}") } else { format!("the player it names {verb}s") },
        }
    }
}

/// "you control it", "another player controls it".
fn whose(yours: bool, verb: &str) -> String {
    if yours { format!("you {verb} it") } else { format!("another player {verb}s it") }
}

fn zone_said(zone: Zone) -> &'static str {
    match zone {
        Zone::Library => "library", Zone::Hand => "hand", Zone::Battlefield => "battlefield", Zone::Graveyard => "graveyard",
        Zone::Stack => "stack", Zone::Exile => "exile", Zone::Command => "command zone", Zone::Gone => "nowhere",
    }
}

/// "a" or "an" before a word.
fn article(word: &str) -> String {
    let an = word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']);
    format!("{} {word}", if an { "an" } else { "a" })
}

/// "a creature or a player", "a spell".
fn sorts_said(sorts: &[Sort]) -> String {
    sorts.iter().map(|sort| match sort {
        Sort::Player => article("player"),
        Sort::Permanent => article("permanent"),
        Sort::Of(kind) => article(kind.word()),
        Sort::Card => article("card"),
        Sort::Spell => article("spell"),
        Sort::Ability => article("ability"),
    }).collect::<Vec<_>>().join(" or ")
}

/// Where the things described are looked for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Domain {
    LinkedExile(crate::binding::AbilityKey),
    CostExile(crate::binding::AbilityKey),
    /// Cards in the named owned zones (400.1, 400.3), optionally alongside permanents:
    /// "creatures from the battlefield and/or creature cards from graveyards" (115.2).
    Zones { zones: Vec<Zone>, yours: bool, also_permanents: bool },
    /// "creature or player you control": each arm is a description of its own.
    Union(Vec<Set>),
    /// Players, permanents, spells or abilities. `players`: players and nothing else.
    Sorts { sorts: Vec<Sort>, players: bool },
}

/// Owned card zones and whether permanents are included in the same description.
fn card_zones(set: &ir::ObjectSet) -> Res<Option<(Vec<Zone>, bool, bool)>> {
    let Some(zone) = &set.zone else { return Ok(None) };
    // Permanents already denote the battlefield; spelling that zone adds no new domain.
    if zone.zones.items == [ir::Zone::Battlefield] && zone.owner.is_none() && zone.slice.is_none()
        && set.domain == ir::Domain::Permanent { return Ok(None); }

    let players = |who: &ir::Selector| who.set == ir::ObjectSet::of(ir::Domain::Player) && who.chooser.is_none() && who.random.is_none();
    let yours = match zone.owner.as_deref() {
        Some(who) if is_you(who) => true,
        Some(who) if who.set.domain == ir::Domain::Player && who.chooser.is_none() && who.random.is_none()
            && (crate::select::controlled_by_you(who) == Some(false) || matches!(who.quant, Quant::Definite(_))) => false,
        // "a graveyard", or "graveyards", which are all of them
        Some(who) if players(who) && matches!(&who.quant, Quant::Indefinite(Count::Exactly(ir::Amount::Const(1))) | Quant::Each) => false,
        _ => return unsupported("noun phrase: an unsupported zone or zone owner"),
    };
    let also_permanents = zone.zones.items == [ir::Zone::Battlefield, ir::Zone::Graveyard]
        && zone.zones.conj == ir::Conj::AndOr && set.domain == ir::Domain::Object && !yours;
    let zones = if also_permanents { vec![Zone::Graveyard] } else {
        if set.domain != ir::Domain::Card { return unsupported("noun phrase: in an owned zone, other than a card"); }
        zone.zones.items.iter().map(|z| match z {
            ir::Zone::Graveyard => Ok(Zone::Graveyard),
            ir::Zone::Library => Ok(Zone::Library),
            ir::Zone::Hand => Ok(Zone::Hand),
            _ => unsupported("noun phrase: an unsupported card zone"),
        }).collect::<Res<Vec<_>>>()?
    };
    if zones.is_empty() || zone.slice.is_some() || set.controller.is_some() {
        return unsupported("noun phrase: empty zones, a card controller or a zone slice");
    }
    Ok(Some((zones, yours, also_permanents)))
}

/// A description of players, permanents, stack objects or cards in explicitly named zones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    domain: Domain,
    /// “Other” excludes an explicitly bound comparison/acting subject when present,
    /// otherwise the ordinary source/controller of the effect.
    other: bool,
    other_than: Option<Subject>,
    controller: Option<Relation>,
    owner: Option<Relation>,
    filter: Filter,
}

/// Positive linked membership implies the exile zone. Negated/alternative relations
/// cannot establish that domain and remain explicit refusals.
pub(crate) fn linked_exile_filter(filter: &ir::Filter) -> Option<ir::Filter> {
    match filter {
        ir::Filter::Atom(ir::Atom::ExiledWith(ir::Ref::This)) => Some(ir::Filter::All(Vec::new())),
        ir::Filter::All(parts) if parts.iter().any(|p| linked_exile_filter(p).is_some()) =>
            Some(ir::Filter::All(parts.iter().map(|p| linked_exile_filter(p).unwrap_or_else(|| p.clone())).collect())),
        _ => None,
    }
}

fn linked_exile_filter_bound(filter: &ir::Filter, bind: Bind<'_>) -> Res<Option<ir::Filter>> {
    match filter {
        ir::Filter::Atom(ir::Atom::ExiledWith(reference)) => {
            let selector = ir::Selector::new(Quant::Definite(reference.clone()), ir::ObjectSet::of(ir::Domain::Permanent));
            if Subject::compile(&selector, bind)? != Subject::Source {
                return unsupported("linked exile: the producer is not this object");
            }
            Ok(Some(ir::Filter::All(Vec::new())))
        }
        ir::Filter::All(parts) => {
            let mut linked = false;
            let parts = parts.iter().map(|p| Ok(match linked_exile_filter_bound(p, bind)? {
                Some(p) => { linked = true; p }, None => p.clone(),
            })).collect::<Res<_>>()?;
            Ok(linked.then_some(ir::Filter::All(parts)))
        }
        _ => Ok(None),
    }
}

impl Set {
    pub(crate) fn has_quality(&self) -> bool {
        self.filter.has_quality() || matches!(&self.domain, Domain::Union(arms) if arms.iter().any(Self::has_quality))
    }
    /// The zone/domain pool before identity-dependent quality tests. Used only to
    /// decide whether skipping a choice would disclose a hidden qualification.
    pub(crate) fn unfiltered(&self) -> Self {
        let mut pool = self.clone();
        pool.filter = Filter::All(Vec::new());
        pool.other = false;
        pool.other_than = None;
        if let Domain::Union(arms) = &mut pool.domain {
            for arm in arms { *arm = arm.unfiltered(); }
        }
        pool
    }
    /// A compiler-proven root exclusion; an explicit comparison already in the text wins.
    pub(crate) fn excluding(mut self, subject: Subject) -> Self {
        if self.other && self.other_than.is_none() { self.other_than = Some(subject); }
        self
    }
    /// Control predicates enumerate battlefield objects (109.2, 110.2). A designation
    /// such as "commander" can denote a card in any zone in other contexts, but this
    /// query already supplies the battlefield. Preserve card-vs-token membership.
    pub(crate) fn controlled(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<Set> {
        Self::controlled_with(set, support, bind, None)
    }
    pub(crate) fn controlled_root(set: &ir::ObjectSet, player: &ir::Selector, support: &Support<'_>, bind: Bind<'_>) -> Res<Set> {
        Self::controlled_with(set, support, bind, Some(player))
    }
    fn controlled_with(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>, player: Option<&ir::Selector>) -> Res<Set> {
        let mut compile = |set: &ir::ObjectSet| {
            let compiled = Self::compile(set, support, &mut *bind)?;
            match player {
                Some(player) => Self::exclude_root(compiled, set, || bind.root_controlled_other(set, player), false),
                None => Ok(compiled),
            }
        };
        if set.domain == ir::Domain::Card && set.zone.is_none() {
            let mut permanent = set.clone();
            permanent.domain = ir::Domain::Permanent;
            let mut result = compile(&permanent)?;
            result.domain = Domain::Sorts { sorts: vec![Sort::Card], players: false };
            Ok(result)
        } else { compile(set) }
    }
    /// [`Filter::reads`], of the whole description: its domain, owner and controller
    /// relations name zones, sorts and players, which are in no bit of [`Reads`].
    pub(crate) fn reads(&self) -> Reads {
        self.filter.reads() | match &self.domain {
            Domain::Union(arms) => Reads::each(arms, Set::reads),
            Domain::LinkedExile(_) | Domain::CostExile(_) | Domain::Zones { .. } | Domain::Sorts { .. } => Reads::NONE,
        }
    }

    /// An exclusion proven for this root object selector only. Nested descriptions retain
    /// their own contexts; explicit comparison subjects remain authoritative (201.2a).
    pub(crate) fn compile_excluding(
        set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>,
        excluded: impl FnOnce() -> Res<Option<Subject>>,
        refuse_actor_default: bool,
    ) -> Res<Set> {
        let compiled = Self::compile(set, support, bind)?;
        Self::exclude_root(compiled, set, excluded, refuse_actor_default)
    }

    /// Conditions use the same root resolver without making it an ambient Set binding.
    /// A nested singular player target is announced/captured by Emit before this query.
    pub(crate) fn with_controller(set: &ir::ObjectSet, who: Subject, support: &Support<'_>, bind: Bind<'_>) -> Res<Set> {
        let mut plain = set.clone();
        plain.controller = None;
        let mut compiled = Self::compile(&plain, support, bind)?;
        compiled.controller = Some(Relation::Named { who, negated: false });
        Ok(compiled)
    }

    pub(crate) fn compile_root(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<Set> {
        let compiled = Self::compile(set, support, &mut *bind)?;
        Self::exclude_root(compiled, set, || bind.root_other(set), false)
    }

    fn exclude_root(mut compiled: Set, set: &ir::ObjectSet,
        excluded: impl FnOnce() -> Res<Option<Subject>>, refuse_actor_default: bool,
    ) -> Res<Set> {
        if set.other && compiled.filter.other_than(true)?.is_none() {
            let Some(subject) = excluded()? else {
                // Explicit comparison/target referents remain authoritative. A distributive
                // actor default would exclude a group or the spell, not each dealing member.
                if refuse_actor_default {
                    return unsupported("distributive damage: per-source other recipient set not implemented");
                }
                return Ok(compiled);
            };
            // A direct controller possessive here names the proven target, rather than
            // the self-naming action source used by characteristic amounts elsewhere.
            if set.controller.as_ref().is_some_and(|r| matches!(r.who.quant, Quant::Definite(Ref::ControllerOf(_)))) {
                compiled.controller = Some(Relation::Named { who: Subject::ControllerOf(Box::new(subject.clone())), negated: false });
            }
            compiled.other_than = Some(subject);
        }
        Ok(compiled)
    }

    pub(crate) fn compile(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<Set> {
        if set.caster.is_some() { return unsupported("noun phrase: a caster"); }
        let owner = set.owner.as_ref().map(|r|Relation::compile(r,bind)).transpose()?;
        if set.domain == ir::Domain::Card && let Some(filter) = linked_exile_filter_bound(&set.filter, bind)? {
            if set.zone.is_some() || set.controller.is_some() { return unsupported("linked exile: another zone or controller"); }
            let writer = support.linked_exile_producer()?;
            let domain = if support.cost_exile_writer(writer.clone()) { Domain::CostExile(writer) } else { Domain::LinkedExile(writer) };
            return Ok(Set { domain, other: set.other,
                other_than: if set.other { Some(Subject::Local(bind.exiled()?)) } else { None },
                controller: None, owner, filter: Filter::compile(&filter, support, bind)? });
        }
        if let Some((zones, yours, also_permanents)) = card_zones(set)? {
            let mut owner = owner;
            if !yours && let Some(who) = set.zone.as_ref().and_then(|z| z.owner.as_deref())
                && (crate::select::controlled_by_you(who) == Some(false) || matches!(who.quant, Quant::Definite(_))) {
                // A card in an owned zone has that zone's owner (400.3). Reuse the
                // ordinary relation rather than building a second zone predicate.
                let relation = Relation::compile(&ir::Relation { who: Box::new(who.clone()), negated: false, past: false }, bind)?;
                if owner.as_ref().is_some_and(|existing| existing != &relation) {
                    return unsupported("noun phrase: distinct card and zone owner constraints");
                }
                owner = Some(relation);
            }
            let filter = Filter::compile(&set.filter, support, bind)?;
            return Ok(Set { domain: Domain::Zones { zones, yours, also_permanents }, other: set.other, other_than: filter.other_than(set.other)?.or_else(|| set.other.then(|| bind.other_subject()).flatten()), controller: None, owner, filter });
        }
        // A commander is a physical-card designation (903.3), while a card has a
        // controller only on the battlefield or stack (108.4). An unzoned controlled
        // commander description therefore names those objects, not cards in owned zones.
        fn commander(f: &ir::Filter) -> bool {
            match f {
                ir::Filter::Atom(ir::Atom::Status(ir::selector::Status::Commander)) => true,
                ir::Filter::All(items) => items.iter().any(commander),
                ir::Filter::Any(items) => !items.is_empty() && items.iter().all(commander),
                _ => false,
            }
        }
        let domain = match &set.domain {
            ir::Domain::Card if set.zone.is_none() && set.controller.is_some() && commander(&set.filter) =>
                Domain::Sorts { sorts: vec![Sort::Permanent, Sort::Spell], players: false },
            ir::Domain::Union(arms) if arms.is_empty() => return unsupported("an empty selector union"),
            ir::Domain::Union(arms) => Domain::Union(arms.iter().map(|arm| Set::compile(arm, support, bind)).collect::<Res<_>>()?),
            ir::Domain::Permanent | ir::Domain::Spell | ir::Domain::Ability | ir::Domain::Player | ir::Domain::AnyTarget => Domain::Sorts {
                sorts: set.domain.atomic_alternatives().expect("atomic domain").iter().map(|a| a.sort).collect(),
                players: set.domain == ir::Domain::Player,
            },
            other => return unsupported(format!("noun phrase: {}", name(other))),
        };
        let controller = set.controller.as_ref().map(|r|Relation::compile(r,bind)).transpose()?;
        let filter = Filter::compile(&set.filter, support, bind)?;
        Ok(Set { domain, other: set.other, other_than: filter.other_than(set.other)?.or_else(|| set.other.then(|| bind.other_subject()).flatten()), controller, owner, filter })
    }

    /// Entry prohibitions about cards in their source zones do not inspect the
    /// prospective permanent (614.17d). Other entry subjects need that distinct view.
    pub(crate) fn source_zone_cards(&self) -> bool {
        matches!(self.domain, Domain::Zones { also_permanents: false, .. })
    }

    /// A conservative immutable-zone envelope for local layer recipient indexing.
    /// No effective type, controller, keyword, negative predicate or current match enters
    /// this proof. Callers with prospective/LKI membership use the broad path instead.
    pub(crate) fn layer_zones(&self) -> Option<u16> {
        match &self.domain {
            Domain::Sorts { sorts, .. } => sorts.iter().try_fold(0, |mask, sort| Some(mask | match sort {
                Sort::Permanent | Sort::Of(_) => 1 << Zone::Battlefield as usize,
                Sort::Spell | Sort::Ability => 1 << Zone::Stack as usize,
                Sort::Player => 0,
                Sort::Card => return None,
            })),
            Domain::Zones { zones, also_permanents, .. } => Some(zones.iter().fold(
                if *also_permanents { 1 << Zone::Battlefield as usize } else { 0 },
                |mask, zone| mask | (1 << *zone as usize))),
            Domain::Union(arms) => arms.iter().try_fold(0, |mask, arm|Some(mask | arm.layer_zones()?)),
            Domain::LinkedExile(_) | Domain::CostExile(_) => None,
        }
    }

    /// A fixed public graveyard card domain for inherited-member discovery. This is
    /// a read envelope, not proof that any of the donor's abilities function there.
    pub(crate) fn graveyard_donors(&self) -> bool {
        matches!(&self.domain, Domain::Zones { zones, also_permanents: false, .. }
            if zones.as_slice() == [Zone::Graveyard])
    }

    /// Whether this description explicitly names cards outside the battlefield.
    /// Layer discovery uses this domain fact to include recipients in those zones.
    pub(crate) fn off_battlefield_cards(&self) -> bool {
        match &self.domain {
            Domain::Zones { .. } | Domain::LinkedExile(_) | Domain::CostExile(_) => true,
            Domain::Union(parts) => parts.iter().any(Self::off_battlefield_cards),
            Domain::Sorts { .. } => false,
        }
    }

    /// Is this object or player one the set describes?
    pub fn contains(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> bool {
        // A departed player cannot belong to any player domain, including unions such as
        // "opponent or planeswalker" (800.4a). Apply this before domain-specific filtering.
        if let Target::Player(p) = t
            && game.players[p].lost
        {
            return false;
        }
        if let (Some(owner),Target::Object(o)) = (&self.owner,t)
            && !owner.matches(game,ctx,locals,game.obj(o).owner) { return false; }
        let in_domain = match &self.domain {
            Domain::CostExile(writer) => matches!(t, Target::Object(o) if ctx.source.is_some_and(|source| game.obj(source).cast_exiled.contains(&(writer.clone(), o)))),
            Domain::LinkedExile(writer) => matches!(t, Target::Object(o) if game.obj(o).zone == Zone::Exile && game.is(t, Sort::Card)
                && ctx.source.is_some_and(|source| game.exiled_with.contains(&(source, Some(ctx.link(writer.clone())), o, ctx.link_generation))
                    && game.linked_generation_live(source, &ctx.link(writer.clone()), ctx.link_generation))),
            // A card in an owned zone belongs to that zone's owner (400.3).
            Domain::Zones { zones, yours, also_permanents } => {
                let Target::Object(o) = t else { return false };
                let card = game.obj(o);
                let there = match card.zone {
                    zone if zones.contains(&zone) => (!yours || card.owner == ctx.controller) && game.is(t, Sort::Card),
                    Zone::Battlefield => *also_permanents,
                    _ => false,
                };
                // "another target artifact card": not the card whose text it is, which may be
                // in the graveyard as the new object it became there ("When this creature
                // dies, …")
                let this = self.other && match &self.other_than {
                    Some(subject) => subject.objects(game, ctx, locals).any(|other| other == o),
                    None => ctx.source.is_some_and(|source| o == source || game.became(source) == Some(o)),
                };
                return there && !this && self.filter.matches(game, ctx, locals, t);
            }
            Domain::Union(arms) => arms.iter().any(|arm| arm.contains(game, ctx, locals, t)),
            Domain::Sorts { sorts, .. } => {
                // An object described as it last existed is what it was in the zone it was last
                // in (608.2h): a permanent that has left the battlefield was no spell, and a
                // spell that has left the stack was no permanent (400.7).
                let was_in = |o: ObjId, zone: Zone| ctx.was == Some(o) && {
                    let obj = game.obj(o);
                    (if obj.zone == Zone::Gone { obj.left } else { Some(obj.zone) }) == Some(zone)
                };
                let permanent = |o: ObjId| ctx.on_battlefield(game, o) || was_in(o, Zone::Battlefield);
                sorts.iter().any(|sort| match (*sort, t) {
                    (Sort::Player, Target::Player(_)) => true,
                    (Sort::Permanent, Target::Object(o)) => permanent(o),
                    (Sort::Of(kind), Target::Object(o)) => permanent(o) && ctx.seen_for(game, o, crate::layers::Aspect::Types).is(kind),
                    // a spell cast earlier, asked about after it has left the stack (608.2i)
                    (Sort::Spell, Target::Object(o)) => (game.obj(o).zone == Zone::Stack || was_in(o, Zone::Stack)) && game.obj(o).ability.is_none(),
                    (Sort::Ability, Target::Object(o)) => game.obj(o).zone == Zone::Stack && game.obj(o).ability.is_some(),
                    (Sort::Card, Target::Object(_)) => game.is(t, Sort::Card),
                    _ => false,
                })
            }
        };
        let players = matches!(self.domain, Domain::Sorts { players: true, .. });
        // "another" is other than the object whose text it is, and than the spell that became
        // it (400.7d): "unless you've cast another red spell this turn" does not count the
        // spell this permanent was, asked as it enters or after
        let was_this = |o: ObjId| ctx.source.is_some_and(|source| game.became(o) == Some(source) || game.prospective == Some((source, Some(o))));
        let excluded = self.other && match &self.other_than {
            Some(subject) => subject.evaluate(game, ctx, locals).contains(&t),
            None => ctx.source.map(Target::Object) == Some(t) || matches!(t, Target::Object(o) if was_this(o)) || (players && t == Target::Player(ctx.controller)),
        };
        if !in_domain || excluded {
            return false;
        }
        if let (Some(rel), Target::Object(o)) = (&self.controller, t)
            && !rel.matches(game, ctx, locals, ctx.controlled_by(game, o))
        {
            return false;
        }
        self.filter.matches(game, ctx, locals, t)
    }

    /// How many it describes, for a number. A set that looks back at what attacked or was
    /// dealt damage this turn counts also what has since left the battlefield, as it last
    /// existed (608.2i: an effect that takes no action on them).
    pub(crate) fn counted(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> usize {
        let mut members = self.members(game, ctx, locals);
        let Some(atom) = self.looks_back() else { return members.len() };
        for o in game.looked_back(atom) {
            let was = Ctx { was: Some(o), ..ctx.clone() };
            if game.obj(o).zone == Zone::Gone && !members.contains(&Target::Object(o)) && self.contains(game, &was, locals, Target::Object(o)) {
                members.push(Target::Object(o));
            }
        }
        members.len()
    }

    fn looks_back(&self) -> Option<&Atom> {
        let parts = match &self.filter { Filter::All(parts) => parts.as_slice(), one => std::slice::from_ref(one) };
        parts.iter().find_map(|p| match p {
            Filter::Atom(a @ (Atom::FoughtThisTurn { attacked: true } | Atom::TappedFor { .. } | Atom::Damage { .. })) => Some(a),
            _ => None,
        })
    }

    /// Everything the set describes: players, battlefield objects, stack objects (112.1),
    /// or cards in named zones. Enumerating only permanents would omit
    /// legal spell targets (115.1a).
    pub fn members(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<Target> {
        if let Domain::CostExile(writer) = self.domain.clone() {
            return ctx.source.into_iter().flat_map(|source| game.obj(source).cast_exiled.iter())
                .filter(|(key, _)| *key == writer).map(|(_, card)| Target::Object(*card))
                .filter(|t| self.contains(game, ctx, locals, *t)).collect();
        }
        if matches!(self.domain, Domain::LinkedExile(_)) {
            return game.exile.iter().copied().map(Target::Object).filter(|t| self.contains(game, ctx, locals, *t)).collect();
        }
        if let Domain::Union(arms) = &self.domain {
            // Each arm may search a different zone. Preserve encounter order and emit an
            // object once even when several arms match it (700.1, 608.2c).
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::new();
            for arm in arms {
                for target in arm.members(game, ctx, locals) {
                    if seen.insert(target) && self.contains(game, ctx, locals, target) {
                        out.push(target);
                    }
                }
            }
            return out;
        }
        if let Domain::Zones { zones, also_permanents, .. } = &self.domain {
            let cards = game.players.iter().flat_map(|p| zones.iter().flat_map(move |zone| match zone {
                Zone::Graveyard => p.graveyard.iter(), Zone::Library => p.library.iter(), Zone::Hand => p.hand.iter(),
                _ => unreachable!("compiled owned card zones"),
            })).copied();
            return ctx.permanents(game).iter().copied().filter(|_| *also_permanents).chain(cards)
                .map(Target::Object).filter(|t| self.contains(game, ctx, locals, *t)).collect();
        }
        (0..game.players.len()).filter(|p| !game.players[*p].lost).map(Target::Player)
            .chain(ctx.permanents(game).iter().copied().map(Target::Object))
            .chain(game.stack.iter().copied().map(Target::Object))
            .filter(|t| self.contains(game, ctx, locals, *t)).collect()
    }

    /// A target must match its description and obey targeting restrictions (115.1, 608.2b).
    /// What is not targeted is asked of [`Set::contains`] and [`Set::members`] instead.
    pub fn legal_target(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> bool {
        self.contains(game, ctx, locals, t) && game.can_target(ctx, t)
    }

    /// Why `t` is not a legal target of this description, with the rule it breaks: the first
    /// thing [`Set::legal_target`] finds false of it.
    pub fn why_not(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> String {
        if let Some(reason) = self.unlike(game, ctx, locals, t) {
            return format!("{reason} (115.1)");
        }
        if game.protected_from(t, ctx) {
            return "it has protection from that (702.16b)".into();
        }
        if let Target::Object(o) = t {
            if game.has(o, "Shroud") { return "it has shroud (702.18a)".into(); }
            if game.has(o, "Hexproof") && ctx.controlled_by(game, o) != ctx.controller {
                return "it has hexproof, and an opponent of its controller is targeting it (702.11b)".into();
            }
        }
        game.target_restriction(ctx, t).unwrap_or_else(|| "an effect says it can't be the target of that (115.1)".into())
    }

    /// Only what sort of thing the description is of: without "another", its controller, its
    /// owner and its other qualities.
    fn bare(&self) -> Set {
        let mut bare = self.clone();
        bare.other = false;
        bare.controller = None;
        bare.owner = None;
        bare.filter = Filter::All(Vec::new());
        bare
    }

    /// What of the description `t` is not, in words; None when it is all of it.
    fn unlike(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, t: Target) -> Option<String> {
        if self.contains(game, ctx, locals, t) {
            return None;
        }
        if let Target::Player(p) = t
            && game.players[p].lost
        {
            return Some("that player has left the game".into());
        }
        // of "a spell or a permanent an opponent controls", what the arms it is of say: when one
        // takes it, what is wrong is said of the whole below
        if let Domain::Union(arms) = &self.domain
            && !arms.iter().any(|arm| arm.contains(game, ctx, locals, t))
        {
            let fitting: Vec<&Set> = arms.iter().filter(|arm| arm.bare().contains(game, ctx, locals, t)).collect();
            let asked = if fitting.is_empty() { arms.iter().collect() } else { fitting };
            let mut reasons: Vec<String> = Vec::new();
            for reason in asked.iter().filter_map(|arm| arm.unlike(game, ctx, locals, t)) {
                if !reasons.contains(&reason) { reasons.push(reason); }
            }
            let it = if matches!(t, Target::Player(_)) { "that player" } else { "it" };
            return Some(match reasons.len() {
                0 => format!("{it} doesn't match the target's description"),
                1 => reasons.remove(0),
                _ => format!("{it} is none of what the target allows: {}", reasons.join("; ")),
            });
        }
        let (it, Target::Object(o)) = ("it", t) else {
            let Target::Player(p) = t else { unreachable!() };
            let (who, is) = if p == ctx.controller { ("you", "are") } else { ("that player", "is") };
            return Some(match &self.domain {
                Domain::Sorts { sorts, .. } if !sorts.contains(&Sort::Player) => format!("a player isn't {}", sorts_said(sorts)),
                Domain::Sorts { .. } | Domain::Union(_) => {
                    if self.other && self.other_than.is_none() && p == ctx.controller {
                        "you are the player whose spell or ability this is, and the target says \"another\"".into()
                    } else {
                        match self.filter.unlike(game, ctx, locals, t) {
                            Some(said) if said.starts_with("it isn't ") => format!("{who} {}n't {}", is, &said["it isn't ".len()..]),
                            Some(said) if said.starts_with("it is ") => format!("{who} {is} {}", &said["it is ".len()..]),
                            _ => format!("{who} {is}n't one the target describes"),
                        }
                    }
                }
                _ => "a player isn't a card".into(),
            });
        };
        if let Some(owner) = &self.owner
            && !owner.matches(game, ctx, locals, game.obj(o).owner)
        {
            return Some(format!("the target is one {}, and {}", owner.said("own"), whose(game.obj(o).owner == ctx.controller, "own")));
        }
        match &self.domain {
            Domain::CostExile(_) => return Some(format!("{it} isn't a card exiled to pay this cost")),
            Domain::LinkedExile(_) => return Some(format!("{it} isn't a card exiled with this")),
            Domain::Union(_) => {}
            Domain::Zones { zones, yours, also_permanents } => {
                let card = game.obj(o);
                let named = zones.iter().map(|z| zone_said(*z)).collect::<Vec<_>>().join(" or ");
                if card.zone == Zone::Battlefield && !*also_permanents || card.zone != Zone::Battlefield && !zones.contains(&card.zone) {
                    return Some(format!("{it} isn't in {} {named}", if *yours { "your" } else { "a" }));
                }
                if card.zone != Zone::Battlefield && *yours && card.owner != ctx.controller {
                    return Some(format!("{it} is in an opponent's {}, not yours", zone_said(card.zone)));
                }
            }
            Domain::Sorts { sorts, .. } => {
                if !self.bare().contains(game, ctx, locals, t) {
                    // a player is never on the battlefield: only the permanents described are
                    let (players, permanents): (Vec<Sort>, Vec<Sort>) = sorts.iter().cloned().partition(|s| matches!(s, Sort::Player));
                    let elsewhere = !permanents.is_empty() && permanents.iter().all(|s| matches!(s, Sort::Permanent | Sort::Of(_)))
                        && game.obj(o).zone != Zone::Battlefield;
                    if !elsewhere { return Some(format!("{it} isn't {}", sorts_said(sorts))); }
                    let permanents = format!("{} on the battlefield", sorts_said(&permanents));
                    return Some(if players.is_empty() { format!("{it} isn't {permanents}") }
                        else { format!("{it} isn't {}, or {permanents}", sorts_said(&players)) });
                }
            }
        }
        let mut not_other = self.clone();
        not_other.other = false;
        if not_other.contains(game, ctx, locals, t) {
            // "other" than what the sentence names ("Enchanted creature deals damage … to any
            // other target"), or else than the object whose ability this is
            if self.other_than.is_some() { return Some(format!("{it} is the object the target must be other than")); }
            return Some(format!("{it} is the object whose ability this is, and the target says \"another\""));
        }
        if let Some(controller) = &self.controller
            && !controller.matches(game, ctx, locals, ctx.controlled_by(game, o))
        {
            return Some(format!("the target is one {}, and {}", controller.said("control"), whose(ctx.controlled_by(game, o) == ctx.controller, "control")));
        }
        Some(self.filter.unlike(game, ctx, locals, t).unwrap_or_else(|| format!("{it} doesn't match the target's description")))
    }

    /// Every legal target of this description.
    pub fn targets(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<Target> {
        self.members(game, ctx, locals).into_iter().filter(|t| game.can_target(ctx, *t)).collect()
    }

    /// The chosen values the description reads, for checking they are produced before it.
    pub fn scalars(&self) -> Vec<Scalar> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<Scalar>) {
        if let Domain::Union(arms) = &self.domain {
            arms.iter().for_each(|arm| arm.collect(out));
        }
        if let Some(subject) = &self.other_than { subject.scalars(out); }
        if let Some(relation) = &self.controller { relation.scalars(out); }
        if let Some(relation) = &self.owner { relation.scalars(out); }
        self.filter.scalars(out);
    }
}

/// A description of a card: "a card", "a creature card" (108.2). The action or the zone it
/// names says where to look (a card is discarded from a hand, 701.9a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cards {
    filter: Filter,
}

impl Cards {
    pub(crate) fn has_quality(&self) -> bool { self.filter.has_quality() }
    pub(super) fn any_card() -> Self {Self {filter: Filter::All(Vec::new())}}
    pub(crate) fn compile(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<Cards> {
        if set.domain != ir::Domain::Card || set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() {
            return unsupported("noun phrase: a card with an owner, a controller or a zone");
        }
        Ok(Cards { filter: Filter::compile(&set.filter, support, bind)? })
    }

    pub(crate) fn reads(&self) -> Reads { self.filter.reads() }

    /// Is this card one the description names?
    pub fn contains(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, card: ObjId) -> bool {
        (game.is(Target::Object(card), Sort::Card) || ctx.was == Some(card) && game.obj(card).card.is_some() && game.obj(card).ability.is_none())
            && self.filter.matches(game, ctx, locals, Target::Object(card))
    }

    pub fn scalars(&self) -> Vec<Scalar> {
        let mut out = Vec::new();
        self.filter.scalars(&mut out);
        out
    }
}

/// A player's zone whose cards can be counted without revealing them (400.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pile {
    Hand,
    Library,
    Graveyard,
}

/// "creature cards in your graveyard", "cards in their hand": described cards in one zone
/// of the player the text names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneCards {
    pile: Pile,
    owner: Subject,
    cards: Cards,
}

impl ZoneCards {
    pub(crate) fn compile(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<ZoneCards> {
        let owner = set.zone.as_ref().and_then(|zone| zone.owner.as_deref()).ok_or_else(|| crate::Unsupported("zone: no owner".into()))?;
        let owner = if matches!(owner.quant, Quant::Each | Quant::All) && owner.set.domain == ir::Domain::Player
            && owner.chooser.is_none() && owner.random.is_none() {
            Subject::Players(Box::new(Set::compile(&owner.set, support, &mut *bind)?))
        } else { Subject::compile(owner, &mut *bind)? };
        Self::with_owner(set, owner, support, bind)
    }

    pub(crate) fn with_owner(set: &ir::ObjectSet, owner: Subject, support: &Support<'_>, bind: Bind<'_>) -> Res<ZoneCards> {
        let Some(zone) = &set.zone else { return unsupported("count: no zone") };
        if zone.slice.is_some() {
            return unsupported("count: a slice of a zone");
        }
        let pile = match zone.zones.items.as_slice() {
            [ir::Zone::Hand] => Pile::Hand,
            [ir::Zone::Library] => Pile::Library,
            [ir::Zone::Graveyard] => Pile::Graveyard,
            _ => return unsupported("count: other than a player's hand, library or graveyard"),
        };
        Ok(ZoneCards { pile, owner, cards: Cards::compile(&ir::ObjectSet { zone: None, ..set.clone() }, support, bind)? })
    }

    pub(crate) fn zone(&self) -> Zone {
        match self.pile { Pile::Hand => Zone::Hand, Pile::Library => Zone::Library, Pile::Graveyard => Zone::Graveyard }
    }

    pub(crate) fn members(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<ObjId> {
        self.owner.players(game, ctx, locals).filter(|p| !game.players[*p].lost)
            .flat_map(|p| self.in_player(game, ctx, locals, p)).collect()
    }

    fn in_player<'a>(&'a self, game: &'a Game, ctx: &'a Ctx, locals: &'a dyn Locals, player: PlayerId) -> impl Iterator<Item = ObjId> + 'a {
        let pile = match self.pile {
            Pile::Hand => &game.players[player].hand,
            Pile::Library => &game.players[player].library,
            Pile::Graveyard => &game.players[player].graveyard,
        };
        pile.iter().copied().filter(|card| self.cards.contains(game, ctx, locals, *card))
    }

    pub(crate) fn scalars(&self) -> Vec<Scalar> {
        let mut out = self.cards.scalars();
        self.owner.scalars(&mut out);
        out
    }

    /// How many there are in the zones of the players the text names, as they now are.
    pub(crate) fn owned(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> u32 {
        self.owner.players(game, ctx, locals).map(|player| self.count(game, ctx, locals, player)).sum()
    }

    /// How many there are in that player's zone (608.2h). Counting a hidden zone does not
    /// reveal its cards.
    pub(crate) fn count(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, player: PlayerId) -> u32 {
        self.in_player(game, ctx, locals, player).count() as u32
    }
}

/// Completed result qualification, shared by counts and former-controller projection.
/// The retained original incarnation supplies frozen characteristics/LKI (608.2h).
pub(super) fn qualifying_deaths(game: &Game, ctx: &Ctx, locals: &dyn Locals, slot: super::Slot, filter: &Filter) -> Vec<ObjId> {
    let Some(Value::Destructions(results)) = locals.local(slot) else { unreachable!("verified destruction result") };
    results.iter().filter(|r| r.from == Zone::Battlefield && r.to == Zone::Graveyard
        && filter.matches(game, ctx, locals, Target::Object(r.before))).map(|r| r.before).collect()
}

/// How a completed instruction's result group holds what it did (400.7, 608.2c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The cards as they now are, in the zone it put them.
    Cards,
    /// Each object it moved as it was and as it now is, in pairs.
    Moved,
    /// Its destructions.
    Destroyed,
}
impl Shape {
    pub(crate) fn kind(self) -> Kind { if self == Self::Destroyed { Kind::Destructions } else { Kind::Cards } }
}

/// The members of result groups, each as it was before or as it became, with where it was
/// put when that differs among them: a destroyed permanent goes where replacement effects
/// send it (400.7, 608.2h, 614.6).
pub(super) fn members(locals: &dyn Locals, groups: &[(super::Slot, Shape)], before: bool) -> Vec<(ObjId, Option<Zone>)> {
    let mut out = Vec::new();
    for (slot, shape) in groups {
        match (shape, locals.local(*slot)) {
            (Shape::Cards, Some(Value::Cards(cards))) => out.extend(cards.iter().map(|c| (*c, None))),
            (Shape::Moved, Some(Value::Cards(pairs))) => out.extend(pairs.chunks(2).map(|p| (if before { p[0] } else { p[1] }, None))),
            (Shape::Destroyed, Some(Value::Destructions(results))) => out.extend(results.iter().map(|r| (if before { r.before } else { r.after }, Some(r.to)))),
            _ => unreachable!("verified completed result group"),
        }
    }
    out
}

/// "card exiled from their hand this way": what the instruction did, the one zone of a player
/// the text names that it took the cards from, and the rest of the description.
fn moved_from_this_way(filter: &ir::Filter) -> Option<(ir::Verb, Zone, ir::Filter)> {
    let ir::Filter::All(parts) = filter else { return None };
    let from = |p: &ir::Filter| {
        let ir::Filter::Atom(ir::Atom::Did { action, when: ir::amount::When::ThisWay, negated: false }) = p else { return None };
        let ir::EventAction::Deed(ir::Deed { verb, passive: true, args }) = &**action else { return None };
        let [(ir::Role::From, ir::Arg::Zone(ir::event::ZoneArg { zone, other_than: false }))] = args.as_slice() else { return None };
        let owner = zone.owner.as_deref().filter(|o| matches!(o.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::They | ir::Anaphor::That)) | Quant::Definite(Ref::You)))?;
        if owner.chooser.is_some() || zone.slice.is_some() { return None; }
        let zone = match zone.zone()? { ir::Zone::Hand => Zone::Hand, ir::Zone::Library => Zone::Library, ir::Zone::Graveyard => Zone::Graveyard, _ => return None };
        Some((*verb, zone))
    };
    let [(verb, zone)] = parts.iter().filter_map(from).collect::<Vec<_>>()[..] else { return None };
    Some((verb, zone, ir::Filter::All(parts.iter().filter(|p| from(p).is_none()).cloned().collect())))
}

/// The zone a member of moved groups left: what it was, as last known (400.7).
fn left_from(game: &Game, locals: &dyn Locals, groups: &[(super::Slot, Shape)], member: ObjId, before: bool) -> Option<Zone> {
    groups.iter().find_map(|(slot, _)| match locals.local(*slot) {
        Some(Value::Cards(pairs)) => pairs.chunks(2).find(|p| (if before { p[0] } else { p[1] }) == member).map(|p| p[0]),
        _ => None,
    }).and_then(|was| game.obj(was).left)
}

/// An event said in a description, as the turn's history is asked about it: "put into its
/// owner's graveyard" is into whichever graveyard it went to, its owner's (404.2).
fn history_action(action: &ir::EventAction) -> ir::EventAction {
    let mut action = action.clone();
    if let ir::EventAction::PutIntoGraveyard { into, .. } = &mut action
        && matches!(&into.quant, ir::Quant::Definite(ir::Ref::OwnerOf(of)) if of.quant == ir::Quant::Definite(ir::Ref::Anaphor(ir::Anaphor::It))) {
        **into = ir::Selector::new(ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), ir::ObjectSet::of(ir::Domain::Player));
    }
    action
}

/// "the number of creatures that died under your control this turn", "creatures that entered
/// the battlefield under your control this turn": what is counted is each such event of the
/// turn, of what it happened to as it was, whether or not it is still there (608.2i). The
/// set's own relations ("you control") are of objects as they are now, and are not this.
fn history_count(set: &ir::ObjectSet) -> Option<ir::EventPattern> {
    let ir::Filter::All(parts) = &set.filter else { return None };
    let did = |p: &ir::Filter| matches!(p, ir::Filter::Atom(ir::Atom::Did { when: ir::amount::When::ThisTurn, negated: false, action })
        if matches!(&**action, ir::EventAction::Deed(d) if matches!(d.verb, ir::Verb::Die | ir::Verb::Enter | ir::Verb::LeaveBattlefield) && !d.passive)
            || matches!(&**action, ir::EventAction::PutIntoGraveyard { .. }));
    let [action] = parts.iter().filter(|p| did(p)).collect::<Vec<_>>()[..] else { return None };
    let ir::Filter::Atom(ir::Atom::Did { action, .. }) = action else { unreachable!() };
    if set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() || set.other
        || parts.iter().any(|p| !did(p) && matches!(p, ir::Filter::Atom(ir::Atom::Did { .. } | ir::Atom::Reference(_)))) { return None; }
    let mut subject = set.clone();
    subject.filter = ir::Filter::All(parts.iter().filter(|p| !did(p)).cloned().collect());
    Some(ir::EventPattern::new(vec![ir::Selector::new(ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), subject)], vec![history_action(action)]))
}

/// Identify the positive operation-local dying predicate, preserving all other filters.
fn death_filter(filter: &ir::Filter) -> Option<ir::Filter> {
    match filter {
        ir::Filter::Atom(ir::Atom::Did { action, when: ir::amount::When::ThisWay, negated: false })
            if matches!(&**action, ir::EventAction::Deed(d) if d.verb == ir::Verb::Die && !d.passive && d.args.is_empty()) => Some(ir::Filter::All(Vec::new())),
        ir::Filter::Atom(ir::Atom::Did { action, when: ir::amount::When::ThisWay, negated: false })
            if matches!(&**action, ir::EventAction::PutIntoGraveyard { into, from: None | Some(ir::Zone::Battlefield) }
                if **into == ir::Selector::new(ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), ir::ObjectSet::of(ir::Domain::Player))) => Some(ir::Filter::All(Vec::new())),
        ir::Filter::All(parts) if parts.iter().any(|p| death_filter(p).is_some()) => Some(ir::Filter::All(parts.iter().map(|p| death_filter(p).unwrap_or_else(|| p.clone())).collect())),
        _ => None,
    }
}

/// A number the text says, as it is computed. Values within calculations and comparisons
/// stay signed (107.1b): a negative component still contributes to a total, and a negative
/// threshold is not replaced with zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Number {
    Product(Box<Number>, Box<Number>),
    Half { of: Box<Number>, round_up: bool },
    Sum(Box<Number>, Box<Number>),
    Difference(Box<Number>, Box<Number>),
    Const(u32),
    /// The announced X, or what "where X is …" made it (107.3).
    X,
    EventAmount,
    /// Distinct colors actually paid to cast this source; uncast copies paid none.
    ManaColorsSpent,
    /// "The life lost this way": the identified life-loss instruction's result.
    LifeLost(super::Slot),
    Damage { slot: super::Slot, to: Option<Subject>, kind: ir::event::DamageKind, excess: bool },
    ResultCount(super::Slot),
    RemovedCount { slot: super::Slot, kind: Option<String> },
    /// Filter the completed discard-time identities, not resulting-zone characteristics.
    FilteredResultCount { slot: super::Slot, filter: Box<Filter> },
    DeathCount { slot: super::Slot, filter: Box<Filter> },
    DeathCountByActor { slot: super::Slot, filter: Box<Filter> },
    /// The described members of completed instructions' result groups, all of the
    /// producers "this way" names (608.2c). `before`: each as it was when the instruction
    /// moved it (608.2h), else as it became (400.7). `into`: only those it put there.
    /// `yours`: only the controller's own, of what each player did to their own cards.
    /// `other`: "another Desert": not the source of the ability, as it was.
    /// `from`: "cards exiled from their hand this way": only the members moved from that zone.
    Members { groups: Vec<(super::Slot, Shape)>, filter: Box<Filter>, before: bool, into: Option<Zone>, yours: bool, other: bool, from: Option<Zone> },
    /// How many events of this turn the pattern describes ([`history_count`]).
    Happened(Box<crate::trigger::Pattern>),
    /// Described cards in the hand, library or graveyard of the player the text names.
    CountZone(Box<ZoneCards>),
    Count(Box<Set>),
    /// Distinct card types, counting each type once even on multitype cards (205.2).
    CardTypes(Box<Set>),
    /// Union of the five colors among current effective members (105.2–3).
    Colors(Box<Set>),
    /// "Its power", "that card's mana value": of the object named, as it is or last was.
    Stat(Subject, Stat),
    /// The counters on what is named.
    Counters(Subject, Option<ir::CounterKind>),
    CountersAmong(Box<Set>, Option<ir::CounterKind>),
    PlayerCounters(Subject, String),
    /// "Your life total", "that player's life total".
    Life(Subject),
    /// "where X is your speed" (702.179f).
    Speed(Subject),
    Aggregate(Aggregate, Stat, Box<Set>),
    /// "Each time it was kicked": how many times what is named was (702.33d).
    Kicked(Subject),
    Replicated(Subject),
    /// X in an enters-the-battlefield ability of a permanent: the X of the spell it was as
    /// that spell resolved, or 0 when it was no such spell (107.3m).
    SpellX,
}

/// The finite card-type vocabulary represented once for counts and collective costs.
pub(crate) fn card_type_mask(game: &Game, ctx: &Ctx, object: ObjId) -> u16 {
    ctx.seen_for(game, object, crate::layers::Aspect::Types).types.iter()
        .fold(0, |mask, kind| mask | (1u16 << ir::CardType::ALL.iter().position(|t| t == kind).expect("card type vocabulary")))
}

impl Number {
    /// Whether changing only power/toughness can change this compiled number.
    /// Subject identities, domain sorts and controller/owner relations read no P/T.
    fn reads_pt(&self) -> bool {
        fn filter(f: &Filter) -> bool {
            match f {
                Filter::All(parts) | Filter::Any(parts) => parts.iter().any(filter),
                Filter::Not(inner) => filter(inner),
                Filter::Atom(Atom::Stat { stat, value, .. }) => *stat != Stat::ManaValue || value.reads_pt(),
                Filter::Atom(Atom::Most{stat,among,..})=> *stat!=Stat::ManaValue || set(among),
                Filter::Atom(Atom::Targets(sets)) => sets.iter().any(set),
                Filter::Atom(Atom::HappenedTo(_)) => true,
                Filter::Atom(Atom::Type(_) | Atom::PermanentType | Atom::HasAdventure | Atom::PreparedSpell | Atom::BasicLandType | Atom::ManaCost(_) | Atom::Subtype(_) | Atom::Supertype(_)
                    | Atom::Color(_) | Atom::Colorless | Atom::Multicolored | Atom::Monocolored
                    | Atom::Opponent | Atom::Keyword(_) | Atom::Tapped(_) | Atom::Token | Atom::Saddled | Atom::Commander | Atom::FaceDown
                    | Atom::Combat(_) | Atom::Attached(_) | Atom::Ability { .. } | Atom::Chosen(_) | Atom::SameName(_) | Atom::SharesCreatureType(_) | Atom::SharesColor(_)
                    | Atom::TargetCount(_) | Atom::Named(_) | Atom::EnteredThisTurn | Atom::LostLifeThisTurn | Atom::DealtDamageBy(_) | Atom::TappedFor { .. } | Atom::Damage { .. } | Atom::FoughtThisTurn { .. } | Atom::Counters { .. } | Atom::Level { .. } | Atom::Entered(_) | Atom::InSubjects(_)) => false,
            }
        }
        fn set(s: &Set) -> bool {
            filter(&s.filter) || match &s.domain {
                Domain::Union(parts) => parts.iter().any(set),
                Domain::LinkedExile(_) | Domain::CostExile(_) | Domain::Zones { .. } | Domain::Sorts { .. } => false,
            }
        }
        match self {
            Number::FilteredResultCount { filter: f, .. } | Number::DeathCount { filter: f, .. } | Number::DeathCountByActor { filter: f, .. } | Number::Members { filter: f, .. } => filter(f), // conservative for nested numeric readers
            Number::Happened(_) => true,
            Number::Half { of, .. } => of.reads_pt(),
            Number::Product(a,b) | Number::Sum(a,b) | Number::Difference(a,b) => a.reads_pt() || b.reads_pt(),
            Number::Stat(_,stat) => *stat != Stat::ManaValue,
            Number::Aggregate(_,stat,among) => *stat != Stat::ManaValue || set(among),
            Number::Colors(among) | Number::CardTypes(among) | Number::Count(among) | Number::CountersAmong(among,_) => set(among),
            Number::CountZone(zone) => filter(&zone.cards.filter),
            Number::Const(_) | Number::X | Number::SpellX | Number::EventAmount | Number::ManaColorsSpent | Number::LifeLost(_) | Number::Damage { .. } | Number::ResultCount(_) | Number::RemovedCount { .. }
                | Number::Counters(_,_) | Number::PlayerCounters(_,_) | Number::Life(_) | Number::Speed(_) | Number::Kicked(_) | Number::Replicated(_) => false,
        }
    }

    /// What computing this number can read ([`Reads`]). What it is a number of is named,
    /// not described: an identity reads none of them.
    pub(crate) fn reads(&self) -> Reads {
        match self {
            Number::ManaColorsSpent => Reads::HISTORY,
            Number::FilteredResultCount { filter, .. } | Number::DeathCount { filter, .. } | Number::DeathCountByActor { filter, .. } | Number::Members { filter, .. } => filter.reads(),
            Number::Happened(_) => Reads::ALL,
            Number::Half { of, .. } => of.reads(),
            Number::Product(a,b) | Number::Sum(a,b) | Number::Difference(a,b) => a.reads() | b.reads(),
            Number::Aggregate(_,_,among) | Number::Colors(among) | Number::CardTypes(among) | Number::Count(among) | Number::CountersAmong(among,_) => among.reads(),
            Number::CountZone(zone) => zone.cards.reads(),
            Number::Const(_) | Number::X | Number::SpellX | Number::EventAmount | Number::LifeLost(_) | Number::Damage { .. } | Number::ResultCount(_) | Number::RemovedCount { .. } | Number::Stat(_,_)
                | Number::Counters(_,_) | Number::PlayerCounters(_,_) | Number::Life(_) | Number::Speed(_) | Number::Kicked(_) | Number::Replicated(_) => Reads::NONE,
        }
    }

    pub(crate) fn death_count(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        if set.domain != ir::Domain::Permanent || set.zone.is_some() || set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.other {
            return unsupported("death result: unsupported domain or relationship");
        }
        let slot = bind.destructions()?;
        Ok(Self::DeathCount { slot, filter: Box::new(Filter::compile(&set.filter, support, bind)?) })
    }

    /// "creatures destroyed this way", "a creature card exiled this way": the described
    /// members of the result groups the reference names (608.2c). A permanent is described as
    /// it last was on the battlefield (608.2h), a card as the card it became (400.7).
    /// `by_you`: "you exiled": of what the controller did. Each player discards and mills
    /// their own cards (400.3), so of those the controller's are their own.
    /// Of the members of result groups it counts, those it counts, in order; none of
    /// another number.
    pub(crate) fn members_counted(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Vec<ObjId> {
        let Number::Members { groups, filter, before, into, yours, other, from } = self else { return Vec::new() };
        members(locals, groups, *before).into_iter()
            .filter(|(o, put)| put.is_none_or(|put| into.is_none_or(|zone| zone == put)) && (!yours || game.obj(*o).owner == ctx.controller)
                && from.is_none_or(|zone| left_from(game, locals, groups, *o, *before) == Some(zone))
                && !(*other && ctx.source == Some(*o))
                && filter.matches(game, ctx, locals, Target::Object(*o)))
            .map(|(o, _)| o).collect()
    }

    pub(crate) fn members(set: &ir::ObjectSet, verb: Option<ir::Verb>, into: Option<Zone>, by_you: bool, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        if !matches!(set.domain, ir::Domain::Card | ir::Domain::Permanent) || set.zone.is_some() || set.controller.is_some()
            || set.owner.is_some() || set.caster.is_some() || set.other && set.domain != ir::Domain::Permanent {
            return unsupported("result group: unsupported domain or relationship");
        }
        let groups = bind.this_way(verb, into)?;
        let yours = by_you && groups.iter().any(|(_, _, yours)| !yours);
        if yours && !matches!(verb, Some(ir::Verb::Discard | ir::Verb::Mill)) {
            return unsupported("result group: what the controller did, of an instruction several players did");
        }
        let groups = groups.into_iter().map(|(slot, shape, _)| (slot, shape)).collect();
        Ok(Self::Members { groups, filter: Box::new(Filter::compile(&set.filter, support, bind)?), before: set.domain != ir::Domain::Card, into, yours, other: set.other, from: None })
    }

    /// "a card for each card exiled from their hand this way": the members of the groups
    /// that the instruction moved from that zone, each known by the zone it left (400.7).
    /// Only instructions that keep each card as it was can say where it was.
    fn members_from(set: &ir::ObjectSet, verb: ir::Verb, from: Zone, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        let Self::Members { groups, filter, before, into, yours, other, .. } = Self::members(set, Some(verb), None, false, support, bind)? else { unreachable!() };
        if groups.iter().any(|(_, shape)| *shape != Shape::Moved) { return unsupported("result group: where a member was, of an instruction that does not keep it"); }
        Ok(Self::Members { groups, filter, before, into, yours, other, from: Some(from) })
    }

    pub(crate) fn compile(amount: &ir::Amount, support: &Support<'_>, bind: Bind<'_>) -> Res<Number> {
        // "your hand", "their graveyard": a zone of one the text refers to, not "a
        // graveyard" or "all graveyards", which describe cards wherever they are
        let of_one = |set: &ir::ObjectSet| {
            set.domain == ir::Domain::Card && set.zone.as_ref().and_then(|z| z.owner.as_deref()).is_some_and(|who| matches!(who.quant, Quant::Definite(_)))
        };
        Ok(match amount {
            ir::Amount::That { many } => {
                if !*many && let Some(amount) = bind.condition_amount() {
                    amount.0
                } else if matches!(support.ability(), Some(ir::Ability::Triggered { trigger, .. }) if crate::trigger::bound_shape(trigger).event_amount.is_some()) {
                    Number::EventAmount
                } else if *many {
                    // as many as the objects the instruction before it acted on (608.2c)
                    let (slot, shape) = bind.latest_group()?;
                    Number::Members { groups: vec![(slot, shape)], filter: Box::new(Filter::All(Vec::new())), before: false, into: None, yours: false, other: false, from: None }
                } else {
                    return unsupported("that much/many without a quantitative trigger event");
                }
            }
            ir::Amount::ManaSpent { spell, measure: ir::amount::ManaMeasure::Colors }
                if spell.quant == Quant::Definite(ir::Ref::This) => {
                    if !bind.cast_payment_available() {
                        return unsupported("mana spent quantity: unavailable before cast payment");
                    }
                    Number::ManaColorsSpent
                },
            ir::Amount::CountersRemoved { kind, when: ir::amount::When::ThisWay } => Number::RemovedCount { slot: bind.removals()?, kind: kind.as_ref().map(|k| k.0.clone()) },
            ir::Amount::Damage { by: None, to, kind, excess, when: ir::amount::When::ThisWay | ir::amount::When::Context } => {
                Number::Damage { slot: bind.damage()?, to: to.as_ref().map(|s| bind.amount_subject(s)).transpose()?, kind: *kind, excess: *excess }
            }
            ir::Amount::Const(n) => Number::Const(*n),
            ir::Amount::Recalled(ir::amount::Recalled::Number) => bind.number()?.0,
            ir::Amount::Half { of, round_up: Some(round_up) } => Number::Half { of: Box::new(Self::compile(of, support, bind)?), round_up: *round_up },
            // Literal multiples of announced X reuse checked Product arithmetic.
            // Other multiplicative quantity forms remain outside this bounded owner.
            ir::Amount::Multiple(factor,n) if matches!(n.as_ref(), ir::Amount::X) => Number::Product(Box::new(Number::Const(*factor)),Box::new(Self::compile(n,support,bind)?)),
            ir::Amount::Plus(a,b) => Number::Sum(Box::new(Self::compile(a,support,bind)?),Box::new(Self::compile(b,support,bind)?)),
            ir::Amount::Minus(a,b) => Number::Difference(Box::new(Self::compile(a,support,bind)?),Box::new(Self::compile(b,support,bind)?)),
            ir::Amount::LifeChange { player: None, gained: false, when: ir::amount::When::ThisWay } => Number::LifeLost(bind.life_lost()?),
            ir::Amount::X if bind.x() || support.x_available() => Number::X,
            ir::Amount::X if support.spell_x() => Number::SpellX,
            // "for each target" names the announced targets, not all objects/players
            // that could be targets. It needs a bound occurrence (601.2c), which this
            // unqualified set does not carry.
            ir::Amount::Count(set) if set.domain == ir::Domain::AnyTarget => return unsupported("amount: counting announced targets requires a bound occurrence"),
            ir::Amount::Count(set) if matches!(&set.filter, ir::Filter::All(parts) if parts.iter().any(|p| matches!(p, ir::Filter::Atom(ir::Atom::Reference(r)) if r.form == RefForm::ActionResult))) => {
                let ir::Filter::All(parts) = &set.filter else { unreachable!() };
                let references: Vec<_> = parts.iter().filter_map(|p| if let ir::Filter::Atom(ir::Atom::Reference(r)) = p { Some(r) } else { None }).collect();
                let [r] = references.as_slice() else { return unsupported("result quantity: ambiguous card reference"); };
                let [ir::reference::Qualifier::ThisWay(verb)] = r.qualifiers.as_slice() else { return unsupported("result quantity: ambiguous action reference"); };
                if r.expected != Kind::Cards { return unsupported("result quantity: noncard result"); }
                let filtered = parts.len() != 1;
                // the described members of the groups it names, each as it was or became; a
                // discard is counted of the cards as they were in hand
                if *verb != ir::Verb::Discard {
                    let mut rest = (**set).clone();
                    rest.filter = ir::Filter::All(parts.iter().filter(|p| !matches!(p, ir::Filter::Atom(ir::Atom::Reference(_)))).cloned().collect());
                    return Self::members(&rest, Some(*verb), None, false, support, bind);
                }
                if set.domain != ir::Domain::Card || set.controller.is_some() || set.owner.is_some() || set.caster.is_some() || set.zone.is_some() || set.other {
                    return unsupported("result quantity: qualified card set");
                }
                let slot = bind.result_cards(*verb, filtered)?;
                if filtered {
                    let filter = ir::Filter::All(parts.iter().filter(|p| !matches!(p, ir::Filter::Atom(ir::Atom::Reference(_)))).cloned().collect());
                    Number::FilteredResultCount { slot, filter: Box::new(Filter::compile(&filter, support, bind)?) }
                } else { Number::ResultCount(slot) }
            }
            ir::Amount::Count(set) if moved_from_this_way(&set.filter).is_some() => {
                let (verb, from, rest) = moved_from_this_way(&set.filter).expect("matched");
                Self::members_from(&ir::ObjectSet { filter: rest, ..(**set).clone() }, verb, from, support, bind)?
            }
            ir::Amount::Count(set) if death_filter(&set.filter).is_some() => {
                let mut original = (**set).clone();
                original.filter = death_filter(&set.filter).unwrap();
                Self::death_count(&original, support, bind)?
            }
            ir::Amount::Count(set) if history_count(set).is_some() => Number::Happened(Box::new(crate::trigger::Pattern::compile(&history_count(set).unwrap(), support)?)),
            ir::Amount::Count(set) if of_one(set) => Number::CountZone(Box::new(ZoneCards::compile(set, support, bind)?)),
            ir::Amount::Distinct { what: ir::amount::Characteristic::Color, among } => Number::Colors(Box::new(Set::compile(among, support, bind)?)),
            ir::Amount::Distinct { what: ir::amount::Characteristic::CardType, among } => Number::CardTypes(Box::new(Set::compile(among, support, bind)?)),
            ir::Amount::Count(set) => Number::Count(Box::new(Set::compile(set, support, bind)?)),
            ir::Amount::Stat { of, stat } => {
                let subject = Subject::compile_amount(of, bind)?;
                bind.singular_read(&subject)?;
                Number::Stat(subject, Stat::compile(*stat, "amount")?)
            },
            ir::Amount::Counters { on, kind } => {
                let subject = Subject::compile(on, bind)?;
                bind.singular_read(&subject)?;
                Number::Counters(subject, kind.clone())
            },
            ir::Amount::CountersAmong { among, kind } => Number::CountersAmong(Box::new(Set::compile(among, support, bind)?), kind.clone()),
            ir::Amount::PlayerCounters { player, kind } => Number::PlayerCounters(Subject::compile(player, bind)?, player_counter_kind(kind)?),
            ir::Amount::Life(who) => Number::Life(Subject::compile(who, bind)?),
            ir::Amount::Speed(who) => Number::Speed(Subject::compile(who, bind)?),
            // an ability that asks whether its object was kicked is linked to that object's
            // kicker ability and refers to nothing else (702.33e): "it" is its source
            ir::Amount::Times { what, fact: ir::amount::Occurrence::Replicated } if matches!(what.quant, Quant::Definite(Ref::This)) => Number::Replicated(Subject::Source),
            ir::Amount::Times { what, fact: ir::amount::Occurrence::Kicked } if matches!(what.quant, Quant::Definite(Ref::This | Ref::Anaphor(_))) => Number::Kicked(Subject::Source),
            ir::Amount::Aggregate { agg, stat, among } => {
                let among = Set::compile(among, support, bind)?;
                Number::Aggregate(*agg, Stat::compile(*stat, "amount")?, Box::new(among))
            }
            other => return unsupported(format!("amount: {}", name(other))),
        })
    }

    fn checked_evaluate(&self, game:&Game, ctx:&Ctx, locals:&dyn Locals)->Res<i64> {
        let overflow=||crate::Unsupported("quantity expression exceeds the engine numerical range".into());
        let value=|n:&Number|n.checked_evaluate(game,ctx,locals);
        let counters=|o:ObjId, kind:Option<&ir::CounterKind>|->Res<i64> {
            game.obj(o).counters.iter().filter(|(k,_)|kind.is_none_or(|kind|kind.0==**k))
                .try_fold(0i64,|sum,(_,n)|sum.checked_add(i64::from(*n)).ok_or_else(overflow))
        };
        match self {
            Number::Damage { slot, to, kind, excess } => {
                let Some(Value::Damage(results)) = locals.local(*slot) else { unreachable!("verified completed damage result") };
                let recipients = to.as_ref().map(|s| s.evaluate(game, ctx, locals));
                results.iter().filter(|r| recipients.as_ref().is_none_or(|to| to.contains(&r.recipient))
                    && match kind { ir::event::DamageKind::Any => true, ir::event::DamageKind::Combat => r.combat, ir::event::DamageKind::Noncombat => !r.combat })
                    .try_fold(0i64, |sum, r| sum.checked_add(i64::from(if *excess { r.excess } else { r.amount })).ok_or_else(overflow))
            },
            Number::Product(a,b)=>value(a)?.checked_mul(value(b)?).ok_or_else(overflow),
            Number::Sum(a,b)=>value(a)?.checked_add(value(b)?).ok_or_else(overflow),
            Number::Difference(a,b)=>value(a)?.checked_sub(value(b)?).ok_or_else(overflow),
            Number::Half{of,round_up}=>{let n=value(of)?;Ok(n.div_euclid(2)+i64::from(*round_up&&n.rem_euclid(2)!=0))},
            Number::Counters(on,kind)=>on.objects(game,ctx,locals).try_fold(0i64,|sum,o|sum.checked_add(counters(o,kind.as_ref())?).ok_or_else(overflow)),
            Number::CountersAmong(set,kind)=>set.members(game,ctx,locals).into_iter().filter_map(|t|if let Target::Object(o)=t{Some(o)}else{None})
                .try_fold(0i64,|sum,o|sum.checked_add(counters(o,kind.as_ref())?).ok_or_else(overflow)),
            // Leaf quantities retain their authoritative bindings and occurrence semantics.
            _=>Ok(self.evaluate(game,ctx,locals)),
        }
    }

    pub(crate) fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> i64 {
        let objects = |set: &Set| set.members(game, ctx, locals).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None });
        match self {
            Number::Half { of, round_up } => { let n = of.evaluate(game, ctx, locals); n.div_euclid(2) + i64::from(*round_up && n.rem_euclid(2) != 0) },
            Number::Product(a, b) => a.evaluate(game, ctx, locals).saturating_mul(b.evaluate(game, ctx, locals)),
            Number::Sum(a,b) => a.evaluate(game,ctx,locals).saturating_add(b.evaluate(game,ctx,locals)),
            Number::Difference(a,b) => a.evaluate(game,ctx,locals).saturating_sub(b.evaluate(game,ctx,locals)),
            Number::Const(n) => i64::from(*n),
            Number::Damage { slot, to, kind, excess } => {
                let Some(Value::Damage(results)) = locals.local(*slot) else { unreachable!("verified completed damage result") };
                let recipients = to.as_ref().map(|s| s.evaluate(game, ctx, locals));
                results.iter().filter(|r| recipients.as_ref().is_none_or(|to| to.contains(&r.recipient))
                    && match kind { ir::event::DamageKind::Any => true, ir::event::DamageKind::Combat => r.combat, ir::event::DamageKind::Noncombat => !r.combat })
                    .map(|r| i64::from(if *excess { r.excess } else { r.amount })).sum()
            }
            Number::EventAmount => i64::from(locals.event_amount().expect("compiled quantity requires its occurrence event amount")),
            Number::ManaColorsSpent => ctx.source.map_or(0, |source|
                game.obj(source).mana_paid.colors.iter().filter(|n| **n > 0).count() as i64),
            // an instruction that found no player to lose life lost none
            Number::FilteredResultCount { slot, filter } => match locals.local(*slot) {
                Some(Value::Cards(cards)) => cards.iter().filter(|card| filter.matches(game, ctx, locals, Target::Object(**card))).count() as i64,
                _ => unreachable!("verified original-card result"),
            },
            Number::RemovedCount { slot, kind } => {
                let Some(Value::Removals(results)) = locals.local(*slot) else { unreachable!("verified counter removal result") };
                results.iter().filter(|r| kind.as_ref().is_none_or(|k| k == &r.kind)).map(|r| i64::from(r.count)).sum()
            }
            Number::DeathCount { slot, filter } => qualifying_deaths(game, ctx, locals, *slot, filter).len() as i64,
            Number::Members { .. } => self.members_counted(game, ctx, locals).len() as i64,
            Number::DeathCountByActor { slot, filter } => {
                let actor = locals.actor().expect("result creation evaluates one actor");
                qualifying_deaths(game, ctx, locals, *slot, filter).into_iter().filter(|o| game.controller(*o) == actor).count() as i64
            },
            Number::ResultCount(slot) => match locals.local(*slot) { Some(Value::Cards(cards)) => cards.len() as i64, _ => unreachable!("verified completed card result") },
            Number::LifeLost(slot) => match locals.local(*slot) { Some(Value::Number(n)) => i64::from(*n), _ => unreachable!("compiled numeric producer is defined") },
            Number::X => i64::from(ctx.x.expect("X is known wherever an amount compiled with X is evaluated (107.3)")),
            Number::SpellX => ctx.source.and_then(|o| game.obj(o).spell_x).map_or(0, i64::from),
            Number::CountZone(count) => i64::from(count.owned(game, ctx, locals)),
            Number::Colors(set) => {
                let mut colors = std::collections::HashSet::new();
                for object in objects(set) {
                    colors.extend(ctx.seen_for(game, object, crate::layers::Aspect::Colors).colors.iter().copied());
                }
                colors.len() as i64
            },
            Number::CardTypes(set) => objects(set).fold(0u16, |mask, object| mask | card_type_mask(game, ctx, object)).count_ones() as i64,
            Number::Count(set) => set.counted(game, ctx, locals) as i64,
            Number::Happened(pattern) => game.happened(ctx, pattern, crate::history::Turn::This, false) as i64,
            // of each object named, which is one where the text says "its"; of nothing, zero
            Number::Stat(of, stat) => of.objects(game, ctx, locals).map(|o| stat.value(game, ctx, o)).sum(),
            Number::Counters(on, kind) => on.objects(game, ctx, locals).map(|o| i64::try_from(counters_on(game, o, kind.as_ref())).unwrap_or(i64::MAX)).sum(),
            Number::CountersAmong(among, kind) => objects(among).map(|o| i64::try_from(counters_on(game, o, kind.as_ref())).unwrap_or(i64::MAX)).sum(),
            Number::PlayerCounters(who, kind) => who.players(game, ctx, locals).map(|p| i64::from(game.counters(crate::Target::Player(p)).get(kind).copied().unwrap_or(0))).sum(),
            Number::Life(who) => who.players(game, ctx, locals).map(|p| i64::from(game.players[p].life)).sum(),
            Number::Speed(who) => who.players(game, ctx, locals).map(|p| i64::from(game.speed(p))).sum(),
            Number::Replicated(what) => what.objects(game, ctx, locals).map(|o| i64::from(game.obj(o).replicated)).sum(),
            Number::Kicked(what) => what.objects(game, ctx, locals).map(|o| i64::from(game.obj(o).kicked)).sum(),
            Number::Aggregate(agg, stat, among) => {
                let values = objects(among).map(|o| stat.value(game, ctx, o));
                match agg {
                    Aggregate::Greatest => values.max().unwrap_or(0),
                    Aggregate::Least => values.min().unwrap_or(0),
                    Aggregate::Total => values.sum(),
                }
            }
        }
    }

    pub(crate) fn scalars(&self, out: &mut Vec<Scalar>) {
        match self {
            Number::CountZone(count) => {
                count.owner.scalars(out);
                count.cards.filter.scalars(out);
            }
            Number::Colors(set) | Number::CardTypes(set) | Number::Count(set) | Number::CountersAmong(set, _) | Number::Aggregate(_, _, set) => set.collect(out),
            Number::Stat(subject, _) | Number::Counters(subject, _) | Number::PlayerCounters(subject, _) | Number::Life(subject) | Number::Speed(subject) | Number::Kicked(subject) | Number::Replicated(subject) => subject.scalars(out),
            Number::Half { of, .. } => of.scalars(out),
            Number::Happened(_) => {}
            Number::Product(a, b) | Number::Sum(a,b) | Number::Difference(a,b) => { a.scalars(out); b.scalars(out); }
            Number::FilteredResultCount { slot, filter } => { out.push(Scalar::Local { slot: *slot, kind: Kind::Cards }); filter.scalars(out); },
            Number::DeathCount { slot, filter } | Number::DeathCountByActor { slot, filter } => { out.push(Scalar::Local { slot: *slot, kind: Kind::Destructions }); filter.scalars(out); },
            Number::Members { groups, filter, .. } => { out.extend(groups.iter().map(|(slot, shape)| Scalar::Local { slot: *slot, kind: shape.kind() })); filter.scalars(out); },
            Number::ResultCount(slot) => out.push(Scalar::Local { slot: *slot, kind: Kind::Cards }),
            Number::RemovedCount { slot, .. } => out.push(Scalar::Local { slot: *slot, kind: Kind::Removals }),
            Number::Damage { slot, to, .. } => { out.push(Scalar::Local { slot: *slot, kind: Kind::Damage }); if let Some(to) = to { to.scalars(out); } },
            Number::LifeLost(slot) => out.push(Scalar::Local { slot: *slot, kind: Kind::Number }),
            Number::Const(_) | Number::X | Number::SpellX | Number::EventAmount | Number::ManaColorsSpent => {}
        }
    }
}

/// A number that would be negative is zero (107.1b); one beyond what the engine counts in
/// is the most it counts.
pub(crate) fn nonnegative(n: i64) -> u32 {
    u32::try_from(n.max(0)).unwrap_or(u32::MAX)
}

/// A quantity an effect uses: how many cards, how much damage, how many counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount(Number);

impl Amount {
    /// Arithmetic depending only on constants and the original announced X.
    pub(crate) fn announcement_only(&self) -> bool {
        fn known(n: &Number) -> bool {
            match n {
                Number::Const(_) | Number::X | Number::SpellX => true,
                Number::Product(a, b) | Number::Sum(a, b) | Number::Difference(a, b) => known(a) && known(b),
                Number::Half { of, .. } => known(of),
                _ => false,
            }
        }
        known(&self.0)
    }
    pub(crate) fn reads_pt(&self) -> bool { self.0.reads_pt() }
    #[allow(dead_code)]
    pub(crate) fn reads(&self) -> Reads { self.0.reads() }

    pub(super) fn from_number(n: Number) -> Self { Self(n) }
    pub(crate) fn constant(n: u32) -> Self { Self(Number::Const(n)) }
    pub(crate) fn announced_x() -> Self { Self(Number::X) }
    pub(crate) fn is_x(&self) -> bool { matches!(self.0, Number::X) }
    pub(super) fn as_constant(&self) -> Option<u32> { if let Number::Const(n) = self.0 { Some(n) } else { None } }
    pub(crate) fn times(self, each: Self) -> Self { Self(Number::Product(Box::new(self.0), Box::new(each.0))) }

    pub(crate) fn result_controllers(set: &ir::ObjectSet, support: &Support<'_>, bind: Bind<'_>) -> Res<(super::Slot, Box<Filter>)> {
        let mut original = set.clone();
        original.filter = death_filter(&set.filter).ok_or_else(|| crate::Unsupported("result creation: unsupported completed result".into()))?;
        let Number::DeathCount { slot, filter } = Number::death_count(&original, support, bind)? else { unreachable!() };
        Ok((slot, filter))
    }
    pub(crate) fn death_count_by_actor(slot: super::Slot, filter: Box<Filter>) -> Self { Self(Number::DeathCountByActor { slot, filter }) }

    pub(crate) fn compile(amount: &ir::Amount, support: &Support<'_>, bind: Bind<'_>) -> Res<Amount> {
        Number::compile(amount, support, bind).map(Amount)
    }

    /// A coordinated definition can name the quantity immediately before it.
    /// Bind that typed number rather than reading the resulting characteristic.
    pub(crate) fn compile_recalling(amount: &ir::Amount, previous: Option<&Self>, support: &Support<'_>, bind: Bind<'_>) -> Res<Self> {
        struct Recall<'a, 'b> { previous: Option<&'a Amount>, parent: Bind<'b> }
        impl Binding for Recall<'_, '_> {
            fn scalar(&mut self, r: &UnresolvedRef) -> Res<Scalar> { self.parent.scalar(r) }
            fn subject(&mut self, s: &ir::Selector) -> Res<Subject> { self.parent.subject(s) }
            fn amount_subject(&mut self, s: &ir::Selector) -> Res<Subject> { self.parent.amount_subject(s) }
            fn singular_read(&self, subject: &Subject) -> Res<()> { self.parent.singular_read(subject) }
            fn other_subject(&self) -> Option<Subject> { self.parent.other_subject() }
            fn x(&mut self) -> bool { self.parent.x() }
            fn cast_payment_available(&self) -> bool { self.parent.cast_payment_available() }
            fn number(&mut self) -> Res<Amount> { self.previous.cloned().ok_or_else(|| crate::Unsupported("that number: no preceding numeric producer".into())) }
            fn exiled(&mut self) -> Res<super::Slot> { self.parent.exiled() }
            fn entered(&mut self) -> Res<super::Slot> { self.parent.entered() }
            fn damage(&mut self) -> Res<super::Slot> { self.parent.damage() }
            fn life_lost(&mut self) -> Res<super::Slot> { self.parent.life_lost() }
            fn this_way(&mut self, verb: Option<ir::Verb>, into: Option<Zone>) -> Res<Vec<(super::Slot, Shape, bool)>> { self.parent.this_way(verb, into) }
            fn latest_group(&mut self) -> Res<(super::Slot, Shape)> { self.parent.latest_group() }
        }
        Self::compile(amount, support, &mut Recall { previous, parent: bind })
    }

    /// The number it is, now. Only the final result is clamped to zero (107.1b).
    pub fn evaluate(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> u32 {
        nonnegative(self.0.evaluate(game, ctx, locals))
    }

    /// Checked requests cannot use the legacy clamped numeric projection. Compute
    /// arithmetic and counter inventories before converting the final nonnegative value.
    pub(crate) fn checked(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Res<u32> {
        let n=self.0.checked_evaluate(game,ctx,locals)?;
        u32::try_from(n.max(0)).map_err(|_|crate::Unsupported("quantity exceeds the engine numerical range".into()))
    }
    pub(crate) fn checked_for_actor(&self, game:&Game, ctx:&Ctx, locals:&dyn Locals, player:PlayerId)->Res<u32> {
        self.checked(game,ctx,&Actor{parent:locals,player})
    }
    pub(crate) fn checked_for_source(&self, game:&Game, ctx:&Ctx, locals:&dyn Locals, source:ObjId)->Res<u32> {
        self.checked(game,ctx,&ObjectActor{parent:locals,source})
    }


    pub(crate) fn for_actor(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals, player: PlayerId) -> u32 {
        self.evaluate(game, ctx, &Actor { parent: locals, player })
    }


    pub fn scalars(&self) -> Vec<Scalar> {
        let mut out = Vec::new();
        self.0.scalars(&mut out);
        out
    }
}

mod footprint;

#[cfg(test)]
mod tests;

/// Player-counter kinds whose rule consequences are executable here (107.14, 122.1).
pub(crate) fn player_counter_kind(kind: &ir::PlayerCounter) -> Res<String> {
    match kind {
        ir::PlayerCounter::Energy => Ok("energy".into()),
        ir::PlayerCounter::Named(kind) if kind.0 == "rad" => unsupported("player counter: rad upkeep trigger (122.1i) is not implemented"),
        ir::PlayerCounter::Named(kind) => { crate::layers::counter_ok(&kind.0)?; Ok(kind.0.clone()) },
    }
}

#[cfg(test)]
mod checked_evaluate_tests {
    use super::*;
    #[test]
    fn r107_1b_completed_damage_total_refuses_unrepresentable_followup_quantity() {
        let g=Game::new(Vec::new(),2,20);let ctx=Ctx::new(0,ObjId(0));
        let locals=vec![Some(Value::Damage(vec![
            crate::damage::CompletedDamage { recipient:Target::Player(0),sources:vec![],amount:u32::MAX,excess:0,combat:false },
            crate::damage::CompletedDamage { recipient:Target::Player(1),sources:vec![],amount:1,excess:0,combat:false },
        ]))];
        let amount=Amount(Number::Damage {slot:0,to:None,kind:ir::event::DamageKind::Any,excess:false});
        // Individual stored recipient results fit; their requested total does not.
        assert!(amount.checked(&g,&ctx,&locals).unwrap_err().0.contains("numerical range"));
    }
    #[test]
    fn r107_3_literal_multiple_of_announced_x_uses_checked_damage_quantity() {
        struct AnnouncedX;
        impl Binding for AnnouncedX {
            fn scalar(&mut self, _: &UnresolvedRef) -> Res<Scalar> { unsupported("no subject in quantity fixture") }
            fn x(&mut self) -> bool { true }
        }
        let g=Game::new(Vec::new(),2,20);let mut ctx=Ctx::new(0,ObjId(0));
        let definition=crate::CardDef::new("Quantity","{X}","Instant","You gain 1 life.",None).unwrap();
        let support=Support::definition(&definition,None);
        let amount=Amount::compile(&ir::Amount::Multiple(2,Box::new(ir::Amount::X)),&support,&mut AnnouncedX).unwrap();
        ctx.x=Some(3);assert_eq!(amount.checked(&g,&ctx,&NoLocals).unwrap(),6);
        // 107.1b is not a Magic upper limit: refuse the unrepresentable result;
        // this is the same checked quantity used before Damage commitment.
        ctx.x=Some(u32::MAX);assert!(amount.checked(&g,&ctx,&NoLocals).unwrap_err().0.contains("numerical range"));
    }
    #[test]
    fn r107_1b_checked_counter_expression_rejects_large_result_but_negative_is_zero() {
        let g=Game::new(Vec::new(),2,20);let ctx=Ctx::new(0,ObjId(0));
        let large=Amount(Number::Product(Box::new(Number::Const(u32::MAX)),Box::new(Number::Const(2))));
        assert_eq!(large.checked(&g,&ctx,&NoLocals).unwrap_err().0, "quantity exceeds the engine numerical range");
        let negative=Amount(Number::Difference(Box::new(Number::Const(1)),Box::new(Number::Const(2))));
        assert_eq!(negative.checked(&g,&ctx,&NoLocals).unwrap(),0);
    }
}

#[cfg(test)]
mod quantity_tests;
