//! Numbered keyword actions that are other instructions by their rules, read as those
//! instructions, as [`crate::amass`] reads amass:
//! - "Bolster N" means "Choose a creature you control with the least toughness or tied for
//!   least toughness among creatures you control. Put N +1/+1 counters on that creature."
//!   (701.39a)
//! - "Support N" on a permanent means "Put a +1/+1 counter on each of up to N other target
//!   creatures."; on an instant or sorcery spell, "… up to N target creatures." (701.41a)
//! - To "blight N" means to put N -1/-1 counters on a creature you control (701.68a).
//! - "Monstrosity N" means "If this permanent isn't monstrous, put N +1/+1 counters on it and
//!   it becomes monstrous." (701.37a) Monstrosity X, whose X other abilities may read
//!   (701.37c), is not.
//! - "Earthbend N" means "Target land you control becomes a 0/0 land creature with haste in
//!   addition to its other types. Put N +1/+1 counters on it. When that land dies or is put
//!   into exile, return it to the battlefield tapped under your control." (701.66a)
//!
//! Only a sentence that is the action alone, by the ability's controller, with a fixed N, is
//! read so ("Bolster 2.", "Then support 2."); anything else is left as it is, and refused
//! where it is lowered.
use std::borrow::Cow;
use grammar::kernel::Outcome;
use ir::effect::{Act, Body, Clause, Effect, Lead, Sentence};
use ir::{Action, Amount, NumberedAction};

/// The effect, each lone such sentence in it said as what it means. `spell`: the effect is
/// an instant's or sorcery's (701.41a).
pub(crate) fn expand(effect: &Effect, spell: bool) -> Cow<'_, Effect> {
    if !effect.0.iter().any(|s| alone(s).is_some_and(|(_, action, n)| steps(action, n, spell).is_some())) { return Cow::Borrowed(effect); }
    let mut said = Vec::new();
    for sentence in &effect.0 {
        let Some((then, action, n)) = alone(sentence) else { said.push(sentence.clone()); continue };
        let Some(mut steps) = steps(action, n, spell) else { said.push(sentence.clone()); continue };
        if then { steps[0] = Sentence::Led { lead: Lead::Then, rest: Box::new(steps[0].clone()) }; }
        said.extend(steps);
    }
    Cow::Owned(Effect(said))
}

/// "[Then] bolster 2.", done by the ability's controller: the action and its N.
fn alone(sentence: &Sentence) -> Option<(bool, NumberedAction, u32)> {
    let (then, body) = match sentence {
        Sentence::Do(body) => (false, body),
        Sentence::Led { lead: Lead::Then, rest } => match rest.as_ref() { Sentence::Do(body) => (true, body), _ => return None },
        _ => return None,
    };
    let Body { first: Clause { subject: None, may: false, also: false, first: Act { action: Action::Numbered(action, Amount::Const(n)), mods, may: false }, rest }, rest: more } = body else { return None };
    (mods.is_empty() && rest.is_empty() && more.is_empty()).then_some((then, *action, *n))
}

/// What the rule says the action is, with this N; none for an action read otherwise, or an
/// N with nothing to do (a count of none).
fn steps(action: NumberedAction, n: u32, spell: bool) -> Option<Vec<Sentence>> {
    let words = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    let count = *words.get(n as usize)?;
    let counters = if n == 1 { "a" } else { count };
    let text = match action {
        // "or tied for least toughness": a creature with the least toughness is one of those
        NumberedAction::Bolster => format!("Choose a creature you control with the least toughness among creatures you control. \
            Put {counters} +1/+1 counter{} on it.", if n == 1 { "" } else { "s" }),
        NumberedAction::Support => {
            let other = if spell { "" } else { "other " };
            if n == 1 { format!("Put a +1/+1 counter on up to one {other}target creature.") }
            else { format!("Put a +1/+1 counter on each of up to {count} {other}target creatures.") }
        }
        NumberedAction::Monstrosity => format!("If ~ isn't monstrous, put {counters} +1/+1 counter{} on ~ and it becomes monstrous.",
            if n == 1 { "" } else { "s" }),
        NumberedAction::Earthbend => format!("Target land you control becomes a 0/0 land creature with haste in addition to its other types. \
            Put {counters} +1/+1 counter{} on it. When that land dies or is put into exile, return it to the battlefield tapped under your control.",
            if n == 1 { "" } else { "s" }),
        NumberedAction::Blight => format!("Choose a creature you control. Put {counters} -1/-1 counter{} on it.", if n == 1 { "" } else { "s" }),
        _ => return None,
    };
    let Outcome::Accepted { value: Effect(sentences), .. } = grammar::ability::effect().parse_all(&oracle::token::tokenize(&text)) else { return None };
    Some(sentences)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(text: &str) -> Effect {
        match grammar::ability::effect().parse_all(&oracle::token::tokenize(text)) {
            Outcome::Accepted { value, .. } => value,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn r701_39a_41a_68a_numbered_actions_read_as_their_instructions() {
        for (text, n) in [("Bolster 2.", 2), ("Support 2.", 1), ("Then support 1.", 1), ("Blight 1.", 2), ("Bolster 1.", 2), ("Monstrosity 3.", 1), ("Earthbend 2.", 3)] {
            let said = effect(text);
            let expanded = expand(&said, false);
            assert_eq!(expanded.0.len(), n, "{text}: {:?}", expanded.0);
            assert!(matches!(expanded, Cow::Owned(_)), "{text}");
        }
        assert!(matches!(&expand(&effect("Then support 1."), false).0[0], Sentence::Led { lead: Lead::Then, .. }));
        // a spell's support has no "other" (701.41a)
        assert_ne!(expand(&effect("Support 2."), true).0, expand(&effect("Support 2."), false).0);
        // inside a larger sentence: left as it is
        let within = effect("You gain 2 life and bolster 2.");
        assert!(matches!(expand(&within, false), Cow::Borrowed(_)));
    }
}
