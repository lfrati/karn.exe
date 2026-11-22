//! The game state as plain data, and the actions of the lexicon performed on it.
//!
//! Objects live in one arena and are named by [`ObjId`]. An object that changes zones becomes a
//! new object (CR 400.7): the old id stays as a memory of what left, the new one is what
//! arrived. Each action is implemented once here and logs the [`Event`] it is.

use crate::card::CardDef;
use crate::combat::Combat;
use crate::event::{Event, Target};
use crate::replace::{Choices, Entry};
use crate::trigger::Fact;
use ir::mana::ManaSymbol;
use ir::vocab::SubtypeKind;
use ir::{CardType, Color, Needs, Part, Role, Sort, Subtype, Supertype, Verb};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type PlayerId = usize;

/// Opaque game-local incarnation handle: low 32 bits name a storage slot, high 32 bits
/// its generation. Hosts round-trip the whole value; a slot number alone is not an identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjId(pub u64);

/// Accepted casting choices (601.2b), retained separately from completed payment.
/// Copies inherit the choices, but do not cast or pay again (707.10).
#[derive(Debug, Clone)]
pub(crate) struct CastAnnouncement {
    pub route: crate::announcement::CastRoute,
    pub permission: Option<(crate::cast_permission::Use, Option<crate::ability::FrozenDefinition>)>,
    pub payment: Option<CastPayment>,
    /// The keyword ability an effect gave the card in a hand (613.1f) whose alternative cost
    /// it was cast for, as it was then: "Warp {2}{R}" (702.185a).
    pub given_alternative: Option<std::sync::Arc<ir::token::KeywordInstance>>,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct CastPayment {
    pub convoke: Vec<(ObjId, Option<Color>)>,
    pub delve: Vec<ObjId>,
}

/// A physical card across incarnations (903.3). Never sent to players or used as a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CardId(u64);

mod arena;
mod retention;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Stack,
    Exile,
    /// Emblems live in this public zone (114.2, 400.2).
    Command,
    /// It left its zone: this id is what it was (400.7).
    Gone,
}

/// What an object is, copied from its card or token description when it comes to be. Nothing
/// changes this baseline except copying; layers derive effective characteristics from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chars {
    pub name: String,
    pub supertypes: Vec<Supertype>,
    pub types: Vec<CardType>,
    pub subtypes: Vec<Subtype>,
    pub colors: Vec<Color>,
    pub pt: Option<(i32, i32)>,
    pub mana_value: u32,
    /// The keyword abilities a token was created with ("with flying", 111.4); a card's are
    /// in its text.
    pub keywords: Vec<ir::Keyword>,
    /// Of a predefined token (111.10): the rules' definition that is its text (111.3), whose
    /// abilities [`Game::def`] finds ([`crate::token`]). None for a card and a described token.
    pub predefined: Option<ir::Predefined>,
}

#[derive(Debug, Clone)]
pub struct Object {
    /// Which part of a split card this spell is; the departed stack object keeps this as LKI.
    pub cast_as: crate::card::CastAs,
    /// What this permanent is attached to (303.4b). Kept on departed objects as last known
    /// information; use Game::attached_to to exclude an object that left its zone.
    pub attached_to: Option<Target>,
    pub id: ObjId,
    /// The physical card index; None for a token, emblem, or cardless stack ability.
    pub card: Option<usize>,
    pub(crate) card_id: Option<CardId>,
    pub chars: Chars,
    /// Immutable emblem text, independent of the effect that created it (114.3).
    pub(crate) emblem: Option<crate::ability::Template>,
    pub(crate) copy_acquisition: Option<std::sync::Arc<crate::copy::Acquisition>>,
    pub(crate) copied: Option<crate::copy::Values>,
    pub(crate) copy_effects: crate::copy::Effects,
    /// Immutable creating baseline for tokens and card copies that can become tokens
    /// (111.3, 111.13, 707.8a). Separate from later copying effects.
    pub(crate) token_faces: Option<std::sync::Arc<crate::copy::Faces>>,
    /// A copy of a card an effect made to be cast (707.12), in the zone it was made in or on
    /// the stack as a spell. It is not a card and not a token; anywhere else it ceases to
    /// exist (707.10a). The permanent it becomes is a token (111.13).
    pub(crate) card_copy: bool,
    /// Exact stack occurrence copied (707.10); independent of name and targets.
    pub(crate) stack_copy_of: Option<ObjId>,
    /// Of a permanent: which face is up (708.2). Kept on what it last was once it has left
    /// the battlefield; what it becomes elsewhere is face up.
    pub face: crate::face::Face,
    /// Of an exiled card: exiled face down (406.3), and who may look at it. Kept on what it
    /// last was once it has left exile; what it becomes elsewhere is face up.
    pub hidden: Option<crate::face::Hidden>,
    /// Of an exiled card: foretold, by whom and on which turn (702.143). What it becomes
    /// elsewhere is not.
    pub foretold: Option<crate::foretell::Foretold>,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
    pub damage: i32,
    /// Dealt damage by a source with deathtouch since state-based actions were last checked
    /// (702.2b).
    pub deathtouched: bool,
    pub counters: BTreeMap<String, u32>,
    /// Timestamp shared by counters of a kind (613.7c).
    pub(crate) counter_stamps: BTreeMap<String, u32>,
    /// A spell's or ability's targets, one group per "target" in its text, in order (601.2c).
    pub targets: Vec<Vec<Target>>,
    /// What each target of a group that divides or distributes an amount gets, in the order
    /// of `targets` (601.2d); empty for a group that divides nothing.
    pub division: Vec<Vec<u32>>,
    /// On the stack, an ability rather than a spell (113.3c): the object whose ability it is
    /// and which of that card's abilities.
    pub ability: Option<(Option<ObjId>, usize)>,
    /// Of an ability on the stack: a delayed or reflexive triggered ability an effect made
    /// (603.7, 603.12), whose instructions are no ability of its source.
    pub(crate) delayed: bool,
    /// Immutable executable identity captured at announcement, independent of the live source.
    pub(crate) invocations: Option<crate::Res<Vec<crate::program::Invocation>>>,
    /// The turn in which it came under its controller's control; 0 for what a game was set up
    /// with. A creature attacks only if that was before its controller's turn began (302.6).
    pub since: u32,
    /// The turn in which it came to be in its zone, as the object it is (400.7); 0 for what a
    /// game was set up with. A permanent "entered this turn" if that turn is this one.
    pub(crate) entered: u32,
    /// Once-only activation uses of this incarnation (602.5b); not copiable or reset each turn.
    pub(crate) activation_uses: Vec<crate::binding::AbilityKey>,
    /// Of an ability on the stack: what its cost or trigger condition named (608.2k).
    pub(crate) bound: Bound,
    /// Its timestamp: received as it entered its zone (613.7d). A continuous effect of a static
    /// ability has the timestamp of the object the ability is on (613.7a).
    pub stamp: u32,
    /// Of a permanent that has left the battlefield: what it was as it last existed there,
    /// its last known information (608.2h, 113.7a).
    pub(crate) last: Option<crate::layers::Seen>,
    /// Inverse attachment facts of this incarnation at departure (301.5a, 303.4b, 608.2h).
    pub(crate) last_attachments: [bool; 2],
    /// Of a spell: its flashback or harmonize cost was paid, so it is exiled instead of being
    /// put anywhere else when it leaves the stack (702.34a, 702.180a).
    pub(crate) flashback: Option<(crate::functions::Scope, Zone)>,
    /// Of a spell cast for its harmonize cost: the creature its caster chose to tap as it is
    /// paid, whose power reduces its generic mana (702.180b).
    pub(crate) harmonized: Option<ObjId>,
    /// Of a spell cast under an effect's offer that says so: exiled instead of being put into
    /// its owner's graveyard, while it is of these types (Bilbo, Thief in the Night).
    pub(crate) cast_rider: Option<crate::permission::Rider>,
    /// Of a double-faced permanent: its back face is up (701.27g, 712.8e). Kept on what it
    /// last was once it has left the battlefield; what it becomes there is its front face.
    pub transformed: bool,
    /// The time it last transformed; 0 if it has not. An ability of the permanent itself
    /// transforms it only if it has not transformed since that ability was put on the stack
    /// (701.27f).
    pub(crate) transformed_at: u32,
    /// Its level, a designation the permanent keeps even if it stops being a Class; none
    /// until one is given, read as 1 (716.2b, 716.2d). Not copiable; a new object has none.
    pub level: Option<u32>,
    /// Of a Room permanent, its "left half unlocked" and "right half unlocked" designations
    /// (709.5c). Not copiable; a new object has neither.
    pub unlocked: [bool; 2],
    /// Its "monstrous" and "renowned" designations, kept until it leaves the battlefield
    /// (701.37b, 702.112b). Not copiable; a new object has neither.
    pub monstrous: bool,
    pub renowned: bool,
    /// Persistent noncopiable designation, reset by a new incarnation (719.3b).
    pub solved: bool,
    /// Noncopiable designation and exact occurrence links (722.3a–c).
    pub prepared: bool,
    pub(crate) prepared_copy: Option<ObjId>,
    pub(crate) prepare_source: Option<ObjId>,
    /// Saddled (702.171b): until the end of the turn ([`Game::cleanup`]) or it leaves the
    /// battlefield, a new object not being so; not copiable.
    pub saddled: bool,
    /// Of a spell that was cast: the zone its card was in as it was (601.2a). A copy of a
    /// spell is put on the stack without being cast (707.10) and has none.
    pub(crate) cast_from: Option<Zone>,
    /// Caster of this spell and the permanent it becomes (400.7d). Cleared on other
    /// zone changes; neither the owner nor a later controller is necessarily its caster.
    pub(crate) cast_by: Option<PlayerId>,
    /// Of a spell or activated ability on the stack whose cost has {X}: the value announced
    /// for it (107.3a). Elsewhere there is none, and X is 0 (107.3g).
    pub x: Option<u32>,
    /// Of a permanent a resolving spell became: the X announced for that spell, which its
    /// own enters-the-battlefield triggered abilities and replacement effects read; the
    /// permanent's own X stays 0 (107.3m).
    pub spell_x: Option<u32>,
    /// Of a spell, and of the permanent it becomes: how many times its controller declared
    /// they would pay its kicker cost as they cast it (601.2b, 702.33d).
    pub kicked: u32,
    /// Of a spell, and of the permanent it becomes: its controller declared its optional
    /// additional cost paid as it was cast (601.2b, 118.8b): it was bargained (702.166b), its
    /// offspring or casualty cost was paid (702.175a, 702.153a), or the cost its text prints.
    pub optional_paid: bool,
    /// Of a spell, and of the permanent it becomes: the row of the alternative cost it was
    /// cast for, if any (118.9, 601.2b): "if its evoke cost was paid" (702.74a).
    pub(crate) alternative: Option<usize>,
    /// Buyback cost was announced and paid for this retained spell (702.27a).
    pub buyback_paid: bool,
    /// All modes were chosen with the optional entwine cost (702.42).
    pub entwined: bool,
    /// Payments of the one supported replicate cost (702.56a), copied with the spell.
    pub replicated: u32,
    /// Of a spell: the opponent its controller chose for its gift as they cast it, an
    /// additional cost; its gift was promised (702.174a, 702.174k).
    pub gift: Option<PlayerId>,
    /// Of a spell or ability on the stack: the alternative of each choice among alternatives its
    /// cost has that its controller announced paying, as it is said (601.2b, 602.2b). Public.
    pub announced: Vec<crate::cost::SelectedBranch>,
    pub(crate) cast_announcement: Option<Box<CastAnnouncement>>,
    /// Of an object that is no more: the zone it was last in, where those who could see it
    /// there saw it (400.2).
    pub(crate) left: Option<Zone>,
    /// Sequence of the zone change that created this incarnation (400.7). Setup objects
    /// have no arrival event and exist from the beginning of the game.
    pub(crate) arrived: Option<crate::event::Seq>,
    /// Its immediate successor in a public zone, for the exceptions to 400.7 that may
    /// find it there (400.7e). This is not a link through subsequent zone changes.
    pub(crate) successor: Option<ObjId>,
    /// Of an ability on the stack that is a delayed triggered ability (603.7): its
    /// instructions, which are of no ability its source has.
    /// Typed choices, by linked producer and chooser (607.2d). A new incarnation starts
    /// empty (400.7); the departed object retains its records for effects already produced.
    pub choices: Vec<crate::binding::Record>,
    /// Of a spell: mana was spent to cast it whose text says "that spell can't be countered"
    /// (106.6). A "can't" takes precedence over an instruction to counter it (101.2).
    pub uncounterable: bool,
    /// Actual mana paid for this spell, not its printed cost (601.2h).
    pub(crate) mana_paid: PaidMana,
    /// Cards actually exiled by linked casting costs (607.2q, 400.7d).
    pub(crate) cast_exiled: Vec<(crate::binding::AbilityKey, ObjId)>,
    /// Of a modal spell or ability on the stack: the modes chosen for it, by their place among
    /// the modes printed (700.2a–b). Its instructions are theirs.
    pub modes: Vec<usize>,
}

/// A triggered ability that has triggered and waits to be put on the stack (603.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triggered {
    /// Rule-created abilities such as the monarch triggers have no source (725.2).
    pub source: Option<ObjId>,
    pub controller: PlayerId,
    /// The definition the ability belongs to: its source's own, or one it was given.
    pub origin: crate::binding::Origin,
    /// Which of that definition's abilities.
    pub index: usize,
    /// What the event that made it trigger was about: what its effect's "it" and "that
    /// creature" refer to.
    pub bound: Bound,
    /// Some: a delayed or reflexive triggered ability an effect made (603.7/603.12), with its instructions;
    /// `index` is then of no ability of the source.
    pub delayed: Option<crate::program::ProgramId>,
    /// Captured when this ability triggers, before it waits for order/targets or SBAs.
    pub(crate) captured: Option<crate::Res<crate::program::Captured>>,
}

impl Triggered {
    pub(crate) fn context(&self) -> crate::select::Ctx<'static> {
        let key = crate::binding::AbilityKey { origin: self.origin.clone(), index: self.index };
        let ctx = self.source.map_or_else(|| crate::select::Ctx::rule(self.controller, key), |source| crate::select::Ctx::new(self.controller, source));
        if let Some(Ok(captured)) = &self.captured { captured.context() }
        else { ctx.with_key(crate::binding::AbilityKey { origin: self.origin.clone(), index: self.index }) }
    }
}

/// A delayed triggered ability (603.7): made by an effect as it was followed, to trigger
/// later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delayed {
    /// The spell that made it, or the source of the ability that did (603.7d–e).
    pub source: ObjId,
    /// The player who controlled that spell or ability as it resolved.
    pub controller: PlayerId,
    /// What it watches for, happening to these: the particular objects its words named as it
    /// was made (603.7c).
    pub(crate) pattern: crate::trigger::DelayedPattern,
    pub(crate) subjects: Vec<Target>,
    /// Its instructions, and what the text that made it had named.
    pub(crate) captured: crate::program::Captured,
    pub(crate) bound: Bound,
    pub(crate) lasts: Lasts,
    /// Explicit next occurrence, or the default when no duration is stated (603.7b).
    pub(crate) once: bool,
}

/// How long a delayed triggered ability watches (603.7b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lasts {
    /// No expiry is stated. Consumption is independent of this lifetime.
    Indefinite,
    /// "this turn", "until end of turn": expires at the cleanup step.
    ThisTurn,
    /// "until your next turn": expires as its controller's next turn begins.
    UntilYourNextTurn,
}

/// What a text has named so far, for the words that refer back to it: "it", "that creature",
/// "that player", "they". An ability's effect refers to what its cost or trigger condition
/// referred to (608.2k), and an instruction to what the instructions before it named (608.2c);
/// what is named stays that object whatever becomes of it (113.7a, 608.2h).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bound {
    pub(crate) inputs: Vec<(crate::program::input::Input, Vec<Target>)>,
    /// Revealed for as long as this activated occurrence remains on the stack (702.49b).
    pub(crate) revealed: Vec<ObjId>,
    /// The creating ability for a delayed instruction (603.7, 607.2d).
    pub(crate) linked_ability: Option<crate::binding::AbilityKey>,
    /// The quantity of the event that caused this trigger (603.2, 608.2h): "that much" or
    /// "that many". It belongs to this occurrence, not the source's current statistics
    /// or the latest event in the log. None means the trigger does not bind a quantity.
    pub(crate) event_amount: Option<u32>,
    /// Of an ability that triggered when an object moved to a public zone: the object named,
    /// and the new object it became there, which the ability can find (400.7e). And of a
    /// spell or ability whose cost or effect moved an object to a public zone, which its
    /// effect can find (400.7j).
    pub(crate) became: Vec<(ObjId, ObjId)>,
    pub(crate) cost_exiled: Vec<(crate::binding::AbilityKey, ObjId)>,
    /// Of a crew ability: the creatures tapped to pay its cost, which crewed the Vehicle
    /// (702.122c); it becomes crewed as the ability resolves (702.122e).
    pub(crate) crew: Option<Vec<ObjId>>,
}

/// What the game is waiting for a player to decide. While one is pending the only legal move
/// is that player's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The saved payment needs correction after an intervening mana/replacement choice.
    Payment { player: PlayerId, source: ObjId, reason: String },
    /// Choose the targets of a triggered ability as it goes on the stack (603.3d): one group
    /// per "target" of its text. `modes` retains a preceding mode answer (700.2c).
    Targets { player: PlayerId, ability: Triggered, groups: usize, modes: Vec<usize> },
    /// Choose the modes of a modal triggered ability as it goes on the stack (603.3c), and
    /// then the targets of the modes chosen (700.2c).
    Modes { player: PlayerId, ability: Triggered },
    /// Put the abilities that have triggered on the stack in an order of the player's choice
    /// (603.3b): these, which that player controls.
    Order { player: PlayerId, abilities: Vec<Triggered> },
    /// Discard down to the maximum hand size in the cleanup step (514.1): which `count` cards.
    Discard { player: PlayerId, count: usize },
    /// Declare whether to take a mulligan (103.5).
    Mulligan { player: PlayerId },
    /// Having taken a mulligan, put `count` cards of the new hand on the bottom of the
    /// library in any order (103.5).
    Bottom { player: PlayerId, count: usize },
    /// Declare attackers (508.1): which creatures attack, and what each attacks.
    Attackers { player: PlayerId },
    /// Declare blockers (509.1): which creatures block, and which attacking creature each.
    Blockers { player: PlayerId },
    /// Divide a blocked creature's combat damage among the creatures blocking it (510.1c), and
    /// with trample what it is attacking (702.19b).
    Divide { player: PlayerId, attacker: ObjId, among: Vec<Target>, amount: u32 },
    /// A choice an effect offers while it is applied (608.2d): the instructions of the spell
    /// or ability that is resolving wait for it.
    Choose { player: PlayerId, choice: Choice },
}

/// A feasible alternative action, including the resources it would require. Printed indices
/// remain stable when another alternative is impossible. These objects are shown only to
/// the player choosing, just as in the following object-selection request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOption {
    pub index: usize,
    pub body: ActionOptionBody,
}

/// Where an object put into another zone goes (400.3): a zone, or a place in its owner's
/// library (401.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Zone(Zone),
    Library(ir::action::LibraryPosition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOptionBody {
    Counters { on: ObjId, kind: String, count: u32 },
    Objects { verb: Verb, among: Vec<ObjId>, count: usize },
    Create { tokens: Vec<(ir::Predefined, u32)> },
}

/// The bounded successful-completion replacement roles, scoped by the resolving spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationEffect { Buyback, Rebound, CastRider, Omen, Flashback, GraveyardReplacement(usize) }
impl DestinationEffect {
    pub fn name(self) -> String {
        match self { Self::Buyback=>"buyback".into(), Self::Rebound=>"rebound".into(), Self::CastRider=>"cast_rider".into(),
            Self::Omen=>"omen".into(), Self::Flashback=>"flashback".into(), Self::GraveyardReplacement(i)=>format!("graveyard:{i}") }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        match name { "buyback"=>Some(Self::Buyback), "rebound"=>Some(Self::Rebound), "cast_rider"=>Some(Self::CastRider),
            "omen"=>Some(Self::Omen), "flashback"=>Some(Self::Flashback),
            _=>name.strip_prefix("graveyard:").and_then(|i|i.parse().ok()).map(Self::GraveyardReplacement) }
    }
}

/// What an effect asks a player to choose as it is applied (608.2d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPolicy { RequiredSingle, ChooseNew }

/// What a [`Choice::Pay`] of a waterbend cost lets its payer tap (701.67a): up to `most` of
/// `among`, each paying {1} of its `generic` mana.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Waterbend { pub among: Vec<ObjId>, pub generic: u32, pub most: u32 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Public destination choice; relative ordering is a separate private question (401.4).
    LibraryDestination { cards: Vec<ObjId>, positions: Vec<ir::action::LibraryPosition> },
    /// Existing stack object: required single replacement or optional new targets (115.7).
    ChangeTargets { object: ObjId, original: Vec<Vec<Target>>, slots: Vec<crate::legal::Slot>, policy: TargetPolicy },
    SpellDestination { on: ObjId, among: Vec<DestinationEffect> },
    /// The owner may put this commander into the command zone (903.9).
    CommandZone { object: ObjId },
    /// New targets for one spell copy (707.10c). Counts and the original division stay fixed.
    Retarget { copy: ObjId, original: Vec<Vec<Target>>, slots: Vec<crate::legal::Slot> },
    /// Choose one feasible action by its printed position, or decline an optional choice.
    Alternative { among: Vec<ActionOption>, optional: bool },
    /// Look at the whole library, choosing matching cards. A qualified hidden-zone
    /// search may find fewer, even none (701.23a–d); other players see neither list.
    Search { cards: Vec<ObjId>, among: Vec<ObjId>, min: usize, max: usize },
    /// Select objects without targeting or revealing a hidden zone.
    Select { among: Vec<ObjId>, min: usize, max: usize },
    /// What an Aura entering without being cast will enchant (303.4f). This does not target.
    Attach { aura: ObjId, among: Vec<Target> },
    /// Any subset of counter-bearing players and permanents, without targeting (701.34a).
    Holders { verb: Verb, among: Vec<Target> },
    /// What a creature put onto the battlefield attacking attacks: a defending player, or a
    /// planeswalker one controls (508.4). This does not target. `nth` of `of`: which of the
    /// creatures entering attacking together this is for, from 1.
    Defender { among: Vec<Target>, nth: usize, of: usize },
    /// "You may …": whether to do it. A private, impossible action still asks,
    /// with only a decline legal, so the question does not disclose hidden qualities.
    May { possible: bool },
    /// Consent and the conditional object selection for one optional action (608.2d).
    OptionalObjects { verb: Verb, to: Option<Placement>, among: Vec<ObjId>, count: usize },
    /// "You may cast it without paying its mana cost" (608.2g): cast one of these cards now,
    /// with the move a spell is cast with, or decline what is left with [`Answer::No`].
    Cast { cards: Vec<ObjId> },
    /// Offer payment with explicit [`Answer::Pay`] instructions, or decline (118.12).
    /// `life` must be affordable (119.4); `waterbend` gives eligible objects (701.67a).
    Pay { cost: ir::mana::ManaCost, life: u32, waterbend: Option<Waterbend> },
    /// A selection constrained by a property of the whole chosen group (701.59a).
    ConstrainedObjects { verb: Verb, among: Vec<ObjId>, selection: crate::cost::Selection },
    /// Which `count` of these to do the action to: the cards a player discards (701.9b), the
    /// permanents a player sacrifices (701.21a). `to`: where they are moved, for a zone change
    /// ("return a land you control to its owner's hand", "put a card on top of their library").
    Objects { verb: Verb, to: Option<Placement>, among: Vec<ObjId>, count: usize },
    /// Owner-selected simultaneous arrival order (401.4, 404.3), top first within
    /// the incoming block. This purpose is distinct from selecting a set of objects.
    OrderCards { to: Placement, cards: Vec<ObjId> },
    /// Which mana to add, of what the text offers: "{R} or {G}", "one mana of any color"
    /// (106.1a: any of the five colors): one of these.
    Mana { options: Vec<Vec<ManaSymbol>> },
    /// One value from the rules-defined domain (105.1, 205.3m).
    Value(crate::binding::Kind),
    /// A card name of a card with these characteristics (201.4a): "choose a nonland card name".
    Name(crate::binding::Names),
    /// Inclusive voluntary numeric domain; never clamped by library size (121.3).
    Number { min: u32, max: u32 },
    /// Scry or surveil (701.22a, 701.25a): the player has looked at these cards, the top
    /// cards of their library, the first of them the top card, and puts each of them back on
    /// top, in any order, or away: on the bottom (scry) or into the graveyard (surveil).
    Arrange { verb: Verb, cards: Vec<ObjId> },
    /// Which of these players: "an opponent separates those cards", of a player who has
    /// several opponents. No player is targeted.
    Player { among: Vec<PlayerId> },
    /// Which of the piles cards were separated into (700.3). A pile may be empty (700.3d).
    Pile { piles: Vec<Vec<ObjId>> },
    /// Choose a replacement for a proposed event (616.1). `on` is its affected object or
    /// player. `optional` permits Answer::No to perform the original event; a mandatory
    /// replacement-order question never permits that answer.
    /// Per-hit proposed damage replacements, before result amounts freeze (120.4b).
    /// Order prevention and modifiers; shields have no per-hit identity (615.7, 616.1).
    DamageOrder { on: Target, hits: Vec<crate::operation::ProposedHit>, among: Vec<crate::operation::DamageOption> },
    /// Mandatory bounded allocation of one finite shield over eligible simultaneous hits.
    Prevention { on: Target, shield: crate::operation::ReplacementId, total: u32, hits: Vec<crate::operation::ProposedHit> },
    DamageReplacement { on: Target, hits: Vec<crate::damage::ProposedHit>, among: Vec<crate::damage::Offer> },
    Replacement { on: Target, kind: String, count: u32, counters: Option<crate::operation::CounterProposal>, tokens:Option<crate::operation::TokenProposal>, among: Vec<crate::operation::ReplacementId>, optional: bool },
    /// Which of the skip effects waiting for this step or phase replaces it (614.10a, 616.1),
    /// asked of the player whose it is before it would begin. The others wait.
    Skip { what: crate::turn::Occurrence, among: Vec<crate::turn::SkipId> },
}

/// A player's answer to a [`Choice`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    LibraryPosition(ir::action::LibraryPosition),
    SpellDestination { on: ObjId, effect: DestinationEffect },
    Targets(Vec<Vec<Target>>),
    Alternative(usize),
    Attach(Target),
    Holders(Vec<Target>),
    Defender(Target),
    Yes,
    No,
    Objects(Vec<ObjId>),
    /// None declines without selecting anything; Some accepts with the exact selection.
    OptionalObjects(Option<Vec<ObjId>>),
    /// The mana chosen: one of the options, as it is offered.
    Mana(Vec<ManaSymbol>),
    /// The typed value chosen.
    Value(crate::binding::Value),
    /// The player chosen: one of those offered.
    Player(PlayerId),
    /// The pile chosen, by its place among those offered.
    Pile(usize),
    /// Where the cards of a [`Choice::Arrange`] go: `top`, back on top of the library, the
    /// first of them the top card; `away`, the rest, on the bottom with the last of them the
    /// bottom card (scry), or into the graveyard (surveil). Each card once.
    Arrange { top: Vec<ObjId>, away: Vec<ObjId> },
    /// The replacement effect applied next: one of those offered.
    Replacement(crate::operation::ReplacementId),
    DamageReplacement(crate::damage::Offer),
    DamageOrder(crate::operation::DamageOption),
    Prevention(Vec<(usize,u32)>),
    /// Pay what a [`Choice::Pay`] asks: these mana abilities first (605.3a), then the cost
    /// with this mana of the pool, or as the engine chooses; `tapped`: the artifacts and
    /// creatures tapped for its waterbend cost, each paying {1} (701.67a).
    Pay { plan: Vec<crate::legal::Step>, with: Option<Pool>, tapped: Vec<ObjId> },
    /// The skip effect that applies: one of those offered.
    Skip(crate::turn::SkipId),
}

/// The creature types (205.3m): the subtypes creatures and kindreds share, as the rule lists
/// them.
pub fn creature_types() -> Vec<Subtype> {
    Subtype::of_kind(SubtypeKind::Creature).to_vec()
}

/// A public answer made before simultaneous actions occur (101.4b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicChoice {
    /// A public incoming block's selected top-first order, before simultaneous arrival.
    OrderCards { player: PlayerId, to: Placement, cards: Vec<ObjId> },
    LibraryDestination { player: PlayerId, cards: Vec<ObjId>, position: ir::action::LibraryPosition },
    Alternative { player: PlayerId, source: ObjId, ability: Option<crate::binding::AbilityKey>, instruction: usize, index: usize },
    SpellDestination { player: PlayerId, on: ObjId, effect: DestinationEffect },
    /// A scalar resolution choice, identified by its source, ability and local slot.
    Value { player: PlayerId, source: ObjId, ability: Option<crate::binding::AbilityKey>, slot: usize, value: crate::binding::Value },
    Objects { player: PlayerId, verb: Verb, objects: Vec<ObjId> },
    /// An entry answer already made, visible to later choosers before anything enters.
    Entry { player: PlayerId, card: ObjId, answer: Answer },
    /// A replacement effect already chosen for the event on this object or player (616.1).
    DamageOrder { player: PlayerId, on: Target, offer: crate::operation::DamageOption },
    Prevention { player: PlayerId, on: Target, shield: crate::operation::ReplacementId, allocation: Vec<(usize,u32)> },
    DamageReplacement { player: PlayerId, on: Target, offer: crate::damage::Offer },
    Replacement { player: PlayerId, on: Target, effect: crate::operation::ReplacementId },
}

/// A spell or ability whose instructions are being followed (608.2): where they have got to,
/// and what the steps done have settled for the steps to come. A step that needs a player's
/// choice waits for it here, so that the game can be saved, copied and gone on with at any
/// decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Completion {
    Stack,
    /// What an opening-hand action has its player do (103.6), as it is taken: no stack, no
    /// priority; the opening actions go on after.
    Opening,
    /// `checked`: the answers were checked against every choice of mana the ability could
    /// make; one left over is of a choice its text did not make this time (608.2c). `more`:
    /// the answers to the choices of the triggered mana abilities it triggers, which resolve
    /// next, in turn (605.4a). `triggered`: it is one of those.
    Mana { source: ObjId, player: PlayerId, taps_for_mana: bool, answers: std::collections::VecDeque<Vec<ManaSymbol>>, checked: bool,
        more: std::collections::VecDeque<Vec<ManaSymbol>>, triggered: bool, activation:Option<crate::binding::AbilityKey> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolving {
    pub(crate) payment_scope: Option<Box<crate::payment_audit::Scope>>,
    pub(crate) completion: Completion,
    /// Some only after the body and Resolved bookkeeping have completed once.
    pub(crate) destination: Option<crate::completion::Destination>,
    /// The spell or ability on the stack.
    pub object: ObjId,
    pub(crate) execution: Option<crate::program::Resolution>,
    /// Number of announced target groups consumed by completed invocations.
    pub(crate) next: usize,
    /// Choices already made for this simultaneous instruction that later choosers
    /// may know (101.4b). Hidden-card choices and library ordering are not included.
    pub public_choices: Vec<PublicChoice>,
    /// What the text has named so far: what "it" and "that creature" refer to.
    pub(crate) bound: Bound,

}

impl Resolving {
    /// Debug/validation view of the currently executing compiled ability.
    pub fn program_frame(&self) -> Option<&crate::program::Frame> {
        self.execution.as_ref()?.frame()
    }

    pub(crate) fn answer(&mut self, answer: Answer) {
        if let Some(destination) = &mut self.destination { destination.answer(answer); return; }
        if let Some(frame) = self.execution.as_mut().and_then(|e| e.frame_mut()) {
            frame.answer(answer);
        } else { unreachable!("the active procedure owns the answer") }
    }

    pub(crate) fn new(object: ObjId, bound: Bound) -> Resolving {
        Resolving { payment_scope: None, completion: Completion::Stack, destination: None, object, execution: None, next: 0, public_choices: Vec::new(), bound }
    }
}

impl Object {
    /// Is it of this card type as its card or token says? What a permanent is now is asked of
    /// the game ([`Game::is_a`]), through the continuous effects that change it (613.1d).
    pub fn is(&self, t: CardType) -> bool {
        self.chars.types.contains(&t)
    }

    /// An emblem is an object with abilities, not a card or token (114.1).
    pub fn emblem(&self) -> bool { self.emblem.is_some() }

    pub fn token(&self) -> bool {
        self.card.is_none() && self.ability.is_none() && self.emblem.is_none() && !self.card_copy
    }
}

/// Mana a player has: by color (in WUBRG order), and colorless; and, one by one, the mana
/// whose text restricts spending or retains an additional spending effect (106.6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pool {
    pub colors: [u32; 5],
    pub colorless: u32,
    pub restricted: Vec<Restricted>,
}

impl Pool {
    pub fn total(&self) -> u32 {
        self.colors.iter().sum::<u32>() + self.colorless + self.restricted.len() as u32
    }
}

/// Bounded casting facts: mana paid by type, without retaining producing objects or history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PaidMana {
    pub colors: [u32; 5],
    pub colorless: u32,
}
impl PaidMana {
    pub fn total(&self) -> u32 { self.colors.iter().sum::<u32>() + self.colorless }
    pub fn between(before: &Pool, after: &Pool, restricted: &[Restricted]) -> Self {
        let mut paid = Self { colors: std::array::from_fn(|i| before.colors[i] - after.colors[i]), colorless: before.colorless - after.colorless };
        for m in restricted {
            match m.mana {
                ManaSymbol::Color(c) => paid.colors[c as usize] += 1,
                ManaSymbol::Colorless => paid.colorless += 1,
                _ => unreachable!("mana has a color or is colorless"),
            }
        }
        paid
    }
}

/// One mana that a spell or ability produced with a restriction on how it can be spent, or
/// with an effect or delayed trigger on what it is spent on (106.6): "Spend this mana only to cast a creature
/// spell of the chosen type, and that spell can't be countered." That does not change its
/// type (106.6), and it is spent only on what its text says ([`crate::mana`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpendTrigger {
    pub on: crate::program::mana::Only,
    pub captured: crate::program::Captured,
    pub bound: Bound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restricted {
    /// Authenticated at production, never reconstructed from a later source.
    pub provenance: Option<std::sync::Arc<crate::audit::RestrictedManaProvenance>>,
    pub ability: Option<crate::binding::AbilityKey>,
    pub(crate) trigger: Option<SpendTrigger>,
    /// Its type: a color, or colorless.
    pub mana: ManaSymbol,
    /// What it may be spent on: any of these, as its text was lowered.
    pub only: crate::program::mana::Only,
    /// The spell it is spent on can't be countered.
    pub uncounterable: bool,
    /// Whose text produced it: what "the chosen type" and "you" in that text are said of
    /// (607.2d), also once that permanent has left the battlefield.
    pub source: ObjId,
    pub controller: PlayerId,
    /// Kept as steps and phases end, until this ends (106.4): "Until end of combat, you don't
    /// lose this mana as steps and phases end."
    pub kept: Option<Kept>,
}

/// Until when mana is not lost as steps and phases end (106.4, 500.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// "until end of combat": the combat phase's end (511.3).
    Combat,
    /// "until end of turn": the cleanup step's end (514.2).
    Turn,
}

/// What mana is spent on (106.6): the spell a card is cast as (601.2h), or an ability of a
/// permanent being activated (602.2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spent {
    Spell(ObjId, crate::card::CastAs),
    /// Announcement characteristics captured before source changes or payment begins.
    Ability { source: ObjId, keyword: Option<ir::Keyword>, mana: bool },
    /// A payment an effect of this object asks for as it is applied (118.12): neither
    /// casting a spell nor activating an ability.
    Effect(ObjId),
}

#[derive(Debug, Clone)]
pub struct Player {
    pub counters: BTreeMap<String, u32>,
    pub life: i32,
    /// The top card is last.
    pub library: Vec<ObjId>,
    pub hand: Vec<ObjId>,
    pub graveyard: Vec<ObjId>,
    pub pool: Pool,
    pub land_played: bool,
    /// The turn in which this player's most recent turn began (302.6); 0 before their first.
    pub turn_began: u32,
    /// How many turns this player has begun, extra turns included (500.7).
    pub turns_taken: u32,
    /// Tried to draw from an empty library since state-based actions were last checked (704.5b).
    pub drew_from_empty: bool,
    pub lost: bool,
    /// None until a rule or effect sets it (702.179b); at most 4.
    pub speed: Option<u8>,
}

/// A retained 704.3 check: choices precede the simultaneous commitment of this snapshot.
#[derive(Debug, Clone, Default)]
pub(crate) struct StateBased {
    batch: Option<SbaBatch>,
    pub(crate) changed: bool,
    pub(crate) choices: Vec<PublicChoice>,
}
#[derive(Debug, Clone)]
struct SbaBatch {
    tokens: Vec<ObjId>,
    attachments: Vec<(ObjId, Target)>,
    permanents: Vec<(ObjId, crate::layers::Seen, bool, Option<bool>)>,
    losses: Vec<PlayerId>,
    // Each name group is chosen once. A player can choose their groups in either order.
    legends: Vec<(PlayerId, Vec<ObjId>)>,
    removed: Vec<ObjId>,
    /// Sagas that have read their final chapter: their controllers sacrifice them (714.4).
    sagas: Vec<ObjId>,
    departures: Option<crate::departure::Batch>,
    commander_returns: Vec<(PlayerId, ObjId)>,
    to_command: Vec<ObjId>,
    checked_through: crate::event::Seq,
    /// 704.5q: collected with deaths; departing objects keep both kinds for LKI (704.8).
    cancellation: Vec<(ObjId, u32)>,
    /// 704.5aa: whose speed becomes 1.
    engines: Vec<PlayerId>,
}
impl StateBased {
    pub(crate) fn public_choices(&self) -> impl Iterator<Item = &PublicChoice> {
        self.choices.iter().chain(self.batch.iter().filter_map(|batch| batch.departures.as_ref())
            .flat_map(|departures| departures.choices()))
    }
    pub(crate) fn answer(&mut self, player: PlayerId, answer: Answer) {
        if let Some(departures) = self.batch.as_mut().and_then(|b| b.departures.as_mut()) {
            departures.answer(answer);
            return;
        }
        if let Some(batch) = &mut self.batch && !batch.commander_returns.is_empty() && matches!(answer, Answer::Yes | Answer::No) {
            let (owner, object) = batch.commander_returns.remove(0);
            assert_eq!(owner, player);
            let chosen = if answer == Answer::Yes { batch.to_command.push(object); vec![object] } else { Vec::new() };
            self.choices.push(PublicChoice::Objects { player, verb: Verb::Move, objects: chosen });
            return;
        }
        let Answer::Objects(chosen) = answer else { unreachable!("validated legend choice") };
        let keep = chosen[0];
        let batch = self.batch.as_mut().expect("retained SBA snapshot");
        let at = batch.legends.iter().position(|(p, group)| *p == player && group.contains(&keep))
            .expect("the chosen legend belongs to this player's pending groups");
        let (_, group) = batch.legends.remove(at);
        batch.removed.extend(group.into_iter().filter(|id| *id != keep));
        self.choices.push(PublicChoice::Objects { player, verb: Verb::Choose, objects: chosen });
    }
}

/// A conservative branch-local invalidation revision, not a globally unique game identity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct LayerRevision(u64);
impl LayerRevision {
    fn next(self) -> Self { Self(self.0.checked_add(1).expect("layer revision exhausted")) }
}

/// Owned game storage exposed immutably by Game. Mutable access can only be lent by
/// Game's tracked boundary; even direct field writes and fixture edits pass through it.
#[derive(Debug, Clone)]
pub struct GameState {
    pub(crate) until: Vec<crate::until::Return>,
    pub defs: Arc<Vec<CardDef>>,
    objects: arena::Arena,
    pub players: Vec<Player>,
    pub battlefield: Vec<ObjId>,
    pub exile: Vec<ObjId>,
    pub command: Vec<ObjId>,
    /// The top of the stack is last.
    pub stack: Vec<ObjId>,
    pub active: PlayerId,
    pub(crate) turn_schedule: crate::turn_control::Schedule,
    /// The life total each player began the game with (103.4).
    pub starting_life: i32,
    /// Where the turn is (500.1): a step, or a main phase.
    pub part: Part,
    /// How many turns have begun; the first is 1.
    pub turn: u32,
    /// The sole player with the monarch designation (725.1), if established.
    pub monarch: Option<PlayerId>,
    /// Players received priority in this cleanup step, so another follows it (514.3a).
    pub(crate) another_cleanup: bool,
    /// How many steps and main phases have begun this game: which occurrence of one the game
    /// is in, as a second combat damage step (510.4) or another cleanup step (514.3a) is
    /// another occurrence of the same part of the same phase.
    pub(crate) parts_begun: u64,
    /// The phases of this turn in progress and to come (500.1, 500.8).
    pub plan: crate::turn::TurnPlan,
    /// The skip effects waiting for what they skip (614.10), oldest first.
    pub skips: Vec<crate::turn::Skip>,
    pub(crate) skip_ids: u32,
    /// A harness's setting: a question the rules have a player answer is asked even when it
    /// has one possible answer (a creature put onto the battlefield attacking with one
    /// possible defender, 508.4), as a record of a game played elsewhere has it asked.
    pub(crate) ask_forced: bool,
    /// The combat phase so far (506).
    pub combat: Combat,
    /// What resolved spells and abilities have put in force for a while (611.2a).
    pub(crate) lasting: Vec<crate::restrict::Lasting>,
    /// The regeneration shields of this turn (701.19a): a permanent once for each, until one
    /// replaces its destruction or the turn ends.
    pub(crate) shields: Vec<ObjId>,
    /// Resolved turn-long damage prevention, separate from regeneration (615.7).
    pub(crate) prevention: Vec<crate::prevention::Effect>,
    /// The continuous effects resolved spells and abilities began, which change what the
    /// objects they affected are (611.2).
    pub continuous: Vec<crate::layers::Continuous>,
    /// The last timestamp given (613.7).
    pub(crate) clock: u32,
    /// Sorted, disjoint ranges of timestamps given at once (613.7m), whose relative order
    /// is APNAP order and the players' choices rather than this engine's.
    pub(crate) simultaneous: Vec<crate::layers::AtOnce>,
    /// The abilities activated this turn, each time: the object and which of its abilities
    /// ("Activate only once each turn", 602.5b).
    pub(crate) activated: Vec<(ObjId, crate::binding::AbilityKey)>,
    /// Begun activations in the same reservation order, including unfinished payment (702.177b).
    pub(crate) activation_started: Vec<(PlayerId, Option<ir::Keyword>, bool)>,
    /// CR 605.3c: an exact mana ability cannot be reentered before it resolves.
    pub(crate) active_mana_abilities: Vec<(ObjId, crate::binding::AbilityKey)>,
    pub(crate) ability_uses: Vec<crate::ability_limit::Use>,
    /// The spell or ability whose instructions are being followed and wait for a choice.
    pub(crate) procedures: Vec<crate::procedure::Procedure>,
    pub priority: PlayerId,
    /// Players who passed in succession (117.4).
    pub(crate) passes: usize,
    pub log: crate::event::Log,
    /// What each seat was shown and has not taken yet ([`crate::observation`]).
    pub(crate) observations: crate::observation::Waiting,
    /// How much of the log has been checked for triggers.
    pub(crate) seen: crate::event::Seq,
    /// Abilities that have triggered and are not on the stack yet.
    pub(crate) triggered: Vec<Triggered>,
    /// The delayed triggered abilities effects have made, which watch for their events
    /// (603.7).
    pub(crate) delayed: Vec<Delayed>,
    /// How many of them, from the first, are in the order they go on the stack in: their
    /// controller's choice where they had one (603.3b).
    pub(crate) ordered: usize,
    /// The stretches of the log that are one event: what one check of state-based actions
    /// performed (704.3: simultaneously), what one instruction did ("Destroy all
    /// creatures."). Kept until triggers have been checked against them.
    pub(crate) together: Vec<(crate::event::Seq, crate::event::Seq)>,
    /// Complete battlefield views immediately before departures, owned only until trigger
    /// delivery. An enclosing simultaneous event supersedes individual movement views.
    pub(crate) lookbacks: Vec<(crate::event::Seq, crate::event::Seq, crate::layers::Working)>,
    pub decision: Option<Decision>,
    /// Where what is random comes from: a shuffle (701.24a).
    pub(crate) random: crate::start::Random,
    /// Privileged replay outcomes, scoped to one harness response; ordinary games use RNG.
    pub(crate) captured_random: Option<Vec<crate::start::CapturedRandom>>,
    pub(crate) replay_random: Option<std::collections::VecDeque<crate::start::RecordedRandom>>,
    /// The mulligans, until the first turn begins (103.5).
    pub(crate) opening: Option<crate::start::Opening>,
    /// A permanent that is about to enter the battlefield and waits for a choice made before
    /// it does (614.12a).
    /// The rules operations under way that wait for a choice or for their caller, the
    /// innermost last (616.1). Not the stack of spells and abilities.
    /// A temporary 614.12 query: the prospective permanent, and its prior incarnation.
    /// The prospective object is never inserted in battlefield enumeration by this view.
    pub(crate) prospective: Option<(ObjId, Option<ObjId>)>,
    /// Acquired linked relationship lifetimes, independent of activation-use keys.
    pub(crate) linked_acquisitions: Vec<crate::linked_lifetime::Acquisition>,
    /// The cards an instruction of an object's ability exiled: that object, and the card as it
    /// is in exile. What another ability of the object calls "the exiled cards" (607.2a).
    pub(crate) exiled_with: Vec<(ObjId, Option<crate::binding::AbilityKey>, ObjId, Option<usize>)>,
    /// Each card a madness trigger did not cast and moved to a public zone as it resolved: the
    /// card as it was in exile, and the new object it became. Effects that refer to the
    /// discarded card find that object (400.7k, 702.35c).
    pub(crate) madness_found: Vec<(ObjId, ObjId)>,
    /// The exiled cards effects let a player play this turn.
    pub(crate) permissions: Vec<crate::permission::Permission>,
    pub(crate) companions: Vec<Option<crate::companion::Chosen>>,
    pub(crate) commanders: Vec<crate::commander::Commander>,
    next_card: u64,
    /// Events before this boundary were already considered by state-based checks (903.9a).
    last_sba: crate::event::Seq,
    pub(crate) cast_permission_uses: Vec<crate::cast_permission::Use>,
    /// What has happened this turn that a condition could ask about (608.2i).
    pub(crate) this_turn: crate::history::ThisTurn,
}

#[derive(Clone)]
pub struct Game {
    state_data: GameState,
    layer_revision: LayerRevision,
    pub(crate) query_cache: crate::query::Cache,
    /// Opt-in host evidence; never part of rules state or ordinary play.
    pub(crate) payment_scope: Option<Box<crate::payment_audit::Scope>>,
    pub(crate) payment_audit: Option<std::sync::Arc<std::sync::Mutex<Vec<crate::payment_audit::Record>>>>,
    /// Debug builds' check of 510.1e: the player assigning combat damage, and the game as
    /// it was before they first divided; None once another move was taken while they
    /// assigned. Never part of rules state.
    #[cfg(debug_assertions)]
    pub(crate) assignment_began: Option<(PlayerId, Option<Box<Game>>)>,
    /// A refusal found where a rules path cannot return one ([`Game::defer_refusal`]),
    /// while an action is in progress ([`Game::guarded`]); clones made during it share it.
    /// Never rules state.
    deferred: Option<Arc<std::sync::OnceLock<crate::Unsupported>>>,
}
// Revision/cache bookkeeping must not enter rules-state comparisons or search keys.
impl std::fmt::Debug for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.state_data.fmt(f) }
}
impl std::ops::Deref for Game {
    type Target = GameState;
    fn deref(&self) -> &GameState { &self.state_data }
}
impl std::ops::DerefMut for Game {
    fn deref_mut(&mut self) -> &mut GameState { self.state_mut() }
}

impl Game {
    pub fn layer_revision(&self) -> LayerRevision { self.layer_revision }
    /// Read-only view; it cannot bypass the tracked mutable boundary.
    /// ```compile_fail
    /// let game = game::Game::new(Vec::<game::CardDef>::new(), 2, 20);
    /// game.state_view().players[0].life = 1;
    /// ```
    pub fn state_view(&self) -> &GameState { &self.state_data }
    /// Invalidate before lending, including loans that ultimately make no change. Queries
    /// cannot borrow this Game until the mutable view/field loan ends. All completed-answer families
    /// are invalidated; narrow typed internal paths can preserve certified unaffected results.
    pub fn state_mut(&mut self) -> &mut GameState {
        self.layer_revision = self.layer_revision.next();
        self.query_cache.invalidate();
        &mut self.state_data
    }
    // Only these field-specific methods can bypass full GameState lending. Each names
    // every fact its returned field can change; no caller receives arbitrary state.
    fn changed(&mut self, writes: crate::program::footprint::Properties) {
        self.layer_revision = self.layer_revision.next();
        self.query_cache.changed(writes,self.layer_revision);
    }
    pub(crate) fn observation_waiting_mut(&mut self) -> &mut crate::observation::Waiting {
        self.changed(crate::program::footprint::Properties::NONE);
        &mut self.state_data.observations
    }
    pub(crate) fn event_log_mut(&mut self) -> &mut crate::event::Log {
        use crate::program::footprint::Properties as P;
        self.changed(P::HISTORY | P::BINDINGS);
        &mut self.state_data.log
    }
    pub(crate) fn turn_history_mut(&mut self) -> &mut crate::history::ThisTurn {
        use crate::program::footprint::Properties as P;
        self.changed(P::HISTORY | P::BINDINGS);
        &mut self.state_data.this_turn
    }
    pub(crate) fn life_total_mut(&mut self, player: PlayerId) -> &mut i32 {
        self.changed(crate::program::footprint::Properties::LIFE);
        &mut self.state_data.players[player].life
    }
    pub(crate) fn mana_pool_mut(&mut self, player: PlayerId) -> &mut Pool {
        self.changed(crate::program::footprint::Properties::MANA);
        &mut self.state_data.players[player].pool
    }
    pub(crate) fn tapped_mut(&mut self, object: ObjId) -> &mut bool {
        self.changed(crate::program::footprint::Properties::TAPPED);
        &mut self.state_data.objects.get_mut(object).expect("retained incarnation").tapped
    }
    pub(crate) fn counter_fields_mut(&mut self, object: ObjId) -> (&mut BTreeMap<String,u32>, &mut BTreeMap<String,u32>) {
        use crate::program::footprint::Properties as P;
        self.changed(P::COUNTERS | P::TIMESTAMP);
        let o=self.state_data.objects.get_mut(object).expect("retained incarnation");
        (&mut o.counters,&mut o.counter_stamps)
    }
    /// Adopt a speculative/reversed state without reusing either branch's cached answers.
    pub(crate) fn adopt(&mut self, mut proposed: Game) {
        proposed.layer_revision = self.layer_revision.max(proposed.layer_revision).next();
        proposed.query_cache = Default::default();
        *self = proposed;
    }

    /// Refuse the action in progress once it stops, where the rules path that found the
    /// reason cannot return it: a query (613.7m) or an infallible zone change. The first
    /// reason stands. Outside an action ([`Game::guarded`]) a query refuses nothing: every
    /// action ends by asking what its result left, so a later read finds nothing new.
    pub(crate) fn defer_refusal(&self, why: crate::Unsupported) {
        if let Some(slot) = &self.deferred { let _ = slot.set(why); }
    }
    /// Is an action in progress, so that a refusal can be deferred to its end?
    pub(crate) fn acting(&self) -> bool { self.deferred.is_some() }
    /// Has the action in progress a refusal deferred to its end?
    pub(crate) fn refusing(&self) -> bool { self.deferred.as_ref().is_some_and(|slot| slot.get().is_some()) }

    /// Run one action with its own deferred refusals. A deferred refusal, or an
    /// unsupported result, restores the game as it was before.
    pub(crate) fn guarded<T>(&mut self, act: impl FnOnce(&mut Game) -> Result<T, crate::play::ActionError>)
        -> Result<T, crate::play::ActionError> {
        let before = self.clone();
        let result = self.deferring(act);
        if matches!(result, Err(crate::play::ActionError::Unsupported(_))) { self.adopt(before); }
        result
    }
    /// Run one action with its own deferred refusals, a deferred one making its result
    /// unsupported. The caller restores an unsupported result's game ([`Game::guarded`],
    /// or a player's move, which already holds the game as it was).
    pub(crate) fn deferring<T>(&mut self, act: impl FnOnce(&mut Game) -> Result<T, crate::play::ActionError>)
        -> Result<T, crate::play::ActionError> {
        let outer = self.deferred.replace(Default::default());
        let result = act(self);
        // Ask every current characteristic once the action has stopped, so that an order
        // of simultaneous timestamps it left deciding one is refused with it (613.7m).
        if !matches!(result, Err(crate::play::ActionError::Unsupported(_))) { self.check_simultaneous_order(); }
        let deferred = self.deferred.as_ref().and_then(|slot| slot.get().cloned());
        self.deferred = outer;
        match (result, deferred) {
            (Err(crate::play::ActionError::Unsupported(why)), _) | (_, Some(why)) => Err(crate::play::ActionError::Unsupported(why)),
            (result, None) => result,
        }
    }
}

/// A refused request changes nothing in the game (the request id is the host's): debug
/// builds compare the game after a refusal with the game it must equal by debug identity,
/// which leaves out revision, cache and evidence bookkeeping, and stop at the first
/// difference. Only refusals pay for it, and release builds never do.
#[cfg(debug_assertions)]
pub(crate) fn assert_unchanged(expected: &Game, actual: &Game, refusal: &str) {
    let (expected, actual) = (format!("{expected:?}"), format!("{actual:?}"));
    if expected == actual { return; }
    let at = expected.bytes().zip(actual.bytes()).position(|(e, a)| e != a).unwrap_or(expected.len().min(actual.len()));
    let around = |text: &str| String::from_utf8_lossy(&text.as_bytes()[at.saturating_sub(300)..(at + 300).min(text.len())]).into_owned();
    panic!("a refused request changed the game (\"{refusal}\"); at byte {at} it was\n…{}…\nand is\n…{}…", around(&expected), around(&actual));
}

impl Game {
    /// A game with these card definitions and empty zones, set up in the precombat main phase
    /// of the first player's first turn, that player having priority; cards are added with
    /// [`Game::put`].
    /// Pass a shared `Arc<Vec<CardDef>>` to reuse one compiled catalog across independent
    /// games, or an owned vector when constructing a single game or fixture.
    pub fn new(defs: impl Into<Arc<Vec<CardDef>>>, players: usize, life: i32) -> Game {
        let player = Player {
            counters: BTreeMap::new(),
            life,
            library: Vec::new(),
            hand: Vec::new(),
            graveyard: Vec::new(),
            pool: Pool::default(),
            land_played: false,
            turn_began: 0,
            turns_taken: 0,
            drew_from_empty: false,
            speed: None,
            lost: false,
        };
        let companions = vec![None; players];
        let mut players = vec![player; players];
        // the first player's first turn has begun
        players[0].turn_began = 1;
        players[0].turns_taken = 1;
        let defs: Arc<Vec<CardDef>> = defs.into();
        let mut this_turn = crate::history::ThisTurn::default();
        this_turn.every = crate::history::asks_back(&defs);
        Game {
            query_cache: Default::default(),
            payment_scope: None, payment_audit: None, deferred: None,
            #[cfg(debug_assertions)]
            assignment_began: None,
            layer_revision: LayerRevision::default(),
            state_data: GameState {
                until: Vec::new(),
                defs,
                objects: arena::Arena::default(),
                players,
                battlefield: Vec::new(),
                command: Vec::new(),
                exile: Vec::new(),
                stack: Vec::new(),
                active: 0,
                turn_schedule: crate::turn_control::Schedule::default(),
                starting_life: life,
                part: Part::PrecombatMain,
                turn: 1,
                monarch: None,
                another_cleanup: false,
                parts_begun: 0,
                plan: crate::turn::TurnPlan::at(1, Part::PrecombatMain, 0),
                skips: Vec::new(),
                skip_ids: 0,
                ask_forced: false,
                combat: Combat::default(),
                lasting: Vec::new(),
                shields: Vec::new(),
                prevention: Vec::new(),
                continuous: Vec::new(),
                clock: 0,
                simultaneous: Vec::new(),
                activated: Vec::new(),
                activation_started: Vec::new(), active_mana_abilities: Vec::new(),
                ability_uses: Vec::new(),
                procedures: Vec::new(),
                priority: 0,
                passes: 0,
                log: crate::event::Log::default(),
                observations: Default::default(),
                seen: crate::event::Seq::default(),
                triggered: Vec::new(),
                delayed: Vec::new(),
                ordered: 0,
                together: Vec::new(),
                lookbacks: Vec::new(),
                random: crate::start::Random::new(0),
                captured_random: None,
                replay_random: None,
                opening: None,
                prospective: None,
                linked_acquisitions: Vec::new(),
                exiled_with: Vec::new(),
                madness_found: Vec::new(),
                permissions: Vec::new(),
                cast_permission_uses: Vec::new(),
                companions,
                commanders: Vec::new(),
                next_card: 0,
                last_sba: crate::event::Seq::default(),
                this_turn,
                decision: None,
            },
        }
    }

    /// Lookup validates the incarnation's generation; a stale handle never names a reused slot.
    pub fn get_object(&self, id: ObjId) -> Option<&Object> { self.objects.get(id) }

    /// Privileged harness identity for a physical card across object incarnations.
    /// This is not a legal player-facing target or a seat-visible observation.
    pub fn physical_card_identity(&self, id: ObjId) -> Option<u64> {
        self.get_object(id)?.card_id.map(|card| card.0)
    }

    /// Number of retained object payloads, including referenced last-known information.
    pub(crate) fn audit_retained_objects(&self) -> impl Iterator<Item = &Object> { self.objects.iter() }

    pub fn object_count(&self) -> usize { self.objects.iter().count() }

    /// Current incarnations of physically designated commanders, independent of control (903.3).
    pub(crate) fn owned_commanders(&self, owner: PlayerId) -> Vec<ObjId> {
        self.objects.iter().filter(|o|o.zone!=Zone::Gone && o.card_id.is_some_and(|id|self.commanders.iter().any(|c|c.card==id && c.owner==owner))).map(|o|o.id).collect()
    }

    /// Arena high-water storage, including reusable empty slots.
    pub fn object_slots(&self) -> usize { self.objects.slots() }

    pub fn obj(&self, id: ObjId) -> &Object {
        // gone from the game, as the turn's history last saw it (608.2h)
        self.get_object(id).or_else(|| self.this_turn.archived(id)).expect("object handle must name a retained incarnation")
    }

    pub(crate) fn obj_mut(&mut self, id: ObjId) -> &mut Object {
        self.objects.get_mut(id).expect("object handle must name a retained incarnation")
    }

    /// The definition of the card an object is; of a predefined token, which no card
    /// represents, the rules' definition that is its text (111.3, 111.10); of an emblem,
    /// the immutable definition supplied by its creating effect (114.3); of a face-down
    /// permanent, the face-down characteristics, in front of its copy effects (708.2a).
    #[inline]
    pub fn def(&self, id: ObjId) -> Option<&CardDef> {
        let o = self.obj(id);
        if let Some(emblem) = o.emblem { return Some(emblem.def()); }
        if let crate::face::Face::Down(down) = o.face { return Some(crate::face::definition(down.by)); }
        if o.hidden.is_some() { return Some(crate::face::hidden_definition()); }
        self.underlying(id)
    }

    /// The mode rows under an object's ability at `index` among [`Game::abilities`] (700.2):
    /// none for an ability that is not modal.
    pub fn modes_of(&self, o: ObjId, index: usize) -> Vec<ir::Mode> {
        self.members(o).get(index).map_or_else(Vec::new, |member| member.def().modes_of(member.key.index).to_vec())
    }

    fn zone_list(&mut self, zone: Zone, owner: PlayerId) -> Option<&mut Vec<ObjId>> {
        Some(match zone {
            Zone::Library => &mut self.players[owner].library,
            Zone::Hand => &mut self.players[owner].hand,
            Zone::Graveyard => &mut self.players[owner].graveyard,
            Zone::Battlefield => &mut self.battlefield,
            Zone::Exile => &mut self.exile,
            Zone::Command => &mut self.command,
            Zone::Stack => &mut self.stack,
            Zone::Gone => return None,
        })
    }

    pub(crate) fn new_object(&mut self, card: Option<usize>, chars: Chars, owner: PlayerId, controller: PlayerId, zone: Zone) -> ObjId {
        let stamp = self.tick();
        let since = self.turn;
        let card_id = card.map(|_| {
            let id = CardId(self.next_card);
            self.next_card = self.next_card.checked_add(1).expect("physical card identity exhausted");
            id
        });
        let id = self.objects.insert(|id| Object {
            cast_as: crate::card::CastAs::Whole,
            attached_to: None,
            stamp,
            id,
            card,
            card_id,
            chars,
            copy_acquisition: None,
            copied: None,
            copy_effects: crate::copy::Effects::default(),
            token_faces: None,
            card_copy: false, stack_copy_of: None,
            face: crate::face::Face::Up,
            hidden: None,
            foretold: None,
            emblem: None,
            owner,
            controller,
            zone,
            tapped: false,
            damage: 0,
            deathtouched: false,
            counters: BTreeMap::new(),
            counter_stamps: BTreeMap::new(),
            targets: Vec::new(),
            division: Vec::new(),
            ability: None,
            invocations: None,
            delayed: false,
            since,
            entered: since,
            activation_uses: Vec::new(),
            bound: Bound::default(),
            last: None,
            last_attachments: [false; 2],
            flashback: None,
            harmonized: None,
            cast_rider: None,
            transformed: false,
            transformed_at: 0,
            level: None,
            unlocked: [false; 2],
            monstrous: false,
            renowned: false,
            solved: false,
            prepared: false, prepared_copy: None, prepare_source: None,
            saddled: false,
            cast_from: None,
            cast_by: None,
            x: None,
            spell_x: None,
            kicked: 0,
            optional_paid: false,
            alternative: None,
            buyback_paid: false,
            entwined: false,
            replicated: 0,
            gift: None,
            announced: Vec::new(),
            cast_announcement: None,
            left: None,
            arrived: None,
            successor: None,
            choices: Vec::new(),
            uncounterable: false,
            mana_paid: PaidMana::default(),
            cast_exiled: Vec::new(),
            modes: Vec::new(),
        });
        if let Some(list) = self.zone_list(zone, owner) {
            list.push(id);
        }
        id
    }

    /// A copy of this card, made in the zone the card is in for this player to cast
    /// (707.12): it has the card's copiable values (707.2) and is owned by that player.
    pub(crate) fn copy_card(&mut self, card: ObjId, player: PlayerId) -> ObjId {
        let values = self.token_copy(card);
        let zone = self.obj(card).zone;
        let copy = self.new_object(None, values.current().chars.clone(), player, player, zone);
        let made = self.obj_mut(copy);
        made.token_faces = Some(values.faces);
        made.transformed = values.transformed;
        made.card_copy = true;
        copy
    }

    /// A revealed library card that is reordered stops being revealed and becomes a
    /// new object (701.20d). It did not change zones, so emit no zone-change event.
    pub(crate) fn hide_reordered(&mut self, id: ObjId) -> Option<ObjId> {
        let old = self.obj(id).clone();
        let index = self.players[old.owner].library.iter().position(|o| *o == id)?;
        self.obj_mut(id).zone = Zone::Gone;
        self.obj_mut(id).left = Some(Zone::Library);
        let now = self.new_object(old.card, old.chars, old.owner, old.owner, Zone::Library);
        self.obj_mut(now).card_id = old.card_id;
        self.players[old.owner].library.pop(); // replace its existing place; no extra card
        self.players[old.owner].library[index] = now;
        Some(now)
    }

    /// Put these cards, already ordered with the first nearest the top, in their owner's
    /// library (400.3, 401.4, 401.7). Each group has one owner. Replacements decide whether
    /// an object actually arrives there; cards already there do not change zones.
    pub(crate) fn place_in_library(&mut self, owner: PlayerId, cards: &[ObjId], position: ir::action::LibraryPosition, revealed: &[ObjId]) {
        // The public zone-change log must not disclose the owner's private ordering
        // (401.4). Create new objects in a canonical order, then arrange them privately.
        let mut moving = cards.to_vec();
        moving.sort_by_key(|o| o.0);
        let mut became = Vec::new();
        for card in &moving {
            if !self.takes(Verb::Move, Role::What, Target::Object(*card)) { continue; }
            debug_assert_eq!(self.obj(*card).owner, owner);
            self.event_log_mut().push(Event::Moved { object: *card, to: Zone::Library });
            let now = if self.obj(*card).zone == Zone::Library {
                if revealed.contains(card) { self.hide_reordered(*card) } else { Some(*card) }
            } else {
                self.move_to(*card, Zone::Library, owner)
            };
            if let Some(now) = now.filter(|o| self.obj(*o).zone == Zone::Library) { became.push((*card, now)); }
        }
        let placed: Vec<_> = cards.iter().filter_map(|card| became.iter().find(|(was, _)| was == card).map(|(_, now)| *now)).collect();
        self.arrange_library(owner, &placed, position);
    }

    /// Reorder an already committed group without another zone change or public order log.
    pub(crate) fn arrange_library(&mut self, owner: PlayerId, placed: &[ObjId], position: ir::action::LibraryPosition) {
        self.players[owner].library.retain(|o| !placed.contains(o));
        let library = &self.players[owner].library;
        let index = match position {
            ir::action::LibraryPosition::Top => library.len(),
            ir::action::LibraryPosition::Bottom => 0,
            ir::action::LibraryPosition::FromTop(n) if n <= 1 => library.len(),
            ir::action::LibraryPosition::FromTop(n) => library.iter().enumerate().rev()
                .filter(|(_, o)| self.is(Target::Object(**o), Sort::Card))
                .nth((n - 2) as usize).map_or(0, |(index, _)| index),
        };
        self.players[owner].library.splice(index..index, placed.iter().copied().rev());
    }

    /// Put an ability on the stack (113.3c): an object that is not a card, with its source's
    /// characteristics for a name, the modes chosen for it and its targets.
    pub(crate) fn ability_on_stack(&mut self, t: &Triggered, modes: Vec<usize>, targets: Vec<Vec<Target>>) -> ObjId {
        let (card, chars) = match t.source {
            Some(source) => { let source=self.obj(source); (source.card,source.chars.clone()) },
            // a rule's ability "has no source" (702.179d, 725.2), so no source's name either:
            // its definition's name is the engine's, not the game's
            None => (None, Chars { name: String::new(), ..t.context().definition(self).expect("a rule occurrence retains its compiled definition").chars() }),
        };
        let invocation = t.captured.clone().unwrap_or_else(|| crate::program::capture(self, t.context()));
        let id = self.new_object(card, chars, t.controller, t.controller, Zone::Stack);
        let o = self.obj_mut(id);
        o.ability = Some((t.source, t.index));
        o.delayed = t.delayed.is_some();
        o.invocations = Some(invocation.and_then(|i| i.select(&modes)));
        o.modes = modes;
        o.targets = targets;
        o.bound = t.bound.clone();
        self.targeting_events(id);
        id
    }

    /// Choosing several target slots naming the same object makes it a target once
    /// (603.2d, 115.9). Capture these events before mana/costs can remove their watchers.
    pub(crate) fn targeting_events(&mut self, of: ObjId) {
        let mut seen = Vec::new();
        for object in self.obj(of).targets.iter().flatten().copied() {
            if !seen.contains(&object) { seen.push(object); }
        }
        for object in seen {
            let ordinal = self.note_targeted(object, of);
            self.event_log_mut().push(Event::BecameTarget { object, of, ordinal });
        }
    }

    /// An ability leaves the stack and ceases to exist (608.2n).
    pub(crate) fn ability_gone(&mut self, id: ObjId) {
        self.stack.retain(|x| *x != id);
        self.obj_mut(id).zone = Zone::Gone;
        self.obj_mut(id).left = Some(Zone::Stack);
    }

    /// Put a card of `player`'s into a zone from outside the game: setting a game up.
    pub fn put(&mut self, player: PlayerId, def: usize, zone: Zone) -> ObjId {
        let d = &self.defs[def];
        // a Room put onto the battlefield has neither door unlocked (709.5d)
        let chars = if zone == Zone::Battlefield { d.room([false; 2]).chars() } else { d.chars() };
        let id = self.new_object(Some(def), chars, player, player, zone);
        self.obj_mut(id).since = 0;
        self.obj_mut(id).entered = 0;
        if zone == Zone::Stack { self.capture_spell(id); }
        id
    }

    /// Setting a game up: a permanent put onto the battlefield is tapped. Nothing happened: no
    /// event is logged.
    pub fn set_tapped(&mut self, permanent: ObjId, tapped: bool) {
        self.obj_mut(permanent).tapped = tapped;
    }

    /// A declared settled scenario premise, not an opening or saved-game restoration.
    /// No entry, step-beginning or turn-based actions are executed. A draw-step
    /// position is after its draw (504.1–2); the supplied hand/library are final.
    pub fn settle_initial_position(&mut self, turn: u32, part: Part, active: PlayerId,
        priority: PlayerId, passes: usize, controls: &[(ObjId, PlayerId, bool)]) -> Result<(), String> {
        let before=self.clone();
        let result=self.prepare_initial_position(turn,part,active,priority,passes,controls)
            .and_then(|()|self.finish_initial_position());
        if result.is_err() {self.adopt(before);}
        result
    }

    /// Establish timing and base controllers before installing existing declared
    /// causes. Does not settle an incomplete board; call finish_initial_position
    /// after all setup fields and retained causes have been installed.
    pub fn prepare_initial_position(&mut self, turn: u32, part: Part, active: PlayerId,
        priority: PlayerId, passes: usize, controls: &[(ObjId, PlayerId, bool)]) -> Result<(), String> {
        if self.players.len()!=2 || turn==0 || active!=(turn as usize-1)%2 || priority>=2 || passes>1
            || !matches!(part,Part::Upkeep|Part::Draw|Part::PrecombatMain|Part::PostcombatMain|Part::BeginningOfCombat|Part::EndOfCombat|Part::End)
            || !self.stack.is_empty() || !self.command.is_empty() || self.decision.is_some()
            || self.combat!=crate::combat::Combat::default() {
            return Err("unsupported initial priority/turn/history".into());
        }
        if turn==1 && part==Part::Draw {
            return Err("initial draw step is skipped on the first turn (103.8a)".into());
        }
        if crate::history::initial_cast_history_needs_more(&self.defs) {
            return Err("initial position cannot restore history-sensitive definitions".into());
        }
        for id in &self.battlefield {
            if self.obj(*id).card.is_some_and(|index| {
                let physical=&self.defs[index];
                physical.is_room() || physical.back.is_some() || !physical.halves.is_empty()
            }) {
                return Err("initial permanent requires unsupported face/unlocked designation".into());
            }
            let def=self.def(*id).ok_or("initial permanent lacks a definition")?;
            // Relative timestamps, copy/entry choices and other static state need
            // their own premises. Refusing text/statics avoids guessing that order.
            if !def.characteristics.is_empty() || def.abilities.iter().enumerate().any(|(i,ability)|
                !matches!(ability,ir::Ability::Activated{..}|ir::Ability::Triggered{..}|ir::Ability::Keywords(_))
                && !def.replacements.iter().any(|r|r.ability==i && r.result.as_ref().is_ok_and(crate::replace::Replacement::initial_without_history))) {
                return Err("initial permanent requires unsupported static/entry/ability history".into());
            }
        }
        if controls.len()!=self.battlefield.len() || controls.iter().map(|(id,_,_)|*id).collect::<std::collections::BTreeSet<_>>().len()!=controls.len()
            || controls.iter().any(|(id,by,_)|*by>=2 || !self.battlefield.contains(id)) {
            return Err("initial control duration must cover every permanent".into());
        }
        self.turn=turn; self.part=part; self.active=active; self.priority=priority; self.passes=passes;
        self.plan=crate::turn::TurnPlan::at(turn,part,0);
        for (player,state) in self.players.iter_mut().enumerate() {
            state.turn_began=if player==active {turn}else{turn.saturating_sub(1)};
            state.turns_taken=turn/2+u32::from(player==0 && turn%2==1);
            state.land_played=false;
        }
        for (id,by,this_turn) in controls {
            let object=self.obj_mut(*id); object.controller=*by;
            object.since=if *this_turn {turn}else{0};
        }
        Ok(())
    }

    /// Check the complete declared board at priority (117.5/704.3). Any required
    /// action, trigger or decision refuses the premise instead of repairing it.
    /// On failure, preserve the unfinalized setup for inspection.
    pub fn finish_initial_position(&mut self) -> Result<(), String> {
        let log=self.log.end();
        // Any failure restores the position given, as an unsupported action does.
        self.guarded(|game| {
            game.settle().map_err(|error| crate::Unsupported(error.to_string()))?;
            if game.log.end()!=log || game.finished() || !game.stack.is_empty() || game.decision.is_some() {
                return Err(crate::Unsupported("initial position is not settled before priority (117.5/704.3)".into()).into());
            }
            Ok(())
        }).map_err(|error| error.to_string())
    }

    /// Fixture-only consumption of the ordinary land play (305.2/305.4).
    /// No land-play event or retrospective land entry is manufactured.
    pub fn set_initial_land_plays(&mut self, uses:[usize;2]) -> Result<(),String> {
        if self.players.len()!=2 || uses.iter().any(|uses|*uses>1) || uses[1-self.active]!=0 {
            return Err("unsupported initial land-play consumption".into());
        }
        for (player,uses) in self.players.iter_mut().zip(uses) {player.land_played=uses==1;}
        Ok(())
    }

    /// Fixture-only marked damage; no damage event, source, deathtouch or trigger is invented.
    pub fn set_initial_damage(&mut self, object:ObjId, damage:i32) -> Result<(),String> {
        if damage<0 || !self.battlefield.contains(&object) {return Err("invalid initial marked damage".into());}
        self.obj_mut(object).damage=damage;
        Ok(())
    }

    /// Fixture setup only: exact counters, without replacements, events or state-based actions.
    /// Running games must use the retained counter operation, not this initializer.
    pub fn set_initial_counters(&mut self, object: ObjId, kind: &str, count: u32) {
        crate::layers::counter_ok(kind).expect("supported counter kind");
        let stamp = self.tick();
        let o = self.obj_mut(object);
        if count == 0 { o.counters.remove(kind); o.counter_stamps.remove(kind); }
        else { o.counters.insert(kind.to_owned(), count); o.counter_stamps.insert(kind.to_owned(), stamp); }
    }

    /// A zone change (400.7): the object leaves and a new object arrives, under `controller`'s
    /// control on the battlefield or the stack, otherwise in its owner's zone. A token that
    /// leaves the battlefield remains in its destination until state-based actions (111.7).
    /// Returns the new object; tokens cannot change zones again (111.8).
    ///
    /// A permanent enters the battlefield as the replacement effects that modify how it
    /// enters have it (614.12), which are asked before it moves ([`crate::replace`]): tapped
    /// from the first, with counters that are put on it as it enters (122.6).
    pub(crate) fn move_to(&mut self, id: ObjId, to: Zone, controller: PlayerId) -> Option<ObjId> {
        self.move_as(id, to, controller, &Choices::default())
    }

    /// A zone change in which a permanent enters the battlefield also as a choice made before
    /// it entered has it (614.12a): conditional arrivals and linked choice records.
    pub(crate) fn move_as(&mut self, id: ObjId, to: Zone, controller: PlayerId, chosen: &Choices) -> Option<ObjId> {
        let destination = self.departure_destination(id, to);
        let to = destination.zone;
        // Ordinary non-stack entry uses the front face; a modal instant front cannot
        // enter by substituting its land back (712.14b).
        if to==Zone::Battlefield && self.obj(id).zone!=Zone::Stack && !chosen.transformed
            && self.obj(id).card.is_some_and(|i|self.defs[i].modal && !self.defs[i].types.iter().any(|t|t.permanent())) {
            return None;
        }
        if to == Zone::Battlefield && self.def(id).is_some_and(crate::attach::aura)
            && !chosen.attached_to.is_some_and(|t| self.can_enchant(id, controller, t))
        {
            // An Aura with nothing legal to enchant stays where it was (303.4g, i).
            if self.obj(id).zone == Zone::Stack { self.move_to(id, Zone::Graveyard, controller); }
            return None;
        }
        let entry = if to == Zone::Battlefield { self.entry_as(Some(id), controller, chosen) } else { Entry::default() };
        self.move_single(id, destination, controller, entry)
    }

    /// The zone change itself, a permanent entering the battlefield as `entry` says.
    pub(crate) fn move_with(&mut self, id: ObjId, to: Zone, controller: PlayerId, entry: Entry) -> Option<ObjId> {
        let destination = self.departure_destination(id, to);
        self.move_single(id, destination, controller, entry)
    }

    // Low-level single movements own a fallback view; departure batches capture once
    // around all their commits instead (603.10a).
    fn move_single(&mut self, id: ObjId, destination: crate::departure::Destination, controller: PlayerId, entry: Entry) -> Option<ObjId> {
        let from = self.log.end();
        let before = (self.obj(id).zone == Zone::Battlefield).then(|| self.departure_snapshot(&self.battlefield));
        let now = self.move_prepared(id, destination, controller, entry);
        if let Some(before) = before { self.finish_simultaneous(from, before); }
        now
    }

    pub(crate) fn move_prepared(&mut self, id: ObjId, destination: crate::departure::Destination, controller: PlayerId, entry: Entry) -> Option<ObjId> {
        self.move_deferring(id, destination, controller, entry, None)
    }

    /// [`Game::move_prepared`] as one part of a simultaneous event that may also put cards at
    /// specified positions in a library: the shuffle a replacement's modified event includes
    /// is left in `shuffles`, the library with the cards it hides (701.24a), for the event to
    /// do once those cards are in place (701.24g).
    pub(crate) fn move_deferring(&mut self, id: ObjId, destination: crate::departure::Destination, controller: PlayerId, entry: Entry,
                                 shuffles: Option<&mut Vec<(PlayerId, Vec<ObjId>)>>) -> Option<ObjId> {
        let movement_allowed = destination.zone != Zone::Exile || self.can(Verb::Exile, controller, Target::Object(id));
        let owner = self.obj(id).owner;
        // "reveal Darksteel Colossus and shuffle it into its owner's library instead": the
        // modified event, done in place of the one replaced (614.6)
        let by = destination.by;
        if by.as_ref().is_some_and(|b| b.shuffle) { self.reveal(owner, vec![id]); }
        // A replacement still replaces the original event when its move is impossible
        // (614.6, 609.3); independent instructions remain applicable.
        let now = movement_allowed.then(|| self.commit_move(id, destination.zone, controller, entry)).flatten();
        if let (Some(now), Some(position)) = (now, destination.position) {
            self.arrange_library(self.obj(now).owner, &[now], position);
        }
        if let Some(by) = by {
            if by.shuffle && let Some(now) = now {
                // revealed as it was, it is not known where it now is (701.24a)
                let mut revealed = self.resolving().and_then(|r| r.program_frame()).map(|f| f.known_cards()).unwrap_or_default();
                revealed.push(now);
                match shuffles {
                    Some(shuffles) => shuffles.push((owner, revealed)),
                    None => self.shuffle_except(owner, &[], &revealed),
                }
            }
            // "instead exile it and you gain 2 life"
            // A zone change cannot return a refusal: one is deferred to the action's end,
            // which restores the game (107.1b). Outside an action only fixtures move objects.
            if let Some((player, life)) = by.gain && let Err(why) = self.gain_life(player, life) {
                assert!(self.acting(), "fixture zone change: {}", why.0);
                self.defer_refusal(why);
            }
            // "When you do, …": it triggers as the replacement is applied, for its source's
            // controller as it was then (603.12, 603.10a)
            if let Some(program) = by.then.filter(|_| now.is_some()) {
                let source = by.id.source;
                let ability = by.id.ability().expect("reflexive followup belongs to an ability").clone();
                let ctx = crate::select::Ctx::new(self.controller(source), source).with_key(ability.clone());
                let captured = crate::program::reflexive_capture(ctx.clone(), program);
                let bound = Bound { linked_ability: Some(ability.clone()), ..Default::default() };
                self.triggered.push(Triggered { source: Some(source), controller: ctx.controller, origin: ability.origin.clone(), index: 0,
                    bound, delayed: Some(captured.identity()), captured: Some(Ok(captured)) });
            }
        }
        now
    }

    fn commit_move(&mut self, id: ObjId, to: Zone, controller: PlayerId, entry: Entry) -> Option<ObjId> {
        // A token that has left the battlefield stays in that zone until cessation;
        // even a return during the same resolving effect cannot move it again (111.8).
        if self.obj(id).token() && self.obj(id).zone != Zone::Battlefield { return None; }
        // A copy of a card leaves the zone it was made in only by being cast (707.12);
        // it stays there until it ceases to exist (707.10a).
        if self.obj(id).card_copy && self.obj(id).zone != Zone::Stack && to != Zone::Stack { return None; }
        // A face-down permanent's owner reveals it as it leaves the battlefield, and a
        // face-down spell's as it leaves the stack other than for the battlefield (708.9): the
        // face-down object is shown as the card it is; what it becomes is another object.
        let zone = self.obj(id).zone;
        if (zone == Zone::Battlefield || zone == Zone::Stack && to != Zone::Battlefield) && matches!(self.obj(id).face, crate::face::Face::Down(_)) {
            let owner = self.obj(id).owner;
            self.show_face_down(owner, &[id], true);
        }
        // What an object was in the zone it just left is kept (608.2h), including color
        // changes to spells and cards. This snapshot belongs to the old identity only.
        if self.obj(id).zone != Zone::Gone {
            let last = self.seen(id, None);
            let attachments = self.inverse_attachments(&crate::select::Ctx::new(controller, id), id);
            self.obj_mut(id).last = Some(last);
            self.obj_mut(id).last_attachments = attachments;
        }
        let old = self.obj(id).clone();
        if let Some(list) = self.zone_list(old.zone, old.owner) {
            list.retain(|x| *x != id);
        }
        self.obj_mut(id).zone = Zone::Gone;
        self.obj_mut(id).left = Some(old.zone);
        // a permanent that leaves the battlefield is removed from combat (506.4)
        if old.zone == Zone::Battlefield {
            self.combat.remove(id);
        }
        let now = {
            let controller = if matches!(to, Zone::Battlefield | Zone::Stack) { controller } else { old.owner };
            // A split card has its combined characteristics again outside the stack (709.4); a
            // Room put onto the battlefield has neither door unlocked (709.5d).
            let chars = old.token_faces.as_ref().map(|faces| faces.front.chars.clone())
                .or_else(|| old.card.map(|i| if to == Zone::Battlefield { self.defs[i].room([false; 2]).chars() } else { self.defs[i].chars() }))
                .unwrap_or_else(|| old.chars.clone());
            Some(if let Some(prepared) = entry.prepared {
                self.obj_mut(prepared).zone = to;
                if let Some(list) = self.zone_list(to, old.owner) { list.push(prepared); }
                prepared
            } else { self.new_object(old.card, chars, old.owner, controller, to) })
        };
        if let Some(now) = now { self.obj_mut(now).card_id = old.card_id; }
        // A token keeps its creating baseline until cessation (111.3, 707.2), with
        // its front face up outside the battlefield (712.8a). Later copy effects end
        // on the zone change, as they do for physical cards.
        if (old.token() || old.card_copy) && let Some(now) = now {
            self.obj_mut(now).token_faces = old.token_faces.clone();
            // A permanent spell copy becomes a token on resolution (111.13). Its entry
            // copy replacement, if any, is already on the prepared permanent.
            self.obj_mut(now).card_copy = old.card_copy && to != Zone::Battlefield;
            if to == Zone::Stack { self.obj_mut(now).prepare_source = old.prepare_source; }
        }
        // The general new-object rule has an exception for effects on permanent spells:
        // they continue onto the permanent that spell becomes (112.4, 400.7a), including
        // while entry replacements and triggers inspect it. Extend the same effect so
        // its original timestamp and duration remain unchanged; later zone changes do
        // not inherit it. The old identity retains its last-known characteristics.
        if let Some(permanent) = now.filter(|_| to == Zone::Battlefield)
            && old.zone == Zone::Stack && old.ability.is_none()
            && old.last.as_ref().is_some_and(|seen| seen.types.iter().any(|t| t.permanent()))
        {
            // its abilities linked to its kicker ask of the spell it was (607.2, 702.33e)
            self.obj_mut(permanent).kicked = old.kicked;
            self.obj_mut(permanent).optional_paid = old.optional_paid;
            self.obj_mut(permanent).alternative = old.alternative;
            self.obj_mut(permanent).cast_announcement = old.cast_announcement.clone();
            self.obj_mut(permanent).spell_x = old.x;
            for effect in &mut self.continuous {
                if effect.affected.contains(&id) { effect.affected.push(permanent); }
            }
            // its warp cost was paid: the delayed exile of what it became (702.185a)
            self.warp_resolved(id, permanent);
        }
        let attacking = entry.attacking;
        let frozen_counters = entry.frozen_counters.clone();
        let counters = match now {
            Some(now) if to == Zone::Battlefield => self.arrive(now, entry),
            _ => Vec::new(),
        };
        if let Some(now) = now {
            self.obj_mut(now).arrived = Some(self.log.end());
            // a card is put on the stack only by being cast (601.2a)
            if to == Zone::Stack { self.obj_mut(now).cast_from = Some(old.zone); }
        }
        self.obj_mut(id).successor = now.filter(|_| matches!(to, Zone::Graveyard | Zone::Battlefield | Zone::Stack | Zone::Exile));
        let moved = Event::ZoneChanged { was: id, now, from: old.zone, to };
        self.event_log_mut().push(moved.clone());
        if let Some(aura) = now.filter(|_| to == Zone::Battlefield)
            && let Some(to) = self.attached_to(aura) {
            self.event_log_mut().push(Event::Attached { aura, to });
        }
        if let Some(now) = now {
            if let Some(inventory) = frozen_counters { inventory.install(self,now); }
            else {
                // Legacy direct fixture movement; production entry carries a frozen inventory.
                self.commit_entry_counters(now, counters, controller).expect("bounded fixture entry counters");
            }
        }
        if let (Some(permanent), Some(defender)) = (now, attacking) { self.enter_attacking(permanent, defender); }
        // Entering counters and combat status are part of the entry event (122.6, 508.4).
        // Event-time characteristic history must observe the complete entrant.
        self.note(&moved);
        self.end_control_durations();
        now
    }

    // ------------------------------------------------------------ what things are

    /// Is this thing of this sort, now? A permanent is a card or token on the battlefield
    /// (110.1), a spell a card on the stack (112.1), an ability on the stack an object of its
    /// own (113.1c). A represented card remains a card in those zones too (108.2); tokens
    /// and stack abilities are not cards. A player who has lost has left the
    /// game (104.3a, 800.4a).
    pub fn is(&self, t: Target, sort: Sort) -> bool {
        let o = match t {
            Target::Player(p) => return sort == Sort::Player && !self.players[p].lost,
            Target::Object(o) => self.obj(o),
        };
        match sort {
            Sort::Player => false,
            Sort::Permanent => o.zone == Zone::Battlefield,
            // what card types it has now (613.1d)
            Sort::Of(t) => o.zone == Zone::Battlefield && self.is_a(o.id, t),
            Sort::Spell => o.zone == Zone::Stack && o.ability.is_none(),
            Sort::Ability => o.zone == Zone::Stack && o.ability.is_some(),
            Sort::Card => o.zone != Zone::Gone && o.card.is_some() && o.ability.is_none(),
        }
    }

    /// Can this thing fill this role of the action? The lexicon's declaration, which the
    /// grammar type-checks text against, asked of the game as it is: a creature can be dealt
    /// damage, a land that is not one cannot (120.1); only a permanent is destroyed (701.8a).
    pub fn takes(&self, verb: Verb, role: Role, t: Target) -> bool {
        verb.kinds(role).iter().any(|k| self.is(t, *k))
    }

    /// Is what an action's rule needs first true of this thing, for the player who acts?
    fn meets(&self, need: Needs, by: PlayerId, o: ObjId) -> bool {
        let obj = self.obj(o);
        match need {
            Needs::Untapped => !obj.tapped,
            Needs::Tapped => obj.tapped,
            Needs::Controlled => self.controller(obj.id) == by,
            Needs::InHand => self.players[by].hand.contains(&o),
        }
    }

    /// Can this player do this action to this thing, now? It is of a sort the action takes,
    /// and what the action's rule needs first is true of it: only an untapped permanent is
    /// tapped (701.26a), a player sacrifices only a permanent they control (701.21a) and
    /// discards from their own hand (701.9a). The lexicon's declaration again; the actions ask
    /// it before they happen, and a cost before it is paid (118.3).
    pub fn can(&self, verb: Verb, by: PlayerId, what: Target) -> bool {
        self.takes(verb, Role::What, what)
            && (verb != Verb::Exile || !self.forbidden(&crate::trigger::Fact::new(Verb::Exile, None, vec![(Role::What, what)])))
            && match what {
                Target::Object(o) => verb.needs().iter().all(|n| self.meets(*n, by, o)),
                Target::Player(_) => true,
            }
    }

    /// Everything in the game that can fill this role of the action: "what can be destroyed?"
    pub fn all_that(&self, verb: Verb, role: Role) -> Vec<Target> {
        let players = (0..self.players.len()).map(Target::Player);
        let objects = self.objects.iter().map(|o| Target::Object(o.id));
        players.chain(objects).filter(|t| self.takes(verb, role, *t)).collect()
    }

    // ------------------------------------------------------------ the actions of the lexicon

    /// Mill (701.17): the top cards of the library go to the graveyard, as many as there are.
    pub fn mill(&mut self, player: PlayerId, n: u32) -> Vec<ObjId> {
        let mut milled = Vec::new();
        for _ in 0..n {
            let Some(top) = self.top_cards(player, 1).first().copied() else { break };
            if let Some(card) = self.move_to(top, Zone::Graveyard, player) {
                self.event_log_mut().push(Event::Milled { player, card });
                milled.push(card);
            }
        }
        milled
    }

    /// The top `n` cards of a player's library, the first of them the top card: what a player
    /// who scries or surveils looks at (701.22a, 701.25a), as many as there are.
    pub fn top_cards(&self, player: PlayerId, n: usize) -> Vec<ObjId> {
        // Tokens can briefly remain here during a resolving effect (111.7), but are
        // not cards (111.6). Drawing, milling and looking at top cards all use this query.
        self.players[player].library.iter().rev().copied()
            .filter(|o| self.is(Target::Object(*o), Sort::Card)).take(n).collect()
    }

    /// Show the cards the action can still apply to (701.20a–c). A reference to an
    /// object that no longer exists cannot reveal its replacement (400.7, 701.20d).
    /// The resolving effect retains the returned identities for the reveal's lifetime.
    pub(crate) fn reveal(&mut self, player: PlayerId, cards: Vec<ObjId>) -> Vec<ObjId> {
        let cards: Vec<_> = cards.into_iter().filter(|o| self.takes(Verb::Reveal, Role::What, Target::Object(*o))).collect();
        if !cards.is_empty() { self.event_log_mut().push(Event::Revealed { player, cards: cards.clone() }); }
        self.disclose(crate::observation::Kind::Revealed, player, &cards);
        cards
    }

    /// Behold these (701.4a), as a cost of a spell or an effect, or as a permanent enters: a
    /// card of the hand is revealed, shown to all players (701.20a); a permanent the player
    /// controls is chosen, which every player is told of. Nothing else happens to either.
    pub(crate) fn behold(&mut self, player: PlayerId, objects: Vec<ObjId>) {
        let (cards, permanents): (Vec<_>, Vec<_>) = objects.into_iter().partition(|o| self.obj(*o).zone == Zone::Hand);
        self.reveal(player, cards);
        if !permanents.is_empty() { self.event_log_mut().push(Event::Beheld { player, permanents }); }
    }

    /// Is this a way to put back these cards a player looked at as they scry or surveil: each
    /// of them once, on top or away?
    pub fn arrangement(cards: &[ObjId], top: &[ObjId], away: &[ObjId]) -> bool {
        let all: Vec<ObjId> = top.iter().chain(away).copied().collect();
        all.len() == cards.len() && cards.iter().all(|c| all.contains(c))
    }

    /// Deal noncombat damage as an effect through the retained rules operation (120.4).
    pub fn damage(&mut self, source: ObjId, to: Target, amount: u32) {
        self.try_damage(source, to, amount).expect("bounded fixture damage results");
    }

    pub fn try_damage(&mut self, source: ObjId, to: Target, amount: u32) -> crate::Res<()> {
        self.guarded(|game| Ok(game.damage_as_rule(&[(source,to,amount)],false)?)).map_err(|error| match error {
            crate::play::ActionError::Unsupported(reason) => reason,
            other => crate::Unsupported(other.to_string()),
        })
    }

    /// A turn-based or public damage action retains its parent until all results commit.
    pub(crate) fn damage_as_rule(&mut self, damage: &[(ObjId, Target, u32)], combat: bool) -> crate::Res<()> {
        let parent = self.procedures.len();
        if let Err(decision) = self.damage_operation(damage, combat,if combat {crate::operation::EventOrigin::TurnBased}else{crate::operation::EventOrigin::Effect})? {
            self.procedures.insert(parent, crate::procedure::Procedure::RuleAction { complete: false });
            self.decision = Some(decision);
        }
        Ok(())
    }

    /// Would this much damage from these sources together be lethal to this creature (120.6)?
    /// With what is already marked on it, it is at least its toughness; any amount is when one
    /// of the sources has deathtouch (702.2c).
    pub fn lethal(&self, creature: ObjId, sources: &[ObjId], amount: u32) -> bool {
        let o = self.obj(creature);
        let deathtouch = amount > 0 && sources.iter().any(|s| self.has(*s, "Deathtouch"));
        deathtouch || self.toughness(creature).is_some_and(|t| o.damage + amount as i32 >= t)
    }

    /// Destroy (701.8): a permanent goes from the battlefield to its owner's graveyard,
    /// unless an effect says it can't be destroyed (101.2: indestructible, 702.12b), or a
    /// regeneration shield replaces its destruction (701.19a). Returns whether it was
    /// destroyed.
    pub fn destroy(&mut self, object: ObjId) -> bool {
        self.destroy_as(object, true)
    }

    /// Destroy a permanent that can't be regenerated: its regeneration shields are not
    /// applied (701.19c).
    pub fn destroy_for_good(&mut self, object: ObjId) -> bool {
        self.destroy_as(object, false)
    }

    pub(crate) fn destruction_allowed(&self, object: ObjId) -> bool {
        let what = Target::Object(object);
        self.takes(Verb::Destroy, Role::What, what)
            && !self.forbidden(&Fact::new(Verb::Destroy, None, vec![(Role::What, what)]))
    }

    fn destroy_as(&mut self, object: ObjId, regenerable: bool) -> bool {
        if !self.destruction_allowed(object) { return false; }
        self.destroy_allowed(object, regenerable)
    }

    /// Perform a permitted destruction, applying regeneration. A simultaneous SBA batch
    /// determines permission before any of its other actions changes the battlefield.
    pub(crate) fn destroy_allowed(&mut self, object: ObjId, regenerable: bool) -> bool {
        // "The next time [it] would be destroyed this turn, instead remove all damage marked
        // on it and its controller taps it. If it's an attacking or blocking creature, remove
        // it from combat." (701.19a): the destruction never happens (614.6), and the shield
        // has done what it does once
        if let Some(shield) = self.shields.iter().position(|s| *s == object).filter(|_| regenerable) {
            self.shields.remove(shield);
            let o = self.obj_mut(object);
            o.damage = 0;
            o.deathtouched = false;
            self.tap(object);
            self.combat.remove(object);
            self.event_log_mut().push(Event::Regenerated { object });
            return false;
        }
        self.event_log_mut().push(Event::Destroyed { object });
        let c = self.controller(object);
        self.move_to(object, Zone::Graveyard, c);
        true
    }

    /// Characteristics immediately before a simultaneous event that may move these objects.
    pub(crate) fn departure_snapshot(&self, objects: &[ObjId]) -> crate::layers::Working {
        let seen = objects.iter().copied().filter(|o| self.obj(*o).zone != Zone::Gone)
            .map(|o| (o, self.seen(o, None))).collect();
        crate::layers::Working::before(seen).freeze_attachments(self.attachment_snapshot())
    }

    fn attachment_snapshot(&self) -> Vec<(ObjId, Target)> {
        self.battlefield.iter().filter_map(|id| self.obj(*id).attached_to.map(|to| (*id, to))).collect()
    }

    /// Complete one event before checking its triggers (603.10a, 608.2h). Zone changes
    /// retain the pre-event characteristics, and leaving objects see each other's departures.
    pub(crate) fn finish_simultaneous(&mut self, from: crate::event::Seq, before: crate::layers::Working) {
        for (id, last) in &before.objects {
            if self.obj(*id).zone == Zone::Gone {
                let ctx = crate::select::Ctx { view: Some(&before), ..crate::select::Ctx::new(last.controller, *id) };
                let attachments = self.inverse_attachments(&ctx, *id);
                self.obj_mut(*id).last = Some(last.clone());
                self.obj_mut(*id).last_attachments = attachments;
            }
        }
        self.reconcile_linked_acquisitions();
        let end = self.log.end();
        self.together.push((from, end));
        if !before.objects.is_empty() {
            self.lookbacks.push((from, end, before));
        }
    }

    /// Regenerate (701.19a), as the effect of a spell or ability that resolves: a replacement
    /// effect that protects the permanent the next time it would be destroyed this turn, a
    /// regeneration shield. Nothing is regenerated yet (701.19c).
    pub fn regenerate(&mut self, permanent: ObjId) {
        if self.takes(Verb::Regenerate, Role::What, Target::Object(permanent)) {
            self.shields.push(permanent);
        }
    }

    /// Exile (701.13a): an object goes to the exile zone from wherever it is.
    pub fn exile(&mut self, object: ObjId) {
        let _ = self.exile_result(object);
    }

    /// The identity produced by this public-zone move (400.7j), if it succeeded.
    /// Later instructions in the same effect bind this result, not the departed ID.
    pub(crate) fn exile_with(&mut self, object: ObjId, source: ObjId, ability: Option<crate::binding::AbilityKey>, generation: Option<usize>) -> Option<ObjId> {
        let now = self.exile_result(object)?;
        if self.obj(now).zone == Zone::Exile { self.exiled_with.push((source, ability, now, generation)); }
        Some(now)
    }

    pub(crate) fn exile_result(&mut self, object: ObjId) -> Option<ObjId> {
        if !self.can(Verb::Exile, self.controller(object), Target::Object(object)) { return None; }
        self.event_log_mut().push(Event::Exiled { object });
        let c = self.controller(object);
        self.move_to(object, Zone::Exile, c)
    }

    /// Counter a spell or ability (701.6): remove it without resolving it. Costs are not
    /// refunded (701.6b). A spell goes to its owner's graveyard, subject to replacements;
    /// an ability ceases to exist. Being uncounterable does not make it an illegal target.
    pub fn counter(&mut self, object: ObjId) -> bool {
        self.counter_to(object,Zone::Graveyard)
    }
    /// A countering effect can replace the spell's destination (701.6a, 614.1a).
    /// A prohibited counter changes nothing, including its destination.
    pub(crate) fn counter_to(&mut self, object: ObjId, destination: Zone) -> bool {
        if !self.countered(object) { return false; }
        if self.obj(object).ability.is_none() {
            if destination == Zone::Exile { self.exile_result(object); }
            else { self.move_to(object, destination, self.controller(object)); }
        }
        true
    }

    /// Counter it, if it can be: an ability ceases to exist; a spell is left to be put where
    /// countering puts it (701.6a). Has it been countered?
    pub(crate) fn countered(&mut self, object: ObjId) -> bool {
        let target = Target::Object(object);
        if !self.takes(Verb::Counter, Role::What, target) || self.obj(object).uncounterable
            || self.forbidden(&Fact::new(Verb::Counter, None, vec![(Role::What, target)]))
        {
            return false;
        }
        self.event_log_mut().push(Event::Countered { object });
        if self.obj(object).ability.is_some() { self.ability_gone(object); }
        true
    }

    /// Return or put an object into a zone of its owner.
    pub fn move_object(&mut self, object: ObjId, to: Zone) {
        self.move_result(object, to);
    }

    pub(crate) fn move_result(&mut self, object: ObjId, to: Zone) -> Option<ObjId> {
        if self.takes(Verb::Move, Role::What, Target::Object(object)) {
            self.event_log_mut().push(Event::Moved { object, to });
            let c = self.controller(object);
            return self.move_to(object, to, c);
        }
        None
    }

    /// The new object this one became as it last changed zones, when the zone it moved to is
    /// a public zone (400.2: graveyard, battlefield, stack, exile), where it can be found
    /// (400.7e).
    pub(crate) fn became(&self, was: ObjId) -> Option<ObjId> {
        self.obj(was).successor
    }

    /// The permanents that left the battlefield at the same time as this one, itself among
    /// them: in the same event of the log (603.10a).
    pub(crate) fn left_with(&self, was: ObjId) -> Vec<ObjId> {
        let left = |e: &Event| match e {
            Event::ZoneChanged { was, from: Zone::Battlefield, .. } => Some(*was),
            _ => None,
        };
        let Some((at, _)) = self.log.sequenced().rev().find(|(_, e)| left(e) == Some(was)) else { return vec![was] };
        match self.together.iter().rev().find(|(from, to)| *from <= at && at < *to) {
            Some((from, to)) => self.log.range(*from..*to).iter().filter_map(left).collect(),
            None => vec![was],
        }
    }

    /// Discard (701.9a): a card goes from its owner's hand to that player's graveyard, or
    /// where a replacement of its discard puts it ([`crate::discard`]).
    pub fn discard(&mut self, player: PlayerId, card: ObjId) {
        self.discard_caused(player, card, None);
    }

    /// Sacrifice (701.21a): its controller moves a permanent from the battlefield to its
    /// owner's graveyard. Not destruction: nothing that replaces destruction applies.
    pub fn sacrifice(&mut self, player: PlayerId, permanent: ObjId) {
        if self.can(Verb::Sacrifice, player, Target::Object(permanent)) {
            self.event_log_mut().push(Event::Sacrificed { player, object: permanent });
            self.move_to(permanent, Zone::Graveyard, player);
        }
    }

    /// Can a player pay this much life (119.4)? Only with a life total at least that; 0
    /// always (119.4b).
    pub fn can_pay_life(&self, player: PlayerId, amount: u32) -> bool {
        amount == 0 || i64::from(self.players[player].life) >= i64::from(amount)
    }

    /// Pay life (119.4): the player loses that much life. Returns whether it was paid.
    pub fn pay_life(&mut self, player: PlayerId, amount: u32) -> crate::Res<bool> {
        let can = self.can_pay_life(player, amount);
        if can {
            self.lose_life(player, amount)?;
        }
        Ok(can)
    }

    /// Gain life (119.3), unless an effect says the player can't (101.2, 119.7).
    pub fn gain_life(&mut self, player: PlayerId, amount: u32) -> crate::Res<()> {
        let amount = self.life_gain_amount(player, amount)?;
        self.gain_life_result(player, amount)
    }

    pub(crate) fn life_gain_amount(&self, player: PlayerId, amount: u32) -> crate::Res<u32> {
        if amount == 0 || self.forbidden(&Fact::new(Verb::GainLife, Some(Target::Player(player)), Vec::new())) { return Ok(0); }
        self.replace_life_gain(player, amount)
    }

    /// The player's life total changed by `by`. A life total has no bound in the rules
    /// (119.3, 107.1b); one this engine cannot represent is refused, never wrapped or clamped.
    pub(crate) fn changed_life(&self, player: PlayerId, by: i64) -> crate::Res<i32> {
        i32::try_from(i64::from(self.players[player].life) + by)
            .map_err(|_| crate::Unsupported("life total exceeds the supported numerical range (107.1b)".into()))
    }

    /// Apply an already determined life-gain result, including one of simultaneous damage.
    pub(crate) fn gain_life_result(&mut self, player: PlayerId, amount: u32) -> crate::Res<()> {
        if amount > 0 {
            *self.life_total_mut(player) = self.changed_life(player, i64::from(amount))?;
            self.event_log_mut().push(Event::LifeGained { player, amount });
        }
        Ok(())
    }

    /// Lose life (119.3). Refused before any change when the result is not representable.
    pub fn lose_life(&mut self, player: PlayerId, amount: u32) -> crate::Res<()> {
        if amount > 0 {
            let life = self.changed_life(player, -i64::from(amount))?;
            self.note_life_loss(player, amount)?;
            *self.life_total_mut(player) = life;
            self.event_log_mut().push(Event::LifeLost { player, amount });
        }
        Ok(())
    }

    /// Add mana (106.4): it goes into the player's mana pool, where it stays until it is
    /// spent or the step or phase ends.
    pub fn add_mana(&mut self, player: PlayerId, mana: ManaSymbol) {
        match mana {
            ManaSymbol::Color(c) => self.mana_pool_mut(player).colors[c as usize] += 1,
            _ => self.mana_pool_mut(player).colorless += 1,
        }
    }

    /// Add mana whose text restricts how it can be spent (106.4, 106.6): it goes into the
    /// pool as itself, remembering what it may be spent on.
    pub fn add_restricted_mana(&mut self, player: PlayerId, mana: Restricted) {
        let mana = match mana.mana {
            ManaSymbol::Color(_) => mana,
            _ => Restricted { mana: ManaSymbol::Colorless, ..mana },
        };
        self.mana_pool_mut(player).restricted.push(mana);
    }

    /// Has this permanent not been under its controller's control continuously since their
    /// most recent turn began (302.6)? A creature that has not can't attack, and its abilities
    /// with {T} or {Q} in their cost can't be activated (602.5a), unless it has haste (702.10b).
    pub fn summoning_sick(&self, permanent: ObjId) -> bool {
        let o = self.obj(permanent);
        // what a game was set up with has been its controller's since before any turn began,
        // also for a player whose first turn has not begun yet
        o.since != 0 && o.since >= self.players[self.controller(permanent)].turn_began && !self.has(permanent, "Haste")
    }

    /// Tap (701.26a): only an untapped permanent can be tapped.
    pub fn tap(&mut self, object: ObjId) {
        if self.can(Verb::Tap, self.controller(object), Target::Object(object)) {
            *self.tapped_mut(object) = true;
            self.event_log_mut().push(Event::Tapped { object });
        }
    }

    /// Untap (701.26b): only a tapped permanent can be untapped.
    pub fn untap(&mut self, object: ObjId) {
        if self.can(Verb::Untap, self.controller(object), Target::Object(object)) {
            // All stun counters create one replacement, removing one counter per attempt (122.1d).
            if self.obj(object).counters.get("stun").copied().unwrap_or(0)>0 {
                self.remove_counters(object,"stun",1);return;
            }
            *self.tapped_mut(object) = false;
            self.event_log_mut().push(Event::Untapped { object });
        }
    }

    /// Transform a permanent (701.27a): its other face is up. It is the same object, and
    /// what applied to it still does (712.18). Nothing happens to what is not a double-faced
    /// permanent (701.27c). `since`: asked by an ability of the permanent itself put on the
    /// stack at that time, which does nothing if it has transformed since (701.27f).
    pub fn transform(&mut self, object: ObjId, since: Option<u32>) -> bool {
        let o = self.obj(object);
        if o.zone != Zone::Battlefield || since.is_some_and(|put| o.transformed_at > put) { return false; }
        // A face-down permanent has no other face to turn to (708.2a, 701.27c).
        if matches!(o.face, crate::face::Face::Down(_)) { return false; }
        let turned = !o.transformed;
        let face_chars = if let Some(faces) = &o.token_faces {
            if faces.back.is_none() { return false; }
            faces.face(turned).chars.clone()
        } else {
            let Some(card) = o.card.filter(|card| self.defs[*card].back.is_some()) else { return false };
            if turned { self.defs[card].back().unwrap().chars() } else { self.defs[card].chars() }
        };
        let chars = if o.copied.is_some() { o.chars.clone() } else { face_chars.clone() };
        if chars.types.iter().any(|t| matches!(t, CardType::Instant | CardType::Sorcery)) { return false; }
        // it receives a new timestamp as it transforms (613.7g)
        let stamp = self.tick();
        let o = self.obj_mut(object);
        if let Some(base) = &mut o.copy_effects.base {
            if base.copied.is_none() { base.chars = face_chars; }
        }
        o.transformed = turned;
        o.transformed_at = stamp;
        o.stamp = stamp;
        o.chars = chars;
        self.event_log_mut().push(Event::Transformed { object });
        true
    }

    /// A permanent's level becomes `level` (716.2a).
    /// Give a permanent the monstrous, renowned or saddled designation (701.37a, 702.112a,
    /// 702.171a); only a permanent can be or become so (701.37b, 702.112b, 702.171b).
    pub(crate) fn designate(&mut self, object: ObjId, designation: ir::condition::ObjectDesignation) {
        use ir::condition::ObjectDesignation::*;
        if self.obj(object).zone != Zone::Battlefield { return; }
        if designation == ir::condition::ObjectDesignation::Prepared { self.prepare(object); return; }
        let o = self.obj_mut(object);
        match designation {
            Solved => o.solved = true,
            Monstrous => o.monstrous = true,
            Renowned => o.renowned = true,
            Saddled => o.saddled = true,
            Suspended | Prepared => return,
        }
        self.event_log_mut().push(Event::Designated { object, designation });
    }

    pub(crate) fn become_level(&mut self, object: ObjId, level: u32) {
        if self.obj(object).zone != Zone::Battlefield { return; }
        self.obj_mut(object).level = Some(level);
        self.event_log_mut().push(Event::BecameLevel { object, level });
    }

    /// Structural holders supported by this engine (122.1). Placement prohibitions are
    /// separate: they never hide existing counters or prevent their removal.
    pub(crate) fn holds_counters(&self, holder: Target) -> bool {
        match holder {
            Target::Player(p) => self.players.get(p).is_some_and(|p| !p.lost),
            Target::Object(o) => self.objects.get(o).is_some_and(|o| o.zone == Zone::Battlefield),
        }
    }
    pub fn counters(&self, holder: impl Into<Target>) -> &BTreeMap<String, u32> {
        let holder = holder.into();
        if !self.holds_counters(holder) {
            static EMPTY: std::sync::OnceLock<BTreeMap<String,u32>> = std::sync::OnceLock::new();
            return EMPTY.get_or_init(BTreeMap::new);
        }
        match holder { Target::Player(p) => &self.players[p].counters, Target::Object(o) => &self.obj(o).counters }
    }
    pub(crate) fn counters_mut(&mut self, holder: Target) -> &mut BTreeMap<String,u32> {
        match holder {
            Target::Player(p) => {
                self.changed(crate::program::footprint::Properties::COUNTERS);
                &mut self.state_data.players[p].counters
            }
            Target::Object(o) => self.counter_fields_mut(o).0,
        }
    }
    /// Fixture placement without replacements. Runtime effects use the retained operation.
    /// Placement legality and commitment use the same frozen boundary as runtime batches.
    pub fn put_counters(&mut self, holder: impl Into<Target>, kind: &str, count: u32) {
        self.try_put_counters(holder, kind, count).expect("bounded fixture counter inventory");
    }
    /// Checked fixture placement, without replacement processing.
    pub fn try_put_counters(&mut self, holder: impl Into<Target>, kind: &str, count: u32) -> crate::Res<()> {
        let on=holder.into();
        let by=match on {Target::Player(p)=>p,Target::Object(o)=>self.controller(o)};
        let placement=crate::operation::Placement{on,kind:kind.into(),count,by};
        self.prepare_counters(vec![placement]).commit(self).map(|_|())
    }

    /// Remove counters that exist (122.1). Costs check the full amount first (118.3);
    /// effects remove as many as possible. A zone change is not counter removal (122.2).
    pub fn remove_counters(&mut self, holder: impl Into<Target>, kind: &str, count: u32) -> u32 {
        let from = holder.into();
        let removed = count.min(self.counters(from).get(kind).copied().unwrap_or(0));
        if removed == 0 { return 0; }
        let counters = self.counters_mut(from);
        let left = counters.get_mut(kind).unwrap();
        *left -= removed;
        if *left == 0 {
            counters.remove(kind);
            if let Target::Object(object) = from { self.counter_fields_mut(object).1.remove(kind); }
        }
        self.event_log_mut().push(Event::CountersRemoved { from, kind: kind.to_string(), count: removed });
        removed
    }

    /// Fixture-only installation of one token with bounded entry counters. Runtime
    /// creation uses the retained creation owner, including 701.7b replacements.
    pub fn create_token(&mut self, controller: PlayerId, chars: Chars, tapped: bool) -> ObjId {
        self.try_create_token(controller,chars,tapped).expect("bounded fixture token entry inventory")
    }
    /// Checked fixture counterpart; it also bypasses creation replacement ordering.
    pub fn try_create_token(&mut self, controller: PlayerId, chars: Chars, tapped: bool) -> crate::Res<ObjId> {
        let before=self.clone();
        let entry = self.try_entry_as(None,controller,&crate::replace::Choices::default())?;
        let token = self.new_object(None, chars, controller, controller, Zone::Battlefield);
        self.obj_mut(token).tapped = tapped;
        let counters = match self.try_arrive(token,entry) {Ok(counters)=>counters,Err(reason)=>{self.adopt(before);return Err(reason);}};
        let created = Event::Created { token };
        self.note(&created);
        self.event_log_mut().push(created);
        if let Err(reason)=self.commit_entry_counters(token,counters,controller) {
            self.adopt(before);return Err(reason);
        }
        Ok(token)
    }

    // ------------------------------------------------------------ the rules around them

    /// Run state-based checks (704.3), retaining the current batch if its controller must
    /// choose. A pending check counts as work for cleanup's priority exception (514.3a).
    pub fn state_based_actions(&mut self) -> bool {
        self.state_based_checked().expect("bounded fixture state-based entry results")
    }
    pub(crate) fn state_based_checked(&mut self) -> crate::Res<bool> {
        if self.decision.is_some() { return Ok(false); }
        self.procedures.push(crate::procedure::Procedure::StateBased(StateBased::default()));
        let result = self.drive_procedures()?;
        Ok(result.state_based || self.procedures.iter().any(|p| matches!(p, crate::procedure::Procedure::StateBased(_))))
    }

    fn sba_batch(&self) -> SbaBatch {
        // and a copy of a card that is neither on the stack nor on the battlefield (707.10a)
        let tokens = self.objects.iter().filter(|o| o.token() && !matches!(o.zone, Zone::Battlefield | Zone::Gone)
            || o.card_copy && !matches!(o.zone, Zone::Stack | Zone::Battlefield | Zone::Gone) && self.prepared_copy_controller(o.id).is_none())
            .map(|o| o.id).collect();
        let mut names: BTreeMap<(PlayerId, String), Vec<ObjId>> = BTreeMap::new();
        // Of the Roles one player controls attached to one permanent, all but the one with the
        // most recent timestamp are put into their owners' graveyards (303.7a, 704.5z).
        let role = ir::Subtype::role();
        let mut roles: Vec<((PlayerId, Target), Vec<ObjId>)> = Vec::new();
        for id in &self.battlefield {
            if let Some(on) = self.attached_to(*id) && self.seen(*id, None).subtypes.contains(&role) {
                let key = (self.controller(*id), on);
                match roles.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, group)) => group.push(*id),
                    None => roles.push((key, vec![*id])),
                }
            }
        }
        let superseded: Vec<ObjId> = roles.into_iter().flat_map(|(_, mut group)| {
            group.sort_by_key(|o| self.obj(*o).stamp);
            let kept = group.pop();
            // Roles that became attached at once are ordered by their controller's choice
            // (613.7m): which is the most recent is not this engine's to decide.
            if let (Some(kept), Some(next)) = (kept, group.last()) && self.at_once(self.obj(kept).stamp, self.obj(*next).stamp) {
                self.defer_refusal(crate::Unsupported("Roles with timestamps given at once: which is the most recent is chosen in APNAP order (613.7m, 303.7a); the engine does not ask for it".into()));
            }
            group
        }).collect();
        let permanents = self.battlefield.iter().copied().map(|id| {
            let o = self.obj(id);
            let seen = self.seen(id, None);
            if seen.supertypes.contains(&Supertype::Legendary) {
                names.entry((self.controller(id), o.chars.name.clone())).or_default().push(id);
            }
            let toughness = seen.pt.map(|(_, t)| t);
            let detach = o.attached_to.is_some_and(|to| !self.can_attach(id, to));
            let illegal_aura = self.aura(id)
                && !self.attached_to(id).is_some_and(|t| self.can_enchant(id, self.controller(id), t));
            let zero_toughness = seen.is(CardType::Creature) && toughness.is_some_and(|t| t <= 0);
            let zero_loyalty = seen.is(CardType::Planeswalker) && o.counters.get("loyalty").copied().unwrap_or(0) == 0;
            let lethal = seen.is(CardType::Creature)
                && (toughness.is_some_and(|t| o.damage >= t) && o.damage > 0 || o.deathtouched);
            let death = if illegal_aura || zero_toughness || zero_loyalty || superseded.contains(&id) { Some(false) }
                else if lethal && self.destruction_allowed(id) { Some(true) }
                else { None };
            (id, seen, detach, death)
        }).collect();
        let mut legends: Vec<_> = names.into_iter().filter(|(_, group)| group.len() > 1)
            .map(|((player, _), group)| (player, group)).collect();
        legends.sort_by_key(|(p, _)| (*p + self.players.len() - self.active) % self.players.len());
        let losses: Vec<_> = self.players.iter().enumerate().filter(|(player, p)| !p.lost && (p.life <= 0 || p.counters.get("poison").copied().unwrap_or(0) >= 10 || p.drew_from_empty || self.lethal_commander_damage(*player)))
            .map(|(p, _)| p).collect();
        let sagas = self.battlefield.iter().copied().filter(|o| self.saga_finished(*o)).collect();
        let mut commander_returns: Vec<_> = if self.commanders.is_empty() { Vec::new() } else { self.players.iter().flat_map(|p| p.graveyard.iter()).chain(self.exile.iter())
            .copied().filter(|o| self.is_commander(*o) && self.obj(*o).arrived.is_some_and(|at| at >= self.last_sba)
                && !self.players[self.obj(*o).owner].lost && !losses.contains(&self.obj(*o).owner))
            .map(|o| (self.obj(o).owner, o)).collect() };
        commander_returns.sort_by_key(|(owner, _)| (*owner + self.players.len() - self.active) % self.players.len());
        let cancellation = self.battlefield.iter().filter_map(|id| {
            let counters = &self.obj(*id).counters;
            let n = counters.get("+1/+1").copied().unwrap_or(0).min(counters.get("-1/-1").copied().unwrap_or(0));
            (n > 0).then_some((*id, n))
        }).collect();
        SbaBatch { tokens, attachments: self.attachment_snapshot(), permanents, losses, legends, removed: Vec::new(), sagas, departures: None,
            commander_returns, to_command: Vec::new(), checked_through: self.log.end(), cancellation, engines: self.starting_engines() }
    }

    /// Advance this retained rules procedure. Child returns can interrupt the next check,
    /// but can never cause the just-committed snapshot to be performed twice.
    pub(crate) fn advance_state_based(&mut self, mut state: StateBased) -> bool {
        let parent = self.procedures.len();
        loop {
            let batch = state.batch.get_or_insert_with(|| self.sba_batch());
            let apnap = |player: PlayerId| (player + self.players.len() - self.active) % self.players.len();
            if let Some((player, object)) = batch.commander_returns.first()
                && batch.legends.first().is_none_or(|(legend_owner, _)| apnap(*player) <= apnap(*legend_owner))
            {
                self.decision = Some(Decision::Choose { player: *player, choice: Choice::CommandZone { object: *object } });
                self.procedures.push(crate::procedure::Procedure::StateBased(state));
                return false;
            }
            if let Some((player, _)) = batch.legends.first() {
                let player = *player;
                let among = batch.legends.iter().filter(|(p, _)| *p == player).flat_map(|(_, g)| g.iter().copied()).collect();
                self.decision = Some(Decision::Choose { player, choice: Choice::Objects { verb: Verb::Choose, to: None, among, count: 1 } });
                self.procedures.push(crate::procedure::Procedure::StateBased(state));
                return false;
            }
            if batch.departures.is_none() {
                let mut actions: Vec<_> = batch.permanents.iter().filter_map(|(id, _, _, death)| {
                    if batch.losses.contains(&self.obj(*id).owner) || batch.losses.contains(&self.controller(*id)) { return None; }
                    let death = if batch.removed.contains(id) { Some(false) } else { *death };
                    match death {
                        Some(destroy) => Some((*id, if destroy { crate::departure::Kind::Destroy { regenerable: true } } else { crate::departure::Kind::RuleMove(Zone::Graveyard) })),
                        // a Saga that has read its final chapter, which nothing else moves
                        None if batch.sagas.contains(id) => Some((*id, crate::departure::Kind::Sacrifice(self.controller(*id)))),
                        None => None,
                    }
                }).collect();
                actions.extend(batch.to_command.iter().map(|o| (*o, crate::departure::Kind::RuleMove(Zone::Command))));
                batch.departures = Some(crate::departure::Batch::new(self, actions));
            }
            if let Some(decision) = batch.departures.as_mut().unwrap().prepare() {
                self.decision = Some(decision);
                self.procedures.push(crate::procedure::Procedure::StateBased(state));
                return false;
            }
            let batch = state.batch.take().expect("current state-based snapshot");
            let any = self.commit_sba_batch(batch);
            state.changed |= any;
            state.choices.clear();
            self.check_triggers();
            if self.procedures.len() > parent {
                self.procedures.insert(parent, crate::procedure::Procedure::StateBased(state));
                return false;
            }
            if !any { return state.changed; }
        }
    }

    fn commit_sba_batch(&mut self, mut batch: SbaBatch) -> bool {
        let from = self.log.end();
        let mut any = !batch.to_command.is_empty();
        self.last_sba = batch.checked_through;
        self.monarch_leaving(&batch.losses);
        let ended = !batch.losses.is_empty();
        for player in &batch.losses {
            self.players[*player].lost = true;
            self.event_log_mut().push(Event::Lost { player: *player });
            any = true;
        }
        // A loser leaves a game that goes on (800.4a); a loss that ends the game ends it with
        // everything where it is (104.2a, 104.3a), as in every two-player game.
        if !self.finished() {
            for player in batch.losses { self.leave_game(player); }
        }
        // At the end of the game, all face-down permanents and spells are revealed (708.9).
        if ended && self.finished() { self.reveal_face_down_of(None); }
        for player in &mut self.players { player.drew_from_empty = false; }
        let before = crate::layers::Working::before(batch.permanents.iter().map(|(id, last, _, _)| (*id, last.clone())).collect())
            .freeze_attachments(batch.attachments);
        let departures = batch.departures.take().expect("prepared state-based departures");
        let moved: Vec<_> = batch.permanents.iter().filter(|(id, _, _, death)| death.is_some() || batch.removed.contains(id) || batch.sagas.contains(id)).map(|(id, _, _, _)| *id).collect();
        departures.commit(self);
        // Remove only from survivors: 704.8 preserves the counters of every dying object.
        for (id, n) in batch.cancellation {
            if self.obj(id).zone != Zone::Battlefield { continue; }
            for kind in ["+1/+1", "-1/-1"] {
                debug_assert!(self.counters(id).get(kind).copied().unwrap_or(0) >= n);
                self.remove_counters(id, kind, n);
            }
            any = true;
        }
        for (id, _, detach, _) in batch.permanents {
            if moved.contains(&id) {
                any = true;
            }
            if detach && self.obj(id).zone == Zone::Battlefield {
                self.obj_mut(id).attached_to = None;
                any = true;
            }
        }
        for token in batch.tokens {
            let (zone, owner) = (self.obj(token).zone, self.obj(token).owner);
            if zone == Zone::Gone { continue; }
            let last = self.seen(token, None);
            if let Some(list) = self.zone_list(zone, owner) { list.retain(|o| *o != token); }
            let object = self.obj_mut(token);
            object.last = Some(last);
            object.zone = Zone::Gone;
            object.left = Some(zone);
            any = true;
        }
        let engines: Vec<_> = batch.engines.into_iter().filter(|p| !self.players[*p].lost).collect();
        if !engines.is_empty() { self.start_engines(&engines); any = true; }
        for id in self.battlefield.clone() { self.obj_mut(id).deathtouched = false; }
        self.finish_simultaneous(from, before);
        self.update_combat();
        any
    }

    /// "A player can concede the game at any time. A player who concedes leaves the game
    /// immediately. That player loses the game." (104.3a). The loss is the one a state-based
    /// action commits ([`Self::commit_sba_batch`]): a loser leaves a game that goes on
    /// (800.4a), and a game that ends ends with everything where it is (104.2a), its
    /// face-down permanents and spells revealed (708.9). In a game that goes on, a concession
    /// by the player who must answer the pending decision is refused: what becomes of that
    /// decision is not supported.
    pub(crate) fn concede(&mut self, player: PlayerId) -> crate::play::ActionResult<()> {
        if self.finished() { return Err("the game is over".into()); }
        if self.players.get(player).is_none_or(|p| p.lost) {
            return Err("that player is not in the game (800.4a)".into());
        }
        let ends = self.players.iter().enumerate().filter(|(p, state)| *p != player && !state.lost).count() < 2;
        let actor = self.decision_actor();
        if !ends && (player == actor || player == self.decision_maker(actor)) {
            return Err(crate::Unsupported("a concession while that player's decision is pending is not supported in a game that goes on".into()).into());
        }
        // the mulligans keep each player's place (103.5); a player leaving them is not supported
        if !ends && !self.begun() {
            return Err(crate::Unsupported("a concession before the first turn is not supported in a game that goes on".into()).into());
        }
        self.event_log_mut().push(Event::Conceded { player });
        self.monarch_leaving(&[player]);
        self.players[player].lost = true;
        self.event_log_mut().push(Event::Lost { player });
        if !self.finished() { self.leave_game(player); }
        if self.finished() { self.reveal_face_down_of(None); }
        Ok(())
    }

    /// A player leaves the game (800.4a): every object they own leaves the game, abilities
    /// they controlled on the stack among them, and what they still control, owned by
    /// another, is exiled. An object that has left the game is in no zone.
    fn leave_game(&mut self, player: PlayerId) {
        // The face-down permanents and spells, and the foretold cards, they own are revealed
        // (708.9, 702.143f).
        self.reveal_face_down_of(Some(player));
        let owned: Vec<ObjId> = self.objects.iter().filter(|o| o.owner == player && o.zone != Zone::Gone).map(|o| o.id).collect();
        let before = self.departure_snapshot(&owned);
        for (id, last) in before.objects {
            let zone = self.obj(id).zone;
            self.obj_mut(id).last = Some(last);
            if let Some(list) = self.zone_list(zone, player) {
                list.retain(|x| *x != id);
            }
            self.obj_mut(id).zone = Zone::Gone;
            self.obj_mut(id).left = Some(zone);
            self.combat.remove(id);
        }
        // 800.4a: effects giving the departing player control end before objects
        // they still control are exiled. Earlier control effects become visible again.
        let previous = self.control_snapshot();
        self.continuous.retain(|c| !matches!(c.change, crate::layers::Change::Control(p) if p == player));
        self.prune_control_durations();
        self.control_transitions(previous);
        let controlled: Vec<ObjId> =
            self.battlefield.iter().chain(&self.stack).copied().filter(|o| self.controller(*o) == player).collect();
        for id in controlled {
            self.exile(id);
        }
        if self.priority == player {
            self.priority_from(player);
        }
    }

    /// A win or simultaneous-loss draw ends the game (104.1, 104.4a). This is distinct
    /// from `winner`: a drawn game has no winner, just as an unfinished game has none.
    pub fn finished(&self) -> bool {
        self.players.iter().filter(|p| !p.lost).take(2).count() < 2
    }

    /// The player who has not lost, when only one is left.
    pub fn winner(&self) -> Option<PlayerId> {
        let mut alive = (0..self.players.len()).filter(|p| !self.players[*p].lost);
        match (alive.next(), alive.next()) {
            (Some(p), None) => Some(p),
            _ => None,
        }
    }
}

impl Game {
    /// Names in common (201.2a, 709.4b), including either name of an uncast split card.
    /// Name-changing/copy effects must feed this query when their mechanics are added.
    /// `first`: the name the first object goes by, a spell being cast that of its half or
    /// face (709.3a, [`crate::select::Ctx::name_of`]).
    pub(crate) fn shares_name(&self, first: &str, second: ObjId) -> bool {
        first.split(" // ").filter(|name| !name.is_empty())
            .any(|name| self.obj(second).chars.name.split(" // ").any(|other| name == other))
    }
}
