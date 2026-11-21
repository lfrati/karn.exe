//! Discards (701.9) propose their destinations before any move. A replacement of the
//! destination keeps the discard: "If a spell or ability an opponent controls causes you to
//! discard this card, put it onto the battlefield instead", and madness, "that player
//! discards it, but exiles it instead of putting it into their graveyard" (702.35a). Where
//! more than one applies to a discard, the affected player chooses one (616.1); once one has,
//! the card no longer goes to the graveyard, and the other does not apply. The shared entry
//! operation owns any before-entry questions.
use crate::binding::AbilityKey;
use crate::departure::Kind;
use crate::entry::EntryRequest;
use crate::event::{Cause, Event, Target};
use crate::operation::{Completed, Replaced, ReplacementId};
use crate::replace::{Choices, Replacement};
use crate::select::Ctx;
use crate::{Game, ObjId, PlayerId, Zone};
use crate::state::Decision;

/// Where a discarded card goes (701.9a), as the replacements of its discard leave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Into {
    Graveyard,
    /// Into exile, by this madness ability (702.35a).
    Exile(AbilityKey),
    Battlefield,
    Destination(crate::departure::Destination),
}

/// One card's discard, proposed.
#[derive(Debug, Clone)]
pub(crate) struct Discarding {
    player: PlayerId,
    card: ObjId,
    cause: Option<Cause>,
    into: Into,
}

impl Replaced for Discarding {
    type Change = Into;
    fn affected(&self) -> Target { Target::Object(self.card) }
    /// The player discarding it: its owner, whose hand it is in (616.1).
    fn controller(&self, _: &Game) -> PlayerId { self.player }
    fn question(&self) -> (String, u32) { ("discard".into(), 1) }
    fn applicable(&mut self, game: &mut Game, applied: &[ReplacementId]) -> crate::Res<Vec<(ReplacementId, Into)>> {
        // each says "instead of putting it into their graveyard"
        if self.into != Into::Graveyard { return Ok(Vec::new()); }
        let mut choices = game.discard_replacements(self.player, self.card, self.cause);
        choices.extend(game.redirects(self.card, Zone::Graveyard).into_iter().map(|destination| {
            (destination.by.as_ref().expect("redirect identity").id.clone(), Into::Destination(destination))
        }));
        Ok(choices.into_iter().filter(|(id, _)| !applied.contains(id)).collect())
    }
    fn apply(&mut self, change: Into) -> crate::Res<()> { self.into = change; Ok(()) }
}

/// The discards, gone where they go: onto the battlefield by the entry operation, into a
/// graveyard or exile as departures.
pub(crate) fn destinations(game: &Game, discards: &[Discarding]) -> (Vec<EntryRequest>, crate::departure::Batch) {
    let mut entries = Vec::new();
    let mut moves = Vec::new();
    for d in discards {
        match &d.into {
            Into::Battlefield => entries.push(EntryRequest::card(d.card, d.player, Choices::default(), true)),
            Into::Graveyard => moves.push((d.card, Kind::Move(Zone::Graveyard), crate::departure::Destination::to(Zone::Graveyard))),
            Into::Destination(destination) => moves.push((d.card, Kind::Move(Zone::Graveyard), destination.clone())),
            Into::Exile(madness) => moves.push((d.card, Kind::Madness(madness.clone()), crate::departure::Destination::to(Zone::Exile))),
        }
    }
    (entries, crate::departure::Batch::destined(game, moves))
}

/// Each discard is recorded once after its entire replacement/entry operation commits.
pub(crate) fn record(game: &mut Game, discards: &[Discarding], result: &Completed) {
    for d in discards {
        if let Some((_, now)) = result.changed().find(|(was, _)| *was == d.card) {
            let madness = result.madness.iter().find(|(card, _)| *card == now).map(|(_, key)| key.clone());
            let event = Event::Discarded { player: d.player, card: now, cause: d.cause, madness };
            game.log.push(event.clone());
            game.note(&event);
        }
    }
}

impl Game {
    /// The replacements that would apply to this player's discard of this card: its own, which
    /// function in a hand (113.6, 614.4). "Causes you to discard" reads the controller of the
    /// causing occurrence, not the source's current controller; a discard nothing caused
    /// (a cost, the cleanup step) is not one.
    fn discard_replacements(&self, player: PlayerId, card: ObjId, cause: Option<Cause>) -> Vec<(ReplacementId, Into)> {
        let ctx = Ctx::new(player, card);
        self.replacements(card).filter_map(|(key, r)| {
            let id = ReplacementId::from_ability(card, key.clone(), 0);
            match r {
                Replacement::DiscardToBattlefield { cause: who }
                    if cause.is_some_and(|cause| who.is_none_or(|who| who.matches(&ctx, cause.controller))) => Some((id, Into::Battlefield)),
                Replacement::Madness { .. } => Some((id, Into::Exile(key))),
                _ => None,
            }
        }).collect()
    }

    /// The triggered ability `index` of this card resolved, having moved the card on from
    /// exile as `now`. When it is the card's madness trigger and the card was moved to a public
    /// zone without being cast, effects that refer to the discarded card can find it there
    /// (400.7k, 702.35c). A cast card went to the stack as a spell, not by this move.
    pub(crate) fn madness_resolved(&mut self, card: ObjId, index: usize, now: ObjId) {
        let madness = self.def(card).is_some_and(|d| crate::keyword::madness(d).iter().any(|(.., trigger)| *trigger == index));
        let public = matches!(self.obj(now).zone, Zone::Graveyard | Zone::Battlefield | Zone::Exile | Zone::Command);
        if madness && public { self.madness_found.push((card, now)); }
    }

    pub(crate) fn discard_batch(&mut self, cards: &[(PlayerId, ObjId)], cause: Cause) -> crate::Res<Result<Completed, Decision>> {
        self.discard_proposed(cards, Some(cause))
    }

    pub(crate) fn discard_proposed(&mut self, cards: &[(PlayerId, ObjId)], cause: Option<Cause>) -> crate::Res<Result<Completed, Decision>> {
        let discards = cards.iter().copied()
            .filter(|&(player, card)| self.can(ir::Verb::Discard, player, Target::Object(card)))
            .map(|(player, card)| Discarding { player, card, cause, into: Into::Graveyard }).collect::<Vec<_>>();
        if discards.is_empty() { return Ok(Ok(Completed::default())); }
        self.discard_operation(discards)
    }

    /// Bounded fixture convenience; questions remain owned by the discard operation.
    pub(crate) fn discard_caused(&mut self, player: PlayerId, card: ObjId, cause: Option<Cause>) {
        let parent = self.procedures.len();
        if let Err(decision) = self.discard_proposed(&[(player, card)], cause).expect("bounded fixture discard") {
            self.procedures.insert(parent, crate::procedure::Procedure::RuleAction { complete: false });
            self.decision = Some(decision);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::legal::Asked;
    use crate::state::Answer;
    use crate::{CardDef, Game, Move, Zone};

    fn game(sifter: bool) -> Game {
        let defs = vec![
            CardDef::new("Fiery Temper", "{1}{R}{R}", "Instant", "Fiery Temper deals 3 damage to any target.\nMadness {R}", None).unwrap(),
            CardDef::new("Rummage", "{0}", "Instant", "Discard a card.", None).unwrap(),
            CardDef::new("Sifter", "{0}", "Enchantment", "Whenever you discard a card, exile that card from your graveyard.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        if sifter { g.put(0, 2, Zone::Battlefield); }
        g.put(0, 0, Zone::Hand);
        let rummage = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        // the madness trigger on top, to resolve first
        if let Some((0, Asked::Order(abilities))) = g.asked() {
            let order = [vec![0, 1], vec![1, 0]].into_iter().find(|order| {
                let mut copy = g.clone();
                copy.act(0, Move::Order(order.clone())).is_ok() && copy.obj(*copy.stack.last().unwrap()).ability.map(|(s, _)| s) == Some(abilities.iter().find(|(s, _)| copy.obj(s.unwrap()).chars.name == "Fiery Temper").unwrap().0)
            }).unwrap();
            g.act(0, Move::Order(order)).unwrap();
        }
        // the madness trigger resolves, declined
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(0, Move::Choose(Answer::No)).unwrap();
        g
    }

    fn collect(g: &mut Game) { g.retire_events(g.log.end()); g.collect_departed(); }

    #[test]
    fn r400_7k_the_link_lasts_while_something_refers_to_the_discarded_card() {
        // the Sifter's ability waits, naming the discarded card: the link outlives a collection
        let mut g = game(true);
        assert_eq!(g.madness_found.len(), 1);
        collect(&mut g);
        assert_eq!(g.madness_found.len(), 1);
        let (was, found) = g.madness_found[0];
        assert_eq!(g.obj(found).zone, Zone::Graveyard);
        assert_eq!(g.obj(was).zone, Zone::Gone);
        // it resolves, finding the card; the link is then of nothing, and goes
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.obj(found).zone, Zone::Gone);
        collect(&mut g);
        assert!(g.madness_found.is_empty());
        assert!(g.get_object(was).is_none(), "the link kept nothing alive");
        // nothing refers to the discarded card: the link keeps nothing alive
        let mut g = game(false);
        assert_eq!(g.madness_found.len(), 1);
        collect(&mut g);
        assert!(g.madness_found.is_empty());
        assert_eq!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).chars.name == "Fiery Temper").count(), 1);
    }

    fn competition() -> Game {
        // Full Oracle clauses; reminder text omitted, not executable card text.
        let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).unwrap();
        let defs = vec![
            card("Darksteel Colossus", "{11}", "Artifact Creature — Golem", "Trample\nIndestructible\nIf Darksteel Colossus would be put into a graveyard from anywhere, reveal Darksteel Colossus and shuffle it into its owner's library instead.", Some((11,11))),
            card("Rest in Peace", "{1}{W}", "Enchantment", "When this enchantment enters, exile all graveyards.\nIf a card or token would be put into a graveyard from anywhere, exile it instead.", None),
            card("Thrill of Possibility", "{1}{R}", "Instant", "As an additional cost to cast this spell, discard a card.\nDraw two cards.", None),
            card("Fiery Temper", "{1}{R}{R}", "Instant", "Fiery Temper deals 3 damage to any target.\nMadness {R}", None),
            card("Stinkweed Imp", "{2}{B}", "Creature — Imp", "Flying\nWhenever this creature deals combat damage to a creature, destroy that creature.\nDredge 5", Some((1,2))),
            card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        ];
        for def in &defs { crate::play::check(def).expect("full fixture admission"); }
        Game::new(defs, 2, 20)
    }

    fn id(source: crate::ObjId, row: usize) -> crate::operation::ReplacementId {
        crate::operation::ReplacementId::from_ability(source, crate::binding::AbilityKey::printed(row), 0)
    }

    fn offered(g: &Game, card: crate::ObjId, ids: &[crate::operation::ReplacementId]) {
        let Some(crate::state::Decision::Choose { player: 0, choice: crate::state::Choice::Replacement {
            on: crate::Target::Object(on), kind, count: 1, among, optional: false, ..
        } }) = &g.decision else { panic!("required discard replacement: {:?}", g.decision) };
        assert_eq!(*on, card);
        assert_eq!(kind, "discard");
        assert_eq!(among, ids);
    }

    fn discard_events(g: &Game) -> Vec<(crate::ObjId, bool)> {
        g.log.iter().filter_map(|event| if let crate::Event::Discarded { card, madness, .. } = event {
            Some((*card, madness.is_some()))
        } else { None }).collect()
    }

    fn start_thrill(g: &mut Game, card: crate::ObjId) -> crate::ObjId {
        let thrill = g.put(0, 2, Zone::Hand);
        for _ in 0..3 { g.put(0, 5, Zone::Library); }
        g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
        g.add_mana(0, ir::mana::ManaSymbol::Colorless);
        let with = crate::state::Pool { colorless: 1, colors: [0,0,0,1,0], ..Default::default() };
        g.act(0, Move::Paying(with, Box::new(Move::WithCosts(vec![vec![card]], Box::new(Move::Cast { card: thrill, targets: vec![] }))))).unwrap();
        g.became(thrill).expect("casting creates the stack incarnation (400.7)")
    }

    #[test]
    fn r616_1_discard_cost_retains_both_destinations_and_cloned_once_only_payment() {
        // 616.1e–f, 614.15: Colossus is a static replacement, not a self-replacement.
        // 118.11: discarding into either replacement destination still pays Thrill's cost.
        let mut g = competition();
        let old_rip = g.put(0, 1, Zone::Battlefield);
        let hand_rip = g.move_to(old_rip, Zone::Hand, 0).unwrap();
        g.move_to(hand_rip, Zone::Exile, 0).unwrap();
        let rip = g.put(0, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        let thrill = start_thrill(&mut g, card);
        let alternatives = [id(card, 2), id(rip, 1)];
        offered(&g, card, &alternatives);
        assert!(discard_events(&g).is_empty());
        assert!(!g.log.iter().any(|event| matches!(event, crate::Event::Cast { .. })), "casting is not complete while its discard cost waits");
        assert_eq!(g.obj(card).zone, Zone::Hand);
        let snapshot = format!("{g:?}");
        assert_eq!(g.act(1, Move::Choose(Answer::Replacement(alternatives[1].clone()))).unwrap_err(), "that decision is another player's");
        assert_eq!(format!("{g:?}"), snapshot);
        assert_eq!(g.act(0, Move::Choose(Answer::Replacement(id(old_rip, 1)))).unwrap_err(), "choose one of the replacement effects that apply (616.1)");
        assert_eq!(format!("{g:?}"), snapshot);
        let other = g.clone();
        for (mut branch, selected, destination, size) in [(g, alternatives[0].clone(), Zone::Library, 4), (other, alternatives[1].clone(), Zone::Exile, 3)] {
            branch.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
            assert!(branch.decision.is_none());
            assert_eq!(branch.stack, vec![thrill]);
            assert_eq!(branch.log.iter().filter(|event| matches!(event, crate::Event::Cast { .. })).count(), 1);
            assert_eq!(branch.players[0].pool, crate::state::Pool::default());
            let events = discard_events(&branch);
            assert_eq!(events.len(), 1);
            assert!(!events[0].1);
            assert_eq!(branch.obj(events[0].0).card_id, branch.obj(card).card_id);
            assert!(branch.log.iter().any(|event| matches!(event, crate::Event::ZoneChanged { was, to, .. } if *was == card && *to == destination)));
            assert_eq!(branch.players[0].library.iter().filter(|o| branch.obj(**o).card_id == branch.obj(card).card_id).count(), usize::from(destination == Zone::Library));
            assert_eq!(branch.players[0].library.len(), size);
            for _ in 0..2 { branch.act(branch.priority, Move::Pass).unwrap(); }
            assert_eq!(branch.players[0].hand.len(), 2);
            assert!(branch.stack.is_empty());
            assert_eq!(discard_events(&branch).len(), 1);
            assert_eq!(branch.players[0].library.len(), size - 2);
        }
    }

    #[test]
    fn r702_35_discard_cost_competes_with_rest_in_peace_before_madness_applies() {
        // 702.35a, 616.1f: only choosing madness makes this exile trigger its second ability.
        let mut g = competition();
        let rip = g.put(0, 1, Zone::Battlefield);
        let temper = g.put(0, 3, Zone::Hand);
        let thrill = start_thrill(&mut g, temper);
        let alternatives = [id(temper, 1), id(rip, 1)];
        offered(&g, temper, &alternatives);
        let other = g.clone();
        for (mut branch, selected, madness) in [(g, alternatives[0].clone(), true), (other, alternatives[1].clone(), false)] {
            branch.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
            let discarded = discard_events(&branch);
            assert_eq!(discarded.len(), 1);
            assert_eq!(discarded[0].1, madness);
            assert_eq!(branch.obj(discarded[0].0).zone, Zone::Exile);
            assert_eq!(branch.stack.len(), if madness { 2 } else { 1 });
            assert_eq!(branch.stack[0], thrill);
            if madness { assert_eq!(branch.obj(branch.stack[1]).ability.and_then(|(source,_)| source), Some(discarded[0].0)); }
            assert_eq!(branch.players[0].pool, crate::state::Pool::default());
        }
    }

    #[test]
    fn r514_1_cleanup_finishes_only_after_the_authored_discard_replacement() {
        // 514.1 precedes 514.2. The damaged permanent stays damaged while the choice waits.
        let mut g = competition();
        let rip = g.put(0, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        for _ in 0..7 { g.put(0, 5, Zone::Hand); }
        for p in 0..2 { for _ in 0..4 { g.put(p, 5, Zone::Library); } }
        let marked = g.put(0, 4, Zone::Battlefield);
        g.obj_mut(marked).damage = 1;
        g.begin_part(ir::Part::Cleanup).unwrap();
        g.act(0, Move::Discard(vec![card])).unwrap();
        let alternatives = [id(card, 2), id(rip, 1)];
        offered(&g, card, &alternatives);
        assert_eq!(g.obj(marked).damage, 1);
        assert_eq!(g.players[0].hand.len(), 8);
        let other = g.clone();
        for (mut branch, selected, zone) in [(g, alternatives[0].clone(), Zone::Library), (other, alternatives[1].clone(), Zone::Exile)] {
            branch.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
            assert_eq!(branch.players[0].hand.len(), 7);
            assert_eq!(branch.obj(marked).damage, 0);
            assert_eq!(discard_events(&branch).len(), 1);
            assert_eq!(branch.obj(discard_events(&branch)[0].0).card_id, branch.obj(card).card_id);
            assert!(branch.log.iter().any(|event| matches!(event, crate::Event::ZoneChanged { was, to, .. } if *was == card && *to == zone)));
            assert_eq!(branch.active, 1);
        }
    }

    #[test]
    fn r702_52_dredge_retains_milling_choices_before_one_return_and_no_draw() {
        // 702.52a, 701.17c: replaced milling still pays dredge; no ordinary draw happens.
        let mut g = competition();
        let rip = g.put(0, 1, Zone::Battlefield);
        crate::play::check(&g.defs[4]).expect("full Stinkweed Imp fixture must be admitted");
        let dredger = g.put(0, 4, Zone::Graveyard);
        for _ in 0..4 { g.put(0, 5, Zone::Library); }
        let colossus = g.put(0, 0, Zone::Library);
        g.draw(0, 1);
        let dredge = id(dredger, 2);
        assert!(matches!(&g.decision, Some(crate::state::Decision::Choose { player: 0, choice: crate::state::Choice::Replacement { among, optional: true, .. } }) if among == &vec![dredge.clone()]));
        g.act(0, Move::Choose(Answer::Replacement(dredge))).unwrap();
        assert!(matches!(&g.decision, Some(crate::state::Decision::Choose { player: 0, choice: crate::state::Choice::Replacement { among, .. } }) if among == &vec![id(colossus, 2), id(rip, 1)]));
        assert_eq!(g.players[0].library.len(), 5);
        assert!(g.players[0].hand.is_empty());
        assert_eq!(g.obj(dredger).zone, Zone::Graveyard);
        let other = g.clone();
        for (mut branch, selected, library_size) in [(g, id(colossus, 2), 1), (other, id(rip, 1), 0)] {
            branch.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
            assert!(branch.decision.is_none());
            assert_eq!(branch.players[0].hand.len(), 1);
            assert_eq!(branch.obj(branch.players[0].hand[0]).chars.name, "Stinkweed Imp");
            assert_eq!(branch.players[0].library.len(), library_size);
            assert_eq!(branch.log.iter().filter(|event| matches!(event, crate::Event::Milled { .. })).count(), 5);
            assert!(!branch.log.iter().any(|event| matches!(event, crate::Event::Drew { .. })));
            assert!(branch.stack.is_empty());
            assert_eq!(branch.players[0].life, 20);
            assert_eq!(branch.players[0].hand.len(), 1);
        }
    }


    #[test]
    fn r616_1_cleanup_retains_public_choices_without_partial_discards() {
        // 101.4, 616.1: each card's affected owner chooses; the simultaneous discards
        // remain proposals until every choice has been made. 514.2 waits for them all.
        let mut g = competition();
        let rip = g.put(0, 1, Zone::Battlefield);
        let first = g.put(0, 0, Zone::Hand);
        let second = g.put(0, 0, Zone::Hand);
        for _ in 0..7 { g.put(0, 5, Zone::Hand); }
        for p in 0..2 { for _ in 0..4 { g.put(p, 5, Zone::Library); } }
        let marked = g.put(0, 4, Zone::Battlefield);
        g.obj_mut(marked).damage = 1;
        g.begin_part(ir::Part::Cleanup).unwrap();
        g.act(0, Move::Discard(vec![first, second])).unwrap();
        offered(&g, first, &[id(first, 2), id(rip, 1)]);
        g.act(0, Move::Choose(Answer::Replacement(id(rip, 1)))).unwrap();
        offered(&g, second, &[id(second, 2), id(rip, 1)]);
        assert_eq!(g.public_choices().cloned().collect::<Vec<_>>(), vec![crate::state::PublicChoice::Replacement {
            player: 0, on: crate::Target::Object(first), effect: id(rip, 1),
        }]);
        assert!(discard_events(&g).is_empty());
        assert_eq!(g.players[0].hand.len(), 9);
        assert_eq!(g.obj(marked).damage, 1);
        let before = format!("{g:?}");
        assert_eq!(g.act(0, Move::Choose(Answer::Replacement(id(first, 2)))).unwrap_err(), "choose one of the replacement effects that apply (616.1)");
        assert_eq!(format!("{g:?}"), before);
        let other = g.clone();
        for (mut branch, selected, library_size) in [(g, id(second, 2), 5), (other, id(rip, 1), 4)] {
            branch.act(0, Move::Choose(Answer::Replacement(selected))).unwrap();
            assert_eq!(discard_events(&branch).len(), 2);
            assert_eq!(branch.players[0].library.len(), library_size);
            assert_eq!(branch.players[0].hand.len(), 7);
            assert_eq!(branch.obj(marked).damage, 0);
            assert_eq!(branch.active, 1);
        }
    }

}
