//! Where an object's ability functions (CR 113.6).
//!
//! The abilities of a permanent function while it is on the battlefield (113.6). An ability
//! whose cost or effect moves the object it is on out of a zone functions only in that zone
//! (113.6m): "Whenever an opponent is dealt damage by a red instant or sorcery spell you
//! control …, return this card from your graveyard to your hand." triggers while the card is
//! in its owner's graveyard, and not while it is on the battlefield. The engine asks this where
//! abilities trigger ([`crate::trigger`]) and where they are activated ([`crate::activate`]).

use crate::card::CardDef;
use crate::state::{Game, ObjId, Zone};
use crate::{Res, unsupported};
use ir::action::Action;
use ir::{Ability, CostPart, Effect, Objects, Quant, Ref, Selector, Sentence};

/// A static ability stating that its own object can't be countered functions on the stack
/// (113.6g), including on a permanent card. This classification is shared by execution
/// validation, spell instructions, and the restrictions that are in force.
/// Is this, of an instant or sorcery, instructions followed as it resolves (608.2c), rather
/// than a keyword, a cost, where it may be cast from, or what it has on the stack?
pub(crate) fn spell_instructions(a: &Ability) -> bool {
    !matches!(a, Ability::Keywords(_)) && !stack_static(a) && !crate::cost_change::owns(a) && casting_cost(a).is_none()
        && optional_cost(a).is_none() && alternative_cost(a).is_none() && casting_zone(a).is_none()
}

pub(crate) fn stack_static(a: &Ability) -> bool {
    let Some(s) = crate::statement::read(a) else { return false };
    if !matches!(s.subject, ir::Subject::Selector(selector) if selector.quant == Quant::Definite(Ref::This))
        || s.acts.is_empty() { return false; }
    // A self-grant of a casting modifier functions only on the stack (113.6e).
    s.acts.iter().any(|(action, _, _)| matches!(action, Action::Gains(granted)
        if granted.iter().any(|g| matches!(g, ir::action::Granted::Keyword(k) if k.keyword.name() == "Flash"))))
        || s.acts.iter().all(|(action, _, scale)| {
            *scale == crate::statement::Scale::Once
                && matches!(action, Action::Cant(items) if !items.is_empty() && items.iter().all(|item|
                    item.within.is_none() && matches!(&item.event, ir::EventAction::Deed(d)
                        if d.verb == ir::Verb::Counter && d.passive && d.args.is_empty())))
        })
}

/// An unconditional mandatory additional casting cost, functioning on the stack
/// (113.6d). Optional costs and declarations embedded in other instructions remain refused.
pub(crate) fn casting_cost(a: &Ability) -> Option<&ir::Cost> {
    let Ability::Text(Effect(rows)) = a else { return None };
    match rows.as_slice() {
        [Sentence::AdditionalCost { optional: false, cost }] => Some(cost),
        _ => None,
    }
}

/// "As an additional cost to cast this spell, you may blight 1.": an optional additional cost
/// (118.8b), announced as the spell is cast (601.2b) and functioning on the stack (113.6d).
pub(crate) fn optional_cost(a: &Ability) -> Option<&ir::Cost> {
    let Ability::Text(Effect(rows)) = a else { return None };
    match rows.as_slice() {
        [Sentence::AdditionalCost { optional: true, cost }] => Some(cost),
        _ => None,
    }
}

/// "You may sacrifice two Mountains rather than pay this spell's mana cost.": an alternative
/// cost of the card itself, optional and possibly conditional (118.9, 118.9b), functioning on the
/// stack as the spell is cast (113.6d): casting metadata, not an instruction of the spell.
/// Those of other spells and required ones remain refused.
pub(crate) fn alternative_cost(a: &Ability) -> Option<(ir::Cost, Option<ir::Condition>)> {
    let Ability::Text(Effect(rows)) = a else { return None };
    let [sentence] = rows.as_slice() else { return None };
    let mut sentence = sentence;
    let mut conditions = Vec::new();
    while let Sentence::Led { lead: ir::Lead::Modifier(ir::effect::Modifier::If(condition)), rest } = sentence {
        conditions.push(condition.clone());
        sentence = rest;
    }
    let cost = match sentence {
        Sentence::Static(rule) => {
            let ir::statics::Static::AlternativeCost { cond, cost, of: None, optional: true } = rule.as_ref() else { return None };
            if cost.conj != ir::Conj::And && cost.items.len() > 1 { return None; }
            conditions.extend(cond.iter().cloned());
            ir::Cost(cost.items.iter().cloned().map(CostPart::Action).collect())
        }
        // A printed permission to cast this very spell without paying its mana cost is
        // an empty alternative cost (118.9), not an instruction to cast itself on resolution.
        Sentence::Do(body) => {
            let c = &body.first;
            let ir::Subject::Selector(actor) = c.subject.as_ref()? else { return None };
            if !crate::select::is_you(actor) || !c.may || c.also || !body.rest.is_empty()
                || !c.rest.is_empty() || c.first.may || !c.first.mods.is_empty() { return None; }
            let Action::Cast { play: false, what, free: true } = &c.first.action else { return None };
            let [card] = what.items.as_slice() else { return None };
            // In "If this spell is the first ... you may cast it", the
            // condition explicitly supplies the sole self antecedent (201.5).
            let self_reference = card.quant == Quant::Definite(Ref::This)
                || card.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It))
                    && matches!(conditions.as_slice(), [ir::Condition::NthCastThisGame { what, .. }]
                        if what.quant == Quant::Definite(Ref::This));
            if !self_reference || card.chooser.is_some() || card.random.is_some()
                || !matches!(card.set.domain, ir::Domain::Object | ir::Domain::Permanent | ir::Domain::Spell | ir::Domain::Card)
                || card.set != ir::ObjectSet::of(card.set.domain.clone()) { return None; }
            ir::Cost(Vec::new())
        }
        _ => return None,
    };
    let condition = match conditions.len() { 0 => None, 1 => conditions.pop(), _ => Some(ir::Condition::And(conditions)) };
    Some((cost, condition))
}

/// A card's unconditional permission to cast itself from its owner's graveyard
/// (113.6f, 601.3). This is a static permission, not an optional instruction at resolution.
/// Other actors, zones, conditions, free casting and playing lands remain unsupported.
pub(crate) fn casting_zone(a: &Ability) -> Option<Zone> {
    let Ability::Text(Effect(rows)) = a else { return None };
    let [Sentence::Do(body)] = rows.as_slice() else { return None };
    let clause = &body.first;
    let ir::Subject::Selector(actor) = clause.subject.as_ref()? else { return None };
    if !crate::select::is_you(actor) || !clause.may || clause.also
        || !body.rest.is_empty() || !clause.rest.is_empty()
        || clause.first.may || !clause.first.mods.is_empty() { return None; }
    let Action::Cast { play: false, what, free: false } = &clause.first.action else { return None };
    let [card] = what.items.as_slice() else { return None };
    if card.set.domain != ir::Domain::Card || card.set.filter != ir::Filter::any_object()
        || card.set.controller.is_some() || card.set.owner.is_some() || card.set.caster.is_some()
        || card.set.other || card.chooser.is_some() || card.random.is_some() { return None; }
    out_of(card).ok().flatten()
}

/// The functioning scope of an ability (113.6). Zone permissions function everywhere,
/// while most abilities function in one zone. This does not itself authorize casting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    In(Zone),
    Everywhere,
}
impl Scope {
    pub fn contains(self, zone: Zone) -> bool {
        matches!(self, Scope::Everywhere) || self == Scope::In(zone)
    }
}

/// The zone of the object's own the phrase moves it out of: "this card from your graveyard".
pub(crate) fn out_of(s: &Selector) -> Res<Option<Zone>> {
    if s.quant != Quant::Definite(Ref::This) {
        return Ok(None);
    }
    let Some(zone) = &s.set.zone else { return Ok(None) };
    let yours = zone.owner.as_deref().is_some_and(crate::select::is_you);
    match zone.zones.items.as_slice() {
        // a graveyard is its owner's (400.3): the card's own is "your graveyard"
        [ir::Zone::Graveyard] if yours && zone.slice.is_none() => Ok(Some(Zone::Graveyard)),
        [ir::Zone::Hand] if yours && zone.slice.is_none() => Ok(Some(Zone::Hand)),
        _ => unsupported("an ability that moves its own card out of a zone other than its owner's hand or graveyard (113.6m)"),
    }
}

/// The zones the instructions move their own object out of.
fn moved_out(actions: &[&Action]) -> Res<Vec<Zone>> {
    let mut out = Vec::new();
    let objects = |o: &Objects, out: &mut Vec<Zone>| -> Res<()> {
        for s in &o.items {
            out.extend(out_of(s)?);
        }
        Ok(())
    };
    for a in actions {
        match a {
            Action::Move { moves, .. } => moves.iter().try_for_each(|(o, _)| objects(o, &mut out))?,
            Action::Discard(o) if o.items.iter().any(|s| s.quant == Quant::Definite(Ref::This)) => { objects(o, &mut out)?; out.push(Zone::Hand); },
            Action::Exile(o) | Action::Destroy(o) | Action::Sacrifice(o) | Action::Discard(o) | Action::Reveal(o) => objects(o, &mut out)?,
            _ => {}
        }
    }
    Ok(out)
}

/// Every action of a text, in the order written.
fn actions(effect: &Effect) -> Vec<&Action> {
    fn sentence<'a>(s: &'a Sentence, out: &mut Vec<&'a Action>) {
        match s {
            Sentence::Do(b) => {
                for c in std::iter::once(&b.first).chain(b.rest.iter().map(|(_, c)| c)) {
                    out.extend(std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)).map(|a| &a.action));
                }
            }
            Sentence::Led { rest, .. } => sentence(rest, out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    effect.0.iter().for_each(|s| sentence(s, &mut out));
    out
}

/// A positive condition that explicitly confines this source to exile (113.6b/k).
/// Other subjects and disjunctions without that same constraint prove no source zone.
/// `it_is_this`: the condition is of an event whose only subject is this object, so "it" there
/// is this object ("whenever a time counter is removed from this card while it's exiled").
fn requires_source_exiled(condition: &ir::Condition, it_is_this: bool) -> bool {
    match condition {
        ir::Condition::Is { what, pred, negated: false } if what.quant == Quant::Definite(Ref::This)
            || it_is_this && what.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) => match pred {
            ir::condition::Predicate::Designated(ir::condition::ObjectDesignation::Suspended) => true,
            ir::condition::Predicate::In(zone) => zone.zones.items == [ir::Zone::Exile] && zone.owner.is_none() && zone.slice.is_none(),
            _ => false,
        },
        ir::Condition::And(conditions) => conditions.iter().any(|c| requires_source_exiled(c, it_is_this)),
        ir::Condition::Or(conditions) => !conditions.is_empty() && conditions.iter().all(|c| requires_source_exiled(c, it_is_this)),
        _ => false,
    }
}
fn exile_only(ability: &Ability) -> bool {
    // "it" in a trigger condition is the event's grammatical subject (as `input::find` binds it)
    fn of_this(pattern: &ir::EventPattern) -> bool {
        matches!(pattern.subjects.as_slice(), [subject] if subject.quant == Quant::Definite(Ref::This))
    }
    fn event_condition(trigger: &ir::Trigger) -> bool {
        match trigger {
            ir::Trigger::Event { pattern, .. } => pattern.condition.as_deref().is_some_and(|c| requires_source_exiled(c, of_this(pattern))),
            ir::Trigger::Any(triggers) => !triggers.is_empty() && triggers.iter().all(event_condition),
            _ => false,
        }
    }
    match ability {
        Ability::Activated { effect, .. } => effect.0.iter().any(|sentence| matches!(sentence,
            Sentence::Restrictions(restrictions) if restrictions.iter().any(|restriction|
                matches!(restriction, ir::Restriction::If(condition) if requires_source_exiled(condition, false))))),
        Ability::Triggered { trigger, effect } => event_condition(trigger) || matches!(effect.0.first(),
            Some(Sentence::Led { lead: ir::Lead::Modifier(ir::Modifier::If(condition)), .. }) if requires_source_exiled(condition, false)),
        _ => false,
    }
}

/// The zone a triggered or activated ability of a permanent card functions in (113.6): the
/// battlefield, or the zone its cost or effect moves the object out of (113.6m), including its owner's hand or graveyard. Of an ability whose trigger condition puts the object into that zone
/// ("When this creature dies, return this card from your graveyard …") the exception of
/// 113.6m is not followed yet, and the ability is refused. Read as a definition is loaded:
/// a game asks [`CardDef::scope`].
pub fn scope(a: &Ability, modes: &[ir::Mode]) -> Res<Scope> {
    if matches!(a, Ability::Keywords(_)) {
        return unsupported("keyword rows use their declared contribution scopes (113.6)");
    }
    if exile_only(a) { return Ok(Scope::In(Zone::Exile)); }
    // A source cannot be cast from the battlefield. Its own cast trigger functions
    // on the stack (113.6k), regardless of its permanent card type.
    if let Ability::Triggered { trigger: ir::Trigger::Event { pattern, .. }, .. } = a {
        let self_cast = |action: &ir::EventAction| match action {
            ir::EventAction::Deed(d) if d.verb == ir::Verb::Cast => {
                d.args.iter().any(|(role, arg)| *role == ir::Role::What
                        && matches!(arg, ir::event::Arg::Noun(s) if s.quant == Quant::Definite(Ref::This)))
            },
            _ => false,
        };
        if pattern.actions.iter().any(self_cast) {
            if !pattern.actions.iter().all(self_cast) { return unsupported("trigger conditions functioning in different zones (113.6k)"); }
            return Ok(Scope::In(Zone::Stack));
        }
    }
    if casting_zone(a).is_some() || crate::program::registration::defining_pt(a).is_some() { return Ok(Scope::Everywhere); }
    if stack_static(a) || casting_cost(a).is_some() || optional_cost(a).is_some() || crate::cost_change::self_modifier(a) { return Ok(Scope::In(Zone::Stack)); }
    let (cost, effect) = match a {
        Ability::Activated { cost, effect } => (Some(cost), effect),
        Ability::Triggered { effect, .. } | Ability::Chapter { effect, .. } => (None, effect),
        _ => return Ok(Scope::In(Zone::Battlefield)),
    };
    let mut acts: Vec<&Action> = cost.into_iter().flat_map(|c| &c.0).filter_map(|p| if let CostPart::Action(a) = p { Some(a) } else { None }).collect();
    acts.extend(actions(effect));
    for m in modes {
        if let ir::Mode::Bullet(e) = m {
            acts.extend(actions(e));
        }
    }
    let zones = moved_out(&acts)?;
    match zones.as_slice() {
        [] => Ok(Scope::In(Zone::Battlefield)),
        [z, rest @ ..] if rest.iter().all(|r| r == z) => {
            if let Ability::Triggered { trigger, .. } = a
                && puts_there(trigger)
            {
                return unsupported("an ability whose trigger condition puts its object into the zone it moves it out of (113.6m)");
            }
            Ok(Scope::In(*z))
        }
        _ => unsupported("an ability that moves its own object out of two zones"),
    }
}

/// Where each ability of a definition functions (113.6) and what its static abilities say of
/// casting it, read once when the definition is loaded. Nothing of a game is in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Functioning {
    /// Of an instant or sorcery, each paragraph followed as it resolves, by its index: refused
    /// when it is an ability other than the spell's instructions.
    spell_bodies: Vec<(usize, Res<()>)>,
    /// One for each ability in the order of [`Game::abilities`]: printed rows, then the
    /// executable meanings of keywords. Ordinary abilities have one scope; keyword rows
    /// have one per declared contribution, never an executable union of their zones.
    scopes: Vec<Res<Vec<Scope>>>,
    /// The zones other than a hand its own text lets it be cast from (113.6f, 601.3).
    cast_from: Vec<Zone>,
    /// Its unconditional mandatory additional casting costs (113.6d), each as written with
    /// the index of its ability. This is the text a payment is lowered from, not a payment.
    casting_costs: Vec<(usize, ir::Cost)>,
    /// Its printed alternative costs (118.9), each by the row that states it.
    alternative_costs: Vec<(usize, (ir::Cost, Option<ir::Condition>))>,
}

impl Functioning {
    /// The one place the bodies of a definition's abilities are read for where they function.
    pub(crate) fn compile(def: &CardDef) -> Functioning {
        let abilities = || def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a));
        let mut generated_scopes = crate::keyword::contributions(def).filter_map(|(scope, gives)|
            matches!(gives, crate::keyword::Gives::Ability(_)).then_some(scope));
        let mut cast_from = Vec::new();
        for zone in abilities().filter_map(casting_zone) {
            if !cast_from.contains(&zone) { cast_from.push(zone); }
        }
        // Spree's and Tiered's row is the spell's choice of the modes printed under it
        // (702.172a, 702.183a)
        let modal = crate::keyword::modal_row(def).map(|(row, _)| row);
        let spell_bodies = if def.is(ir::CardType::Instant) || def.is(ir::CardType::Sorcery) {
            // the gift's effect before any other of the spell's abilities (702.174j)
            crate::keyword::gift_ability(def).map(|i| (i, Ok(()))).into_iter().chain(def.abilities.iter().enumerate().filter(|(i, a)| Some(*i) == modal || spell_instructions(a))
                // a paragraph that replaces the one before it is that paragraph's last sentence
                .filter(|(i, a)| !(*i > 0 && matches!(def.abilities[i - 1], Ability::Text(_)) && crate::program::replaces_previous(a).is_some()))
                // a paragraph after another is followed as that one's later sentences (608.2c)
                .filter(|(i, _)| !crate::program::continues_previous(def, *i))
                .map(|(i, a)| (i, if matches!(a, Ability::Text(_)) || Some(i) == modal { Ok(()) } else { unsupported(format!("ability on a spell: {}", crate::select::name(a))) }))).collect()
        } else { Vec::new() };
        Functioning {
            spell_bodies,
            scopes: abilities().enumerate().map(|(i, a)| {
                if i >= def.abilities.len() {
                    Ok(vec![generated_scopes.next().expect("one scope per generated executable")])
                } else if matches!(a, Ability::Keywords(_)) {
                    def.keyword_declarations.iter().filter(|instance| instance.row == i).try_fold(Vec::new(), |mut scopes, instance| {
                        scopes.extend(instance.declared.as_ref().map_err(Clone::clone)?.gives.iter().map(|(scope, _)| *scope));
                        Ok(scopes)
                    })
                } else { scope(a, def.modes_of(i)).map(|scope| vec![scope]) }
            }).collect(),
            cast_from,
            casting_costs: def.abilities.iter().enumerate().filter_map(|(i, a)| Some((i, casting_cost(a)?.clone()))).collect(),
            alternative_costs: def.abilities.iter().enumerate().filter_map(|(i, a)| Some((i, alternative_cost(a)?)))
                .chain(crate::keyword::alternative_costs(def).map(|(i, cost, when)| (i, (cost.clone(), when.cloned())))).collect(),
        }
    }
}


impl CardDef {
    pub(crate) fn spell_abilities(&self) -> Res<Vec<usize>> { self.functioning.spell_bodies.iter().map(|(i, r)| r.clone().map(|()| *i)).collect() }
    /// Each paragraph followed as the spell resolves, by its index, and whether it is one
    /// of the spell's instructions: what [`Self::spell_abilities`] is, of each on its own.
    pub(crate) fn spell_paragraphs(&self) -> &[(usize, Res<()>)] { &self.functioning.spell_bodies }
    /// The single scope of an executable ability (113.6). A keyword row with different
    /// contribution scopes has no single executable scope; its consumers read declarations.
    /// None: this definition has no such ability.
    pub(crate) fn scope(&self, index: usize) -> Option<Res<Scope>> {
        self.functioning.scopes.get(index).map(|scopes| {
            let scopes = scopes.as_ref().map_err(Clone::clone)?;
            match scopes.as_slice() {
                [first, rest @ ..] if rest.iter().all(|s| s == first) => Ok(*first),
                _ => unsupported("keyword row has separate contribution scopes (113.6)"),
            }
        })
    }

    /// Validate all individual scopes without treating their union as an executable scope.
    pub(crate) fn scope_ok(&self, index: usize) -> Option<Res<()>> {
        self.functioning.scopes.get(index).map(|scopes| scopes.as_ref().map(|_| ()).map_err(Clone::clone))
    }

    /// Does its own text let it be cast from this zone (113.6f, 601.3)? A hand needs no such
    /// text and is not asked here.
    pub(crate) fn cast_from(&self, zone: Zone) -> bool {
        self.functioning.cast_from.contains(&zone)
    }

    /// Its unconditional mandatory additional casting costs (113.6d) as written, each with
    /// the index of the ability stating it.
    pub(crate) fn casting_costs(&self) -> impl Iterator<Item = (usize, &ir::Cost)> {
        self.functioning.casting_costs.iter().map(|(i, cost)| (*i, cost))
    }

    /// Its printed alternative costs (118.9), each by the row that states it.
    pub(crate) fn alternative_costs(&self) -> impl Iterator<Item = (usize, &ir::Cost, Option<&ir::Condition>)> {
        self.functioning.alternative_costs.iter().map(|(i, (cost, cond))| (*i, cost, cond.as_ref()))
    }
}

impl Game {
    /// Where the ability of this object at `index` among [`Game::abilities`] functions, when
    /// the engine follows where it does: what the definition it belongs to keeps.
    #[cfg(test)]
    pub(crate) fn scope(&self, o: ObjId, index: usize) -> Option<Scope> {
        self.scope_of(o, self.key(o, index).ok()?)
    }

    /// Where this ability of this object functions, when the engine follows where it does:
    /// what the definition it belongs to keeps, its own or the one it was given.
    pub(crate) fn scope_of(&self, o: ObjId, key: crate::binding::AbilityKey) -> Option<Scope> {
        let scope = self.definition(o, &key.origin)?.scope(key.index)?.ok()?;
        // An emblem's own abilities function in the command zone (114.4).
        Some(if self.obj(o).emblem.is_some() { Scope::In(Zone::Command) } else { scope })
    }

    /// [`Game::functions_as`], of the ability at `index` among [`Game::abilities`].
    pub(crate) fn functions(&self, o: ObjId, index: usize) -> bool {
        self.key(o, index).is_ok_and(|key| self.functions_as(o, key))
    }

    /// Does this ability of this object function where the object is now (113.6)? An object
    /// that has just left the battlefield functions there still for what looks back at that
    /// event (603.10a).
    pub(crate) fn functions_as(&self, o: ObjId, key: crate::binding::AbilityKey) -> bool {
        let obj = self.obj(o);
        let here = if obj.zone == Zone::Gone { obj.left } else { Some(obj.zone) };
        here.is_some_and(|zone| self.scope_of(o, key).is_some_and(|scope| scope.contains(zone)))
    }
}

/// Does the trigger condition say its object is put into a graveyard: "when this creature
/// dies", "when this card is put into a graveyard"?
fn puts_there(t: &ir::Trigger) -> bool {
    match t {
        ir::Trigger::Event { pattern, .. } => {
            let about_itself = pattern.subjects.iter().any(|s| s.quant == Quant::Definite(Ref::This));
            about_itself
                && pattern.actions.iter().any(|a| {
                    matches!(a, ir::EventAction::PutIntoGraveyard { .. }) || matches!(a.reading(), Some((ir::Verb::Die, _)))
                })
        }
        ir::Trigger::Any(v) => v.iter().any(puts_there),
        ir::Trigger::Step { .. } | ir::Trigger::State(_) => false,
    }
}

#[cfg(test)]
mod tests;
