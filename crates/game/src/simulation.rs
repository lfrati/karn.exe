//! Correlated, failure-contained execution of explicitly supplied player actions.
//!
//! Requests describe the pending rules choice. The caller supplies each answer;
//! observing or refusing an action never selects an answer or passes priority.
//! Privileged fixture and replay paths retain their own input/failure boundaries.
//! Only unwinding panics are contained; a failed simulation cannot be resumed.

use crate::{Game, Move, PlayerId};
use crate::legal::Asked;
use crate::observation::Disclosure;
use crate::snapshot::Snapshot;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// What is asked of this seat: a pending rules choice or priority. Hosts must apply their observation privacy policy before
/// exporting a request to any other seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: RequestId,
    pub player: PlayerId,
    /// Player whose action/resources/choice this is (723.3, 723.5a).
    pub acting: PlayerId,
    pub question: Asked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage { Request, Response, Observation }

/// Failure context is collected only on failure. The host retains setup and accepted inputs
/// for replay; this report identifies the boundary at which execution stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub request_id: RequestId,
    pub stage: Stage,
    pub player: Option<PlayerId>,
    pub turn: u32,
    pub message: String,
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "simulation fault at {:?} request {} (turn {}, player {:?}): {}", self.stage, self.request_id.0, self.turn, self.player, self.message)
    }
}
impl std::error::Error for Fault {}

/// What kind of refusal a refused request is. A host says it with the refusal, so that a
/// caller can tell a move the rules forbid from one the engine cannot carry out, a request
/// it could not read, a name it may not use, and an answer to a question no longer asked.
/// A fault is no refusal: the engine failed, and the game cannot go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The rules forbid it; the refusal says the rules it cites ([`cited_rules`]).
    Illegal,
    /// The engine does not run something it needs; the game is as it was.
    Unsupported,
    /// The request does not parse, or breaks the protocol.
    Malformed,
    /// It names an object, player, choice or id the requester cannot refer to: unknown,
    /// hidden from that seat, or out of its scope.
    Unreferable,
    /// It answers a question no longer asked, or uses an outdated session or request id.
    Stale,
}

impl Class {
    /// The name a host says it by.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Illegal => "illegal",
            Self::Unsupported => "unsupported",
            Self::Malformed => "malformed",
            Self::Unreferable => "unreferable",
            Self::Stale => "stale",
        }
    }
}

/// The rules a refusal's text cites, in order, each once: a numbered rule as the
/// Comprehensive Rules number it (`601.2f`, `702.49a`). A refusal citing a range
/// (`601.2g-h`) cites its first rule; one citing a whole section by number alone cites none.
pub fn cited_rules(message: &str) -> Vec<String> {
    // as `scripts/rules.py` finds citations: \b[1-9]\d{2}\.\d+[a-z]{0,2}\b
    let bytes = message.as_bytes();
    let word = |at: usize| bytes.get(at).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    let digit = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_digit);
    let mut rules: Vec<String> = Vec::new();
    for at in 0..bytes.len() {
        if word(at.wrapping_sub(1)) || bytes[at] == b'0' || !(at..at + 3).all(digit)
            || bytes.get(at + 3) != Some(&b'.') || !digit(at + 4) { continue; }
        let mut end = at + 4;
        while digit(end) { end += 1; }
        end += (end..end + 2).take_while(|i| bytes.get(*i).is_some_and(u8::is_ascii_lowercase)).count();
        let rule = &message[at..end];
        if !word(end) && !rules.iter().any(|r| r == rule) { rules.push(rule.to_string()); }
    }
    rules
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseError {
    Stale { expected: RequestId, received: RequestId },
    WrongPlayer { expected: PlayerId, received: PlayerId },
    Finished,
    Illegal(String),
    /// The engine does not run something the answer needs: the game is as it was.
    Unsupported(String),
    /// Trusted fixture input that does not say what the tool takes.
    Malformed(String),
    /// An answer names an object or player that is not there, or (a player's,
    /// [`crate::prompt`]) an object they may not refer to now: refused before the game is
    /// asked, so that it tells them nothing.
    Unreferable(crate::Target),
    ReplayInput(crate::start::ReplayInput),
    Fault(Fault),
}
impl ResponseError {
    /// What kind of refusal this is; none for a fault, which is no refusal. An answer
    /// to a request issued to another player names a request that is not theirs.
    pub fn class(&self) -> Option<Class> {
        Some(match self {
            Self::Stale { .. } | Self::Finished => Class::Stale,
            Self::WrongPlayer { .. } | Self::Unreferable(_) => Class::Unreferable,
            Self::Illegal(_) => Class::Illegal,
            Self::Unsupported(_) => Class::Unsupported,
            Self::Malformed(_) | Self::ReplayInput(_) => Class::Malformed,
            Self::Fault(_) => return None,
        })
    }
}
impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale { expected, received } => write!(f, "stale request {}; expected {}", received.0, expected.0),
            Self::WrongPlayer { expected, received } => write!(f, "request belongs to player {expected}, not {received}"),
            Self::Finished => write!(f, "the game is over"),
            Self::Illegal(message) | Self::Unsupported(message) | Self::Malformed(message) => f.write_str(message),
            Self::Unreferable(crate::Target::Object(_)) => write!(f, "the answer names an object you cannot refer to now"),
            Self::Unreferable(crate::Target::Player(player)) => write!(f, "the game has no player {player}"),
            Self::ReplayInput(error) => error.fmt(f),
            Self::Fault(fault) => fault.fmt(f),
        }
    }
}
impl std::error::Error for ResponseError {}
impl From<crate::play::ActionError> for ResponseError {
    fn from(refused: crate::play::ActionError) -> Self {
        use crate::play::ActionError;
        match refused {
            ActionError::Refused(why) | ActionError::Restarted(why) => Self::Illegal(why),
            ActionError::Unsupported(why) => Self::Unsupported(why.0),
            ActionError::Unreferable(named) => Self::Unreferable(named),
        }
    }
}

/// Setup can reject an input normally or fail unexpectedly before a simulation exists.
/// A fault returns no partially initialized game and cannot be resumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError<E> {
    Rejected(E),
    Fault { message: String },
}

impl<E: std::fmt::Display> std::fmt::Display for SetupError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => error.fmt(f),
            Self::Fault { message } => write!(f, "simulation setup fault: {message}"),
        }
    }
}
impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for SetupError<E> {}

fn panic_message(payload: &dyn std::any::Any) -> String {
    payload.downcast_ref::<String>().cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "non-string panic payload".into())
}

#[derive(Debug, Clone)]
pub struct Simulation {
    game: Option<Game>,
    id: RequestId,
    request: Option<Request>,
    /// The game as each viewer may see it (None: all of it), taken since the game last
    /// could have changed. A copy of the simulation shares them until it goes on itself.
    snapshots: BTreeMap<Option<PlayerId>, Arc<Snapshot>>,
    /// The names each seat has been given for objects (`game::prompt`), by seat.
    pub(crate) labels: Vec<crate::prompt::Labels>,
    fault: Option<Fault>,
}

impl Simulation {
    /// Accept an already prepared game. Use [`Self::from_setup`] to contain rules execution
    /// during initialization too (for example `Game::start` or `Game::begin`).
    pub fn new(game: Game) -> Self {
        Self { game: Some(game), id: RequestId(1), request: None, snapshots: BTreeMap::new(), labels: Vec::new(), fault: None }
    }

    /// Construct a game inside the failure boundary. This closure creates a new owned
    /// game; it never receives access to an existing simulation's mutable state.
    /// Ordinary setup errors remain distinct from unwinding failures.
    pub fn from_setup<E>(setup: impl FnOnce() -> Result<Game, E>) -> Result<Self, SetupError<E>> {
        match catch_unwind(AssertUnwindSafe(setup)) {
            Ok(Ok(game)) => Ok(Self::new(game)),
            Ok(Err(error)) => Err(SetupError::Rejected(error)),
            Err(payload) => Err(SetupError::Fault { message: panic_message(payload.as_ref()) }),
        }
    }

    /// Trusted fixture/host inspection outside panic containment, not a seat-filtered
    /// observation. Use [`Self::observe`] for projections that evaluate rules queries.
    /// A failed game's state is discarded and cannot be cloned out to resume it.
    pub fn game(&self) -> Result<&Game, Fault> {
        match &self.fault {
            Some(fault) => Err(fault.clone()),
            None => Ok(self.game.as_ref().expect("live simulation owns its game")),
        }
    }

    pub fn fault(&self) -> Option<&Fault> { self.fault.as_ref() }

    /// Project a read-only observation within the same terminal failure boundary as rules
    /// advancement. The host applies seat visibility in its projection; no game copy is
    /// made and no mutable game access is exposed. Observation never consumes a request.
    pub fn observe<T>(&mut self, project: impl FnOnce(&Game) -> T) -> Result<T, Fault> {
        self.contain(Stage::Observation, None, |game| project(game))
    }

    /// The game as this viewer may see it (None: all of it, for a harness), within the
    /// failure boundary. It is taken once for each viewer while the game cannot have
    /// changed: asking again, asking for the request and taking what a seat
    /// was shown give the same one, and nothing of it is worked out again. An answer the
    /// game is given, taken or refused, ends that; so does anything else that lets the game
    /// go on. The pending question, events and disclosures have their own delivery.
    pub fn snapshot(&mut self, viewer: Option<PlayerId>) -> Result<Arc<Snapshot>, Fault> {
        if let Some(fault) = &self.fault { return Err(fault.clone()); }
        if let Some(held) = self.snapshots.get(&viewer) { return Ok(held.clone()); }
        // The request is asked for first: the game is handed to that question whole, and
        // what is seen is what is there once it has been asked.
        self.request()?;
        let taken = Arc::new(self.contain(Stage::Observation, viewer, |game| Snapshot::of(game, viewer))?);
        self.snapshots.insert(viewer, taken.clone());
        Ok(taken)
    }

    /// Take what this seat was shown (cards looked at or revealed) since it last took:
    /// delivery needs no request, and a look that no question follows is here all the
    /// same. What is taken is the host's to keep; another seat's is untouched, and so is
    /// the current request. A copy of the simulation has what was not yet taken.
    pub fn take_observations(&mut self, seat: PlayerId) -> Result<Vec<Arc<Disclosure>>, Fault> {
        self.contain(Stage::Observation, Some(seat), |game| game.take_observations(seat))
    }

    /// The current request. It is cheap for priority: no move is searched for. Repeated
    /// calls return the same request.
    pub fn request(&mut self) -> Result<Option<&Request>, Fault> {
        if self.request.is_none() {
            let id = self.id;
            // the game is handed over to be asked: what was seen of it before is not kept
            self.snapshots.clear();
            self.request = self.contain(Stage::Request, None, |g| {
                g.asked().map(|(acting, question)| Request { id, player: g.decision_maker(acting), acting, question })
            })?;
        }
        Ok(self.request.as_ref())
    }

    /// The current request, if this ID is the one it was issued under and this answer may be
    /// given by this player: one issued to them, or a concession, which any player may make
    /// whoever the request belongs to (104.3a).
    fn current_for(&mut self, id: RequestId, player: PlayerId, action: &Move) -> Result<&Request, ResponseError> {
        if *action != Move::Concede { return self.current(id, player); }
        if let Some(fault) = &self.fault { return Err(ResponseError::Fault(fault.clone())); }
        if id != self.id { return Err(ResponseError::Stale { expected: self.id, received: id }); }
        self.request().map_err(ResponseError::Fault)?.ok_or(ResponseError::Finished)
    }

    /// The current request, if this ID and seat are the ones it was issued to.
    fn current(&mut self, id: RequestId, player: PlayerId) -> Result<&Request, ResponseError> {
        if let Some(fault) = &self.fault { return Err(ResponseError::Fault(fault.clone())); }
        if id != self.id { return Err(ResponseError::Stale { expected: self.id, received: id }); }
        let request = self.request().map_err(ResponseError::Fault)?.ok_or(ResponseError::Finished)?;
        if player != request.player {
            return Err(ResponseError::WrongPlayer { expected: request.player, received: player });
        }
        Ok(request)
    }

    /// Authenticate a response envelope before a host decodes state-dependent
    /// choices such as restricted mana indices. Concessions bypass ownership.
    pub fn validate_request(&mut self, id: RequestId, player: PlayerId) -> Result<(), ResponseError> {
        self.current(id, player).map(|_| ())
    }

    /// Execute this action and stop at the next rules choice. Refusal never chooses
    /// another action on the caller's behalf.
    pub fn respond(&mut self, id: RequestId, player: PlayerId, action: Move) -> Result<(), ResponseError> {
        self.current_for(id, player, &action)?;
        self.apply(player, action, true)
    }

    /// Privileged modal-capture probe only: execute the genuine modes response
    /// while retaining departed payloads until its immediately following targets
    /// response. A combined announcement has no collection boundary between these
    /// choices, so preserving its allocation schedule permits exact comparison.
    /// The caller must use an isolated clone and complete/refuse that probe; live
    /// player responses must use `respond`, which always collects normally.
    #[doc(hidden)]
    pub fn probe_modal_modes(&mut self, id: RequestId, player: PlayerId, modes: Vec<usize>) -> Result<(), ResponseError> {
        let request = self.current(id, player)?;
        if !matches!(request.question, Asked::Modes { .. }) {
            return Err(ResponseError::Malformed("modal probe needs a modes request".into()));
        }
        // Stop at the native targets boundary of the same announcement.
        self.apply_with_collection(player, Move::ChooseModes(modes), true, false)
    }

    /// Trusted replay of full-library shuffle receipts. Replay-input failures restore
    /// the entire pending request; unrelated engine faults remain terminal.
    pub fn respond_with_shuffles(&mut self, id: RequestId, player: PlayerId, action: Move,
                                 orders: Vec<crate::start::ShuffleOrder>) -> Result<(), ResponseError> {
        self.respond_with_random(id, player, action, orders.into_iter().map(Into::into).collect(), false).map(|_| ())
    }

    /// Keep receipts not consumed by this response, rebound to current objects.
    pub fn respond_with_shuffles_keeping_unused(&mut self, id: RequestId, player: PlayerId, action: Move,
                                                orders: Vec<crate::start::ShuffleOrder>)
        -> Result<Vec<crate::start::ShuffleOrder>, ResponseError> {
        self.respond_with_random(id, player, action, orders.into_iter().map(Into::into).collect(), true)
            .map(|orders| orders.into_iter().map(|unused| crate::start::ShuffleOrder { player: unused.outcome.player.expect("shuffle owner"), order: unused.outcome.order }).collect())
    }

    /// Explicit random outcomes, scoped to one response and its rules continuations.
    /// Shuffles are keyed by owner in occurrence order. Selected-card orders match
    /// their exact physical set; actual execution supplies owner/destination provenance.
    /// Only this privileged path clones a simulation to make bad replay input atomic; an
    /// answer the game refuses is refused as by [`Self::respond`].
    pub fn respond_with_random(&mut self, id: RequestId, player: PlayerId, action: Move,
                               orders: Vec<crate::start::RandomOutcome>, keep_unused: bool)
        -> Result<Vec<crate::start::UnusedRandom>, ResponseError> {
        self.current_for(id, player, &action)?;
        let before = self.clone();
        let attempted = catch_unwind(AssertUnwindSafe(|| {
            self.contain(Stage::Response, Some(player), |g| g.record_random(orders))
                .map_err(ResponseError::Fault)?.map_err(|why| ResponseError::ReplayInput(crate::start::ReplayInput(why)))?;
            self.respond(id, player, action)?;
            self.contain(Stage::Response, Some(player), |g| {
                let unused = g.replay_random.take().expect("scoped tape random outcomes");
                if !keep_unused && !unused.is_empty() {
                    crate::start::reject_replay("tape has unused shuffle outcomes or random-order outcomes");
                }
                g.random.recorded_only(false);
                if keep_unused { g.unconsumed_random(unused) } else { Vec::new() }
            }).map_err(ResponseError::Fault)
        }));
        match attempted {
            Ok(Ok(unused)) => Ok(unused),
            Ok(Err(error @ ResponseError::Fault(_))) => Err(error),
            // The game refused the answer as the rules do: unchanged, or an illegal total
            // combat damage assignment returned to before its player began (510.1e), a new
            // request. Only the replay scope ends; restoring the game would undo the restart.
            Ok(Err(error @ ResponseError::Illegal(_))) => {
                self.contain(Stage::Response, Some(player), |g| {
                    g.replay_random = None;
                    g.random.recorded_only(false);
                }).map_err(ResponseError::Fault)?;
                Err(error)
            }
            Ok(Err(error)) => { *self = before; Err(error) }
            Err(payload) => match payload.downcast::<crate::start::ReplayInput>() {
                Ok(error) => { *self = before; Err(ResponseError::ReplayInput(*error)) }
                Err(payload) => std::panic::resume_unwind(payload),
            },
        }
    }

    /// Privileged recording: capture each random permutation before later effects,
    /// including selected-card placement before zone changes or replacement processing.
    pub fn respond_capturing_random(&mut self, id: RequestId, player: PlayerId, action: Move)
        -> Result<Vec<crate::start::CapturedRandom>, ResponseError> {
        self.current_for(id,player,&action)?;
        self.contain(Stage::Response,Some(player),|game| { game.captured_random=Some(Vec::new()); }).map_err(ResponseError::Fault)?;
        let result = self.respond(id,player,action);
        if self.fault.is_some() { return result.map(|_| Vec::new()); }
        let captured = self.contain(Stage::Response,Some(player),|game|game.captured_random.take().expect("scoped capture")).map_err(ResponseError::Fault)?;
        result?;
        Ok(captured)
    }

    /// Trusted fixture edit at a priority boundary. A changed library invalidates the
    /// request and viewer snapshots; it does not perform a rules shuffle.
    pub fn set_library_order(&mut self, id: RequestId, player: PlayerId, order: &[crate::ObjId]) -> Result<(), ResponseError> {
        let request = self.request().map_err(ResponseError::Fault)?.ok_or(ResponseError::Finished)?.clone();
        self.current(id, request.player)?;
        if request.question != Asked::Priority {
            return Err(ResponseError::Malformed("set_library_order requires a priority boundary".into()));
        }
        self.contain(Stage::Response, None, |g| {
            #[cfg(debug_assertions)]
            let before = g.clone();
            let edited = g.set_library_order(player, order);
            #[cfg(debug_assertions)]
            if let Err(why) = &edited { crate::state::assert_unchanged(&before, g, why); }
            edited
        }).map_err(ResponseError::Fault)?.map_err(ResponseError::Malformed)?;
        self.snapshots.clear();

        self.request = None;
        self.id = RequestId(self.id.0.checked_add(1).expect("request ID exhausted"));
        Ok(())
    }

    /// Give the game this answer to the current request and, if it takes it, move on to
    /// the next request ID. `retire`: the host has what was logged before this answer.
    fn apply(&mut self, player: PlayerId, action: Move, retire: bool) -> Result<(), ResponseError> {
        self.apply_with_collection(player, action, retire, true)
    }

    fn apply_with_collection(&mut self, player: PlayerId, action: Move, retire: bool, collect: bool) -> Result<(), ResponseError> {
        let id = self.id;
        // Before the game is given the answer: one it refuses may have changed it too.
        self.snapshots.clear();
        let next = self.contain(Stage::Response, Some(player), |g| {
            // Hosts consume/snapshot the preceding response before advancing the game.
            // Keep newly produced events and any old ranges still owned by a procedure.
            let preceding = g.log.end();
            g.attempt(player, action)?;
            if retire { g.retire_events(preceding); }
            if collect { g.collect_departed(); }
            // Never wrap a live ID, even for identical subsequent answer domains.
            Ok::<_, crate::play::ActionError>(id.0.checked_add(1).expect("request ID exhausted"))
        }).map_err(ResponseError::Fault)?;
        if let Err(why) = next {
            // A refused answer can still move the game to another question: an illegal total
            // combat damage assignment returns to before its player began to assign (510.1e).
            // That question is a new request; an unchanged one stays the request it was.
            let asked = self.contain(Stage::Request, None, |g| g.asked()).map_err(ResponseError::Fault)?;
            if self.request.as_ref().is_some_and(|r| asked.as_ref() != Some(&(r.acting, r.question.clone()))) {
                self.request = None;

                self.id = RequestId(id.0.checked_add(1).expect("request ID exhausted"));
            }
            return Err(why.into());
        }
        let next = next.expect("accepted");
        self.request = None;

        self.id = RequestId(next);
        Ok(())
    }

    // Private seam also permits controlled tests of containment after partial mutation.
    fn contain<T>(&mut self, stage: Stage, player: Option<PlayerId>, run: impl FnOnce(&mut Game) -> T) -> Result<T, Fault> {
        if let Some(fault) = &self.fault { return Err(fault.clone()); }
        let game = self.game.as_mut().expect("live simulation owns its game");
        match catch_unwind(AssertUnwindSafe(|| run(game))) {
            Ok(value) => Ok(value),
            Err(payload) => {
                // An expected input refusal belongs to the outer replay transaction.
                // It aborts immediately, before later instructions can observe bad state.
                if game.random.requires_recorded() && payload.is::<crate::start::ReplayInput>() {
                    std::panic::resume_unwind(payload);
                }
                let message = panic_message(payload.as_ref());
                let fault = Fault { request_id: self.id, stage, player, turn: game.turn, message };
                self.fault = Some(fault.clone());
                self.request = None;

                self.snapshots.clear();
                self.game = None;
                Err(fault)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef, Zone};
    use crate::observation::Kind;

    /// Each player a Bear on the battlefield and one in hand, and one in each library.
    fn seated() -> Simulation {
        let mut game = Game::new(vec![CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap()], 2, 20);
        for player in 0..2 {
            for zone in [Zone::Battlefield, Zone::Hand, Zone::Library] { game.put(player, 0, zone); }
        }
        Simulation::new(game)
    }

    #[test]
    fn a_refusal_cites_the_rules_its_reason_names_as_the_rules_tool_reads_them() {
        assert_eq!(cited_rules("you have played a land this turn (305.2)"), ["305.2"]);
        // several, each once, subrules and ranges by their first rule
        assert_eq!(cited_rules("not settled (117.5/704.3), see 117.5 and 702.139a–702.139c"),
            ["117.5", "704.3", "702.139a", "702.139c"]);
        // not a rule: inside a longer number or word, a zero hundred, more than two letters
        assert!(cited_rules("1601.2 a601.2 060.1 601.2abc 601. 4.5").is_empty());
        assert!(cited_rules("an object handle is stale or unknown").is_empty());
    }

    #[test]
    fn each_refusal_keeps_the_kind_the_game_gave_it() {
        // a move the engine does not run is unsupported, not illegal, and changes nothing
        let mut simulation = Simulation::new(Game::new(vec![], 3, 20));
        let request = simulation.request().unwrap().unwrap().clone();
        let before = format!("{:?}", simulation.game().unwrap());
        let refused = simulation.respond(request.id, request.player, Move::Concede).unwrap_err();
        assert!(matches!(refused, ResponseError::Unsupported(_)), "{refused:?}");
        assert_eq!(refused.class(), Some(Class::Unsupported));
        assert_eq!(format!("{:?}", simulation.game().unwrap()), before);
        // the same request is still the one asked
        assert_eq!(simulation.request().unwrap(), Some(&request));
        // an answer naming an object the game does not have, another request or another player
        let refused = simulation.respond(request.id, request.player, Move::PlayLand(crate::ObjId(999_999))).unwrap_err();
        assert_eq!((refused.class(), refused), (Some(Class::Unreferable), ResponseError::Unreferable(crate::Target::Object(crate::ObjId(999_999)))));
        let stale = simulation.respond(RequestId(request.id.0 + 1), request.player, Move::Pass).unwrap_err();
        assert_eq!(stale.class(), Some(Class::Stale));
        let wrong = simulation.respond(request.id, (request.player + 1) % 3, Move::Pass).unwrap_err();
        assert_eq!(wrong.class(), Some(Class::Unreferable));
        simulation.respond(request.id, request.player, Move::Pass).unwrap();
    }

    #[test]
    fn a_snapshot_is_taken_once_while_the_game_cannot_have_changed() {
        let mut sim = seated();
        let first = sim.snapshot(Some(0)).unwrap();
        let (id, player) = { let r = sim.request().unwrap().unwrap(); (r.id, r.player) };
        assert!(Arc::ptr_eq(&first, &sim.snapshot(Some(0)).unwrap()), "after the request");
        // refused before the game is given anything: nothing can have changed
        assert!(matches!(sim.respond(RequestId(id.0 + 1), player, Move::Pass), Err(ResponseError::Stale { .. })));
        assert!(matches!(sim.respond(id, 1 - player, Move::Pass), Err(ResponseError::WrongPlayer { .. })));
        assert!(Arc::ptr_eq(&first, &sim.snapshot(Some(0)).unwrap()), "after answers that were not for it");
        assert_eq!(*first, Snapshot::of(sim.game().unwrap(), Some(0)));
    }

    #[test]
    fn an_answer_given_to_the_game_ends_every_snapshot_taken_or_refused() {
        let mut sim = seated();
        let (id, player) = { let r = sim.request().unwrap().unwrap(); (r.id, r.player) };
        let before: Vec<_> = [None, Some(0), Some(1)].into_iter().map(|v| sim.snapshot(v).unwrap()).collect();
        // the game refuses it, having been handed it
        let refused = sim.respond(id, player, Move::Keep);
        assert!(matches!(refused, Err(ResponseError::Illegal(_))), "{refused:?}");
        assert_eq!(sim.request().unwrap().unwrap().id, id, "the request is still the one asked");
        let after: Vec<_> = [None, Some(0), Some(1)].into_iter().map(|v| sim.snapshot(v).unwrap()).collect();
        for (a, b) in before.iter().zip(&after) {
            assert!(!Arc::ptr_eq(a, b), "taken again after a refused answer");
            assert_eq!(a, b, "which changed nothing here");
        }
        // the game takes it
        sim.respond(id, player, Move::Pass).unwrap();
        let passed = sim.snapshot(None).unwrap();
        assert!(!Arc::ptr_eq(&after[0], &passed));
        assert_eq!(*passed, Snapshot::of(sim.game().unwrap(), None));
        assert_ne!(*passed, *after[0], "priority was passed");
    }

    #[test]
    fn each_viewer_has_its_own_snapshot_and_a_copy_shares_them_until_it_goes_on() {
        let mut sim = seated();
        let (all, mine, theirs) = (sim.snapshot(None).unwrap(), sim.snapshot(Some(0)).unwrap(), sim.snapshot(Some(1)).unwrap());
        assert!(Arc::ptr_eq(&mine, &sim.snapshot(Some(0)).unwrap()) && Arc::ptr_eq(&all, &sim.snapshot(None).unwrap()));
        // 402.3: a hand is its player's to look at
        assert!(mine.players[0].hand.is_some() && mine.players[1].hand.is_none());
        assert!(theirs.players[1].hand.is_some() && theirs.players[0].hand.is_none());
        assert!(all.players.iter().all(|p| p.hand.is_some()));
        assert_eq!((mine.objects.len(), theirs.objects.len(), all.objects.len()), (3, 3, 4));
        let mut copy = sim.clone();
        assert!(Arc::ptr_eq(&mine, &copy.snapshot(Some(0)).unwrap()), "an unchanged copy is given the same one");
        let (id, player) = { let r = copy.request().unwrap().unwrap(); (r.id, r.player) };
        copy.respond(id, player, Move::Pass).unwrap();
        assert!(!Arc::ptr_eq(&mine, &copy.snapshot(Some(0)).unwrap()));
        assert!(Arc::ptr_eq(&mine, &sim.snapshot(Some(0)).unwrap()), "the original has not gone on");
        assert_eq!(*mine, Snapshot::of(sim.game().unwrap(), Some(0)));
    }

    #[test]
    fn what_a_seat_was_shown_is_given_once_and_is_no_part_of_a_snapshot() {
        let mut game = seated().game.take().unwrap();
        let top = game.players[1].library[0];
        game.disclose(Kind::Looked, 0, &[top]);
        game.disclose(Kind::Revealed, 1, &[top]);
        let mut sim = Simulation::new(game);
        let seen = sim.snapshot(Some(0)).unwrap();
        let shown = sim.take_observations(0).unwrap();
        assert_eq!(shown.iter().map(|d| d.kind).collect::<Vec<_>>(), [Kind::Looked, Kind::Revealed]);
        assert!(sim.take_observations(0).unwrap().is_empty(), "taken once");
        assert!(Arc::ptr_eq(&seen, &sim.snapshot(Some(0)).unwrap()), "taking them changed nothing seen");
        // the other seat's wait for it, whatever was taken or seen meanwhile
        assert_eq!(sim.clone().take_observations(1).unwrap().len(), 1);
        assert_eq!(sim.take_observations(1).unwrap().len(), 1);
        // having been shown a card is no permission to look at it where it is (401.2)
        assert!(seen.objects.iter().all(|o| o.id != top));
    }

    #[test]
    fn replay_scope_does_not_turn_genuine_engine_panics_into_input_refusals() {
        let mut sim = seated();
        sim.game.as_mut().unwrap().record_random(vec![]).unwrap();
        let fault = sim.contain(Stage::Response, Some(0), |game| {
            game.players[0].life = 1;
            panic!("genuine controlled engine defect");
        }).map(|()| ()).unwrap_err();
        assert_eq!(fault, Fault { request_id: RequestId(1), stage: Stage::Response,
            player: Some(0), turn: 1, message: "genuine controlled engine defect".into() });
        assert_eq!(sim.fault(), Some(&fault));
        assert_eq!(sim.game().unwrap_err(), fault);
        assert_eq!(sim.request().unwrap_err(), fault);
        assert!(sim.game.is_none(), "partial mutation was discarded");
        assert!(sim.request.is_none());
        assert!(sim.snapshots.is_empty());
    }

    #[test]
    fn unsupported_raw_randomness_aborts_with_a_typed_replay_input() {
        let mut sim = seated();
        sim.game.as_mut().unwrap().record_random(vec![]).unwrap();
        let before = sim.game().unwrap().random.clone();
        let outcome = catch_unwind(AssertUnwindSafe(|| sim.contain(Stage::Response, Some(0), |game| game.random.below(2))));
        let payload = outcome.unwrap_err();
        let error = payload.downcast::<crate::start::ReplayInput>().unwrap();
        assert!(error.0.contains("unrecorded randomness"));
        assert!(sim.fault().is_none());
        assert_eq!(sim.game().unwrap().random, before, "no seeded draw occurred");
    }

    #[test]
    fn a_failed_game_gives_no_snapshot() {
        let mut sim = seated();
        sim.snapshot(None).unwrap();
        let fault = sim.contain(Stage::Response, Some(0), |_| panic!("controlled boundary failure")).map(|()| ()).unwrap_err();
        assert_eq!(sim.snapshot(None).unwrap_err(), fault);
        assert!(sim.snapshots.is_empty());
    }

    #[test]
    fn an_unwind_discards_only_the_failed_simulation_and_cannot_resume() {
        for stage in [Stage::Request, Stage::Response] {
            let mut failed = Simulation::new(Game::new(vec![], 2, 20));
            let mut healthy = failed.clone();
            let id = failed.request().unwrap().unwrap().id;
            let fault = failed.contain(stage, Some(0), |g| {
                g.players[0].life = 1;
                panic!("controlled boundary failure");
            }).unwrap_err();
            assert_eq!(fault.request_id, id);
            assert_eq!(fault.stage, stage);
            assert_eq!(failed.request().unwrap_err(), fault);
            assert_eq!(failed.game().unwrap_err(), fault);
            assert_eq!(failed.respond(id, 0, Move::Pass), Err(ResponseError::Fault(fault.clone())));
            assert_eq!(failed.clone().request().unwrap_err(), fault);
            healthy.respond(id, 0, Move::Pass).unwrap();
            assert_eq!(healthy.game().unwrap().players[0].life, 20);
        }
    }
}
