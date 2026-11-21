//! Leveler cards (711). A level symbol is a keyword ability that stands for a static ability
//! (711.2): "{LEVEL N1-N2} [abilities] [P/T]" means "As long as this creature has at least N1
//! level counters on it, but no more than N2 level counters on it, it has base power and
//! toughness [P/T] and has [abilities]." A leveler's text is read here as the static abilities
//! its level symbols stand for, so the rest of the engine runs them as the statements they are
//! ([`crate::statement`]); no part of it knows of levels.

use crate::card::CardDef;
use ir::action::Granted;
use ir::effect::Modifier;
use ir::selector::Count;
use ir::statics::StaticAct;
use ir::{Ability, Action, Amount, Atom, Condition, Effect, Filter, Lead, Mode, Predicate, Pt, Sentence};
use std::sync::OnceLock;

/// The statement rule 711.2a is, read by the grammar once: one static ability that says both
/// what the creature's power and toughness are and what it has, so that both are one
/// continuous effect (613.6).
fn template() -> &'static Ability {
    static READ: OnceLock<Ability> = OnceLock::new();
    crate::token::init(&READ, || {
        CardDef::new("Level symbol rule", "", "Creature",
            "As long as this creature has a level counter on it, this creature has base power and toughness 1/1 and has flying.", Some((0, 0)))
            .unwrap_or_else(|e| panic!("the grammar reads rule 711.2a: {e}")).abilities[0].clone()
    })
}

/// One level symbol as the static ability it stands for (711.2a, 711.2b): while the level
/// counters on the creature are in the band, it has this base power and toughness and these
/// abilities.
fn band(from: u32, to: Option<u32>, pt: &(Pt, Pt), has: Vec<Granted>) -> Option<Ability> {
    let mut ability = template().clone();
    let Ability::Text(Effect(sentences)) = &mut ability else { return None };
    let [Sentence::Led { lead: Lead::Modifier(Modifier::AsLongAs(condition)), rest }] = sentences.as_mut_slice() else { return None };
    let Condition::Is { pred: Predicate::Has(Filter::Atom(Atom::Counters { count, .. })), .. } = condition else { return None };
    *count = match to {
        Some(to) => Count::Range { min: Box::new(Amount::Const(from)), max: Box::new(Amount::Const(to)) },
        None => Count::AtLeast(Amount::Const(from)),
    };
    let Sentence::Do(body) = rest.as_mut() else { return None };
    let clause = &mut body.first;
    let Action::SetStat { set, .. } = &mut clause.first.action else { return None };
    set.to = ir::action::SetTo::Profile(pt.0.clone(), pt.1.clone());
    if has.is_empty() {
        clause.rest.clear();
        return Some(ability);
    }
    let [(_, act)] = clause.rest.as_mut_slice() else { return None };
    let Action::Static(stated) = &mut act.action else { return None };
    let StaticAct::Has(what) = stated.as_mut() else { return None };
    *what = has;
    Some(ability)
}

/// A leveler card's abilities as the rules give them (711.2): each level symbol, with the
/// power and toughness and the abilities printed under it, is the one static ability it
/// stands for. What is printed before the first symbol is as it reads (711.4). A text with a
/// level symbol this cannot say is returned as it was, and is not run. `forms`: the Oracle
/// text's words for the objects each ability names ([`crate::CardDef::forms`]), kept for
/// what is printed before the first symbol; a band is worded by the rules, not printed.
pub(crate) fn read(abilities: Vec<Ability>, modes: Vec<Vec<Mode>>, forms: Vec<Vec<String>>) -> (Vec<Ability>, Vec<Vec<Mode>>, Vec<Vec<String>>) {
    if !abilities.iter().any(|a| matches!(a, Ability::LevelBand { .. })) {
        return (abilities, modes, forms);
    }
    let mut out = Vec::new();
    let mut out_forms = Vec::new();
    let mut open: Option<Band> = None;
    // the modal rows of what is printed before the first symbol; nothing after it has any
    let mut out_modes = Vec::new();
    let close = |open: &mut Option<Band>, out: &mut Vec<Ability>, out_forms: &mut Vec<Vec<String>>| {
        let Some(Band { from, to, pt, has, states }) = open.take() else { return Some(()) };
        let stated = band(from, to, &pt?, has)?;
        let Ability::Text(Effect(sentences)) = &stated else { return None };
        let [Sentence::Led { lead, .. }] = sentences.as_slice() else { return None };
        let said: Vec<_> = states.into_iter().map(|rest| Ability::Text(Effect(vec![Sentence::Led { lead: lead.clone(), rest: Box::new(rest) }]))).collect();
        out.push(stated);
        out.extend(said);
        out_forms.resize(out.len(), Vec::new());
        Some(())
    };
    for (i, (ability, rows)) in abilities.iter().zip(&modes).enumerate() {
        match (ability, &mut open) {
            (Ability::LevelBand { from, to }, _) => {
                if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
                open = Some(Band { from: *from, to: *to, pt: None, has: Vec::new(), states: Vec::new() });
            }
            // before the first level symbol: the level up ability and what else it always has
            (_, None) => {
                out.push(ability.clone());
                out_modes.push(rows.clone());
                out_forms.push(forms.get(i).cloned().unwrap_or_default());
            }
            (Ability::LevelPt(power @ Pt::Const(_), toughness @ Pt::Const(_)), Some(Band { pt: pt @ None, .. })) => *pt = Some((power.clone(), toughness.clone())),
            (Ability::Keywords(items), Some(band)) => band.has.extend(items.iter().map(|item| Granted::Keyword(Box::new(item.clone())))),
            // a modal ability is not one a quoted ability can be
            (Ability::Activated { .. } | Ability::Triggered { .. } | Ability::Text(_), Some(band)) if rows.is_empty() => match plain(ability) {
                Some(sentence) => band.states.push(sentence.clone()),
                None => band.has.push(Granted::Ability(Box::new(ability.clone()))),
            },
            _ => return (abilities, modes, forms),
        }
    }
    if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
    out_modes.resize(out.len(), Vec::new());
    (out, out_modes, out_forms)
}

/// A level symbol being read: its numbers, its power and toughness, the abilities it gives
/// the creature, and the statements that hold while the creature is in it.
struct Band {
    from: u32,
    to: Option<u32>,
    pt: Option<(Pt, Pt)>,
    has: Vec<Granted>,
    states: Vec<Sentence>,
}

/// The sentence of a static ability that states something outright ("Elves you control have
/// …", "Other creatures you control get +1/+1"). The creature having that ability while it is
/// in the band is the statement holding for as long, which is how it is said: a statement
/// the engine holds in force is not one it gives as a quoted ability.
pub(crate) fn plain(ability: &Ability) -> Option<&Sentence> {
    let Ability::Text(Effect(sentences)) = ability else { return None };
    let [sentence @ Sentence::Do(_)] = sentences.as_slice() else { return None };
    crate::statement::read(ability)?.acts.iter().all(|(_, when, _)| *when == crate::statement::When::Always).then_some(sentence)
}
