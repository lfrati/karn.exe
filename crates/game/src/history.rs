//! What happened earlier in the turn, or in the turn before, for the conditions that look
//! back (608.2i): "if a creature died this turn", "if you've cast another spell this turn",
//! "if no spells were cast last turn".
//!
//! The log is not kept for this: its events retire once they are delivered. Each event is
//! noted once, in the terms a trigger sees it in, as triggers are checked for it (603.2), and
//! one not yet checked is read from the log: what was done earlier in the same instruction
//! has happened. Every event is noted when a card of the game looks back at what happened
//! ([`asks_back`]); else only deaths and casts are. As a turn begins, what the turn before it noted of the kinds in
//! [`KEPT_A_TURN`] becomes "last turn" and the rest is forgotten, so what is kept is bounded
//! by what happens in two turns.
use crate::event::Event;
use crate::retention::Trace;
use crate::state::{Game, ObjId};
use crate::trigger::{Fact, Pattern};
use crate::select::Ctx;
use ir::Verb;

/// What is still asked about in the turn after: the spells that were cast.
pub(crate) const KEPT_A_TURN: [Verb; 1] = [Verb::Cast];
/// What is noted in a game none of whose cards looks back at anything else ([`asks_back`]):
/// what is kept is what can be asked, and nothing else is kept alive by it.
const ALWAYS: [Verb; 3] = [Verb::Die, Verb::Cast, Verb::CommitCrime];

/// Does any of these cards look back at what happened in a turn: "if you attacked this turn",
/// "creatures that died this turn", "for the first time each turn"? Read from the text as
/// written, wherever it is (a granted ability, a token's), not as lowered.
pub(crate) fn asks_back(defs: &[crate::card::CardDef]) -> bool {
    fn walk(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Array(items) => items.iter().any(walk),
            serde_json::Value::Object(fields) => {
                let window = |w: Option<&serde_json::Value>| w.is_some_and(|w| matches!(w.as_str(), Some("ThisTurn" | "LastTurn")));
                fields.get("nth_each_turn").is_some_and(|n| !n.is_null())
                    || fields.get("Happened").is_some_and(|h| window(h.get("window")))
                    || fields.get("Did").is_some_and(|d| window(d.get("when")))
                    || fields.values().any(walk)
            }
            _ => false,
        }
    }
    fn of(def: &crate::card::CardDef) -> bool {
        serde_json::to_value((&def.abilities, &def.keyword_abilities, &def.modes)).is_ok_and(|v| walk(&v))
            || def.halves.iter().any(of) || def.back.as_deref().is_some_and(of) || def.alternative_spell().is_some_and(of)
    }
    defs.iter().any(of)
}

/// The constructor's exact cast inventory establishes plain cast ordinals.
/// Queries about other event kinds or cast attributes remain outside this profile.
pub(crate) fn initial_cast_history_needs_more(defs:&[crate::CardDef]) -> bool {
    fn walk(value:&serde_json::Value)->bool {
        match value {
            serde_json::Value::Array(items)=>items.iter().any(walk),
            serde_json::Value::Object(fields)=> {
                let prior=|value:Option<&serde_json::Value>|value.is_some_and(|value|matches!(value.as_str(),Some("ThisTurn"|"LastTurn")));
                fields.get("nth_each_turn").is_some_and(|value|!value.is_null())
                    || ["NthResolution","NthCombatPhase","NthCastThisTurn","DealtDamageBy"].iter().any(|key|fields.contains_key(*key))
                    || prior(fields.get("when")) || prior(fields.get("window"))
                    || fields.get("cast").is_some_and(|cast|cast.get("this_turn")==Some(&serde_json::Value::Bool(true)) || cast.get("from").is_some_and(|from|!from.is_null()))
                    || fields.get("Happened").is_some_and(|value|prior(value.get("window")))
                    || fields.get("Did").is_some_and(|value|prior(value.get("when")))
                    || fields.values().any(walk)
            },
            _=>false,
        }
    }
    fn needs(ability:&ir::Ability)->bool {
        if let ir::Ability::Prefixed {ability:inner,..}=ability {return needs(inner);}
        if let ir::Ability::Triggered {trigger:ir::Trigger::Event{pattern,..},effect}=ability {
            let plain=pattern.nth_each_turn.is_some() && pattern.condition.is_none() && pattern.cause.is_none()
                && matches!(pattern.subjects.as_slice(),[who] if who.quant==ir::Quant::Definite(ir::Ref::You))
                && matches!(pattern.actions.as_slice(),[ir::event::EventAction::Deed(deed)]
                    if deed.verb==Verb::Cast && !deed.passive
                    && matches!(deed.args.as_slice(),[(ir::lexicon::Role::What,ir::event::Arg::Noun(noun))]
                        if noun.set==ir::ObjectSet::of(ir::Domain::Spell)));
            if plain {return serde_json::to_value(effect).is_ok_and(|value|walk(&value));}
        }
        serde_json::to_value(ability).is_ok_and(|value|walk(&value))
    }
    fn of(def:&crate::CardDef)->bool {
        def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_,ability)|ability)).any(needs)
            || serde_json::to_value(&def.modes).is_ok_and(|value|walk(&value))
            || def.halves.iter().any(of) || def.back.as_deref().is_some_and(of) || def.alternative_spell().is_some_and(of)
    }
    defs.iter().any(of)
}

/// Which turn a condition looks back over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Turn { This, Last }

/// What happened this turn, in order, and what of the turn before is still asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThisTurn {
    now: Vec<Fact>, last: Vec<Fact>,
    /// The events before this are in `now`.
    noted: crate::event::Seq,
    /// Some card of the game looks back at more than deaths and casts ([`asks_back`]): every
    /// event is noted.
    pub(crate) every: bool,
    /// Each creature declared as an attacker this turn, once however many combats it attacked
    /// in, and the player who declared it (508.1): "if you attacked with three or more
    /// creatures this turn".
    declared: Vec<(crate::PlayerId, ObjId, crate::layers::Seen)>,
    // Event-time type counts, bounded by seats × card types. No object/history retention.
    entries: Vec<(crate::PlayerId, ir::CardType, usize)>,
    draws: Vec<u32>,
    life_lost: Vec<u32>,
    departures: Vec<usize>,
    /// Which source dealt damage to what, once each and whether in combat: "dealt damage by ~
    /// this turn", "that was dealt combat damage this turn".
    damaged: Vec<(ObjId, crate::event::Target, bool)>,
    /// The creatures declared as attackers and the players who declared them, and the creatures
    /// declared as blockers: "creatures that attacked this turn" (508.1), "target player who
    /// attacked this turn", "target creature that blocked this turn" (509.1). A creature put
    /// onto the battlefield attacking or blocking never attacked or blocked (508.4, 509.4).
    attacked: Vec<crate::event::Target>,
    blocked: Vec<ObjId>,
    /// Each creature tapped to pay a saddle (true) or crew (false) cost this turn, and the
    /// permanent it saddled or crewed: "creature that saddled it this turn" (702.171c,
    /// 702.122b).
    tapped_for: Vec<(bool, ObjId, ObjId)>,
    /// The cards discarded this turn, as they are where they went (400.7): "if you discarded it
    /// this turn" (702.187b). Each is kept only while it is in a graveyard.
    pub(crate) discarded: Vec<ObjId>,
    /// What only this turn's other entries still refer to, once it is gone from the game:
    /// described as it last was (608.2h), and kept out of the game's own objects, so that
    /// what is noted changes nothing else about the game, not even which handles the objects
    /// made later get.
    archive: Archive,
    /// The active player's completed draws in the current draw step.
    draw_step: u32,
    /// How many times each object became the target of a spell or ability each player
    /// controls: "for the first time each turn" (Valiant).
    targeted: Vec<(crate::event::Target, crate::PlayerId, u32)>,
}

impl ThisTurn {
    fn of(&self, turn: Turn) -> &[Fact] {
        match turn { Turn::This => &self.now, Turn::Last => &self.last }
    }

    /// The objects the entries kept only to be asked about refer to: those of the events
    /// other than deaths and casts, and the creatures declared as attackers. They keep
    /// nothing in the game ([`Trace`]); what is gone of them is archived.
    pub(crate) fn trace_noted(&self, visit: &mut dyn FnMut(ObjId)) {
        for fact in self.now.iter().chain(&self.last).filter(|f| !ALWAYS.contains(&f.verb())) { fact.trace(visit); }
        for (_, attacker, _) in &self.declared { attacker.trace(visit); }
    }

    /// Keep this object, gone from the game, as it last was.
    pub(crate) fn archive(&mut self, object: crate::state::Object) {
        if self.archived(object.id).is_none() { self.archive.0.push(object); }
    }

    /// What this object, gone from the game, last was, if this turn's entries kept it.
    pub(crate) fn archived(&self, id: ObjId) -> Option<&crate::state::Object> {
        self.archive.0.iter().find(|o| o.id == id)
    }
}

/// Objects no longer in the game, equal as the same objects.
#[derive(Debug, Clone, Default)]
struct Archive(Vec<crate::state::Object>);
impl PartialEq for Archive {
    fn eq(&self, other: &Self) -> bool { self.0.iter().map(|o| o.id).eq(other.0.iter().map(|o| o.id)) }
}
impl Eq for Archive {}

impl Trace for ThisTurn {
    // What died or was cast is described as it last was (608.2h): it is kept until the turn
    // is over. What else was noted keeps nothing ([`ThisTurn::trace_noted`]).
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        for fact in self.now.iter().chain(&self.last).filter(|f| ALWAYS.contains(&f.verb())) { fact.trace(visit); }
        for (source, to, _) in &self.damaged { source.trace(visit); to.trace(visit); }
        for attacker in &self.attacked { attacker.trace(visit); }
        for blocker in &self.blocked { blocker.trace(visit); }
        for (_, creature, permanent) in &self.tapped_for { creature.trace(visit); permanent.trace(visit); }
    }
}

impl Game {
    /// A complete declared inventory of current-turn casts, not replayed cast
    /// events. Frozen spell incarnations support later rule lookback (608.2h).
    pub fn set_initial_cast_history(&mut self, casts:&[(ObjId,crate::PlayerId)]) -> Result<(),String> {
        if !self.this_turn.now.is_empty() || !self.this_turn.last.is_empty() {
            return Err("initial cast inventory requires empty tracked history".into());
        }
        let mut seen=std::collections::BTreeSet::new();
        for (source,player) in casts {
            if *player>=self.players.len() || !seen.insert(*source) || self.get_object(*source).is_none_or(|object|
                object.zone!=crate::Zone::Gone || object.left!=Some(crate::Zone::Stack) || object.card.is_none()) {
                return Err("initial cast inventory requires unique frozen spell incarnations and valid casters".into());
            }
            if self.def(*source).is_some_and(|definition|definition.spell_x()) {
                return Err("initial cast inventory requires unrepresented historical X".into());
            }
        }
        self.turn_history_mut().now=casts.iter().map(|(source,player)|
            Fact::new(ir::Verb::Cast,Some(crate::Target::Player(*player)),vec![(ir::lexicon::Role::What,crate::Target::Object(*source))]).remembered()).collect();
        Ok(())
    }

    pub fn audit_casts_this_turn(&self) -> serde_json::Value {
        use crate::event::Target;
        let casts:Vec<_>=self.this_turn.now.iter().filter_map(|fact| {
            if fact.verb()!=ir::Verb::Cast {return None;}
            match (fact.doer(),fact.role(ir::lexicon::Role::What)) {
                (Some(Target::Player(player)),Some(Target::Object(source)))=>Some(serde_json::json!({"source":source.0,"player":player})),
                _=>None,
            }
        }).collect();
        serde_json::json!(casts)
    }

    /// Note this event, just put in the log, if a condition could ask about it later.
    pub(crate) fn note(&mut self, event: &Event) {
        if let Event::ZoneChanged { was, from: crate::Zone::Battlefield, .. } = event {
            let controller = self.controller(*was);
            let seats = self.players.len();
            self.turn_history_mut().departures.resize(seats, 0);
            self.turn_history_mut().departures[controller] += 1;
        }
        if let Event::DamageDealt { source, to, combat, .. } = event && !self.this_turn.damaged.contains(&(*source, *to, *combat)) {
            self.turn_history_mut().damaged.push((*source, *to, *combat));
        }
        match event {
            Event::Attacked { attacker, .. } => {
                let controller = self.controller(*attacker);
                // as it was declared: what it is since does not change what it attacked as (608.2i)
                let seen = (!self.this_turn.declared.iter().any(|(_, o, _)| o == attacker)).then(|| self.seen(*attacker, None));
                let history = self.turn_history_mut();
                for who in [crate::event::Target::Object(*attacker), crate::event::Target::Player(controller)] { if !history.attacked.contains(&who) { history.attacked.push(who); } }
                if let Some(seen) = seen { history.declared.push((controller, *attacker, seen)); }
            }
            Event::BecameBlocking { blocker } if !self.this_turn.blocked.contains(blocker) => self.turn_history_mut().blocked.push(*blocker),
            Event::Discarded { card, .. } => self.turn_history_mut().discarded.push(*card),
            Event::Saddles { mount: permanent, creatures } | Event::Crews { vehicle: permanent, creatures } => {
                let saddle = matches!(event, Event::Saddles { .. });
                let history = self.turn_history_mut();
                for c in creatures { if !history.tapped_for.contains(&(saddle, *c, *permanent)) { history.tapped_for.push((saddle, *c, *permanent)); } }
            }
            _ => {}
        }
        let entered = match event {
            Event::ZoneChanged { now, to: crate::Zone::Battlefield, .. } => *now,
            Event::Created { token } => Some(*token),
            _ => None,
        };
        if let Some(object) = entered {
            let seen = self.seen_for(object, crate::layers::Aspect::Types);
            for kind in seen.types {
                if let Some((_, _, count)) = self.turn_history_mut().entries.iter_mut().find(|(p,t,_)| *p == seen.controller && *t == kind) { *count += 1; }
                else { self.turn_history_mut().entries.push((seen.controller, kind, 1)); }
            }
        }
    }

    /// Note what has happened since the last time, as triggers are about to be checked for it
    /// (603.2) or as the turn ends.
    pub(crate) fn note_events(&mut self) {
        let from = self.this_turn.noted.max(self.log.start());
        let game: &Game = self;
        let kept: Vec<Fact> = game.log.sequenced().filter(|(at, _)| *at >= from)
            .flat_map(|(at, e)| crate::trigger::facts(game, e).into_iter().filter(|f| game.kept(f)).map(move |f| f.as_entered(game).remembered().at(at))).collect();
        self.this_turn.noted = self.log.end();
        self.turn_history_mut().now.extend(kept);
    }

    fn kept(&self, f: &Fact) -> bool { self.this_turn.every || ALWAYS.contains(&f.verb()) }

    /// What happened this turn or the one before: what was noted, and this turn what has not
    /// been yet, as it is now.
    fn facts_of(&self, turn: Turn) -> Vec<std::borrow::Cow<'_, Fact>> {
        use std::borrow::Cow;
        let mut all: Vec<Cow<'_, Fact>> = self.this_turn.of(turn).iter().map(Cow::Borrowed).collect();
        if turn == Turn::This {
            let from = self.this_turn.noted.max(self.log.start());
            all.extend(self.log.sequenced().filter(|(at, _)| *at >= from)
                .flat_map(|(at, e)| crate::trigger::facts(self, e).into_iter().filter(|f| self.kept(f)).map(move |f| Cow::Owned(f.remembered().at(at)))));
        }
        all
    }

    /// The players who declared these creatures as attackers this turn, each once, and what
    /// each was as it was declared (508.1).
    pub(crate) fn declared_attackers(&self) -> &[(crate::PlayerId, ObjId, crate::layers::Seen)] { &self.this_turn.declared }

    /// Revolt needs the departure controller, not the departed object's payload.
    pub(crate) fn departed_this_turn(&self, player: Option<crate::PlayerId>) -> usize {
        player.map_or_else(|| self.this_turn.departures.iter().sum(), |p| self.this_turn.departures.get(p).copied().unwrap_or(0))
    }

    /// Has this source dealt damage to this object or player this turn (120.1)?
    pub(crate) fn dealt_damage_this_turn(&self, source: ObjId, to: crate::event::Target) -> bool {
        self.this_turn.damaged.iter().any(|(s, t, _)| (*s, *t) == (source, to))
    }

    /// The objects that attacked this turn, or that dealt or were dealt damage this turn, as
    /// a look back at them asks (608.2i).
    pub(crate) fn looked_back(&self, atom: &crate::program::expr::Atom) -> Vec<ObjId> {
        use crate::event::Target;
        use crate::program::expr::Atom;
        let mut out = Vec::new();
        let mut add = |o: ObjId| if !out.contains(&o) { out.push(o) };
        match atom {
            Atom::FoughtThisTurn { attacked: true } => self.this_turn.attacked.iter().for_each(|t| if let Target::Object(o) = t { add(*o) }),
            Atom::TappedFor { saddle, .. } => self.this_turn.tapped_for.iter().filter(|(s, _, _)| s == saddle).for_each(|(_, c, _)| add(*c)),
            Atom::Damage { dealt: true, .. } => self.this_turn.damaged.iter().for_each(|(s, _, _)| add(*s)),
            Atom::Damage { dealt: false, .. } => self.this_turn.damaged.iter().for_each(|(_, t, _)| if let Target::Object(o) = t { add(*o) }),
            _ => {}
        }
        out
    }

    /// Did this creature or player attack this turn (508.1), or this creature block (509.1)?
    /// Did its owner discard this card this turn, and it is the card that went to their
    /// graveyard then (400.7, 702.187b)?
    pub(crate) fn discarded_this_turn(&self, card: ObjId) -> bool { self.this_turn.discarded.contains(&card) }
    pub(crate) fn attacked_this_turn(&self, who: crate::event::Target) -> bool { self.this_turn.attacked.contains(&who) }
    pub(crate) fn blocked_this_turn(&self, creature: ObjId) -> bool { self.this_turn.blocked.contains(&creature) }
    /// Did this creature saddle (`saddle`) or crew this permanent this turn (702.171c, 702.122b)?
    pub(crate) fn tapped_for_this_turn(&self, saddle: bool, creature: ObjId, permanent: ObjId) -> bool {
        self.this_turn.tapped_for.contains(&(saddle, creature, permanent))
    }

    /// Has damage of this kind been dealt this turn by this source, or to this object or
    /// player, or both (120.1)? "that was dealt damage this turn", "that dealt damage to you
    /// this turn".
    pub(crate) fn damage_this_turn(&self, source: Option<ObjId>, to: Option<crate::event::Target>, kind: ir::event::DamageKind) -> bool {
        use ir::event::DamageKind;
        self.this_turn.damaged.iter().any(|(s, t, combat)| source.is_none_or(|o| o == *s) && to.is_none_or(|to| to == *t)
            && match kind { DamageKind::Any => true, DamageKind::Combat => *combat, DamageKind::Noncombat => !*combat })
    }

    /// Actual life lost, including payments and damage (119.3–4), independent of later gains.
    /// Refused, with nothing noted, when the turn's total is not representable (107.1b).
    pub(crate) fn note_life_loss(&mut self,player:crate::PlayerId,amount:u32) -> crate::Res<()> {
        let lost = self.this_turn.life_lost.get(player).copied().unwrap_or(0).checked_add(amount)
            .ok_or_else(|| crate::Unsupported("life lost this turn exceeds the supported numerical range (107.1b)".into()))?;
        let seats = self.players.len();
        self.turn_history_mut().life_lost.resize(seats,0);
        self.turn_history_mut().life_lost[player]=lost;
        Ok(())
    }
    /// It became the target of `of` (115.9): the how-manyth time this turn it became the
    /// target of a spell or ability `of`'s controller controls, counted as it happens.
    pub(crate) fn note_targeted(&mut self, object: crate::event::Target, of: ObjId) -> u32 {
        let by = self.controller(of);
        let targeted = &mut self.this_turn.targeted;
        match targeted.iter_mut().find(|(o, p, _)| *o == object && *p == by) {
            Some((_, _, n)) => { *n += 1; *n }
            None => { targeted.push((object, by, 1)); 1 }
        }
    }

    pub fn life_lost_this_turn(&self,player:crate::PlayerId)->u32 {
        self.this_turn.life_lost.get(player).copied().unwrap_or(0)
    }

    /// Count only a completed, unreplaced draw (121.2, 614.6). Its event retains the
    /// ordinal until delivery, so several draws can finish before any trigger is stacked.
    /// Also counted, within a player's own draw step: "except the first one they draw in each
    /// of their draw steps" (the turn-based draw only if it was not replaced).
    pub(crate) fn note_draw(&mut self, player: crate::PlayerId) -> (u32, Option<u32>) {
        let seats = self.players.len();
        self.this_turn.draws.resize(seats, 0);
        self.this_turn.draws[player] += 1;
        let in_draw_step = (self.part == ir::Part::Draw && self.active == player).then(|| {
            self.this_turn.draw_step += 1;
            self.this_turn.draw_step
        });
        (self.this_turn.draws[player], in_draw_step)
    }

    /// A step or phase begins: each draw step, an extra one too, counts its own draws.
    pub(crate) fn forget_step(&mut self) { self.this_turn.draw_step = 0; }

    /// A type at entry is a historical fact, unaffected by later control/type changes.
    pub(crate) fn entered_this_turn(&self, player: Option<crate::PlayerId>, kind: ir::CardType) -> usize {
        self.this_turn.entries.iter().filter(|(p,t,_)| player.is_none_or(|who| who == *p) && *t == kind).map(|(_,_,n)| n).sum()
    }

    /// How many times this happened in that turn, as this text's controller sees it.
    /// `each`: said of a player ("a player cast two or more spells"), so the most that any
    /// one player did it.
    pub(crate) fn happened(&self, ctx: &Ctx, pattern: &Pattern, turn: Turn, each: bool) -> usize {
        if turn == Turn::This && pattern.exhaust_starts() {
            let facts: Vec<_> = self.activation_started.iter().map(|(player, keyword, mana)|
                Fact::activation(*player, *keyword, *mana, None)).collect();
            let such: Vec<_> = facts.iter().filter(|fact| pattern.described(self, ctx, fact).is_some()).collect();
            if !each { return such.len(); }
            return such.iter().map(|fact| such.iter().filter(|other| other.doer() == fact.doer()).count()).max().unwrap_or(0);
        }
        let all = self.facts_of(turn);
        let such: Vec<&Fact> = all.iter().map(|f| &**f).filter(|f| pattern.described(self, ctx, f).is_some()).collect();
        if !each { return such.len(); }
        such.iter().map(|f| such.iter().filter(|other| other.doer() == f.doer()).count()).max().unwrap_or(0)
    }

    /// Did an event of this turn the pattern describes happen to this object or player, as the
    /// object it is now (400.7)? "a card that was put into a graveyard from anywhere this turn"
    pub(crate) fn happened_to(&self, ctx: &Ctx, pattern: &Pattern, t: crate::event::Target) -> bool {
        self.facts_of(Turn::This).iter().any(|f| pattern.subject_of(self, ctx, f) == Some(t))
    }

    /// Which such event of the turn this one is, among those of the same subject ("your
    /// second spell each turn", "whenever this creature attacks for the first time each
    /// turn", 603.2): 1 for the first. None: it was not noted.
    pub(crate) fn nth_this_turn(&self, ctx: &Ctx, pattern: &Pattern, f: &Fact) -> Option<u32> {
        let subject = pattern.described(self, ctx, f)?.subject;
        let mut n = 0;
        for kept in self.facts_of(Turn::This) {
            if pattern.described(self, ctx, &kept).is_none_or(|m| m.subject != subject) { continue; }
            n += 1;
            if kept.same_event(f) { return Some(n); }
        }
        None
    }

    /// A turn begins: "this turn" is another, and the one that ended is "last turn".
    pub(crate) fn forget_turn(&mut self) {
        self.note_events();
        self.this_turn.declared.clear();
        self.this_turn.entries.clear();
        self.this_turn.departures.clear();
        self.this_turn.draws.clear();
        self.this_turn.life_lost.clear();
        self.this_turn.damaged.clear();
        self.this_turn.attacked.clear();
        self.this_turn.blocked.clear();
        self.this_turn.discarded.clear();
        self.this_turn.draw_step = 0;
        self.this_turn.targeted.clear();
        self.this_turn.archive.0.clear();
        let ended = std::mem::take(&mut self.this_turn.now);
        self.this_turn.last = ended.into_iter().filter(|f| KEPT_A_TURN.contains(&f.verb())).collect();
    }
}

#[cfg(test)]
mod tests {
    use crate::event::Event;
    use crate::state::{Game, Zone};
    use crate::card::CardDef;
    use ir::Part;

    fn in_step(g: &Game) -> Vec<(usize, Option<u32>)> {
        g.log.iter().filter_map(|e| if let Event::Drew { player, in_draw_step, .. } = e { Some((*player, *in_draw_step)) } else { None }).collect()
    }

    /// "Each of their draw steps" (504): an additional draw step counts its draws anew; draws
    /// outside a draw step, or by a player whose draw step it isn't, are not numbered.
    #[test]
    fn r504_each_draw_step_numbers_its_active_players_draws_anew() {
        let island = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
        let mut g = Game::new(vec![island], 2, 20);
        for p in 0..2 { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
        g.active = 1;
        g.begin_part(Part::Upkeep).unwrap();
        g.draw(1, 1);
        g.begin_part(Part::Draw).unwrap();
        g.draw(1, 1);
        g.draw(0, 1);
        // an additional draw step in the same turn
        g.begin_part(Part::Draw).unwrap();
        g.draw(1, 1);
        assert_eq!(in_step(&g), vec![(1, None), (1, Some(1)), (1, Some(2)), (0, None), (1, Some(1)), (1, Some(2))]);
    }

    #[test]
    fn r608_2i_a_card_that_looks_back_is_recognized_as_written() {
        let card = |text: &str| CardDef::new("Probe", "{0}", "Creature", text, Some((1, 1))).unwrap();
        for text in [
            "If you attacked this turn, this creature gets +1/+0.",
            "As long as another creature entered the battlefield under your control this turn, this creature gets +2/+0.",
            "Whenever you gain life for the first time each turn, draw a card.",
            "When this creature enters, put a +1/+1 counter on it for each creature that left the battlefield under your control this turn.",
        ] {
            assert!(super::asks_back(&[card(text)]), "{text}");
        }
        assert!(!super::asks_back(&[card("Whenever this creature attacks, draw a card."), card("")]));
    }
}
