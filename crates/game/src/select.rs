//! Noun phrases against the game: which objects and players a set description names, and what
//! number an amount is. Anything not evaluated yet is [`Unsupported`], never guessed: it is
//! refused where the description is lowered ([`crate::program::expr`]), which is the one
//! evaluator of descriptions and amounts.

use crate::event::Target;
use crate::program::expr::{self, NoLocals};
use crate::state::{Game, ObjId, PlayerId};
use crate::{Res, unsupported};
use ir::{Amount, Atom, Count, Domain, Filter, ObjectSet, Quant, Ref};

/// Whose text is being followed: "you" and "~".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx<'a> {
    /// A borrowed layer or pre-event characteristic view; never retained by an occurrence.
    pub view: Option<&'a crate::layers::Working>,
    pub controller: PlayerId,
    pub ability: Option<crate::binding::AbilityKey>,
    /// Captured relationship generation, separate from the stable activation-use key.
    pub link_generation: Option<usize>,
    /// The printed effect being executed, within the selected definition. Separate from
    /// linked-choice provenance: a delayed effect can retain links without being that
    /// original printed program.
    pub effect: Option<usize>,
    /// None for a source-free rules-created occurrence (725.2).
    pub source: Option<ObjId>,
    /// An object that has left the battlefield, described as it last was there: what a "dies"
    /// or "leaves the battlefield" trigger looks back at (603.10a).
    pub was: Option<ObjId>,
    /// Some: what permanents are is seen as the layers before this one have left it, which
    /// is how a continuous effect of that layer sees what it applies to (613.1). None: as
    /// they are now.
    pub layer: Option<crate::layers::Layer>,
    /// What X is, where the text says: "…, where X is the number of creatures you control"
    /// (107.3c).
    pub x: Option<u32>,
    /// Characteristics of a split spell being announced before it moves (709.3a).
    pub casting: Option<(ObjId, crate::card::CastAs)>,
    /// Whether these targets belong to a spell (115.1a–d). An ability can have a spell
    /// as its source, so the source's zone does not answer this question.
    pub targeting_spell: bool,
    /// A permanent asked whether it is face down as it was when an event happened, and whether
    /// it was: what an event turned face up (708.8); a face-down creature that entered this
    /// turn, though it is face up since (608.2i). Everything else is asked of it as it is.
    pub face_then: Option<(ObjId, bool)>,
    /// An object asked about as it was when an event happened, kept with the event: what
    /// entered this turn as it entered, what attacked as it was declared (608.2i). Not the
    /// object's last-known information, which [`Ctx::was`] asks for.
    pub then: Option<(ObjId, &'a crate::layers::Seen)>,
}

impl<'a> Ctx<'a> {
    fn spell<'g>(&self, game: &'g Game, o: ObjId) -> Option<&'g crate::card::CardDef> {
        let (card, choice) = self.casting?;
        (card == o).then(|| game.def(o)?.spell(choice).ok()).flatten()
    }

    /// The object's name: of a spell being cast, the name of the half or face cast (709.3a,
    /// 601.3e); otherwise its name, a split card's both names joined by " // " (709.4).
    pub(crate) fn name_of<'g>(&self, game: &'g Game, o: ObjId) -> &'g str {
        self.spell(game, o).map_or(game.obj(o).chars.name.as_str(), |d| d.name.as_str())
    }

    pub(crate) fn seen(&self, game: &Game, o: ObjId) -> crate::layers::Seen {
        self.seen_for(game, o, crate::layers::Aspect::All)
    }

    /// [`Ctx::seen`], as far as this question reads. A spell being cast is as the face
    /// chosen for it, and a view this context carries (a partial layer world, what was
    /// before an event) answers by itself: only the game as it is now is asked for less.
    pub(crate) fn seen_for(&self, game: &Game, o: ObjId, aspect: crate::layers::Aspect) -> crate::layers::Seen {
        if let Some((then, seen)) = self.then && then == o { return seen.clone(); }
        match self.spell(game, o) {
            Some(d) => {
                let mut seen = crate::layers::Seen {
                    controller: self.controller,
                    types: d.types.clone(), subtypes: d.subtypes.clone(), supertypes: d.supertypes.clone(),
                    colors: d.colors.clone(), pt: d.pt, keywords: d.keywords.clone(), base_abilities: true, intrinsic_abilities: true, given: Vec::new(), inherited: Vec::new(),
                };
                let before = aspect.within(self.layer);
                if before.is_none_or(|before| before > crate::layers::Layer::Types) {
                    for statement in d.characteristics.iter().filter_map(|r| r.result.as_ref().ok()) {
                        for types in statement.defined_types() { types.apply(&mut seen); }
                    }
                    // cast bestowed, it is an Aura spell (702.103b, 702.103d)
                    if game.obj(o).bestowed { crate::bestow::types().apply(&mut seen); }
                }
                // Color definitions work in every zone too (604.3, 702.114a): a devoid card
                // being cast is a colorless spell for what its mana may be spent on (106.6).
                if before.is_none_or(|before| before > crate::layers::Layer::Colors) {
                    for statement in d.characteristics.iter().filter_map(|r| r.result.as_ref().ok()) {
                        if let Some(colors) = statement.color_definition() { seen.colors = colors.to_vec(); }
                    }
                }
                seen
            },
            None => match self.view {
                Some(view) => {
                    let mut seen = view.get(o).cloned().unwrap_or_else(|| game.base_seen(o));
                    if view.battlefield().is_none() { game.finish_seen(o, &mut seen); }
                    seen
                },
                None => game.seen(o, aspect.within(self.layer)),
            },
        }
    }

    /// The battlefield used by this query: live for ordinary/layer queries, frozen for
    /// look-back trigger discovery. This view is never retained by an occurrence.
    pub(crate) fn permanents<'g>(&self, game: &'g Game) -> &'g [ObjId] where 'a: 'g {
        self.view.and_then(crate::layers::Working::battlefield).unwrap_or(&game.battlefield)
    }

    pub(crate) fn on_battlefield(&self, game: &Game, object: ObjId) -> bool {
        self.view.and_then(crate::layers::Working::battlefield).map_or_else(
            || game.obj(object).zone == crate::Zone::Battlefield, |permanents| permanents.contains(&object))
    }

    pub(crate) fn controlled_by(&self, game: &Game, object: ObjId) -> PlayerId {
        if let Some((then, seen)) = self.then && then == object { return seen.controller; }
        self.view.and_then(|view| view.get(object)).map_or_else(|| game.controller(object), |seen| seen.controller)
    }

    pub(crate) fn mana_value(&self, game: &Game, o: ObjId) -> u32 {
        self.spell(game, o).map_or(game.obj(o).chars.mana_value, |d| d.spell_mana_value(self.x))
    }

    pub fn for_spell(controller: PlayerId, source: ObjId) -> Ctx<'a> {
        Ctx { targeting_spell: true, ..Self::new(controller, source) }
    }

    /// Following the ability at this place of the source's own definition.
    pub fn with_ability(self, index: usize) -> Self {
        self.with_key(crate::binding::AbilityKey::printed(index))
    }

    /// Following this ability of the source, of whichever definition it belongs to.
    pub fn with_key(mut self, key: crate::binding::AbilityKey) -> Self {
        self.ability = Some(key.clone());
        self.effect = Some(key.index);
        self
    }

    /// The definition the ability followed belongs to: the source's own unless it says.
    pub(crate) fn origin(&self) -> crate::binding::Origin {
        self.ability.clone().map_or(crate::binding::Origin::Own, |key| key.origin)
    }

    /// Compiled links are relative to their definition; occurrences supply the acquisition.
    pub(crate) fn link(&self, mut key: crate::binding::AbilityKey) -> crate::binding::AbilityKey {
        if key.origin == crate::binding::Origin::Own { key.origin = self.origin(); }
        key
    }

    /// Object-bound costs and instructions require the source declared by their definition.
    pub(crate) fn object_source(&self) -> ObjId {
        self.source.expect("this instruction belongs to an object-bound definition")
    }

    pub(crate) fn rule(controller: PlayerId, key: crate::binding::AbilityKey) -> Ctx<'a> {
        Ctx { link_generation: None, view: None, controller, source: None, ability: Some(key.clone()), effect: Some(key.index), was: None, layer: None, x: None, casting: None, targeting_spell: false, face_then: None, then: None }
    }

    pub(crate) fn definition<'g>(&'g self, game: &'g Game) -> Option<&'g crate::CardDef> {
        match self.ability.as_ref().map(|key| &key.origin) {
            Some(crate::binding::Origin::Rule(what) | crate::binding::Origin::Given { what, .. }) => Some(what.def()),
            Some(origin) => self.source.and_then(|o| game.definition(o, origin)),
            None => self.source.and_then(|o| game.def(o)),
        }
    }

    pub fn new(controller: PlayerId, source: ObjId) -> Ctx<'a> {
        Ctx { link_generation: None, view: None, controller, source: Some(source), ability: None, effect: None, was: None, layer: None, x: None, casting: None, targeting_spell: false, face_then: None, then: None }
    }
}

/// The name of an enum value's variant, for saying what is not supported.
pub(crate) fn name<T: std::fmt::Debug>(x: &T) -> String {
    format!("{x:?}").chars().take_while(|c| c.is_alphanumeric()).collect()
}

pub(crate) fn is_you(who: &ir::Selector) -> bool {
    who.quant == Quant::Definite(Ref::You)
}

/// "an opponent": one player who is an opponent.
pub(crate) fn is_an_opponent(who: &ir::Selector) -> bool {
    matches!(&who.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))))
        && who.set.domain == Domain::Player
        && who.set.filter == Filter::All(vec![Filter::Atom(Atom::Opponent)])
}

/// "your opponents": every player who is an opponent.
fn is_your_opponents(who: &ir::Selector) -> bool {
    who.quant == Quant::All && who.set.domain == Domain::Player && who.set.filter == Filter::All(vec![Filter::Atom(Atom::Opponent)])
}

/// Whose control a description is about, when the engine can tell: "you control" (true),
/// or "an opponent controls" and "your opponents control" (false), which say the same of
/// one permanent, a permanent having one controller (110.2).
pub(crate) fn controlled_by_you(who: &ir::Selector) -> Option<bool> {
    if is_you(who) {
        Some(true)
    } else if is_an_opponent(who) || is_your_opponents(who) {
        Some(false)
    } else {
        None
    }
}

/// Definition-only knowledge required to lower operands. Runtime values and legal membership
/// are deliberately absent: empty candidate sets cannot hide unsupported descriptions.
/// Compilation and live queries lower through the same functions (601.2c, 608.2b).
#[derive(Clone)]
pub(crate) struct Support<'a> {
    // Definition-time dependencies resolved by the same binder as executable reads.
    linked_exile_writers: std::rc::Rc<std::cell::RefCell<Vec<usize>>>,
    /// How far lowering this definition's text has got, for what a refusal left unreached.
    lowering: std::rc::Rc<std::cell::Cell<crate::program::Lowering>>,
    def: Option<&'a crate::card::CardDef>,
    ability: Option<crate::binding::AbilityKey>,
    x_available: bool,
    /// The text is an enters-the-battlefield triggered ability or replacement effect of a
    /// permanent card with {X} in its mana cost: its X is that of the spell the permanent
    /// was (107.3m).
    spell_x: bool,
    /// The event a delayed triggered ability made by this text watches for, while that
    /// ability's own instructions are lowered: "it" there is of that event (603.7c).
    event: Option<&'a ir::EventPattern>,
}

/// "When this enters": a triggered ability of this permanent's own entry, triggering on
/// nothing else, which is what 107.3m calls an enters-the-battlefield triggered ability.
fn own_entry(trigger: &ir::Trigger) -> bool {
    let ir::Trigger::Event { pattern, .. } = trigger else { return false };
    matches!(&pattern.subjects[..], [s] if s.quant == Quant::Definite(Ref::This) && s.chooser.is_none() && s.random.is_none() && s.set.zone.is_none())
        && matches!(&pattern.actions[..], [ir::event::EventAction::Deed(d)] if d.verb == ir::Verb::Enter && !d.passive && d.args.is_empty())
        && pattern.nth_each_turn.is_none() && pattern.during.is_none() && pattern.cause.is_none() && pattern.except_nth_in_step.is_none()
}

/// A trigger of this exact spell being cast, with no other event source (107.3e).
fn own_cast(trigger: &ir::Trigger) -> bool {
    let ir::Trigger::Event { pattern, .. } = trigger else { return false };
    matches!(pattern.subjects.as_slice(), [player] if is_you(player))
        && matches!(pattern.actions.as_slice(), [ir::event::EventAction::Deed(d)]
            if d.verb == ir::Verb::Cast && !d.passive
            && matches!(d.args.as_slice(), [(ir::Role::What, ir::event::Arg::Noun(source))]
                if source.quant == Quant::Definite(Ref::This) && source.chooser.is_none() && source.random.is_none()
                    && matches!(source.set.domain, ir::Domain::Permanent | ir::Domain::Spell)
                    && source.set == ObjectSet::of(source.set.domain.clone())))
        && pattern.nth_each_turn.is_none() && pattern.during.is_none() && pattern.cause.is_none()
        && pattern.condition.is_none() && pattern.except_nth_in_step.is_none()
}

impl<'a> Support<'a> {
    pub(crate) fn definition(def: &'a crate::card::CardDef, ability: Option<crate::binding::AbilityKey>) -> Self {
        // An announced X is read by the text it was announced for: an instant's or sorcery's
        // own instructions, or the activated ability whose cost has {X} (107.3a).
        let x = |cost: &ir::mana::ManaCost| cost.0.contains(&crate::cost::X);
        let row = ability.clone().and_then(|key| def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(key.index));
        let x_available = match row {
            Some(ir::Ability::Activated { cost, .. }) => cost.0.iter().any(|part| part.mana().is_some_and(x) || matches!(part, ir::CostPart::Action(ir::Action::PayLife(ir::Amount::X)))
                || matches!(part, ir::CostPart::Action(ir::Action::Waterbend(cost)) if x(cost))),
            Some(ir::Ability::Text(_)) => def.spell_x(),
            _ => false,
        };
        let spell_x = matches!(row, Some(ir::Ability::Triggered { trigger, .. }) if own_entry(trigger)) && def.permanent_x()
            && ability.clone().is_some_and(|key| !matches!(key.origin, crate::binding::Origin::Given { .. } | crate::binding::Origin::Rule(_)));
        Self { linked_exile_writers: Default::default(), lowering: Default::default(), def: Some(def), ability, x_available, spell_x, event: None }
    }

    pub(crate) fn runtime(game: &'a Game, ctx: &'a Ctx) -> Self {
        let def = match ctx.casting.filter(|(source, _)| Some(*source) == ctx.source) {
            Some((source, choice)) => game.def(source).and_then(|d| d.spell(choice).ok()),
            None => ctx.definition(game),
        };
        let spell_x = def.is_some_and(|def| Self::definition(def, ctx.ability.clone()).spell_x);
        Self { linked_exile_writers: Default::default(), lowering: Default::default(), def, ability: ctx.ability.clone(), x_available: ctx.x.is_some(), spell_x, event: None }
    }

    /// The same definition, for the instructions of a delayed triggered ability that
    /// watches for this event.
    pub(crate) fn with_event(mut self, pattern: &'a ir::EventPattern) -> Self {
        self.event = Some(pattern);
        self
    }
    pub(crate) fn event(&self) -> Option<&'a ir::EventPattern> { self.event }

    /// The linked keyword instance whose trigger offers this exact cost: its printed row
    /// (702.35a, 702.94a), separately from the generated trigger's index.
    pub(crate) fn linked_cost_offer(&self, cost: &ir::Cost) -> Option<usize> {
        let index = self.ability.clone()?.index;
        crate::keyword::offered_costs(self.def?).into_iter().find(|(_, said, trigger)| *trigger == index && said == cost).map(|(row, ..)| row)
    }

    /// A self-cast offer keeps the source zone proved by this very ability's scope.
    pub(crate) fn source_scope(&self) -> Option<crate::functions::Scope> {
        self.def?.scope(self.ability.clone()?.index)?.ok()
    }
    /// Only the generated last-counter ability carries Suspend's haste rule (702.62a).
    pub(crate) fn suspend_offer(&self) -> bool { self.generated_by("Suspend") }
    pub(crate) fn generated_by(&self, name: &str) -> bool {
        let (Some(def), Some(key)) = (self.def, self.ability.clone()) else { return false };
        crate::keyword::generated_by(def).find_map(|(index, instance)| (index == key.index).then_some(instance))
            .is_some_and(|instance| instance.item.keyword.name() == name)
    }
    pub(crate) fn ability_key(&self) -> Option<crate::binding::AbilityKey> { self.ability.clone() }
    pub(crate) fn cost_exile_writer(&self, key: crate::binding::AbilityKey) -> bool {
        self.def.is_some_and(|def| crate::binding::cost_exile_writer(def, key.index))
    }

    /// Where each paragraph of a spell's instructions after the first begins among its
    /// sentences, when its consecutive paragraphs are followed as one text (608.2c).
    pub(crate) fn paragraph_starts(&self) -> Vec<usize> {
        let (Some(def), Some(key)) = (self.def, self.ability.clone()) else { return Vec::new() };
        if key != crate::binding::AbilityKey::printed(key.index) || key.index >= def.abilities.len() { return Vec::new(); }
        let length = |at: usize| match &def.abilities[at] { ir::Ability::Text(e) => e.0.len(), _ => 0 };
        (key.index + 1..def.abilities.len()).take_while(|j| crate::program::continues_previous(def, *j))
            .scan(length(key.index), |start, j| { let at = *start; *start += length(j); Some(at) }).collect()
    }

    pub(crate) fn ability(&self) -> Option<&'a ir::Ability> {
        let key = self.ability.clone()?;
        self.def?.abilities.iter().chain(self.def?.keyword_abilities.iter().map(|(_, a)| a)).nth(key.index)
    }

    /// Object payments supplied to this occurrence (601.2h, 602.2b, 608.2k).
    pub(crate) fn costs(&self) -> Vec<&'a ir::Cost> {
        match self.ability() {
            Some(ir::Ability::Activated { cost, .. }) => vec![cost],
            Some(ir::Ability::Text(_)) => self.def.into_iter().flat_map(|d| d.casting_costs().map(|(_, cost)| cost)).collect(),
            _ => Vec::new(),
        }
    }

    /// A name in a "named" predicate is the literal name of this definition (201.5),
    /// not an anaphor that follows the source object's changing characteristics.
    /// Is this the text of an instant or sorcery spell, which does what it does only as it
    /// resolves?
    pub(crate) fn spell_text(&self) -> bool {
        matches!(self.ability(), Some(ir::Ability::Text(_)))
            && self.def.is_some_and(|d| d.is(ir::CardType::Instant) || d.is(ir::CardType::Sorcery))
    }

    pub(crate) fn printed_name(&self) -> Res<&str> {
        self.def.map(|d| d.name()).ok_or_else(|| crate::Unsupported("named predicate needs a definition".into()))
    }

    pub(crate) fn exile_producer(&self) -> Res<crate::binding::AbilityKey> {
        let def = self.def.ok_or_else(|| crate::Unsupported("linked exile needs a definition".into()))?;
        let reader = self.ability.clone().ok_or_else(|| crate::Unsupported("linked exile needs an ability identity".into()))?;
        self.record_linked_exile(crate::binding::exile_producer(def, reader, false)?)
    }

    pub(crate) fn linked_exile_producer(&self) -> Res<crate::binding::AbilityKey> {
        let def = self.def.ok_or_else(|| crate::Unsupported("linked exile needs a definition".into()))?;
        let reader = self.ability.clone().ok_or_else(|| crate::Unsupported("linked exile needs an ability identity".into()))?;
        self.record_linked_exile(crate::binding::exile_producer(def, reader, true)?)
    }

    fn record_linked_exile(&self, writer: crate::binding::AbilityKey) -> Res<crate::binding::AbilityKey> {
        let mut writers = self.linked_exile_writers.borrow_mut();
        if !writers.contains(&writer.index) { writers.push(writer.index); }
        Ok(writer)
    }
    pub(crate) fn linked_exile_writers(&self) -> Vec<usize> { self.linked_exile_writers.borrow().clone() }
    pub(crate) fn lowering(&self) -> &std::cell::Cell<crate::program::Lowering> { &self.lowering }

    pub(crate) fn choice_candidates(&self, kind: crate::binding::Kind) -> Res<Vec<(crate::binding::LinkId, crate::binding::Kind)>> {
        match (self.def, self.ability.clone()) {
            (Some(def), Some(reader)) => def.links.candidates(reader, kind),
            _ => Ok(Vec::new()),
        }
    }

    pub(crate) fn choice(&self, kind: crate::binding::Kind) -> Res<(crate::binding::LinkId, crate::binding::Kind)> {
        let reader = self.ability.clone().ok_or_else(|| crate::Unsupported("linked choice: no ability identity (607.2d)".into()))?;
        let def = self.def.ok_or_else(|| crate::Unsupported("linked choice: no definition".into()))?;
        def.links.resolve(reader, kind)
    }

    /// Is the text being lowered this definition's hideaway trigger (702.75a), whose exiled
    /// card gains that the controller of the permanent may look at it?
    pub(crate) fn hideaway(&self) -> bool {
        let (Some(def), Some(key)) = (self.def, self.ability.clone()) else { return false };
        key.origin == crate::binding::Origin::Own
            && key.index.checked_sub(def.abilities.len()).and_then(|i| def.keyword_abilities.get(i))
                .is_some_and(|(row, _)| matches!(&def.abilities[*row], ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Hideaway")))
    }

    /// A kicked condition refers to this definition's own kicker (702.33e).
    /// Is the text being lowered what an opening-hand action has its player do (103.6)?
    pub(crate) fn opening(&self) -> bool {
        self.ability.clone().is_some_and(|key| key.origin == crate::binding::Origin::Own && self.def.is_some_and(|d| d.opening.contains(&key.index)))
    }
    /// Lexical source compatibility for completed-clause "other", not runtime membership.
    /// Granted templates have no source type; a grantor's type is never evidence here.
    pub(crate) fn completed_source_creature(&self) -> Option<bool> {
        if self.ability.clone().is_some_and(|key| matches!(key.origin, crate::binding::Origin::Given { .. } | crate::binding::Origin::Rule(_))) { return None; }
        let types = self.def?.types();
        if types.contains(&ir::CardType::Creature) { Some(true) }
        else if types.iter().any(|t| matches!(t, ir::CardType::Instant | ir::CardType::Sorcery | ir::CardType::Planeswalker)) { Some(false) }
        else { None }
    }
    pub(crate) fn has_kicker(&self) -> bool { self.def.is_some_and(|d| crate::keyword::kicker(d).is_some()) }
    /// Whether the spell whose text this is has bargain (702.166a).
    pub(crate) fn has_bargain(&self) -> bool { self.def.is_some_and(crate::keyword::bargain) }
    /// The one optional additional cost of the spell whose text this is (118.8b), and what it
    /// is: what "if this spell's additional cost was paid" and its kind's phrases ask of.
    pub(crate) fn optional(&self) -> Option<(crate::keyword::Optional, &ir::Cost)> {
        let mut costs = crate::keyword::optional_costs(self.def?);
        let (_, kind, cost) = costs.next()?;
        costs.next().is_none().then_some((kind, cost))
    }
    /// Whether the spell whose text this is has gift (702.174a).
    pub(crate) fn has_gift(&self) -> bool { self.def.is_some_and(|d| crate::keyword::gift(d).is_some()) }
    /// The row of the keyword whose alternative cost the text names: "its evoke cost" (702.74a).
    /// The rule's own trigger asks of its own instance (113.2c); other text, of the one
    /// instance the card prints, or of none when there are several.
    pub(crate) fn alternative_row(&self, keyword: ir::Keyword) -> Option<usize> {
        let def = self.def?;
        let rows = crate::keyword::alternative_rows(def, keyword);
        if let Some(key) = self.ability.clone() && rows.contains(&def.ability_row(key.index)) { return Some(def.ability_row(key.index)); }
        match rows.as_slice() { [one] => Some(*one), _ => None }
    }

    /// Whether the text being lowered has an X to read (107.3).
    pub(crate) fn with_x(mut self, available: bool) -> Self {
        self.x_available = available;
        self
    }

    pub(crate) fn x_available(&self) -> bool {
        self.x_available
    }

    /// The same definition, lowering an entry replacement effect of the permanent itself
    /// ("this creature enters with X +1/+1 counters on it"), which reads its spell's X (107.3m).
    pub(crate) fn with_spell_x(mut self) -> Self {
        self.spell_x = self.def.is_some_and(|def| def.permanent_x());
        self
    }

    /// X of the retained spell whose own cast caused this triggered occurrence (107.3e).
    pub(crate) fn cast_x(&self) -> bool {
        let (Some(def), Some(key)) = (self.def, self.ability.as_ref()) else { return false };
        if matches!(&key.origin, crate::binding::Origin::Given { .. } | crate::binding::Origin::Rule(_)) { return false; }
        def.cost().is_some_and(|cost| cost.0.contains(&crate::cost::X))
            && matches!(def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).nth(key.index),
                Some(ir::Ability::Triggered { trigger, .. }) if own_cast(trigger))
    }

    /// Whether X here is that of the spell the permanent was (107.3m).
    pub(crate) fn spell_x(&self) -> bool {
        self.spell_x
    }

    /// Can the engine evaluate this description at all? Checked whole, whatever is on the
    /// battlefield, so that a card is refused before it is cast rather than halfway through.
    pub(crate) fn supported(&self, set: &ObjectSet) -> Res<()> {
        self.set(set).map(|_| ())
    }

    /// Can the engine tell whether a card is one this description names?
    pub(crate) fn cards_supported(&self, set: &ObjectSet) -> Res<()> {
        self.cards(set).map(|_| ())
    }

    /// Text with no local producer in scope: what was chosen is read through the
    /// definition's links (607.2d).
    fn set(&self, set: &ObjectSet) -> Res<expr::Set> {
        expr::Set::compile(set, self, &mut expr::linked(self))
    }

    #[cfg(test)]
    fn filter(&self, f: &Filter) -> Res<expr::Filter> {
        expr::Filter::compile(f, self, &mut expr::linked(self))
    }

    fn cards(&self, set: &ObjectSet) -> Res<expr::Cards> {
        expr::Cards::compile(set, self, &mut expr::linked(self))
    }
}

// What follows asks one question of text that is not compiled yet: each lowers the
// description with what the game knows of the definition, then evaluates what it lowered.
// Nothing here evaluates syntax itself.

/// Can the engine evaluate this description at all? This is independent of which objects
/// currently exist; actual membership and target restrictions are checked separately.
pub fn supported(game: &Game, ctx: &Ctx, set: &ObjectSet) -> Res<()> {
    Support::runtime(game, ctx).supported(set)
}

pub fn cards_supported(game: &Game, ctx: &Ctx, set: &ObjectSet) -> Res<()> {
    Support::runtime(game, ctx).cards_supported(set)
}

/// Is this card one the description names?
pub fn card_in_set(game: &Game, ctx: &Ctx, set: &ObjectSet, card: ObjId) -> Res<bool> {
    Ok(Support::runtime(game, ctx).cards(set)?.contains(game, ctx, &NoLocals, card))
}

/// Is this object or player one the set describes?
pub fn in_set(game: &Game, ctx: &Ctx, set: &ObjectSet, t: Target) -> Res<bool> {
    Ok(Support::runtime(game, ctx).set(set)?.contains(game, ctx, &NoLocals, t))
}

/// Everything the set describes: players, battlefield objects, stack objects (112.1),
/// or cards in graveyards where it is of those.
pub fn all_in(game: &Game, ctx: &Ctx, set: &ObjectSet) -> Res<Vec<Target>> {
    Ok(Support::runtime(game, ctx).set(set)?.members(game, ctx, &NoLocals))
}

/// The number an amount is, now. A quantity used by an effect: only the final result is
/// clamped to zero (107.1b).
pub fn amount(game: &Game, ctx: &Ctx, a: &Amount) -> Res<u32> {
    Ok(expr::nonnegative(number(game, ctx, a)?))
}

/// Values within calculations and comparisons stay signed (107.1b); a negative component
/// still contributes to a total, and a negative threshold is not replaced with zero.
pub(crate) fn number(game: &Game, ctx: &Ctx, amount: &Amount) -> Res<i64> {
    let support = Support::runtime(game, ctx);
    Ok(expr::Number::compile(amount, &support, &mut expr::linked(&support))?.evaluate(game, ctx, &NoLocals))
}

/// The counters on an object: of one kind, or all of them.
pub(crate) fn counters_on(game: &Game, o: ObjId, kind: Option<&ir::CounterKind>) -> u64 {
    game.obj(o).counters.iter().filter(|(k, _)| kind.is_none_or(|kind| kind.0 == **k)).map(|(_, n)| u64::from(*n)).sum()
}

/// Fixed target cardinality used by compiled announcement specifications. Dynamic
/// quantities remain an explicit unsupported operand until their announcement binding is
/// compiled; accepting them here without that binding would freeze the wrong value.
pub(crate) fn constant_target_bounds(count: &Count) -> Res<(u32, u32)> {
    match count {
        Count::Exactly(Amount::Const(n)) => Ok((*n, *n)),
        Count::UpTo(Amount::Const(n)) => Ok((0, *n)),
        Count::AnyNumber => Ok((0, u32::MAX)),
        Count::Range { min, max } => match (&**min, &**max) {
            (Amount::Const(min), Amount::Const(max)) if min <= max => Ok((*min, *max)),
            _ => unsupported("compiled target range: ordered constant bounds required"),
        },
        other => unsupported(format!("compiled target count: {}", name(other))),
    }
}

/// How many targets a "target" takes: at least and at most.
pub fn bounds(game: &Game, ctx: &Ctx, count: &Count) -> Res<(u32, u32)> {
    match count {
        Count::Exactly(n) => {
            let n = amount(game, ctx, n)?;
            Ok((n, n))
        }
        Count::UpTo(n) => Ok((0, amount(game, ctx, n)?)),
        Count::AnyNumber => Ok((0, u32::MAX)),
        Count::Range { .. } => constant_target_bounds(count),
        other => unsupported(format!("number of targets: {}", name(other))),
    }
}

#[cfg(test)]
mod support_tests {
    use super::*;
    use crate::CardDef;
    use crate::state::Zone;
    use ir::Stat;

    #[test]
    fn r607_2d_operand_support_binds_choice_provenance_without_a_game_or_record() {
        let chosen = Filter::Atom(Atom::Reference(ir::reference::UnresolvedRef::choice(crate::binding::Kind::Color)));
        let source = CardDef::new("Choice", "", "Land", "As this land enters, choose a color.", None).unwrap();
        let reader = Some(crate::binding::AbilityKey::printed(0));
        assert!(Support::definition(&source, reader.clone()).filter(&chosen).is_ok());
        assert_eq!(Support::definition(&source, None).filter(&chosen).unwrap_err().0, "linked choice: no ability identity (607.2d)");
        let unrelated = CardDef::new("Other", "", "Land", "", None).unwrap();
        assert_eq!(Support::definition(&unrelated, reader).filter(&chosen).unwrap_err().0, "linked choice: unknown ability provenance or kind (607.5)");
    }

    #[test]
    fn r608_2h_unsupported_aggregate_operands_are_refused_even_when_the_set_is_empty() {
        let source = CardDef::new("Source", "", "Land", "", None).unwrap();
        let support = Support::definition(&source, None);
        let unsupported = Amount::Aggregate {
            agg: ir::amount::Aggregate::Greatest,
            stat: Stat::BasePower,
            among: Box::new(ObjectSet::of(Domain::Permanent)),
        };
        assert!(expr::Number::compile(&unsupported, &support, &mut expr::linked(&support)).unwrap_err().0.contains("BasePower"));
        let mut game = Game::new(vec![source], 2, 20);
        let object = game.put(0, 0, Zone::Hand);
        assert!(number(&game, &Ctx::new(0, object), &unsupported).unwrap_err().0.contains("BasePower"));
    }

    #[test]
    fn r601_2c_every_union_target_arm_requires_a_supported_description() {
        let source = CardDef::new("Source", "", "Land", "", None).unwrap();
        let union = ObjectSet::of(Domain::Union(vec![
            ObjectSet::of(Domain::Permanent),
            ObjectSet::of(Domain::Card), // cards without a zone do not denote announcement targets
        ]));
        assert_eq!(Support::definition(&source, None).supported(&union).unwrap_err().0, "noun phrase: Card");
        let mut game = Game::new(vec![source], 2, 20);
        let object = game.put(0, 0, Zone::Hand);
        assert_eq!(expr::Set::compile(&union, &Support::runtime(&game, &Ctx::for_spell(0, object)), &mut expr::linked(&Support::runtime(&game, &Ctx::for_spell(0, object)))).unwrap_err().0, "noun phrase: Card");
    }
}

impl crate::retention::Trace for Ctx<'_> {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { source, was, casting, view, controller: _, ability: _, link_generation: _, effect: _, layer: _, x: _, targeting_spell: _, face_then, then } = self;
        assert!(view.is_none() && then.is_none(), "query-local views cannot be retained");
        source.trace(visit);
        was.trace(visit);
        if let Some((object, _)) = face_then { object.trace(visit); }
        if let Some((object, _)) = casting { object.trace(visit); }
    }
}
