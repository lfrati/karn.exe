//! Closed vocabularies from the Comprehensive Rules.
//!
//! Small stable sets are enums. The large lists (463 subtypes, 194 keyword abilities) are indexes
//! into `data/cr_lists.json` (written by `scripts/inventory.py` from the CR), serialized by name.
//! Deferred (team formats, ante, subgames and draft procedures): conspiracy, dungeon, phenomenon, plane,
//! scheme, vanguard card types and the ongoing supertype.

use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::sync::OnceLock;

static LISTS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// One CR list by its key in `cr_lists.json`, with typographic apostrophes folded to the
/// straight ones Oracle text prints ("Urza’s" → "Urza's").
pub fn list(key: &str) -> &'static [String] {
    LISTS
        .get_or_init(|| {
            let raw: HashMap<String, Vec<String>> =
                serde_json::from_str(include_str!("../data/cr_lists.json")).expect("data/cr_lists.json is valid");
            raw.into_iter().map(|(k, v)| (k, v.into_iter().map(|s| s.replace('’', "'")).collect())).collect()
        })
        .get(key)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("no CR list {key:?}"))
}

macro_rules! word_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $word:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The word as Oracle text prints it mid-sentence.
            pub fn word(self) -> &'static str {
                match self { $($name::$variant => $word),+ }
            }

            pub fn from_word(w: &str) -> Option<$name> {
                match w { $($word => Some($name::$variant),)+ _ => None }
            }
        }
    };
}

word_enum! {
    /// CR 300.1, minus the currently deferred card types.
    CardType {
        Artifact = "artifact", Battle = "battle", Creature = "creature", Enchantment = "enchantment",
        Instant = "instant", Kindred = "kindred", Land = "land", Planeswalker = "planeswalker",
        Sorcery = "sorcery",
    }
}

impl CardType {
    /// The six permanent types (110.4). Kindred depends on the card's other types;
    /// merely being a card type does not make it a permanent type.
    pub fn permanent(self) -> bool {
        matches!(self, Self::Artifact | Self::Battle | Self::Creature | Self::Enchantment | Self::Land | Self::Planeswalker)
    }
}

word_enum! {
    /// CR 205.4a, minus ongoing.
    Supertype { Basic = "basic", Legendary = "legendary", Snow = "snow", World = "world" }
}

word_enum! {
    /// CR 105.1.
    Color { White = "white", Blue = "blue", Black = "black", Red = "red", Green = "green" }
}

word_enum! {
    /// CR 400.1 (ante is deferred).
    Zone {
        Library = "library", Hand = "hand", Battlefield = "battlefield", Graveyard = "graveyard",
        Stack = "stack", Exile = "exile", Command = "command zone",
    }
}

/// Which CR list a subtype comes from (CR 205.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SubtypeKind {
    Artifact,
    Enchantment,
    Land,
    Planeswalker,
    Spell,
    Creature,
    Battle,
}

const SUBTYPE_LISTS: &[(&str, SubtypeKind)] = &[
    ("artifact types", SubtypeKind::Artifact),
    ("enchantment types", SubtypeKind::Enchantment),
    ("land types", SubtypeKind::Land),
    ("planeswalker types", SubtypeKind::Planeswalker),
    ("spell types", SubtypeKind::Spell),
    ("creature types", SubtypeKind::Creature),
    ("battle types", SubtypeKind::Battle),
];

struct Table {
    names: Vec<&'static str>,
    index: HashMap<&'static str, u16>,
}

fn table(lists: impl IntoIterator<Item = &'static str>) -> Table {
    let names: Vec<&'static str> = lists.into_iter().flat_map(|k| list(k).iter().map(String::as_str)).collect();
    let index = names.iter().enumerate().map(|(i, n)| (*n, i as u16)).collect();
    Table { names, index }
}

struct Subtypes {
    words: Table,
    kinds: Vec<SubtypeKind>,
    groups: Vec<(SubtypeKind, Vec<Subtype>)>,
}

fn subtypes() -> &'static Subtypes {
    static T: OnceLock<Subtypes> = OnceLock::new();
    T.get_or_init(|| {
        let words = table(SUBTYPE_LISTS.iter().map(|(k, _)| *k));
        let mut kinds = Vec::with_capacity(words.names.len());
        let groups = SUBTYPE_LISTS.iter().map(|(key, kind)| {
            let first = kinds.len();
            kinds.extend(std::iter::repeat_n(*kind, list(key).len()));
            (*kind, (first..kinds.len()).map(|i| Subtype(i as u16)).collect())
        }).collect();
        Subtypes { words, kinds, groups }
    })
}

/// A subtype (CR 205.3): "Goblin", "Equipment", "Aura", "Forest", "Ajani", "Adventure".
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Subtype(u16);

impl Subtype {
    pub fn from_name(name: &str) -> Option<Subtype> {
        subtypes().words.index.get(name).copied().map(Subtype)
    }

    pub fn name(self) -> &'static str {
        subtypes().words.names[self.0 as usize]
    }

    pub fn kind(self) -> SubtypeKind {
        subtypes().kinds[self.0 as usize]
    }

    /// The immutable CR 205.3 vocabulary of one subtype kind, in rulebook order.
    pub fn of_kind(kind: SubtypeKind) -> &'static [Subtype] {
        &subtypes().groups.iter().find(|(k, _)| *k == kind).expect("every subtype kind has a CR list").1
    }

    pub fn all() -> impl Iterator<Item = Subtype> {
        (0..subtypes().words.names.len() as u16).map(Subtype)
    }

    /// The basic land types, in the order of their colors' mana symbols (305.6).
    pub fn basic_land_types() -> &'static [Subtype; 5] {
        static T: OnceLock<[Subtype; 5]> = OnceLock::new();
        T.get_or_init(|| ["Plains", "Island", "Swamp", "Mountain", "Forest"].map(Subtype::named))
    }

    /// Is this one of the five basic land types (305.6)?
    pub fn is_basic_land_type(self) -> bool {
        Subtype::basic_land_types().contains(&self)
    }

    fn named(name: &str) -> Subtype {
        Subtype::from_name(name).unwrap_or_else(|| panic!("{name} is a subtype of the pinned rulebook (205.3)"))
    }
}

/// Subtypes that rules give behavior, by name in the pinned vocabulary.
macro_rules! named_subtypes {
    ($($(#[$doc:meta])* $method:ident = $name:literal;)*) => {
        impl Subtype {
            $($(#[$doc])* pub fn $method() -> Subtype {
                static S: OnceLock<Subtype> = OnceLock::new();
                *S.get_or_init(|| Subtype::named($name))
            })*
        }
    };
}

named_subtypes! {
    /// Equipment (301.5).
    equipment = "Equipment";
    /// Aura (303.4).
    aura = "Aura";
    /// Role, an Aura subtype (303.7).
    role = "Role";
    /// Saga (714).
    saga = "Saga";
    /// Class (716).
    class = "Class";
    /// Room, a split permanent with two doors (709.5).
    room = "Room";
    /// Adventure, a spell type (715).
    adventure = "Adventure";
    /// Omen, a spell type (720).
    omen = "Omen";
}

fn keywords() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| table(["keyword abilities (702)"]))
}

/// A keyword ability (CR 702): "Flying", "First Strike", "Ward".
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Keyword(u16);

impl Keyword {
    /// By name, case-insensitively ("first strike").
    pub fn from_name(name: &str) -> Option<Keyword> {
        let t = keywords();
        t.index
            .get(name)
            .copied()
            .or_else(|| t.names.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|i| i as u16))
            .map(Keyword)
    }

    /// The CR heading spelling ("First Strike").
    pub fn name(self) -> &'static str {
        keywords().names[self.0 as usize]
    }

    pub fn all() -> impl Iterator<Item = Keyword> {
        (0..keywords().names.len() as u16).map(Keyword)
    }

    /// Does this keyword give its object a cost, so that "its echo cost" names something? Those
    /// whose own 702 section writes one ("Equip [cost]", 702.6a; "Suspend N—[cost]", 702.62a)
    /// or names one ("the crew cost of a Vehicle", 702.122d).
    pub fn has_cost(self) -> bool {
        static C: OnceLock<Vec<Keyword>> = OnceLock::new();
        C.get_or_init(|| list("keyword abilities with a cost (702)").iter().filter_map(|n| Keyword::from_name(n)).collect()).contains(&self)
    }
}

macro_rules! by_name {
    ($t:ty) => {
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({:?})", stringify!($t), self.name())
            }
        }
        impl Serialize for $t {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }
    };
}
by_name!(Subtype);
by_name!(Keyword);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocab() {
        assert_eq!(Subtype::from_name("Goblin").map(Subtype::kind), Some(SubtypeKind::Creature));
        assert_eq!(Subtype::from_name("Aura").map(Subtype::kind), Some(SubtypeKind::Enchantment));
        assert_eq!(Subtype::from_name("Urza's").map(Subtype::kind), Some(SubtypeKind::Land));
        assert_eq!(Subtype::all().count(), 463);
        assert_eq!(Keyword::from_name("first strike").map(Keyword::name), Some("First Strike"));
        assert_eq!(CardType::from_word("creature"), Some(CardType::Creature));
        assert_eq!(serde_json::to_string(&Subtype::from_name("Elf")).unwrap(), "\"Elf\"");
    }

    #[test]
    fn r205_3_each_subtype_kind_is_exactly_its_rulebook_list() {
        let mut all = Vec::new();
        for (key, kind) in SUBTYPE_LISTS {
            let group = Subtype::of_kind(*kind);
            assert_eq!(group.iter().map(|s| s.name()).collect::<Vec<_>>(), list(key));
            assert!(group.iter().all(|s| s.kind() == *kind));
            all.extend_from_slice(group);
        }
        assert_eq!(all, Subtype::all().collect::<Vec<_>>());
        // each name is one identity, so comparing identities is comparing names
        assert!(Subtype::all().all(|s| Subtype::from_name(s.name()) == Some(s)));
        for (s, kind) in [(Subtype::equipment(), SubtypeKind::Artifact), (Subtype::aura(), SubtypeKind::Enchantment),
            (Subtype::role(), SubtypeKind::Enchantment), (Subtype::saga(), SubtypeKind::Enchantment),
            (Subtype::class(), SubtypeKind::Enchantment), (Subtype::room(), SubtypeKind::Enchantment),
            (Subtype::adventure(), SubtypeKind::Spell), (Subtype::omen(), SubtypeKind::Spell)] {
            assert_eq!(s.kind(), kind, "{}", s.name());
        }
        assert_eq!(Subtype::basic_land_types().map(Subtype::name), ["Plains", "Island", "Swamp", "Mountain", "Forest"]);
        assert!(Subtype::basic_land_types().iter().all(|s| s.kind() == SubtypeKind::Land));
    }

    #[test]
    fn r702_keywords_with_a_cost() {
        let k = |n: &str| Keyword::from_name(n).unwrap_or_else(|| panic!("{n}"));
        let listed = list("keyword abilities with a cost (702)");
        assert!(listed.iter().all(|n| Keyword::from_name(n).is_some()), "every listed name is a keyword");
        for n in ["Equip", "Cumulative Upkeep", "Echo", "Suspend", "Splice", "Crew", "Foretell", "Megamorph", "Warp"] {
            assert!(k(n).has_cost(), "{n}");
        }
        for n in ["Flying", "Flanking", "Infect", "Menace", "Partner", "Station"] {
            assert!(!k(n).has_cost(), "{n}");
        }
    }
}
