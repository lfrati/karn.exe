//! Static loyalty timing permissions, queried for the effective source (606.3, 611.3a).
use crate::program::registration::{Registered, Subject, When};
use crate::program::expr;
use crate::select::{Ctx, Support};
use crate::{CardDef, Game, ObjId, PlayerId, Res, Target, unsupported};
use ir::{Ability, Condition, Effect, Lead, Quant, Ref, Sentence};

#[derive(Debug, Clone)]
pub(crate) struct Permission {
    source: Subject,
    when: When,
    scope: crate::functions::Scope,
}

fn statement<'a>(sentence: &'a Sentence, conditions: &mut Vec<&'a Condition>) -> Option<&'a ir::statics::AbilitiesOf> {
    match sentence {
        Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::AsLongAs(c) | ir::effect::Modifier::If(c)), rest } => {
            conditions.push(c);
            statement(rest, conditions)
        }
        Sentence::Static(s) => match s.as_ref() {
            ir::statics::Static::ActivateAtInstantSpeed { of } => Some(of),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn registrations(def: &CardDef) -> Vec<Registered<Permission>> {
    def.abilities.iter().enumerate().filter_map(|(ability, a)| {
        let Ability::Text(Effect(rows)) = a else { return None };
        let [sentence] = rows.as_slice() else { return None };
        let mut conditions = Vec::new();
        let of = statement(sentence, &mut conditions)?;
        let result = (|| {
            let ir::statics::AbilitiesOf::Of { kind: ir::action::AbilityKind::Loyalty, of } = of else {
                return unsupported("activation timing: only loyalty ability permissions are implemented (606.3)");
            };
            let [source] = of.items.as_slice() else { return unsupported("activation timing: one source description required"); };
            let support = Support::definition(def, Some(crate::binding::AbilityKey::printed(ability)));
            let mut bind = expr::linked(&support);
            let source = match source {
                ir::statics::Who::Set(set) => ir::Subject::Set(set.clone()),
                ir::statics::Who::One(selector) => {
                    let mut selector = selector.clone();
                    // A possessive pronoun names the explicit source in its condition,
                    // not an arbitrary permanent or another planeswalker (201.5).
                    if selector.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) {
                        if selector.set != ir::ObjectSet::of(ir::Domain::Object) || selector.chooser.is_some() || selector.random.is_some() {
                            return unsupported("activation timing: qualified source pronoun");
                        }
                        let antecedents: Vec<_> = conditions.iter().filter_map(|c| match c {
                            Condition::Happened { pattern, .. } => match pattern.subjects.as_slice() {
                                [source] if source.quant == Quant::Definite(Ref::This) => Some(source),
                                _ => None,
                            },
                            _ => None,
                        }).collect();
                        let [antecedent] = antecedents.as_slice() else { return unsupported("activation timing: source pronoun has no unique antecedent"); };
                        selector = (*antecedent).clone();
                    }
                    ir::Subject::Selector(selector)
                }
            };
            let source = Subject::compile(&source, &support, &mut bind)?;
            let when = When::compile(&crate::statement::When::All(conditions.iter().map(|c| crate::statement::When::AsLongAs(c)).collect()), &support, &mut bind)?;
            if when.reads_targets() {
                return unsupported("activation timing: a static permission cannot retain an announcement's choices");
            }
            Ok(Permission { source, when, scope: def.scope(ability).expect("a declared static permission")? })
        })();
        Some(Registered { ability, result })
    }).collect()
}

impl CardDef {
    pub(crate) fn activation_timing_ok(&self, ability: usize) -> Option<Res<()>> {
        self.activation_timing.iter().find(|r| r.ability == ability).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }
}

impl Game {
    pub(super) fn instant_loyalty(&self, player: PlayerId, source: ObjId) -> bool {
        let changed = self.membership_changed();
        self.battlefield.iter().filter(|grant| self.controller(**grant) == player).any(|grant| {
            self.origins(*grant, crate::ability::View::Effective { membership_changed: changed }).into_iter().any(|(origin, def)| {
                def.activation_timing.iter().any(|registration| {
                    let Ok(permission) = &registration.result else { return false };
                    if !permission.scope.contains(self.obj(*grant).zone) { return false; }
                    let ctx = Ctx::new(player, *grant).with_key(crate::binding::AbilityKey { origin: origin.clone(), index: registration.ability });
                    permission.source.contains(self, &ctx, Target::Object(source)) && permission.when.holds(self, &ctx)
                })
            })
        })
    }
}
