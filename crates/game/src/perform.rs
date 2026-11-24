//! Evaluated ordinary actions invoke the game operations here. A suspended operation owns
//! its request; the parent consumes its result without evaluating or submitting it again.
use crate::entry::EntryRequest;
use crate::event::Target;
use crate::operation::{Completed, Placement};
use crate::program::action::CounterRemoval;
use crate::state::{Decision, Game, ObjId, PlayerId, Zone};

/// What each of some players does, the same number of times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerAction {
    Draw,
    Mill,
    GainLife,
    LoseLife,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Movement {
    Zone { object: ObjId, zone: Zone },
    Library { object: ObjId, position: ir::action::LibraryPosition, order: Option<ir::action::Order>, known: bool },
    Enter(EntryRequest),
    Exile { object: ObjId, source: ObjId, ability: Option<crate::binding::AbilityKey>, generation: Option<usize> },
}

/// The two permanents counters move between, and the player who moves them (122.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Moving { pub(crate) from: ObjId, pub(crate) onto: ObjId, pub(crate) by: PlayerId }

impl Moving {
    /// What moves: each kind and how many (122.5). Nothing moves from a permanent onto
    /// itself, a kind it doesn't have, or a kind the other can't have put on it.
    fn counters(&self, game: &Game, kind: &Option<String>, count: Option<u32>) -> Vec<(String, u32)> {
        if self.from == self.onto { return Vec::new(); }
        let have = game.counters(self.from);
        let kinds: Vec<(String, u32)> = match kind {
            Some(kind) => vec![(kind.clone(), count.unwrap_or(u32::MAX))],
            None => have.iter().map(|(kind, n)| (kind.clone(), *n)).collect(),
        };
        kinds.into_iter().map(|(kind, n)| { let n = n.min(have.get(&kind).copied().unwrap_or(0)); (kind, n) })
            .filter(|(kind, n)| *n > 0 && game.may_place(Target::Object(self.onto), kind, self.by)).collect()
    }
}

/// An action with its operands evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    BecomeMonarch { players: Vec<PlayerId> },
    /// Those still in the game among them win it (104.2b), and every other player loses.
    WinGame { players: Vec<PlayerId> },
    /// Those still in the game among them lose it, at once (104.3e); `controller` controls
    /// the spell or ability that says so.
    LoseGame { players: Vec<PlayerId>, controller: PlayerId },
    IncreaseSpeed { players: Vec<PlayerId> },
    GetEmblem { players: Vec<PlayerId>, definition: crate::ability::Template },
    Destroy { objects: Vec<ObjId>, regenerable: bool },
    Counter { objects: Vec<ObjId>, destination: crate::program::action::CounterDestination },
    ExileUntil { objects: Vec<ObjId>, source: ObjId, ability: Option<crate::binding::AbilityKey>, generation: Option<usize> },
    Move(Vec<Movement>),
    /// Each player in turn, in the order given.
    Players { kind: PlayerAction, players: Vec<(PlayerId, u32)> },
    /// Tap, or untap (701.26).
    Tap { objects: Vec<ObjId>, tapped: bool },
    /// Transform (701.27). `own`: asked by an ability of this permanent, put on the stack at
    /// this time; it does not transform again if it has since (701.27f).
    Transform { objects: Vec<ObjId>, own: Option<(ObjId, u32)> },
    /// Each permanent among them has this level (716.2a).
    Level { objects: Vec<ObjId>, level: u32 },
    Unprepare { objects: Vec<ObjId> },
    Designate { objects: Vec<ObjId>, designation: ir::condition::ObjectDesignation },
    /// Turn face up (708.8); a manifested instant or sorcery is revealed instead (701.40g).
    /// Or turn face down (708.2a).
    TurnFace { objects: Vec<ObjId>, up: bool },
    /// One simultaneous damage event: source, recipient, amount (120.4).
    Damage(Vec<(ObjId, Target, u32)>),
    /// One such event that can't be prevented (615.12): "… and the damage can't be prevented".
    UnpreventableDamage(Vec<(ObjId, Target, u32)>),
    Prevent(crate::prevention::Effect),
    /// Counters put at once, as the replacement effects that apply have made each
    /// placement (616.1).
    PutCounters(Vec<Placement>),
    /// A named kind with a bounded/all count, or every kind (122.1, 609.3).
    RemoveCounters { from: Vec<Target>, counters: CounterRemoval },
    /// Counters removed from one permanent and put onto another (122.5); nothing when there
    /// are not both.
    MoveCounters { moving: Option<Moving>, kind: Option<String>, count: Option<u32> },
    /// Tokens created at once (701.7), as how they enter is replaced (614.12).
    Create(Vec<crate::creation::Request>),
    /// A regeneration shield for each (701.19a).
    Regenerate(Vec<ObjId>),
    PhaseOut(Vec<ObjId>),
    /// Each attachment, and what it is attached to (701.3a).
    Attach(Vec<(ObjId, Target)>),
    /// Two creatures to pair, by the player who controls the soulbond ability (702.95a).
    Pair(Option<(ObjId, ObjId, PlayerId)>),
}

impl Action {
    /// Collect all optional player decisions before committing their shared action (608.2e).
    pub(crate) fn retain_players(&mut self, accepted: &[PlayerId]) {
        match self {
            Self::BecomeMonarch { players } | Self::WinGame { players } | Self::LoseGame { players, .. } | Self::IncreaseSpeed { players } | Self::GetEmblem { players, .. } => players.retain(|p| accepted.contains(p)),
            Self::Players { players, .. } => players.retain(|(p, _)| accepted.contains(p)),
            Self::PutCounters(batch) => batch.retain(|placement| match placement.on {
                Target::Player(p) => accepted.contains(&p), Target::Object(_) => true,
            }),
            Self::Create(requests) => requests.retain(|r| match r {
                crate::creation::Request { player,.. } => accepted.contains(player),
            }),
            _ => {}, // object actions have a single controller's consent
        }
    }
    /// Can this evaluated action be chosen as an optional instruction/payment? (608.2d,
    /// 118.12). Drawing from an empty library is explicitly possible (121.3).
    pub(crate) fn possible(&self, game: &Game, payment: bool) -> bool {
        match self {
            // no more cards can be milled than the library has (701.17b)
            Self::Players { kind: PlayerAction::Mill, players } => !players.is_empty()
                && players.iter().all(|(p, count)| game.players[*p].library.len() >= *count as usize),
            Self::Players { players, .. } => !players.is_empty(),
            Self::Create(_) => true,
            Self::PutCounters(batch) => {
                // Referenced payments require the full action (118.3/118.12);
                // standalone optional effects may happen in part (609.3).
                let legal = |p: &Placement| game.holds_counters(p.on)
                    && (p.count == 0 || game.may_place(p.on, &p.kind, p.by));
                !batch.is_empty() && if payment { batch.iter().all(legal) } else { batch.iter().any(legal) }
            },
            // Optional removal cannot choose an impossible full removal (608.2d/118.12).
            // Mandatory effects still use run()'s partial-removal rule (609.3).
            Self::RemoveCounters { from, counters } => !from.is_empty() && from.iter().all(|holder|
                game.holds_counters(*holder) && match counters {
                    CounterRemoval::Named { kind, count } => count.is_none_or(|n| game.counters(*holder).get(kind).copied().unwrap_or(0) >= n),
                    CounterRemoval::All => true,
                }),
            Self::Move(moves) => !moves.is_empty(),
            Self::MoveCounters { moving, kind, count } => moving.is_some_and(|m| {
                let moved = m.counters(game, kind, *count);
                !moved.is_empty() && count.is_none_or(|n| moved.iter().all(|(_, m)| *m == n))
            }),
            Self::Tap { objects, tapped } => !objects.is_empty() && objects.iter().all(|o| game.obj(*o).tapped != *tapped),
            _ => true,
        }
    }
}

/// Start once. A decision transfers ownership to the pending operation.
pub(crate) fn run(game: &mut Game, action: Action) -> crate::Res<Result<Completed, Decision>> {
    let mut result = Completed::default();
    match action {
        Action::BecomeMonarch { players } => { for player in players { game.become_monarch(player); } },
        Action::WinGame { players } => game.win_by_effect(&players)?,
        Action::LoseGame { players, controller } => game.lose_by_effect(&players, controller)?,
        Action::IncreaseSpeed { players } => { for player in players { game.increase_speed(player); } },
        Action::GetEmblem { players, definition } => {
            let first = game.clock + 1;
            for player in players {
                let object = game.new_object(None, definition.def().chars(), player, player, Zone::Command);
                game.obj_mut(object).emblem = Some(definition);
                game.obj_mut(object).arrived = Some(game.log.end());
                game.log.push(crate::Event::EmblemCreated { object });
            }
            // the emblems are put into the command zone at once (114.2, 613.7m)
            game.stamped_at_once(first);
        },
        Action::Move(groups) => {
            let mut entries = Vec::new();
            let mut actions = Vec::new();
            for movement in groups {
                match movement {
                    Movement::Enter(request) => entries.push(request),
                    Movement::Library {object,position,order,known} => actions.push((object,crate::departure::Kind::Library {position,order,known})),
                    Movement::Zone { object, zone } => actions.push((object, crate::departure::Kind::Move(zone))),
                    Movement::Exile { object, source, ability, generation } => actions.push((object, crate::departure::Kind::Exile { source, ability, generation })),
                }
            }
            let departures = crate::departure::Batch::new(game, actions);
            if !entries.is_empty() { return game.prepare_entry_departures(entries, departures); }
            return game.run_operation(crate::operation::Operation::Depart(departures));
        },
        Action::ExileUntil { objects, source, ability, generation } => result.transitions = game.exile_until(&objects, source, ability, generation),
        Action::Destroy { objects, regenerable } => return game.departure_batch(objects.into_iter().map(|o| (o, crate::departure::Kind::Destroy { regenerable })).collect()),
        Action::RemoveCounters { from, counters } => {
            // Freeze the inventory for one instruction before removing any kind. Every
            // holder and kind is committed before state-based actions (608.2c, 704.4).
            let mut seen = Vec::new();
            let mut removals = Vec::new();
            for holder in from {
                if seen.contains(&holder) { continue; }
                seen.push(holder);
                match &counters {
                    CounterRemoval::Named { kind, count } => {
                        let requested = count.unwrap_or_else(|| game.counters(holder).get(kind).copied().unwrap_or(0));
                        removals.push((holder, kind.clone(), requested));
                    }
                    CounterRemoval::All => {
                        removals.extend(game.counters(holder).iter().filter(|(_, n)| **n > 0)
                            .map(|(kind, n)| (holder, kind.clone(), *n)));
                    }
                }
            }
            let first = game.log.end();
            for (holder, kind, requested) in removals {
                let actual = game.remove_counters(holder, &kind, requested);
                if actual > 0 { result.removed.push(crate::binding::Removal { from: holder, kind, count: actual }); }
            }
            if game.log.end() != first { game.finish_simultaneous(first, crate::layers::Working::before(Vec::new())); }
        },
        // removed from the one, then put onto the other, as those events (122.5); what
        // replaces putting counters applies to the second (614.1)
        Action::MoveCounters { moving: Some(moving), kind, count } => {
            let moved = moving.counters(game, &kind, count);
            let first = game.log.end();
            let mut batch = Vec::new();
            for (kind, n) in moved {
                let actual = game.remove_counters(moving.from, &kind, n);
                if actual == 0 { continue; }
                result.removed.push(crate::binding::Removal { from: Target::Object(moving.from), kind: kind.clone(), count: actual });
                batch.push(Placement { on: Target::Object(moving.onto), kind, count: actual, by: moving.by });
            }
            if game.log.end() != first { game.finish_simultaneous(first, crate::layers::Working::before(Vec::new())); }
            if !batch.is_empty() && let Err(decision) = game.put_counter_batch(batch, crate::operation::EventOrigin::Effect)? { return Ok(Err(decision)); }
        }
        Action::MoveCounters { moving: None, .. } => {}
        Action::Counter { objects, destination } => {
            use crate::program::action::CounterDestination;
            let countered: Vec<_> = objects.into_iter().filter(|o| game.countered(*o)).collect();
            let spells = countered.iter().copied().filter(|o| game.obj(*o).ability.is_none());
            let departures = if destination == CounterDestination::Graveyard {
                crate::departure::Batch::new(game, spells.map(|o| (o, crate::departure::Kind::RuleMove(Zone::Graveyard))).collect())
            } else {
                let (zone, position) = match destination {
                    CounterDestination::Exile => (Zone::Exile, None), CounterDestination::Hand => (Zone::Hand, None),
                    CounterDestination::Library(position) => (Zone::Library, Some(position)),
                    CounterDestination::Graveyard => unreachable!("ordinary destination handled above"),
                };
                // Self-replacement first (614.15, 616.1a). Flashback can still replace
                // this departure to a hand/library; a graveyard-only redirect cannot.
                crate::departure::Batch::destined(game, spells.map(|object| {
                    let mut to = game.departure_destination(object, zone);
                    if to.zone == Zone::Library { to.position = position; }
                    (object, crate::departure::Kind::RuleMove(zone), to)
                }).collect())
            };
            return game.run_operation(crate::operation::Operation::Counter { departures, countered });
        },
        Action::Players { kind: PlayerAction::Draw, players } => return game.draw_batch(players),
        // their top cards are milled at once, as one event (701.17a)
        Action::Players { kind: PlayerAction::Mill, players } => return game.departure_batch(players.into_iter()
            .flat_map(|(p, count)| game.top_cards(p, count as usize).into_iter().map(move |card| (card, crate::departure::Kind::Mill(p))))
            .collect()),
        Action::Players { kind, players } => {
            for (p, count) in players {
                match kind {
                    PlayerAction::Draw | PlayerAction::Mill => unreachable!("draws retain their own sequential progress; mills are one batch"),
                    PlayerAction::GainLife => game.gain_life(p, count)?,
                    PlayerAction::LoseLife => {
                        let before = game.players[p].life;
                        game.lose_life(p, count)?;
                        let lost = u32::try_from(i64::from(before) - i64::from(game.players[p].life)).unwrap_or(0);
                        result.lost = result.lost.checked_add(lost).ok_or_else(||
                            crate::Unsupported("life lost exceeds the engine numerical range".into()))?;
                    }
                }
            }
        }
        Action::Tap { objects, tapped } => {
            for o in objects {
                if tapped { game.tap(o) } else { game.untap(o) }
            }
        }
        // each one transformed or turned is given its new timestamp at once (613.7g, 613.7f, 613.7m)
        Action::Transform { objects, own } => {
            let first = game.clock + 1;
            for o in objects {
                game.transform(o, own.filter(|(source, _)| *source == o).map(|(_, put)| put));
            }
            game.stamped_at_once(first);
        }
        Action::Level { objects, level } => {
            for o in objects { game.become_level(o, level); }
        }
        Action::Unprepare { objects } => { for o in objects { game.unprepare(o); } }
        Action::Designate { objects, designation } => {
            for o in objects { game.designate(o, designation); }
        }
        // what an "as this is turned face up" ability puts on each is put as it turns (708.11)
        Action::TurnFace { objects, up: true } => {
            let mut instructions = Vec::new();
            let first = game.clock + 1;
            for o in objects {
                if game.turn_face_up(o) == crate::face::TurnedUp::Turned { instructions.extend(game.as_turned_face_up(o)?); }
            }
            game.stamped_at_once(first);
            if !instructions.is_empty() { if let Err(decision)=game.put_counter_instructions(instructions,crate::operation::EventOrigin::Effect)? { return Ok(Err(decision)); } }
        }
        Action::TurnFace { objects, up: false } => {
            let first = game.clock + 1;
            for o in objects { game.turn_face_down(o); }
            game.stamped_at_once(first);
        }
        Action::Damage(damage) => return game.damage_operation(&damage, false,crate::operation::EventOrigin::Effect),
        Action::UnpreventableDamage(damage) => return game.unpreventable_damage(&damage),
        Action::PutCounters(batch) => if let Err(decision)=game.put_counter_batch(batch,crate::operation::EventOrigin::Effect)? { return Ok(Err(decision)); },
        Action::Create(requests) => return game.create_tokens(requests,crate::operation::EventOrigin::Effect),
        Action::Prevent(effect) => game.begin_prevention(effect),
        Action::Regenerate(objects) => { for object in objects { game.regenerate(object); } },
        Action::PhaseOut(objects) => game.phase_out(&objects),
        // an attachment that is not legal, or that is already there, does nothing (701.3b)
        // what becomes attached together is given its timestamps at once (613.7e, 613.7m)
        Action::Attach(attachments) => {
            let first = game.clock + 1;
            for (object, to) in attachments { game.attach(object, to); }
            game.stamped_at_once(first);
        },
        Action::Pair(pair) => if let Some((one, other, controller)) = pair { game.pair(one, other, controller); },
    }
    Ok(Ok(result))
}
