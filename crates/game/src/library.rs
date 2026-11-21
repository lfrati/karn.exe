//! Retained owner ordering (401.4, 404.3) and library placement (701.24).
use crate::event::Target;
use crate::state::{Answer, Choice, Game, ObjId, PlayerId};
use ir::action::{LibraryPosition, Order};
use ir::Verb;
/// Shared placement preparation: owners order their groups in APNAP order, before any move.
/// Keeping preparation separate lets every caller suspend without replaying a mutation.
pub(crate) fn library_groups(game: &Game, cards: Vec<ObjId>) -> Vec<(PlayerId, Vec<ObjId>)> {
    let mut groups = vec![Vec::new(); game.players.len()];
    for card in cards {
        if game.takes(Verb::Move, ir::Role::What, Target::Object(card)) {
            let group = &mut groups[game.obj(card).owner];
            if !group.contains(&card) { group.push(card); }
        }
    }
    (0..groups.len()).map(|i| {
        let owner = (game.active + i) % groups.len();
        (owner, std::mem::take(&mut groups[owner]))
    }).collect()
}

/// An owner's permutation of an incoming block, top first. Every owner answers before
/// any card moves; callers own final destinations, placement and their continuations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ordering {
    groups: Vec<(PlayerId, Vec<ObjId>)>,
    next: usize,
}
impl Ordering {
    pub(crate) fn new(game: &Game, cards: Vec<ObjId>) -> Self {
        Self { groups: library_groups(game, cards), next: 0 }
    }
    pub(crate) fn exclude(&mut self, excluded: &[ObjId]) {
        for (_, cards) in &mut self.groups { cards.retain(|c| !excluded.contains(c)); }
    }
    pub(crate) fn groups(&self) -> &[(PlayerId, Vec<ObjId>)] { &self.groups }
    /// Every owner's cards in the order they were given, each onto the ones before it, the
    /// last the top card: the order is already chosen, and nobody is asked.
    pub(crate) fn as_given(&mut self) {
        for (_, cards) in &mut self.groups { cards.reverse(); }
        self.next = self.groups.len();
    }
    pub(crate) fn pending(&self) -> Option<(PlayerId, &[ObjId])> {
        self.groups.get(self.next).map(|(owner, cards)| (*owner, cards.as_slice()))
    }
    pub(crate) fn answer(&mut self, answer: Answer) {
        let Some((_, cards)) = self.groups.get_mut(self.next) else { return };
        if let Answer::Objects(ordered) = answer
            && ordered.len() == cards.len()
            && ordered.iter().enumerate().all(|(i, card)| cards.contains(card) && !ordered[..i].contains(card)) {
            *cards = ordered;
            self.next += 1;
        }
    }
    pub(crate) fn prepare(&mut self, placement: impl Fn(usize) -> crate::state::Placement,
                         by: Option<PlayerId>, required: bool) -> Option<crate::state::Decision> {
        while let Some((owner, cards)) = self.groups.get(self.next) {
            if cards.len() > 1 && required {
                return Some(crate::state::Decision::Choose { player: by.unwrap_or(*owner),
                    choice: Choice::OrderCards { to: placement(self.next), cards: cards.clone() } });
            }
            self.next += 1;
        }
        None
    }
}

pub(crate) fn place_library_groups(
    game: &mut Game, groups: Vec<(PlayerId, Vec<ObjId>)>, position: LibraryPosition,
    order: Option<Order>, known: &[ObjId], before_shuffle: bool,
) -> Vec<ObjId> {
    let mut placed = Vec::new();
    for (owner, mut cards) in groups {
        if before_shuffle {
            // ShuffleInto's insertion is an implementation detail: the following full
            // shuffle is its sole random event (701.24c-d). Preserve the historical
            // seeded stream, but recorded replay needs only that shuffle's outcome.
            if !game.random.requires_recorded() { game.random.shuffle(&mut cards); }
        } else if order == Some(Order::Random) {
            game.random_library_order(owner, &mut cards, position);
        }
        game.place_in_library(owner, &cards, position, known);
        placed.extend(cards);
    }
    placed
}


/// Retained ordering: each owner answers once, all answers precede the simultaneous move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placement {
    ordering: Ordering,
    position: LibraryPosition,
    order: Option<Order>,
    before_shuffle: bool,
    known: Vec<ObjId>,
    /// The player the text has put them "in any order", for every library; None: each owner.
    by: Option<PlayerId>,
    returns: crate::commander::Returns,
    destination: Option<([LibraryPosition; 2], Option<PlayerId>)>,
    destinations: Vec<LibraryPosition>,
    choosing: usize,
    public_cards: Vec<ObjId>,
}
impl Placement {
    pub(crate) fn choices(&self) -> &[crate::state::PublicChoice] { &self.returns.choices }
    pub(crate) fn new(game: &Game, cards: Vec<ObjId>, position: LibraryPosition, order: Option<Order>, known: Vec<ObjId>) -> Self {
        Self { ordering: Ordering::new(game, cards), position, order, before_shuffle: false, known, by: None, returns: Default::default(), destination: None, destinations: Vec::new(), choosing: 0, public_cards: Vec::new() }
    }
    /// Internal insertion immediately followed by ShuffleInto's full-library shuffle.
    /// No owner ordering choice or separate random-order receipt belongs to insertion.
    pub(crate) fn before_shuffle(self, before_shuffle: bool) -> Self { Self { before_shuffle, ..self } }

    /// Only callers that actually move the cards own destination choices. Departure
    /// batches use this type for ordering alone and already own those replacements.
    pub(crate) fn moving(mut self, game: &Game) -> Self {
        self.returns = crate::commander::Returns::new(game, self.ordering.groups.iter().flat_map(|(_, cards)| cards.iter().map(|c| (*c, crate::Zone::Library))));
        self
    }
    pub(crate) fn exclude(&mut self, excluded: &[ObjId]) {
        self.ordering.exclude(excluded);
    }
    pub(crate) fn destination(mut self, game: &Game, destination: Option<([LibraryPosition; 2], Option<PlayerId>)>) -> Self {
        self.destination = destination;
        if destination.is_some() {
            self.public_cards = self.ordering.groups.iter().flat_map(|(_, cards)| cards.iter().copied())
                .filter(|card| (0..game.players.len()).all(|player| game.perceives(player, *card))).collect();
        }
        self.destinations = vec![self.position; self.ordering.groups.len()];
        self
    }
    pub(crate) fn ordered_by(self, by: Option<PlayerId>) -> Self { Self { by, ..self } }
    pub(crate) fn answer(&mut self, answer: Answer) {
        if self.returns.decision().is_some() { self.returns.answer(answer); return; }
        if let Some((between, chooser)) = self.destination
            && self.choosing < self.ordering.groups.len() {
            if let Answer::LibraryPosition(position) = answer && between.contains(&position) {
                let (owner, group) = &self.ordering.groups[self.choosing];
                let mut cards: Vec<_> = if chooser.is_some() { self.ordering.groups.iter().flat_map(|(_, cards)| cards.iter().copied()).collect() } else { group.clone() };
                cards.retain(|card| self.public_cards.contains(card));
                self.returns.choices.push(crate::state::PublicChoice::LibraryDestination {
                    player: chooser.unwrap_or(*owner), cards, position,
                });
                if chooser.is_some() {
                    // One specified player chooses a destination for the whole instruction,
                    // rather than independently partitioning its cards by library owner.
                    self.destinations.fill(position);
                    self.choosing = self.ordering.groups.len();
                } else {
                    self.destinations[self.choosing] = position;
                    self.choosing += 1;
                }
            }
            return;
        }
        self.ordering.answer(answer);
    }
    /// Collect order without committing; destination replacements share this preparation.
    pub(crate) fn prepare(&mut self) -> Option<crate::state::Decision> {
        self.ordering.prepare(|index| crate::state::Placement::Library(self.destinations.get(index).copied().unwrap_or(self.position)),
            self.by, !self.before_shuffle && self.order != Some(Order::Random))
    }
    pub(crate) fn advance(&mut self, game: &mut Game) -> Option<crate::state::Decision> {
        if let Some(decision) = self.returns.decision() { return Some(decision); }
        let diverted = self.returns.selected.clone();
        self.exclude(&diverted);
        if let Some((between, chooser)) = self.destination {
            while let Some((owner, cards)) = self.ordering.groups.get(self.choosing) {
                if cards.is_empty() { self.choosing += 1; continue; }
                return Some(crate::state::Decision::Choose { player: chooser.unwrap_or(*owner),
                    choice: Choice::LibraryDestination { cards: if chooser.is_some() { self.ordering.groups.iter().flat_map(|(_, cards)| cards.iter().copied()).collect::<Vec<_>>() } else { cards.clone() }
                        .into_iter().filter(|card| game.perceives(chooser.unwrap_or(*owner), *card)).collect(), positions: between.to_vec() } });
            }
        }
        if let Some(decision) = self.prepare() { return Some(decision); }
        for object in diverted { game.move_object(object, crate::Zone::Command); }
        for (index, group) in std::mem::take(&mut self.ordering.groups).into_iter().enumerate() {
            let position = self.destinations.get(index).copied().unwrap_or(self.position);
            place_library_groups(game, vec![group], position, self.order, &self.known, self.before_shuffle);
        }
        None
    }
}
