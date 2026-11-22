//! The turn (CR 500–514): the game walks the parts [`ir::Part`] declares, performing each one's
//! turn-based actions (703.4) as it begins and giving the active player priority where the
//! part has it (117.3a). A part with priority ends when all players pass in succession with the
//! stack empty (500.2); one without ends when its actions are done (500.3).
//!
//! The rest of a turn is a [`TurnPlan`]: the phase occurrences still to come (500.1), to which
//! effects can add phases (500.8). What would begin next is checked first against the steps the
//! rules leave out (103.8a, 508.8) and then against the skip effects waiting for it (614.10):
//! a skipped step or phase never begins, so nothing triggers at its beginning and nothing
//! scheduled for its "next" occurrence happens (614.10a).

use std::collections::VecDeque;

use crate::event::Event;
use crate::state::{Choice, Decision, Game, ObjId, PlayerId, Pool};
use ir::turn::Phase;
use ir::Part;

/// "Each player has a maximum hand size, which is normally seven cards" (402.2).
pub(crate) const MAXIMUM_HAND_SIZE: usize = 7;

/// One phase occurrence, unique in the game: "this phase", "this combat" (500.8, 500.5a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhaseId(pub u32);

/// A phase of a turn as it occurs: an ordinary one or one an effect added (500.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseRun {
    pub id: PhaseId,
    pub phase: Phase,
    /// An effect added it (500.8).
    pub added: bool,
    /// Its parts still to begin, front first ([`Phase::parts`]).
    parts: VecDeque<Part>,
}

/// What would begin next: a phase, or a part of the phase in progress. It is what a skip
/// effect names (614.10): "their next draw step" a part, "their next combat phase" a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Occurrence {
    Turn,
    Part(Part),
    Phase(Phase),
}

/// A skip effect's identity, in the order they were created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkipId(pub u32);

/// How long a skip effect waits for what it skips (614.10a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipLasts {
    /// "their next draw step": the first such occurrence of that player's that is not skipped
    /// otherwise, whenever that is.
    Next,
    /// "their next combat phase this turn": as `Next`, but over as this turn ends.
    NextThisTurn { turn: u32 },
}

impl SkipLasts {
    /// Two skips of one occurrence are interchangeable when they last alike: whichever applies,
    /// what remains is the same (614.10a).
    fn class(self) -> u8 {
        match self {
            SkipLasts::Next => 0,
            SkipLasts::NextThisTurn { .. } => 1,
        }
    }
}

/// "Target player skips their next draw step" once it has resolved (614.1b, 614.10): a
/// replacement effect that waits for the occurrence it replaces with nothing. Its source is
/// the object and ability whose effect created it, by which a player choosing among skip
/// effects names it (616.1); nothing of the effect depends on them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub id: SkipId,
    /// The player whose occurrence it is.
    pub player: PlayerId,
    pub what: Occurrence,
    pub lasts: SkipLasts,
    /// The object whose ability created the effect, when it was an object's.
    pub source: Option<ObjId>,
    /// That ability.
    pub ability: Option<crate::binding::AbilityKey>,
}

enum SkipSelection { None, Automatic(SkipId), Choose(Vec<SkipId>) }

/// The phases of the turn in progress and still to come (500.1, 500.8). Owned by the game
/// state and cloned with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnPlan {
    /// The turn and the part it last began: a game set up by hand at another part has the
    /// ordinary plan of that part ([`Game::sync_plan`]).
    turn: u32,
    at: Part,
    /// The phase in progress; None before the turn's first phase has begun.
    current: Option<PhaseRun>,
    upcoming: VecDeque<PhaseRun>,
    next_id: u32,
    /// Combat phases begun this turn: "the first combat phase" is an actual ordinal (506.8d).
    combats: u32,
    /// The occurrence that would begin, held while its player chooses which of the skip
    /// effects offered applies (616.1): nothing has begun, and no player has priority (500.12).
    pending: Option<(Occurrence, Vec<SkipId>)>,
    /// A phase was added to this turn or skipped in it: it is not the ordinary turn.
    modified: bool,
}

impl TurnPlan {
    fn run(&mut self, phase: Phase, added: bool) -> PhaseRun {
        let id = PhaseId(self.next_id);
        self.next_id = self.next_id.checked_add(1).expect("phase identity exhausted");
        PhaseRun { id, phase, added, parts: phase.parts().iter().copied().collect() }
    }

    /// A turn about to begin: its five phases, none begun (500.1).
    pub(crate) fn turn(turn: u32, next_id: u32) -> TurnPlan {
        let mut plan = TurnPlan { turn, at: Part::Cleanup, current: None, upcoming: VecDeque::new(), next_id, combats: 0, pending: None, modified: false };
        for phase in Phase::ORDER {
            let run = plan.run(phase, false);
            plan.upcoming.push_back(run);
        }
        plan
    }

    /// The ordinary turn, with this part begun.
    pub(crate) fn at(turn: u32, part: Part, next_id: u32) -> TurnPlan {
        let mut plan = TurnPlan::turn(turn, next_id);
        while let Some(run) = plan.upcoming.pop_front() {
            plan.combats += u32::from(run.phase == Phase::Combat);
            let found = run.phase == part.phase();
            plan.current = Some(run);
            if found { break; }
        }
        let current = plan.current.as_mut().expect("every part is of a phase");
        while current.parts.pop_front().is_some_and(|p| p != part) {}
        plan.at = part;
        plan
    }

    /// What would begin next, if anything: a part of the phase in progress, or the next phase.
    fn next(&self) -> Option<Occurrence> {
        match self.current.as_ref().and_then(|c| c.parts.front()) {
            Some(part) => Some(Occurrence::Part(*part)),
            None => self.upcoming.front().map(|run| Occurrence::Phase(run.phase)),
        }
    }

    /// Proceed past what would have begun as though it did not exist (500.11).
    fn pass_over(&mut self, what: Occurrence) {
        match what {
            Occurrence::Turn => unreachable!("whole turns are skipped by the schedule owner"),
            Occurrence::Part(_) => { self.current.as_mut().and_then(|c| c.parts.pop_front()); }
            Occurrence::Phase(_) => {
                self.upcoming.pop_front();
                self.modified = true;
            }
        }
    }

    /// The phase in progress.
    pub fn phase(&self) -> Option<&PhaseRun> {
        self.current.as_ref()
    }

    /// The phases still to come this turn, front first.
    pub fn upcoming(&self) -> impl Iterator<Item = &PhaseRun> {
        self.upcoming.iter()
    }

    pub fn combats(&self) -> u32 {
        self.combats
    }

    /// Phases were added to or skipped in this turn.
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// What would begin while its player chooses which skip applies to it (616.1).
    pub fn skipping(&self) -> Option<Occurrence> {
        self.pending.as_ref().map(|(what, _)| *what)
    }

    /// The combat phase "this combat" names: the one in progress, else the next this turn
    /// (500.5a). None when no combat phase is in progress or to come.
    pub(crate) fn this_combat(&self) -> Option<PhaseId> {
        self.current.iter().chain(&self.upcoming).find(|run| run.phase == Phase::Combat).map(|run| run.id)
    }
}

impl Game {
    /// Give priority to this player, or the next living player when they have left
    /// (800.4a, 800.4j). The departed active player's turn itself still continues.
    pub(crate) fn priority_from(&mut self, player: crate::PlayerId) {
        let n = self.players.len();
        if let Some(next) = (0..n).map(|i| (player + i) % n).find(|p| !self.players[*p].lost) {
            self.priority = next;
        }
    }

    /// A game at the very start of its first turn: [`Game::new`] is set up in that turn's
    /// precombat main phase, for a board put together by hand.
    pub fn begin(&mut self) {
        self.begin_checked().expect("bounded fixture turn actions");
    }

    /// [`Self::begin`], refused (unchanged) as an answer is, with what kind of refusal it is.
    pub fn try_begin(&mut self) -> Result<(),crate::simulation::ResponseError> {
        let before=self.clone();
        match self.begin_checked() {
            Ok(())=>Ok(()),
            Err(error)=>{self.adopt(before);Err(error.into())}
        }
    }
    pub(crate) fn begin_checked(&mut self) -> crate::play::ActionResult<()> {
        self.validate_setup_control_history()?;
        let next_id = self.plan.next_id;
        self.plan = TurnPlan::turn(self.turn, next_id);
        self.advance()
    }

    /// The phases of this turn in progress and to come (500.1, 500.8).
    pub fn turn_plan(&self) -> std::borrow::Cow<'_, TurnPlan> {
        if self.plan.pending.is_none() && (self.plan.turn, self.plan.at) != (self.turn, self.part) {
            std::borrow::Cow::Owned(TurnPlan::at(self.turn, self.part, self.plan.next_id))
        } else {
            std::borrow::Cow::Borrowed(&self.plan)
        }
    }

    /// Compare the actual remaining phase/step program, ignoring native phase
    /// identities. Modified turns, waiting skips and turn-control queues cannot
    /// be described by the bounded ordinary schedule profile.
    pub(crate) fn ordinary_turn_suffix(&self) -> bool {
        let plan=self.turn_plan();
        let ordinary=TurnPlan::at(self.turn,self.part,0);
        let run=|phase:&PhaseRun| (phase.phase,phase.added,phase.parts.clone());
        !plan.modified && plan.pending.is_none() && self.skips.is_empty()
            && self.ordinary_turn_schedule() && plan.combats==ordinary.combats
            && plan.current.as_ref().map(run)==ordinary.current.as_ref().map(run)
            && plan.upcoming.iter().map(run).eq(ordinary.upcoming.iter().map(run))
    }

    /// A game put at another part by hand has the ordinary plan of that part.
    pub(crate) fn sync_plan(&mut self) {
        if self.plan.pending.is_none() && (self.plan.turn, self.plan.at) != (self.turn, self.part) {
            let plan = TurnPlan::at(self.turn, self.part, self.plan.next_id);
            self.plan = plan;
        }
    }

    /// Is this part left out this turn by the rules, not by an effect? In a two-player game
    /// the player who plays first skips the draw step of their first turn (103.8a). The
    /// declare blockers and combat damage steps are skipped if no creatures are declared as
    /// attackers (508.8). Neither is a skip effect: no skip effect is used up by it.
    fn omitted(&self, part: Part) -> bool {
        match part {
            Part::Draw => self.turn == 1 && self.players.len() == 2,
            Part::DeclareBlockers | Part::CombatDamage => !self.combat.declared,
            _ => false,
        }
    }

    /// The current part ends (500.5): unspent mana empties from every pool (703.4q); then the
    /// next part begins, or after the cleanup step the next turn (or another cleanup step, when
    /// players received priority in this one, 514.3a).
    pub(crate) fn end_part(&mut self) -> crate::play::ActionResult<()> {
        self.sync_plan();
        // but mana an effect keeps, until that effect ends: the combat phase's end (511.3), the
        // cleanup step's (514.2)
        let part = self.part;
        let retaining: Vec<[bool; 5]> = (0..self.players.len())
            .map(|p| self.kept_mana_colors(p)).collect();
        for (player, p) in self.players.iter_mut().enumerate() {
            let old_colors = p.pool.colors;
            let kept = std::mem::take(&mut p.pool.restricted).into_iter().filter(|m| matches!(m.mana, ir::mana::ManaSymbol::Color(c) if retaining[player][c as usize]) || match m.kept {
                Some(crate::state::Kept::Combat) => part.phase() == ir::turn::Phase::Combat && part != Part::EndOfCombat,
                Some(crate::state::Kept::Turn) => part != Part::Cleanup,
                None => false,
            }).collect();
            p.pool = Pool { colors: std::array::from_fn(|c| if retaining[player][c] { old_colors[c] } else { 0 }), restricted: kept, ..Pool::default() };
        }
        if self.part == Part::EndOfCombat {
            // the combat phase ends with its end of combat step (500.5a)
            self.end_combat();
            let ended = self.plan.phase().map(|run| run.id);
            self.end_combat_rules(ended);
        }
        // after a combat damage step in which only creatures with first strike or double
        // strike assigned damage, the phase gets a second one (510.4)
        if self.part == Part::CombatDamage && self.second_combat_damage_step() {
            return self.begin_part(Part::CombatDamage);
        }
        if self.part == Part::Cleanup && std::mem::take(&mut self.another_cleanup) {
            return self.begin_part(Part::Cleanup);
        }
        self.advance()
    }

    /// What comes next in the plan begins, or after the last phase the next turn. Each
    /// occurrence is checked as it would begin, before anything happens in it: one the rules
    /// leave out is passed over; one a skip effect waits for is replaced with nothing,
    /// using up one such effect (614.10, 614.10a). When skips that last differently compete,
    /// the active player, whose occurrence it is, chooses which applies (616.1).
    fn advance(&mut self) -> crate::play::ActionResult<()> {
        loop {
            if self.finished() {
                return Ok(());
            }
            let Some(next) = self.plan.next() else { return self.begin_turn() };
            if let Occurrence::Part(part) = next
                && self.omitted(part)
            {
                self.plan.pass_over(next);
                continue;
            }
            let active = self.active;
            match self.skip_selection(active, next) {
                SkipSelection::None => {},
                SkipSelection::Automatic(id) => { self.skip(id, next); continue; },
                SkipSelection::Choose(among) => {
                    self.plan.pending = Some((next, among.clone()));
                    self.decision = Some(Decision::Choose { player: active, choice: Choice::Skip { what: next, among } });
                    return Ok(());
                },
            }
            match next {
                Occurrence::Turn => unreachable!("a phase plan does not schedule whole turns"),
                Occurrence::Phase(phase) => {
                    let run = self.plan.upcoming.pop_front().expect("the next phase");
                    self.plan.combats += u32::from(phase == Phase::Combat);
                    self.plan.current = Some(run);
                }
                Occurrence::Part(part) => {
                    self.plan.pass_over(next);
                    self.plan.at = part;
                    return self.begin_part(part);
                }
            }
        }
    }

    /// Replacement selection shared by phase/step and whole-turn candidates (616.1).
    fn skip_selection(&self, player: PlayerId, what: Occurrence) -> SkipSelection {
        let matching: Vec<_> = self.skips.iter().filter(|s| s.player == player && s.what == what).collect();
        let Some(first) = matching.first() else { return SkipSelection::None };
        if matching.iter().all(|s| s.lasts.class() == first.lasts.class()) {
            SkipSelection::Automatic(first.id)
        } else { SkipSelection::Choose(matching.iter().map(|s| s.id).collect()) }
    }
    /// Whole-turn candidates are replaced before any turn-begin hooks or player counters.
    pub(crate) fn skip_turn_candidate(&mut self, player: PlayerId) -> bool {
        match self.skip_selection(player, Occurrence::Turn) {
            SkipSelection::None => false,
            SkipSelection::Automatic(id) => {
                self.skips.retain(|s| s.id != id);
                self.log.push(Event::Skipped { player, what: Occurrence::Turn, skip: id });
                true
            },
            // Currently admitted whole-turn replacements are plain next skips with no
            // riders or distinct duration classes; admission refuses other turn forms.
            SkipSelection::Choose(_) => unreachable!("whole-turn skips have one admitted replacement class"),
        }
    }

    /// This skip effect replaces that occurrence with nothing, and is used up (614.10).
    fn skip(&mut self, id: SkipId, what: Occurrence) {
        self.skips.retain(|s| s.id != id);
        self.plan.pass_over(what);
        let player = self.active;
        self.log.push(Event::Skipped { player, what, skip: id });
    }

    /// The answer to [`Choice::Skip`]: which of the skip effects offered applies (616.1). Only
    /// that one is used up; then the turn goes on from the occurrence after it.
    pub(crate) fn choose_skip(&mut self, player: PlayerId, id: SkipId) -> crate::play::ActionResult<()> {
        let Some((what, among)) = self.plan.pending.clone() else {
            return Err("no step or phase is waiting for a skip to be chosen".into());
        };
        if player != self.active {
            return Err("that decision is another player's".into());
        }
        let active = self.active;
        if !among.contains(&id) || !self.skips.iter().any(|s| s.id == id && s.player == active && s.what == what) {
            return Err("choose one of the skip effects offered (616.1)".into());
        }
        self.decision = None;
        self.plan.pending = None;
        self.skip(id, what);
        self.advance()?;
        Ok(())
    }

    /// A resolving effect: `players` skip their next such occurrence (614.10), this turn only
    /// when `this_turn`.
    /// Ask the questions the rules have a player answer even when one answer is possible
    /// ([`crate::state::GameState::ask_forced`]): for a harness replaying a record.
    pub fn set_ask_forced(&mut self, ask: bool) { self.ask_forced = ask; }

    /// `source` and `ability`: the object and ability whose effect it is (616.1).
    pub(crate) fn add_skip(&mut self, players: &[PlayerId], what: Occurrence, this_turn: bool,
                           source: Option<ObjId>, ability: Option<crate::binding::AbilityKey>) {
        let turn = self.turn;
        let lasts = if this_turn { SkipLasts::NextThisTurn { turn } } else { SkipLasts::Next };
        for player in players {
            let id = SkipId(self.skip_ids);
            self.skip_ids = self.skip_ids.checked_add(1).expect("skip identity exhausted");
            self.skips.push(Skip { id, player: *player, what, lasts, source, ability: ability.clone() });
        }
    }

    /// "After this main phase, there is an additional combat phase followed by an additional
    /// main phase" (500.8): `count` times, this block of phases directly after the phase in
    /// progress, when it is the one named. The block most recently added comes first, and
    /// its phases stay together. They are phases of this turn, whoever's effect added them.
    /// Returns how many phases were added.
    pub(crate) fn add_phases(&mut self, after: ir::action::AfterPhase, kinds: &[Phase], count: u32) -> crate::Res<usize> {
        self.sync_plan();
        let Some(current) = self.plan.phase() else { return Ok(0) };
        let holds = match after {
            ir::action::AfterPhase::This => true,
            ir::action::AfterPhase::ThisMain => current.phase.main(),
            ir::action::AfterPhase::ThisCombat => current.phase == Phase::Combat,
        };
        if !holds || kinds.is_empty() || count == 0 {
            return Ok(0);
        }
        if kinds.contains(&Phase::Ending) { return crate::unsupported("additional ending phase requires a cleanup continuation (500.8)"); }
        if self.turn == 1 && self.players.len() == 2 && kinds.contains(&Phase::Beginning) {
            return crate::unsupported("additional beginning phase during the starting player's first turn: draw-step omission not implemented (103.8a)");
        }
        for _ in 0..count {
            for kind in kinds.iter().rev() {
                let run = self.plan.run(*kind, true);
                self.plan.upcoming.push_front(run);
            }
        }
        self.plan.modified = true;
        Ok(kinds.len() * count as usize)
    }

    /// The next player in turn order takes their turn (500.1, 101.4): one land may be played in
    /// it (305.2).
    fn begin_turn(&mut self) -> crate::play::ActionResult<()> {
        // the turn has ended: what was to be skipped only this turn no longer is (614.10a)
        let ended = self.turn;
        self.skips.retain(|s| s.lasts != SkipLasts::NextThisTurn { turn: ended });
        self.active = self.next_scheduled_turn();
        self.turn += 1;
        self.begin_turn_rules();
        let state = self.state_mut();
        state.players[state.active].turn_began = state.turn;
        state.players[state.active].turns_taken += 1;
        // "until your next turn" ends as that turn begins
        let active = self.active;
        self.delayed.retain(|d| (d.lasts, d.controller) != (crate::state::Lasts::UntilYourNextTurn, active));
        for p in &mut self.players {
            p.land_played = false;
        }
        self.activated.clear();
        self.activation_started.clear();
        self.ability_uses.clear();
        self.cast_permission_uses.clear();
        self.forget_turn();
        let active = self.active;
        self.log.push(Event::TurnBegan { player: active });
        let next_id = self.plan.next_id;
        self.plan = TurnPlan::turn(self.turn, next_id);
        self.advance()
    }


    /// A part begins: abilities that trigger at its beginning trigger (500.6), its turn-based
    /// actions happen (703.4), and the active player receives priority if it has any (117.3a).
    pub(crate) fn begin_part(&mut self, part: Part) -> crate::play::ActionResult<()> {
        if self.finished() {
            return Ok(());
        }
        self.part = part;
        self.parts_begun += 1;
        self.forget_step();
        self.passes = 0;
        self.priority_from(self.active);
        self.log.push(Event::Began { part });
        if part == Part::Upkeep { self.record_upkeep_boundary(); }
        self.begin_part_permissions(part);
        match part {
            // the active player untaps their permanents (502.3); no player receives priority
            // (502.4), so what triggers here waits for the upkeep (503.1a)
            // (all but those an effect keeps from untapping)
            Part::Untap => {
                let untap: Vec<ObjId> =
                    self.battlefield.iter().copied().filter(|id| self.controller(*id) == self.active && !self.kept_tapped(*id)).collect();
                // all at once: one event (502.3, 603.2c)
                let from = self.log.end();
                for id in untap {
                    self.untap(id);
                }
                self.finish_simultaneous(from, crate::layers::Working::before(Vec::new()));
                self.untap_step_passed();
                return self.end_part();
            }
            // the active player draws a card (504.1)
            Part::Draw if !self.players[self.active].lost => self.try_draw(self.active, 1)?,
            Part::Draw => {},
            Part::Cleanup => return self.cleanup(),
            // the active player declares attackers (508.1), each defending player blockers
            // (509.1), combat damage is assigned and dealt (510.1–2)
            Part::DeclareAttackers => self.begin_declare_attackers(),
            Part::DeclareBlockers => self.begin_declare_blockers(),
            Part::CombatDamage => self.begin_combat_damage()?,
            // the active player puts a lore counter on each of their Sagas (714.3c)
            Part::PrecombatMain => self.saga_lore()?,
            Part::Upkeep | Part::BeginningOfCombat | Part::EndOfCombat | Part::PostcombatMain | Part::End => {}
        }
        // a turn-based action that waits for a player's decision goes on when it is made;
        // then, as otherwise now, the active player receives priority (508.2, 509.2, 510.3)
        if self.decision.is_none() {
            self.settle()?;
        }        Ok(())
    }

    /// The cleanup step. First the active player discards down to their maximum hand size
    /// (514.1): which cards is theirs to decide.
    fn cleanup(&mut self) -> crate::play::ActionResult<()> {
        let excess = self.maximum_hand_size(self.active).map_or(0, |maximum| self.players[self.active].hand.len().saturating_sub(maximum));
        if excess > 0 {
            self.decision = Some(Decision::Discard { player: self.active, count: excess });
        } else {
            self.finish_cleanup()?;
        }        Ok(())
    }

    /// The answer to [`Decision::Discard`]: exactly that many cards of the player's hand.
    pub(crate) fn discard_to_hand_size(&mut self, player: PlayerId, count: usize, cards: &[ObjId]) -> crate::play::ActionResult<()> {
        let distinct = cards.iter().enumerate().all(|(i, c)| !cards[..i].contains(c));
        if cards.len() != count || !distinct {
            return Err(format!("discard exactly {count} card(s) (514.1)").into());
        }
        if !cards.iter().all(|c| self.players[player].hand.contains(c)) {
            return Err("that card is not in your hand".into());
        }
        self.decision = None;
        let parent = self.procedures.len();
        let discards = cards.iter().map(|card| (player, *card)).collect::<Vec<_>>();
        match self.discard_proposed(&discards, None)? {
            Ok(_) => self.finish_cleanup()?,
            Err(decision) => {
                self.procedures.insert(parent, crate::procedure::Procedure::Cleanup { complete: false });
                self.decision = Some(decision);
            }
        }
        Ok(())
    }

    /// Second, all damage marked on permanents is removed and effects that last "until end of
    /// turn" or "this turn" end, at once (514.2). No player receives priority
    /// (514.3) unless a state-based action is performed or an ability waits to go on the stack;
    /// then the active player does, and another cleanup step follows this one (514.3a).
    pub(crate) fn finish_cleanup(&mut self) -> crate::play::ActionResult<()> {
        for id in self.battlefield.clone() {
            self.obj_mut(id).damage = 0;
            // saddled until the end of the turn (702.171b)
            self.obj_mut(id).saddled = false;
        }
        self.end_turn_rules();
        self.end_turn_permissions();
        self.delayed.retain(|d| d.lasts != crate::state::Lasts::ThisTurn);
        self.shields.clear();
        self.prevention.clear();
        let previous = self.control_snapshot();
        self.continuous.retain(|c| c.lifetime != crate::layers::Lifetime::Turn);
        self.end_turn_copies();
        self.prune_control_durations();
        self.control_transitions(previous);
        let acted = self.state_based_checked()?;
        self.check_triggers();
        if acted || !self.triggered.is_empty() {
            self.another_cleanup = true;
            self.settle()?;
        } else {
            self.end_part()?;
        }        Ok(())
    }
}
