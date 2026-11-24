//! Soulbond's pairs (702.95). A soulbond ability pairs two creatures as it resolves
//! (702.95a, c, d); they stay paired until another player gains control of either, either
//! stops being a creature or either leaves the battlefield (702.95e).
//!
//! A pair names the two permanents as they were paired and the player who controlled them
//! then. A permanent that leaves the battlefield is a new object (400.7), so the pair is read
//! as over the moment either is no longer on the battlefield; control and creature-ness are
//! the layers' to say, so [`Game::reconcile_pairs`] ends the pairs they break after each
//! completed act, as rules events are checked ([`Game::capture_triggers`]).
use crate::{Game, ObjId, PlayerId, Zone};

/// Two creatures soulbond paired, and the player who controlled both then (702.95a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pair {
    pub(crate) creatures: [ObjId; 2],
    pub(crate) controller: PlayerId,
}

impl Game {
    /// The creature this permanent is paired with (702.95b), if it is paired.
    pub(crate) fn partner(&self, object: ObjId) -> Option<ObjId> {
        if self.pairs.is_empty() { return None; }
        let here = |o: ObjId| self.get_object(o).is_some_and(|o| o.zone == Zone::Battlefield);
        self.pairs.iter().find_map(|pair| match pair.creatures {
            [a, b] if a == object => Some(b),
            [a, b] if b == object => Some(a),
            _ => None,
        }).filter(|other| here(object) && here(*other))
    }

    /// Pair these two as a soulbond ability controlled by this player resolves. Neither
    /// becomes paired if either is no longer a creature on the battlefield under that
    /// player's control (702.95c), or is already paired: a creature is paired with only one
    /// other (702.95d).
    pub(crate) fn pair(&mut self, one: ObjId, other: ObjId, controller: PlayerId) {
        // a pair an earlier part of this resolution broke is over first (702.95e)
        self.reconcile_pairs();
        let fits = |o: ObjId| self.get_object(o).is_some_and(|x| x.zone == Zone::Battlefield)
            && self.controller(o) == controller && self.seen(o, None).is(ir::CardType::Creature)
            && self.partner(o).is_none();
        if one != other && fits(one) && fits(other) {
            self.state_mut().pairs.push(Pair { creatures: [one, other], controller });
        }
    }

    /// End the pairs 702.95e ends: of a creature another player gained control of, that
    /// stopped being a creature or that left the battlefield. Called after a completed act;
    /// a check that ends none changes nothing.
    pub(crate) fn reconcile_pairs(&mut self) {
        if self.pairs.is_empty() { return; }
        let holds = |pair: &Pair| pair.creatures.iter().all(|o| self.get_object(*o).is_some_and(|x| x.zone == Zone::Battlefield)
            && self.controller(*o) == pair.controller && self.seen(*o, None).is(ir::CardType::Creature));
        if self.pairs.iter().all(holds) { return; }
        let kept: Vec<Pair> = self.pairs.iter().copied().filter(|pair| holds(pair)).collect();
        self.state_mut().pairs = kept;
    }
}
