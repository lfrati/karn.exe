//! Scrying (701.22) and surveilling (701.25): a player looks at the top cards of their
//! library and puts each back on top, in any order, or away.
//!
//! A [`Pending`] is one doing of it: who does it and the cards each looks at are read once,
//! as it begins, all of them looking at the same time; each then decides where their cards
//! go, in turn order (101.4), and only when every decision is made does anything move, all
//! of it at once (701.22c), as one departure event: a card surveilled into a graveyard goes
//! where the replacement effects that apply to it have it, its owner choosing among them
//! (616.1), and the cards put back stay where they were put whatever shuffle a replacement
//! does in the same event (701.24g). What has been decided so far is kept here, not asked
//! again.
use crate::departure::{Batch, Kind};
use crate::operation::{Completed, Operation};
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, Zone};
use crate::Event;
use ir::Verb;
use ir::action::LibraryPosition;

/// One player's part: the cards they look at, the first of them the top card, and where
/// they decided each goes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Part {
    player: PlayerId,
    consent: Option<bool>,
    cards: Vec<ObjId>,
    /// Back on top, the first of them the top card; and away, in the order they go.
    decided: Option<(Vec<ObjId>, Vec<ObjId>)>,
}

/// How far it has got: the decisions; the cards moving, which may itself wait for the
/// choices of their owners (616.1); done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage { Deciding, Moving, Done }

/// One doing of "scry N" or "surveil N": the players in turn order, what each has decided,
/// and whether it has been done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    verb: Verb,
    parts: Vec<Part>,
    stage: Stage,
}

impl Pending {
    /// The action as it begins: each of these players looks at the top `count` cards of
    /// their library, as many as there are, read from the game as it now is. Nothing is read
    /// again. A player told to scry or surveil 0 does not do it at all (701.22b, 701.25c).
    pub fn new(game: &Game, players: Vec<PlayerId>, verb: Verb, count: u32) -> Pending {
        debug_assert!(matches!(verb, Verb::Scry | Verb::Surveil), "{verb:?} does not arrange a library");
        let n = game.players.len();
        // each decides in turn order from the active player (101.4), once
        let mut order: Vec<PlayerId> = Vec::new();
        for player in players {
            if count > 0 && !game.players[player].lost && !order.contains(&player) {
                order.push(player);
            }
        }
        order.sort_by_key(|p| (p + n - game.active) % n);
        let parts = order.into_iter().map(|player| Part { player, consent: Some(true), cards: game.top_cards(player, count as usize), decided: None }).collect();
        Pending { verb, parts, stage: Stage::Deciding }
    }

    pub fn optional(mut self, optional: bool) -> Self {
        if optional { for part in &mut self.parts { part.consent = None; } }
        self
    }

    /// Go on: the decision it waits for, the same one until it is answered; or, when every
    /// decision is made, move all the cards at once, asking what their move asks, and return
    /// None once they have moved. Once done, nothing more happens.
    pub fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        match self.stage {
            Stage::Done => return Ok(None),
            // the departure has returned (Self::operation_complete)
            Stage::Moving => { self.finish(game); return Ok(None); }
            Stage::Deciding => {}
        }
        if let Some(part) = self.parts.iter().find(|p| p.consent.is_none()) {
            return Ok(Some(Decision::Choose { player: part.player, choice: Choice::May { possible: true } }));
        }
        for part in &mut self.parts {
            if part.consent == Some(false) { continue; }
            if part.decided.is_some() {
                continue;
            }
            if !part.cards.is_empty() {
                return Ok(Some(Decision::Choose { player: part.player, choice: Choice::Arrange { verb: self.verb, cards: part.cards.clone() } }));
            }
            // with no card to look at there is nothing to decide; the player still does it
            part.decided = Some((Vec::new(), Vec::new()));
        }
        // The cards surveilled go into their owner's graveyard as one event, as the
        // replacements that apply to each have it (614.1a, 616.1), each onto the ones put
        // there before it; the cards scried away go to the bottom, the last the bottom card;
        // the rest stay on top, the first the top card. Cards that stay in the library stay
        // the same objects (400.7 is of zone changes).
        let mut away = Vec::new();
        let mut kept = Vec::new();
        for part in self.parts.iter().filter(|p| p.consent != Some(false)) {
            let (top, rest) = part.decided.clone().expect("every part is decided");
            kept.push((part.player, top, LibraryPosition::Top));
            match self.verb {
                Verb::Scry => kept.push((part.player, rest, LibraryPosition::Bottom)),
                _ => away.extend(rest.into_iter().map(|card| (card, Kind::RuleMove(Zone::Graveyard)))),
            }
        }
        self.stage = Stage::Moving;
        if away.is_empty() {
            // nothing leaves the library, and no replacement applies
            for (player, cards, position) in kept { game.arrange_library(player, &cards, position); }
        } else {
            let batch = kept.into_iter().fold(Batch::new(game, away).stacked_as_given(), |batch, (player, cards, position)| batch.arranging(player, cards, position));
            if let Err(decision) = game.run_operation(Operation::Depart(batch))? { return Ok(Some(decision)); }
        }
        self.finish(game);
        Ok(None)
    }

    /// The cards have moved: each player has scried or surveilled (701.22d, 701.25d), even
    /// if some of it was impossible.
    fn finish(&mut self, game: &mut Game) {
        self.stage = Stage::Done;
        for part in self.parts.iter().filter(|p| p.consent != Some(false)) {
            game.event_log_mut().push(Event::Arranged { player: part.player, verb: self.verb });
        }
    }

    /// The answer to the decision [`Self::advance`] last returned, which the game has
    /// checked against it. Anything else is not taken, and the decision is asked again.
    pub fn answer(&mut self, answer: Answer) {
        if let Some(part) = self.parts.iter_mut().find(|p| p.consent.is_none()) {
            match answer { Answer::Yes => part.consent = Some(true), Answer::No => part.consent = Some(false), _ => {} }
            return;
        }
        let Some(part) = self.parts.iter_mut().find(|part| part.consent == Some(true) && part.decided.is_none()) else { return };
        if let Answer::Arrange { top, away } = answer
            && Game::arrangement(&part.cards, &top, &away)
        {
            part.decided = Some((top, away));
        }
    }

    /// The departure the cards' move waited for has been done.
    pub(super) fn operation_complete(&mut self, _result: Completed) {
        assert_eq!(self.stage, Stage::Moving, "an arrangement's departure completes once, while it moves");
    }

    /// Has it been done?
    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }
}

#[cfg(test)]
mod tests;
