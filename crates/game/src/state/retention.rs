//! Reclaim departed payloads only at a quiescent rules boundary. A suspended procedure
//! conservatively keeps the entire arena; its frames/operations are not partially traced.
use super::*;
use crate::retention::Trace;

impl Game {
    /// Reclaim unreferenced departed incarnations after event delivery/retirement. Current
    /// events still retain their participants. No collection happens while any procedure
    /// or prospective-entry query is active, even if a procedure is waiting for a player.
    /// Hosts must materialize observations before calling this; an external event recorder
    /// owns its payloads rather than references into the authoritative object arena.
    pub fn collect_departed(&mut self) -> usize {
        if !self.procedures.is_empty() || self.prospective.is_some() { return 0; }
        // Conditional effect sources must remain live; ended durations are not roots.
        self.end_control_durations();
        let mut marked = vec![false; self.objects.slots()];
        let mut pending = Vec::new();
        // A linked exile row stops being useful as soon as that incarnation leaves exile.
        // The source alone is not a root: otherwise orphaned relationships retain history.
        {
            let state = self.state_mut();
            state.exiled_with.retain(|(_, _, card, _)| state.objects.get(*card).is_some_and(|o| o.zone == Zone::Exile));
        }
        self.trace_roots(&mut |id| pending.push(id));
        while let Some(id) = pending.pop() {
            let object = self.obj(id); // every owning edge must still be valid
            if marked[id.slot()] { continue; }
            marked[id.slot()] = true;
            object.trace(&mut |next| pending.push(next));
            for (_, _, card, _) in self.exiled_with.iter().filter(|(source, _, _, _)| *source == id) {
                pending.push(*card);
            }
        }
        let state = self.state_mut();
        state.linked_acquisitions.retain(|a| state.objects.get(a.source).is_some() && marked[a.source.slot()]);
        state.exiled_with.retain(|(source, _, _, _)| state.objects.get(*source).is_some() && marked[source.slot()]);
        // 400.7k: kept while something still refers to the discarded card and the object it
        // became is where it went. Neither end is a root: the link keeps nothing alive.
        state.madness_found.retain(|(was, now)| state.objects.get(*was).is_some() && marked[was.slot()]
            && state.objects.get(*now).is_some_and(|o| o.zone != Zone::Gone));
        // Resolved characteristic effects query membership by identity. Once an object
        // departs, its frozen Seen owns the LKI; no future incarnation inherits the effect.
        for effect in &mut state.continuous {
            effect.affected.retain(|id| state.objects.get(*id).is_some_and(|o| o.zone != Zone::Gone));
        }
        state.continuous.retain(|effect| !effect.affected.is_empty());
        state.prevention.retain(|e| !matches!(&e.to, Some(crate::statement::Affected::Things(ts))
            if ts.iter().all(|t| matches!(t, crate::Target::Object(o) if !state.objects.get(*o).is_some_and(|o|o.zone==Zone::Battlefield)))));
        state.shields.retain(|id| state.objects.get(*id).is_some_and(|o| o.zone == Zone::Battlefield));
        // Used static permissions are identity labels, not roots or historical readers.
        state.cast_permission_uses.retain(|u|state.objects.get(u.source).is_some_and(|o|o.zone==Zone::Battlefield));
        // A permission is about the card as it is in exile (400.7); it keeps nothing alive.
        state.permissions.retain(|p| state.objects.get(p.object).is_some_and(|o| o.zone == Zone::Exile));
        // a card discarded this turn is asked about while it is in the graveyard (702.187b)
        state.this_turn.discarded.retain(|c| state.objects.get(*c).is_some_and(|o| o.zone == Zone::Graveyard));
        // what only the turn's history refers to leaves the game's objects as it would were
        // nothing noted, and the history keeps it as it last was (608.2h)
        let mut pending = Vec::new();
        state.this_turn.trace_noted(&mut |id| pending.push(id));
        while let Some(id) = pending.pop() {
            let Some(object) = state.objects.get(id) else { continue };
            if marked[id.slot()] || state.this_turn.archived(id).is_some() { continue; }
            object.trace(&mut |next| pending.push(next));
            let object = object.clone();
            state.this_turn.archive(object);
        }
        state.objects.sweep(&marked)
    }

    fn trace_roots(&self, visit: &mut dyn FnMut(ObjId)) {
        // Keep this inventory exhaustive: a new retained field needs an explicit ownership
        // decision here. Procedures/prospective state are guarded before this function.
        let super::GameState { objects, players, battlefield, exile, command, stack, until, combat, lasting, log,
            triggered, delayed, decision, this_turn, lookbacks, completed_choices,
            defs: _, active: _, turn_schedule: _, starting_life: _, part: _, turn: _, monarch: _, another_cleanup: _, parts_begun: _,
            // the turn plan names phases and players, never objects; a skip effect keeps the
            // object whose effect it is, by which it is named (614.10, 616.1)
            plan: _, skips, skip_ids: _, ask_forced: _,
            // simultaneous timestamp ranges name timestamps, never objects
            shields: _, prevention, continuous, clock: _, simultaneous: _, activated: _, activation_started: _, active_mana_abilities: _, procedures: _, priority: _,
            // Replay outcomes retain physical CardIds, never incarnation handles.
            passes: _, observations: _, seen: _, ordered: _, together: _, random: _, captured_random: _, replay_random: _, opening: _,
            prospective: _, linked_acquisitions: _, exiled_with: _, madness_found: _, permissions: _, cast_permission_uses: _, ability_uses: _, companions: _, commanders: _, next_card: _, last_sba: _ } = self.state_view();
        // Zone membership is authoritative, including fixture objects not in a zone list.
        for object in objects.iter().filter(|o| o.zone != Zone::Gone) { visit(object.id); }
        battlefield.trace(visit);
        exile.trace(visit);
        command.trace(visit);
        stack.trace(visit);
        players.trace(visit);
        until.trace(visit);
        combat.trace(visit);
        lasting.trace(visit);
        prevention.trace(visit);
        // Causal readback retains the resolved source incarnation independently
        // of wherever its physical card went (113.7a, 611.2a).
        for effect in continuous {if let Some(producer)=&effect.producer {visit(producer.source);}}
        for event in log.iter() { event.trace(visit); }
        triggered.trace(visit);
        delayed.trace(visit);
        decision.trace(visit);
        completed_choices.trace(visit);
        this_turn.trace(visit);
        for skip in skips { if let Some(source) = skip.source { visit(source); } }
        for (_, _, before) in lookbacks {
            for id in before.battlefield().expect("a look-back view freezes membership") { visit(*id); }
        }
        // activated and Grant.by are identity labels. Continuous.affected and shields
        // refer only to live-zone membership and are pruned, not payload roots.
    }
}

impl Trace for Object {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { attached_to, targets, ability, invocations, bound, choices, cast_exiled, hidden, harmonized, cast_announcement, foretold: _, delayed: _,
            cast_as: _, id: _, card: _, card_id: _, chars: _, copy_acquisition: _, copied: _, copy_effects: _, token_faces: _, card_copy: _, stack_copy_of, face: _, emblem: _, owner: _, controller: _, zone: _,
            tapped: _, damage: _, deathtouched: _, counters: _, counter_stamps: _, since: _, stamp: _, last: _, last_attachments: _,
            flashback: _, cast_rider: _, transformed: _, transformed_at: _, level: _, unlocked: _, monstrous: _, renowned: _, solved: _, prepared: _, prepared_copy, prepare_source, saddled: _, cast_from: _, cast_by: _, entered: _, activation_uses: _, x: _, spell_x: _, kicked: _, optional_paid: _, alternative: _, buyback_paid: _, entwined: _, replicated: _, gift: _, announced: _, left: _, arrived: _, successor: _, uncounterable: _, mana_paid: _, modes: _, division: _ } = self;
        if let Some(announcement) = cast_announcement {
            if let crate::announcement::CastRoute::Harmonize { tapped: Some(object) } = announcement.route { visit(object); }
            // Permission owners are frozen values; their source is an identity label.
            if let Some(payment) = &announcement.payment {
                for (object, _) in &payment.convoke { visit(*object); }
                for object in &payment.delve { visit(*object); }
            }
        }
        stack_copy_of.trace(visit);
        prepared_copy.trace(visit);
        prepare_source.trace(visit);
        attached_to.trace(visit);
        targets.trace(visit);
        // the creature chosen to tap for harmonize, while the spell is announced (702.180b)
        if let Some(creature) = harmonized { visit(*creature); }
        if let Some((source, _)) = ability { source.trace(visit); }
        if let Some(Ok(invocations)) = invocations { invocations.trace(visit); }
        bound.trace(visit);
        for (_, card) in cast_exiled { card.trace(visit); }
        choices.trace(visit);
        // the permanent whose controller may look at a card exiled face down (702.75a)
        if let Some(watcher) = hidden.as_ref().and_then(|h| h.watcher) { visit(watcher); }
        // copy_acquisition retains immutable values and a historical donor handle only;
        // no rules query follows that handle. It is not a payload retention edge.
        // token_faces and copied hold immutable values/definitions, never source handles.
        // last holds copied characteristics and immutable granted definitions. Its grant
        // provenance is identity only. The incidental successor metadata is not an owning
        // edge: pending events or captured Bound retain permitted transition endpoints.
    }
}

impl Trace for Bound {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { inputs, became, revealed, cost_exiled, crew, linked_ability: _, event_amount: _ } = self;
        revealed.trace(visit);
        if let Some(crew) = crew { crew.trace(visit); }
        for (_, subjects) in inputs { subjects.trace(visit); }
        for (_, card) in cost_exiled { card.trace(visit); }
        for (was, now) in became { was.trace(visit); now.trace(visit); }
    }
}
impl Trace for Triggered {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { source, bound, captured, controller: _, origin: _, index: _, delayed: _ } = self;
        source.trace(visit);
        bound.trace(visit);
        if let Some(Ok(captured)) = captured { captured.trace(visit); }
    }
}
impl Trace for Delayed {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { source, subjects, captured, bound, controller: _, pattern: _, lasts: _, once: _ } = self;
        source.trace(visit);
        subjects.trace(visit);
        captured.trace(visit);
        bound.trace(visit);
    }
}
impl Trace for Player {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { library, hand, graveyard, pool, counters: _, life: _, land_played: _, turn_began: _, turns_taken: _, drew_from_empty: _, lost: _, speed: _ } = self;
        library.trace(visit);
        hand.trace(visit);
        graveyard.trace(visit);
        pool.trace(visit);
    }
}
impl Trace for crate::legal::Step {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        self.source.trace(visit); self.paying.trace(visit); self.with.trace(visit); self.plan.trace(visit);
    }
}
impl Trace for Pool {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { restricted, colors: _, colorless: _ } = self;
        restricted.trace(visit);
    }
}
impl Trace for Restricted {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { source, ability: _, provenance: _, mana: _, only: _, uncounterable: _, controller: _, trigger, kept: _ } = self;
        source.trace(visit);
        if let Some(t)=trigger {t.captured.trace(visit);t.bound.trace(visit);}
    }
}
impl Trace for Decision {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Decision::Targets { ability, .. } | Decision::Modes { ability, .. } => ability.trace(visit),
            Decision::Order { abilities, .. } => abilities.trace(visit),
            Decision::Divide { attacker, among, .. } => { attacker.trace(visit); among.trace(visit); },
            Decision::Choose { choice, .. } => choice.trace(visit),
            Decision::Payment { source, .. } => source.trace(visit),
            Decision::Discard { .. } | Decision::Mulligan { .. } | Decision::Bottom { .. }
            | Decision::Attackers { .. } | Decision::Blockers { .. } => {},
        }
    }
}
impl Trace for Choice {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Choice::ChangeTargets { object, original, slots, .. } => { object.trace(visit); original.trace(visit); for slot in slots { slot.among.trace(visit); if let Some(d)=&slot.dependent { for (target, among) in &d.options { target.trace(visit); among.trace(visit); } } } },
            Choice::Retarget { copy, original, slots } => { copy.trace(visit); original.trace(visit); for slot in slots { slot.among.trace(visit); } },
            Choice::Holders { among, .. } | Choice::Defender { among, .. } => among.trace(visit),
            Choice::ConstrainedObjects { among, .. } | Choice::Select { among, .. } => among.trace(visit),
            Choice::CommandZone { object } => object.trace(visit),
            Choice::Search { cards, among, .. } => { cards.trace(visit); among.trace(visit); },
            Choice::Attach { aura, among } => { aura.trace(visit); among.trace(visit); },
            Choice::Objects { among, .. } | Choice::OptionalObjects { among, .. } => among.trace(visit),
            Choice::OrderCards { cards, .. } | Choice::Arrange { cards, .. } | Choice::Cast { cards } => cards.trace(visit),
            Choice::SpellDestination{on,..}=>on.trace(visit),
            Choice::DamageOrder{on,hits,among}=> {on.trace(visit);for h in hits {h.source.trace(visit);}for o in among {o.trace(visit);}},
            Choice::Prevention{on,shield,hits,..}=>{on.trace(visit);shield.source.trace(visit);for h in hits {h.source.trace(visit);}},
            Choice::DamageReplacement { on, hits, among } => {
                on.trace(visit);
                for hit in hits { hit.source.trace(visit); }
                for offer in among { offer.effect.source.trace(visit); offer.source.trace(visit); }
            },
            Choice::Replacement { on, among, .. } => {
                on.trace(visit);
                for replacement in among { replacement.source.trace(visit); }
            },
            Choice::Pay { waterbend, .. } => {
                if let Some(bend) = waterbend { bend.among.trace(visit); }
            },
            Choice::Pile { piles } => { for pile in piles { pile.trace(visit); } },
            Choice::Alternative { among, .. } => { for option in among { match &option.body { super::ActionOptionBody::Objects { among, .. } => among.trace(visit), super::ActionOptionBody::Counters { on,.. } => on.trace(visit), _ => {} } } },
            Choice::LibraryDestination { cards, .. } => cards.trace(visit),
            Choice::May { .. } | Choice::Mana { .. } | Choice::Value(_) | Choice::Name(_) | Choice::Number { .. } | Choice::Player { .. } | Choice::Skip { .. } => {},
        }
    }
}
impl Trace for PublicChoice {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Self::OrderCards { cards, .. } | Self::LibraryDestination { cards, .. } => cards.trace(visit),
            Self::Objects { objects, .. } => objects.trace(visit),
            Self::Alternative { source, .. } => source.trace(visit),
            Self::Value { source, value, .. } => { source.trace(visit); value.trace(visit); },
            Self::SpellDestination { on, .. } => on.trace(visit),
            Self::Entry { card, answer, .. } => { card.trace(visit); answer.trace(visit); },
            Self::DamageOrder { on, offer, .. } => { on.trace(visit); offer.trace(visit); },
            Self::Prevention { on, shield, .. } => { on.trace(visit); shield.source.trace(visit); },
            Self::DamageReplacement { on, offer, .. } => { on.trace(visit); offer.effect.source.trace(visit); offer.source.trace(visit); },
            Self::Replacement { on, effect, .. } => { on.trace(visit); effect.source.trace(visit); },
        }
    }
}
impl Trace for Answer {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Answer::Targets(targets) => targets.trace(visit),
            Answer::Holders(holders) => holders.trace(visit),
            Answer::Attach(target) | Answer::Defender(target) => target.trace(visit),
            Answer::Objects(objects) => objects.trace(visit),
            Answer::OptionalObjects(objects) => { if let Some(objects) = objects { objects.trace(visit); } },
            Answer::Value(value) => value.trace(visit),
            Answer::Arrange { top, away } => { top.trace(visit); away.trace(visit); },
            Answer::Replacement(replacement) => replacement.source.trace(visit),
            Answer::SpellDestination{on,..}=>on.trace(visit),
            Answer::DamageOrder(o)=>o.trace(visit),
            Answer::Prevention(_)=>{},
            Answer::DamageReplacement(offer) => {offer.effect.source.trace(visit);offer.source.trace(visit);},
            Answer::Pay { plan, with, tapped } => {
                plan.trace(visit);
                tapped.trace(visit);
                with.trace(visit);
            },
            Answer::LibraryPosition(_) | Answer::Alternative(_) | Answer::Yes | Answer::No | Answer::Mana(_) | Answer::Player(_) | Answer::Pile(_) | Answer::Skip(_) => {},
        }
    }
}
impl crate::retention::Players for Answer {
    fn players(&self, visit: &mut dyn FnMut(PlayerId)) {
        match self {
            Answer::Targets(targets) => targets.players(visit),
            Answer::Holders(holders) => holders.players(visit),
            Answer::Attach(target) | Answer::Defender(target) => target.players(visit),
            Answer::Player(player) => visit(*player),
            Answer::Value(value) => value.players(visit),
            Answer::LibraryPosition(_) | Answer::SpellDestination { .. } | Answer::Alternative(_) | Answer::Yes | Answer::No
            | Answer::Objects(_) | Answer::OptionalObjects(_) | Answer::Mana(_) | Answer::Pile(_) | Answer::Arrange { .. }
            | Answer::Replacement(_) | Answer::DamageReplacement(_) | Answer::DamageOrder(_) | Answer::Prevention(_)
            | Answer::Pay { .. } | Answer::Skip(_) => {},
        }
    }
}

#[cfg(test)]
mod tests;
