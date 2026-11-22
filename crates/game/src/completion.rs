//! Successful stack departure (608.2n), including alternative completion replacements.
use crate::{Game, ObjId, Zone};
use crate::state::{Answer, Choice, Decision, DestinationEffect, PublicChoice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Destination {
    pub spell: ObjId,
    controller: usize,
    among: Vec<DestinationEffect>,
    answer: Option<DestinationEffect>,
    redirects: Vec<crate::departure::Destination>,
    revealed: Vec<ObjId>,
}
impl Destination {
    pub(crate) fn new(game: &Game, spell: ObjId, revealed: Vec<ObjId>) -> Self {
        let object = game.obj(spell);
        let mut among = Vec::new();
        // Flashback replaces every departure, so all orders end in exile (702.34a).
        let flashback = object.flashback.is_some_and(|(scope, _)| scope.contains(object.zone));
        let completion = match object.cast_as {
            crate::card::CastAs::Omen => Some(DestinationEffect::Omen),
            crate::card::CastAs::Adventure => Some(DestinationEffect::Adventure),
            _ => None,
        };
        if let Some(effect) = completion && object.zone == Zone::Stack && object.ability.is_none() {
            among.push(effect);
            // Flashback can still change the Omen's shuffle or Adventure's permission.
            // The affected controller chooses the first replacement (616.1e).
            if flashback { among.push(DestinationEffect::Flashback); }
        }
        if !flashback && object.zone == Zone::Stack && object.ability.is_none() {
            if object.buyback_paid && game.has(spell, "Buyback") { among.push(DestinationEffect::Buyback); }
            if game.rebound_applies(spell) { among.push(DestinationEffect::Rebound); }
            if object.cast_rider.as_ref().is_some_and(|r| r.exiles(game, spell)) { among.push(DestinationEffect::CastRider); }
        }
        // Reuse general graveyard redirects and their captured side effects. They compete
        // with successful-completion replacements (616.1), including buyback/rebound.
        let redirects = if !flashback && object.zone == Zone::Stack && object.ability.is_none() {
            game.redirects(spell, Zone::Graveyard)
        } else { Vec::new() };
        for i in 0..redirects.len() {
            if redirects[..i].iter().any(|d| d.outcome() == redirects[i].outcome()) { continue; }
            among.push(DestinationEffect::GraveyardReplacement(i));
        }
        Self { spell, controller: object.controller, among, answer: None, redirects, revealed }
    }
    fn depart(&self, game: &mut Game, zone: Zone) {
        let destination = game.departure_destination(self.spell, zone);
        game.record_departure(self.spell, crate::departure::Kind::RuleMove(zone), destination.zone);
        game.move_prepared(self.spell, destination, self.controller, crate::replace::Entry::default());
    }
    pub(crate) fn answer(&mut self, answer: Answer) {
        let Answer::SpellDestination { on, effect } = answer else { unreachable!("validated destination answer") };
        assert_eq!(on, self.spell);
        assert!(self.among.contains(&effect) && self.answer.is_none());
        self.answer = Some(effect);
    }
    /// After a chosen graveyard-only replacement, none of the others applies (616.1f).
    pub(crate) fn advance(&mut self, game: &mut Game, choices: &mut Vec<PublicChoice>) -> Option<Decision> {
        if self.among.len() > 1 && self.answer.is_none() {
            return Some(Decision::Choose { player: self.controller,
                choice: Choice::SpellDestination { on: self.spell, among: self.among.clone() } });
        }
        let chosen = self.answer.or_else(|| self.among.first().copied());
        if self.among.len() > 1 {
            choices.push(PublicChoice::SpellDestination { player: self.controller, on: self.spell, effect: chosen.unwrap() });
        }
        if game.obj(self.spell).zone != Zone::Stack { return None; }
        match chosen {
            Some(DestinationEffect::Adventure) => {
                if let Some(card) = game.exile_result(self.spell) && game.obj(card).zone == Zone::Exile {
                    game.permit_adventure(self.controller, card);
                }
            }
            Some(DestinationEffect::Omen) => {
                let owner = game.obj(self.spell).owner;
                self.depart(game, Zone::Library);
                game.shuffle_library(owner, self.controller, &self.revealed);
            }
            Some(DestinationEffect::GraveyardReplacement(i)) => {
                let destination = self.redirects[i].clone();
                game.record_departure(self.spell, crate::departure::Kind::RuleMove(Zone::Graveyard), destination.zone);
                game.move_prepared(self.spell, destination, self.controller, crate::replace::Entry::default());
            }
            Some(DestinationEffect::Flashback) => { self.depart(game, Zone::Exile); }
            Some(DestinationEffect::Rebound) => {
                if let Some(card) = game.exile_result(self.spell) && game.obj(card).zone == Zone::Exile {
                    game.rebound_exiled(self.spell, card);
                }
            }
            Some(DestinationEffect::Buyback) => { game.move_result(self.spell, Zone::Hand); }
            Some(DestinationEffect::CastRider) => { game.move_to(self.spell, Zone::Exile, self.controller); }
            None => { self.depart(game, Zone::Graveyard); }
        }
        None
    }
}

/// Whose effect a spell destination effect is, by which the player choosing among them
/// names it (616.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestinationSource {
    /// An object's ability: the spell's own keyword ability (buyback 702.27, rebound 702.88,
    /// flashback 702.34a), or the one whose offer it was cast under ("exile it instead").
    /// `ability`: None where the object's ability is not known.
    Ability { source: Option<ObjId>, ability: Option<crate::binding::AbilityKey> },
    /// A replacement effect redirecting it from the graveyard, as replacements are named.
    Replacement(crate::operation::ReplacementId),
    /// A rule's: an Omen's shuffle (720.3d) or Adventure's exile (715.3d).
    Rule(&'static str),
}

impl Game {
    /// The spell's own ability whose keyword is this one, as the abilities it has now say
    /// it ([`Game::members`]): the row that holds it.
    fn keyword_row(&self, spell: ObjId, keyword: &str) -> Option<crate::binding::AbilityKey> {
        self.members(spell).into_iter().find(|member| matches!(&**member,
            ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == keyword))).map(|member| member.key)
    }

    /// Whose effect this destination effect offered for `spell` is (616.1).
    pub fn spell_destination_source(&self, spell: ObjId, effect: DestinationEffect) -> DestinationSource {
        let own = |keyword: &str| DestinationSource::Ability { source: Some(spell), ability: self.keyword_row(spell, keyword) };
        match effect {
            DestinationEffect::Adventure => DestinationSource::Rule("715.3d"),
            DestinationEffect::Omen => DestinationSource::Rule("720.3d"),
            DestinationEffect::Buyback => own("Buyback"),
            DestinationEffect::Rebound => own("Rebound"),
            DestinationEffect::Flashback => own("Flashback"),
            DestinationEffect::CastRider => {
                let by = self.obj(spell).cast_rider.as_ref().and_then(|r| r.by.clone());
                let (source, ability) = by.unwrap_or((None, None));
                DestinationSource::Ability { source, ability }
            }
            DestinationEffect::GraveyardReplacement(i) => {
                // the redirects frozen when the question was asked, else as they are now
                let live = self.resolving().and_then(|r| r.destination.as_ref()).filter(|d| d.spell == spell).map(|d| d.redirects.clone());
                let redirects = live.unwrap_or_else(|| self.redirects(spell, Zone::Graveyard));
                let by = redirects.get(i).and_then(|d| d.by.as_ref()).expect("an offered graveyard redirect is a replacement effect");
                DestinationSource::Replacement(by.id.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef,Move,Event};
    fn fixture()->(Game,ObjId) {
        let d=CardDef::new("Completion fixture","{0}","Instant","Buyback {0}\nRebound\nYou gain 1 life.",None).unwrap();
        crate::play::check(&d).unwrap();let mut g=Game::new(vec![d],2,20);let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::WithBuyback(true,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();let spell=g.stack[0];
        // Explicit typed fixture: test the three existing replacement roles without inventing a text producer.
        g.obj_mut(spell).cast_rider=Some(crate::permission::Rider{types:vec![],by:None});(g,spell)
    }
    fn resolve(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
    #[test]
    fn all_three_completion_roles_freeze_once_and_only_rebound_creates_delay() {
        for effect in [DestinationEffect::Buyback,DestinationEffect::Rebound,DestinationEffect::CastRider] {
            let(mut g,spell)=fixture();resolve(&mut g);
            assert!(matches!(&g.decision,Some(Decision::Choose{player:0,choice:Choice::SpellDestination{on,among}}) if *on==spell&&among.len()==3));
            assert_eq!(g.players[0].life,21);let count=g.log.len();let clone=g.clone();
            for mut g in [g,clone] {g.act(0,Move::Choose(Answer::SpellDestination{on:spell,effect})).unwrap();
                assert_eq!(g.players[0].life,21);assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Resolved{spell:s} if *s==spell)).count(),1);
                assert!(g.log.len()>count);assert_eq!(g.delayed.len(),usize::from(effect==DestinationEffect::Rebound));
                assert_eq!(g.players[0].hand.len(),usize::from(effect==DestinationEffect::Buyback));
                assert_eq!(g.exile.len(),usize::from(effect!=DestinationEffect::Buyback));}
        }
    }
    #[test]
    fn affected_controller_chooses_but_buyback_returns_to_owner() {
        let(mut g,spell)=fixture();g.obj_mut(spell).controller=1;resolve(&mut g);
        assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::SpellDestination{..}})));
        // Typed destination-controller fixture, not a spell-control effect producer.
        let before = format!("{g:?}");
        assert_eq!(g.act(0,Move::Choose(Answer::SpellDestination{on:spell,effect:DestinationEffect::Buyback})),
            Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"), before);
        g.act(1,Move::Choose(Answer::SpellDestination{on:spell,effect:DestinationEffect::Buyback})).unwrap();
        assert_eq!(g.players[0].hand.len(),1);assert!(g.players[1].hand.is_empty());assert!(g.delayed.is_empty());
        assert_eq!(g.obj(g.players[0].hand[0]).owner, 0);
        assert!(g.stack.is_empty());assert!(g.decision.is_none());
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Resolved { spell: on } if *on == spell)).count(), 1);
    }
}
