//! Static abilities, replacement and prevention effects, and rule changes (CR 604, 609.4, 613,
//! 614, 615, 601.2f): sentences that are true for a while rather than done once.
//!
//! Two shapes. A whole sentence about the game ([`Static`]: "If that creature would die this
//! turn, exile it instead", "~'s power is equal to the number of cards in your hand") is a
//! [`crate::effect::Sentence::Static`]. When or while a sentence applies ("As long as you control
//! a Forest,", "during your turn") is a modifier like any other ([`crate::effect::Modifier`]). A verb phrase with a subject ([`StaticAct`]: "has flying",
//! "enters tapped", "costs {1} less to cast", "attacks each combat if able") is an
//! [`crate::action::Action::Static`] inside an ordinary clause, so it takes subjects, "and"
//! chains, durations and modifiers like any other action ("~ gets +1/+1 and has flying", "This
//! spell costs {1} less to cast for each creature you control").

use crate::action::{Action, Becoming, Delta, Granted, Prevention, Whose};
use crate::amount::Amount;
use crate::condition::{Condition, Restriction};
use crate::effect::Body;
use crate::event::{EventAction, EventPattern};
use crate::lexicon::Verb;
use crate::mana::{ManaCost, ManaSpec, SpendOn};
use crate::selector::{CounterKind, ObjectSet, Selector, ZoneSpec};
use crate::vocab::{Color, Keyword};
use serde::Serialize;

/// A sentence that sets up a continuous, replacement or prevention effect, or changes a rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Static {
    /// "Spells and abilities your opponents control can't cause you to sacrifice permanents."
    /// The causing sources, affected player and forbidden event remain separate descriptions.
    CantCause { sources: ObjectSet, player: Selector, event: EventAction },
    /// "~'s power and toughness are each equal to the number of creatures you control", "~'s
    /// power is equal to …": a characteristic-defining ability (604.3) or a power/toughness
    /// setting effect (613.4b); `None` leaves that value alone.
    Pt { what: Selector, power: Option<Amount>, toughness: Option<Amount> },
    /// "You may sacrifice a Mountain rather than pay ~'s mana cost", "you may tap an untapped
    /// creature you control rather than pay ~'s mana cost if you control a Plains" (`cond`; in
    /// front it is the sentence's lead): an alternative cost (118.9); `cost` is its actions;
    /// `of`: "rather than pay the mana cost for spells you cast" (another spell's mana cost;
    /// None is ~'s).
    /// `optional`: "you may" (118.9b: alternative costs are generally optional, but an effect that
    /// allows casting a spell may require one: "If you cast a spell this way, pay life equal to its
    /// mana value rather than pay its mana cost"; `of` is then that spell, "its mana cost").
    AlternativeCost { cond: Option<Condition>, cost: crate::action::Coord<Action>, of: Option<Who>, optional: bool },
    /// "You may exert ~ as it attacks": an optional cost to attack (701.43d, 508.1g); a "When you
    /// do" ability after it is linked to it (607.2h).
    ExertAsItAttacks,
    /// Substitute payment for a named keyword ability cost (118.9, 702).
    AlternativeKeywordCost { cost: crate::action::Coord<Action>, of: Selector, keyword: Keyword },
    /// "If a triggered ability of an Ally you control triggers, that ability triggers an
    /// additional time", "If a creature dying causes a triggered ability of a permanent you
    /// control to trigger, it triggers an additional time" (`it`: the pronoun) (603.2d).
    TriggersAdditional { cause: TriggerCause, it: bool },
    /// "A deck can have any number of cards named ~", "A deck can have up to seven cards named
    /// Seven Dwarves" (100.2a's exception).
    DeckCanHave(Selector),
    /// "You may pay {U} to end this effect." (any player, any time they have priority).
    PayToEnd(ManaCost),
    /// "This effect doesn't remove ~": the protection the sentence before grants doesn't make
    /// these Auras or Equipment fall off (702.16c–d).
    DoesntRemove(Selector),
    /// "If ~ is in your opening hand, you may begin the game with it on the battlefield"
    /// (103.6a); "If ~ is in your opening hand and you're not the starting player, you may begin
    /// the game with ~ on the battlefield with a luck counter on it." `not_starting`: only a
    /// player who is not the starting player may; `named`: the card is said by its name ("~"),
    /// not "it"; `counters`: what it enters with.
    BeginOnBattlefield { not_starting: bool, named: bool, counters: Option<crate::CountersOn> },
    /// "You may reveal ~ from your opening hand." (103.6b)
    RevealFromOpeningHand,
    /// "Creatures entering don't cause abilities to trigger.", "Creatures entering or dying
    /// don't cause abilities to trigger.", "Permanents entering don't cause abilities of
    /// permanents your opponents control to trigger." (603.2: the event triggers nothing.)
    DontCauseTriggers { what: ObjectSet, dying: bool, of: Option<ObjectSet> },
    /// "While voting, you get an additional vote." (`vote_again` false) and "While voting, you
    /// may vote an additional time." (true) (701.38d).
    AdditionalVote { vote_again: bool },
    /// "~ is also a Cleric, Rogue, Warrior, and Wizard.": creature types it has besides its own
    /// (205.3).
    AlsoSubtypes(Vec<crate::vocab::Subtype>),
    /// "If you would lose unspent mana, that mana becomes black instead." (106.4; None:
    /// colorless.)
    UnspentManaBecomes(Option<crate::vocab::Color>),
    /// "This effect can't reduce the mana in that cost to less than one mana" (601.2f, 118.7).
    ReductionFloorOneMana,
    /// "Players play with the top card of their libraries revealed."
    PlayersPlayRevealed,
    /// "Players can't activate planeswalkers' loyalty abilities." (606)
    NoLoyaltyAbilities,
    /// "All lands are no longer snow." (205.4)
    LandsNoLongerSnow,
    /// "Counters remain on ~ as it moves to any zone other than a player's hand or library."
    /// (an exception to 400.7)
    CountersRemain,
    /// "If a creature an opponent controls attacks, all creatures that opponent controls attack
    /// if able.": a requirement on the rest once one of them attacks (508.1d).
    IfAttacksAllAttack { attacker: Selector, all: ObjectSet },
    /// "You may reveal the first card you draw each turn as you draw it." (`may`) and "Reveal
    /// the first card you draw each turn."
    RevealFirstDraw { may: bool },
    /// "You may activate loyalty abilities of planeswalkers you control on any player's turn
    /// any time you could cast an instant." (606.3)
    LoyaltyAtInstantSpeed,
    /// A permission scoped to the named sources' abilities (606.3, 602.5).
    ActivateAtInstantSpeed { of: AbilitiesOf },
    /// "You may activate the loyalty abilities of planeswalkers you control twice each turn
    /// rather than only once." (`this_turn`: "twice this turn") (606.3)
    LoyaltyTwice { this_turn: bool },
    /// "The \"legend rule\" doesn't apply to permanents you control" (704.5j); None: to anything
    /// the text names before (or everything).
    NoLegendRule { to: Option<Party>, this_turn: bool },
    /// "If that creature would die this turn, exile it instead", "The next time a source of your
    /// choice would deal damage to you this turn, prevent that damage": a replacement (614.1a)
    /// or prevention (615.1a) effect.
    Replacement(Box<Replacement>),
    /// "The next 1 damage that would be dealt to ~ this turn is dealt to target creature
    /// instead", "All damage that would be dealt to you is dealt to ~ instead": a redirection
    /// (614.9); `damage` describes the damage as a prevention would.
    Redirect { damage: Prevention, onto: Party },
    /// "Damage can't be prevented this turn", "The damage can't be prevented", "Damage that would
    /// be dealt by ~ can't be prevented" (615.12); "Damage" alone is all damage. `redirect`: "…
    /// can't be prevented or dealt instead to another permanent or player": it can't be
    /// redirected (614.9) either.
    Unpreventable { damage: Prevention, this_turn: bool, redirect: bool },
    /// "X can't be 0", "X can't be greater than the number of players in the game" (107.3).
    X(XLimit),
    /// "The flashback cost is equal to its mana cost" (702.34a and the like): a granted keyword's
    /// cost is the mana cost of `of`.
    KeywordCost { keyword: Keyword, of: Selector },
    /// "You may cast that card this turn, and mana of any type can be spent to cast that spell",
    /// "Mana of any type can be spent to cast a spell this way" (609.4b): the same permission as
    /// spending mana as though it were mana of any type (after a permission it qualifies:
    /// [`Static::After`]).
    AnyTypeMana { to: SpendOn },
    /// "You may play cards exiled with ~, and mana of any type can be spent to cast them", "~ can't
    /// be countered and the damage can't be prevented", "Enchanted creature can't attack or block,
    /// and its activated abilities can't be activated": a sentence about the game that goes on
    /// from a clause before it, which it refers to ("its", "the damage", "that spell"); one wrapper
    /// for every static that follows a body.
    After { body: Body, join: crate::effect::Join, rest: Box<Static> },
    /// "Each opponent's maximum hand size is reduced by one", "Your maximum hand size is eight"
    /// (402.2).
    MaxHandSize { whose: Selector, size: HandSize },
    /// "Activated abilities of artifacts can't be activated", "Enchanted creature can't attack or
    /// block, and its activated abilities can't be activated", "Activated abilities of creatures
    /// you control cost {2} less to activate" (602.5, 601.2f); "its" refers to the clause before
    /// ([`Static::After`]).
    ActivatedAbilities { of: AbilitiesOf, rule: AbilityRule, this_turn: bool },
    /// "This mana can't be spent to cast nonartifact spells" (106.6: a restriction on how mana
    /// may be spent).
    ManaCantBeSpent(SpendOn),
    /// "This mana can't be spent to cast spells from your hand." (106.6)
    ManaCantBeSpentFromHand,
    /// "Remove ~ from your deck before playing if you're not playing for ante." (407.1)
    AnteOnly,
    /// "Double all damage that creature sources you control would deal." (614.1a, 701.10)
    DoubleDamageOf(ObjectSet),
    /// "While an opponent is choosing targets as part of casting a spell they control or
    /// activating an ability they control, that player must choose at least one Flagbearer on
    /// the battlefield if able." (115)
    Flagbearer,
    /// "Room abilities of dungeons you own trigger an additional time." (309.4)
    RoomAbilitiesTriggerAgain,
    /// "Your opponents can't cast spells or activate abilities of artifacts, creatures, or
    /// enchantments." (101.2)
    OpponentsCantCastOrActivate,
    /// "Draft ~ face up." (a draft matters card, 905)
    DraftFaceUp,
    /// "~ can't attack a player it has already attacked this turn." (508.1c)
    CantAttackSamePlayerAgain,
    /// "If ~ is in a graveyard, effects from spells named Muscle Burst count it as a card named
    /// Muscle Burst." (201.2)
    CountsAsNamedInGraveyard(crate::selector::Name),
    /// "No more than two creatures can attack you each combat." (508.1c)
    NoMoreThanAttackYou(Amount),
    /// "~ can be attached only to a creature with toughness 4 or greater." (301.5, 303.4)
    AttachOnlyTo(Selector),
    /// "You may activate abilities of creatures you control as though those creatures had
    /// haste." (302.6)
    ActivateAsThoughHaste,
    /// "If you would roll one or more dice, instead roll that many dice plus one and ignore the
    /// lowest roll." (706.6)
    RollExtraIgnoreLowest,
    /// "Each player may attack only the nearest opponent in the last chosen direction and
    /// planeswalkers controlled by that opponent." (`last`: "the last chosen direction")
    AttackNearestInDirection { last: bool },
    /// "Reveal ~ as you draft it and note how many cards you've drafted this draft round,
    /// including ~." (a draft matters card, 905)
    DraftRevealNoteCount,
    /// "No more than one creature can attack each combat" (508.1c, 509.1b); `activity`: attack
    /// or block.
    NoMoreThan { count: Amount, activity: Verb },
    /// "~ enters tapped unless you control two or more basic lands", "~ can't attack unless
    /// defending player controls an Island", "you lose 2 life unless you control another
    /// Pirate": the body applies unless the condition holds (a payment after "unless" is the act
    /// modifier [`crate::effect::Modifier::Unless`] instead).
    Unless { body: Body, cond: Condition },
}

/// What makes an ability trigger an additional time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum TriggerCause {
    /// "a triggered ability of a Ninja creature you control triggers".
    AbilityOf(Selector),
    /// "a creature dying causes a triggered ability of a permanent you control to trigger": an
    /// event, said with the gerund.
    Event { event: EventPattern, of: Selector },
}

/// "play lands" (`play`) or "cast creature spells".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Playable {
    pub play: bool,
    pub what: ObjectSet,
}

/// What a skip effect skips (614.10): always the skipping player's own ("your" after "you" and
/// in an imperative, "their" after a third-person subject).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Skipped {
    /// "your draw step", "their next untap step" (`next`: only the next one).
    Step { step: SkipStep, next: bool },
    /// "your next turn", "your next two turns".
    Turns(Amount),
    /// "all combat phases of their next turn": every combat phase of that one turn, however
    /// many it has (500.11, 614.10a).
    CombatPhasesOfNextTurn,
    /// "that turn": the turn a replacement names.
    ThatTurn,
    /// "that draw": the draw a replacement names.
    ThatDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SkipStep {
    Untap,
    Upkeep,
    Draw,
    Combat,
}

/// A maximum hand size (402.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum HandSize {
    /// "is eight".
    Is(Amount),
    /// "is reduced by one".
    ReducedBy(Amount),
    /// "is increased by one".
    IncreasedBy(Amount),
}

/// Whose activated abilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum AbilitiesOf {
    /// An ability family as a whole, independently of its source or controller.
    Matching(crate::ObjectSet),
    /// "its activated abilities": the subject of the clause before.
    Its,
    /// "activated abilities of artifacts", "loyalty abilities of planeswalkers your opponents
    /// control".
    Of { kind: crate::action::AbilityKind, of: Party },
    /// "equip abilities you activate", "equip abilities you activate that target ~".
    KeywordYouActivate { keyword: Keyword, target: Option<Box<Selector>> },
    /// "exhaust abilities of other permanents you control".
    KeywordOf { keyword: Keyword, of: Party },
}

/// What is true of those abilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum AbilityRule {
    /// "can't be activated", "can't be activated unless they're mana abilities" (`unless_mana`).
    CantBeActivated { unless_mana: bool },
    /// "cost {2} less to activate".
    CostChange { more: bool, by: ManaCost },
    /// "can be activated an additional time".
    AdditionalTimes(Amount),
    /// The named ability family does not trigger (603.2).
    DontTrigger,
}

/// Where an object would have gone, in "instead of putting it …" (614.1a: the act replaces that
/// zone change).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Elsewhere {
    /// "into its owner's graveyard", "into your graveyard", "into a graveyard" (None).
    Graveyard(Option<Whose>),
    /// "into your hand".
    Hand(Whose),
    /// "anywhere else".
    AnywhereElse,
    /// "into a graveyard as it resolves", "into its owner's graveyard as it resolves": the
    /// resolving spell's own last step (608.2n).
    GraveyardAsItResolves(Option<Whose>),
}

/// Which mana "spend … as though" covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SpentMana {
    /// "mana".
    Any,
    /// "white mana".
    Color(Color),
    /// "colorless mana".
    Colorless,
}

/// "mana of any color" or "mana of any type" (609.4b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum AnyMana {
    Color,
    Type,
}

/// A limit on the value chosen for X (107.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum XLimit {
    /// "X can't be 0".
    NotZero,
    /// "X can't be greater than …".
    AtMost(Amount),
}

/// "If [event] would [happen], [substitute] instead" (614.1a, 614.6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Replacement {
    /// "The next time …": it applies once, then ends (615.8 for damage from a source).
    pub next_time: bool,
    /// The event it replaces, as "would" says it: "that creature would die", "~ would be put
    /// into a graveyard from anywhere", "damage would be dealt to ~".
    pub event: EventPattern,
    /// "while it has a +1/+1 counter on it", "while your library has no cards in it": the
    /// replacement applies only then.
    pub while_: Option<Condition>,
    /// "this turn": the effect lasts until the turn ends.
    pub this_turn: bool,
    /// What happens instead (its "instead" is an act modifier, or `instead_first`).
    pub with: Substitute,
    /// "…, instead exile it": "instead" opens the substitute.
    pub instead_first: bool,
}

/// Which parts of the proposed counter inventory a replacement transforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CounterKinds {
    Named(CounterKind),
    /// Each kind already proposed, not every possible kind of counter.
    EachProposed,
}

/// The printed replacement instruction, retaining who performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CounterVoice {
    Passive,
    /// "put ... instead": the placing player from the proposed event.
    Imperative,
    /// "it puts ... instead": the effect named as the event's cause.
    Cause,
}

/// What a replacement does instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Substitute {
    /// Instructions: "exile it instead".
    Body(Box<Body>),
    /// "twice that many of those tokens are created instead" (614.1a, 111.1).
    TokensCreated { count: Amount, effect_voice: bool },
    /// "those tokens plus a Clue token are created instead".
    TokensPlus { token: crate::token::TokenSpec, active: bool },
    /// "that many plus one +1/+1 counters are put on it instead".
    CountersPut { count: Amount, kinds: CounterKinds, on: Selector, voice: CounterVoice },
    /// "it produces {B} instead of any other type", "it produces three times as much of that mana
    /// instead": what a permanent tapped for mana produces instead (106.12b: the replacement
    /// modifies the mana production event). Only with an event of tapping for mana.
    Produces(Produces),
    /// "that damage is dealt to ~ instead", "it deals that damage to target creature instead": a
    /// redirection (614.9) replaces the damage dealt to one object or player with the same damage
    /// dealt to another; the damage keeps its source, so saying it as passive or active is one
    /// meaning. Only with an event of dealing damage.
    Redirect(Party),
}

/// What mana a replaced mana production becomes (106.12b).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Produces {
    pub mana: Produced,
    pub instead_of: Displaced,
}

/// The mana produced instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Produced {
    /// "{B}", "{C}{C}": exactly this mana.
    Exactly(ManaSpec),
    /// "colorless mana": each mana produced is colorless.
    Colorless,
    /// "mana of a color of your choice": each mana produced is of a color its controller chooses.
    OfYourChoice,
    /// "twice as much of that mana", "three times as much of that mana": that many times the mana
    /// produced, of the same type; the amount is a multiple of "that much".
    AsMuch(Amount),
}

/// The mana that is not produced.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Displaced {
    /// "any other type": the type of mana it would produce is replaced.
    AnyOtherType,
    /// "any other type and amount": the type and the amount are replaced.
    AnyOtherTypeAndAmount,
    /// "white mana": only that color.
    Color(Color),
    /// Multiplying leaves the type alone: "instead".
    Nothing,
}

/// A static verb phrase: what a subject has, is, costs or must do while the effect lasts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum StaticAct {
    /// "has flying", "have trample and \"{T}: Add {G}.\"": an ability-adding effect (layer 6,
    /// 613.1f), the continuous counterpart of [`crate::action::Action::Gains`].
    Has(Vec<Granted>),
    /// "loses flying", "loses all abilities", "lose all other abilities" (613.1f).
    Loses(Lost),
    /// "isn't a creature", "aren't legendary": a type- or color-removing effect (613.1d–e), the
    /// negation of [`crate::action::Action::Is`].
    IsNot(Becoming),
    /// "enters tapped", "enters with two +1/+1 counters on it": how a permanent enters, a
    /// replacement effect (614.1c–d).
    Enters(Vec<Arrival>),
    /// "escapes with a +1/+1 counter on it": if it escaped, it enters that way (702.138c).
    EscapesWith(Vec<Arrival>),
    /// "saddles Mounts and crews Vehicles as though its power were 2 greater", "… using its
    /// toughness rather than its power" (None) (702.122, 702.171).
    SaddlesAndCrews(Option<Amount>),
    /// "crews Vehicles as though its power were 2 greater" (702.122).
    CrewsAsThoughGreater(Amount),
    /// "costs {1} less to cast", "cost {2} more to activate": a cost increase or reduction
    /// (601.2f, 118.7); "for each …", "if …" and "where X is …" are the act's modifiers.
    CostChange { more: bool, by: ManaCost, of: CostOf },
    /// "costs {X} less to cast this way": the change applies only when the spell is cast the
    /// way the text before allowed (a flashback, a permission).
    CostChangeThisWay { more: bool, by: ManaCost },
    /// A minimum mana component of the total cost, applied after increases and reductions
    /// (601.2f). "Each spell that would cost less than three mana to cast costs three mana to cast."
    CostMinimum { amount: Amount, of: CostOf },
    /// "attacks each combat if able", "blocks ~ this turn if able", "must be blocked this turn if
    /// able": a requirement (508.1d, 509.1c) that an event of the subject happen, any of them
    /// when several are named ("attacks or blocks"); `when` is the window the text names.
    IfAble { what: Vec<EventAction>, when: Option<Within> },
    /// "can block only creatures with flying" (509.1b: a blocking restriction).
    CanBlockOnly(ObjectSet),
    /// "can block an additional creature each combat", "can block up to two additional creatures
    /// this turn", "can block any number of creatures" (509.1b); `count` None: any number.
    CanBlockAdditional { count: Option<Amount>, up_to: bool, when: Option<Within> },
    /// "can attack as though it didn't have defender", "can block creatures with shadow as though
    /// they didn't have shadow", "can be blocked as though it didn't have swampwalk", "can be cast
    /// as though it had flash" (609.4): the subject may do or undergo these events as though
    /// `as_though` were true. `when`: the window the text names ("can attack this turn …").
    Can { events: Vec<EventAction>, when: Option<Within>, as_though: Premise },
    /// "activate exhaust abilities as though they haven’t been activated" (702.177b).
    ActivateAsThoughUnused(ObjectSet),
    /// "assigns no combat damage" (510.1).
    AssignsNoCombatDamage,
    /// "assigns combat damage equal to its toughness rather than its power" (510.1a).
    AssignsByToughness,
    /// "assign its combat damage as though it weren't blocked", "… this turn as though they
    /// weren't blocked" (609.4, 510.1c); `plural`: the pronouns are "their … they", agreeing
    /// with the subject.
    AssignAsThoughUnblocked { this_turn: bool, plural: bool },
    /// "look at the top card of your library any time" (a permission to look at a hidden card).
    LookAnyTime(Who),
    /// "cast ~ as though it had flash", "cast creature spells this turn as though they had
    /// flash" (601.3b, 609.4).
    CastAsThoughFlash { what: Who, this_turn: bool },
    /// The subject may cast matching spells only under these restrictions (601.3).
    /// Sorcery timing requires priority, their own main phase and an empty stack (307.5).
    CastOnly { what: ObjectSet, restrictions: Vec<Restriction> },
    /// "gets an additional +1/+1" (613.4c, on top of another change the text makes).
    GetsAdditional { power: Delta, toughness: Delta },
    /// "control enchanted creature" (613.1b: a control-changing effect).
    Controls(Selector),
    /// "play an additional land on each of your turns", "play two additional lands on each of
    /// their turns" (305.2); `their`: the possessive agrees with a third-person subject.
    AdditionalLandsEachTurn { count: Amount, their: bool },
    /// "choose not to untap ~ during your untap step" (502.3).
    ChooseNotToUntap(Selector),
    /// "can be your commander" (903.3a).
    CanBeYourCommander,
    /// "(all creatures able to block ~) do so": each of the subject blocks what its "able to
    /// block" filter names (509.1c).
    DoSo,
    /// "don't lose the game for having 0 or less life" (104.3b).
    NoLossAtZeroLife,
    /// "Damage that would reduce your life total to less than 1 reduces it to 1 instead": a
    /// replacement (614.1a) of the life loss damage causes (120.3a), said of the subject, the
    /// player whose life total it is: damage dealt to them makes them lose only the life that
    /// leaves `floor`.
    DamageLifeFloor { floor: Amount },
    /// "deals double that damage instead", "deals that much damage plus 1 instead", "~ deals 2
    /// damage instead": the replaced damage event, modified, to the same recipient (614.1a).
    DealsToSame(Amount),
    /// "untap all permanents you control during each other player's untap step" (502.3).
    UntapDuringOthersUntapSteps(crate::action::Objects),
    /// "play lands and cast spells from your graveyard", "cast creature spells from the top of
    /// your library": permission to play cards from a zone (601.3, 305.1); with no zone, from
    /// where they can be cast anyway, and the permission is paid some other way ("cast creature
    /// spells with mana value 3 or less by paying {E} rather than paying their mana costs":
    /// [`crate::effect::Modifier::By`]).
    PlayFrom { what: Vec<Playable>, from: Option<ZoneSpec> },
    /// "skip your draw step", "skip your next turn", "skip your next two turns" (614.10).
    Skip(Skipped),
    /// "play with the top card of your library revealed".
    PlayWithRevealed(Selector),
    /// "can't attack alone", "can't attack or block alone" (508.1c, 509.1b).
    CantAlone(Vec<Verb>),
    /// "spend mana as though it were mana of any color to cast that spell", "spend white mana as
    /// though it were mana of any color" (609.4b); `mana`: only mana of that color.
    SpendAsThough { mana: SpentMana, any: AnyMana, to: Option<SpendOn> },
    /// "have no maximum hand size" (402.2).
    NoMaximumHandSize,
}

/// The condition an "as though" effect treats as true (609.4). It speaks of the object the ability
/// is about: the subject ("it had flash", "it didn't have defender"), except after "block", where
/// what did not have the ability is the creature blocked ("they didn't have shadow") and what had
/// it is the blocker ("it had reach").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Premise {
    /// "it had flash", "they had reach".
    Had(Vec<Granted>),
    /// "it didn't have defender", "they didn't have shadow", "it didn't have swampwalk".
    DidntHave(crate::token::KeywordInstance),
    /// "they were untapped" (what a tapped creature can do, 509.1a).
    Untapped,
}

/// "by discarding a card in addition to paying its other costs", "by paying {1} rather than paying
/// its mana cost": the cost a permission to cast says the spell is cast with (118.8, 118.9).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CastCost {
    pub actions: crate::action::Coord<Action>,
    pub kind: CastCostKind,
    /// "its other costs" / "their other costs": agrees with the spells cast (a number, not a
    /// meaning).
    pub plural: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CastCostKind {
    /// "in addition to paying its other costs": an additional cost (118.8).
    Additional,
    /// "rather than paying its mana cost": an alternative cost (118.9).
    Alternative,
}

/// What an ability-removing effect takes away.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Lost {
    /// "flying", "flying and first strike", a quoted ability.
    Abilities(Vec<Granted>),
    /// "all abilities", "all creature types", "all other card types and abilities", "all land
    /// types and abilities": everything of these kinds (613.1d type-changing, 613.1f abilities);
    /// `other`: the word after "all", leaving what the same effect sets (205.1a).
    AllOf { other: bool, what: Vec<Lose> },
    /// "this ability": the ability whose text says it.
    ThisAbility,
}

/// What kind of thing [`Lost::AllOf`] takes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Lose {
    Abilities,
    /// A whole class of types: "creature types", "card types".
    Types(crate::action::Every),
}

/// One part of how a permanent enters (614.1c–d).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Arrival {
    Tapped,
    /// "tapped and attacking".
    TappedAndAttacking,
    /// "with two +1/+1 counters on it", "with an additional +1/+1 counter on it", "with a
    /// loyalty counter on her" (`on`: a character's pronoun for "it").
    Counters(crate::action::CountersOn),
    /// "with your choice of a reach counter or a vigilance counter on it": one of these, as its
    /// controller chooses while it enters (614.12a).
    ChoiceOfCounters(Vec<crate::action::Counters>),
    /// "with two +1/+1 counters and a lifelink counter on it": counters of several kinds.
    SeveralCounters { counters: Vec<crate::action::Counters>, on: crate::action::On },
    /// "prepared" (722.3a).
    Prepared,
    /// "as a copy of a creature you control" (614.1c, 707).
    AsCopyOf(Selector),
    /// "untapped" ("Lands you control enter untapped").
    Untapped,
    /// "under the control of an opponent of your choice".
    UnderControlOf(Selector),
    /// "with flying", "with \"{T}: Add {G}.\"": it enters with these abilities (614.1c: "[This
    /// permanent] enters with . . ."), after another part ("two +1/+1 counters on it and with
    /// flying").
    Has(Vec<Granted>),
}

/// Whose cost a cost change changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CostOf {
    /// "to cast" (a spell's total cost, 601.2f).
    Cast,
    /// "to activate" (an activated ability's total cost, 602.2b).
    Activate,
}

/// The window of a requirement, or what a prohibition's limit is counted within.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Within {
    /// "each combat".
    EachCombat,
    /// "this turn".
    ThisTurn,
    /// "this combat".
    ThisCombat,
    /// "each turn": what a limit is counted within ("more than one spell each turn").
    EachTurn,
    /// "during their untap steps": each player's own untap step (502.3).
    UntapStep,
}

/// One thing a prohibition forbids (101.2): an event its subject can't do or have done to it,
/// said as a rule says it. "attack", "block creatures with flying" (any such creature), "be
/// blocked by more than one creature", "be countered", "gain life", "cast noncreature spells".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Forbidden {
    pub event: EventAction,
    /// The span a limit is counted within ("cast more than one spell *each turn*", "untap more
    /// than two permanents *during their untap steps*"); or how long the prohibition lasts, where
    /// the phrase goes on after it ("be blocked *this turn* except by Walls": at the end of the
    /// phrase it is the act's duration).
    pub within: Option<Within>,
}

/// Several [`Who`] joined by a conjunction: "you and creatures you control", "you and/or
/// permanents you control", "target creature or player".
pub type Party = crate::action::Coord<Who>;

/// Where "this turn" stands inside a damage phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TurnAt {
    /// "that would be dealt this turn to …".
    AfterDealt,
    /// "that would be dealt to you this turn by …".
    BeforeBy,
    /// "that would be dealt to ~ this turn" at the end of a redirection's subject.
    End,
}

/// Objects or players a static names: a selector ("~", "you", "each creature") or a bare plural
/// set ("creatures with flying", "planeswalkers you control"). Several in a list are
/// alternatives ("you or planeswalkers you control").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Who {
    One(Selector),
    Set(ObjectSet),
}
