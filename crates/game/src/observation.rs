//! What players were shown: cards looked at or revealed (CR 701.20, 701.22–23, 701.25),
//! kept for each seat entitled to it until that seat has taken it.
//!
//! Looking at a card is knowledge, not a question. A resolving effect lets its player see
//! the cards it shows for as long as it needs them ([`Game::sees`]), and that ends with the
//! effect; what the player learned does not. So each showing is written down when it
//! happens, as a [`Disclosure`]: the cards as they were then, complete in itself. Nothing in
//! it is looked up later, so it stays true after the cards move, are shuffled away or cease
//! to exist, and it gives no permission to look at them where they are now.
//!
//! A disclosure waits in the game for each seat it is for: every seat for a reveal, the one
//! player for a look. A seat takes its own ([`Game::take_observations`]), which removes them:
//! what was delivered is the host's to keep, and copies of the game made to search ahead do
//! not carry it. A copy carries what is still waiting, as it carries every other part of
//! the game. A host that plays seats must take them, or they accumulate; none is dropped.
//!
//! Not here: permission to look at a card that lasts beyond the effect that gave it (a
//! face-down exiled card its player may look at, 406.3), or the controller of a face-down
//! permanent looking at it (708.5): that is current access ([`Game::sees`]). What was shown of
//! a face-down permanent or a card exiled face down is written down here as the card it is.

use std::sync::Arc;

use crate::state::{Game, ObjId, PlayerId, Zone};

/// How the cards were shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One player looked at them (a search, scry, surveil, "look at").
    Looked,
    /// They were revealed: shown to every player (701.20a).
    Revealed,
}

/// One card as it was when it was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The object it was then. A card that has since changed zones is another object
    /// (400.7), which this does not name.
    pub object: ObjId,
    /// Its definition among [`Game::defs`], shared by every game; None for what no card
    /// represents.
    pub card: Option<usize>,
    pub name: String,
    pub owner: PlayerId,
    pub zone: Zone,
}

/// One showing of cards to those entitled to see them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    /// Its place among the showings of this game, the same for every seat it is for.
    pub seq: u64,
    pub kind: Kind,
    /// The player who looked, or whose effect revealed.
    pub by: PlayerId,
    /// In the order they were shown: of a library, the top card first.
    pub cards: Vec<Shown>,
}

/// The disclosures each seat has not taken yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Waiting {
    next: u64,
    seats: Vec<Vec<Arc<Disclosure>>>,
}

impl Game {
    /// Write down that these cards were shown, as they are now: to every player
    /// ([`Kind::Revealed`]) or to `by` and anyone controlling that player ([`Kind::Looked`], 723.4).
    /// Showing no card is nothing.
    pub(crate) fn disclose(&mut self, kind: Kind, by: PlayerId, cards: &[ObjId]) {
        if cards.is_empty() {
            return;
        }
        let cards = cards
            .iter()
            .map(|o| {
                let obj = self.obj(*o);
                Shown { object: *o, card: obj.card, name: self.shown_name(*o).to_owned(), owner: obj.owner, zone: obj.zone }
            })
            .collect();
        let seats = self.players.len();
        let controller = self.decision_maker(by);
        let waiting = self.observation_waiting_mut();
        waiting.seats.resize_with(seats, Vec::new);
        let disclosure = Arc::new(Disclosure { seq: waiting.next, kind, by, cards });
        waiting.next += 1;
        match kind {
            Kind::Revealed => waiting.seats.iter_mut().for_each(|seat| seat.push(disclosure.clone())),
            Kind::Looked => {
                waiting.seats[by].push(disclosure.clone());
                if controller != by { waiting.seats[controller].push(disclosure); }
            },
        }
    }

    /// What this seat was shown and has not taken, oldest first. Reading it takes nothing.
    pub fn observations(&self, seat: PlayerId) -> &[Arc<Disclosure>] {
        self.observations.seats.get(seat).map_or(&[], Vec::as_slice)
    }

    /// Take what this seat was shown: it is the caller's now, and no longer waits here.
    /// Another seat's is untouched.
    pub fn take_observations(&mut self, seat: PlayerId) -> Vec<Arc<Disclosure>> {
        self.observation_waiting_mut().seats.get_mut(seat).map(std::mem::take).unwrap_or_default()
    }
}
