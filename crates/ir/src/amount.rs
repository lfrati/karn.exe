//! Amounts: numbers in card text.
//!
//! A literal, X, or an expression over the game: a count of a set, a stat of an object, an
//! aggregate over a set, counters on an object, a player's life or devotion, an earlier amount
//! ("that much"), a tally of what happened (life lost this way, damage dealt this turn, mana spent
//! to cast a spell, times a spell was kicked), and a little arithmetic.

use crate::action::PlayerCounter;
use crate::event::{DamageKind, EventPattern};
use crate::mana::ManaSymbol;
use crate::selector::{CounterKind, ObjectSet, Selector, Stat};
use crate::vocab::Color;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Amount {
    /// A literal ("two", "3").
    Const(u32),
    /// Total mana value of the distinct permanents this spell targets, rather than
    /// the number of target occurrences (115.3). Evaluated after target selection.
    TargetedPermanentManaValue,
    /// The spell's or ability's X (CR 107.3).
    X,
    /// "for each strength vote": the votes an option got (701.38a).
    Votes(String),
    /// "the number of creatures you control", "for each card in your graveyard".
    Count(Box<ObjectSet>),
    /// "its power", "~'s toughness", "the sacrificed creature's mana value", "the power of target
    /// creature you control".
    Stat {
        of: Box<Selector>,
        stat: Stat,
    },
    /// "the greatest power among creatures you control", "the total power of creatures you
    /// control".
    Aggregate {
        agg: Aggregate,
        stat: Stat,
        among: Box<ObjectSet>,
    },
    /// "the number of +1/+1 counters on ~", "for each charge counter on ~".
    Counters {
        on: Box<Selector>,
        kind: Option<CounterKind>,
    },
    /// "the number of cards looked at while scrying this way" (701.22).
    LookedAtScrying,
    /// "for each flip you won" (705.2).
    FlipsWon,
    /// "for each flip you lose" (705.2).
    FlipsLost,
    /// "the highest number you noted for cards named ~" (a draft matters card, 905).
    HighestNoted,
    /// "the number of unlocked doors among Rooms you control" (709.5).
    UnlockedDoors,
    /// "for each graveyard with seven or more cards in it": the graveyards that hold such cards.
    GraveyardsWith(Box<Selector>),
    /// "for each mana from a Treasure spent to cast it": the mana spent on the spell that such
    /// a source produced (CR 106.6).
    ManaSpentFrom {
        spell: Box<Selector>,
        from: Box<ObjectSet>,
    },
    /// "the greatest number of creatures a player controls", "the greatest number of cards a
    /// player discarded this way": the largest of the counts the phrase leaves open.
    GreatestCount(Box<Amount>),
    /// "for each kind of counter on target permanent or player": the kinds of counters it has.
    CounterKindsOn(Box<Selector>),
    /// "for each kind of counter among creatures you control".
    CounterKindsAmong(Box<ObjectSet>),
    /// "the number of +1/+1 counters on creatures you control", "the number of counters among
    /// permanents you control": counters summed over a set (CR 122.1).
    CountersAmong {
        among: Box<ObjectSet>,
        kind: Option<CounterKind>,
    },
    /// "the number of experience counters you have", "the amount of {E} you have", "for each
    /// poison counter your opponents have": counters on players (CR 122.1, 107.14).
    PlayerCounters {
        player: Box<Selector>,
        kind: PlayerCounter,
    },
    /// "the amount of unspent mana you have", "for each unspent green mana you have" (CR 106.4):
    /// the mana in a player's pool, of one color or of any.
    Unspent {
        player: Box<Selector>,
        color: Option<Color>,
    },
    /// "your life total", "that player's life total".
    Life(Box<Selector>),
    /// "your starting life total" (CR 103.4).
    StartingLife(Box<Selector>),
    /// "the highest life total among all players", "the lowest life total among your opponents":
    /// the greatest or least of the players' life totals (119.1: each player has one). Apart from
    /// [`Amount::Aggregate`], whose stats are object characteristics; `agg` is never `Total`.
    LifeAggregate {
        agg: Aggregate,
        among: Box<ObjectSet>,
    },
    /// "your speed" (CR 702.179f: 0 for a player who has no speed).
    Speed(Box<Selector>),
    /// "the number of creatures in your party" (CR 700.8).
    Party(Box<Selector>),
    /// "your devotion to black", "your devotion to that color" (CR 700.5).
    Devotion {
        player: Box<Selector>,
        to: DevotionTo,
    },
    /// "the number of basic land types among lands you control" (domain), "colors among …",
    /// "the number of differently named lands you control".
    Distinct {
        what: Characteristic,
        among: Box<ObjectSet>,
    },
    /// "the number of colors it is", "for each of its colors" (CR 105.2).
    Colors(Box<Selector>),
    /// "the number of red mana symbols in the mana costs of permanents you control", "for each
    /// white mana symbol in its mana cost" (CR 202.1, as devotion counts them, 700.5). A single
    /// object is the set of its members ([`crate::Atom::Among`]).
    ManaSymbols {
        color: Color,
        among: Box<ObjectSet>,
    },
    /// "that much" / "that many": the amount of an earlier step or of the trigger event, bound
    /// by the reference resolver. `many` is the countable spelling.
    That {
        many: bool,
    },
    /// A number an earlier instruction produced, by what produced it: "the result", "that
    /// number", "the chosen number", "the difference"; bound by the reference resolver.
    Recalled(Recalled),
    /// "the amount of life you gained this turn", "the life lost this way", "the life that player
    /// lost" (CR 119.3). `player` is None when the text names nobody ("the life lost this way").
    LifeChange {
        player: Option<Box<Selector>>,
        gained: bool,
        when: When,
    },
    /// "the damage dealt this way", "the amount of damage ~ dealt to that player", "the excess
    /// damage dealt this way" (CR 120; excess damage 120.10).
    Damage {
        by: Option<Box<Selector>>,
        to: Option<Box<Selector>>,
        kind: DamageKind,
        excess: bool,
        when: When,
    },
    /// "the damage prevented this way", "for each 1 damage prevented this way" (CR 615.1).
    DamagePrevented {
        when: When,
    },
    /// "the amount of mana spent to cast ~", "the number of colors of mana spent to cast it",
    /// "for each {S} spent to cast ~" (paid as the spell was cast, CR 601.2h; sunburst counts
    /// colors, 702.44a).
    ManaSpent {
        spell: Box<Selector>,
        measure: ManaMeasure,
    },
    /// "the total amount of mana paid this way", "the amount of {E} paid this way", "the life
    /// paid as it entered" (a cost paid as it entered, CR 607.2g).
    Paid {
        player: Option<Box<Selector>>,
        what: Payment,
        when: When,
    },
    /// "for each time it was kicked" (CR 702.33d), "the number of times ~ has mutated"
    /// (702.140), "for each time you've cast your commander from the command zone this game"
    /// (903.8).
    Times {
        what: Box<Selector>,
        fact: Occurrence,
    },
    /// "for each charge counter removed this way", "the number of +1/+1 counters removed this
    /// way" (CR 122).
    CountersRemoved {
        kind: Option<CounterKind>,
        when: When,
    },
    /// "the number of creatures that died this turn", "for each card you've drawn this turn",
    /// "for each opponent who lost life this turn": the objects or players that took part in an
    /// event matching `pattern` during `when` (a query over the event log, as conditions ask "if
    /// a creature died this turn", CR 700.4). The counted participant is the `counted` slot of
    /// the pattern, a selector "a <set>".
    Participants {
        pattern: Box<EventPattern>,
        counted: Role,
        when: When,
    },
    /// "twice X" (2), "three times X": a whole multiple of an amount (CR 107.1). Named apart from
    /// [`Amount::Times`], which counts occurrences.
    Multiple(u32, Box<Amount>),
    /// "half X, rounded up" / "rounded down" (CR 107.1a). `round_up` is None when the rounding is
    /// not next to the amount ("draw half X cards, rounded down") or not stated.
    Half {
        of: Box<Amount>,
        round_up: Option<bool>,
    },
    Plus(Box<Amount>, Box<Amount>),
    Minus(Box<Amount>, Box<Amount>),
    /// "the number of Mountains you control as you cast ~", "… as you activate this ability":
    /// the amount fixed at that moment rather than when the effect applies (CR 601.2f, 608.2h).
    AsOf {
        amount: Box<Amount>,
        moment: Moment,
    },
}

/// The colors a devotion is counted for (CR 700.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum DevotionTo {
    /// "black", "white and blue": one color, or two counted together (700.5).
    Colors(Vec<Color>),
    /// "that color": the color the instruction before named ("Choose a color. Add an amount of
    /// mana of that color equal to your devotion to that color.").
    ThatColor,
}

/// When an [`Amount::AsOf`] is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Moment {
    /// "as you cast ~": while its total cost is determined (CR 601.2f).
    Casting,
    /// "as you activate this ability" (CR 602.2b).
    Activating,
    /// "as this ability resolves" (CR 608.2h).
    Resolving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Aggregate {
    Greatest,
    Least,
    Total,
}

/// What a distinct count counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Characteristic {
    Color,
    CardType,
    BasicLandType,
    CreatureType,
    Name,
    ManaValue,
    Power,
    /// Artifact, battle, creature, enchantment, land, planeswalker (CR 110.4).
    PermanentType,
    /// "planeswalker types among planeswalkers you control" (CR 205.3j).
    PlaneswalkerType,
    /// "different kinds of counters among permanents you control" (CR 122.1).
    CounterKind,
}

/// A comparison in "power 2 or less", "mana value 3 or greater", "toughness equal to 1".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Cmp {
    Eq,
    Le,
    Ge,
    Lt,
    Gt,
}

/// When the events an amount tallies happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum When {
    /// The text says nothing: the trigger event or the instruction just done, bound by the
    /// reference resolver ("Whenever you gain life, … equal to the amount of life you gained").
    Context,
    /// "this way": what an earlier instruction of the same ability did (CR 608.2c: later text
    /// may refer back to earlier text).
    ThisWay,
    /// "this turn".
    ThisTurn,
    /// "this game" (commander casts, CR 903.8).
    ThisGame,
    /// "as it entered", "as ~ entered": a cost paid as the permanent entered (CR 607.2g).
    AsEntered,
}

/// Which participant of an event pattern a count counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Role {
    /// The subject: "creatures that died", "opponents who lost life".
    Subject,
    /// The object of the action: "cards you've drawn", "opponents you attacked".
    Object,
}

/// A number an earlier instruction produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Recalled {
    /// "the result": of a die roll (CR 706.2) or another random process.
    Result,
    /// "the other result": the second of two results.
    OtherResult,
    /// "that number": a number an earlier sentence computed or chose.
    Number,
    /// "the chosen number": a number chosen earlier (CR 607.2d).
    ChosenNumber,
    /// "the difference": between two amounts an earlier condition compared ("If target player
    /// has fewer than nine poison counters, they get … equal to the difference").
    Difference,
}

/// What a tally of mana spent measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ManaMeasure {
    /// "the amount of mana spent".
    Total,
    /// "the number of colors of mana spent", "for each color of mana spent" (CR 702.44a).
    Colors,
    /// "the amount of {S} spent", "for each {S} spent".
    Symbol(ManaSymbol),
}

/// What was paid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Payment {
    Mana,
    /// {E} (CR 107.14).
    Energy,
    Life,
}

/// What a "number of times" counts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Occurrence {
    /// Number of payments of this spell's replicate cost (702.56a).
    Replicated,
    /// "it was kicked" (CR 702.33d: a spell with multikicker may be kicked several times).
    Kicked,
    /// "~ has mutated" (CR 702.140).
    Mutated,
    /// "you've cast your commander from the command zone this game", "it's been cast from the
    /// command zone this game" (CR 903.8). `by` is the caster when the text names one.
    CastFromCommandZone { by: Option<Box<Selector>> },
}
