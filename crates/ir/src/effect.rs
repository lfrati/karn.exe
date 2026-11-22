//! Effects: what a spell or ability does when it resolves (CR 608), as sentences of clauses
//! (corpus measurements: corpus-structure.md §1).
//!
//! The shape follows the text closely enough to print it back (sentence breaks, "then", who the
//! subject is), and no closer: the engine reads each clause as steps in order, each an action
//! by an actor.

use crate::action::Action;
use crate::amount::Amount;
use crate::condition::{Condition, Restriction};
use crate::cost::Cost;
use crate::event::{EventPattern, Step, StepOwner};
use crate::selector::{ObjectSet, Selector};
use serde::Serialize;

/// Sentences in order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Effect(pub Vec<Sentence>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Sentence {
    /// "Draw a card.", "Target player draws two cards, then discards a card."
    Do(Body),
    /// "[lead,] [sentence]": what stands in front of a sentence governs all of it ("If you
    /// control a Swamp, draw a card.", "Until end of turn, target creature gains haste."). The
    /// rest is any sentence, so leads stack in any order the text uses ("Then if …, …", "When you
    /// do, for each …, …", "Until your next turn, whenever …, …", "If …, choose one —").
    Led { lead: Lead, rest: Box<Sentence> },
    /// "X is the number of creatures you control.": what X is in the sentences before, said in
    /// a sentence of its own (107.3).
    XIs(Amount),
    /// A trailing instruction rounds each fractional quantity in the preceding instructions (107.1a).
    RoundEach { up: bool },
    /// "Activate only as a sorcery." (602.5)
    Restrictions(Vec<Restriction>),
    /// A sentence about the ability rather than an instruction; ability composition moves it to
    /// the ability.
    Rider(Rider),
    /// "Cast ~ only during combat." (a spell's own timing restriction, 601.3)
    CastRestrictions(Vec<Restriction>),
    /// "The same is true for first strike, trample, and vigilance.", "The same is true for
    /// creature spells you control and creature cards you own that aren't on the battlefield.":
    /// the sentence before, again for each of these.
    SameIsTrueFor(SameFor),
    /// "Choose one —", "Choose one or both —": the modes follow as bulleted rows (700.2).
    /// `unchosen`: "choose one that hasn't been chosen [this turn] —" (700.2).
    /// `random`: "choose one at random —", the mode is picked by chance, not by a player.
    /// `chooser`: "An opponent chooses one —", a player other than the controller chooses
    /// (700.2e); None: the controller does.
    ChooseModes { modes: Modes, unchosen: Option<Unchosen>, random: bool, chooser: Option<Selector> },
    /// "As an additional cost to cast this spell, sacrifice a creature." (601.2f; a static of
    /// the spell, read here with the spell's text).
    /// `optional`: "you may" pay it (an optional additional cost, 601.2b). Alternatives
    /// ("sacrifice a creature or pay {3}") are the cost's own.
    AdditionalCost { optional: bool, cost: Cost },
    /// A static ability, replacement or prevention effect, or rule change
    /// ([`crate::statics::Static`]).
    Static(Box<crate::statics::Static>),
}

/// What stands in front of a sentence and governs the whole of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Lead {
    /// A modifier in front, the same one an act takes after it: "If you control a Swamp,"
    /// (608.2c), "Until end of turn," (611.2a), "For each opponent," (the sentence once per
    /// counted object or player, which "that player" and the like refer to), "At the beginning
    /// of your next end step," and "At end of combat," (delayed triggers, 603.7, 511.2), "As
    /// long as you control a Mountain,", "During your turn,", "Unless you pay {R},", and
    /// "instead" after another lead ("If ~ was kicked, instead search …": the sentence replaces
    /// what the sentences before said to do).
    Modifier(Modifier),
    /// "Once each turn,", "Once during each of your turns," (`yours`): the permission the
    /// sentence gives can be used once per turn.
    Once { yours: bool },
    /// "As ~ enters,": choices and payments made as the permanent enters, before it is on the
    /// battlefield (614.1c, 614.12a).
    AsEnters(Selector),
    /// "As ~ is turned face up,": applied while the permanent is being turned face up, not
    /// afterward (708.11).
    AsTurnedFaceUp(Selector),
    /// "As ~ enters or is turned face up,": at either of the two (614.1c, 708.11).
    AsEntersOrTurnedFaceUp(Selector),
    /// "Then": after the previous sentence is done (608.2c).
    Then,
    /// "Otherwise,": the else branch of the previous "If".
    Otherwise,
    /// "When you do,": a reflexive triggered ability on the previous step (603.12).
    WhenYouDo,
    /// "As ~ transforms into ~,": applied as this permanent transforms to this face (701.27).
    AsTransformsIntoThis,
    /// "When it regenerates this way,": a delayed trigger on the use of the regeneration shield
    /// this effect made (701.19, 603.7).
    WhenRegeneratesThisWay,
    /// "When you pay this cost one or more times,": the reflexive trigger after a payment made
    /// any number of times; "that many" is how often it was paid.
    WhenPaidTimes,
    /// A delayed trigger bound to each mana this text produces (106.6a).
    WhenManaSpent { on: crate::mana::SpendOn },
    /// "When that creature dies this turn,", "Whenever a creature becomes tapped,": a delayed
    /// triggered ability the effect creates (603.7); `limit` bounds it; `whenever`: it triggers
    /// each time (603.7c), not once.
    /// `next`: "When you next cast a spell this turn,", only the first time within the duration
    /// (603.7b).
    When { pattern: EventPattern, limit: Option<DelayLimit>, whenever: bool, next: bool },
    /// "Starting with you,": players act in turn order from the one named, instead of APNAP
    /// order (101.4).
    StartingWith(Selector),
    /// "Starting with the next opponent in turn order,": the first opponent after the
    /// instructed player (101.4).
    StartingWithNextOpponent,
}

/// How many modes are chosen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Modes {
    Exactly(u32),
    /// "up to one", "up to two": none, or as many as that.
    UpTo(u32),
    /// "one or both".
    OneOrBoth,
    /// "one or more".
    OneOrMore,
    /// "both".
    Both,
    /// "any number".
    AnyNumber,
    /// "X": a number chosen as the spell is cast.
    X,
    /// "up to five {P} worth of modes": any modes whose pawprints together are at most that
    /// many (700.2i).
    Pawprints(u32),
}

/// Clauses of one sentence: "You draw a card and you lose 1 life" (each later clause has its own
/// subject).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Body {
    pub first: Clause,
    pub rest: Vec<(Join, Clause)>,
}

/// One subject and what it does: "Target player draws two cards, then discards a card".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Clause {
    /// None: an imperative, done by the ability's controller.
    pub subject: Option<Subject>,
    /// "you may …", "that player may …": the actor chooses whether to do it (a yes/no decision).
    pub may: bool,
    /// "that creature also gains trample": in addition to what an earlier sentence did.
    pub also: bool,
    pub first: Act,
    /// Further actions of the same subject, with how each is joined.
    pub rest: Vec<(Join, Act)>,
}

/// Who or what a clause is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Subject {
    /// "target player", "~", "each opponent", "you".
    Selector(Selector),
    /// A bare plural: "Creatures you control get +1/+1 until end of turn" (every object in the
    /// set, fixed when the effect starts, 611.2c).
    Set(ObjectSet),
    /// "You and permanents you control gain hexproof", "You and that player each draw a card"
    /// (`each`: each one does it separately).
    And { items: Vec<Subject>, each: bool },
    /// "Up to two target creatures each get +1/+0": the act applies to each one.
    Each(Selector),
    /// "Your life total becomes 10", "each player's life total becomes …": a player's life
    /// total, the subject of [`crate::action::Action::LifeTotalBecomes`].
    LifeTotal(Selector),
}

/// How long a continuous effect from a one-shot lasts (CR 611.2; corpus-structure.md §6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Duration {
    /// "until end of turn" (514.2), "until end of combat", "until your next turn", "until the
    /// end of your next turn", "until your next end step", "until that player's next turn":
    /// until a point of the turn structure.
    Until(crate::event::Time),
    /// "until ~ leaves the battlefield" (610.3: a one-shot with a duration), "until an
    /// opponent becomes the monarch": until an event happens.
    UntilEvent(Box<EventPattern>),
    /// "until you exile another card with ~": while the card is the last one this object exiled.
    UntilExileAnother,
    /// "this turn".
    ThisTurn,
    /// "this combat" (506.1: until the combat phase ends).
    ThisCombat,
    /// "for the rest of the game".
    RestOfGame,
    /// "for as long as it remains exiled", "for as long as you control ~", "for as long as it
    /// has a flood counter on it": while the condition holds (611.2b).
    While(Box<crate::condition::Condition>),
}

/// An action with what modifies it, in printed order: "draw a card for each creature you
/// control", "sacrifice it at the beginning of the next end step".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Act {
    pub action: Action,
    pub mods: Vec<Modifier>,
    /// "… and may choose new targets for the copy": a later act of a chain that is optional on
    /// its own (the clause's "may" covers the whole chain).
    pub may: bool,
}

/// What modifies an act when it follows it, or a whole sentence when it leads it
/// ([`Lead::Modifier`]): one set for both places.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Modifier {
    /// An extra turn inserted after this turn or a previously named future turn (500.7).
    AfterTurn(TurnRef),
    /// "for each creature you control": the action's amount is multiplied (or the action
    /// repeated) by the count.
    ForEach(Amount),
    /// Iterate the values of a class, binding "that type/color" for each (608.2c, 205.2a).
    ForEachValue(crate::action::ValueKind),
    /// ", where X is …": X for this sentence (107.3).
    WhereX(Amount),
    /// "unless its controller pays {2}", "unless you sacrifice a creature": the action happens
    /// unless that player performs this one (608.2d), as if they may do it and the action
    /// happens if they don't (118.12a). "unless they discard a card or sacrifice a creature",
    /// "unless they pay {B} or {3}": alternatives joined by "or", the player picks one.
    Unless { who: Selector, actions: crate::action::Coord<Action> },
    /// "at the beginning of the next end step": a delayed triggered ability does it (603.7).
    At { step: Step, whose: StepOwner },
    /// "at end of combat" (511, a delayed trigger).
    AtEndOfCombat,
    /// "until end of turn", "this turn": how long the act's continuous effect lasts; with
    /// "and" it covers the earlier acts of the clause too ("gets +2/+2 and gains trample until
    /// end of turn").
    Duration(Duration),
    /// "… instead": replaces what an earlier sentence would do (608.2c).
    Instead,
    /// "face down" ("exile the top card of your library face down", 406.3).
    FaceDown,
    /// ", divided as you choose": the amount is split among the targets (601.2d).
    DividedAsYouChoose,
    /// "without paying their mana costs", after a permission to cast a kind of spell ("you may
    /// cast spells from your hand without paying their mana costs", 118.9).
    WithoutPayingManaCosts,
    /// "using its blitz ability": the keyword ability the cast is made with.
    Using(crate::vocab::Keyword),
    /// "in a face-down pile" ("exile them in a face-down pile", 700.3): the cards exiled are
    /// grouped into a pile.
    InPile(crate::procedure::PileKind),
    /// "with three time counters on it" (exiling a card with counters, 702.62a).
    WithCounters(crate::action::CountersOn),
    /// "exile it instead of putting it into its owner's graveyard": the act replaces that zone
    /// change (614.1a).
    InsteadOf(crate::statics::Elsewhere),
    /// "for that player to ignore this effect until end of turn": what the payment buys.
    ToIgnoreThisEffect,
    /// "copy the next instant or sorcery spell you cast this turn when you cast it" (707.10).
    WhenYouCastIt,
    /// "exile those tokens when ~ leaves the battlefield": a delayed trigger said after the
    /// instruction (603.7).
    WhenThisLeaves,
    /// "except for Giants, Wizards, and lands", after where the things go ("return all
    /// permanents to their owners' hands except for …"): these are left out.
    ExceptFor(crate::selector::ObjectSet),
    /// "instead of one": the count this instruction names replaces the one an earlier
    /// instruction named ("put up to two creature cards … into your hand instead of one").
    InsteadOfOne,
    /// "twice", "three times", "X times", "that many times": the action repeated.
    Times(Amount),
    /// "it explores, then it explores again": the act restates one done before in the
    /// sentence, and is done once more as it is said.
    Again,
    /// ", rounded up", ", rounded down" said at the end of the act, apart from the half it
    /// rounds ("create half X Food tokens, rounded up", 107.1a).
    Rounded { up: bool },
    /// ", except it's an artifact in addition to its other types and it has haste": changes to
    /// the copy a copy action makes (707.9a, 707.9b).
    Except(Vec<Exception>),
    /// "by discarding a card in addition to paying its other costs", "by paying {1} rather than
    /// paying its mana cost": what a permission to cast says the cast is paid with (118.8, 118.9).
    By(crate::statics::CastCost),
    /// "if it attacked this turn": a postposed condition.
    If(Condition),
    /// "as long as you control a Mountain": it applies only while the condition holds (611.3b);
    /// with "and" it covers the earlier acts of the clause too, as a duration does.
    AsLongAs(Condition),
    /// "during your turn", "during combat", "during turns other than yours": it applies only
    /// then (611.3b). The same description of when as restrictions ("activate only during your
    /// turn") and event patterns ("whenever you cast a spell during your turn") use.
    During(crate::event::During),
    /// "it becomes day as ~ enters": the act is done as the permanent enters, as part of the
    /// replacement effect of its entering (614.1c, 614.12a). It governs this act only; in front
    /// of a sentence the same words govern all of it ([`Lead::AsEnters`]).
    AsEnters(Selector),
}

/// How two actions of one clause are joined; "then" orders them (608.2c), "and" and the comma
/// are read in order as well; "or" offers the actor one of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Join {
    /// " or ": "you may sacrifice an artifact or discard a card".
    Or,
    /// " and "
    And,
    /// ", "
    Comma,
    /// ", then "
    CommaThen,
    /// ", and "
    CommaAnd,
}

/// Sentences that qualify their ability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Rider {
    /// "This ability triggers only once each turn."
    TriggersOnlyOnceEachTurn,
    /// "This ability triggers only once." (603.2)
    TriggersOnlyOnce,
    /// "Repeat this process until no one puts a card onto the battlefield."
    RepeatUntilNoOnePuts,
    /// "Do this only once each turn." (603.2h)
    DoThisOnlyOnceEachTurn,
    /// "Any player may activate this ability." (602.2)
    AnyPlayerMayActivate,
    /// "Only your opponents may activate this ability."
    OnlyOpponentsMayActivate,
    /// "Any player may activate this ability but only as a sorcery.", "Only your opponents may
    /// activate this ability and only as a sorcery." (`opponents`) (602.5).
    MayActivateOnly { opponents: bool, restrictions: Vec<crate::condition::Restriction> },
    /// "You may choose the same mode more than once." (700.2d)
    SameModeMoreThanOnce,
    /// "The new target must be a creature.": what a changed target may be (115.7).
    NewTargetMustBe(crate::selector::Selector),
    /// "This ability can't cause the total number of +1/+0 counters on ~ to be greater than
    /// four.": a cap on what the ability puts there.
    CounterCap { kind: crate::selector::CounterKind, max: crate::amount::Amount },
    /// "The escape cost is equal to the card's mana cost plus exile three other cards from your
    /// graveyard." (702.138; `its`: "its mana cost")
    EscapeCostManaPlusExile { cards: crate::amount::Amount, its: bool },
    /// "Mana of any type can be spent to cast a spell this way." (`plural`: "to cast spells
    /// this way") (609.4b)
    AnyManaThisWay { plural: bool },
    /// "Creatures it was blocking that had become blocked by only ~ this combat become
    /// unblocked." (`that_creature`: "by only that creature") (506.4)
    BlockedOnlyByBecomeUnblocked { that_creature: bool },
    /// "The copy targets that token.", "The copy targets the chosen creature.": what the copy
    /// made before is aimed at (707.10c).
    CopyTargets(crate::selector::Selector),
    /// "If two or more creatures are tied for least power, you choose one of them.": who breaks
    /// the tie of the superlative the sentence before used (`cards`: said of cards).
    TiedYouChoose { cards: bool, greatest: bool, stat: crate::selector::Stat },
    /// "Each mode must target a different player."
    ModesTargetDifferentPlayers,
    /// "Each copy targets a different one of those creatures." (707.10)
    CopiesTargetDifferent(Selector),
    /// "This effect reduces only the amount of colored mana you pay.", "… of red mana you pay."
    /// (None: colored mana of any color.)
    ReducesOnly(Option<crate::vocab::Color>),
    /// "Spend this mana only on costs that contain {X}." (106.6, 107.3)
    SpendOnlyOnXCosts,
    /// "Excess damage is dealt to that creature's controller instead." (120.4a)
    ExcessToController,
    /// "Spend this mana only to pay cumulative upkeep costs." (106.6, 702.24)
    SpendOnlyOnCumulativeUpkeep,
    /// "This cost is reduced by {2} for each basic land type among lands you control." (118.7)
    CostReducedForEach { by: crate::mana::ManaCost, each: crate::amount::Amount },
    /// "Its foretell cost is equal to its mana cost reduced by {2}." (702.143)
    ForetellCostReduced(crate::mana::ManaCost),
    /// "Spend this mana only to cast a creature spell.", "… to cast artifact spells or activate
    /// abilities of artifacts.", "… to cast a Dragon spell or to activate an ability of a Dragon"
    /// (106.6). `uncounterable`: "…, and that spell
    /// can't be countered": an additional effect on the spell the mana is spent on (106.6),
    /// a "can't" that takes precedence over an instruction to counter it (101.2).
    SpendOnly { on: crate::action::Coord<crate::mana::SpendOn>, uncounterable: bool },
    /// "Spend only black mana on X.", "Spend only mana produced by basic lands to cast ~.": the
    /// same restriction (106.6) said of the payment, naming the mana by kind or source.
    SpendOnlyMana { mana: crate::mana::SpendableMana, on: crate::action::Coord<crate::mana::SpendOn> },
}

/// What "the same is true for" repeats the sentence before for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SameFor {
    Keywords(Vec<crate::vocab::Keyword>),
    Sets(Vec<crate::selector::ObjectSet>),
}

/// One exception to a copy (707.9): what the copy is or has instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Exception {
    pub subject: CopyRef,
    pub change: CopyChange,
}

/// How an exception names the copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CopyRef {
    /// "it".
    It,
    /// "the copy".
    TheCopy,
    /// "the token".
    TheToken,
    /// "they".
    They,
    /// "the tokens".
    TheTokens,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CopyChange {
    /// "it's an artifact in addition to its other types", "it's a 1/1 black Zombie", "it's 4/3".
    Is(crate::action::Becoming),
    /// "it isn't legendary" (707.9b).
    NotLegendary,
    /// "it has haste", "it has \"…\"" (707.9a).
    Has(Vec<crate::action::Granted>),
    /// "it has this ability": the ability doing the copying (707.9a).
    HasThisAbility,
    /// "it loses all other card types", "it loses all abilities": what the copy does not keep
    /// (707.9b, 613.1d).
    Loses(crate::statics::Lost),
    /// "its name is ~".
    NamedThis,
    /// "it enters with a shield counter on it if you control that creature", "it enters with X
    /// additional +1/+1 counters on it": counters the copy enters with (614.1c).
    EntersWith { counters: crate::action::Counters, if_: Option<Box<crate::condition::Condition>> },
}


/// How far a delayed trigger made by an effect reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DelayLimit {
    /// "this turn".
    ThisTurn,
    /// "this way": only for what the ability itself just did ("When you discard one or more cards
    /// this way, …", 608.2c).
    ThisWay,
}

/// Which modes a "choose one that hasn't been chosen" may pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Unchosen {
    /// "that hasn't been chosen": ever, for this object.
    Ever,
    /// "that hasn't been chosen this turn".
    ThisTurn,
}

/// Temporal reference bound by the instruction that names a future turn (500.7, 723.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TurnRef { This, That }
