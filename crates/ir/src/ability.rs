//! Abilities: one printed ability as a whole (CR 113.3), and a card face as its abilities.
//!
//! The parts come from the sub-grammars (triggers, costs, effects, keywords); an ability only
//! says how they combine.

use crate::cost::Cost;
use crate::effect::Effect;
use crate::event::Trigger;
use crate::mana::ManaCost;
use crate::condition::Condition;
use crate::token::{KeywordInstance, Pt};
use crate::vocab::Keyword;
use serde::Serialize;

/// The companion conditions that are not "each … card in your starting deck has …" (702.139).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CompanionRule {
    /// "Each nonland card in your starting deck has a different name."
    DifferentNames,
    /// "Your starting deck contains only cards with even mana values."
    EvenManaValues,
    /// "Your starting deck contains only cards with odd mana values and land cards."
    OddManaValuesAndLands,
    /// "Your starting deck contains only cards with mana value 3 or greater and land cards."
    ManaValueThreeOrGreaterAndLands,
    /// "Each creature card in your starting deck is a Cat, Elemental, Nightmare, Dinosaur, or
    /// Beast card."
    CreatureTypesOfKaheera,
    /// "Each permanent card in your starting deck has an activated ability."
    ActivatedAbilities,
    /// "Each nonland card in your starting deck shares a card type."
    SharedCardType,
    /// "Your starting deck contains at least twenty cards more than the minimum deck size."
    TwentyMoreCards,
    /// "No card in your starting deck has more than one of the same mana symbol in its mana
    /// cost."
    NoRepeatedManaSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Ability {
    /// A universally quantified starting-deck condition (702.139a-b).
    Companion { each: crate::selector::ObjectSet, has: crate::selector::Filter },
    /// A starting-deck condition of another shape (702.139a-b): each is one companion's own.
    CompanionRule(CompanionRule),
    /// Keyword abilities on one row: "Flying, vigilance", "Toxic 2" (702).
    Keywords(Vec<KeywordInstance>),
    /// Keyword abilities followed by sentences about them: "Equip {2}. Activate only once each
    /// turn.", "Suspend X—{X}{W}{W}. X can't be 0." (702).
    KeywordsWith { keywords: Vec<KeywordInstance>, rider: Effect },
    /// "When ~ enters, draw a card." (603.1)
    Triggered { trigger: Trigger, effect: Effect },
    /// "{T}: Add {G}.", "+1: Draw a card." (602.1, 606.4): the cost, then the effect with any
    /// "Activate only …" restrictions among its sentences.
    Activated { cost: Cost, effect: Effect },
    /// "I, II — Draw a card." (714.2b)
    Chapter { chapters: Vec<u32>, effect: Effect },
    /// "Exhaust — {2}{R}: Put a +1/+1 counter on ~.", "Max speed — You may cast ~ from your
    /// graveyard." (702.177a, 702.178a, 702.193a, 702.142a, 702.169a): a keyword that adds rules
    /// to the ability after its dash.
    Prefixed { keyword: Keyword, ability: Box<Ability> },
    /// "To solve — You've gained 5 or more life this turn." (719.3a)
    ToSolve(Condition),
    /// "{3}{U}: Level 2" (716.2a); the abilities of the level are the rows that follow.
    ClassLevel { cost: ManaCost, level: u32 },
    /// "LEVEL 1-3", "LEVEL 4+" (711.2a, 711.2b); `to` is None for "N+".
    LevelBand { from: u32, to: Option<u32> },
    /// The power and toughness row under a level band: "3/3" (711.2a).
    LevelPt(Pt, Pt),
    /// "9+ | Flying": what a station card has at a charge-counter threshold (702.184).
    Station { threshold: u32, ability: Box<Ability> },
    /// Spell text (an instant's or sorcery's effect, 113.3a) or a static ability (113.3d): the
    /// row's sentences.
    Text(Effect),
}

/// A mode row under a modal ability (700.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Mode {
    /// "• Draw a card."
    Bullet(Effect),
    /// "+ {1} — Draw a card.", "• {2} — ~ deals 2 damage to each creature.": a mode with its own
    /// cost, paid as an additional cost when the mode is chosen. Spree (702.172a) chooses one or
    /// more, Tiered (702.183a) chooses one; which one is the keyword on the card, not the row.
    /// The plus sign and the bullet are the same row (702.172b: no rules meaning).
    Costed { cost: ManaCost, effect: Effect },
    /// "• Khans — At the beginning of combat on your turn, …": an ability that exists while the
    /// anchor word was the one chosen as the permanent entered (614.12c); linked to the choice
    /// among words (607.2m).
    Anchor { word: String, effect: Effect },
    /// "{P}{P} — Draw a card." (pawprint modes, 700.2i)
    Pawprint { paws: u32, effect: Effect },
    /// "1—9 | Draw a card.", "20 | …", "15+ | …": a row of the results table of a die roll
    /// (706.3a), "If the result was in this range, [effect]."
    Result { range: ResultRange, effect: Effect },
}

/// The results of a die roll a table row covers (706.3a): a single number (both ends equal), a
/// range "N1–N2", or "N+" (no upper end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ResultRange {
    pub from: u32,
    pub to: Option<u32>,
}

/// One ability block: the ability and its mode rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Block {
    pub ability: Ability,
    pub modes: Vec<Mode>,
}
