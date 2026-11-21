//! Construction from declared rule facts. No historical instruction is executed.
use super::*;

/// A retained zone-change result is identified by its rule producer, never a VM slot.
/// The compiler's validator proves that the delayed body needs no other old locals.
fn exiled_delay(parent: &Program, clause: usize) -> Res<(Vec<(Slot, Kind)>, &Instr)> {
    let mut exiled = None;
    let mut bindings=Vec::new();
    let mut found = 0;
    if parent.targets.len()!=1 {return unsupported("initial delayed exile producer requires one target group");}
    for instruction in &parent.instrs {
        match instruction {
            Instr::Capture { operand: action::Operand::Targets(0), out } => {bindings.push((*out,Kind::Subjects));},
            Instr::Action { action: action::Action::Move { groups, .. }, created: Some(out), attempt }
                if exiled.is_none() && attempt.as_ref().is_none_or(|attempt|!attempt.optional && !attempt.payment)
                    && groups.iter().all(|(_, to)| matches!(to, action::Destination::Zone(Zone::Exile))) => {
                    exiled = Some(*out);
                    bindings.push((*out,Kind::Cards));
                },
            Instr::Subjects {cards,out,still_in:None|Some(Zone::Exile)} if bindings.contains(&(*cards,Kind::Cards)) => {
                bindings.push((*out,Kind::Subjects));
            },
            Instr::Delayed { program, subjects, .. } if subjects.is_empty() => {
                found += 1;
                if found != clause { continue; }
                let slot = exiled.ok_or_else(|| crate::Unsupported("initial delayed ability has no fixed exile result".into()))?;
                // Normalize "that card" to its retained public-zone result. This
                // profile never needs the original target's characteristics or
                // a historical transition graph: it only returns that result.
                if program.instrs.iter().any(|instruction|match instruction {
                    Instr::Action {action:action::Action::Move{groups,..},..}=>groups.iter().any(|(objects,to)|
                        objects.iter().any(|object|!matches!(object,action::Operand::Reference(slot) if bindings.iter().any(|(bound,_)|slot==bound)))
                        || !matches!(to,action::Destination::Battlefield {counters,attacking:None,face_down:None,..} if counters.iter().all(|(_,amount)|amount.as_constant().is_some()))),
                    Instr::Subjects{..}=>false,
                    _=>true,
                }) {return unsupported("initial delayed body requires more than a fixed retained-card return");}
                let mut initial = vec![None; program.initial.len()];
                if slot >= initial.len() { return unsupported("initial delayed exile result is not captured"); }
                for (slot,kind) in &bindings {if *slot<initial.len() {initial[*slot]=Some(*kind);}}
                // This checks every branch and nested body, including values used only
                // after a later choice. It does not run any operation or make an event.
                Program::with_initial(program.instrs.clone(), program.acts.clone(), program.targets.clone(), initial)
                    .map_err(|_| crate::Unsupported("initial delayed body requires other historical bindings".into()))?;
                return Ok((bindings, instruction));
            },
            _ => return unsupported("initial delayed producer requires fixed exile/capture instructions"),
        }
    }
    unsupported("initial delayed clause unavailable")
}

pub(crate) fn delayed(game: &Game, source: ObjId, paragraph: usize, controller: usize,
    clause: usize, exiled: &[ObjId]) -> Res<crate::state::Delayed> {
    let def = game.def(source).ok_or_else(|| crate::Unsupported("initial delayed definition unavailable".into()))?;
    let index = paragraph.checked_sub(1).ok_or_else(|| crate::Unsupported("initial delayed paragraph must be positive".into()))?;
    if game.obj(source).zone!=Zone::Gone || !def.chars().types.iter().any(|kind|matches!(kind,ir::CardType::Instant|ir::CardType::Sorcery))
        || def.back.is_some() || !def.halves.is_empty() || def.is_room()
        || game.obj(source).copy_acquisition.is_some() || !matches!(def.abilities.get(index), Some(ir::Ability::Text(_))) {
        return unsupported("initial delayed producer requires an immutable printed spell");
    }
    let parent = definition_program(def, index).ok_or_else(|| crate::Unsupported("initial delayed program unavailable".into()))??;
    if parent.targets.len()!=1 || exiled.len()>parent.targets[0].bounds.1 as usize {
        return unsupported("initial retained exile group requires unrepresented split-object history");
    }
    let (bindings, instruction) = exiled_delay(&parent, clause)?;
    let Instr::Delayed { pattern, subjects: _, program, lasts, once } = instruction else { unreachable!() };
    if pattern.initial_step().is_none() {return unsupported("initial delayed timing requires an unqualified next step");}
    if controller >= game.players.len() || clause == 0 || exiled.iter().any(|id| game.obj(*id).zone != Zone::Exile)
        || exiled.iter().copied().collect::<std::collections::BTreeSet<_>>().len() != exiled.len() {
        return unsupported("invalid initial delayed retained exile identities");
    }
    let ctx = Ctx::for_spell(controller, source).with_ability(index);
    let mut values = vec![None; parent.slots];
    for (slot,kind) in bindings {
        values[slot]=Some(match kind {Kind::Cards=>Value::Cards(exiled.to_vec()),Kind::Subjects=>Value::Subjects(exiled.iter().copied().map(Target::Object).collect()),_=>unreachable!()});
    }
    let captured = Captured::checked_sparse(ctx,program.clone(),values)?;
    Ok(crate::state::Delayed { source, controller, pattern: pattern.clone(), subjects: Vec::new(), captured,
        bound: crate::state::Bound { linked_ability: Some(crate::binding::AbilityKey::printed(index)), ..Default::default() },
        lasts: *lasts, once: *once })
}

/// Recover only authenticated definition-owned bindings, never publish register numbers.
pub(crate) fn delayed_exile_facts(game: &Game, delayed: &crate::state::Delayed) -> Option<(usize, usize, Vec<ObjId>)> {
    let ctx = delayed.captured.context();
    let key = ctx.ability.as_ref()?;
    if ctx.source != Some(delayed.source) || ctx.controller != delayed.controller || !matches!(key.origin, crate::binding::Origin::Own) { return None; }
    let parent = definition_program(game.def(delayed.source)?, key.index)?.ok()?;
    let mut clause = 0;
    for instruction in &parent.instrs {
        if let Instr::Delayed { program, pattern, lasts, once, .. } = instruction {
            clause += 1;
            if !super::same_instance(program, delayed.captured.program()) { continue; }
            let (bindings, _) = exiled_delay(&parent, clause).ok()?;
            if pattern != &delayed.pattern || lasts != &delayed.lasts || once != &delayed.once || !delayed.subjects.is_empty() { return None; }
            let slot=bindings.iter().find(|(_,kind)|*kind==Kind::Cards)?.0;
            let Some(Value::Cards(objects)) = delayed.captured.values.get(slot)?.as_ref() else { return None; };
            for (slot,kind) in bindings {
                let expected=match kind {Kind::Cards=>Value::Cards(objects.clone()),Kind::Subjects=>Value::Subjects(objects.iter().copied().map(Target::Object).collect()),_=>return None};
                let value=delayed.captured.values.get(slot)?.as_ref()?;
                if value!=&expected {
                    // Actual resolutions may retain the pre-exile target and
                    // its explicit successful public-zone transition (400.7j).
                    let Value::Subjects(subjects)=value else {return None};
                    let resolved=subjects.iter().map(|subject|match subject {Target::Object(was)=>Some(Target::Object(delayed.bound.became.iter().find(|(old,_)|old==was).map_or(*was,|(_,now)|*now))),_=>None}).collect::<Option<Vec<_>>>()?;
                    if Value::Subjects(resolved)!=expected {return None;}
                }
            }
            return Some((key.index + 1, clause, objects.clone()));
        }
    }
    None
}

pub(crate) fn verify_delayed_inventory(game:&Game)->Res<()> {
    let mut checked=std::collections::BTreeSet::new();
    for delayed in &game.delayed {
        let ctx=delayed.captured.context();
        let index=ctx.ability.as_ref().ok_or_else(||crate::Unsupported("initial delayed ability identity unavailable".into()))?.index;
        if !checked.insert((delayed.source,index)) {continue;}
        let parent=definition_program(game.def(delayed.source).ok_or_else(||crate::Unsupported("initial delayed definition unavailable".into()))?,index)
            .ok_or_else(||crate::Unsupported("initial delayed parent unavailable".into()))??;
        let expected=parent.instrs.iter().filter(|instruction|matches!(instruction,Instr::Delayed{..})).count();
        let actual=game.delayed.iter().filter(|other|other.source==delayed.source
            && other.captured.context().ability.as_ref().is_some_and(|key|key.index==index)).count();
        if expected!=actual {return unsupported("incomplete initial delayed producer inventory");}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_capture_requires_every_read_binding_and_its_actual_kind() {
        let program=Arc::new(Program::with_initial(vec![Instr::Capture {operand:action::Operand::Reference(0),out:2}],
            vec![ActSpan{end:1,moves:false,keeps_x:false}],Vec::new(),vec![Some(Kind::Subjects),Some(Kind::Consent)]).unwrap());
        let ctx=Ctx::new(0,ObjId(0));
        assert_eq!(Captured::checked_sparse(ctx.clone(),program.clone(),vec![None,None]).unwrap_err().0,
            "sparse capture is missing a required historical binding");
        assert_eq!(Captured::checked_sparse(ctx.clone(),program.clone(),vec![Some(Value::Cards(Vec::new())),None]).unwrap_err().0,
            "sparse capture has the wrong retained value kind");
        let captured=Captured::checked_sparse(ctx,program,vec![Some(Value::Subjects(Vec::new())),None]).unwrap();
        assert!(captured.sparse_checked,"the unused Consent prefix does not become a fabricated choice");
    }
}
