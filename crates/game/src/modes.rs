//! Modal spells and abilities (CR 700.2): "Choose one —" and the modes printed under it.
//!
//! The modes are chosen as the spell is cast or the ability activated (601.2b, 700.2a), or as a
//! triggered ability is put on the stack (603.3c, 700.2b), and the spell or ability then has the
//! instructions of the modes chosen and only those: it targets only what they target (700.2c),
//! and they are followed in the order they are printed (608.2c), a mode chosen once (700.2d)
//! unless the text says "You may choose the same mode more than once": then a mode chosen n
//! times is as if it appeared n times in sequence, each with its own targets (700.2d).
//! Once its modes are chosen a modal ability is one text like any other, which is all the rest
//! of the engine sees.

use crate::{Res, unsupported};
use ir::effect::Modes;
use ir::{Effect, Mode, Sentence};

/// The "Choose …" an ability's text is, when it is modal: how many of its modes are chosen.
fn header(effect: &Effect) -> Option<&Modes> {
    effect.0.iter().find_map(|s| match s {
        Sentence::ChooseModes { modes, .. } => Some(modes),
        _ => None,
    })
}

/// "You may choose the same mode more than once." (700.2d)
pub fn repeats(effect: &Effect) -> bool {
    effect.0.iter().any(|s| matches!(s, Sentence::Rider(ir::effect::Rider::SameModeMoreThanOnce)))
}

/// What a mode the engine can choose does: a bullet's text, or the text after a mode's own
/// cost (700.2h). None: a row chosen some other way (pawprints, anchor words, die results).
pub fn effect_of(row: &Mode) -> Option<&Effect> {
    match row {
        Mode::Bullet(effect) | Mode::Costed { effect, .. } | Mode::Pawprint { effect, .. } => Some(effect),
        _ => None,
    }
}

/// The additional costs of the modes chosen, one for each time a mode is chosen (700.2h):
/// "+ {1}{U} — Counter target spell." chosen adds {1}{U} to the spell's total cost (601.2f).
pub fn costs<'a>(rows: &'a [Mode], chosen: &[usize]) -> impl Iterator<Item = &'a ir::mana::ManaCost> {
    chosen.iter().filter_map(|m| match rows.get(*m) {
        Some(Mode::Costed { cost, .. }) => Some(cost),
        _ => None,
    })
}

/// How many {P} the modes chosen are worth together, each as many times as it is chosen
/// (700.2i, 700.2d).
pub fn pawprints(rows: &[Mode], chosen: &[usize]) -> u32 {
    chosen.iter().map(|m| match rows.get(*m) { Some(Mode::Pawprint { paws, .. }) => *paws, _ => 1 }).sum()
}

/// Is this ability modal (700.2)?
pub fn modal(effect: &Effect, rows: &[Mode]) -> bool {
    header(effect).is_some() || !rows.is_empty()
}

/// How few and how many of its modes are chosen (700.2a), and, of "Choose one. If this spell
/// was kicked, choose both instead.", how many when what it asks of the spell's announcement
/// holds (601.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounds<'a> { pub min: usize, pub max: usize, pub instead: Option<(&'a ir::Condition, usize, usize)>, pub paws: Option<u32> }

/// "If this spell was kicked, choose both instead.", "If this spell was cast using teamwork,
/// choose both instead.": what it asks, and how many modes then.
fn instead(sentence: &Sentence) -> Option<(&ir::Condition, &Modes)> {
    use ir::effect::{Body, Clause, Lead, Modifier};
    let Sentence::Led { lead: Lead::Modifier(Modifier::If(condition)), rest } = sentence else { return None };
    let Sentence::Do(Body { first: Clause { subject: None, may: false, also: false, first, rest: then }, rest: more }) = rest.as_ref() else { return None };
    if !then.is_empty() || !more.is_empty() || first.may || first.mods != [Modifier::Instead] { return None; }
    let ir::Action::Choose(ir::action::Choice::Modes(modes)) = &first.action else { return None };
    matches!(condition, ir::Condition::CastWas { negated: false, .. }).then_some((condition, modes))
}

/// How few and how many of its modes are chosen, for a modal ability the engine can run
/// (700.2a): "Choose one —" one, "Choose one or both —" one or two, "Choose one or more —" any
/// but none, "Choose any number —" any. None: the ability is not modal.
pub fn bounds<'a>(effect: &'a Effect, rows: &[Mode]) -> Res<Option<Bounds<'a>>> {
    if !modal(effect, rows) {
        return Ok(None);
    }
    let [Sentence::ChooseModes { modes, unchosen, random, chooser }, rest @ ..] = effect.0.as_slice() else {
        return unsupported("a modal ability with more than its choice of modes");
    };
    let (instead, rest) = match rest {
        [first, rest @ ..] if instead(first).is_some() => (instead(first), rest),
        _ => (None, rest),
    };
    if !matches!(rest, [] | [Sentence::Rider(ir::effect::Rider::SameModeMoreThanOnce)]) {
        return unsupported("a modal ability with more than its choice of modes");
    }
    if chooser.is_some() {
        return unsupported("modes: chosen by a player other than the controller (700.2e)");
    }
    if *random {
        return unsupported("modes: chosen at random");
    }
    if unchosen.is_some() {
        return unsupported("modes: one that hasn't been chosen");
    }
    // pawprint rows under "up to N {P} worth of modes", and only there (700.2i)
    if matches!(modes, Modes::Pawprints(_)) != rows.iter().all(|m| matches!(m, Mode::Pawprint { .. })) {
        return unsupported("modes: pawprints (700.2i) with another header or rows");
    }
    if let Some(row) = rows.iter().find(|m| effect_of(m).is_none()) {
        return unsupported(format!(
            "modes: {}",
            match row {
                Mode::Anchor { .. } => "anchor words (614.12c)",
                _ => "die-roll results (706.3)",
            }
        ));
    }
    let repeated = repeats(effect);
    let (min, max) = count(modes, rows.len(), repeated)?;
    let instead = match instead {
        Some((condition, modes)) => { let (a, b) = count(modes, rows.len(), repeated)?; Some((condition, a, b)) }
        None => None,
    };
    let paws = match modes { Modes::Pawprints(k) => Some(*k), _ => None };
    Ok(Some(Bounds { min, max, instead, paws }))
}

/// How few and how many of `n` modes this many is (700.2a).
fn count(modes: &Modes, n: usize, repeats: bool) -> Res<(usize, usize)> {
    if repeats && matches!(modes, Modes::OneOrMore | Modes::AnyNumber) {
        // repeated, any number of them has no most (700.2d)
        return unsupported("modes: any number of them, the same more than once");
    }
    let (min, max) = match modes {
        Modes::Exactly(k) => (*k as usize, *k as usize),
        Modes::OneOrBoth => (1, 2),
        Modes::OneOrMore => (1, n),
        Modes::Both => (2, 2),
        Modes::AnyNumber => (0, n),
        Modes::X => return unsupported("modes: X of them"),
        Modes::UpTo(k) => (0, *k as usize),
        // each mode is at least one {P}: at most that many modes (700.2i)
        Modes::Pawprints(k) => (0, if repeats { *k as usize } else { (*k as usize).min(n) }),
    };
    if (max > n && !repeats) || n < 2 || min > max {
        return unsupported(format!("modes: {min} to {max} of {n}"));
    }
    Ok((min, max))
}

/// Is this a choice of modes the player may make (700.2a, 700.2d)? As many as the text says,
/// each a mode the ability has, none of them twice. Of an ability that is not modal, no modes.
/// As announced with none of the choices "choose both instead" asks of (601.2b).
pub fn legal(effect: &Effect, rows: &[Mode], chosen: &[usize]) -> Result<(), String> {
    match bounds(effect, rows).map_err(|u| u.0)? {
        None if chosen.is_empty() => Ok(()),
        None => Err("that is not modal: no mode is chosen (700.2)".into()),
        Some(Bounds { min, max, paws, .. }) => {
            if let Some(most) = paws.filter(|most| pawprints(rows, chosen) > *most) {
                return Err(format!("choose up to {most} {{P}} worth of modes (700.2i)"));
            }
            if chosen.len() < min || chosen.len() > max {
                return Err(match (min, max) {
                    (a, b) if a == b => format!("choose {a} of its modes (700.2a)"),
                    (a, b) => format!("choose {a} to {b} of its modes (700.2a)"),
                });
            }
            if chosen.iter().any(|m| *m >= rows.len()) {
                return Err(format!("it has {} modes (700.2)", rows.len()));
            }
            if !repeats(effect) && chosen.iter().enumerate().any(|(i, m)| chosen[..i].contains(m)) {
                return Err("a mode is chosen once (700.2d)".into());
            }
            Ok(())
        }
    }
}

/// Every choice of modes the player may make, each in the order the modes are printed; of
/// an ability that is not modal, the one choice of none. With "choose both instead", those
/// of either announcement (601.2b).
pub fn choices(effect: &Effect, rows: &[Mode]) -> Res<Vec<Vec<usize>>> {
    Ok(match bounds(effect, rows)? {
        None => vec![Vec::new()],
        Some(Bounds { min, max, instead, paws }) => {
            let mut all: Vec<_> = (min..=max).flat_map(|k| combinations(rows.len(), k, repeats(effect)))
                .filter(|choice| paws.is_none_or(|most| pawprints(rows, choice) <= most)).collect();
            for k in instead.map_or(1..=0, |(_, a, b)| a..=b) {
                for choice in combinations(rows.len(), k, repeats(effect)) { if !all.contains(&choice) { all.push(choice); } }
            }
            all
        }
    })
}

/// Each choice of `k` of `n` modes in printed order: distinct ones, or with `repeats` a mode
/// as many times as it is chosen (700.2d).
pub(crate) fn combinations(n: usize, k: usize, repeats: bool) -> Vec<Vec<usize>> {
    fn from(n: usize, start: usize, k: usize, repeats: bool) -> Vec<Vec<usize>> {
        if k == 0 {
            return vec![Vec::new()];
        }
        (start..n).flat_map(|first| from(n, if repeats { first } else { first + 1 }, k - 1, repeats).into_iter()
            .map(move |rest| std::iter::once(first).chain(rest).collect())).collect()
    }
    from(n, 0, k, repeats)
}

/// What a check of whether the engine can run the ability walks: each of its modes alone,
/// any of which may be chosen; of an ability that is not modal, its text.
pub fn each(effect: &Effect, rows: &[Mode]) -> Res<Vec<Vec<usize>>> {
    Ok(match bounds(effect, rows)? {
        None => vec![Vec::new()],
        Some(_) => (0..rows.len()).map(|m| vec![m]).collect(),
    })
}

/// The instructions the ability has with these modes chosen: theirs, in the order they are
/// printed (608.2c); of an ability that is not modal, its own text. The choice is one
/// [`legal`] allows.
pub fn instructions(effect: &Effect, rows: &[Mode], chosen: &[usize]) -> Res<Effect> {
    if bounds(effect, rows)?.is_none() {
        return Ok(effect.clone());
    }
    let mut order = chosen.to_vec();
    order.sort_unstable();
    let mut sentences = Vec::new();
    for m in order {
        match rows.get(m).and_then(effect_of) {
            Some(e) => sentences.extend(e.0.iter().cloned()),
            None => return unsupported("a mode the ability does not have"),
        }
    }
    Ok(Effect(sentences))
}
