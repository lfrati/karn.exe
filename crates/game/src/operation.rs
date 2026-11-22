//! Proposed events and replacement ordering (614–616). Simultaneous batches collect choices
//! before committing; sequential draw operations finish each event before proposing the next.
pub use crate::creation::{Proposal as TokenProposal,ProposedToken};

pub use crate::damage::{Offer as DamageOffer, ProposedHit, DamageOption, valid_allocation};
use crate::entry::{EntryEvent, EntryRequest};
use crate::replace::{CounterChange, CounterScope, EntryPayments, Replacement};
use crate::select::Ctx;
use crate::event::Target;
use crate::state::{Answer, Choice, Decision, Game, ObjId, PlayerId, PublicChoice};

/// One replacement instance: exact source incarnation and its ability or counter-rule owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementId { pub source: ObjId, pub effect: ReplacementEffect }

/// The effect's semantic owner. Counter rules are not abilities (122.1h).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementEffect {
    Ability { key: crate::binding::AbilityKey, part: usize },
    Finality,
}
impl ReplacementId {
    pub fn from_ability(source: ObjId, key: crate::binding::AbilityKey, part: usize) -> Self {
        Self { source, effect: ReplacementEffect::Ability { key, part } }
    }
    pub fn ability(&self) -> Option<&crate::binding::AbilityKey> {
        match &self.effect { ReplacementEffect::Ability { key, .. } => Some(key), ReplacementEffect::Finality => None }
    }
    pub fn part(&self) -> Option<usize> {
        match self.effect { ReplacementEffect::Ability { part, .. } => Some(part), ReplacementEffect::Finality => None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement { pub on: Target, pub kind: String, pub count: u32, pub by: PlayerId }

/// The proposed counter instruction shown while its replacement order is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterProposal { pub by: PlayerId, pub inventory: Vec<(String,u32)> }

/// A frozen placement batch. Only preparation can construct it; commitment never queries
/// rules or offers choices (122.6, 616.1).
#[derive(Debug, Clone)]
// Instruction → recipient inventory → counter kinds. Written instructions remain
// sequential; recipients within one instruction receive their counters simultaneously.
pub(crate) struct Prepared(Vec<Vec<Vec<Placement>>>);
impl Prepared {
    pub(crate) fn check(&self, game: &Game) -> crate::Res<()> {
        // Preflight the complete commitment, including separate instructions sharing a
        // holder. No stamp, counter, or event is changed until every final total fits.
        let mut totals = std::collections::HashMap::new();
        for placed in self.0.iter().flatten().flatten() {
            let key = (placed.on, placed.kind.clone());
            let total = totals.entry(key).or_insert_with(|| game.counters(placed.on).get(&placed.kind).copied().unwrap_or(0));
            *total = counter_add(*total, placed.count)?;
        }
        Ok(())
    }
    pub(crate) fn commit(self, game: &mut Game) -> crate::Res<Vec<Placement>> {
        self.check(game)?;
        let placements = self.0.iter().flatten().flatten().cloned().collect();
        for instruction in self.0 {
            Installed::install(game, vec![instruction]).record(game);
        }
        Ok(placements)
    }

    /// Damage owns a larger simultaneous event than its counter inventories. Install
    /// frozen results now, and let that owner record after its other results commit.
    /// The caller must check before mutating any results; installation cannot suspend.
    pub(crate) fn install(self, game: &mut Game) -> Installed {
        Installed::install(game, self.0)
    }
}

/// Installed inventories awaiting their owning event's completion. No queries,
/// replacements or trigger/history events occur during installation.
pub(crate) struct Installed(Vec<Vec<Vec<(Placement, u32)>>>);
impl Installed {
    fn install(game: &mut Game, instructions: Vec<Vec<Vec<Placement>>>) -> Self {
        // What these instructions put on objects is put on at once (613.7c, 613.7m).
        let first = game.clock + 1;
        let installed = Self(instructions.into_iter().map(|instruction| {
            instruction.into_iter().map(|batch| {
                batch.into_iter().filter(|placed| placed.count != 0).map(|placed| {
                    if let Target::Object(object) = placed.on {
                        let stamp = game.tick();
                        game.counter_fields_mut(object).1.insert(placed.kind.clone(), stamp);
                    }
                    let counters = game.counters_mut(placed.on);
                    let before = counters.get(&placed.kind).copied().unwrap_or(0);
                    *counters.entry(placed.kind.clone()).or_default() += placed.count;
                    (placed, before)
                }).collect()
            }).collect()
        }).collect());
        game.stamped_at_once(first);
        installed
    }

    pub(crate) fn record(self, game: &mut Game) {
        for inventory in self.0.into_iter().flatten() {
            let from = game.log.end();
            for (placed, before) in inventory {
                let recipient = match placed.on {
                    Target::Object(on) => Some(Box::new(game.seen(on, None))),
                    Target::Player(_) => None,
                };
                game.event_log_mut().push(crate::Event::CountersPut {
                    on: placed.on, kind: placed.kind, count: placed.count, before, by: placed.by, recipient,
                });
            }
            let end = game.log.end();
            if end != from { game.together.push((from, end)); }
        }
    }
}

pub(crate) fn counter_add(left: u32, right: u32) -> crate::Res<u32> {
    left.checked_add(right).ok_or_else(|| crate::Unsupported("counter amount exceeds the engine numerical range".into()))
}

/// Counter totals frozen before entry. Installation is only on the fresh incarnation,
/// whose counter inventory is empty; no addition is left to perform at commitment.
#[derive(Debug, Clone)]
pub(crate) struct EntryInventory(Vec<(String, u32)>);
impl EntryInventory {
    pub(crate) fn new(counters: &[(String,u32)]) -> crate::Res<Self> {
        let mut totals = std::collections::BTreeMap::new();
        for (kind,count) in counters {
            let total = totals.entry(kind.clone()).or_default();
            *total = counter_add(*total,*count)?;
        }
        Ok(Self(totals.into_iter().collect()))
    }
    pub(crate) fn install(&self, game: &mut Game, object: ObjId) {
        for (kind,count) in &self.0 {
            if *count == 0 { continue; }
            let stamp = game.tick();
            let (counters, stamps) = game.counter_fields_mut(object);
            counters.insert(kind.clone(),*count);
            stamps.insert(kind.clone(),stamp);
        }
    }
    /// Entry counters are installed with each entrant, but their recipient information
    /// belongs to the completed simultaneous entry event (603.10, 608.2i). The entry
    /// operation records them only after all entries and departures have committed.
    pub(crate) fn record(self, game: &mut Game, object: ObjId, by: PlayerId) {
        let from = game.log.end();
        for (kind,count) in self.0 {
            if count == 0 { continue; }
            let recipient = Some(Box::new(game.seen(object, None)));
            game.event_log_mut().push(crate::Event::CountersPut { on: Target::Object(object), kind, count, before:0, by, recipient });
        }
        let end = game.log.end();
        if end != from { game.together.push((from, end)); }
    }
}

/// Retained results of one operation, delivered exactly once to its parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Completed {
    /// Original identities successfully countered by this instruction (701.6a), including
    /// abilities that ceased to exist. Empty when no counter succeeded.
    pub countered: Vec<ObjId>,
    pub arrived: Vec<ObjId>,
    pub transitions: Vec<(ObjId, ObjId)>,
    pub destroyed: Vec<crate::binding::Destruction>,
    pub removed: Vec<crate::binding::Removal>,
    pub damage: Vec<crate::damage::CompletedDamage>,
    /// The life the players it had lose life lost, in all (119.3).
    pub lost: u32,
    /// Each card it discarded into exile by a madness replacement, as it is now, with that
    /// madness ability (702.35a).
    pub madness: Vec<(ObjId, crate::binding::AbilityKey)>,
    /// Each object its departures moved, as it was and as it is now, into a hidden zone too:
    /// a card revealed and shuffled into its library instead (614.6).
    pub moved: Vec<(ObjId, ObjId)>,
}
impl Completed {
    /// What it moved, and what entered the battlefield, each once.
    pub(crate) fn changed(&self) -> impl Iterator<Item = (ObjId, ObjId)> + '_ {
        self.transitions.iter().copied().chain(self.moved.iter().copied().filter(|m| !self.transitions.contains(m)))
    }
}

/// Event-specific applicability and modification; ordering and once-only application are
/// shared by every event, including effects that become applicable later (616.2).
pub(crate) trait Replaced: Clone + std::fmt::Debug {
    type Change;
    fn affected(&self) -> Target;
    fn controller(&self, game: &Game) -> PlayerId;
    fn question(&self) -> (String, u32);
    fn counters(&self, _game: &Game) -> Option<CounterProposal> { None }
    fn tokens(&self,_game:&Game)->Option<TokenProposal> {None}
    fn applicable(&mut self, game: &mut Game, applied: &[ReplacementId]) -> crate::Res<Vec<(ReplacementId, Self::Change)>>;
    fn apply(&mut self, change: Self::Change) -> crate::Res<()>;
}

#[derive(Debug, Clone)]
pub(crate) struct Ordering<E> {
    events: Vec<E>,
    applied: Vec<Vec<ReplacementId>>,
    order: Vec<usize>,
    cursor: usize,
    asked: Option<Vec<ReplacementId>>,
    pub(crate) choices: Vec<PublicChoice>,
}

impl<E: Replaced> Ordering<E> {
    pub(crate) fn waiting(&self) -> bool { self.asked.is_some() }
    pub(crate) fn new(game: &Game, events: Vec<E>) -> Self {
        let mut order: Vec<usize> = (0..events.len()).collect();
        order.sort_by_key(|i| (events[*i].controller(game) + game.players.len() - game.active) % game.players.len());
        let applied = vec![Vec::new(); events.len()];
        Self { events, applied, order, cursor: 0, asked: None, choices: Vec::new() }
    }

    pub(crate) fn advance(&mut self, game: &mut Game) -> crate::Res<Option<Decision>> {
        while let Some(&at) = self.order.get(self.cursor) {
            let event = &mut self.events[at];
            let mut applicable = event.applicable(game, &self.applied[at])?;
            match applicable.len() {
                0 => self.cursor += 1,
                1 => {
                    let (id, change) = applicable.pop().unwrap();
                    event.apply(change)?;
                    self.applied[at].push(id);
                }
                _ => {
                    let among: Vec<_> = applicable.into_iter().map(|(id, _)| id).collect();
                    self.asked = Some(among.clone());
                    let (kind, count) = event.question();
                    return Ok(Some(Decision::Choose { player: event.controller(game),
                        choice: Choice::Replacement { on: event.affected(), kind, count, counters: event.counters(game), tokens:event.tokens(game), among, optional: false } }));
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn answer(&mut self, game: &mut Game, player: PlayerId, answer: Answer) -> crate::Res<Option<Decision>> {
        let (Some(among), Answer::Replacement(chosen)) = (self.asked.take(), answer) else {
            unreachable!("the operation's answer was validated")
        };
        assert!(among.contains(&chosen));
        let at = self.order[self.cursor];
        let event = &mut self.events[at];
        let change = event.applicable(game, &self.applied[at])?.into_iter()
            .find(|(id, _)| *id == chosen).expect("an offered effect still applies").1;
        self.choices.push(PublicChoice::Replacement { player, on: event.affected(), effect: chosen.clone() });
        event.apply(change)?;
        self.applied[at].push(chosen);
        self.advance(game)
    }
}

/// The original placement instruction, independent of any outer resolution frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventOrigin { Effect, AnnouncementCost, TurnBased }

/// One inventory instruction affecting one holder. All its kinds share 614.5 history.
#[derive(Debug, Clone)]
pub(crate) struct CounterEvent {
    on: Target, by: PlayerId, inventory: Vec<(String,u32)>, origin: EventOrigin,
    modified: bool,
    instruction: usize,
}
impl CounterEvent {
    pub(crate) fn new(placement: Placement, origin: EventOrigin) -> Self {
        Self { on: placement.on, by: placement.by, inventory: vec![(placement.kind,placement.count)], origin, modified:false, instruction:0 }
    }
    pub(crate) fn batch(placements: Vec<Placement>, origin: EventOrigin) -> crate::Res<Vec<Self>> {
        let mut events: Vec<Self> = Vec::new();
        for p in placements {
            if let Some(event)=events.iter_mut().find(|e|e.on==p.on && e.by==p.by) {
                if let Some((_,n))=event.inventory.iter_mut().find(|(k,_)|*k==p.kind) {
                    *n=counter_add(*n,p.count)?; }
                else {event.inventory.push((p.kind,p.count));}
            } else { events.push(Self::new(p,origin)); }
        }
        Ok(events)
    }
}
impl Ordering<CounterEvent> {
    pub(crate) fn prepare_counters(self,game:&Game)->Prepared {
        let mut instructions = std::collections::BTreeMap::<usize, Vec<Placement>>::new();
        for event in self.events {
            instructions.entry(event.instruction).or_default().extend(event.inventory.into_iter()
                .map(|(kind,count)| Placement { on:event.on, by:event.by, kind, count }));
        }
        Prepared(instructions.into_values().flat_map(|placements| game.prepare_counters(placements).0).collect())
    }
}
impl Replaced for CounterEvent {
    type Change = (CounterScope,CounterChange);
    fn affected(&self) -> Target { self.on }
    fn controller(&self, game: &Game) -> PlayerId { match self.on { Target::Object(o) => game.controller(o), Target::Player(p) => p } }
    fn question(&self) -> (String, u32) {
        self.inventory.first().filter(|_|self.inventory.len()==1).cloned().unwrap_or_default()
    }
    fn counters(&self, _game: &Game) -> Option<CounterProposal> {
        Some(CounterProposal { by:self.by, inventory:self.inventory.iter().filter(|(_,n)|*n>0).cloned().collect() })
    }
    fn applicable(&mut self, game: &mut Game, applied: &[ReplacementId]) -> crate::Res<Vec<(ReplacementId, Self::Change)>> {
        for (kind,count) in &mut self.inventory {
            if !game.may_place(self.on,kind,self.by) {*count=0;}
        }
        Ok(game.counter_replacements(self.on,self.by,&self.inventory,self.origin==EventOrigin::Effect || self.modified,applied))
    }
    fn apply(&mut self, (kinds,change): Self::Change) -> crate::Res<()> {
        let inventory = self.inventory.iter().map(|(kind,count)| {
            let count = if *count>0 && kinds.includes(kind) {change.apply(*count)?} else {*count};
            Ok((kind.clone(),count))
        }).collect::<crate::Res<Vec<_>>>()?;
        self.inventory = inventory;
        // A replacement-produced placement can make an effect-qualified modifier apply (614.16).
        self.modified=true;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Operation {
    Create { ordering:Ordering<crate::creation::Event>,entered:bool,result:Option<Completed> },
    Depart(crate::departure::Batch),
    /// The successful counter's identities survive all destination/commander choices.
    Counter { departures: crate::departure::Batch, countered: Vec<ObjId> },
    Draw(crate::draw::Drawing),
    /// Retained damage proposals, then frozen results and counter placements (120.4b/c).
    Damage(crate::damage::Dealing),
    PutCounters { ordering: Ordering<CounterEvent> },
    Enter { ordering: Option<Ordering<EntryEvent>>, preparation: Option<crate::entry::prepare::Preparation>, departures: crate::departure::Batch, payments: EntryPayments },
    /// Where each discarded card goes, with the affected player's choice among the
    /// replacements of its discard (616.1); then they go there, as one entry operation.
    Discard { ordering: Ordering<crate::discard::Discarding>, entered: bool, result: Option<Completed> },
}
impl Operation {
    pub(crate) fn child_complete(&mut self, result: Completed) {
        match self {
            Self::Draw(drawing) => drawing.child_complete(result),
            Self::Create {result:completed,..} | Self::Discard {result:completed,..} => *completed=Some(result),
            _ => unreachable!("this operation does not start children"),
        }
    }
    pub(crate) fn entry_context(&self) -> Option<(ObjId, Option<ObjId>)> {
        let Self::Enter { ordering, preparation, .. } = self else { return None };
        if let Some(preparation) = preparation { return preparation.context().map(|(card, _)| (card, Some(card))); }
        let ordering = ordering.as_ref()?;
        let event = ordering.events.get(*ordering.order.get(ordering.cursor)?)?;
        Some((match event.affected() { Target::Object(o) => o, Target::Player(_) => unreachable!("entry affects an object") }, event.shown_entering()))
    }
    /// The question an entering permanent's own text asks as it enters (614.1c, 614.12a),
    /// while that is what this operation waits for.
    pub(crate) fn entry_asks(&self) -> Option<&crate::replace::Asks> {
        let Self::Enter { preparation: Some(preparation), .. } = self else { return None };
        preparation.asks()
    }
    pub(crate) fn waiting(&self) -> bool { match self {
        Self::Create {ordering,..} => ordering.waiting(),
        Self::Depart(batch) | Self::Counter { departures: batch, .. } => batch.waiting(),
        Self::Draw(drawing) => drawing.waiting(),
        Self::Damage(damage) => damage.waiting(),
        Self::PutCounters { ordering, .. } => ordering.asked.is_some(),
        Self::Enter { ordering, preparation, departures, .. } => preparation.as_ref().is_some_and(|p| p.waiting()) || ordering.as_ref().is_some_and(|o| o.asked.is_some()) || departures.waiting(),
        Self::Discard { ordering, .. } => ordering.asked.is_some(),
    } }
    pub(crate) fn public_choices(&self) -> impl Iterator<Item = &PublicChoice> {
        let primary: &[PublicChoice] = match self {
        Self::Create {ordering,..} => &ordering.choices,
        Self::Depart(batch) | Self::Counter { departures: batch, .. } => batch.choices(),
        Self::Draw(drawing) => &drawing.choices,
        Self::Damage(_) => &[],
        Self::PutCounters { ordering, .. } => &ordering.choices,
        Self::Enter { ordering, preparation, .. } => match preparation {
            Some(p) => &p.choices, None => &ordering.as_ref().expect("prepared entry ordering").choices,
        },
        Self::Discard { ordering, .. } => &ordering.choices,
        };
        let departures: &[PublicChoice] = match self { Self::Enter { departures, .. } => departures.choices(), _ => &[] };
        primary.iter().chain(departures.iter()).chain(match self { Self::Damage(d)=>Some(d.choices()),_=>None }.into_iter().flatten())
    }
}

impl Game {
    pub(crate) fn may_place(&self, on: Target, kind: &str, by: PlayerId) -> bool {
        self.holds_counters(on) && !self.forbidden(&crate::trigger::Fact::counter_placement(on,kind,by))
    }
    pub(crate) fn prepare_counters(&self, placements: Vec<Placement>) -> Prepared {
        self.prepare_counters_with(placements, |g, p| g.may_place(p.on, &p.kind, p.by))
    }
    fn prepare_counters_with(&self, mut placements: Vec<Placement>, legal: impl Fn(&Game, &Placement) -> bool) -> Prepared {
        for placed in &mut placements {
            if !legal(self, placed) { placed.count = 0; }
        }
        let mut batches: Vec<Vec<Placement>> = Vec::new();
        for placed in placements {
            if let Some(batch) = batches.iter_mut().find(|b| b[0].on == placed.on && b[0].by == placed.by) {
                batch.push(placed);
            } else { batches.push(vec![placed]); }
        }
        Prepared(vec![batches])
    }
    /// Already replaced entry counters, committed as one kind batch on the entered object.
    pub(crate) fn commit_entry_counters(&mut self, object: ObjId, counters: Vec<(String, u32)>, by: PlayerId) -> crate::Res<()> {
        let placements = counters.into_iter().map(|(kind, count)| Placement { on: Target::Object(object), kind, count, by }).collect();
        self.prepare_counters(placements).commit(self)?;
        Ok(())
    }

    pub(crate) fn counter_replacements(&self, on: impl Into<Target>, by: PlayerId, inventory: &[(String,u32)], effect: bool, applied: &[ReplacementId]) -> Vec<(ReplacementId, (CounterScope, CounterChange))> {
        let target = on.into();
        if !self.holds_counters(target) { return Vec::new(); }
        let mut found = Vec::new();
        for source in &self.battlefield {
            let ctx = Ctx::new(self.controller(*source), *source);
            for (key, replacement) in self.replacements(*source) {
                let id = ReplacementId::from_ability(*source, key.clone(), 0);
                if let Replacement::Counters { on, by: actor, effect_only, kinds, change } = replacement
                    && !applied.contains(&id) && (!effect_only || effect)
                    && inventory.iter().any(|(kind,count)|*count>0 && kinds.includes(kind))
                    && on.is(self, &ctx.clone().with_key(key.clone()), target)
                    && actor.as_ref().is_none_or(|actor|actor.is(self,&ctx.clone().with_key(key),Target::Player(by)))
                { found.push((id,(kinds.clone(),*change))); }
            }
        }
        found
    }

    pub(crate) fn token_replacements(&self,by:PlayerId,requests:&[crate::creation::Request],effect:bool,applied:&[ReplacementId])->Vec<(ReplacementId,(crate::replace::TokenScope,crate::replace::TokenChange))> {
        let mut found=Vec::new();
        for source in &self.battlefield {
            if self.controller(*source)!=by {continue;}
            for (key,replacement) in self.replacements(*source) {
                let id=ReplacementId::from_ability(*source,key,0);
                if let Replacement::Tokens {scope,change,effect_only}=replacement
                    && !applied.contains(&id) && (!effect_only || effect)
                    && requests.iter().any(|r|r.count>0 && scope.matches(&r.values)) {
                    found.push((id,(*scope,change.clone())));
                }
            }
        }
        found
    }
    pub(crate) fn create_tokens(&mut self,requests:Vec<crate::creation::Request>,origin:EventOrigin)->crate::Res<Result<Completed,Decision>> {
        let events=crate::creation::Event::batch(requests,origin);
        self.run_operation(Operation::Create {ordering:Ordering::new(self,events),entered:false,result:None})
    }

    pub(crate) fn put_counter_batch(&mut self, request: Vec<Placement>, origin: EventOrigin) -> crate::Res<Result<(), Decision>> {
        self.put_counter_instructions(vec![request],origin)
    }

    /// Preserve independent placement instructions while retaining all their proposed
    /// inventories until every order is chosen. Turning face up can supply several
    /// distinct replacement instructions (702.37b, 708.11), not one combined placement.
    pub(crate) fn put_counter_instructions(&mut self, instructions: Vec<Vec<Placement>>, origin: EventOrigin) -> crate::Res<Result<(), Decision>> {
        let events=instructions.into_iter().enumerate().map(|(instruction, request)| {
            let mut events = CounterEvent::batch(request,origin)?;
            for event in &mut events { event.instruction = instruction; }
            Ok(events)
        })
            .collect::<crate::Res<Vec<_>>>()?.into_iter().flatten().collect();
        let operation=Operation::PutCounters { ordering:Ordering::new(self,events) };
        Ok(self.run_operation(operation)?.map(|_|()))
    }

    /// Own preparation and replacement choices as one retained entry operation.
    pub(crate) fn prepare_entry_batch(&mut self, request: Vec<EntryRequest>, moves: Vec<(ObjId, crate::Zone)>) -> crate::Res<Result<Completed, Decision>> {
        let departures = crate::departure::Batch::new(self, moves.into_iter().map(|(object, zone)| (object, crate::departure::Kind::Move(zone))).collect());
        self.prepare_entry_departures(request, departures)
    }
    pub(crate) fn prepare_entry_departures(&mut self, request: Vec<EntryRequest>, departures: crate::departure::Batch) -> crate::Res<Result<Completed, Decision>> {
        let preparation = crate::entry::prepare::Preparation::new(self, request);
        self.run_operation(Operation::Enter { ordering: None, preparation: Some(preparation), departures, payments: EntryPayments::default() })
    }

    pub(crate) fn run_operation(&mut self, mut operation: Operation) -> crate::Res<Result<Completed, Decision>> {
        let parent = self.procedures.len();
        let decision = match &mut operation {
            Operation::Create {ordering,entered,result} => {
                if let Some(question)=ordering.advance(self)? {Some(question)}
                else if !*entered {
                    *entered=true;
                    let requests=ordering.events.iter().flat_map(crate::creation::Event::entries).collect();
                    match self.prepare_entry_batch(requests,Vec::new())? {
                        Ok(completed)=>{*result=Some(completed);None},Err(question)=>Some(question),
                    }
                } else {None}
            },
            Operation::Depart(batch) | Operation::Counter { departures: batch, .. } => batch.prepare(),
            Operation::Draw(drawing) => drawing.advance(self)?,
            Operation::Damage(damage) => damage.advance(self)?,
            Operation::PutCounters { ordering, .. } => ordering.advance(self)?,
            Operation::Enter { ordering, preparation, payments, departures, .. } => {
                if let Some(p) = preparation {
                    if let Some(decision) = p.advance(self)? { self.push_operation(operation); return Ok(Err(decision)); }
                    let events = p.eligible(self).into_iter().map(|r| EntryEvent::new(self, r)).collect::<crate::Res<Vec<_>>>()?;
                    let mut ordered = Ordering::new(self, events);
                    ordered.choices = p.choices.clone();
                    *payments = p.payments.clone();
                    *ordering = Some(ordered);
                    *preparation = None;
                }
                ordering.as_mut().expect("prepared entry").advance(self)?.or_else(|| departures.prepare())
            },
            Operation::Discard { ordering, entered, result } => {
                if let Some(question) = ordering.advance(self)? { Some(question) }
                else if !*entered {
                    *entered = true;
                    let (entries, departures) = crate::discard::destinations(self, &ordering.events);
                    match self.prepare_entry_departures(entries, departures)? {
                        Ok(completed) => { *result = Some(completed); None },
                        Err(question) => Some(question),
                    }
                } else { None }
            },
        };
        if let Some(decision) = decision {
            // A child may already be retained above this operation's destination.
            self.procedures.insert(parent, crate::procedure::Procedure::Operation(operation));
            return Ok(Err(decision));
        }
        Ok(match operation {
            Operation::Create {result,..} => Ok(result.expect("creation entry child completes once")),
            Operation::Depart(batch) => Ok(batch.commit(self)),
            Operation::Counter { departures, countered } => {
                let mut result = departures.commit(self);
                result.countered = countered;
                Ok(result)
            },
            Operation::Draw(_) => Ok(Completed::default()),
            Operation::Damage(damage) => Ok(Completed { damage: damage.commit(self)?, ..Completed::default() }),
            Operation::PutCounters { ordering, .. } => {
                ordering.prepare_counters(self).commit(self)?;
                Ok(Completed::default())
            }
            Operation::Enter { ordering, payments, departures, .. } => {
                let events = ordering.expect("prepared entry").events.into_iter()
                    .map(|event| event.freeze(self)).collect::<crate::Res<Vec<_>>>()?;
                payments.pay(self)?;
                let mut result = Completed::default();
                let mut counters = Vec::new();
                let committing = self.clock + 1;
                for event in events {
                    let was = event.was();
                    if let Some((now, inventory)) = event.commit(self) {
                        result.arrived.push(now);
                        if let Some(was) = was { result.transitions.push((was, now)); }
                        counters.push((now, inventory));
                    }
                }
                let departed = departures.commit(self);
                result.transitions.extend(departed.transitions);
                result.madness.extend(departed.madness);
                result.moved.extend(departed.moved);
                // Each entrant's timestamp was reserved as it was prepared; they, the
                // departures and the entry counters are all given at once (613.7m).
                let first = result.arrived.iter().map(|now| self.obj(*now).stamp).fold(committing, u32::min);
                self.stamped_at_once(first);
                // Existing entry effects do not specify a placer (122.6a). Read the
                // default controller in the completed event, just like characteristics.
                for (now, inventory) in counters {
                    inventory.record(self, now, self.controller(now));
                }
                Ok(result)
            }
            // each where its replacements took it, all at once (701.9a)
            Operation::Discard { ordering, result, .. } => {
                let completed = result.expect("discard entry child completes once");
                crate::discard::record(self, &ordering.events, &completed);
                Ok(completed)
            }
        })
    }

    /// Damage and its result replacements form one operation: no partial child commitment.
    pub(crate) fn damage_operation(&mut self, damage: &[(ObjId, Target, u32)], combat: bool, origin: EventOrigin) -> crate::Res<Result<Completed, Decision>> {
        self.run_operation(Operation::Damage(crate::damage::Dealing::new(self,damage,combat,origin)?))
    }

    /// Discard these cards, asking the affected player which replacement applies where
    /// more than one would (616.1).
    pub(crate) fn discard_operation(&mut self, events: Vec<crate::discard::Discarding>) -> crate::Res<Result<Completed, Decision>> {
        let operation = Operation::Discard { ordering: Ordering::new(self, events), entered: false, result: None };
        self.run_operation(operation)
    }

    /// Advance a retained operation without replay; its next child may itself suspend.
    pub(crate) fn complete_operation(&mut self) -> crate::Res<Result<Completed, Decision>> {
        let operation = self.take_operation();
        self.run_operation(operation)
    }

    pub(crate) fn operation_asks(&self) -> bool { self.operation().is_some_and(Operation::waiting) }
    pub(crate) fn answer_operation(&mut self, player: PlayerId, answer: Answer) -> crate::Res<()> {
        let mut operation = self.take_operation();
        self.decision = match &mut operation {
            Operation::Create {ordering,..} => ordering.answer(self,player,answer)?,
            Operation::Depart(batch) | Operation::Counter { departures: batch, .. } => { batch.answer(answer); batch.prepare() },
            Operation::Draw(drawing) => { drawing.answer(self, answer)?; None },
            Operation::Damage(damage) => damage.answer(self,player,answer)?,
            Operation::PutCounters { ordering, .. } => ordering.answer(self, player, answer)?,
            Operation::Enter { ordering, preparation, departures, .. } => {
                if let Some(p) = preparation { p.answer(self, answer); p.advance(self)? }
                else if ordering.as_ref().is_some_and(|o| o.asked.is_some()) { ordering.as_mut().unwrap().answer(self, player, answer)? }
                else { departures.answer(answer); departures.prepare() }
            },
            Operation::Discard { ordering, .. } => ordering.answer(self, player, answer)?,
        };
        if self.decision.is_none() {
            if let Operation::Enter { ordering, preparation: Some(p), payments, .. } = &mut operation {
                let events = p.eligible(self).into_iter().map(|r| EntryEvent::new(self, r)).collect::<crate::Res<Vec<_>>>()?;
                let mut ordered = Ordering::new(self, events);
                ordered.choices = p.choices.clone();
                *payments = p.payments.clone();
                self.decision = ordered.advance(self)?;
                *ordering = Some(ordered);
                if let Operation::Enter { preparation, .. } = &mut operation { *preparation = None; }
            }
        }
        self.push_operation(operation);
        Ok(())
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::binding::{AbilityKey, Grant, Origin};

    #[test]
    fn r614_5_applied_replacements_distinguish_grants_and_components() {
        let def = crate::CardDef::new("Source", "{0}", "Artifact", "{T}: Add {G}.", None).unwrap();
        let what = crate::ability::template(&def.abilities()[0]).unwrap();
        let mut game = Game::new(vec![def], 2, 20);
        let source = game.put(0, 0, crate::Zone::Battlefield);
        let own = ReplacementId::from_ability(source, AbilityKey::printed(0), 0);
        let given = ReplacementId::from_ability(source, AbilityKey { index: 0,
            origin: Origin::Given { what, grant: Grant { by: Some(source), stamp: 1, part: 0 } } }, 0);
        let another = ReplacementId::from_ability(source, AbilityKey { index: 0,
            origin: Origin::Given { what, grant: Grant { by: Some(source), stamp: 1, part: 1 } } }, 0);
        let applied = vec![own, given.clone()];
        assert!(!applied.contains(&another), "another giving of the same definition is another effect");
        assert!(!applied.contains(&ReplacementId::from_ability(source, given.ability().unwrap().clone(), 1)));
        let cloned = applied.clone();
        assert!(cloned.contains(&given), "the same effect cannot apply again after cloning");
    }
}

#[cfg(test)]
mod prepared_tests {
    use super::*;
    #[test]
    fn r122_6_all_placement_legality_precedes_the_first_write() {
        let def = crate::CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap();
        let mut game = Game::new(vec![def], 2, 20);
        let bear = game.put(0, 0, crate::Zone::Battlefield);
        let placements = [1,2].into_iter().map(|count| Placement { on: bear.into(), kind: "+1/+1".into(), count, by: 0 }).collect();
        let reads = std::cell::Cell::new(0);
        let prepared = game.prepare_counters_with(placements, |g, _| {
            reads.set(reads.get()+1);
            g.obj(bear).counters.is_empty()
        });
        assert_eq!(reads.get(), 2);
        assert!(game.obj(bear).counters.is_empty());
        let placed = prepared.commit(&mut game).unwrap();
        assert_eq!(placed.iter().map(|p|p.count).collect::<Vec<_>>(), vec![1,2]);
        assert_eq!(game.obj(bear).counters["+1/+1"], 3);
    }
}

#[cfg(test)]
mod counter_arithmetic_reproduction {
    use super::*;
    /// Representation overflow is refused; u32::MAX is not a Magic rules limit (107.1b).
    #[test]
    fn r107_1b_inventory_merge_does_not_silently_clamp_the_rule_result() {
        let request=vec![Placement{on:Target::Player(0),kind:"energy".into(),count:2_147_483_648,by:0};2];
        assert!(CounterEvent::batch(request,EventOrigin::Effect).unwrap_err().0.contains("engine numerical range"));
    }
}

#[cfg(test)]
mod checked_inventory_tests {
    use super::*;
    #[test]
    fn r107_1b_preflight_covers_separate_events_before_any_commit() {
        let mut game=Game::new(Vec::new(),2,20);
        let before=format!("{game:?}");
        let placements=[1,u32::MAX].into_iter().map(|count|Placement{on:Target::Player(0),kind:"energy".into(),count,by:0}).collect();
        assert_eq!(game.prepare_counters(placements).commit(&mut game).unwrap_err().0,
            "counter amount exceeds the engine numerical range");
        assert_eq!(format!("{game:?}"),before);
    }
    #[test]
    fn r107_1b_mixed_kind_inventory_has_no_semantic_aggregate_limit() {
        let mut game=Game::new(Vec::new(),2,20);
        let placements=["energy","experience"].into_iter().map(|kind|Placement{on:Target::Player(0),kind:kind.into(),count:u32::MAX,by:0}).collect();
        let events=CounterEvent::batch(placements,EventOrigin::Effect).unwrap();
        assert_eq!(events[0].question(),("".into(),0));
        assert_eq!(events[0].counters(&game).unwrap().inventory.len(),2);
        Ordering::new(&game,events).prepare_counters(&game).commit(&mut game).unwrap();
        for kind in ["energy","experience"]{assert_eq!(game.counters(Target::Player(0))[kind],u32::MAX);}
    }
}

#[cfg(test)]
mod entry_placement_attribution {
    use super::*;

    #[test]
    fn r122_6a_frozen_entry_installer_preserves_an_explicit_placer() {
        // CR122.6a: an entry effect may explicitly specify a player. The installer
        // must preserve that decision, rather than replacing it with the controller.
        // Current lowered arrivals don't specify one; their callers pass the default.
        let def = crate::CardDef::new("Entrant", "{0}", "Creature", "", Some((2,2))).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let on = g.put(0,0,crate::Zone::Battlefield);
        let inventory = EntryInventory::new(&[("charge".into(),2)]).unwrap();
        inventory.install(&mut g,on);
        inventory.record(&mut g,on,1);
        assert!(matches!(g.log.last(), Some(crate::Event::CountersPut { by:1, recipient:Some(seen), .. }) if seen.controller==0));
    }
}

#[cfg(test)]
mod instruction_snapshot_tests {
    use super::*;

    #[test]
    fn r608_2c_retained_instructions_keep_their_own_post_event_state() {
        // CR608.2c/603.10: retained replacement choices must not turn two
        // independent instructions into one simultaneous counter event.
        let defs = vec![
            crate::CardDef::new("Elf", "{0}", "Creature — Elf", "", Some((2,2))).unwrap(),
            crate::CardDef::new("Lord", "{0}", "Creature — Elf", "Other Elf creatures you control get +1/+1 for each +1/+1 counter on this creature.", Some((2,2))).unwrap(),
        ];
        for def in &defs { crate::play::check(def).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        let elf = game.put(0, 0, crate::Zone::Battlefield);
        let lord = game.put(0, 1, crate::Zone::Battlefield);
        let placement = |on| Placement { on: Target::Object(on), kind:"+1/+1".into(), count:1, by:0 };
        game.put_counter_instructions(vec![vec![placement(elf)], vec![placement(lord)]], EventOrigin::Effect).unwrap().unwrap();
        let snapshot = game.log.iter().find_map(|e| match e {
            crate::Event::CountersPut { on:Target::Object(on), recipient:Some(seen), .. } if *on == elf => Some(seen.pt),
            _ => None,
        }).unwrap();
        assert_eq!(snapshot, Some((3,3)));
        assert_eq!(game.seen(elf,None).pt, Some((4,4)));

        // CR107.1b: implementation-range refusal preflights every retained
        // instruction before any write, including snapshots and timestamps.
        let before = format!("{game:?}");
        let mut overflow = placement(lord);
        overflow.count = u32::MAX;
        assert_eq!(game.put_counter_instructions(vec![vec![placement(elf)],vec![overflow]],EventOrigin::Effect).unwrap_err().0,
            "counter amount exceeds the engine numerical range");
        assert_eq!(format!("{game:?}"), before);
    }
}
