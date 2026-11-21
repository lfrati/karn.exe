//! Mana symbols and mana costs (CR 107.4, 202).

use crate::vocab::Color;
use serde::Serialize;

/// One mana symbol in a cost (CR 107.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ManaSymbol {
    /// {0}, {1}, {2} … (107.4b)
    Generic(u32),
    /// {X} {Y} {Z} (107.3)
    Var(char),
    /// {W} {U} {B} {R} {G} (107.4a)
    Color(Color),
    /// {C} (107.4c)
    Colorless,
    /// {W/U} … (107.4e)
    Hybrid(Color, Color),
    /// {2/W} … (107.4e)
    TwoHybrid(Color),
    /// {C/W} … (107.4e)
    ColorlessHybrid(Color),
    /// {W/P} … (107.4f)
    Phyrexian(Color),
    /// {W/U/P} … (107.4f)
    HybridPhyrexian(Color, Color),
    /// {S} (107.4h)
    Snow,
}

/// A mana cost: symbols in printed order ("{2}{U}{U}").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ManaCost(pub Vec<ManaSymbol>);

impl ManaSymbol {
    /// The complete nonhybrid alternatives for a symbol paid solely with ordinary mana
    /// types (107.4a–e, 601.2b). Snow needs source provenance; Phyrexian needs a life choice.
    pub fn mana_payments(self) -> Option<(Self, Option<Self>)> {
        Some(match self {
            Self::Generic(_) | Self::Color(_) | Self::Colorless => (self, None),
            Self::Hybrid(a, b) => (Self::Color(a), Some(Self::Color(b))),
            Self::TwoHybrid(c) => (Self::Color(c), Some(Self::Generic(2))),
            Self::ColorlessHybrid(c) => (Self::Color(c), Some(Self::Colorless)),
            _ => return None,
        })
    }

    fn color_letter(c: char) -> Option<Color> {
        Some(match c {
            'W' => Color::White,
            'U' => Color::Blue,
            'B' => Color::Black,
            'R' => Color::Red,
            'G' => Color::Green,
            _ => return None,
        })
    }

    fn letter(c: Color) -> char {
        match c {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        }
    }

    /// From the inside of a `{…}` symbol; None for non-mana symbols ({T}, {Q}, {E}, {P} …).
    pub fn parse(s: &str) -> Option<ManaSymbol> {
        let parts: Vec<&str> = s.split('/').collect();
        let one = |p: &str| p.chars().next().filter(|_| p.chars().count() == 1);
        let col = |p: &str| one(p).and_then(ManaSymbol::color_letter);
        Some(match parts.as_slice() {
            [n] if n.chars().all(|c| c.is_ascii_digit()) => ManaSymbol::Generic(n.parse().ok()?),
            [v] if matches!(*v, "X" | "Y" | "Z") => ManaSymbol::Var(one(v)?),
            ["C"] => ManaSymbol::Colorless,
            ["S"] => ManaSymbol::Snow,
            [c] => ManaSymbol::Color(col(c)?),
            ["2", c] => ManaSymbol::TwoHybrid(col(c)?),
            ["C", c] => ManaSymbol::ColorlessHybrid(col(c)?),
            [c, "P"] => ManaSymbol::Phyrexian(col(c)?),
            [a, b] => ManaSymbol::Hybrid(col(a)?, col(b)?),
            [a, b, "P"] => ManaSymbol::HybridPhyrexian(col(a)?, col(b)?),
            _ => return None,
        })
    }

    /// The inside of the printed `{…}`.
    pub fn text(self) -> String {
        let l = ManaSymbol::letter;
        match self {
            ManaSymbol::Generic(n) => n.to_string(),
            ManaSymbol::Var(v) => v.to_string(),
            ManaSymbol::Color(c) => l(c).to_string(),
            ManaSymbol::Colorless => "C".into(),
            ManaSymbol::Hybrid(a, b) => format!("{}/{}", l(a), l(b)),
            ManaSymbol::TwoHybrid(c) => format!("2/{}", l(c)),
            ManaSymbol::ColorlessHybrid(c) => format!("C/{}", l(c)),
            ManaSymbol::Phyrexian(c) => format!("{}/P", l(c)),
            ManaSymbol::HybridPhyrexian(a, b) => format!("{}/{}/P", l(a), l(b)),
            ManaSymbol::Snow => "S".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip() {
        for s in ["0", "12", "X", "W", "C", "S", "W/U", "2/B", "C/G", "R/P", "G/U/P"] {
            assert_eq!(ManaSymbol::parse(s).map(ManaSymbol::text).as_deref(), Some(s));
        }
        for s in ["T", "Q", "E", "P", "H", "W/Q"] {
            assert_eq!(ManaSymbol::parse(s), None, "{s}");
        }
    }
}

/// The mana an "add" produces (CR 106.1, 106.3, 605): how much, of what type, or one of
/// several such.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ManaSpec {
    /// "{G}", "{C}{C}", "six {G}": these mana. A number before a symbol is that many of it
    /// (107.4a, 107.4c), one meaning with the run of symbols.
    Symbols(ManaCost),
    /// "{R} or {G}", "{W}, {U}, or {B}", "{W} or one mana of the chosen color", "X {G} or X
    /// {W}": one of these, as the player adding the mana chooses. No alternative is itself a
    /// choice of several.
    OneOf(Vec<ManaSpec>),
    /// An amount of mana said by its type: "one mana of any color", "X mana of any one color",
    /// "two mana in any combination of colors", "an amount of mana of that color equal to your
    /// devotion to that color".
    Of { amount: crate::amount::Amount, color: ManaColor },
    /// An amount of one symbol's mana that the text does not fix: "X {R}", "an amount of {G}
    /// equal to ~'s power", "that much {C}".
    AmountOf { symbol: ManaSymbol, amount: crate::amount::Amount },
}

/// What type each mana of "N mana …" is (CR 106.1b), and how far the player adding it chooses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ManaColor {
    /// Each mana is of one of these types, chosen for it (608.2d). Of one mana: "of any
    /// color", "of any of the exiled card's colors". Of several: "in any combination of
    /// colors", "in any combination of {R} and/or {G}", "in any combination of its colors".
    Any(ManaTypes),
    /// "of any one color": all of one color, chosen once.
    AnyOne,
    /// "of the chosen color" (607.2d).
    Chosen,
    /// "of that color": the color the instruction before named ("Choose a color. Add one mana of
    /// that color.", "For each color among permanents you control, add one mana of that color.").
    /// Not the linked "chosen" value of 607.2d, which another ability made.
    That,
    /// "of that type": the type chosen under the sentence before ("Add one mana of any type
    /// that a land you control could produce. If …, add three mana of that type instead.").
    ThatType,
    /// "of different colors".
    Different,
}

/// The types of mana a choice is made among (CR 106.1a colors, 106.1b types). A set that turns
/// out empty produces no mana (106.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ManaTypes {
    /// The five colors (106.1a).
    Colors,
    /// "{R} and/or {G}": these.
    Symbols(Vec<ManaSymbol>),
    /// "in your commander's color identity" (903.4).
    CommanderIdentity,
    /// "the exiled card's colors", "its colors": the colors those objects are (105.2).
    ColorsOf(Box<crate::selector::Selector>),
    /// "among legendary permanents you control": the colors any object of the set is (105.2).
    ColorsAmong(Box<crate::selector::ObjectSet>),
    /// "that land produced", "that a land an opponent controls could produce" (106.7): `types`:
    /// types (colorless included) rather than colors; `could`: what the objects could produce
    /// rather than what they did.
    Produced { types: bool, by: Box<crate::selector::Selector>, could: bool },
}

/// What mana is spent on (CR 106.6, 601.2h): the one slot of every rule about spending mana,
/// whether it restricts the mana produced ("Spend this mana only to cast a creature spell"), is
/// asked about later ("if that mana is spent on …"), or bends the rule ("spend mana as though it
/// were mana of any color to cast that spell").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SpendOn {
    /// "cast a creature spell", "cast artifact spells", "cast that spell" (601.2): spells said by
    /// a selector or by a bare plural set.
    Cast(crate::action::Coord<crate::statics::Who>),
    /// "activate those abilities", "activate this ability", "activate equip abilities" (602.2):
    /// the abilities themselves, named or said by what they are (a keyword's, 702; a kind, 113.3).
    Activate(crate::statics::Who),
    /// "activate abilities", "activate abilities of artifacts": abilities, said by their source.
    ActivateAbilities(Option<crate::selector::ObjectSet>),
    /// "activate an ability", "activate an ability of an artifact source".
    ActivateAnAbility(Option<crate::selector::Selector>),
    /// "on X": the part of a cost that is X (107.3).
    X,
}

/// Mana in a player's mana pool (CR 106.4): what an effect that keeps or takes it away is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Pooled {
    /// "this mana": what the text itself produced (106.6).
    This,
    /// "unspent mana", "unspent green mana": whatever remains in the pool (106.4).
    Unspent(Option<Color>),
}

/// Which mana a rule about spending names by what it is, rather than as "this mana" (106.6):
/// "Spend only black mana on X", "Spend only mana produced by basic lands to cast ~".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SpendableMana {
    /// "black mana", "black and/or red mana".
    Colors(Vec<Color>),
    /// "colored mana": any color but not colorless (105.1).
    Colored,
    /// "mana produced by basic lands": by the source that made it (106.6).
    ProducedBy(crate::selector::ObjectSet),
    /// "mana of the chosen color" (607.2d).
    Chosen,
}
