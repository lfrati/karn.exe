//! Cards separated into two piles, one of which is then chosen (700.3).
//!
//! A [`Pending`] is one doing of it: who separates, the two piles they made, and which the
//! chooser took. The cards stay in the zone they are in (700.3c); each is in exactly one
//! pile (700.3a); a pile may be empty (700.3d). What has been decided is kept here, not asked
//! again.
use crate::state::{Answer, Choice, Decision, ObjId, PlayerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    cards: Vec<ObjId>,
    /// Who chooses a pile, and who chooses the player that separates where several may.
    chooser: PlayerId,
    /// The players one of whom separates.
    among: Vec<PlayerId>,
    separator: Option<PlayerId>,
    piles: Option<[Vec<ObjId>; 2]>,
    chosen: Option<usize>,
}

impl Pending {
    /// These cards, to be separated by one of `among` (in the order they would choose) and
    /// one pile of them chosen by `chooser`.
    pub fn new(cards: Vec<ObjId>, chooser: PlayerId, among: Vec<PlayerId>) -> Pending {
        let separator = if let [one] = among.as_slice() { Some(*one) } else { None };
        Pending { cards, chooser, among, separator, piles: None, chosen: None }
    }

    /// The decision it waits for, the same one until it is answered; None once a pile is
    /// chosen, or where there is no player to separate the cards.
    pub fn decision(&self) -> Option<Decision> {
        if self.among.is_empty() { return None; }
        let Some(separator) = self.separator else {
            return Some(Decision::Choose { player: self.chooser, choice: Choice::Player { among: self.among.clone() } });
        };
        let Some(piles) = &self.piles else {
            return Some(Decision::Choose { player: separator, choice: Choice::Select { among: self.cards.clone(), min: 0, max: self.cards.len() } });
        };
        if self.chosen.is_none() {
            return Some(Decision::Choose { player: self.chooser, choice: Choice::Pile { piles: piles.to_vec() } });
        }
        None
    }

    /// Is it waiting for its separator to separate the cards into two piles (700.3)?
    pub fn separating(&self) -> bool {
        !self.among.is_empty() && self.separator.is_some() && self.piles.is_none()
    }

    /// The answer to the decision [`Self::decision`] last returned, which the game has
    /// checked against it. Anything else is not taken, and the decision is asked again.
    pub fn answer(&mut self, answer: Answer) {
        match answer {
            Answer::Player(player) if self.separator.is_none() && self.among.contains(&player) => self.separator = Some(player),
            // the cards chosen are one pile, in the order they were; the rest are the other
            Answer::Objects(first) if self.separator.is_some() && self.piles.is_none() => {
                let (one, other) = self.cards.iter().copied().partition(|card| first.contains(card));
                self.piles = Some([one, other]);
            }
            Answer::Pile(pile) if self.piles.is_some() && self.chosen.is_none() && pile < 2 => self.chosen = Some(pile),
            _ => {}
        }
    }

    /// The pile chosen and the other one. Both empty where nothing was separated.
    pub fn result(&self) -> (Vec<ObjId>, Vec<ObjId>) {
        match (&self.piles, self.chosen) {
            (Some([one, other]), Some(0)) => (one.clone(), other.clone()),
            (Some([one, other]), Some(_)) => (other.clone(), one.clone()),
            _ => (Vec::new(), Vec::new()),
        }
    }
}

impl crate::Game {
    /// Is the pending selection the separation of cards into two piles (700.3): the cards
    /// selected one pile, the rest the other?
    pub fn separating(&self) -> bool {
        let Some(crate::procedure::Procedure::Resolution(resolving)) = self.procedures.last() else { return false };
        matches!(self.decision, Some(Decision::Choose { choice: Choice::Select { .. }, .. }))
            && resolving.destination.is_none()
            && resolving.program_frame().is_some_and(super::Frame::separating)
    }
}

/// A whole pile reference, without silently dropping object constraints.
pub(super) fn reference(selector: &ir::Selector) -> Option<(ir::procedure::PileRef, Option<&ir::Selector>)> {
    if !matches!(selector.quant, ir::Quant::All | ir::Quant::Each) || selector.chooser.is_some() || selector.random.is_some()
        || selector.set.controller.is_some() || selector.set.owner.is_some() || selector.set.caster.is_some()
        || selector.set.zone.is_some() || selector.set.other { return None; }
    let ir::Filter::All(parts) = &selector.set.filter else { return None };
    let [ir::Filter::Atom(ir::Atom::InPile { pile, chooser })] = parts.as_slice() else { return None };
    Some((*pile, chooser.as_deref()))
}

/// The chooser named by the following whole-pile instruction, before its selection runs.
pub(super) fn chooser(action: &ir::Action) -> Option<&ir::Selector> {
    let objects = match action {
        ir::Action::Sacrifice(objects) | ir::Action::Destroy(objects) | ir::Action::Exile(objects) => objects,
        _ => return None,
    };
    let [selector] = objects.items.as_slice() else { return None };
    reference(selector)?.1
}
