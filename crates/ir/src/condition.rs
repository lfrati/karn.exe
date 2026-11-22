//! Conditions: "if …", "as long as …", "unless …", intervening ifs (CR 603.4), and the "only …"
//! restrictions on activating and casting (corpus measurements: corpus-structure.md §5).
//!
//! The condition language is small; its length comes from the selectors, amounts and event
//! patterns it embeds, which are the shared sub-languages.

use crate::action::Action;
use crate::amount::{Amount, Characteristic, Cmp};
use crate::event::{During, EventPattern, Quantity, Step, StepOwner, Tense};
use crate::mana::{ManaCost, SpendOn};
use crate::selector::{Filter, ObjectSet, Selector, ZoneSpec};
use crate::vocab::{Color, Keyword};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Condition {
    /// "you control two or more artifacts", "defending player controls an Island", "you
    /// control no creatures", "you don't control a Swamp". The selector's determiner is the
    /// count: "a" is at least one, "two or more" at least two, "no" none. Several selectors are
    /// one coordination: alternatives ("you control a Plains or a Swamp"), all of them ("you
    /// control an artifact and an enchantment") or one or more ("lands and/or Treefolk").
    Controls {
        player: Box<Selector>,
        what: crate::action::Coord<Selector>,
        negated: bool,
    },
    /// "it regenerates this way": the regeneration shield this effect made is used (701.19).
    RegeneratesThisWay,
    /// "you chose a creature other than ~ as your Ring-bearer" (701.54).
    ChoseOtherRingBearer,
    /// "one or more of them entered from a graveyard or was cast from a graveyard" (`exile`:
    /// "from exile or was cast from exile").
    EnteredOrCastFrom { exile: bool },
    /// "green is the most common color among all permanents or is tied for most common" (105).
    MostCommonColor(crate::vocab::Color),
    /// "you revealed a Dragon card or controlled a Dragon as you cast ~": the additional cost
    /// was paid by revealing, or the player controlled one, as the spell was cast.
    RevealedOrControlledAsCast(crate::vocab::Subtype),
    /// "you own a card in exile", "you own a card named ~ in exile": the player owns such an
    /// object (108.3).
    Owns {
        player: Box<Selector>,
        what: crate::action::Coord<Selector>,
    },
    /// "you both own and control ~ and a creature named Hanweir Garrison": the player owns
    /// (108.3) and controls (110.2) each of them (meld, 701.42a).
    OwnsAndControls {
        player: Box<Selector>,
        what: crate::action::Coord<Selector>,
    },
    /// "an opponent is poisoned", "its controller is poisoned": the player has one or more
    /// poison counters (122.1f).
    Poisoned(Box<Selector>),
    /// "it isn't being declared as an attacker": the tapping is not the cost of attacking
    /// (508.1f).
    NotBeingDeclaredAttacker,
    /// "it hasn't dealt damage yet": this object has dealt no damage since it became what it
    /// is (120).
    HasntDealtDamageYet,
    /// "a creature is attacking you" (506.2).
    CreatureAttackingYou,
    /// "~ is in your graveyard with a creature card directly above it" (404.2: the order of a
    /// graveyard).
    InGraveyardBelowCreature,
    /// "a permanent's ability is countered this way": what the counter just countered was an
    /// ability of a permanent ("that permanent" afterwards).
    PermanentsAbilityCountered,
    /// "your library has no cards in it", "a graveyard has twenty or more cards in it": the
    /// zone holds that many cards.
    ZoneHas { zone: Box<Selector>, cards: Box<Selector> },
    /// "there are seven or more cards in your graveyard", "there is a creature with power 4 or
    /// greater on the battlefield".
    Exists {
        what: Box<Selector>,
    },
    /// "you have no cards in hand", "you have seven or more cards in hand".
    Has {
        player: Box<Selector>,
        what: Box<Selector>,
    },
    /// "~ is tapped", "it's a land card", "~ is in your graveyard", "enchanted permanent isn't a
    /// creature".
    Is {
        what: Box<Selector>,
        pred: Predicate,
        negated: bool,
    },
    /// "~ remains tapped", "it remains exiled", "~ remains on the battlefield": the predicate has
    /// held without a break since the effect began, so a "for as long as" duration ends the
    /// first time it stops holding (CR 611.2b).
    Remains {
        what: Box<Selector>,
        pred: Predicate,
    },
    /// "a creature died this turn", "you've cast a noncreature spell this turn", "~ didn't
    /// attack this turn", "a creature card is exiled this way", "you cast it from your hand": a
    /// query over the event log (CR 608.2i looks back in time). No window: an event that
    /// happens once to the definite object the pattern names, its casting or entering ("if you
    /// cast it", "~ was cast from a graveyard").
    Happened {
        pattern: EventPattern,
        window: Option<Window>,
        /// "has been activated four or more times this turn": how many times it happened in the
        /// window; None is at least once.
        times: Option<Quantity>,
        tense: Tense,
        negated: bool,
    },
    /// "~ was kicked", "the gift wasn't promised", "~'s madness cost was paid".
    CastWas {
        what: Box<Selector>,
        fact: CastFact,
        negated: bool,
    },
    /// "it's your turn", "it's not your turn".
    Turn {
        whose: Box<Selector>,
        negated: bool,
    },
    /// "it's your first, second, or third turn of the game": that player's turn, and of the
    /// turns they have taken, one of these (extra turns are turns too, 500.7).
    NthTurn {
        whose: Box<Selector>,
        turns: Vec<u32>,
    },
    /// "it's your main phase": the current step or phase and whose turn it is (CR 500.1).
    Step {
        step: Step,
        whose: StepOwner,
    },
    /// "you're the monarch", "you have the city's blessing", "you've completed a dungeon".
    Designation {
        player: Box<Selector>,
        designation: Designation,
        /// "you're not the monarch", "you don't have the initiative".
        negated: bool,
    },
    /// "X is 5 or more", "~'s power is 4 or greater", "a player has 10 or less life" (as
    /// Life(player) ≤ 10).
    Compare {
        left: Amount,
        cmp: Cmp,
        right: Amount,
    },
    /// "its controller pays {2}", "you sacrifice a land", "they sacrifice a nonland permanent of
    /// their choice or discard a card": after "unless", an offer the player may take, not a
    /// state (CR 118.12a).
    /// "If you don't put the card onto the battlefield" (`negated`): the deed not done.
    Does {
        player: Box<Selector>,
        actions: crate::action::Coord<Action>,
        negated: bool,
    },
    /// "you win the flip" (CR 705.2), "you lose the flip"; "you win" after a clash (701.30d),
    /// where `contest` is None: the contest an earlier instruction started.
    Won {
        player: Box<Selector>,
        won: bool,
        contest: Option<Contest>,
    },
    /// "an opponent controls more lands than you", "you have more cards in hand than each
    /// opponent", "you control fewer creatures than each opponent", "an opponent has more life
    /// than you": `player`'s measure against each of `than`'s. `margin`: "at least two more
    /// lands than you", "at least 10 life more than your starting life total" (the difference
    /// is at least that, not just one); None is "more than".
    MoreThan {
        player: Box<Selector>,
        measure: Measure,
        fewer: bool,
        margin: Option<Amount>,
        than: Than,
    },
    /// "it shares a creature type with ~", "it shares a card type with the exiled card": the two
    /// have at least one value of the characteristic in common (CR 105.2 for colors; 205.2a
    /// card types, 205.3 subtypes).
    Shares {
        what: Box<Selector>,
        characteristic: Characteristic,
        with: Box<Selector>,
        negated: bool,
    },
    /// "this is the second time this ability has resolved this turn"; `short`: "it's the third
    /// time", the same question after a first full sentence (Oracle's wording, kept to print).
    NthResolution {
        n: u32,
        short: bool,
    },
    /// "it's night", "it's neither day nor night" (CR 731.1).
    DayNight(DayNight),
    /// "it targets a tapped creature": one of the spell's or ability's targets matches (CR
    /// 115.1).
    Targets {
        what: Box<Selector>,
        target: Box<Selector>,
    },
    /// "{B} was spent to cast ~", "at least three white mana was spent to cast ~", "five or more
    /// mana was spent to cast that spell", "mana from a Treasure was spent to activate this
    /// ability" (the mana paid as the cost of what it was spent on, CR 601.2h, 602.2b), "that mana
    /// is spent on a creature spell" (the mana a text produced, when it is spent, CR 106.6).
    /// `negated`: "{C} wasn't spent to cast it".
    ManaSpent {
        mana: SpentMana,
        on: SpendOn,
        negated: bool,
    },
    /// "it was a creature card", "it had counters on it", "its mana value was 2 or less", "you
    /// controlled that permanent", "you won": the condition as it was when the object last
    /// existed or when the earlier event happened (last known information, CR 608.2h).
    Was(Box<Condition>),
    /// "if you pay {2} more to cast it": an additional cost the caster may pay (CR 601.2f).
    PaysMore {
        cost: ManaCost,
        spell: Box<Selector>,
    },
    /// "it's the first combat phase of the turn" (CR 500.8: a turn may have more than one).
    NthCombatPhase {
        n: u32,
    },
    /// "it's the second creature spell you cast this turn": the spell whose casting is being
    /// spoken of is the n-th such spell that player has cast in the turn (a look back at the
    /// turn, CR 608.2i).
    /// The named spell is the n-th matching spell its controller has cast this game (608.2i).
    NthCastThisGame { what: Box<Selector>, n: u32, set: ObjectSet },
    NthCastThisTurn {
        n: u32,
        set: ObjectSet,
    },
    /// "if you do", "if the player does", "if they don't", "if you can't": the outcome of the
    /// instruction before, which offered or required something of that player (CR 118.12a).
    Did {
        who: Box<Selector>,
        outcome: Outcome,
    },
    /// "if able": the instruction is a requirement, obeyed only when it can be (CR 508.1d for
    /// attacks, 118.8c for casting).
    Able,
    /// "grace gets more votes", "chaos gets more votes or the vote is tied" (701.38a).
    MoreVotes { option: String, or_tied: bool },
    /// "you're not playing for ante" (CR 407.1).
    PlayingForAnte {
        negated: bool,
    },
    And(Vec<Condition>),
    Or(Vec<Condition>),
    /// "if you control a commander as you cast ~": the condition as it was while the spell was
    /// being cast (601.2).
    AsYouCast(Box<Condition>),
}

/// What "is" asserts of an object.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Predicate {
    /// "tapped", "attacking", "red", "legendary".
    Adjective(Filter),
    /// "a creature", "a land card".
    A(ObjectSet),
    /// "in your graveyard", "on the battlefield".
    In(ZoneSpec),
    /// After "has": "flying", "a +1/+1 counter on it".
    Has(Filter),
    /// "paired with another creature" (soulbond, CR 702.95b).
    PairedWith(Box<Selector>),
    /// "in your opening hand" (CR 103.6).
    InOpeningHand,
    /// "attacking alone", "blocking alone": "A creature is attacking alone if it's attacking but
    /// no other creatures are" (CR 506.5); only attacking and blocking have it.
    Alone(crate::selector::Status),
    /// "your commander" (CR 903.3): the commander of that player.
    CommanderOf(Box<Selector>),
    /// "monstrous", "renowned", "suspended": designations an object can have.
    Designated(ObjectDesignation),
    /// "in the command zone or on the battlefield": any of these.
    Any(Vec<Predicate>),
}

/// What [`Condition::Did`] asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Outcome {
    /// "does"
    Did,
    /// "doesn't"
    Didnt,
    /// "can't": the instruction was impossible to follow.
    Couldnt,
}

/// Designations of objects that only conditions ask about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ObjectDesignation {
    /// CR 719.3b.
    Solved,
    /// CR 701.37b.
    Monstrous,
    /// CR 702.112b.
    Renowned,
    /// Until the end of the turn or it leaves the battlefield (CR 702.171b).
    Saddled,
    /// In exile with suspend and a time counter (CR 702.62b).
    Suspended,
    /// CR 722.3a.
    Prepared,
}

/// What [`Condition::MoreThan`] measures against.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Than {
    /// The same measure of other players: "than you", "than each opponent".
    Players(Box<Selector>),
    /// A number: "than your starting life total" (CR 103.4).
    Amount(Amount),
}

/// What [`Condition::MoreThan`] compares.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Measure {
    /// "controls more lands": the number of such permanents each player controls.
    Controls(ObjectSet),
    /// "has more cards in hand": the number of such cards each player has.
    Has(ObjectSet),
    /// "has more life" (life totals, CR 119).
    Life,
}

/// A contest a player wins or loses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Contest {
    /// "the flip" (CR 705.2).
    Flip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DayNight {
    Day,
    Night,
    /// "neither day nor night": the game has neither designation (CR 731.1).
    Neither,
}

/// Mana spent on a spell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SpentMana {
    /// "{B}", "{R}{R}": at least one mana of the symbol's kind per symbol.
    Symbols(ManaCost),
    /// How much mana of a kind: "at least three white mana", "five or more mana", "no colored
    /// mana" (exactly none), "mana from a Treasure" (at least one).
    Count { count: Quantity, kind: ManaKind },
    /// "two or more colors of mana", "exactly three colors of mana": how many different colors
    /// the mana was (CR 105.1; sunburst counts them, 702.44a).
    Colors(Quantity),
    /// "that mana", "this mana": the mana the text produced (106.6).
    That,
    /// "any of that mana": at least one of it.
    AnyOfThat,
}

/// Which mana a count of mana is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ManaKind {
    /// "mana".
    Any,
    /// "white mana".
    Color(Color),
    /// "colorless mana" (CR 105.1: colorless is not a color, {C}).
    Colorless,
    /// "colored mana": mana of any of the five colors.
    Colored,
    /// "mana of the same color": that many of one color.
    SameColor,
    /// "mana from a Treasure", "mana from creatures": produced by such a source (CR 106.6).
    From(ObjectSet),
}

/// The stretch of the game a history condition looks back over (CR 608.2i).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Window {
    ThisTurn,
    LastTurn,
    /// "this game": since it began.
    ThisGame,
    /// "during your last turn", "during their last turn": the most recent turn of that player.
    LastTurnOf(Box<Selector>),
    /// "since your last upkeep", "since your last turn": from the most recent such step or turn
    /// of that player until now.
    Since { step: Step, whose: Box<Selector> },
    /// "that turn": the turn the context named (the turn of the event that triggered).
    ThatTurn,
    /// "this step".
    ThisStep,
    /// "this combat" (the combat phase, CR 506).
    ThisCombat,
    /// "this way": the events the resolving spell's or ability's earlier instructions caused
    /// (CR 608.2c, its instructions are followed in order).
    ThisWay,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CastFact {
    /// "was kicked" (702.33).
    Kicked,
    /// "was kicked with its {2}{U} kicker": that one of its kicker costs was paid (702.33f).
    KickedWith(ManaCost),
    /// "was bargained" (702.166b).
    Bargained,
    /// "the gift was promised" (702.174k): the subject is the spell whose gift it is.
    GiftPromised,
    /// "was cast using teamwork" (702.194b).
    Teamwork,
    /// "tribute wasn't paid" (702.104b), of the creature with tribute.
    TributePaid,
    /// "was foretold" (702.143c).
    Foretold,
    /// "escaped" (702.138b).
    Escaped,
    /// "a Dragon was beheld" (701.4b): the behold cost was paid with an object of that set.
    Beheld(ObjectSet),
    /// "evidence was collected" (701.59c).
    EvidenceCollected,
    /// "~'s additional cost was paid" (601.2b).
    AdditionalCostPaid,
    /// "the {2}{U} cost was paid": the spell was cast by paying that mana cost (601.2b).
    ManaCostPaid(ManaCost),
    /// "~'s madness cost was paid", "its prowl cost was paid": cast for that keyword's
    /// alternative cost (601.2b).
    CostPaid(Keyword),
    /// "that creature was cast for its warp cost": of any object, that it was a spell cast
    /// for that keyword's alternative cost (601.2b), the permanent it became remembering so.
    CastFor(Keyword),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Designation {
    /// CR 725.
    Monarch,
    /// CR 702.131 (ascend).
    CitysBlessing,
    /// CR 726.
    Initiative,
    /// CR 309.7.
    CompletedDungeon,
    /// "you have an enduring story" (storied, CR 702.195b).
    EnduringStory,
}

/// "Activate only as a sorcery", "only once each turn", "only during your upkeep", "only if …"
/// (CR 602.5, 307.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Restriction {
    Sorcery,
    /// "only after combat": in the postcombat main phase or later that turn.
    AfterCombat,
    /// "only as an instant": a mana ability that can't be activated while mana abilities are
    /// (605.3), only when an instant could be cast.
    Instant,
    /// "only during any upkeep step": any player's upkeep.
    AnyUpkeep,
    /// "only before the combat damage step" (510).
    BeforeCombatDamage,
    OnceEachTurn,
    /// "only once" (602.5).
    Once,
    /// "no more than twice each turn", "no more than three times each turn".
    TimesEachTurn(Amount),
    /// "only during your turn", "only during your upkeep", "only during combat".
    During(During),
    /// "only before attackers are declared", "only during combat before blockers are declared",
    /// "only during combat after blockers are declared" (CR 508.1, 509.1).
    Declaration { in_combat: bool, after: bool, blockers: bool },
    /// "only during your turn, before attackers are declared": both at once.
    DuringBefore { during: During, blockers: bool },
    If(Box<Condition>),
}
