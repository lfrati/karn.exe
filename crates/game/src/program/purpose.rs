//! What objects chosen as an instruction is followed are chosen for (608.2d): the action the
//! text goes on to do to them, and for a zone change, where they go. "You choose a nonland card
//! from it. That player discards that card": the card is chosen to be discarded.
use super::action::{self, Destination, Operand};
use super::{Dest, Instr, Slot};
use crate::state::Placement;
use ir::Verb;

/// Whether the text says "return" or "put" of the objects selected into `selected`, when a
/// zone change after `from` moves them.
pub(super) fn moved_as(instrs: &[Instr], from: usize, selected: Slot) -> Option<ir::action::MoveVerb> {
    let mut names = vec![selected];
    for instr in instrs.iter().skip(from + 1) {
        let reads = |operands: &[&Operand]| operands.iter().any(|o| matches!(o, Operand::Reference(s) if names.contains(s)));
        match instr {
            Instr::Subjects { cards, out, .. } | Instr::Gather { objects: Operand::Reference(cards), out } if names.contains(cards) => names.push(*out),
            Instr::Action { action: action::Action::Move { groups, said }, .. }
                if groups.iter().any(|(objects, _)| reads(&objects.iter().collect::<Vec<_>>())) => return Some(*said),
            Instr::Action { action, .. } if reads(&action.operands()) => return None,
            _ => {}
        }
    }
    None
}

/// The action done to the objects selected into `selected`, by the first instruction after
/// `from` that reads them, directly or as the subjects made of them; None if none does.
pub(super) fn chosen_for(instrs: &[Instr], from: usize, selected: Slot) -> Option<(Verb, Option<Placement>)> {
    let mut names = vec![selected];
    for instr in instrs.iter().skip(from + 1) {
        let reads = |operands: &[&Operand]| operands.iter().any(|o| matches!(o, Operand::Reference(s) if names.contains(s)));
        match instr {
            Instr::Subjects { cards, out, .. } | Instr::Gather { objects: Operand::Reference(cards), out } if names.contains(cards) => names.push(*out),
            Instr::Choice { action, .. } if reads(&action.operands()) => return Some((action.verb().said(), action.verb().to())),
            Instr::Place { objects, position, .. } if reads(&objects.iter().collect::<Vec<_>>()) => return Some((Verb::Move, library(position))),
            Instr::Move { src, to, .. } if names.contains(src) => return Some((Verb::Move, Some(dest(to)))),
            Instr::MoveTogether {parts,..} => {
                if let Some((_,to))=parts.iter().find(|(src,_)|names.contains(src)) { return Some((Verb::Move,Some(dest(to)))); }
            }
            Instr::Action { action: action::Action::Move { groups, .. }, .. } => {
                if let Some((_, to)) = groups.iter().find(|(objects, _)| reads(&objects.iter().collect::<Vec<_>>())) {
                    return Some((Verb::Move, Some(destination(to))));
                }
            }
            // the top cards of a library a "you may" offers to exile, reveal or look at
            Instr::Exile { from, .. } | Instr::Show { from, .. } if names.contains(from) => return shown(instr),
            Instr::Action { action, .. } if reads(&action.operands()) => return None,
            _ => {}
        }
    }
    None
}

/// What exiling or showing cards does: to exile them is to move them to exile (701.13a); to
/// show them to everyone is to reveal them (701.20a), and otherwise, that they are looked at.
fn shown(instr: &Instr) -> Option<(Verb, Option<Placement>)> {
    match instr {
        Instr::Exile { .. } => Some((Verb::Move, Some(Placement::Zone(crate::Zone::Exile)))),
        Instr::Show { to: super::Audience::Everyone, .. } => Some((Verb::Reveal, None)),
        Instr::Show { .. } => Some((Verb::LookAt, None)),
        _ => None,
    }
}

/// What a "may" asked as `instr` is followed lets its player do (608.2d): the action, and for a
/// zone change where to. None for an instruction that is no such action.
pub(super) fn done_by(instr: &Instr) -> Option<(Verb, Option<Placement>)> {
    use action::Action as A;
    Some(match instr {
        Instr::Action { action, .. } => match action {
            A::BecomeMonarch { .. } => (Verb::BecomeMonarch, None),
            A::Destroy { .. } => (Verb::Destroy, None),
            A::Counter { .. } => (Verb::Counter, None),
            // to exile is to move to exile (701.13a), as a targeted exile is said
            A::ExileUntil { .. } => (Verb::Move, Some(Placement::Zone(crate::Zone::Exile))),
            // one destination for all that is moved, or none said
            A::Move { groups, .. } => (Verb::Move, match groups.as_slice() {
                [(_, to), rest @ ..] if rest.iter().all(|(_, other)| destination(other) == destination(to)) => Some(destination(to)),
                _ => None,
            }),
            A::Draw { .. } => (Verb::Draw, None),
            A::Mill { .. } => (Verb::Mill, None),
            A::GainLife { .. } => (Verb::GainLife, None),
            A::LoseLife { .. } => (Verb::LoseLife, None),
            A::Tap { tapped: true, .. } => (Verb::Tap, None),
            A::Tap { tapped: false, .. } => (Verb::Untap, None),
            A::Transform { .. } => (Verb::Transform, None),
            A::TurnFace { .. } => (Verb::TurnFace, None),
            A::Fight { .. } => (Verb::Fight, None),
            A::Damage { .. } | A::DealDivided { .. } => (Verb::DealDamage, None),
            A::RemoveCounters { .. } => (Verb::RemoveCounters, None),
            A::PutCounters { .. } => (Verb::PutCounters, None),
            A::Distribute { .. } => (Verb::Distribute, None),
            A::Create { .. } => (Verb::Create, None),
            A::Regenerate { .. } => (Verb::Regenerate, None),
            A::Attach { .. } => (Verb::Attach, None),
            A::Pair { .. } => (Verb::Pair, None),
            A::IncreaseSpeed { .. } | A::GetEmblem { .. } | A::Level { .. } | A::Unprepare { .. } | A::Designate { .. } | A::Prevent { .. } => return None,
        },
        Instr::Choice { action, .. } => (action.verb().said(), action.verb().to()),
        Instr::Place { position, .. } => (Verb::Move, library(position)),
        Instr::Move { to, .. } => (Verb::Move, Some(dest(to))),
        Instr::MoveTogether { parts, .. } => (Verb::Move, match parts.as_slice() {
            [(_, to), rest @ ..] if rest.iter().all(|(_, other)| dest(other) == dest(to)) => Some(dest(to)),
            _ => None,
        }),
        Instr::Search { .. } => (Verb::Search, None),
        Instr::Shuffle { .. } => (Verb::Shuffle, None),
        Instr::Observe { .. } => (Verb::Reveal, None),
        Instr::Exile { .. } | Instr::Show { .. } => return shown(instr),
        Instr::Arrange { verb, .. } => (*verb, None),
        Instr::Explore { .. } => (Verb::Explore, None),
        Instr::Connive { .. } => (Verb::Connive, None),
        Instr::Proliferate { .. } => (Verb::Proliferate, None),
        _ => return None,
    })
}

/// What the "you may" asked by the gate at `at` lets its player do: what the first instruction
/// it governs does. The instructions that only test the answer or bind what that one acts on
/// come first: the player that does it, "it" of "you may attach this Equipment to it", the
/// top cards of a library, which taking changes nothing.
pub(super) fn gated(instrs: &[Instr], at: usize) -> Option<(Verb, Option<Placement>)> {
    instrs.iter().skip(at + 1)
        .find(|i| !matches!(i, Instr::Branch { .. } | Instr::SkipUnless { .. } | Instr::Capture { .. } | Instr::Input { .. }
            | Instr::Gather { .. } | Instr::Subjects { .. } | Instr::TakeTop { .. }))
        .and_then(done_by)
}

fn library(position: &ir::Position) -> Option<Placement> {
    match position {
        ir::Position::One(p) => Some(Placement::Library(*p)),
        ir::Position::Choice { .. } | ir::Position::BeneathTopX => None,
    }
}

fn dest(to: &Dest) -> Placement {
    match to {
        Dest::Hand => Placement::Zone(crate::Zone::Hand),
        Dest::Graveyard => Placement::Zone(crate::Zone::Graveyard),
        Dest::Library { position, .. } => Placement::Library(*position),
        Dest::Battlefield(_) => Placement::Zone(crate::Zone::Battlefield),
    }
}

fn destination(to: &Destination) -> Placement {
    match to {
        Destination::Zone(zone) => Placement::Zone(*zone),
        Destination::Battlefield { .. } => Placement::Zone(crate::Zone::Battlefield),
    }
}
