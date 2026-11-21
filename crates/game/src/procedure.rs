//! The continuation stack is separate from the Magic stack. A parent remains below its
//! child; only the top procedure owns a decision. Resolution, entry/counter operations
//! and immediate returns all retain their progress here (608, 610.3, 614–616).
use crate::operation::Operation;
use crate::state::{Game, Resolving};

#[derive(Debug, Clone)]
pub(crate) enum Procedure {
    Opening(crate::start::Actions),
    Resolution(Resolving),
    Payment(crate::payment::Payment),
    AttackDeclaration(crate::combat::Declaration),
    RuleAction { complete: bool },
    Cleanup { complete: bool },
    StateBased(crate::state::StateBased),
    Operation(Operation),
    Land { player: crate::PlayerId, result: Option<crate::operation::Completed>, library_tops: Vec<Option<crate::ObjId>> },
    Returns { from: crate::event::Seq, complete: bool },
}
#[derive(Default)]
pub(crate) struct Progress {
    pub(crate) unused_mana_answers: usize,
    /// A mana answer said for a question asked here was not one it offers (605.3b).
    pub(crate) unoffered_mana_answer: bool,
    pub(crate) state_based: bool,
    pub(crate) payment_error: Option<String>,
}
impl Game {
    /// Retire delivered events only after every rules consumer has finished with them.
    /// A suspended action keeps its open simultaneous batch; completed instructions do not.
    pub fn retire_events(&mut self, delivered: crate::event::Seq) -> usize {
        assert!(delivered <= self.log.end(), "cannot deliver future events");
        let mut floor = delivered.min(self.seen);
        for procedure in &self.procedures {
            let retained = match procedure {
                Procedure::Resolution(at) => at.execution.as_ref().and_then(|execution| execution.event_floor()),
                Procedure::Returns { from, .. } => Some(*from),
                Procedure::AttackDeclaration(at) => Some(at.from),
                _ => None,
            };
            if let Some(from) = retained { floor = floor.min(from); }
        }
        // A previously acknowledged response is harmless; it cannot resurrect old events.
        let floor = floor.max(self.log.start());
        self.log.retire_before(floor)
    }

    /// The active procedure owns the answer; no search through suspended parents.
    pub(crate) fn answer_procedure(&mut self, player: crate::PlayerId, answer: crate::state::Answer) -> crate::Res<()> {
        if self.operation_asks() { return self.answer_operation(player, answer); }
        match self.procedures.last_mut().expect("a decision has an owning procedure") {
            Procedure::Opening(at) => at.answer(answer),
            Procedure::Resolution(at) => at.answer(answer),
            Procedure::Payment(payment) => payment.answer(answer),
            Procedure::AttackDeclaration(at) => at.answer = Some(answer),
            Procedure::StateBased(state) => state.answer(player, answer),
            _ => unreachable!("this procedure has no direct player decision"),
        }
        Ok(())
    }
    /// Read-only view of an in-progress stack resolution, including while a child waits.
    pub fn resolving(&self) -> Option<&Resolving> {
        self.procedures.iter().rev().find_map(|p| if let Procedure::Resolution(r) = p { Some(r) } else { None })
    }
    pub(crate) fn start_resolution(&mut self, resolution: Resolving) {
        self.procedures.push(Procedure::Resolution(resolution));
    }
    /// Deliver a completed child only to its immediate parent.
    pub(crate) fn deliver_operation(&mut self, result: crate::operation::Completed) {
        match self.procedures.last_mut().expect("operation has a retained parent") {
            Procedure::Opening(at) => at.completed(Some(&result)),
            Procedure::Resolution(at) => at.execution.as_mut().expect("resolution plan").operation_complete(result),
            Procedure::Land { result: destination, .. } => {
                assert!(destination.is_none());
                *destination = Some(result);
            }
            Procedure::RuleAction { complete } | Procedure::Cleanup { complete } | Procedure::Returns { complete, .. } => { assert!(!*complete); *complete = true; }
            Procedure::Payment(payment) => payment.operation_complete(result),
            Procedure::AttackDeclaration(_) => unreachable!("attack declaration does not call operations directly"),
            Procedure::StateBased(_) => unreachable!("SBA batches do not directly call replacement operations"),
            Procedure::Operation(operation) => operation.child_complete(result),
        }
    }
    /// Whether an effect's payment was made, delivered to the resolution that asked for it.
    pub(crate) fn deliver_payment(&mut self, paid: bool) {
        match self.procedures.last_mut() {
            Some(Procedure::Resolution(at)) => at.execution.as_mut().expect("resolution plan").payment_complete(paid),
            Some(Procedure::AttackDeclaration(at)) => at.paid = Some(paid),
            _ => unreachable!("an effect's payment has the resolution that asked for it directly below"),
        }
    }
    /// The offer the resolution on top is asking about was taken by beginning a cast.
    pub(crate) fn cast_begun(&mut self) {
        match self.procedures.last_mut() {
            Some(Procedure::Resolution(at)) => at.execution.as_mut().expect("resolution plan").cast_begun(),
            _ => unreachable!("an effect's offer to cast is asked by the resolution on top"),
        }
    }
    pub(crate) fn operation(&self) -> Option<&Operation> {
        match self.procedures.last() { Some(Procedure::Operation(operation)) => Some(operation), _ => None }
    }
    pub(crate) fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.procedures.iter().filter_map(|p| if let Procedure::Operation(operation) = p { Some(operation) } else { None })
    }
    pub(crate) fn push_operation(&mut self, operation: Operation) { self.procedures.push(Procedure::Operation(operation)); }
    pub(crate) fn take_operation(&mut self) -> Operation {
        match self.procedures.pop() { Some(Procedure::Operation(operation)) => operation, _ => unreachable!("the owning operation must be the top continuation") }
    }
    pub(crate) fn returning(&self) -> bool { self.procedures.iter().any(|p| matches!(p, Procedure::Returns { .. })) }
}
