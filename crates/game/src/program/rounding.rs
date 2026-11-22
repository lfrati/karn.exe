//! Bind a trailing rounding instruction to the direct quantities of its preceding sentence.
use crate::{Res, unsupported};
use ir::{Action, Amount, Effect, Sentence};

/// "You gain half X life and draw half X cards. Round down each time."
/// Preserve each clause's subject, joins and optionality; only its half quantities change.
pub(super) fn apply(effect: &mut Effect) -> Res<()> {
    let mut sentences = Vec::new();
    for sentence in std::mem::take(&mut effect.0) {
        let Sentence::RoundEach { up } = sentence else { sentences.push(sentence); continue };
        let Some(Sentence::Do(body)) = sentences.last_mut() else {
            return unsupported("round each: a preceding ordinary quantity sentence required");
        };
        let mut halves = 0;
        for clause in std::iter::once(&mut body.first).chain(body.rest.iter_mut().map(|(_, c)| c)) {
            for act in std::iter::once(&mut clause.first).chain(clause.rest.iter_mut().map(|(_, a)| a)) {
                if !act.mods.is_empty() { return unsupported("round each: modified quantity instructions are not implemented"); }
                let amount = match &mut act.action {
                    Action::GainLife(n) | Action::LoseLife(n) | Action::Draw(n) => n,
                    _ => return unsupported("round each: only direct life and draw quantities are implemented"),
                };
                if let Amount::Half { round_up, .. } = amount {
                    if round_up.is_some_and(|own| own != up) { return unsupported("round each: conflicts with a local rounding instruction"); }
                    *round_up = Some(up);
                    halves += 1;
                }
            }
        }
        if halves == 0 { return unsupported("round each: the preceding sentence has no direct half quantity"); }
    }
    effect.0 = sentences;
    Ok(())
}
