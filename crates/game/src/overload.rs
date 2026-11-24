//! Overload (702.96). "Overload [cost]" means "You may choose to pay [cost] rather than pay
//! this spell's mana cost" and "If you chose to pay this spell's overload cost, change its
//! text by replacing all instances of the word 'target' with the word 'each'" (702.96a), a
//! text-changing effect (702.96c, 612).
//!
//! A card with overload is read twice: as printed, and with its text so changed
//! ([`CardDef::overloaded`]), each row in its place, so an ability's index is the same in both.
//! Cast for its overload cost, it is cast as [`CastAs::Overloaded`]: the cost is chosen before
//! targets are (601.2b–c), so the spell is announced with the changed text and has none
//! (702.96b). The choice is one made for the spell, which a copy of it copies (707.10).
use crate::card::{CardDef, CastAs};
use crate::announcement::CastCost;
use crate::Game;
use oracle::token::{Kind, Token};

/// A row's tokens with each word "target" changed to "each", in quoted abilities too (702.96a).
pub(crate) fn changed(tokens: Vec<Token>) -> Vec<Token> {
    tokens.into_iter().map(|token| {
        let kind = match token.kind {
            Kind::Word(word) if word == "target" => Kind::Word("each".into()),
            Kind::Word(word) if word == "Target" => Kind::Word("Each".into()),
            Kind::Quote { mark, body, closed } => Kind::Quote { mark, body: changed(body), closed },
            kind => kind,
        };
        Token { kind, ..token }
    }).collect()
}

/// Does this definition print overload (702.96a)?
pub(crate) fn prints(def: &CardDef) -> bool {
    def.keyword_declarations.iter().any(|i| i.item.keyword.name() == "Overload")
}

impl Game {
    /// Is the alternative cost at this row of the definition an overload cost (702.96a)?
    pub(crate) fn overload_row(def: &CardDef, row: usize) -> bool {
        ir::Keyword::from_name("Overload").is_some_and(|overload| crate::keyword::alternative_rows(def, overload).contains(&row))
    }

    /// What this card is cast as for this cost: overloaded for its overload cost (702.96a),
    /// and only then; otherwise as it was said.
    pub(crate) fn cast_as_for(&self, card: crate::ObjId, cast_as: CastAs, chosen: CastCost) -> Result<CastAs, String> {
        let overload = |def: &CardDef| matches!(chosen, CastCost::Alternative(row) if Self::overload_row(def, row));
        let Some(def) = self.def(card) else { return Ok(cast_as) };
        match cast_as {
            CastAs::Whole if def.spell(CastAs::Whole).is_ok_and(overload) => Ok(CastAs::Overloaded),
            CastAs::Overloaded if def.overloaded().is_some_and(overload) => Ok(cast_as),
            CastAs::Overloaded => Err("a spell is cast overloaded only for its overload cost (702.96a)".into()),
            _ => Ok(cast_as),
        }
    }
}
