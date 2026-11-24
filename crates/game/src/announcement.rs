//! Announcement metadata produced by compilation, never by dry execution.
use crate::select::Ctx;
use crate::state::Game;
use crate::Res;
use ir::Selector;

/// The permission used to cast the chosen spell (601.3). A route is distinct from
/// its cost: retrace adds a discard cost; flashback and escape supply alternative costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CastRoute { #[default] Normal, Flashback, Retrace, Escape, Foretell, Permission { source: crate::ObjId, ability: usize },
    /// Its harmonize cost from a graveyard (702.180a), and the untapped creature its caster
    /// chose, if any, to tap as it is paid, its power taken from the generic mana (702.180b).
    Harmonize { tapped: Option<crate::ObjId> } }

/// Which cost a spell is cast for (601.2b, 601.2f): its mana cost, or an alternative cost
/// its card prints (118.9), named by the row of the ability that states it. At most one
/// alternative cost is paid (118.9a); flashback, the alternative cost of casting it from a
/// graveyard (702.34a), is selected by [`CastRoute::Flashback`], with an index when the
/// card has distinct flashback costs (113.2c). An additional cost is not this
/// choice (118.8), nor is which face or half is cast ([`crate::card::CastAs`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CastCost {
    #[default]
    Mana,
    Alternative(usize),
    /// One of the distinct effective flashback costs, in keyword order (113.2c).
    /// An unqualified flashback route is sufficient only when that cost is unique.
    Flashback(usize),
}

/// The announcements a spell's text can make a target depend on (601.2c), each a bit of a
/// mask: whether it was kicked (702.33g), whether its gift was promised (702.174m) and
/// whether its optional additional cost was paid, as bargain's (702.166d). Bit `kicked |
/// promised << 1 | optional paid << 2` stands
/// for that combination.
pub(crate) const ANY: u8 = 0b1111_1111;
/// The announcements in which it was kicked.
pub(crate) const KICKED: u8 = 0b1010_1010;
/// The announcements in which its gift was promised.
pub(crate) const PROMISED: u8 = 0b1100_1100;
/// The announcements in which its optional additional cost was paid.
pub(crate) const OPTIONAL_PAID: u8 = 0b1111_0000;

/// The bit of this announcement.
pub(crate) fn announced(kicks: u32, promised: bool, optional_paid: bool) -> u8 {
    1 << (u8::from(kicks > 0) | u8::from(promised) << 1 | u8::from(optional_paid) << 2)
}

/// A collective restriction on the members of one target instance (601.2c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRelation { SameGraveyard, DifferentCreatureControllers, DifferentManaValues }

/// One declared target group, including relationships to earlier groups (115.1, 601.2c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSpec {
    pub selector: Selector,
    pub set: crate::program::expr::Set,
    pub bounds: (u32, u32),
    pub relation: Option<TargetRelation>,
    /// The announcements ([`ANY`]) in which the group is chosen; in others it is empty.
    /// Empty groups preserve compiled target indices (702.33g, 702.174m).
    pub(crate) announced_mask: u8,
    dynamic_bounds: Option<(crate::program::expr::Amount, crate::program::expr::Amount)>,
    /// Each chosen object/player must differ from every choice in these earlier groups.
    pub distinct_from: Vec<usize>,
    pub(crate) dependency: Option<usize>,
    pub(crate) base: usize,
    /// The amount the text divides or distributes among the group's targets (601.2d).
    pub(crate) divided: Option<crate::program::expr::Amount>,
}
impl TargetSpec {
    pub(crate) fn compile(selector: Selector, support: &crate::select::Support<'_>) -> Res<Self> {
        Self::compile_with_x(selector, support, None)
    }
    pub(crate) fn compile_with_x(selector: Selector, support: &crate::select::Support<'_>, defined_x: Option<&ir::Amount>) -> Res<Self> {
        Self::compile_after(selector,support,defined_x,&[])
    }
    pub(crate) fn compile_after(selector:Selector,support:&crate::select::Support<'_>,defined_x:Option<&ir::Amount>,earlier:&[Self])->Res<Self> {
        Self::compile_after_with_attached_dealer(selector, support, defined_x, earlier, false, 0)
    }
    /// A damage recipient's "other" excludes its attached dealer, using the same set for
    /// announcement and resolution legality. No body-local reference is imported here.
    /// `sentence`: the first of `earlier` its own sentence declares.
    pub(crate) fn compile_after_with_attached_dealer(selector: Selector, support: &crate::select::Support<'_>, defined_x: Option<&ir::Amount>, earlier: &[Self], attached: bool, sentence: usize) -> Res<Self> {
        let ir::Quant::Target(count) = &selector.quant else { return crate::unsupported("expected a target group"); };
        let (bounds, dynamic_bounds) = match crate::select::constant_target_bounds(count) {
            Ok(bounds) => (bounds, None),
            Err(error) => {
                use crate::program::expr;
                let compile = |n: &ir::Amount| {
                    let n = if matches!(n, ir::Amount::X) { defined_x.unwrap_or(n) } else { n };
                    let amount = expr::Amount::compile(n, support, &mut expr::linked(support))?;
                    Ok(amount)
                };
                let (min,max) = match count {
                    ir::Count::UpTo(n) => (expr::Amount::constant(0), compile(n)?),
                    ir::Count::Exactly(n) => (compile(n)?, compile(n)?),
                    _ => return Err(error),
                };
                ((0,u32::MAX), Some((min,max)))
            }
        };
        // SameZone is a group predicate, never a unary object matcher. Only the
        // positive conjunct on cards in a graveyard is implemented here.
        fn extract(filter: &mut ir::Filter) -> bool {
            match filter {
                ir::Filter::Atom(ir::Atom::SameZone) => { *filter = ir::Filter::All(Vec::new()); true },
                ir::Filter::All(items) => items.iter_mut().fold(false, |found, item| extract(item) || found),
                _ => false,
            }
        }
        fn extract_mana_values(filter: &mut ir::Filter) -> bool {
            match filter {
                ir::Filter::Atom(ir::Atom::Different(ir::Characteristic::ManaValue)) => {
                    *filter = ir::Filter::All(Vec::new()); true
                },
                ir::Filter::All(items) => items.iter_mut().fold(false, |found, item| extract_mana_values(item) || found),
                _ => false,
            }
        }
        // "two target cards from an opponent's graveyard": one opponent's graveyard, all of
        // them (the Deadeye Tracker ruling), not one card from each of several opponents'
        fn one_opponents_graveyard(set: &ir::ObjectSet, bounds: (u32, u32)) -> bool {
            bounds.1 > 1 && set.zone.as_ref().is_some_and(|zone| zone.zones.items == [ir::Zone::Graveyard]
                && zone.owner.as_deref().is_some_and(|who| who.chooser.is_none() && who.random.is_none()
                    && matches!(who.quant, ir::Quant::Indefinite(ir::Count::Exactly(ir::Amount::Const(1))))
                    && crate::select::controlled_by_you(who) == Some(false)))
        }
        let mut unary = selector.set.clone();
        let different_mana_values = extract_mana_values(&mut unary.filter);
        let pair_filter = ir::Filter::All(vec![
            ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature)),
            ir::Filter::Atom(ir::Atom::Controllers { same: false }),
        ]);
        let relation = if unary.domain == ir::Domain::Permanent && unary.filter == pair_filter
            && bounds == (2, 2) && dynamic_bounds.is_none()
            && unary == (ir::ObjectSet { filter: pair_filter.clone(), ..ir::ObjectSet::of(ir::Domain::Permanent) })
        {
            unary.filter = ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))]);
            Some(TargetRelation::DifferentCreatureControllers)
        } else if different_mana_values {
            if extract(&mut unary.filter) || unary.domain != ir::Domain::Card || unary.zone.as_ref().is_none_or(|zone|
                zone.zones.items != [ir::Zone::Graveyard] || zone.slice.is_some()) {
                return crate::unsupported("different mana values: one explicit graveyard target group required");
            }
            Some(TargetRelation::DifferentManaValues)
        } else if extract(&mut unary.filter) || one_opponents_graveyard(&unary, bounds) {
            if unary.domain != ir::Domain::Card || unary.zone.as_ref().is_none_or(|zone|
                zone.zones.items != [ir::Zone::Graveyard] || zone.slice.is_some()) {
                return crate::unsupported("same zone target group: explicit graveyard cards required");
            }
            Some(TargetRelation::SameGraveyard)
        } else { None };
        let mut bind=TargetBinding{support,earlier,reads:Vec::new(),attached,sentence};
        let set=crate::program::expr::Set::compile(&unary,support,&mut bind)?;
        if relation.is_some() && !bind.reads.is_empty() {
            return crate::unsupported("same graveyard target group: dependent earlier targets not implemented");
        }
        if bind.reads.len()>1 {return crate::unsupported("target relation: more than one earlier group");}
        Ok(Self { selector,set,bounds,relation,dynamic_bounds,announced_mask:ANY,distinct_from:Vec::new(),dependency:bind.reads.first().copied(),base:0,divided:None })
    }
    /// Compare actual graveyard identities among otherwise legal members. An
    /// already-illegal retained target cannot anchor a new target (115.7d-e),
    /// nor invalidate surviving targets at resolution (608.2b).
    pub(crate) fn group_legal(&self, game: &Game, ctx: &Ctx, locals: &dyn crate::program::expr::Locals, group: &[crate::Target]) -> bool {
        if self.relation.is_none() { return true; }
        if self.relation == Some(TargetRelation::DifferentCreatureControllers) {
            // Read both exact incarnations before effects. An independently illegal
            // member still supplies current controller information, or departure LKI
            // (608.2b/h and Run Away Together rulings). Never follow a reentered card.
            return matches!(group, [crate::Target::Object(a), crate::Target::Object(b)]
                if a != b && game.controller(*a) != game.controller(*b));
        }
        if self.relation == Some(TargetRelation::DifferentManaValues) {
            let mut values = std::collections::BTreeSet::new();
            return group.iter().copied().filter(|target| self.set.legal_target(game, ctx, locals, *target))
                .all(|target| match target {
                    crate::Target::Object(object) => values.insert(ctx.mana_value(game, object)),
                    crate::Target::Player(_) => false,
                });
        }
        let mut owner = None;
        for target in group.iter().copied().filter(|t| self.set.legal_target(game, ctx, locals, *t)) {
            let crate::Target::Object(object) = target else { return false };
            let object = game.obj(object);
            if object.zone != crate::Zone::Graveyard { return false; }
            if owner.is_some_and(|previous| previous != object.owner) { return false; }
            owner = Some(object.owner);
        }
        true
    }
    pub(crate) fn legal_target(&self, game: &Game, ctx: &Ctx, locals: &dyn crate::program::expr::Locals, group: &[crate::Target], target: crate::Target) -> bool {
        self.set.legal_target(game, ctx, locals, target) && self.group_legal(game, ctx, locals, group)
    }
    /// The group as this announcement leaves it: none chosen where its text does not apply.
    pub(crate) fn announce(&mut self, kicks: u32, promised: bool, optional_paid: bool) {
        if self.announced_mask & announced(kicks, promised, optional_paid) == 0 {
            self.bounds = (0, 0);
            self.dynamic_bounds = None;
        }
    }
    /// Evaluate during announcement, before costs can change the quantity's source. Of a
    /// divided group, each target gets at least one of the amount (601.2d), so there are at
    /// most that many.
    pub(crate) fn limits(&self, game: &Game, ctx: &Ctx) -> (u32,u32) {
        let (min, max) = match &self.dynamic_bounds {
            Some((min,max)) => (min.evaluate(game,ctx,&crate::program::expr::NoLocals), max.evaluate(game,ctx,&crate::program::expr::NoLocals)),
            None => self.bounds,
        };
        match self.divided_total(game, ctx) { Some(total) => (min, max.min(total)), None => (min, max) }
    }
    /// The amount divided among this group's targets, as announced (601.2d).
    pub(crate) fn divided_total(&self, game: &Game, ctx: &Ctx) -> Option<u32> {
        self.divided.as_ref().map(|n| n.evaluate(game, ctx, &crate::program::expr::NoLocals))
    }
}
/// Target discovery for separate spell abilities uses local indices; combine them once.
pub(crate) fn append_targets(into: &mut Vec<TargetSpec>, more: Vec<TargetSpec>) {
    let offset = into.len();
    into.extend(more.into_iter().map(|mut slot| {
        slot.base += offset;
        slot.distinct_from.iter_mut().for_each(|i| *i += offset);
        slot
    }));
}

pub fn target_slots(game: &Game, ctx: Ctx<'static>, modes: &[usize]) -> Res<Vec<TargetSpec>> {
    let mut slots = Vec::new();
    for program in crate::program::selected(game, ctx, modes)? { append_targets(&mut slots, program.targets.clone()); }
    Ok(slots)
}

/// Activation timing is compiled metadata, not an instruction followed during resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Limit { Sorcery, Lifetime, Once, Times(u32), If(crate::program::condition::Condition) }
impl Limit {
    pub(crate) fn compile(r: &ir::Restriction, support: &crate::select::Support<'_>) -> Res<Self> {
        match r {
            ir::Restriction::Once => Ok(Self::Lifetime),
            ir::Restriction::Sorcery => Ok(Self::Sorcery), ir::Restriction::OnceEachTurn => Ok(Self::Once),
            ir::Restriction::TimesEachTurn(ir::Amount::Const(n)) => Ok(Self::Times(*n)),
            ir::Restriction::If(c) => Ok(Self::If(crate::program::condition::Condition::compile(c, support, &mut crate::program::expr::linked(support))?)),
            // "Activate only during your turn": a condition on whose turn it is (602.5)
            ir::Restriction::During(ir::event::During::Turn(whose) | ir::event::During::NotTurn(whose)) => {
                let negated = matches!(r, ir::Restriction::During(ir::event::During::NotTurn(_)));
                let turn = ir::Condition::Turn { whose: whose.clone(), negated };
                Ok(Self::If(crate::program::condition::Condition::compile(&turn, support, &mut crate::program::expr::linked(support))?))
            }
            other => crate::unsupported(format!("activate only: {}", crate::select::name(other))),
        }
    }
}
/// A self cost adjustment belongs to announcement, not to the resolving body (602.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CostChange { pub more: bool, pub amount: crate::program::expr::Amount }
pub(crate) fn activation_change(sentence: &ir::Sentence, support: &crate::select::Support<'_>) -> Option<Res<CostChange>> {
    use ir::{Action, Subject, Quant, Ref, Domain};
    use crate::statement::{Scale, When};
    let ability = ir::Ability::Text(ir::Effect(vec![sentence.clone()]));
    let stated = crate::statement::read(&ability)?;
    let Subject::Selector(s) = stated.subject else { return None };
    if s.quant != Quant::Definite(Ref::This) || s.set.domain != Domain::Ability { return None; }
    let [(Action::Static(action), When::Always, scale)] = stated.acts.as_slice() else { return None };
    let ir::StaticAct::CostChange { more, by, of: ir::CostOf::Activate } = action.as_ref() else { return None };
    Some((|| {
        if !matches!(support.ability(), Some(ir::Ability::Activated { .. })) { return crate::unsupported("self activation cost adjustment outside an activated ability"); }
        let mut n = 0u32;
        for symbol in &by.0 {
            let ir::mana::ManaSymbol::Generic(value) = symbol else { return crate::unsupported("activation cost adjustment: non-generic mana"); };
            n = n.saturating_add(*value);
        }
        let mut amount = crate::program::expr::Amount::constant(n);
        match scale {
            Scale::Once => {},
            Scale::ForEach(each) => amount = amount.times(crate::program::expr::Amount::compile(each, support, &mut crate::program::expr::linked(support))?),
            _ => return crate::unsupported("activation cost adjustment: undefined amount"),
        }
        Ok(CostChange { more: *more, amount })
    })())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    pub(crate) adds_mana: bool,
    pub(crate) library_movement: bool,
    pub(crate) limits: Vec<Limit>,
    pub(crate) cost_changes: Vec<CostChange>,
}
impl Facts {
    pub(crate) fn compile(effect: &ir::Effect, support: &crate::select::Support<'_>) -> Res<Self> {
        use ir::{Action, Sentence, Destination};
        // "at the beginning of the next turn's upkeep", "at end of combat": a delayed
        // triggered ability does it (603.7), not this one
        fn later(m: &ir::effect::Modifier) -> bool { matches!(m, ir::effect::Modifier::At { .. } | ir::effect::Modifier::AtEndOfCombat) }
        fn acts<'a>(sentence: &'a Sentence, out: &mut Vec<&'a ir::effect::Act>) {
            match sentence {
                Sentence::Do(b) => for clause in std::iter::once(&b.first).chain(b.rest.iter().map(|(_, c)| c)) {
                    out.extend(std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, a)| a)).filter(|a| !a.mods.iter().any(later)));
                },
                // Registering a delayed ability does not perform that child's instructions now:
                // Barbed Sextant's "Draw a card at the beginning of the next turn's upkeep"
                // moves no card of a library as its mana ability resolves (605.1a, 603.7a).
                Sentence::Led { lead: ir::Lead::When { .. }, .. } => {},
                Sentence::Led { lead: ir::Lead::Modifier(m), .. } if later(m) => {},
                Sentence::Led { rest, .. } => acts(rest, out), _ => {},
            }
        }
        let mut all = Vec::new();
        for sentence in &effect.0 { acts(sentence, &mut all); }
        let adds_mana = all.iter().any(|a| matches!(a.action, Action::AddMana(_) | Action::AddAdditionalMana(_)));
        let in_library = |objects: &ir::action::Objects| objects.items.iter().any(|s| s.set.zone.as_ref().is_some_and(|z| z.zones.items.contains(&ir::Zone::Library)));
        let library_movement = all.iter().any(|a| match &a.action {
            Action::Draw(_) | Action::Mill(_) => true,
            Action::Search { zone, .. } => zone.zones.items.contains(&ir::Zone::Library),
            Action::Move { moves, .. } => moves.iter().any(|(objects, to)| in_library(objects) || matches!(to, Destination::Library(_))),
            Action::Exile(o) | Action::Destroy(o) | Action::Discard(o) | Action::Sacrifice(o) | Action::Reveal(o) | Action::Tap(o) | Action::Untap(o) => in_library(o),
            _ => false,
        });

        let limits = effect.0.iter().filter_map(|s| if let Sentence::Restrictions(r) = s { Some(r) } else { None }).flatten().map(|limit| Limit::compile(limit, support)).collect::<Res<_>>()?;
        let cost_changes = effect.0.iter().filter_map(|s| activation_change(s, support)).collect::<Res<_>>()?;
        Ok(Self { adds_mana, library_movement, limits, cost_changes })
    }
}

/// Read already announced groups without confusing them with body-local result slots.
pub(crate) struct TargetValues<'a> {
    pub parent: &'a dyn crate::program::expr::Locals,
    pub chosen: &'a [Vec<crate::Target>],
    pub base: usize,
}
impl crate::program::expr::Locals for TargetValues<'_> {
    fn successor(&self, was: crate::ObjId) -> Option<crate::ObjId> { self.parent.successor(was) }
    fn local(&self,n:usize)->Option<&crate::binding::Value>{self.parent.local(n)}
    fn input(&self,k:crate::program::input::Input)->Option<&[crate::Target]>{self.parent.input(k)}
    fn event_amount(&self)->Option<u32>{self.parent.event_amount()}
    fn actor(&self)->Option<crate::state::PlayerId>{self.parent.actor()}
    fn targets(&self,n:usize)->Option<&[crate::Target]>{self.chosen.get(self.base+n).map(Vec::as_slice)}
}
struct TargetBinding<'a,'s>{support:&'a crate::select::Support<'s>,earlier:&'a [TargetSpec],reads:Vec<usize>,attached:bool,sentence:usize}
impl crate::program::expr::Binding for TargetBinding<'_,'_> {
    fn other_subject(&self) -> Option<crate::program::expr::Subject> {
        self.attached.then_some(crate::program::expr::Subject::Attached)
    }
    fn scalar(&mut self,r:&ir::reference::UnresolvedRef)->Res<crate::binding::Scalar>{
        crate::program::input::Binding(self.support).scalar(r)
    }
    fn subject(&mut self,s:&Selector)->Res<crate::program::expr::Subject>{
        use ir::{Quant,Ref,Anaphor};
        // Match a stated noun against each alternative of an earlier target declaration.
        fn named(reference:&ir::ObjectSet,prior:&ir::ObjectSet)->bool {
            if let ir::Domain::Union(parts)=&prior.domain {return parts.iter().any(|p|named(reference,p));}
            if reference.domain!=prior.domain {return false;}
            match (&reference.filter,&prior.filter) {
                (ir::Filter::All(wanted),ir::Filter::All(said))=>wanted.iter().all(|p|said.contains(p)),
                (a,b)=>a==b,
            }
        }
        if matches!(s.quant,Quant::Definite(Ref::Anaphor(Anaphor::That|Anaphor::It|Anaphor::The{plural:false}))) {
            let mut candidates:Vec<_>=self.earlier.iter().enumerate().filter(|(_,p)|named(&s.set,&p.selector.set)).map(|(i,_)|i).collect();
            // of several, the one its own sentence declares: "Target player gains 1 life. This
            // spell deals 2 damage to target opponent … that player controls."
            if candidates.len()>1 && let [one]=candidates.iter().copied().filter(|i|*i>=self.sentence).collect::<Vec<_>>().as_slice() {candidates=vec![*one];}
            match candidates.as_slice() {
                [one]=>{
                    if self.earlier[*one].bounds!=(1,1) || self.earlier[*one].dependency.is_some(){return crate::unsupported("target relation: needs one independent earlier target");}
                    if !self.reads.contains(one){self.reads.push(*one);}
                    return Ok(crate::program::expr::Subject::Targets(*one));
                },
                []=>{},
                _=>return crate::unsupported("target relation: ambiguous earlier targets"),
            }
        }
        // "target player shuffles any number of target cards from their graveyard": "their" is
        // the player that target names, chosen before these (601.2c)
        if s.quant==Quant::Definite(Ref::Anaphor(Anaphor::They)) && s.set.domain==ir::Domain::Player {
            let candidates:Vec<_>=self.earlier.iter().enumerate().filter(|(_,p)|p.selector.set.domain==ir::Domain::Player && named(&s.set,&p.selector.set)).map(|(i,_)|i).collect();
            match candidates.as_slice() {
                [one]=>{
                    if self.earlier[*one].bounds!=(1,1) || self.earlier[*one].dependency.is_some(){return crate::unsupported("target relation: needs one independent earlier target");}
                    if !self.reads.contains(one){self.reads.push(*one);}
                    return Ok(crate::program::expr::Subject::Targets(*one));
                },
                []=>{},
                _=>return crate::unsupported("target relation: ambiguous earlier targets"),
            }
        }
        crate::program::input::Binding(self.support).subject(s)
    }
}

#[cfg(test)]
mod tests {
    use crate::{CardDef,Game,Move,Target,Zone};
    #[test]
    fn same_zone_is_not_a_unary_disjunction_negation_or_other_zone_predicate() {
        let d = CardDef::new("Fixture", "{0}", "Instant", "", None).unwrap();
        let support = crate::select::Support::definition(&d, None);
        for (zone, filter, reason) in [
            (ir::Zone::Hand, ir::Filter::Atom(ir::Atom::SameZone), "same zone target group: explicit graveyard cards required"),
            (ir::Zone::Graveyard, ir::Filter::Not(Box::new(ir::Filter::Atom(ir::Atom::SameZone))), "description: SameZone"),
            (ir::Zone::Graveyard, ir::Filter::Any(vec![ir::Filter::Atom(ir::Atom::SameZone), ir::Filter::All(Vec::new())]), "description: SameZone"),
        ] {
            let mut set = ir::ObjectSet::of(ir::Domain::Card);
            set.zone = Some(ir::ZoneSpec::of(zone, Some(ir::Selector::new(
                ir::Quant::Each, ir::ObjectSet::of(ir::Domain::Player)))));
            set.filter = filter;
            let selector = ir::Selector::new(ir::Quant::Target(ir::Count::UpTo(ir::Amount::Const(2))), set);
            assert_eq!(super::TargetSpec::compile(selector, &support).unwrap_err().0, reason);
        }
        // The zone/owner premise itself is supported; only the negative group shapes fail.
        let mut set = ir::ObjectSet::of(ir::Domain::Card);
        set.zone = Some(ir::ZoneSpec::of(ir::Zone::Graveyard, Some(ir::Selector::new(
            ir::Quant::Each, ir::ObjectSet::of(ir::Domain::Player)))));
        set.filter = ir::Filter::Atom(ir::Atom::SameZone);
        let selector = ir::Selector::new(ir::Quant::Target(ir::Count::UpTo(ir::Amount::Const(2))), set);
        assert_eq!(super::TargetSpec::compile(selector, &support).unwrap().relation,
            Some(super::TargetRelation::SameGraveyard));
    }
    #[test]
    fn r608_2b_a_control_change_invalidates_the_dependent_target() {
        let spell=CardDef::new("Directed","{0}","Instant","This spell deals 2 damage to target player and 2 damage to target creature that player controls.",None).unwrap();
        crate::play::check(&spell).unwrap();
        let creature=CardDef::new("Bear","{0}","Creature — Bear","",Some((3,3))).unwrap();
        let mut g=Game::new(vec![spell,creature],3,20);
        let creature=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)],vec![Target::Object(creature)]]}).unwrap();
        g.obj_mut(creature).controller=2;
        for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
        assert_eq!(g.players[1].life,18);assert_eq!(g.obj(creature).damage,0);
    }
}
