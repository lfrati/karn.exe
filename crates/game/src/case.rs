//! Case solving uses the ordinary triggered ability/intervening-if pipeline (719.3a).
use ir::{Ability, Condition, Sentence, Lead, Modifier};
use std::sync::OnceLock;

pub(crate) fn to_solve(ability: &Ability) -> Option<Ability> {
    let Ability::ToSolve(condition) = ability else { return None };
    static RULE: OnceLock<Ability> = OnceLock::new();
    let mut rule = RULE.get_or_init(|| {
        let text = "At the beginning of your end step, if this permanent isn't solved, this permanent becomes solved.";
        let normalized=oracle::normalize::normalize(text,"Case","Case","Enchantment — Case");
        match grammar::ability::abilities().ability.parse_all(&oracle::token::tokenize(&normalized.rows[0])) {
            grammar::kernel::Outcome::Accepted { value, .. } => value,
            other => panic!("719.3a rule reads: {other:?}"),
        }
    }).clone();
    let Ability::Triggered { effect, .. } = &mut rule else { unreachable!() };
    let Sentence::Led { lead: Lead::Modifier(Modifier::If(unsolved)), .. } = &mut effect.0[0] else { unreachable!() };
    *unsolved = Condition::And(vec![condition.clone(), unsolved.clone()]);
    Some(rule)
}
