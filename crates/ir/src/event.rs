//! Event patterns: what triggers listen for (CR 603), what history conditions ask about ("if a
//! creature died this turn"), and later what replacements and "until" durations match.

use crate::amount::Amount;
use crate::condition::Condition;
use crate::lexicon::{Role, Verb};
use crate::selector::{CounterKind, Selector, Status as ObjectStatus, ZoneSpec};
use crate::vocab::Zone;
use serde::Serialize;

/// The trigger condition of a triggered ability (CR 603.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Trigger {
    /// "When ~ enters", "Whenever a creature you control dies". `word` is the printed
    /// conjunction; Oracle uses "When" for events that happen once to an object, and the engine
    /// treats both the same.
    Event { word: TriggerWord, pattern: EventPattern },
    /// "At the beginning of your upkeep", "At the beginning of combat on your turn" (CR 603.1,
    /// 503.1a …).
    Step { step: Step, whose: StepOwner },
    /// "When ~ enters and whenever you cast your first spell each turn": one ability with
    /// several trigger conditions.
    Any(Vec<Trigger>),
    /// "When you control no Islands": a state trigger, when the condition becomes true (CR
    /// 603.8).
    State(Condition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TriggerWord {
    When,
    Whenever,
}

/// What produces an event, separate from the acting player and affected recipient.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum EventCause {
    /// A specified spell or ability, e.g. one an opponent controls.
    Source(Box<Selector>),
    /// An effect, including replacement/prevention-produced events (614.16).
    Effect,
}

/// Any of `subjects` doing any of `actions`: "~ or another Ally you control enters", "~ enters
/// or attacks".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct EventPattern {
    /// Who or what does it. Empty for the passive, where anyone may do it: "a creature card is
    /// exiled this way", "no spells were cast last turn", "you've been attacked this step" (the
    /// object or player acted on is the action's argument).
    pub subjects: Vec<Selector>,
    pub actions: Vec<EventAction>,
    /// Only the n-th such event this turn: "your second spell each turn", "attacks for the first
    /// time each turn".
    pub nth_each_turn: Option<u32>,
    /// "during your turn", "during your main phase", "during combat".
    pub during: Option<During>,
    /// "while saddled", "while you control a creature with power 4 or greater": what must be
    /// true as the event happens.
    pub condition: Option<Box<Condition>>,
    /// "a spell or ability an opponent controls causes you to discard ~": what made it happen.
    pub cause: Option<EventCause>,
    /// "except the first one they draw in each of their draw steps": all such events but the
    /// n-th its subject does in each step of this kind of their own turns.
    pub except_nth_in_step: Option<(u32, Step)>,
}

impl EventPattern {
    pub fn new(subjects: Vec<Selector>, actions: Vec<EventAction>) -> EventPattern {
        EventPattern { subjects, actions, nth_each_turn: None, during: None, condition: None, cause: None, except_nth_in_step: None }
    }

    /// Does it say which of such events, by number: an ordinal or an exception of one?
    pub fn counted(&self) -> bool { self.nth_each_turn.is_some() || self.except_nth_in_step.is_some() }
}

/// An action of the lexicon as a verb phrase: the verb, whether its subject does it or has it
/// done to it, and what fills its other roles. One value whichever construction says it:
/// "sacrifices a creature" (it happens), "sacrificed a creature" (it happened), "would
/// sacrifice a creature", "can't be sacrificed", "the sacrificed creature".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Deed {
    pub verb: Verb,
    /// The subject is what it is done to ([`Verb::passive`]), whoever does it: "a card is
    /// exiled", "~ becomes tapped", "you've been attacked".
    pub passive: bool,
    /// What fills its roles besides the subject, in the order they are said.
    pub args: Vec<(Role, Arg)>,
}

/// What fills a role of a [`Deed`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Arg {
    /// An object or a player: "a creature", "you".
    Noun(Selector),
    /// A zone a zone change comes from ([`Role::From`]: "from your hand", "from anywhere other
    /// than your hand") or goes to ([`Role::To`]: "into exile", "into a library"), CR 400.1.
    Zone(ZoneArg),
    /// The status an object has as it arrives ([`Role::As`]): "enters tapped", "enters
    /// attacking" (CR 110.5, 614.12).
    Status(ObjectStatus),
    /// What a die roll shows ([`Role::Result`]): "rolls a natural 20", "rolls a 4 or higher",
    /// "rolls a die's highest natural result" (CR 706.2).
    Roll(RollResult),
    /// How much mana tapping a permanent for mana produces ([`Role::Result`]): "is tapped for
    /// two or more mana" (CR 106.12).
    Mana(Quantity),
}

/// A result of a die roll (706.2): the natural result is the number on the die's top face,
/// before any modifier; the result is the number after them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RollResult {
    pub natural: bool,
    pub value: RollValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum RollValue {
    /// "a natural 20", "a 4 or higher".
    Number(Quantity),
    /// "a die's highest natural result": the number of the die's sides (706.1a).
    Highest,
}

impl Deed {
    /// The noun phrase in a role.
    pub fn noun(&self, role: Role) -> Option<&Selector> {
        self.args.iter().find_map(|(r, a)| match a {
            Arg::Noun(s) if *r == role => Some(s),
            _ => None,
        })
    }

    /// The zone in a role.
    pub fn zone(&self, role: Role) -> Option<&ZoneArg> {
        self.args.iter().find_map(|(r, a)| match a {
            Arg::Zone(z) if *r == role => Some(z),
            _ => None,
        })
    }

    /// What it is done to, when the phrase says: "sacrifices *a creature*".
    pub fn what(&self) -> Option<&Selector> {
        self.noun(Role::What)
    }
}

impl EventAction {
    /// "sacrifices a creature": the subject does it to `what`.
    pub fn does(verb: Verb, what: Selector) -> EventAction {
        EventAction::Deed(Deed { verb, passive: false, args: vec![(Role::What, Arg::Noun(what))] })
    }

    /// "is exiled", "becomes tapped": it is done to the subject.
    pub fn undergoes(verb: Verb) -> EventAction {
        EventAction::Deed(Deed { verb, passive: true, args: Vec::new() })
    }

    /// "scries", "enters", "dies": the verb alone.
    pub fn just(verb: Verb) -> EventAction {
        EventAction::Deed(Deed { verb, passive: false, args: Vec::new() })
    }

    /// "enters under your control": under whose control it happens.
    pub fn under(verb: Verb, who: Selector) -> EventAction {
        EventAction::Deed(Deed { verb, passive: false, args: vec![(Role::Under, Arg::Noun(who))] })
    }

    /// The deed of this verb, when this is one.
    pub fn deed(&self, verb: Verb) -> Option<&Deed> {
        match self {
            EventAction::Deed(d) if d.verb == verb => Some(d),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum EventAction {
    /// An action said the regular way, from its lexicon entry: "sacrifices a creature", "casts
    /// a spell from their hand", "scries", "enters under your control", "dies"; in the passive
    /// "is exiled", "becomes tapped", "was cast from a graveyard". The variants below are the actions whose phrases still take an
    /// argument of their own shape (a kind of damage, counters, a zone, "under your control").
    Deed(Deed),
    /// "is put into a graveyard from the battlefield" / "from anywhere" / "from your library".
    PutIntoGraveyard {
        /// Whose graveyard: "a graveyard", "your graveyard", "an opponent's graveyard".
        into: Box<Selector>,
        from: Option<Zone>,
    },
    /// "leave your graveyard": moves out of a zone other than the battlefield (CR 400.7).
    LeavesZone {
        zone: ZoneSpec,
    },
    /// "attacks", "attacks alone", "attacks a player", "attack you or a planeswalker you
    /// control"; with a player subject, "you attack" (attacks with creatures).
    Attacks {
        alone: bool,
        defender: Vec<Selector>,
        /// "you attack with three or more creatures": the attackers, after a player subject.
        with: Option<Box<Selector>>,
    },
    /// "attacks and isn't blocked" (509.1h, 603.2).
    AttacksUnblocked,
    /// "blocks", "blocks a creature".
    Blocks {
        blocked: Option<Box<Selector>>,
    },
    /// "becomes blocked", "becomes blocked by a creature" (509.1h); in a rule about what can
    /// happen, "be blocked by creatures with flying" (`by`: any of these), and with `except`
    /// "be blocked except by creatures with flying": by anything else.
    BecomesBlocked {
        by: Vec<Selector>,
        except: bool,
    },
    /// "deals combat damage to a player", "deals damage", "deals noncombat damage to an
    /// opponent", "deals damage to you or a permanent you control" (CR 120.1).
    DealsDamage {
        kind: DamageKind,
        /// "deals 4 or more damage", "deals exactly 1 damage".
        amount: Option<Quantity>,
        to: Option<crate::action::Objects>,
    },
    /// "is dealt damage", "is dealt combat damage"; "is dealt damage by a red instant or sorcery
    /// spell you control or by a red planeswalker you control" (`by`: the source, the doer
    /// named after the passive, any of these).
    IsDealtDamage {
        kind: DamageKind,
        /// Completed recipient-level excess, not another ordinary hit (120.10).
        excess: bool,
        by: Vec<Selector>,
    },
    /// "is turned face up" (708.8).
    TurnedFaceUp,
    /// "leaves the battlefield without dying": any way of leaving other than being put into a
    /// graveyard (700.4).
    LeavesWithoutDying,
    /// "becomes the target of a spell or ability".
    BecomesTarget {
        of: Option<Box<Selector>>,
    },
    /// "gains life"; "gained 3 or more life this turn" (`amount`: in total over a history
    /// condition's window).
    GainsLife {
        amount: Option<Quantity>,
    },
    /// "loses life"; "lost 3 or more life this turn", "lose exactly 1 life".
    LosesLife {
        amount: Option<Quantity>,
    },
    /// "gained or lost life this turn": either of the two.
    GainsOrLosesLife,
    /// "can't be enchanted by other Auras" (303.4).
    EnchantedByOtherAuras,
    /// "you activate an ability of a Sarkhan planeswalker", "you activate a loyalty ability of a
    /// Chandra planeswalker" (`loyalty`) (602.2).
    ActivatesAbilityOf {
        loyalty: bool,
        of: Box<Selector>,
    },
    /// "blocked or was blocked this turn", "blocked or was blocked by a Zombie this turn": the
    /// creature blocked, or was blocked by, such a creature (509).
    BlockedOrWasBlocked {
        by: Option<Box<Selector>>,
    },
    /// "puts one or more +1/+1 counters on a creature you control", "puts a +1/+1 counter on
    /// another creature"; the subject is the player.
    PutsCounters {
        kind: Option<CounterKind>,
        one_or_more: bool,
        on: Box<Selector>,
    },
    /// "one or more +1/+1 counters are put on ~", "a counter is put on ~"; the subject is the
    /// object the counters are put on.
    CountersPut {
        kind: Option<CounterKind>,
        one_or_more: bool,
        /// "the fourth plan counter is put on ~" (CR 122.7).
        nth: Option<CounterNth>,
    },
    /// "searches their library", "you search your library this way" (CR 701.23).
    Searches {
        zone: ZoneSpec,
    },
    /// "an Equipment is put onto the battlefield this way".
    PutsOntoBattlefield {
        what: Box<Selector>,
    },
    /// "damage is prevented this way": damage a prevention effect stopped (CR 615.1).
    DamagePrevented,
    /// "transforms", "transforms into ~" (CR 701.27e).
    Transforms {
        into: Option<Box<Selector>>,
    },
    /// "the Ring tempts you" (CR 701.54d); the subject is the player tempted.
    RingTempts,
    /// "players finish voting" (701.38).
    FinishVoting,
    /// "the final chapter ability of a Saga you control resolves" (714.2).
    FinalChapterResolves,
    /// "you get one or more {E}", "you would get one or more poison counters", "… one or more
    /// counters" (`kind` None): a player gets counters (CR 122.1).
    GetsCounters {
        kind: Option<crate::action::PlayerCounter>,
    },
    /// "you expend 4" (CR 700.14).
    Expends(Amount),
    /// "becomes level 2": a Class's level (CR 716.2a).
    BecomesLevel(u32),
    /// "you fully unlock a Room" (CR 709.5i).
    FullyUnlocks {
        what: Box<Selector>,
    },
    /// "day becomes night" (CR 731.1a).
    BecomesNight,
    /// "night becomes day" (CR 731.1a).
    BecomesDay,
    /// "a time counter is removed from ~"; the subject is the object the counters leave.
    CountersRemoved {
        kind: Option<CounterKind>,
        one_or_more: bool,
        /// "the last time counter is removed from ~": the one that leaves none.
        nth: Option<CounterNth>,
    },
    /// "becomes attached to a creature" (CR 301.5, 303.4).
    BecomesAttached {
        to: Option<Box<Selector>>,
    },
    /// "becomes unattached from a permanent".
    BecomesUnattached {
        from: Option<Box<Selector>>,
    },
    /// "shuffles their library" (CR 701.24).
    Shuffles {
        zone: ZoneSpec,
    },
    /// "loses control of ~".
    LosesControl {
        of: Box<Selector>,
    },
}

/// When something happens or may be done: "during your turn", "during an opponent's turn",
/// "during your main phase", "during the declare attackers step", "during combat" (the combat
/// phase, CR 506.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum During {
    Turn(Box<Selector>),
    /// "turns other than yours": every turn but that player's.
    NotTurn(Box<Selector>),
    Step(Step, StepOwner),
    Combat,
    /// "any time a sorcery couldn't have been cast": outside sorcery timing (CR 307.1).
    NotSorceryTiming,
    /// "any turn you attacked with a Rogue": a turn in which that has happened.
    TurnWhen(Box<EventPattern>),
}

/// How a history condition puts its event: "a creature died this turn" (past), "you've cast a
/// spell this turn" (perfect), "a creature card is revealed this way" (present, for the events of
/// the resolving ability). Oracle uses the past and the perfect for the same question about the
/// event log ("you gained life this turn", "you've gained 3 or more life this turn"); kept to
/// print the condition back, not read by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Tense {
    Present,
    Past,
    Perfect,
}

/// A zone in a zone change (CR 400): where an object comes from, "from your hand", "from a
/// graveyard", "from exile", "from anywhere other than your hand" (`other_than`), or where it
/// goes, "into exile", "into a library".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ZoneArg {
    pub zone: ZoneSpec,
    pub other_than: bool,
}

/// How much of something one event is about: "3 or more" damage or life, "exactly 1" life
/// (an object deals an amount of damage, CR 120.1; a player gains or loses an amount of life,
/// 119.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Quantity {
    AtLeast(Amount),
    Exactly(Amount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DamageKind {
    Any,
    Combat,
    Noncombat,
}

/// A part of the turn structure the text names (CR 500–514): a step, a phase, or the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Step {
    /// The turn itself, in points of time ("your next turn", "end of turn").
    Turn,
    /// "untap step" (502).
    Untap,
    Upkeep,
    Draw,
    /// "first main phase" / "precombat main phase" (505.1).
    FirstMain,
    /// "combat": the beginning of combat step (507).
    Combat,
    /// "the declare attackers step" (508).
    DeclareAttackers,
    /// "the declare blockers step" (509).
    DeclareBlockers,
    SecondMain,
    /// "main phase": either of the two (505.1), as in "during your main phase".
    Main,
    /// "end of combat": the end of combat step ("At end of combat", 511.2).
    EndOfCombat,
    End,
    /// "cleanup step" (514).
    Cleanup,
}

/// Which of the steps of that kind: "your upkeep", "each upkeep", "the next end step".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum StepOwner {
    /// A player's: "your upkeep", "each opponent's upkeep", "that player's upkeep", "the upkeep
    /// of enchanted creature's controller", "combat on your turn" (Oracle's spelling for the
    /// combat of that player's turns); `next`: only the next one ("your next upkeep", "that
    /// player's next turn").
    Player { who: Box<Selector>, next: bool },
    /// "your first upkeep" (`who`), "the first upkeep" (none): the first of the game, said of
    /// what an opening-hand action does (103.6).
    First { who: Option<Box<Selector>> },
    /// "each upkeep", "each combat".
    Each,
    /// "the end step": the next one (for delayed triggers) or each one, by context; `next`:
    /// "the next end step" (delayed triggers, CR 603.7).
    The { next: bool },
    /// "the next turn's upkeep".
    NextTurns,
    /// "that turn's end step", "that turn": of a turn the context names.
    ThatTurns,
    /// The current one: the turn of "until end of turn", the combat of "until end of combat".
    This,
}

/// A point of the turn structure (CR 500): where a step, a phase or a turn begins, or where
/// it ends. "end of turn", "your next turn" (its beginning), "the end of your next turn",
/// "your next end step", "that player's next untap step".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Time {
    /// "the end of": where it ends rather than where it begins.
    pub end: bool,
    pub step: Step,
    pub whose: StepOwner,
}

/// Which counter of a kind an event is about (CR 122.7): "the fourth plan counter is put on ~"
/// is the one that takes the object from three to four; "the last time counter is removed from
/// ~" is the one that leaves none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CounterNth {
    Nth(u32),
    Last,
}
