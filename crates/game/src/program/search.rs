//! A search retains its private candidate set and chosen result (701.23): of a library, or of
//! several of a player's zones at once ("graveyard, hand, and library"), one choice of cards
//! among all of them.
use super::{expr, Frame};
use crate::{Res, unsupported, Target};
use crate::select::Support;
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action { set: expr::Cards, min: expr::Amount, max: expr::Amount, bound: u32, qualified: bool, zones: Vec<crate::Zone> }
impl Action {
    /// `zones`: the zones of each player searched, in the order the text names them.
    pub(super) fn compile(s: &ir::Selector, zones: Vec<crate::Zone>, support: &Support<'_>, bind: &mut dyn expr::Binding) -> Res<Self> {
        if s.chooser.is_some() || s.random.is_some() { return unsupported("search: another chooser or random choice"); }
        let ir::Quant::Indefinite(count) = &s.quant else { return unsupported("search: a quantity of cards is required"); };
        let (min, max) = match count {
            ir::Count::Exactly(n) => (expr::Amount::compile(n, support, bind)?, expr::Amount::compile(n, support, bind)?),
            ir::Count::UpTo(n) => (expr::Amount::constant(0), expr::Amount::compile(n, support, bind)?),
            ir::Count::AnyNumber => (expr::Amount::constant(0), expr::Amount::constant(u32::MAX)),
            _ => return unsupported("search: cardinality not lowered"),
        };
        let bound = match count { ir::Count::Exactly(ir::Amount::Const(n)) | ir::Count::UpTo(ir::Amount::Const(n)) => *n, _ => u32::MAX };
        Ok(Self { set: expr::Cards::compile(&s.set, support, bind)?, min, max, bound, qualified: s.set.filter != ir::Filter::All(Vec::new()), zones })
    }
    pub(super) fn max_bound(&self) -> u32 { self.bound }
    pub(super) fn source_zone(&self) -> Option<crate::Zone> {
        match self.zones.as_slice() { [zone] => Some(*zone), _ => None }
    }
    pub(super) fn scalars(&self) -> Vec<crate::binding::Scalar> {
        self.set.scalars().into_iter().chain(self.min.scalars()).chain(self.max.scalars()).collect()
    }
    /// These searches, one after another: each of a searcher and the player whose zones
    /// they search.
    pub(super) fn begin(&self, searches: Vec<(PlayerId, PlayerId)>) -> Pending {
        Pending { player: searches.first().map_or(0, |(p, _)| *p), libraries: searches, choice: None, found: Vec::new() }
    }
    fn choice(&self, game: &Game, frame: &Frame, whose: PlayerId) -> Choice {
        let p = &game.players[whose];
        let cards: Vec<_> = self.zones.iter().flat_map(|z| match z {
            crate::Zone::Graveyard => p.graveyard.iter(),
            crate::Zone::Hand => p.hand.iter(),
            _ => p.library.iter(),
        }).copied().filter(|o| game.is(Target::Object(*o), ir::Sort::Card)).collect();
        let among: Vec<_> = cards.iter().copied().filter(|o| self.set.contains(game, &frame.ctx, frame, *o)).collect();
        let min = if self.qualified { 0 } else { (self.min.evaluate(game, &frame.ctx, frame) as usize).min(among.len()) };
        let max = (self.max.evaluate(game, &frame.ctx, frame) as usize).min(among.len());
        Choice::Search { cards, among, min, max }
    }
}
/// A search being made: the player searching now, the searches not yet made (by whom, of
/// whose zones), the one being made, and what was found in those made before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending { player: PlayerId, libraries: Vec<(PlayerId, PlayerId)>, choice: Option<Choice>, found: Vec<ObjId> }
impl Pending {
    /// Begin searching the next player's zones, when none is being searched: the cards the
    /// player now looks through, hidden ones too (701.23a).
    pub(super) fn open(&mut self, action: &Action, game: &Game, frame: &Frame) -> Option<Vec<ObjId>> {
        if self.choice.is_some() || self.libraries.is_empty() { return None; }
        let (player, whose) = self.libraries.remove(0);
        self.player = player;
        let choice = action.choice(game, frame, whose);
        let Choice::Search { cards, .. } = &choice else { unreachable!() };
        let cards = cards.clone();
        self.choice = Some(choice);
        Some(cards)
    }
    pub(super) fn decision(&self) -> Option<Decision> { self.choice.clone().map(|choice| Decision::Choose { player: self.player, choice }) }
    pub(super) fn player(&self) -> PlayerId { self.player }
    /// Every card found, once every library has been searched.
    pub(super) fn result(&self) -> &[ObjId] { &self.found }
    pub(super) fn answer(&mut self, answer: Answer) {
        let Some(Choice::Search { among, min, max, .. }) = &self.choice else { unreachable!() };
        if let Answer::Objects(cards) = answer
            && (*min..=*max).contains(&cards.len())
            && cards.iter().enumerate().all(|(i, card)| among.contains(card) && !cards[..i].contains(card)) {
            self.found.extend(cards);
            self.choice = None;
        }
    }
}
