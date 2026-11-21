//! The actions a player does to things of their own choosing: discarding cards (701.9) and
//! sacrificing permanents (701.21).
//!
//! An [`Action`] says who acts, what they do it to and how many. A [`Pending`] is one doing of
//! it: who acts and what each could do it to are read once, as it begins; each player who has
//! more than the action asks for then chooses, in turn order (101.4), and only when every
//! choice is made is anything done, all of it at once (608.2e). What has been chosen so far
//! is kept here, not asked again.
use super::action::{self, Operand};
use super::{Frame, expr};
use crate::binding::Scalar;
use crate::event::Target;
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, PublicChoice};

/// What is done to what is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Discard,
    Sacrifice,
    /// "You may exile it. If you do, …": of objects the text names, none chosen.
    Exile,
    /// Exile a chosen graveyard group and retain one semantic collection (701.59).
    CollectEvidence,
    /// "Choose an Army creature you control" (608.2d, 701.47a): nothing is done to it; what
    /// was chosen is what the text goes on to name.
    Choose,
    /// "… unless you return an untapped Island you control to its owner's hand": the
    /// permanent goes to its owner's hand (400.3).
    Return,
    /// "… unless you tap an untapped creature you control" (701.26a).
    Tap,
    /// "You may behold a Dragon": a card of the hand revealed or a permanent chosen, nothing
    /// else done to it (701.4a).
    Behold,
}

impl Verb {
    pub(super) fn said(self) -> ir::Verb {
        match self {
            Verb::Discard => ir::Verb::Discard,
            Verb::Sacrifice => ir::Verb::Sacrifice,
            Verb::Exile => ir::Verb::Exile,
            Verb::CollectEvidence => ir::Verb::CollectEvidence,
            Verb::Choose => ir::Verb::Choose,
            Verb::Return => ir::Verb::Move,
            Verb::Tap => ir::Verb::Tap,
            Verb::Behold => ir::Verb::Behold,
        }
    }
    /// Where what is chosen goes, when the action is "put" there (400.3).
    pub(super) fn to(self) -> Option<crate::state::Placement> {
        (self == Verb::Return).then_some(crate::state::Placement::Zone(crate::Zone::Hand))
    }
}

/// What the action is done to, of what the player who acts has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Of {
    /// Coordinated descriptions are choices for one simultaneous action, not separate
    /// sacrifices with mutations between them (608.2e).
    Together(Vec<Action>),
    /// One of several actions; selected before choosing its objects (608.2d).
    Alternatives(Vec<Action>),
    /// Cards of that player's hand (701.9a).
    Cards(expr::Cards),
    /// Permanents that player controls (701.21a).
    Permanents(expr::Set),
    /// Cards of that player's hand or permanents they control, either: what is beheld (701.4a).
    Either(expr::Cards, expr::Set),
    /// Objects in a compiled, explicitly named zone.
    Objects(expr::Set),
    /// The source itself ("sacrifice this creature"): nothing is chosen.
    Source,
    /// The objects the text names ("exile it"): all of them, nothing is chosen. One that is
    /// no longer where it was named cannot be acted on, and the action is then impossible.
    Named(Vec<Operand>),
}

/// How many: a number, of the player's choice among what there is, or all there is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Count {
    Exactly(expr::Amount),
    All,
    ManaValueAtLeast(expr::Amount),
}

/// Who chooses: the player who acts (701.9b), or the controller of the effect ("of your
/// choice").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chooser {
    Actor,
    Controller,
}

/// "Each player discards two cards", "target player sacrifices a creature of their choice".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    verb: Verb,
    players: Operand,
    of: Of,
    count: Count,
    chooser: Chooser,
}

impl Action {
    pub(super) fn together(actions: Vec<Action>) -> crate::Res<Action> {
        let Some(first) = actions.first() else { return crate::unsupported("empty coordinated choice"); };
        if actions.iter().any(|a| (a.verb != first.verb && !matches!((a.verb,first.verb), (Verb::CollectEvidence,Verb::Exile))) || a.players != first.players) { return crate::unsupported("coordinated choices must share their action and actors"); }
        if actions.len() == 1 { return Ok(actions.into_iter().next().unwrap()); }
        Ok(Action { verb: first.verb, players: first.players.clone(), of: Of::Together(actions), count: Count::All, chooser: Chooser::Actor })
    }
    pub(super) fn alternatives(actions: Vec<Action>) -> crate::Res<Action> {
        let Some(first) = actions.first() else { return crate::unsupported("no alternative actions"); };
        if actions.iter().any(|a| a.players != first.players) { return crate::unsupported("alternative actions by different players"); }
        Ok(Action { verb: first.verb, players: first.players.clone(), of: Of::Alternatives(actions), count: Count::All, chooser: Chooser::Actor })
    }
    pub(super) fn payment_ok(&self) -> crate::Res<()> {
        if let Of::Alternatives(actions) = &self.of { return actions.iter().try_for_each(Action::payment_ok); }
        if let Of::Together(actions) = &self.of {
            // Only deterministic named exile prefixes can be projected before the final
            // evidence choice. Other compound payments still need their own feasibility model.
            if actions.last().is_some_and(|a| a.verb == Verb::CollectEvidence)
                && actions[..actions.len()-1].iter().all(|a| a.verb == Verb::Exile && matches!(a.of, Of::Named(_))) { return Ok(()); }
            return crate::unsupported("coordinated payment feasibility is not implemented");
        }
        Ok(())
    }

    pub fn discard(players: Operand, cards: expr::Cards, count: Count) -> Action {
        Action { verb: Verb::Discard, players, of: Of::Cards(cards), count, chooser: Chooser::Actor }
    }

    pub fn discard_named(players: Operand, objects: Vec<Operand>) -> Action {
        Action { verb: Verb::Discard, players, of: Of::Named(objects), count: Count::All, chooser: Chooser::Actor }
    }

    pub fn sacrifice_named(players: Operand, objects: Vec<Operand>) -> Action {
        Action { verb: Verb::Sacrifice, players, of: Of::Named(objects), count: Count::All, chooser: Chooser::Actor }
    }

    pub fn sacrifice(players: Operand, permanents: expr::Set, count: Count) -> Action {
        Action { verb: Verb::Sacrifice, players, of: Of::Permanents(permanents), count, chooser: Chooser::Actor }
    }

    /// "Choose an Army creature you control": of the permanents each player controls.
    pub fn choose(players: Operand, permanents: expr::Set, count: Count) -> Action {
        Action { verb: Verb::Choose, players, of: Of::Permanents(permanents), count, chooser: Chooser::Actor }
    }

    /// "Sacrifice this creature."
    pub fn sacrifice_source(players: Operand) -> Action {
        Action { verb: Verb::Sacrifice, players, of: Of::Source, count: Count::Exactly(expr::Amount::constant(1)), chooser: Chooser::Actor }
    }

    /// "Exile it": the player exiles every object named.
    pub fn exile_selected(players: Operand, objects: expr::Set, count: Count) -> Action {
        Action { verb: Verb::Exile, players, of: Of::Objects(objects), count, chooser: Chooser::Actor }
    }

    pub fn collect_evidence(players: Operand, objects: expr::Set, minimum: expr::Amount) -> Action {
        Action { verb: Verb::CollectEvidence, players, of: Of::Objects(objects), count: Count::ManaValueAtLeast(minimum), chooser: Chooser::Actor }
    }

    pub fn exile(players: Operand, objects: Vec<Operand>) -> Action {
        Action { verb: Verb::Exile, players, of: Of::Named(objects), count: Count::All, chooser: Chooser::Actor }
    }

    /// "Return a land you control to its owner's hand", of the permanents the set describes.
    pub fn return_selected(players: Operand, permanents: expr::Set, count: Count) -> Action {
        Action { verb: Verb::Return, players, of: Of::Objects(permanents), count, chooser: Chooser::Actor }
    }

    /// "Tap an untapped creature you control", of the permanents the set describes.
    pub fn tap_selected(players: Operand, permanents: expr::Set, count: Count) -> Action {
        Action { verb: Verb::Tap, players, of: Of::Objects(permanents), count, chooser: Chooser::Actor }
    }

    /// "Behold a Dragon": one Dragon card of the player's hand or Dragon permanent they
    /// control (701.4a).
    pub fn behold(players: Operand, cards: expr::Cards, permanents: expr::Set) -> Action {
        Action { verb: Verb::Behold, players, of: Of::Either(cards, permanents), count: Count::Exactly(expr::Amount::constant(1)), chooser: Chooser::Actor }
    }

    pub fn chosen_by(self, chooser: Chooser) -> Action {
        Action { chooser, ..self }
    }

    pub fn verb(&self) -> Verb {
        self.verb
    }

    /// Every operand the action reads.
    pub(super) fn operands(&self) -> Vec<&Operand> {
        if let Of::Together(actions) | Of::Alternatives(actions) = &self.of { return actions.iter().flat_map(Action::operands).collect(); }
        let named = match &self.of {
            Of::Named(objects) => objects.as_slice(),
            _ => &[],
        };
        std::iter::once(&self.players).chain(named).collect()
    }

    /// The chosen values the action reads, for checking they are produced before it.
    pub(super) fn scalars(&self) -> Vec<Scalar> {
        let of = match &self.of {
            Of::Together(actions) | Of::Alternatives(actions) => actions.iter().flat_map(Action::scalars).collect(),
            Of::Cards(cards) => cards.scalars(),
            Of::Permanents(set) | Of::Objects(set) => set.scalars(),
            Of::Either(cards, set) => cards.scalars().into_iter().chain(set.scalars()).collect(),
            Of::Source => Vec::new(),
            Of::Named(objects) => objects.iter().flat_map(Operand::scalars).collect(),
        };
        let count = match &self.count {
            Count::Exactly(n) | Count::ManaValueAtLeast(n) => n.scalars(),
            Count::All => Vec::new(),
        };
        self.players.scalars().into_iter().chain(of).chain(count).collect()
    }
}

/// One player's part: what they could do it to, how many of it, and what was settled on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Part {
    player: PlayerId,
    verb: Verb,
    selection: Option<crate::cost::Selection>,
    chooser: PlayerId,
    among: Vec<ObjId>,
    /// None: all of it.
    count: Option<usize>,
    chosen: Option<Vec<ObjId>>,
    consent: Option<bool>,
}

impl Part {
    /// Is there something to choose: more than the action asks for?
    fn open(&self) -> bool {
        self.chosen.is_none() && (self.selection.is_some() && !self.among.is_empty() || self.count.is_some_and(|n| n > 0 && self.among.len() > n))
    }

    fn short(&self) -> bool {
        self.selection.as_ref().map_or_else(|| self.count.is_some_and(|n| self.among.len() < n), |s| !s.accepts(&self.among))
    }
}

/// One doing of an [`Action`]: the players in turn order, what each has chosen, and whether
/// it has been done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    alternatives: Option<Vec<Pending>>,
    compound: bool,
    verb: Verb,
    parts: Vec<Part>,
    done: bool,
    optional: bool,
    payment: bool,
    started: bool,
    /// What exiles, for what is exiled to be found again as exiled with it (607.2a).
    source: ObjId,
    controller: PlayerId,
    ability: Option<crate::binding::AbilityKey>,
    generation: Option<usize>,
    /// What each object exiled became (400.7j).
    transitions: Vec<(ObjId, ObjId)>,
    actual: Vec<ObjId>,
    waiting: bool,
    completed: Option<crate::operation::Completed>,
}

impl Pending {
    /// The action as this frame begins it: who acts, what each could do it to and how many,
    /// read from the game as it now is. Nothing is read again.
    pub(super) fn new(game: &Game, f: &Frame, action: &Action) -> Pending {
        if let Of::Alternatives(actions) = &action.of {
            let alternatives: Vec<_> = actions.iter().map(|a| Self::new(game,f,a)).collect();
            let mut pending = alternatives[0].clone();
            pending.alternatives = Some(alternatives);
            return pending;
        }
        if let Of::Together(actions) = &action.of {
            let compound = actions.last().is_some_and(|a| a.verb == Verb::CollectEvidence);
            let mut projected = game.clone();
            let mut parts = Vec::new();
            // Evaluate the final eligibility and mana values after deterministic required
            // prefixes, through the same handlers as real execution. No SBA or trigger
            // resolution runs here. Projected successors are never retained as real IDs.
            for action in actions {
                let pending = Self::new(&projected, f, action);
                if action.verb == Verb::Exile && matches!(action.of, Of::Named(_)) {
                    for part in &pending.parts { for object in &part.among {
                        projected.exile_with(*object, f.ctx.object_source(), f.ctx.ability.clone(), f.ctx.link_generation);
                    } }
                }
                parts.extend(pending.parts);
            }
            parts.sort_by_key(|p| (p.player + game.players.len() - game.active) % game.players.len());
            return Self { alternatives: None, compound, verb: action.verb, parts, done: false, optional: false, payment: false, started: false, source: f.ctx.object_source(), controller: f.ctx.controller, ability: f.ctx.ability.clone(), generation: f.ctx.link_generation, transitions: Vec::new(), actual: Vec::new(), waiting: false, completed: None };
        }
        let verb = if action.verb == Verb::CollectEvidence { ir::Verb::Exile } else { action.verb.said() };
        // "you" in the description is the controller of the effect, whoever acts (109.5);
        // that it is of the acting player's own hand or permanents is the action's rule
        let ctx = &f.ctx;
        let mut count = match &action.count {
            Count::Exactly(n) => Some(n.evaluate(game, ctx, f) as usize),
            Count::All | Count::ManaValueAtLeast(_) => None,
        };
        // what the text names is what it now is (400.7e/j), where it still is that; every
        // one named is asked for, so that one departed makes the whole impossible (118.3)
        let named: Option<Vec<ObjId>> = if let Of::Named(objects) = &action.of {
            let mut all = Vec::new();
            for target in objects.iter().flat_map(|operand| action::named(game, f, operand)) {
                if !all.contains(&target) { all.push(target); }
            }
            count = Some(all.len().max(1));
            Some(all.into_iter().filter_map(|target| {
                let Target::Object(old) = target else { return None };
                action::move_identity(game, f, old)
            }).collect())
        } else { None };
        // each chooses in turn order (101.4)
        let parts = action::players(game, f, &action.players).into_iter().map(|player| {
            let actor = expr::Actor { parent: f, player };
            let pool: Vec<ObjId> = match &action.of {
                Of::Together(_) | Of::Alternatives(_) => unreachable!("coordinated parts were expanded above"),
                Of::Cards(cards) => game.players[player].hand.iter().copied().filter(|o| cards.contains(game, ctx, f, *o)).collect(),
                Of::Permanents(set) => {
                    game.battlefield.iter().copied().filter(|o| game.controller(*o) == player && set.contains(game, ctx, &actor, Target::Object(*o))).collect()
                }
                Of::Either(cards, set) => game.players[player].hand.iter().copied().filter(|o| cards.contains(game, ctx, f, *o))
                    .chain(game.battlefield.iter().copied().filter(|o| game.controller(*o) == player && set.contains(game, ctx, &actor, Target::Object(*o))))
                    .collect(),
                Of::Objects(set) => set.members(game, ctx, f).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None }).collect(),
                Of::Source => vec![f.ctx.object_source()],
                Of::Named(_) => named.clone().unwrap_or_default(),
            };
            let pool: Vec<_> = pool.into_iter().filter(|o| game.can(verb, player, Target::Object(*o))).collect();
            // "All" still asks for the prohibited permanents. A mandatory instruction does
            // as much as possible, but a payment requiring them cannot be paid (118.3).
            let count = count.or_else(|| (action.verb == Verb::Sacrifice).then_some(pool.len()));
            let cause = crate::event::Cause { source: ctx.object_source(), controller: ctx.controller };
            let among: Vec<ObjId> = pool.into_iter().filter(|o| action.verb != Verb::Sacrifice || game.can_cause_sacrifice(cause, player, *o)).collect();
            let chooser = match action.chooser {
                Chooser::Actor => player,
                Chooser::Controller => f.ctx.controller,
            };
            let selection = if let Count::ManaValueAtLeast(n) = &action.count {
                Some(crate::cost::Selection::ManaValueAtLeast { minimum: n.evaluate(game, ctx, f), values: Vec::new() }.evaluated(game, ctx, &among))
            } else { None };
            Part { player, verb: action.verb, selection, chooser, among, count, chosen: None, consent: None }
        }).collect();
        Pending { alternatives: None, compound: false, verb: action.verb, parts, done: false, optional: false, payment: false, started: false, source: f.ctx.object_source(), controller: f.ctx.controller, ability: f.ctx.ability.clone(), generation: f.ctx.link_generation, transitions: Vec::new(), actual: Vec::new(), waiting: false, completed: None }
    }

    pub(super) fn offer(mut self, optional: bool, payment: bool) -> Self {
        self.optional = optional;
        self.payment = payment;
        self
    }

    /// Paying an impossible cost is not offered (118.3); ordinary mandatory instructions
    /// still do as much as possible (609.3).
    pub(super) fn possible(&self) -> bool { self.alternatives.as_ref().map_or_else(|| !self.parts.is_empty() && !self.short(), |choices| choices.iter().any(Self::possible)) }
    pub(super) fn started(&self) -> bool { self.started }

    /// Go on: the decision it waits for, the same one until it is answered; or, when every
    /// choice is made, do all of it at once and return None. Once done, nothing more happens.
    pub fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        if self.done { return Ok(None); }
        if let Some(alternatives) = &self.alternatives {
            let among: Vec<_> = alternatives.iter().enumerate().filter(|(_,p)| p.possible()).map(|(i,p)| {
                let part = &p.parts[0];
                crate::state::ActionOption { index: i, body: crate::state::ActionOptionBody::Objects { verb: p.verb.said(), among: part.among.clone(), count: part.count.unwrap_or(part.among.len()) } }
            }).collect();
            if among.is_empty() { self.done = true; return Ok(None); }
            return Ok(Some(Decision::Choose { player: self.parts[0].player, choice: Choice::Alternative { among, optional: self.optional } }));
        }
        if self.waiting {
            let completed = self.completed.take().expect("the child completes before the parent advances");
            self.finish_objects(game, completed);
            return Ok(None);
        }
        if self.done {
            return Ok(None);
        }
        if !self.optional && self.payment && !self.possible() {
            self.done = true;
            return Ok(None);
        }
        if !self.optional { self.started = true; }
        if self.optional && self.compound && self.short() {
            for part in &mut self.parts { part.consent = Some(false); }
        }
        let mut chosen_before = Vec::new();
        for part in &mut self.parts {
            // Each affected player decides before the shared action commits (608.2e).
            // One impossible or declined payment must not suppress another player's choice.
            if self.optional {
                if part.short() { part.consent = Some(false); }
                match part.consent {
                    None => return Ok(Some(Decision::Choose { player: part.player, choice: Choice::May })),
                    Some(false) => { part.chosen = Some(Vec::new()); continue; }
                    Some(true) => { self.started = true; }
                }
            }
            // One permanent cannot be sacrificed twice for two descriptions (701.21a).
            part.among.retain(|o| !chosen_before.contains(o));
            if part.open() {
                let choice = if let Some(selection) = &part.selection {
                    Choice::ConstrainedObjects { verb: part.verb.said(), among: part.among.clone(), selection: selection.clone() }
                } else { Choice::Objects { verb: part.verb.said(), to: part.verb.to(), among: part.among.clone(), count: part.count.unwrap_or(0) } };
                return Ok(Some(Decision::Choose { player: part.chooser, choice }));
            }
            // with no more than is asked for, what there is (609.3)
            part.chosen.get_or_insert_with(|| if part.count == Some(0) { Vec::new() } else { part.among.clone() });
            chosen_before.extend(part.chosen.iter().flatten().copied());
        }
        if self.verb == Verb::Discard {
            let cards = self.parts.iter().flat_map(|part| part.chosen.iter().flatten().map(move |card| (part.player, *card))).collect::<Vec<_>>();
            self.waiting = true;
            match game.discard_batch(&cards, self.cause())? {
                Ok(completed) => self.finish_objects(game, completed),
                Err(decision) => return Ok(Some(decision)),
            }
            return Ok(None);
        }
        if matches!(self.verb, Verb::Sacrifice | Verb::Return) {
            let kind = |player| if self.verb == Verb::Sacrifice { crate::departure::Kind::Sacrifice(player) } else { crate::departure::Kind::Move(crate::Zone::Hand) };
            let actions = self.parts.iter().flat_map(|p| p.chosen.iter().flatten().map(move |o| (*o, kind(p.player)))).collect();
            self.waiting = true;
            match game.departure_batch(actions)? {
                Ok(completed) => self.finish_objects(game, completed),
                Err(decision) => return Ok(Some(decision)),
            }
            return Ok(None);
        }
        self.done = true;
        // each player's chosen card or permanent, beheld at once (701.4a)
        if self.verb == Verb::Behold {
            for part in &self.parts { game.behold(part.player, part.chosen.clone().unwrap_or_default()); }
            return Ok(None);
        }
        for part in &self.parts {
            let from = game.log.end();
            let before = game.departure_snapshot(&game.battlefield);
            for o in part.chosen.iter().flatten() {
                match part.verb {
                    Verb::Discard | Verb::Return => unreachable!("zone changes commit through their departure operation"),
                    Verb::Choose => {}
                    Verb::Behold => unreachable!("beheld together above"),
                    Verb::Tap => game.tap(*o),
                    Verb::Sacrifice => game.sacrifice(part.player, *o),
                    Verb::Exile | Verb::CollectEvidence => {
                        if let Some(now) = game.exile_with(*o, self.source, self.ability.clone(), self.generation) {
                            self.transitions.push((*o, now));
                        }
                    }
                }
            }
            if part.verb == Verb::CollectEvidence && (!self.optional || part.consent == Some(true)) {
                game.finish_simultaneous(from, before);
                game.event_log_mut().push(crate::Event::EvidenceCollected { player: part.player });
            }
        }
        Ok(None)
    }

    fn cause(&self) -> crate::event::Cause { crate::event::Cause { source: self.source, controller: self.controller } }

    pub(super) fn operation_complete(&mut self, completed: crate::operation::Completed) {
        assert!(self.waiting && self.completed.is_none(), "one result for the waiting choice");
        self.completed = Some(completed);
    }

    fn finish_objects(&mut self, _game: &mut Game, completed: crate::operation::Completed) {
        self.actual.extend(completed.transitions.iter().map(|(_, now)| *now));
        self.transitions.extend(completed.transitions);
        self.waiting = false;
        self.done = true;
    }

    /// The answer to the decision [`Self::advance`] last returned, which the game has
    /// checked against it. Anything else is not taken, and the decision is asked again.
    pub fn answer(&mut self, answer: Answer) {
        if let Some(alternatives) = &self.alternatives {
            match answer {
                Answer::No if self.optional => self.done = true,
                Answer::Alternative(index) if alternatives.get(index).is_some_and(Self::possible) => {
                    let mut selected = alternatives[index].clone();
                    selected.payment = self.payment || self.optional;
                    *self = selected;
                }
                _ => {}
            }
            return;
        }
        if self.optional && matches!(answer, Answer::Yes | Answer::No)
            && let Some(part) = self.parts.iter_mut().find(|p| p.consent.is_none()) {
            let consent = match answer { Answer::Yes => Some(true), Answer::No => Some(false), _ => None };
            part.consent = consent;
            if self.compound { for part in &mut self.parts { part.consent = consent; } }
            return;
        }
        let Some(part) = self.parts.iter_mut().find(|part| part.open()) else { return };
        if let Answer::Objects(chosen) = answer
            && part.selection.as_ref().map_or(Some(chosen.len()) == part.count, |s| s.accepts(&chosen))
            && chosen.iter().enumerate().all(|(i, o)| part.among.contains(o) && !chosen[..i].contains(o))
        {
            part.chosen = Some(chosen);
        }
    }

    /// Has it been done?
    pub fn done(&self) -> bool {
        self.done
    }

    /// What each player settled on so far, in turn order: the objects as they were before
    /// the action.
    pub fn by_player(&self) -> Vec<(PlayerId, Vec<ObjId>)> {
        self.parts.iter().filter_map(|part| Some((part.player, part.chosen.clone()?))).collect()
    }

    /// Payment intent and feasibility by the retained actor, distinct from moved objects.
    pub(super) fn outcomes(&self) -> [Vec<Target>; 3] {
        let mut groups: [Vec<Target>; 3] = Default::default();
        for part in &self.parts {
            let paid = self.started && (!self.optional || part.consent == Some(true));
            groups[usize::from(!paid)].push(Target::Player(part.player));
            if part.short() { groups[2].push(Target::Player(part.player)); }
        }
        for group in &mut groups { group.dedup(); }
        groups
    }

    /// Everything settled on so far: what "it" or "the discarded card" then refers to.
    pub fn chosen(&self) -> Vec<ObjId> {
        self.by_player().into_iter().flat_map(|(_, objects)| objects).collect()
    }

    /// Completed object actions, distinct from choices or begun cost payments.
    pub(super) fn actual(&self) -> &[ObjId] { &self.actual }
    /// Original cards from this choice's completed discards, never merely selected cards.
    pub(super) fn discarded_before(&self) -> Vec<ObjId> {
        assert_eq!(self.verb, Verb::Discard);
        self.transitions.iter().filter(|(_, now)| self.actual.contains(now)).map(|(before, _)| *before).collect()
    }

    /// What each object it moved to a public zone became (400.7j).
    pub(super) fn transitions(&self) -> &[(ObjId, ObjId)] {
        &self.transitions
    }

    /// Did some player have fewer than the action asks for?
    pub fn short(&self) -> bool {
        self.parts.iter().any(Part::short)
    }

    /// The choices made so far that everyone sees: of permanents to sacrifice, return or tap,
    /// which are on the battlefield (101.4b). Cards to discard are chosen in a hidden zone and
    /// stay private until all are discarded.
    pub fn public(&self) -> Vec<PublicChoice> {
        if !matches!(self.verb, Verb::Sacrifice | Verb::Return | Verb::Tap) {
            return Vec::new();
        }
        self.parts.iter().filter_map(|part| {
            Some(PublicChoice::Objects { player: part.chooser, verb: self.verb.said(), objects: part.chosen.clone()? })
        }).collect()
    }
}

#[cfg(test)]
mod tests;
