//! Finite class iteration over a retained card group. Choices precede one move (608.2c).
use super::*;

/// "For each card type, you may put a card of that type … into your hand."
/// Each offered selection fixes one type and excludes earlier picks. Their shared
/// movement happens only after every choice, preserving simultaneous arrival.
pub(super) fn hand_selection(sentence: &Sentence, support: &Support<'_>, emit: &mut Emit, binder: &mut Binder) -> Res<()> {
    let Sentence::Do(body) = sentence else { return unsupported("card-type iteration: an ordinary selection clause required"); };
    let clause = &body.first;
    let act = &clause.first;
    if !body.rest.is_empty() || !clause.rest.is_empty() || clause.also || !(clause.may || act.may)
        || !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) {
        return unsupported("card-type iteration: one controller's optional selection required");
    }
    let Action::Move { moves, from: None, .. } = &act.action else {
        return unsupported("card-type iteration: only putting retained cards into hand is implemented");
    };
    let [(objects, Destination::Hand(Whose::You))] = moves.as_slice() else {
        return unsupported("card-type iteration: one hand destination required");
    };
    let [selector] = objects.items.as_slice() else { return unsupported("card-type iteration: one described card required"); };
    let mention = mention(selector).ok_or_else(|| crate::Unsupported("card-type iteration: no retained card producer".into()))?;
    if mention.taking != Taking::Exactly(1) || mention.set.other {
        return unsupported("card-type iteration: exactly one optional member per type required");
    }
    let ir::Filter::All(filters) = &mention.set.filter else { return unsupported("card-type iteration: no bound type reference"); };
    if filters.iter().filter(|f| **f == ir::Filter::Atom(Atom::ThatType)).count() != 1 {
        return unsupported("card-type iteration: one that-type reference required");
    }
    emit.unite(&mention.reference, binder);
    let group = binder.resolve(&mention.reference)?;
    let (from, zone, mut taken) = (group.slot, group.zone, group.taken.clone());
    if zone != Zone::Library { return unsupported("card-type iteration: a retained library group required"); }
    let mut movements = Vec::new();
    let mut selected = Vec::new();
    // Deferred card types cannot occur on an admitted definition. Include every
    // represented type, especially kindred, independently of the card's reminder text.
    for kind in ir::CardType::ALL {
        let mut set = mention.set.clone();
        set.filter = ir::Filter::All(filters.iter().map(|f| if *f == ir::Filter::Atom(Atom::ThatType) {
            ir::Filter::Atom(Atom::Type(*kind))
        } else { f.clone() }).collect());
        let set = binder.selection(&set, support)?;
        let (remaining, chosen) = (emit.slot(), emit.slot());
        emit.instrs.push(Instr::Remainder { of: from, minus: taken.clone(), still_in: zone, out: remaining });
        emit.instrs.push(Instr::Select { from: remaining, set: set.into(), pick: Pick::UpTo(1), chooser: Who::Controller,
            gate: None, still_in: zone, out: chosen });
        movements.push((chosen, Dest::Hand));
        selected.push(chosen);
        taken.push(chosen);
    }
    binder.resolve(&mention.reference)?.taken.extend(selected);
    emit.instrs.push(Instr::MoveTogether { parts: movements, entered: Vec::new() });
    Ok(())
}

/// One card's mandatory alternatives: pay life, or put that card on top of your library.
/// The payment owner enforces affordability; this is not an unconditional life loss.
pub(super) fn life_or_top(sentence: &Sentence) -> Option<&Amount> {
    let Sentence::Do(body) = sentence else { return None };
    let clause = &body.first;
    if !body.rest.is_empty() || clause.may || clause.also || !controller_only(clause.subject.as_ref())
        || clause.first.may || !clause.first.mods.is_empty() { return None; }
    let Action::PayLife(amount) = &clause.first.action else { return None };
    let [(Join::Or, alternative)] = clause.rest.as_slice() else { return None };
    if alternative.may || !alternative.mods.is_empty() { return None; }
    let Action::Move { moves, from: None, .. } = &alternative.action else { return None };
    let [(objects, Destination::Library(place))] = moves.as_slice() else { return None };
    let [card] = objects.items.as_slice() else { return None };
    if card.quant != Quant::Definite(Ref::Anaphor(ir::Anaphor::The { plural: false }))
        || card.set != ObjectSet::of(Domain::Card) || card.chooser.is_some() || card.random.is_some()
        || place.library != Some(Whose::You) || place.back || place.order.is_some()
        || place.position != ir::Position::One(LibraryPosition::Top) { return None; }
    Some(amount)
}

/// Finite per-card choices compile to ordinary selections, payments and placements.
/// The whole private group is chosen first. Each next member is chosen before its
/// payment, so putting two back also retains the player's chosen library order.
pub(super) fn card_payments(set: &ObjectSet, sentence: &Sentence, support: &Support<'_>, emit: &mut Emit, binder: &mut Binder) -> Res<()> {
    let Some(amount) = life_or_top(sentence) else { return unsupported("per-card payment: life or library top required"); };
    let ir::Filter::All(filters) = &set.filter else { return unsupported("per-card payment: one retained group required"); };
    let [ir::Filter::Atom(Atom::Among(group))] = filters.as_slice() else { return unsupported("per-card payment: one retained group required"); };
    if set.domain != Domain::Object || *set != (ObjectSet { filter: set.filter.clone(), ..ObjectSet::of(Domain::Object) }) {
        return unsupported("per-card payment: an unqualified retained group required");
    }
    let action::Operand::Reference(source) = emit.operand(group, support, binder)? else {
        return unsupported("per-card payment: an explicit bounded hand choice required");
    };
    let named = binder.subjects.iter().find(|n| n.slot == source && n.explicit_choice)
        .ok_or_else(|| crate::Unsupported("per-card payment: an explicit bounded hand choice required".into()))?;
    if named.set.domain != Domain::Card || named.bounds.1 > 2 || named.set.zone.as_ref().is_none_or(|zone|
        zone.zones.items != [ir::Zone::Hand] || zone.owner.as_deref().is_none_or(|owner| !crate::select::is_you(owner))) {
        return unsupported("per-card payment: a hand choice of at most two cards required");
    }
    let max = named.bounds.1;
    let from = emit.slot();
    emit.instrs.push(Instr::Gather { objects: action::Operand::Reference(source), out: from });
    let cost = crate::cost::Plan::compile(&ir::Cost(vec![ir::CostPart::Action(Action::PayLife(amount.clone()))]), support)?;
    let any = expr::Cards::compile(&ObjectSet::of(Domain::Card), support, &mut binder.scope(support))?;
    let mut taken = Vec::new();
    for _ in 0..max {
        let (remaining, chosen, member, paid) = (emit.slot(), emit.slot(), emit.slot(), emit.slot());
        emit.instrs.push(Instr::Remainder { of: from, minus: taken.clone(), still_in: Zone::Hand, out: remaining });
        emit.instrs.push(Instr::Select { from: remaining, set: any.clone().into(), pick: Pick::Exactly(1), chooser: Who::Controller, gate: None, still_in: Zone::Hand, out: chosen });
        emit.instrs.push(Instr::Subjects { cards: chosen, out: member, still_in: Some(Zone::Hand) });
        let present = emit.instrs.len();
        emit.instrs.push(Instr::Branch { condition: condition::Condition::present(member), end: 0, remember: None, skipped_values: Vec::new() });
        emit.instrs.push(Instr::Pay { payer: action::Operand::Controller, cost: cost.clone(), each: None, paid });
        let payment = emit.instrs.len();
        emit.instrs.push(Instr::Branch { condition: condition::Condition::stored(paid, false), end: 0, remember: None, skipped_values: Vec::new() });
        emit.instrs.push(Instr::Place { objects: vec![action::Operand::Reference(member)], position: ir::Position::One(LibraryPosition::Top), chooser: None, order: None, attempt: None, by: None, before_shuffle: false });
        let end = emit.instrs.len();
        for branch in [present, payment] {
            let Instr::Branch { end: target, .. } = &mut emit.instrs[branch] else { unreachable!() };
            *target = end;
        }
        emit.end_act(true, None);
        taken.push(chosen);
    }
    Ok(())
}
