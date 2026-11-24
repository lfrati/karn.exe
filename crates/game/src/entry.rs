//! Battlefield entry as a proposed event (614.12). A reserved identity is in no zone until
//! commit. Characteristic queries temporarily view only that object as a permanent; other
//! simultaneous arrivals never become existing sources. No events or full-game copies are
//! needed for this view.
pub(crate) mod prepare;

use crate::event::{Event, Target};
use crate::operation::{Replaced, ReplacementId};
use crate::replace::{Choices, CounterChange, CounterScope, Entry};
use crate::select::Ctx;
use crate::state::{Game, ObjId, Object, PlayerId, Zone};
use ir::CardType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryRequest {
    Card { card: ObjId, controller: PlayerId, chosen: Choices, moved: bool },
    Token { controller: PlayerId, values: crate::copy::Token, tapped: bool, chosen: Choices, prepared: Option<ObjId> },
}
impl EntryRequest {
    pub(crate) fn card(card: ObjId, controller: PlayerId, chosen: Choices, moved: bool) -> Self {
        Self::Card { card, controller, chosen, moved }
    }
    pub(crate) fn token(controller: PlayerId, values: crate::copy::Token, tapped: bool) -> Self {
        Self::Token { controller, values, tapped, chosen: Choices::default(), prepared: None }
    }
    /// A token put onto the battlefield attacking this player or planeswalker (508.4).
    pub(crate) fn attacking(mut self, defender: Option<Target>) -> Self {
        if let Self::Token { chosen, .. } = &mut self { chosen.attacking = defender; }
        self
    }
    pub(crate) fn choose_defender(mut self, pending:bool)->Self {
        if let Self::Token {chosen,..}=&mut self {chosen.choose_defender=pending;}
        self
    }
    /// Of what it may attack, what the effect allows (508.4).
    pub(crate) fn defending(mut self, defending: Option<crate::creation::Defending>) -> Self {
        if let Self::Token { chosen, .. } = &mut self { chosen.defending = defending; }
        self
    }
    /// A token put onto the battlefield attached to this (303.4f): an Aura's choice made by
    /// the effect, its legality checked as it enters (303.4i).
    pub(crate) fn attached(mut self, to: Option<Target>) -> Self {
        if let (Self::Token { chosen, .. }, Some(to)) = (&mut self, to) { chosen.attached_to = Some(to); }
        self
    }
    pub(crate) fn context(&self) -> Option<(ObjId, PlayerId)> {
        match self {
            Self::Card { card, controller, .. } => Some((*card, *controller)),
            Self::Token { controller, prepared, .. } => prepared.map(|o| (o,*controller)),
        }
    }
    pub(crate) fn parts(&mut self) -> (ObjId, PlayerId, &mut Choices, bool) {
        match self {
            Self::Card { card, controller, chosen, .. } => (*card,*controller,chosen,false),
            Self::Token { controller, prepared, chosen, .. } => (prepared.expect("reserved token"),*controller,chosen,true),
        }
    }
    pub(crate) fn prohibited(&self, game: &Game) -> bool {
        let Self::Card { card, chosen, .. } = self else { return false };
        if game.entry_face(*card, chosen).is_none() { return true; }
        let fact = crate::trigger::Fact::new(ir::Verb::Enter, None, vec![(ir::Role::What, Target::Object(*card))]);
        if let Some(by) = chosen.face_down {
            // What would enter is the face-down object (701.40f): the prohibition is asked
            // of it, a creature card wherever the card now is.
            let mut view = game.clone();
            view.obj_mut(*card).face = crate::face::Face::Down(crate::face::Down { by });
            view.refresh_face(*card);
            return view.forbidden(&fact);
        }
        game.forbidden(&fact)
    }
    pub(crate) fn can_enter(&self, game: &mut Game) -> bool {
        if self.prohibited(game) { return false; }
        match self {
            Self::Token { prepared: Some(on), controller, chosen, .. } => game.entry_attachment_legal(*on, *controller, chosen),
            Self::Token { prepared: None, .. } => unreachable!("tokens are reserved during preparation"),
            Self::Card { card, controller, chosen, .. } => {
                let permanent = game.entry_face(*card, chosen).is_some_and(|(chars, _)|
                    chars.types.iter().any(|t| t.permanent()) && !chars.types.contains(&CardType::Instant) && !chars.types.contains(&CardType::Sorcery));
                let o = game.obj(*card);
                o.zone != Zone::Battlefield && (game.is(Target::Object(*card), ir::Sort::Card) || o.card_copy && o.zone == Zone::Stack)
                    && permanent
                    && game.entry_attachment_legal(*card, *controller, chosen)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EntryEvent {
    request: EntryRequest,
    on: ObjId,
    entry: Entry,
    question_kind: String,
    question_inventory: Vec<(String,u32)>,
}

/// One entry effect changes the proposed event once, not once per counter it describes.
pub(crate) enum EntryChange {
    Arrival { tapped: bool, prepared: bool, counters: Vec<(String, u32)> },
    Counters { kinds: CounterScope, change: CounterChange },
}

/// Restore the query overlay even if a query unwinds. The live world's zones, events and
/// other objects are never modified by characteristic inspection.
struct Viewing<'a> {
    game: &'a mut Game,
    on: ObjId,
    before: Object,
    previous: Option<(ObjId, Option<ObjId>)>,
}
impl<'a> Viewing<'a> {
    fn new(game: &'a mut Game, on: ObjId, was: Option<ObjId>, controller: PlayerId, entry: &Entry) -> Self {
        let before = game.obj(on).clone();
        let previous = game.prospective;
        game.prospective = Some((on, was));
        let prepared = entry.enters_prepared && game.has_prepare_spell(on);
        let o = game.obj_mut(on);
        o.zone = Zone::Battlefield;
        o.controller = controller;
        o.attached_to = entry.attached_to;
        o.choices = entry.values.clone();
        o.tapped = entry.tapped;
        o.prepared = prepared;
        o.counters.clear();
        for (kind, count) in &entry.counters { *o.counters.entry(kind.clone()).or_default() += count; }
        Self { game, on, before, previous }
    }
}

impl Game {
    /// The unlocked designations a Room is given as it enters: the half that was cast as
    /// the spell it was; neither for one put onto the battlefield (709.5d).
    pub(crate) fn entering_unlocked(&self, card: ObjId) -> [bool; 2] {
        let o = self.obj(card);
        match o.cast_as {
            crate::card::CastAs::Half(half) if o.zone == Zone::Stack => std::array::from_fn(|i| i == half),
            _ => [false; 2],
        }
    }

    /// The physical card's entering face, before copy replacements (712.13–14).
    /// An ordinary one-faced copy cannot enter transformed even when its text says so.
    /// What enters face down has the face-down characteristics whatever card it is (708.3).
    fn entry_face(&self, card: ObjId, chosen: &Choices) -> Option<(crate::state::Chars, bool)> {
        if let Some(by) = chosen.face_down {
            return self.is(Target::Object(card), ir::Sort::Card).then(|| (crate::face::definition(by).chars(), false));
        }
        let transformed = chosen.transformed;
        let o = self.obj(card);
        let back = transformed || ((o.transformed || o.cast_as==crate::card::CastAs::Face(1)) && (o.zone == Zone::Stack || o.zone == Zone::Gone && o.token()));
        let chars = if let Some(index) = o.card {
            let def = &self.defs[index];
            if back { def.back()?.chars() } else { def.room(self.entering_unlocked(card)).chars() }
        } else if let Some(faces) = &o.token_faces {
            let values = if back { faces.back.as_ref()? } else { &faces.front };
            values.permanent_chars(self, self.entering_unlocked(card))
        } else if back { return None; } else { o.chars.clone() };
        (!back || chars.types.iter().any(|t| t.permanent())).then_some((chars, back))
    }

    /// Ask the entering card's own choices from its prospective battlefield abilities
    /// (614.12), including ability removal already in force. Temporarily reuse its current
    /// identity: no incarnation is created until preparation has finished. Its entering
    /// face is selected first; the guard restores all source-zone values before a pause.
    fn entry_view(&mut self, card: ObjId, controller: PlayerId, chosen: &Choices) -> Viewing<'_> {
        let mut entry = Entry::default();
        entry.attached_to = chosen.attached_to;
        entry.attacking = chosen.attacking;
        entry.values = chosen.values.clone();
        let ctx = Ctx::new(controller, card);
        for arrival in &chosen.arrivals { arrival.apply(self, &ctx, &mut entry); }
        // The entrant's own static effects will have a new zone timestamp. Observe
        // that ordering without consuming a timestamp merely to ask this question.
        let stamp = self.clock + 1;
        let unlocked = self.entering_unlocked(card);
        let face = self.entry_face(card, chosen);
        let view = Viewing::new(self, card, Some(card), controller, &entry);
        view.game.obj_mut(card).stamp = stamp;
        view.game.obj_mut(card).unlocked = unlocked;
        if let Some((chars, transformed)) = face {
            view.game.obj_mut(card).chars = chars;
            view.game.obj_mut(card).transformed = transformed;
        }
        if let Some(by) = chosen.face_down { view.game.obj_mut(card).face = crate::face::Face::Down(crate::face::Down { by }); }
        // The permanent a copied spell will become is a token (111.13).
        if view.before.card_copy && view.before.zone == Zone::Stack { view.game.obj_mut(card).card_copy = false; }
        if let Some(values) = &chosen.copied {
            let chars = values.permanent_chars(view.game, unlocked);
            view.game.obj_mut(card).chars = chars;
            view.game.obj_mut(card).copied = Some(values.clone());
        }
        let prepared = entry.enters_prepared && view.game.has_prepare_spell(card);
        view.game.obj_mut(card).prepared = prepared;
        view
    }
    pub(crate) fn entry_attachment_choices(&mut self, card: ObjId, controller: PlayerId, chosen: &Choices) -> Option<Vec<Target>> {
        let view = self.entry_view(card, controller, chosen);
        view.game.enters_attached(card).then(|| view.game.enchant_choices(card, controller))
    }
    fn entry_attachment_legal(&mut self, card: ObjId, controller: PlayerId, chosen: &Choices) -> bool {
        let view = self.entry_view(card, controller, chosen);
        !view.game.enters_attached(card)
            || chosen.attached_to.is_some_and(|t| view.game.can_enchant(card, controller, t))
    }
    /// Defenders are chosen using the entrant's completed prospective values (508.4).
    pub(crate) fn entry_defenders(&mut self,card:ObjId,controller:PlayerId,chosen:&Choices)->(PlayerId,Vec<Target>) {
        let view=self.entry_view(card,controller,chosen);
        let player=view.game.controller(card);
        let mut among=if view.game.is_a(card,CardType::Creature) && view.game.entering_attacking(player) {view.game.attack_recipients()} else {Vec::new()};
        // "attacking that player or a planeswalker they control": of those, what the effect allows
        if let Some(defending) = &chosen.defending { among.retain(|t| defending.allows(view.game, *t)); }
        (player,among)
    }
    pub(crate) fn entry_choice(&mut self, card: ObjId, controller: PlayerId, chosen: &Choices) -> Option<crate::replace::Entering> {
        let view = self.entry_view(card, controller, chosen);
        // Copy replacements precede other entry replacements (616.1c). Reinspect the
        // copied definition: its own as-enters choices have not been made (707.5-7).
        let copy = view.game.replacements(card).find_map(|(_, r)| {
            let crate::replace::Replacement::CopyEntry(effect) = r else { return None };
            if chosen.copies_applied.contains(effect) { return None; }
            let among = effect.from.members(view.game, &Ctx::new(controller, card), &crate::program::expr::NoLocals)
                .into_iter().filter_map(|t| match t { Target::Object(o) if o != card && view.game.obj(o).zone == Zone::Battlefield => Some(o), _ => None }).collect();
            Some(crate::replace::Entering { card, player: controller, asks: crate::replace::Asks::Copy { effect: effect.clone(), among } })
        });
        copy.or_else(|| view.game.enters_asking(card, controller, chosen))
    }
}

impl Drop for Viewing<'_> {
    fn drop(&mut self) {
        *self.game.obj_mut(self.on) = self.before.clone();
        self.game.prospective = self.previous;
    }
}

impl EntryEvent {
    pub(crate) fn new(game: &mut Game, request: EntryRequest) -> crate::Res<Self> {
        let unlocked = match &request {
            EntryRequest::Card { card, .. } => game.entering_unlocked(*card),
            EntryRequest::Token { .. } => [false; 2],
        };
        let (card, chars, owner, controller, chosen, tapped, transformed) = match &request {
            EntryRequest::Card { card, controller, chosen, .. } => {
                let old = game.obj(*card);
                let (chars, transformed) = game.entry_face(*card, chosen).expect("eligible entry has a permanent face");
                (old.card, chars, old.owner, *controller, chosen.clone(), false, transformed)
            }
            EntryRequest::Token { controller, values, tapped, chosen, .. } => (None, values.current().permanent_chars(game, unlocked), *controller, *controller, chosen.clone(), *tapped, values.transformed),
        };
        let chars = chosen.copied.as_ref().map(|v| v.permanent_chars(game, unlocked)).unwrap_or(chars);
        let on = if let EntryRequest::Token { prepared: Some(on), .. } = &request {
            game.obj_mut(*on).chars = chars;
            *on
        } else { game.new_object(card, chars, owner, controller, Zone::Gone) };
        if let EntryRequest::Card { card, .. } = &request { game.obj_mut(on).card_id = game.obj(*card).card_id; }
        game.obj_mut(on).copy_acquisition = chosen.copy_acquisition.clone();
        game.obj_mut(on).copied = chosen.copied.clone();
        game.obj_mut(on).transformed = transformed;
        game.obj_mut(on).unlocked = unlocked;
        if let Some(by) = chosen.face_down {
            game.obj_mut(on).face = crate::face::Face::Down(crate::face::Down { by });
            game.refresh_face(on);
        }
        // The permanent a spell becomes was kicked as the spell was: its abilities linked to
        // its kicker ask of it as it enters (607.2, 702.33e).
        if let EntryRequest::Card { card, .. } = &request && game.obj(*card).zone == Zone::Stack {
            game.obj_mut(on).kicked = game.obj(*card).kicked;
            game.obj_mut(on).alternative = game.obj(*card).alternative;
            // a bestowed Aura spell becomes a bestowed Aura (702.103b)
            game.obj_mut(on).bestowed = game.obj(*card).bestowed;
            game.obj_mut(on).spell_x = game.obj(*card).x;
            game.obj_mut(on).cast_by = game.obj(*card).cast_by;
            game.obj_mut(on).mana_paid = game.obj(*card).mana_paid;
            game.obj_mut(on).cast_exiled = game.obj(*card).cast_exiled.clone();
            // The reserved permanent needs its own abilities during replacement queries,
            // before the copied spell moves. Both faces survive that move (707.10g).
            game.obj_mut(on).token_faces = game.obj(*card).token_faces.clone();
            game.obj_mut(on).transformed = transformed;
        }
        let mut entry = Entry::default();
        entry.prepared = Some(on);
        entry.attached_to = chosen.attached_to;
        entry.attacking = chosen.attacking;
        entry.tapped = tapped;
        entry.values = chosen.values;
        let ctx = Ctx::new(controller, on);
        for arrival in &chosen.arrivals {
            arrival.checked_apply(game, &ctx, &mut entry)?;
        }
        entry.counters.extend(game.printed_loyalty(on));
        entry.counters.extend(game.saga_entry(on));
        Ok(Self { request, on, entry, question_kind: "entry".into(), question_inventory: Vec::new() })
    }

    pub(crate) fn was(&self) -> Option<ObjId> { match self.request { EntryRequest::Card { card, .. } => Some(card), _ => None } }

    /// The card players are told is entering. What enters face down is only the face-down
    /// object it will be: nothing names the hidden card it was (708.3, 701.40a).
    pub(crate) fn shown_entering(&self) -> Option<ObjId> {
        match &self.request {
            EntryRequest::Card { chosen, .. } if chosen.face_down.is_some() => Some(self.on),
            _ => self.was(),
        }
    }

    fn inspect<R>(&self, game: &mut Game, f: impl FnOnce(&Game) -> crate::Res<R>) -> crate::Res<R> {
        crate::operation::EntryInventory::new(&self.entry.counters)?;
        let controller = self.controller(game);
        let view = Viewing::new(game, self.on, self.was(), controller, &self.entry);
        f(view.game)
    }

    /// Validate every final entry inventory before the operation pays or moves anything.
    pub(crate) fn freeze(mut self, game: &mut Game) -> crate::Res<FrozenEntry> {
        let inventory = self.inspect(game, |view| {
            let counters = self.entry.counters.iter().filter(|(kind,_)|
                view.may_place(Target::Object(self.on),kind,view.controller(self.on))).cloned().collect::<Vec<_>>();
            crate::operation::EntryInventory::new(&counters)
        })?;
        self.entry.freeze_counters(inventory)?;
        Ok(FrozenEntry(self))
    }

    fn commit(self, game: &mut Game) -> Option<ObjId> {
        match self.request {
            EntryRequest::Card { card, controller, moved, .. } => {
                if moved { game.log.push(Event::Moved { object: card, to: Zone::Battlefield }); }
                let now = game.move_with(card, Zone::Battlefield, controller, self.entry);
                // a door it enters unlocked is unlocked by its controller (709.5h)
                if let Some(now) = now {
                    let unlocked = game.obj(now).unlocked;
                    for half in (0..2).filter(|h| unlocked[*h]) {
                        game.log.push(Event::Unlocked { object: now, half, player: controller });
                    }
                }
                // Its controller may look at a face-down permanent (708.5): what they saw
                // as it entered is theirs.
                if let Some(now) = now && game.face_down(now) {
                    let controller = game.controller(now);
                    game.disclose(crate::observation::Kind::Looked, controller, &[now]);
                }
                now
            }
            EntryRequest::Token { .. } => {
                game.obj_mut(self.on).zone = Zone::Battlefield;
                game.battlefield.push(self.on);
                game.record_control_entry(self.on);
                let attacking = self.entry.attacking;
                let inventory = self.entry.frozen_counters.clone();
                game.arrive(self.on, self.entry);
                let created = Event::Created { token: self.on };
                game.log.push(created.clone());
                if let Some(inventory) = inventory { inventory.install(game,self.on); }
                if let Some(defender) = attacking { game.enter_attacking(self.on, defender); }
                game.note(&created);
                Some(self.on)
            }
        }
    }
}

/// Only frozen entry events can be installed by the production operation.
pub(crate) struct FrozenEntry(EntryEvent);
impl FrozenEntry {
    pub(crate) fn was(&self) -> Option<ObjId> { self.0.was() }
    pub(crate) fn commit(self, game: &mut Game) -> Option<(ObjId, crate::operation::EntryInventory)> {
        let inventory = self.0.entry.frozen_counters.clone().expect("frozen entry inventory");
        self.0.commit(game).map(|now| (now, inventory))
    }
}

impl Replaced for EntryEvent {
    type Change = EntryChange;
    fn affected(&self) -> Target { Target::Object(self.on) }
    fn controller(&self, game: &Game) -> PlayerId { game.controller(self.on) }
    fn question(&self) -> (String, u32) {
        match self.question_inventory.as_slice() { [] => (self.question_kind.clone(),0), [only] => only.clone(), _ => (String::new(),0) }
    }
    fn counters(&self, game: &Game) -> Option<crate::operation::CounterProposal> {
        Some(crate::operation::CounterProposal { by: game.controller(self.on), inventory: self.question_inventory.clone() })
    }
    fn applicable(&mut self, game: &mut Game, applied: &[ReplacementId]) -> crate::Res<Vec<(ReplacementId, EntryChange)>> {
        let (found, inventory) = self.inspect(game, |view| {
            let mut found: Vec<_> = view.arrival_replacements(self.on, applied)?.into_iter()
                .map(|(id, tapped, prepared, counters)| (id, EntryChange::Arrival { tapped, prepared, counters })).collect();
            let mut inventory=std::collections::BTreeMap::<String,u32>::new();
            for (kind,count) in &self.entry.counters {
                if view.may_place(Target::Object(self.on),kind,view.controller(self.on)) {
                    let total = inventory.entry(kind.clone()).or_default();
                    *total = crate::operation::counter_add(*total,*count)?;
                }
            }
            let inventory:Vec<_>=inventory.into_iter().filter(|(_,n)|*n>0).collect();
            // Entry counters are produced by its intrinsic/other entry replacement effects
            // (including 714.3a), not the later turn-based Saga action (714.3c).
            found.extend(view.counter_replacements(self.on,view.controller(self.on),&inventory,true,applied).into_iter()
                // The admitted counter modifier patterns describe general subsets. An
                // entrant's own such ability is not in force yet (614.12); only
                // existing external sources, including their grants, are discovered.
                .filter(|(id,_)|id.source!=self.on)
                .map(|(id,(kinds,change))|(id,EntryChange::Counters{kinds,change})));
            Ok((found,inventory))
        })?;
        self.question_inventory=inventory;
        if let Some(kind) = self.entry.counters.first().map(|(k, _)| k.clone()).or_else(|| found.iter().find_map(|(_, change)| match change {
            EntryChange::Arrival { counters, .. } => counters.first().map(|(k, _)| k.clone()),
            EntryChange::Counters { kinds: CounterScope::Named(kind), .. } => Some(kind.clone()),
            EntryChange::Counters { kinds: CounterScope::EachProposed, .. } => None,
        })) { self.question_kind = kind; }
        Ok(found)
    }
    fn apply(&mut self, change: EntryChange) -> crate::Res<()> {
        match change {
            EntryChange::Arrival { tapped, prepared, counters } => {
                self.entry.tapped |= tapped;
                self.entry.enters_prepared |= prepared;
                self.entry.counters.extend(counters);
            }
            EntryChange::Counters { kinds, change } => {
                let mut inventory=std::collections::BTreeMap::<String,u32>::new();
                for (kind,count) in &self.entry.counters {
                    let total=inventory.entry(kind.clone()).or_default();
                    *total=crate::operation::counter_add(*total,*count)?;
                }
                for (kind,count) in &mut inventory {
                    if *count>0 && kinds.includes(kind) {*count=change.apply(*count)?;}
                }
                self.entry.counters=inventory.into_iter().collect();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardDef;

    #[test]
    fn r614_12_the_view_uses_own_and_existing_layers_without_making_an_entry_source() {
        // A targeted rules fixture: its own static ability affects its prospective
        // characteristics, but must not start pumping the existing Bear before entry.
        let defs = vec![
            CardDef::new("Prospective lord", "{G}", "Creature — Bear", "Creatures you control get +3/+3.", Some((1, 1))).unwrap(),
            CardDef::new("Existing anthem", "{G}", "Enchantment", "Creatures you control get +1/+1.", None).unwrap(),
            CardDef::new("Existing bear", "{G}", "Creature — Bear", "", Some((1, 1))).unwrap(),
        ];
        for def in &defs { crate::play::check(def).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        game.put(0, 1, Zone::Battlefield);
        let bear = game.put(0, 2, Zone::Battlefield);
        let spell = game.put(0, 0, Zone::Stack);
        let event = EntryEvent::new(&mut game, EntryRequest::card(spell, 0, Choices::default(), false)).unwrap();
        assert_eq!(game.power(bear),Some(2)); // retain ordinary world before apply/restore
        let battlefield = game.battlefield.clone();
        let log = game.log.clone();
        event.inspect(&mut game, |view| {
            assert_eq!(view.power(event.on), Some(5));
            assert_eq!(view.power(bear), Some(2));
            assert_eq!(view.battlefield, battlefield);
            assert_eq!(view.obj(spell).zone, Zone::Stack);
            Ok(())
        }).unwrap();
        assert_eq!(game.obj(event.on).zone, Zone::Gone);
        assert_eq!(game.battlefield, battlefield);
        assert_eq!(game.log, log);
        assert_eq!(game.prospective, None);
        assert_eq!(game.power(bear), Some(2));
    }

    #[test]
    fn r614_12_entry_overlay_invalidates_on_apply_restore_and_unwind() {
        let def = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        assert_eq!(g.controller(card),0); // cache ordinary card before overlay
        let before = format!("{g:?}");
        let old = g.layer_revision();
        let applied;
        {
            let view = Viewing::new(&mut g, card, Some(card), 1, &Entry::default());
            applied = view.game.layer_revision();
            assert!(applied > old);
            assert_eq!(view.game.obj(card).zone, Zone::Battlefield);
            assert_eq!(view.game.controller(card), 1);
        }
        assert!(g.layer_revision() > applied);
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(g.controller(card),0); // restore must drop the prospective answer
        let old = g.layer_revision();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let view = Viewing::new(&mut g, card, Some(card), 1, &Entry::default());
            assert_eq!(view.game.controller(card), 1);
            panic!("entry query fixture unwind");
        }));
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"entry query fixture unwind"));
        assert!(g.layer_revision() > old);
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(g.prospective, None);
        assert_eq!(g.obj(card).zone, Zone::Hand);
        assert_eq!(g.controller(card), 0);
    }

    #[test]
    fn r616_2_proposed_entry_counters_can_make_another_entry_effect_applicable() {
        let defs = vec![
            CardDef::new("Faithful Watchdog", "{G}{W}", "Creature — Dog", "Vigilance\nThis creature enters with three +1/+1 counters on it.", Some((0, 0))).unwrap(),
            CardDef::new("Power entry fixture", "{0}", "Enchantment", "Creatures with power 3 or greater enter with an additional +1/+1 counter on them.", None).unwrap(),
        ];
        for def in &defs { crate::play::check(def).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        let external = game.put(0, 1, Zone::Battlefield);
        let spell = game.put(0, 0, Zone::Stack);
        let mut event = EntryEvent::new(&mut game, EntryRequest::card(spell, 0, Choices::default(), false)).unwrap();
        let first = event.applicable(&mut game, &[]).unwrap();
        assert_eq!(first.len(), 1);
        let (own, change) = first.into_iter().next().unwrap();
        assert_eq!(own.source, event.on);
        event.apply(change).unwrap();
        let next = event.applicable(&mut game, &[own]).unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].0.source, external);
        assert!(game.obj(event.on).counters.is_empty(), "the proposed counters have not been put");
    }
}

#[cfg(test)]
mod prohibition_tests {
    use super::*;
    use crate::CardDef;

    fn game() -> Game {
        let defs = vec![
            CardDef::new("Grafdigger's Cage", "{1}", "Artifact", "Creature cards in graveyards and libraries can't enter the battlefield.\nPlayers can't cast spells from graveyards or libraries.", None).unwrap(),
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Entry choice witness", "{0}", "Creature — Bear", "As this creature enters, choose a color.", Some((2, 2))).unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None).unwrap(),
        ];
        for def in &defs { crate::play::check(def).unwrap(); }
        Game::new(defs, 3, 20)
    }

    #[test]
    fn r614_17c_prohibited_source_zone_entries_do_not_ask_replacements_or_move_cards() {
        let mut game = game();
        game.put(1, 0, Zone::Battlefield);
        let mut requests = Vec::new();
        let mut blocked = Vec::new();
        let mut allowed = Vec::new();
        for owner in 0..3 {
            for zone in [Zone::Graveyard, Zone::Library] {
                for def in [1, 2] {
                    let card = game.put(owner, def, zone);
                    blocked.push((card, zone));
                    requests.push(EntryRequest::card(card, (owner + 1) % 3, Choices::default(), true));
                }
                let land = game.put(owner, 3, zone);
                allowed.push(land);
                requests.push(EntryRequest::card(land, (owner + 1) % 3, Choices::default(), true));
            }
            let hand = game.put(owner, 1, Zone::Hand);
            allowed.push(hand);
            requests.push(EntryRequest::card(hand, owner, Choices::default(), true));
        }
        let before = game.log.len();
        let result = game.prepare_entry_batch(requests, vec![]).unwrap().expect("prohibited entrants must not ask entry choices");
        assert_eq!(result.arrived.len(), allowed.len());
        for (card, zone) in blocked {
            assert_eq!(game.obj(card).zone, zone);
            assert_eq!(game.became(card), None);
            assert!(!game.log.iter().skip(before).any(|e| matches!(e,
                Event::Moved { object, .. } if *object == card
            )));
        }
        for card in allowed { assert!(result.transitions.iter().any(|(was, _)| *was == card)); }
    }

    #[test]
    fn r614_17a_a_prohibition_departing_with_the_batch_still_stops_entry() {
        let mut game = game();
        let cage = game.put(1, 0, Zone::Battlefield);
        let bear = game.put(2, 1, Zone::Graveyard);
        let result = game.prepare_entry_batch(vec![EntryRequest::card(bear, 0, Choices::default(), true)], vec![(cage, Zone::Exile)]).unwrap().unwrap();
        assert!(result.arrived.is_empty());
        assert_eq!(game.obj(bear).zone, Zone::Graveyard);
        assert!(game.battlefield.is_empty());
        let result = game.prepare_entry_batch(vec![EntryRequest::card(bear, 0, Choices::default(), true)], vec![]).unwrap().unwrap();
        assert_eq!(result.arrived.len(), 1, "a subsequent event sees the prohibition gone");
        assert_eq!(game.obj(result.arrived[0]).owner, 2);
        assert_eq!(game.controller(result.arrived[0]), 0);
    }

    #[test]
    fn r601_3_origin_queries_cover_each_library_and_graveyard_independently_of_the_caster() {
        let mut game = game();
        game.put(0, 0, Zone::Battlefield);
        for owner in 0..3 {
            for zone in [Zone::Library, Zone::Graveyard, Zone::Hand, Zone::Exile, Zone::Stack] {
                let card = game.put(owner, 1, zone);
                for caster in 0..3 {
                    let cast = crate::trigger::Fact::new(ir::Verb::Cast, Some(Target::Player(caster)), vec![(ir::Role::What, Target::Object(card))]);
                    assert_eq!(game.forbidden(&cast), matches!(zone, Zone::Library | Zone::Graveyard));
                }
            }
        }
    }
}

#[cfg(test)]
mod frozen_counter_tests {
    use super::*;
    #[test]
    fn r603_10_entry_counter_snapshots_include_all_arrivals_and_departures() {
        // CR122.6, 603.10, 608.2i: token and card entries share the same completed
        // event boundary. A simultaneous departing anthem is no longer in force.
        for tokens in [false, true] {
            for reverse in [false, true] {
                let defs = vec![
                    crate::CardDef::new("Elf", "{0}", "Creature — Elf", "This creature enters with a +1/+1 counter on it.", Some((2,2))).unwrap(),
                    crate::CardDef::new("Lord", "{0}", "Creature — Elf", "This creature enters with a +1/+1 counter on it.\nOther Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2,2))).unwrap(),
                    crate::CardDef::new("Anthem", "{0}", "Enchantment", "Elf creatures you control get +1/+1.", None).unwrap(),
                ];
                for d in &defs { crate::play::check(d).unwrap(); }
                let mut game = Game::new(defs, 2, 20);
                let elf = game.put(1,0,Zone::Hand);
                let lord = game.put(1,1,Zone::Hand);
                let anthem = game.put(0,2,Zone::Battlefield);
                let mut cards = vec![elf,lord];
                if reverse { cards.reverse(); }
                let entries = cards.into_iter().map(|card| if tokens {
                    EntryRequest::token(0,game.token_copy(card),false)
                } else {
                    EntryRequest::card(card,0,Choices::default(),true)
                }).collect();
                let result = game.prepare_entry_batch(entries,vec![(anthem,Zone::Exile)]).unwrap().unwrap();
                assert_eq!(result.arrived.len(),2);
                let snapshots:Vec<_> = game.log.iter().filter_map(|e| match e {
                    Event::CountersPut { on:Target::Object(on), by:0, recipient:Some(seen), .. } => Some((*on,seen)),
                    _ => None,
                }).collect();
                assert_eq!(snapshots.len(),2);
                for (on,seen) in snapshots {
                    let expected = if game.obj(on).chars.name=="Elf" { (4,4) } else { (3,3) };
                    assert_eq!(seen.pt,Some(expected));
                    assert_eq!(seen.controller,0);
                    assert_eq!(seen.pt,game.seen(on,None).pt);
                }
            }
        }
    }

    #[test]
    fn r614_17d_simultaneous_external_ban_does_not_change_frozen_entry_inventory() {
        for reverse in [false,true] {
            let defs=vec![
                crate::CardDef::new("Ban","{0}","Enchantment","Creatures can't have counters put on them.",None).unwrap(),
                crate::CardDef::new("Entrant","{0}","Creature","This creature enters with a +1/+1 counter on it.",Some((1,1))).unwrap(),
            ];
            for d in &defs{crate::play::check(d).unwrap();}
            let mut game=Game::new(defs,2,20);
            let ban=game.put(0,0,Zone::Hand);let creature=game.put(0,1,Zone::Hand);
            let mut cards=vec![ban,creature];if reverse{cards.reverse();}
            let entries=cards.into_iter().map(|card|EntryRequest::card(card,0,Choices::default(),true)).collect();
            let result=game.prepare_entry_batch(entries,vec![]).unwrap().unwrap();
            let entrant=result.transitions.iter().find(|(was,_)|*was==creature).unwrap().1;
            assert_eq!(game.obj(entrant).counters["+1/+1"],1);
            assert!(!game.may_place(entrant.into(),"+1/+1",0),"the ban applies only after this entry event");
        }
    }
}
