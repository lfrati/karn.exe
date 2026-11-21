//! The turn (CR 500–514): its phases and their steps, in the order they happen.
//!
//! Declared once. The engine walks this order and the text's names for parts of the turn
//! ([`crate::event::Step`]: "upkeep", "main phase", "combat") are said of these.

use crate::event::Step;
use serde::Serialize;

/// "A turn consists of five phases, in this order: beginning, precombat main, combat,
/// postcombat main, and ending" (500.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

impl Phase {
    /// The phases of a turn, in order (500.1).
    pub const ORDER: [Phase; 5] = [Phase::Beginning, Phase::PrecombatMain, Phase::Combat, Phase::PostcombatMain, Phase::Ending];

    /// The parts of this phase, in order: its steps (501.1, 506.1, 512.1), or a main phase
    /// itself, which has none (505.2). Every main phase but the first of a turn is a
    /// postcombat main phase (505.1a), additional ones included.
    pub fn parts(self) -> &'static [Part] {
        match self {
            Phase::Beginning => &[Part::Untap, Part::Upkeep, Part::Draw],
            Phase::PrecombatMain => &[Part::PrecombatMain],
            Phase::Combat => &[Part::BeginningOfCombat, Part::DeclareAttackers, Part::DeclareBlockers, Part::CombatDamage, Part::EndOfCombat],
            Phase::PostcombatMain => &[Part::PostcombatMain],
            Phase::Ending => &[Part::End, Part::Cleanup],
        }
    }

    pub fn main(self) -> bool {
        matches!(self, Phase::PrecombatMain | Phase::PostcombatMain)
    }
}

/// Where a turn is: a step, or a main phase, which has no steps (505.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Part {
    /// 502.
    Untap,
    /// 503.
    Upkeep,
    /// 504.
    Draw,
    /// 505.
    PrecombatMain,
    /// 507.
    BeginningOfCombat,
    /// 508.
    DeclareAttackers,
    /// 509.
    DeclareBlockers,
    /// 510.
    CombatDamage,
    /// 511.
    EndOfCombat,
    /// 505.
    PostcombatMain,
    /// 513.
    End,
    /// 514.
    Cleanup,
}

impl Part {
    /// The parts of a turn in order: the beginning phase's untap, upkeep and draw steps
    /// (501.1), the precombat main phase, the combat phase's five steps (506.1), the postcombat
    /// main phase, the ending phase's end and cleanup steps (512.1).
    pub const ORDER: [Part; 12] = [
        Part::Untap,
        Part::Upkeep,
        Part::Draw,
        Part::PrecombatMain,
        Part::BeginningOfCombat,
        Part::DeclareAttackers,
        Part::DeclareBlockers,
        Part::CombatDamage,
        Part::EndOfCombat,
        Part::PostcombatMain,
        Part::End,
        Part::Cleanup,
    ];

    pub fn phase(self) -> Phase {
        match self {
            Part::Untap | Part::Upkeep | Part::Draw => Phase::Beginning,
            Part::PrecombatMain => Phase::PrecombatMain,
            Part::BeginningOfCombat
            | Part::DeclareAttackers
            | Part::DeclareBlockers
            | Part::CombatDamage
            | Part::EndOfCombat => Phase::Combat,
            Part::PostcombatMain => Phase::PostcombatMain,
            Part::End | Part::Cleanup => Phase::Ending,
        }
    }

    /// Is it a main phase, in which the active player may play a land and cast spells that are
    /// not instants while the stack is empty (505.6a, 505.6b, 117.1a)?
    pub fn main(self) -> bool {
        matches!(self, Part::PrecombatMain | Part::PostcombatMain)
    }

    /// Do players receive priority in it? Not during the untap step (502.4), and normally not
    /// during the cleanup step (514.3).
    pub fn priority(self) -> bool {
        !matches!(self, Part::Untap | Part::Cleanup)
    }

    /// Is this part what the text calls `step`? "Main phase" is either of the two (505.1);
    /// "combat" in "at the beginning of combat" is the beginning of combat step (507). The
    /// turn itself is not a part.
    pub fn named(self, step: Step) -> bool {
        match step {
            Step::Turn => false,
            Step::Untap => self == Part::Untap,
            Step::Upkeep => self == Part::Upkeep,
            Step::Draw => self == Part::Draw,
            Step::FirstMain => self == Part::PrecombatMain,
            Step::Combat => self == Part::BeginningOfCombat,
            Step::DeclareAttackers => self == Part::DeclareAttackers,
            Step::DeclareBlockers => self == Part::DeclareBlockers,
            Step::SecondMain => self == Part::PostcombatMain,
            Step::Main => self.main(),
            Step::EndOfCombat => self == Part::EndOfCombat,
            Step::End => self == Part::End,
            Step::Cleanup => self == Part::Cleanup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r500_1_five_phases_in_order() {
        let mut phases: Vec<Phase> = Part::ORDER.iter().map(|p| p.phase()).collect();
        phases.dedup();
        assert_eq!(
            phases,
            [Phase::Beginning, Phase::PrecombatMain, Phase::Combat, Phase::PostcombatMain, Phase::Ending]
        );
    }

    #[test]
    fn r500_1_each_phase_has_its_parts_in_turn_order() {
        let walked: Vec<Part> = Phase::ORDER.iter().flat_map(|p| p.parts().iter().copied()).collect();
        assert_eq!(walked, Part::ORDER);
        assert!(Phase::ORDER.iter().all(|p| p.parts().iter().all(|part| part.phase() == *p)));
    }

    #[test]
    fn r505_1_the_main_phase_is_either_of_the_two() {
        let named: Vec<Part> = Part::ORDER.iter().copied().filter(|p| p.named(Step::Main)).collect();
        assert_eq!(named, [Part::PrecombatMain, Part::PostcombatMain]);
        assert!(Part::PrecombatMain.named(Step::FirstMain) && !Part::PostcombatMain.named(Step::FirstMain));
    }
}
