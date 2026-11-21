//! Individual draws in a retained sequence (121.2, 614.11). Both instruction execution and
//! turn-based actions call this operation. A substituted draw finishes before the next draw
//! asks which replacements now apply; no earlier draw or instruction is replayed.
use crate::event::{Event, Target};
use crate::operation::{Completed, Operation, ReplacementId};
use crate::replace::Replacement;
use crate::state::{Answer, Choice, Decision, Game, PlayerId, PublicChoice, Zone};
use ir::{Amount, token::{KeywordInstance, KeywordParam}};

/// The parameter of the rules-defined replacement (702.52a), lowered once. Granting keyword
/// abilities outside the battlefield is not implemented; this recognizes printed instances.
pub(crate) fn dredge(item: &KeywordInstance) -> Option<crate::Res<u32>> {
    if item.keyword.name() != "Dredge" { return None; }
    Some(match &item.param {
        KeywordParam::Number(Amount::Const(n)) => Ok(*n),
        _ => crate::unsupported("dredge: a fixed nonnegative number (702.52a)"),
    })
}


#[derive(Debug, Clone, Copy)]
enum Change {
    Dredge(u32),
    Draw(u32),
}

#[derive(Debug, Clone)]
enum Phase {
    Propose,
    Asked(Vec<(ReplacementId, Change)>),
    Child { count: u32, applied: Vec<ReplacementId> },
    WaitingChild,
    Mill { batch: crate::departure::Batch, dredger: crate::ObjId },
    /// Milling for dredge has already happened; or this is the unreplaced draw's move.
    Move { batch: crate::departure::Batch, drawn: Option<crate::ObjId> },
}

#[derive(Debug, Clone)]
pub(crate) struct Drawing {
    players: Vec<(PlayerId, u32)>,
    cursor: usize,
    // Effects already used on an ancestor event cannot apply to its modified draws (614.5).
    // Sibling draws independently inherit this history; a choice on one cannot affect another.
    applied: Vec<ReplacementId>,
    phase: Phase,
    pub(crate) choices: Vec<PublicChoice>,
}

impl Drawing {
    fn new(game: &Game, mut players: Vec<(PlayerId, u32)>) -> Self {
        // Each player finishes their draws, in APNAP order (121.2c).
        players.sort_by_key(|(p, _)| (p + game.players.len() - game.active) % game.players.len());
        Self { players, cursor: 0, applied: Vec::new(), phase: Phase::Propose, choices: Vec::new() }
    }
    pub(crate) fn waiting(&self) -> bool { match &self.phase {
        Phase::Asked(_) => true, Phase::Move { batch, .. } | Phase::Mill { batch, .. } => batch.waiting(), _ => false,
    } }

    fn draw_one(&mut self, game: &mut Game, player: PlayerId) {
        if let Some(top) = game.top_cards(player, 1).first().copied().filter(|c| game.is_commander(*c)) {
            self.phase = Phase::Move { batch: crate::departure::Batch::new(game, vec![(top, crate::departure::Kind::RuleMove(Zone::Hand))]), drawn: Some(top) };
        } else {
            // No commander destination replacement can apply to this ordinary draw.
            game.commit_draw(player);
            self.players[self.cursor].1 -= 1;
        }
    }

    fn offered(&self) -> Decision {
        let Phase::Asked(options) = &self.phase else { unreachable!("retained offer") };
        let player = self.players[self.cursor].0;
        Decision::Choose { player, choice: Choice::Replacement {
            on: Target::Player(player), kind: "draw".into(), count: 1, counters: None, tokens:None,
            optional: options.iter().all(|(_, change)| matches!(change, Change::Dredge(_))),
            among: options.iter().map(|(id, _)| id.clone()).collect(),
        } }
    }

    /// A replacement body's result belongs to this exact proposed draw. Advancing its cursor
    /// here, once, prevents a paused child from replaying an earlier draw on resumption.
    pub(crate) fn child_complete(&mut self, result: Completed) {
        assert!(matches!(self.phase, Phase::WaitingChild));
        assert!(result.arrived.is_empty() && result.transitions.is_empty(), "draw result");
        self.players[self.cursor].1 -= 1;
        self.phase = Phase::Propose;
    }

    fn apply(&mut self, game: &mut Game, id: ReplacementId, change: Change) -> crate::Res<()> {
        let player = self.players[self.cursor].0;
        match change {
            Change::Draw(count) => {
                let mut applied = self.applied.clone();
                applied.push(id);
                self.phase = Phase::Child { count, applied };
            }
            Change::Dredge(count) => {
                // Retain the complete milling event before returning the dredger (702.52a).
                let milled = game.top_cards(player, count as usize).into_iter().map(|card| (card, crate::departure::Kind::Mill(player))).collect();
                self.phase = Phase::Mill { batch: crate::departure::Batch::new(game, milled), dredger: id.source };
            }
        }        Ok(())
    }

    pub(crate) fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        loop {
            if let Phase::Mill { batch, .. } = &mut self.phase {
                if let Some(decision) = batch.prepare() { return Ok(Some(decision)); }
                let Phase::Mill { batch, dredger } = std::mem::replace(&mut self.phase, Phase::Propose) else { unreachable!() };
                batch.commit(game);
                self.phase = Phase::Move { batch: crate::departure::Batch::new(game,
                    vec![(dredger, crate::departure::Kind::RuleMove(Zone::Hand))]), drawn: None };
                continue;
            }
            if let Phase::Move { batch, .. } = &mut self.phase {
                if let Some(decision) = batch.prepare() { return Ok(Some(decision)); }
                let Phase::Move { batch, drawn } = std::mem::replace(&mut self.phase, Phase::Propose) else { unreachable!() };
                let physical = drawn.and_then(|old| game.obj(old).card_id);
                batch.commit(game);
                let player = self.players[self.cursor].0;
                // A replaced library-to-hand move is not a draw (121.1, 614.6).
                if let Some(physical) = physical && let Some(card) = game.players[player].hand.iter().copied().find(|c| game.obj(*c).card_id == Some(physical)) {
                    let (ordinal, in_draw_step) = game.note_draw(player);
                    game.log.push(Event::Drew { player, card, ordinal, in_draw_step });
                }
                self.players[self.cursor].1 -= 1;
                continue;
            }
            match &self.phase {
                Phase::Move { .. } | Phase::Mill { .. } => unreachable!("movement handled above"),
                Phase::Asked(_) => return Ok(Some(self.offered())),
                Phase::WaitingChild => unreachable!("only the top procedure advances"),
                Phase::Child { count, applied } => {
                    let mut child = Self::new(game, vec![(self.players[self.cursor].0, *count)]);
                    child.applied = applied.clone();
                    self.phase = Phase::WaitingChild;
                    match game.run_operation(Operation::Draw(child))? {
                        Ok(result) => self.child_complete(result),
                        Err(decision) => return Ok(Some(decision)),
                    }
                    continue;
                }
                Phase::Propose => {}
            }
            let Some(&(player, remaining)) = self.players.get(self.cursor) else { return Ok(None) };
            if remaining == 0 || game.players[player].lost { self.cursor += 1; continue; }
            let mut options = Vec::new();
            // Dredge functions in its owner's graveyard (702.52a). Discovery uses compiled
            // registrations; other draw replacements function on the battlefield.
            for source in &game.players[player].graveyard {
                for (key, replacement) in game.replacements(*source) {
                    if let Replacement::Dredge(instances) = replacement {
                        for &(part, count) in instances {
                            let id = ReplacementId::from_ability(*source, key.clone(), part);
                            if !self.applied.contains(&id) && game.top_cards(player, count as usize).len() == count as usize {
                                options.push((id, Change::Dredge(count)));
                            }
                        }
                    }
                }
            }
            for source in &game.battlefield {
                let ctx = crate::select::Ctx::new(game.controller(*source), *source);
                for (key, replacement) in game.replacements(*source) {
                    let id = ReplacementId::from_ability(*source, key.clone(), 0);
                    if let Replacement::Draw { who, count } = replacement
                        && !self.applied.contains(&id)
                        && who.is(game, &ctx.clone().with_key(key), Target::Player(player))
                    { options.push((id, Change::Draw(*count))); }
                }
            }
            if options.len() == 1 && matches!(options[0].1, Change::Draw(_)) {
                let (id, change) = options[0].clone();
                self.apply(game, id, change)?;
                continue;
            }
            if !options.is_empty() {
                self.phase = Phase::Asked(options);
                return Ok(Some(self.offered()));
            }
            self.draw_one(game, player);
        }
    }

    pub(crate) fn answer(&mut self, game: &mut Game, answer: Answer) -> crate::Res<()> {
        if let Phase::Move { batch, .. } | Phase::Mill { batch, .. } = &mut self.phase {
            let published = batch.choices().len();
            batch.answer(answer);
            self.choices.extend_from_slice(&batch.choices()[published..]);
            return Ok(());
        }
        let Phase::Asked(options) = std::mem::replace(&mut self.phase, Phase::Propose) else { unreachable!("retained draw question") };
        let player = self.players[self.cursor].0;
        match answer {
            Answer::No => {
                assert!(options.iter().all(|(_, change)| matches!(change, Change::Dredge(_))));
                self.draw_one(game, player);
            }
            Answer::Replacement(chosen) => {
                let change = options.into_iter().find(|(id, _)| *id == chosen).expect("validated replacement").1;
                self.choices.push(PublicChoice::Replacement { player, on: Target::Player(player), effect: chosen.clone() });
                self.apply(game, chosen, change)?;
            }
            _ => unreachable!("validated replacement answer"),
        }        Ok(())
    }
}

impl Game {
    pub(crate) fn draw_batch(&mut self, players: Vec<(PlayerId, u32)>) -> crate::Res<Result<Completed, Decision>> {
        self.run_operation(Operation::Draw(Drawing::new(self, players)))
    }

    /// Draw as a rules action, including the draw step (504.1) and opening hands (103.5).
    /// Instruction execution calls the same operation with its frame as the parent.
    /// Bounded fixture convenience. Player rules actions use the checked continuation.
    pub fn draw(&mut self, player: PlayerId, count: u32) {
        self.try_draw(player,count).expect("bounded fixture draw");
    }
    pub(crate) fn try_draw(&mut self, player: PlayerId, count: u32) -> crate::Res<()> {
        let parent = self.procedures.len();
        if let Err(decision) = self.draw_batch(vec![(player, count)])? {
            self.procedures.insert(parent, crate::procedure::Procedure::RuleAction { complete: false });
            self.decision = Some(decision);
        }
        Ok(())
    }

    /// Commit one unreplaced draw (121.1). Runtime callers must use the draw operation.
    fn commit_draw(&mut self, player: PlayerId) {
        match self.top_cards(player, 1).first().copied() {
            Some(top) => {
                if let Some(card) = self.move_to(top, Zone::Hand, player) {
                    let (ordinal, in_draw_step) = self.note_draw(player);
                    self.log.push(Event::Drew { player, card, ordinal, in_draw_step });
                }
            }
            None => self.players[player].drew_from_empty = true,
        }
    }
}
