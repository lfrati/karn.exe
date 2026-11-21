//! Effective static casting routes (601.3, 611.3), including library-top registrations.
//! Usage belongs to one ability of one incarnation; failed announcements do not consume it.
use crate::binding::AbilityKey;
use crate::program::expr::{self, Filter};
use crate::select::{Ctx, Support};
use crate::{CardDef, Game, ObjId, PlayerId, Res, Target, unsupported};
use ir::{Ability, Action, Domain, Quant, Sentence};
#[derive(Debug, Clone)]
pub(crate) struct Registration {
    pub ability: usize,
    pub result: Res<Permission>,
}
#[derive(Debug, Clone)]
pub(crate) struct Permission {
    filter: Filter,
    yours: bool,
}
/// Announcement uses a player-visible effective index; the payment retains the exact key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Use {
    pub source: ObjId,
    pub key: AbilityKey,
}
fn clause(a: &Ability) -> Option<(bool, &ir::effect::Clause)> {
    let Ability::Text(ir::Effect(rows)) = a else { return None };
    let [Sentence::Led { lead: ir::Lead::Once { yours }, rest }] = rows.as_slice() else { return None };
    let Sentence::Do(body) = rest.as_ref() else { return None };
    if !body.rest.is_empty() {
        return None;
    }
    Some((*yours, &body.first))
}
pub(crate) fn registrations(def: &CardDef) -> Vec<Registration> {
    def.abilities
        .iter()
        .enumerate()
        .filter_map(|(ability, a)| {
            clause(a).map(|(yours, c)| Registration { ability, result: compile(def, ability, yours, c) })
        })
        .collect()
}
fn compile(def: &CardDef, index: usize, yours: bool, c: &ir::effect::Clause) -> Res<Permission> {
    if !matches!(&c.subject,Some(ir::Subject::Selector(s)) if crate::select::is_you(s))
        || !c.may
        || c.also
        || !c.rest.is_empty()
        || c.first.may
        || !c.first.mods.is_empty()
    {
        return unsupported("limited casting permission: a single optional cast by its controller");
    }
    let Action::Cast { play: false, what, free: false } = &c.first.action else {
        return unsupported("limited casting permission: ordinary casting costs");
    };
    let [s] = what.items.as_slice() else {
        return unsupported("limited casting permission: one described spell");
    };
    if !matches!(s.quant, Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))))
        || s.set.domain != Domain::Spell
        || s.chooser.is_some()
        || s.random.is_some()
    {
        return unsupported("limited casting permission: a spell description");
    }
    let mut set = s.set.clone();
    let zone = set.zone.take();
    if !zone.is_some_and(|z| {
        z.zones.items == [ir::Zone::Graveyard]
            && z.owner.as_deref().is_some_and(crate::select::is_you)
            && z.slice.is_none()
    }) {
        return unsupported("limited casting permission: your graveyard");
    }
    let filter = std::mem::replace(&mut set.filter, ir::Filter::any_object());
    if set != ir::ObjectSet::of(Domain::Spell) {
        return unsupported("limited casting permission: spell qualifiers");
    }
    let support = Support::definition(def, Some(AbilityKey::printed(index)));
    Ok(Permission { filter: Filter::compile(&filter, &support, &mut expr::linked(&support))?, yours })
}
impl Game {
    /// Candidate routes are discovered before X and modes are announced. Their spell
    /// filter is checked by castable with the actual announcement, not with X guessed zero.
    pub(crate) fn casting_sources(
        &self,
        player: PlayerId,
        card: ObjId,
        cast_as: crate::card::CastAs,
    ) -> Vec<(usize, Use)> {
        // An effect's in-resolution cast uses that effect's permission (608.2g), not
        // a static zone permission with ordinary timing. Validating an explicit route
        // uses this query without consuming its turn allowance.
        if self.offered(player, card)
            || cast_as == crate::card::CastAs::Fused
        {
            return Vec::new();
        }
        let mut out = self.top_cast_sources(player,card);
        if !self.players[player].graveyard.contains(&card){return out;}
        for source in &self.battlefield {
            if self.controller(*source) != player {
                continue;
            }
            for (index, key) in self.held(*source).into_iter().enumerate() {
                let Some(def) = self.definition(*source, &key.origin) else { continue };
                let Some(p) =
                    def.cast_permissions.iter().find(|r| r.ability == key.index).and_then(|r| r.result.as_ref().ok())
                else {
                    continue;
                };
                let usage = Use { source: *source, key: key.clone() };
                if p.yours && self.active != player || self.cast_permission_uses.contains(&usage) {
                    continue;
                }
                out.push((index, usage));
            }
        }
        out
    }
    pub(crate) fn casting_permissions(
        &self,
        player: PlayerId,
        card: ObjId,
        cast_as: crate::card::CastAs,
        x: Option<u32>,
    ) -> Vec<(usize, Use)> {
        self.casting_sources(player, card, cast_as)
            .into_iter()
            .filter(|(_, usage)| {
                if self.library_top(player)==Some(card){return self.top_cast_matches(player,card,cast_as,x,usage.clone());}
                let def = self.definition(usage.source, &usage.key.origin).expect("effective definition");
                let p = def
                    .cast_permissions
                    .iter()
                    .find(|r| r.ability == usage.key.index)
                    .unwrap()
                    .result
                    .as_ref()
                    .unwrap();
                let ctx =
                    Ctx { casting: Some((card, cast_as)), x, ..Ctx::new(player, usage.source).with_key(usage.key.clone()) };
                p.filter.matches(self, &ctx, &expr::NoLocals, Target::Object(card))
            })
            .collect()
    }
    pub(crate) fn casting_permission(
        &self,
        player: PlayerId,
        card: ObjId,
        cast_as: crate::card::CastAs,
        x: Option<u32>,
        source: ObjId,
        ability: usize,
    ) -> Option<Use> {
        self.casting_permissions(player, card, cast_as, x)
            .into_iter()
            .find(|(i, u)| *i == ability && u.source == source)
            .map(|(_, u)| u)
    }
}
