//! Token descriptions: what "create …" makes (CR 111).

use crate::amount::Amount;
use crate::cost::Cost;
use crate::mana::ManaCost;
use crate::selector::{Filter, ObjectSet};
use crate::vocab::{CardType, Color, Keyword, Subtype, Supertype};
use serde::Serialize;

/// "a 1/1 white Spirit creature token with flying", "two Treasure tokens", "a tapped Powerstone
/// token".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TokenSpec {
    pub count: Amount,
    /// Created tapped ("a tapped Treasure token").
    pub tapped: bool,
    pub kind: TokenKind,
    /// "that's tapped and attacking", "that's blocking that creature": put onto the battlefield
    /// in combat.
    pub combat: Option<Combat>,
    /// "a Sorcerer Role token attached to target creature" (303.4f, an Aura created attached).
    pub attached_to: Option<Box<crate::selector::Selector>>,
}

/// How a token enters combat (506.3e, 508.4): it is attacking or blocking without having been
/// declared an attacker or blocker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Combat {
    /// "that's attacking" (its controller chooses what, 508.4), "that's attacking that opponent"
    /// (the effect specifies it).
    Attacking(Option<Box<crate::selector::Selector>>),
    /// "that's blocking that creature" (506.3e): the attacking creature it blocks.
    Blocking(Box<crate::selector::Selector>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum TokenKind {
    /// A predefined token (CR 111.10): its characteristics come from the rules.
    Predefined(Predefined),
    Described(TokenDesc),
    /// "create five of those tokens instead": the token the text described before.
    Those,
}

/// A token whose characteristics the text spells out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TokenDesc {
    pub supertypes: Vec<Supertype>,
    pub pt: Option<(Pt, Pt)>,
    /// None when the text names no color; `Some(vec![])` is "colorless".
    pub colors: Option<Vec<Color>>,
    pub subtypes: Vec<Subtype>,
    pub types: Vec<CardType>,
    pub name: Option<String>,
    /// "with flying", "with haste and \"At the beginning of the end step, sacrifice ~.\"".
    pub abilities: Vec<crate::action::Granted>,
}

/// One printed power or toughness.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Pt {
    Const(u32),
    X,
}

/// A keyword with its parameter, if it has one ("toxic 1", "equip {2}", "enchant creature").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct KeywordInstance {
    pub keyword: Keyword,
    pub param: KeywordParam,
}

/// A keyword's parameter, by shape (CR 702 gives each keyword's).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum KeywordParam {
    None,
    /// "toxic 2", "crew 3", "annihilator 2".
    Number(Amount),
    /// "equip {2}", "kicker {2}{G}", "cycling {3}".
    Mana(ManaCost),
    /// "kicker {1}{U} and/or {B}" (702.33b), "cumulative upkeep {W} or {U}": several mana costs
    /// joined as costs are (two or more items).
    ManaList(crate::action::Coord<ManaCost>),
    /// "ward—Pay 3 life.", "cumulative upkeep—Pay 1 life.": a cost that is not just mana.
    Cost(Cost),
    /// "suspend 2—{1}{W}" (702.62a).
    NumberAndMana(Amount, ManaCost),
    /// "enchant creature you control", "enchant player" (702.5a).
    Enchant(ObjectSet),
    /// "protection from black and from red", "protection from everything" (702.16a, 702.16g);
    /// also "hexproof from black" (702.11d). Several qualities are joined as anything is: "and"
    /// is all of them, "or" is one the controller chooses.
    Protection(crate::action::Coord<Quality>),
    /// "affinity for artifacts" (702.41a).
    Affinity(ObjectSet),
    /// "swampwalk", "snow forestwalk", "legendary landwalk": the type in "[type]walk" (702.14a),
    /// any supertypes, card types and a subtype.
    Landwalk(Filter),
    /// "plainscycling {2}", "basic landcycling {1}{B}" (702.29e): the type of card searched for.
    Typecycling(Filter, ManaCost),
    /// "Goblin offering", "artifact offering" (702.48a): the quality of the permanent sacrificed.
    Offering(Filter),
    /// "devour artifact 1", "devour Food 3" (702.82c): the quality of the permanents sacrificed,
    /// and N.
    DevourQuality(Filter, Amount),
    /// "splice onto Arcane {1}{U}", "splice onto Arcane—Exile four cards from your graveyard."
    /// (702.47a): the cost is any cost, written after a dash unless it is only mana.
    Splice(ObjectSet, Cost),
    /// "gift a Food", "gift a card", "gift a tapped Fish" (702.174a): what the chosen opponent gets.
    Gift(GiftOf),
    /// "prototype {1}{B} — 1/1" (702.160a).
    Prototype(ManaCost, Pt, Pt),
    /// "partner—Friends forever": partner with the cards that have the same text (702.124i).
    PartnerVariant(String),
    /// "partner with Ley Weaver" (702.124j).
    PartnerWith(String),
    /// "choose a Background" (702.124k).
    ChooseBackground,
    /// "Doctor's companion" (702.124m).
    DoctorsCompanion,
    /// "craft with artifact {3}{W}" (702.167a).
    Craft(ObjectSet, ManaCost),
    /// "craft with one or more creatures {5}", "craft with one or more {5}" (any permanents or
    /// cards: None).
    CraftMany(Option<ObjectSet>, ManaCost),
    /// "equip legendary creature {3}" (702.6c).
    EquipQuality(ObjectSet, ManaCost),
    /// "champion a creature", "champion an Elemental" (702.72a).
    Champion(ObjectSet),
    /// "bands with other legendary creatures" (702.22b; the keyword is banding).
    BandsWithOther(ObjectSet),
}

/// What a gift gives (702.174).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum GiftOf {
    /// "a card": the player draws a card.
    Card,
    /// "an extra turn".
    ExtraTurn,
    /// "a Food", "a Treasure", "a tapped Fish", "an Octopus": a token, by its name.
    Token { tapped: bool, name: String },
}

/// A protection quality (702.16a).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Quality {
    Color(Color),
    /// "multicolored".
    Multicolored,
    /// "monocolored".
    Monocolored,
    /// "everything" (702.16j).
    Everything,
    /// "the chosen color".
    ChosenColor,
    /// "the chosen player" (702.16k).
    ChosenPlayer,
    /// "the color of your choice".
    ColorOfChoice,
    /// "the color of its controller's choice".
    ColorOfControllersChoice,
    /// "each of its colors".
    EachOfItsColors,
    /// "each color among permanents you control".
    EachColorAmong(ObjectSet),
    /// "that player", "each of your opponents": a player, and everything that player controls
    /// or owns as a source.
    Player(Box<crate::selector::Selector>),
    /// "each color" (702.16h).
    EachColor,
    /// "snow": a supertype alone (702.16a: a quality can be a supertype).
    Filter(Filter),
    /// "artifacts", "creatures", "Humans", "instants": objects with these characteristics.
    Set(ObjectSet),
}

macro_rules! predefined {
    ($($variant:ident = $name:literal),+ $(,)?) => {
        /// The predefined tokens of CR 111.10.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        pub enum Predefined { $($variant),+ }

        impl Predefined {
            pub const ALL: &'static [Predefined] = &[$(Predefined::$variant),+];

            /// As printed before "token": "Treasure", "Wicked Role".
            pub fn name(self) -> &'static str {
                match self { $(Predefined::$variant => $name),+ }
            }
        }
    };
}

predefined! {
    Treasure = "Treasure", Food = "Food", Gold = "Gold", Walker = "Walker", Shard = "Shard",
    Clue = "Clue", Blood = "Blood", Powerstone = "Powerstone", Incubator = "Incubator",
    CursedRole = "Cursed Role", MonsterRole = "Monster Role", RoyalRole = "Royal Role",
    SorcererRole = "Sorcerer Role", VirtuousRole = "Virtuous Role", WickedRole = "Wicked Role",
    YoungHeroRole = "Young Hero Role", Map = "Map", Junk = "Junk", Lander = "Lander",
    Mutagen = "Mutagen", Vibranium = "Vibranium", Heartwood = "Heartwood",
}
