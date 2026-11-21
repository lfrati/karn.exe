//! Rule-defined tokens, including the predefined tokens (CR 111.10): "Some effects instruct a player to create a predefined
//! token. These effects use the definition below to determine the characteristics the token is
//! created with."
//!
//! A definition is the token's text (111.3), read once by the grammar from the rule's own words
//! into an ordinary [`CardDef`] and admitted as any card is ([`crate::play::check`]). A token
//! made from one carries the kind in its characteristics ([`crate::state::Chars::predefined`]);
//! [`crate::Game::def`] finds the definition from it, so the token's abilities are activated,
//! paid for and resolved by the same machinery as a card's. The token is still no card
//! (111.6): it has no card index.

use crate::card::CardDef;
use crate::{Res, Unsupported};
use ir::Predefined;
use ir::token::{TokenKind, Pt};
use crate::select::name;
use crate::state::Chars;
use std::sync::OnceLock;

/// A predefined token the engine runs, as its rule words it: "111.10a A Treasure token is a
/// colorless Treasure artifact token with “{T}, Sacrifice this token: Add one mana of any
/// color.”"
#[derive(Debug, Clone, Copy)]
pub struct Defined {
    pub kind: Predefined,
    /// The rule that defines it.
    pub rule: &'static str,
    /// What kind of permanent it is.
    pub form: Form,
    /// The abilities quoted in the rule, each a sentence: the token's text.
    pub text: &'static [&'static str],
}

/// The two shapes 111.10 defines a token in.
#[derive(Debug, Clone, Copy)]
pub enum Form {
    /// A colorless artifact of this subtype (205.3g), named for it (111.4).
    Artifact { subtype: &'static str },
    /// A colorless Aura Role enchantment with enchant creature, named this (303.7).
    Role { named: &'static str },
}

/// The predefined tokens the engine creates. Every other kind of 111.10 is refused.
pub const DEFINED: &[Defined] = &[
    Defined { kind: Predefined::Treasure, rule: "111.10a", form: Form::Artifact { subtype: "Treasure" }, text: &["{T}, Sacrifice this token: Add one mana of any color."] },
    Defined { kind: Predefined::Food, rule: "111.10b", form: Form::Artifact { subtype: "Food" }, text: &["{2}, {T}, Sacrifice this token: You gain 3 life."] },
    Defined { kind: Predefined::Clue, rule: "111.10f", form: Form::Artifact { subtype: "Clue" }, text: &["{2}, Sacrifice this token: Draw a card."] },
    Defined { kind: Predefined::Blood, rule: "111.10g", form: Form::Artifact { subtype: "Blood" }, text: &["{1}, {T}, Discard a card, Sacrifice this token: Draw a card."] },
    Defined { kind: Predefined::CursedRole, rule: "111.10j", form: Form::Role { named: "Cursed" }, text: &["Enchanted creature has base power and toughness 1/1."] },
    Defined { kind: Predefined::MonsterRole, rule: "111.10k", form: Form::Role { named: "Monster" }, text: &["Enchanted creature gets +1/+1 and has trample."] },
    Defined { kind: Predefined::SorcererRole, rule: "111.10n", form: Form::Role { named: "Sorcerer" }, text: &["Enchanted creature gets +1/+1 and has ‘Whenever this creature attacks, scry 1.’"] },
    Defined { kind: Predefined::VirtuousRole, rule: "111.10p", form: Form::Role { named: "Virtuous" }, text: &["Enchanted creature gets +1/+1 for each enchantment you control."] },
    Defined { kind: Predefined::WickedRole, rule: "111.10q", form: Form::Role { named: "Wicked" }, text: &["Enchanted creature gets +1/+1.", "When this token is put into a graveyard from the battlefield, each opponent loses 1 life."] },
    Defined { kind: Predefined::YoungHeroRole, rule: "111.10r", form: Form::Role { named: "Young Hero" }, text: &["Enchanted creature has ‘Whenever this creature attacks, if its toughness is 3 or less, put a +1/+1 counter on it.’"] },
    Defined { kind: Predefined::Map, rule: "111.10s", form: Form::Artifact { subtype: "Map" }, text: &["{1}, {T}, Sacrifice this token: Target creature you control explores. Activate only as a sorcery."] },
];

impl Defined {
    /// The sentence of the rule, as the Comprehensive Rules print it.
    pub fn sentence(&self) -> String {
        // “A,” and “B”: a list's comma inside the quotation marks, its last period kept
        let quoted = match self.text {
            [one] => format!("“{one}”"),
            [first @ .., last] => format!("{} and “{last}”", first.iter().map(|t| format!("“{},”", t.trim_end_matches('.'))).collect::<Vec<_>>().join(" ")),
            [] => String::new(),
        };
        match self.form {
            Form::Artifact { subtype } => format!("{} A {} token is a colorless {subtype} artifact token with {quoted}", self.rule, self.kind.name()),
            Form::Role { named } => {
                let with = if self.text.len() == 1 { format!("enchant creature and {quoted}") } else { format!("enchant creature, {quoted}") };
                format!("{} A {} token is a colorless Aura Role enchantment token named {named} with {with}", self.rule, self.kind.name())
            }
        }
    }

    /// Its name: a Role's is the one its definition gives; an artifact's definition names
    /// none, so it is its subtype plus the word "Token" (111.4).
    pub fn name(&self) -> String {
        match self.form {
            Form::Artifact { subtype } => format!("{subtype} Token"),
            Form::Role { named } => named.to_string(),
        }
    }

    fn read(&self) -> Res<CardDef> {
        let refused = |why: String| Unsupported(format!("token: {} ({}): {why}", self.kind.name(), self.rule));
        // a quotation inside the quoted ability is the grammar's ordinary quotation
        let text = self.text.iter().map(|t| t.replace(['‘', '’'], "\"")).collect::<Vec<_>>();
        let (type_line, text) = match self.form {
            Form::Artifact { subtype } => (format!("Artifact — {subtype}"), text.join("\n")),
            Form::Role { .. } => ("Enchantment — Aura Role".to_string(), std::iter::once("Enchant creature".to_string()).chain(text).collect::<Vec<_>>().join("\n")),
        };
        let def = CardDef::new(&self.name(), "", &type_line, &text, None).map_err(refused)?;
        crate::play::check(&def).map_err(|u| refused(u.0))?;
        Ok(def)
    }
}

/// The definitions, read once and shared by every game.
/// Each is read on its own, by [`ready`] before the first card is compiled, so that none is
/// compiled beneath a card's own compilation: the two nested would be one deep stack.
fn table() -> &'static [OnceLock<Res<CardDef>>] {
    static TABLE: OnceLock<Vec<OnceLock<Res<CardDef>>>> = OnceLock::new();
    TABLE.get_or_init(|| DEFINED.iter().map(|_| OnceLock::new()).collect())
}

thread_local! { static READING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

/// Only the initializing thread may bypass token-first compilation. Reset even
/// when compilation unwinds, so a retry still initializes the whole table.
struct Reading;
impl Reading {
    fn begin() -> Option<Self> {
        READING.with(|reading| {
            if reading.get() { return None; }
            reading.set(true);
            Some(Self)
        })
    }
}
impl Drop for Reading {
    fn drop(&mut self) { READING.with(|reading| reading.set(false)); }
}

/// Read every definition now, if not yet, unless this is a definition being read.
pub(crate) fn ready() {
    static READY: OnceLock<()> = OnceLock::new();
    if READY.get().is_some() { return; }
    let Some(_reading) = Reading::begin() else { return; };
    for (at, d) in DEFINED.iter().enumerate() { table()[at].get_or_init(|| d.read()); }
    let _ = READY.set(());
}

/// A value read once whose reading compiles cards: the predefined tokens are read first.
/// Every thread takes the token table before such a cell, so none waits on a token while
/// holding a cell that a token's own reading waits on (a definition being read on this thread
/// returns at once).
pub(crate) fn init<'a, T>(cell: &'a OnceLock<T>, read: impl FnOnce() -> T) -> &'a T {
    if cell.get().is_none() { ready(); }
    cell.get_or_init(read)
}

/// The definition of a predefined token (111.10), or why the engine does not create it.
pub fn definition(kind: Predefined) -> Res<&'static CardDef> {
    let Some(at) = DEFINED.iter().position(|d| d.kind == kind) else {
        return Err(Unsupported(format!("token: {} (111.10): a predefined token the engine does not create yet", kind.name())));
    };
    // Take the same token-first boundary as CardDef::compile and init before
    // acquiring a token cell: its own compilation must not reenter that cell.
    ready();
    match table()[at].get_or_init(|| DEFINED[at].read()) {
        Ok(def) => Ok(def),
        Err(e) => Err(e.clone()),
    }
}

/// What a creating instruction makes a token of (111.3, 111.4): its characteristics, and of a
/// token whose description quotes abilities ("a 0/1 colorless Eldrazi Spawn creature token
/// with \"Sacrifice this token: Add {C}.\""), the definition that is its text. That definition
/// is compiled and admitted once, as the instruction is lowered, as any card's is; every token
/// the instruction makes shares it. Its abilities are then found, activated, paid for and
/// copied as a card's are ([`crate::Game::def`], [`crate::copy`]). The token is still no card
/// (111.6).
pub(crate) fn token_values(kind: &TokenKind) -> Res<crate::copy::Token> {
    let chars = token_chars(kind)?;
    let TokenKind::Described(d) = kind else { return Ok(crate::copy::Token::defined(chars)) };
    if !d.abilities.iter().any(|a| matches!(a, ir::action::Granted::Ability(_)) || matches!(a, ir::action::Granted::Keyword(k) if crate::keyword::changeling(k) || !flag(k))) {
        return Ok(crate::copy::Token::defined(chars));
    }
    // Any other keyword needs a definition too: a keyword flag alone cannot keep a defining
    // ability, a parameter or a triggered keyword ("with prowess") through copies (604.3a,
    // 111.3). Whether the engine follows it is decided as for any card's.
    // Its keywords and quoted abilities, in the order the description says them
    let base = Chars { keywords: Vec::new(), ..chars };
    let def = CardDef::copied(&base, None, &[ir::CopyChange::Has(d.abilities.clone())]);
    crate::play::check(&def).map_err(|u| Unsupported(format!("token: with abilities: {}", u.0)))?;
    Ok(crate::copy::Token::described(def))
}

/// The nonlegendary, blue, zero-loyalty token defined by 701.71a. Its two
/// loyalty abilities use ordinary definition, payment and resolution machinery.
pub(crate) fn empower_jace() -> Res<crate::copy::Token> {
    static READY: OnceLock<Res<crate::copy::Token>> = OnceLock::new();
    READY.get_or_init(|| {
        let def = CardDef::planeswalker("Jace Token", "", "Planeswalker — Jace",
            "−1: Surveil 1.\n−3: Draw a card.", 0).map_err(Unsupported)?;
        let mut chars = def.chars();
        chars.colors = vec![ir::Color::Blue];
        let def = CardDef::copied(&chars, Some(&def), &[]);
        crate::play::check(&def)?;
        Ok(crate::copy::Token::described(def))
    }).clone()
}

/// A keyword the token's characteristics hold as a flag, with nothing more to say.
fn flag(k: &ir::token::KeywordInstance) -> bool { k.param == ir::token::KeywordParam::None && crate::keyword::known(k.keyword) }

pub(crate) fn token_chars(kind: &TokenKind) -> crate::Res<Chars> {
    use crate::unsupported;
    let d = match kind {
        TokenKind::Described(d) => d,
        TokenKind::Predefined(kind) => {
            let def = crate::token::definition(*kind)?;
            return Ok(Chars { predefined: Some(*kind), ..def.chars() });
        }
        TokenKind::Those => return unsupported(format!("token: {}", name(kind))),
    };
    // "with flying": the keyword abilities it is created with. A quoted ability is its text,
    // compiled into a definition by [`token_values`].
    let mut keywords = Vec::new();
    for granted in &d.abilities {
        match granted {
            ir::action::Granted::Keyword(k) if flag(k) => keywords.push(k.keyword),
            // "toxic 1", "protection from black", "prowess": kept in token_values' definition
            ir::action::Granted::Keyword(_) => {},
            ir::action::Granted::Ability(_) => {},
            ir::action::Granted::Quoted(text) => return unsupported(format!("token: with \"{text}\", which the grammar does not read")),
            _ => return unsupported("token: with abilities other than keywords and quoted abilities"),
        }
    }
    let pt = match &d.pt {
        Some((Pt::Const(p), Pt::Const(t))) => Some((*p as i32, *t as i32)),
        Some(_) => return unsupported("token: X power or toughness"),
        None => None,
    };
    // "~'s Coil": a name made from the creating card's own name (201.5), which is not resolved here
    if d.name.as_deref().is_some_and(|n| n.contains(oracle::normalize::SELF)) {
        return unsupported("token: name made from the card's own name");
    }
    let name = d.name.clone().unwrap_or_else(|| d.subtypes.iter().map(|s| s.name()).chain(["Token"]).collect::<Vec<_>>().join(" "));
    Ok(Chars {
        name,
        supertypes: d.supertypes.clone(),
        types: d.types.clone(),
        subtypes: d.subtypes.clone(),
        colors: d.colors.clone().unwrap_or_default(),
        pt,
        mana_value: 0,
        keywords,
        predefined: None,
    })
}

/// The copiable N/N white Spirit specified by 701.63a, not a 0/0 with counters.
pub(crate) fn endure_spirit(amount: u32) -> Res<crate::copy::Token> {
    i32::try_from(amount).map_err(|_|Unsupported("token power/toughness exceeds the engine integer representation".into()))?;
    token_values(&TokenKind::Described(ir::token::TokenDesc {
        supertypes: Vec::new(), pt: Some((Pt::Const(amount),Pt::Const(amount))),
        colors: Some(vec![ir::Color::White]), subtypes: vec![ir::Subtype::from_name("Spirit").expect("Spirit is a rules subtype")],
        types: vec![ir::CardType::Creature], name: None, abilities: Vec::new(),
    }))
}

#[cfg(test)]
mod initialization_tests {
    use super::*;

    #[test]
    fn reading_guard_recovers_after_unwind_without_clearing_an_outer_read() {
        std::thread::spawn(|| {
            let failed = std::panic::catch_unwind(|| {
                let _reading = Reading::begin().unwrap();
                assert!(Reading::begin().is_none());
                assert!(READING.with(|r| r.get()));
                panic!("definition initialization unwound");
            });
            let panic = failed.expect_err("the injected initialization panic must unwind");
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"definition initialization unwound"));
            assert!(!READING.with(|r| r.get()));
            let retry = Reading::begin().expect("unwinding releases the recursion guard");
            drop(retry);
            let clue = definition(Predefined::Clue).unwrap();
            let chars = clue.chars();
            // CR 111.4, 111.10f: the rule defines these intrinsic characteristics.
            assert_eq!(chars.name, "Clue Token");
            assert_eq!(chars.types, [ir::CardType::Artifact]);
            assert_eq!(chars.subtypes, [ir::Subtype::from_name("Clue").unwrap()]);
            assert!(chars.colors.is_empty());
            assert_eq!(clue.abilities, CardDef::new("Clue Token", "", "Artifact — Clue",
                "{2}, Sacrifice this token: Draw a card.", None).unwrap().abilities);
        }).join().unwrap();
    }
}
