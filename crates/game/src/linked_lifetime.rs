//! Acquired linked relationships have a lifetime separate from stable ability-use keys.
//! Whole activated families are acquired/lost together by the supported inheritance owner.
use crate::{Game, ObjId, Zone};
use crate::binding::{AbilityKey, Origin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Acquisition {
    pub(crate) source: ObjId,
    origin: Origin,
    current: Option<usize>,
    // Ended generations cannot be revived by a reader or a late old writer. Intact
    // departed-source generations remain available to retained occurrences (608.2h).
    generations: Vec<bool>,
}

impl Game {
    /// Read current membership once, then reconcile. Queries themselves never create a
    /// relationship or alter its history. Call after a complete semantic simultaneous act.
    pub(crate) fn reconcile_linked_acquisitions(&mut self) {
        if self.linked_acquisitions.is_empty() && !self.membership_changed() { return; }
        let active = {
            let sources = self.battlefield.iter().copied().chain(self.linked_acquisitions.iter()
                .filter(|a| self.get_object(a.source).is_some_and(|o| o.zone == Zone::Gone)).map(|a| a.source));
            let mut active = Vec::new();
            for source in sources {
                let members = self.members(source);
                for member in &members {
                    let Origin::Inherited(family) = &member.key.origin else { continue };
                    let definition = family.definition.def();
                    let activated = |index| matches!(definition.abilities.iter()
                        .chain(definition.keyword_abilities.iter().map(|(_, a)| a)).nth(index), Some(ir::Ability::Activated { .. }));
                    // The retained definition may also contain trigger/static readers;
                    // those are not acquired by this activated-member owner.
                    let pairs: Vec<_> = definition.programs.iter().filter(|row| activated(row.ability)).flat_map(|row|
                        row.linked_exile_writers.iter().copied().filter(|writer| activated(*writer))
                            .map(move |writer| (writer, row.ability))).collect();
                    if pairs.is_empty() { continue; }
                    // Both members must survive: an unrelated remaining row cannot keep
                    // the pair linked. Selective independent pair loss needs a finer owner
                    // before admission; current inheritance acquires the entire family.
                    let complete = pairs.iter().all(|(writer, reader)| [writer, reader].into_iter().all(|index|
                        members.iter().any(|m| m.key == AbilityKey { origin: member.key.origin.clone(), index: *index })));
                    let key = (source, member.key.origin.clone());
                    if complete && !active.contains(&key) { active.push(key); }
                }
            }
            active
        };
        // Reconciliation without a transition does not invalidate query answers.
        if self.linked_acquisitions.iter().all(|a| a.current.is_some() == active.contains(&(a.source, a.origin.clone())))
            && active.iter().all(|(source, origin)| self.linked_acquisitions.iter().any(|a| a.source == *source && a.origin == *origin)) {
            return;
        }
        let acquisitions = &mut self.state_mut().linked_acquisitions;
        for acquisition in acquisitions.iter_mut() {
            if !active.contains(&(acquisition.source, acquisition.origin.clone()))
                && let Some(generation) = acquisition.current.take() {
                acquisition.generations[generation] = false;
            }
        }
        for (source, origin) in active {
            match acquisitions.iter_mut().find(|a| a.source == source && a.origin == origin) {
                Some(a) if a.current.is_none() => {
                    a.current = Some(a.generations.len());
                    a.generations.push(true);
                },
                Some(_) => {},
                None => acquisitions.push(Acquisition { source, origin, current: Some(0), generations: vec![true] }),
            }
        }
    }

    pub(crate) fn link_generation(&self, source: Option<ObjId>, key: Option<&AbilityKey>) -> Option<usize> {
        let (source, key) = (source?, key?);
        self.linked_acquisitions.iter().find(|a| a.source == source && a.origin == key.origin).and_then(|a| a.current)
    }
    pub(crate) fn linked_generation_live(&self, source: ObjId, key: &AbilityKey, generation: Option<usize>) -> bool {
        match &key.origin {
            Origin::Inherited(_) => generation.is_some_and(|generation| self.linked_acquisitions.iter()
                .find(|a| a.source == source && a.origin == key.origin)
                .is_some_and(|a| a.generations.get(generation) == Some(&true))),
            _ => true,
        }
    }
}
