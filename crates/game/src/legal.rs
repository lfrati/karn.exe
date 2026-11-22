//! Pending rules questions and the domains used to validate player choices.

pub use crate::cost::Selection;
use crate::event::Target;
use crate::select::Ctx;
use crate::state::{Choice, Decision, Game, ObjId, PlayerId, Pool};
use ir::mana::{ManaCost, ManaSymbol};
use ir::Verb;

/// One "target" of a text: what could be chosen for it now, and how many it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub among: Vec<Target>,
    pub min: u32,
    pub max: u32,
    /// The chosen targets must differ from all choices in these earlier groups.
    pub distinct_from: Vec<usize>,
    /// Legal alternatives indexed by a single earlier target (601.2c).
    pub dependent: Option<DependentTargets>,
    /// The whole chosen group must fit one actual graveyard's candidate partition.
    /// Target-change offers also include unchanged illegal handles in each partition.
    pub same_graveyard: Option<Vec<Vec<Target>>>,
    /// Allowed two-creature pairs with distinct controllers (601.2c).
    pub controller_pairs: Option<Vec<[Target; 2]>>,
    /// At most one card from each mana-value partition. Retained illegal targets
    /// are outside these partitions and do not constrain new legal choices (115.7).
    pub different_mana_values: Option<Vec<Vec<Target>>>,
    /// The amount divided or distributed among the chosen targets, announced with them
    /// ([`crate::Move::WithDivision`]): each gets at least one (601.2d).
    pub divide: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependentTargets {
    pub group: usize,
    pub options: Vec<(Target, Vec<Target>)>,
}
impl Slot {
    pub fn allows_group(&self, group: &[Target]) -> bool {
        self.different_mana_values.as_ref().is_none_or(|partitions|
            partitions.iter().all(|partition| group.iter().filter(|target| partition.contains(target)).count() <= 1))
            && self.controller_pairs.as_ref().is_none_or(|pairs| pairs.iter().any(|pair|
            group.len() == 2 && group.contains(&pair[0]) && group.contains(&pair[1])))
            && (group.is_empty() || self.same_graveyard.as_ref().is_none_or(|partitions|
            partitions.iter().any(|partition| group.iter().all(|target| partition.contains(target)))))
    }
    pub fn allows(&self,target:Target,chosen:&[Vec<Target>])->bool {
        !self.distinct_from.iter().any(|i|chosen.get(*i).is_none_or(|group|group.contains(&target)))
            && self.dependent.as_ref().is_none_or(|d|chosen.get(d.group).is_some_and(|prior|
                d.options.iter().any(|(key,among)|prior.contains(key) && among.contains(&target))))
    }
}

/// A creature that could be declared as an attacker (508.1a): what it could attack, and
/// whether an effect says it attacks if able (508.1d), so that a declaration without it is
/// illegal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attacker {
    pub creature: ObjId,
    pub attacks: Vec<Target>,
    /// Fixed mana costs by exact recipient, cumulative over the selected attackers.
    pub costs: Vec<(Target, ManaCost)>,
    pub must: bool,
}

/// One mana ability of a plan: a permanent, which of its abilities, and for each part of its
/// cost that asks for a choice what it is paid with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Explicit mana window and announcements for this activation alone.
    pub plan: Vec<Step>,
    pub x: Option<u32>,
    pub life: Vec<bool>,
    pub source: ObjId,
    pub ability: usize,
    pub paying: Vec<Vec<ObjId>>,
    /// Exact mana spent on this activation's own cost; omission supplies none.
    pub with: Option<Pool>,
    /// The mana it adds, for each choice of mana its text offers ("{T}: Add {R} or {G}.").
    pub adding: Vec<Vec<ManaSymbol>>,
    /// For each choice among alternatives its cost has, which is paid, numbered from 0
    /// (601.2b, 602.2b): "{T}, Forage: Add two mana." exiles three cards or sacrifices a Food.
    pub chose: Vec<usize>,
    pub cost_branches: Option<Vec<crate::cost::BranchSelection>>,
}

/// Every way to take `n` of these.
fn combinations<T: Copy>(among: &[T], n: usize) -> Vec<Vec<T>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let Some((first, rest)) = among.split_first() else { return Vec::new() };
    let mut out: Vec<Vec<T>> = combinations(rest, n - 1).into_iter().map(|c| std::iter::once(*first).chain(c).collect()).collect();
    out.extend(combinations(rest, n));
    out
}

/// Witness that required target-change slots admit a jointly legal assignment (115.7).
/// Backtracking is necessary when an earlier broad group can consume a later narrow one.
pub(crate) fn target_selection(slots: &[Slot]) -> Option<Vec<Vec<Target>>> {
    if slots.iter().all(|s| s.distinct_from.is_empty() && s.dependent.is_none() && s.same_graveyard.is_none() && s.controller_pairs.is_none() && s.different_mana_values.is_none()) {
        return slots.iter().map(|s| (s.min <= s.max && s.min as usize <= s.among.len())
            .then(|| s.among.iter().take(s.min as usize).copied().collect())).collect();
    }
    let counts: Vec<_> = slots.iter().map(|s| s.min as usize).collect();
    let mut result = None;
    let mut budget = TargetWork { left: None, exhausted: false };
    walk_target_assignments(slots, &counts, &mut Vec::new(), &mut budget, &mut |chosen| {
        result = Some(chosen.to_vec()); false
    });
    result
}

struct TargetWork { left: Option<usize>, exhausted: bool }
fn target_work(budget: &mut TargetWork) -> bool {
    let Some(left) = &mut budget.left else { return true; };
    if *left == 0 { budget.exhausted = true; return false; }
    *left -= 1; true
}

/// Shared lazy group backtracking. Only the owning slot predicates decide legality;
/// partial choices need not themselves be valid complete announcements.
fn walk_target_assignments(slots: &[Slot], counts: &[usize], chosen: &mut Vec<Vec<Target>>,
                           budget: &mut TargetWork, emit: &mut impl FnMut(&[Vec<Target>]) -> bool) -> bool {
    let Some(slot) = slots.get(chosen.len()) else { return emit(chosen); };
    let count = counts[chosen.len()];
    if count < slot.min as usize || count > slot.max as usize
        || slot.distinct_from.iter().any(|i| *i >= chosen.len()) { return true; }
    fn group(slots: &[Slot], counts: &[usize], chosen: &mut Vec<Vec<Target>>, from: usize,
             members: &mut Vec<Target>, budget: &mut TargetWork, emit: &mut impl FnMut(&[Vec<Target>]) -> bool) -> bool {
        if !target_work(budget) { return false; }
        let slot = &slots[chosen.len()];
        if members.len() == counts[chosen.len()] {
            if !slot.allows_group(members) { return true; }
            chosen.push(members.clone());
            let go = walk_target_assignments(slots,counts,chosen,budget,emit);
            chosen.pop(); return go;
        }
        for index in from..slot.among.len() {
            if !target_work(budget) { return false; }
            let target = slot.among[index];
            if members.contains(&target) || !slot.allows(target,chosen) { continue; }
            members.push(target);
            let go = group(slots,counts,chosen,index+1,members,budget,emit);
            members.pop(); if !go { return false; }
        }
        true
    }
    group(slots,counts,chosen,0,&mut Vec::new(),budget,emit)
}

/// What the game waits for: whose move it is and what that player may do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    CoinCall,
    LibraryDestination { cards: Vec<ObjId>, positions: Vec<ir::action::LibraryPosition> },
    SpellDestination { on: ObjId, among: Vec<crate::state::DestinationEffect> },
    /// Which of these skip effects replaces the step or phase that would begin (616.1); the
    /// others wait for a later one. Nothing of it has begun.
    Skip { what: crate::turn::Occurrence, among: Vec<crate::turn::Skip> },
    CommandZone { object: ObjId },
    Retarget { copy: ObjId, original: Vec<Vec<Target>>, slots: Vec<Slot> },
    ChangeTargets { object: ObjId, original: Vec<Vec<Target>>, slots: Vec<Slot>, policy: crate::state::TargetPolicy },
    Alternative { among: Vec<crate::state::ActionOption>, optional: bool },
    /// Cast one of these cards now, as the effect being applied offers (608.2g), or decline
    /// with [`crate::Answer::No`]. This does not grant priority.
    Cast { cards: Vec<ObjId> },
    /// Correct an unfinished casting payment; this does not grant priority.
    Payment { source: ObjId, reason: String },
    /// Pay this effect's cost with explicit [`crate::Answer::Pay`] instructions, or decline
    /// (118.12). `waterbend` gives the eligible objects, never a selected payment.
    Pay { cost: ManaCost, life: u32, waterbend: Option<crate::state::Waterbend> },
    Search { cards: Vec<ObjId>, among: Vec<ObjId>, min: usize, max: usize },
    /// Select objects without targeting or revealing a hidden zone.
    Select { among: Vec<ObjId>, min: usize, max: usize },
    Attach { aura: ObjId, among: Vec<Target> },
    Holders { verb: Verb, among: Vec<Target> },
    Defender { among: Vec<Target>, nth: usize, of: usize },
    /// The player has priority (117.1); submitted moves are checked when made.
    Priority,
    /// Choose the targets of a triggered ability of this object as it goes on the stack
    /// (603.3d).
    Targets { source: Option<ObjId>, slots: Vec<Slot> },
    /// Choose the modes of a modal triggered ability of this object as it goes on the stack,
    /// and their targets (603.3c, 700.2b–c): one of these choices of modes, each with what
    /// its "target"s could be.
    Modes { source: Option<ObjId>, options: Vec<(Vec<usize>, Vec<Slot>)> },
    /// Put these abilities that triggered on the stack in any order (603.3b): each the object
    /// whose ability it is and its retained definition/instance identity.
    Order(Vec<(Option<ObjId>, crate::binding::AbilityKey)>),
    /// Discard down to the maximum hand size (514.1): which `count` of these.
    Discard { among: Vec<ObjId>, count: usize },
    /// Take a mulligan or keep the hand (103.5).
    Mulligan,
    /// Put `count` of these on the bottom of the library, in any order (103.5).
    Bottom { among: Vec<ObjId>, count: usize },
    /// Declare attackers (508.1): each creature that could attack.
    Attackers(Vec<Attacker>),
    /// Declare blockers (509.1). `able`: each creature that could block, and the attacking
    /// creatures it could block, alone or as one of as many creatures as `together` says.
    /// `together`: the attacking creatures that only several creatures at once can block,
    /// and how many it takes (509.1b: "can't be blocked except by two or more creatures").
    Blockers { able: Vec<(ObjId, Vec<ObjId>)>, together: Vec<(ObjId, usize)>, capacity: Vec<(ObjId, usize)> },
    /// Divide a creature's combat damage (510.1c). `lethal`: for each of `among`, the least of
    /// that damage that would be lethal to it (702.19b); None for what it attacks and for a
    /// creature all of it would not be lethal to.
    Divide { attacker: ObjId, among: Vec<Target>, amount: u32, lethal: Vec<Option<u32>> },
    /// "You may …": yes or no (608.2d).
    May,
    /// One optional object action: accept with the selected objects, or decline.
    OptionalObjects { verb: Verb, to: Option<crate::state::Placement>, among: Vec<ObjId>, count: usize },
    /// Which `count` of these to do the action to (608.2d).
    /// A selection constrained by a property of the whole chosen group (701.59a).
    ConstrainedObjects { verb: Verb, among: Vec<ObjId>, selection: crate::cost::Selection },
    Objects { verb: Verb, to: Option<crate::state::Placement>, among: Vec<ObjId>, count: usize },
    /// Order every incoming card, top first, before the simultaneous movement commits.
    OrderCards { to: crate::state::Placement, cards: Vec<ObjId> },
    /// Which of these to add: the mana a resolving spell or ability offers a choice of
    /// (608.2d).
    Mana { options: Vec<Vec<ManaSymbol>> },
    /// One typed value from this rules-defined domain (105.1, 205.3m).
    Value { kind: crate::binding::Kind, options: Vec<crate::binding::Value> },
    Number { min: u32, max: u32 },
    /// Scry or surveil (701.22a, 701.25a): put each of these cards, the first of them the
    /// top card of the library, back on top in any order or away.
    Arrange { verb: Verb, cards: Vec<ObjId> },
    /// One of these players: who does what the effect has one of them do.
    Player { among: Vec<PlayerId> },
    /// One of the piles cards were separated into (700.3), by its place here.
    Pile { piles: Vec<Vec<ObjId>> },
    /// A replacement for the proposed event (616.1). Optional substitutions can be declined;
    /// mandatory ordering cannot. The affected target may be an object or a player.
    /// Order prevention and modifiers; shields have no per-hit identity (615.7, 616.1).
    DamageOrder { on: Target, hits: Vec<crate::operation::ProposedHit>, among: Vec<crate::operation::DamageOption> },
    /// Mandatory bounded allocation of one finite shield over eligible simultaneous hits.
    Prevention { on: Target, shield: crate::operation::ReplacementId, total: u32, hits: Vec<crate::operation::ProposedHit> },
    DamageReplacement { on: Target, hits: Vec<crate::operation::ProposedHit>, among: Vec<crate::operation::DamageOffer> },
    Replacement { on: Target, kind: String, count: u32, counters: Option<crate::operation::CounterProposal>, tokens:Option<crate::operation::TokenProposal>, among: Vec<crate::operation::ReplacementId>, optional: bool },
}

impl Game {
    /// What is chosen for one "target" of a text: every object and player its description
    /// names now.
    pub(crate) fn slot_with(&self, ctx: &Ctx, spec: &crate::announcement::TargetSpec, locals: &dyn crate::program::expr::Locals) -> Slot {
        let among = spec.set.targets(self, ctx, locals);
        let (min, max) = spec.limits(self, ctx);
        Slot { max: max.min(among.len() as u32), min, among, distinct_from: spec.distinct_from.clone(), dependent:None, same_graveyard:None, controller_pairs:None, different_mana_values:None, divide: spec.divided_total(self, ctx) }
    }

    pub(crate) fn target_offers(&self,ctx:&Ctx,specs:&[crate::announcement::TargetSpec],locals:&dyn crate::program::expr::Locals)->Vec<Slot> {
        self.target_offers_contexts(&vec![ctx.clone(); specs.len()], specs, locals, None)
    }

    pub(crate) fn target_offers_contexts(&self, contexts: &[Ctx], specs: &[crate::announcement::TargetSpec], locals: &dyn crate::program::expr::Locals, kept: Option<&[Vec<Target>]>) -> Vec<Slot> {
        let mut offers:Vec<Slot>=Vec::new();
        for (index, spec) in specs.iter().enumerate() {
            let ctx = &contexts[index];
            let Some(dependency)=spec.dependency else {
                let mut offer=self.slot_with(ctx,spec,locals);
                if let Some(kept)=kept { for target in &kept[index] { if !offer.among.contains(target) {offer.among.push(*target);} } }
                if spec.relation == Some(crate::announcement::TargetRelation::SameGraveyard) {
                    let mut partitions = Vec::new();
                    let mut illegal = Vec::new();
                    for target in &offer.among {
                        if !spec.set.legal_target(self, ctx, locals, *target) { illegal.push(*target); }
                    }
                    for owner in 0..self.players.len() {
                        let mut partition: Vec<_> = offer.among.iter().copied().filter(|target|
                            matches!(target, Target::Object(o) if self.obj(*o).zone == crate::Zone::Graveyard
                                && self.obj(*o).owner == owner) && !illegal.contains(target)).collect();
                        if !partition.is_empty() { partition.extend(&illegal); partitions.push(partition); }
                    }
                    if partitions.is_empty() && !illegal.is_empty() { partitions.push(illegal); }
                    offer.max = offer.max.min(partitions.iter().map(|p| p.len() as u32).max().unwrap_or(0));
                    offer.same_graveyard = Some(partitions);
                }
                if spec.relation == Some(crate::announcement::TargetRelation::DifferentCreatureControllers) {
                    let mut pairs = Vec::new();
                    for (i, a) in offer.among.iter().enumerate() {
                        for b in &offer.among[i+1..] {
                            if spec.group_legal(self, ctx, locals, &[*a, *b]) { pairs.push([*a, *b]); }
                        }
                    }
                    if pairs.is_empty() { offer.max = 0; }
                    offer.controller_pairs = Some(pairs);
                }
                if spec.relation == Some(crate::announcement::TargetRelation::DifferentManaValues) {
                    let mut partitions = std::collections::BTreeMap::<u32,Vec<Target>>::new();
                    let mut illegal = 0;
                    for target in &offer.among {
                        if !spec.set.legal_target(self, ctx, locals, *target) { illegal += 1; continue; }
                        if let Target::Object(object) = target { partitions.entry(ctx.mana_value(self, *object)).or_default().push(*target); }
                    }
                    offer.max = offer.max.min(partitions.len() as u32 + illegal);
                    offer.different_mana_values = Some(partitions.into_values().collect());
                }
                offers.push(offer);continue
            };
            let group=spec.base+dependency;
            let mut options=Vec::new();let mut among=Vec::new();
            for prior in &offers[group].among {
                let mut chosen=vec![Vec::new();specs.len()];chosen[group]=vec![*prior];
                let values=crate::announcement::TargetValues{parent:locals,chosen:&chosen,base:spec.base};
                let mut candidates=spec.set.targets(self,ctx,&values);
                if let Some(kept)=kept { for target in &kept[index] { if !candidates.contains(target) {candidates.push(*target);} } }
                for target in &candidates {if !among.contains(target){among.push(*target);}}
                options.push((*prior,candidates));
            }
            let (min,max)=spec.limits(self,ctx);
            offers.push(Slot{max:max.min(among.len() as u32),min,among,distinct_from:spec.distinct_from.clone(),dependent:Some(DependentTargets{group,options}),same_graveyard:None, controller_pairs:None, different_mana_values:None,divide:spec.divided_total(self,ctx)});
        }
        offers
    }

    /// Are there enough legal targets for every "target" of the text (601.2c)?
    fn blocked_by(&self, attacker: ObjId, blockers: &[ObjId]) -> Option<(usize, Vec<ObjId>)> {
        (1..=3).find_map(|count| {
            let mut among = Vec::new();
            for set in combinations(blockers, count) {
                if set.iter().all(|b| !self.cant_block(*b, attacker, &set)) {
                    among.extend(set.into_iter().filter(|b| !among.contains(b)).collect::<Vec<_>>());
                }
            }
            (!among.is_empty()).then_some((count, among))
        })
    }

    /// Whose move it is and what is asked of them; None when the game is over.
    pub fn asked(&mut self) -> Option<(PlayerId, Asked)> {
        if self.finished() {
            return None;
        }
        Some(match self.decision.clone() {
            None => (self.priority, Asked::Priority),
            Some(Decision::Targets { player, ability, modes, .. }) => {
                let ctx = ability.context();
                let specs=self.ability_slots(&ability,&modes);
                let slots = self.target_offers(&ctx,&specs,&ability.bound);
                (player, Asked::Targets { source: ability.source, slots })
            }
            Some(Decision::Modes { player, ability }) => {
                let ctx = ability.context();
                let options = self
                    .trigger_modes(&ability)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|modes| {
                        let specs=self.ability_slots(&ability,&modes);
                        let slots = self.target_offers(&ctx,&specs,&ability.bound);
                        (modes, slots)
                    })
                    .collect();
                (player, Asked::Modes { source: ability.source, options })
            }
            Some(Decision::Order { player, abilities }) => (player, Asked::Order(abilities.iter().map(|t| (t.source, crate::binding::AbilityKey { origin: t.origin.clone(), index: t.index })).collect())),
            Some(Decision::Discard { player, count }) => (player, Asked::Discard { among: self.players[player].hand.clone(), count }),
            Some(Decision::Mulligan { player }) => (player, Asked::Mulligan),
            Some(Decision::Bottom { player, count }) => (player, Asked::Bottom { among: self.players[player].hand.clone(), count }),
            Some(Decision::Attackers { player }) => {
                let defending = self.attack_recipients();
                let able = self.battlefield.clone().into_iter().filter(|o| self.able_to_attack(*o));
                let attackers = able
                    .map(|creature| Attacker {
                        creature,
                        attacks: defending.iter().copied().filter(|t| !self.cant_attack(creature, *t)).collect(),
                        costs: defending.iter().copied().filter(|t| !self.cant_attack(creature, *t))
                            .map(|t| (t, self.attack_cost(&[(creature, t)]))).collect(),
                        must: self.required_to_attack(creature),
                    })
                    .collect();
                (player, Asked::Attackers(attackers))
            },
            Some(Decision::Blockers { player }) => {
                let attacking: Vec<ObjId> = self.combat.attackers.iter().filter(|(_, t)| self.defending(*t) == player).map(|(a, _)| *a).collect();
                let blockers: Vec<ObjId> = self.battlefield.iter().copied().filter(|b| self.can_block(player, *b).is_ok()).collect();
                let mut able: Vec<(ObjId, Vec<ObjId>)> = blockers.iter().map(|b| (*b, Vec::new())).collect();
                let mut together = Vec::new();
                for attacker in attacking {
                    let Some((count, among)) = self.blocked_by(attacker, &blockers) else { continue };
                    able.iter_mut().filter(|(b, _)| among.contains(b)).for_each(|(_, blocks)| blocks.push(attacker));
                    if count > 1 {
                        together.push((attacker, count));
                    }
                }
                let capacity = able.iter().map(|(b, among)| (*b, self.block_capacity(*b).min(among.len()))).collect();
                (player, Asked::Blockers { able, together, capacity })
            },
            Some(Decision::Divide { player, attacker, among, amount }) => {
                let lethal = among
                    .iter()
                    .map(|t| match t {
                        Target::Object(o) if self.combat.blocking(*o) || self.combat.attacking(*o) => (1..=amount).find(|n| self.lethal(*o, &[attacker], *n)),
                        _ => None,
                    })
                    .collect();
                (player, Asked::Divide { attacker, among, amount, lethal })
            }
            Some(Decision::Payment { player, source, reason }) => (player, Asked::Payment { source, reason }),
            Some(Decision::Choose { player, choice: Choice::Alternative { among, optional } }) => (player, Asked::Alternative { among, optional }),
            Some(Decision::Choose { player, choice: Choice::ChangeTargets { object, original, slots, policy } }) => (player, Asked::ChangeTargets { object, original, slots, policy }),
            Some(Decision::Choose { player, choice: Choice::Retarget { copy, original, slots } }) => (player, Asked::Retarget { copy, original, slots }),
            Some(Decision::Choose { player, choice: Choice::CommandZone { object } }) => (player, Asked::CommandZone { object }),
            Some(Decision::Choose { player, choice: Choice::May { .. } }) => (player, Asked::May),
            Some(Decision::Choose { player, choice: Choice::OptionalObjects { verb, to, among, count } }) => (player, Asked::OptionalObjects { verb, to, among, count }),
            Some(Decision::Choose { player, choice: Choice::Cast { cards } }) => {
                (player, Asked::Cast { cards })
            }
            Some(Decision::Choose { player, choice: Choice::Pay { cost, life, waterbend } }) => (player, Asked::Pay { cost, life, waterbend }),
            Some(Decision::Choose { player, choice: Choice::Search { cards, among, min, max } }) =>
                (player, Asked::Search { cards, among, min, max }),
            Some(Decision::Choose { player, choice: Choice::Select { among, min, max } }) => (player, Asked::Select { among, min, max }),
            Some(Decision::Choose { player, choice: Choice::Holders { verb, among } }) => (player, Asked::Holders { verb, among }),
            Some(Decision::Choose { player, choice: Choice::Attach { aura, among } }) => (player, Asked::Attach { aura, among }),
            Some(Decision::Choose { player, choice: Choice::Defender { among, nth, of } }) => (player, Asked::Defender { among, nth, of }),
            Some(Decision::Choose { player, choice: Choice::ConstrainedObjects { verb, among, selection } }) => (player, Asked::ConstrainedObjects { verb, among, selection }),
            Some(Decision::Choose { player, choice: Choice::Objects { verb, to, among, count } }) => (player, Asked::Objects { verb, to, among, count }),
            Some(Decision::Choose { player, choice: Choice::OrderCards { to, cards } }) => (player, Asked::OrderCards { to, cards }),
            Some(Decision::Choose { player, choice: Choice::Mana { options } }) => (player, Asked::Mana { options }),
            Some(Decision::Choose { player, choice: Choice::LibraryDestination { cards, positions } }) => (player, Asked::LibraryDestination { cards, positions }),
            Some(Decision::Choose { player, choice: Choice::Number { min, max } }) => (player, Asked::Number { min, max }),
            Some(Decision::Choose { player, choice: Choice::CoinCall }) => (player, Asked::CoinCall),
            Some(Decision::Choose { player, choice: Choice::Value(kind) }) => (player, Asked::Value { kind, options: crate::binding::options(kind) }),
            Some(Decision::Choose { player, choice: Choice::Name(names) }) => (player, Asked::Value { kind: crate::binding::Kind::CardName,
                options: crate::binding::options(crate::binding::Domain { kind: crate::binding::Kind::CardName, names }) }),
            Some(Decision::Choose { player, choice: Choice::Arrange { verb, cards } }) => (player, Asked::Arrange { verb, cards }),
            Some(Decision::Choose { player, choice: Choice::Player { among } }) => (player, Asked::Player { among }),
            Some(Decision::Choose { player, choice: Choice::Pile { piles } }) => (player, Asked::Pile { piles }),
            Some(Decision::Choose { player, choice: Choice::SpellDestination{on,among} }) => (player,Asked::SpellDestination{on,among}),
            Some(Decision::Choose { player, choice: Choice::DamageOrder{on,hits,among} }) => (player,Asked::DamageOrder{on,hits,among}),
            Some(Decision::Choose { player, choice: Choice::Prevention{on,shield,total,hits} }) => (player,Asked::Prevention{on,shield,total,hits}),
            Some(Decision::Choose { player, choice: Choice::DamageReplacement { on,hits,among } }) =>
                (player,Asked::DamageReplacement {on,hits,among}),
            Some(Decision::Choose { player, choice: Choice::Replacement { on, kind, count, counters, tokens, among, optional } }) =>
                (player, Asked::Replacement { on, kind, count, counters, tokens, among, optional }),
            Some(Decision::Choose { player, choice: Choice::Skip { what, among } }) => {
                let among = among.iter().filter_map(|id| self.skips.iter().find(|s| s.id == *id).cloned()).collect();
                (player, Asked::Skip { what, among })
            }
        })
    }
}

#[cfg(test)]
mod target_feasibility_tests {
    use super::*;
#[test]
fn r601_2c_joint_selection_backtracks_before_committing_a_broad_first_target() {

    let a = Target::Object(ObjId(1));
    let b = Target::Player(0);
    let slots = vec![
        Slot { among: vec![a, b], min: 1, max: 1, distinct_from: vec![], dependent:None, same_graveyard:None, controller_pairs:None, different_mana_values:None, divide: None },
        Slot { among: vec![a], min: 1, max: 1, distinct_from: vec![0], dependent:None, same_graveyard:None, controller_pairs:None, different_mana_values:None, divide: None },
    ];
    // Uncited (implementation): a unit test of the target_selection search over synthetic slots; it must backtrack to the one assignment the distinct_from constraint allows.
    assert_eq!(target_selection(&slots), Some(vec![vec![b], vec![a]]));
}

}
