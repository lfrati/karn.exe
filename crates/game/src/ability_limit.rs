//! Per-turn uses of one ability instance (603.2h), independent of casting permissions.
use crate::{binding::AbilityKey, select::Ctx, state::{Game, ObjId, PlayerId}, Res, unsupported};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Limit { Trigger, Action }

impl Limit {
    pub(crate) fn of(effect: &ir::Effect) -> Res<Option<Self>> {
        let limits: Vec<_> = effect.0.iter().filter_map(|s| match s {
            ir::Sentence::Rider(ir::effect::Rider::TriggersOnlyOnceEachTurn) => Some(Self::Trigger),
            ir::Sentence::Rider(ir::effect::Rider::DoThisOnlyOnceEachTurn) => Some(Self::Action),
            _ => None,
        }).collect();
        match limits.as_slice() { [] => Ok(None), [one] => Ok(Some(*one)), _ => unsupported("ability: competing turn limits") }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Use { source: Option<ObjId>, ability: AbilityKey, actor: Option<PlayerId> }
impl Use {
    /// A rules-created ability with no source (702.179d, 725.2) is each player's own.
    pub(crate) fn of(ctx: &Ctx, limit: Limit) -> Option<Self> {
        let ability = ctx.ability.clone()?;
        if ctx.source.is_none() && !matches!(ability.origin, crate::binding::Origin::Rule(_)) { return None; }
        Some(Self { source: ctx.source, ability, actor: (limit == Limit::Action || ctx.source.is_none()).then_some(ctx.controller) })
    }
}
impl Game {
    pub(crate) fn ability_used(&self, ctx: &Ctx, limit: Limit) -> bool {
        Use::of(ctx, limit).is_some_and(|used| self.ability_uses.contains(&used))
    }
    pub(crate) fn use_ability(&mut self, ctx: &Ctx, limit: Limit) {
        if let Some(used) = Use::of(ctx, limit) && !self.ability_uses.contains(&used) { self.ability_uses.push(used); }
    }
}
