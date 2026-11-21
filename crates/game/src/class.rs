//! Class cards (716). A class level bar is a keyword ability that stands for an activated
//! ability and a static ability (716.2): "[Cost]: Level N — [Abilities]" means "[Cost]: This
//! Class's level becomes N. Activate only if this Class is level N-1 and only as a sorcery"
//! and "As long as this Class is level N or greater, it has [abilities]" (716.2a). A Class's
//! text is read here as those abilities, so the rest of the engine runs them as what they
//! are; only a permanent's level and the event of its becoming one are of levels (716.2b).

use crate::card::CardDef;
use ir::action::{Becoming, Granted};
use ir::condition::{Predicate, Restriction};
use ir::effect::Modifier;
use ir::selector::Count;
use ir::statics::StaticAct;
use ir::cost::{Cost, CostPart};
use ir::{Ability, Action, Amount, Atom, Condition, Effect, Filter, Lead, Mode, Sentence};
use std::sync::OnceLock;

/// The two abilities rule 716.2a says a level bar is, read by the grammar once in its words;
/// [`at_level`] and [`bar`] put each bar's numbers in place.
fn templates() -> &'static (Ability, Ability) {
    static READ: OnceLock<(Ability, Ability)> = OnceLock::new();
    crate::token::init(&READ, || {
        let read = |text: &str| CardDef::new("Class level rule", "", "Enchantment — Class", text, None)
            .unwrap_or_else(|e| panic!("the grammar reads rule 716.2a: {e}")).abilities()[0].clone();
        (read("{1}: This permanent becomes level 2. Activate only if this permanent is level 1 and only as a sorcery."),
         read("As long as this permanent is level 2 or greater, this permanent has flying."))
    })
}

/// "this Class is level N" or "level N or greater", with this N.
fn at_level(condition: &mut Condition, level: u32) -> Option<()> {
    let Condition::Is { pred: Predicate::Adjective(Filter::Atom(Atom::Level(Count::AtLeast(n) | Count::Precisely(n)))), .. } = condition else { return None };
    *n = Amount::Const(level);
    Some(())
}

/// "[Cost]: This Class's level becomes N. Activate only if this Class is level N-1 and only
/// as a sorcery."
fn bar(cost: &ir::mana::ManaCost, level: u32) -> Option<Ability> {
    let mut ability = templates().0.clone();
    let Ability::Activated { cost: paid, effect: Effect(sentences) } = &mut ability else { return None };
    *paid = Cost(vec![CostPart::Mana(cost.clone())]);
    let [Sentence::Do(body), Sentence::Restrictions(restrictions)] = sentences.as_mut_slice() else { return None };
    body.first.first.action = Action::Becomes(Becoming::Level(level));
    let [Restriction::If(condition), Restriction::Sorcery] = restrictions.as_mut_slice() else { return None };
    at_level(condition, level.checked_sub(1)?)?;
    Some(ability)
}

/// The level a level bar's ability makes its Class: N of "This Class's level becomes N" (716.2a).
pub(crate) fn bar_level(ability: &Ability) -> Option<u32> {
    let Ability::Activated { effect: Effect(sentences), .. } = ability else { return None };
    let [Sentence::Do(body), Sentence::Restrictions(_)] = sentences.as_slice() else { return None };
    match body.first.first.action { Action::Becomes(Becoming::Level(level)) => Some(level), _ => None }
}

/// "As long as this Class is level N or greater, it has [abilities]": the statement for what
/// it gives, and each statement said outright, which holds for as long (as for levelers,
/// [`crate::level::plain`]). Each comes with the Oracle text's words for what it names: the
/// Class for the condition, then those of what it gives or states.
fn section(level: u32, has: Vec<Granted>, has_forms: Vec<String>, states: Vec<(Sentence, Vec<String>)>, out: &mut Vec<(Ability, Vec<String>)>) -> Option<()> {
    let class = || "this Class".to_string();
    let mut stated = templates().1.clone();
    let Ability::Text(Effect(sentences)) = &mut stated else { return None };
    let [Sentence::Led { lead: Lead::Modifier(Modifier::AsLongAs(condition)), rest }] = sentences.as_mut_slice() else { return None };
    at_level(condition, level)?;
    let lead = Lead::Modifier(Modifier::AsLongAs(condition.clone()));
    if !has.is_empty() {
        let Sentence::Do(body) = rest.as_mut() else { return None };
        let Action::Static(what) = &mut body.first.first.action else { return None };
        let StaticAct::Has(granted) = what.as_mut() else { return None };
        *granted = has;
        out.push((stated, [class(), class()].into_iter().chain(has_forms).collect()));
    }
    out.extend(states.into_iter().map(|(rest, forms)| (Ability::Text(Effect(vec![Sentence::Led { lead: lead.clone(), rest: Box::new(rest) }])),
        std::iter::once(class()).chain(forms).collect())));
    Some(())
}

/// A Class card's abilities as the rules give them (716.2): each level bar is the activated
/// ability and the static ability it stands for, with what is printed in its section. What
/// is printed before the first bar is as it reads (716.3). A text with a section this cannot
/// say is returned as it was, and is not run. `forms` are kept for what is printed before the
/// first bar, as for levelers ([`crate::level::read`]).
pub(crate) fn read(abilities: Vec<Ability>, modes: Vec<Vec<Mode>>, forms: Vec<Vec<String>>) -> (Vec<Ability>, Vec<Vec<Mode>>, Vec<Vec<String>>) {
    if !abilities.iter().any(|a| matches!(a, Ability::ClassLevel { .. })) {
        return (abilities, modes, forms);
    }
    let mut out = Vec::new();
    let mut out_forms = Vec::new();
    let mut out_modes = Vec::new();
    // the section being read: its level, what it gives (with the words of what that names)
    // and what it states
    type Open = Option<(u32, Vec<Granted>, Vec<String>, Vec<(Sentence, Vec<String>)>)>;
    let mut open: Open = None;
    let close = |open: &mut Open, out: &mut Vec<Ability>, out_forms: &mut Vec<Vec<String>>| match open.take() {
        Some((level, has, has_forms, states)) => {
            let mut said = Vec::new();
            section(level, has, has_forms, states, &mut said)?;
            for (ability, forms) in said {
                out.push(ability);
                out_forms.push(forms);
            }
            Some(())
        }
        None => Some(()),
    };
    for (i, (ability, rows)) in abilities.iter().zip(&modes).enumerate() {
        match (ability, &mut open) {
            (Ability::ClassLevel { cost, level }, _) => {
                let Some(bar) = bar(cost, *level) else { return (abilities, modes, forms) };
                if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
                out.push(bar);
                out_forms.push(Vec::new());
                open = Some((*level, Vec::new(), Vec::new(), Vec::new()));
            }
            (_, None) => {
                out.push(ability.clone());
                out_modes.push(rows.clone());
                out_forms.push(forms.get(i).cloned().unwrap_or_default());
                continue;
            }
            (Ability::Keywords(items), Some((_, has, _, _))) => has.extend(items.iter().map(|item| Granted::Keyword(Box::new(item.clone())))),
            // a modal ability is not one a quoted ability can be
            (Ability::Activated { .. } | Ability::Triggered { .. } | Ability::Text(_), Some((_, has, has_forms, states))) if rows.is_empty() => {
                let said = forms.get(i).cloned().unwrap_or_default();
                match crate::level::plain(ability) {
                    Some(sentence) => states.push((sentence.clone(), said)),
                    None => {
                        has.push(Granted::Ability(Box::new(ability.clone())));
                        has_forms.extend(said);
                    }
                }
            }
            _ => return (abilities, modes, forms),
        }
        out_modes.resize(out.len(), Vec::new());
        out_forms.resize(out.len(), Vec::new());
    }
    if close(&mut open, &mut out, &mut out_forms).is_none() { return (abilities, modes, forms); }
    out_modes.resize(out.len(), Vec::new());
    out_forms.resize(out.len(), Vec::new());
    (out, out_modes, out_forms)
}
