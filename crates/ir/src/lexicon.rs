//! The lexicon: the game's actions, each defined once.
//!
//! "Anything that happens in a game is an event" (CR 700.1), and card text talks about the same
//! events in several ways: it instructs them ("destroy target creature"), watches for them
//! ("whenever a creature is destroyed"), replaces them, forbids them. An entry here says what
//! one action is, whatever construction names it:
//!
//! - the rule that defines it;
//! - who performs it ([`Doer`]);
//! - what each of its roles takes ([`Role`]), as the rules sort things ([`Sort`]).
//!
//! The grammar reads the kinds from here to type its slots, and the engine implements each
//! entry once: how the action is performed and the event it becomes.
//!
//! Not every [`crate::Action`] is in the lexicon yet: what it says of a subject while an effect
//! lasts ("gets +2/+2", "gains flying", "can't block") is a continuous effect (611), not an
//! action, and keyword actions without typed roles are still to be entered.

use crate::action::Action;
use crate::event::{Arg, EventAction, EventPattern};
use crate::selector::{Anaphor, Atom, Domain, Filter, Quant, Ref, Selector, Status};
use crate::vocab::CardType;
use serde::Serialize;

/// Who or what performs an action: the role of a clause's subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Doer {
    /// An effect directly changes a game structure; no player or object performs it (500.8).
    None,
    /// A player does it; with no subject, the controller ("Draw a card", "target player draws").
    Player,
    /// An object does it, and the text names it ("~ deals 3 damage", "it fights target creature").
    Thing,
    /// It is said of a subject the text names, of any kind ("gains hexproof", "can't block").
    Named,
    /// Either ("investigate", "it explores"): not yet told apart.
    Any,
}

/// A role of an action besides its doer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Role {
    /// What it is done to: "destroy *target creature*".
    What,
    /// "attach it *to target creature*", "deals 3 damage *to any target*", "move a counter … *onto*".
    To,
    /// "remove a counter *from ~*", "unattach an Equipment *from ~*".
    From,
    /// "put a +1/+1 counter *on target creature*".
    On,
    /// "clash *with an opponent*", "exchange life totals *with target player*".
    With,
    /// "a token that's a copy *of target creature*", "double the power *of each creature*".
    Of,
    /// "distribute three counters *among* …".
    Among,
    /// "becomes blocked *by a creature*": the doer, named after a passive.
    By,
    /// "created *under your control*": the player who controls what comes to be.
    Under,
    /// "rolls *a natural 20*": what a die roll shows (706.2).
    Result,
    /// "enters *tapped*": the status it has as it arrives (110.5, 614.12).
    As,
}

/// What the subject of an event is: the one who does it ("you sacrifice a creature"), or what
/// it happens to ("a creature dies", "~ becomes tapped", "~ is dealt damage").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SubjectIs {
    Doer,
    Role(Role),
}

/// What kind of thing something is, as the rules sort what is in a game: its players (102.1)
/// and its objects (109.1). What an action can be done to, and by what, is said in these, and
/// both the grammar (does this noun phrase name such a thing?) and the engine (is this thing
/// one, now?) ask it of the same declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Sort {
    /// A player (102.1).
    Player,
    /// A card or token on the battlefield (110.1).
    Permanent,
    /// A permanent of a card type: "only a creature can attack or block" (506.3), "objects can
    /// deal damage to battles, creatures, planeswalkers, and players" (120.1).
    Of(CardType),
    /// An object represented by a Magic card (108.2), including permanent and spell cards.
    Card,
    /// A card or copy on the stack (112.1).
    Spell,
    /// An activated or triggered ability on the stack (113.1c).
    Ability,
}

impl Sort {
    /// Is everything of this sort also of that one? A creature is a permanent.
    pub fn is_a(self, other: Sort) -> bool {
        self == other || matches!((self, other), (Sort::Of(_), Sort::Permanent))
    }
}

const PERMANENT: &[Sort] = &[Sort::Permanent];
const CARD: &[Sort] = &[Sort::Card];
const PLAYER: &[Sort] = &[Sort::Player];
/// "Only a creature can attack or block" (506.3); creatures fight (701.14a).
const CREATURE: &[Sort] = &[Sort::Of(CardType::Creature)];
/// A spell or ability on the stack (701.6a: what can be countered).
const ON_STACK: &[Sort] = &[Sort::Spell, Sort::Ability];
/// What a zone change moves: a card, a permanent, a spell (400.7).
const MOVABLE: &[Sort] = &[Sort::Card, Sort::Permanent, Sort::Spell];
/// What can be exiled (701.13a): an object, wherever it is.
const OBJECT: &[Sort] = &[Sort::Card, Sort::Permanent, Sort::Spell, Sort::Ability];
/// What damage can be dealt to (120.1): battles, creatures, planeswalkers and players.
const DAMAGEABLE: &[Sort] =
    &[Sort::Of(CardType::Creature), Sort::Of(CardType::Planeswalker), Sort::Of(CardType::Battle), Sort::Player];
/// What counters can be put on (122.1): an object or a player.
const COUNTERABLE: &[Sort] = &[Sort::Permanent, Sort::Card, Sort::Player];
const CARD_OR_PERMANENT: &[Sort] = &[Sort::Card, Sort::Permanent];
/// What control can be gained or exchanged of (701.12b, 613.1b).
const CONTROL_TARGETS: &[Sort] = &[Sort::Permanent, Sort::Spell, Sort::Player];
const CONTROLLED: &[Sort] = &[Sort::Permanent, Sort::Spell];
/// What an instruction can copy (707.10, 707.12): a spell, an ability, a card.
const COPIABLE: &[Sort] = &[Sort::Spell, Sort::Ability, Sort::Card];
/// What can be cast: a card, or the spell a card or copy becomes (601.2a).
const CASTABLE: &[Sort] = &[Sort::Card, Sort::Spell];
/// What can be played: that, and a land, which the text names as the permanent it becomes
/// ("play a land", 305.1).
const PLAYABLE: &[Sort] = &[Sort::Card, Sort::Spell, Sort::Permanent];
/// What a token can be a copy of (707.2).
const TOKEN_COPY_OF: &[Sort] = &[Sort::Permanent, Sort::Card, Sort::Spell];
/// What an Aura or Equipment attaches to (701.3; an Aura can enchant a player or a card, 303.4).
const ATTACHED_TO: &[Sort] = &[Sort::Permanent, Sort::Player, Sort::Card];
/// What "airbend" exiles (701.65a).
const PERMANENT_OR_SPELL: &[Sort] = &[Sort::Permanent, Sort::Spell];
/// "Only a player, a planeswalker, or a battle can be attacked" (506.3).
const ATTACKED: &[Sort] = &[Sort::Player, Sort::Of(CardType::Planeswalker), Sort::Of(CardType::Battle)];
/// What can be chosen: anything.
const ANYTHING: &[Sort] = &[Sort::Permanent, Sort::Card, Sort::Spell, Sort::Ability, Sort::Player];
/// An activated or triggered ability (602.2).
const ABILITY: &[Sort] = &[Sort::Ability];
/// A spell with convoke (702.51a), or the permanent it became (702.51c refers to it as the spell).
const ON_STACK_OR_PERMANENT: &[Sort] = &[Sort::Spell, Sort::Permanent];

/// What must be true of the thing an action is done to, beyond its sort, for the action to be
/// possible: said by the action's own rule. The engine asks it before performing the action
/// and of a cost before it is paid (118.3: "a permanent that's already tapped can't be tapped
/// to pay a cost"); the grammar asks whether a noun phrase denies it ("tap target tapped
/// creature").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Needs {
    /// "Only untapped permanents can be tapped" (701.26a).
    Untapped,
    /// "Only tapped permanents can be untapped" (701.26b).
    Tapped,
    /// "A player can't sacrifice … a permanent they don't control" (701.21a).
    Controlled,
    /// "To discard a card, move it from its owner's hand" (701.9a): the player's own hand.
    InHand,
}

/// One entry of the lexicon.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The verb in its base form: "sacrifice", "look at", "leave the battlefield". What it is
    /// done to follows it, or stands at `_` ("tap _ for mana"). Empty: no construction but the
    /// action's own instruction says it.
    pub word: &'static str,
    /// The Comprehensive Rules number that defines the action.
    pub rule: &'static str,
    pub doer: Doer,
    /// What each role takes; a role not listed is not typed (a number, a mana cost).
    pub roles: &'static [(Role, &'static [Sort])],
    /// What must be true of what it is done to (`needs Untapped`).
    pub needs: &'static [Needs],
}

macro_rules! lexicon {
    ($($(#[$doc:meta])* $verb:ident: $word:literal, $rule:literal, $doer:ident, [$($role:ident => $kinds:expr),*] $(needs $need:ident)?;)+) => {
        /// A game action.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        pub enum Verb { $($(#[$doc])* $verb),+ }

        impl Verb {
            pub const ALL: &'static [Verb] = &[$(Verb::$verb),+];

            pub fn entry(self) -> Entry {
                match self {
                    $(Verb::$verb => Entry {
                        word: $word,
                        rule: $rule,
                        doer: Doer::$doer,
                        roles: &[$((Role::$role, $kinds)),*],
                        needs: &[$(Needs::$need)?],
                    },)+
                }
            }
        }
    };
}

lexicon! {
    // cards and zones
    Draw: "draw", "121.1", Player, [What => CARD];
    Discard: "discard", "701.9a", Player, [What => CARD] needs InHand;
    Mill: "mill", "701.17", Player, [What => CARD];
    /// "enters": a permanent comes onto the battlefield (603.6a); the text may name it as the
    /// card it was ("when an exiled card enters under your control").
    Enter: "enter", "603.6", Thing, [What => CARD_OR_PERMANENT, Under => PLAYER];
    /// "dies": is put into a graveyard from the battlefield (700.4).
    Die: "die", "700.4", Thing, [What => PERMANENT, Under => PLAYER];
    /// "leaves the battlefield" (603.6c).
    LeaveBattlefield: "leave the battlefield", "603.6", Thing, [What => PERMANENT, Under => PLAYER];
    // combat
    Attack: "attack", "508.1", Any, [To => ATTACKED, With => CREATURE];
    Block: "block", "509.1", Thing, [What => CREATURE, By => CREATURE];
    Sacrifice: "sacrifice", "701.21a", Player, [What => PERMANENT] needs Controlled;
    Destroy: "destroy", "701.8", Player, [What => PERMANENT];
    Exile: "exile", "701.13a", Player, [What => OBJECT];
    /// "return", "put": a zone change with no other cause.
    Move: "put", "400.7", Player, [What => MOVABLE];
    /// A token's or copy's arrival is not a move; what an until-loop reveals or exiles is a card.
    RevealUntil: "", "701.20", Player, [What => CARD];
    Reveal: "reveal", "701.20", Player, [What => CARD_OR_PERMANENT];
    /// Seeing a card in a hidden zone, or a face-down one (400.2).
    LookAt: "look at", "400.2", Player, [What => CARD_OR_PERMANENT];
    Search: "search", "701.23", Player, [What => CARD];
    Shuffle: "shuffle", "701.24", Player, [What => CARD_OR_PERMANENT];
    Scry: "scry", "701.22", Player, [];
    Surveil: "surveil", "701.25", Player, [];
    Cast: "cast", "601.2", Player, [What => CASTABLE];
    Play: "play", "701.18", Player, [What => PLAYABLE];
    SeparateIntoPiles: "separate", "700.3", Player, [What => CARD_OR_PERMANENT];
    // permanents
    Tap: "tap", "701.26", Player, [What => PERMANENT] needs Untapped;
    Untap: "untap", "701.26", Player, [What => PERMANENT] needs Tapped;
    Exert: "exert", "701.43", Player, [What => PERMANENT];
    Regenerate: "regenerate", "701.19", Player, [What => PERMANENT];
    Fight: "fight", "701.14", Thing, [What => CREATURE];
    Flip: "flip", "710.4", Player, [What => PERMANENT];
    TurnFace: "turn", "708.2", Player, [What => CARD_OR_PERMANENT];
    RemoveFromCombat: "remove _ from combat", "506.4", Player, [What => PERMANENT];
    Attach: "attach", "701.3", Player, [What => PERMANENT, To => ATTACHED_TO];
    Unattach: "unattach", "701.3", Player, [What => PERMANENT, From => PERMANENT];
    GainControl: "gain control of", "613.1, 723", Player, [What => CONTROL_TARGETS];
    ExchangeControl: "exchange control of", "701.12", Player, [What => CONTROLLED];
    Transform: "transform", "701.27", Player, [What => PERMANENT];
    Convert: "convert", "701.28", Player, [What => PERMANENT];
    Goad: "goad", "701.15", Player, [What => PERMANENT];
    Detain: "detain", "701.35", Player, [What => PERMANENT];
    Suspect: "suspect", "701.60", Player, [What => PERMANENT];
    Harness: "harness", "701.64", Player, [What => PERMANENT];
    Airbend: "airbend", "701.65", Player, [What => PERMANENT_OR_SPELL];
    Manifest: "manifest", "701.40", Player, [What => CARD];
    Cloak: "cloak", "701.58", Player, [What => CARD];
    DoublePt: "double", "701.10", Player, [Of => PERMANENT];
    TriplePt: "triple", "701.11", Player, [Of => PERMANENT];
    SwitchPt: "switch", "613.4", Player, [Of => PERMANENT];
    // the stack
    Counter: "counter", "701.6", Player, [What => ON_STACK];
    Copy: "copy", "707.10", Player, [What => COPIABLE];
    ChangeTarget: "", "115.7", Player, [What => ON_STACK];
    BeTargeted: "", "115.9", None, [What => ANYTHING, Of => ON_STACK];
    // damage, life, counters
    /// Its source deals it (120.1), any object (609.7), named after the passive with "by".
    DealDamage: "deal", "120.1", Thing, [To => DAMAGEABLE, By => OBJECT];
    GainLife: "gain", "119.3", Player, [];
    LoseLife: "lose", "119.3", Player, [];
    /// A player's life total made twice its value by gaining or losing life (701.10d).
    DoubleLife: "double", "701.10", Player, [Of => PLAYER];
    PutCounters: "put", "122.1", Player, [On => COUNTERABLE];
    RemoveCounters: "remove", "122.1", Player, [From => COUNTERABLE];
    MoveCounters: "move", "122.5", Player, [From => PERMANENT, To => PERMANENT];
    DoubleCounters: "double", "701.10", Player, [On => PERMANENT];
    Distribute: "distribute", "122.1", Player, [Among => PERMANENT];
    // tokens
    Create: "create", "701.7", Player, [What => PERMANENT, Under => PLAYER];
    CreateCopy: "create", "707.2", Player, [Of => TOKEN_COPY_OF];
    // players
    Clash: "clash", "701.30", Player, [With => PLAYER];
    ExchangeLifeTotals: "exchange", "701.12", Player, [With => PLAYER];
    /// A player's life total and a permanent's power or toughness exchanged (701.12g): the life
    /// total is the player's (`Of`), the stat the permanent's (`With`).
    ExchangeLifeWithStat: "exchange", "701.12", Player, [Of => PLAYER, With => PERMANENT];
    /// Any turn as it begins, before replacement effects can skip it.
    BeginTurn: "begin a turn", "500.1", Player, [];
    /// A turn beyond the ones the turn order gives, as it begins.
    BeginExtraTurn: "begin an extra turn", "500.7", Player, [];
    PayLife: "pay life", "119.4", Player, [];
    LoseGame: "lose the game", "104.3", Player, [];
    WinGame: "win the game", "104.2", Player, [];
    WinFlip: "win a coin flip", "705.2", Player, [];
    /// The player who flipped and called a coin, when the call did not match (705.2).
    LoseFlip: "lose a coin flip", "705.2", Player, [];
    RollDice: "roll one or more dice", "706.1", Player, [];
    BecomeMonarch: "become the monarch", "725.1", Player, [];
    /// A permanent's level becoming a number (716.2a); the event says which.
    BecomeLevel: "", "716.2a", None, [What => PERMANENT];
    CompleteDungeon: "complete a dungeon", "309.7", Player, [];
    /// Unlocking the half of a Room the ability is on (709.5h).
    UnlockDoor: "unlock this door", "709.5", Player, [];
    /// A permanent card was put into the player's graveyard this turn.
    Descend: "descend", "700.11", Player, [];
    CommitCrime: "commit a crime", "700.13", Player, [];
    Proliferate: "proliferate", "701.34", Player, [];
    // what a permanent does itself
    Explore: "explore", "701.44", Thing, [];
    Connive: "connive", "701.50", Thing, [];
    BecomeMonstrous: "become monstrous", "701.37", Thing, [];
    Evolve: "evolve", "702.100", Thing, [];
    Mutate: "mutate", "702.140", Thing, [];
    /// A phased-out permanent's status changes back (702.26c).
    PhaseIn: "phase in", "702.26", Thing, [];
    /// A choice made as an effect is applied ("the chosen creature", "the chosen player").
    Choose: "choose", "608.2d", Player, [What => ANYTHING];
    // spells and abilities
    /// Putting an ability on the stack and paying its costs.
    Activate: "activate", "602.2", Player, [What => ABILITY];
    /// Activating a mana ability of a permanent that has {T} in its cost.
    TapForMana: "tap _ for mana", "106.12", Player, [What => PERMANENT];
    /// Discarding a card to pay the cost of its cycling ability, from the hand, where alone
    /// that ability functions (702.29a, 702.29c).
    Cycle: "cycle", "702.29", Player, [What => CARD];
    /// Casting a spell for its warp cost (702.185c).
    Warp: "warp", "702.185", Player, [What => CASTABLE];
    /// Sacrificing a creature to an exploit ability; the creature with exploit does it.
    Exploit: "exploit", "702.110", Thing, [What => PERMANENT];
    /// A creature is tapped to pay the cost of a Vehicle's crew ability (702.122b); the Vehicle is
    /// "crewed by" that creature (702.122c).
    Crew: "crew", "702.122", Thing, [What => PERMANENT, By => CREATURE];
    /// A creature tapped to pay for mana in a spell's total cost "convoked" that spell (702.51c).
    Convoke: "convoke", "702.51", Thing, [What => ON_STACK_OR_PERMANENT];
    /// What a permanent sacrificed as it entered, as a result of its devour ability, "devoured"
    /// (702.82b).
    Devour: "devour", "702.82", Thing, [What => CREATURE];
    /// A card exiled by a haunt ability "haunts" the object that ability targeted (702.55b),
    /// whether or not that object is still a creature.
    Haunt: "haunt", "702.55", Thing, [What => PERMANENT];
    /// Exiled by a permanent as the direct result of its champion ability (702.72c).
    Champion: "champion", "702.72c", Thing, [What => PERMANENT, By => PERMANENT];
    /// Being tapped to pay a saddle cost (702.171c).
    Saddle: "saddle", "702.171", Thing, [What => PERMANENT];
    // keyword actions a player or a permanent takes (701, 702)
    /// Revealing a card of that quality from the hand or choosing a permanent of it (701.4a).
    Behold: "behold", "701.4", Player, [What => CARD_OR_PERMANENT];
    Investigate: "investigate", "701.16", Player, [];
    Discover: "discover", "701.57", Player, [];
    CollectEvidence: "collect evidence", "701.59", Player, [];
    Forage: "forage", "701.61", Player, [];
    ManifestDread: "manifest dread", "701.62", Player, [];
    /// Exiling a card face down from the hand to cast it later (702.143a).
    Foretell: "foretell", "702.143", Player, [What => CARD];
    /// Exiling a card from the hand to cast it later without paying its mana cost (702.170a);
    /// the card is plotted (702.170c).
    Plot: "plot", "702.170", Player, [What => CARD];
    /// Tapping a non-attacking creature to add its power to the attacker (702.154a).
    Enlist: "enlist", "702.154", Thing, [What => CREATURE];
    /// Putting a +1/+1 counter on an attacking creature with lesser power (702.134a).
    Mentor: "mentor", "702.134", Thing, [What => CREATURE];
    /// Putting a +1/+1 counter on itself when attacking with a stronger creature (702.149a).
    Train: "train", "702.149", Thing, [];
    /// A permanent is treated as though it does not exist until it phases in (702.26b).
    PhaseOut: "phase out", "702.26", Thing, [];
}

/// The forms of a verb: "draw", "draws", "drew", "drawn", "drawing".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Inflection {
    Base,
    /// Third person singular: "a player draws".
    Third,
    /// Simple past: "you drew".
    Past,
    /// Past participle: "you've drawn", "is drawn", "the drawn card".
    Participle,
    /// "drawing".
    Gerund,
}

/// The verbs that do not follow the rule: (base, third, past, participle, gerund).
const IRREGULAR: &[[&str; 5]] = &[
    ["draw", "draws", "drew", "drawn", "drawing"],
    ["cast", "casts", "cast", "cast", "casting"],
    ["put", "puts", "put", "put", "putting"],
    ["deal", "deals", "dealt", "dealt", "dealing"],
    ["lose", "loses", "lost", "lost", "losing"],
    ["leave", "leaves", "left", "left", "leaving"],
    ["fight", "fights", "fought", "fought", "fighting"],
    ["choose", "chooses", "chose", "chosen", "choosing"],
    ["die", "dies", "died", "died", "dying"],
    ["pay", "pays", "paid", "paid", "paying"],
    ["win", "wins", "won", "won", "winning"],
    ["become", "becomes", "became", "become", "becoming"],
    ["commit", "commits", "committed", "committed", "committing"],
    // a final consonant doubled though the stress is not obvious from the spelling
    ["untap", "untaps", "untapped", "untapped", "untapping"],
    ["surveil", "surveils", "surveilled", "surveilled", "surveilling"],
    ["foretell", "foretells", "foretold", "foretold", "foretelling"],
];

/// One word in a form, by the regular rules of English spelling.
pub fn inflect(word: &str, form: Inflection) -> String {
    if form == Inflection::Base {
        return word.to_string();
    }
    if let Some(row) = IRREGULAR.iter().find(|row| row[0] == word) {
        return row[form as usize].to_string();
    }
    let vowel = |c: char| "aeiou".contains(c);
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let last = chars[n - 1];
    let before = if n >= 2 { chars[n - 2] } else { ' ' };
    let stem = &word[..word.len() - last.len_utf8()];
    // consonant + "y": "copy" → "copies", "copied"
    let consonant_y = last == 'y' && !vowel(before);
    // one syllable ending consonant-vowel-consonant doubles the last: "tap" → "tapped"
    let doubles = chars.iter().filter(|c| vowel(**c)).count() == 1
        && n >= 3
        && !vowel(last)
        && !"wxy".contains(last)
        && vowel(before)
        && !vowel(chars[n - 3]);
    match form {
        Inflection::Base => unreachable!(),
        Inflection::Third if consonant_y => format!("{stem}ies"),
        Inflection::Third if "sxz".contains(last) || word.ends_with("ch") || word.ends_with("sh") => format!("{word}es"),
        Inflection::Third => format!("{word}s"),
        Inflection::Past | Inflection::Participle if consonant_y => format!("{stem}ied"),
        Inflection::Past | Inflection::Participle if last == 'e' => format!("{word}d"),
        Inflection::Past | Inflection::Participle if doubles => format!("{word}{last}ed"),
        Inflection::Past | Inflection::Participle => format!("{word}ed"),
        Inflection::Gerund if last == 'e' && before != 'e' => format!("{stem}ing"),
        Inflection::Gerund if doubles => format!("{word}{last}ing"),
        Inflection::Gerund => format!("{word}ing"),
    }
}

impl Verb {
    /// The verb in a form, up to where what it is done to stands: "sacrifices", "looked at",
    /// "left the battlefield", "tapped" (of "tap _ for mana"). Its first word is the one inflected.
    pub fn head(self, form: Inflection) -> String {
        let word = self.entry().word;
        let head = word.split(" _").next().unwrap_or(word);
        match head.split_once(' ') {
            Some((first, rest)) => format!("{} {rest}", inflect(first, form)),
            None => inflect(head, form),
        }
    }

    /// What follows the thing it is done to: "for mana" in "tap _ for mana"; empty for most.
    pub fn tail(self) -> &'static str {
        self.entry().word.split_once(" _ ").map_or("", |(_, tail)| tail)
    }

    /// The whole verb in a form, when nothing stands inside it: "tapped for mana", "sacrificed".
    pub fn phrase(self, form: Inflection) -> String {
        match self.tail() {
            "" => self.head(form),
            tail => format!("{} {tail}", self.head(form)),
        }
    }

    /// The verb where nothing can follow it. A zone change is "put" with where it goes ("put
    /// into your graveyard") and "return" without ("the returned card", "cards you returned this
    /// way"): two words of the text for one action.
    pub fn bare(self, form: Inflection) -> String {
        match self {
            Verb::Move => inflect("return", form),
            v => v.phrase(form),
        }
    }

    /// What performs the action, as the rules sort it: a player, or an object, and for the
    /// actions only a creature performs, a creature: "only a creature can attack or block"
    /// (506.3), creatures fight (701.14a). The text may say an attack of the player whose
    /// creatures attack (508.3d), which is why [`Doer`] leaves its subject open.
    pub fn performer(self) -> &'static [Sort] {
        match (self, self.entry().doer) {
            (Verb::Attack | Verb::Block | Verb::Fight, _) => CREATURE,
            (_, Doer::None) => &[],
            (_, Doer::Player) => PLAYER,
            (_, Doer::Thing) => OBJECT,
            (_, Doer::Named | Doer::Any) => ANYTHING,
        }
    }

    /// What the subject of the active verb is: who does it, or, for what happens to a thing
    /// without anyone doing it ("enters", "dies", "leaves the battlefield"), the thing.
    pub fn subject(self) -> SubjectIs {
        match self {
            Verb::Enter | Verb::Die | Verb::LeaveBattlefield => SubjectIs::Role(Role::What),
            _ => SubjectIs::Doer,
        }
    }

    /// The role that becomes the subject in the passive: what it is done to ("a card is
    /// exiled"); the one attacked ("you've been attacked"). None: the verb has no passive of
    /// that shape. What enters, dies or leaves is the subject of the active verb; a verb that
    /// ends in a preposition ("gain control of") is not said in the passive; the damage and
    /// the counters of "is dealt damage" and "counters are put on" stay inside the phrase.
    pub fn passive(self) -> Option<Role> {
        match self {
            Verb::Attack => Some(Role::To),
            Verb::Enter | Verb::Die | Verb::LeaveBattlefield | Verb::GainControl | Verb::ExchangeControl => None,
            v if v.entry().word.is_empty() => None,
            v => v.entry().roles.iter().any(|(r, _)| *r == Role::What).then_some(Role::What),
        }
    }
}

impl Verb {
    /// Is what the action did a lasting relation its rule names, which later text refers to with no
    /// time ("the creature it haunts", 702.55b; "creatures it devoured", 702.82b; "creatures that
    /// convoked it", 702.51c), rather than an event in a window ("this turn": a creature that
    /// crewed a Vehicle this turn, 702.122b)?
    pub fn relation(self) -> bool {
        matches!(self, Verb::Convoke | Verb::Devour | Verb::Haunt)
    }

    /// Does the action move an object from one zone to another, so that the zone it comes from
    /// can be said ([`Role::From`], "enters from a graveyard", "cast from exile", "exiled from
    /// the battlefield")? A zone change has an origin and a destination (400.1); the verbs that
    /// fix one of them ("dies": from the battlefield, to a graveyard, 700.4; "discard": from the
    /// hand, 701.9a) do not say it.
    pub fn changes_zone(self) -> bool {
        matches!(self, Verb::Enter | Verb::Exile | Verb::Move | Verb::Cast | Verb::Play)
    }

    /// Is what happens to the object a change of state it acquires ("becomes tapped", "becomes
    /// goaded", "becomes crewed", "becomes plotted"), rather than something done to it that "is"
    /// said ("is exiled")? The participle names a state the object has from then on: tapped
    /// (701.26a), suspected (701.60b), goaded (701.15b), saddled (702.171b), crewed (702.122e),
    /// plotted (702.170c).
    pub fn becomes(self) -> bool {
        matches!(self, Verb::Tap | Verb::Untap | Verb::Goad | Verb::Suspect | Verb::Saddle | Verb::Crew | Verb::Plot)
    }

    /// Does the action bring its object onto the battlefield, so that the status it arrives
    /// with can be said ([`Role::As`]: "enters tapped", 614.12)?
    pub fn arrives(self) -> bool {
        self == Verb::Enter
    }

    /// What a role of this action takes; empty when the role is not typed.
    pub fn kinds(self, role: Role) -> &'static [Sort] {
        self.entry().roles.iter().find(|(r, _)| *r == role).map_or(&[], |(_, k)| k)
    }

    /// What must be true of what this action is done to, by its rule.
    pub fn needs(self) -> &'static [Needs] {
        self.entry().needs
    }
}

impl Action {
    /// The action this instruction performs, when the lexicon has it.
    pub fn verb(&self) -> Option<Verb> {
        use crate::action::ObjectAction as O;
        Some(match self {
            Action::Draw(_) | Action::DrawUpTo(_) | Action::DrawAdditional(_) => Verb::Draw,
            Action::Discard(_) => Verb::Discard,
            Action::Mill(_) => Verb::Mill,
            Action::Sacrifice(_) => Verb::Sacrifice,
            Action::Destroy(_) => Verb::Destroy,
            Action::Exile(_) => Verb::Exile,
            Action::Move { .. } => Verb::Move,
            Action::Reveal(_) => Verb::Reveal,
            Action::Until { .. } => Verb::RevealUntil,
            Action::LookAt(_) => Verb::LookAt,
            Action::Search { .. } => Verb::Search,
            Action::Shuffle(_) | Action::ShuffleInto { .. } => Verb::Shuffle,
            Action::Scry(_) => Verb::Scry,
            Action::Surveil(_) => Verb::Surveil,
            Action::Cast { play: false, .. } => Verb::Cast,
            Action::Cast { play: true, .. } | Action::PlayAdditionalLands(_) => Verb::Play,
            Action::SeparateIntoPiles { .. } => Verb::SeparateIntoPiles,
            Action::ChoosePile(_) => Verb::Choose,
            Action::MovePile(_) => Verb::Move,
            Action::Tap(_) | Action::TapOrUntap(_) => Verb::Tap,
            Action::Untap(_) => Verb::Untap,
            Action::Exert(_) => Verb::Exert,
            Action::Regenerate(_) => Verb::Regenerate,
            Action::Fight(_) | Action::FightEachOther => Verb::Fight,
            Action::Flip(_) => Verb::Flip,
            Action::TurnFace { .. } => Verb::TurnFace,
            Action::RemoveFromCombat(_) => Verb::RemoveFromCombat,
            Action::Attach { .. } => Verb::Attach,
            Action::Unattach { .. } => Verb::Unattach,
            Action::GainControl(_) => Verb::GainControl,
            Action::ExchangeControl(_) => Verb::ExchangeControl,
            Action::OnObjects(k, _) => match k {
                O::Transform => Verb::Transform,
                O::Convert => Verb::Convert,
                O::Goad => Verb::Goad,
                O::Detain => Verb::Detain,
                O::Suspect => Verb::Suspect,
                O::Harness => Verb::Harness,
                O::Airbend => Verb::Airbend,
                O::Manifest => Verb::Manifest,
                O::Cloak => Verb::Cloak,
                O::Behold => Verb::Behold,
            },
            Action::ScaleStat { factor: crate::action::Factor::Double, .. } => Verb::DoublePt,
            Action::ScaleStat { factor: crate::action::Factor::Triple, .. } => Verb::TriplePt,
            Action::SwitchPt { .. } => Verb::SwitchPt,
            Action::Counter(_) => Verb::Counter,
            Action::Copy(_) => Verb::Copy,
            Action::ChangeTarget(_) | Action::ChooseNewTargets { .. } => Verb::ChangeTarget,
            Action::DealDamage(_) | Action::DealDivided { .. } => Verb::DealDamage,
            Action::GainLife(_) => Verb::GainLife,
            Action::LoseLife(_) => Verb::LoseLife,
            Action::DoubleLife { .. } => Verb::DoubleLife,
            Action::PutCounters(_) | Action::PutCountersOf { .. } => Verb::PutCounters,
            Action::RemoveCounters { .. } => Verb::RemoveCounters,
            Action::MoveCounters { .. } => Verb::MoveCounters,
            Action::DoubleCounters { .. } => Verb::DoubleCounters,
            Action::Distribute { .. } => Verb::Distribute,
            Action::Create(_) => Verb::Create,
            Action::CreateCopy { .. } => Verb::CreateCopy,
            Action::Clash(_) => Verb::Clash,
            Action::ExchangeLifeTotals { .. } => Verb::ExchangeLifeTotals,
            Action::ExchangeLifeWithStat { .. } => Verb::ExchangeLifeWithStat,
            Action::Keyword(k) => {
                use crate::action::KeywordAction as K;
                match k {
                    K::Proliferate => Verb::Proliferate,
                    K::Connive => Verb::Connive,
                    K::Explore => Verb::Explore,
                    K::BecomeMonarch => Verb::BecomeMonarch,
                    K::WinGame => Verb::WinGame,
                    K::LoseGame => Verb::LoseGame,
                    _ => return None,
                }
            }
            _ => return None,
        })
    }

    /// Who performs this instruction. What is said of a subject while an effect lasts takes the
    /// subject the text names; an object's own deeds ("gets +2/+2") take an object.
    pub fn doer(&self) -> Doer {
        if let Some(v) = self.verb() {
            return v.entry().doer;
        }
        match self {
            Action::AdditionalPhase { .. } => Doer::None,
            // Colors are characteristics of objects (105.2), never of players.
            Action::Becomes(crate::Becoming::Colors { .. }) | Action::Is(crate::Becoming::Colors { .. }) => Doer::Thing,
            // what an object's stats are set to is said of the object; "~'s base power becomes
            // 3" names the object itself and has no other subject (a player may "have" it so)
            Action::Gets { .. } | Action::GetsEither(_) | Action::SetStat { of: None, .. } => Doer::Thing,
            Action::SetStat { of: Some(_), .. } => Doer::Player,
            Action::Gains(_)
            | Action::GainsChoice(_)
            | Action::LosesChoice(_)
            | Action::Becomes(_)
            | Action::Is(_)
            | Action::Cant(_)
            | Action::DoesntUntap { .. } => Doer::Named,
            Action::Static(s) if matches!(**s, crate::StaticAct::CastOnlyAsSorcery | crate::StaticAct::CastAsThoughFlash { .. }) => Doer::Player,
            Action::Keyword(_) | Action::Numbered(..) | Action::Static(_) | Action::LifeTotalBecomes(_) => Doer::Any,
            _ => Doer::Player,
        }
    }
}

/// A reference that does not say what kind of thing it is: "it", "they", "the rest", "~" (the
/// reference resolver finds out), or a commander (a card wherever it is, 903.3).
pub fn open(s: &Selector) -> bool {
    fn commander(f: &Filter) -> bool {
        match f {
            Filter::Atom(Atom::Status(Status::Commander)) => true,
            Filter::All(v) => v.iter().any(commander),
            _ => false,
        }
    }
    s.set.domain == Domain::Object
        || matches!(&s.quant, Quant::Definite(Ref::This | Ref::Granting | Ref::Anaphor(Anaphor::They)))
        || commander(&s.set.filter)
}

/// Whether every alternative names a player (true) or an object (false). A union of
/// objects and players guarantees neither reference kind. Unlike role compatibility,
/// this asks what the phrase actually denotes, not what an open reference could fit.
pub fn denotes_players(s: &Selector) -> Option<bool> {
    let possibilities = s.set.possibilities();
    let first = possibilities.first()?.domain.sort == Sort::Player;
    possibilities.iter().all(|p| (p.domain.sort == Sort::Player) == first).then_some(first)
}

/// Can everything this noun phrase names be of one of these sorts? What the grammar asks of a
/// role's declaration; whether a thing is of the sort now is the engine's question of the same
/// declaration. "Destroy target creature" can; "destroy target player" cannot (a player is never
/// a permanent), nor "can't be blocked by noncreature permanents" (only a creature blocks). "A
/// permanent" or "a Wall" may be a creature, so it can be dealt damage or block. No sorts: the
/// role is not typed. A reference that does not say what it is fits anywhere.
pub fn of_kind(s: &Selector, kinds: &[Sort]) -> bool {
    let fits = |p: &crate::selector::Possibility| {
        kinds.iter().any(|k| match (p.domain.sort, *k) {
            (Sort::Permanent | Sort::Of(_), Sort::Of(t)) => !p.excluded_types.contains(&t),
            (sort, k) => sort.is_a(k),
        })
    };
    kinds.is_empty() || open(s) || s.set.possibilities().iter().all(fits)
}

/// Does the noun phrase say of everything it names that it has this status? "Tapped creatures"
/// does, "tapped or attacking creatures" does not.
fn says(f: &Filter, status: Status) -> bool {
    match f {
        Filter::Atom(Atom::Status(s)) => *s == status,
        Filter::All(v) => v.iter().any(|x| says(x, status)),
        Filter::Any(v) => !v.is_empty() && v.iter().all(|x| says(x, status)),
        Filter::Atom(_) | Filter::Not(_) => false,
    }
}

/// Does the noun phrase say that what the action needs is not so? "Tap target tapped
/// creature" does: only an untapped permanent is tapped (701.26a). Who controls a thing and
/// whose hand a card is in depend on who acts, which the phrase alone does not say.
fn denies(s: &Selector, need: Needs) -> bool {
    match need {
        Needs::Untapped => says(&s.set.filter, Status::Tapped),
        Needs::Tapped => says(&s.set.filter, Status::Untapped),
        Needs::Controlled | Needs::InHand => false,
    }
}

/// Can what this noun phrase names fill this role of the action? It can be of a sort the role
/// takes ([`of_kind`]) and, where the action is done to it, the phrase does not deny what the
/// action's rule needs first.
pub fn fills(s: &Selector, verb: Verb, role: Role) -> bool {
    of_kind(s, verb.kinds(role)) && (role != Role::What || open(s) || !verb.needs().iter().any(|n| denies(s, *n)))
}

/// Can this subject fill the action's doer role? No subject may mean an imperative
/// performed by the controller or a direct effect with no doer.
pub fn can_do(doer: Doer, subject: Option<&Selector>) -> bool {
    let player = |s: &Selector| s.set.domain == Domain::Player;
    match (doer, subject) {
        (Doer::None, subject) => subject.is_none(),
        (Doer::Any, _) => true,
        (Doer::Player, None) => true,
        (Doer::Player, Some(s)) => player(s) || open(s),
        (Doer::Thing, Some(s)) => !player(s) || open(s),
        (Doer::Named, Some(_)) => true,
        (Doer::Thing | Doer::Named, None) => false,
    }
}

impl EventAction {
    /// The action of the lexicon this event is, and what its subject is, when the lexicon has
    /// it. "a creature dies" is `Die` happening to its subject; "you sacrifice a creature" is
    /// `Sacrifice` done by its subject.
    pub fn reading(&self) -> Option<(Verb, SubjectIs)> {
        use EventAction as E;
        use SubjectIs::{Doer as D, Role as R};
        Some(match self {
            E::Deed(d) if d.passive => (d.verb, R(d.verb.passive()?)),
            E::Deed(d) => (d.verb, d.verb.subject()),
            E::PutIntoGraveyard { .. } | E::LeavesZone { .. } => (Verb::Move, R(Role::What)),
            E::Attacks { .. } | E::AttacksUnblocked => (Verb::Attack, D),
            E::Blocks { .. } => (Verb::Block, D),
            E::BecomesBlocked { .. } => (Verb::Block, R(Role::What)),
            E::BecomesTarget { .. } => (Verb::BeTargeted, R(Role::What)),
            E::DealsDamage { .. } => (Verb::DealDamage, D),
            E::IsDealtDamage { .. } => (Verb::DealDamage, R(Role::To)),
            E::GainsLife { .. } => (Verb::GainLife, D),
            E::LosesLife { .. } => (Verb::LoseLife, D),
            E::PutsCounters { .. } => (Verb::PutCounters, D),
            E::CountersPut { .. } => (Verb::PutCounters, R(Role::On)),
            E::CountersRemoved { .. } => (Verb::RemoveCounters, R(Role::From)),
            E::Searches { .. } => (Verb::Search, D),
            E::Shuffles { .. } => (Verb::Shuffle, D),
            E::PutsOntoBattlefield { .. } => (Verb::Move, D),
            E::Transforms { .. } => (Verb::Transform, R(Role::What)),
            E::TurnedFaceUp => (Verb::TurnFace, R(Role::What)),
            E::BecomesLevel(_) => (Verb::BecomeLevel, R(Role::What)),
            _ => return None,
        })
    }

    /// The noun phrases that fill this event's roles besides its subject.
    pub fn fillers(&self) -> Vec<(Role, &Selector)> {
        use EventAction as E;
        match self {
            E::Deed(d) => d
                .args
                .iter()
                .filter_map(|(r, a)| match a {
                    Arg::Noun(s) => Some((*r, s)),
                    Arg::Zone(_) | Arg::Status(_) | Arg::Roll(_) => None,
                })
                .collect(),
            E::Attacks { defender, with, .. } => {
                defender.iter().map(|d| (Role::To, d)).chain(with.iter().map(|w| (Role::With, &**w))).collect()
            }
            E::Blocks { blocked } => blocked.iter().map(|b| (Role::What, &**b)).collect(),
            E::BecomesBlocked { by, .. } => by.iter().map(|b| (Role::By, b)).collect(),
            E::BecomesTarget { of } => of.iter().map(|s| (Role::Of, s.as_ref())).collect(),
            E::DealsDamage { to, .. } => to.iter().flat_map(|t| t.items.iter().map(|t| (Role::To, t))).collect(),
            E::IsDealtDamage { by, .. } => by.iter().map(|b| (Role::By, b)).collect(),
            E::PutsCounters { on, .. } => vec![(Role::On, on)],
            E::PutsOntoBattlefield { what } => vec![(Role::What, what)],
            _ => Vec::new(),
        }
    }
}

impl EventAction {
    /// Can this be the subject of the phrase: the doer of the action, or what it is done to?
    /// An action the lexicon does not have takes any subject.
    pub fn takes_subject(&self, subject: &Selector) -> bool {
        match self.reading() {
            Some((verb, SubjectIs::Doer)) => can_do(verb.entry().doer, Some(subject)),
            Some((verb, SubjectIs::Role(r))) => of_kind(subject, verb.kinds(r)),
            None => true,
        }
    }

    /// Is every role the phrase fills itself filled by something the action takes there?
    pub fn roles_fit(&self) -> bool {
        let Some((verb, _)) = self.reading() else { return true };
        self.fillers().iter().all(|(r, s)| of_kind(s, verb.kinds(*r)))
    }
}

impl EventPattern {
    /// Is every role of every event of the pattern filled by something it takes? "Whenever a
    /// creature dies" is; "whenever a player dies" and "whenever a creature draws a card" are not.
    pub fn typed(&self) -> bool {
        self.actions.iter().all(|a| self.subjects.iter().all(|s| a.takes_subject(s)) && a.roles_fit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r500_8_adding_a_phase_has_no_acting_player_or_object() {
        let action = Action::AdditionalPhase {
            after: crate::action::AfterPhase::This,
            kind: crate::turn::Phase::Combat,
            count: crate::Amount::Const(1),
            then_main: false,
        };
        assert_eq!(action.doer(), Doer::None);
        assert!(can_do(action.doer(), None));
        assert!(!can_do(action.doer(), Some(&Selector::you())));
        assert!(!can_do(action.doer(), Some(&Selector::new(crate::Quant::The, crate::ObjectSet::of(Domain::Permanent)))));
    }

    #[test]
    fn every_entry_cites_a_rule_and_types_each_role_once() {
        for v in Verb::ALL {
            let e = v.entry();
            assert!(e.rule.chars().next().is_some_and(|c| c.is_ascii_digit()), "{v:?} cites a rule");
            for (i, (r, kinds)) in e.roles.iter().enumerate() {
                assert!(!kinds.is_empty(), "{v:?} {r:?} takes something");
                assert!(e.roles[..i].iter().all(|(r2, _)| r2 != r), "{v:?} types {r:?} once");
            }
        }
    }
}
