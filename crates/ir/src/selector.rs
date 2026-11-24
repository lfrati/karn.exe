//! Selectors: how card text names objects and players.
//!
//! "up to two target nonland permanents you don't control" is
//! `Selector { quant: Target(UpTo(2)), set: ObjectSet { domain: Permanent,
//! filter: Not(Type(Land)), controller: Some(Controller { who: you, negated: true }), .. } }`.
//!
//! A determiner
//! ([`Quant`]) over a set description ([`ObjectSet`]: domain × filter × controller × owner × zone
//! × exclusion). The set description alone is what event patterns, conditions and counts use
//! ("whenever a creature you control dies", "the number of Elves you control").

use crate::action::Coord;
use crate::amount::{Amount, Characteristic, Cmp, When};
use crate::condition::{Measure, Outcome};
use crate::event::EventAction;
use crate::lexicon::Verb;
use crate::mana::ManaSymbol;
use crate::token::KeywordInstance;
use crate::vocab::{CardType, Color, Keyword, Subtype, Supertype, Zone};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Selector {
    pub quant: Quant,
    pub set: ObjectSet,
    /// "of their choice", "of your choice": who picks, when the text says so. Otherwise the
    /// player the action instructs picks (decided by the action, not here).
    pub chooser: Option<Box<Selector>>,
    /// "at random", "chosen at random".
    pub random: Option<Random>,
}

/// The determiner: how many, and whether targeted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Quant {
    /// "target", "two target", "up to one target", "any number of target" (CR 115).
    Target(Count),
    /// "a", "an", "two", "up to three", "one or more", "any number of": untargeted. In an effect
    /// the instructed player chooses on resolution; in a trigger or condition it is existential.
    Indefinite(Count),
    /// "each creature": every object in the set.
    Each,
    /// "all creatures": the same universal quantifier as [`Quant::Each`] in the plural. Kept apart
    /// because Oracle chooses between them freely ("Destroy all creatures" / "Destroy each
    /// creature with …") and the engine treats them identically. A bare plural ("Creatures you
    /// control get +1/+1") is not a selector but an [`ObjectSet`].
    All,
    /// The referent is fixed by context: "that creature", "enchanted creature", "~", "you".
    Definite(Ref),
    /// "no creatures" (in conditions).
    No,
    /// "the next instant or sorcery spell you cast this turn": the first object of the set to
    /// come to be (cast, CR 601.2, or entering) during the rest of this turn.
    NextThisTurn,
    /// "the first creature spell you cast each turn", "the second card you draw each turn": the
    /// nth object of the set to come to be in each turn.
    NthEachTurn(u32),
    /// "half the creatures they control of their choice, rounded up": half of the set, rounded
    /// as stated (None: not stated, 107.1a).
    Half(Option<bool>),
    /// "any player may activate this ability", "unless any player pays {2}", "a copy of any
    /// creature on the battlefield": whichever one, with no restriction on which; a permission
    /// or offer extends to every player (who answer in turn order, CR 101.4), a choice ranges
    /// over every object of the set.
    Any,
    /// A definite description that picks its objects out by itself: "the top card of your
    /// library", "the top three cards of your library".
    The,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Count {
    Exactly(Amount),
    UpTo(Amount),
    AtLeast(Amount),
    AnyNumber,
    /// "exactly one creature", "exactly two creatures you control": that many and no more (an
    /// integer, CR 107.1), where a bare count in a condition is a lower bound ("you control two
    /// creatures").
    Precisely(Amount),
    /// "one or two target creatures": at least `min`, at most `max` (a variable number of
    /// targets, announced as the spell is cast, CR 601.2c).
    Range { min: Box<Amount>, max: Box<Amount> },
}

/// What kind of thing the head noun names. The domain is the only thing a zone defaults from:
/// "creature" is on the battlefield, "creature spell" on the stack, "creature card" in a zone the
/// text must name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Domain {
    /// A permanent: "creature", "permanent", "Goblin", "token".
    Permanent,
    /// A card in a zone: "card".
    Card,
    /// A spell on the stack.
    Spell,
    /// An activated or triggered ability on the stack: "ability".
    Ability,
    /// A player: "player", "opponent".
    Player,
    /// A source of damage (any object, CR 120.2 / 609.7): "source".
    Source,
    /// "any target": a creature, player, planeswalker or battle (CR 115.4).
    AnyTarget,
    /// Any object, when the text does not say ("it" before the reference is resolved).
    Object,
    /// A union of complete set descriptions. Every arm keeps its own domain and
    /// qualifiers (109.2, 113.7a); constraints on the enclosing set apply to all arms.
    Union(Vec<ObjectSet>),
}

/// One alternative in the meaning of a noun's domain (109.2, 112.1, 115.4).
/// Both static role checking and runtime membership use this declaration. Compound surface
/// nouns do not need their own interpretation in each engine subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainAlternative {
    pub sort: crate::lexicon::Sort,
}

impl Domain {
    pub fn atomic_alternatives(&self) -> Option<&'static [DomainAlternative]> {
        use crate::lexicon::Sort;
        const fn one(sort: Sort) -> DomainAlternative { DomainAlternative { sort } }
        const PERMANENT: DomainAlternative = one(Sort::Permanent);
        const PLAYER: DomainAlternative = one(Sort::Player);
        const SPELL: DomainAlternative = one(Sort::Spell);
        const ABILITY: DomainAlternative = one(Sort::Ability);
        const CARD: DomainAlternative = one(Sort::Card);
        const CREATURE: DomainAlternative = one(Sort::Of(CardType::Creature));
        const PLANESWALKER: DomainAlternative = one(Sort::Of(CardType::Planeswalker));
        const BATTLE: DomainAlternative = one(Sort::Of(CardType::Battle));
        Some(match self {
            Self::Permanent => &[PERMANENT],
            Self::Player => &[PLAYER],
            Self::Card => &[CARD],
            Self::Spell => &[SPELL],
            Self::Ability => &[ABILITY],
            Self::AnyTarget => &[CREATURE, PLANESWALKER, BATTLE, PLAYER],
            Self::Source | Self::Object => &[PERMANENT, CARD, SPELL, ABILITY],
            Self::Union(_) => return None,
        })
    }
}

/// Facts about one possible alternative of a selector, not a claim that a matching object
/// currently exists. Types overlap (205.2); only an explicit denial excludes a card type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Possibility {
    pub domain: DomainAlternative,
    pub excluded_types: Vec<CardType>,
}

/// A set description: everything about a selector except the determiner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ObjectSet {
    pub domain: Domain,
    pub filter: Filter,
    /// "you control", "an opponent controls", "you don't control".
    pub controller: Option<Relation>,
    /// "you own", "you don't own".
    pub owner: Option<Relation>,
    /// "spells you cast", "spells your opponents cast", "spells you cast from your graveyard".
    pub caster: Option<Casting>,
    pub zone: Option<ZoneSpec>,
    /// "other", "another", "each other": excludes the source or the antecedent; the reference
    /// resolver decides which.
    pub other: bool,
}

/// Who cast a spell, and how: "you cast", "you cast from your graveyard", "you cast this turn".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Casting {
    pub by: Relation,
    /// "from your graveyard": the zone it was cast from (601.2a).
    pub from: Option<ZoneSpec>,
    /// "from anywhere other than your hand": cast from any zone but `from`.
    pub other_than: bool,
    /// "this turn": cast during this turn.
    pub this_turn: bool,
}

/// "controlled by X" / "owned by X", possibly negated ("you don't control").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Relation {
    pub who: Box<Selector>,
    pub negated: bool,
    /// "a creature you controlled", "creatures they controlled that were exiled this way": the
    /// relation as it last held, for an object that has left the battlefield (its last known
    /// information, CR 608.2h).
    pub past: bool,
}

/// "in your graveyard", "from a graveyard", "in all graveyards", "of your library", "from your
/// hand or graveyard", "their graveyard, hand, and library".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ZoneSpec {
    /// The zone, or several zones of the one owner (the cards in any of them).
    pub zones: Coord<Zone>,
    /// Whose zone ("your graveyard", "target player's hand", "a graveyard", "all graveyards").
    /// None for shared zones (battlefield, stack, exile).
    pub owner: Option<Box<Selector>>,
    /// Part of an ordered zone: "the top three cards of your library".
    pub slice: Option<Slice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Slice {
    Top(Amount),
    Bottom(Amount),
    /// "the top creature card of your graveyard": the topmost card of the zone that the set's
    /// filter admits.
    TopOfKind,
}

/// A predicate over objects, as And/Or/Not over atoms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Filter {
    Atom(Atom),
    Not(Box<Filter>),
    /// Conjunction; `All(vec![])` is "anything in the domain".
    All(Vec<Filter>),
    Any(Vec<Filter>),
}

impl Filter {
    pub fn any_object() -> Filter {
        Filter::All(Vec::new())
    }

    /// Conjoin, flattening nested conjunctions and dropping the empty one.
    pub fn and(self, other: Filter) -> Filter {
        let mut parts = Vec::new();
        for f in [self, other] {
            match f {
                Filter::All(v) => parts.extend(v),
                f => parts.push(f),
            }
        }
        if parts.len() == 1 { parts.pop().expect("one part") } else { Filter::All(parts) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Atom {
    /// Membership in the result of an earlier instruction (608.2c).
    Reference(crate::reference::UnresolvedRef),
    Type(CardType),
    Subtype(Subtype),
    /// At least one of the five basic land types (205.3i), regardless of basic supertype.
    BasicLandType,
    Supertype(Supertype),
    Color(Color),
    Colorless,
    /// Two or more colors (CR 105.2c).
    Multicolored,
    /// Exactly one color (CR 105.2b).
    Monocolored,
    Status(Status),
    /// "with power 2 or less", "with mana value 3 or greater".
    Stat {
        stat: Stat,
        cmp: Cmp,
        value: Amount,
    },
    /// "with an even mana value", "with odd mana values", "with mana value of the chosen
    /// quality" (after "choose odd or even"): the stat is even or odd (107.1: integers).
    Parity {
        stat: Stat,
        parity: Parity,
    },
    /// "with flying", "without flying" (via Not).
    Keyword(Keyword),
    /// "an Aura card with enchant creature", "a creature with toxic 2": a keyword together with
    /// its parameter (CR 702.5a enchant; a keyword without one is [`Atom::Keyword`]).
    KeywordWith(Box<KeywordInstance>),
    /// A player who is an opponent of the ability's controller ("opponent", CR 102.3).
    Opponent,
    /// Opponent of the named player, rather than of this ability's controller (102.3).
    OpponentOf(Box<Selector>),
    /// "named ~", "named Relentless Rats", "named Bogbrew Witch or Festering Newt" (CR 201.2).
    /// "or" / "and/or": the object has any of the names. "and" ("artifacts you control named A
    /// and B"): the set holds one object for each name, so no single object satisfies it.
    Named(Coord<Name>),
    /// "level 2 or greater": a permanent's level, 1 if it has none (716.2d). Said by the rules
    /// a class level bar stands for (716.2a), not read from card text.
    Level(Count),
    /// "with a +1/+1 counter on it", "with three or more time counters on it".
    Counters {
        kind: Option<CounterKind>,
        count: Count,
    },
    /// "exiled with ~": linked to the ability that exiled it (CR 607).
    ExiledWith(Ref),
    /// "tokens created with ~": made by an ability of that object (CR 607).
    CreatedWith(Ref),
    /// "an exiled creature card used to craft ~" (702.167).
    UsedToCraft(Ref),
    /// "each opponent who voted for a choice you voted for" (`with`), "… you didn't vote for"
    /// (701.38).
    VotedAsYou { with: bool },
    /// "each other creature with the same controller": the controller of the object the text
    /// named before.
    SameController,
    /// "the creature put onto the battlefield with ~": by that object's ability (CR 607).
    PutOntoBattlefieldWith(Ref),
    /// "of the chosen type", "of the chosen color", "with the chosen name": a value chosen
    /// earlier and remembered by the source (CR 607.2d).
    Chosen(Chosen),
    /// "creatures of the creature type of your choice", "of the color of your choice": a value the
    /// player chooses as the instruction resolves (unlike [`Atom::Chosen`], chosen earlier and
    /// remembered; CR 608.2d choices made on resolution).
    OfChoice(Chosen),
    /// A permanent card or spell: artifact, battle, creature, enchantment, land or planeswalker
    /// (CR 110.4b).
    PermanentType,
    /// "attached to it", "attached to a creature you control".
    AttachedTo(Box<Selector>),
    /// "from among them", and the partitive "one of those cards": one of a group the text
    /// already named.
    Among(Box<Selector>),
    /// "creatures able to block ~", "… able to block target creature this turn" (`this_turn`):
    /// what could block it (with "do so", a blocking requirement, 509.1c).
    AbleToBlock { what: Box<Selector>, this_turn: bool },
    /// "a card you own from outside the game": in no zone of the game (400.11; a sideboard).
    OutsideTheGame,
    /// "from among cards exiled with ~": one of the objects of a set the text describes.
    AmongSet(Box<ObjectSet>),
    /// "a nonland card from it": a card in the hand or pile "it" names ("Target opponent
    /// reveals their hand. You choose a nonland card from it.").
    FromIt,
    /// Objects in a previously made pile (700.3); `chooser` names whose pick it is.
    InPile { pile: crate::procedure::PileRef, chooser: Option<Box<Selector>> },
    /// "dealt damage by ~ this turn"; without `this_turn`, "a permanent dealt damage by ~
    /// would die this turn", where the replacement's own "this turn" bounds it.
    DealtDamageBy { by: Box<Selector>, this_turn: bool },
    /// "creatures that died this turn", "target creature that was dealt damage this turn", "each
    /// player who lost life this turn", "creatures that didn't attack this turn": the object or
    /// player was the subject of an event matching `action` during `when` (a look back at
    /// previous game actions, CR 608.2i; "died", 700.4). With the window [`When::ThisWay`] it
    /// is what an earlier instruction of the same ability did to it (CR 608.2c: later text may
    /// refer back to earlier text): "cards revealed this way", "the creature exiled this way",
    /// "cards put into your graveyard this way", "cards exiled from their hand this way", "cards
    /// not cast this way" (`negated`). The doer, when the text says ("cards they exiled this
    /// way"), is the `by` role of the passive [`crate::event::Deed`].
    Did {
        action: Box<EventAction>,
        when: When,
        negated: bool,
    },
    /// "a spell that targets ~", "spells that target you or a creature you control": a spell or
    /// ability with a target `what` names (CR 115.9b); `only`: "that targets only a single
    /// creature", every one of its targets does (115.9c).
    Targets {
        what: Coord<Selector>,
        only: bool,
    },
    /// "on the battlefield or in the command zone" (as a union), "cards you own that aren't on
    /// the battlefield" (via Not): in that zone (CR 400.1). A single positive zone is the set's
    /// [`ObjectSet::zone`].
    InZone(ZoneSpec),
    /// "a creature spell that has an Adventure": an adventurer card or an object with its
    /// alternative characteristics (CR 715.2a).
    HasAdventure,
    /// A generated prepare spell and its copies (722.3c–d).
    PreparedSpell,
    /// "a spell with {X} in its mana cost", "without {X} in its mana cost" (via Not): the symbol
    /// appears in its mana cost (CR 202.1, 107.3 for X).
    InManaCost(ManaSymbol),
    /// "an artifact card with mana cost {0} or {1}": its mana cost is one of these, exactly as
    /// printed (CR 202.1), not its mana value (202.3). An object with no mana cost has none.
    ManaCost(Vec<crate::mana::ManaCost>),
    /// "each other creature you control that spell could target": a legal target for that spell
    /// or ability (CR 115.7a: a target may be changed only to another legal target).
    CouldTarget(Box<Selector>),
    /// "an Aura card that could enchant it": an Aura whose enchant ability would let it be
    /// attached to that object (CR 303.4a).
    CouldEnchant(Box<Selector>),
    /// "target spell with a single target", "a spell with one or more targets": how many times
    /// an object or player was chosen as its target (CR 115.9a).
    TargetCount(Count),
    /// "target activated ability", "a mana ability", "loyalty abilities": the kind of an ability
    /// (CR 113.3b–c, 605.1, 606.1).
    AbilityKind(AbilityKind),
    /// "a creature with a mana ability", "lands without mana abilities", "a permanent with an
    /// activated ability": the object has an ability of that kind (CR 113.3, 605.1, 606.1).
    HasAbility(AbilityKind),
    /// "abilities from artifact sources", "activated ability from an artifact": the ability's
    /// source is one of these (CR 113.7a: the source of an ability on the stack is the object that
    /// has it; 609.7 for sources in general). "Abilities of creatures you control" is not this: it
    /// speaks of the abilities an object has (113.3), which is [`crate::AbilitiesOf`]'s.
    Source(Box<ObjectSet>),
    /// "creatures blocking it", "each creature blocking target attacking creature": blocking
    /// one of the creatures `0` names (CR 509.1g).
    Blocking(Box<Selector>),
    /// "creatures blocked by ~", "each creature it's blocking": an attacking creature one of the
    /// creatures `0` names is blocking (CR 509.1g).
    BlockedBy(Box<Selector>),
    /// "creatures attacking you", "each creature attacking a planeswalker you control": attacking
    /// that player, planeswalker or battle (CR 506.3, 508.1).
    Attacking(Box<Selector>),
    /// "with the same name as that creature" (CR 201.2a: at least one name in common), "that
    /// shares a color with ~" (105.2), "that shares a creature type with it" (205.3m): at least
    /// one value of `what` in common with each object `with` names.
    Shares {
        what: Characteristic,
        with: Box<Selector>,
    },
    /// The candidate has at least one name and no name in common with any object
    /// the reference names (201.2c). Referenced nameless objects do not invalidate this.
    /// Unlike negated Shares, this rejects a nameless candidate and covers the whole group.
    DifferentNameFrom(Box<Selector>),
    /// "with that name": the name an earlier instruction chose or named ("Choose a card name.
    /// Search … for any number of cards with that name", CR 201.4); the resolver binds it.
    ThatName,
    /// "of that type": a card type or subtype (CR 205.2a, 205.3) an earlier instruction chose or
    /// named ("Choose a creature type. … creatures of that type"); the resolver binds it.
    ThatType,
    /// "of that color": a color an earlier instruction chose or named ("Choose a color. … can't
    /// be blocked by creatures of that color", CR 105.1); the resolver binds it.
    ThatColor,
    /// "with lesser mana value", "with greater power", "with equal or lesser mana value": `stat`
    /// compared with the same characteristic of the object the context names ("Whenever you cast
    /// your first spell each turn, … if it's a spell with lesser mana value"; CR 202.3 mana value,
    /// 208.1 power and toughness). With the object named ("with lesser mana value than that
    /// creature") it is an ordinary [`Atom::Stat`] against that object's characteristic.
    Compared {
        stat: Stat,
        cmp: Cmp,
    },
    /// "with the greatest power among creatures they control", "with the least toughness", "with
    /// the greatest mana value or tied for the greatest mana value": no object of `among` (the
    /// selection's own candidates when None) has a greater (`least`: lesser) `stat` (CR 208.1
    /// power and toughness, 202.3 mana value); `tied`: an object tied for it also counts, else
    /// the text leaves ties to the rules of the instruction.
    Most {
        stat: Stat,
        least: bool,
        among: Option<Box<ObjectSet>>,
        tied: bool,
    },
    /// "each opponent who does", "each player who doesn't", "each opponent who can't": how the
    /// player answered the instruction before, which offered or required something of each of
    /// them (as [`crate::Condition::Did`] asks of one player; CR 118.12a).
    DidSo(Outcome),
    /// "a player who controls eight or more lands", "an opponent who controls an artifact named
    /// ~": the player controls what the selector names (CR 110.2), its determiner being the
    /// count (as [`crate::Condition::Controls`] asks of one player).
    Controls(Box<Selector>),
    /// "each opponent who controls more lands than you", "a player who has more life than you",
    /// "who has more cards in hand than you": the player's `measure` against each of `than`'s
    /// (as [`crate::Condition::MoreThan`] asks of one player; life totals, CR 119).
    MoreThan {
        measure: Box<Measure>,
        fewer: bool,
        than: Box<Selector>,
    },
    /// "the player with the most life or tied for most life", "each player who controls the
    /// most creatures", "who controls the fewest creatures": no player has a greater (`least`:
    /// lesser) `measure` (life totals, CR 119); `tied`: a player tied for it also counts.
    MostOf {
        measure: Box<Measure>,
        least: bool,
        tied: bool,
    },
    /// "with different names" (CR 201.2b), "with different powers": a collective predicate,
    /// true of the selected objects together when no two of them share a value of `what`.
    Different(Characteristic),
    /// "that share a color", "that share a creature type", "with the same name": a collective
    /// predicate, true of the selected objects together when they all have a value of `what` in
    /// common (CR 105.2, 205.3m, 201.2a).
    Alike(Characteristic),
    /// "two target creatures controlled by different players" (`same` false), "controlled by the
    /// same player": a collective predicate on the selected objects' controllers (CR 110.2).
    Controllers {
        same: bool,
    },
    /// "up to two target cards from a single graveyard": a collective predicate, true of the
    /// selected objects together when they are all in one zone (one player's graveyard, CR
    /// 404.1: each player has their own); the set's zone names which kind.
    SameZone,
    /// "with total power 6 or greater", "with total mana value 5 or less": a collective
    /// predicate on the sum of `stat` over the selected objects (CR 208.1 power, 202.3 mana
    /// value).
    Total {
        stat: Stat,
        cmp: Cmp,
        value: Amount,
    },
}

/// Kinds of abilities a selector can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum AbilityKind {
    /// "[Cost]: [Effect.]" (CR 113.3b).
    Activated,
    /// "When/Whenever/At …" (CR 113.3c).
    Triggered,
    /// An activated or triggered ability that could add mana and has no target (CR 605.1).
    Mana,
    /// A planeswalker's "[+1]: …" ability (CR 606.1).
    Loyalty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Name {
    This,
    /// The name of the object that granted the quoted ability this is in (`§`, CR 201.5a).
    Granting,
    Card(String),
}

/// A counter kind as printed ("+1/+1", "-1/-1", "loyalty", "time"); an open set in the corpus
/// (256 kinds), so kept by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CounterKind(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Chosen {
    Type,
    CreatureType,
    Color,
    Name,
}

/// Statuses and designations an object can have (CR 110.5, 701, 702, 708 …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Status {
    Tapped,
    Untapped,
    Attacking,
    Blocking,
    Blocked,
    Unblocked,
    FaceDown,
    FaceUp,
    Token,
    /// Has a counter, is equipped or enchanted by a permanent its controller controls (700.9).
    Modified,
    /// Artifact, legendary or Saga (700.6).
    Historic,
    /// An Assassin, Mercenary, Pirate, Rogue and/or Warlock (700.12).
    Outlaw,
    /// A card in exile that has suspend and a time counter on it (702.62b).
    Suspended,
    /// Enchanted by an Aura (as a filter: "each enchanted creature", not "enchanted creature").
    Enchanted,
    Equipped,
    /// A commander (903.3).
    Commander,
    Suspected,
    Saddled,
    Goaded,
    Transformed,
    /// "a kicked spell": its controller declared the intention to pay a kicker cost (CR
    /// 702.33d).
    Kicked,
}

/// Whether a number is even or odd, or which of the two an earlier "choose odd or even" chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Parity {
    Even,
    Odd,
    Chosen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Stat {
    Power,
    Toughness,
    /// "base power": the power before effects that modify it, only after those that set it
    /// (CR 613.4b: layer 7b sets, 7c modifies; 208.1).
    BasePower,
    /// "base toughness" (CR 613.4b).
    BaseToughness,
    /// "total power and toughness": the sum of the thing's power and its toughness, one number
    /// of one object ("target creature with total power and toughness 5 or less"; ruling of
    /// 2022-09-09 on Cut Down: "the total power and toughness of a 3/2 creature is 5"). Not
    /// [`Atom::Total`], which sums one stat over the objects selected together.
    PowerAndToughness,
    ManaValue,
    Loyalty,
    Defense,
}

/// A reference to something the context already fixed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Ref {
    /// A typed instruction-result reference, awaiting lexical binding.
    Unresolved(crate::reference::UnresolvedRef),
    /// The object the ability is on (`~`).
    This,
    /// The object that granted a quoted ability, named inside it (`§`, CR 201.5a).
    Granting,
    /// The ability's controller ("you").
    You,
    /// The permanent this is attached to: "enchanted creature" (an Aura, CR 303.4), "equipped
    /// creature" (an Equipment, 301.5), "fortified land" (a Fortification, 301.6).
    AttachedTo(Attachment),
    /// "defending player" (CR 506.2).
    DefendingPlayer,
    /// "the player or planeswalker it's attacking", "the player or planeswalker that creature
    /// is attacking": what that creature attacks (506.3; a battle is not named).
    AttackedBy(Box<Selector>),
    /// "the permanent it becomes": what a permanent spell becomes as it resolves (608.3).
    PermanentItBecomes,
    /// "your team": the player and their teammates (810).
    YourTeam,
    /// "the active player" (102.1).
    ActivePlayer,
    /// "the monarch": the player who has that designation (725.1, 725.3), if any; a static
    /// effect about the monarch does nothing while there is none (725.5).
    Monarch,
    /// "the amassed Army": the Army the amass before put counters on (701.47).
    AmassedArmy,
    /// "all revealed cards not cast this way": every card the effect revealed, the one it
    /// offered among them unless that card was cast. Not "the rest", which leaves the offered
    /// card out whether or not it was cast (Treasure Keeper ruling of 2020-08-07).
    RevealedNotCast,
    /// "the attacking player": the active player during combat (CR 506.2).
    AttackingPlayer,
    /// "its controller", "enchanted creature's controller".
    ControllerOf(Box<Selector>),
    /// "its owner", "that card's owner".
    OwnerOf(Box<Selector>),
    /// "that player or that planeswalker's controller": whichever of these references applies
    /// (after "target player or planeswalker", the player, or the planeswalker's controller, CR
    /// 110.2).
    Either(Vec<Selector>),
    /// Unresolved anaphora ("that creature", "those cards", "that player"); the reference
    /// resolver binds it to a target, an event object, a cost or an earlier step's result.
    Anaphor(Anaphor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Attachment {
    Enchanted,
    Equipped,
    Fortified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Anaphor {
    /// "that creature", "that player".
    That,
    /// "those cards".
    Those,
    /// "they", "their" (a player or set from context: "they control", "of their choice").
    They,
    /// "it" (an object from context: "attached to it").
    It,
    /// "them": objects from context ("shuffle them into your library").
    Them,
    /// "the other", "the others": the remaining one(s) of a pair or group.
    Other { plural: bool },
    /// "the creature", "the cards": the one(s) the context introduced. `plural` matters to the
    /// resolver, as "that"/"those" does.
    The { plural: bool },
    /// "the exiled card", "the sacrificed creatures", "the chosen player": the object(s) an earlier
    /// action or choice in this ability (or its cost) affected, named by the action's participle.
    TheParticiple { what: Verb, plural: bool },
    /// "the copy", "the copies": what the ability's copy instruction made (707.10).
    Copy { plural: bool },
    /// "that copy": the copy the sentence before made.
    ThatCopy,
    /// "one" (of the cards just named: "Put one into your hand and the other into your
    /// graveyard").
    One,
    /// "both creatures": a soulbond pair (702.95).
    BothPaired,
    /// The n-th participant named by the instruction ("the first player", 608.2c).
    Ordinal { n: u32 },
    /// "itself": the subject of its own clause ("Each creature deals damage to itself").
    Itself,
}

/// How a random choice is spelled: "a card at random", "a creature chosen at random".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Random {
    AtRandom,
    ChosenAtRandom,
}

impl ZoneSpec {
    /// One whole zone: "your graveyard" (`owner`), "exile" (no owner).
    pub fn of(zone: Zone, owner: Option<Selector>) -> ZoneSpec {
        ZoneSpec { zones: Coord::one(zone), owner: owner.map(Box::new), slice: None }
    }

    /// The zone, when it names one.
    pub fn zone(&self) -> Option<Zone> {
        match self.zones.items.as_slice() {
            [z] => Some(*z),
            _ => None,
        }
    }

    /// The owner, when this is exactly one whole owned zone of that kind ("your library").
    pub fn owner_of(&self, zone: Zone) -> Option<&Selector> {
        if self.zone() == Some(zone) && self.slice.is_none() { self.owner.as_deref() } else { None }
    }
}

impl Selector {
    pub fn new(quant: Quant, set: ObjectSet) -> Selector {
        Selector { quant, set, chooser: None, random: None }
    }

    /// "you".
    pub fn you() -> Selector {
        Selector::new(Quant::Definite(Ref::You), ObjectSet::of(Domain::Player))
    }
}

impl ObjectSet {
    pub fn of(domain: Domain) -> ObjectSet {
        ObjectSet {
            domain,
            filter: Filter::any_object(),
            controller: None,
            owner: None,
            caster: None,
            zone: None,
            other: false,
        }
    }
}

/// The card types a description says its objects do not have, one list per alternative:
/// "noncreature permanent" denies creature, "creature or land" has two alternatives denying
/// nothing. Card types do not exclude one another (205.2: an object can have several), so only
/// a denial keeps a permanent from being of one: a Wall, an artifact, a land may be a creature.
fn denied_types(f: &Filter) -> Vec<Vec<CardType>> {
    match f {
        Filter::Not(inner) => match inner.as_ref() {
            Filter::Atom(Atom::Type(t)) => vec![vec![*t]],
            Filter::Any(v) => vec![v
                .iter()
                .filter_map(|x| match x {
                    Filter::Atom(Atom::Type(t)) => Some(*t),
                    _ => None,
                })
                .collect()],
            _ => vec![Vec::new()],
        },
        Filter::All(v) => v.iter().map(denied_types).fold(vec![Vec::new()], |acc, alts| {
            acc.iter().flat_map(|a| alts.iter().map(move |b| a.iter().chain(b).copied().collect())).collect()
        }),
        Filter::Any(v) => v.iter().flat_map(denied_types).collect(),
        Filter::Atom(_) => vec![Vec::new()],
    }
}


impl ObjectSet {
    /// Does this description explicitly name a card type in a positive position?
    /// Operation capability checks use this for dependencies that a broad noun such as
    /// "permanent" does not require. Negating a type excludes that dependency.
    pub fn names_type(&self, kind: CardType) -> bool {
        fn names(f: &Filter, kind: CardType, positive: bool) -> bool {
            match f {
                Filter::Atom(Atom::Type(t)) => positive && *t == kind,
                Filter::All(v) | Filter::Any(v) => v.iter().any(|f| names(f, kind, positive)),
                Filter::Not(f) => names(f, kind, !positive),
                _ => false,
            }
        }
        names(&self.filter, kind, true)
            || matches!(&self.domain, Domain::Union(arms) if arms.iter().any(|a| a.names_type(kind)))
    }

    /// Rule-derived sort facts, shared by all grammar role checks. A union retains its
    /// alternatives; a conjunction combines their negative type constraints.
    pub fn possibilities(&self) -> Vec<Possibility> {
        use crate::lexicon::Sort;
        let alternatives: Vec<Possibility> = match &self.domain {
            Domain::Union(arms) => arms.iter().flat_map(ObjectSet::possibilities).collect(),
            domain => domain.atomic_alternatives().expect("atomic domain").iter()
                .map(|domain| Possibility { domain: *domain, excluded_types: Vec::new() }).collect(),
        };
        alternatives.into_iter().flat_map(|p| {
            let excluded = if matches!(p.domain.sort, Sort::Permanent | Sort::Of(_)) {
                denied_types(&self.filter)
            } else { vec![Vec::new()] };
            excluded.into_iter().map(move |more| {
                let mut excluded_types = p.excluded_types.clone();
                excluded_types.extend(more);
                Possibility { domain: p.domain, excluded_types }
            })
        }).collect()
    }
}
