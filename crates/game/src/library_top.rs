//! Effective top-library permissions (401.5, 611.3). Announcements retain access boundaries.
use crate::binding::AbilityKey;
use crate::program::expr::{self, Filter};
use crate::select::{Ctx, Support};
use crate::{CardDef, Game, ObjId, PlayerId, Res, Target, unsupported};
use ir::{Ability, Action, Amount, Domain, Quant, Sentence};
#[derive(Debug, Clone)]
pub(crate) struct Registration {
    pub ability: usize,
    pub result: Res<Grant>,
}
#[derive(Debug, Clone)]
pub(crate) enum Grant {
    Look,
    Play { lands: bool, spell: Option<Filter>, life: bool },
}
fn own_top(z: &ir::selector::ZoneSpec) -> bool {
    z.zones.items == [ir::Zone::Library]
        && z.owner.as_deref().is_some_and(crate::select::is_you)
        && z.slice == Some(ir::selector::Slice::Top(Amount::Const(1)))
}
fn clause(a: &Ability) -> Option<(&ir::effect::Clause, &[Sentence])> {
    let Ability::Text(ir::Effect(rows)) = a else { return None };
    let (Sentence::Do(body), rest) = rows.split_first()? else { return None };
    if !body.rest.is_empty() {
        return None;
    }
    Some((&body.first, rest))
}
/// Discover the family even when its qualifiers are unsupported; admission must not omit them.
pub(crate) fn owns(a: &Ability) -> bool {
    clause(a).is_some_and(|(c,_)|matches!(&c.first.action,Action::Static(s) if match s.as_ref(){ir::StaticAct::LookAnyTime(_)=>true,ir::StaticAct::PlayFrom{from:Some(z),..}=>z.zones.items.contains(&ir::Zone::Library),_=>false}))
}
fn is_it(s: &ir::Selector) -> bool {
    s == &ir::Selector::new(
        Quant::Definite(ir::Ref::Anaphor(ir::selector::Anaphor::It)),
        ir::ObjectSet::of(Domain::Object),
    )
}
fn life_rider(rows: &[Sentence]) -> bool {
    let [
        Sentence::Led {
            lead:
                ir::Lead::Modifier(ir::effect::Modifier::If(ir::Condition::Happened {
                    pattern,
                    window: Some(ir::condition::Window::ThisWay),
                    times: None,
                    tense: ir::event::Tense::Past,
                    negated: false,
                })),
            rest,
        },
    ] = rows
    else {
        return false;
    };
    if pattern.subjects.len() != 1
        || !crate::select::is_you(&pattern.subjects[0])
        || pattern.nth_each_turn.is_some()
        || pattern.during.is_some()
        || pattern.condition.is_some()
        || pattern.cause.is_some()
        || pattern.except_nth_in_step.is_some()
    {
        return false;
    }
    let [ir::EventAction::Deed(d)] = pattern.actions.as_slice() else { return false };
    let [(ir::Role::What, ir::event::Arg::Noun(s))] = d.args.as_slice() else { return false };
    if d.verb != ir::Verb::Cast
        || d.passive
        || s.quant != Quant::Indefinite(ir::Count::Exactly(Amount::Const(1)))
        || s.set != ir::ObjectSet::of(Domain::Spell)
        || s.chooser.is_some()
        || s.random.is_some()
    {
        return false;
    }
    let Sentence::Static(st) = rest.as_ref() else { return false };
    let ir::statics::Static::AlternativeCost { cond: None, cost, of: Some(ir::statics::Who::One(of)), optional: false } =
        st.as_ref()
    else {
        return false;
    };
    is_it(of)
        && cost.conj == ir::Conj::And
        && matches!(cost.items.as_slice(),[Action::PayLife(Amount::Stat{of,stat:ir::selector::Stat::ManaValue})] if is_it(of))
}
fn compile(def: &CardDef, index: usize, a: &Ability) -> Res<Grant> {
    let (c, rows) = clause(a).unwrap();
    if !matches!(&c.subject,Some(ir::Subject::Selector(s)) if crate::select::is_you(s))
        || !c.may
        || c.also
        || !c.rest.is_empty()
        || c.first.may
        || !c.first.mods.is_empty()
    {
        return unsupported("library permission: one optional controller permission");
    }
    let Action::Static(s) = &c.first.action else { unreachable!() };
    match s.as_ref() {
        ir::StaticAct::LookAnyTime(ir::statics::Who::One(s))
            if rows.is_empty() && s.quant == Quant::The && s.chooser.is_none() && s.random.is_none() =>
        {
            let mut set = s.set.clone();
            let z = set.zone.take();
            if set == ir::ObjectSet::of(Domain::Card) && z.as_ref().is_some_and(own_top) {
                Ok(Grant::Look)
            } else {
                unsupported("library look: your single top card")
            }
        }
        ir::StaticAct::PlayFrom { what, from: Some(z) } if own_top(z) => {
            let life = if rows.is_empty() {
                false
            } else if life_rider(rows) {
                true
            } else {
                return unsupported("library play: unsupported permission cost rider");
            };
            let support = Support::definition(def, Some(AbilityKey::printed(index)));
            let (mut lands, mut spell) = (false, None);
            for p in what {
                let mut set = p.what.clone();
                let filter = std::mem::replace(&mut set.filter, ir::Filter::any_object());
                if p.play
                    && set == ir::ObjectSet::of(Domain::Permanent)
                    && filter == ir::Filter::All(vec![ir::Filter::Atom(ir::selector::Atom::Type(ir::CardType::Land))])
                {
                    lands = true;
                } else if !p.play && set == ir::ObjectSet::of(Domain::Spell) && spell.is_none() {
                    spell = Some(Filter::compile(&filter, &support, &mut expr::linked(&support))?);
                } else {
                    return unsupported("library play: lands and described spells only");
                }
            }
            if what.is_empty() || life && spell.is_none() {
                return unsupported("library play: empty permission");
            }
            Ok(Grant::Play { lands, spell, life })
        }
        _ => unsupported("library permission: only your single top card"),
    }
}
pub(crate) fn registrations(def: &CardDef) -> Vec<Registration> {
    def.abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| owns(a))
        .map(|(ability, a)| Registration { ability, result: compile(def, ability, a) })
        .collect()
}
impl Game {
    fn top_grants(&self, player: PlayerId) -> Vec<(usize, crate::cast_permission::Use, Grant)> {
        let mut out = Vec::new();
        for source in &self.battlefield {
            if self.controller(*source) != player {
                continue;
            }
            for (index, key) in self.held(*source).into_iter().enumerate() {
                if let Some(grant) = self
                    .definition(*source, &key.origin)
                    .and_then(|d| d.library_permissions.iter().find(|r| r.ability == key.index))
                    .and_then(|r| r.result.as_ref().ok())
                {
                    out.push((index, crate::cast_permission::Use { source: *source, key: key.clone() }, grant.clone()));
                }
            }
        }
        out
    }
    pub(crate) fn library_top(&self, player: PlayerId) -> Option<ObjId> {
        self.top_cards(player, 1).first().copied()
    }
    pub(crate) fn library_tops(&self) -> Vec<Option<ObjId>> {
        (0..self.players.len()).map(|p| self.library_top(p)).collect()
    }
    pub(crate) fn top_access_finished(&self, player: PlayerId, card: ObjId) -> bool {
        self.procedures.iter().all(|p| match p {
            crate::procedure::Procedure::Payment(p) => {
                p.library_tops.as_ref().is_none_or(|tops| tops[player] == Some(card))
            }
            crate::procedure::Procedure::Land { library_tops, .. } => library_tops[player] == Some(card),
            _ => true,
        })
    }
    pub(crate) fn continuous_top_look(&self, player: PlayerId, card: ObjId) -> bool {
        self.library_top(player) == Some(card)
            && self.top_access_finished(player, card)
            && self.top_grants(player).iter().any(|(_, _, g)| matches!(g, Grant::Look))
    }
    pub(crate) fn top_cast_sources(&self, player: PlayerId, card: ObjId) -> Vec<(usize, crate::cast_permission::Use)> {
        if self.library_top(player) != Some(card) {
            return Vec::new();
        }
        self.top_grants(player)
            .into_iter()
            .filter(|(_, _, g)| matches!(g, Grant::Play { spell: Some(_), .. }))
            .map(|(i, u, _)| (i, u))
            .collect()
    }
    pub(crate) fn top_cast_matches(
        &self,
        player: PlayerId,
        card: ObjId,
        choice: crate::card::CastAs,
        x: Option<u32>,
        usage: crate::cast_permission::Use,
    ) -> bool {
        self.top_grants(player).into_iter().any(|(_,u,g)|u==usage && matches!(g,Grant::Play{spell:Some(f),..} if f.matches(self,&Ctx{casting:Some((card,choice)),x,..Ctx::new(player,u.source).with_key(u.key)},&expr::NoLocals,Target::Object(card))))
    }
    pub(crate) fn top_life_cost(&self, route: crate::announcement::CastRoute) -> bool {
        let crate::announcement::CastRoute::Permission { source, ability } = route else { return false };
        self.top_grants(self.controller(source))
            .into_iter()
            .any(|(i, u, g)| i == ability && u.source == source && matches!(g, Grant::Play { life: true, .. }))
    }
    pub(crate) fn top_land_allowed(&self, player: PlayerId, card: ObjId) -> bool {
        self.library_top(player) == Some(card)
            && self.top_grants(player).iter().any(|(_, _, g)| matches!(g, Grant::Play { lands: true, .. }))
    }
}
