//! Named inputs of an ability occurrence (608.2k). Definition-time schemas and occurrence
//! values share keys; no runtime search for the most recently mentioned object or event.
use crate::{Res, unsupported};
use crate::select::Support;
use ir::{Ability, Amount, CostPart, Count, Quant, Ref, Role, Selector};
use ir::event::Trigger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Subject,
    /// No longer produced by a trigger: a role is not the same participant in each of a
    /// pattern's actions.
    /// What took part in the event besides the subject, by its place in the action that
    /// happened: the phrase's arguments ([`crate::trigger::arguments`]), then what every
    /// such event says that the phrase leaves unsaid ([`crate::trigger::unsaid`]). The same
    /// producer whichever of the pattern's actions happened, whatever role it fills in each.
    Argument(usize),
    Cost(usize),
    /// The explicitly named source of a nonmoving payment, such as removing
    /// counters from this creature. Kept independently of selected cost groups.
    CostSource,
    /// Defender attacked immediately before this cost was paid (702.49c).
    CostDefender(usize),
    /// The opponent chosen for a spell's gift as it was cast, an additional cost (702.174a):
    /// "the chosen player". None was chosen where the gift was not promised.
    Gift,
    /// Successful tapping resources of this spell, independent of mana production.
    Convoked,
}

/// How the text names a participant, which decides what a later "that creature" or "that
/// player" can be said of. The text says "this creature" and "you" again of its own source
/// and controller, and "equipped creature" again of what it is attached to; "that creature"
/// is said of what it introduced with "a creature".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Standing {
    /// "this creature", "you".
    Own,
    /// Any other definite phrase: "enchanted creature", "equipped creature".
    Definite,
    /// "a creature", "a player"; and what the phrase leaves unsaid.
    Introduced,
}

fn standing<'a>(ss: impl IntoIterator<Item = &'a Selector>) -> Standing {
    ss.into_iter().map(|s| match s.quant {
        Quant::Definite(Ref::This | Ref::You) => Standing::Own,
        Quant::Definite(_) => Standing::Definite,
        _ => Standing::Introduced,
    }).max().unwrap_or(Standing::Introduced)
}

#[derive(Clone)]
struct Producer {
    input: Input,
    paid_by: Option<ir::Verb>,
    players: bool,
    max: u32,
    standing: Standing,
    /// The phrase says it, in at least one of the pattern's actions; false for what every
    /// such event names but the phrase does not.
    said: bool,
    /// What it is said to be, one description for each alternative.
    described: Vec<ir::ObjectSet>,
}

impl Producer {
    /// "that planeswalker" after "deals damage to a planeswalker": the same noun.
    fn named_by(&self, set: &ir::ObjectSet) -> bool {
        self.described.iter().any(|d| d.domain == set.domain && d.filter == set.filter)
    }
}

/// A participant's declared upper bound. Selector alternatives describe the same
/// occurrence slot, so their bound is the maximum, not a combined object count.
fn maximum<'a>(selectors: impl IntoIterator<Item = &'a Selector>) -> u32 {
    selectors.into_iter().map(|selector| match &selector.quant {
        Quant::Indefinite(Count::Exactly(Amount::Const(n)) | Count::Precisely(Amount::Const(n)) | Count::UpTo(Amount::Const(n)))
        | Quant::Target(Count::Exactly(Amount::Const(n)) | Count::Precisely(Amount::Const(n)) | Count::UpTo(Amount::Const(n))) => *n,
        Quant::Indefinite(Count::Range { max, .. }) | Quant::Target(Count::Range { max, .. }) =>
            if let Amount::Const(n) = &**max { *n } else { u32::MAX },
        Quant::Indefinite(_) | Quant::Target(_) | Quant::All | Quant::Each | Quant::Half(_) | Quant::The => u32::MAX,
        Quant::Definite(Ref::Anaphor(ir::Anaphor::Them | ir::Anaphor::They | ir::Anaphor::Those | ir::Anaphor::The { plural: true })) => u32::MAX,
        Quant::No => 0,
        _ => 1,
    }).max().unwrap_or(1)
}

fn players(s: &Selector) -> Option<bool> { ir::lexicon::denotes_players(s) }
fn alternatives<'a>(ss: impl IntoIterator<Item = &'a Selector>) -> Option<bool> {
    let mut ss = ss.into_iter();
    let first = players(ss.next()?)?;
    ss.all(|s| players(s) == Some(first)).then_some(first)
}

fn trigger(t: &Trigger) -> Vec<Producer> {
    match t {
        Trigger::Step { .. } => vec![Producer { input: Input::Subject, paid_by: None, players: true, max: 1, standing: Standing::Introduced, said: true, described: Vec::new() }],
        Trigger::Event { pattern, .. } => event(pattern),
        Trigger::Any(ts) => {
            let schemas: Vec<_> = ts.iter().map(trigger).collect();
            schemas.first().into_iter().flatten().filter(|p| schemas.iter().all(|schema| schema.iter().any(|s| s.input == p.input && s.players == p.players && s.standing == p.standing && s.said == p.said)))
                .map(|p| Producer { max: schemas.iter().flat_map(|schema| schema.iter().filter(|s| s.input == p.input).map(|s| s.max)).max().unwrap_or(p.max), ..p.clone() }).collect()
        }
        // "when Garruk has two or fewer loyalty counters on him, transform him": what the
        // state is said of
        Trigger::State(ir::Condition::Is { what, .. }) => players(what).into_iter()
            .map(|players| Producer { paid_by: None, input: Input::Subject, players, max: 1, standing: standing([&**what]), said: true, described: vec![what.set.clone()] }).collect(),
        Trigger::State(_) => Vec::new(),
    }
}

/// What an event that matches the pattern supplies.
fn event(pattern: &ir::EventPattern) -> Vec<Producer> {
    let mut result = Vec::new();
    if let Some(players) = alternatives(&pattern.subjects) {
        // the subject, whichever of the actions happened
        result.push(Producer { input: Input::Subject, paid_by: None, players, max: maximum(&pattern.subjects), standing: standing(&pattern.subjects), said: true,
            described: pattern.subjects.iter().map(|s| s.set.clone()).collect() });
    }
    // What else takes part in each action, in the order the matched event supplies
    // it: the phrase's arguments, then what every such event says that the phrase
    // leaves unsaid. None: alternatives that are not all players or all objects.
    let subject_named = !pattern.subjects.is_empty();
    let said = crate::trigger::said(pattern);
    // "you attack with three or more creatures": the attackers counted, all of them (508.3c)
    let batched = matches!(pattern.subjects.as_slice(), [s] if s.quant == Quant::Indefinite(Count::AtLeast(Amount::Const(1))));
    let counted = |action: &ir::event::EventAction, role: Role| role == Role::With && matches!(action, ir::event::EventAction::Attacks { .. });
    let actions: Vec<Vec<(Option<bool>, Vec<&Selector>, bool)>> = said.iter().map(|action| {
        let said = crate::trigger::arguments(action);
        said.into_iter().map(|(role, said)| (alternatives(said.iter().copied()), said, counted(action, role)))
            // of "one or more" of them, what is unsaid is of each of them, however many (603.2c)
            .chain(crate::trigger::unsaid(action, subject_named).into_iter().map(|(_, players)| (Some(players), Vec::new(), batched)))
            .collect()
    }).collect();
    // Only what every action supplies there, as the same kind of thing.
    if let Some(first) = actions.first() {
        for (at, (players, _, _)) in first.iter().enumerate() {
            if let Some(players) = *players && actions.iter().all(|others| others.get(at).map(|(p, _, _)| *p) == Some(Some(players))) {
                let said: Vec<&Selector> = actions.iter().flat_map(|a| a[at].1.iter().copied()).collect();
                let max = if actions.iter().any(|a| a[at].2) { u32::MAX } else { maximum(said.iter().copied()) };
                result.push(Producer { input: Input::Argument(at), paid_by: None, players, max, standing: standing(said.iter().copied()),
                    said: !said.is_empty(), described: said.iter().map(|s| s.set.clone()).collect() });
            }
        }
    }
    result
}

fn schema(support: &Support<'_>) -> Vec<Producer> {
    // a delayed triggered ability's instructions are of the event it watches for
    if let Some(pattern) = support.event() { return event(pattern); }
    match support.ability() {
        Some(Ability::Triggered { trigger: t, .. }) => trigger(t),
        Some(Ability::Activated { .. } | Ability::Text(_)) => {
            let mut result = Vec::new();
            for part in support.costs().into_iter().flat_map(|cost| &cost.0) {
                let CostPart::Action(action) = part else { continue };
                let Some((verb, objects)) = crate::activate::object_payment(action) else { continue };
                let max = objects.items.iter().try_fold(0u32, |sum, s| {
                    let n = match &s.quant { Quant::Definite(Ref::This) => 1,
                        Quant::Indefinite(Count::Exactly(Amount::Const(n))) => *n, _ => return None };
                    sum.checked_add(n)
                }).unwrap_or(u32::MAX);
                result.push(Producer { input: Input::Cost(result.len()), paid_by: Some(verb), players: false, max, standing: standing(&objects.items), said: true,
                    described: objects.items.iter().map(|s| s.set.clone()).collect() });
            }
            let source = support.costs().into_iter().flat_map(|cost| &cost.0).find_map(|part| match part {
                CostPart::Action(ir::Action::RemoveCounters { from: Some(from), .. })
                    if from.quant == Quant::Definite(Ref::This) => Some(from),
                _ => None,
            });
            if let Some(source) = source {
                result.push(Producer { input: Input::CostSource, paid_by: Some(ir::Verb::RemoveCounters), players: false, max: 1,
                    standing: Standing::Own, said: true, described: vec![source.set.clone()] });
            }
            // a spell's gift: "you may choose an opponent" (702.174a)
            if matches!(support.ability(), Some(Ability::Text(_))) && support.has_gift() {
                result.push(Producer { input: Input::Gift, paid_by: Some(ir::Verb::Choose), players: true, max: 1, standing: Standing::Introduced, said: true,
                    described: vec![ir::ObjectSet::of(ir::Domain::Player)] });
            }
            result
        }
        _ => Vec::new(),
    }
}

/// Whether the text names that input as its own source or controller ("this creature",
/// "you"): a later "that creature" is said of something else where it can be.
pub(super) fn own(support: &Support<'_>, input: Input) -> bool {
    schema(support).iter().any(|p| p.input == input && p.standing == Standing::Own)
}

/// Personal pronouns carry no noun in the IR. A singular player can be "them";
/// an object occurrence supplies a plural group, or an explicitly self-named source
/// such as Ran and Shaw (one object despite its plural proper name, CR 201.5).
/// Qualified nouns retain their declared sorts and ordinary matching rules.
pub(super) fn bare_personal(selector: &Selector) -> bool {
    matches!(selector.quant, Quant::Definite(Ref::Anaphor(ir::selector::Anaphor::Them | ir::selector::Anaphor::They)))
        && selector.set == ir::ObjectSet::of(ir::Domain::Object)
        && selector.chooser.is_none() && selector.random.is_none()
}

/// The occurrence declaration, rather than the placeholder noun, owns its type and bound.
pub(super) fn signature(support: &Support<'_>, input: Input) -> Res<(bool, u32)> {
    schema(support).iter().find(|producer| producer.input == input)
        .map(|producer| (producer.players, producer.max))
        .ok_or_else(|| crate::Unsupported("input reference: missing declared participant".into()))
}

pub(super) fn resolve(support: &Support<'_>, selector: &Selector) -> Res<Input> {
    find(support, selector)?.ok_or_else(|| crate::Unsupported("input reference: nothing before it supplies the required participant/cardinality".into()))
}

/// One defender retained from one cost participant. A plural payment cannot silently
/// choose the first of several attacked players (508.4, 702.49c).
pub(super) fn cost_defender(support: &Support<'_>, selector: &Selector) -> Res<Input> {
    let input = resolve(support, selector)?;
    match input {
        Input::Cost(index) if schema(support).iter().any(|p| p.input == input && p.max == 1) => Ok(Input::CostDefender(index)),
        _ => unsupported("entry defender requires a singular cost participant"),
    }
}

/// What of the occurrence the reference names; none where nothing of it can be meant.
pub(super) fn find(support: &Support<'_>, selector: &Selector) -> Res<Option<Input>> {
    let Quant::Definite(Ref::Anaphor(form)) = selector.quant else { return unsupported("input reference: unsupported form"); };
    use ir::selector::Anaphor;
    if let Anaphor::TheParticiple { what, plural } = form {
        let candidates:Vec<_>=schema(support).into_iter().filter(|p|p.paid_by==Some(what) && (plural || p.max<=1) && p.named_by(&selector.set)).collect();
        return match candidates.as_slice() {
            [one]=>Ok(Some(one.input)), []=>Ok(None),
            _=>unsupported("input reference: ambiguous paid-action producers"),
        };
    }
    let singular = match form {
        Anaphor::It | Anaphor::That | Anaphor::The { plural: false } => true,
        Anaphor::Them | Anaphor::They | Anaphor::Those | Anaphor::The { plural: true } => false,
        _ => return unsupported("input reference: qualified form not bound"),
    };
    let possibilities = selector.set.possibilities();
    let personal = bare_personal(selector);
    let fits = |p: &Producer| (!singular || p.max <= 1) && if personal {
        p.players || p.max > 1 || p.standing == Standing::Own
    } else {
        possibilities.iter().any(|s| (s.domain.sort == ir::Sort::Player) == p.players)
    };
    let candidates: Vec<_> = schema(support).into_iter().filter(fits).collect();
    // "it" denotes the trigger's grammatical subject; other nouns can name a role.
    if form == Anaphor::It && let Some(subject) = candidates.iter().find(|p| p.input == Input::Subject) { return Ok(Some(subject.input)); }
    // Each step keeps what the text more plainly means while anything is left: what the
    // phrase says over what it leaves unsaid ("whenever a creature blocks, … that creature's
    // controller": the blocker); what it introduced over what it names definitely ("whenever
    // equipped creature deals damage to a creature, exile that creature"); what it called by
    // the same noun ("to a planeswalker, destroy that planeswalker"). Several left is a
    // reference the engine does not follow.
    let mut candidates: Vec<&Producer> = candidates.iter().collect();
    let narrow = |candidates: &mut Vec<&Producer>, keep: &dyn Fn(&Producer) -> bool| {
        if candidates.iter().any(|p| keep(p)) { candidates.retain(|p| keep(p)); }
    };
    narrow(&mut candidates, &|p| p.said);
    let best = candidates.iter().map(|p| p.standing).max();
    narrow(&mut candidates, &|p| Some(p.standing) == best);
    if candidates.len() > 1 { narrow(&mut candidates, &|p| p.named_by(&selector.set)); }
    match candidates.as_slice() {
        [one] => Ok(Some(one.input)),
        [] => Ok(None),
        _ => unsupported("input reference: ambiguous occurrence participants"),
    }
}

/// A pronoun's type follows its occurrence producer, not the generic noun's default
/// permanent domain. Copying an activated event's "it" needs an actual stack participant.
pub(super) fn stack_reference(support: &Support<'_>, selector: &Selector) -> Option<Input> {
    let bare = matches!(selector.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::It)));
    let explicit = matches!(selector.set.domain, ir::Domain::Spell | ir::Domain::Ability)
        && matches!(selector.quant, Quant::Definite(Ref::Anaphor(_)));
    if !(bare || explicit) || selector.chooser.is_some() || selector.random.is_some()
        || selector.set != ir::ObjectSet::of(selector.set.domain.clone()) { return None; }
    let input = find(support, selector).ok().flatten()?;
    schema(support).iter().any(|p| p.input == input && !p.players && p.max == 1
        && !p.described.is_empty() && p.described.iter().all(|set|
            matches!(set.domain, ir::Domain::Spell | ir::Domain::Ability))).then_some(input)
}

/// Announcement queries can read identified occurrence inputs before a body frame runs.
/// Earlier body-local results are unavailable at announcement and remain refused.
pub(crate) struct Binding<'a, 's>(pub &'a Support<'s>);
impl super::expr::Binding for Binding<'_, '_> {
    fn scalar(&mut self, r: &ir::reference::UnresolvedRef) -> Res<crate::binding::Scalar> {
        let (link,kind)=self.0.choice(r.expected)?;Ok(crate::binding::Scalar::Linked{link,kind})
    }
    fn subject(&mut self, s: &Selector) -> Res<super::expr::Subject> {
        let input = resolve(self.0,s)?;
        if matches!(input,Input::Cost(_) | Input::CostSource | Input::Gift) { return unsupported("target query: a cost result is not available during announcement"); }
        Ok(super::expr::Subject::Input(input))
    }
}
impl super::expr::Locals for crate::state::Bound {
    fn local(&self, _: usize) -> Option<&crate::binding::Value> { None }
    fn input(&self, key: Input) -> Option<&[crate::Target]> {
        self.inputs.iter().find(|(k,_)|*k==key).map(|(_,v)|v.as_slice())
    }
    fn event_amount(&self) -> Option<u32> { self.event_amount }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{Domain, ObjectSet};

    fn definition(condition: &str) -> crate::CardDef {
        crate::CardDef::new("Witness", "{0}", "Creature", &format!("Whenever {condition}, you gain 1 life."), Some((2, 2))).unwrap()
    }
    fn trigger_of(def: &crate::CardDef) -> Trigger {
        let Ability::Triggered { trigger, .. } = &def.abilities()[0] else { panic!("triggered definition") };
        trigger.clone()
    }
    fn pronoun(form: ir::Anaphor) -> Selector {
        Selector::new(Quant::Definite(Ref::Anaphor(form)), ObjectSet::of(Domain::Object))
    }

    #[test]
    fn personal_reference_distinguishes_a_said_plural_argument_from_its_player_actor() {
        // Compiler contract: the declared two-artifact participant is plural even
        // though its actor "you" is singular. This does not admit a runtime batch trigger.
        let def = definition("you sacrifice two artifacts");
        let support = Support::definition(&def, Some(crate::binding::AbilityKey::printed(0)));
        assert_eq!(find(&support, &pronoun(ir::Anaphor::Them)).unwrap(), Some(Input::Argument(0)));
        assert_eq!(signature(&support, Input::Argument(0)).unwrap(), (false, 2));
        assert_eq!(find(&support, &pronoun(ir::Anaphor::It)).unwrap(), None);
        // Two introduced participants, a player and a plural object group, remain
        // ambiguous: the engine must not silently choose either of them.
        let def = definition("an opponent sacrifices two artifacts");
        let support = Support::definition(&def, Some(crate::binding::AbilityKey::printed(0)));
        assert!(find(&support, &pronoun(ir::Anaphor::Them)).unwrap_err().0.contains("ambiguous occurrence participants"));
    }

    #[test]
    fn personal_reference_to_a_self_named_source_keeps_its_single_object_signature() {
        let def = crate::CardDef::new("Ran and Shaw", "{0}", "Creature — Dragon",
            "When Ran and Shaw enter, if you cast them, you gain 1 life.", Some((4, 4))).unwrap();
        let support = Support::definition(&def, Some(crate::binding::AbilityKey::printed(0)));
        assert_eq!(find(&support, &pronoun(ir::Anaphor::Them)).unwrap(), Some(Input::Subject));
        assert_eq!(signature(&support, Input::Subject).unwrap(), (false, 1));
        // An introduced singular creature remains outside this self-reference case;
        // the separate player argument retains the personal-pronoun signature.
        let def = definition("a creature enters");
        let support = Support::definition(&def, Some(crate::binding::AbilityKey::printed(0)));
        assert_eq!(find(&support, &pronoun(ir::Anaphor::Them)).unwrap(), Some(Input::Argument(0)));
        assert_eq!(signature(&support, Input::Argument(0)).unwrap(), (true, 1));
    }

    #[test]
    fn alternative_occurrence_schemas_preserve_the_largest_declared_participant() {
        // Compiler contract: either event may happen, so a singular reference is
        // not guaranteed merely because the first alternative names one artifact.
        let one = definition("you sacrifice an artifact");
        let two = definition("you sacrifice two artifacts");
        let schema = trigger(&Trigger::Any(vec![trigger_of(&one), trigger_of(&two)]));
        let artifact = schema.iter().find(|producer| producer.input == Input::Argument(0)).unwrap();
        assert_eq!((artifact.players, artifact.max), (false, 2));
    }
}
