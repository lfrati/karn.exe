//! Static total-cost modifiers (118.7, 601.2f), compiled at definition time. The query runs
//! after announcement and before mana production; its result becomes the retained cost.
use crate::{CardDef, Game, ObjId, Res, Target, unsupported};
use crate::binding::AbilityKey;
use crate::program::expr::{self, NoLocals, linked};
use crate::program::registration::{Registered, Subject, When};
use crate::select::{Ctx, Support};
use ir::{Ability, Action};
use ir::mana::ManaSymbol;
use ir::statics::{CostOf, StaticAct};

#[derive(Debug, Clone)]
pub(crate) struct Modifier {
    subject: Subject,
    scope: crate::functions::Scope,
    changes: Vec<(When, Change, expr::Amount)>,
    /// "Spells you cast from anywhere other than your hand": the zones the spell was cast
    /// from (601.2a), or all but them.
    origin: Option<Origin>,
    /// Completed matching casts before this announcement (601.2f, 601.2i).
    ordinal: Option<(u32, crate::trigger::Pattern)>,
}

#[derive(Debug, Clone)]
struct Origin { zones: Vec<crate::Zone>, other_than: bool }

impl Origin {
    /// "from your graveyard", "from anywhere other than your hand": of the caster's own
    /// zones, as a spell is cast by its owner from those (601.2a).
    fn compile(from: &ir::ZoneSpec, other_than: bool) -> Res<Origin> {
        if from.slice.is_some() || !from.owner.as_deref().is_some_and(crate::select::is_you) {
            return unsupported("cost change: cast from a zone other than its caster's own");
        }
        let zones = from.zones.items.iter().map(|zone| match zone {
            ir::Zone::Hand => Ok(crate::Zone::Hand),
            ir::Zone::Graveyard => Ok(crate::Zone::Graveyard),
            ir::Zone::Library => Ok(crate::Zone::Library),
            _ => unsupported("cost change: cast from that zone"),
        }).collect::<Res<_>>()?;
        Ok(Origin { zones, other_than })
    }
    fn holds(&self, cast_from: Option<crate::Zone>) -> bool {
        cast_from.is_some_and(|zone| self.zones.contains(&zone) != self.other_than)
    }
}

#[derive(Debug, Clone, Copy)]
enum Change { Increase, Reduce, Minimum }

/// Whole-ability ownership: a mixed or qualified unsupported statement must still refuse.
fn statement(a: &Ability) -> Option<crate::statement::Statement<'_>> {
    let stated = crate::statement::read(a)?;
    stated.acts.iter().any(|(a, _, _)| matches!(a, Action::Static(s) if matches!(s.as_ref(), StaticAct::CostChange { .. } | StaticAct::CostMinimum { .. }))).then_some(stated)
}

pub(crate) fn owns(a: &Ability) -> bool { statement(a).is_some() }

pub(crate) fn self_modifier(a: &Ability) -> bool {
    statement(a).is_some_and(|s| matches!(s.subject, ir::Subject::Selector(s) if s.quant == ir::Quant::Definite(ir::Ref::This)))
}

pub(crate) fn registrations(def: &CardDef) -> Vec<Registered<Modifier>> {
    def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, ability)| ability)).enumerate().filter_map(|(index, ability)| {
        let stated = statement(ability)?;
        let support = Support::definition(def, Some(AbilityKey::printed(index)));
        let result = (|| {
            let mut subject = stated.subject.clone();
            let ordinal = if let ir::Subject::Selector(s) = &subject
                && let ir::Quant::NthEachTurn(n) = s.quant {
                if n == 0 || s.set.domain != ir::Domain::Spell || s.chooser.is_some() || s.random.is_some() {
                    return unsupported("cost change: ordinal must describe a spell cast each turn");
                }
                let mut set = s.set.clone();
                let Some(caster) = set.caster.take() else { return unsupported("cost change: ordinal requires a caster"); };
                if caster.by.negated || caster.by.past || !crate::select::is_you(&caster.by.who)
                    || caster.from.is_some() || caster.this_turn || caster.other_than {
                    return unsupported("cost change: ordinal casts by this controller, without a zone qualifier");
                }
                let pattern = ir::event::EventPattern {
                    subjects: vec![(*caster.by.who).clone()],
                    actions: vec![ir::EventAction::Deed(ir::event::Deed {
                        verb: ir::Verb::Cast, passive: false,
                        args: vec![(ir::Role::What, ir::event::Arg::Noun(ir::Selector::new(
                            ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))), set)))],
                    })], nth_each_turn: None, during: None, except_nth_in_step: None, condition: None, cause: None,
                };
                let ordinal = Some((n, crate::trigger::Pattern::compile(&pattern, &support)?));
                subject = ir::Subject::Set(s.set.clone());
                ordinal
            } else { None };
            // During 601.2 the spell's controller is its caster (601.2a). This narrow
            // announcement-only equivalence does not equate them in ordinary queries.
            // Where it was cast from is the spell's own record of its announcement.
            fn announced_set(set: &mut ir::ObjectSet, origin: &mut Option<Origin>) -> Res<()> {
                if let Some(caster) = set.caster.take() {
                    if caster.this_turn || set.controller.is_some() {
                        return unsupported("cost change: qualified caster relation");
                    }
                    if let Some(from) = &caster.from {
                        if origin.is_some() { return unsupported("cost change: several cast origins"); }
                        *origin = Some(Origin::compile(from, caster.other_than)?);
                    }
                    set.controller = Some(caster.by);
                }
                if let ir::Domain::Union(arms) = &mut set.domain {
                    for arm in arms { announced_set(arm, origin)?; }
                }
                Ok(())
            }
            let mut origin = None;
            match &mut subject {
                ir::Subject::Set(set) => announced_set(set, &mut origin)?,
                ir::Subject::Selector(s) => announced_set(&mut s.set, &mut origin)?,
                _ => {}, // Subject::compile refuses unsupported subject forms.
            }
            let subject = Subject::compile(&subject, &support, &mut linked(&support))?;
            let scope = crate::functions::scope(ability, def.modes_of(index))?;
            let mut linked = linked(&support);
            let mut bind = expr::SourceBinding { parent: &mut linked, source: subject == Subject::Source };
            let mut changes = Vec::new();
            for (action, when, scale) in stated.acts {
                let Action::Static(action) = action else { return unsupported("cost change: mixed statement"); };
                if let StaticAct::CostMinimum { amount, of: CostOf::Cast } = action.as_ref() {
                    if !matches!(scale, crate::statement::Scale::Once) { return unsupported("cost minimum: scaled quantity"); }
                    changes.push((When::compile(&when, &support, &mut bind)?, Change::Minimum,
                        expr::Amount::compile(amount, &support, &mut bind)?));
                    continue;
                }
                let StaticAct::CostChange { more, by, of: CostOf::Cast } = action.as_ref() else {
                    return unsupported("cost change: only casting cost modifiers are implemented");
                };
                // A printed {X} in a modifier is its stated quantity, not another
                // casting announcement. Lower it with the same amount expressions (107.3c).
                let mut amount = ir::Amount::Const(0);
                for symbol in &by.0 {
                    let part = match (symbol, scale) {
                        (ManaSymbol::Generic(n), _) => ir::Amount::Const(*n),
                        (ManaSymbol::Var('X'), crate::statement::Scale::WhereX(n)) => n.clone(),
                        _ => return unsupported("cost change: only generic mana and explicitly defined X modifiers are implemented"),
                    };
                    amount = ir::Amount::Plus(Box::new(amount), Box::new(part));
                }
                let amount = expr::Amount::compile(&amount, &support, &mut bind)?;
                let amount = match scale {
                    crate::statement::Scale::ForEach(n) => amount.times(expr::Amount::compile(n, &support, &mut bind)?),
                    _ => amount,
                };
                changes.push((When::compile(&when, &support, &mut bind)?, if *more { Change::Increase } else { Change::Reduce }, amount));
            }
            let targets = changes.iter().any(|(when, _, _)| when.reads_targets());
            if targets {
                // Listings partition a single required target by its locked total cost.
                // Multi-target combinations need a relational offer rather than pretending
                // every combination in a Cartesian product has the same payment.
                if subject != Subject::Source { return unsupported("target-dependent cost: only the source spell's cost"); }
                if crate::program::spell_bodies(def)?.iter().any(|(_, body)| matches!(body, crate::program::Body::Modes { .. })) {
                    return unsupported("target-dependent cost: modal target choices");
                }
                let programs = crate::program::spell_programs(def, &[], crate::program::ModeCount::default())?;
                let slots: Vec<_> = programs.iter().flat_map(|(_, programs)| programs).flat_map(|p| p.targets()).collect();
                if slots.len() != 1 || slots[0].bounds != (1, 1) || slots[0].announced_mask != crate::announcement::ANY || slots[0].dependency.is_some() {
                    return unsupported("target-dependent cost: one unconditional required target");
                }
            }
            Ok(Modifier { subject, scope, changes, origin, ordinal })
        })();
        Some(Registered { ability: index, result })
    }).collect()
}

impl CardDef {
    pub(crate) fn cost_change_ok(&self, index: usize) -> Option<Res<()>> {
        self.cost_changes.iter().find(|r| r.ability == index).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }
}

impl Game {
    /// The announced spell is on the stack, with its chosen half, controller and X. Ask
    /// effective ability origins once; a removed or granted cost modifier uses that same view.
    pub(crate) fn modified_cast_cost(&self, spell: ObjId, cost: crate::cost::Plan) -> crate::cost::Plan {
        let mut increase = self.commander_tax(spell);
        let mut reduction = 0;
        let mut minimum = 0;
        let membership_changed = self.membership_changed();
        for source in self.battlefield.iter().copied().chain([spell]) {
            for (origin, def) in self.origins(source, crate::ability::View::Effective { membership_changed }) {
                for registration in &def.cost_changes {
                    let Ok(modifier) = &registration.result else { continue };
                    if !modifier.scope.contains(self.obj(source).zone) { continue; }
                    let ctx = Ctx { x: self.obj(source).x, ..Ctx::new(self.controller(source), source).with_key(AbilityKey { origin: origin.clone(), index: registration.ability }) };
                    if !modifier.subject.contains(self, &ctx, Target::Object(spell)) { continue; }
                    if modifier.origin.as_ref().is_some_and(|o| !o.holds(self.obj(spell).cast_from)) { continue; }
                    if modifier.ordinal.as_ref().is_some_and(|(n, pattern)|
                        self.happened(&ctx, pattern, crate::history::Turn::This, false) != (*n - 1) as usize) { continue; }
                    for (when, change, amount) in &modifier.changes {
                        if !when.holds(self, &ctx) { continue; }
                        let count = amount.evaluate(self, &ctx, &NoLocals);
                        match change { Change::Increase => increase += count, Change::Reduce => reduction += count, Change::Minimum => minimum = minimum.max(count) }
                    }
                }
            }
        }
        // harmonize: the power of the creature chosen to tap, as the total cost is
        // determined, reduces only its generic mana, after every increase (702.180b, 601.2f)
        if let Some(creature) = self.obj(spell).harmonized {
            reduction += u32::try_from(self.power(creature).unwrap_or(0)).unwrap_or(0);
        }
        cost.change_generic(increase, reduction).minimum(minimum)
    }
}
