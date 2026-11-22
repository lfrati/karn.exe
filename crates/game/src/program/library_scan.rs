//! Exile one current library top at a time, retaining actual results (701.57a, 702.85a).
//! The optional cast and random-bottom continuation are ordinary program instructions.
use crate::{Game, ObjId, PlayerId, Zone};
use crate::state::Decision;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    pub player: PlayerId,
    pub bound: u32,
    pub limit: super::ScanLimit,
    source: Option<ObjId>,
    ability: Option<crate::binding::AbilityKey>,
    generation: Option<usize>,
    pub exiled: Vec<ObjId>,
    pub hit: Option<ObjId>,
    /// 701.57c's final qualifying exiled card need not be the nonland hit.
    pub designated: Option<ObjId>,
    waiting: Option<ObjId>,
    result: Option<crate::operation::Completed>,
    done: bool,
}
impl Pending {
    pub(super) fn new(player: PlayerId, bound: u32, limit: super::ScanLimit, ctx: &crate::select::Ctx) -> Self {
        Self { player, bound, limit, source: ctx.source, ability: ctx.ability.clone(), generation: ctx.link_generation,
            exiled: Vec::new(), hit: None, designated: None, waiting: None, result: None, done: false }
    }
    pub(super) fn operation_complete(&mut self, result: crate::operation::Completed) {
        assert!(self.waiting.is_some(), "library scan child belongs to one retained top card");
        assert!(self.result.is_none(), "library scan child completion is delivered once");
        self.result = Some(result);
    }
    pub(super) fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        loop {
            if let Some(result) = self.result.take() {
                let before = self.waiting.take().expect("completed library scan move has a retained top");
                let after = result.changed().find_map(|(old, now)| (old == before).then_some(now));
                match after {
                    Some(now) if game.obj(now).zone == Zone::Exile => {
                        self.exiled.push(now);
                        let small = self.limit.allows(game.obj(now).chars.mana_value, self.bound);
                        self.designated = small.then_some(now);
                        if small && !game.is_a(now, ir::CardType::Land) { self.hit = Some(now); self.done = true; }
                    }
                    Some(now) if game.obj(now).zone == Zone::Library =>
                        return crate::unsupported("library scan: library-returning exile replacement not implemented"),
                    Some(_) => {}, // an actual replacement moved it elsewhere; no exiled hit
                    None => return crate::unsupported("library scan: allowed library exile made no progress"),
                }
            }
            assert!(self.waiting.is_none(), "library scan cannot resume before its departure child completes");
            if self.done || game.players[self.player].lost { return Ok(None); }
            let Some(top) = game.top_cards(self.player, 1).first().copied() else { self.done = true; return Ok(None) };
            // Current action prohibitions are persistent query facts; no replacement is
            // consumed by this query. Unexpected nonprogress after an allowed operation
            // is refused above, rather than assuming every future replacement impossible.
            if !game.can(ir::Verb::Exile, self.player, crate::Target::Object(top)) {
                self.done = true; return Ok(None);
            }
            let kind = match self.source {
                Some(source) => crate::departure::Kind::Exile { source, ability: self.ability.clone(), generation: self.generation },
                None => crate::departure::Kind::Move(Zone::Exile),
            };
            self.waiting = Some(top);
            match game.departure_batch(vec![(top, kind)])? {
                Ok(result) => self.operation_complete(result),
                Err(decision) => return Ok(Some(decision)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ctx() -> crate::select::Ctx<'static> { let mut ctx=crate::select::Ctx::new(0, crate::ObjId(0));ctx.source=None;ctx }
    fn game() -> Game {
        Game::new(vec![
            crate::CardDef::new("Land", "", "Basic Land — Forest", "", None).unwrap(),
            crate::CardDef::new("Huge", "{6}", "Creature — Wurm", "", Some((6,6))).unwrap(),
            crate::CardDef::new("Bear", "{2}", "Creature — Bear", "", Some((2,2))).unwrap(),
        ],2,20)
    }
    #[test]
    fn r701_57c_designation_is_final_actual_exiled_card_not_nonland_hit() {
        for indices in [vec![],vec![0,0],vec![1,0],vec![0,1],vec![2,0]] {
            let mut g=game();for index in &indices {g.put(0,*index,Zone::Library);}
            let mut scan=Pending::new(0,3,super::super::ScanLimit::AtMost,&ctx());
            assert!(scan.advance(&mut g).unwrap().is_none());
            // 701.57a excludes lands from the stopping condition; 701.57c does
            // not exclude the last land of an exhausted all-land library.
            assert_eq!(scan.hit.is_some(),indices.contains(&2));
            let expected=indices.first().copied().filter(|i|*i!=1);
            assert_eq!(scan.designated.and_then(|o|g.obj(o).card),expected);
            assert_eq!(scan.exiled.len(),indices.len());
        }
    }
    #[test]
    fn r701_57a_child_result_resumes_retained_top_once_and_uses_actual_destination() {
        let mut g=game();let old=g.put(0,2,Zone::Library);
        let mut scan=Pending::new(0,3,super::super::ScanLimit::AtMost,&ctx());
        // Synthetic parent/child handoff: current departure replacements only
        // ask for graveyard moves, so no real library-exile chooser is claimed.
        scan.waiting=Some(old);g.move_object(old,Zone::Exile);let now=g.exile[0];
        scan.operation_complete(crate::operation::Completed{moved:vec![(old,now)],..Default::default()});
        assert!(scan.advance(&mut g).unwrap().is_none());
        assert_eq!((scan.exiled.clone(),scan.hit,scan.designated),(vec![now],Some(now),Some(now)));
        assert!(scan.advance(&mut g).unwrap().is_none());assert_eq!(scan.exiled,vec![now]);
        // A move elsewhere is not an exile and cannot designate a discovered card.
        let mut g=game();let old=g.put(0,2,Zone::Library);let mut scan=Pending::new(0,3,super::super::ScanLimit::AtMost,&ctx());
        scan.waiting=Some(old);g.move_object(old,Zone::Hand);let now=g.players[0].hand[0];
        scan.operation_complete(crate::operation::Completed{moved:vec![(old,now)],..Default::default()});
        assert!(scan.advance(&mut g).unwrap().is_none());assert!(scan.exiled.is_empty());assert_eq!((scan.hit,scan.designated),(None,None));
    }
    #[test]
    fn unsupported_nonprogress_is_not_silently_completed_discover() {
        let mut g=game();let old=g.put(0,2,Zone::Library);let mut scan=Pending::new(0,3,super::super::ScanLimit::AtMost,&ctx());
        scan.waiting=Some(old);scan.operation_complete(Default::default());
        assert_eq!(scan.advance(&mut g).unwrap_err().0,"library scan: allowed library exile made no progress");
        let mut scan=Pending::new(0,3,super::super::ScanLimit::AtMost,&ctx());scan.waiting=Some(old);
        scan.operation_complete(crate::operation::Completed{moved:vec![(old,old)],..Default::default()});
        assert_eq!(scan.advance(&mut g).unwrap_err().0,"library scan: library-returning exile replacement not implemented");
    }
}
