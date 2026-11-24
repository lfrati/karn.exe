//! Target changes retain positions and the original targeting context (115.7, 115.9).
//! Required single changes use an offered legal alternative. Optional new targets share
//! the copy validator; they may keep targets that have since become illegal.
use crate::announcement::TargetSpec;
use crate::select::Ctx;
use crate::state::{Choice, TargetPolicy};
use crate::{Game, ObjId, Target, Zone};

impl Game {
    pub(crate) fn stack_target_metadata(&self, object: ObjId) -> (Vec<TargetSpec>, Vec<Ctx<'static>>) {
        let mut specs = Vec::new();
        let mut contexts = Vec::new();
        if let Some(Ok(invocations)) = &self.obj(object).invocations {
            for invocation in invocations {
                contexts.extend(std::iter::repeat_n(invocation.ctx.clone(), invocation.program.targets.len()));
                crate::announcement::append_targets(&mut specs, invocation.program.targets.clone());
            }
        } else if let Some(spec) = self.enchant_spec(object) {
            specs.push(spec.clone());
            contexts.push(Ctx { x: self.obj(object).x, ..Ctx::for_spell(self.obj(object).controller, object) });
        }
        (specs, contexts)
    }

    pub(crate) fn change_target_offer(&self, object: ObjId, policy: TargetPolicy) -> Option<Choice> {
        if self.obj(object).zone != Zone::Stack || !self.stack.contains(&object) {
            return None;
        }
        let original = self.obj(object).targets.clone();
        let (specs, contexts) = self.stack_target_metadata(object);
        if specs.len() != original.len() {
            return None;
        }
        let mut slots = self.target_offers_contexts(
            &contexts,
            &specs,
            &self.obj(object).bound,
            (policy == TargetPolicy::ChooseNew).then_some(original.as_slice()),
        );
        for (slot, before) in slots.iter_mut().zip(&original) {
            slot.min = before.len() as u32;
            slot.max = slot.min;
            // 115.5 applies to the live stack object, independently of the captured source
            // incarnation. Keep offers consistent with the final-set validator.
            slot.among.retain(|target| *target != Target::Object(object));
            if let Some(dependent) = &mut slot.dependent {
                dependent.options.retain(|(prior, _)| *prior != Target::Object(object));
                for (_, among) in &mut dependent.options {
                    among.retain(|target| *target != Target::Object(object));
                }
            }
        }
        if policy == TargetPolicy::RequiredSingle {
            if original.iter().map(Vec::len).sum::<usize>() != 1 {
                return None;
            }
            let (index, before) = original.iter().enumerate().find(|(_, g)| !g.is_empty()).unwrap();
            let slot = &mut slots[index];
            slot.among.retain(|target| *target != before[0] && *target != Target::Object(object));
            if slot.among.is_empty() {
                return None;
            } // 115.7a: leave even an illegal original.
        }
        Some(Choice::ChangeTargets { object, original, slots, policy })
    }

    pub(crate) fn legal_changed_targets(
        &self,
        object: ObjId,
        original: &[Vec<Target>],
        slots: &[crate::legal::Slot],
        policy: TargetPolicy,
        targets: &[Vec<Target>],
    ) -> Result<(), String> {
        if self.obj(object).zone != Zone::Stack || !self.stack.contains(&object) || self.obj(object).targets != original
        {
            return Err("the retained stack object or original targets no longer exist".into());
        }
        match policy {
            TargetPolicy::RequiredSingle => {
                if targets.len() != original.len()
                    || targets.iter().zip(original).any(|(a, b)| a.len() != b.len())
                    || targets == original
                    || original.iter().map(Vec::len).sum::<usize>() != 1
                {
                    return Err("change the single target to an offered different target (115.7a)".into());
                }
                if targets.iter().zip(slots).any(|(group, slot)| group.iter().any(|t| !slot.among.contains(t))) {
                    return Err("choose an offered legal alternative (115.7a)".into());
                }
                Ok(())
            }
            TargetPolicy::ChooseNew => {
                let (specs, contexts) = self.stack_target_metadata(object);
                self.legal_new_targets(object, &specs, &contexts, original, targets,
                    "new targets keep each target group's number of targets, chosen as it was put on the stack, and its division (601.2c, 115.7f)")
            }
        }
    }

    /// The ordinary distinct-value relation is proved for graveyard targets.
    /// Retaining an already-illegal member while changing its peers needs a
    /// separate cross-target LKI proof; preserve the unchanged/decline route.
    pub(crate) fn retained_target_relations(&self, object: ObjId, original: &[Vec<Target>], targets: &[Vec<Target>]) -> crate::Res<()> {
        if original == targets { return Ok(()); }
        let (specs, contexts) = self.stack_target_metadata(object);
        for (index, ((spec, before), after)) in specs.iter().zip(original).zip(targets).enumerate() {
            if before == after || spec.relation != Some(crate::announcement::TargetRelation::DifferentManaValues) { continue; }
            let values = crate::announcement::TargetValues { parent: &self.obj(object).bound, chosen: original, base: spec.base };
            if before.iter().zip(after).any(|(old, new)| old == new && !spec.set.legal_target(self, &contexts[index], &values, *old)) {
                return crate::unsupported("different mana values: changing peers of a retained illegal target requires cross-target LKI support");
            }
        }
        Ok(())
    }

    /// Shared 115.7d-e validator, extracted from the existing 707.10c copy path. `counts` is
    /// why the answer's shape is refused: what keeps the number of targets differs for a copy.
    pub(crate) fn legal_new_targets(
        &self,
        object: ObjId,
        specs: &[TargetSpec],
        contexts: &[Ctx],
        original: &[Vec<Target>],
        targets: &[Vec<Target>],
        counts: &str,
    ) -> Result<(), String> {
        if targets.len() != original.len() || targets.iter().zip(original).any(|(a, b)| a.len() != b.len()) {
            return Err(counts.into());
        }
        if targets == original {
            return Ok(());
        }
        // The specs are the object's own, and its target groups were announced one per spec
        // (601.2c) and copied with it: a difference is the engine's, never the answer's.
        assert_eq!(specs.len(), original.len(), "{object:?}: target specs and announced target groups differ");
        for (index, ((spec, group), before)) in specs.iter().zip(targets).zip(original).enumerate() {
            let ctx = &contexts[index];
            if group.iter().enumerate().any(|(n, t)| group[..n].contains(t)) {
                return Err("a target cannot be chosen twice in one group (115.3)".into());
            }
            if spec.distinct_from.iter().any(|i| *i >= index || targets[*i].iter().any(|t| group.contains(t))) {
                return Err("new targets must preserve distinct target groups (115.7d)".into());
            }
            let values =
                crate::announcement::TargetValues { parent: &self.obj(object).bound, chosen: targets, base: spec.base };
            let old_values = crate::announcement::TargetValues {
                parent: &self.obj(object).bound,
                chosen: original,
                base: spec.base,
            };
            for (old, new) in before.iter().zip(group) {
                if old != new && (*new == Target::Object(object) || !spec.legal_target(self, ctx, &values, group, *new))
                    || old == new
                        && spec.legal_target(self, ctx, &old_values, before, *old)
                        && !spec.legal_target(self, ctx, &values, group, *new)
                {
                    return Err(
                        "a changed target must be legal without invalidating an unchanged target (115.7d)".into()
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn commit_changed_targets(
        &mut self,
        object: ObjId,
        original: &[Vec<Target>],
        targets: Vec<Vec<Target>>,
    ) {
        if self.obj(object).zone != Zone::Stack || !self.stack.contains(&object) {
            return;
        }
        let mut new = Vec::new();
        for target in targets.iter().flatten() {
            if !original.iter().flatten().any(|old| old == target) && !new.contains(target) {
                new.push(*target);
            }
        }
        self.obj_mut(object).targets = targets;
        // Becoming targeted belongs to the original stack object/controller, not the changer.
        for target in new {
            let ordinal = self.note_targeted(target, object);
            self.log.push(crate::Event::BecameTarget { object: target, of: object, ordinal });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::{ResponseError, Simulation};
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Game, Move, ObjId, Target, Zone};

    /// Player 0's Shock at player 1, copied by player 1's Twincast: the copy's new targets
    /// are asked of player 1.
    fn retargeting() -> (Game, ObjId) {
        let mut g = Game::new(vec![
            CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
            CardDef::new("Twincast", "{U}{U}", "Instant",
                "Copy target instant or sorcery spell. You may choose new targets for the copy.", None).unwrap(),
        ], 2, 20);
        let (shock, twincast) = (g.put(0, 0, Zone::Hand), g.put(1, 1, Zone::Hand));
        for p in 0..2 { g.players[p].pool.colors = [9; 5]; }
        g.act(0, Move::Paying(crate::state::Pool { colors:[0,0,0,1,0], ..Default::default() }, Box::new(Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] }))).unwrap();
        let spell = *g.stack.last().unwrap();
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Paying(crate::state::Pool { colors:[0,2,0,0,0], ..Default::default() }, Box::new(Move::Cast { card: twincast, targets: vec![vec![Target::Object(spell)]] }))).unwrap();
        while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { player: 1, choice: Choice::Retarget { copy, .. } }) = g.decision.clone() else {
            panic!("{:?}", g.decision)
        };
        (g, copy)
    }

    fn answer(sim: &mut Simulation, targets: Vec<Vec<Target>>) -> Result<(), ResponseError> {
        let id = sim.request().unwrap().unwrap().id;
        sim.respond(id, 1, Move::Choose(Answer::Targets(targets)))
    }

    /// A malformed answer is the player's: refused, and the game goes on.
    #[test]
    fn new_targets_of_another_shape_are_refused() {
        let (g, _) = retargeting();
        let mut sim = Simulation::new(g);
        for targets in [vec![], vec![vec![Target::Player(0)], vec![Target::Player(1)]], vec![vec![Target::Player(0), Target::Player(1)]]] {
            let refused = answer(&mut sim, targets);
            assert!(matches!(&refused, Err(ResponseError::Illegal(why)) if why.contains("retains target counts and division (115.7f")), "{refused:?}");
        }
        answer(&mut sim, vec![vec![Target::Player(0)]]).unwrap();
        assert!(sim.game().is_ok());
    }

    /// Target specs that no longer match the copy's announced targets are the engine's fault:
    /// the simulation ends with it, rather than refusing the player.
    #[test]
    fn target_specs_that_do_not_match_the_announced_targets_are_an_engine_fault() {
        let (mut g, copy) = retargeting();
        g.obj_mut(copy).invocations = None;
        let mut sim = Simulation::new(g);
        let Err(ResponseError::Fault(fault)) = answer(&mut sim, vec![vec![Target::Player(0)]]) else { panic!("not a fault") };
        assert!(fault.message.contains("target specs and announced target groups differ"), "{}", fault.message);
        assert_eq!(sim.game().unwrap_err(), fault);
    }
}
