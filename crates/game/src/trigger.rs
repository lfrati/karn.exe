//! Triggered abilities (CR 603): "when it happens".
//!
//! A trigger's event pattern and the engine's events name the same actions of the lexicon, so
//! matching is one comparison: the same action, and each role filled by something the pattern
//! describes. An ability that triggers waits ([`crate::state::Triggered`]) and is put on the
//! stack the next time a player would receive priority (603.3), its controller choosing its
//! targets then.

use crate::ability::View;
use crate::binding::AbilityKey;
use crate::card::CardDef;
use crate::event::{Event, Target};
use crate::program::condition;
use crate::program::expr::{self, NoLocals, linked};
use crate::program::registration::Registered;
use crate::select::{Ctx, Support, name};
use crate::state::{Bound, Game, ObjId, Triggered, Zone};
use crate::{Res, unsupported};
use ir::event::{Arg, DamageKind, EventAction, EventPattern, Step, StepOwner, Trigger};
use ir::effect::Modifier;
use ir::{Ability, Amount, CardType, Condition, Count, Lead, Part, Quant, Ref, Role, Selector, Sentence, SubjectIs, Verb};


/// One thing that happened, or that would happen, in the lexicon's terms: the action, who
/// did it, and what filled its roles. `was`: an object that left the battlefield in it, to be
/// described as it last was. What a trigger watches for and what a prohibition forbids
/// ([`crate::restrict`]) are asked of the same description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fact {
    verb: Verb,
    /// Captured activation family and mana classification; source changes cannot rewrite it.
    activation: Option<(Option<ir::Keyword>, bool)>,
    doer: Option<Target>,
    roles: Vec<(Role, Target)>,
    was: Option<ObjId>,
    /// An involved permanent may already have left, without this being a zone-change
    /// event that lets departed watchers trigger (603.10, 113.7a).
    last_known: Option<ObjId>,
    /// Of damage: combat damage or not (510, 120.2a).
    combat: Option<bool>,
    /// Derived recipient excess occurrence, distinct from every ordinary damage hit.
    excess: bool,
    amount: Option<u32>,
    zones: Option<(Zone, Zone)>,
    ordinal: Option<u32>,
    /// Of a draw in the drawing player's own draw step: its ordinal in that step.
    in_draw_step: Option<u32>,
    counters: Option<(String, u32)>,
    /// Of counters put: how many of that kind it had before (714.2b).
    had: Option<u32>,
    /// Of a removal: counters of its named kind left immediately afterward.
    remaining: Option<u32>,
    /// Of a permanent's level becoming a number: that number (716.2a).
    level: Option<u32>,
    /// Of unlocking: the Room and which of its halves (709.5h).
    door: Option<(ObjId, usize)>,
    /// Of an attack: the creatures declared as attackers with it (508.1).
    attackers: Option<Vec<ObjId>>,
    /// Where in the log its event is, as it is noted or checked: which of two alike it is.
    at: Option<crate::event::Seq>,
    /// Of an entry kept to be asked about later: whether the permanent entered face down,
    /// which it is still said to have though it turns since (608.2i).
    entered_face_down: Option<bool>,
    /// Of an entry kept to be asked about later: what the permanent was as it entered, which
    /// it is still said to have been though it changes since (608.2i).
    entered_as: Option<Box<crate::layers::Seen>>,
    /// Counter recipient at placement time, not its current type/controller (608.2i).
    counter_recipient: Option<Box<crate::layers::Seen>>,
}

impl Fact {
    pub(crate) fn verb(&self) -> Verb { self.verb }
    pub(crate) fn doer(&self) -> Option<Target> { self.doer }
    pub(crate) fn new(verb: Verb, doer: Option<Target>, roles: Vec<(Role, Target)>) -> Fact {
        Fact { verb, activation: None, doer, roles, was: None, last_known: None, combat: None, excess: false, amount: None, zones: None, ordinal: None, in_draw_step: None, counters: None, had: None, remaining: None, level: None, door: None, attackers: None, at: None, entered_face_down: None, entered_as: None, counter_recipient: None }
    }

    /// Counter-placement proposal, distinct from its eventual committed event.
    pub(crate) fn counter_placement(on: Target, kind: &str, by: crate::PlayerId) -> Self {
        Self { counters: Some((kind.to_owned(), 1)), ..Self::new(Verb::PutCounters,
            Some(Target::Player(by)), vec![(Role::On,on)]) }
    }
    pub(crate) fn counter_kind(&self) -> Option<&str> { self.counters.as_ref().map(|(kind,_)|kind.as_str()) }

    pub(crate) fn activation(player: crate::PlayerId, keyword: Option<ir::Keyword>, mana: bool, ability: Option<ObjId>) -> Self {
        Fact { activation: Some((keyword, mana)), ..Self::new(Verb::Activate, Some(Target::Player(player)),
            ability.into_iter().map(|a| (Role::What, Target::Object(a))).collect()) }
    }

    pub(crate) fn at(self, at: crate::event::Seq) -> Fact { Fact { at: Some(at), ..self } }

    pub(crate) fn role(&self, r: Role) -> Option<Target> {
        self.roles.iter().find(|(x, _)| *x == r).map(|(_, t)| *t)
    }

    fn context<'a>(&'a self, ctx: Ctx<'a>) -> Ctx<'a> {
        // what was turned face up was face down as it happened (708.8); what entered is face
        // down or not as it entered, though it turns since (608.2i)
        let face_then = match (self.verb, self.role(Role::What)) {
            (Verb::TurnFace, Some(Target::Object(o))) => Some((o, true)),
            (Verb::Enter, Some(Target::Object(o))) => self.entered_face_down.map(|down| (o, down)).or(ctx.face_then),
            _ => ctx.face_then,
        };
        let then = match (self.role(Role::What), self.entered_as.as_deref()) {
            (Some(Target::Object(o)), Some(seen)) => Some((o, seen)),
            _ => match (self.role(Role::On), self.counter_recipient.as_deref()) {
                (Some(Target::Object(o)), Some(seen)) => Some((o, seen)),
                _ => ctx.then,
            },
        };
        Ctx { was: self.was.or(self.last_known), face_then, then, ..ctx }
    }

    /// Kept to be asked about later ([`crate::history`]): what it happened to may be gone by
    /// then, and is described as it last was (608.2h).
    /// Is this the same event, whether or not it was kept to be asked about later?
    pub(crate) fn same_event(&self, other: &Fact) -> bool {
        self.verb == other.verb && self.doer == other.doer && self.roles == other.roles
            && (self.at.is_none() || other.at.is_none() || self.at == other.at)
    }

    /// Of an entry, kept as the permanent entered: face down or not (608.2i).
    pub(crate) fn as_entered(mut self, game: &Game) -> Fact {
        if let (Verb::Enter, Some(Target::Object(o))) = (self.verb, self.role(Role::What)) && game.obj(o).zone == Zone::Battlefield {
            self.entered_face_down = Some(game.face_down(o));
            self.entered_as = Some(Box::new(game.seen(o, None)));
        }
        self
    }

    pub(crate) fn remembered(mut self) -> Fact {
        if self.was.is_none() && self.last_known.is_none() && let Some(Target::Object(what)) = self.role(Role::What).or(self.doer) {
            self.last_known = Some(what);
        }
        self
    }
}

impl crate::retention::Trace for Fact {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { activation: _, doer, roles, was, last_known, verb: _, combat: _, excess: _, amount: _, zones: _, ordinal: _, in_draw_step: _, counters: _, had: _, remaining: _, level: _, door, attackers, at: _, entered_face_down: _, entered_as: _, counter_recipient: _ } = self;
        doer.trace(visit);
        attackers.trace(visit);
        for (_, t) in roles { t.trace(visit); }
        was.trace(visit);
        if let Some((room, _)) = door { room.trace(visit); }
        last_known.trace(visit);
    }
}

/// What an event of the log is, as the lexicon names it. A zone change is up to three things:
/// entering, dying, leaving the battlefield.
pub(crate) fn facts(game: &Game, e: &Event) -> Vec<Fact> {
    use Target::{Object as O, Player as P};
    let what = |t: Target| vec![(Role::What, t)];
    match e {
        Event::ManaAdded { .. } => Vec::new(), // mana-production triggers need 605.1b
        // nothing asks about a speed changing
        Event::SpeedChanged { .. } => Vec::new(),
        Event::TappedForMana { object, player } => vec![Fact {
            last_known: (game.obj(*object).zone != Zone::Battlefield).then_some(*object),
            ..Fact::new(Verb::TapForMana, Some(P(*player)), what(O(*object)))
        }],
        Event::Shuffled { player, by } => vec![Fact::new(Verb::Shuffle, Some(P(*by)), vec![(Role::Of, P(*player))])],
        Event::CoinFlipped { result, .. } => match result.won() {
            Some(won) => vec![Fact::new(if won {Verb::WinFlip} else {Verb::LoseFlip}, Some(P(result.player)), vec![])],
            None => Vec::new(),
        },
        Event::Attached { .. } => Vec::new(), // attachment triggers are not accepted yet
        Event::Drew { player, card, ordinal, in_draw_step } => vec![Fact { ordinal: Some(*ordinal), in_draw_step: *in_draw_step, ..Fact::new(Verb::Draw, Some(P(*player)), what(O(*card))) }],
        Event::MonarchChanged { player, .. } => player.iter().map(|p| Fact::new(Verb::BecomeMonarch, Some(P(*p)), vec![])).collect(),
        Event::Plotted { player, card } => vec![Fact::new(Verb::Plot, Some(P(*player)), what(O(*card)))],
        Event::Foretold { player, card } => vec![Fact::new(Verb::Foretell, Some(P(*player)), what(O(*card)))],
        // each creature crews the Vehicle (702.122b); the Vehicle becomes crewed once, as its
        // crew ability resolves, said of it alone (702.122e)
        Event::Crews { vehicle, creatures } => creatures.iter().map(|c| Fact::new(Verb::Crew, Some(O(*c)), vec![(Role::What, O(*vehicle)), (Role::By, O(*c))])).collect(),
        Event::BecameCrewed { vehicle, .. } => vec![Fact::new(Verb::Crew, None, what(O(*vehicle)))],
        Event::Saddles { mount, creatures } => creatures.iter().map(|c| Fact::new(Verb::Saddle, Some(O(*c)), vec![(Role::What, O(*mount)), (Role::By, O(*c))])).collect(),
        Event::Milled { player, card } => vec![Fact::new(Verb::Mill, Some(P(*player)), what(O(*card)))],
        Event::Revealed { player, cards } => cards.iter().map(|card| Fact::new(Verb::Reveal, Some(P(*player)), what(O(*card)))).collect(),
        Event::Beheld { player, permanents } => permanents.iter().map(|o| Fact::new(Verb::Behold, Some(P(*player)), what(O(*o)))).collect(),
        // "whenever you scry", "whenever you surveil" (701.22d, 701.25d)
        Event::Connived {object,player} => vec![Fact {last_known:(game.obj(*object).zone!=Zone::Battlefield).then_some(*object),..Fact::new(Verb::Connive,Some(O(*object)),vec![(Role::By,P(*player))])}],
        Event::CrimeCommitted { player } => vec![Fact::new(Verb::CommitCrime, Some(P(*player)), vec![])],
        Event::EvidenceCollected { player } => vec![Fact::new(Verb::CollectEvidence, Some(P(*player)), vec![])],
        Event::Discovered { player, value } => vec![Fact { amount: Some(*value), ..Fact::new(Verb::Discover, Some(P(*player)), vec![]) }],
        Event::Explored {object,player} => vec![Fact {last_known:(game.obj(*object).zone!=Zone::Battlefield).then_some(*object),..Fact::new(Verb::Explore,Some(O(*object)),vec![(Role::By,P(*player))])}],
        Event::Arranged { player, verb } => vec![Fact::new(*verb, Some(P(*player)), vec![])],
        // A source that is no more deals damage as it last existed (113.7a, 608.2h): the spell
        // that made a delayed ability, which has left the stack when that ability resolves
        // (603.7d), or a permanent that has left the battlefield.
        Event::ExcessDamageDealt { to, combat, .. } => vec![Fact {
            combat: Some(*combat), excess: true,
            // Aggregate trigger quantities remain refused until their checked joining owner.
            ..Fact::new(Verb::DealDamage, None, vec![(Role::To, *to)])
        }],
        Event::DamageDealt { source, to, combat, amount } => vec![Fact {
            combat: Some(*combat), amount: Some(*amount),
            last_known: (game.obj(*source).zone == Zone::Gone).then_some(*source),
            ..Fact::new(Verb::DealDamage, Some(O(*source)), vec![(Role::To, *to)])
        }],
        // "whenever ~ attacks", "… attacks you" (508.3a); "whenever you attack" (508.3d)
        Event::Attacked { attacker, target } => vec![Fact { attackers: Some(game.combat.declared_attackers.clone()),
            ..Fact::new(Verb::Attack, Some(O(*attacker)), vec![(Role::To, *target)]) }],
        Event::AttackersDeclared { player } => vec![Fact { attackers: Some(game.combat.declared_attackers.clone()), ..Fact::new(Verb::Attack, Some(P(*player)), vec![]) }],
        // "whenever ~ blocks [a creature]", "… becomes blocked by a creature" (509.3a–b, d)
        Event::BecameBlocking { blocker } => vec![Fact::new(Verb::Block, Some(O(*blocker)), vec![])],
        Event::Blocked { blocker, attacker } => {
            vec![Fact::new(Verb::Block, Some(O(*blocker)), vec![(Role::What, O(*attacker)), (Role::By, O(*blocker))])]
        }
        // "whenever ~ becomes blocked": once, however many creatures block it (509.3c)
        Event::BecameBlocked { attacker } => vec![Fact::new(Verb::Block, None, what(O(*attacker)))],
        // what was destroyed or exiled has left, and is seen as it last existed (603.10a)
        Event::Destroyed { object } => vec![Fact { last_known: (game.obj(*object).zone == Zone::Gone).then_some(*object), ..Fact::new(Verb::Destroy, None, what(O(*object))) }],
        Event::BecameTarget { object, of, ordinal } => vec![Fact { ordinal: Some(*ordinal), ..Fact::new(Verb::BeTargeted, None, vec![(Role::What, *object), (Role::Of, O(*of))]) }],
        Event::Countered { object } => vec![Fact::new(Verb::Counter, None, what(O(*object)))],
        Event::Exiled { object } => vec![Fact { last_known: (game.obj(*object).zone == Zone::Gone).then_some(*object), ..Fact::new(Verb::Exile, None, what(O(*object))) }],
        Event::LifeGained { player, amount } => vec![Fact { amount: Some(*amount), ..Fact::new(Verb::GainLife, Some(P(*player)), vec![]) }],
        Event::LifeLost { player, amount } => vec![Fact { amount: Some(*amount), ..Fact::new(Verb::LoseLife, Some(P(*player)), vec![]) }],
        Event::Tapped { object } => vec![Fact::new(Verb::Tap, None, what(O(*object)))],
        Event::Transformed { object } => vec![Fact::new(Verb::Transform, None, what(O(*object)))],
        // "when this creature becomes monstrous" (701.37c)
        Event::Designated { object, designation: ir::condition::ObjectDesignation::Monstrous } => vec![Fact::new(Verb::BecomeMonstrous, Some(O(*object)), vec![])],
        Event::BecameLevel { object, level } => vec![Fact { level: Some(*level), ..Fact::new(Verb::BecomeLevel, None, what(O(*object))) }],
        Event::Unlocked { object, half, player } => vec![Fact { door: Some((*object, *half)), ..Fact::new(Verb::UnlockDoor, Some(P(*player)), vec![]) }],
        Event::TurnedFaceUp { object } => vec![Fact::new(Verb::TurnFace, None, what(O(*object)))],
        // no trigger condition the engine reads is about turning face down
        Event::TurnedFaceDown { .. } | Event::Designated { .. } => Vec::new(),
        Event::Untapped { object } => vec![Fact::new(Verb::Untap, None, what(O(*object)))],
        Event::CountersPut { on, kind, count, before, by, recipient } => vec![Fact { counters: Some((kind.clone(), *count)), had: Some(*before),
            counter_recipient: recipient.clone(),
            last_known: match on { O(o) => Some(*o), P(_) => None },
            ..Fact::new(Verb::PutCounters, Some(P(*by)), vec![(Role::On, *on)]) }],
        Event::CountersRemoved { from, kind, count, remaining } => vec![Fact {
            counters: Some((kind.clone(), *count)), remaining: Some(*remaining),
            ..Fact::new(Verb::RemoveCounters, None, vec![(Role::From, *from)]) }],
        // to create a token is to put it onto the battlefield (701.7a): it enters
        Event::Created { token } => {
            let controller = P(game.controller(*token));
            vec![
                Fact::new(Verb::Create, Some(controller), what(O(*token))),
                Fact::new(Verb::Enter, None, vec![(Role::What, O(*token)), (Role::Under, controller)]),
            ]
        }
        Event::SpellCopied { copy, by, .. } | Event::AbilityCopied { copy, by, .. } => vec![Fact::new(Verb::Copy, Some(P(*by)), what(O(*copy)))],
        // to play a card is to play it as a land or cast it as a spell (701.18b): a cast is
        // also a play, so "when you play a card" sees spells; a land-only phrase still asks
        // for a land, and a cast copy is not a card (707.12)
        Event::Cast { spell, by } => vec![
            Fact::new(Verb::Cast, Some(P(*by)), what(O(*spell))),
            Fact::new(Verb::Play, Some(P(*by)), what(O(*spell))),
        ],
        Event::LandPlayed { land, by } => vec![Fact::new(Verb::Play, Some(P(*by)), what(O(*land)))],
        Event::Discarded { player, card, .. } => vec![Fact::new(Verb::Discard, Some(P(*player)), what(O(*card)))],
        // what was sacrificed is described as it last was on the battlefield (603.10a)
        Event::Sacrificed { player, object } => {
            vec![Fact { was: Some(*object), ..Fact::new(Verb::Sacrifice, Some(P(*player)), what(O(*object))) }]
        }
        // a permanent enters, leaves or dies under a player's control: the one who controls it,
        // or last did (603.10a)
        Event::ZoneChanged { was, now, from, to } => {
            let under = |o: ObjId| vec![(Role::What, O(o)), (Role::Under, P(game.controller(o)))];
            let mut v = Vec::new();
            if let (Zone::Battlefield, Some(now)) = (to, now) {
                v.push(Fact::new(Verb::Enter, None, under(*now)));
            }
            // 603.6c: "from anywhere" uses the arriving card and the state after the move,
            // unlike a battlefield-departure trigger's look-back view (603.10a).
            if *to == Zone::Graveyard && let Some(now) = now {
                v.push(Fact { zones: Some((*from, *to)),
                    ..Fact::new(Verb::Move, None, vec![(Role::What, O(*now)), (Role::To, P(game.obj(*now).owner))]) });
            }
            if *from == Zone::Graveyard {
                v.push(Fact { zones: Some((*from, *to)), last_known: Some(*was),
                    ..Fact::new(Verb::Move, None, vec![(Role::What, O(*was)), (Role::From, P(game.obj(*was).owner))]) });
            }
            if *from == Zone::Battlefield {
                let left = |verb| Fact { was: Some(*was), ..Fact::new(verb, None, under(*was)) };
                v.push(left(Verb::LeaveBattlefield));
                if *to == Zone::Graveyard {
                    v.push(Fact { zones: Some((*from, *to)), was: Some(*was),
                        ..Fact::new(Verb::Move, None, vec![(Role::What, O(*was)), (Role::To, P(game.obj(*was).owner))]) });
                }
                if *to == Zone::Graveyard && game.is_a(*was, CardType::Creature) {
                    v.push(left(Verb::Die));
                }
                if *to == Zone::Exile {
                    v.push(Fact { zones: Some((*from, *to)), was: Some(*was), ..Fact::new(Verb::Move, None, vec![(Role::What, O(*was))]) });
                }
            }
            v
        }
        Event::Activated { source, by, ability, keyword } => vec![Fact::activation(*by, *keyword, ability.is_none(),
            Some(ability.unwrap_or(*source)))],
        // "whenever a creature regenerates" is not watched for yet.
        Event::Regenerated { .. }
        | Event::Moved { .. }
        | Event::Resolved { .. }
        | Event::AnnouncementReversed { .. }
        | Event::Fizzled { .. }
        | Event::TurnBegan { .. }
        | Event::Began { .. }
        | Event::Skipped { .. }
        | Event::Conceded { .. } | Event::Lost { .. } | Event::EmblemCreated { .. } => Vec::new(),
    }
}

/// What every event of an action says, whatever pattern watches for it: who did it (None) or
/// what filled a role, and whether that is a player. An effect can refer to these (608.2k)
/// because they are always there; [`facts`] is where they are filled.
pub(crate) fn always_named(verb: Verb) -> &'static [(Option<Role>, bool)] {
    const DOER: Option<Role> = None;
    const WHAT: Option<Role> = Some(Role::What);
    match verb {
        Verb::Draw | Verb::Mill | Verb::Discard | Verb::Sacrifice | Verb::Cast | Verb::Copy | Verb::Play | Verb::Create | Verb::TapForMana => &[(DOER, true), (WHAT, false)],
        Verb::Enter | Verb::Die | Verb::LeaveBattlefield => &[(WHAT, false), (Some(Role::Under), true)],
        Verb::Destroy | Verb::Exile | Verb::Tap | Verb::Untap | Verb::Transform => &[(WHAT, false)],
        Verb::GainLife | Verb::LoseLife | Verb::Scry | Verb::Surveil | Verb::Shuffle | Verb::Discover | Verb::CollectEvidence | Verb::CommitCrime => &[(DOER, true)],
        Verb::DealDamage => &[(DOER, false)],
        Verb::BeTargeted => &[(Some(Role::Of), false)],
        _ => &[],
    }
}

/// The events the engine produces: what a trigger can watch for so far.
const WATCHED: &[Verb] = &[
    Verb::BeTargeted,
    Verb::Shuffle,
    Verb::Draw,
    Verb::Plot,
    Verb::Foretell,
    Verb::Crew,
    Verb::Saddle,
    Verb::BecomeMonarch,
    Verb::UnlockDoor,
    Verb::Mill,
    Verb::Scry,
    Verb::Surveil,
    Verb::Connive,
    Verb::Explore,
    Verb::Discover,
    Verb::CollectEvidence,
    Verb::CommitCrime,
    Verb::DealDamage,
    Verb::Destroy,
    Verb::Exile,
    Verb::GainLife,
    Verb::LoseLife,
    Verb::Tap,
    Verb::TapForMana,
    Verb::Untap,
    Verb::Transform,
    Verb::BecomeLevel,
    Verb::BecomeMonstrous,
    // only ever turned face up: no event says a permanent was turned face down
    Verb::TurnFace,
    Verb::PutCounters,
    Verb::RemoveCounters,
    Verb::Create,
    Verb::Cast,
    Verb::Activate,
    Verb::Copy,
    Verb::Play,
    Verb::Discard,
    Verb::Sacrifice,
    Verb::Attack,
    Verb::Block,
    Verb::Enter,
    Verb::Die,
    Verb::LeaveBattlefield,
];

/// A pattern's noun phrase, lowered: what one object or player of an event is tested
/// against. It names nothing: who "you" is and what it is attached to are asked of the
/// permanent whose ability it is, as the event is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Noun {
    /// "this creature"
    This,
    /// "you"
    You,
    /// "enchanted creature", "equipped creature"
    Attached,
    /// "a creature", "each player": whichever one it is
    Described(expr::Set),
    /// An event names a card independently of its current zone (108.2).
    Card(expr::Cards),
    Activation(crate::activate::ActivationFilter),
}

impl Noun {
    /// The one place a pattern's noun phrase is refused.
    pub(crate) fn compile(s: &Selector, support: &Support<'_>) -> Res<Noun> {
        use ir::selector::Attachment;
        if s.chooser.is_some() || s.random.is_some() {
            return unsupported("trigger: a chooser or a random pick");
        }
        if s.set.domain == ir::Domain::Ability && matches!(s.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))) | Quant::Each | Quant::All) {
            return crate::activate::ActivationFilter::compile(&s.set).map(Noun::Activation);
        }
        // A definite life-extremum noun describes the event's own player; it
        // does not choose an antecedent or arbitrarily select one tied player.
        let mut bare = s.set.clone(); bare.filter = ir::Filter::any_object();
        if s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::The { plural: false }))
            && bare == ir::ObjectSet::of(ir::Domain::Player)
            && matches!(&s.set.filter, ir::Filter::All(parts) if matches!(parts.as_slice(),
                [ir::Filter::Atom(ir::Atom::MostOf { measure, .. })] if matches!(measure.as_ref(), ir::condition::Measure::Life))) {
            return expr::Set::compile(&s.set, support, &mut linked(support)).map(Noun::Described);
        }
        match &s.quant {
            Quant::Definite(Ref::This) => Ok(Noun::This),
            Quant::Definite(Ref::You) => Ok(Noun::You),
            Quant::Definite(Ref::AttachedTo(Attachment::Enchanted | Attachment::Equipped)) => Ok(Noun::Attached),
            Quant::Indefinite(Count::Exactly(Amount::Const(1))) | Quant::Each | Quant::All => {
                if s.set.domain == ir::Domain::Card && s.set.zone.is_none() {
                    return expr::Cards::compile(&s.set, support, &mut linked(support)).map(Noun::Card);
                }
                expr::Set::compile(&s.set, support, &mut linked(support)).map(Noun::Described)
            }
            q => unsupported(format!("trigger: \"{}\" in a pattern", name(q))),
        }
    }

    /// Is this object or player what the noun phrase describes?
    pub(crate) fn is(&self, game: &Game, ctx: &Ctx, t: Target) -> bool {
        match self {
            Noun::Activation(_) => false, // matches occurrence metadata, never a permanent
            Noun::This => Some(t) == ctx.source.map(Target::Object),
            Noun::You => t == Target::Player(ctx.controller),
            // what it was attached to as that left the battlefield is still what it names
            // (603.10a)
            Noun::Attached => {
                ctx.source.is_some_and(|source| game.attached_to(source) == Some(t) || ctx.was.is_some_and(|o| t == Target::Object(o) && game.obj(source).attached_to == Some(t)))
            }
            Noun::Described(set) => set.contains(game, ctx, &NoLocals, t),
            Noun::Card(cards) => matches!(t, Target::Object(o) if cards.contains(game, ctx, &NoLocals, o)),
        }
    }
}

/// The form of an event the engine watches: no "under your control", no amount of life.
/// "is put into exile", with no zone it is from: said of a permanent, a zone change from the
/// battlefield ([`ZoneChange::IntoExile`]).
fn into_exile(a: &EventAction) -> bool {
    matches!(a, EventAction::Deed(d) if d.verb == Verb::Move && d.passive
        && matches!(d.args.as_slice(), [(Role::To, Arg::Zone(z))] if !z.other_than && z.zone.zone() == Some(ir::Zone::Exile) && z.zone.owner.is_none() && z.zone.slice.is_none()))
}

fn plain(a: &EventAction) -> bool {
    use EventAction as E;
    match a {
        // A shuffle distinguishes its actor from the library owner (701.24a, 720.3d). A pattern can
        // name that library as "their library" or require it to be "your library".
        E::Shuffles { zone } => zone.owner_of(ir::Zone::Library).is_some_and(|s|
            matches!(s.quant, Quant::Definite(Ref::You | Ref::Anaphor(ir::Anaphor::They)))),
        // "attacks", "attacks alone" (506.5), "attacks with" a number of creatures (508.3c),
        // each counted from the creatures declared as attackers ([`Attacking`])
        E::Attacks { .. } => true,
        // triggers when blockers are declared and none is for it (509.3g): not watched yet
        E::AttacksUnblocked => false,
        E::BecomesBlocked { by, except } => !except && by.len() <= 1,
        // "transforms into [this face]" is said by the face it names, which the permanent
        // has, with this ability, only once it has transformed into it (701.27e, 712.8e)
        E::Transforms { into } => into.as_ref().is_none_or(|face| face.quant == Quant::Definite(Ref::This)),
        // "is attacked" triggers once however many creatures attack (508.3b): not watched yet
        E::Deed(d) if d.verb == Verb::Attack && d.passive => false,
        E::GainsLife { amount } | E::LosesLife { amount } | E::DealsDamage { amount, .. } => amount.is_none(),
        // Departures retain the controller immediately before leaving, including revolt; an
        // entry, the controller it entered under (603.6a, 603.10a).
        E::Deed(d) => d.args.iter().all(|(r, a)| (*r != Role::Under || matches!(d.verb, Verb::LeaveBattlefield | Verb::Enter | Verb::Die)) && matches!(a, Arg::Noun(_))),
        E::PutsCounters { .. } => true,
        // "the fourth plan counter is put on ~" (122.7): not watched
        E::CountersPut { nth, .. } => nth.is_none(),
        E::CountersRemoved { kind, nth, .. } => nth.is_none()
            || (*nth == Some(ir::event::CounterNth::Last) && kind.is_some()),
        _ => true,
    }
}

/// One action of a pattern as its phrase says it, lowered: the event it is, what its
/// subject is in that event, and what fills each role it names.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Action {
    verb: Verb,
    subject: SubjectIs,
    /// "you shuffle your library": the one who shuffles is the ability's controller.
    yours: bool,
    /// "deals combat damage", "noncombat damage"
    kind: DamageKind,
    excess: bool,
    /// The phrase's [`arguments`]: each role, with the alternatives said for it.
    arguments: Vec<(Role, Vec<Noun>)>,
    /// What every such event says that the phrase leaves [`unsaid`], in order.
    unsaid: Vec<Option<Role>>,
    zone_change: Option<ZoneChange>,
    /// Kind, whether one batch is one occurrence, and whether it must remove the last.
    counters: Option<(Option<String>, bool, bool)>,
    attacking: Option<Attacking>,
    /// "becomes level 2": the level it must become (716.2a).
    level: Option<u32>,
}

/// What a declaration of attackers must have been (508.1), counted from the creatures declared
/// in it and never from one put onto the battlefield attacking (508.3a).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Attacking {
    /// "attacks alone": the only creature declared as an attacker (506.5, 702.83b)
    Alone,
    /// "you attack with three or more creatures" (508.3c), "~ and at least two other creatures
    /// attack": at least this many such attackers besides the subject, which are the event's
    /// argument `at`. `besides`: what `those` says, "other" meaning other than the attacking
    /// subject, which is left out already, not than the ability's source (508.3a, 603.2c)
    With { at: usize, those: Noun, besides: Noun, at_least: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ZoneChange {
    IntoGraveyard { owner: Noun, battlefield: bool },
    /// "is put into exile", said of a permanent: from the battlefield.
    IntoExile,
    Leaves { from: Zone, owner: Option<Noun> },
}

impl Attacking {
    /// Was the declaration this event is part of as the phrase says? Some: it was, with the
    /// attackers it counted, when it counts some. The creatures declared, kept as declared:
    /// one removed from combat before the triggers are checked (506.4) was still declared.
    fn declared(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Option<(usize, Vec<Target>)>> {
        let all = f.attackers.as_deref().unwrap_or(&game.combat.declared_attackers);
        let declared = all.iter().copied();
        // an attacker that has left since is described as it last was (608.2h)
        let is = |noun: &Noun, o: ObjId| noun.is(game, &Ctx { was: Some(o), ..ctx.clone() }, Target::Object(o));
        match self {
            Attacking::Alone => {
                let Some(Target::Object(attacker)) = f.doer else { return None };
                (all == [attacker]).then_some(None)
            }
            Attacking::With { at, those, besides, at_least } => {
                // a player attacks with the creatures they control; a creature with the others
                let counted: Vec<Target> = match f.doer? {
                    Target::Player(p) => declared.filter(|o| game.controller(*o) == p).filter(|o| is(those, *o)).map(Target::Object).collect(),
                    Target::Object(attacker) => declared.filter(|o| *o != attacker).filter(|o| is(besides, *o)).map(Target::Object).collect(),
                };
                (counted.len() as u32 >= *at_least).then_some(Some((*at, counted)))
            }
        }
    }
}

impl Action {
    /// `subject_named`: the pattern says who or what does it.
    fn compile(a: &EventAction, subject_named: bool, support: &Support<'_>) -> Res<Action> {
        let Some((verb, subject)) = a.reading() else { return unsupported(format!("trigger: {}", name(a))) };
        let zone_change = match a {
            EventAction::PutIntoGraveyard { into, from: from @ (None | Some(ir::Zone::Battlefield)) } => Some(ZoneChange::IntoGraveyard { owner: Noun::compile(into, support)?, battlefield: from.is_some() }),
            EventAction::LeavesZone { zone } if zone.zone() == Some(ir::Zone::Graveyard) && zone.slice.is_none() =>
                Some(ZoneChange::Leaves { from: Zone::Graveyard, owner: zone.owner.as_deref().map(|s| Noun::compile(s, support)).transpose()? }),
            _ if into_exile(a) => Some(ZoneChange::IntoExile),
            _ => None,
        };
        if (!WATCHED.contains(&verb) && zone_change.is_none()) || !(plain(a) || into_exile(a)) {
            // an action said from its lexicon entry is named by its verb
            let said = match a {
                EventAction::Deed(d) => name(&d.verb),
                a => name(a),
            };
            return unsupported(format!("trigger: {said}"));
        }
        let mut attacking = match a {
            EventAction::Attacks { alone: true, with: None, .. } => Some(Attacking::Alone),
            _ => None,
        };
        let mut filled = Vec::new();
        for (role, alternatives) in arguments(a) {
            if role == Role::With && verb == Verb::Attack {
                let [s] = alternatives.as_slice() else { return unsupported("trigger: attacking with one of several") };
                let Quant::Indefinite(Count::AtLeast(Amount::Const(n))) = s.quant else {
                    return unsupported("trigger: attacking with other than at least a number of creatures");
                };
                let one = Selector { quant: Quant::Indefinite(Count::Exactly(Amount::Const(1))), ..(*s).clone() };
                let mut besides = one.clone();
                besides.set.other = false;
                attacking = Some(Attacking::With { at: filled.len(), those: Noun::compile(&one, support)?, besides: Noun::compile(&besides, support)?, at_least: n });
                filled.push((role, Vec::new()));
                continue;
            }
            let mut nouns = Vec::with_capacity(alternatives.len());
            for s in alternatives {
                if verb == Verb::DealDamage && role == Role::To {
                    crate::event::damage_recipient_ok(&s.set)?;
                }
                let noun = Noun::compile(s, support)?;
                if verb != Verb::Activate && matches!(noun, Noun::Activation(_)) {
                    return unsupported("trigger: activation predicate requires an activation event");
                }
                nouns.push(noun);
            }
            filled.push((role, nouns));
        }
        let excess = matches!(a, EventAction::IsDealtDamage { excess: true, .. });
        if matches!(a, EventAction::IsDealtDamage { excess: true, by, .. } if !by.is_empty()) {
            return unsupported("excess trigger: source-qualified aggregate requires explicit allocation semantics");
        }
        let yours = matches!(a, EventAction::Shuffles { zone } if zone.owner_of(ir::Zone::Library).is_some_and(|s| s.quant == Quant::Definite(Ref::You)));
        let kind = match a {
            EventAction::DealsDamage { kind, .. } | EventAction::IsDealtDamage { kind, .. } => *kind,
            _ => DamageKind::Any,
        };
        let unsaid = unsaid(a, subject_named).into_iter().map(|(role, _)| role).collect();
        let counters = match a {
            EventAction::CountersPut { kind, one_or_more, .. } | EventAction::PutsCounters { kind, one_or_more, .. } => Some((kind.as_ref().map(|k| k.0.clone()), *one_or_more, false)),
            EventAction::CountersRemoved { kind, one_or_more, nth } =>
                Some((kind.as_ref().map(|k| k.0.clone()), *one_or_more, *nth == Some(ir::event::CounterNth::Last))),
            _ => None,
        };
        let level = match a { EventAction::BecomesLevel(n) => Some(*n), _ => None };
        Ok(Action { verb, subject, yours, kind, excess, arguments: filled, unsaid, zone_change, counters, attacking, level })
    }

    /// Is the event this action as the phrase says it, whoever its subject is? `ctx`: as the
    /// fact gives it ([`Fact::context`]).
    fn matches(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Matched> {
        if self.verb != f.verb || self.excess != f.excess { return None; }
        if self.verb == Verb::Shuffle {
            let owner = if self.yours { Some(Target::Player(ctx.controller)) } else { f.doer };
            if f.role(Role::Of) != owner { return None; }
        }
        match &self.zone_change {
            Some(ZoneChange::IntoGraveyard { owner, battlefield }) if f.zones.is_none_or(|(from,to)| to != Zone::Graveyard || *battlefield && from != Zone::Battlefield)
                || f.was.is_some() != *battlefield || !owner.is(game, ctx, f.role(Role::To)?) => return None,
            Some(ZoneChange::IntoExile) if f.zones != Some((Zone::Battlefield, Zone::Exile)) => return None,
            Some(ZoneChange::Leaves { from, owner }) if f.zones.is_none_or(|(zone, _)| zone != *from)
                || owner.as_ref().is_some_and(|owner| f.role(Role::From).is_none_or(|p| !owner.is(game, ctx, p))) => return None,
            _ => {}
        }
        // "deals combat damage", "noncombat damage": the kind the event was
        if matches!((self.kind, f.combat), (DamageKind::Combat, Some(false)) | (DamageKind::Noncombat, Some(true))) {
            return None;
        }
        // an event that says by whom it was done happens once for each of them ("becomes
        // blocked by a creature", 509.3d); said of what it is done to alone, it is the event
        // that happens once ("becomes blocked", 509.3c)
        let asks_by = self.arguments.iter().any(|(r, _)| *r == Role::By);
        if matches!(self.subject, SubjectIs::Role(_)) && f.role(Role::By).is_some() && !asks_by {
            return None;
        }
        // "blocks" fires once for becoming a blocker; "blocks a creature" fires for
        // each pair (509.3a–b). Pair facts must not also match the unqualified form.
        if self.verb == Verb::Block && matches!(self.subject, SubjectIs::Doer)
            && f.role(Role::What).is_some() && !self.arguments.iter().any(|(r, _)| *r == Role::What) {
            return None;
        }
        // each role the phrase fills is filled in the event by something it names. What "by"
        // names after a passive is the doer ("is dealt damage by a red instant or sorcery
        // spell you control": the source of the damage)
        let group = match &self.attacking {
            None => None,
            Some(attacking) => Some(attacking.declared(game, ctx, f)?),
        };
        let mut filled = Vec::with_capacity(self.arguments.len());
        for (role, alternatives) in &self.arguments {
            if alternatives.is_empty() { filled.push(None); continue; }
            if *role == Role::What && alternatives.iter().any(|noun| matches!(noun, Noun::Activation(_))) {
                let (keyword, mana) = f.activation?;
                if !alternatives.iter().any(|noun| matches!(noun, Noun::Activation(filter) if filter.matches(keyword, mana))) { return None; }
                filled.push(f.role(*role));
                continue;
            }
            let t = f.role(*role).or(if *role == Role::By { f.doer } else { None })?;
            if !alternatives.iter().any(|noun| noun.is(game, ctx, t)) {
                return None;
            }
            filled.push(Some(t));
        }
        // the subject: who did it, or what it happened to; none named in the passive
        let subject = match self.subject {
            SubjectIs::Doer => f.doer,
            SubjectIs::Role(r) => f.role(r),
        };
        if self.level.is_some() && f.level != self.level { return None; }
        // "When you unlock this door": the half of this Room the ability is printed on
        // (709.5h, 709.5j)
        if self.verb == Verb::UnlockDoor {
            let source = ctx.source?;
            let door = game.def(source)?.door_of(ctx.ability.clone()?.index)?;
            if f.door != Some((source, door)) { return None; }
        }
        let times = if let Some((wanted, together, last)) = &self.counters {
            let (kind, count) = f.counters.as_ref()?;
            if *count == 0 || wanted.as_ref().is_some_and(|w| w != kind) { return None; }
            // The last counter is one occurrence, even when several leave together; its
            // event-time remainder cannot be reconstructed from the current counter map.
            if *last && f.remaining != Some(0) { return None; }
            // 603.2c: "a counter" is an occurrence per counter; "one or more" is one batch.
            if *together || *last { 1 } else { *count }
        } else { 1 };
        let said = filled.len();
        Some(Matched { subject, participants: filled, times, group: group.flatten(), action: 0, said })
    }
}

/// A pattern's actions as its phrase means them: "blocks or becomes blocked by a Vampire"
/// says the Vampire of both, what it blocks and what blocks it. Lowering the pattern and
/// naming what took part in its event ([`crate::program::input`]) share this.
pub(crate) fn said(p: &EventPattern) -> Vec<EventAction> {
    let mut actions = p.actions.clone();
    for at in 1..actions.len() {
        if let EventAction::BecomesBlocked { by, except: false } = &actions[at]
            && let [by] = by.as_slice()
            && actions[at - 1] == (EventAction::Blocks { blocked: None })
        {
            actions[at - 1] = EventAction::Blocks { blocked: Some(Box::new(by.clone())) };
        }
    }
    actions
}

/// The bounded draw counter describes all cards, not a filtered subset. Other ordinal
/// predicates need their own event-time summary rather than inspecting today's objects.
fn ordinal_draw(p: &EventPattern) -> bool {
    let [EventAction::Deed(deed)] = p.actions.as_slice() else { return false };
    if deed.verb != Verb::Draw || deed.passive { return false; }
    let [(Role::What, ir::event::Arg::Noun(card))] = deed.args.as_slice() else { return false };
    *card == Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), ir::ObjectSet::of(ir::Domain::Card))
}

/// "becomes the target of a spell or ability you control for the first time each turn": the
/// event's own ordinal counts exactly these, every spell and ability its controller controls.
fn ordinal_targeted(p: &EventPattern) -> bool {
    let [EventAction::BecomesTarget { of: Some(of) }] = p.actions.as_slice() else { return false };
    let ir::Domain::Union(sets) = &of.set.domain else { return false };
    let plain = |s: &ir::ObjectSet| s.filter == ir::Filter::All(Vec::new()) && s.controller.is_none() && s.owner.is_none() && s.caster.is_none() && s.zone.is_none() && !s.other;
    let mut domains: Vec<_> = sets.iter().filter(|s| plain(s)).map(|s| s.domain.clone()).collect();
    domains.sort_by_key(|d| format!("{d:?}"));
    domains == [ir::Domain::Ability, ir::Domain::Spell] && sets.len() == 2
        && of.set.filter == ir::Filter::All(Vec::new()) && of.set.owner.is_none() && of.set.caster.is_none() && of.set.zone.is_none() && !of.set.other
        && of.set.controller.as_ref().is_some_and(|r| !r.negated && !r.past && r.who.quant == Quant::Definite(Ref::You))
}

fn actions(p: &EventPattern, subject_named: bool, support: &Support<'_>) -> Res<Vec<Action>> {
    if p.during.is_some() || p.condition.is_some() || p.cause.is_some() {
        return unsupported("trigger: what caused it");
    }
    // "your second spell each turn": counted among what is noted as it happens in a turn
    // what the turn's history counts is each event of the pattern, by the same subject
    // ([`crate::history`]): of several actions, which of them is the counted one is unsaid
    if p.nth_each_turn.is_some() && p.actions.len() != 1 && !ordinal_draw(p) && !ordinal_targeted(p) {
        return unsupported("trigger: an ordinal, of what is not counted in a turn");
    }
    // A player's attack is the declaration (508.1), which names nobody attacked: "whenever
    // you attack enchanted player" would never be seen to happen.
    let by_player = p.subjects.iter().any(|s| ir::lexicon::denotes_players(s) != Some(false));
    if by_player && p.actions.iter().any(|a| matches!(a, EventAction::Attacks { defender, .. } if !defender.is_empty())) {
        return unsupported("trigger: a player attacks whom the pattern names");
    }
    // what is put into exile from the battlefield is a permanent; a card put there from
    // elsewhere is not watched for
    if p.actions.iter().any(into_exile) && !p.subjects.iter().all(|s| s.set.domain == ir::Domain::Permanent && s.set.zone.is_none()) {
        return unsupported("trigger: put into exile, of other than a permanent");
    }
    said(p).iter().map(|a| Action::compile(a, subject_named, support)).collect()
}

/// What a delayed triggered ability watches for (603.7), lowered as the effect that makes it
/// is: its pattern's actions, whoever their subject is. The particular objects it is made of
/// are what whoever asks supplies (603.7c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence(Vec<Action>);

impl Occurrence {
    /// Refused where the engine cannot watch for what the pattern says happens.
    pub(crate) fn compile(p: &EventPattern, support: &Support<'_>) -> Res<Occurrence> {
        if p.nth_each_turn.is_some() || p.except_nth_in_step.is_some() { return unsupported("trigger: an ordinal, of a delayed ability"); }
        actions(p, true, support).map(Occurrence)
    }

    /// Did this happen to one of these? `ctx`: of the text that made the delayed ability.
    #[cfg(test)]
    pub(crate) fn happened_to(&self, game: &Game, ctx: &Ctx, f: &Fact, subjects: &[Target]) -> bool {
        self.times_on(game, ctx, f, subjects).is_some()
    }
    fn times_on(&self, game: &Game, ctx: &Ctx, f: &Fact, subjects: &[Target]) -> Option<u32> {
        let ctx = f.context(ctx.clone());
        self.0.iter().find_map(|a| {
            let matched = a.matches(game, &ctx, f)?;
            matched.subject.is_some_and(|s| subjects.contains(&s)).then_some(matched.times)
        })
    }
}

/// Another of the subjects of one event joins the trigger it made (603.2c): each of them is
/// the subject, each participant left unsaid is kept, and the amounts add up.
fn join_batch(triggers: &mut [Triggered], matched: &Matched, fact: &Fact, joined: &Bound) {
    for prior in triggers {
        if let Some(subject) = matched.subject && let Some((_, subjects)) = prior.bound.inputs.iter_mut().find(|(input, _)| *input == crate::program::input::Input::Subject) {
            if !subjects.contains(&subject) { subjects.push(subject); }
        }
        for (at, other) in matched.participants.iter().enumerate().skip(matched.said) {
            let Some(other) = other else { continue };
            let input = crate::program::input::Input::Argument(at);
            match prior.bound.inputs.iter_mut().find(|(i, _)| *i == input) {
                Some((_, those)) => if !those.contains(other) { those.push(*other) },
                None => prior.bound.inputs.push((input, vec![*other])),
            }
        }
        prior.bound.event_amount = prior.bound.event_amount.zip(fact.amount).map(|(a, b)| a.saturating_add(b));
        // and where each of them went is followed too (400.7e)
        for pair in &joined.became {
            if !prior.bound.became.contains(pair) { prior.bound.became.push(*pair); }
        }
    }
}

/// Any of a pattern's subjects doing any of its actions, lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pattern {
    subjects: Vec<Noun>,
    actions: Vec<Action>,
    /// "your second spell each turn": only the n-th such event its doer has done in the turn.
    nth: Option<u32>,
    /// The event's own ordinal is the one asked: of every card its player draws, of every
    /// spell and ability its controller targets with ([`ordinal_draw`], [`ordinal_targeted`]).
    /// Else the turn's history counts the events the pattern describes.
    own_ordinal: bool,
    /// "except the first one they draw in each of their draw steps": not the n-th draw of the
    /// drawing player's own draw step.
    except_in_draw_step: Option<u32>,
    grouped: bool,
    /// "during your turn", "during an opponent's turn": whose turn it is as it happens, or
    /// (true) whose turn it isn't ("during turns other than yours").
    during: Option<(Noun, bool)>,
    /// "during your main phase": in one of these parts of that player's turn as it happens.
    during_part: Option<(Vec<ir::Part>, Noun)>,
    /// "attacks while saddled", "attacks while you control a creature with power 4 or
    /// greater": true as the event happens, or it does not trigger (603.2). Unlike an
    /// intervening "if" (603.4) it is not asked again as the ability resolves: "attacks while
    /// saddled" triggers only if the creature was saddled as it was declared an attacker.
    condition: Option<Box<condition::Condition>>,
}

impl Pattern {
    /// 702.177b asks whether an exhaust activation began, rather than whether its payment finished.
    pub(crate) fn exhaust_starts(&self) -> bool {
        !self.actions.is_empty() && self.actions.iter().all(|action| action.verb == Verb::Activate
            && action.arguments.iter().any(|(role, nouns)| *role == Role::What
                && !nouns.is_empty() && nouns.iter().all(|noun| matches!(noun, Noun::Activation(filter) if filter.only_exhaust()))))
    }

    pub(crate) fn compile(p: &EventPattern, support: &Support<'_>) -> Res<Pattern> {
        let (mut during, mut during_part) = (None, None);
        match &p.during {
            None => {}
            Some(ir::event::During::Turn(whose)) => during = Some((Noun::compile(whose, support)?, false)),
            Some(ir::event::During::NotTurn(whose)) => during = Some((Noun::compile(whose, support)?, true)),
            // "during your main phase": either of the two (505.1)
            Some(ir::event::During::Step(step @ (ir::event::Step::Main | ir::event::Step::FirstMain | ir::event::Step::SecondMain), ir::event::StepOwner::Player { who, next: false })) => {
                let parts = match step {
                    ir::event::Step::FirstMain => vec![ir::Part::PrecombatMain],
                    ir::event::Step::SecondMain => vec![ir::Part::PostcombatMain],
                    _ => vec![ir::Part::PrecombatMain, ir::Part::PostcombatMain],
                };
                during_part = Some((parts, Noun::compile(who, support)?));
            }
            Some(_) => return unsupported("trigger: during a step or phase other than a player's main phase"),
        }
        let actions = actions(&EventPattern { during: None, condition: None, ..p.clone() }, !p.subjects.is_empty(), support)?;
        let condition = p.condition.as_deref().map(|c| crate::program::initial_condition(c, support).map(Box::new)).transpose()?;
        let except_in_draw_step = match p.except_nth_in_step {
            None => None,
            // counted as draws complete: of "draws a card", nothing narrower
            Some((n, ir::event::Step::Draw)) if ordinal_draw(p) && p.nth_each_turn.is_none() => Some(n),
            Some(_) => return unsupported("trigger: an exception of other than a draw in a draw step"),
        };
        // "whenever one or more creatures you control die": once for each event, however many
        // of them it was of (603.2c), each of them the subject
        let grouped_subject = matches!(p.subjects.as_slice(), [s] if s.quant == Quant::Indefinite(Count::AtLeast(Amount::Const(1))));
        // Several counter kinds in one placement are still one "one or more" event.
        // The named recipient remains in the grouping key, so different recipients
        // each trigger; commitment preserves distinct placement instructions (603.2c).
        let grouped = grouped_subject || actions.iter().any(|a| a.counters.as_ref().is_some_and(|(_, together, _)| *together));
        let subjects = p.subjects.iter().map(|s| {
            let mut one = s.clone();
            if grouped_subject { one.quant = Quant::Indefinite(Count::Exactly(Amount::Const(1))); }
            Noun::compile(&one, support)
        }).collect::<Res<_>>()?;
        let own_ordinal = ordinal_draw(p) || ordinal_targeted(p);
        Ok(Pattern { actions, subjects, nth: p.nth_each_turn, own_ordinal, except_in_draw_step, grouped, during, during_part, condition })
    }

    /// [`Pattern::described`], and the n-th such event of the turn where the pattern says
    /// which ([`crate::history`]).
    pub(crate) fn matches(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Matched> {
        let matched = self.described(game, ctx, f)?;
        if let Some((whose, not)) = &self.during && whose.is(game, ctx, Target::Player(game.active)) == *not { return None; }
        if let Some((parts, whose)) = &self.during_part && (!parts.contains(&game.part) || !whose.is(game, ctx, Target::Player(game.active))) { return None; }
        if self.except_in_draw_step.is_some() && f.in_draw_step == self.except_in_draw_step { return None; }
        // as the event happens: of what it happened to as it was then (603.2, 603.10)
        if let Some(condition) = &self.condition && !condition.evaluate(game, &f.context(ctx.clone()), &bound_of(game, matched.clone(), f)) { return None; }
        let ordinal = |f: &Fact| if self.own_ordinal { f.ordinal } else { game.nth_this_turn(ctx, self, f) };
        self.nth.is_none_or(|n| ordinal(f) == Some(n)).then_some(matched)
    }

    /// What or who it happened to, of an event one of the pattern's actions describes, whatever
    /// the pattern says its subject is: "a card put into a graveyard this turn" is that card.
    pub(crate) fn subject_of(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Target> {
        let ctx = f.context(ctx.clone());
        self.actions.iter().find_map(|a| a.matches(game, &ctx, f)?.subject)
    }

    /// Does what happened match the pattern? Some: it does, as this one of the pattern's
    /// actions, whose subject in the event was this (when the pattern names a subject): of
    /// "blocks or becomes blocked by a creature", the creature that blocked in the one, the
    /// one that was blocked in the other.
    pub(crate) fn described(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Matched> {
        let ctx = f.context(ctx.clone());
        let subject_named = !self.subjects.is_empty();
        for (action, a) in self.actions.iter().enumerate() {
            let Some(matched) = a.matches(game, &ctx, f) else { continue };
            if subject_named && !matched.subject.is_some_and(|t| self.subjects.iter().any(|noun| noun.is(game, &ctx, t))) {
                continue;
            }
            let mut participants = matched.participants;
            participants.extend(a.unsaid.iter().map(|role| match role {
                None => f.role(Role::By).or(f.doer),
                Some(r) => f.role(*r),
            }));
            return Some(Matched { subject: matched.subject.filter(|_| subject_named), participants, times: matched.times, group: matched.group, action, said: matched.said });
        }
        None
    }
}

/// What a triggered ability watches for, lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Watch {
    Event(Pattern),
    /// "At the beginning of your upkeep": the part its text names, in a turn of the player it
    /// says; None: in anyone's ("each upkeep", "the end step").
    Step { step: Step, whose: Option<Noun> },
    /// One ability with several trigger conditions.
    Any(Vec<Watch>),
    /// "When you control no Islands": a state trigger, which watches for no event; it
    /// triggers as soon as the game is as it says (603.8). `of`: what the state is said of,
    /// which its effect can refer back to: the ability's source (false) or its controller.
    State { condition: condition::Condition, of: Option<bool> },
    /// A chapter ability (714.2b): "When one or more lore counters are put onto this Saga, if
    /// the number of lore counters on it was less than N and became at least N, …", for each N
    /// of its chapter symbol (714.2c), judged by the counters as they were put.
    Chapter(Vec<u32>),
}

#[derive(Clone, Copy)]
enum Interest { Event(Verb), Step, State }

impl Watch {
    /// Reject unrelated subscriptions before constructing occurrence contexts or
    /// querying the source's controller. This reads only the compiled pattern.
    fn interested(&self, interest: Interest) -> bool {
        match (self, interest) {
            (Self::Any(watches), kind) => watches.iter().any(|watch| watch.interested(kind)),
            (Self::Event(pattern), Interest::Event(verb)) => pattern.actions.iter().any(|action| action.verb == verb),
            (Self::Chapter(_), Interest::Event(Verb::PutCounters)) => true,
            (Self::Step { .. }, Interest::Step) | (Self::State { .. }, Interest::State) => true,
            _ => false,
        }
    }
    fn self_connive(&self)->bool {match self {
        Self::Event(p)=>p.subjects==[Noun::This], Self::Any(w)=>w.iter().any(Self::self_connive), _=>false
    }}
    fn compile(t: &Trigger, support: &Support<'_>) -> Res<Watch> {
        match t {
            Trigger::Event { pattern, .. } => Pattern::compile(pattern, support).map(Watch::Event),
            Trigger::Any(v) => v.iter().map(|t| Watch::compile(t, support)).collect::<Res<_>>().map(Watch::Any),
            Trigger::Step { step, whose } => {
                if !Part::ORDER.iter().any(|p| p.named(*step)) {
                    return unsupported("trigger: the beginning of a turn");
                }
                let whose = match whose {
                    StepOwner::Each | StepOwner::The { next: false } => None,
                    StepOwner::Player { who, next: false } => Some(Noun::compile(who, support)?),
                    // "the next end step": a delayed trigger an effect makes (603.7), not an
                    // ability a permanent has
                    other => return unsupported(format!("trigger: a step that is {}", name(other))),
                };
                Ok(Watch::Step { step: *step, whose })
            }
            Trigger::State(c) => {
                let of = match c {
                    Condition::Is { what, .. } if what.quant == Quant::Definite(Ref::This) => Some(false),
                    Condition::Is { what, .. } if what.quant == Quant::Definite(Ref::You) => Some(true),
                    _ => None,
                };
                Ok(Watch::State { condition: condition::Condition::compile(c, support, &mut linked(support))?, of })
            }
        }
    }

    /// Is the game as a state trigger says (603.8)? Some: it is, said of this.
    fn is_so(&self, game: &Game, ctx: &Ctx) -> Option<Option<Target>> {
        match self {
            Watch::State { condition, of } => condition.evaluate(game, ctx, &NoLocals)
                .then(|| of.and_then(|player| if player { Some(Target::Player(ctx.controller)) } else { ctx.source.map(Target::Object) })),
            Watch::Any(v) => v.iter().find_map(|w| w.is_so(game, ctx)),
            Watch::Event(_) | Watch::Step { .. } | Watch::Chapter(_) => None,
        }
    }

    /// Does it trigger as this step or phase begins (500.6)? "Your upkeep" in its
    /// controller's turn, "each upkeep" and "the end step" in anyone's.
    fn grouped(&self) -> bool { matches!(self, Watch::Event(pattern) if pattern.grouped) }
    fn begins(&self, game: &Game, ctx: &Ctx, part: Part) -> bool {
        match self {
            Watch::Step { step, whose } => part.named(*step) && whose.as_ref().is_none_or(|who| who.is(game, ctx, Target::Player(game.active))),
            Watch::Any(v) => v.iter().any(|w| w.begins(game, ctx, part)),
            Watch::Event(_) | Watch::State { .. } | Watch::Chapter(_) => false,
        }
    }

    /// Does what happened make it trigger? Some: it does, as this: the trigger condition's
    /// subject in the event, when it names one, and what its phrase's arguments were.
    fn matches(&self, game: &Game, ctx: &Ctx, f: &Fact) -> Option<Matched> {
        match self {
            Watch::Event(pattern) => pattern.matches(game, ctx, f),
            Watch::Any(v) => v.iter().find_map(|w| w.matches(game, ctx, f)),
            Watch::Chapter(chapters) => {
                // lore counters put onto this Saga: each chapter whose number they reached
                let (Some((kind, count)), Some(before)) = (f.counters.as_ref(), f.had) else { return None };
                if f.verb != Verb::PutCounters || kind != "lore" || f.role(Role::On) != ctx.source.map(Target::Object) { return None; }
                let reached = chapters.iter().filter(|n| before < **n && before + count >= **n).count() as u32;
                (reached > 0).then(|| Matched { subject: ctx.source.map(Target::Object), participants: Vec::new(), times: reached, group: None, action: 0, said: 0 })
            }
            Watch::Step { .. } | Watch::State { .. } => None,
        }
    }
}

/// A delayed subscription either retains event participants or waits for a future step.
/// Step matching is the same compiled query used by printed triggered abilities (603.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedPattern(DelayedWatch);
#[derive(Debug, Clone, PartialEq, Eq)]
enum DelayedWatch { Event(Occurrence), Any(Pattern), Step(Watch) }
impl DelayedPattern {
    /// Rule-level timing for portable outstanding subscriptions. This profile
    /// has no dynamic player predicate or retained event subject to summarize.
    pub(crate) fn initial_step(&self) -> Option<&'static str> {
        let DelayedWatch::Step(Watch::Step {step,whose:None})=&self.0 else {return None;};
        Some(match step {
            Step::Untap=>"untap",Step::Upkeep=>"upkeep",Step::Draw=>"draw",Step::FirstMain=>"precombat_main",
            Step::Combat=>"begin_combat",Step::DeclareAttackers=>"declare_attackers",Step::DeclareBlockers=>"declare_blockers",
            Step::SecondMain=>"postcombat_main",Step::EndOfCombat=>"end_combat",Step::End=>"end",Step::Cleanup=>"cleanup",
            Step::Turn|Step::Main=>return None,
        })
    }
    pub(crate) fn event(pattern: Occurrence) -> Self { Self(DelayedWatch::Event(pattern)) }
    /// "whenever a creature an opponent controls attacks": the event happening to whatever
    /// the pattern describes, as a printed triggered ability watches for it.
    pub(crate) fn any(pattern: Pattern) -> Self { Self(DelayedWatch::Any(pattern)) }
    /// Does it watch a mana event (605.1b), as [`mana_event`] asks of a printed trigger?
    fn mana_event(&self) -> bool {
        let actions = match &self.0 { DelayedWatch::Event(Occurrence(actions)) => actions, DelayedWatch::Any(pattern) => &pattern.actions, DelayedWatch::Step(_) => return false };
        actions.iter().any(|a| a.verb == Verb::TapForMana)
    }
    fn described(&self, game: &Game, ctx: &Ctx, fact: &Fact) -> Option<Matched> {
        match &self.0 { DelayedWatch::Any(pattern) => pattern.matches(game, ctx, fact), _ => None }
    }
    /// "Whenever one or more creatures you control die this turn": once for each event.
    fn grouped(&self) -> bool { matches!(&self.0, DelayedWatch::Any(pattern) if pattern.grouped) }
    pub(crate) fn step(step: Step, whose: &StepOwner, support: &Support<'_>) -> Res<Self> {
        let whose = match whose {
            StepOwner::The { .. } => StepOwner::The { next: false },
            StepOwner::Player { who, .. } => StepOwner::Player { who: who.clone(), next: false },
            // "your first upkeep", "the first upkeep": made before the game begins (103.6), the
            // next one is the first; made later, the first is past (refused where lowered)
            StepOwner::First { .. } if !support.opening() => return unsupported("delayed trigger: the first upkeep, said other than by an opening-hand action"),
            StepOwner::First { who: Some(who) } => StepOwner::Player { who: who.clone(), next: false },
            StepOwner::First { who: None } => StepOwner::The { next: false },
            _ => return unsupported("delayed trigger: relative turn or recurring step"),
        };
        Watch::compile(&Trigger::Step { step, whose }, support).map(|w| Self(DelayedWatch::Step(w)))
    }
    fn begins(&self, game: &Game, ctx: &Ctx, part: Part) -> bool {
        matches!(&self.0, DelayedWatch::Step(w) if w.begins(game,ctx,part))
    }
    fn times_on(&self, game: &Game, ctx: &Ctx, fact: &Fact, subjects: &[Target]) -> Option<u32> {
        match &self.0 { DelayedWatch::Event(w) => w.times_on(game,ctx,fact,subjects), _ => None }
    }
}

/// A triggered ability's subscription, lowered once as its definition is loaded: what it
/// watches for and its intervening "if" (603.4). It names no object: the permanent whose
/// ability it is, who controls it and what it last was are supplied as events are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Template {
    watch: Watch,
    intervening: Option<condition::Condition>,
    /// The owning keyword action creates this linked occurrence directly: Madness
    /// discard replacement or Miracle reveal (702.35a, 702.94a), never an unrelated event.
    applied: bool,
    limit: Option<crate::ability_limit::Limit>,
}

impl Template {
    /// The ability's subscription, when it is a triggered ability; None when it is not one.
    /// The one place a trigger condition is refused. What the text refers back to is read
    /// through the definition's links (607.2d).
    fn compile(a: &Ability, support: &Support<'_>) -> Option<Res<Template>> {
        // a chapter ability is a triggered ability (714.2)
        if let Ability::Chapter { chapters, .. } = a {
            return Some(Ok(Template { watch: Watch::Chapter(chapters.clone()), intervening: None, applied: false, limit: None }));
        }
        let Ability::Triggered { trigger, effect, .. } = a else { return None };
        Some((|| {
            // "Whenever you cast a spell, if it's the second creature spell you cast this
            // turn, …": said of the event itself, so it is what is watched for, "you cast
            // your second creature spell each turn". What was the second stays the second:
            // nothing is left to ask again as the ability resolves (603.4).
            if let Some(Condition::NthCastThisTurn { n, set }) = intervening_if(a) {
                let Trigger::Event { pattern, .. } = trigger else { return unsupported("trigger: the n-th spell cast, said of no spell cast"); };
                let mut nth = pattern.clone();
                let you = matches!(nth.subjects.as_slice(), [who] if who.quant == Quant::Definite(Ref::You));
                let ([EventAction::Deed(deed)], None, true) = (nth.actions.as_mut_slice(), nth.nth_each_turn, you) else { return unsupported("trigger: the n-th spell cast, said of other than a spell you cast"); };
                let [(Role::What, ir::event::Arg::Noun(what))] = deed.args.as_mut_slice() else { return unsupported("trigger: the n-th spell cast, said of other than a spell you cast"); };
                let any_spell = ir::ObjectSet { filter: ir::Filter::All(Vec::new()), ..set.clone() };
                if deed.verb != Verb::Cast || deed.passive || what.set != any_spell { return unsupported("trigger: the n-th spell cast, said of other than any spell you cast"); }
                what.set = set.clone();
                nth.nth_each_turn = Some(*n);
                return Ok(Template { watch: Watch::Event(Pattern::compile(&nth, support)?), intervening: None, applied: false, limit: crate::ability_limit::Limit::of(effect)? });
            }
            let watch = Watch::compile(trigger, support)?;
            // asked of the first event of a batch, before the rest is gathered: not of them all
            let life_loss = matches!(trigger, Trigger::Event { pattern, .. } if pattern.actions.iter().all(|a| a.reading().is_some_and(|(v, _)| v == Verb::LoseLife)));
            if watch.grouped() && intervening_if(a).is_some() && !life_loss {
                return unsupported("trigger: an intervening \"if\" of one or more of them");
            }
            let intervening = intervening_if(a).map(|c| crate::program::initial_condition(c, support)).transpose()?;
            Ok(Template { watch, intervening, applied: false, limit: crate::ability_limit::Limit::of(effect)? })
        })())
    }

    /// Does the ability trigger, as far as its intervening "if" goes (603.4)? It triggers
    /// only if the condition is true when the trigger event occurs.
    fn take(&self, game: &Game, ctx: &Ctx, uses: &mut Vec<crate::ability_limit::Use>) -> bool {
        let Some(limit) = self.limit else { return true };
        if game.ability_used(ctx, limit) { return false; }
        if limit == crate::ability_limit::Limit::Trigger && let Some(used) = crate::ability_limit::Use::of(ctx, limit) {
            if uses.contains(&used) { return false; }
            uses.push(used);
        }
        true
    }
    fn holds(&self, game: &Game, ctx: &Ctx) -> bool {
        self.intervening.as_ref().is_none_or(|c| c.evaluate(game, ctx, &NoLocals))
    }
}

/// Every triggered ability of a definition, lowered once: its printed abilities, then those
/// its keywords are, each under the index the rest of the engine knows it by.
pub(crate) fn templates(def: &CardDef) -> Vec<Registered<Template>> {
    let abilities = def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a));
    let offered = crate::keyword::offered_costs(def);
    abilities.enumerate().filter_map(|(i, a)| {
        let support = Support::definition(def, Some(crate::binding::AbilityKey::printed(i)));
        let applied = offered.iter().any(|(_, _, trigger)| *trigger == i);
        Template::compile(a, &support).map(|result| Registered { ability: i, result: result.map(|t| Template { applied, ..t }) })
    }).collect()
}

/// An effective static additional-trigger rule (603.2d). This describes the object
/// whose ordinary ability triggers; it never discovers another occurrence itself.
#[derive(Debug, Clone)]
pub(crate) struct ModifierTemplate { of: expr::Set }

pub(crate) fn modifiers(def: &CardDef) -> Vec<Registered<ModifierTemplate>> {
    def.abilities.iter().enumerate().filter_map(|(index, ability)| {
        let Ability::Text(ir::Effect(rows)) = ability else { return None };
        if !rows.iter().any(|s| matches!(s, Sentence::Static(s) if matches!(s.as_ref(), ir::statics::Static::TriggersAdditional { .. }))) { return None; }
        let support = Support::definition(def, Some(AbilityKey::printed(index)));
        let result = (|| {
            let [Sentence::Static(st)] = rows.as_slice() else { return unsupported("additional triggers: mixed statement"); };
            let ir::statics::Static::TriggersAdditional { cause: ir::statics::TriggerCause::AbilityOf(of), .. } = st.as_ref() else {
                return unsupported("additional triggers: event-qualified cause");
            };
            if of.quant != Quant::Indefinite(Count::Exactly(Amount::Const(1))) || of.chooser.is_some() || of.random.is_some() {
                return unsupported("additional triggers: a described source object");
            }
            Ok(ModifierTemplate { of: expr::Set::compile(&of.set, &support, &mut linked(&support))? })
        })();
        Some(Registered { ability: index, result })
    }).collect()
}

/// Is this ability of the definition a triggered mana ability (605.1b): it watches a mana
/// event, has no target and could add mana?
fn mana_trigger(def: &CardDef, index: usize) -> bool {
    let Some(Ability::Triggered { trigger, .. }) = def.abilities.get(index) else { return false };
    mana_event(trigger) && def.executable_body(index).is_ok_and(|body| {
        let programs = body.programs();
        programs.iter().all(|p| p.targets.is_empty()) && programs.iter().any(|p| p.announcement.adds_mana)
    })
}

/// Does this trigger watch a mana event (605.1b)? The currently supported one is a
/// tap-cost mana ability resolving. Such a trigger that adds mana without targets must
/// not be admitted as an ordinary ability that uses the stack.
pub(crate) fn mana_event(t: &Trigger) -> bool {
    match t {
        Trigger::Event { pattern, .. } => pattern.actions.iter().any(|a| a.reading().is_some_and(|(v, _)| v == Verb::TapForMana)),
        Trigger::Any(v) => v.iter().any(mana_event),
        _ => false,
    }
}

/// The noun phrases a phrase fills its action's roles with besides its subject, in the order
/// it says them; several for one role are alternatives ("attacks you or a planeswalker you
/// control").
pub(crate) fn arguments(a: &EventAction) -> Vec<(Role, Vec<&Selector>)> {
    // The named graveyard supplies its owner to later "that player" references.
    if let EventAction::PutIntoGraveyard { into, .. } = a { return vec![(Role::To, vec![into])]; }
    let mut groups: Vec<(Role, Vec<&Selector>)> = Vec::new();
    for (role, s) in a.fillers() {
        match groups.iter_mut().find(|(r, _)| *r == role) {
            Some((_, alternatives)) => alternatives.push(s),
            None => groups.push((role, vec![s])),
        }
    }
    groups
}

/// What every event of this action says ([`always_named`]) that its phrase leaves unsaid: not
/// its subject (when the pattern names one), not one of its [`arguments`].
///
/// Whatever took part in an event besides its subject is, in order, the phrase's arguments
/// and then these. An effect refers to one by its place in that order (608.2k), not by the
/// role it filled, which differs between a pattern's actions: of "blocks or becomes blocked
/// by a creature", the other creature is what was blocked in the one, unsaid, and what
/// blocked in the other, said.
pub(crate) fn unsaid(a: &EventAction, subject_named: bool) -> Vec<(Option<Role>, bool)> {
    let Some((verb, subject)) = a.reading() else { return Vec::new() };
    // what "by" names after a passive is the doer
    let key = |r: Role| (r != Role::By).then_some(r);
    let subject = subject_named.then(|| match subject {
        SubjectIs::Doer => None,
        SubjectIs::Role(r) => key(r),
    });
    let said: Vec<Option<Role>> = arguments(a).iter().map(|(r, _)| key(*r)).collect();
    always_named(verb).iter().copied().filter(|(r, _)| subject != Some(*r) && !said.contains(r)).collect()
}

/// How an event matched an action as a phrase says it: its subject in the event (who did
/// it, or what it happened to; none, when the event names nobody there), and what else took
/// part: what filled each of the phrase's [`arguments`], then (of a pattern's action) what
/// it left [`unsaid`].
#[derive(Clone)]
pub(crate) struct Matched {
    pub(crate) subject: Option<Target>,
    pub(crate) participants: Vec<Option<Target>>,
    times: u32,
    /// The attackers counted ([`Attacking::With`]): the participant at that place, all of them.
    group: Option<(usize, Vec<Target>)>,
    /// Of a pattern's actions, which one it matched, and how many of the participants are
    /// what the phrase said; the rest are what it left unsaid.
    action: usize,
    said: usize,
}

/// The intervening "if" of a triggered ability (603.4): "When/Whenever/At [trigger event], if
/// [condition], [effect]". Only the "if" that immediately follows the trigger condition is
/// one, which in the text as read is what leads the first sentence of the effect; "if" has its
/// normal meaning anywhere else, and "if you do" asks about an instruction, not the game.
pub(crate) fn intervening_if(a: &Ability) -> Option<&Condition> {
    let Ability::Triggered { effect, .. } = a else { return None };
    match effect.0.first()? {
        Sentence::Led { lead: Lead::Modifier(Modifier::If(c)), .. } if !matches!(c, Condition::Did { .. }) => Some(c),
        _ => None,
    }
}

/// What the event that made an ability trigger was about, for the effect to refer to
/// (608.2k): the trigger condition's subject first, which is what "it" means when it is an
/// object, then whatever else took part ("whenever this creature blocks or becomes blocked by
/// a creature, destroy that creature"), each under the key the ability's schema gave it
/// ([`crate::program::input`]): the subject, and each other participant by its place in the
/// action that matched.
fn bound_of(game: &Game, matched: Matched, f: &Fact) -> Bound {
    let Matched { subject, participants, group, .. } = matched;
    debug_assert_eq!(f.amount.is_some(), !f.excess && has_amount(f.verb), "the event must supply the quantity its schema promises");
    // what the check before a card is cast counted on is there
    debug_assert!(
        (if f.excess { &[][..] } else { always_named(f.verb) }).iter().all(|(role, players)| {
            let named = role.map_or(f.doer, |r| f.role(r));
            named.is_some_and(|t| matches!(t, Target::Player(_)) == *players)
        }),
        "an event of {:?} names what every such event does",
        f.verb
    );
    let mut bound = Bound::default();
    bound.event_amount = f.amount;
    use crate::program::input::Input;
    if let Some(subject) = subject { bound.inputs.push((Input::Subject, vec![subject])); }
    for (at, participant) in participants.into_iter().enumerate() {
        if let Some(participant) = participant { bound.inputs.push((Input::Argument(at), vec![participant])); }
    }
    if let Some((at, those)) = group { bound.inputs.push((Input::Argument(at), those)); }
    // what the event moved is named as it last was (603.10a), and the ability can find the
    // new object it became where it went (400.7e)
    if let Some(was) = f.was
        && let Some(now) = game.became(was)
    {
        bound.became.push((was, now));
    }
    bound
}

/// Quantitative facts guaranteed by every alternative of a trigger schema (603.2).
/// Identity/cardinality schemas belong to program::input; no dummy runtime binding is
/// needed to discover targets or validate references.
pub(crate) fn bound_shape(t: &Trigger) -> Bound {
    let quantitative = match t {
        Trigger::Event { pattern, .. } => !pattern.actions.is_empty()
            && pattern.actions.iter().all(|a| !matches!(a, EventAction::IsDealtDamage { excess: true, .. }) && a.reading().is_some_and(|(verb, _)| has_amount(verb))),
        Trigger::Any(items) => !items.is_empty() && items.iter().all(|t| bound_shape(t).event_amount.is_some()),
        _ => false,
    };
    Bound { event_amount: quantitative.then_some(0), ..Default::default() }
}

/// The event schema's quantitative facts. Merely referring to an event such as attacking
/// does not bind a number. Earlier-instruction tallies are a separate reference mechanism.
fn has_amount(verb: Verb) -> bool {
    matches!(verb, Verb::DealDamage | Verb::GainLife | Verb::LoseLife | Verb::Discover)
}

/// The triggered abilities of this permanent or card that the engine watches for, each with
/// the index of its ability: what its definition keeps lowered. Nothing is lowered here.
/// The abilities its keywords are (extort, 702.101a) follow its printed ones.
fn watched(def: &CardDef) -> impl Iterator<Item = (usize, &Template)> {
    def.triggers.iter().filter_map(|r| Some((r.ability, r.result.as_ref().ok()?))).filter(|(_, t)| !t.applied)
}

impl Game {
    /// The triggered abilities this object has that the engine watches for, each with its
    /// key: those of its own definition, and those effects give it now or had given it as
    /// it last existed (603.10a), each giving its own instance.
    fn watching_before(&self, source: ObjId, membership_changed: bool, before: Option<&crate::layers::Working>, interest: Interest) -> Vec<(AbilityKey, &Template)> {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::span(crate::profile::Scope::Watchers);
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::WatcherVisits, 1);
        let origins = match before.and_then(|view| view.get(source)) {
            Some(seen) => self.origins_seen(source, seen, false),
            None => self.origins(source, View::Effective { membership_changed }),
        };
        origins.into_iter()
            .flat_map(|(origin, def)| watched(def).filter(move |(_, t)| t.watch.interested(interest))
                .map(move |(index, t)| (AbilityKey { origin: origin.clone(), index }, t)))
            .collect()
    }

    /// Add each applicable effective instance once. The caller has already checked
    /// ordinary trigger eligibility; delayed/reflexive paths do not call this (603.2d).
    fn trigger_count(&self, ctx: &Ctx, template: &Template, membership_changed: bool) -> usize {
        if template.limit == Some(crate::ability_limit::Limit::Trigger) { return 1; }
        let Some(source) = ctx.source else { return 1 };
        let mut times = 1usize;
        for modifier in ctx.permanents(self) {
            let origins = match ctx.view.and_then(|view| view.get(*modifier)) {
                Some(seen) => self.origins_seen(*modifier, seen, false),
                None => self.origins(*modifier, View::Effective { membership_changed }),
            };
            let controller = ctx.controlled_by(self, *modifier);
            for (origin, def) in origins {
                for registration in &def.trigger_modifiers {
                    let Ok(rule) = &registration.result else { continue };
                    let modifier_ctx = Ctx { view: ctx.view, ..Ctx::new(controller, *modifier).with_key(AbilityKey { origin: origin.clone(), index: registration.ability }) };
                    if rule.of.contains(self, &modifier_ctx, &NoLocals, Target::Object(source)) { times = times.saturating_add(1); }
                }
            }
        }
        times
    }

    /// Check what has happened since the last check against every triggered ability that
    /// functions: those of permanents on the battlefield (113.6), and of a permanent that just
    /// left it, which looks back (603.10a). The ones that trigger wait in APNAP order (603.3b).
    pub(crate) fn check_triggers(&mut self) {
        self.capture_triggers();
        // Immediate until-returns follow capture, never interrupt a damage result commit.
        self.begin_returns();
    }

    /// Capture unchecked events without advancing returns, procedures, SBAs or the stack.
    /// Call only after an entire simultaneous span has been closed with finish_simultaneous.
    pub(crate) fn capture_triggers(&mut self) {
        // Automatic turn/cleanup changes are semantic boundaries too; a query must not
        // manufacture their acquired-link lifetime transitions.
        self.reconcile_linked_acquisitions();
        self.reconcile_pairs();
        #[cfg(feature = "profile")]
        let _profile = crate::profile::span(crate::profile::Scope::Triggers);
        let membership_changed = self.membership_changed();
        // what is counted is noted first: "the second time" is this one (603.2)
        self.note_events();
        let events: Vec<Event> = self.log.since(self.seen).to_vec();
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::TriggerEvents, events.len());
        self.seen = self.log.end();
        // What triggered is found reading the game as it is, so that what each source is
        // asked is asked once; what finding it uses up is taken away after.
        let (mut found, spent, uses, passed) = self.triggers_since(&events, membership_changed);
        // the delayed triggered mana abilities, known before one that triggers once is gone
        let mana_delayed: Vec<std::sync::Arc<crate::program::Program>> = self.delayed.iter()
            .filter(|d| d.pattern.mana_event() && d.captured.mana_ability()).map(|d| d.captured.program().clone()).collect();
        for (at, _) in spent.iter().enumerate().rev().filter(|(_, spent)| **spent) {
            self.delayed.remove(at);
        }
        // the active player's first, then each other player's in turn order
        let n = self.players.len();
        found.sort_by_key(|t| (t.controller + n - self.active) % n);
        // a triggered mana ability resolves as soon as the mana ability that triggered it
        // does, without using the stack (605.4a): each in turn, the first found first
        let (immediate, found): (Vec<Triggered>, Vec<Triggered>) = found.into_iter()
            .partition(|t| self.mana_triggered(t) || t.delayed.is_some() && t.captured.as_ref().and_then(|c| c.as_ref().ok())
                .is_some_and(|c| mana_delayed.iter().any(|p| crate::program::same_instance(p, c.program()))));
        self.triggered.extend(found);
        for t in immediate.into_iter().rev() { self.resolve_mana_trigger(t); }
        self.ability_uses.extend(uses);
        // what a turn use an attack declaration not yet paid for spent passed over: had
        // when the declaration is reversed (733.1)
        for (used, trigger) in passed {
            for p in self.procedures.iter_mut() {
                if let crate::procedure::Procedure::AttackDeclaration(d) = p { d.passed_over(used.clone(), &trigger); }
            }
        }
        // what has been checked needs no telling apart any more
        let seen = self.seen;
        self.together.retain(|(_, to)| *to > seen);
        self.lookbacks.retain(|(_, to, _)| *to > seen);
    }

    /// Is this a triggered mana ability (605.1b): one of an object, with no target, that
    /// triggers from a mana ability and could add mana? A delayed one is told by its pattern.
    pub(crate) fn mana_triggered(&self, t: &Triggered) -> bool {
        let Some(source) = t.source.filter(|_| t.delayed.is_none()) else { return false };
        let key = AbilityKey { origin: t.origin.clone(), index: t.index };
        self.definition(source, &t.origin).is_some_and(|def| mana_trigger(def, t.index))
            && self.ability_of(source, &key).is_some()
    }


    /// Resolve a triggered mana ability at once (605.4a), as a mana ability resolves: what
    /// it adds goes to the pool, whatever is being paid.
    fn resolve_mana_trigger(&mut self, t: Triggered) {
        let source = t.source.expect("a triggered mana ability has a source");
        let captured = t.captured.clone().unwrap_or_else(|| crate::program::capture(self, t.context()));
        let Ok(invocations) = captured.and_then(|c| c.select(&[])) else { return };
        self.start_mana(source, t.controller, invocations, t.bound, false, &[], false, &[], true);
    }

    /// The triggered ability of this card's madness ability `key`, triggered by its discard
    /// into exile (702.35a): the card as it was exiled is its source, and what the ability's
    /// pattern names of the discard is bound as for any triggered ability.
    fn madness_trigger(&self, card: ObjId, key: AbilityKey, discard: &Event) -> Option<Triggered> {
        let index = self.replacements(card).find_map(|(at, r)| match r {
            crate::replace::Replacement::Madness { trigger } if at == key => Some(*trigger), _ => None,
        })?;
        let template = self.definition(card, &key.origin)?.triggers.iter()
            .find(|r| r.ability == index).and_then(|r| r.result.as_ref().ok())?;
        let controller = self.obj(card).owner;
        let ability = AbilityKey { origin: key.origin.clone(), index };
        let ctx = Ctx::new(controller, card).with_key(ability);
        facts(self, discard).into_iter().find_map(|fact| {
            let matched = template.watch.matches(self, &ctx, &fact)?;
            Some(Triggered { source: Some(card), controller, origin: key.origin.clone(), index, bound: bound_of(self, matched, &fact), delayed: None,
                captured: Some(crate::program::capture(self, ctx.clone())) })
        })
    }

    /// A rules-created subscription shares the compiled trigger and occurrence pipeline,
    /// with a controller but no object source (725.2).
    fn monarch_trigger(&self, rule: crate::monarch::Rule, controller: crate::PlayerId, event: &Event) -> Option<Triggered> {
        let definition=rule.definition();
        let key=AbilityKey { origin: crate::binding::Origin::Rule(definition), index:0 };
        let ctx=Ctx::rule(controller,key.clone());
        let template=definition.def().triggers[0].result.as_ref().expect("compiled monarch subscription");
        let bound=if let Event::Began {part}=event {
            if !template.watch.begins(self,&ctx,*part) {return None;}
            Bound::default()
        } else {
            facts(self,event).into_iter().find_map(|fact| {
                template.watch.matches(self,&ctx,&fact).map(|matched| bound_of(self,matched,&fact))
            })?
        };
        Some(Triggered {source:None,controller,origin:key.origin.clone(),index:0,bound,delayed:None,captured:Some(crate::program::capture(self,ctx))})
    }

    /// The speed rule a player with speed has (702.179d): sourceless, limited to once each
    /// turn for that player, with its intervening "if" (603.4).
    fn speed_trigger(&self, controller: crate::PlayerId, event: &Event, uses: &mut Vec<crate::ability_limit::Use>) -> Option<Triggered> {
        let definition = crate::speed::rule();
        let key = AbilityKey { origin: crate::binding::Origin::Rule(definition), index: 0 };
        let ctx = Ctx::rule(controller, key.clone());
        let template = definition.def().triggers[0].result.as_ref().expect("compiled speed rule");
        let bound = facts(self, event).into_iter().find_map(|fact| {
            template.watch.matches(self, &ctx, &fact).map(|matched| bound_of(self, matched, &fact))
        })?;
        (template.holds(self, &ctx) && template.take(self, &ctx, uses)).then(|| Triggered {
            source: None, controller, origin: key.origin.clone(), index: 0, bound, delayed: None, captured: Some(crate::program::capture(self, ctx)),
        })
    }

    /// What these events, the last the log holds, triggered, in the order found; and which
    /// of the delayed triggered abilities doing so used up (603.7b), by their place. One used
    /// up by an event is not there for the next: nothing is changed for that here.
    fn triggers_since(&self, events: &[Event], membership_changed: bool) -> (Vec<Triggered>, Vec<bool>, Vec<crate::ability_limit::Use>, Vec<(crate::ability_limit::Use, Triggered)>) {
        let mut found = Vec::new();
        // what triggered but for a turn use already spent, and that use
        let mut passed: Vec<(crate::ability_limit::Use, Option<(crate::event::Seq, (usize, Vec<Option<Target>>))>, Triggered)> = Vec::new();
        let mut uses = Vec::new();
        // a batch: one event (a simultaneous span of the log), and what else took part in it
        // ("to a player": once for each player dealt damage)
        let mut grouped: Vec<(ObjId, AbilityKey, crate::event::Seq, (usize, Vec<Option<Target>>), std::ops::Range<usize>)> = Vec::new();
        // the same, of the delayed triggered abilities: which one, the event, what was said
        let mut delayed_grouped: Vec<(usize, crate::event::Seq, (usize, Vec<Option<Target>>), std::ops::Range<usize>)> = Vec::new();
        let mut spent = vec![false; self.delayed.len()];
        // the objects whose abilities could function: permanents (113.6), and cards in
        // graveyards, where an ability that moves its card out of one functions (113.6m)
        let graveyards: Vec<ObjId> = self.players.iter().flat_map(|p| p.graveyard.iter().copied()).collect();
        let exile: Vec<ObjId> = self.exile.iter().copied().filter(|object| self.obj(*object).hidden.is_none()).collect();
        // Each incarnation sees only events after its arrival (400.7); setup objects
        // exist from the beginning. Arrival belongs to the object, not retained history.
        let first = self.seen.0 - events.len();
        // The first change supplies the monarch before this event batch. Keep event-time
        // ownership even if an effect changed the designation again before delivery.
        let mut monarch=events.iter().find_map(|e| if let Event::MonarchChanged {previous,..}=e {Some(*previous)} else {None}).unwrap_or(self.monarch);
        for (at, e) in events.iter().enumerate() {
            if let Event::MonarchChanged {player,..}=e {monarch=*player;}
            if let Some(controller)=monarch {
                let rule=match e {
                    Event::Began {part: ir::Part::End} => Some(crate::monarch::Rule::EndStep),
                    Event::DamageDealt {combat:true,to:Target::Player(player),..} if *player==controller => Some(crate::monarch::Rule::CombatDamage),
                    _=>None,
                };
                if let Some(rule)=rule && let Some(ability)=self.monarch_trigger(rule,controller,e) {found.push(ability);}
            }
            for controller in (0..self.players.len()).filter(|p| !self.players[*p].lost && self.players[*p].speed.is_some()) {
                if let Some(ability) = self.speed_trigger(controller, e, &mut uses) { found.push(ability); }
            }

            let at = crate::event::Seq(first + at);
            let graveyards: Vec<ObjId> = graveyards.iter().copied()
                .filter(|g| self.obj(*g).arrived.is_none_or(|from| from < at)).collect();
            let exile: Vec<ObjId> = exile.iter().copied()
                .filter(|object| self.obj(*object).arrived.is_none_or(|from| from < at)).collect();
            let emblems: Vec<ObjId> = self.command.iter().copied()
                .filter(|o| self.obj(*o).arrived.is_none_or(|from| from < at)).collect();
            if let Event::Began { part } = e {
                for source in self.battlefield.iter().chain(&emblems).chain(&graveyards).chain(&exile).copied().collect::<Vec<_>>() {
                    let watching = self.watching_before(source, membership_changed, None, Interest::Step);
                    if watching.is_empty() { continue; }
                    let controller = self.controller(source);
                    let ctx = Ctx::new(controller, source);
                    for (key, template) in &watching {
                        let (ctx, AbilityKey { origin, index }) = (ctx.clone().with_key(key.clone()), key.clone());
                        if self.functions_as(source, key.clone()) && template.watch.begins(self, &ctx, *part) && template.holds(self, &ctx) && template.take(self, &ctx, &mut uses) {
                            // the step is the active player's: "that player"
                            let mut bound = Bound::default();
                            bound.inputs.push((crate::program::input::Input::Subject, vec![Target::Player(self.active)]));
                            found.extend(std::iter::repeat_n(Triggered { source: Some(source), controller, origin, index, bound, delayed: None, captured: Some(crate::program::capture(self, ctx.clone())) }, self.trigger_count(&ctx, template, membership_changed)));
                        }
                    }
                }
            }
            if let Event::Began { part } = e {
                for (index, delayed) in self.delayed.iter().enumerate() {
                    if spent[index] { continue; }
                    let ctx = Ctx { ability: delayed.bound.linked_ability.clone(), ..Ctx::new(delayed.controller,delayed.source) };
                    if delayed.pattern.begins(self,&ctx,*part) {
                        found.push(Triggered { source: Some(delayed.source), controller: delayed.controller,
                            origin: crate::binding::Origin::Own, index: 0, bound: delayed.bound.clone(),
                            delayed: Some(delayed.captured.identity()), captured: Some(Ok(delayed.captured.clone())) });
                        spent[index] = true;
                    }
                }
            }
            // a card discarded into exile by its madness ability: that ability's trigger
            // triggers (702.35a), whatever zone it is in, controlled by the card's owner
            if let Event::Discarded { card, madness: Some(key), .. } = e {
                found.extend(self.madness_trigger(*card, key.clone(), e));
            }
            for fact in facts(self, e).into_iter().map(|f| f.at(at)) {
                // a leaves-the-battlefield ability looks back in time (603.10a): what left
                // is seen by its own abilities and by those of the permanents that left the
                // battlefield at the same time as it
                let before = fact.was.and_then(|_| self.lookbacks.iter().rev()
                    .find(|(from, to, _)| *from <= at && at < *to).map(|(_, _, before)| before));
                let mut sources = before.and_then(crate::layers::Working::battlefield)
                    .unwrap_or(&self.battlefield).to_vec();
                if before.is_none() && let Some(was) = fact.was {
                    sources.extend(self.left_with(was));
                }
                sources.extend(graveyards.iter().copied());
                sources.extend(exile.iter().copied());
                sources.extend(emblems.iter().copied());
                // Self-cast abilities function on this spell, not on the battlefield
                // (113.6k). The event supplies it; no all-stack scan is needed.
                if let Event::Cast { spell, .. } = e { sources.push(*spell); }
                // what connived or explored and left: its own "whenever this …" sees it (701.44c)
                if let Event::Connived{object,..} | Event::Explored{object,..}=e && self.obj(*object).zone==Zone::Gone {sources.push(*object);}
                for source in sources {
                    let watching = self.watching_before(source, membership_changed, before, Interest::Event(fact.verb));
                    if watching.is_empty() { continue; }
                    let controller = before.and_then(|view| view.get(source))
                        .map_or_else(|| self.controller(source), |seen| seen.controller);
                    let occurrence = Ctx::new(controller, source);
                    let ctx = Ctx { view: before, ..occurrence.clone() };
                    for (key, template) in &watching {
                        let (ctx, AbilityKey { origin, index }) = (ctx.clone().with_key(key.clone()), key.clone());
                        let departed_conniver=matches!(e,Event::Connived{object,..} | Event::Explored{object,..} if *object==source && self.obj(source).zone==Zone::Gone);
                        if (!departed_conniver || template.watch.self_connive()) && self.functions_as(source, key.clone())
                            && let Some(matched) = template.watch.matches(self, &ctx, &fact)
                        {
                            let bound = bound_of(self, matched.clone(), &fact);
                            if !template.intervening.as_ref().is_none_or(|c| c.evaluate(self, &fact.context(ctx.clone()), &bound)) { continue; }
                            if template.watch.grouped() {
                                let batch = self.together.iter().rev().find(|(from, to)| *from <= at && at < *to).map_or(at, |(from, _)| *from);
                                // what the phrase said tells batches apart; of what it left
                                // unsaid ("dealt damage" by whichever sources) each is kept
                                let said = (matched.action, matched.participants[..matched.said].to_vec());
                                if let Some((_, _, _, _, index)) = grouped.iter().find(|(o, k, b, others, _)| *o == source && k == key && *b == batch && *others == said) {
                                    join_batch(&mut found[index.clone()], &matched, &fact, &bound);
                                    continue;
                                }
                                grouped.push((source, key.clone(), batch, (matched.action, matched.participants[..matched.said].to_vec()), found.len()..found.len()));
                            }
                            let trigger = Triggered { source: Some(source), controller, origin, index, bound, delayed: None, captured: Some(crate::program::capture(self, occurrence.clone().with_key(key.clone()))) };
                            if !template.take(self, &ctx, &mut uses) {
                                if template.watch.grouped() { grouped.pop(); }
                                if let Some(limit) = template.limit && let Some(used) = crate::ability_limit::Use::of(&ctx, limit) {
                                    // of one event, one; the rest of its subjects join it
                                    let batch = template.watch.grouped().then(|| (
                                        self.together.iter().rev().find(|(from, to)| *from <= at && at < *to).map_or(at, |(from, _)| *from),
                                        (matched.action, matched.participants[..matched.said].to_vec())));
                                    match passed.iter_mut().find(|(u, b, _)| *u == used && batch.is_some() && *b == batch) {
                                        Some((_, _, prior)) => join_batch(std::slice::from_mut(prior), &matched, &fact, &trigger.bound),
                                        None => passed.push((used, batch, trigger)),
                                    }
                                }
                                continue;
                            }
                            // A single counter batch may contain several triggering occurrences
                            // (603.2c), but a trigger limit permits only the first (101.1).
                            let occurrences = if template.limit == Some(crate::ability_limit::Limit::Trigger) { 1 } else { matched.times as usize };
                            let times = occurrences.saturating_mul(self.trigger_count(&ctx, template, membership_changed));
                            found.extend(std::iter::repeat_n(trigger, times));
                            if template.watch.grouped() { grouped.last_mut().unwrap().4.end = found.len(); }
                        }
                    }
                }
                // the delayed triggered abilities effects have made (603.7): each watches
                // for its event happening to the particular objects it was made of
                for (index, d) in self.delayed.iter().enumerate() {
                    let ctx = Ctx { view: before, ability: d.bound.linked_ability.clone(), ..Ctx::new(d.controller, d.source) };
                    // of one or more of them, the rest of an event it already triggered on
                    // join that trigger, even once it is spent (603.2c, 603.7c)
                    if d.pattern.grouped() {
                        let batch = self.together.iter().rev().find(|(from, to)| *from <= at && at < *to).map_or(at, |(from, _)| *from);
                        if let Some(matched) = d.pattern.described(self, &ctx, &fact) {
                            let said = (matched.action, matched.participants[..matched.said].to_vec());
                            let bound = Bound { linked_ability: d.bound.linked_ability.clone(), ..bound_of(self, matched.clone(), &fact) };
                            if let Some((_, _, _, range)) = delayed_grouped.iter().find(|(i, b, s, _)| *i == index && *b == batch && *s == said) {
                                join_batch(&mut found[range.clone()], &matched, &fact, &bound);
                                continue;
                            }
                            if spent[index] { continue; }
                            let start = found.len();
                            found.push(Triggered { source: Some(d.source), controller: d.controller, origin: crate::binding::Origin::Own, index: 0, bound, delayed: Some(d.captured.identity()), captured: Some(Ok(d.captured.clone())) });
                            delayed_grouped.push((index, batch, said, start..found.len()));
                            if d.once { spent[index] = true; }
                        }
                        continue;
                    }
                    let at = index;
                    if spent[at] { continue; }
                    if let Some(matched) = d.pattern.described(self, &ctx, &fact) {
                        // what its words name is of this event; the rest, of the text that made it
                        let times = if d.once { 1 } else { matched.times };
                        let bound = Bound { linked_ability: d.bound.linked_ability.clone(), ..bound_of(self, matched, &fact) };
                        found.extend(std::iter::repeat_n(Triggered { source: Some(d.source), controller: d.controller, origin: crate::binding::Origin::Own, index: 0, bound, delayed: Some(d.captured.identity()), captured: Some(Ok(d.captured.clone())) }, times as usize));
                        if d.once { spent[at] = true; }
                    } else if let Some(times) = d.pattern.times_on(self, &ctx, &fact, &d.subjects) {
                        let times = if d.once { 1 } else { times };
                        // what the text that made it had named, and where what the event
                        // moved has gone (400.7e)
                        let mut bound = d.bound.clone();
                        if let Some(was) = fact.was
                            && let Some(now) = self.became(was)
                        {
                            bound.became.push((was, now));
                        }
                        found.extend(std::iter::repeat_n(Triggered { source: Some(d.source), controller: d.controller, origin: crate::binding::Origin::Own, index: 0, bound, delayed: Some(d.captured.identity()), captured: Some(Ok(d.captured.clone())) }, times as usize));
                        // its first matching occurrence consumes a single-use registration (603.7b)
                        if d.once {
                            spent[at] = true;
                        }
                    }
                }
            }
        }
        // State triggers (603.8): each triggers as soon as the game is as it says, and not
        // again while it waits to be put on the stack or is there.
        for source in self.battlefield.iter().chain(&graveyards).copied().collect::<Vec<_>>() {
            let watching = self.watching_before(source, membership_changed, None, Interest::State);
            if watching.is_empty() { continue; }
            let controller = self.controller(source);
            let ctx = Ctx::new(controller, source);
            for (key, template) in &watching {
                let (ctx, AbilityKey { origin, index }) = (ctx.clone().with_key(key.clone()), key.clone());
                if !self.functions_as(source, key.clone()) || !template.holds(self, &ctx) { continue; }
                let Some(of) = template.watch.is_so(self, &ctx) else { continue };
                let waiting = self.triggered.iter().chain(&found).any(|t| (t.source, t.origin.clone(), t.index, t.delayed) == (Some(source), origin.clone(), index, None))
                    || self.stack.iter().any(|on| {
                        // the ability it is, by the key it was captured with: one its
                        // source was given is not its own of the same index
                        let on = self.obj(*on);
                        on.ability.is_some_and(|(of, _)| of == Some(source)) && match &on.invocations {
                            Some(Ok(invocations)) => invocations.iter().any(|i| i.triggered && i.ctx.ability == Some(key.clone())),
                            _ => on.ability == Some((Some(source), index)),
                        }
                    });
                if !waiting && template.take(self, &ctx, &mut uses) {
                    let mut bound = Bound::default();
                    if let Some(of) = of { bound.inputs.push((crate::program::input::Input::Subject, vec![of])); }
                    found.extend(std::iter::repeat_n(Triggered { source: Some(source), controller, origin, index, bound, delayed: None, captured: Some(crate::program::capture(self, ctx.clone())) }, self.trigger_count(&ctx, template, membership_changed)));
                }
            }
        }
        (found, spent, uses, passed.into_iter().map(|(used, _, trigger)| (used, trigger)).collect())
    }
}

#[cfg(test)]
mod tests;
