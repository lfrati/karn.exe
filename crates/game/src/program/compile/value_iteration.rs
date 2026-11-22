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
