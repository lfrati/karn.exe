//! Starting the game (CR 103): each player's deck is shuffled and becomes their library
//! (103.3), each player draws a hand of seven cards (103.5), and each in turn order, the
//! starting player first, declares whether to take a mulligan; those who do take it at the
//! same time: the hand is shuffled back, seven cards are drawn, and as many of them as the
//! player has taken mulligans go on the bottom of the library in any order. This repeats
//! until no player takes one (103.5). Opening-hand actions follow in turn order (103.6),
//! including any entry decisions, before the starting player takes the first turn (103.8).
//!
//! What is random in a game (a shuffle, 701.24a) comes from [`Random`], which is part of the
//! game: the same seed and the same choices are the same game.

use crate::state::{CardId, Decision, Game, ObjId, PlayerId, Zone};
use crate::CardDef;

/// "Each player draws a number of cards equal to their starting hand size, which is normally
/// seven" (103.5).
const STARTING_HAND_SIZE: u32 = 7;

/// The cards of a deck file by name, one for each copy: "4 [C14:204] Llanowar Elves" as the
/// files of `decks/` have it, or "4 Llanowar Elves". The lines of the sideboard ("SB: 2
/// [M14:171] Duress") are not in the starting deck (103.2a).
pub fn deck_list(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("SB:") {
            continue;
        }
        let Some((copies, rest)) = line.split_once(' ') else { continue };
        let Ok(copies) = copies.parse::<usize>() else { continue };
        // the printing, when the line says which
        let name = match rest.trim_start().strip_prefix('[') {
            Some(printed) => printed.split_once("] ").map_or("", |(_, name)| name),
            None => rest,
        };
        if !name.trim().is_empty() {
            names.extend(std::iter::repeat_n(name.trim().to_string(), copies));
        }
    }
    names
}

/// Which random order a seed gives ([`Random`]): SplitMix64 numbers, each below `n` by
/// rejection, shuffled Fisher–Yates from the last place down. A change to any of these, so
/// that one seed orders anything differently, is a new identity (a test pins its first
/// numbers). The same seed is the same game only with the same identity, the same decks in
/// the same order, the same choices, and an engine that asks for random numbers at the same
/// moments: a changed engine may draw them for something new.
pub const RANDOM: &str = "splitmix64-fisher-yates/1";

/// Where the outcomes of the shuffles that start a game (103.3) come from.
#[derive(Debug, Clone, Copy)]
pub enum Initial<'a> {
    /// Random numbers from this seed ([`Random`]).
    Seed(u64),
    /// For each player in deck order, the library order their deck's shuffle produced, top
    /// first, as positions into that player's deck list: a record of a game played
    /// elsewhere. No random number is drawn.
    Orders(&'a [Vec<usize>]),
}

/// A two-sided equally likely result (705.1). A call is a separate player choice (705.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinFace { Heads, Tails }
impl CoinFace {
    pub fn name(self) -> &'static str { match self { Self::Heads => "heads", Self::Tails => "tails" } }
}
/// A completed flip. Face-only flips have no call and no winner or loser (705.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoinFlip { pub player: PlayerId, pub call: Option<CoinFace>, pub face: CoinFace }
impl CoinFlip {
    pub fn won(self) -> Option<bool> { self.call.map(|call| call == self.face) }
}

/// Physical result observed at the random operation, before following instructions.
#[derive(Debug, Clone)]
pub enum CapturedRandom {
    Order { kind: RandomKind, player: PlayerId, position: Option<ir::action::LibraryPosition>, order: Vec<u64> },
    Coin { player: PlayerId, face: CoinFace },
}

/// Distinct permutation operations; a placement order is nearest the top first (401.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomKind {
    Shuffle,
    RandomOrder,
}

/// Trusted, owned random receipt. Object handles are resolved to physical cards before
/// execution, so intermediate zone changes cannot invalidate the receipt (400.7).
#[derive(Debug, Clone)]
pub enum RandomOutcome {
    Shuffle { player: PlayerId, order: Vec<ObjId> },
    /// A shuffle with an explicit assertion about every card kept out of its
    /// random permutation (701.24b/g). The order names only randomized cards.
    ScopedShuffle { player: PlayerId, order: Vec<ObjId>, scope: ShuffleScope },
    /// Membership determines its owner; an optional supplied hint must agree.
    RandomOrder { player: Option<PlayerId>, order: Vec<ObjId> },
    Coin { player: PlayerId, face: CoinFace },
}

/// Scope object handles bind to physical cards before execution; the scope is
/// checked when the shuffle consumes its receipt, after preceding instructions.
#[derive(Debug, Clone, Default)]
pub struct ShuffleScope {
    pub excluding: Vec<ObjId>,
    pub fixed: Vec<(ObjId, ir::action::LibraryPosition)>,
}

impl From<ShuffleOrder> for RandomOutcome {
    fn from(value: ShuffleOrder) -> Self {
        Self::Shuffle { player: value.player, order: value.order }
    }
}

/// Why no game was set up, and what kind of refusal that is: a deck the rules do not allow
/// is illegal (903.5a), a setup the engine does not carry out is unsupported, and setup
/// input that describes no game (one deck, a card no definition, an order that is no
/// permutation of its deck) is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused { pub class: crate::simulation::Class, pub why: String }
impl Refused {
    pub(crate) fn illegal(why: impl Into<String>) -> Self { Self { class: crate::simulation::Class::Illegal, why: why.into() } }
    pub(crate) fn unsupported(why: impl Into<String>) -> Self { Self { class: crate::simulation::Class::Unsupported, why: why.into() } }
    pub(crate) fn malformed(why: impl Into<String>) -> Self { Self { class: crate::simulation::Class::Malformed, why: why.into() } }
}
impl From<crate::Unsupported> for Refused { fn from(why: crate::Unsupported) -> Self { Self::unsupported(why.0) } }
impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.why) }
}
impl std::error::Error for Refused {}
/// What a refusal says, read as text.
impl PartialEq<&str> for Refused { fn eq(&self, other: &&str) -> bool { self.why == *other } }

/// Replay input is refused transactionally; it is not a rules-engine fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayInput(pub String);
impl std::fmt::Display for ReplayInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "replay input: {}", self.0)
    }
}
impl std::error::Error for ReplayInput {}

/// Abort only the trusted replay transaction, without invoking the panic hook.
/// Its outer boundary restores the simulation; genuine engine panics remain terminal.
pub(crate) fn reject_replay(message: impl Into<String>) -> ! {
    std::panic::resume_unwind(Box::new(ReplayInput(message.into())))
}

/// Where random order comes from: a sequence of numbers a seed determines (SplitMix64).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Random { state: u64, recorded_only: bool }

/// A trusted harness supplies the resulting library order, top first, at a shuffle.
/// Handles refer to current cards when the response starts, even if those cards
/// change zones before the shuffle. These are outcomes, never player choices.
#[derive(Debug, Clone)]
pub struct ShuffleOrder {
    pub player: PlayerId,
    pub order: Vec<ObjId>,
}

#[derive(Debug, Clone)]
pub struct UnusedRandom {
    pub input_index: usize,
    pub outcome: RandomOutcome,
}

#[derive(Debug, Clone)]
pub(crate) struct RecordedRandom {
    input_index: usize,
    player: PlayerId,
    result: RecordedResult,
}
#[derive(Debug, Clone)]
enum RecordedResult { Order { kind: RandomKind, cards: Vec<CardId>, scope: Option<Box<RecordedShuffleScope>> }, Coin(CoinFace) }
#[derive(Debug, Clone)]
struct RecordedShuffleScope {
    excluding: Vec<CardId>,
    fixed: Vec<(CardId, ir::action::LibraryPosition)>,
}

impl Random {
    pub fn new(seed: u64) -> Random {
        Random { state: seed, recorded_only: false }
    }

    pub(crate) fn recorded_only(&mut self, required: bool) { self.recorded_only = required; }

    pub(crate) fn requires_recorded(&self) -> bool { self.recorded_only }

    fn next(&mut self) -> u64 {
        if self.recorded_only { reject_replay("tape requested unrecorded randomness"); }
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `n`, each as likely as any other.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        let n = n as u64;
        // numbers past the last whole multiple of n would make the low ones likelier
        let whole = u64::MAX - u64::MAX % n;
        loop {
            let x = self.next();
            if x < whole {
                return (x % n) as usize;
            }
        }
    }

    /// Put these in a random order, every order as likely as any other (Fisher–Yates).
    pub(crate) fn shuffle<T>(&mut self, things: &mut [T]) {
        for i in (1..things.len()).rev() {
            things.swap(i, self.below(i + 1));
        }
    }
}

/// The mulligans of a game that has not begun its first turn (103.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Opening {
    /// How many mulligans each player has taken.
    taken: Vec<u32>,
    /// Who has an opening hand: they chose not to take a mulligan, or can take no more.
    kept: Vec<bool>,
    /// What each player has declared this time round: Some(true), a mulligan.
    declared: Vec<Option<bool>>,
    /// Who has taken this round's mulligan and has yet to put cards on the bottom.
    owing: Vec<PlayerId>,
}

impl Game {
    /// Privileged fixture edit, not a rules action or shuffle event. The supplied
    /// top-first order must contain exactly this library's current objects once each.
    pub fn set_library_order(&mut self, player: PlayerId, order: &[ObjId]) -> Result<(), String> {
        let library = &self.players.get(player).ok_or("no such player")?.library;
        let unique: std::collections::BTreeSet<_> = order.iter().copied().collect();
        if order.len() != library.len() || unique.len() != order.len()
            || !library.iter().all(|id| unique.contains(id)) {
            return Err("library order must be an exact permutation of its current objects".into());
        }
        self.players[player].library = order.iter().rev().copied().collect();
        Ok(())
    }

    pub(crate) fn record_shuffles(&mut self, orders: Vec<ShuffleOrder>) -> Result<(), String> {
        self.record_random(orders.into_iter().map(Into::into).collect())
    }

    pub(crate) fn record_random(&mut self, orders: Vec<RandomOutcome>) -> Result<(), String> {
        let mut recorded = std::collections::VecDeque::new();
        for (input_index, order) in orders.into_iter().enumerate() {
            let (kind, hint, order, scope) = match order {
                RandomOutcome::Shuffle { player, order } => (RandomKind::Shuffle, Some(player), order, None),
                RandomOutcome::ScopedShuffle { player, order, scope } => (RandomKind::Shuffle, Some(player), order, Some(scope)),
                RandomOutcome::RandomOrder { player, order } => (RandomKind::RandomOrder, player, order, None),
                RandomOutcome::Coin { player, face } => {
                    if player >= self.players.len() { return Err("no such random-outcome player".into()); }
                    recorded.push_back(RecordedRandom {input_index, player, result: RecordedResult::Coin(face)});
                    continue;
                }
            };
            let inferred = order.first().and_then(|id| self.get_object(*id)).map(|object| object.owner);
            let player = match (kind, hint, inferred) {
                (RandomKind::Shuffle, Some(player), _) => player,
                (RandomKind::Shuffle, None, _) => return Err("shuffle outcome needs player".into()),
                (RandomKind::RandomOrder, hint, owner) => owner.or(hint).unwrap_or(0),
            };
            if player >= self.players.len() { return Err("no such random-outcome player".into()); }
            if hint.is_some_and(|hint| hint != player) { return Err("random outcome owner differs from its cards".into()); }
            let physical = |id| -> Result<CardId,String> {
                let object = self.get_object(id).ok_or("random outcome names an unknown object")?;
                if object.zone == Zone::Gone || object.owner != player {
                    return Err("random outcome needs current cards owned by that player".into());
                }
                object.card_id.ok_or_else(|| "random outcome needs physical cards".into())
            };
            let mut cards = Vec::new();
            for id in order {
                let card = physical(id)?;
                if cards.contains(&card) { return Err("random outcome repeats a card".into()); }
                cards.push(card);
            }
            let scope = scope.map(|scope| -> Result<_,String> {
                let mut used = cards.clone();
                let mut read = |id| -> Result<CardId,String> {
                    let card = physical(id)?;
                    if used.contains(&card) {return Err("shuffle scope repeats a card or includes a randomized card".into());}
                    used.push(card);
                    Ok(card)
                };
                let excluding = scope.excluding.into_iter().map(&mut read).collect::<Result<_,_>>()?;
                let fixed = scope.fixed.into_iter().map(|(id,position)| {
                    if matches!(position,ir::action::LibraryPosition::FromTop(0)) {return Err("shuffle positions count from one".into());}
                    Ok((read(id)?,position))
                }).collect::<Result<_,String>>()?;
                Ok(Box::new(RecordedShuffleScope {excluding,fixed}))
            }).transpose()?;
            recorded.push_back(RecordedRandom { input_index, player, result: RecordedResult::Order {kind, cards, scope} });
        }
        self.replay_random = Some(recorded);
        self.random.recorded_only(true);
        Ok(())
    }

    /// Unconsumed input receipts, each card rebound to its current object.
    pub(crate) fn unconsumed_random(&self, unused: std::collections::VecDeque<RecordedRandom>) -> Vec<UnusedRandom> {
        unused.into_iter().map(|receipt| {
            let current = |card| self.audit_retained_objects()
                .find(|o| o.card_id == Some(card) && o.zone != Zone::Gone)
                .expect("a physical card has a current object").id;
            let outcome = match receipt.result {
                RecordedResult::Coin(face) => RandomOutcome::Coin {player:receipt.player,face},
                RecordedResult::Order {kind,cards,scope} => {
                    let order=cards.into_iter().map(current).collect();
                    match (kind,scope) {
                        (RandomKind::Shuffle,Some(scope))=>RandomOutcome::ScopedShuffle {player:receipt.player,order,scope:ShuffleScope {
                            excluding:scope.excluding.into_iter().map(current).collect(),
                            fixed:scope.fixed.into_iter().map(|(card,position)|(current(card),position)).collect(),
                        }},
                        (RandomKind::Shuffle,None)=>RandomOutcome::Shuffle {player:receipt.player,order},
                        (RandomKind::RandomOrder,_)=>RandomOutcome::RandomOrder {player:Some(receipt.player),order},
                    }
                }
            };
            UnusedRandom {input_index:receipt.input_index,outcome}
        }).collect()
    }

    /// A game from its start: these decks, each a list of cards by their place in `defs`,
    /// shuffled into libraries (103.3); each player at `life` (103.4) with a hand of seven
    /// (103.5); `first` is the starting player, however the players determined it (103.1).
    /// The game waits for the mulligan declarations; the first turn begins once every player
    /// has an opening hand (103.8).
    pub fn start(defs: impl Into<std::sync::Arc<Vec<CardDef>>>, decks: &[Vec<usize>], life: i32, first: PlayerId, seed: u64) -> Game {
        Self::start_with_companions(defs,decks,&vec![None;decks.len()],life,first,seed).expect("ordinary starting decks")
    }

    /// Designate at most one outside-game companion per seat before shuffling (103.2b).
    /// `decks` are the submitted starting decks after setting aside sideboards (702.139b).
    pub fn start_with_companions(defs: impl Into<std::sync::Arc<Vec<CardDef>>>, decks: &[Vec<usize>], companions: &[Option<usize>], life:i32, first:PlayerId, seed:u64) -> Result<Game,Refused> {
        Self::start_from(defs, decks, companions, life, first, Initial::Seed(seed))
    }

    /// As [`Self::start_with_companions`], the initial shuffles' outcomes coming from a seed
    /// or from a record of a game played elsewhere ([`Initial`]).
    pub fn start_from(defs: impl Into<std::sync::Arc<Vec<CardDef>>>, decks: &[Vec<usize>], companions: &[Option<usize>], life:i32, first:PlayerId, initial: Initial<'_>) -> Result<Game,Refused> {
        if decks.len()<2 || first>=decks.len() || companions.len()!=decks.len(){return Err(Refused::malformed("one optional companion per player and a valid starting player"));}
        let g = Game::new(defs, decks.len(), life);
        g.prepare_start(decks, companions, first, initial)
    }

    /// Format setup may have designated command-zone cards before this shared shuffle,
    /// opening-hand and mulligan procedure begins (103.2c, 903.6).
    pub(crate) fn prepare_start(mut self, decks: &[Vec<usize>], companions: &[Option<usize>], first: PlayerId, initial: Initial<'_>) -> Result<Game, Refused> {
        let g = &mut self;
        if decks.iter().flatten().any(|i|*i>=g.defs.len()){return Err(Refused::malformed("unknown starting-deck definition"));}
        if let Initial::Orders(orders) = initial {
            if orders.len() != decks.len() { return Err(Refused::malformed("orders gives one library order per player")); }
            for (player, (order, deck)) in orders.iter().zip(decks).enumerate() {
                let mut seen = vec![false; deck.len()];
                for &i in order {
                    if i >= deck.len() || std::mem::replace(&mut seen[i], true) {
                        return Err(Refused::malformed(format!("player {player}'s order is not a permutation of the positions of their deck")));
                    }
                }
                if order.len() != deck.len() {
                    return Err(Refused::malformed(format!("player {player}'s order is not a permutation of the positions of their deck")));
                }
            }
        }
        // With recorded orders no random number is drawn during the start; the seed of
        // whatever is random later is 0.
        g.random = Random::new(match initial { Initial::Seed(seed) => seed, Initial::Orders(_) => 0 });
        g.players[0].turn_began = 0;
        g.players[0].turns_taken = 0;
        g.players[first].turn_began = 1;
        g.players[first].turns_taken = 1;
        g.active = first;
        g.priority = first;
        let mut placed: Vec<Vec<ObjId>> = Vec::new();
        for (player, deck) in decks.iter().enumerate() {
            placed.push(deck.iter().map(|def| g.put(player, *def, Zone::Library)).collect());
        }
        // The outcome of each deck's shuffle (103.3, 701.24a), as a record says it was.
        if let Initial::Orders(orders) = initial {
            g.record_shuffles(orders.iter().enumerate().map(|(player, order)|
                ShuffleOrder { player, order: order.iter().map(|i| placed[player][*i]).collect() }).collect()).map_err(Refused::malformed)?;
        }
        for player in 0..decks.len() {
            if let Some(companion)=companions[player]{g.designate_companion(player,companion)?;}
            g.shuffle(player);
        }
        if let Initial::Orders(_) = initial {
            let unused = g.replay_random.take().expect("the recorded initial orders");
            assert!(unused.is_empty(), "every recorded initial order is consumed");
            g.random.recorded_only(false);
        }
        for player in g.turn_order() {
            g.draw(player, STARTING_HAND_SIZE);
        }
        let n = decks.len();
        g.opening = Some(Opening { taken: vec![0; n], kept: vec![false; n], declared: vec![None; n], owing: Vec::new() });
        g.ask_opening();
        Ok(self)
    }

    /// Has the first turn begun? Mulligans and opening-hand actions precede it (103.5–6).
    pub fn begun(&self) -> bool {
        self.opening.is_none() && !self.procedures.iter().any(|p| matches!(p, crate::procedure::Procedure::Opening(_)))
    }

    /// Shuffle a player's library: its cards are put in a random order (701.24a).
    pub fn shuffle(&mut self, player: PlayerId) {
        let revealed = self.resolving().and_then(|r| r.program_frame()).map(|f| f.known_cards()).unwrap_or_default();
        self.shuffle_except(player, &[], &revealed);
    }

    /// Found cards are set aside during a search's shuffle, despite still belonging to
    /// the library (701.24b). Their following destination instruction will place them.
    /// Cards an event puts at a specified position in the library it shuffles are left
    /// where they are the same way (701.24g).
    pub(crate) fn shuffle_except(&mut self, player: PlayerId, found: &[ObjId], revealed: &[ObjId]) {
        self.shuffle_library_except(player, player, found, revealed);
    }

    /// Randomize the owner's library, recording who performed the shuffle (720.3d).
    pub(crate) fn shuffle_library(&mut self, owner: PlayerId, by: PlayerId, revealed: &[ObjId]) {
        self.shuffle_library_except(owner, by, &[], revealed);
    }

    fn shuffle_library_except(&mut self, player: PlayerId, by: PlayerId, found: &[ObjId], revealed: &[ObjId]) {
        // Resolve the outcome before hiding revealed cards changes their incarnation
        // (701.20d). Physical identities survive that change and ordinary zone moves.
        // The outcome is this player's first one left: players who shuffle in one response
        // (two who take a mulligan in the same round, 103.5) do so in turn order, whatever
        // order their outcomes were given in.
        let recorded = self.replay_random.as_mut().map(|orders| {
            let at = orders.iter().position(|order| order.player == player
                && matches!(order.result, RecordedResult::Order {kind: RandomKind::Shuffle, ..}))
                .unwrap_or_else(|| reject_replay("tape requested an unexpected extra shuffle"));
            let RecordedResult::Order {cards, scope, ..} = orders.remove(at).expect("located outcome").result else { unreachable!("a shuffle") };
            (cards,scope)
        });
        // A recorded order may leave out the cards a search set aside (701.24b): they keep their
        // positions, and the order names the rest, as a record written after the search lists the
        // library without the cards it found.
        let recorded = recorded.map(|(cards,scope)| {
            if let Some(scope) = scope {
                let library = &self.players[player].library;
                let held: Vec<_> = library.iter().rev().filter(|id|found.contains(id))
                    .map(|id|self.obj(*id).card_id.expect("a held card has physical identity")).collect();
                let stated: Vec<_> = scope.excluding.iter().copied().chain(scope.fixed.iter().map(|(card,_)|*card)).collect();
                if held.len()!=stated.len() || !held.iter().all(|card|stated.contains(card)) {
                    reject_replay("tape shuffle scope differs from the cards kept out of the shuffle");
                }
                for (card,position) in &scope.fixed {
                    let index=match position {
                        ir::action::LibraryPosition::Top=>0,
                        ir::action::LibraryPosition::Bottom=>library.len().saturating_sub(1),
                        ir::action::LibraryPosition::FromTop(n)=>(*n as usize-1).min(library.len().saturating_sub(1)),
                    };
                    if library.iter().rev().nth(index).and_then(|id|self.obj(*id).card_id)!=Some(*card) {
                        reject_replay("tape shuffle fixed position differs from its actual placement");
                    }
                }
            }
            let library = &self.players[player].library;
            let set_aside: Vec<ObjId> = library.iter().rev().copied().filter(|id| found.contains(id)
                && self.obj(*id).card_id.is_some_and(|c| !cards.contains(&c))).collect();
            if set_aside.is_empty() || cards.len() + set_aside.len() != library.len() { return cards; }
            let mut merged = Vec::with_capacity(library.len());
            let mut rest = cards.into_iter();
            for id in library.iter().rev() {
                if set_aside.contains(id) { merged.push(self.obj(*id).card_id.expect("a set-aside card is a card")); }
                else if let Some(card) = rest.next() { merged.push(card); }
            }
            merged.extend(rest);
            merged
        });
        if let Some(cards) = &recorded {
            let library = &self.players[player].library;
            if cards.len() != library.len() { reject_replay("tape shuffle library membership differs"); }
            if !library.iter().all(|id| self.obj(*id).card_id.is_some_and(|c| cards.contains(&c))) {
                reject_replay("tape shuffle library membership differs");
            }
            // A search's set-aside cards are not shuffled (701.24b), nor cards put in a
            // specified position in the same event (701.24g).
            for (index, id) in library.iter().rev().enumerate() {
                if found.contains(id) {
                    if Some(cards[index]) != self.obj(*id).card_id { reject_replay("tape shuffle moved a set-aside card"); }
                }
            }
        }
        for card in revealed {
            if !found.contains(card) && self.players[player].library.contains(card) {
                let _ = self.hide_reordered(*card);
            }
        }
        if let Some(cards) = recorded {
            let order: Vec<_> = cards.iter().map(|card| *self.players[player].library.iter()
                .find(|id| self.obj(**id).card_id == Some(*card)).expect("validated physical shuffle card")).collect();
            self.set_library_order(player, &order).expect("validated shuffle permutation");
            self.capture_shuffle(player);
            self.log.push(crate::Event::Shuffled { player, by });
            return;
        }
        let mut library = std::mem::take(&mut self.players[player].library);
        let mut shuffled: Vec<ObjId> = library.iter().copied().filter(|o| !found.contains(o)).collect();
        self.random.shuffle(&mut shuffled);
        let mut shuffled = shuffled.into_iter();
        for card in &mut library {
            if !found.contains(card) { *card = shuffled.next().expect("one randomized card per position"); }
        }
        self.players[player].library = library;
        self.capture_shuffle(player);
        self.log.push(crate::Event::Shuffled { player, by });
    }

    /// Randomize exactly a placement group, without shuffling its destination library.
    /// Capture before placement changes incarnations or replacements remove cards.
    pub(crate) fn random_library_order(&mut self, player: PlayerId, cards: &mut [ObjId], position: ir::action::LibraryPosition) {
        // No choice among permutations exists for zero or one selected card. Unlike
        // an explicit shuffle (701.24e), this placement causes no shuffle event either.
        if cards.len() < 2 { return; }
        let kind = RandomKind::RandomOrder;
        let recorded = if self.replay_random.is_some() {
            let members: Vec<_> = cards.iter().map(|id| self.obj(*id).card_id).collect();
            let orders = self.replay_random.as_mut().expect("active recorded random scope");
            let at = orders.iter().position(|order| order.player == player
                && matches!(&order.result, RecordedResult::Order {kind: recorded, cards, ..} if *recorded == kind
                    && cards.len() == members.len() && members.iter().all(|card| card.is_some_and(|c| cards.contains(&c)))))
                .unwrap_or_else(|| reject_replay("tape requested an unrecorded random library order"));
            let RecordedResult::Order {cards, ..} = orders.remove(at).expect("located outcome").result else { unreachable!("a random order") };
            Some(cards)
        } else { None };
        if let Some(order) = recorded {
            if order.len() != cards.len() || !cards.iter().all(|id| self.obj(*id).card_id.is_some_and(|c| order.contains(&c))) {
                reject_replay("tape random-order membership differs");
            }
            let ordered: Vec<_> = order.iter().map(|card| *cards.iter()
                .find(|id| self.obj(**id).card_id == Some(*card)).expect("validated random-order card")).collect();
            cards.copy_from_slice(&ordered);
        } else {
            self.random.shuffle(cards);
        }
        if self.captured_random.is_some() {
            let order = cards.iter().map(|id| self.physical_card_identity(*id).expect("physical random-order card")).collect();
            self.captured_random.as_mut().unwrap().push(CapturedRandom::Order { kind, player, position: Some(position), order });
        }
    }

    fn capture_shuffle(&mut self, player: PlayerId) {
        if self.captured_random.is_some() {
            let order=self.players[player].library.iter().rev().map(|id|self.physical_card_identity(*id).expect("shuffle card identity")).collect();
            self.captured_random.as_mut().unwrap().push(CapturedRandom::Order {kind: RandomKind::Shuffle,player,position: None,order});
        }
    }

    /// One fair physical result, after the player has made any required call (705.1–2).
    /// The existing replay scope supplies outcomes by player in occurrence order.
    pub(crate) fn flip_coin(&mut self, player: PlayerId) -> CoinFace {
        let face = if let Some(outcomes) = self.replay_random.as_mut() {
            let at = outcomes.iter().position(|outcome| outcome.player == player && matches!(outcome.result, RecordedResult::Coin(_)))
                .unwrap_or_else(|| reject_replay("tape requested an unrecorded coin flip"));
            let RecordedResult::Coin(face) = outcomes.remove(at).expect("located coin outcome").result else { unreachable!("a coin") };
            face
        } else if self.random.below(2) == 0 { CoinFace::Heads } else { CoinFace::Tails };
        if let Some(captured) = &mut self.captured_random { captured.push(CapturedRandom::Coin {player, face}); }
        face
    }

    /// The players in turn order from the active player (101.4).
    fn turn_order(&self) -> Vec<PlayerId> {
        let n = self.players.len();
        (0..n).map(|i| (self.active + i) % n).collect()
    }

    /// How many cards a player who has taken this many mulligans puts on the bottom: as many
    /// (103.5); in a multiplayer game the first one doesn't count (103.5c).
    fn to_bottom(&self, taken: u32) -> u32 {
        if self.players.len() > 2 { taken.saturating_sub(1) } else { taken }
    }

    /// What the mulligans wait for next (103.5): a player who owes cards to the bottom of
    /// their library; or the next declaration, the starting player's first, then each other
    /// player's in turn order; or, all having declared, the mulligans themselves, taken at
    /// the same time. When no player takes one, the first turn begins (103.8).
    pub(crate) fn ask_opening(&mut self) {
        loop {
            let Some(opening) = &self.opening else { return };
            if let Some(player) = opening.owing.first().copied() {
                let count = self.to_bottom(opening.taken[player]) as usize;
                self.decision = Some(Decision::Bottom { player, count });
                return;
            }
            // a player whose opening hand would be zero cards may take no further mulligans
            let no_more: Vec<PlayerId> =
                (0..self.players.len()).filter(|p| !opening.kept[*p] && self.to_bottom(opening.taken[*p]) >= STARTING_HAND_SIZE).collect();
            let order = self.turn_order();
            let opening = self.opening.as_mut().expect("the mulligans");
            for player in no_more {
                opening.kept[player] = true;
            }
            if let Some(player) = order.iter().copied().find(|p| !opening.kept[*p] && opening.declared[*p].is_none()) {
                self.decision = Some(Decision::Mulligan { player });
                return;
            }
            let takers: Vec<PlayerId> = order.into_iter().filter(|p| opening.declared[*p] == Some(true)).collect();
            for (player, declared) in opening.declared.iter_mut().enumerate() {
                if *declared == Some(false) {
                    opening.kept[player] = true;
                }
                *declared = None;
            }
            if takers.is_empty() {
                self.opening = None;
                self.decision = None;
                let order = self.turn_order();
        self.procedures.push(crate::procedure::Procedure::Opening(Actions { order, next: 0, answer: None, waiting: false, revealed: Vec::new(), then: None, entered: None }));
                return;
            }
            for player in takers {
                for card in self.players[player].hand.clone() {
                    self.move_to(card, Zone::Library, player);
                }
                self.shuffle(player);
                self.draw(player, STARTING_HAND_SIZE);
                let opening = self.opening.as_mut().expect("the mulligans");
                opening.taken[player] += 1;
                let taken = opening.taken[player];
                if self.to_bottom(taken) > 0 {
                    self.opening.as_mut().expect("the mulligans").owing.push(player);
                }
            }
        }
    }

    /// The answer to [`Decision::Mulligan`]: the player's declaration (103.5).
    pub(crate) fn declare_mulligan(&mut self, player: PlayerId, mulligan: bool) -> crate::play::ActionResult<()> {
        if let Some(opening) = &mut self.opening {
            opening.declared[player] = Some(mulligan);
        }
        self.ask_opening();
        if self.decision.is_none() { self.go_on()?; }
        Ok(())
    }

    /// The answer to [`Decision::Bottom`]: exactly that many cards of the player's hand, put
    /// on the bottom of their library in the order given, the last of them the bottom card
    /// (103.5: "in any order").
    pub(crate) fn put_on_bottom(&mut self, player: PlayerId, count: usize, cards: &[ObjId]) -> crate::play::ActionResult<()> {
        let hand = &self.players[player].hand;
        let distinct = cards.iter().enumerate().all(|(i, c)| !cards[..i].contains(c));
        if cards.len() != count.min(hand.len()) || !distinct || cards.iter().any(|c| !hand.contains(c)) {
            return Err(format!("put exactly {count} card{} of your hand on the bottom of your library (103.5)", if count == 1 { "" } else { "s" }).into());
        }
        for card in cards {
            if let Some(moved) = self.move_to(*card, Zone::Library, player) {
                let library = &mut self.players[player].library;
                library.retain(|c| *c != moved);
                library.insert(0, moved);
            }
        }
        if let Some(opening) = &mut self.opening {
            opening.owing.retain(|p| *p != player);
        }
        self.ask_opening();
        if self.decision.is_none() { self.go_on()?; }
        Ok(())
    }
}

/// An opening-hand action a card offers (103.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Begin the game with it on the battlefield (103.6a), with these counters; only a player
    /// who is not the starting player, where it says so.
    Begin { not_starting: bool, counters: Option<(String, u32)> },
    /// Reveal it from the opening hand (103.6b).
    Reveal,
}

/// The opening-hand action this row of a card's text is, when it is one: the action, alone or
/// followed by "If you do, …" ([`follow_up`]).
pub(crate) fn kind(ability: &ir::Ability) -> Option<Kind> {
    use ir::statics::Static;
    let ir::Ability::Text(ir::Effect(rows)) = ability else { return None };
    let [ir::Sentence::Static(action), rest @ ..] = rows.as_slice() else { return None };
    if !rest.is_empty() && follow_up(ability).is_none() { return None; }
    match &**action {
        Static::BeginOnBattlefield { not_starting, counters, .. } => {
            let counters = match counters {
                None => None,
                Some(on) if !on.counters.up_to && on.counters.extra.is_none() => match on.counters.count {
                    ir::Amount::Const(n) => Some((on.counters.kind.0.clone(), n)),
                    _ => return None,
                },
                Some(_) => return None,
            };
            Some(Kind::Begin { not_starting: *not_starting, counters })
        }
        Static::RevealFromOpeningHand => Some(Kind::Reveal),
        _ => None,
    }
}

/// "If you do, exile a card from your hand.": what taking the action has its player do, the
/// sentence after "If you do" and those that follow it, which a program is compiled for.
pub(crate) fn follow_up(ability: &ir::Ability) -> Option<ir::Effect> {
    let ir::Ability::Text(ir::Effect(rows)) = ability else { return None };
    let [ir::Sentence::Static(_), ir::Sentence::Led { lead: ir::Lead::Modifier(ir::effect::Modifier::If(ir::Condition::Did { who, outcome: ir::condition::Outcome::Did })), rest }, more @ ..] = rows.as_slice() else { return None };
    if !crate::select::is_you(who) { return None; }
    Some(ir::Effect(std::iter::once((**rest).clone()).chain(more.iter().cloned()).collect()))
}

/// Definition-time recognition of the rows that are opening-hand actions (103.6). A whole
/// row must be one; an unsupported following instruction is never discarded.
pub(crate) fn compile(abilities: &[ir::Ability]) -> Vec<usize> {
    abilities.iter().enumerate().filter_map(|(i, ability)| kind(ability).map(|_| i)).collect()
}

/// Each player can use their opening-hand actions in any order, seeing the completed
/// preceding action before deciding again (103.6). Entry is an ordinary resumable child.
#[derive(Debug, Clone)]
pub(crate) struct Actions {
    order: Vec<PlayerId>,
    next: usize,
    answer: Option<Vec<ObjId>>,
    waiting: bool,
    /// The cards revealed from an opening hand: each only once (103.6b).
    revealed: Vec<ObjId>,
    /// A card put onto the battlefield whose action has its player do more: that row, by them.
    then: Option<(usize, PlayerId)>,
    /// What the entry just completed put onto the battlefield.
    entered: Option<ObjId>,
}
impl Actions {
    pub(crate) fn answer(&mut self, answer: crate::state::Answer) {
        let crate::state::Answer::Objects(cards) = answer else { unreachable!("validated opening selection") };
        assert!(!self.waiting && self.answer.is_none());
        self.answer = Some(cards);
    }
    /// The child the action started has completed: an entry, with what it put onto the
    /// battlefield, or what the action had its player do.
    pub(crate) fn completed(&mut self, entry: Option<&crate::operation::Completed>) {
        assert!(self.waiting);
        self.waiting = false;
        if let Some(result) = entry { self.entered = result.arrived.first().copied(); }
    }
}
impl Game {
    /// This card's opening-hand action, its row, when this player may take it now (103.6):
    /// "you're not the starting player"; a card is revealed this way only once (103.6b).
    fn opening_action(&self, player: PlayerId, card: ObjId, at: &Actions) -> Option<(usize, Kind)> {
        let def = self.def(card)?;
        let row = *def.opening.first()?;
        let kind = kind(&def.abilities[row])?;
        let starting = at.order.first() == Some(&player);
        let may = match &kind {
            Kind::Begin { not_starting, .. } => !(*not_starting && starting),
            Kind::Reveal => !at.revealed.contains(&card),
        };
        may.then_some((row, kind))
    }

    /// What taking the action at this row has its player do ("If you do, …"), of `source`: the
    /// permanent the card became, or the revealed card. Its compiled program, followed now as
    /// a rule has it done: no stack, no priority (103.6).
    fn opening_follow_up(&mut self, source: ObjId, player: PlayerId, row: usize) {
        let ctx = crate::select::Ctx::new(player, source).with_ability(row);
        let invocations = crate::program::capture(self, ctx).and_then(|captured| captured.select(&[]))
            .expect("an admitted opening-hand action's program");
        let mut at = crate::state::Resolving::new(source, crate::state::Bound::default());
        at.completion = crate::state::Completion::Opening;
        at.execution = Some(crate::program::Resolution::new(invocations, &at.bound));
        self.start_resolution(at);
    }

    pub(crate) fn advance_opening_actions(&mut self) -> crate::play::ActionResult<()> {
        use crate::procedure::Procedure;
        use crate::state::Choice;
        let Some(Procedure::Opening(mut at)) = self.procedures.pop() else { unreachable!() };
        assert!(!at.waiting, "entry child returns before its pregame parent advances");
        // the card has entered: what the action has its player do, of the permanent it became
        if let Some((row, player)) = at.then.take()
            && let Some(source) = at.entered.take().filter(|o| self.obj(*o).zone == Zone::Battlefield)
        {
            at.waiting = true;
            self.procedures.push(Procedure::Opening(at));
            self.opening_follow_up(source, player, row);
            return Ok(());
        }
        at.entered = None;
        if let Some(cards) = at.answer.take() {
            if let Some(card) = cards.first().copied() {
                let player = at.order[at.next];
                let (row, kind) = self.opening_action(player, card, &at).expect("an offered opening-hand action");
                let follows = self.def(card).and_then(|d| follow_up(&d.abilities[row])).is_some();
                match kind {
                    // 103.6a: the player puts it onto the battlefield, with what it says
                    Kind::Begin { counters, .. } => {
                        at.waiting = true;
                        if follows { at.then = Some((row, player)); }
                        self.procedures.push(Procedure::Opening(at));
                        let arrivals = counters.into_iter().map(|(kind, count)| crate::replace::Enters::Counters { kind, count: crate::program::expr::Amount::constant(count) }).collect();
                        let request = crate::entry::EntryRequest::card(card, player, crate::replace::Choices::arriving(arrivals), true);
                        match self.prepare_entry_batch(vec![request], Vec::new())? {
                            Ok(result) => self.deliver_operation(result),
                            Err(decision) => self.decision = Some(decision),
                        }
                        return Ok(());
                    }
                    // 103.6b: the player reveals it; it stays revealed until the first turn
                    Kind::Reveal => {
                        at.revealed.push(card);
                        self.log.push(crate::Event::Revealed { player, cards: vec![card] });
                        if follows {
                            at.waiting = true;
                            self.procedures.push(Procedure::Opening(at));
                            self.opening_follow_up(card, player, row);
                            return Ok(());
                        }
                    }
                }
            } else {
                at.next += 1;
            }
        }
        while let Some(player) = at.order.get(at.next).copied() {
            let among: Vec<_> = self.players[player].hand.iter().copied()
                .filter(|card| self.opening_action(player, *card, &at).is_some()).collect();
            if !among.is_empty() {
                self.decision = Some(Decision::Choose { player, choice: Choice::Select { among, min: 0, max: 1 } });
                self.procedures.push(Procedure::Opening(at));
                return Ok(());
            }
            at.next += 1;
        }
        // These permanents were controlled before any player's first turn.
        for object in self.battlefield.clone() { self.obj_mut(object).since = 0; }
        self.begin_checked()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 103.3, 701.24a: a start from recorded orders draws no random number (the generator is
    /// where a seed of 0 leaves it), consumes every order, and leaves later randomness to the
    /// generator as an unrecorded game has it.
    #[test]
    fn r103_3_recorded_orders_draw_no_random_number() {
        let land = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
        let orders = vec![(0..10).rev().collect::<Vec<_>>(), (0..10).collect()];
        let g = Game::start_from(vec![land], &[vec![0; 10], vec![0; 10]], &[None, None], 20, 0, Initial::Orders(&orders)).unwrap();
        assert_eq!(g.random, Random::new(0));
        assert!(!g.random.recorded_only);
        assert!(g.replay_random.is_none());
    }
}
