//! One-shot zone changes with an "until" duration (CR 610.3). The return is another
//! one-shot effect, not a triggered ability. It happens before the next instruction.

use crate::replace::Choices;

use crate::{Game, ObjId, Zone};

#[derive(Debug, Clone)]
pub(crate) struct Return {
    pub until: ObjId,
    pub object: ObjId,
    pub zone: Zone,
}

impl Game {
    /// Capture the zone of each object before exiling it. If the specified event already
    /// happened, the initial zone change does not happen (610.3a-b).
    pub(crate) fn exile_until(&mut self, objects: &[ObjId], source: ObjId, ability: Option<crate::binding::AbilityKey>, generation: Option<usize>) -> Vec<(ObjId, ObjId)> {
        if self.obj(source).zone != Zone::Battlefield {
            return Vec::new();
        }
        let mut transitions = Vec::new();
        let before: Vec<_> = objects.iter().map(|o| (*o, self.obj(*o).zone)).collect();
        for (object, zone) in before {
            if zone == Zone::Gone || zone == Zone::Exile {
                continue;
            }
            if let Some(now) = self.exile_with(object, source, ability.clone(), generation).filter(|o| self.obj(*o).zone == Zone::Exile) {
                transitions.push((object, now));
                self.until.push(Return { until: source, object: now, zone });
            }
        }
        transitions
    }

    /// After the event's abilities have triggered, apply its second one-shot effects. New
    /// objects elsewhere in exile are never substituted for an object that left (400.7).
    pub(crate) fn begin_returns(&mut self) -> bool {
        if self.returning() {
            return false;
        }
        let mut ready = Vec::new();
        let mut waiting = Vec::new();
        for effect in std::mem::take(&mut self.until) {
            if self.obj(effect.object).zone != Zone::Exile {
                continue;
            }
            if self.obj(effect.until).zone == Zone::Gone {
                ready.push(effect);
            } else {
                waiting.push(effect);
            }
        }
        self.until = waiting;
        if ready.is_empty() {
            return false;
        }
        let from = self.log.end();
        let mut entries = Vec::new();
        let mut moves = Vec::new();
        for effect in ready {
            if effect.zone == Zone::Battlefield {
                entries.push(crate::entry::EntryRequest::card(effect.object, self.obj(effect.object).owner, Choices::default(), true));
            } else { moves.push((effect.object, effect.zone)); }
        }
        self.procedures.push(crate::procedure::Procedure::Returns { from, complete: false });
        let departures = crate::departure::Batch::new(self, moves.into_iter().map(|(object, zone)| (object, crate::departure::Kind::Move(zone))).collect());
        let preparation = crate::entry::prepare::Preparation::new(self, entries);
        self.procedures.push(crate::procedure::Procedure::Operation(crate::operation::Operation::Enter {
            ordering: None, preparation: Some(preparation), departures,
            payments: crate::replace::EntryPayments::default(),
        }));
        true
    }

    /// All answers belong to the child entry operation. The return consumes its completion
    /// once, without rebuilding its requests, repeating payments or collecting answers again.
    pub(crate) fn finish_returns(&mut self) {
        let Some(crate::procedure::Procedure::Returns { from, complete: true }) = self.procedures.pop() else { unreachable!("return operation resumes its parent") };
        let end = self.log.end();
        self.together.push((from, end));
        self.update_combat();
    }
}

/// The supported 610.3 lifetime: the source leaving the battlefield.
pub(crate) fn source_leaves(duration: &ir::effect::Duration) -> bool {
    let ir::effect::Duration::UntilEvent(pattern) = duration else { return false };
    !pattern.counted() && pattern.during.is_none() && pattern.condition.is_none() && pattern.cause.is_none()
        && matches!(pattern.subjects.as_slice(), [s] if s.quant == ir::Quant::Definite(ir::Ref::This))
        && matches!(pattern.actions.as_slice(), [ir::EventAction::Deed(d)] if d.verb == ir::Verb::LeaveBattlefield && !d.passive && d.args.is_empty())
}

impl crate::retention::Trace for Return {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        self.until.trace(visit);
        self.object.trace(visit);
    }
}
