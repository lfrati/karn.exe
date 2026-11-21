//! Sagas (714). A chapter symbol is a triggered ability ([`crate::trigger`]'s chapter watch,
//! 714.2b); what is here are the rules around it: a Saga enters with a lore counter (714.3a),
//! its controller puts another on it as their precombat main phase begins (714.3c), and one
//! that has read its final chapter is sacrificed once that chapter's ability has left the
//! stack (714.4). Each uses the ordinary counter operation, with its replacements and
//! triggers, and the ordinary state-based departure.
use crate::ability::View;
use crate::operation::Placement;
use crate::state::{Game, ObjId};
use ir::{Ability, Subtype};

/// Is this a Saga (714.1)?
pub(crate) fn is_saga(subtypes: &[Subtype]) -> bool {
    subtypes.contains(&ir::Subtype::saga())
}

impl Game {
    /// The chapter numbers of the chapter abilities this permanent has now (714.2a, 714.2c).
    pub(crate) fn chapters(&self, o: ObjId) -> Vec<u32> {
        let view = View::Effective { membership_changed: self.membership_changed() };
        self.origins(o, view).into_iter()
            .flat_map(|(_, def)| def.abilities.iter().filter_map(|a| match a { Ability::Chapter { chapters, .. } => Some(chapters.clone()), _ => None }))
            .flatten().collect()
    }

    /// Occurrences retain their chapter identity even if their source changes abilities.
    /// Waiting for targets/order and on-stack are both pending (714.4).
    fn chapter_pending(&self, o: ObjId) -> bool {
        self.triggered.iter().any(|t| t.source == Some(o)
            && t.captured.as_ref().is_some_and(|c| c.as_ref().is_ok_and(|c| c.chapter)))
            || self.stack.iter().any(|on| {
                let on = self.obj(*on);
                on.ability.is_some_and(|(source, _)| source == Some(o))
                    && on.invocations.as_ref().is_some_and(|i| i.as_ref().is_ok_and(|i| i.iter().any(|i| i.chapter)))
            })
    }

    /// Of a Saga entering the battlefield, the lore counter it enters with (714.3a). Read ahead
    /// (714.3b) is a keyword the engine refuses.
    pub(crate) fn saga_entry(&self, o: ObjId) -> Option<(String, u32)> {
        is_saga(&self.obj(o).chars.subtypes).then(|| ("lore".to_string(), 1))
    }

    /// As the active player's precombat main phase begins, they put a lore counter on each
    /// Saga they control with one or more chapter abilities (714.3c). A turn-based action,
    /// done with the ordinary counter operation; what it asks is answered before priority.
    pub(crate) fn saga_lore(&mut self) -> crate::Res<()> {
        let player = self.active;
        let placements: Vec<Placement> = self.battlefield.iter().copied()
            .filter(|o| self.controller(*o) == player && is_saga(&self.seen(*o, None).subtypes) && !self.chapters(*o).is_empty())
            .map(|on| Placement { on: on.into(), kind: "lore".to_string(), count: 1, by: player }).collect();
        if placements.is_empty() { return Ok(()); }
        let parent = self.procedures.len();
        if let Err(decision) = self.put_counter_batch(placements,crate::operation::EventOrigin::TurnBased)? {
            self.procedures.insert(parent, crate::procedure::Procedure::RuleAction { complete: false });
            self.decision = Some(decision);
        }
        Ok(())
    }

    /// Has this Saga read its final chapter (714.2d, 714.4)? As many lore counters as its
    /// greatest chapter number, and no chapter ability of it still to resolve.
    pub(crate) fn saga_finished(&self, o: ObjId) -> bool {
        if !is_saga(&self.seen(o, None).subtypes) { return false; }
        let Some(last) = self.chapters(o).into_iter().max() else { return false };
        self.obj(o).counters.get("lore").copied().unwrap_or(0) >= last && !self.chapter_pending(o)
    }
}
