//! Actions: what a player does, as a cost ("Sacrifice a creature:") or as an effect ("Target player
//! sacrifices a creature."). The actor is not part of the action: a cost's actor is the player
//! paying it, an effect's is its subject (corpus measurements: corpus-structure.md §2).
//!
//! Each variant is one primitive with its own rules hooks (§2.2); zone moves keep their surface
//! verb because the rules hook on the cause (sacrifice ≠ destroy, discard ≠ put into a graveyard).

use crate::amount::Amount;
use crate::mana::{ManaCost, ManaSpec};
use crate::selector::{CounterKind, ObjectSet, Selector, Stat};
use crate::token::{KeywordInstance, Pt, TokenSpec};
use crate::vocab::{CardType, Color, Subtype, Supertype};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Action {
    /// "sacrifice a creature", "sacrifice ~" (CR 701.21).
    Sacrifice(Objects),
    /// "discard a card", "discard two cards at random", "discard your hand" (701.9).
    Discard(Objects),
    /// "exile ~ from your graveyard", "exile the top card of your library".
    Exile(Objects),
    /// "return ~ to its owner's hand", "put a card from your hand on top of your library",
    /// "return target creature card from your graveyard to the battlefield", "put one of them
    /// into your hand and the rest on the bottom of your library in a random order": a zone
    /// change (400.7) by an effect, several when "and" joins them.
    /// `from`: where they are moved from, said after the destination for objects that cannot
    /// carry a zone of their own ("put your commander into your hand from the command zone",
    /// "put ~ onto the battlefield from your graveyard"). Only when one move is made and its
    /// objects name no zone themselves.
    Move { verb: MoveVerb, moves: Vec<(Objects, Destination)>, from: Option<crate::ZoneSpec> },
    /// "tap an untapped creature you control", "tap enchanted creature" (701.26).
    Tap(Objects),
    /// "untap ~".
    Untap(Objects),
    /// "reveal ~ from your hand", "reveal a card from your hand".
    Reveal(Objects),
    /// "exert ~" (701.43).
    Exert(Objects),
    /// "pay 2 life" (119.4).
    PayLife(Amount),
    /// "pay {2}{U}".
    PayMana(ManaCost),
    /// "pay {E}{E}", "pay eight {E}" (107.14, 122).
    PayEnergy(Amount),
    /// "pay its mana cost", "pay enchanted creature's mana cost", "pay its mana cost reduced by
    /// {2}": a cost named rather than written in symbols (118.1, 202.1).
    PayCost(crate::cost::CostExpr),
    /// "mill a card", "mill two cards" (701.17).
    Mill(Amount),
    /// "put a -1/-1 counter on ~", "put a +1/+1 counter on that creature and a plan counter on
    /// ~" (122.1): one or more placements.
    PutCounters(Vec<Placement>),
    /// "remove a +1/+1 counter from ~", "remove any number of storage counters from ~", "remove
    /// all +1/+1 counters from ~", "remove a counter from a creature you control"; `among`: "remove
    /// two +1/+1 counters from among creatures you control" (split as the payer likes).
    /// `from: None`: the subject's own counters, "target player loses all rad counters"
    /// (a player's counters are lost, 810.10b).
    RemoveCounters { count: CounterCount, kind: Option<CounterKind>, from: Option<Box<Selector>>, among: bool },
    /// "unattach §", "unattach an Equipment from ~" (701.3d).
    Unattach { what: Objects, from: Option<Box<Selector>> },
    /// "waterbend {3}": pay the cost, tapping artifacts or creatures for its generic part (701.67).
    Waterbend(ManaCost),
    /// A keyword action without arguments: "investigate", "proliferate", "forage" (701).
    Keyword(KeywordAction),
    /// A keyword action with a number: "adapt 2", "bolster 1", "collect evidence 4" (701).
    Numbered(NumberedAction, Amount),
    /// A keyword action on objects: "transform ~", "goad target creature" (701).
    OnObjects(ObjectAction, Objects),
    /// "attach it to target creature you control" (701.3).
    Attach { what: Objects, to: Box<Selector> },
    /// "pair this creature with another unpaired creature you control": soulbond's pairing
    /// (702.95a-b).
    Pair { what: Objects, with: Box<Selector> },
    /// "fights target creature you don't control" (701.14).
    Fight(Objects),
    /// "those creatures fight each other" (701.14).
    FightEachOther,
    /// "amass Zombies 2" (701.47).
    Amass { subtype: Subtype, amount: Amount },
    /// "clash with an opponent" (701.30).
    Clash(Box<Selector>),
    /// "roll a d20", "roll two d6" (706); `modifier`: what the instruction adds to or subtracts
    /// from the natural result, "roll a d20 and add the number of cards in your hand" (706.2);
    /// `kept`: the rolls that are not ignored, "roll two d20 and ignore the lower roll" (706.6).
    RollDie { count: Amount, sides: u32, modifier: Option<RollModifier>, kept: Option<KeptRolls> },
    /// "cast that card", "play those cards this turn", "cast it without paying its mana cost"
    /// (601, 305.1); `play`: the verb is "play" (lands too); `free`: "without paying its mana
    /// cost" (118.9).
    Cast { play: bool, what: Objects, free: bool },
    /// "play an additional land", "play two additional lands" (305.2).
    PlayAdditionalLands(Amount),
    /// "draw a card", "draw two cards" (121).
    Draw(Amount),
    /// "draw up to three cards": the player chooses how many to draw, at most that; an effect
    /// that offers a choice to draw multiple cards (121.2b).
    DrawUpTo(Amount),
    /// "create a 1/1 white Spirit creature token with flying", "create X 1/1 Halfling tokens and X
    /// Food tokens" (701.7, 111); "create a Food token or a Treasure token": with "or", the
    /// creator chooses one (608.2d).
    Create(Coord<TokenSpec>),
    /// "deals 3 damage to any target" (120), "deals 1 damage to any target and 1 damage to you";
    /// the subject is the source.
    DealDamage(Vec<Hit>),
    /// "deals 3 damage divided as you choose among one, two, or three targets" (601.2d).
    DealDivided { amount: Amount, among: Objects, division: DamageDivision },
    /// "becomes 10" with a life total as the subject: the player gains or loses the difference
    /// (119.5).
    LifeTotalBecomes(Amount),
    /// "double your life total", "double target player's life total": the player gains or loses
    /// the life that makes their life total twice what it is (701.10d). Apart from
    /// [`Action::ScaleStat`], a continuous effect on a creature (613.4c); the rules define no
    /// tripling of a life total (701.11 triples power and toughness only).
    DoubleLife { of: Box<Selector> },
    /// "separates those cards into two piles" (an opponent divides; the cards' owner usually
    /// picks a pile).
    SeparateIntoPiles { what: Objects, piles: crate::procedure::Piles },
    /// "chooses one of those piles" (700.3).
    ChoosePile(crate::procedure::PileRef),
    /// "put one pile into your hand and the other into your graveyard", "put that pile into
    /// your hand": the objects of a pile are moved (700.3).
    MovePile(Vec<(crate::procedure::PileRef, Destination)>),
    /// "gets an emblem with \"…\"" (114.2).
    GetEmblem(Vec<Granted>),
    /// "remove it from combat" (506.4).
    RemoveFromCombat(Objects),
    /// "move a +1/+1 counter from ~ onto target creature", "move all counters from …", "move any
    /// number of +1/+1 counters from …" (122.5); no kind: counters of any kind.
    MoveCounters { count: CounterCount, kind: Option<CounterKind>, from: Box<Selector>, onto: Box<Selector> },
    /// "flip ~" (a flip card, 710).
    Flip(Objects),
    /// "turn ~ face down", "turn the exiled card face up" (708, 110.5).
    TurnFace { what: Objects, up: bool },
    /// "double target creature's power and toughness", "double the toughness of each creature you
    /// control", "triple target creature's power": the creature gets +X/+0, +0/+X or +X/+Y, where
    /// X is its power as the effect resolves (twice that for tripling), a continuous effect that
    /// modifies without setting (701.10b, 701.11b, 613.4c).
    ScaleStat { of: Box<Selector>, stats: Vec<Stat>, factor: Factor },
    /// "pay any amount of mana", "pay any amount of {E}", "pay any amount of life" (107.1c: the
    /// payer chooses the number); `at_least_one`: "pay one or more {E}".
    PayAnyAmount { what: AnyAmountOf, at_least_one: bool },
    /// "switch target creature's power and toughness", "switch the power and toughness of each
    /// of up to two target creatures" (`possessive`: the first spelling) (613.4d).
    SwitchPt { of: Box<Selector>, possessive: bool },
    /// "distribute three +1/+1 counters among one, two, or three target creatures": each gets
    /// at least one (115.7f, 601.2d, 608.2d).
    Distribute { counters: Counters, among: Objects },
    /// "destroy target creature" (701.8).
    Destroy(Objects),
    /// "gain 3 life" (119.3).
    GainLife(Amount),
    /// "lose 2 life".
    LoseLife(Amount),
    /// "scry 2" (701.22).
    Scry(Amount),
    /// "surveil 1" (701.25).
    Surveil(Amount),
    /// "doesn't untap during its controller's next untap step" (502.3); `next`: only the next one
    /// (otherwise the duration says how long).
    DoesntUntap { whose: UntapWhose, next: bool },
    /// "after this phase, there is an additional combat phase [followed by an additional main
    /// phase]", "after this main phase, there are two additional combat phases", "there is an
    /// additional beginning phase after this phase": phases added directly after the one named
    /// (500.8). Nothing does it: the clause has no subject.
    AdditionalPhase { after: AfterPhase, kind: crate::turn::Phase, count: Amount, then_main: bool },
    /// "don't lose this mana as steps and phases end", "don't lose unspent red mana as steps
    /// end" (106.4); `phases`: "and phases".
    KeepMana { mana: crate::mana::Pooled, phases: bool },
    /// "lose all unspent mana", "loses all unspent red mana" (106.4): the pool is emptied of it.
    LoseUnspentMana(Option<crate::vocab::Color>),
    /// "it's still a land", "they're 2/2 Cyberman artifact creatures": what something is, as a
    /// continuous effect of the ability (the subject's verb is "'s" or "they're").
    Is(Becoming),
    /// "has base power and toughness 4/4", "~'s base power becomes 3", "you may have ~'s base power
    /// and toughness become equal to its power and toughness": sets power and/or toughness to a
    /// value, a layer 7b effect (613.4b; the duration is a modifier). `of` is the object whose
    /// stats are named when the text does not make it the subject ("the base power and toughness
    /// of each creature become 0/2"); None: the subject's. `caused`: said of a subject, a player
    /// who "has" it so ("you may have ~'s base power become 3"); otherwise the named stats are
    /// the whole clause ("~'s base power becomes 3").
    SetStat { of: Option<Box<Selector>>, caused: bool, set: Setting },
    /// "exile cards from the top of your library until you exile a nonland card", "reveals cards
    /// from the top of their library until they reveal a creature card": the actor's own library,
    /// one card at a time, until a card that matches (the cards so moved are "exiled/revealed this
    /// way").
    Until { verb: UntilVerb, until: Box<Selector> },
    /// "loses your choice of flying, first strike, or trample": the controller picks one (613.1f;
    /// plain losses are `StaticAct::Loses`).
    LosesChoice(Vec<Granted>),
    /// "gains your choice of banding, flying, first strike, or trample".
    GainsChoice(Vec<Granted>),
    /// "draw an additional card": a draw on top of the normal one (121.2).
    DrawAdditional(Amount),
    /// "put its counters on ~", "put those counters on up to one target creature": the counters
    /// an earlier object had (last known information, 608.2h).
    PutCountersOf { whose: CountersOf, on: Box<Selector> },
    /// "put your choice of a reach counter, a vigilance counter, or a trample counter on it":
    /// the player picks one of these as the instruction is carried out (608.2d).
    PutChoiceOfCounters { choices: Vec<Counters>, on: Box<Selector> },
    /// "put a counter of that kind on it", "give that permanent or player another counter of
    /// that kind" (`give`): one counter of the kind the text before named ("for each kind of
    /// counter on …", "choose a counter on …").
    PutCounterOfThatKind { extra: Option<Extra>, on: Box<Selector>, give: bool },
    /// "choose a counter on target permanent", "choose a kind of counter on a creature you
    /// control": a kind of counter that thing has.
    ChooseCounterOn(Box<Selector>),
    /// "you control target player during that player's next turn" (723).
    ControlPlayerNextTurn(Box<Selector>),
    /// "put the same number and kind of counters on ~": as many of each kind as the event put.
    PutSameCounters(Box<Selector>),
    /// "look at and play those cards", "look at and cast that card": the player may look at
    /// the cards and may play (`play`) or cast them.
    LookAtAndCast { play: bool, what: Objects },
    /// "secretly choose an opponent", "each player secretly votes for a player": the choice
    /// or vote is hidden until the text reveals it.
    Secretly(Box<Action>),
    /// "put a charge counter on ~ or remove one from it": the player puts the counter or
    /// removes one of that kind from the same thing.
    OrRemoveOne(Box<Action>),
    /// "pay {1}{G} any number of times": the payment made as often as the player likes; "that
    /// many" afterwards is how often.
    AnyNumberOfTimes(Box<Action>),
    /// "exchange control of target land you control and target land an opponent controls" (701.12b).
    ExchangeControl(Objects),
    /// "exchange life totals with target opponent", "two target players exchange life totals"
    /// (701.12c); `with`: the other player, when the subject is one.
    ExchangeLifeTotals { with: Option<Box<Selector>> },
    /// "exchange your life total with ~'s toughness", "exchange target opponent's life total with
    /// ~'s toughness": two numerical values exchanged, each becoming the other's previous value;
    /// the player gains or loses the life that makes the life total equal the stat (701.12g).
    /// `player`: whose life total; `of`: the permanent whose `stat` (power or toughness) it is.
    ExchangeLifeWithStat { player: Box<Selector>, of: Box<Selector>, stat: Stat },
    /// "change the target of target spell with a single target" (115.7a).
    ChangeTarget(Objects),
    /// "reselect which player or permanent target attacking creature is attacking", "reselect
    /// which player ~ is attacking" (`player_only`): the attacker's defender is chosen anew
    /// (506.4, 508.1b).
    ReselectAttacked { attacker: Box<Selector>, player_only: bool },
    /// "change a target of target spell or ability to ~": one of its targets becomes that
    /// object (115.7).
    ChangeATargetTo { of: Box<Selector>, to: Box<Selector> },
    /// "votes for grace or condemnation", "votes for an artifact, creature, or enchantment card in
    /// your graveyard" (701.38a).
    Vote(VoteFor),
    /// "faces a villainous choice — You draw a card, or that player discards a card" (701.55a):
    /// the player chooses one of the two options, then all actions of the chosen one are
    /// performed.
    FaceVillainousChoice(Box<[crate::effect::Body; 2]>),
    /// "tap or untap target permanent".
    TapOrUntap(Objects),
    /// "shuffle it into its owner's library", "shuffles their graveyard into their library"
    /// (701.24).
    /// `from`: "shuffle ~ into your library from your graveyard", as for [`Action::Move`].
    ShuffleInto { what: Objects, whose: Whose, from: Option<crate::ZoneSpec> },
    /// "double the number of +1/+1 counters on it", "double the number of each kind of counter on
    /// it" (`kind` None: every kind it has) (701.10e).
    DoubleCounters { kind: Option<CounterKind>, on: Box<Selector> },
    /// "copy that spell", "copy target instant or sorcery spell" (707.10).
    Copy(Objects),
    /// "counter target spell" (701.6).
    Counter(Objects),
    /// "repeat this process", "repeat this process once", "repeat this process any number of
    /// times", "repeat this process until no one pays life".
    Repeat(crate::procedure::Repeats),
    /// "shuffle", "shuffle your library", "shuffles their library" (701.24).
    Shuffle(crate::procedure::Shuffled),
    /// "add {G}", "add one mana of any color" (106.1).
    AddMana(ManaSpec),
    /// "adds an additional {G}": mana on top of what a mana ability just produced (a triggered
    /// mana ability, 605.1b, 106.12a).
    AddAdditionalMana(ManaSpec),
    /// "gets +2/+2", "get -1/-1" (613.4c; the duration is a modifier of the act).
    Gets { power: Delta, toughness: Delta },
    /// "gets +1/-1 or -1/+1": one of these changes, as the player chooses.
    GetsEither(Vec<(Delta, Delta)>),
    /// "gains flying", "gain haste and lifelink", "gains \"{T}: Add {G}.\"" (613.1f).
    Gains(Vec<Granted>),
    /// "can't block", "can't attack or block", "can't be blocked by creatures with flying",
    /// "can't be countered", "can't cast noncreature spells": a prohibition (101.2) on events
    /// with this subject, any of the lexicon's.
    Cant(Vec<crate::statics::Forbidden>),
    /// "becomes a 3/3 Elemental creature with haste", "become artifact creatures", "becomes blue",
    /// "becomes the color of your choice", "becomes prepared" (613.1d–f, 613.4b; designations).
    Becomes(Becoming),
    /// "prevent the next 3 damage that would be dealt to any target", "prevent all combat damage
    /// that would be dealt by target creature", "prevent that damage" (615.1a).
    Prevent(Prevention),
    /// "choose new targets for the copy", "choose a new target for that spell" (`one`) (707.10c,
    /// 115.7).
    ChooseNewTargets { for_: Objects, one: bool },
    /// "search your library for a basic land card" (701.23): `zone` is where, `what` the card(s)
    /// found (a selector over that zone's cards).
    /// "search your library and/or graveyard for …", "search its controller's graveyard, hand,
    /// and library for …": several zones of one owner.
    Search { zone: crate::selector::ZoneSpec, what: Objects },
    /// "look at the top three cards of your library", "look at target player's hand".
    LookAt(Objects),
    /// "meld them into Hanweir, the Writhing Township": the two exiled cards return as the one
    /// melded permanent of that name (701.42a).
    Meld(crate::selector::Name),
    /// "Change the text of target spell or permanent by replacing all instances of one color
    /// word with another [or one basic land type with another]" (612.1, 612.2): the player
    /// picks one of the listed kinds of word; `to` names the new word where the text does
    /// ("with Vampire"), else the player picks it too.
    ChangeText { what: Box<Selector>, words: Vec<TextWord>, to: Option<Subtype> },
    /// "regenerate ~" (701.19).
    Regenerate(Objects),
    /// "choose target creature", "choose a color", "choose a nonland card name".
    Choose(Choice),
    /// "gain control of target creature" (613.1b; the duration is a modifier).
    GainControl(Objects),
    /// "you get {E}{E}", "you get an experience counter", "get a poison counter" (122.1).
    GetCounters { count: Amount, kind: PlayerCounter },
    /// "create a token that's a copy of target creature" (707.2); "two tokens", "a tapped token".
    /// `attacking`: "a tapped and attacking token" (508.4). `defender`: what it is attacking,
    /// where the effect says ("that's tapped and attacking that player or a planeswalker they
    /// control"); none, its controller chooses (508.4).
    CreateCopy { count: Amount, tapped: bool, attacking: bool, of: Box<Selector>,
        #[serde(skip_serializing_if = "Option::is_none")]
        defender: Option<Box<Objects>> },
    /// "have it deal 2 damage to target creature": the actor has another object or player act.
    Have { who: Box<Selector>, action: Box<Action> },
    /// A static verb phrase: "has flying", "enters tapped", "costs {1} less to cast"
    /// ([`crate::statics::StaticAct`]).
    Static(Box<crate::statics::StaticAct>),
}

/// A modifier a roll instruction includes (706.2): the result is the natural result plus or
/// minus this.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum RollModifier {
    /// "and add the number of cards in your hand"
    Add(Amount),
    /// "and subtract the number of cards in your hand"
    Subtract(Amount),
}

/// The rolls of an instruction that count; the others are ignored, as if they never happened
/// (706.6): "ignore the lower roll" of two keeps the highest one, "ignore all but the highest
/// roll" too.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct KeptRolls {
    /// The highest results are kept (else the lowest).
    pub highest: bool,
    pub count: Amount,
}

/// What a non-modal "choose" picks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Choice {
    Objects(Objects),
    /// "choose a color", "choose two basic land types", "choose a color other than red",
    /// "choose a creature type at random": one or more values of a class (608.2d); `except`
    /// removes values from what may be chosen. A restricted class is always written this way and
    /// never as a list of its members (that is [`Choice::Among`]).
    Value { kind: ValueKind, count: Amount, except: Vec<Exclusion>, random: bool },
    /// "choose a number", "choose a number between 0 and 13": `range` is the inclusive, ordered
    /// bounds; without it any number may be chosen.
    Number { range: Option<(Amount, Amount)> },
    /// "choose Khans or Dragons", "choose odd or even", "choose black or red at random": one of
    /// the options listed (an explicit list, not a class).
    Among { options: Options, random: bool },
    /// "choose a color and an opponent": several choices made together.
    All(Vec<Choice>),
    /// "choose both instead", "choose one or more instead": how many modes (700.2).
    Modes(crate::effect::Modes),
}

/// A class of values a choice draws from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ValueKind {
    /// "a color" (105.1).
    Color,
    /// "a creature type" (205.3m).
    CreatureType,
    /// "a card type" (205.2a).
    CardType,
    /// "a basic land type" (305.6).
    BasicLandType,
    /// "a land type".
    LandType,
    /// "a planeswalker type".
    PlaneswalkerType,
    /// "a card name", "a nonland card name" (201.3): a name such cards have.
    CardName(ObjectSet),
    /// "a color of a card in your graveyard", "a color of a permanent you control": one of
    /// the colors an object of the set is (105.2), as the mana [`crate::mana::ManaTypes::ColorsAmong`]
    /// is; colorless is no color (105.1), and an empty or colorless set leaves nothing to choose.
    ColorAmong(ObjectSet),
}

/// A value a choice may not pick: "other than red", "other than creature or land", "other than
/// Wall", "other than a basic land card name".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Exclusion {
    Color(Color),
    CardType(CardType),
    Subtype(Subtype),
    CardName(ObjectSet),
}

/// The explicit options of a choice.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Options {
    /// Words with no rules meaning (607.2f), such as an anchor word (614.12c): "Khans or
    /// Dragons", "odd or even", "friend or foe", "left or right".
    Words(Vec<String>),
    /// "black or red".
    Colors(Vec<Color>),
    /// "flying or indestructible", "flying, first strike, trample, or rampage 3".
    Keywords(Vec<KeywordInstance>),
}

/// Counters a player gets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum PlayerCounter {
    /// {E} (107.14), printed as symbols.
    Energy,
    /// "an experience counter", "a poison counter", "a rad counter".
    Named(CounterKind),
}

/// The verb of a zone move: "return" (the object was there before, or is going home) or
/// "put"; the rules treat them alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum MoveVerb {
    Return,
    Put,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Destination {
    /// "to its owner's hand", "into your hand".
    Hand(Whose),
    /// "to the battlefield", "onto the battlefield tapped under your control".
    Battlefield(Vec<Entry>),
    /// "into your graveyard", "into its owner's graveyard".
    Graveyard(Whose),
    /// "to the command zone", "into the command zone" (CR 903.9: a commander's owner may put it
    /// there).
    CommandZone,
    /// "on top of your library", "on the bottom of its owner's library in a random order",
    /// "into its owner's library third from the top", "on their choice of the top or bottom of
    /// their library", "back in any order", "on top" (CR 401.4, 401.7).
    Library(crate::zone_move::LibraryPlace),
}

/// Whose zone a move goes to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Whose {
    /// "your".
    You,
    /// "its owner's" after one object, "their owner's" after several.
    Owner,
    /// "their owners' hands": several objects that may have different owners.
    Owners,
    /// "their": the subject's.
    Their,
    /// "that player's".
    Player(Box<Selector>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum LibraryPosition {
    Top,
    Bottom,
    /// "second from the top", "third from the top".
    FromTop(u32),
}

/// "in any order" (the owner chooses) or "in a random order".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Order {
    Any,
    Random,
}

/// How a permanent enters, in printed order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Entry {
    Tapped,
    Attacking,
    /// The defender attacked by a participant named earlier (702.49c).
    AttackingSameAs(Box<Selector>),
    /// "tapped and attacking".
    TappedAndAttacking,
    /// "tapped and transformed".
    TappedAndTransformed,
    /// "under your control", "under its owner's control".
    Under(Whose),
    /// "under the control of that card's owner".
    UnderControlOf(Box<Selector>),
    /// "with a finality counter on it", "with two +1/+1 counters on it".
    WithCounters(CountersOn),
    Transformed,
    FaceDown,
    /// "attached to that creature".
    AttachedTo(Box<Selector>),
}

/// A kind of word a text-changing effect replaces (612.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TextWord {
    Color,
    BasicLandType,
    CreatureType,
}

/// A signed change to power or toughness: "+2", "-1", "+X", "+0".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Delta {
    pub minus: bool,
    pub amount: Amount,
}

/// An ability an effect grants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Granted {
    Keyword(Box<KeywordInstance>),
    /// A quoted ability: "gains \"{T}: Add {G}.\"" (113.2c); `~` inside is the object that has it,
    /// `§` the object that granted it (201.5a).
    Ability(Box<crate::ability::Ability>),
    /// A quoted ability the ability grammar does not read yet, as its normalized text (the
    /// whole-card gate counts it as a failure).
    Quoted(String),
    /// "all activated abilities of all land cards in all graveyards": abilities taken from other
    /// objects at runtime, recomputed from the selector (613.1f).
    AbilitiesOf { kind: AbilityKind, from: Box<Selector> },
    /// "gains all creature types", "has all land types": the object has every member of a class
    /// of subtypes (205.3m; a type-changing effect, 613.1d, said in a list with abilities).
    Every(Every),
    /// "that ability": the keyword an earlier instruction chose ("choose flying or
    /// indestructible. ~ gains that ability").
    ThatAbility,
}

/// A whole class of the rules' closed vocabulary, as one value: "all creature types" (205.3m,
/// 702.73a: "every creature type"), "every land type", "every basic land type" (305.6),
/// "all other card types" (205.2). The members are `Subtype::all()` of that kind, the basic
/// land types, or `CardType::ALL`. (Colors are a finite list, "all colors" being the five of
/// 105.1, and stay an explicit list.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Every {
    CardTypes,
    CreatureTypes,
    LandTypes,
    BasicLandTypes,
    /// "every nonbasic land type" (205.3i: the land types besides the basic ones of 305.6).
    NonbasicLandTypes,
}

/// A type an earlier sentence would have replaced and this one keeps: "a land", "a Cave land"
/// (205.1b).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Kept {
    pub subtypes: Vec<Subtype>,
    pub card_type: CardType,
}

/// Which abilities [`Granted::AbilitiesOf`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum AbilityKind {
    Activated,
    Triggered,
    /// "activated and triggered abilities".
    ActivatedAndTriggered,
    /// "loyalty abilities" (606.3).
    Loyalty,
}

/// One thing, or several joined by a conjunction: all of them ("and"), one of them as the actor
/// picks ("or"), or one or more ("and/or"). The one coordination of the grammar, whatever is
/// joined: objects, players, zones, actions. A single item has [`Conj::And`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Coord<T> {
    pub items: Vec<T>,
    pub conj: Conj,
}

/// The objects of an action: one selector, several joined by "and" ("sacrifice two lands and
/// ~"), alternatives joined by "or" ("sacrifice another creature or an enchantment": the actor
/// picks one), or "and/or" ("a creature card and/or a land card": one or both).
pub type Objects = Coord<Selector>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Conj {
    And,
    Or,
    AndOr,
}

impl<T> Coord<T> {
    pub fn one(item: T) -> Coord<T> {
        Coord { items: vec![item], conj: Conj::And }
    }
}

/// How many counters an action removes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CounterCount {
    Exactly(Amount),
    /// "any number of": the payer chooses (X is defined by it, 107.3f).
    AnyNumber,
    /// "one or more": the payer chooses, at least one.
    OneOrMore,
    /// "all".
    All,
    /// "up to three": the remover chooses how many, at most that.
    UpTo(Amount),
}

/// Keyword actions without arguments (701), as their verbs read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum KeywordAction {
    /// 701.16.
    Investigate,
    /// 701.34.
    Proliferate,
    /// 701.36.
    Populate,
    /// 701.48.
    Learn,
    /// 701.70.
    Recruit,
    /// "venture into the dungeon" (701.49).
    Venture,
    /// "time travel" (701.56).
    TimeTravel,
    /// "manifest dread" (701.62).
    ManifestDread,
    /// 701.61.
    Forage,
    /// "it connives" (701.50): the subject is the conniving permanent.
    Connive,
    /// "it explores" (701.44).
    Explore,
    /// "you become the monarch" (725).
    BecomeMonarch,
    /// "the Ring tempts you" (701.54); the clause has no other subject.
    RingTemptsYou,
    /// "flip a coin" (705).
    FlipCoin,
    /// "take an extra turn after this one" (500.7).
    ExtraTurn,
    /// "end the turn" (724).
    EndTheTurn,
    /// "then those votes are revealed": the secret votes are shown (701.38).
    RevealVotes,
    /// "then those choices are revealed": the secret choices are shown.
    RevealChoices,
    /// "flip a coin until you lose a flip" (705).
    FlipUntilLose,
    /// "planeswalk" (701.31).
    Planeswalk,
    /// "chaos ensues" (311.7).
    ChaosEnsues,
    /// "remove those counters": the counters the condition before named ("if there are three
    /// or more landmark counters on it, remove those counters").
    RemoveThoseCounters,
    /// "Each opponent attacking that player does the same.": what the sentence before had its
    /// player do.
    DoTheSame,
    /// "you take the initiative" (726.3).
    TakeInitiative,
    /// "~ phases out" (702.26b).
    PhaseOut,
    /// "it becomes day" (731.1): the game gains the day designation; "it" is the game, no
    /// object. (Day and night are the game's designations, so there is no subject to name.)
    BecomeDay,
    /// "it becomes night" (731.1).
    BecomeNight,
    /// "your speed increases by 1" (702.179d): the rules' own ability, which names no subject.
    IncreaseSpeed,
    /// "you win the game" (104.2b).
    WinGame,
    /// "that player loses the game" (104.3e).
    LoseGame,
}

/// Keyword actions with a number N (701).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum NumberedAction {
    /// 701.37.
    Monstrosity,
    /// 701.46.
    Adapt,
    /// 701.39.
    Bolster,
    /// 701.41.
    Support,
    /// 701.53.
    Incubate,
    /// 701.57.
    Discover,
    /// 701.66.
    Earthbend,
    /// "it endures 2" (701.63).
    Endure,
    /// "it connives 2", "connives X": draw that many, discard that many (701.50e).
    Connive,
    /// 701.29.
    Fateseal,
    /// 701.59.
    CollectEvidence,
    /// "blight 2": put that many -1/-1 counters on a creature you control (701.68).
    Blight,
    /// "empower Jace 3" (701.71).
    EmpowerJace,
}

/// Keyword actions on objects (701).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ObjectAction {
    /// 701.27.
    Transform,
    /// 701.28.
    Convert,
    /// 701.15.
    Goad,
    /// 701.35.
    Detain,
    /// 701.60.
    Suspect,
    /// 701.64.
    Harness,
    /// 701.65.
    Airbend,
    /// "manifest the top card of your library" (701.40).
    Manifest,
    /// 701.58.
    Cloak,
    /// "behold a Dragon": reveal such a card from your hand or choose such a permanent you
    /// control (701.4a).
    Behold,
}

/// What a permanent or spell becomes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Becoming {
    /// "a 0/0 Elemental creature with haste", "an artifact creature", "an Island", "artifact
    /// creatures" (after a plural subject).
    Is(Characteristics),
    /// "blue", "colorless" (an empty list), "black and green"; optionally "in addition
    /// to its other colors" (105.3). Otherwise the colors replace all previous colors.
    Colors { colors: Vec<Color>, added: bool },
    /// "your choice of a 3/3 creature, a 2/2 creature with flying, or a 1/6 creature with
    /// defender": one of these, as the player chooses.
    ChoiceOf(Vec<Becoming>),
    /// "the color of your choice".
    ColorOfChoice,
    /// "the color or colors of your choice".
    ColorsOfChoice,
    /// "the creature type of your choice".
    CreatureTypeOfChoice,
    /// "the basic land type of your choice" (305.7).
    BasicLandTypeOfChoice,
    /// "the creature type of your choice in addition to its other types" (`land`: "the basic
    /// land type of your choice in addition to its other types"): the chosen type is added.
    TypeOfChoiceAlso { land: bool },
    /// "blocked" (509.1h).
    Blocked,
    /// "prepared" (722.3a).
    Prepared,
    /// Remove the prepared designation (722.3b).
    Unprepared,
    /// "This Class's level becomes N" (716.2a): what a class level bar's activated ability
    /// does, not read from card text.
    Level(u32),
    /// "is the chosen type", "are the chosen type in addition to their other types".
    ChosenType(Keeps),
    /// "is the chosen color".
    ChosenColor,
    /// "is goaded" (701.15b).
    Goaded,
    /// "plotted" (702.170).
    Plotted,
    /// "foretold" (702.143).
    Foretold,
    /// "monstrous" (701.37b).
    Solved,
    Monstrous,
    /// "renowned" (702.112b).
    Renowned,
    /// "saddled" (702.171b): until the end of the turn or it leaves the battlefield.
    Saddled,
    /// "4/3" (an exception to a copy: its power and toughness, 707.9b).
    Pt(Pt, Pt),
    /// "a copy of target creature", "copies of that creature" (707.2, 613.2: layer 1).
    CopyOf(Box<Selector>),
    /// "still a land", "still lands": keeps a type an earlier sentence would have replaced
    /// (205.1b).
    Still(Kept),
}

/// A description of characteristics, in printed order: "a 3/3 blue Elemental artifact creature
/// with flying that's still a land".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Default)]
pub struct Characteristics {
    pub pt: Option<(Pt, Pt)>,
    /// None when the text names no color; `Some(vec![])` is "colorless".
    pub colors: Option<Vec<Color>>,
    /// "legendary", "legendary and snow" (205.4).
    pub supertypes: Vec<Supertype>,
    pub subtypes: Vec<Subtype>,
    pub types: Vec<CardType>,
    /// "named Fenric": the object's name (201.1).
    pub name: Option<String>,
    /// "with haste", "with all creature types", "with \"{T}: Add {G}.\"" (a bare "every
    /// creature type" is `Granted::Every` here with nothing else named).
    pub abilities: Vec<Granted>,
    /// "with base power and toughness 3/3", "with power and toughness each equal to its mana
    /// value" (613.4b).
    pub base_pt: Option<Setting>,
    pub keeps: Keeps,
}

/// "base power and toughness 3/3", "base power 0", "base power and toughness each equal to its
/// mana value": which of power and toughness are set (a list, in printed order) and to what.
///
/// 208.4a: "Effects that set a creature's power and/or toughness to specific values may refer to
/// base power and/or toughness." Setting a stat is setting its base value (613.4b, layer 7b), so
/// "base" is the wording of the action, not another stat: a stat set without the word means the
/// same. (Reading a creature's base power, 208.4b, is a different matter and belongs to the
/// selector side.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Setting {
    pub stats: Vec<Stat>,
    pub to: SetTo,
}

/// "double" (701.10) or "triple" (701.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Factor {
    Double,
    Triple,
}

/// The value a stat is set to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SetTo {
    /// Each stat to the same amount: "base power 0", "base power and toughness each equal to its
    /// mana value", "base toughness becomes equal to 1 plus the power of target creature".
    Each(Amount),
    /// Two stats, power then toughness, to a printed profile: "base power and toughness 3/2".
    Profile(Pt, Pt),
    /// Each stat to the same characteristic of another object: "base power and toughness become
    /// equal to ~'s power and toughness".
    Matching(Box<Selector>),
}

/// What the new characteristics leave of the old ones (205.1b).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Default)]
pub enum Keeps {
    /// Only what the effect does not set.
    #[default]
    Nothing,
    /// "in addition to its other types".
    OtherTypes,
    /// "in addition to its other creature types": the new subtypes join the creature types.
    OtherCreatureTypes,
    /// "in addition to its other land types".
    OtherLandTypes,
    /// "in addition to its other colors and types".
    OtherColorsAndTypes,
    /// "in addition to its other colors".
    OtherColors,
    /// "that's still a land": the old type is kept.
    StillA(Kept),
}

/// The damage a prevention effect stops.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Prevention {
    pub amount: Prevented,
    /// "combat damage", "noncombat damage", or any damage.
    pub kind: crate::event::DamageKind,
    /// "dealt to …": objects, players or bare sets ("to you and creatures you control").
    pub to: Option<crate::statics::Party>,
    /// "dealt by …": the source restriction.
    pub by: Option<crate::statics::Party>,
    /// "to and dealt by …" names either direction, rather than their intersection.
    #[serde(skip_serializing_if = "prevention_conjunction")]
    pub to_or_by: bool,
    /// Where "this turn" stands inside the phrase ("that would be dealt this turn to …"); at the
    /// end of a "prevent" sentence it is the act's duration instead.
    pub this_turn: Option<crate::statics::TurnAt>,
}

fn prevention_conjunction(either: &bool) -> bool { !either }

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Prevented {
    /// "the next 3 damage" (a shield, 615.7).
    Next(Amount),
    /// "all damage".
    All,
    /// "that damage": damage an earlier part of the text names.
    That,
    /// "1 of that damage", "X of that damage": part of the damage an earlier part names.
    OfThat(Amount),
    /// "all but 1 of that damage".
    AllButOfThat(Amount),
    /// "half that damage, rounded down" (`round_up`: "rounded up").
    HalfThat { round_up: bool },
}

/// Whose untap step a "doesn't untap" names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum UntapWhose {
    /// "its controller's", "their controller's" (after a plural subject).
    Controller,
    /// "their controllers' … untap steps".
    Controllers,
    /// "your".
    Yours,
    /// "that player's".
    ThatPlayer,
}

/// The phase an [`Action::AdditionalPhase`] adds phases after: the one the effect happens in,
/// as the text calls it (500.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum AfterPhase {
    /// "this phase".
    This,
    /// "this main phase".
    ThisMain,
    /// "this combat phase".
    ThisCombat,
}

/// The verb of an [`Action::Until`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum UntilVerb {
    /// 701.20.
    Reveal,
    /// 701.13.
    Exile,
}

/// Counters put on one object or set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Placement {
    /// "two +1/+1 counters and a reach counter".
    pub counters: Vec<Counters>,
    pub on: Box<Selector>,
}

/// What "pay any amount of" pays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum AnyAmountOf {
    Mana,
    Energy,
    Life,
}

/// Some counters of one kind: "a +1/+1 counter", "two additional +1/+1 counters", "another
/// +1/+1 counter", "up to X +1/+0 counters".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Counters {
    pub count: Amount,
    /// "up to X": the one who puts them chooses how many, at most `count`.
    pub up_to: bool,
    pub extra: Option<Extra>,
    pub kind: CounterKind,
}

impl Counters {
    pub fn new(count: Amount, kind: CounterKind) -> Counters {
        Counters { count, up_to: false, extra: None, kind }
    }
}

/// Counters a thing gets as it moves or enters: "with two +1/+1 counters on it", "with a
/// finality counter on it".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CountersOn {
    pub counters: Counters,
    pub on: On,
}

/// The pronoun after "counters on": the object itself ("it"; "him", "her" read as "it").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum On {
    It,
    Them,
}

/// "an additional +1/+1 counter", "another +1/+1 counter": on top of counters already there or
/// already coming (an arrival's own counters, 614.1c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Extra {
    Additional,
    Another,
}

/// Damage to one recipient (or set) in a "deals" action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Hit {
    pub amount: Amount,
    pub to: Objects,
    /// The amount follows the recipients: "damage to target player equal to ...". Those
    /// recipients have been named when its references are evaluated (608.2h).
    pub amount_after_targets: bool,
    /// A condition on this damage component, independently of the other hits (608.2c).
    pub condition: Option<Box<crate::condition::Condition>>,
}

/// Whose counters "put … counters on" moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CountersOf {
    /// "its counters".
    Its,
    /// "those counters".
    Those,
}

/// What players vote for (701.38b): words with no rules meaning, objects, or colors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum VoteFor {
    /// "grace or condemnation", "Redhorn Pass or Mines of Moria".
    Words(Vec<String>),
    Objects(Box<Selector>),
    /// "blue, black, red, or green".
    Colors(Vec<crate::vocab::Color>),
}

/// How one amount of damage is divided: a chosen distribution is fixed while
/// announcing targets (601.2d); an even division uses the recipients on resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DamageDivision {
    Chosen,
    Evenly { round_up: bool },
}
