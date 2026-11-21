//! Bounded changes to one proposed damage amount (614.1a), preserving source and recipient.
use super::*;
use ir::effect::Subject;
use ir::statics::{Static, StaticAct, Substitute};
use ir::{Action, Domain, EventAction, Filter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    filter: expr::Filter,
    controller: Option<expr::Controller>,
    other: bool,
}
impl Source {
    fn compile(s: &Selector, support: &Support<'_>) -> Res<Self> {
        if s.set.domain != Domain::Source
            || !matches!(s.quant, Quant::Indefinite(ir::Count::Exactly(Amount::Const(1))))
            || s.chooser.is_some()
            || s.random.is_some()
            || s.set.zone.is_some()
            || s.set.owner.is_some()
            || s.set.caster.is_some()
        {
            return unsupported("damage replacement: a bounded source description");
        }
        Ok(Self {
            filter: expr::Filter::compile(&s.set.filter, support, &mut linked(support))?,
            controller: s.set.controller.as_ref().map(expr::Controller::compile).transpose()?,
            other: s.set.other,
        })
    }
    pub(crate) fn matches(&self, game: &Game, ctx: &Ctx, source: ObjId) -> bool {
        (!self.other || ctx.source != Some(source))
            && self.controller.is_none_or(|c| c.matches(ctx, game.controller(source)))
            && self.filter.matches(game, ctx, &NoLocals, source.into())
    }
}

fn unqualified(s: &Selector) -> bool {
    s.chooser.is_none()
        && s.random.is_none()
        && s.set.controller.is_none()
        && s.set.owner.is_none()
        && s.set.caster.is_none()
        && s.set.zone.is_none()
        && !s.set.other
}
fn same_source(s: &Selector) -> bool {
    unqualified(s)
        && s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It))
        && s.set.filter == Filter::All(vec![])
        && matches!(s.set.domain, Domain::Object | Domain::Source)
}
fn same_recipient(s: &Selector, to: &[Selector]) -> bool {
    if !unqualified(s) || !matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::That))) {
        return false;
    }
    to.iter().any(|original| s.set == original.set)
}

pub(super) fn compile(a: &Ability, support: &Support<'_>) -> Option<Res<Replacement>> {
    let Ability::Text(Effect(sentences)) = a else { return None };
    let [Sentence::Static(statement)] = sentences.as_slice() else { return None };
    let Static::Replacement(r) = statement.as_ref() else { return None };
    let [EventAction::DealsDamage { kind, amount: None, to: Some(to) }] = r.event.actions.as_slice() else {
        return None;
    };
    Some((|| {
        if r.next_time
            || r.this_turn
            || r.instead_first
            || r.while_.is_some()
            || r.event.counted()
            || r.event.during.is_some()
            || r.event.condition.is_some()
            || r.event.cause.is_some()
        {
            return unsupported("damage replacement: only unconditional amount changes");
        }
        let [source] = r.event.subjects.as_slice() else {
            return unsupported("damage replacement: one source");
        };
        let Substitute::Body(body) = &r.with else {
            return unsupported("damage replacement: preserve the damage recipient");
        };
        let clause = &body.first;
        if !body.rest.is_empty()
            || clause.may
            || clause.also
            || !clause.rest.is_empty()
            || clause.first.may
            || clause.first.mods.as_slice() != [ir::Modifier::Instead]
        {
            return unsupported("damage replacement: one amount change");
        }
        let Some(Subject::Selector(actor)) = &clause.subject else {
            return unsupported("damage replacement: same source");
        };
        if !same_source(actor) {
            return unsupported("damage replacement: same source");
        }
        let amount = match &clause.first.action {
            Action::Static(act) => match act.as_ref() {
                StaticAct::DealsToSame(n) => n,
                _ => return unsupported("damage replacement: amount only"),
            },
            Action::DealDamage(hits) => {
                let [hit] = hits.as_slice() else {
                    return unsupported("damage replacement: one hit");
                };
                if hit.amount_after_targets || hit.to.items.len() != 1 || !same_recipient(&hit.to.items[0], &to.items) {
                    return unsupported("damage replacement: same recipient");
                }
                &hit.amount
            }
            _ => return unsupported("damage replacement: amount only"),
        };
        let that = |n: &Amount| matches!(n, Amount::That { many: false });
        let change = match amount {
            Amount::Plus(a, b) if that(a) => match b.as_ref() {
                Amount::Const(n) => CounterChange::Plus(*n),
                _ => return unsupported("damage replacement: constant addition"),
            },
            Amount::Multiple(k, n) if that(n) => CounterChange::Times(*k),
            _ => return unsupported("damage replacement: constant plus or multiple of that damage"),
        };
        if to.items.is_empty() {
            return unsupported("damage replacement: recipient");
        }
        let to = to
            .items
            .iter()
            .map(|s| {
                crate::event::damage_recipient_ok(&s.set)?;
                Noun::compile(s, support)
            })
            .collect::<Res<Vec<_>>>()?;
        Ok(Replacement::Damage { from: Source::compile(source, support)?, to, kind: *kind, change })
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_full_cards_lower_damage_amount_replacements() {
        for name in ["Embermaw Hellion", "Torbran, Thane of Red Fell", "Furnace of Rath"] {
            let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
            let def = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
            assert!(
                def.replacements.iter().any(|r| matches!(r.result, Ok(Replacement::Damage { .. }))),
                "{name}: {:?} {:?}",
                def.replacements,
                def.abilities
            );
        }
    }
    #[test]
    fn source_applicability_uses_lki_controller_color_and_another() {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let hellion = CardDef::from_faces(CardDef::catalog(&faces).get("Embermaw Hellion").unwrap()).unwrap();
        let red = CardDef::new("Red", "{R}", "Creature", "", Some((3, 3))).unwrap();
        let blue = CardDef::new("Blue", "{U}", "Creature", "", Some((3, 3))).unwrap();
        let mut g = Game::new(vec![hellion, red, blue], 2, 20);
        let provider = g.put(0, 0, crate::Zone::Battlefield);
        let source = g.put(0, 1, crate::Zone::Battlefield);
        let wrong_controller = g.put(1, 1, crate::Zone::Battlefield);
        let wrong_color = g.put(0, 2, crate::Zone::Battlefield);
        let Replacement::Damage { from, .. } = g.defs[0].replacements[0].result.as_ref().unwrap() else {
            panic!("modifier");
        };
        let from = from.clone();
        let ctx = Ctx::new(0, provider);
        assert!(from.matches(&g, &ctx, source));
        assert!(!from.matches(&g, &ctx, provider));
        assert!(!from.matches(&g, &ctx, wrong_controller));
        assert!(!from.matches(&g, &ctx, wrong_color));
        g.move_to(source, crate::Zone::Graveyard, 0);
        assert_eq!(g.obj(source).zone, crate::Zone::Gone);
        assert!(from.matches(&g, &ctx, source), "departed incarnation keeps its red/controller facts");
    }
    #[test]
    fn unsupported_damage_substitutions_are_not_admitted() {
        for (text, reason) in [
            ("If a source would deal damage to a player, it deals that much damage plus X instead.", "damage replacement: constant addition"),
            ("If a source would deal damage to a player, it deals 3 damage instead.", "damage replacement: constant plus or multiple of that damage"),
            ("If a source would deal damage to a player, it deals that much damage plus 1 to target creature instead.", "damage replacement: same recipient"),
            ("If a source would deal damage to a player, it deals that much damage plus 1 to that creature instead.", "damage replacement: same recipient"),
            ("If a source would deal damage to a player this turn, it deals double that damage instead.", "damage replacement: only unconditional amount changes"),
        ] {
            let def = CardDef::new("Bound", "{0}", "Enchantment", text, None).unwrap_or_else(|error| panic!("{text}: {error}"));
            assert_eq!(crate::play::check(&def).unwrap_err().0, reason, "{text}");
        }
    }
}
