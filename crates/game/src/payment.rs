//! A casting payment owns its unfinished mana sequence (601.2g-h, 605.3b).
//! Mana resolutions are ordinary child procedures. Their completion never replays a step.
use std::collections::VecDeque;
use crate::{Game, ObjId, PlayerId, Event};
use crate::cost::Plan;
use crate::legal::Step;
use crate::procedure::Procedure;
use crate::select::Ctx;
use crate::state::{Decision, Pool, Spent};

#[derive(Debug, Clone)]
pub(crate) struct Payment {
    audit: Option<Box<crate::payment_audit::Scope>>,
    next_child: usize,
    pub player: PlayerId,
    /// CR 401.5 access boundary of this announcement, including its nested mana children.
    pub(crate) library_tops: Option<Vec<Option<ObjId>>>,
    permission: Option<crate::cast_permission::Use>,
    pub source: ObjId,
    ctx: Ctx<'static>,
    finish: Finish,
    undo: Undo,
    triggers: std::ops::Range<usize>,
    cost: Plan,
    steps: VecDeque<Step>,
    paying: Vec<Vec<ObjId>>,
    with: Option<Pool>,
    waiting: bool,
    committing: Option<Commitment>,
    /// An exposed question commits the prefix; later invalid plans ask for correction.
    pub interactive: bool,
    /// The resolution directly below asked for this and is told how it ended: an effect's
    /// payment, or a spell cast while an effect is applied (608.2g).
    reports: bool,
}
#[derive(Debug, Clone)]
pub(crate) enum Undo {
    Companion,
    /// Turning a permanent face up moved nothing; what was shown stays shown (701.40b).
    TurnFaceUp,
    /// Paying an unlock cost moved nothing (709.5e).
    Unlock,
    /// A card exiled from a hand by a special action (702.170a, 702.143a) goes back there.
    ExiledFromHand { original: ObjId, object: Box<crate::state::Object>, position: usize },
    Spell { original: ObjId, object: Box<crate::state::Object>, position: usize },
    /// The lifetime index belongs to the exact incarnation retained in `Payment::source`.
    Ability { reservation: usize, started: usize, lifetime: Option<usize> },
    /// A payment an effect asked for announced nothing: abandoning it only declines it.
    Effect,
}
#[derive(Debug, Clone)]
pub(crate) enum Finish {
    Companion,
    Plot,
    /// The {2} of foretelling is paid: the card is foretold (702.143a).
    Foretell,
    /// The cost of turning a face-down permanent face up is paid: turn it (701.40b), with a
    /// +1/+1 counter if it was a megamorph cost (702.37b).
    TurnFaceUp { megamorph: bool },
    /// The unlock cost of this half of a Room is paid: it is unlocked (709.5e).
    Unlock { half: usize },
    Spell,
    Ability { keyword: Option<ir::Keyword>, on_stack: Option<ObjId>, invocations: Vec<crate::program::Invocation>,
        taps_for_mana: bool, adding: Vec<Vec<ir::mana::ManaSymbol>>,
        /// `adding` was checked against every choice of mana the ability could make.
        checked: bool,
        /// What is said for the triggered mana abilities it triggers (605.4a).
        more: Vec<Vec<ir::mana::ManaSymbol>> },
    /// "You may pay {B}" as an effect is applied (118.12): whether it was paid goes back to
    /// the resolution directly below this payment, and to nothing else.
    Effect,
}
impl Payment {
    pub(crate) fn audit_child(mut self, scope: Option<Box<crate::payment_audit::Scope>>) -> Self { self.audit=scope; self }
    pub(crate) fn companion(player: PlayerId, card: ObjId, cost: Plan, steps: &[Step], with: Option<&Pool>) -> Self {
        Self {audit:None,next_child:0,library_tops:None,permission:None,player,source:card,ctx:Ctx::new(player,card),finish:Finish::Companion,undo:Undo::Companion,triggers:0..0,cost,steps:steps.iter().cloned().collect(),paying:Vec::new(),with:with.cloned(),waiting:false,committing:None,interactive:false,reports:false}
    }
    pub(crate) fn library_tops(mut self,tops:Vec<Option<ObjId>>)->Self {self.library_tops=Some(tops);self}
    pub(crate) fn permission(mut self, usage: Option<crate::cast_permission::Use>) -> Self {self.permission=usage;self}
    pub(crate) fn turn_face_up(player: PlayerId, permanent: ObjId, cost: Plan, megamorph: bool, steps: &[Step], with: Option<&Pool>) -> Self {
        Self {audit:None,next_child:0,library_tops:None,permission:None,player,source:permanent,ctx:Ctx::new(player,permanent),finish:Finish::TurnFaceUp { megamorph },undo:Undo::TurnFaceUp,triggers:0..0,cost,steps:steps.iter().cloned().collect(),paying:Vec::new(),with:with.cloned(),waiting:false,committing:None,interactive:false,reports:false}
    }
    pub(crate) fn unlock(player: PlayerId, room: ObjId, half: usize, cost: Plan, steps: &[Step], with: Option<&Pool>) -> Self {
        Self { finish: Finish::Unlock { half }, undo: Undo::Unlock, ..Self::turn_face_up(player, room, cost, false, steps, with) }
    }
    pub(crate) fn plot(player: PlayerId, card: ObjId, cost: Plan, steps: &[Step], with: Option<&Pool>, undo: Undo, triggers: std::ops::Range<usize>) -> Self {
        Self {audit:None,next_child:0,library_tops:None, permission: None, player, source: card, ctx: Ctx::new(player,card), finish: Finish::Plot, undo, triggers, cost, steps: steps.iter().cloned().collect(), paying: Vec::new(), with: with.cloned(), waiting: false, committing: None, interactive: false, reports: false }
    }
    pub(crate) fn foretell(player: PlayerId, card: ObjId, cost: Plan, steps: &[Step], with: Option<&Pool>, undo: Undo, triggers: std::ops::Range<usize>) -> Self {
        Self { finish: Finish::Foretell, ..Self::plot(player, card, cost, steps, with, undo, triggers) }
    }

    /// A payment the effect of `source` asks of `player` while it is applied.
    pub(crate) fn effect(player: PlayerId, source: ObjId, cost: Plan, steps: Vec<Step>, paying: Vec<Vec<ObjId>>, with: Option<Pool>) -> Self {
        Self {audit:None,next_child:0,library_tops:None, permission: None, player, source, ctx: Ctx::new(player, source), finish: Finish::Effect, undo: Undo::Effect, triggers: 0..0, cost,
            steps: steps.into(), paying, with, waiting: false, committing: None, interactive: false, reports: true }
    }
    pub(crate) fn spell(player: PlayerId, spell: ObjId, cost: Plan, steps: &[Step], paying: &[Vec<ObjId>], with: Option<&Pool>, undo: Undo, triggers: std::ops::Range<usize>) -> Self {
        Self {audit:None,next_child:0,library_tops:None, permission: None, player, source: spell, ctx: Ctx::new(player, spell), finish: Finish::Spell, undo, triggers, cost, steps: steps.iter().cloned().collect(), paying: paying.to_vec(),
            with: with.cloned(), waiting: false, committing: None, interactive: false, reports: false }
    }
    /// This spell is cast while an effect is applied: the resolution below is told when it
    /// has been cast, or that the cast was abandoned.
    pub(crate) fn reporting(mut self) -> Self { self.reports = true; self }
    pub(crate) fn ability(ctx: Ctx<'static>, cost: Plan, finish: Finish, steps: &[Step], paying: Vec<Vec<ObjId>>, with: Option<&Pool>, undo: Undo, triggers: std::ops::Range<usize>) -> Self {
        Self {audit:None,next_child:0,library_tops:None, permission: None, player: ctx.controller, source: ctx.object_source(), ctx, finish, undo, triggers, cost,
            steps: steps.iter().cloned().collect(), paying, with: with.cloned(), waiting: false, committing: None, interactive: false, reports: false }
    }
    pub(crate) fn operation_complete(&mut self, result: crate::operation::Completed) {
        let commitment = self.committing.as_mut().expect("discard cost commitment");
        assert!(commitment.discard_started && commitment.discard_result.is_none());
        commitment.discard_result = Some(result);
    }
    pub(crate) fn answer(&mut self, answer: crate::state::Answer) {
        self.committing.as_mut().expect("cost ordering owns this answer").answer(answer);
    }
    /// Lock and validate a payment once its explicitly supplied mana window is over.
    /// A leaf is prepared transactionally before becoming a child, so a rejected
    /// activation still belongs to the parent's correction boundary (733.1).
    pub(crate) fn prepare(&mut self, game:&mut Game) -> crate::play::ActionResult<()> {
        if !self.steps.is_empty() || self.committing.is_some() {return Ok(());}
        self.audit_scope(game);
        let on = match &self.finish {
            Finish::Spell => Spent::Spell(self.source, crate::card::CastAs::Whole),
            Finish::Ability { keyword, on_stack, .. } => Spent::Ability { source: self.source, keyword: *keyword, mana: on_stack.is_none() },
            Finish::Companion | Finish::Effect | Finish::Plot | Finish::Foretell | Finish::TurnFaceUp { .. } | Finish::Unlock { .. } => Spent::Effect(self.source),
        };
        if self.committing.is_none() {
            self.committing = Some(game.prepare_cost(self.ctx.clone(), &self.cost, &self.paying, self.with.as_ref(), on)?);
            // A replacement question can interrupt cost commitment. Keep the validated
            // announcement's reveal grants visible through that pause as well (701.20a).
            let announced = match self.finish {
                Finish::Spell => Some(self.source),
                Finish::Ability { on_stack, .. } => on_stack,
                _ => None,
            };
            if let Some(announced) = announced {
                for &card in &self.committing.as_ref().unwrap().bound.revealed {
                    if !game.obj(announced).bound.revealed.contains(&card) { game.obj_mut(announced).bound.revealed.push(card); }
                }
            }
        }
        Ok(())
    }
    fn audit_scope(&mut self, game:&mut Game) {
        if game.payment_audit.is_some() {
            let scope=self.audit.get_or_insert_with(||Box::new(crate::payment_audit::Scope::root()));
            scope.prior.get_or_insert_with(||game.players[self.player].pool.clone());
            game.payment_scope=Some(scope.clone());
        }
    }
    pub(crate) fn advance(&mut self, game: &mut Game) -> crate::play::ActionResult<bool> {
        if self.waiting {
            game.check_triggers();
            self.waiting = false;
            if game.returning() { return Ok(false); }
        }
        if let Some(step) = self.steps.front() {
            if let Some(scope)=&mut game.payment_scope { scope.path.push(self.next_child); scope.prior=None; }
            game.begin_payment_mana(self.player, step, self.interactive)?;
            self.next_child += 1;
            self.steps.pop_front();
            self.waiting = true;
            return Ok(false);
        }
        self.prepare(game)?;
        if let Some(decision) = self.committing.as_mut().unwrap().advance(game)? {
            game.decision = Some(decision);
            return Ok(false);
        }
        if game.returning() { return Ok(false); }
        let Commitment { bound, spent, paid, .. } = self.committing.take().unwrap();
        match &self.finish {
            Finish::Companion => game.complete_companion(self.player,self.source),
            // already shown as it was announced
            Finish::TurnFaceUp { megamorph } => game.turn_up_paid(self.source, *megamorph)?,
            Finish::Unlock { half } => game.unlock(self.source, *half, self.player),
            Finish::Foretell => game.complete_foretell(self.player, self.source),
            Finish::Plot => { game.permit_plotted(self.player,self.source); game.log.push(Event::Plotted { player: self.player, card: self.source }); },
            Finish::Spell => {
                if let Some(usage)=self.permission.clone() && !matches!(&self.undo,Undo::Spell{object,..} if object.zone==crate::Zone::Library) {game.cast_permission_uses.push(usage);}
                let mut bound = bound;
                // "the chosen player" of a spell's gift: the opponent chosen, or none (702.174a)
                if game.def(self.source).is_some_and(|d| crate::keyword::gift(d).is_some()) {
                    let chosen = game.obj(self.source).gift.map(crate::Target::Player).into_iter().collect();
                    bound.inputs.push((crate::program::input::Input::Gift, chosen));
                }
                game.obj_mut(self.source).cast_exiled = bound.cost_exiled.clone();
                game.obj_mut(self.source).bound = bound;
                game.obj_mut(self.source).mana_paid = paid;
                if let Some(announcement) = &mut game.obj_mut(self.source).cast_announcement {
                    announcement.payment = Some(self.cost.cast_payment(&self.paying));
                }
                game.obj_mut(self.source).uncounterable = spent.iter().any(|m| m.uncounterable);
                game.commander_cast_completed(self.source, self.player);
                game.prepared_cast_completed(self.source);
                let cast = Event::Cast { spell: self.source, by: self.player };
                game.note(&cast);
                game.log.push(cast);
                game.announcement_crime(self.player, self.source);
                // Each marked mana creates one delayed occurrence only after casting completes
                // (106.6a, 603.7e); failed/reversed commitments never reach this point.
                for unit in &spent {
                    if let Some(trigger)=&unit.trigger {
                        let ctx=trigger.captured.context();
                        if trigger.on.allows(game,&ctx,Spent::Spell(self.source,crate::card::CastAs::Whole)) {
                            game.triggered.push(crate::state::Triggered {source:Some(unit.source),controller:unit.controller,origin:crate::binding::Origin::Own,index:0,bound:trigger.bound.clone(),delayed:Some(trigger.captured.identity()),captured:Some(Ok(trigger.captured.clone()))});
                        }
                    }
                }
            },
            Finish::Ability { keyword, on_stack, invocations, taps_for_mana, adding, checked, more, .. } => {
                let mut bound = bound;
                // the creatures tapped to pay a crew ability's cost crew the Vehicle (702.122b-c)
                // and those tapped to pay a saddle ability's cost saddle the permanent (702.171c)
                let key = invocations.first().and_then(|i| i.ctx.ability.clone());
                let def = key.as_ref().and_then(|key| game.definition(self.source, &key.origin));
                let (crew, saddle) = match (key.as_ref(), def) {
                    (Some(key), Some(d)) => (crate::keyword::crews(d, key.index), crate::keyword::saddles(d, key.index)),
                    _ => (false, false),
                };
                let tapped: Vec<ObjId> = bound.inputs.iter().find(|(i, _)| *i == crate::program::input::Input::Cost(0))
                    .map(|(_, paid)| paid.iter().filter_map(|t| if let crate::Target::Object(o) = t { Some(*o) } else { None }).collect())
                    .unwrap_or_default();
                if crew { bound.crew = Some(tapped.clone()); }
                if let Some(ability) = on_stack { game.obj_mut(*ability).bound = bound.clone(); }
                game.log.push(Event::Activated { source: self.source, by: self.player, ability: *on_stack, keyword: *keyword });
                if let Some(ability) = on_stack { game.announcement_crime(self.player, *ability); }
                let tapping = if let Some(creatures) = bound.crew.clone() { Some(Event::Crews { vehicle: self.source, creatures }) }
                    else if saddle { Some(Event::Saddles { mount: self.source, creatures: tapped }) } else { None };
                if let Some(event) = tapping { game.note(&event); game.log.push(event); }
                if on_stack.is_none() {
                    game.start_mana(self.source, self.player, invocations.clone(), bound, *taps_for_mana, adding, *checked, more, false);
                }
            },
            // Its parent is told by the driver, once this payment is off the stack.
            Finish::Effect => {},
        }
        Ok(true)
    }
}
/// Once total payment is validated, each cost part commits once. A destination-order
/// question retains the current part, prior cost inputs and actual public successors.
#[derive(Debug, Clone)]
pub(crate) struct Commitment {
    player: PlayerId,
    payments: VecDeque<crate::cost::Payment>,
    bound: crate::state::Bound,
    spent: Vec<crate::state::Restricted>,
    paid: crate::state::PaidMana,
    departure: Option<(crate::event::Seq, crate::layers::Working, crate::departure::Batch)>,
    counters: Option<crate::operation::Ordering<crate::operation::CounterEvent>>,
    counter_answer: Option<crate::state::Answer>,
    discard_started: bool,
    discard_result: Option<crate::operation::Completed>,
}
impl Commitment {
    pub(crate) fn new(player: PlayerId, mut payments: Vec<crate::cost::Payment>, bound: crate::state::Bound, spent: Vec<crate::state::Restricted>, paid: crate::state::PaidMana) -> Self {
        // 601.2h permits choosing payment order. Tap/remove counters before moving the
        // resource out of its zone, even if the printed cost lists its sacrifice first.
        payments.sort_by_key(|p| matches!(p, crate::cost::Payment::Objects { verb, .. } if *verb != ir::Verb::Tap));
        Self { player, payments: payments.into(), bound, spent, paid, departure: None, counters: None, counter_answer: None, discard_started: false, discard_result: None }
    }
    fn answer(&mut self, answer: crate::state::Answer) {
        if self.counters.is_some() { self.counter_answer = Some(answer); }
        else { self.departure.as_mut().expect("the ordered cost part").2.answer(answer); }
    }
    fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        use crate::cost::Payment as Part;
        use ir::Verb;
        while let Some(part) = self.payments.front().cloned() {
            let placement = match &part {
                Part::Loyalty(on, n) if *n >= 0 => Some((*on, "loyalty", *n as u32)),
                Part::Blight { on, count } => Some((*on, "-1/-1", *count)),
                _ => None,
            };
            if let Some((on, kind, count)) = placement {
                let ordering = self.counters.get_or_insert_with(|| crate::operation::Ordering::new(game,
                    vec![crate::operation::CounterEvent::new(crate::operation::Placement {
                        on: crate::Target::Object(on), kind: kind.into(), count, by: self.player },crate::operation::EventOrigin::AnnouncementCost)]));
                let decision = if let Some(answer) = self.counter_answer.take() {
                    ordering.answer(game, self.player, answer)?
                } else { ordering.advance(game)? };
                if let Some(decision) = decision { return Ok(Some(decision)); }
                self.counters.take().unwrap().prepare_counters(game).commit(game)?;
                game.reconcile_linked_acquisitions();

                game.check_triggers();
                // A legally paid placement remains paid after replacement (118.11).
                self.payments.pop_front();
                if game.returning() { return Ok(None); }
                continue;
            }
            match part {
                Part::Mana(_) | Part::Reveal(_) => {},
                Part::Tap(o) => game.tap(o),
                Part::Untap(o) => game.untap(o),
                Part::Life(n) => { game.pay_life(self.player, n)?; },
                Part::Loyalty(_, n) if n >= 0 => unreachable!("positive counter cost handled above"),
                Part::Loyalty(o, n) => {
                    game.remove_counters(o, "loyalty", n.unsigned_abs());
                },
                Part::RemoveCounters { on, kind, count } => { game.remove_counters(on, &kind, count); },
                // blight N (701.68a)
                Part::Blight { .. } => unreachable!("counter cost handled above"),
                // "Tap two untapped creatures you control": one event (118.3, 603.2c)
                Part::Objects { verb: Verb::Tap, objects, .. } => {
                    let from = game.log.end();
                    for object in objects { game.tap(object); }
                    game.finish_simultaneous(from, crate::layers::Working::before(Vec::new()));
                },
                // beholding reveals a card from the hand or chooses a permanent (701.4a)
                Part::Objects { verb: Verb::Behold, objects, .. } => game.behold(self.player, objects),
                // the cards a cost reveals from the hand are shown to all players (701.20a)
                Part::Objects { verb: Verb::Reveal, objects, .. } => { game.reveal(self.player, objects); },
                Part::Objects { verb: verb @ (Verb::Sacrifice | Verb::Move), objects, .. } => {
                    let (_, _, batch) = self.departure.get_or_insert_with(|| (
                        game.log.end(), game.departure_snapshot(&game.battlefield),
                        crate::departure::Batch::new(game, objects.iter().map(|o| (*o, if verb == Verb::Move {
                            // The cost compiler admits only returning a permanent to its owner's hand.
                            crate::departure::Kind::Move(crate::Zone::Hand)
                        } else { crate::departure::Kind::Sacrifice(self.player) })).collect()),
                    ));
                    if let Some(decision) = batch.prepare() { return Ok(Some(decision)); }
                    let (from, before, batch) = self.departure.take().unwrap();
                    self.bound.became.extend(batch.commit(game).transitions);
                    game.finish_simultaneous(from, before);
                },
                Part::Objects { verb: Verb::Discard, objects, .. } => {
                    if !self.discard_started {
                        self.discard_started = true;
                        let cards = objects.iter().map(|card| (self.player, *card)).collect::<Vec<_>>();
                        match game.discard_proposed(&cards, None)? {
                            Ok(result) => self.discard_result = Some(result),
                            Err(decision) => return Ok(Some(decision)),
                        }
                    }
                    let result = self.discard_result.take().expect("discard child completed before cost resumes");
                    self.bound.became.extend(result.transitions);
                    self.discard_started = false;
                },
                Part::Objects { verb, objects, producer } => {
                    let from = game.log.end();
                    let before = game.departure_snapshot(&game.battlefield);
                    for object in objects {
                        match verb {
                            Verb::Exile | Verb::CollectEvidence => {
                                if let Some(now) = game.exile_result(object)
                                    && game.obj(now).zone == crate::Zone::Exile
                                    && let Some(ref producer) = producer {
                                    self.bound.cost_exiled.push((producer.clone(), now));
                                }
                            },
                            _ => unreachable!("supported object cost"),
                        }
                        if let Some(now) = game.became(object) { self.bound.became.push((object, now)); }
                    }
                    game.finish_simultaneous(from, before);
                    if verb == Verb::CollectEvidence { game.event_log_mut().push(Event::EvidenceCollected { player: self.player }); }
                },
            }
            self.payments.pop_front();
            // Written cost parts are sequential even within one announcement (601.2h).
            game.reconcile_linked_acquisitions();
            game.check_triggers();
            if game.returning() { return Ok(None); }
        }
        Ok(None)
    }
}

/// The retained root payment and the triggered-mana choice currently interrupting it.
struct PendingManaPayment<'a> {
    index: usize,
    payment: &'a Payment,
    scope: &'a [usize],
    trigger_source: ObjId,
    trigger_ability: crate::binding::AbilityKey,
}

impl Game {
    /// Only a real triggered mana choice inside an unfinished root spell payment.
    /// No future production, spending or completion is asserted by this boundary.
    fn pending_mana_payment(&self) -> Option<PendingManaPayment<'_>> {
        use crate::state::{Choice, Completion, Decision};
        let Some(Decision::Choose { player, choice: Choice::Mana { .. } }) = &self.decision else { return None };
        let Some(Procedure::Resolution(at)) = self.procedures.last() else { return None };
        if !matches!(at.completion, Completion::Mana { triggered: true, .. }) { return None; }
        let scope = at.payment_scope.as_ref()?;
        let (index, payment) = self.procedures.iter().enumerate().rev().find_map(|(i, p)| match p {
            Procedure::Payment(p) if matches!(p.finish,Finish::Spell) => Some((i, p)), _ => None,
        })?;
        if payment.player != *player || !payment.waiting
            || payment.committing.is_some() || !payment.audit.as_ref()?.path.is_empty() || scope.path.is_empty() { return None; }
        Some(PendingManaPayment { index, payment, scope: &scope.path, trigger_source: at.object,
            trigger_ability: at.execution.as_ref()?.ability()? })
    }

    pub fn payment_mana_boundary(&self) -> Option<serde_json::Value> {
        let pending = self.pending_mana_payment()?;
        let payment = pending.payment;
        Some(serde_json::json!({"stage":"mana_choice","path":[],"kind":"spell",
            "player":payment.player,"source":payment.source.0,"scope":pending.scope,
            "trigger":self.audit_ability(Some(pending.trigger_source),&pending.trigger_ability),
            "pool":Self::audit_payment_pool(&self.players[payment.player].pool)}))
    }

    /// Printed indices for an ordinary retained casting cost, checked against
    /// the actual post-answer locked symbols. Alternative faces/costs need their
    /// own original announcement provenance and are not inferred here.
    pub fn pending_payment_symbol_indices(&self, symbols:&[String]) -> Option<Vec<usize>> {
        let source=self.pending_mana_payment()?.payment.source;
        let object=self.obj(source);
        if object.alternative.is_some() || object.cast_as!=crate::card::CastAs::Whole {return None;}
        self.payment_symbol_indices(source,None,object.cast_as,&[],object.x,symbols)
    }

    pub(crate) fn continue_mana_payment(&mut self, player:PlayerId, caller:PlayerId, source:ObjId, mana:Pool,
        adding:Vec<ir::mana::ManaSymbol>) -> crate::play::ActionResult<()> {
        let pending = self.pending_mana_payment().ok_or("no triggered-mana choice owns a pending root spell payment")?;
        if pending.payment.source != source || pending.payment.player != player {
            return Err("pending payment selector names another root or payer".into());
        }
        // Work on a disposable clone: even an offered answer must not commit if
        // its exact selector cannot pay the retained cost after production.
        let index = pending.index;
        let mut proposed = self.clone();
        let Procedure::Payment(payment) = &mut proposed.procedures[index] else { unreachable!("authenticated retained root") };
        payment.with = Some(mana);
        proposed.act(caller,crate::Move::Choose(crate::state::Answer::Mana(adding)))?;
        if matches!(proposed.decision,Some(crate::state::Decision::Payment { source: pending, .. }) if pending == source) {
            return Err("explicit pending payment selection does not pay its retained cost".into());
        }
        self.adopt(proposed);
        Ok(())
    }

    /// A fully validated mana spend whose remaining object commitment owns an
    /// explicit ordering decision. This is an unfinished announcement, not a cast
    /// or activation event. Quotes alone expose this retained boundary.
    pub fn payment_order_boundary(&self) -> Option<serde_json::Value> {
        if !matches!(self.decision, Some(Decision::Choose { choice: crate::state::Choice::OrderCards { .. }, .. })) { return None; }
        let payment = self.procedures.iter().rev().find_map(|procedure| match procedure {
            Procedure::Payment(payment) => Some(payment), _ => None,
        })?;
        if payment.waiting || !payment.steps.is_empty() || payment.committing.is_none()
            || !payment.audit.as_ref()?.path.is_empty() { return None; }
        // Only a spell has the retained casting announcement authenticated below.
        // Other unfinished payment forms do not acquire substitute evidence here.
        if !matches!(payment.finish,Finish::Spell) {return None;}
        let kind="spell";
        let mut boundary=serde_json::json!({"path":[],"player":payment.player,"source":payment.source.0,"kind":kind});
        if kind=="spell" {
            let selected=payment.cost.cast_payment(&payment.paying);
            boundary["substitutions"]=serde_json::json!({
                "convoke":selected.convoke.iter().map(|(object,color)|serde_json::json!({"object":object.0,"color":color.map(|c| match c {ir::Color::White=>"W",ir::Color::Blue=>"U",ir::Color::Black=>"B",ir::Color::Red=>"R",ir::Color::Green=>"G"})})).collect::<Vec<_>>(),
                "delve":selected.delve.iter().map(|object|object.0).collect::<Vec<_>>()});
        }
        Some(boundary)
    }
    pub(crate) fn advance_payment(&mut self) -> crate::play::ActionResult<()> {
        let Some(Procedure::Payment(mut payment)) = self.procedures.pop() else { unreachable!("payment owns its progress") };
        let parent = self.procedures.len();
        payment.audit_scope(self);
        let result=payment.advance(self);
        self.payment_scope=None;
        match result {
            Ok(true) => if payment.reports { self.deliver_payment(true); },
            Ok(false) => self.procedures.insert(parent, Procedure::Payment(payment)),
            Err(crate::play::ActionError::Refused(reason)) if payment.interactive => {
                self.decision = Some(Decision::Payment { player: payment.player, source: payment.source, reason });
                self.procedures.push(Procedure::Payment(payment));
            },
            Err(reason) => return Err(reason),
        }
        Ok(())
    }
    pub(crate) fn expose_payments(&mut self) {
        if self.decision.is_some() {
            for procedure in &mut self.procedures {
                if let Procedure::Payment(payment) = procedure { payment.interactive = true; }
            }
        }
    }
    /// Correct only the unfinished payment; announced targets and committed mana stay put.
    pub(crate) fn finish_payment(&mut self, plan: Vec<Step>, paying: Vec<Vec<ObjId>>, with: Option<Pool>) -> crate::play::ActionResult<()> {
        let mut proposed = self.clone();
        let Some(Procedure::Payment(payment)) = proposed.procedures.last_mut() else { unreachable!("payment question owns payment") };
        payment.steps = plan.into();
        payment.paying = paying;
        payment.with = with;
        // Invalid corrections before a new question change nothing, like other answers.
        payment.interactive = false;
        proposed.decision = None;
        proposed.continue_execution()?;
        proposed.expose_payments();
        self.adopt(proposed);
        Ok(())
    }
}

impl Game {
    /// Reverse the unpaid announcement while retaining legal mana abilities (733.1).
    /// Costs of the outer action have not committed. This is not a zone-change action,
    /// and does not reuse any object handle or erase events already delivered to players.
    pub(crate) fn reverse_payment(&mut self) -> crate::play::ActionResult<()> {
        let Some(Procedure::Payment(payment)) = self.procedures.pop() else { unreachable!("payment owns its reversal") };
        if matches!(payment.undo, Undo::Effect) {
            // Nothing was announced. The effect goes on unpaid; mana already produced stays
            // in its player's pool, and no player receives priority (118.12, 733.1).
            self.decision = None;
            self.deliver_payment(false);
            self.go_on()?;
            return Ok(());
        }
        // Triggers append while payment is in progress; none is placed before it ends.
        // Remove exactly this announcement's occurrences, not equal-looking mana triggers.
        self.triggered.drain(payment.triggers);
        let restored = match payment.undo {
            Undo::Companion | Undo::TurnFaceUp | Undo::Unlock => None,
            Undo::ExiledFromHand { original, object, position } => {
                self.exile.retain(|id| *id != payment.source);
                self.obj_mut(payment.source).zone = crate::Zone::Gone;
                *self.obj_mut(original) = *object;
                let hand = &mut self.players[payment.player].hand;
                hand.insert(position.min(hand.len()), original);
                Some(original)
            },
            Undo::Spell { original, object, position } => {
                self.ability_gone(payment.source);
                let (owner, zone) = (object.owner, object.zone);
                *self.obj_mut(original) = *object;
                let list = match zone {
                    crate::Zone::Library => &mut self.players[owner].library,
                    crate::Zone::Hand => &mut self.players[owner].hand,
                    crate::Zone::Graveyard => &mut self.players[owner].graveyard,
                    crate::Zone::Exile => &mut self.exile,
                    crate::Zone::Command => &mut self.command,
                    _ => unreachable!("casting currently begins in hand, graveyard or exile"),
                };
                list.insert(position.min(list.len()), original);
                Some(original)
            },
            Undo::Ability { reservation, started, lifetime } => {
                if let Finish::Ability { on_stack: Some(ability), .. } = payment.finish { self.ability_gone(ability); }
                let (source,key)=&self.activated[reservation];
                if let Some(index)=self.active_mana_abilities.iter().position(|(o,k)|o==source && k==key) {self.active_mana_abilities.remove(index);}
                self.activated.remove(reservation);
                self.activation_started.remove(started);
                if let Some(position) = lifetime { self.obj_mut(payment.source).activation_uses.remove(position); }
                None
            },
            Undo::Effect => unreachable!("declined above"),
        };
        self.log.push(Event::AnnouncementReversed { source: payment.source, restored, player: payment.player });
        self.decision = None;
        if payment.reports {
            // The effect that offered the cast goes on; no player receives priority (608.2g).
            self.deliver_payment(false);
            self.go_on()?;
            return Ok(());
        }
        self.priority_from(payment.player);
        // A parent payment may now need a different mana step; it must not replay one.
        self.go_on()?;
        Ok(())
    }
}
