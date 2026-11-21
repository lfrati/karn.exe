//! Station cards (721). A station symbol is a static ability (721.2): "{N+}[abilities]" means
//! "As long as this permanent has N or more charge counters on it, it has [abilities]."
//! (721.2a), and "{N+}[abilities][P/T]" adds "and is a creature with base power and toughness
//! [P/T] in addition to its other types" (721.2b). What is printed in a symbol's striation is
//! part of it: the rows under a symbol row, up to the next symbol, are. A station card's text
//! is read here as the static abilities its symbols stand for, as a leveler's is
//! ([`crate::level`]); no other part of the engine knows of stations.

use crate::card::CardDef;
use ir::action::Granted;
use ir::effect::Modifier;
use ir::selector::Count;
use ir::statics::StaticAct;
use ir::{Ability, Action, Amount, Atom, Condition, Effect, Filter, Lead, Mode, Predicate, Pt, Sentence};
use std::sync::OnceLock;

/// The statements rule 721.2a and 721.2b are, read by the grammar once.
fn template(creature: bool) -> &'static Ability {
    static HAS: OnceLock<Ability> = OnceLock::new();
    static CREATURE: OnceLock<Ability> = OnceLock::new();
    let (kept, text, rule) = if creature {
        (&CREATURE, "As long as this permanent has eight or more charge counters on it, it's a creature with base power and toughness 1/1 in addition to its other types and has flying.", "721.2b")
    } else {
        (&HAS, "As long as this permanent has eight or more charge counters on it, it has flying.", "721.2a")
    };
    crate::token::init(kept, || {
        CardDef::new("Station symbol rule", "", "Artifact", text, None)
            .unwrap_or_else(|e| panic!("the grammar reads rule {rule}: {e}")).abilities[0].clone()
    })
}

/// One station symbol as the static ability it stands for: while the permanent has `threshold`
/// or more charge counters, it has these abilities, and with `pt` is a creature with that base
/// power and toughness. None when it gives nothing to have.
fn symbol(threshold: u32, pt: Option<(i32, i32)>, has: Vec<Granted>) -> Option<Ability> {
    if pt.is_none() && has.is_empty() { return None; }
    let mut ability = template(pt.is_some()).clone();
    let Ability::Text(Effect(sentences)) = &mut ability else { return None };
    let [Sentence::Led { lead: Lead::Modifier(Modifier::AsLongAs(condition)), rest }] = sentences.as_mut_slice() else { return None };
    let Condition::Is { pred: Predicate::Has(Filter::Atom(Atom::Counters { count, .. })), .. } = condition else { return None };
    *count = Count::AtLeast(Amount::Const(threshold));
    let Sentence::Do(body) = rest.as_mut() else { return None };
    let clause = &mut body.first;
    let granted = if let Some((power, toughness)) = pt {
        let Action::Is(ir::action::Becoming::Is(chars)) = &mut clause.first.action else { return None };
        chars.base_pt.as_mut()?.to = ir::action::SetTo::Profile(Pt::Const(u32::try_from(power).ok()?), Pt::Const(u32::try_from(toughness).ok()?));
        if has.is_empty() {
            clause.rest.clear();
            return Some(ability);
        }
        let [(_, act)] = clause.rest.as_mut_slice() else { return None };
        &mut act.action
    } else {
        &mut clause.first.action
    };
    let Action::Static(stated) = granted else { return None };
    let StaticAct::Has(what) = stated.as_mut() else { return None };
    *what = has;
    Some(ability)
}

/// Whether this face is a station card whose printed power and toughness are its station
/// symbol's (721.2b): it has them only while that symbol applies, and in no other zone than
/// the battlefield (721.2c).
pub(crate) fn takes_pt(abilities: &[Ability]) -> bool {
    abilities.iter().any(|a| matches!(a, Ability::Station { .. }))
}

/// A station card's abilities as the rules give them (721.2): each station symbol, with the
/// abilities printed in its striation, is the one static ability it stands for; the highest
/// one also has the power and toughness box, `pt`, when one is printed (721.2b). What is
/// printed before the first symbol is as it reads (721.4). A text with a symbol this cannot
/// say is returned as it was, and is not run.
pub(crate) fn read(abilities: Vec<Ability>, modes: Vec<Vec<Mode>>, forms: Vec<Vec<String>>, pt: Option<(i32, i32)>) -> (Vec<Ability>, Vec<Vec<Mode>>, Vec<Vec<String>>) {
    if !takes_pt(&abilities) { return (abilities, modes, forms); }
    let highest = abilities.iter().filter_map(|a| match a { Ability::Station { threshold, .. } => Some(*threshold), _ => None }).max();
    let mut out = Vec::new();
    let mut out_forms = Vec::new();
    let mut out_modes = Vec::new();
    let mut open: Option<Striation> = None;
    let close = |open: &mut Option<Striation>, out: &mut Vec<Ability>, out_forms: &mut Vec<Vec<String>>| -> Option<()> {
        let Some(Striation { threshold, has, states }) = open.take() else { return Some(()) };
        let box_pt = pt.filter(|_| Some(threshold) == highest);
        let stated = symbol(threshold, box_pt, has);
        // the condition the symbol's statements hold under
        let lead = Lead::Modifier(Modifier::AsLongAs(match template(false) {
            Ability::Text(Effect(sentences)) => match sentences.as_slice() {
                [Sentence::Led { lead: Lead::Modifier(Modifier::AsLongAs(c)), .. }] => {
                    let mut c = c.clone();
                    let Condition::Is { pred: Predicate::Has(Filter::Atom(Atom::Counters { count, .. })), .. } = &mut c else { return None };
                    *count = Count::AtLeast(Amount::Const(threshold));
                    c
                }
                _ => return None,
            },
            _ => return None,
        }));
        if stated.is_none() && states.is_empty() { return None; }
        out.extend(stated);
        out.extend(states.into_iter().map(|rest| Ability::Text(Effect(vec![Sentence::Led { lead: lead.clone(), rest: Box::new(rest) }]))));
        out_forms.resize(out.len(), Vec::new());
        Some(())
    };
    for (i, (ability, rows)) in abilities.iter().zip(&modes).enumerate() {
        let ability = match ability {
            Ability::Station { threshold, ability } => {
                if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
                open = Some(Striation { threshold: *threshold, has: Vec::new(), states: Vec::new() });
                ability.as_ref()
            }
            other => other,
        };
        match &mut open {
            // before the first symbol: the station ability and what else it always has
            None => {
                out.push(ability.clone());
                out_modes.push(rows.clone());
                out_forms.push(forms.get(i).cloned().unwrap_or_default());
            }
            Some(striation) if rows.is_empty() => match ability {
                Ability::Keywords(items) => striation.has.extend(items.iter().map(|item| Granted::Keyword(Box::new(item.clone())))),
                Ability::Activated { .. } | Ability::Triggered { .. } | Ability::Text(_) => match crate::level::plain(ability) {
                    Some(sentence) => striation.states.push(sentence.clone()),
                    None => striation.has.push(Granted::Ability(Box::new(ability.clone()))),
                },
                _ => return (abilities, modes, forms),
            },
            // a modal ability is not one a quoted ability can be
            Some(_) => return (abilities, modes, forms),
        }
    }
    if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
    out_modes.resize(out.len(), Vec::new());
    (out, out_modes, out_forms)
}

/// A station symbol being read: its number, the abilities it gives the permanent, and the
/// statements that hold while it applies.
struct Striation {
    threshold: u32,
    has: Vec<Granted>,
    states: Vec<Sentence>,
}
