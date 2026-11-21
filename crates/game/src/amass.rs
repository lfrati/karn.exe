//! Amass (701.47): "To amass [subtype] N means 'If you don't control an Army creature, create a
//! 0/0 black [subtype] Army creature token. Choose an Army creature you control. Put N +1/+1
//! counters on that creature. If it isn't a [subtype], it becomes a [subtype] in addition to
//! its other types.'" (701.47a). The keyword action is those instructions, read by the grammar
//! and lowered as any other text is: the token, the choice, the counters (and what replaces
//! them) and the type change are the ordinary ones.
//!
//! Only a sentence that is the action alone ("Amass Orcs 1.", "Then amass Orcs 1.") is read so.
//! Anything that names the amassed Army afterwards (701.47c) or amasses inside a larger
//! sentence is left as it is, and refused where it is lowered.
use std::borrow::Cow;
use grammar::kernel::Outcome;
use ir::effect::{Act, Body, Clause, Effect, Lead, Sentence};
use ir::{Action, Amount};

/// The effect, each lone amass sentence in it said as what it means.
pub(crate) fn expand(effect: &Effect) -> Cow<'_, Effect> {
    if !effect.0.iter().any(|s| alone(s).is_some()) { return Cow::Borrowed(effect); }
    let mut said = Vec::new();
    for sentence in &effect.0 {
        let Some((then, subtype, amount)) = alone(sentence) else { said.push(sentence.clone()); continue };
        let Some(mut steps) = steps(subtype, amount) else { said.push(sentence.clone()); continue };
        // "Then amass Orcs 1.": the first of them is what follows "then"
        if then { steps[0] = Sentence::Led { lead: Lead::Then, rest: Box::new(steps[0].clone()) }; }
        said.extend(steps);
    }
    Cow::Owned(Effect(said))
}

/// "[Then] amass Orcs 1.", done by the ability's controller: the subtype and the number.
fn alone(sentence: &Sentence) -> Option<(bool, ir::Subtype, &Amount)> {
    let (then, body) = match sentence {
        Sentence::Do(body) => (false, body),
        Sentence::Led { lead: Lead::Then, rest } => match rest.as_ref() { Sentence::Do(body) => (true, body), _ => return None },
        _ => return None,
    };
    let Body { first: Clause { subject: None, may: false, also: false, first: Act { action: Action::Amass { subtype, amount }, mods, may: false }, rest }, rest: more } = body else { return None };
    (mods.is_empty() && rest.is_empty() && more.is_empty()).then_some((then, *subtype, amount))
}

/// What 701.47a says amassing is, with this subtype and number of counters.
fn steps(subtype: ir::Subtype, amount: &Amount) -> Option<Vec<Sentence>> {
    let name = subtype.name();
    let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) { "an" } else { "a" };
    let text = format!("If you don't control an Army creature, create a 0/0 black {name} Army creature token. \
        Choose an Army creature you control. Put X +1/+1 counters on it. \
        If it isn't {article} {name}, it becomes {article} {name} in addition to its other types.");
    let Outcome::Accepted { value: Effect(mut sentences), .. } = grammar::ability::effect().parse_all(&oracle::token::tokenize(&text)) else { return None };
    // the number is the action's own: "amass Orcs X, where X is …" keeps that X
    let counted = sentences.iter_mut().find_map(|s| match s {
        Sentence::Do(Body { first: Clause { first: Act { action: Action::PutCounters(placements), .. }, .. }, .. }) => placements.first_mut(),
        _ => None,
    })?;
    let [counters] = counted.counters.as_mut_slice() else { return None };
    counters.count = amount.clone();
    Some(sentences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r701_47a_amass_reads_as_its_four_instructions() {
        let effect = |text: &str| match grammar::ability::effect().parse_all(&oracle::token::tokenize(text)) {
            Outcome::Accepted { value, .. } => value,
            other => panic!("{text}: {other:?}"),
        };
        let amassed = effect("~ deals 1 damage to any target. Then amass Orcs 1.");
        let expanded = expand(&amassed);
        assert_eq!(expanded.0.len(), 5, "the damage, then the four instructions");
        assert!(matches!(&expanded.0[1], Sentence::Led { lead: Lead::Then, .. }));
        // any number, as the action said it
        let x = effect("Amass Zombies X.");
        let expanded = expand(&x);
        assert_eq!(expanded.0.len(), 4);
        let Sentence::Do(Body { first: Clause { first: Act { action: Action::PutCounters(p), .. }, .. }, .. }) = &expanded.0[2] else { panic!("{:?}", expanded.0[2]) };
        assert_eq!(p[0].counters[0].count, Amount::X);
        // inside a larger sentence: left as it is, to be refused where it is lowered
        let within = effect("You lose 1 life and amass Zombies 1.");
        assert!(matches!(expand(&within), Cow::Borrowed(_)));
    }
}
