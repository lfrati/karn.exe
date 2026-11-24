//! Events: what happened, in order. Every change to the game is logged as one, and each names
//! the action of the lexicon it is, so that what the text instructs ("destroy target creature")
//! and what later text watches for ("whenever a creature is destroyed") meet in one place.

use crate::state::{ObjId, PlayerId, Zone};
use ir::mana::ManaSymbol;
use ir::{Part, Verb};

/// Capability boundary shared by damage instructions and patterns observing that
/// damage (120.3h). Merely selecting, destroying or finding a battle is not damage.
/// Broad damage roles still range over the engine's supported game objects; an explicit
/// battle recipient needs defense-counter handling before its card can be admitted.
pub(crate) fn damage_recipient_ok(set: &ir::ObjectSet) -> crate::Res<()> {
    if set.names_type(ir::CardType::Battle) {
        crate::unsupported("damage to battles: defense-counter results (120.3h) are not implemented")
    } else { Ok(()) }
}

/// A player or an object: what damage is dealt to, what a spell targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Player(PlayerId),
    Object(ObjId),
}

/// The resolving effect responsible for an action, distinct from the player doing it
/// (609.1, 701.9). Capture its controller now; the source may later leave or change control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cause {
    pub source: ObjId,
    pub controller: PlayerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// One completed qualifying spell/ability announcement (700.13), not retargeting.
    CrimeCommitted { player: PlayerId },
    /// The whole Discover process finished, even if exhausted or impossible (701.57b).
    Discovered { player: PlayerId, value: u32 },
    /// One completed collection, including a zero-card collection (701.59a).
    EvidenceCollected { player: PlayerId },
    Plotted { player: PlayerId, card: ObjId },
    /// A card was foretold: exiled face down from its player's hand (702.143a); `card` is
    /// what it is in exile.
    Foretold { player: PlayerId, card: ObjId },
    /// These creatures were tapped to pay the cost of a crew ability of `vehicle`: each crews
    /// it, and it is crewed by each (702.122b-c).
    Crews { vehicle: ObjId, creatures: Vec<ObjId> },
    /// A crew ability of `vehicle` resolved: it "becomes crewed" (702.122e); `by`: the
    /// creatures tapped to pay that ability's cost.
    BecameCrewed { vehicle: ObjId, by: Vec<ObjId> },
    /// These creatures were tapped to pay the cost of a saddle ability of `mount`: each
    /// saddles it (702.171c).
    Saddles { mount: ObjId, creatures: Vec<ObjId> },
    /// Rule 733 reversal of an unpaid announcement; not a game action that triggers.
    /// Legal mana actions remain in force and the event sequence never rewinds.
    AnnouncementReversed { source: ObjId, restored: Option<ObjId>, player: PlayerId },
    /// One instruction produced mana; the source is the spell or the ability's source (106.3).
    ManaAdded { source: ObjId, player: PlayerId, mana: Vec<ManaSymbol> },
    /// A mana ability with a tap-symbol cost resolved and produced mana (106.12a).
    TappedForMana { object: ObjId, player: PlayerId },
    /// A library was randomized, even if it had zero or one cards (701.24e).
    Shuffled { player: PlayerId, by: PlayerId },
    CoinFlipped { source: ObjId, result: crate::start::CoinFlip },
    Countered { object: ObjId },
    /// `ordinal`: the how-manyth time this turn `object` became the target of a spell or
    /// ability `of`'s controller controls.
    BecameTarget { object: Target, of: ObjId, ordinal: u32 },
    Attached { aura: ObjId, to: Target },
    /// The completed draw's ordinal for this player in this turn (121.2). `in_draw_step`: its
    /// ordinal in this draw step, when it is the drawing player's own (504).
    Drew { player: PlayerId, card: ObjId, ordinal: u32, in_draw_step: Option<u32> },
    Milled { player: PlayerId, card: ObjId },
    /// Cards were shown to all players (701.20a); they stay where they are (701.20b).
    Revealed { player: PlayerId, cards: Vec<ObjId> },
    /// A player beheld these permanents they control: chose them for a cost (701.4a). Public as
    /// the cost's other parts are, though nothing happens to them.
    Beheld { player: PlayerId, permanents: Vec<ObjId> },
    /// A player scried or surveilled (701.22d, 701.25d): `verb` says which.
    Arranged { player: PlayerId, verb: Verb },
    /// Completed even if drawing/discarding was impossible (701.50f).
    Connived { object: ObjId, player: PlayerId },
    /// 701.44b: the permanent has explored, as its controller then.
    Explored { object: ObjId, player: PlayerId },
    /// `combat`: combat damage (510), dealt by an attacking or blocking creature in its step.
    DamageDealt { source: ObjId, to: Target, amount: u32, combat: bool },
    /// One qualifying recipient of the simultaneous actual damage occurrence (120.10).
    ExcessDamageDealt { to: Target, amount: u32, combat: bool },
    Destroyed { object: ObjId },
    /// It would have been destroyed and a regeneration shield replaced that (701.19a).
    Regenerated { object: ObjId },
    Exiled { object: ObjId },
    /// Returned or put somewhere by an instruction with no other name.
    Moved { object: ObjId, to: Zone },
    LifeGained { player: PlayerId, amount: u32 },
    LifeLost { player: PlayerId, amount: u32 },
    Tapped { object: ObjId },
    Untapped { object: ObjId },
    /// A permanent phased out (702.26b) or in (702.26c).
    PhasedOut { object: ObjId },
    PhasedIn { object: ObjId },
    /// A double-faced permanent's other face is up (701.27a).
    Transformed { object: ObjId },
    /// A permanent's level became this number (716.2a).
    BecameLevel { object: ObjId, level: u32 },
    /// A permanent became monstrous or renowned (701.37a, 702.112a).
    Designated { object: ObjId, designation: ir::condition::ObjectDesignation },
    /// A player gave a Room permanent the unlocked designation of this half, 0 the left
    /// (709.5c), as it entered or later (709.5h).
    Unlocked { object: ObjId, half: usize, player: crate::PlayerId },
    /// A face-down permanent was turned face up (708.8): the same object, which did not enter.
    TurnedFaceUp { object: ObjId },
    /// A face-up permanent was turned face down (708.2a): the same object, now a 2/2.
    TurnedFaceDown { object: ObjId },
    /// `before`: how many of that kind it had before these were put on it ("if the number of
    /// lore counters on it was less than N and became at least N", 714.2b).
    CountersPut { on: Target, kind: String, count: u32, before: u32, by: crate::PlayerId,
        /// Recipient as the counters were put, for immediate matching and lookbacks (608.2i).
        recipient: Option<Box<crate::layers::Seen>> },
    /// Remaining counters of this kind at the removal event, before later instructions.
    CountersRemoved { from: Target, kind: String, count: u32, remaining: u32 },
    Created { token: ObjId },
    /// A player gets an emblem in the command zone (114.2).
    EmblemCreated { object: ObjId },
    Cast { spell: ObjId, by: PlayerId },
    /// A spell copy put on the stack, without casting it (707.10).
    SpellCopied { original: ObjId, copy: ObjId, by: PlayerId },
    AbilityCopied { original: ObjId, copy: ObjId, by: PlayerId },
    LandPlayed { land: ObjId, by: PlayerId },
    /// A player activated an ability of that object (602.2); `ability`: the ability on the
    /// stack, None for a mana ability, which does not use it (605.3b).
    Activated { source: ObjId, by: PlayerId, ability: Option<ObjId>, keyword: Option<ir::Keyword> },
    /// The zone change under any of the above (400.7): `was` is the object that left, `now`
    /// the new object it became, when one exists. Tokens first arrive in the destination;
    /// their later cessation at state-based actions is not a zone change (111.7).
    ZoneChanged { was: ObjId, now: Option<ObjId>, from: Zone, to: Zone },
    Resolved { spell: ObjId },
    /// It left the stack without resolving: every target was illegal (608.2b), or the
    /// intervening "if" of a triggered ability was no longer true (608.2a).
    Fizzled { spell: ObjId },
    /// `madness`: the madness ability whose replacement put the card into exile instead of its
    /// owner's graveyard (702.35a); it makes that ability's trigger trigger.
    Discarded { player: PlayerId, card: ObjId, cause: Option<Cause>, madness: Option<crate::binding::AbilityKey> },
    /// A player sacrificed a permanent they controlled (701.21a).
    Sacrificed { player: PlayerId, object: ObjId },
    /// A creature was declared as an attacker, attacking that player or planeswalker (508.1k).
    Attacked { attacker: ObjId, target: Target },
    /// A player declared one or more attackers (508.3d).
    AttackersDeclared { player: PlayerId },
    /// A creature was declared as a blocker for that attacking creature (509.1g).
    Blocked { blocker: ObjId, attacker: ObjId },
    /// A creature became a blocker, once however many creatures it blocks (509.3a).
    BecameBlocking { blocker: ObjId },
    /// An attacking creature became blocked (509.1h): once, however many creatures block it.
    BecameBlocked { attacker: ObjId },
    TurnBegan { player: PlayerId },
    /// A step or phase began (500.6): abilities that trigger "at the beginning of" it trigger.
    Began { part: Part },
    /// A step or phase of this player's was skipped by that skip effect (614.10): it never
    /// began, and nothing triggered at its beginning.
    Skipped { player: PlayerId, what: crate::turn::Occurrence, skip: crate::turn::SkipId },
    MonarchChanged { player: Option<PlayerId>, previous: Option<PlayerId> },
    /// A player's speed became this (702.179a, 702.179d).
    SpeedChanged { player: PlayerId, speed: u8, previous: Option<u8> },
    /// A player conceded the game (104.3a); [`Event::Lost`] follows.
    Conceded { player: PlayerId },
    Lost { player: PlayerId },
}

impl Event {
    /// The action of the lexicon this event is, when it is one.
    pub fn verb(&self) -> Option<Verb> {
        Some(match self {
            Event::TappedForMana { .. } => Verb::TapForMana,
            Event::Shuffled { .. } => Verb::Shuffle,
            Event::CoinFlipped { result, .. } => match result.won() {
                Some(true) => Verb::WinFlip, Some(false) => Verb::LoseFlip, None => return None,
            },
            Event::Countered { .. } => Verb::Counter,
            Event::BecameTarget { .. } => Verb::BeTargeted,
            Event::Attached { .. } => Verb::Attach,
            Event::Drew { .. } => Verb::Draw,
            Event::Plotted { .. } => Verb::Plot,
            Event::Foretold { .. } => Verb::Foretell,
            Event::Crews { .. } | Event::BecameCrewed { .. } => Verb::Crew,
            Event::Saddles { .. } => Verb::Saddle,
            Event::MonarchChanged { player: Some(_), .. } => Verb::BecomeMonarch,
            Event::MonarchChanged { player: None, .. } => return None,
            Event::Milled { .. } => Verb::Mill,
            Event::Revealed { .. } => Verb::Reveal,
            Event::Beheld { .. } => Verb::Behold,
            Event::Arranged { verb, .. } => *verb,
            Event::Connived {..} => Verb::Connive,
            Event::Explored {..} => Verb::Explore,
            Event::DamageDealt { .. } | Event::ExcessDamageDealt { .. } => Verb::DealDamage,
            Event::Destroyed { .. } => Verb::Destroy,
            Event::Regenerated { .. } => Verb::Regenerate,
            Event::Exiled { .. } => Verb::Exile,
            Event::Moved { .. } => Verb::Move,
            Event::LifeGained { .. } => Verb::GainLife,
            Event::LifeLost { .. } => Verb::LoseLife,
            Event::Tapped { .. } => Verb::Tap,
            Event::Untapped { .. } => Verb::Untap,
            Event::Transformed { .. } => Verb::Transform,
            Event::BecameLevel { .. } => Verb::BecomeLevel,
            Event::Designated { designation: ir::condition::ObjectDesignation::Monstrous, .. } => Verb::BecomeMonstrous,
            Event::Unlocked { .. } => Verb::UnlockDoor,
            Event::TurnedFaceUp { .. } => Verb::TurnFace,
            Event::CountersPut { .. } => Verb::PutCounters,
            Event::CountersRemoved { .. } => Verb::RemoveCounters,
            Event::Created { .. } => Verb::Create,
            Event::Cast { .. } => Verb::Cast,
            Event::SpellCopied { .. } | Event::AbilityCopied { .. } => Verb::Copy,
            Event::Discovered { .. } => Verb::Discover,
            Event::EvidenceCollected { .. } => Verb::CollectEvidence,
            Event::CrimeCommitted { .. } => Verb::CommitCrime,
            Event::LandPlayed { .. } => Verb::Play,
            Event::Activated { .. } => Verb::Activate,
            Event::Discarded { .. } => Verb::Discard,
            Event::Sacrificed { .. } => Verb::Sacrifice,
            Event::Attacked { .. } | Event::AttackersDeclared { .. } => Verb::Attack,
            Event::Blocked { .. } | Event::BecameBlocking { .. } | Event::BecameBlocked { .. } => Verb::Block,
            Event::EmblemCreated { .. }
            | Event::ManaAdded { .. }
            | Event::ZoneChanged { .. }
            | Event::Resolved { .. }
            | Event::AnnouncementReversed { .. }
            | Event::Fizzled { .. }
            | Event::TurnBegan { .. }
            | Event::Began { .. }
            | Event::Skipped { .. }
            // nothing here asks about turning face down
            | Event::TurnedFaceDown { .. }
            // what triggers on phasing is not lowered (603.10b)
            | Event::PhasedOut { .. }
            | Event::PhasedIn { .. }
            | Event::SpeedChanged { .. }
            | Event::Designated { .. }
            | Event::Conceded { .. }
            | Event::Lost { .. } => return None,
        })
    }
}

/// A monotonic position between committed events. It is never a vector offset, and remains
/// meaningful when an earlier delivery batch is retired.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub usize);

/// Events still needed by rules procedures or the current host response. Diagnostic replay
/// belongs to the host; it is not copied into every speculative game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Log {
    start: Seq,
    events: Vec<Event>,
}
impl Log {
    pub fn start(&self) -> Seq { self.start }
    pub fn end(&self) -> Seq { Seq(self.start.0 + self.events.len()) }
    pub fn push(&mut self, event: Event) { self.events.push(event); }
    pub fn capacity(&self) -> usize { self.events.capacity() }
    pub fn since(&self, from: Seq) -> &[Event] { self.range(from..self.end()) }
    pub fn range(&self, range: std::ops::Range<Seq>) -> &[Event] {
        assert!(self.start <= range.start && range.start <= range.end && range.end <= self.end(), "event range must still be retained");
        &self.events[range.start.0 - self.start.0..range.end.0 - self.start.0]
    }
    pub fn sequenced(&self) -> impl DoubleEndedIterator<Item = (Seq, &Event)> + ExactSizeIterator {
        self.events.iter().enumerate().map(|(i, e)| (Seq(self.start.0 + i), e))
    }
    /// Low-level fixture operation. Running games use Game::retire_events, which respects
    /// unconsumed triggers and retained procedure ranges.
    pub fn clear(&mut self) { self.start = self.end(); self.events.clear(); }
    pub(crate) fn retire_before(&mut self, through: Seq) -> usize {
        assert!(self.start <= through && through <= self.end());
        let count = through.0 - self.start.0;
        self.events.drain(..count);
        self.start = through;
        // A past large simultaneous action should not fix every later clone's allocation.
        if self.events.capacity() > 256.max(self.events.len() * 4) { self.events.shrink_to(self.events.len().max(64)); }
        count
    }
}
impl std::ops::Deref for Log {
    type Target = [Event];
    fn deref(&self) -> &Self::Target { &self.events }
}
impl std::ops::DerefMut for Log {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.events }
}

impl crate::retention::Trace for Event {
    fn trace(&self, visit: &mut dyn FnMut(crate::ObjId)) {
        match self {
            Self::BecameTarget { object, of, .. } => { object.trace(visit); of.trace(visit); },
            Self::ManaAdded { source, .. } => source.trace(visit),
            Self::AnnouncementReversed { source, restored, .. } => { source.trace(visit); restored.trace(visit); },
            Self::TappedForMana { object, .. } | Self::Countered { object }
            | Self::Destroyed { object } | Self::Regenerated { object }
            | Self::Exiled { object } | Self::Moved { object, .. }
            | Self::Tapped { object } | Self::Untapped { object } | Self::Transformed { object } | Self::BecameLevel { object, .. } | Self::Designated { object, .. } | Self::Unlocked { object, .. } | Self::TurnedFaceUp { object } | Self::TurnedFaceDown { object }
            | Self::PhasedOut { object } | Self::PhasedIn { object }
            | Self::Sacrificed { object, .. } | Self::Connived { object, .. } | Self::Explored { object, .. } => object.trace(visit),
            Self::Attached { aura, to } => { aura.trace(visit); to.trace(visit); },
            Self::Plotted { card, .. } | Self::Foretold { card, .. } | Self::Drew { card, .. } | Self::Milled { card, .. } => card.trace(visit),
            Self::Crews { vehicle, creatures: by } | Self::BecameCrewed { vehicle, by } | Self::Saddles { mount: vehicle, creatures: by } => { vehicle.trace(visit); by.trace(visit); },
            Self::Discarded { card, cause, .. } => { card.trace(visit); if let Some(cause) = cause { cause.source.trace(visit); } },
            Self::Revealed { cards, .. } => cards.trace(visit),
            Self::Beheld { permanents, .. } => permanents.trace(visit),
            Self::ExcessDamageDealt { to, .. } => to.trace(visit),
            Self::DamageDealt { source, to, .. } => { source.trace(visit); to.trace(visit); },
            Self::CountersPut { on, .. } => on.trace(visit),
            Self::CountersRemoved { from, .. } => from.trace(visit),
            Self::Created { token } => token.trace(visit),
            Self::SpellCopied { original, copy, .. } | Self::AbilityCopied { original, copy, .. } => { original.trace(visit); copy.trace(visit); },
            Self::EmblemCreated { object } => object.trace(visit),
            Self::CoinFlipped { source, .. } => source.trace(visit),
            Self::Cast { spell, .. } | Self::Resolved { spell } | Self::Fizzled { spell } => spell.trace(visit),
            Self::LandPlayed { land, .. } => land.trace(visit),
            Self::Activated { source, ability, .. } => { source.trace(visit); ability.trace(visit); },
            Self::ZoneChanged { was, now, .. } => { was.trace(visit); now.trace(visit); },
            Self::Attacked { attacker, target } => { attacker.trace(visit); target.trace(visit); },
            Self::Blocked { blocker, attacker } => { blocker.trace(visit); attacker.trace(visit); },
            Self::BecameBlocked { attacker } => attacker.trace(visit),
            Self::BecameBlocking { blocker } => blocker.trace(visit),
            Self::CrimeCommitted { .. } | Self::EvidenceCollected { .. } | Self::Discovered { .. } | Self::Shuffled { .. } | Self::Arranged { .. } | Self::LifeGained { .. } | Self::LifeLost { .. }
            | Self::MonarchChanged { .. } | Self::SpeedChanged { .. } | Self::AttackersDeclared { .. } | Self::TurnBegan { .. } | Self::Began { .. } | Self::Skipped { .. } | Self::Conceded { .. } | Self::Lost { .. } => {},
        }
    }
}

impl From<ObjId> for Target { fn from(object: ObjId) -> Self { Self::Object(object) } }
