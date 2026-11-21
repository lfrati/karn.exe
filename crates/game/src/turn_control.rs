//! Turn scheduling (500.7) and decision authority (723). Neither changes object control.
//! Pending next-turn anchors name players, not source objects or predicted turn numbers.
use crate::{Game, PlayerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor(pub(crate) u32);

/// An extra turn created by an effect, independent of the ordinal of turns actually taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreatedTurnId(pub(crate) u32);

#[derive(Debug, Clone)]
struct CreatedTurn {
    id: CreatedTurnId,
    player: PlayerId,
    rules: Vec<crate::restrict::Rule>,
}

#[derive(Debug, Clone)]
struct Pending {
    anchor: Anchor,
    player: PlayerId,
    controller: PlayerId,
    after: Vec<CreatedTurn>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Schedule {
    next: u32,
    normal_from: PlayerId,
    extra: bool,
    /// Stack order: the latest created extra turn is first (500.7).
    turns: Vec<CreatedTurn>,
    current: Option<CreatedTurn>,
    pending: Vec<Pending>,
    control: Option<(PlayerId, PlayerId)>,
}

impl Game {
    /// Verified ordinary future: allocator identities and an obsolete normal cursor
    /// are irrelevant; an extra turn in progress is not (500.7).
    pub(crate) fn ordinary_turn_schedule(&self) -> bool {
        let schedule=&self.turn_schedule;
        !schedule.extra && schedule.current.is_none() && schedule.turns.is_empty()
            && schedule.pending.is_empty() && schedule.control.is_none()
    }

    /// Actual player making choices, independently of whose resources/actions they are.
    pub fn decision_maker(&self, player: PlayerId) -> PlayerId {
        self.turn_schedule
            .control
            .filter(|(on, by)| *on == player && !self.players[*by].lost)
            .map_or(player, |(_, by)| by)
    }
    /// Visibility is shared with the controlled player, never transferred (723.4).
    pub fn shares_information(&self, viewer: PlayerId, player: PlayerId) -> bool {
        viewer == player || self.decision_maker(player) == viewer
    }
    /// Rules actor of the pending decision, or player with priority. No option search.
    pub fn decision_actor(&self) -> PlayerId {
        use crate::state::Decision;
        match &self.decision {
            Some(
                Decision::Targets { player, .. }
                | Decision::Modes { player, .. }
                | Decision::Order { player, .. }
                | Decision::Payment { player, .. }
                | Decision::Discard { player, .. }
                | Decision::Mulligan { player }
                | Decision::Bottom { player, .. }
                | Decision::Attackers { player }
                | Decision::Blockers { player }
                | Decision::Divide { player, .. }
                | Decision::Choose { player, .. },
            ) => *player,
            None => self.priority,
        }
    }
    pub(crate) fn control_next_turn(&mut self, player: PlayerId, controller: PlayerId) -> Anchor {
        let state = self.state_mut();
        let schedule = &mut state.turn_schedule;
        let anchor = Anchor(schedule.next);
        schedule.next = schedule.next.checked_add(1).expect("turn anchor identity exhausted");
        schedule.pending.push(Pending { anchor, player, controller, after: Vec::new() });
        anchor
    }
    pub(crate) fn extra_turn(&mut self, player: PlayerId, after: Option<Anchor>) -> Option<CreatedTurnId> {
        if self.players[player].lost { return None; }
        let schedule = &mut self.state_mut().turn_schedule;
        let id = CreatedTurnId(schedule.next);
        schedule.next = schedule.next.checked_add(1).expect("turn identity exhausted");
        let turn = CreatedTurn { id, player, rules: Vec::new() };
        if let Some(anchor) = after {
            schedule.pending.iter_mut().find(|p| p.anchor == anchor)?.after.push(turn);
        } else {
            schedule.turns.push(turn);
        }
        Some(id)
    }
    /// A resolved instruction names the exact created occurrence, not its player's next turn.
    pub(crate) fn rule_during_created_turn(&mut self, id: CreatedTurnId, rule: crate::restrict::Rule) {
        let schedule = &mut self.state_mut().turn_schedule;
        if let Some(turn) = schedule.turns.iter_mut().chain(schedule.pending.iter_mut().flat_map(|p| &mut p.after)).find(|t| t.id == id) {
            turn.rules.push(rule);
        }
    }
    pub(crate) fn created_turn_rules(&self) -> impl Iterator<Item = &crate::restrict::Rule> {
        self.turn_schedule.current.iter().flat_map(|t| &t.rules)
    }
    /// End the old control only when the next turn begins (723.1), then select an actual
    /// turn. Extra turns preserve the ordinary turn-order cursor, including multiplayer.
    pub(crate) fn next_scheduled_turn(&mut self) -> PlayerId {
        let state = self.state_mut();
        let schedule = &mut state.turn_schedule;
        schedule.current = None;
        if !schedule.extra { schedule.normal_from = state.active; }
        let next = loop {
            let candidate = {
                let state = self.state_mut();
                let schedule = &mut state.turn_schedule;
                if let Some(turn) = schedule.turns.pop() {
                    if state.players[turn.player].lost { continue; }
                    schedule.extra = true;
                    let player = turn.player;
                    schedule.current = Some(turn);
                    player
                } else {
                    schedule.extra = false;
                    let n = state.players.len();
                    (1..=n).map(|i| (schedule.normal_from + i) % n)
                        .find(|p| !state.players[*p].lost).unwrap_or(state.active)
                }
            };
            // A skipped turn never begins: next-turn anchors remain waiting (614.10a).
            if self.skip_turn_candidate(candidate) {
                let schedule = &mut self.state_mut().turn_schedule;
                schedule.current = None;
                if !schedule.extra { schedule.normal_from = candidate; }
                continue;
            }
            break candidate;
        };
        let state = self.state_mut();
        let schedule = &mut state.turn_schedule;
        schedule.control = None;
        // All effects waiting for this player's next actual turn apply to this same turn.
        // The last control wins; extra turns made by each effect remain independent.
        let mut remaining = Vec::new();
        for pending in std::mem::take(&mut schedule.pending) {
            if state.players[pending.player].lost { continue; }
            if pending.player == next {
                if !state.players[pending.controller].lost { schedule.control = Some((next, pending.controller)); }
                schedule.turns.extend(pending.after);
            } else { remaining.push(pending); }
        }
        schedule.pending = remaining;
        next
    }
}
