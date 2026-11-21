//! CR700.13: crime is a completed announcement, classified once from its initial
//! target identities and their current state (115.9b), before eventual resolution.
use crate::{Event, Game, ObjId, PlayerId, Target, Zone};

impl Game {
    /// Only casting/activation commitment and genuine triggered-ability placement call
    /// this owner. Copy creation and target changes are different actions (707.10).
    pub(crate) fn announcement_crime(&mut self, player: PlayerId, object: ObjId) {
        let opponent = |who: PlayerId| who != player && !self.players[who].lost;
        let qualifies = self.obj(object).targets.iter().flatten().any(|target| match *target {
            Target::Player(who) => opponent(who),
            Target::Object(id) => {
                let card = self.obj(id);
                match card.zone {
                    Zone::Battlefield | Zone::Stack => opponent(self.controller(id)),
                    Zone::Graveyard => self.is(*target, ir::Sort::Card) && opponent(card.owner),
                    // Departed incarnations and cards in other zones do not qualify;
                    // never recover a successor or consult target LKI (115.9b).
                    _ => false,
                }
            },
        });
        if qualifies { self.log.push(Event::CrimeCommitted { player }); }
    }
}
