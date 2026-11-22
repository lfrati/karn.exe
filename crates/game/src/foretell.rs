//! Foretell (702.143): a special action from the hand during its player's own turn, with any
//! stack (116.2h), that pays {2} and exiles the card face down; and casting that foretold card
//! for its foretell cost on a later turn.
//!
//! The foretold card is a card exiled face down ([`crate::face::Hidden`], 406.3) that only its
//! player may look at (702.143a), with a public designation of the turn it was foretold
//! (702.143e). It is cast through [`CastRoute::Foretell`]: turned face up just before the
//! announcement (406.3a), then cast for its foretell cost as an alternative cost (118.9).
//! What a foretold card is and costs is read from the card as it is face up.
use crate::card::CastAs;
use crate::cost::Plan;
use crate::state::{Pool, Zone};
use crate::{Game, ObjId, PlayerId};

/// Of a card in exile: foretold by this player on this turn (702.143a, 702.143c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Foretold {
    pub by: PlayerId,
    pub turn: u32,
}

/// What a foretell keyword says its foretell cost is.
pub(crate) fn cost(item: &ir::token::KeywordInstance) -> Option<ir::Cost> {
    if item.keyword.name() != "Foretell" { return None; }
    match &item.param {
        ir::token::KeywordParam::Mana(mana) => Some(ir::Cost(vec![ir::CostPart::Mana(mana.clone())])),
        ir::token::KeywordParam::Cost(cost) => Some(cost.clone()),
        _ => None,
    }
}

/// The cost of the special action: {2} (702.143a).
fn action_cost() -> Plan {
    Plan::default().kicked(&ir::mana::ManaCost(vec![ir::mana::ManaSymbol::Generic(2)]), 1)
}

impl Game {
    /// Foretold, as this exile incarnation is (702.143c); a card that left exile is a new
    /// object that is not (400.7).
    pub fn foretold(&self, card: ObjId) -> Option<Foretold> {
        let o = self.obj(card);
        if o.zone != Zone::Exile { return None; }
        o.foretold
    }

    /// The cost of foretelling this card, if this player may now: any time they have
    /// priority during their own turn (116.2h), a card with foretell in their hand
    /// (702.143a).
    pub(crate) fn foretell_cost(&self, player: PlayerId, card: ObjId) -> crate::play::ActionResult<Plan> {
        if self.active != player {
            return Err("foretell only during your own turn (702.143a)".into());
        }
        if !self.players[player].hand.contains(&card) {
            return Err("foretell a card from your hand (702.143a)".into());
        }
        let def = self.def(card).ok_or("foretell needs a card")?;
        crate::play::check(def)?;
        let castable = def.cast_choices(true).into_iter().any(|cast_as| def.spell(cast_as).is_ok_and(|d| matches!(d.costs.foretell, Some(Ok(_)))));
        if !castable { return Err("that card has no supported foretell ability".into()); }
        Ok(action_cost())
    }

    /// The special action (116.2h, 702.143b): exile the card face down, only its player
    /// looking at it, and pay {2}. It is foretold when the cost is paid.
    pub(crate) fn foretell(&mut self, player: PlayerId, card: ObjId, steps: &[crate::legal::Step], with: Option<&Pool>) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let cost = proposed.foretell_cost(player, card)?;
        let original = proposed.obj(card).clone();
        let position = proposed.players[player].hand.iter().position(|o| *o == card).expect("foretell checked the hand");
        let triggers = proposed.triggered.len();
        let exiled = proposed.exile_hidden(card, &[player], None).ok_or("the foretold card could not be exiled")?;
        if proposed.obj(exiled).zone != Zone::Exile { return Err("the foretold card could not be exiled".into()); }
        proposed.check_triggers();
        let triggers = triggers..proposed.triggered.len();
        let undo = crate::payment::Undo::ExiledFromHand { original: card, object: Box::new(original), position };
        let library_tops = proposed.library_tops();
        proposed.procedures.push(crate::procedure::Procedure::Payment(crate::payment::Payment::foretell(
            player, exiled, cost, steps, with, undo, triggers,
        ).library_tops(library_tops)));
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }

    /// The cost is paid: the card is foretold this turn.
    pub(crate) fn complete_foretell(&mut self, player: PlayerId, card: ObjId) {
        if self.obj(card).zone != Zone::Exile { return; }
        let turn = self.turn;
        self.obj_mut(card).foretold = Some(Foretold { by: player, turn });
        self.log.push(crate::Event::Foretold { player, card });
    }

    /// Could this player cast this foretold card as `cast_as` for its foretell cost, but for
    /// timing and costs: after the turn it was foretold, by the player who foretold it, of a
    /// card that has a foretell cost (702.143a).
    pub(crate) fn foretold_of(&self, player: PlayerId, card: ObjId, cast_as: CastAs) -> bool {
        let Some(foretold) = self.foretold(card) else { return false };
        if foretold.by != player || self.turn <= foretold.turn || cast_as == CastAs::Fused { return false; }
        let Some((_, def)) = self.physical_card(card) else { return false };
        crate::play::check(def).is_ok() && def.spell(cast_as).is_ok_and(|d| d.costs.foretell.is_some())
    }

    /// 406.3a: turned face up just before its player announces it is cast. It is the same
    /// object, now with its card's characteristics; what it was is restored if the cast is
    /// reversed.
    pub(crate) fn turn_foretold_face_up(&mut self, card: ObjId) {
        if self.obj(card).hidden.is_none() { return; }
        self.obj_mut(card).hidden = None;
        let chars = self.face_up_chars(card);
        self.obj_mut(card).chars = chars;
    }

    /// This game as it would be with this foretold card face up, to list the ways of casting
    /// it (406.3a): None if it is not one this player could cast so.
    pub(crate) fn with_foretold_face_up(&self, player: PlayerId, card: ObjId) -> Option<Game> {
        let def = self.physical_card(card)?.1;
        if !def.cast_choices(false).into_iter().any(|cast_as| self.foretold_of(player, card, cast_as)) { return None; }
        let mut view = self.clone();
        view.turn_foretold_face_up(card);
        Some(view)
    }

}
