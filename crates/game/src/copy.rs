//! Copiable values (707.2): an immutable baseline, separate from physical card identity,
//! counters, status, linked choices and later continuous effects. No source handle is kept.
use std::sync::Arc;
use crate::{CardDef, Game, ObjId, Res, unsupported};
use crate::card::CastAs;
use crate::program::expr;
use crate::select::Support;
use crate::state::Chars;
use ir::{Ability, Action, Effect, Modifier, Quant, Ref, Sentence, Subject};

#[derive(Debug, Clone)]
pub(crate) enum Definition {
    Card(usize, CastAs),
    Back(usize),
    Derived(Arc<CardDef>),
}
impl PartialEq for Definition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Card(a,b), Self::Card(c,d)) => a == c && b == d,
            (Self::Back(a), Self::Back(b)) => a == b,
            (Self::Derived(a), Self::Derived(b)) => Arc::ptr_eq(a,b),
            _ => false,
        }
    }
}
impl Eq for Definition {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Values { pub chars: Chars, pub definition: Option<Definition> }
impl Definition {
    pub(crate) fn definition<'a>(&'a self, defs: &'a [CardDef]) -> Option<&'a CardDef> {
        match self {
            Self::Card(i, CastAs::Half(h)) => defs[*i].halves.get(*h),
            Self::Card(i, CastAs::Face(face)) => defs[*i].modal_face(*face).ok(),
            Self::Card(i, CastAs::Adventure) => defs[*i].adventure(),
            Self::Card(i, CastAs::Omen) => defs[*i].omen(),
            Self::Card(i, _) => Some(&defs[*i]),
            Self::Back(i) => defs[*i].back(),
            Self::Derived(d) => Some(d),
        }
    }
}
impl Values {
    pub fn definition<'a>(&'a self, game: &'a Game) -> Option<&'a CardDef> {
        match &self.definition {
            Some(face) => face.definition(&game.defs),
            None => self.chars.predefined.and_then(|k| crate::token::definition(k).ok()),
        }
    }
    /// Battlefield characteristics apply the holder's Room designations without
    /// changing the full, reusable copiable values (709.5).
    pub(crate) fn permanent_chars(&self, game: &Game, unlocked: [bool; 2]) -> Chars {
        self.definition(game).filter(|def| def.is_room())
            .map_or_else(|| self.chars.clone(), |room| room.room(unlocked).chars())
    }
}
/// A creating effect defines immutable faces for a token or castable card copy
/// (111.3, 707.8a, 707.10g, 707.12). This is
/// separate from an entry-copy effect, which changes its copiable characteristics but
/// cannot give a single-faced token a second face. No originating object is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Faces { pub front: Values, pub back: Option<Values> }
impl Faces {
    pub fn face(&self, transformed: bool) -> &Values {
        if transformed { self.back.as_ref().expect("only double-faced tokens transform") } else { &self.front }
    }
}

/// A token creation proposal: shared immutable characteristics and the face it enters with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token { pub(crate) faces: Arc<Faces>, pub(crate) transformed: bool }
impl Token {
    pub(crate) fn defined(chars: Chars) -> Self {
        Self { faces: Arc::new(Faces { front: Values { chars, definition: None }, back: None }), transformed: false }
    }
    /// A token whose text is this definition (111.3): its characteristics are the
    /// definition's, and its abilities are found from it.
    pub(crate) fn described(def: CardDef) -> Self {
        let chars = def.chars();
        Self { faces: Arc::new(Faces { front: Values { chars, definition: Some(Definition::Derived(Arc::new(def))) }, back: None }), transformed: false }
    }
    pub(crate) fn current(&self) -> &Values { self.faces.face(self.transformed) }
}

impl Game {
    /// Copying an existing object (707.8) takes only the face currently up. Token creation
    /// uses the same face values through `token_copy`, with the exception of 707.8a.
    pub(crate) fn copiable(&self, source: ObjId) -> Values {
        let o = self.obj(source);
        // A copy of a face-down permanent has its face-down values (707.2, 708.2a).
        if let crate::face::Face::Down(down) = o.face {
            let def = crate::face::definition(down.by);
            return Values { chars: def.chars(), definition: Some(Definition::Derived(def.clone())) };
        }
        if let Some(values) = &o.copied { return values.clone(); }
        if let Some(faces) = &o.token_faces {
            let mut values = faces.face(o.transformed || o.cast_as == CastAs::Face(1)).clone();
            // A transforming back has no mana cost; copying only that face loses the
            // front's mana value. A modal back has its own mana cost (712.8f).
            if matches!(values.definition, Some(Definition::Back(i)) if !self.defs[i].modal) { values.chars.mana_value = 0; }
            return values;
        }
        // A Room's lock abilities and both halves are copiable; the current
        // unlocked designations are not (709.5, 707.2).
        let mut chars = if o.zone == crate::Zone::Battlefield {
            self.room_card(source).map_or_else(|| o.chars.clone(), CardDef::chars)
        } else { o.chars.clone() };
        if o.transformed && o.card.is_some_and(|i| !self.defs[i].modal) { chars.mana_value = 0; }
        let definition = o.card.map(|i| if o.transformed { Definition::Back(i) } else { Definition::Card(i, o.cast_as) });
        Values { chars, definition }
    }

    /// 707.8a: a newly created token copies both faces, modified by copy effects, and
    /// enters with the source's current face up. The same baseline serves a castable
    /// card copy and the token its permanent spell can become (707.10g, 707.12).
    /// Later layers and status are not copied.
    pub(crate) fn token_copy(&self, source: ObjId) -> Token {
        let o = self.obj(source);
        if let crate::face::Face::Down(_) = o.face {
            return Token { faces: Arc::new(Faces { front: self.copiable(source), back: None }), transformed: false };
        }
        if o.copied.is_none() && let Some(faces) = &o.token_faces {
            return Token { faces: faces.clone(), transformed: o.transformed || o.cast_as == CastAs::Face(1) };
        }
        let physical = o.card.filter(|i| self.defs[*i].back().is_some());
        let double = physical.is_some() || o.token_faces.as_ref().is_some_and(|f| f.back.is_some());
        let faces = if double && o.copied.is_none() {
            let i = physical.expect("a token's own faces were retained above");
            Faces {
                front: Values { chars: self.defs[i].chars(), definition: Some(Definition::Card(i, CastAs::Whole)) },
                back: Some(Values { chars: self.defs[i].back().unwrap().chars(), definition: Some(Definition::Back(i)) }),
            }
        } else {
            // A copy effect on a double-faced object applies to both faces (707.8a).
            let front = self.copiable(source);
            let back = double.then(|| front.clone());
            Faces { front, back }
        };
        Token { faces: Arc::new(faces), transformed: double && (o.transformed || o.cast_as==CastAs::Face(1)) }
    }
}

/// Supported 707.9 exceptions, shared by entry copying and token creation. They modify
/// copiable values; later ability/type layers never get folded into the copy.
pub(crate) fn changes(exceptions: &[ir::Exception], token: bool) -> Res<Vec<ir::CopyChange>> {
    exceptions.iter().map(|e| {
        if !token && e.subject != ir::CopyRef::It { return unsupported("copy exception subject"); }
        match &e.change {
            ir::CopyChange::NotLegendary => {},
            ir::CopyChange::Is(ir::action::Becoming::Is(c)) if c.keeps == ir::action::Keeps::OtherTypes
                && c.pt.is_none() && c.colors.is_none() && c.name.is_none() && c.abilities.is_empty() && c.base_pt.is_none() => {},
            // "except it's 1/1": a power and toughness of its copiable values (707.9b), and
            // what defines them is not copied (707.9d)
            ir::CopyChange::Is(ir::action::Becoming::Pt(ir::token::Pt::Const(_), ir::token::Pt::Const(_))) => {},
            ir::CopyChange::Has(abilities) => for a in abilities {
                match a {
                    ir::action::Granted::Ability(a) => { crate::ability::template(a)?; },
                    ir::action::Granted::Keyword(k) if crate::keyword::passive(k) => {},
                    _ => return unsupported("copy exception ability"),
                }
            },
            _ => return unsupported("copy exception: characteristic replacement (707.9d)"),
        }
        Ok(e.change.clone())
    }).collect()
}
impl Values {
    fn changed(&self, game: &Game, changes: &[ir::CopyChange]) -> Self {
        if changes.is_empty() { return self.clone(); }
        let def = CardDef::copied(&self.chars, self.definition(game), changes);
        Self { chars: def.chars(), definition: Some(Definition::Derived(Arc::new(def))) }
    }
}
impl Token {
    pub(crate) fn changed(self, game: &Game, changes: &[ir::CopyChange]) -> Self {
        if changes.is_empty() { return self; }
        Self { faces: Arc::new(Faces {
            front: self.faces.front.changed(game, changes),
            back: self.faces.back.as_ref().map(|face| face.changed(game, changes)),
        }), transformed: self.transformed }
    }
}

/// Historical entry-copy acquisition for privileged audit. The donor handle is a
/// historical identity only; no rules query follows it or retains its live payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Acquisition { pub donor: ObjId, pub before: Values, pub producer: Option<crate::binding::AbilityKey> }

/// An entry-copy replacement, compiled once. Additive types/abilities and nonlegendary
/// exceptions are supported; characteristic replacement needs 707.9d CDA exclusion too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry { pub producer: Option<crate::binding::AbilityKey>, pub from: expr::Set, pub changes: Vec<ir::CopyChange>, pub optional: bool }
impl Entry {
    pub fn compile(a: &Ability, support: &Support<'_>) -> Option<Res<Self>> {
        let Ability::Text(Effect(sentences)) = a else { return None };
        let [Sentence::Do(body)] = sentences.as_slice() else { return None };
        let clause = &body.first;
        let Action::Have { who, action } = &clause.first.action else { return None };
        let Action::Static(action) = action.as_ref() else { return None };
        let ir::statics::StaticAct::Enters(arrivals) = action.as_ref() else { return None };
        if !arrivals.iter().any(|a| matches!(a, ir::statics::Arrival::AsCopyOf(_))) { return None; }
        Some((|| {
            let [ir::statics::Arrival::AsCopyOf(from)] = arrivals.as_slice() else { return unsupported("copy entry combined with another arrival"); };
            if !body.rest.is_empty() || !clause.rest.is_empty() || clause.also || clause.first.may
                || !matches!(&clause.subject, Some(Subject::Selector(s)) if s.quant == Quant::Definite(Ref::You))
                || who.quant != Quant::Definite(Ref::This) || from.quant != Quant::Any
                || from.chooser.is_some() || from.random.is_some() {
                return unsupported("copy entry: controller chooses the source permanent");
            }
            fn permanent(set: &ir::ObjectSet) -> bool {
                let domain = match &set.domain { ir::Domain::Permanent => true,
                    ir::Domain::Union(arms) => !arms.is_empty() && arms.iter().all(permanent), _ => false };
                domain && set.zone.as_ref().is_none_or(|z| z.zones.items == [ir::Zone::Battlefield]
                    && z.owner.is_none() && z.slice.is_none())
            }
            if !permanent(&from.set) { return unsupported("entry copy: source outside the battlefield"); }
            let mut changes = Vec::new();
            for m in &clause.first.mods {
                let Modifier::Except(exceptions) = m else { return unsupported("copy entry modifier"); };
                changes.extend(self::changes(exceptions, false)?);
            }
            let from = expr::Set::compile(&from.set, support, &mut expr::linked(support))?;
            Ok(Self { producer: support.ability_key(), from, changes, optional: clause.may })
        })())
    }
    pub fn values(&self, game: &Game, source: ObjId) -> Values {
        game.copiable(source).changed(game, &self.changes)
    }
}

/// Layer-one effects are fixed values in timestamp order (613.2a, 707.2b). The live
/// baseline stays in Object.copied/Chars, shared by every existing characteristic query.
/// No copied-from incarnation or expired effect is retained.
#[derive(Debug, Clone, Default)]
pub(crate) struct Effects {
    pub base: Option<Box<Base>>,
    layers: Vec<Resolved>,
}
#[derive(Debug, Clone)]
pub(crate) struct Base {
    pub chars: Chars,
    pub copied: Option<Values>,
}
#[derive(Debug, Clone)]
struct Resolved { values: Values, stamp: u32, turn: bool }

impl Game {
    /// Lookup an acquisition still carried by a layer, including one hidden underneath a
    /// newer copy. Expired occurrences own their compiled programs, not this live lookup.
    pub(crate) fn copied_definition(&self, object: ObjId, stamp: u32) -> Option<&CardDef> {
        self.copied_values(object, stamp)?.definition(self)
    }
    pub(crate) fn copied_values(&self, object: ObjId, stamp: u32) -> Option<&Values> {
        Some(&self.obj(object).copy_effects.layers.iter().find(|effect| effect.stamp == stamp)?.values)
    }
    pub(crate) fn copy_origin(&self, object: ObjId) -> crate::binding::Origin {
        self.obj(object).copy_effects.layers.last().map_or(crate::binding::Origin::Own,
            |effect| crate::binding::Origin::Copied(effect.stamp))
    }
    pub(crate) fn become_copy(&mut self, affected: Vec<ObjId>, values: Values, turn: bool) {
        let previous = self.control_snapshot();
        let stamp = self.tick();
        for object in affected {
            if self.obj(object).zone != crate::Zone::Battlefield { continue; }
            // The base keeps face-up values only; a face-down permanent keeps its face-down
            // characteristics until it is turned face up (708.10).
            let down = self.face_down(object);
            let face_up = if down { self.face_up_chars(object) } else { self.obj(object).chars.clone() };
            let o = self.obj_mut(object);
            if o.copy_effects.base.is_none() {
                o.copy_effects.base = Some(Box::new(Base { chars: face_up, copied: o.copied.clone() }));
            }
            // A permanent copy supersedes all older resolved copies permanently.
            if !turn { o.copy_effects.layers.clear(); }
            o.copy_effects.layers.push(Resolved { values: values.clone(), stamp, turn });
            o.copied = Some(values.clone());
            self.refresh_face(object);
        }
        self.prune_control_durations();
        self.control_transitions(previous);
    }
    pub(crate) fn end_turn_copies(&mut self) {
        for object in self.battlefield.clone() {
            let down = self.face_down(object);
            let o = self.obj_mut(object);
            if !o.copy_effects.layers.iter().any(|effect| effect.turn) { continue; }
            o.copy_effects.layers.retain(|effect| !effect.turn);
            if let Some(effect) = o.copy_effects.layers.last() {
                o.chars = effect.values.chars.clone();
                o.copied = Some(effect.values.clone());
            } else if let Some(base) = o.copy_effects.base.take() {
                o.chars = base.chars;
                o.copied = base.copied;
            }
            // Restore the face-down overlay or apply the Room's own current
            // designations, which may have changed during the copy (708.10, 709.5).
            if down || self.room_card(object).is_some() { self.refresh_face(object); }
        }
    }
}

/// A retained instruction makes and optionally retargets each copy before putting it on
/// the stack (707.10c). A later question never replays an earlier copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StackCopies {
    sources: Vec<ObjId>, times: u32, source: usize, count: u32, player: crate::PlayerId,
    retarget: bool, pending: Option<ObjId>, answer: Option<crate::state::Answer>, made: Vec<ObjId>,
}
impl StackCopies {
    pub(crate) fn new(sources: Vec<ObjId>, times: u32, player: crate::PlayerId, retarget: bool) -> Self {
        Self { sources, times, source: 0, count: 0, player, retarget, pending: None, answer: None, made: Vec::new() }
    }
    pub(crate) fn answer(&mut self, answer: crate::state::Answer) { self.answer = Some(answer); }
    pub(crate) fn advance(&mut self, game: &mut Game) -> Result<Vec<ObjId>, crate::state::Decision> {
        use crate::state::{Answer, Choice, Decision};
        while self.source < self.sources.len() {
            if self.count == self.times { self.source += 1; self.count = 0; continue; }
            let original = self.sources[self.source];
            let copy = match self.pending {
                Some(copy) => copy,
                None => match game.prepare_stack_copy(original, self.player) {
                    Some(copy) => { self.pending = Some(copy); copy },
                    None => { self.source += 1; self.count = 0; continue; }
                }
            };
            if self.retarget && game.obj(copy).targets.iter().any(|g| !g.is_empty()) {
                match self.answer.take() {
                    Some(Answer::Targets(targets)) => game.obj_mut(copy).targets = targets,
                    Some(Answer::No) => {},
                    Some(_) => unreachable!("copy target answer validated at command boundary"),
                    None => return Err(Decision::Choose { player: self.player, choice: Choice::Retarget {
                        copy, original: game.obj(copy).targets.clone(), slots: game.copy_target_offers(copy),
                    }}),
                }
            }
            game.stack.push(copy);
            let event = if game.obj(copy).ability.is_some() {
                crate::Event::AbilityCopied { original, copy, by: self.player }
            } else { crate::Event::SpellCopied { original, copy, by: self.player } };
            game.log.push(event);
            game.targeting_events(copy);
            self.made.push(copy); self.pending = None; self.count += 1;
        }
        Ok(std::mem::take(&mut self.made))
    }
}
impl Game {
    /// A stack spell or ability's copiable choices and retained executable (707.10), including
    /// its last stack incarnation when the copying ability outlives it (608.2h).
    fn prepare_stack_copy(&mut self, original: ObjId, player: crate::PlayerId) -> Option<ObjId> {
        use crate::Zone;
        let old = self.get_object(original)?.clone();
        if !(old.zone == Zone::Stack || old.zone == Zone::Gone && old.left == Some(Zone::Stack)) { return None; }
        let values = old.ability.is_none().then(|| self.token_copy(original));
        let copy = self.new_object(None, old.chars.clone(), player, player, Zone::Stack);
        // It enters the stack only after choosing its new targets (707.10c).
        assert_eq!(self.stack.pop(), Some(copy));
        let made = self.obj_mut(copy);
        if let Some(values) = values {
            made.token_faces = Some(values.faces); made.transformed = values.transformed; made.card_copy = true;
        }
        made.stack_copy_of = Some(original);
        made.ability = old.ability; made.delayed = old.delayed;
        made.targets = old.targets; made.modes = old.modes; made.x = old.x; made.kicked = old.kicked; made.optional_paid = old.optional_paid; made.alternative = old.alternative; made.buyback_paid = old.buyback_paid; made.entwined = old.entwined;
        made.cast_exiled = old.cast_exiled;
        made.cast_announcement = old.cast_announcement.map(|mut announcement| {
            // Alternative casting choices copy; this copy was never cast or paid (707.10).
            announcement.payment = None;
            announcement.permission = None;
            announcement
        });
        // Completion rules also apply to copies of alternative spells (715.3c, 701.24c).
        if matches!(old.cast_as, CastAs::Adventure | CastAs::Omen) { made.cast_as = old.cast_as; }
        // and the division announced, which new targets do not change (707.10, 115.7f)
        made.division = old.division;
        made.replicated = old.replicated; made.gift = old.gift; made.announced = old.announced; made.bound = old.bound;
        made.bound.revealed_through_resolution.clear();
        made.bound.inputs.retain(|(i,_)|*i!=crate::program::input::Input::Convoked);
        made.invocations = old.invocations.map(|invocations| invocations.map(|mut invocations| {
            for invocation in &mut invocations {
                if old.ability.is_none() { invocation.ctx.source = Some(copy); }
                invocation.ctx.controller = player;
                invocation.ctx.casting = None;
                if old.ability.is_none() { invocation.ctx.was = None; }
            }
            invocations
        }));
        Some(copy)
    }
    fn copy_target_offers(&self, copy: ObjId) -> Vec<crate::legal::Slot> {
        let object = self.obj(copy);
        let (specs, contexts) = self.stack_target_metadata(copy);
        let mut offers = self.target_offers_contexts(&contexts, &specs, &object.bound, Some(&object.targets));
        for (offer, original) in offers.iter_mut().zip(&object.targets) {
            offer.min = original.len() as u32; offer.max = offer.min;
        }
        offers
    }
    /// Final target sets obey the same compiled queries as announcement, while unchanged
    /// targets may already be illegal (115.7d-e). The number/order of positions cannot change.
    pub(crate) fn legal_copy_targets(&self, copy: ObjId, original: &[Vec<crate::Target>], targets: &[Vec<crate::Target>]) -> Result<(), String> {
        let (specs, contexts) = self.stack_target_metadata(copy);
        self.legal_new_targets(copy, &specs, &contexts, original, targets,
            "a copy retains target counts and division (115.7f, 707.10)")
    }
}
