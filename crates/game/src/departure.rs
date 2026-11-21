//! Zone-destination proposals (614.1a). One simultaneous action freezes applicability,
//! collects owners' library/graveyard orders (401.4, 404.3), then commits every departure once.
use crate::{Game, ObjId, PlayerId, Zone, Target, Event};
use crate::select::Ctx;
use crate::state::{Answer, Decision};
use crate::operation::Completed;
use ir::{Verb, Role};
use ir::action::LibraryPosition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Destination {
    pub zone: Zone, pub position: Option<LibraryPosition>,
    /// The replacement effect that sends it there instead (614.1a), with what else its
    /// modified event does.
    pub by: Option<Redirect>,
}
impl Destination {
    pub(crate) fn to(zone: Zone) -> Self { Destination { zone, position: None, by: None } }
    /// Where it goes and what else is done, whichever effect does it: the reflexive ability
    /// it triggers by its compiled instance ([`crate::program::same_instance`]).
    pub(crate) fn outcome(&self) -> (Zone, Option<LibraryPosition>, bool, Option<(PlayerId, u32)>, Option<*const crate::program::Program>) {
        let by = self.by.as_ref();
        (self.zone, self.position, by.is_some_and(|b| b.shuffle), by.and_then(|b| b.gain), by.and_then(|b| b.then.as_ref().map(std::sync::Arc::as_ptr)))
    }
}
/// A replacement effect applied to where an object goes (614.1a, 614.6): which one, and
/// what else its modified event does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Redirect {
    pub id: crate::operation::ReplacementId,
    /// "reveal this card and shuffle it into its owner's library instead"
    pub shuffle: bool,
    /// "instead exile it and you gain 2 life": who gains how much
    pub gain: Option<(PlayerId, u32)>,
    /// "When you do, …": the reflexive triggered ability it triggers (603.12)
    pub then: Option<std::sync::Arc<crate::program::Program>>,
}
impl Game {
    /// Where this object goes if it is moved to `to` now, when nobody is asked which
    /// replacement applies: the object's own first, then the others ([`Game::redirects`]).
    pub(crate) fn departure_destination(&self, object: ObjId, to: Zone) -> Destination {
        let o = self.obj(object);
        // Every departure of a flashback spell is exiled, not just resolution (702.34a).
        if let Some((scope, destination)) = o.flashback && scope.contains(o.zone) && to != o.zone {
            return Destination::to(destination);
        }
        // "If an instant or sorcery spell cast this way would be put into your graveyard,
        // exile it instead": the graveyard only, not a hand or library (614.1a)
        if o.zone == Zone::Stack && to == Zone::Graveyard && o.cast_rider.as_ref().is_some_and(|r| r.exiles(self, object)) {
            return Destination::to(Zone::Exile);
        }
        self.redirects(object, to).into_iter().next().unwrap_or(Destination::to(to))
    }

    /// Emit the semantic action of a prepared departure exactly once (614.6, 701.13a).
    pub(crate) fn record_departure(&mut self, object: ObjId, kind: Kind, zone: Zone) {
        if zone == Zone::Exile && !self.can(Verb::Exile, self.controller(object), Target::Object(object)) { return; }
        match kind {
            Kind::RuleMove(_) | Kind::Mill(_) => {},
            // "that player discards it, but exiles it instead" (702.35a)
            Kind::Exile { .. } | Kind::Madness(_) => self.log.push(Event::Exiled { object }),
            Kind::Move(Zone::Exile) => self.log.push(Event::Exiled { object }),
            Kind::Move(_) | Kind::Library {..} => self.log.push(Event::Moved { object, to: zone }),
            Kind::Destroy { .. } => self.log.push(Event::Destroyed { object }),
            Kind::Sacrifice(player) => self.log.push(Event::Sacrificed { player, object }),
        }
        // put into exile instead of where it was going: it is exiled (614.6, 701.13a)
        if zone == Zone::Exile && !matches!(kind, Kind::Exile { .. } | Kind::Madness(_) | Kind::Move(Zone::Exile)) {
            self.log.push(Event::Exiled { object });
        }
    }

    /// Every replacement effect that would change where this object goes if it were put into
    /// `to` now (614.1a), each with where it sends it: its own first ("If this creature would
    /// die, put it on the bottom of its owner's library instead.", "If Darksteel Colossus would
    /// be put into a graveyard from anywhere, …"), then a resolved "If that creature would die
    /// this turn, exile it instead", then the static abilities of permanents. Each replaces a
    /// move to a graveyard with one elsewhere, so once one is applied no other applies (616.1f).
    pub(crate) fn redirects(&self, object: ObjId, to: Zone) -> Vec<Destination> {
        use crate::replace::{Replacement, Moved, Instead};
        use crate::operation::ReplacementId;
        let o = self.obj(object);
        if to != Zone::Graveyard || o.zone == Zone::Graveyard || o.zone == Zone::Gone { return Vec::new(); }
        let dies = o.zone == Zone::Battlefield;
        let owner = o.owner;
        let redirect = |source: ObjId, key, zone: Zone, position, shuffle, gain, then| Destination { zone, position,
            by: Some(Redirect { id: ReplacementId::from_ability(source, key, 0), shuffle, gain, then }) };
        let mut found = Vec::new();
        for (key, r) in self.replacements(object) {
            match r {
                Replacement::DeathToLibrary(position) if dies => found.push(redirect(object, key, Zone::Library, Some(*position), false, None, None)),
                Replacement::Elsewhere { moved: Moved::Own, instead: Instead::Shuffle, .. } => found.push(redirect(object, key, Zone::Library, None, true, None, None)),
                _ => {}
            }
        }
        // Physical counters create one rule-owned replacement, independently of the
        // permanent's text or effective abilities (122.1h). Freeze it with this proposal.
        if dies && o.counters.get("finality").copied().unwrap_or(0) > 0 {
            found.push(Destination { zone: Zone::Exile, position: None, by: Some(Redirect {
                id: ReplacementId { source: object, effect: crate::operation::ReplacementEffect::Finality },
                shuffle: false, gain: None, then: None,
            }) });
        }
        if dies && let Some(id) = self.exiled_instead_of_dying(object) {
            found.push(Destination { zone: Zone::Exile, position: None, by: Some(Redirect { id, shuffle: false, gain: None, then: None }) });
        }
        // the static abilities of permanents function on the battlefield (113.6), the moving
        // permanent's own among them, as it is before it moves
        for source in self.battlefield.iter().copied() {
            let controller = self.controller(source);
            for (key, r) in self.replacements(source) {
                let Replacement::Elsewhere { moved, instead, gain, then } = r else { continue };
                let ctx = Ctx::new(controller, source).with_key(key.clone());
                let applies = match moved {
                    Moved::Own => false,
                    Moved::Dies(what) => dies && what.contains(self, &ctx, &crate::program::expr::NoLocals, Target::Object(object)),
                    Moved::IntoGraveyard { what, into } => what.iter().any(|what| what.is(self, &ctx, Target::Object(object)))
                        && into.as_ref().is_none_or(|into| into.is(self, &ctx, Target::Player(owner))),
                };
                if !applies { continue; }
                let zone = match instead { Instead::Exile => Zone::Exile, Instead::Shuffle => Zone::Library };
                found.push(redirect(source, key, zone, None, *instead == Instead::Shuffle, gain.map(|n| (controller, n)), then.clone()));
            }
        }
        found
    }

    /// The replacements to choose among for this object's move (616.1): none when fewer than
    /// two would apply, or when all would make the same of it.
    pub(crate) fn redirect_choice(&self, object: ObjId, to: Zone) -> Vec<Destination> {
        let all = self.redirects(object, to);
        if all.iter().all(|d| d.outcome() == all[0].outcome()) { Vec::new() } else { all }
    }
    pub(crate) fn departure_batch(&mut self, actions: Vec<(ObjId, Kind)>) -> crate::Res<Result<Completed, Decision>> {
        self.run_operation(crate::operation::Operation::Depart(Batch::new(self, actions)))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind { Move(Zone), Library {position:LibraryPosition,order:Option<ir::action::Order>,known:bool},
    /// Put there by a rule, or by an action with a name of its own (drawn, surveilled), not
    /// "returned or put" by an instruction: no [`Event::Moved`].
    RuleMove(Zone),
    Destroy { regenerable: bool }, Sacrifice(PlayerId), Exile { source: ObjId, ability: Option<crate::binding::AbilityKey>, generation: Option<usize> },
    /// A card discarded into exile by its madness ability (702.35a): this one.
    Madness(crate::binding::AbilityKey),
    /// Put from the top of this player's library into their graveyard: milled (701.17a);
    /// still milled when a replacement puts it elsewhere (701.17c).
    Mill(PlayerId) }
impl Kind {
    /// Where the action puts it, before replacement effects (614.1a).
    fn zone(self) -> Zone {
        match self { Kind::Library {..} => Zone::Library, Kind::Move(zone) | Kind::RuleMove(zone) => zone, Kind::Exile { .. } | Kind::Madness(_) => Zone::Exile, _ => Zone::Graveyard }
    }
}
#[derive(Debug, Clone)]
struct Proposed { object: ObjId, kind: Kind, destination: Destination, regenerates: bool, chosen: bool }
#[derive(Debug, Clone)]
pub(crate) struct Batch {
    events: Vec<Proposed>,
    orders: Vec<(crate::state::Placement, crate::library::Ordering, Option<ir::action::Order>)>,
    next: usize,
    returns: crate::commander::Returns,
    /// Moves two replacements with different outcomes would replace, each asked first
    /// (616.1): the event, who chooses (the object's controller, or its owner), whether it
    /// would die, and where each sends it.
    asks: Vec<(usize, PlayerId, bool, Vec<Destination>)>,
    asked: usize,
    /// Public source cards whose graveyard-order choice later owners may know (101.4b).
    /// Hidden-zone selections remain private until the simultaneous move (101.4a).
    public_cards: Vec<ObjId>,
    /// Cards the same action puts at a position in their owner's library without their
    /// leaving it, each group in the order already chosen, the first nearest the top.
    arranged: Vec<(PlayerId, Vec<ObjId>, LibraryPosition)>,
}
impl Batch {
    pub(crate) fn choices(&self) -> &[crate::state::PublicChoice] { &self.returns.choices }
    pub(crate) fn new(game: &Game, actions: Vec<(ObjId, Kind)>) -> Self {
        Self::proposing(game, actions.into_iter().map(|(object, kind)| (object, kind, None)))
    }
    /// Destinations already selected by the enclosing discard replacement ordering.
    pub(crate) fn destined(game: &Game, actions: Vec<(ObjId, Kind, Destination)>) -> Self {
        Self::proposing(game, actions.into_iter().map(|(object, kind, destination)| (object, kind, Some(destination))))
    }
    fn proposing(game: &Game, actions: impl Iterator<Item = (ObjId, Kind, Option<Destination>)>) -> Self {
        let mut asks = Vec::new();
        let events: Vec<_> = actions.filter_map(|(object, kind, selected)| {
            let allowed = match kind {
                Kind::Exile { .. } => game.can(Verb::Exile, game.controller(object), Target::Object(object)),
                Kind::Move(_) | Kind::Library {..} | Kind::RuleMove(_) | Kind::Madness(_) | Kind::Mill(_) => game.takes(Verb::Move, Role::What, Target::Object(object)),
                Kind::Destroy { .. } => game.destruction_allowed(object),
                Kind::Sacrifice(player) => game.can(Verb::Sacrifice, player, Target::Object(object)),
            };
            if !allowed { return None; }
            let regenerates = matches!(kind, Kind::Destroy { regenerable: true }) && game.shields.contains(&object);
            let zone = kind.clone().zone();
            let chosen = selected.is_some();
            let destination = selected.unwrap_or_else(|| if let Kind::Library {position,..}=kind {Destination {zone,position:Some(position),by:None}} else if regenerates { Destination::to(zone) } else { game.departure_destination(object, zone) });
            Some(Proposed { object, kind, destination, regenerates, chosen })
        }).collect();
        for (at, event) in events.iter().enumerate() {
            let among = if event.regenerates || event.chosen { Vec::new() } else { game.redirect_choice(event.object, event.kind.clone().zone()) };
            if among.is_empty() { continue; }
            // the affected object's controller, or its owner if it has none (616.1)
            let o = game.obj(event.object);
            let chooser = if matches!(o.zone, Zone::Battlefield | Zone::Stack) { game.controller(event.object) } else { o.owner };
            asks.push((at, chooser, o.zone == Zone::Battlefield, among));
        }
        // every destination a choice may still pick is planned for; the others are dropped
        // as it is answered ([`Batch::answer`])
        let candidates = |at: usize, e: &Proposed| -> Vec<Destination> {
            asks.iter().find(|ask| ask.0 == at).map(|ask| ask.3.clone()).unwrap_or_else(|| vec![e.destination.clone()])
        };
        let mut groups: Vec<(PlayerId, crate::state::Placement, Vec<ObjId>, Option<ir::action::Order>)> = Vec::new();
        for (at, event) in events.iter().enumerate() {
            // 401.4/404.3 order cards. Tokens may visit these zones, but are not cards and
            // cease at the next state-based check (111.6-7).
            if event.regenerates || game.obj(event.object).card.is_none() { continue; }
            for destination in candidates(at, event) {
                if destination.zone == game.obj(event.object).zone && !matches!(event.kind,Kind::Library {..}) { continue; }
                let placement = match (destination.zone, destination.position) {
                    (Zone::Library, Some(position)) => crate::state::Placement::Library(position),
                    (Zone::Graveyard, _) => crate::state::Placement::Zone(Zone::Graveyard),
                    _ => continue,
                };
                let owner = game.obj(event.object).owner;
                let order=if destination.by.is_none() {match event.kind {Kind::Library {order,..}=>order,_=>None}}else{None};
                if let Some((_, _, cards, _)) = groups.iter_mut().find(|(p, pos, _, said)| *p == owner && *pos == placement && *said==order) {
                    if !cards.contains(&event.object) { cards.push(event.object); }
                }
                else { groups.push((owner, placement, vec![event.object],order)); }
            }
        }
        groups.sort_by_key(|(owner, _, _, _)| (*owner + game.players.len() - game.active) % game.players.len());
        let orders = groups.into_iter().map(|(_, placement, cards, order)|
            (placement, crate::library::Ordering::new(game, cards),order)).collect();
        let returns = crate::commander::Returns::new(game, events.iter().enumerate().filter(|(_, e)| !e.regenerates)
            .flat_map(|(at, e)| candidates(at, e).into_iter().map(move |d| (e.object, d.zone))));
        // asked in APNAP order (616.1, 101.4)
        asks.sort_by_key(|(_, player, _, _)| (*player + game.players.len() - game.active) % game.players.len());
        let public_cards = events.iter().map(|event| event.object)
            .filter(|card| (0..game.players.len()).all(|player| game.perceives(player, *card))).collect();
        Self { events, orders, next: 0, returns, asks, asked: 0, public_cards, arranged: Vec::new() }
    }
    /// The cards go into each graveyard in the order the action gave them, each onto the ones
    /// before it: their owner chose that order with the action (404.3) and is not asked again.
    pub(crate) fn stacked_as_given(mut self) -> Self {
        for (placement, order, _) in &mut self.orders {
            if *placement == crate::state::Placement::Zone(Zone::Graveyard) { order.as_given(); }
        }
        self
    }
    /// The same action also puts these cards at this position in their owner's library, where
    /// they already are, in the order already chosen, the first nearest the top: the cards a
    /// player scried or surveilled and put back (701.22a, 701.25a). They are in place before
    /// a replacement shuffles that library in the same event, and stay there (701.24g).
    pub(crate) fn arranging(mut self, owner: PlayerId, cards: Vec<ObjId>, position: LibraryPosition) -> Self {
        if !cards.is_empty() { self.arranged.push((owner, cards, position)); }
        self
    }
    pub(crate) fn prepare(&mut self) -> Option<Decision> {
        if let Some((at, player, dies, among)) = self.asks.get(self.asked) {
            return Some(Decision::Choose { player: *player, choice: crate::state::Choice::Replacement {
                on: Target::Object(self.events[*at].object), kind: if *dies { "death" } else { "graveyard" }.into(), count: 1, counters: None, tokens:None,
                among: among.iter().filter_map(|d| d.by.as_ref().map(|b| b.id.clone())).collect(), optional: false } });
        }
        if let Some(decision) = self.returns.decision() { return Some(decision); }
        // These objects never enter the proposed library, so they do not participate in
        // its private ordering. Repeated preparation is harmless and never moves a card.
        for event in &mut self.events {
            if self.returns.selected.contains(&event.object) {
                event.destination = Destination::to(Zone::Command);
            }
        }
        for (_, order, _) in &mut self.orders { order.exclude(&self.returns.selected); }
        while let Some((placement, order, said)) = self.orders.get_mut(self.next) {
            if let Some(decision) = order.prepare(|_| *placement, None, *said!=Some(ir::action::Order::Random)) { return Some(decision); }
            self.next += 1;
        }
        None
    }
    pub(crate) fn waiting(&self) -> bool { self.asked < self.asks.len() || self.returns.decision().is_some() || self.next < self.orders.len() }
    pub(crate) fn answer(&mut self, answer: Answer) {
        if let Some((at, player, _, among)) = self.asks.get(self.asked) {
            let Answer::Replacement(chosen) = answer else { unreachable!("validated replacement answer") };
            let (at, player) = (*at, *player);
            let destination = among.iter().find(|d| d.by.as_ref().is_some_and(|b| b.id == chosen)).cloned().expect("validated replacement answer");
            let object = self.events[at].object;
            // once it is applied the others no longer apply (616.1f): where they would have
            // put it is no longer planned for
            self.events[at].destination = destination.clone();
            for (placement, order, _) in &mut self.orders {
                let remains = match placement {
                    crate::state::Placement::Library(position) => destination.zone == Zone::Library && destination.position == Some(*position),
                    crate::state::Placement::Zone(zone) => destination.zone == *zone,
                };
                if !remains { order.exclude(&[object]); }
            }
            if !matches!(destination.zone, Zone::Hand | Zone::Library) { self.returns.forget(object); }
            self.returns.choices.push(crate::state::PublicChoice::Replacement { player, on: Target::Object(object), effect: chosen });
            self.asked += 1;
            return;
        }
        if self.returns.decision().is_some() { self.returns.answer(answer); return; }
        if let Some((placement, order, _)) = self.orders.get_mut(self.next) {
            if *placement == crate::state::Placement::Zone(Zone::Graveyard)
                && let Some((player, cards)) = order.pending()
                && cards.iter().all(|card| self.public_cards.contains(card))
                && let Answer::Objects(cards) = &answer {
                self.returns.choices.push(crate::state::PublicChoice::OrderCards { player, to: *placement, cards: cards.clone() });
            }
            order.answer(answer);
        }
    }
    pub(crate) fn commit(self, game: &mut Game) -> Completed {
        let from = game.log.end();
        let first = game.clock + 1;
        let before = game.departure_snapshot(&game.battlefield);
        let mut result = Completed::default();
        let mut became = Vec::new();
        // the shuffles replacements' modified events include, done once every card this event
        // puts at a specified position in a library is there (701.24g)
        let mut shuffles = Vec::new();
        for event in self.events {
            let Proposed { object, kind, destination, regenerates, .. } = event;
            if game.obj(object).zone == Zone::Gone { continue; }
            if regenerates { game.destroy_allowed(object, true); continue; }
            game.record_departure(object, kind.clone(), destination.zone);
            let original_zone = game.obj(object).zone;
            let now=if original_zone==Zone::Library && destination.zone==Zone::Library && destination.by.is_none() {
                if matches!(kind,Kind::Library {known:true,..}) {game.hide_reordered(object)} else {Some(object)}
            } else {game.move_deferring(object,destination.clone(),game.controller(object),crate::replace::Entry::default(),Some(&mut shuffles))};
            if let Some(now) = now {
                if let Kind::Exile { source, ref ability, generation } = kind {
                    if destination.zone == Zone::Exile { game.exiled_with.push((source, ability.clone(), now, generation)); }
                }
                if let Kind::Madness(ref ability) = kind && destination.zone == Zone::Exile { result.madness.push((now, ability.clone())); }
                if let Kind::Mill(player) = kind { game.log.push(Event::Milled { player, card: now }); }
                if matches!(kind, Kind::Destroy { .. }) {
                    result.destroyed.push(crate::binding::Destruction { before: object, after: now, from: original_zone, to: destination.zone });
                }
                became.push((object, now));
                result.moved.push((object, now));
                if matches!(destination.zone, Zone::Battlefield | Zone::Graveyard | Zone::Exile | Zone::Stack) { result.transitions.push((object, now)); result.arrived.push(now); }
            }
        }
        // Commit in action order, never chosen library order: public event ordering must
        // not reveal the owner's private arrangement (401.4).
        let mut positioned = Vec::new();
        for (placement, order, said) in self.orders {
            for (owner, cards) in order.groups() {
                let mut placed = cards.iter().filter_map(|was| became.iter().find(|(old, _)| old == was).map(|(_, now)| *now)).collect::<Vec<_>>();
                match placement {
                    crate::state::Placement::Library(position) => {
                        if said==Some(ir::action::Order::Random) {game.random_library_order(*owner,&mut placed,position);}
                        game.arrange_library(*owner, &placed, position);
                        positioned.extend(placed);
                    },
                    crate::state::Placement::Zone(Zone::Graveyard) => {
                        let pile = &mut game.players[*owner].graveyard;
                        pile.retain(|card| !placed.contains(card));
                        pile.extend(placed.into_iter().rev());
                    }
                    _ => unreachable!("a retained departure order is for a library or graveyard"),
                }
            }
        }
        for (owner, mut cards, position) in self.arranged {
            cards.retain(|card| game.players[owner].library.contains(card));
            game.arrange_library(owner, &cards, position);
            positioned.extend(cards);
        }
        // "a shuffled library that's randomized except that the object is in the specified
        // position" (701.24g): the cards put in place keep their places, the rest are shuffled
        for (owner, revealed) in shuffles { game.shuffle_except(owner, &positioned, &revealed); }
        // The objects moved enter their new zones at once (613.7d, 613.7m).
        game.stamped_at_once(first);
        game.finish_simultaneous(from, before);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef, Move};
    use crate::state::Choice;
    const REPLACE: &str = "If this creature would die, put it on the bottom of its owner's library instead.";
    fn card(name: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
        let def = CardDef::new(name, "{0}", types, text, pt).unwrap(); crate::play::check(&def).unwrap(); def
    }
    #[test]
    fn r400_3_stolen_permanents_ask_their_owner_for_library_order_during_an_opponents_payment() {
        let mut g = Game::new(vec![card("Altar", "Artifact", "Sacrifice two creatures: You gain 3 life.", None),
            card("First", "Creature", REPLACE, Some((2,3))), card("Second", "Creature", REPLACE, Some((2,3)))], 2, 20);
        let one = g.put(0,1,Zone::Battlefield); let two = g.put(0,2,Zone::Battlefield);
        g.obj_mut(one).controller = 1; g.obj_mut(two).controller = 1;
        let altar = g.put(1,0,Zone::Battlefield); g.act(0,Move::Pass).unwrap();
        g.act(1,Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![one,two]] }).unwrap();
        assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::OrderCards {
            to: crate::state::Placement::Library(ir::action::LibraryPosition::Bottom),
            cards: vec![one, two],
        } }));
        let before = format!("{g:?}");
        assert_eq!(g.act(1,Move::Choose(Answer::Objects(vec![one,two]))),
            Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"), before);
        g.act(0,Move::Choose(Answer::Objects(vec![one,two]))).unwrap();
        assert_eq!(g.players[0].library.iter().map(|o|g.obj(*o).card.unwrap()).collect::<Vec<_>>(),vec![2,1]);
        assert!(g.players[1].library.is_empty());
        for _ in 0..2 { g.act(g.priority,Move::Pass).unwrap(); }
        assert_eq!(g.players[1].life,23);
    }
    #[test]
    fn r101_4_all_owners_order_in_apnap_order_before_any_departure_commits() {
        let mut g = Game::new(vec![card("Sweep","Instant","Destroy all creatures.",None),
            card("First","Creature",REPLACE,Some((2,3))), card("Second","Creature",REPLACE,Some((2,3)))],3,20);
        g.active = 1; g.priority = 1;
        let cards: Vec<_> = (0..3).map(|p| vec![g.put(p,1,Zone::Battlefield),g.put(p,2,Zone::Battlefield)]).collect();
        let spell = g.put(1,0,Zone::Hand);g.act(1,Move::Cast { card:spell,targets:vec![] }).unwrap();
        for _ in 0..3 { g.act(g.priority,Move::Pass).unwrap(); }
        for p in [1,2,0] {
            assert!(matches!(g.decision,Some(Decision::Choose{player,choice:Choice::OrderCards{..}})if player==p));
            assert_eq!(g.battlefield.len(),6);assert!(g.public_choices().next().is_none());
            g.act(p,Move::Choose(Answer::Objects(cards[p].clone()))).unwrap();
        }
        assert!(g.battlefield.is_empty());
        for p in 0..3 { assert_eq!(g.players[p].library.iter().map(|o|g.obj(*o).card.unwrap()).collect::<Vec<_>>(),vec![2,1]); }
    }
    #[test]
    fn r400_7_a_hidden_replacement_destination_is_not_a_retained_public_successor() {
        let mut g=Game::new(vec![card("Nissa's Chosen","Creature",REPLACE,Some((2,3)))],2,20);
        let old=g.put(0,0,Zone::Battlefield);g.destroy(old);let now=g.players[0].library[0];
        assert_eq!(g.became(old),None);assert!(!g.sees(0,now)&&!g.sees(1,now));
        g.state_based_actions();g.retire_events(g.log.end());g.collect_departed();
        assert!(g.get_object(old).is_none());assert!(g.get_object(now).is_some());
    }

}
