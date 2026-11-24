//! Auras, Equipment and attachment (CR 301.5, 303.4, 701.3, 702.5–6). Enchant descriptions
//! constrain Aura targets and attachments; equip's own-creature restriction is targeting only.

use crate::card::CardDef;
use crate::announcement::TargetSpec;
use crate::program::expr::NoLocals;
use crate::select::{Ctx, Support};
use crate::{Game, ObjId, PlayerId, Res, Target, Zone, unsupported};
use ir::token::KeywordParam;
use ir::{Ability, Amount, CardType, Count, Filter, ObjectSet, Quant, Selector};

pub fn equipment(def: &CardDef) -> bool {
    def.is(CardType::Artifact) && def.subtypes.contains(&ir::Subtype::equipment())
}

pub fn aura(def: &CardDef) -> bool {
    def.is(CardType::Enchantment) && def.subtypes.contains(&ir::Subtype::aura())
}

/// All instances of enchant restrict the same target (702.5c), rather than adding targets.
fn target(def: &CardDef) -> Res<Option<Selector>> {
    if !aura(def) {
        return Ok(None);
    }
    let mut set: Option<ObjectSet> = None;
    for ability in &def.abilities {
        let Ability::Keywords(items) = ability else { continue };
        for item in items {
            let KeywordParam::Enchant(next) = &item.param else { continue };
            match &mut set {
                None => set = Some(next.clone()),
                Some(set) => {
                    if set.domain != next.domain
                        || set.zone != next.zone
                        || set.controller.is_some() && next.controller.is_some() && set.controller != next.controller
                    {
                        return unsupported("enchant: intersecting different domains, zones or controllers");
                    }
                    set.filter = Filter::All(vec![set.filter.clone(), next.filter.clone()]);
                    set.controller = set.controller.clone().or_else(|| next.controller.clone());
                    if next.owner.is_some() || next.caster.is_some() {
                        return unsupported("enchant: owner or caster");
                    }
                }
            }
        }
    }
    let Some(mut set) = set else { return unsupported("an Aura without an enchant ability") };
    // an Aura cannot enchant itself (303.4d); "another" of players would be "other than
    // you", but a Curse can enchant its controller
    if !set.possibilities().iter().any(|p| p.domain.sort == ir::Sort::Player) {
        set.other = true;
    }
    Ok(Some(Selector { quant: Quant::Target(Count::Exactly(Amount::Const(1))), set, chooser: None, random: None }))
}

/// What this definition's Aura enchants, lowered once as the definition is loaded: the
/// target group of the Aura spell (303.4a), whose set is also what it may be attached to
/// (303.4c). Its text is not read again.
pub(crate) fn compile(def: &CardDef) -> Res<Option<TargetSpec>> {
    target(def)?.map(|selector| TargetSpec::compile(selector, &Support::definition(def, None))).transpose()
}

/// The lowered enchant description of an Aura; None for anything else, or for one refused.
pub(crate) fn enchants(def: &CardDef) -> Option<&TargetSpec> {
    def.enchant.as_ref().ok()?.as_ref()
}

impl Game {
    /// Existential inverse relation in the supplied partial/frozen characteristic world.
    /// State-based actions, rather than this query, own illegal detachment.
    pub(crate) fn has_attachment(&self, ctx: &Ctx<'_>, recipient: Target, kind: ir::selector::Attachment) -> bool {
        if let Target::Object(id) = recipient && !ctx.on_battlefield(self, id) {
            // Like characteristics, a specific departed incarnation retains its last facts.
            // Frozen trigger lookback takes precedence over this later resolution history.
            return self.obj(id).zone == Zone::Gone && match kind {
                ir::selector::Attachment::Equipped => self.obj(id).last_attachments[0],
                ir::selector::Attachment::Enchanted => self.obj(id).last_attachments[1],
                _ => false,
            };
        }
        if let Target::Player(p) = recipient && self.players[p].lost { return false; }
        let frozen = ctx.view.and_then(crate::layers::Working::attachments);
        ctx.permanents(self).iter().any(|source| {
            let edge = match frozen {
                Some(edges) => edges.iter().find(|(id, _)| id == source).map(|(_, to)| *to),
                None => self.obj(*source).attached_to,
            };
            if edge != Some(recipient) { return false; }
            let seen = ctx.seen_for(self, *source, crate::layers::Aspect::Types);
            match kind {
                ir::selector::Attachment::Equipped => seen.is(CardType::Artifact) && seen.subtypes.contains(&ir::Subtype::equipment()),
                ir::selector::Attachment::Enchanted => seen.is(CardType::Enchantment) && seen.subtypes.contains(&ir::Subtype::aura()),
                _ => false,
            }
        })
    }

    pub(crate) fn inverse_attachments(&self, ctx: &Ctx<'_>, recipient: ObjId) -> [bool; 2] {
        use ir::selector::Attachment::{Equipped, Enchanted};
        [self.has_attachment(ctx, Target::Object(recipient), Equipped),
         self.has_attachment(ctx, Target::Object(recipient), Enchanted)]
    }

    pub(crate) fn aura(&self, object: ObjId) -> bool {
        let seen = self.seen_for(object, crate::layers::Aspect::Types);
        seen.is(CardType::Enchantment) && seen.subtypes.contains(&ir::Subtype::aura())
    }

    /// The object or player attached to, including an ability's source as it last existed
    /// (301.5f, 303.4m, 608.2h). A reference to an object that left its zone names nothing now.
    pub fn attached_to(&self, source: ObjId) -> Option<Target> {
        self.obj(source).attached_to.filter(|t| match *t {
            Target::Object(o) => self.obj(o).zone != Zone::Gone,
            Target::Player(p) => !self.players[p].lost,
        })
    }

    pub(crate) fn can_enchant(&self, aura: ObjId, controller: PlayerId, to: Target) -> bool {
        if to == Target::Object(aura) || self.is_a(aura, CardType::Creature)
            || self.protected_from(to, &Ctx::new(controller, aura)) {
            return false;
        }
        self.enchant_spec(aura)
            .is_some_and(|s| s.set.contains(self, &Ctx::new(controller, aura), &NoLocals, to))
    }

    pub(crate) fn enchant_choices(&self, aura: ObjId, controller: PlayerId) -> Vec<Target> {
        self.enchant_spec(aura)
            .map(|s| {
                s.set.members(self, &Ctx::new(controller, aura), &NoLocals)
                    .into_iter()
                    .filter(|t| self.can_enchant(aura, controller, *t))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Ongoing attachment legality (301.5, 303.4), distinct from equip targeting: an
    /// Equipment can remain on another player's creature and ignores hexproof and shroud.
    pub(crate) fn can_attach(&self, source: ObjId, to: Target) -> bool {
        let seen = self.seen_for(source, crate::layers::Aspect::Types);
        if self.protected_from(to, &Ctx::new(self.controller(source), source)) { return false; }
        if seen.is(CardType::Creature) || seen.is(CardType::Battle) || to == Target::Object(source) {
            return false;
        }
        let aura = seen.is(CardType::Enchantment) && seen.subtypes.contains(&ir::Subtype::aura());
        let equipment = seen.is(CardType::Artifact) && seen.subtypes.contains(&ir::Subtype::equipment());
        (aura || equipment)
            && (!aura || self.can_enchant(source, self.controller(source), to))
            && (!equipment || matches!(to, Target::Object(o) if self.obj(o).zone == Zone::Battlefield && self.is_a(o, CardType::Creature)))
    }

    /// Attach an Aura or Equipment already on the battlefield (701.3). An illegal attachment or the
    /// attachment it already has does nothing (701.3b). A new one gives a new timestamp.
    pub fn attach(&mut self, source: ObjId, to: Target) -> bool {
        if self.obj(source).zone != Zone::Battlefield
            || !self.can_attach(source, to)
            || self.attached_to(source) == Some(to)
        {
            return false;
        }
        let stamp = self.tick();
        self.obj_mut(source).attached_to = Some(to);
        self.obj_mut(source).stamp = stamp;
        self.log.push(crate::Event::Attached { aura: source, to });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enchanted_player_and_frozen_source_world_are_read_without_mutation() {
        let defs=vec![CardDef::new("Curse","{0}","Enchantment — Aura","Enchant player",None).unwrap()];
        let mut game=Game::new(defs,2,20);let aura=game.put(0,0,Zone::Battlefield);assert!(game.attach(aura,Target::Player(1)));
        let ctx=Ctx::new(0,aura);
        // CR303.4b/e: a player can be enchanted by another player's Aura.
        assert!(game.has_attachment(&ctx,Target::Player(1),ir::selector::Attachment::Enchanted));
        assert!(!game.has_attachment(&ctx,Target::Player(0),ir::selector::Attachment::Enchanted));
        let mut types=game.seen_for(aura,crate::layers::Aspect::Types);types.subtypes.clear();
        let world=crate::layers::Working::before(vec![(aura,types)]).freeze_attachments(vec![(aura,Target::Player(1))]);
        let mut partial=ctx.clone();partial.view=Some(&world);
        // The supplied effective source types override the live source's printed subtype.
        assert!(!game.has_attachment(&partial,Target::Player(1),ir::selector::Attachment::Enchanted));
        assert_eq!(game.attached_to(aura),Some(Target::Player(1)));
    }
}
